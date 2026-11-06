//! Maven build system for ravac.
//!
//! This crate implements `javac_ide_core::project::BuildSystemProvider`; the
//! core crate never depends on it. Callers compose the registry explicitly.

pub mod maven;
pub mod xml;

pub use maven::{
    discover_all, read_project, read_project_with_repository, MavenDependency, MavenProject,
    MavenProvider,
};
