// Origin: javax/lang/model/util/SimpleElementVisitor9.java @ 982d53c (jdk-17.0.19-ga)

//! A simple visitor of program elements with default behavior appropriate for
//! source versions RELEASE_9 through RELEASE_14.
//!
//! PORTING_RULES: Java `SimpleElementVisitor9 extends SimpleElementVisitor8`
//! adds only constructors; the Rust trait is an empty subtrait that keeps the
//! inherited default behavior.

use super::simple_element_visitor8::SimpleElementVisitor8;

/// A simple visitor of program elements with default behavior
/// appropriate for source versions RELEASE_9 through RELEASE_14.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see <a href="SimpleElementVisitor6.html#note_for_subclasses">
/// <strong>Compatibility note for subclasses</strong></a>
/// @see SimpleElementVisitor6
/// @see SimpleElementVisitor7
/// @see SimpleElementVisitor8
/// @since 9
pub trait SimpleElementVisitor9<R: Clone, P>: SimpleElementVisitor8<R, P> {}
