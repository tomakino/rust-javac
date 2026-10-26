// Origin: javax/lang/model/util/Types.java @ 982d53c (jdk-17.0.19-ga)

//! Utility methods for operating on types.
//!
//! PORTING_RULES (§6/§14):
//! * Java interface -> Rust trait.
//! * Java overloads of getDeclaredType -> distinct Rust names
//!   (get_declared_type / get_declared_type_in; §14 overload rule).
//! * `TypeMirror...` -> `&[Rc<dyn TypeMirror>]`; `List<? extends TypeMirror>` ->
//!   `Vec<Rc<dyn TypeMirror>>`; nullable bounds -> `Option<&dyn TypeMirror>`.
//! * Java's IllegalArgumentException checks are runtime behavior of the
//!   implementations (P6), not part of this interface layer.
//!
//! <p><b>Compatibility Note:</b> Methods may be added to this interface
//! in future releases of the platform.
//!
//! @author Joseph D. Darcy
//! @author Scott Seligman
//! @author Peter von der Ah&eacute;
//! @see javax.annotation.processing.ProcessingEnvironment#getTypeUtils
//! @since 1.6

use crate::lang::model::element::{Element, TypeElement};
use crate::lang::model::type_::{
    ArrayType, DeclaredType, ExecutableType, NoType, NullType, PrimitiveType, TypeKind, TypeMirror,
    WildcardType,
};
use std::rc::Rc;

/// Utility methods for operating on types.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @since 1.6
pub trait Types {
    /// Returns the element corresponding to a type.
    /// The type may be a {@code DeclaredType} or {@code TypeVariable}.
    /// Returns {@code null} if the type is not one with a corresponding element.
    ///
    /// @param t the type to map to an element
    /// @return the element corresponding to the given type
    fn as_element(&self, t: &dyn TypeMirror) -> Option<Rc<dyn Element>>;

    /// Tests whether two {@code TypeMirror} objects represent the same type.
    ///
    /// @param t1  the first type
    /// @param t2  the second type
    /// @return {@code true} if and only if the two types are the same
    fn is_same_type(&self, t1: &dyn TypeMirror, t2: &dyn TypeMirror) -> bool;

    /// Tests whether one type is a subtype of another.
    /// Any type is considered to be a subtype of itself.
    ///
    /// @param t1  the first type
    /// @param t2  the second type
    /// @return {@code true} if and only if the first type is a subtype of the second
    /// @throws IllegalArgumentException if given a type for an executable, package, or module
    /// @jls 4.10 Subtyping
    fn is_subtype(&self, t1: &dyn TypeMirror, t2: &dyn TypeMirror) -> bool;

    /// Tests whether one type is assignable to another.
    ///
    /// @param t1  the first type
    /// @param t2  the second type
    /// @return {@code true} if and only if the first type is assignable to the second
    /// @throws IllegalArgumentException if given a type for an executable, package, or module
    /// @jls 5.2 Assignment Contexts
    fn is_assignable(&self, t1: &dyn TypeMirror, t2: &dyn TypeMirror) -> bool;

    /// Tests whether one type argument <i>contains</i> another.
    ///
    /// @param t1  the first type
    /// @param t2  the second type
    /// @return {@code true} if and only if the first type contains the second
    /// @throws IllegalArgumentException if given a type for an executable, package, or module
    /// @jls 4.5.1 Type Arguments of Parameterized Types
    fn contains(&self, t1: &dyn TypeMirror, t2: &dyn TypeMirror) -> bool;

    /// Tests whether the signature of one method is a <i>subsignature</i>
    /// of another.
    ///
    /// @param m1  the first method
    /// @param m2  the second method
    /// @return {@code true} if and only if the first signature is a subsignature of the second
    /// @jls 8.4.2 Method Signature
    fn is_subsignature(&self, m1: &dyn ExecutableType, m2: &dyn ExecutableType) -> bool;

    /// Returns the direct supertypes of a type. The interface types, if any,
    /// will appear last in the list. For an interface type with no direct
    /// super-interfaces, a type mirror representing {@code java.lang.Object}
    /// is returned.
    ///
    /// @param t  the type being examined
    /// @return the direct supertypes, or an empty list if none
    /// @throws IllegalArgumentException if given a type for an executable, package, or module
    /// @jls 4.10 Subtyping
    fn direct_supertypes(&self, t: &dyn TypeMirror) -> Vec<Rc<dyn TypeMirror>>;

    /// Returns the erasure of a type.
    ///
    /// @param t  the type to be erased
    /// @throws IllegalArgumentException if given a type for a package or module
    /// @jls 4.6 Type Erasure
    fn erasure(&self, t: &dyn TypeMirror) -> Rc<dyn TypeMirror>;

