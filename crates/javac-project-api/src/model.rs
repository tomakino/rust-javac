//! Build-system-neutral project model.
//!
//! A [`ProjectModel`] is the single shape the workspace and the index consume;
//! build systems only differ in how they fill it in.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use super::provider::BuildSystemId;

/// One logical group of sources (main/test/generated).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceSetKind {
    Main,
    Test,
    Generated,
}

impl SourceSetKind {
    /// Stable on-disk tag. Never reorder without bumping the index schema.
    pub fn tag(self) -> u8 {
        match self {
            Self::Main => 1,
            Self::Test => 2,
            Self::Generated => 3,
        }
    }

    pub fn from_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            1 => Self::Main,
            2 => Self::Test,
            3 => Self::Generated,
            _ => return None,
        })
    }
}

/// One source set inside a module: roots, its compile class path, and output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSourceSet {
    pub kind: SourceSetKind,
    pub roots: Vec<PathBuf>,
    pub classpath: Vec<PathBuf>,
    pub output: Option<PathBuf>,
}

impl BuildSourceSet {
    pub fn new(
        kind: SourceSetKind,
        roots: Vec<PathBuf>,
        classpath: Vec<PathBuf>,
        output: Option<PathBuf>,
    ) -> Self {
        Self {
            kind,
            roots,
            classpath,
            output,
        }
    }
}

/// One build module (a Maven project, a Gradle subproject, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub root: PathBuf,
    pub name: String,
    pub source_sets: Vec<BuildSourceSet>,
    pub jdk_home: Option<PathBuf>,
    pub source_level: Option<String>,
}

/// Everything that identifies the model for cache invalidation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFingerprint {
    pub provider: BuildSystemId,
    /// Files that feed the model (for example poms) with their content hash.
    pub inputs: Vec<(PathBuf, u64)>,
    pub model_hash: u64,
}

impl Default for BuildFingerprint {
    fn default() -> Self {
        Self {
            provider: BuildSystemId::NONE,
            inputs: Vec::new(),
            model_hash: 0,
        }
    }
}

/// The build-system-independent description of a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectModel {
    pub root: PathBuf,
    pub provider: BuildSystemId,
    pub modules: Vec<Module>,
    pub fingerprint: BuildFingerprint,
}

impl ProjectModel {
    /// An empty model used when no provider can produce anything.
    pub fn empty(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            provider: BuildSystemId::NONE,
            modules: Vec::new(),
            fingerprint: BuildFingerprint::default(),
        }
    }

    /// Union of every source set's roots, deduplicated in discovery order.
    pub fn source_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        for module in &self.modules {
            for set in &module.source_sets {
                for root in &set.roots {
                    if !roots.contains(root) {
                        roots.push(root.clone());
                    }
                }
            }
        }
        roots
    }

    /// Union of every source set's class path, deduplicated.
    pub fn class_path(&self) -> Vec<PathBuf> {
        let mut entries = Vec::new();
        for module in &self.modules {
            for set in &module.source_sets {
                for entry in &set.classpath {
                    if !entries.contains(entry) {
                        entries.push(entry.clone());
                    }
                }
            }
        }
        entries
    }

    /// The first module that declares a source level.
    pub fn source_level(&self) -> Option<String> {
        self.modules
            .iter()
            .filter_map(|module| module.source_level.clone())
            .find(|level| !level.is_empty())
    }
}

/// Content hash of a model input file (0 when unreadable).
pub fn hash_file(path: &Path) -> u64 {
    let Ok(bytes) = std::fs::read(path) else {
        return 0;
    };
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

/// Hash of the model shape, used as `BuildFingerprint::model_hash`.
pub fn hash_modules(modules: &[Module]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for module in modules {
        module.root.hash(&mut hasher);
        module.name.hash(&mut hasher);
        module.jdk_home.hash(&mut hasher);
        module.source_level.hash(&mut hasher);
        for set in &module.source_sets {
            set.kind.tag().hash(&mut hasher);
            set.roots.hash(&mut hasher);
            set.classpath.hash(&mut hasher);
            set.output.hash(&mut hasher);
        }
    }
    hasher.finish()
}
