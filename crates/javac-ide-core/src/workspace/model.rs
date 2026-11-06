//! Workspace configuration and the source-set model the index is built from.
//!
//! The model is derived from [`crate::analysis::AnalysisConfig`] (plus the
//! Maven discovery the LSP server performs before construction). Any change to
//! the source roots or class path invalidates the whole index.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use super::index::key::FileId;
use super::index::model::{SourceSetKind, WorkspaceFingerprint};

/// One logical group of sources (main/test/generated) with its own class path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSet {
    pub kind: SourceSetKind,
    pub roots: Vec<PathBuf>,
    pub classpath: Vec<PathBuf>,
}

impl SourceSet {
    pub fn new(kind: SourceSetKind, roots: Vec<PathBuf>, classpath: Vec<PathBuf>) -> Self {
        Self {
            kind,
            roots,
            classpath,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceModel {
    pub root: PathBuf,
    pub sources: Vec<SourceSet>,
    pub classpath_hash: u64,
    pub source_level: String,
    pub jdk_home: Option<PathBuf>,
    pub platform_dir: Option<PathBuf>,
    pub generation: u64,
}

impl WorkspaceModel {
    pub fn from_config(config: &crate::analysis::AnalysisConfig) -> Self {
        let root = config
            .source_path
            .first()
            .cloned()
            .unwrap_or_else(|| config.output_dir.clone());
        let sources = if config.source_path.is_empty() {
            Vec::new()
        } else {
            vec![SourceSet::new(
                SourceSetKind::Main,
                config.source_path.clone(),
                config.class_path.clone(),
            )]
        };
        Self {
            root,
            sources,
            classpath_hash: hash_paths(&config.class_path),
            source_level: config.source_level.clone(),
            jdk_home: std::env::var_os("JAVA_HOME").map(PathBuf::from),
            platform_dir: std::env::var_os("RAVAC_PLATFORM_DIR").map(PathBuf::from),
            generation: 0,
        }
    }

    /// All `.java` roots across every source set.
    pub fn source_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        for set in &self.sources {
            for root in &set.roots {
                if !roots.contains(root) {
                    roots.push(root.clone());
                }
            }
        }
        roots
    }

    /// Union of every source set's class path, order-preserving and deduped.
    pub fn class_path(&self) -> Vec<PathBuf> {
        let mut entries = Vec::new();
        for set in &self.sources {
            for entry in &set.classpath {
                if !entries.contains(entry) {
                    entries.push(entry.clone());
                }
            }
        }
        entries
    }

    pub fn source_set_for(&self, path: &std::path::Path) -> Option<SourceSetKind> {
        for set in &self.sources {
            for root in &set.roots {
                if path.starts_with(root) {
                    return Some(set.kind);
                }
            }
        }
        None
    }

    pub fn fingerprint(&self) -> WorkspaceFingerprint {
        WorkspaceFingerprint {
            root: self.root.clone(),
            classpath_hash: self.classpath_hash,
            toolchain_hash: self.toolchain_hash(),
            source_level: self.source_level.clone(),
            jdk_home: self.jdk_home.clone(),
            platform_dir: self.platform_dir.clone(),
        }
    }

    /// Hash of everything that identifies the compiler toolchain.
    pub fn toolchain_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.source_level.hash(&mut hasher);
        self.jdk_home.hash(&mut hasher);
        self.platform_dir.hash(&mut hasher);
        hasher.finish()
    }

    pub fn file_id(&self, position: usize) -> FileId {
        position as FileId
    }
}

pub fn hash_paths(paths: &[PathBuf]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for path in paths {
        path.to_string_lossy().hash(&mut hasher);
    }
    hasher.finish()
}

pub fn hash_text(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}
