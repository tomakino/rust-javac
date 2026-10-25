// Origin: com/sun/source/doctree/DocRootTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for an {@code @docRoot} inline tag.
///
/// <pre>
///    {&#064;docRoot}
/// </pre>
///
/// @since 1.8
use super::inline_tag_tree::InlineTagTree;

pub trait DocRootTree: InlineTagTree {
}
