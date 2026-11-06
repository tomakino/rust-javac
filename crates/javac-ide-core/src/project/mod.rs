//! Project models: build-system detection and a build-system-neutral model.
//!
//! The core crate only ships the API and the directory-scan fallback. Build
//! systems live in their own crates and are composed by the caller through
//! [`load_with`].

pub mod external;
pub mod fallback;
pub mod model;
pub mod provider;

pub use fallback::DirScanProvider;
pub use model::{
    BuildFingerprint, BuildSourceSet, Module, ProjectModel, SourceSetKind,
};
pub use provider::{
    load, load_with, providers, BuildSystemId, BuildSystemProvider, CancelFlag, LoadContext,
    LoadError,
};
