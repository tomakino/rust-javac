use super::pattern_tree::PatternTree;
use super::variable_tree::VariableTree;
/// A binding pattern tree
///
/// @since 16
use std::rc::Rc;

pub trait BindingPatternTree: PatternTree {
    /// Returns the binding variable.
    /// @return the binding variable
    fn get_variable(&self) -> Rc<dyn VariableTree>;
}
