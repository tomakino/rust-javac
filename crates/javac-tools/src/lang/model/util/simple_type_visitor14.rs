//! A simple visitor of types with default behavior appropriate for the
//! RELEASE_14 source version.
//!
//! Java `SimpleTypeVisitor14 extends SimpleTypeVisitor9` adds
//! only constructors; the Rust trait is an empty subtrait that keeps the
//! inherited default behavior.

use super::simple_type_visitor9::SimpleTypeVisitor9;

/// A simple visitor of types with default behavior appropriate for the
/// RELEASE_14 source version.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see <a href="SimpleTypeVisitor6.html#note_for_subclasses">
/// <strong>Compatibility note for subclasses</strong></a>
/// @see SimpleTypeVisitor9
/// @since 16
pub trait SimpleTypeVisitor14<R: Clone, P>: SimpleTypeVisitor9<R, P> {}
