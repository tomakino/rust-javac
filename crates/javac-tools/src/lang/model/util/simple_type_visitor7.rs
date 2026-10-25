// Origin: javax/lang/model/util/SimpleTypeVisitor7.java @ 982d53c (jdk-17.0.19-ga)

//! A simple visitor of types with default behavior appropriate for the
//! RELEASE_7 source version.
//!
//! PORTING_RULES: Java `SimpleTypeVisitor7 extends SimpleTypeVisitor6` only
//! overrides `visitUnion` (calls defaultAction).  The Rust trait is a subtrait
//! re-declaring that method; calling it on a concrete type implementing both
//! requires a fully qualified call (registered language-shape limitation).

use super::simple_type_visitor6::SimpleTypeVisitor6;
use crate::lang::model::type_::UnionType;

/// A simple visitor of types with default behavior appropriate for the
/// RELEASE_7 source version.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see <a href="SimpleTypeVisitor6.html#note_for_subclasses">
/// <strong>Compatibility note for subclasses</strong></a>
/// @see SimpleTypeVisitor6
/// @see SimpleTypeVisitor8
/// @see SimpleTypeVisitor9
/// @see SimpleTypeVisitor14
/// @since 1.7
pub trait SimpleTypeVisitor7<R: Clone, P>: SimpleTypeVisitor6<R, P> {
    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    ///
    /// @since 1.7
    fn visit_union(&mut self, t: &dyn UnionType, p: P) -> R {
        self.default_action(t, p)
    }
}

#[cfg(test)]
mod tests {
    use super::SimpleTypeVisitor7;
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
    impl SimpleTypeVisitor7<usize, ()> for DefaultVisitor {}

    #[test]
    fn union_uses_default_action_in_release7() {
        let mut v = DefaultVisitor { value: 3 };
        let union = TypeFixture::new(TypeKind::UNION);
        assert_eq!(SimpleTypeVisitor7::visit_union(&mut v, &union, ()), 3);
    }
}
