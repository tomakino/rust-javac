//! Represents a value of an annotation interface element.
//!
//! PORTING_RULES — the Java `Object getValue()` is mapped to the closed
//! `Value` enum below, which lists exactly the value shapes documented by the
//! upstream interface:
//! <ul><li> a wrapper class (such as Integer) for a primitive type
//!     <li> String
//!     <li> TypeMirror
//!     <li> VariableElement (representing an enum constant)
//!     <li> AnnotationMirror
//!     <li> List of AnnotationValue (the elements of an array value)
//! </ul>
//! This is a representation choice, not a simplification: the upstream method
//! returns one of these alternatives (registered contract).
//!
//! Java `toString()` (declared by the upstream interface) is expressed as the
//! `std::fmt::Display` supertrait, like every Java Object toString.

use super::annotation_mirror::AnnotationMirror;
use super::annotation_value_visitor::AnnotationValueVisitor;
use super::variable_element::VariableElement;
use crate::lang::model::type_::type_mirror::TypeMirror;
use javac_strings::JavaString;
use std::rc::Rc;

/// The value of an annotation interface element.
///
/// PORTING_RULES: representation of the Java `Object` returned by
/// `AnnotationValue.getValue`; see the file header.  Rust `PartialEq`/`Debug`
/// cannot be derived because the trait object alternatives have no such
/// bounds (the Java `Object` has no value equality contract either).
#[derive(Clone)]
pub enum Value {
    /// Java: a `Boolean` wrapper value.
    Boolean(bool),
    /// Java: a `Byte` wrapper value.
    Byte(i8),
    /// Java: a `Character` wrapper value (UTF-16 code unit).
    Character(u16),
    /// Java: a `Double` wrapper value.
    Double(f64),
    /// Java: a `Float` wrapper value.
    Float(f32),
    /// Java: an `Integer` wrapper value.
    Integer(i32),
    /// Java: a `Long` wrapper value.
    Long(i64),
    /// Java: a `Short` wrapper value.
    Short(i16),
    /// Java: a String value.
    String(JavaString),
    /// Java: a TypeMirror value.
    Type(Rc<dyn TypeMirror>),
    /// Java: a VariableElement representing an enum constant.
    EnumConstant(Rc<dyn VariableElement>),
    /// Java: an AnnotationMirror value.
    Annotation(Rc<dyn AnnotationMirror>),
    /// Java: a List of AnnotationValue representing an array value.
    Array(Vec<Rc<dyn AnnotationValue>>),
}

/// Borrowing representation of `AnnotationValue.getValue()`.
///
/// PORTING_RULES: the Java visitor callbacks (`SimpleAnnotationValueVisitor6.
/// defaultAction(Object, P)` and the visitXyz methods) pass the unboxed value as
/// an `Object`; `ValueRef` is the borrowing form of that closed set, used so the
/// ported callbacks do not clone the referenced mirrors (representation choice,
/// registered).
#[derive(Clone, Copy)]
pub enum ValueRef<'a> {
    /// Java: a `Boolean` wrapper value.
    Boolean(bool),
    /// Java: a `Byte` wrapper value.
    Byte(i8),
    /// Java: a `Character` wrapper value (UTF-16 code unit).
    Character(u16),
    /// Java: a `Double` wrapper value.
    Double(f64),
    /// Java: a `Float` wrapper value.
    Float(f32),
    /// Java: an `Integer` wrapper value.
    Integer(i32),
    /// Java: a `Long` wrapper value.
    Long(i64),
    /// Java: a `Short` wrapper value.
    Short(i16),
    /// Java: a String value.
    String(&'a JavaString),
    /// Java: a TypeMirror value.
    Type(&'a dyn TypeMirror),
    /// Java: a VariableElement representing an enum constant.
    EnumConstant(&'a dyn VariableElement),
    /// Java: an AnnotationMirror value.
    Annotation(&'a dyn AnnotationMirror),
    /// Java: a List of AnnotationValue representing an array value.
    Array(&'a [Rc<dyn AnnotationValue>]),
}

/// Represents a value of an annotation interface element.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @since 1.6
pub trait AnnotationValue: std::fmt::Display {
    /// {@return the value}
    fn get_value(&self) -> Value;

    /// Applies a visitor to this value.
    ///
    /// @param <R> the return type of the visitor's methods
    /// @param <P> the type of the additional parameter to the visitor's methods
    /// @param v   the visitor operating on this value
    /// @param p   additional parameter to the visitor
    /// @return a visitor-specified result
    ///
    /// PORTING_RULES: `where Self: Sized` keeps AnnotationValue object safe
    /// (the upstream method is generic).
    fn accept<R, P>(&self, v: &mut dyn AnnotationValueVisitor<R, P>, p: P) -> R
    where
        Self: Sized;
}
