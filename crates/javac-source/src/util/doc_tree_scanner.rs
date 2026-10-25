// Origin: com/sun/source/util/DocTreeScanner.java @ 982d53c (jdk-17.0.19-ga)

//! A DocTreeVisitor that visits all the child doc tree nodes.
//!
//! PORTING_RULES (§14.7): Java class DocTreeScanner<R,P> implements
//! DocTreeVisitor<R,P> -> Rust trait DocTreeScanner<R, P: Clone>:
//! DocTreeVisitor<Option<R>, P>; nullable R -> Option<R>; scan(DocTree, P)
//! double dispatch goes through crate::util::dispatch::dispatch_doc_tree_scanner
//! (the Java accept method is generic); a null tree -> scan_opt; the Java
//! scan(Iterable) -> scan_all.  The 36 visit default methods are mechanically
//! generated from the Java source (children in left-to-right order; the only
//! nullable list, AttributeTree.getValue, is ported explicitly).
//!
//! @author Jonathan Gibbons
//! @since 1.8

use crate::doctree::doc_tree::DocTree;
use crate::doctree::doc_tree_visitor::DocTreeVisitor;
use crate::util::dispatch::dispatch_doc_tree_scanner;
use std::rc::Rc;

/// A DocTreeVisitor that visits all the child doc tree nodes.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @author Jonathan Gibbons
/// @since 1.8
pub trait DocTreeScanner<R, P: Clone>: DocTreeVisitor<Option<R>, P> {
    /// Scans a single node.
    fn scan(&mut self, tree: &dyn DocTree, p: P) -> Option<R> {
        dispatch_doc_tree_scanner(self, tree, p)
    }

    /// Java: scan(DocTree tree, P p) for a possibly-null tree.
    fn scan_opt(&mut self, tree: Option<Rc<dyn DocTree>>, p: P) -> Option<R> {
        match tree {
            Some(tree) => self.scan(tree.as_ref(), p),
            None => None,
        }
    }

    /// Scans a sequence of nodes.
    fn scan_all(&mut self, nodes: &[Rc<dyn DocTree>], p: P) -> Option<R> {
        let mut r: Option<R> = None;
        let mut first = true;
        for node in nodes {
            r = if first { self.scan(node.as_ref(), p.clone()) } else { self.scan_and_reduce(node.as_ref(), p.clone(), r) };
            first = false;
        }
        r
    }

    /// Java: private R scanAndReduce(DocTree node, P p, R r).
    fn scan_and_reduce(&mut self, node: &dyn DocTree, p: P, r: Option<R>) -> Option<R> {
        let scanned = self.scan(node, p);
        self.reduce(scanned, r)
    }

    /// Reduces two results into a combined result (Java: returns the first).
    fn reduce(&mut self, r1: Option<R>, _r2: Option<R>) -> Option<R> {
        r1
    }

    fn visit_attribute(&mut self, node: &dyn crate::doctree::attribute_tree::AttributeTree, p: P) -> Option<R> {
        return match node.get_value() { Some(nodes) => { let mut r: Option<R> = None; for child in nodes { let child: Rc<dyn DocTree> = child; r = self.scan_and_reduce(child.as_ref(), p.clone(), r); } r }, None => None };
    }

