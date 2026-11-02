//! A skeletal visitor of types with default behavior appropriate for the
//! RELEASE_7 source version.
//!
//! Java abstract class `AbstractTypeVisitor7 extends
//! AbstractTypeVisitor6` adds only constructors; in addition it makes visitUnion abstract (Rust cannot cancel a default method in a subtrait, registered difference); the Rust trait is an
//! empty subtrait of the previous level, which keeps the same default
//! behavior.  Implementations implement each level of the Java class hierarchy
//! explicitly (registered language-shape limitation).

use super::abstract_type_visitor6::AbstractTypeVisitor6;

/// A skeletal visitor of types with default behavior appropriate for the
/// RELEASE_7 source version.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see <a href="AbstractTypeVisitor6.html#note_for_subclasses">
/// <strong>Compatibility note for subclasses</strong></a>
/// @see AbstractTypeVisitor6
/// @since 1.7
pub trait AbstractTypeVisitor7<R, P>: AbstractTypeVisitor6<R, P> {}
