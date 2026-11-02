// PORTING_RULES: the Java class Tree lives in the package tree; the Rust module
// tree::tree mirrors that (clippy::module_inception is expected).
#![allow(clippy::module_inception)]


//! Provides interfaces to represent source code as abstract syntax
//! trees (AST).
//!
//! @author Peter von der Ah&eacute;
//! @author Jonathan Gibbons
//! @since 1.6
//!
//! PORTING_RULES: this module holds one Rust file per Java file of the package.

pub mod annotated_type_tree;
pub mod annotation_tree;
pub mod array_access_tree;
pub mod array_type_tree;
pub mod assert_tree;
pub mod assignment_tree;
pub mod binary_tree;
pub mod binding_pattern_tree;
pub mod block_tree;
pub mod break_tree;
pub mod case_label_tree;
pub mod case_tree;
pub mod catch_tree;
pub mod class_tree;
pub mod compilation_unit_tree;
pub mod compound_assignment_tree;
pub mod conditional_expression_tree;
pub mod continue_tree;
pub mod default_case_label_tree;
pub mod directive_tree;
pub mod do_while_loop_tree;
pub mod empty_statement_tree;
pub mod enhanced_for_loop_tree;
pub mod erroneous_tree;
pub mod exports_tree;
pub mod expression_statement_tree;
pub mod expression_tree;
pub mod for_loop_tree;
pub mod guarded_pattern_tree;
pub mod identifier_tree;
pub mod if_tree;
pub mod import_tree;
pub mod instance_of_tree;
pub mod intersection_type_tree;
pub mod labeled_statement_tree;
pub mod lambda_expression_tree;
pub mod line_map;
pub mod literal_tree;
pub mod member_reference_tree;
pub mod member_select_tree;
pub mod method_invocation_tree;
pub mod method_tree;
pub mod modifiers_tree;
pub mod module_tree;
pub mod new_array_tree;
pub mod new_class_tree;
pub mod opens_tree;
pub mod package_tree;
pub mod parameterized_type_tree;
pub mod parenthesized_pattern_tree;
pub mod parenthesized_tree;
pub mod pattern_tree;
pub mod primitive_type_tree;
pub mod provides_tree;
pub mod requires_tree;
pub mod return_tree;
pub mod scope;
pub mod statement_tree;
pub mod switch_expression_tree;
pub mod switch_tree;
pub mod synchronized_tree;
pub mod throw_tree;
pub mod tree;
pub mod tree_visitor;
pub mod try_tree;
pub mod type_cast_tree;
pub mod type_parameter_tree;
pub mod unary_tree;
pub mod union_type_tree;
pub mod uses_tree;
pub mod variable_tree;
pub mod while_loop_tree;
pub mod wildcard_tree;
pub mod yield_tree;
