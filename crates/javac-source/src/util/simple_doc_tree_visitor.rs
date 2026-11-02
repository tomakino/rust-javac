//! A simple visitor for doc tree nodes.
//!
//! (§14.7): Java class SimpleDocTreeVisitor<R,P> implements
//! DocTreeVisitor<R,P> -> Rust trait SimpleDocTreeVisitor<R: Clone, P: Clone>:
//! DocTreeVisitor<R, P>; nullable DEFAULT_VALUE -> default_value() -> Option<&R>
//! (default_action panics if it would return null); the final convenience
//! methods visit(DocTree,P)/visit(Iterable,P) -> visit(Option<&dyn DocTree>,P)
//! -> Option<R> and visit_all(&[Rc<dyn DocTree>],P) -> Option<R>; the 36 visit
//! methods are mechanically generated from the Java source; the double dispatch
//! goes through crate::util::dispatch::dispatch_simple_doc_tree.  Implementations
//! also write the empty impl DocTreeVisitor<R, P> for X {}.
//!
//! @author Jonathan Gibbons
//! @since 1.8

use crate::doctree::attribute_tree::AttributeTree;
use crate::doctree::author_tree::AuthorTree;
use crate::doctree::comment_tree::CommentTree;
use crate::doctree::deprecated_tree::DeprecatedTree;
use crate::doctree::doc_comment_tree::DocCommentTree;
use crate::doctree::doc_root_tree::DocRootTree;
use crate::doctree::doc_tree::DocTree;
use crate::doctree::doc_tree_visitor::DocTreeVisitor;
use crate::doctree::doc_type_tree::DocTypeTree;
use crate::doctree::end_element_tree::EndElementTree;
use crate::doctree::entity_tree::EntityTree;
use crate::doctree::erroneous_tree::ErroneousTree;
use crate::doctree::hidden_tree::HiddenTree;
use crate::doctree::identifier_tree::IdentifierTree;
use crate::doctree::index_tree::IndexTree;
use crate::doctree::inherit_doc_tree::InheritDocTree;
use crate::doctree::link_tree::LinkTree;
use crate::doctree::literal_tree::LiteralTree;
use crate::doctree::param_tree::ParamTree;
use crate::doctree::provides_tree::ProvidesTree;
use crate::doctree::reference_tree::ReferenceTree;
use crate::doctree::return_tree::ReturnTree;
use crate::doctree::see_tree::SeeTree;
use crate::doctree::serial_data_tree::SerialDataTree;
use crate::doctree::serial_field_tree::SerialFieldTree;
use crate::doctree::serial_tree::SerialTree;
use crate::doctree::since_tree::SinceTree;
use crate::doctree::start_element_tree::StartElementTree;
use crate::doctree::summary_tree::SummaryTree;
use crate::doctree::system_property_tree::SystemPropertyTree;
use crate::doctree::text_tree::TextTree;
use crate::doctree::throws_tree::ThrowsTree;
use crate::doctree::unknown_block_tag_tree::UnknownBlockTagTree;
use crate::doctree::unknown_inline_tag_tree::UnknownInlineTagTree;
use crate::doctree::uses_tree::UsesTree;
use crate::doctree::value_tree::ValueTree;
use crate::doctree::version_tree::VersionTree;
use crate::util::dispatch::dispatch_simple_doc_tree;
use std::rc::Rc;

/// A simple visitor for doc tree nodes.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @author Jonathan Gibbons
/// @since 1.8
pub trait SimpleDocTreeVisitor<R: Clone, P: Clone>: DocTreeVisitor<R, P> {
    /// The default value, returned by the default action.
    ///
    /// Java: protected final R DEFAULT_VALUE (null for the no-arg constructor,
    /// which has no Rust form; the accessor returns None for it).
    fn default_value(&self) -> Option<&R>;

    /// The default action, used by all visit methods that are not overridden.
    fn default_action(&mut self, _node: &dyn DocTree, _p: P) -> R {
        match self.default_value() {
            Some(value) => value.clone(),
            None => panic!(
                "SimpleDocTreeVisitor.defaultAction: DEFAULT_VALUE is null (Java); \
                 the Rust implementation must return Some(value) from default_value()"
            ),
        }
    }

