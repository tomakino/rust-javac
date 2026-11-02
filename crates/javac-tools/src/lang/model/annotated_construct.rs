//! Represents a construct that can be annotated.
//!
//! — signature mapping:
//! * `List<? extends AnnotationMirror>` -> `Vec<Rc<dyn AnnotationMirror>>`
//!   (an annotation mirror is a shared Java object reference).
//! * `java.lang.Class<A>` -> the opaque `crate::jdk::Class<A>` placeholder;
//!   `A extends java.lang.annotation.Annotation` -> the opaque
//!   `crate::jdk::Annotation` placeholder trait (annotation processing is out
//!   of scope, docs/PLAN.md §3.1).
//! * Java arrays -> `Vec<A>`.
//! * The two generic methods keep `where Self: Sized` so `AnnotatedConstruct`
//!   stays object-safe: `Element` and `TypeMirror` must be usable as
//!   `Rc<dyn ...>`, exactly like the Java interface references.
//! * `null` returns -> `Option<...>`.

use crate::jdk::{Annotation, Class};
use crate::lang::model::element::AnnotationMirror;
use std::rc::Rc;

/// Represents a construct that can be annotated.
///
/// A construct is either an {@linkplain
/// javax.lang.model.element.Element element} or a {@linkplain
/// javax.lang.model.type.TypeMirror type}.  Annotations on an element
/// are on a <em>declaration</em>, whereas annotations on a type are on
/// a specific <em>use</em> of a type name.
///
/// As defined by <cite>The Java Language Specification</cite>
/// section {@jls 9.7.4}, an annotation on an element is a
/// <em>declaration annotation</em> and an annotation on a type is a
/// <em>type annotation</em>.
///
/// The terms <em>directly present</em>, <em>present</em>,
/// <em>indirectly present</em>, and <em>associated </em> are used
/// throughout this interface to describe precisely which annotations,
/// either declaration annotations or type annotations, are returned by
/// the methods in this interface.
///
/// @since 1.8
/// @jls 9.6 Annotation Interfaces
/// @jls 9.6.4.3 {@code @Inherited}
/// @jls 9.7.4 Where Annotations May Appear
/// @jls 9.7.5 Multiple Annotations of the Same Interface
pub trait AnnotatedConstruct {
    /// Returns the annotations that are <em>directly present</em> on
    /// this construct.
    ///
    /// @return the annotations <em>directly present</em> on this
    /// construct; an empty list if there are none
    fn get_annotation_mirrors(&self) -> Vec<Rc<dyn AnnotationMirror>>;

    /// {@return this construct's annotation of the specified type if
    /// such an annotation is <em>present</em>, else null}
    ///
    /// <p> The annotation returned by this method could contain an element
    /// whose value is of type Class.
    /// This value cannot be returned directly:  information necessary to
    /// locate and load a class (such as the class loader to use) is
    /// not available, and the class might not be loadable at all.
    /// Attempting to read a Class object by invoking the relevant
    /// method on the returned annotation
    /// will result in a {@link MirroredTypeException},
    /// from which the corresponding {@link TypeMirror} may be extracted.
    /// Similarly, attempting to read a Class[]-valued element
    /// will result in a {@link MirroredTypesException}.
    ///
    /// @param <A>  the annotation type
    /// @param annotationType  the Class object corresponding to
    ///          the annotation type
    ///
    /// @see #getAnnotationMirrors()
    /// @jls 9.6.1 Annotation Interface Elements
    fn get_annotation<A: Annotation>(&self, annotation_type: Class<A>) -> Option<A>
    where
        Self: Sized;

    /// Returns annotations that are <em>associated</em> with this construct.
    ///
    /// If there are no annotations associated with this construct, the
    /// return value is an array of length 0.
    ///
    /// @param <A>  the annotation type
    /// @param annotationType  the Class object corresponding to
    ///          the annotation type
    /// @return this construct's annotations for the specified annotation
    ///         type if present on this construct, else an empty array
    ///
    /// @see #getAnnotationMirrors()
    /// @see #getAnnotation(Class)
    /// @jls 9.6 Annotation Interfaces
    /// @jls 9.6.1 Annotation Interface Elements
    fn get_annotations_by_type<A: Annotation>(&self, annotation_type: Class<A>) -> Vec<A>
    where
        Self: Sized;
}
