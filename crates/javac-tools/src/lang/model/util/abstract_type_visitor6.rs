//! A skeletal visitor of types with default behavior appropriate for the
//! RELEASE_6 source version.
//!
//! — class mapping:
//! * Java abstract class -> Rust trait with the upstream default behavior.
//! * the Java `final R visit(TypeMirror, P)` forwards to `t.accept(this, p)`;
//!   Rust cannot call the generic `TypeMirror::accept` on `&dyn TypeMirror`, so
//!   the dispatch is reproduced from `TypeMirror::get_kind` and the
//!   object-conversion accessors (registered adapter contract).  The
//!   single-argument convenience overload `visit(TypeMirror)` forwards null
//!   upstream and is not ported.
//! * `visitUnknown` throws `UnknownTypeException` upstream; the Rust default
//!   panics with the same exception message.
//! * the Java constructors and @SupportedSourceVersion annotations have no Rust
//!   form.

use crate::lang::model::type_::{
    IntersectionType, TypeKind, TypeMirror, TypeVisitor, UnionType,
};

/// A skeletal visitor of types with default behavior appropriate for
/// the RELEASE_6 source version.
///
/// <p id=note_for_subclasses><strong>WARNING:</strong> The TypeVisitor
/// interface implemented by this class may have methods added to it in the
/// future to accommodate new, currently unknown, language structures added to
/// future versions of the Java programming language.  Therefore, methods whose
/// names begin with "visit" may be added to this class in the future; to avoid
/// incompatibilities, classes and subclasses which extend this class should not
/// declare any instance methods with names beginning with "visit".
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
///
/// @see AbstractTypeVisitor7
/// @see AbstractTypeVisitor8
/// @see AbstractTypeVisitor9
/// @see AbstractTypeVisitor14
/// @since 1.6
pub trait AbstractTypeVisitor6<R, P>: TypeVisitor<R, P> {
    /// Visits any type mirror as if by passing itself to that type
    /// mirror's accept method.  The
    /// invocation v.visit(t, p) is equivalent to t.accept(v, p).
    ///
    /// @param t  the type to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    fn visit(&mut self, t: &dyn TypeMirror, p: P) -> R {
        dispatch_type(self, t, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec Visits a UnionType element by calling visitUnknown.
    ///
    /// @param t  {@inheritDoc}
    /// @param p  {@inheritDoc}
    /// @return the result of visitUnknown
    ///
    /// @since 1.7
    fn visit_union(&mut self, t: &dyn UnionType, p: P) -> R {
        <Self as AbstractTypeVisitor6<R, P>>::visit_unknown(self, t, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec Visits an IntersectionType element by calling visitUnknown.
    ///
    /// @param t  {@inheritDoc}
    /// @param p  {@inheritDoc}
    /// @return the result of visitUnknown
    ///
    /// @since 1.8
    fn visit_intersection(&mut self, t: &dyn IntersectionType, p: P) -> R {
        <Self as AbstractTypeVisitor6<R, P>>::visit_unknown(self, t, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec The default implementation of this method in
    /// AbstractTypeVisitor6 will always throw
    /// new UnknownTypeException(t, p).  This behavior is not required of a
    /// subclass.
    ///
    /// @param t  {@inheritDoc}
    /// @param p  {@inheritDoc}
    /// @return a visitor-specified result
    /// @throws UnknownTypeException
    ///  a visitor implementation may optionally throw this exception
    fn visit_unknown(&mut self, t: &dyn TypeMirror, p: P) -> R {
        let _ = p;
        // Java: throw new UnknownTypeException(t, p);
        panic!("Unknown type: \"{}\"", t)
    }
}

/// Java: the type's own `accept(visitor, p)` performs the dispatch.
///
/// `TypeMirror::accept` is generic and therefore not callable on
/// `&dyn TypeMirror`; the dispatch is reproduced from the type kind and the
/// object-conversion accessors (registered adapter contract).  A missing
/// accessor panics instead of silently mis-dispatching.
pub(crate) fn dispatch_type<R, P, V: TypeVisitor<R, P> + ?Sized>(
    v: &mut V,
    t: &dyn TypeMirror,
    p: P,
) -> R {
    match t.get_kind() {
        TypeKind::BOOLEAN
        | TypeKind::BYTE
        | TypeKind::SHORT
        | TypeKind::INT
        | TypeKind::LONG
        | TypeKind::CHAR
        | TypeKind::FLOAT
        | TypeKind::DOUBLE => match t.as_primitive_type() {
            Some(ty) => v.visit_primitive(ty, p),
            None => missing_dispatch(t),
        },
        TypeKind::VOID | TypeKind::NONE | TypeKind::PACKAGE | TypeKind::MODULE => {
            match t.as_no_type() {
                Some(ty) => v.visit_no_type(ty, p),
                None => missing_dispatch(t),
            }
        }
        TypeKind::NULL => match t.as_null_type() {
            Some(ty) => v.visit_null(ty, p),
            None => missing_dispatch(t),
        },
        TypeKind::ARRAY => match t.as_array_type() {
            Some(ty) => v.visit_array(ty, p),
            None => missing_dispatch(t),
        },
        TypeKind::DECLARED => match t.as_declared_type() {
            Some(ty) => v.visit_declared(ty, p),
            None => missing_dispatch(t),
        },
        TypeKind::ERROR => match t.as_error_type() {
            Some(ty) => v.visit_error(ty, p),
            None => missing_dispatch(t),
        },
        TypeKind::TYPEVAR => match t.as_type_variable() {
            Some(ty) => v.visit_type_variable(ty, p),
            None => missing_dispatch(t),
        },
        TypeKind::WILDCARD => match t.as_wildcard_type() {
            Some(ty) => v.visit_wildcard(ty, p),
            None => missing_dispatch(t),
        },
        TypeKind::EXECUTABLE => match t.as_executable_type() {
            Some(ty) => v.visit_executable(ty, p),
            None => missing_dispatch(t),
        },
        TypeKind::UNION => match t.as_union_type() {
            Some(ty) => v.visit_union(ty, p),
            None => missing_dispatch(t),
        },
        TypeKind::INTERSECTION => match t.as_intersection_type() {
            Some(ty) => v.visit_intersection(ty, p),
            None => missing_dispatch(t),
        },
        TypeKind::OTHER => v.visit_unknown(t, p),
    }
}

fn missing_dispatch(t: &dyn TypeMirror) -> ! {
    panic!(
        "ClassCastException: type of kind {} does not provide its subinterface \
         (Rust: concrete TypeMirror implementations must override the \
         as_xxx accessor for their kind)",
        t.get_kind()
    )
}

#[cfg(test)]
mod tests {
    use super::AbstractTypeVisitor6;
    use crate::lang::model::type_::{DeclaredType, TypeKind, TypeMirror, TypeVisitor};
    use crate::lang::model::util::test_support::TypeFixture;

    struct TestVisitor {
        visited: Vec<TypeKind>,
    }

    impl TestVisitor {
        fn new() -> TestVisitor {
            TestVisitor { visited: Vec::new() }
        }
    }

    impl AbstractTypeVisitor6<(), ()> for TestVisitor {}

    impl TypeVisitor<(), ()> for TestVisitor {
        fn visit_declared(&mut self, t: &dyn DeclaredType, _p: ()) {
            self.visited.push(t.get_kind());
        }
        fn visit_unknown(&mut self, t: &dyn TypeMirror, _p: ()) {
            self.visited.push(t.get_kind());
        }
    }

    #[test]
    fn dispatch_uses_the_type_kind() {
        let mut v = TestVisitor::new();
        let declared = TypeFixture::new(TypeKind::DECLARED);
        AbstractTypeVisitor6::visit(&mut v, &declared, ());
        let other = TypeFixture::new(TypeKind::OTHER);
        AbstractTypeVisitor6::visit(&mut v, &other, ());
        assert_eq!(v.visited, vec![TypeKind::DECLARED, TypeKind::OTHER]);
    }

    #[test]
    fn visitor_is_object_safe() {
        let v: &mut dyn AbstractTypeVisitor6<(), ()> = &mut TestVisitor::new();
        let declared = TypeFixture::new(TypeKind::DECLARED);
        AbstractTypeVisitor6::visit(v, &declared, ());
    }
}