    /// Returns the class of a boxed value of the primitive type argument.
    /// That is, <i>boxing conversion</i> is applied.
    ///
    /// @param p  the primitive type to be converted
    /// @jls 5.1.7 Boxing Conversion
    fn boxed_class(&self, p: &dyn PrimitiveType) -> Rc<dyn TypeElement>;

    /// Returns the type (a primitive type) of unboxed values of a given type.
    /// That is, <i>unboxing conversion</i> is applied.
    ///
    /// @param t  the type to be unboxed
    /// @return the type of an unboxed value of type {@code t}
    /// @throws IllegalArgumentException if the given type has no unboxing conversion
    /// @jls 5.1.8 Unboxing Conversion
    fn unboxed_type(&self, t: &dyn TypeMirror) -> Rc<dyn PrimitiveType>;

    /// Applies capture conversion to a type.
    ///
    /// @param t  the type to be converted
    /// @return the result of applying capture conversion
    /// @throws IllegalArgumentException if given a type for an executable, package, or module
    /// @jls 5.1.10 Capture Conversion
    fn capture(&self, t: &dyn TypeMirror) -> Rc<dyn TypeMirror>;

    /// Returns a primitive type.
    ///
    /// @param kind  the kind of primitive type to return
    /// @throws IllegalArgumentException if {@code kind} is not a primitive kind
    fn get_primitive_type(&self, kind: TypeKind) -> Rc<dyn PrimitiveType>;

    /// Returns the null type.  This is the type of {@code null}.
    fn get_null_type(&self) -> Rc<dyn NullType>;

    /// Returns a pseudo-type used where no actual type is appropriate.
    /// The kind of type to return may be either VOID or NONE.
    ///
    /// @param kind  the kind of type to return
    /// @return a pseudo-type of kind VOID or NONE
    /// @throws IllegalArgumentException if {@code kind} is not valid
    fn get_no_type(&self, kind: TypeKind) -> Rc<dyn NoType>;

    /// Returns an array type with the specified component type.
    ///
    /// @param component_type  the component type
    /// @throws IllegalArgumentException if the component type is not valid for an array
    fn get_array_type(&self, component_type: &dyn TypeMirror) -> Rc<dyn ArrayType>;

    /// Returns a new wildcard type.  Either of the wildcard's bounds may be
    /// specified, or neither, but not both.
    ///
    /// @param extends_bound  the extends (upper) bound, or {@code null} if none
    /// @param super_bound    the super (lower) bound, or {@code null} if none
    /// @throws IllegalArgumentException if bounds are not valid
    fn get_wildcard_type(
        &self,
        extends_bound: Option<&dyn TypeMirror>,
        super_bound: Option<&dyn TypeMirror>,
    ) -> Rc<dyn WildcardType>;

    /// Returns the type corresponding to a type element and actual type arguments.
    ///
    /// @param type_elem  the type element
    /// @param type_args  the actual type arguments
    /// @throws IllegalArgumentException if too many or too few
    ///          type arguments are given, or if an inappropriate type
    ///          argument or type element is provided
    fn get_declared_type(
        &self,
        type_elem: &dyn TypeElement,
        type_args: &[Rc<dyn TypeMirror>],
    ) -> Rc<dyn DeclaredType>;

    /// Returns the type corresponding to a type element and actual type
    /// arguments, given a containing type of which it is a member.
    /// (Java: getDeclaredType(DeclaredType, TypeElement, TypeMirror...))
    ///
    /// @param containing  the containing type, or {@code null} if none
    /// @param type_elem    the type element
    /// @param type_args    the actual type arguments
    /// @return the type corresponding to the type element and actual type
    ///          arguments, contained within the given type
    /// @throws IllegalArgumentException if too many or too few
    ///          type arguments are given, or if an inappropriate type
    ///          argument, type element, or containing type is provided
    fn get_declared_type_in(
        &self,
        containing: Option<&dyn DeclaredType>,
        type_elem: &dyn TypeElement,
        type_args: &[Rc<dyn TypeMirror>],
    ) -> Rc<dyn DeclaredType>;

    /// Returns the type of an element when that element is viewed as
    /// a member of, or otherwise directly contained by, a given type.
    ///
    /// @param containing  the containing type
    /// @param element     the element
    /// @return the type of the element as viewed from the containing type
    /// @throws IllegalArgumentException if the element is not a valid one
    ///          for the given type
    fn as_member_of(&self, containing: &dyn DeclaredType, element: &dyn Element) -> Rc<dyn TypeMirror>;
}

