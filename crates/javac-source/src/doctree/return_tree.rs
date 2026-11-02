//! A tree node for an {@code @return} block tag.
//!
//! Java interface -> Rust trait; the Java `default boolean
//! isInline()` returns false and is a Rust default method.

use super::block_tag_tree::BlockTagTree;
use super::doc_tree::DocTree;
use super::inline_tag_tree::InlineTagTree;
use std::rc::Rc;

/// A tree node for an {@code @return} block tag.
///
/// <pre>{@code
///    @return description
///    {@return description}
/// }</pre>
///
/// @since 1.8
pub trait ReturnTree: BlockTagTree + InlineTagTree {
    /// Returns whether this instance is an inline tag.
    ///
    /// @return {@code true} if this instance is an inline tag, and {@code false} otherwise
    /// @implSpec this implementation returns {@code false}.
    /// @since 16
    fn is_inline(&self) -> bool {
        false
    }

    /// Returns the description of the return value of a method.
    /// @return the description
    fn get_description(&self) -> Vec<Rc<dyn DocTree>>;
}
