//! A visitor of documentation trees, in the style of the visitor design
//! pattern.
//!
//! (§14): abstract interface methods carry a panicking Rust
//! default; the 7 Java `default` methods that forward to `visitOther` keep that
//! forwarding as the Rust default.

/// A visitor of trees, in the style of the visitor design pattern.
/// Classes implementing this interface are used to operate
/// on a tree when the kind of tree is unknown at compile time.
/// When a visitor is passed to a tree's {@link DocTree#accept
/// accept} method, the <code>visit<i>Xyz</i></code> method most applicable
/// to that tree is invoked.
///
/// <p> Classes implementing this interface may or may not throw a
/// {@code NullPointerException} if the additional parameter {@code p}
/// is {@code null}; see documentation of the implementing class for
/// details.
///
/// <p> <b>WARNING:</b> It is possible that methods will be added to
/// this interface to accommodate new, currently unknown, doc comment
/// structures added to future versions of the Java programming
/// language.  Therefore, visitor classes directly implementing this
/// interface may be source incompatible with future versions of the
/// platform.
///
/// @param <R> the return type of this visitor's methods.  Use {@link
///            Void} for visitors that do not need to return results.
/// @param <P> the type of the additional parameter to this visitor's
///            methods.  Use {@code Void} for visitors that do not need an
///            additional parameter.
///
/// @since 1.8
use super::attribute_tree::AttributeTree;
use super::author_tree::AuthorTree;
use super::comment_tree::CommentTree;
use super::deprecated_tree::DeprecatedTree;
use super::doc_comment_tree::DocCommentTree;
use super::doc_root_tree::DocRootTree;
use super::doc_tree::DocTree;
use super::doc_type_tree::DocTypeTree;
use super::end_element_tree::EndElementTree;
use super::entity_tree::EntityTree;
use super::erroneous_tree::ErroneousTree;
use super::hidden_tree::HiddenTree;
use super::identifier_tree::IdentifierTree;
use super::index_tree::IndexTree;
use super::inherit_doc_tree::InheritDocTree;
use super::link_tree::LinkTree;
use super::literal_tree::LiteralTree;
use super::param_tree::ParamTree;
use super::provides_tree::ProvidesTree;
use super::reference_tree::ReferenceTree;
use super::return_tree::ReturnTree;
use super::see_tree::SeeTree;
use super::serial_data_tree::SerialDataTree;
use super::serial_field_tree::SerialFieldTree;
use super::serial_tree::SerialTree;
use super::since_tree::SinceTree;
use super::start_element_tree::StartElementTree;
use super::summary_tree::SummaryTree;
use super::system_property_tree::SystemPropertyTree;
use super::text_tree::TextTree;
use super::throws_tree::ThrowsTree;
use super::unknown_block_tag_tree::UnknownBlockTagTree;
use super::unknown_inline_tag_tree::UnknownInlineTagTree;
use super::uses_tree::UsesTree;
use super::value_tree::ValueTree;
use super::version_tree::VersionTree;