#[cfg(test)]
mod tests {
    use super::Types;
    use crate::lang::model::element::{Element, TypeElement};
    use crate::lang::model::type_::{
        ArrayType, DeclaredType, ExecutableType, NoType, NullType, PrimitiveType, TypeKind,
        TypeMirror, WildcardType,
    };
    use crate::lang::model::util::test_support::TypeFixture;
    use std::rc::Rc;

    struct FakeTypes;

    impl Types for FakeTypes {
        fn as_element(&self, _t: &dyn TypeMirror) -> Option<Rc<dyn Element>> {
            None
        }
        fn is_same_type(&self, _t1: &dyn TypeMirror, _t2: &dyn TypeMirror) -> bool {
            false
        }
        fn is_subtype(&self, _t1: &dyn TypeMirror, _t2: &dyn TypeMirror) -> bool {
            false
        }
        fn is_assignable(&self, _t1: &dyn TypeMirror, _t2: &dyn TypeMirror) -> bool {
            false
        }
        fn contains(&self, _t1: &dyn TypeMirror, _t2: &dyn TypeMirror) -> bool {
            false
        }
        fn is_subsignature(&self, _m1: &dyn ExecutableType, _m2: &dyn ExecutableType) -> bool {
            false
        }
        fn direct_supertypes(&self, _t: &dyn TypeMirror) -> Vec<Rc<dyn TypeMirror>> {
            Vec::new()
        }
        fn erasure(&self, _t: &dyn TypeMirror) -> Rc<dyn TypeMirror> {
            panic!("AbstractMethodError: erasure: test fixture")
        }
        fn boxed_class(&self, _p: &dyn PrimitiveType) -> Rc<dyn TypeElement> {
            panic!("AbstractMethodError: boxed_class: test fixture")
        }
        fn unboxed_type(&self, _t: &dyn TypeMirror) -> Rc<dyn PrimitiveType> {
            panic!("AbstractMethodError: unboxed_type: test fixture")
        }
        fn capture(&self, _t: &dyn TypeMirror) -> Rc<dyn TypeMirror> {
            panic!("AbstractMethodError: capture: test fixture")
        }
        fn get_primitive_type(&self, _kind: TypeKind) -> Rc<dyn PrimitiveType> {
            panic!("AbstractMethodError: get_primitive_type: test fixture")
        }
        fn get_null_type(&self) -> Rc<dyn NullType> {
            panic!("AbstractMethodError: get_null_type: test fixture")
        }
        fn get_no_type(&self, _kind: TypeKind) -> Rc<dyn NoType> {
            panic!("AbstractMethodError: get_no_type: test fixture")
        }
        fn get_array_type(&self, _component_type: &dyn TypeMirror) -> Rc<dyn ArrayType> {
            panic!("AbstractMethodError: get_array_type: test fixture")
        }
        fn get_wildcard_type(
            &self,
            _extends_bound: Option<&dyn TypeMirror>,
            _super_bound: Option<&dyn TypeMirror>,
        ) -> Rc<dyn WildcardType> {
            panic!("AbstractMethodError: get_wildcard_type: test fixture")
        }
        fn get_declared_type(
            &self,
            _type_elem: &dyn TypeElement,
            _type_args: &[Rc<dyn TypeMirror>],
        ) -> Rc<dyn DeclaredType> {
            panic!("AbstractMethodError: get_declared_type: test fixture")
        }
        fn get_declared_type_in(
            &self,
            _containing: Option<&dyn DeclaredType>,
            _type_elem: &dyn TypeElement,
            _type_args: &[Rc<dyn TypeMirror>],
        ) -> Rc<dyn DeclaredType> {
            panic!("AbstractMethodError: get_declared_type_in: test fixture")
        }
        fn as_member_of(
            &self,
            _containing: &dyn DeclaredType,
            _element: &dyn Element,
        ) -> Rc<dyn TypeMirror> {
            panic!("AbstractMethodError: as_member_of: test fixture")
        }
    }

    #[test]
    fn types_is_object_safe() {
        let types: &dyn Types = &FakeTypes;
        let int = TypeFixture::new(TypeKind::INT);
        let executable = TypeFixture::new(TypeKind::EXECUTABLE);
        assert!(types.as_element(&int).is_none());
        assert!(!types.is_same_type(&int, &int));
        assert!(!types.is_subtype(&int, &int));
        assert!(!types.is_assignable(&int, &int));
        assert!(!types.contains(&int, &int));
        assert!(!types.is_subsignature(&executable, &executable));
        assert!(types.direct_supertypes(&int).is_empty());
    }
}
