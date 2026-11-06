//! Re-export of the build-system-neutral project model.
//!
//! The model lives in `javac-project-api` so build-system crates can depend on
//! the API without depending on the compiler core.

pub use javac_project_api::model::*;
