//! Core-side provider registry: only the built-in directory-scan fallback.
//!
//! Concrete build systems (Maven, Gradle, ...) live in their own crates and
//! depend on `javac-project-api`, never on this crate. Callers compose the
//! full registry and use [`load_with`].

pub use javac_project_api::provider::*;

use std::path::Path;

use super::fallback::DirScanProvider;
use super::model::ProjectModel;

/// Built-in providers: the directory-scan fallback.
pub fn providers() -> Vec<Box<dyn BuildSystemProvider>> {
    vec![Box::new(DirScanProvider)]
}

/// Load the project model from the built-in registry. Never fails.
pub fn load(root: &Path, ctx: &LoadContext) -> ProjectModel {
    load_with(&providers(), root, ctx)
}