    fn visit_author(&mut self, node: &dyn crate::doctree::author_tree::AuthorTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_name() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_comment(&mut self, _node: &dyn crate::doctree::comment_tree::CommentTree, _p: P) -> Option<R> {
        None
    }

    fn visit_deprecated(&mut self, node: &dyn crate::doctree::deprecated_tree::DeprecatedTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_body() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_doc_comment(&mut self, node: &dyn crate::doctree::doc_comment_tree::DocCommentTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_first_sentence() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        for child in node.get_body() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        for child in node.get_block_tags() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_doc_root(&mut self, _node: &dyn crate::doctree::doc_root_tree::DocRootTree, _p: P) -> Option<R> {
        None
    }

    fn visit_doc_type(&mut self, _node: &dyn crate::doctree::doc_type_tree::DocTypeTree, _p: P) -> Option<R> {
        None
    }

    fn visit_end_element(&mut self, _node: &dyn crate::doctree::end_element_tree::EndElementTree, _p: P) -> Option<R> {
        None
    }

    fn visit_entity(&mut self, _node: &dyn crate::doctree::entity_tree::EntityTree, _p: P) -> Option<R> {
        None
    }

    fn visit_erroneous(&mut self, _node: &dyn crate::doctree::erroneous_tree::ErroneousTree, _p: P) -> Option<R> {
        None
    }

    fn visit_hidden(&mut self, node: &dyn crate::doctree::hidden_tree::HiddenTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_body() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_identifier(&mut self, _node: &dyn crate::doctree::identifier_tree::IdentifierTree, _p: P) -> Option<R> {
        None
    }

    fn visit_index(&mut self, node: &dyn crate::doctree::index_tree::IndexTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_search_term().as_ref(), p.clone());
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_inherit_doc(&mut self, _node: &dyn crate::doctree::inherit_doc_tree::InheritDocTree, _p: P) -> Option<R> {
        None
    }

    fn visit_link(&mut self, node: &dyn crate::doctree::link_tree::LinkTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_reference().as_ref(), p.clone());
        for child in node.get_label() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_literal(&mut self, node: &dyn crate::doctree::literal_tree::LiteralTree, p: P) -> Option<R> {
        return self.scan(node.get_body().as_ref(), p.clone());
    }

    fn visit_param(&mut self, node: &dyn crate::doctree::param_tree::ParamTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_name().as_ref(), p.clone());
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_provides(&mut self, node: &dyn crate::doctree::provides_tree::ProvidesTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_service_type().as_ref(), p.clone());
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_reference(&mut self, _node: &dyn crate::doctree::reference_tree::ReferenceTree, _p: P) -> Option<R> {
        None
    }

    fn visit_return(&mut self, node: &dyn crate::doctree::return_tree::ReturnTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_see(&mut self, node: &dyn crate::doctree::see_tree::SeeTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_reference() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_serial(&mut self, node: &dyn crate::doctree::serial_tree::SerialTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_serial_data(&mut self, node: &dyn crate::doctree::serial_data_tree::SerialDataTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_serial_field(&mut self, node: &dyn crate::doctree::serial_field_tree::SerialFieldTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_name().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_type().as_ref(), p.clone(), r);
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_since(&mut self, node: &dyn crate::doctree::since_tree::SinceTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_body() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_start_element(&mut self, node: &dyn crate::doctree::start_element_tree::StartElementTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_attributes() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_summary(&mut self, node: &dyn crate::doctree::summary_tree::SummaryTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_summary() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_system_property(&mut self, _node: &dyn crate::doctree::system_property_tree::SystemPropertyTree, _p: P) -> Option<R> {
        None
    }

    fn visit_text(&mut self, _node: &dyn crate::doctree::text_tree::TextTree, _p: P) -> Option<R> {
        None
    }

    fn visit_throws(&mut self, node: &dyn crate::doctree::throws_tree::ThrowsTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_exception_name().as_ref(), p.clone());
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_unknown_block_tag(&mut self, node: &dyn crate::doctree::unknown_block_tag_tree::UnknownBlockTagTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_content() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_unknown_inline_tag(&mut self, node: &dyn crate::doctree::unknown_inline_tag_tree::UnknownInlineTagTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_content() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_uses(&mut self, node: &dyn crate::doctree::uses_tree::UsesTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_service_type().as_ref(), p.clone());
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_value(&mut self, node: &dyn crate::doctree::value_tree::ValueTree, p: P) -> Option<R> {
        return self.scan(node.get_reference().as_ref(), p.clone());
    }

    fn visit_version(&mut self, node: &dyn crate::doctree::version_tree::VersionTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_body() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_other(&mut self, _node: &dyn crate::doctree::doc_tree::DocTree, _p: P) -> Option<R> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::DocTreeScanner;
    use crate::doctree::doc_comment_tree::DocCommentTree;
    use crate::doctree::doc_tree::DocTree;
    use crate::doctree::doc_tree::Kind;
    use crate::doctree::doc_tree_visitor::DocTreeVisitor;
    use crate::doctree::text_tree::TextTree;
    use crate::util::test_support::{DocCommentLeaf, DocLeaf};
    use std::rc::Rc;

    struct CountingScanner {
        count: usize,
    }

    impl DocTreeVisitor<Option<usize>, ()> for CountingScanner {}

    impl DocTreeScanner<usize, ()> for CountingScanner {
        fn visit_text(&mut self, _node: &dyn TextTree, _p: ()) -> Option<usize> {
            self.count += 1;
            Some(self.count)
        }
    }

    #[test]
    fn scan_dispatches_through_visit_methods() {
        let leaf = DocLeaf;
        let mut scanner = CountingScanner { count: 0 };
        assert_eq!(scanner.scan(&leaf, ()), Some(1));
        assert_eq!(scanner.count, 1);
    }

    #[test]
    fn default_doc_comment_visit_scans_children() {
        // DocCommentLeaf has no children -> the default visitDocComment scans
        // them all and returns None (Java null).
        let doc: Rc<dyn DocCommentTree> = Rc::new(DocCommentLeaf);
        let mut scanner = CountingScanner { count: 0 };
        assert_eq!(scanner.scan(doc.as_ref(), ()), None);
        assert_eq!(scanner.count, 0);
    }

    #[test]
    fn scan_opt_handles_null_trees() {
        let mut scanner = CountingScanner { count: 0 };
        assert_eq!(scanner.scan_opt(None, ()), None);
        let leaf: Rc<dyn DocTree> = Rc::new(DocLeaf);
        let kind = leaf.get_kind();
        assert_eq!(scanner.scan_opt(Some(leaf), ()), Some(1));
        assert_eq!(kind, Kind::TEXT);
    }
}
