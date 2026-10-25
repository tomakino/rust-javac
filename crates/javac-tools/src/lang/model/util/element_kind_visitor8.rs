// Origin: javax/lang/model/util/ElementKindVisitor8.java @ 982d53c (jdk-17.0.19-ga)

//! A visitor of program elements based on their kind with default behavior
//! appropriate for the RELEASE_8 source version.
//!
//! PORTING_RULES: Java `ElementKindVisitor8 extends ElementKindVisitor7` adds
//! only constructors; the Rust trait is an empty subtrait that keeps the
//! inherited default behavior.

use super::element_kind_visitor7::ElementKindVisitor7;

/// A visitor of program elements based on their kind with default behavior
/// appropriate for the RELEASE_8 source version.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see ElementKindVisitor6
/// @see ElementKindVisitor7
/// @see ElementKindVisitor9
/// @see ElementKindVisitor14
/// @since 1.8
pub trait ElementKindVisitor8<R: Clone, P>: ElementKindVisitor7<R, P> {}