pub trait DocTreeVisitor<R, P> {
    /// Visits an AttributeTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_attribute(&mut self, _node: &dyn AttributeTree, _p: P) -> R {
        panic!(
            "AbstractMethodError: visit_attribute: Rust: visitor interface method not overridden"
        )
    }

    /// Visits an AuthorTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_author(&mut self, _node: &dyn AuthorTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_author: Rust: visitor interface method not overridden")
    }

    /// Visits a CommentTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_comment(&mut self, _node: &dyn CommentTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_comment: Rust: visitor interface method not overridden")
    }

    /// Visits a DeprecatedTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_deprecated(&mut self, _node: &dyn DeprecatedTree, _p: P) -> R {
        panic!(
            "AbstractMethodError: visit_deprecated: Rust: visitor interface method not overridden"
        )
    }

    /// Visits a DocCommentTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_doc_comment(&mut self, _node: &dyn DocCommentTree, _p: P) -> R {
        panic!(
            "AbstractMethodError: visit_doc_comment: Rust: visitor interface method not overridden"
        )
    }

    /// Visits a DocRootTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_doc_root(&mut self, _node: &dyn DocRootTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_doc_root: Rust: visitor interface method not overridden")
    }

    /// Visits a DocTypeTree node.
    ///
    /// @implSpec Visits the provided {@code DocTypeTree} node
    /// by calling {@code visitOther(node, p)}.
    ///
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    /// @since 10
    fn visit_doc_type(&mut self, node: &dyn DocTypeTree, p: P) -> R {
        self.visit_other(node, p)
    }

    /// Visits an EndElementTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_end_element(&mut self, _node: &dyn EndElementTree, _p: P) -> R {
        panic!(
            "AbstractMethodError: visit_end_element: Rust: visitor interface method not overridden"
        )
    }

    /// Visits an EntityTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_entity(&mut self, _node: &dyn EntityTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_entity: Rust: visitor interface method not overridden")
    }

    /// Visits an ErroneousTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_erroneous(&mut self, _node: &dyn ErroneousTree, _p: P) -> R {
        panic!(
            "AbstractMethodError: visit_erroneous: Rust: visitor interface method not overridden"
        )
    }

    /// Visits a HiddenTree node.
    ///
    /// @implSpec Visits the provided {@code HiddenTree} node
    /// by calling {@code visitOther(node, p)}.
    ///
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    ///
    /// @since 9
    fn visit_hidden(&mut self, node: &dyn HiddenTree, p: P) -> R {
        self.visit_other(node, p)
    }

    /// Visits an IdentifierTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_identifier(&mut self, _node: &dyn IdentifierTree, _p: P) -> R {
        panic!(
            "AbstractMethodError: visit_identifier: Rust: visitor interface method not overridden"
        )
    }

    /// Visits an IndexTree node.
    ///
    /// @implSpec Visits the provided {@code IndexTree} node
    /// by calling {@code visitOther(node, p)}.
    ///
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    ///
    /// @since 9
    fn visit_index(&mut self, node: &dyn IndexTree, p: P) -> R {
        self.visit_other(node, p)
    }

    /// Visits an InheritDocTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_inherit_doc(&mut self, _node: &dyn InheritDocTree, _p: P) -> R {
        panic!(
            "AbstractMethodError: visit_inherit_doc: Rust: visitor interface method not overridden"
        )
    }

    /// Visits a LinkTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_link(&mut self, _node: &dyn LinkTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_link: Rust: visitor interface method not overridden")
    }

    /// Visits an LiteralTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_literal(&mut self, _node: &dyn LiteralTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_literal: Rust: visitor interface method not overridden")
    }

    /// Visits a ParamTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_param(&mut self, _node: &dyn ParamTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_param: Rust: visitor interface method not overridden")
    }

    /// Visits a ProvidesTree node.
    ///
    /// @implSpec Visits the provided {@code ProvidesTree} node
    /// by calling {@code visitOther(node, p)}.
    ///
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    ///
    /// @since 9
    fn visit_provides(&mut self, node: &dyn ProvidesTree, p: P) -> R {
        self.visit_other(node, p)
    }

    /// Visits a ReferenceTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_reference(&mut self, _node: &dyn ReferenceTree, _p: P) -> R {
        panic!(
            "AbstractMethodError: visit_reference: Rust: visitor interface method not overridden"
        )
    }

    /// Visits a ReturnTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_return(&mut self, _node: &dyn ReturnTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_return: Rust: visitor interface method not overridden")
    }

    /// Visits a SeeTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_see(&mut self, _node: &dyn SeeTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_see: Rust: visitor interface method not overridden")
    }

    /// Visits a SerialTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_serial(&mut self, _node: &dyn SerialTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_serial: Rust: visitor interface method not overridden")
    }

    /// Visits a SerialDataTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_serial_data(&mut self, _node: &dyn SerialDataTree, _p: P) -> R {
        panic!(
            "AbstractMethodError: visit_serial_data: Rust: visitor interface method not overridden"
        )
    }

    /// Visits a SerialFieldTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_serial_field(&mut self, _node: &dyn SerialFieldTree, _p: P) -> R {
        panic!(
            "AbstractMethodError: visit_serial_field: Rust: visitor interface method not overridden"
        )
    }

    /// Visits a SinceTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_since(&mut self, _node: &dyn SinceTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_since: Rust: visitor interface method not overridden")
    }

    /// Visits a StartElementTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_start_element(&mut self, _node: &dyn StartElementTree, _p: P) -> R {
        panic!(
            "AbstractMethodError: visit_start_element: Rust: visitor interface method not overridden"
        )
    }

    /// Visits a SummaryTree node.
    ///
    /// @implSpec Visits the provided {@code SummaryTree} node
    /// by calling {@code visitOther(node, p)}.
    ///
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    /// @since 10
    fn visit_summary(&mut self, node: &dyn SummaryTree, p: P) -> R {
        self.visit_other(node, p)
    }

    /// Visits a SystemPropertyTree node.
    ///
    /// @implSpec Visits the provided {@code SystemPropertyTree} node
    /// by calling {@code visitOther(node, p)}.
    ///
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    /// @since 12
    fn visit_system_property(&mut self, node: &dyn SystemPropertyTree, p: P) -> R {
        self.visit_other(node, p)
    }

    /// Visits a TextTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_text(&mut self, _node: &dyn TextTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_text: Rust: visitor interface method not overridden")
    }

    /// Visits a ThrowsTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_throws(&mut self, _node: &dyn ThrowsTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_throws: Rust: visitor interface method not overridden")
    }

    /// Visits an UnknownBlockTagTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_unknown_block_tag(&mut self, _node: &dyn UnknownBlockTagTree, _p: P) -> R {
        panic!(
            "AbstractMethodError: visit_unknown_block_tag: Rust: visitor interface method not overridden"
        )
    }

    /// Visits an UnknownInlineTagTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_unknown_inline_tag(&mut self, _node: &dyn UnknownInlineTagTree, _p: P) -> R {
        panic!(
            "AbstractMethodError: visit_unknown_inline_tag: Rust: visitor interface method not overridden"
        )
    }

    /// Visits a UsesTree node.
    ///
    /// @implSpec Visits a {@code UsesTree} node
    /// by calling {@code visitOther(node, p)}.
    ///
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    ///
    /// @since 9
    fn visit_uses(&mut self, node: &dyn UsesTree, p: P) -> R {
        self.visit_other(node, p)
    }

    /// Visits a ValueTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_value(&mut self, _node: &dyn ValueTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_value: Rust: visitor interface method not overridden")
    }

    /// Visits a VersionTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_version(&mut self, _node: &dyn VersionTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_version: Rust: visitor interface method not overridden")
    }

    /// Visits an unknown type of DocTree node.
    /// This can occur if the set of tags evolves and new kinds
    /// of nodes are added to the {@code DocTree} hierarchy.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_other(&mut self, _node: &dyn DocTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_other: Rust: visitor interface method not overridden")
    }
}

