// Origin: com/sun/source/doctree/InheritDocTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for an {@code @inheritDoc} inline tag.
///
/// <pre>
///    {&#064;inheritDoc}
/// </pre>
///
/// @since 1.8
use super::inline_tag_tree::InlineTagTree;

pub trait InheritDocTree: InlineTagTree {
}
