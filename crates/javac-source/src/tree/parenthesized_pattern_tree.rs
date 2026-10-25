// Origin: com/sun/source/tree/ParenthesizedPatternTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a parenthesized pattern.
///
/// For example:
/// <pre>
///   ( <em>pattern</em> )
/// </pre>
///
/// @jls 14.30.1 Kinds of Patterns
///
/// @since 17
use std::rc::Rc;
use super::pattern_tree::PatternTree;

pub trait ParenthesizedPatternTree: PatternTree {
    /// Returns the pattern within the parentheses.
    /// @return the pattern
    fn get_pattern(&self) -> Rc<dyn PatternTree>;

}
