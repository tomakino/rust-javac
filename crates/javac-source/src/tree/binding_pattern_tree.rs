// Origin: com/sun/source/tree/BindingPatternTree.java @ 982d53c (jdk-17.0.19-ga)

/// A binding pattern tree
///
/// @since 16
use std::rc::Rc;
use super::pattern_tree::PatternTree;
use super::variable_tree::VariableTree;

pub trait BindingPatternTree: PatternTree {
    /// Returns the binding variable.
    /// @return the binding variable
    fn get_variable(&self) -> Rc<dyn VariableTree>;

}
