//! A tree node for a member reference expression.
//!
//! Java interface -> Rust trait; the nested enum ReferenceMode
//! lives in this file; `Name getName()` -> Rc<dyn Name>.

use super::expression_tree::ExpressionTree;
use javac_strings::JavaString;
use javac_tools::lang::model::element::Name;
use std::rc::Rc;

/// A tree node for a member reference expression.
///
/// For example:
/// <pre>
///   <em>expression</em> # <em>[ identifier | new ]</em>
/// </pre>
///
/// @since 1.8
pub trait MemberReferenceTree: ExpressionTree {
    /// Returns the mode of the reference.
    /// @return the mode
    fn get_mode(&self) -> ReferenceMode;

    /// Returns the qualifier expression for the reference.
    /// @return the qualifier expression
    fn get_qualifier_expression(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the name of the reference.
    /// @return the name
    fn get_name(&self) -> Rc<dyn Name>;

    /// Returns the type arguments for the reference.
    /// @return the type arguments
    fn get_type_arguments(&self) -> Vec<Rc<dyn ExpressionTree>>;
}

/// There are two kinds of member references: (i) method references and
/// (ii) constructor references
///
/// Java nested enum MemberReferenceTree.ReferenceMode ->
/// member_reference_tree::ReferenceMode.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceMode {
    /// enum constant for method references.
    INVOKE,
    /// enum constant for constructor references.
    NEW,
}

impl ReferenceMode {
    /// Java: enum#name() (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            ReferenceMode::INVOKE => "INVOKE",
            ReferenceMode::NEW => "NEW",
        })
    }

    /// Java: enum#values().
    pub fn values() -> [ReferenceMode; 2] {
        [ReferenceMode::INVOKE, ReferenceMode::NEW]
    }
}

impl std::fmt::Display for ReferenceMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}
