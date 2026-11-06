//! Compiler-independent project index.
//!
//! - `key`   interned ids and symbol keys
//! - `model` immutable snapshot records
//! - `build` batch compiler extraction
//! - `store` `RwLock<Arc<ProjectIndex>>` publication
//! - `query` definition/references/outline/workspace-symbol reads
//! - `update` single-file incremental classification
//! - `dump`  versioned binary persistence

pub mod build;
pub mod dump;
pub mod extract;
pub mod key;
pub mod model;
pub mod query;
pub mod store;
pub mod update;

mod symbols;

pub use build::{
    build, collect_java_files, IndexBuildResult, IndexBuilder, IndexLimits, DEFAULT_BATCH_SIZE,
    DEFAULT_MAX_INDEXED_FILES, DEFAULT_MAX_OCCURRENCE_BYTES,
};
pub use key::{
    FileId, IndexSymbolKind, NameId, SignatureId, StringInterner, SymbolId, SymbolKey,
};
pub use model::*;
pub use store::IndexStore;
