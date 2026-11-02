//! A simple visitor of types with default behavior appropriate for source
//! versions RELEASE_9 through RELEASE_14.
//!
//! PORTING_RULES: Java `SimpleTypeVisitor9 extends SimpleTypeVisitor8` adds
//! only constructors; the Rust trait is an empty subtrait that keeps the
//! inherited default behavior.

use super::simple_type_visitor8::SimpleTypeVisitor8;

/// A simple visitor of types with default behavior appropriate for source
/// versions RELEASE_9 through RELEASE_14.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see <a href="SimpleTypeVisitor6.html#note_for_subclasses">
/// <strong>Compatibility note for subclasses</strong></a>
/// @see SimpleTypeVisitor6
/// @see SimpleTypeVisitor7
/// @see SimpleTypeVisitor8
/// @since 9
pub trait SimpleTypeVisitor9<R: Clone, P>: SimpleTypeVisitor8<R, P> {}
