//! A skeletal visitor for annotation values with default behavior appropriate
//! for the RELEASE_7 source version.
//!
//! PORTING_RULES: Java abstract class `AbstractAnnotationValueVisitor7
//! extends AbstractAnnotationValueVisitor6` adds only constructors; the Rust
//! trait is an empty subtrait that keeps the inherited default behavior.
//! Implementations implement each level of the Java class hierarchy explicitly
//! (registered language-shape limitation).

use super::abstract_annotation_value_visitor6::AbstractAnnotationValueVisitor6;

/// A skeletal visitor for annotation values with default behavior
/// appropriate for the RELEASE_7 source version.
///
/// @param <R> the return type of this visitor's methods
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see <a href="AbstractAnnotationValueVisitor6.html#note_for_subclasses">
/// <strong>Compatibility note for subclasses</strong></a>
/// @see AbstractAnnotationValueVisitor6
/// @since 1.7
pub trait AbstractAnnotationValueVisitor7<R, P>: AbstractAnnotationValueVisitor6<R, P> {}