    /// Invokes the appropriate visit method specific to the type of the node.
    fn visit(&mut self, node: Option<&dyn DocTree>, p: P) -> Option<R> {
        match node {
            Some(node) => Some(dispatch_simple_doc_tree(self, node, p)),
            None => None,
        }
    }

    /// Invokes the appropriate visit method on each of a sequence of nodes.
    fn visit_all(&mut self, nodes: &[Rc<dyn DocTree>], p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for node in nodes {
            r = self.visit(Some(node.as_ref()), p.clone());
        }
        r
    }
    /// A simple visitor for tree nodes.
    ///
    /// @param <R> the return type of this visitor's methods.  Use {@link
    ///            Void} for visitors that do not need to return results.
    /// @param <P> the type of the additional parameter to this visitor's
    ///            methods.  Use {@code Void} for visitors that do not need an
    ///            additional parameter.
    ///
    /// @since 1.8
    /// /
    /// public class SimpleDocTreeVisitor<R,P> implements DocTreeVisitor<R, P> {
    ///     /**
    /// The default value, returned by the {@link #defaultAction default action}.
    /// /
    ///     protected final R DEFAULT_VALUE;
    ///
    ///     /**
    /// Creates a visitor, with a DEFAULT_VALUE of {@code null}.
    /// /
    ///     protected SimpleDocTreeVisitor() {
    ///         DEFAULT_VALUE = null;
    ///     }
    ///
    ///     /**
    /// Creates a visitor, with a specified DEFAULT_VALUE.
    /// @param defaultValue the default value to be returned by the default action
    /// /
    ///     protected SimpleDocTreeVisitor(R defaultValue) {
    ///         DEFAULT_VALUE = defaultValue;
    ///     }
    ///
    ///     /**
    /// The default action, used by all visit methods that are not overridden.
    /// @param node the node being visited
    /// @param p the parameter value passed to the visit method
    /// @return the result value to be returned from the visit method
    /// /
    ///     protected R defaultAction(DocTree node, P p) {
    ///         return DEFAULT_VALUE;
    ///     }
    ///
    ///     /**
    /// Invokes the appropriate visit method specific to the type of the node.
    /// @param node the node on which to dispatch
    /// @param p a parameter to be passed to the appropriate visit method
    /// @return the value returns from the appropriate visit method
    /// /
    ///     public final R visit(DocTree node, P p) {
    ///         return (node == null) ? null : node.accept(this, p);
    ///     }
    ///
    ///     /**
    /// Invokes the appropriate visit method on each of a sequence of nodes.
    /// @param nodes the nodes on which to dispatch
    /// @param p a parameter value to be passed to each appropriate visit method
    /// @return the value return from the last of the visit methods, or null
    ///      if none were called
    /// /
    ///     public final R visit(Iterable<? extends DocTree> nodes, P p) {
    ///         R r = null;
    ///         if (nodes != null) {
    ///             for (DocTree node : nodes)
    ///                 r = visit(node, p);
    ///         }
    ///         return r;
    ///     }
    ///
    ///     /**
    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_attribute(&mut self, node: &dyn AttributeTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_author(&mut self, node: &dyn AuthorTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_comment(&mut self, node: &dyn CommentTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_deprecated(&mut self, node: &dyn DeprecatedTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_doc_comment(&mut self, node: &dyn DocCommentTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_doc_root(&mut self, node: &dyn DocRootTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    /// @since 10
    fn visit_doc_type(&mut self, node: &dyn DocTypeTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_end_element(&mut self, node: &dyn EndElementTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_entity(&mut self, node: &dyn EntityTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_erroneous(&mut self, node: &dyn ErroneousTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return the result of {@code defaultAction}
    ///
    /// @since 9
    fn visit_hidden(&mut self, node: &dyn HiddenTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_identifier(&mut self, node: &dyn IdentifierTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    ///
    /// @since 9
    fn visit_index(&mut self, node: &dyn IndexTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_inherit_doc(&mut self, node: &dyn InheritDocTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_link(&mut self, node: &dyn LinkTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_literal(&mut self, node: &dyn LiteralTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_param(&mut self, node: &dyn ParamTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    ///
    /// @since 9
    fn visit_provides(&mut self, node: &dyn ProvidesTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_reference(&mut self, node: &dyn ReferenceTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_return(&mut self, node: &dyn ReturnTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_see(&mut self, node: &dyn SeeTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_serial(&mut self, node: &dyn SerialTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_serial_data(&mut self, node: &dyn SerialDataTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_serial_field(&mut self, node: &dyn SerialFieldTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_since(&mut self, node: &dyn SinceTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_start_element(&mut self, node: &dyn StartElementTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    /// @since 10
    fn visit_summary(&mut self, node: &dyn SummaryTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    /// @since 12
    fn visit_system_property(&mut self, node: &dyn SystemPropertyTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_text(&mut self, node: &dyn TextTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_throws(&mut self, node: &dyn ThrowsTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_unknown_block_tag(&mut self, node: &dyn UnknownBlockTagTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_unknown_inline_tag(&mut self, node: &dyn UnknownInlineTagTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    ///
    /// @since 9
    fn visit_uses(&mut self, node: &dyn UsesTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_value(&mut self, node: &dyn ValueTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_version(&mut self, node: &dyn VersionTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_other(&mut self, node: &dyn DocTree, p: P) -> R {
        self.default_action(node, p)
    }
}

#[cfg(test)]
mod tests {
    use super::SimpleDocTreeVisitor;
    use crate::doctree::doc_tree::DocTree;
    use crate::doctree::doc_tree_visitor::DocTreeVisitor;
    use crate::doctree::text_tree::TextTree;
    use crate::util::test_support::{DocCommentLeaf, DocLeaf};
    use std::rc::Rc;

    struct DefaultDocVisitor {
        value: usize,
    }

    impl DocTreeVisitor<usize, ()> for DefaultDocVisitor {}

    impl SimpleDocTreeVisitor<usize, ()> for DefaultDocVisitor {
        fn default_value(&self) -> Option<&usize> {
            Some(&self.value)
        }
    }

    struct CustomDocVisitor;

    impl DocTreeVisitor<usize, ()> for CustomDocVisitor {}

    impl SimpleDocTreeVisitor<usize, ()> for CustomDocVisitor {
        fn default_value(&self) -> Option<&usize> {
            None
        }
        fn visit_text(&mut self, _node: &dyn TextTree, _p: ()) -> usize {
            7
        }
    }

    #[test]
    fn default_action_returns_the_default_value() {
        let mut visitor = DefaultDocVisitor { value: 42 };
        assert_eq!(
            SimpleDocTreeVisitor::visit_text(&mut visitor, &DocLeaf, ()),
            42
        );
        assert_eq!(visitor.visit(Some(&DocLeaf), ()), Some(42));
        assert_eq!(visitor.visit(None, ()), None);
    }

    #[test]
    fn dispatch_uses_the_overridden_visit_method() {
        let mut visitor = CustomDocVisitor;
        assert_eq!(visitor.visit(Some(&DocLeaf), ()), Some(7));
    }

    #[test]
    fn visit_all_iterates_and_keeps_the_last_result() {
        let mut visitor = CustomDocVisitor;
        let nodes: Vec<Rc<dyn DocTree>> = vec![Rc::new(DocLeaf), Rc::new(DocLeaf)];
        assert_eq!(visitor.visit_all(&nodes, ()), Some(7));
    }

    #[test]
    #[should_panic(expected = "DEFAULT_VALUE is null")]
    fn default_action_panics_when_default_value_is_null() {
        let mut visitor = CustomDocVisitor;
        // dispatch to the not-overridden visitDocComment
        let doc = DocCommentLeaf;
        let _ = visitor.visit(Some(&doc), ());
    }
}
