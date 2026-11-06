//! IDE analysis engine: transient, per-file analysis plus the query features
//! that need real type information.
//!
//! The resident-type [`crate::workspace::analysis::FileAnalysis`] lives in the
//! workspace module; this module owns its feature implementations and the
//! shared position/type helpers.

pub(crate) mod diagnostics;
pub(crate) mod doc_render;
pub(crate) mod features;
pub(crate) mod host;
pub(crate) mod inspections;
pub(crate) mod jdk_docs;
pub(crate) mod jdk_sources;
pub(crate) mod positions;
pub(crate) mod symbols;
pub(crate) mod signatures;
pub(crate) mod type_names;

pub use crate::workspace::analysis::{AnalysisConfig, AnalysisResult, FileAnalysis};
pub(crate) use positions::*;
pub(crate) use type_names::type_name;

/// Canonical path used as the identity of a source file.
pub(crate) fn canonical(path: &std::path::Path) -> String {
    if let Ok(resolved) = std::fs::canonicalize(path) {
        return resolved.to_string_lossy().into_owned();
    }
    // Unsaved buffers do not exist on disk: make the path absolute so it
    // matches the compiler's SimpleFileObject name.
    if path.is_absolute() {
        return path.to_string_lossy().into_owned();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path).to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}
