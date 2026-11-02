//! A simple visitor of types with default behavior appropriate for the
//! RELEASE_6 source version.
//!
//! — class mapping:
//! * Java concrete class -> Rust trait with the upstream default behavior.
//! * the Java `protected final R DEFAULT_VALUE` field (null default from the
//!   no-arg constructor) -> `default_value() -> Option<&R>`; `default_action`
//!   panics if it would have returned null (Java returns null).
//! * visit methods take `&mut self` and call `default_action`.
//! * visit methods for constructs newer than RELEASE_6 (union, intersection)
//!   are inherited from AbstractTypeVisitor6 and call `visit_unknown`, exactly
//!   as upstream.

use super::abstract_type_visitor6::AbstractTypeVisitor6;
use crate::lang::model::type_::{
    ArrayType, DeclaredType, ErrorType, ExecutableType, NoType, NullType, PrimitiveType, TypeMirror,
    TypeVariable, WildcardType,
};

/// A simple visitor of types with default behavior appropriate for the
/// RELEASE_6 source version.
///
/// Visit methods corresponding to RELEASE_6 language
/// constructs call {@link #defaultAction defaultAction}, passing their
/// arguments to defaultAction's corresponding parameters.
///
/// For constructs introduced in RELEASE_7 and later,
/// visitUnknown is called instead.
///
/// <p id=note_for_subclasses><strong>WARNING:</strong> The TypeVisitor
/// interface implemented by this class may have methods added to it in the
/// future to accommodate new, currently unknown, language structures added to
/// future versions of the Java programming language.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
///
/// @see SimpleTypeVisitor7
/// @see SimpleTypeVisitor8
/// @see SimpleTypeVisitor9
/// @see SimpleTypeVisitor14
/// @since 1.6
pub trait SimpleTypeVisitor6<R: Clone, P>: AbstractTypeVisitor6<R, P> {
    /// Default value to be returned; {@link #defaultAction
    /// defaultAction} returns this value unless the method is
    /// overridden.
    ///
    /// Java: protected final R DEFAULT_VALUE; (null for the no-arg
    /// constructor, which has no Rust form; the accessor returns None for it).
    fn default_value(&self) -> Option<&R>;

    /// The default action for visit methods.
    ///
    /// @implSpec The implementation in this class just returns {@link
    /// #DEFAULT_VALUE}; subclasses will commonly override this method.
    ///
    /// @param e the type to process
    /// @param p a visitor-specified parameter
    /// @return DEFAULT_VALUE unless overridden
    fn default_action(&mut self, _e: &dyn TypeMirror, _p: P) -> R {
        match self.default_value() {
            Some(value) => value.clone(),
            None => panic!(
                "SimpleTypeVisitor6.defaultAction: DEFAULT_VALUE is null (Java); the Rust \
                 implementation must return Some(default value) from default_value()"
            ),
        }
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_primitive(&mut self, t: &dyn PrimitiveType, p: P) -> R {
        self.default_action(t, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_null(&mut self, t: &dyn NullType, p: P) -> R {
        self.default_action(t, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_array(&mut self, t: &dyn ArrayType, p: P) -> R {
        self.default_action(t, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_declared(&mut self, t: &dyn DeclaredType, p: P) -> R {
        self.default_action(t, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_error(&mut self, t: &dyn ErrorType, p: P) -> R {
        self.default_action(t, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_type_variable(&mut self, t: &dyn TypeVariable, p: P) -> R {
        self.default_action(t, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_wildcard(&mut self, t: &dyn WildcardType, p: P) -> R {
        self.default_action(t, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_executable(&mut self, t: &dyn ExecutableType, p: P) -> R {
        self.default_action(t, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_no_type(&mut self, t: &dyn NoType, p: P) -> R {
        self.default_action(t, p)
    }
}

#[cfg(test)]
mod tests {
    use super::SimpleTypeVisitor6;
    use crate::lang::model::type_::{PrimitiveType, TypeKind, TypeMirror, TypeVisitor};
    use crate::lang::model::util::test_support::TypeFixture;

    struct DefaultVisitor {
        value: usize,
    }

    impl TypeVisitor<usize, ()> for DefaultVisitor {}
    impl crate::lang::model::util::abstract_type_visitor6::AbstractTypeVisitor6<usize, ()>
        for DefaultVisitor
    {
    }
    impl SimpleTypeVisitor6<usize, ()> for DefaultVisitor {
        fn default_value(&self) -> Option<&usize> {
            Some(&self.value)
        }
    }

    #[test]
    fn visit_calls_default_action() {
        let mut v = DefaultVisitor { value: 3 };
        let declared = TypeFixture::new(TypeKind::DECLARED);
        assert_eq!(SimpleTypeVisitor6::visit_declared(&mut v, &declared, ()), 3);
        let int = TypeFixture::new(TypeKind::INT);
        assert_eq!(SimpleTypeVisitor6::visit_primitive(&mut v, &int, ()), 3);
        assert_eq!(int.get_kind(), TypeKind::INT);
        let _: &dyn PrimitiveType = &int;
    }
}
