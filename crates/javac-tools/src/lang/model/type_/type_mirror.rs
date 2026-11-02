//! Represents a type in the Java programming language.
//!
//! PORTING_RULES:
//! * `TypeMirror extends AnnotatedConstruct` -> Rust supertraits.  Java
//!   declares `toString()` and re-declares the AnnotatedConstruct methods; the
//!   former is the `std::fmt::Display` supertrait, the latter are inherited and
//!   not repeated.
//! * `equals(Object)`/`hashCode()` are Object methods, carried by the
//!   `Rc<dyn TypeMirror>` references in the port (registered difference);
//!   semantic comparison belongs to `javax.lang.model.util.Types`, which is
//!   not ported yet.
//! * the generic `<R, P> R accept(TypeVisitor<R, P>, P)` keeps
//!   `where Self: Sized` so TypeMirror stays object safe (`dyn TypeMirror`
//!   cannot call accept — language shape limitation, registered).

use super::array_type::ArrayType;
use super::declared_type::DeclaredType;
use super::error_type::ErrorType;
use super::executable_type::ExecutableType;
use super::intersection_type::IntersectionType;
use super::no_type::NoType;
use super::null_type::NullType;
use super::primitive_type::PrimitiveType;
use super::type_kind::TypeKind;
use super::type_variable::TypeVariable;
use super::type_visitor::TypeVisitor;
use super::union_type::UnionType;
use super::wildcard_type::WildcardType;
use crate::lang::model::AnnotatedConstruct;

/// Represents a type in the Java programming language.
/// Types include primitive types, declared types (class and interface types),
/// array types, type variables, and the null type.
/// Also represented are wildcard type arguments, the signature and
/// return types of executables, and pseudo-types corresponding to
/// packages, modules, and the keyword void.
///
/// <p> Types should be compared using the utility methods in {@link
/// Types}.  There is no guarantee that any particular type will always
/// be represented by the same object.
///
/// <p> To implement operations based on the class of a TypeMirror object,
/// either use a visitor or use the result of the getKind method.
/// Using instanceof is <em>not</em> necessarily a reliable idiom for
/// determining the effective class of an object in this modeling
/// hierarchy since an implementation may choose to have a single
/// object implement multiple TypeMirror subinterfaces.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see Element
/// @see Types
/// @jls 4.1 The Kinds of Types and Values
/// @jls 4.2 Primitive Types and Values
/// @jls 4.3 Reference Types and Values
/// @jls 4.4 Type Variables
/// @jls 4.5 Parameterized Types
/// @jls 4.8 Raw Types
/// @jls 4.9 Intersection Types
/// @jls 10.1 Array Types
/// @since 1.6
pub trait TypeMirror: AnnotatedConstruct + std::fmt::Display {
    /// {@return the kind of this type}
    ///
    /// <ul>
    ///
    /// <li> The kind of a primitive type is
    /// one of the kinds for which {@link TypeKind#isPrimitive} returns
    /// true.
    ///
    /// <li> The kind of a null type is NULL.
    ///
    /// <li> The kind of an array type is ARRAY.
    ///
    /// <li> The kind of a declared type is DECLARED.
    ///
    /// <li> The kind of an error type is ERROR.
    ///
    /// <li> The kind of a type variable is TYPEVAR.
    ///
    /// <li> The kind of a wildcard type is WILDCARD.
    ///
    /// <li> The kind of an executable type is EXECUTABLE.
    ///
    /// <li> The kind of a pseudo-type is one
    /// of VOID, PACKAGE, MODULE, or NONE.
    ///
    /// <li> The kind of a union type is UNION.
    ///
    /// <li> The kind of an intersection type is INTERSECTION.
    ///
    /// </ul>
    fn get_kind(&self) -> TypeKind;

    /// Applies a visitor to this type.
    ///
    /// @param <R> the return type of the visitor's methods
    /// @param <P> the type of the additional parameter to the visitor's methods
    /// @param v   the visitor operating on this type
    /// @param p   additional parameter to the visitor
    /// @return a visitor-specified result
    ///
    /// PORTING_RULES: `where Self: Sized` keeps TypeMirror object safe.
    fn accept<R, P>(&self, v: &mut dyn TypeVisitor<R, P>, p: P) -> R
    where
        Self: Sized;

    // PORTING_RULES: Object conversion contract of the visitor adapter.  Java
    // visitors dispatch through `TypeMirror.accept`, which is generic and cannot
    // be called on `&dyn TypeMirror`; the Rust visitor layer instead dispatches
    // from getKind() plus these object-safe accessors (the same information the
    // Java dispatch uses: the Java type kind determines the subinterface).
    // Default None; concrete implementations override the accessor for their
    // kind (registered contract, like Element::as_xxx).
    fn as_primitive_type(&self) -> Option<&dyn PrimitiveType> {
        None
    }

    fn as_null_type(&self) -> Option<&dyn NullType> {
        None
    }

    fn as_array_type(&self) -> Option<&dyn ArrayType> {
        None
    }

    fn as_declared_type(&self) -> Option<&dyn DeclaredType> {
        None
    }

    fn as_error_type(&self) -> Option<&dyn ErrorType> {
        None
    }

    fn as_type_variable(&self) -> Option<&dyn TypeVariable> {
        None
    }

    fn as_wildcard_type(&self) -> Option<&dyn WildcardType> {
        None
    }

    fn as_executable_type(&self) -> Option<&dyn ExecutableType> {
        None
    }

    fn as_no_type(&self) -> Option<&dyn NoType> {
        None
    }

    fn as_union_type(&self) -> Option<&dyn UnionType> {
        None
    }

    fn as_intersection_type(&self) -> Option<&dyn IntersectionType> {
        None
    }
}
