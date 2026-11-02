//! A skeletal visitor of types with default behavior appropriate for the
//! RELEASE_14 source version.
//!
//! PORTING_RULES: Java abstract class `AbstractTypeVisitor14 extends
//! AbstractTypeVisitor9` adds only constructors; the Rust trait is an
//! empty subtrait of the previous level, which keeps the same default
//! behavior.  Implementations implement each level of the Java class hierarchy
//! explicitly (registered language-shape limitation).

use super::abstract_type_visitor9::AbstractTypeVisitor9;

/// A skeletal visitor of types with default behavior appropriate for the
/// RELEASE_14 source version.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see <a href="AbstractTypeVisitor6.html#note_for_subclasses">
/// <strong>Compatibility note for subclasses</strong></a>
/// @see AbstractTypeVisitor6
/// @since 16
pub trait AbstractTypeVisitor14<R, P>: AbstractTypeVisitor9<R, P> {}
