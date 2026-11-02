//! Represents an annotation.
//!
//! — signature mapping:
//! * `Map<? extends ExecutableElement, ? extends AnnotationValue>` ->
//!   `Vec<(Rc<dyn ExecutableElement>, Rc<dyn AnnotationValue>)>`.  The upstream
//!   Javadoc requires the order to match the order of the values in the
//!   annotation's source; the upstream concrete implementation is an ordered
//!   map, so the port keeps an ordered pair sequence (registered collection
//!   mapping).
//! * Java interface references -> `Rc<dyn Trait>`.

use super::annotation_value::AnnotationValue;
use super::executable_element::ExecutableElement;
use crate::lang::model::type_::declared_type::DeclaredType;
use std::rc::Rc;

/// Represents an annotation.  An annotation associates a value with
/// each element of an annotation interface.
///
/// <p> Annotations should be compared using the equals
/// method.  There is no guarantee that any particular annotation will
/// always be represented by the same object.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @since 1.6
pub trait AnnotationMirror {
    /// {@return the type of this annotation}
    fn get_annotation_type(&self) -> Rc<dyn DeclaredType>;

    /// Returns the values of this annotation's elements.
    /// This is returned in the form of a map that associates elements
    /// with their corresponding values.
    /// Only those elements with values explicitly present in the
    /// annotation are included, not those that are implicitly assuming
    /// their default values.
    /// The order of the map matches the order in which the
    /// values appear in the annotation's source.
    ///
    /// <p>Note that an annotation mirror of a marker annotation type
    /// will by definition have an empty map.
    ///
    /// <p>To fill in default values, use {@link
    /// javax.lang.model.util.Elements#getElementValuesWithDefaults
    /// getElementValuesWithDefaults}.
    ///
    /// @return the values of this annotation's elements,
    ///          or an empty map if there are none
    fn get_element_values(&self) -> Vec<(Rc<dyn ExecutableElement>, Rc<dyn AnnotationValue>)>;
}