#[cfg(test)]
mod tests {
    use super::DocTreeVisitor;
    use crate::doctree::doc_tree::DocTree;
    use crate::doctree::doc_tree::Kind;
    use crate::doctree::doc_type_tree::DocTypeTree;
    use crate::doctree::identifier_tree::IdentifierTree;
    use javac_strings::JavaString;
    use javac_tools::lang::model::element::Name;
    use std::rc::Rc;

    struct TestName;

    impl Name for TestName {
        fn length(&self) -> usize {
            1
        }
        fn char_at(&self, _index: usize) -> u16 {
            'x' as u16
        }
        fn sub_sequence(&self, _start: usize, _end: usize) -> JavaString {
            JavaString::from("x")
        }
        fn to_java_string(&self) -> JavaString {
            JavaString::from("x")
        }
        fn content_equals(&self, cs: &JavaString) -> bool {
            *cs == JavaString::from("x")
        }
    }

    struct Ident;

    impl DocTree for Ident {
        fn get_kind(&self) -> Kind {
            Kind::IDENTIFIER
        }
        fn accept<R, D>(&self, _visitor: &mut dyn DocTreeVisitor<R, D>, _data: D) -> R {
            panic!("UnsupportedOperationException: accept: test fixture accept")
        }
    }

    impl IdentifierTree for Ident {
        fn get_name(&self) -> Rc<dyn Name> {
            Rc::new(TestName)
        }
    }

    struct DocType;

    impl DocTree for DocType {
        fn get_kind(&self) -> Kind {
            Kind::DOC_TYPE
        }
        fn accept<R, D>(&self, _visitor: &mut dyn DocTreeVisitor<R, D>, _data: D) -> R {
            panic!("UnsupportedOperationException: accept: test fixture accept")
        }
    }

    impl DocTypeTree for DocType {
        fn get_text(&self) -> JavaString {
            JavaString::from("")
        }
    }

    struct CountingVisitor {
        hits: usize,
        other: usize,
    }

    impl DocTreeVisitor<usize, ()> for CountingVisitor {
        fn visit_identifier(&mut self, _node: &dyn IdentifierTree, _p: ()) -> usize {
            self.hits += 1;
            self.hits
        }
        fn visit_other(&mut self, _node: &dyn DocTree, _p: ()) -> usize {
            self.other += 1;
            self.other
        }
    }

    #[test]
    fn visitor_is_object_safe_and_dispatches() {
        let ident = Ident;
        let mut visitor = CountingVisitor { hits: 0, other: 0 };
        let dyn_visitor: &mut dyn DocTreeVisitor<usize, ()> = &mut visitor;
        assert_eq!(dyn_visitor.visit_identifier(&ident, ()), 1);
        assert_eq!(visitor.hits, 1);
    }

    #[test]
    fn default_visit_methods_forward_to_visit_other() {
        // Java: default R visitDocType(DocTypeTree node, P p) { return visitOther(node, p); }
        let doc_type = DocType;
        let mut visitor = CountingVisitor { hits: 0, other: 0 };
        assert_eq!(visitor.visit_doc_type(&doc_type, ()), 1);
        assert_eq!(visitor.other, 1);
    }

    #[test]
    #[should_panic(expected = "not overridden")]
    fn unimplemented_visit_method_panics() {
        struct Empty;
        impl DocTreeVisitor<(), ()> for Empty {}
        let ident = Ident;
        Empty.visit_other(&ident, ());
    }
}
