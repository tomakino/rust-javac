// Origin: javax/lang/model/util/SimpleElementVisitor8.java @ 982d53c (jdk-17.0.19-ga)

//! A simple visitor of program elements with default behavior appropriate for
//! the RELEASE_8 source version.
//!
//! PORTING_RULES: Java `SimpleElementVisitor8 extends SimpleElementVisitor7`
//! adds only constructors; the Rust trait is an empty subtrait that keeps the
//! inherited default behavior.

use super::simple_element_visitor7::SimpleElementVisitor7;

/// A simple visitor of program elements with default behavior
/// appropriate for the RELEASE_8 source version.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see <a href="SimpleElementVisitor6.html#note_for_subclasses">
/// <strong>Compatibility note for subclasses</strong></a>
/// @see SimpleElementVisitor6
/// @see SimpleElementVisitor7
/// @see SimpleElementVisitor9
/// @see SimpleElementVisitor14
/// @since 1.8
pub trait SimpleElementVisitor8<R: Clone, P>: SimpleElementVisitor7<R, P> {}
