use super::expression_tree::ExpressionTree;
use super::pattern_tree::PatternTree;
/// A guard pattern tree.
///
/// @since 17
use std::rc::Rc;

pub trait GuardedPatternTree: PatternTree {
    /// The guarded pattern expression.
    /// @return the guarded pattern
    fn get_pattern(&self) -> Rc<dyn PatternTree>;

    /// The guard expression.
    /// @return the guard expression
    fn get_expression(&self) -> Rc<dyn ExpressionTree>;
}
