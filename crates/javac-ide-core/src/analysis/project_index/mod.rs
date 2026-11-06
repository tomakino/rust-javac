//! Project-wide index: model, builder, and query surface.

mod builder;
mod model;

pub use builder::{build_project_index, build_workspace_model, IndexBuilder};
pub use model::*;
