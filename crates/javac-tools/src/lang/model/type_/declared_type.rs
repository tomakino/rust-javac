//! Represents a declared type, either a class type or an interface type.
//!
//! PORTING_RULES: `List<? extends TypeMirror>` -> `Vec<Rc<dyn TypeMirror>>`;
//! Java interface references -> `Rc<dyn Trait>`.

use super::reference_type::ReferenceType;
use super::type_mirror::TypeMirror;
use crate::lang::model::element::Element;
use std::rc::Rc;

/// Represents a declared type, either a class type or an interface type.
/// This includes parameterized types such as java.util.Set&lt;String&gt;
/// as well as raw types.
///
/// <p> While a TypeElement represents a class or interface
/// <i>element</i>, a DeclaredType represents a class
/// or interface <i>type</i>, the latter being a use
/// (or <i>invocation</i>) of the former.
/// See {@link TypeElement} for more on this distinction.
///
/// <p> The supertypes (both class and interface types) of a declared
/// type may be found using the {@link
/// Types#directSupertypes(TypeMirror)} method.  This returns the
/// supertypes with any type arguments substituted in.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see TypeElement
/// @since 1.6
pub trait DeclaredType: ReferenceType {
    /// {@return the element corresponding to this type}
    fn as_element(&self) -> Rc<dyn Element>;

    /// Returns the type of the innermost enclosing instance or a
    /// {@code NoType} of kind {@code NONE} if there is no enclosing
    /// instance.  Only types corresponding to inner classes have an
    /// enclosing instance.
    ///
    /// @return a type mirror for the enclosing type
    /// @jls 8.1.3 Inner Classes and Enclosing Instances
    /// @jls 15.9.2 Determining Enclosing Instances
    fn get_enclosing_type(&self) -> Rc<dyn TypeMirror>;

    /// Returns the actual type arguments of this type.
    /// For a type nested within a parameterized type
    /// (such as Outer&lt;String&gt;.Inner&lt;Number&gt;), only the type
    /// arguments of the innermost type are included.
    ///
    /// @return the actual type arguments of this type, or an empty list
    ///           if none
    fn get_type_arguments(&self) -> Vec<Rc<dyn TypeMirror>>;
}
