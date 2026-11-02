//! A skeletal visitor of types with default behavior appropriate for the
//! RELEASE_8 source version.
//!
//! PORTING_RULES: Java abstract class `AbstractTypeVisitor8 extends
//! AbstractTypeVisitor7` adds only constructors; in addition it makes visitIntersection abstract (Rust cannot cancel a default method in a subtrait, registered difference); the Rust trait is an
//! empty subtrait of the previous level, which keeps the same default
//! behavior.  Implementations implement each level of the Java class hierarchy
//! explicitly (registered language-shape limitation).

use super::abstract_type_visitor7::AbstractTypeVisitor7;

/// A skeletal visitor of types with default behavior appropriate for the
/// RELEASE_8 source version.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see <a href="AbstractTypeVisitor6.html#note_for_subclasses">
/// <strong>Compatibility note for subclasses</strong></a>
/// @see AbstractTypeVisitor6
/// @since 1.8
pub trait AbstractTypeVisitor8<R, P>: AbstractTypeVisitor7<R, P> {}
