//! A visitor of types based on their kind with default behavior appropriate for
//! the RELEASE_6 source version.
//!
//! PORTING_RULES: Java concrete class -> Rust trait with the upstream default
//! behavior; the switch default arms that throw AssertionError unconditionally
//! become panics with the upstream message.  Methods shared with
//! SimpleTypeVisitor6 are re-declared by this subtrait with the
//! kind-dispatching behavior; calling them on a concrete type implementing both
//! requires a fully qualified call (registered language-shape limitation).

use super::abstract_type_visitor6::AbstractTypeVisitor6;
use super::simple_type_visitor6::SimpleTypeVisitor6;
use crate::lang::model::type_::{NoType, PrimitiveType, TypeKind};

/// A visitor of types based on their kind with default behavior appropriate for
/// the RELEASE_6 source version.  For types Xyz that may have more than one
/// kind, the visitXyz methods in this class delegate to the visitXyzAsKind
/// method corresponding to the first argument's kind.  The visitXyzAsKind
/// methods call {@link #defaultAction defaultAction}, passing their arguments
/// to defaultAction's corresponding parameters.
///
/// @param <R> the return type of this visitor's methods.  Use Void
///            for visitors that do not need to return results.
/// @param <P> the type of the additional parameter to this visitor's
///            methods.  Use Void for visitors that do not need an
///            additional parameter.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
///
/// @see TypeKindVisitor7
/// @see TypeKindVisitor8
/// @see TypeKindVisitor9
/// @see TypeKindVisitor14
/// @since 1.6
pub trait TypeKindVisitor6<R: Clone, P>: SimpleTypeVisitor6<R, P> {
    /// {@inheritDoc}
    ///
    /// @implSpec This implementation dispatches to the visit method for
    /// the specific kind of primitive type:
    /// BOOLEAN, BYTE, etc.
    fn visit_primitive(&mut self, t: &dyn PrimitiveType, p: P) -> R {
        let k = t.get_kind();
        match k {
            TypeKind::BOOLEAN => self.visit_primitive_as_boolean(t, p),
            TypeKind::BYTE => self.visit_primitive_as_byte(t, p),
            TypeKind::SHORT => self.visit_primitive_as_short(t, p),
            TypeKind::INT => self.visit_primitive_as_int(t, p),
            TypeKind::LONG => self.visit_primitive_as_long(t, p),
            TypeKind::CHAR => self.visit_primitive_as_char(t, p),
            TypeKind::FLOAT => self.visit_primitive_as_float(t, p),
            TypeKind::DOUBLE => self.visit_primitive_as_double(t, p),
            _ => panic!("AssertionError: Bad kind {k} for PrimitiveType {t}"),
        }
    }

    /// Visits a BOOLEAN primitive type.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_primitive_as_boolean(&mut self, t: &dyn PrimitiveType, p: P) -> R {
        self.default_action(t, p)
    }

    /// Visits a BYTE primitive type.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_primitive_as_byte(&mut self, t: &dyn PrimitiveType, p: P) -> R {
        self.default_action(t, p)
    }

    /// Visits a SHORT primitive type.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_primitive_as_short(&mut self, t: &dyn PrimitiveType, p: P) -> R {
        self.default_action(t, p)
    }

    /// Visits an INT primitive type.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_primitive_as_int(&mut self, t: &dyn PrimitiveType, p: P) -> R {
        self.default_action(t, p)
    }

    /// Visits a LONG primitive type.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_primitive_as_long(&mut self, t: &dyn PrimitiveType, p: P) -> R {
        self.default_action(t, p)
    }

    /// Visits a CHAR primitive type.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_primitive_as_char(&mut self, t: &dyn PrimitiveType, p: P) -> R {
        self.default_action(t, p)
    }

    /// Visits a FLOAT primitive type.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_primitive_as_float(&mut self, t: &dyn PrimitiveType, p: P) -> R {
        self.default_action(t, p)
    }

    /// Visits a DOUBLE primitive type.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_primitive_as_double(&mut self, t: &dyn PrimitiveType, p: P) -> R {
        self.default_action(t, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation dispatches to the visit method for
    /// the specific kind of pseudo-type:
    /// VOID, PACKAGE, MODULE, or NONE.
    fn visit_no_type(&mut self, t: &dyn NoType, p: P) -> R {
        let k = t.get_kind();
        match k {
            TypeKind::VOID => self.visit_no_type_as_void(t, p),
            TypeKind::PACKAGE => self.visit_no_type_as_package(t, p),
            TypeKind::MODULE => self.visit_no_type_as_module(t, p),
            TypeKind::NONE => self.visit_no_type_as_none(t, p),
            _ => panic!("AssertionError: Bad kind {k} for NoType {t}"),
        }
    }

    /// Visits a VOID pseudo-type.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_no_type_as_void(&mut self, t: &dyn NoType, p: P) -> R {
        self.default_action(t, p)
    }

    /// Visits a PACKAGE pseudo-type.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_no_type_as_package(&mut self, t: &dyn NoType, p: P) -> R {
        self.default_action(t, p)
    }

    /// Visits a MODULE pseudo-type.
    ///
    /// @implSpec This implementation calls visitUnknown.
    ///
    /// @since 10
    fn visit_no_type_as_module(&mut self, t: &dyn NoType, p: P) -> R {
        <Self as AbstractTypeVisitor6<R, P>>::visit_unknown(self, t, p)
    }

    /// Visits a NONE pseudo-type.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_no_type_as_none(&mut self, t: &dyn NoType, p: P) -> R {
        self.default_action(t, p)
    }
}

#[cfg(test)]
mod tests {
    use super::TypeKindVisitor6;
    use crate::lang::model::type_::{NoType, PrimitiveType, TypeKind, TypeVisitor};
    use crate::lang::model::util::test_support::TypeFixture;

    struct DefaultVisitor {
        value: usize,
    }

    impl TypeVisitor<usize, ()> for DefaultVisitor {}
    impl crate::lang::model::util::abstract_type_visitor6::AbstractTypeVisitor6<usize, ()>
        for DefaultVisitor
    {
    }
    impl crate::lang::model::util::simple_type_visitor6::SimpleTypeVisitor6<usize, ()>
        for DefaultVisitor
    {
        fn default_value(&self) -> Option<&usize> {
            Some(&self.value)
        }
    }
    impl TypeKindVisitor6<usize, ()> for DefaultVisitor {}

    #[test]
    fn primitive_and_no_type_dispatch_by_kind() {
        let mut v = DefaultVisitor { value: 3 };
        let int = TypeFixture::new(TypeKind::INT);
        assert_eq!(TypeKindVisitor6::visit_primitive(&mut v, &int, ()), 3);
        let void = TypeFixture::new(TypeKind::VOID);
        assert_eq!(TypeKindVisitor6::visit_no_type(&mut v, &void, ()), 3);
        let _: &dyn NoType = &void;
        let _: &dyn PrimitiveType = &int;
    }

    #[test]
    #[should_panic(expected = "Unknown type")]
    fn module_pseudo_type_is_unknown_in_release6() {
        let mut v = DefaultVisitor { value: 3 };
        let module = TypeFixture::new(TypeKind::MODULE);
        TypeKindVisitor6::visit_no_type(&mut v, &module, ());
    }
}
