//! Workspace module root.
//!
//! The implementation lives in sibling files:
//!
//! - `analysis`: transient per-file [`analysis::FileAnalysis`]
//! - `model`:    source sets and the workspace fingerprint
//! - `index`:    the compiler-independent project index
//! - `state`:    workspace state and lifecycle
//! - `service`:  the [`javac_analysis_api::WorkspaceService`] implementation
//! - `cache`:    bounded LRU of live per-file analyses

pub mod analysis;
pub mod index;
pub mod model;

mod cache;
mod service;
mod state;

pub use analysis::AnalysisConfig as WorkspaceConfig;
pub use state::Workspace;
