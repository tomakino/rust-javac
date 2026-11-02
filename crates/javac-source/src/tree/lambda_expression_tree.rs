//! A tree node for a lambda expression.
//!
//! PORTING_RULES: Java interface -> Rust trait; the nested enum BodyKind lives
//! in this file.

use super::expression_tree::ExpressionTree;
use super::tree::Tree;
use super::variable_tree::VariableTree;
use javac_strings::JavaString;
use std::rc::Rc;

/// A tree node for a lambda expression.
///
/// For example:
/// <pre>{@code
///   ()->{}
///   (List<String> ls)->ls.size()
///   (x,y)-> { return x + y; }
/// }</pre>
pub trait LambdaExpressionTree: ExpressionTree {
    /// Returns the parameters of this lambda expression.
    /// @return the parameters
    fn get_parameters(&self) -> Vec<Rc<dyn VariableTree>>;

    /// Returns the body of the lambda expression.
    /// @return the body
    fn get_body(&self) -> Rc<dyn Tree>;

    /// Returns the kind of the body of the lambda expression.
    /// @return the kind of the body
    fn get_body_kind(&self) -> BodyKind;
}

/// Lambda expressions come in two forms:
/// <ul>
/// <li> expression lambdas, whose body is an expression, and
/// <li> statement lambdas, whose body is a block
/// </ul>
///
/// PORTING_RULES: Java nested enum LambdaExpressionTree.BodyKind ->
/// lambda_expression_tree::BodyKind.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyKind {
    /// enum constant for expression lambdas
    EXPRESSION,
    /// enum constant for statement lambdas
    STATEMENT,
}

impl BodyKind {
    /// Java: enum#name() (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            BodyKind::EXPRESSION => "EXPRESSION",
            BodyKind::STATEMENT => "STATEMENT",
        })
    }

    /// Java: enum#values().
    pub fn values() -> [BodyKind; 2] {
        [BodyKind::EXPRESSION, BodyKind::STATEMENT]
    }
}

impl std::fmt::Display for BodyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}
