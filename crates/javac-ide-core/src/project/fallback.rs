//! Directory-scan fallback: recognizes the conventional source roots.

use std::path::{Path, PathBuf};

use super::model::{
    hash_modules, BuildFingerprint, BuildSourceSet, Module, ProjectModel, SourceSetKind,
};
use super::provider::{BuildSystemId, BuildSystemProvider, LoadContext, LoadError};

/// Always-detected provider for projects without a known build system.
///
/// It recognizes `src/main/java` and `src/test/java` when they exist and
/// otherwise yields an empty model.
pub struct DirScanProvider;

impl BuildSystemProvider for DirScanProvider {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::NONE
    }

    fn detect(&self, _root: &Path) -> bool {
        true
    }

    fn load(&self, root: &Path, _ctx: &LoadContext) -> Result<ProjectModel, LoadError> {
        let mut source_sets = Vec::new();
        let main = root.join("src").join("main").join("java");
        if main.is_dir() {
            source_sets.push(BuildSourceSet::new(
                SourceSetKind::Main,
                vec![main],
                Vec::new(),
                None,
            ));
        }
        let test = root.join("src").join("test").join("java");
        if test.is_dir() {
            source_sets.push(BuildSourceSet::new(
                SourceSetKind::Test,
                vec![test],
                Vec::new(),
                None,
            ));
        }
        let modules = if source_sets.is_empty() {
            Vec::new()
        } else {
            vec![Module {
                root: root.to_path_buf(),
                name: "project".to_string(),
                source_sets,
                jdk_home: None,
                source_level: None,
            }]
        };
        Ok(ProjectModel {
            root: root.to_path_buf(),
            provider: BuildSystemId::NONE,
            fingerprint: BuildFingerprint {
                provider: BuildSystemId::NONE,
                inputs: Vec::new(),
                model_hash: hash_modules(&modules),
            },
            modules,
        })
    }

    fn watch_paths(&self, _root: &Path) -> Vec<PathBuf> {
        Vec::new()
    }
}
