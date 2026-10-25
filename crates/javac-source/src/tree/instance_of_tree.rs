// Origin: com/sun/source/tree/InstanceOfTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for an {@code instanceof} expression.
///
/// For example:
/// <pre>
///   <em>expression</em> instanceof <em>type</em>
/// </pre>
///
/// @jls 15.20.2 Type Comparison Operator instanceof
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
use std::rc::Rc;
use super::expression_tree::ExpressionTree;
use super::pattern_tree::PatternTree;
use super::tree::Tree;

pub trait InstanceOfTree: ExpressionTree {
    /// Returns the expression to be tested.
    /// @return the expression
    fn get_expression(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the type for which to check.
    /// @return the type
    /// @see #getPattern()
    fn get_type(&self) -> Rc<dyn Tree>;

    /// Returns the tested pattern, or null if this instanceof does not use
    /// a pattern.
    ///
    /// <p>For instanceof with a pattern, i.e. in the following form:
    /// <pre>
    ///   <em>expression</em> instanceof <em>type</em> <em>variable name</em>
    /// </pre>
    /// returns the pattern.
    ///
    /// <p>For instanceof without a pattern, i.e. in the following form:
    /// <pre>
    ///   <em>expression</em> instanceof <em>type</em>
    /// </pre>
    /// returns null.
    ///
    /// @return the tested pattern, or null if this instanceof does not use a pattern
    /// @since 16
    fn get_pattern(&self) -> Option<Rc<dyn PatternTree>>;

}
