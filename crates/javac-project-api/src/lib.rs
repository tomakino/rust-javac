//! Build-system-neutral project API.
//!
//! This crate has no dependency on the compiler or on any build system: build
//! systems implement [`BuildSystemProvider`] in their own crates, and the
//! workspace consumes [`ProjectModel`].

pub mod external;
pub mod model;
pub mod provider;

pub use external::ExternalModelRunner;
pub use model::{
    hash_file, hash_modules, BuildFingerprint, BuildSourceSet, Module, ProjectModel, SourceSetKind,
};
pub use provider::{
    load_with, BuildSystemId, BuildSystemProvider, CancelFlag, LoadContext, LoadError,
};
