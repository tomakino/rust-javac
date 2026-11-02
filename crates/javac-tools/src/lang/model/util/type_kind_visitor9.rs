//! A visitor of types based on their kind with default behavior appropriate for
//! source versions RELEASE_9 through RELEASE_14.
//!
//! Java `TypeKindVisitor9 extends TypeKindVisitor8` only
//! overrides `visitNoTypeAsModule` (calls defaultAction).  The Rust trait is a
//! subtrait re-declaring that method; calling it on a concrete type
//! implementing both requires a fully qualified call (registered language-shape
//! limitation).

use super::type_kind_visitor8::TypeKindVisitor8;
use crate::lang::model::type_::NoType;

/// A visitor of types based on their kind with default behavior appropriate for
/// source versions RELEASE_9 through RELEASE_14.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see TypeKindVisitor6
/// @see TypeKindVisitor7
/// @see TypeKindVisitor8
/// @since 9
pub trait TypeKindVisitor9<R: Clone, P>: TypeKindVisitor8<R, P> {
    /// Visits a MODULE pseudo-type.
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_no_type_as_module(&mut self, t: &dyn NoType, p: P) -> R {
        self.default_action(t, p)
    }
}

#[cfg(test)]
mod tests {
    use super::TypeKindVisitor9;
    use crate::lang::model::type_::{TypeKind, TypeVisitor};
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
    impl crate::lang::model::util::type_kind_visitor6::TypeKindVisitor6<usize, ()> for DefaultVisitor {}
    impl crate::lang::model::util::type_kind_visitor7::TypeKindVisitor7<usize, ()> for DefaultVisitor {}
    impl crate::lang::model::util::type_kind_visitor8::TypeKindVisitor8<usize, ()> for DefaultVisitor {}
    impl TypeKindVisitor9<usize, ()> for DefaultVisitor {}

    #[test]
    fn module_pseudo_type_uses_default_action_in_release9() {
        // RELEASE_9 overrides visitNoTypeAsModule to call defaultAction.
        let module = TypeFixture::new(TypeKind::MODULE);
        let mut v = DefaultVisitor { value: 3 };
        let result = <DefaultVisitor as TypeKindVisitor9<usize, ()>>::visit_no_type_as_module(
            &mut v,
            &module,
            (),
        );
        assert_eq!(result, 3);
    }
}
