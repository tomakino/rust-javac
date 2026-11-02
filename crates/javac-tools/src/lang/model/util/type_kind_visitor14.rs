//! A visitor of types based on their kind with default behavior appropriate for
//! the RELEASE_14 source version.
//!
//! Java `TypeKindVisitor14 extends TypeKindVisitor9` adds only
//! constructors; the Rust trait is an empty subtrait that keeps the inherited
//! default behavior.

use super::type_kind_visitor9::TypeKindVisitor9;

/// A visitor of types based on their kind with default behavior appropriate for
/// the RELEASE_14 source version.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see TypeKindVisitor6
/// @see TypeKindVisitor9
/// @since 16
pub trait TypeKindVisitor14<R: Clone, P>: TypeKindVisitor9<R, P> {}
