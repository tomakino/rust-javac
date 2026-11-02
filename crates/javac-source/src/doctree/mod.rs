// PORTING_RULES: the Java class DocTree lives in the package doctree; the Rust
// module doctree::doc_tree mirrors that (clippy::module_inception is expected).
#![allow(clippy::module_inception)]


//! Provides interfaces to represent documentation comments as abstract syntax
//! trees (AST).
//!
//! PORTING_RULES: one Rust file per Java file of the package.

pub mod attribute_tree;
pub mod author_tree;
pub mod block_tag_tree;
pub mod comment_tree;
pub mod deprecated_tree;
pub mod doc_comment_tree;
pub mod doc_root_tree;
pub mod doc_tree;
pub mod doc_tree_visitor;
pub mod doc_type_tree;
pub mod end_element_tree;
pub mod entity_tree;
pub mod erroneous_tree;
pub mod hidden_tree;
pub mod identifier_tree;
pub mod index_tree;
pub mod inherit_doc_tree;
pub mod inline_tag_tree;
pub mod link_tree;
pub mod literal_tree;
pub mod param_tree;
pub mod provides_tree;
pub mod reference_tree;
pub mod return_tree;
pub mod see_tree;
pub mod serial_data_tree;
pub mod serial_field_tree;
pub mod serial_tree;
pub mod since_tree;
pub mod start_element_tree;
pub mod summary_tree;
pub mod system_property_tree;
pub mod text_tree;
pub mod throws_tree;
pub mod unknown_block_tag_tree;
pub mod unknown_inline_tag_tree;
pub mod uses_tree;
pub mod value_tree;
pub mod version_tree;
