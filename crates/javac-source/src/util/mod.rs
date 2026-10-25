// Origin: com/sun/source/util/package-info.java @ 982d53c (jdk-17.0.19-ga)

//! Provides utilities for operations on abstract syntax trees (AST).
//!
//! @author Peter von der Ah&eacute;
//! @author Jonathan Gibbons
//! @since 1.6
//!
//! PORTING_RULES: one Rust file per Java file of the package; abstract classes
//! (JavacTask/Trees/DocTrees) become traits whose implementations are provided by
//! the P6 API layer, and the visitor/scanner class hierarchy follows §14 as in
//! javax.lang.model.util.  This package is being ported in batches (task board);
//! only completed modules are declared here so the crate always compiles.

pub(crate) mod dispatch;
pub mod doc_source_positions;
pub mod doc_tree_factory;
pub mod doc_tree_scanner;
pub mod doc_tree_path;
pub mod doc_tree_path_scanner;
pub mod doc_trees;
pub mod javac_task;
pub mod parameter_name_provider;
pub mod plugin;
pub mod simple_doc_tree_visitor;
pub mod simple_tree_visitor;
pub mod source_positions;
pub mod task_event;
pub mod task_listener;
pub mod tree_path;
pub mod tree_path_scanner;
pub mod tree_scanner;
pub mod trees;

#[cfg(test)]
pub(crate) mod test_support;
