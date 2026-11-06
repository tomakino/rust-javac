//! Project-wide index: model, builder, and query surface.

mod builder;
mod document_symbols;
mod model;
mod session;
mod symbols;
mod workspace_symbols;

pub use builder::{
    build_project_index, build_project_index_with_overlays, build_workspace_model, IndexBuilder,
};
pub use model::*;
