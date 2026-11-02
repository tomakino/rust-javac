//! A tree node for a literal expression.
//!
//! Java `Object getValue()` returns a boxed primitive value or a
//! String; the port reuses the closed `javac_tools::lang::model::element::Value`
//! representation (the literal sub-set of its variants is used by
//! implementations) and maps null -> None (registered representation).

use super::expression_tree::ExpressionTree;
use javac_tools::lang::model::element::Value;

/// A tree node for a literal expression.
/// Use {@link #getKind getKind} to determine the kind of literal.
///
/// For example:
/// <pre>
///   <em>value</em>
/// </pre>
///
/// @jls 15.28 Constant Expressions
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
pub trait LiteralTree: ExpressionTree {
    /// Returns the value of the literal expression.
    /// The value will be a boxed primitive value, a String, or {@code null}.
    /// @return the value
    fn get_value(&self) -> Option<Value>;
}
