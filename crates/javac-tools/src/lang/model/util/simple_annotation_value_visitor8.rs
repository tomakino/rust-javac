//! A simple visitor for annotation values with default behavior appropriate for
//! the RELEASE_8 source version.
//!
//! Java `SimpleAnnotationValueVisitor8 extends
//! SimpleAnnotationValueVisitor7` adds only constructors; the Rust trait is
//! an empty subtrait that keeps the inherited default behavior.
//! Implementations implement each level of the Java class hierarchy explicitly
//! (registered language-shape limitation).

use super::simple_annotation_value_visitor7::SimpleAnnotationValueVisitor7;

/// A simple visitor for annotation values with default behavior
/// appropriate for the RELEASE_8 source version.
///
/// @param <R> the return type of this visitor's methods
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see <a href="SimpleAnnotationValueVisitor6.html#note_for_subclasses">
/// <strong>Compatibility note for subclasses</strong></a>
/// @see SimpleAnnotationValueVisitor6
/// @since 1.8
pub trait SimpleAnnotationValueVisitor8<R: Clone, P>: SimpleAnnotationValueVisitor7<R, P> {}
