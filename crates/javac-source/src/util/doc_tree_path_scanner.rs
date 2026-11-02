//! A `DocTreeVisitor` that visits all child doc tree nodes and maintains the
//! path to the parent nodes.
//!
//! Java's `DocTreePathScanner` extends `DocTreeScanner`; Rust has no trait
//! inheritance that routes the default visit methods through an override, so
//! the 36 visit defaults are materialized here and scan their children through
//! this trait. `get_current_path`/`set_current_path` replace the Java private
//! path field, and `scan_path` takes an `Rc<DocTreePath>`.
//! * Java DocTreePathScanner<R,P> extends DocTreeScanner<R,P> is ported as an
//!   independent trait (Rust trait inheritance cannot route the default visit
//!   methods through this trait's scan override); the 36 visit defaults are
//!   mechanically converted from DocTreeScanner with the child scans going
//!   through this trait's scan (owned Rc<dyn DocTree>).
//! * required get_current_path/set_current_path provide the Java private path
//!   field; scan(DocTreePath,P) is scan_path(Rc<DocTreePath>,P); scan(Rc<dyn
//!   DocTree>,P) keeps Java's null check / prev-path exchange; a null tree is
//!   scan_opt; P: Clone (same adaptation as the other scanners).
//!
//! @author Jonathan Gibbons
//! @since 1.8

use crate::doctree::doc_tree::DocTree;
use crate::doctree::doc_tree_visitor::DocTreeVisitor;
use crate::util::dispatch::dispatch_doc_tree_path_scanner;
use crate::util::doc_tree_path::DocTreePath;
use std::rc::Rc;

/// A DocTreeVisitor that visits all the child doc tree nodes, and provides
/// support for maintaining a path for the parent nodes.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @author Jonathan Gibbons
/// @since 1.8
pub trait DocTreePathScanner<R, P: Clone>: DocTreeVisitor<Option<R>, P> {
    /// Returns the current path for the node, as built up by the currently
    /// active set of scan calls.
    fn get_current_path(&self) -> Option<Rc<DocTreePath>>;

    /// Java: this.path = path.
    fn set_current_path(&mut self, path: Option<Rc<DocTreePath>>);

    /// Scans a tree from a position identified by a tree path.
    /// (Java: scan(DocTreePath, P))
    fn scan_path(&mut self, path: Rc<DocTreePath>, p: P) -> Option<R> {
        self.set_current_path(Some(path.clone()));
        let leaf = path.get_leaf();
        let result = dispatch_doc_tree_path_scanner(self, leaf.as_ref(), p);
        self.set_current_path(None);
        result
    }

    /// Scans a single node.
    /// The current path is updated for the duration of the scan.
    /// (Java: scan(DocTree, P))
    fn scan(&mut self, tree: Rc<dyn DocTree>, p: P) -> Option<R> {
        let prev = self.get_current_path();
        let path = match &prev {
            Some(prev) => Rc::new(DocTreePath::new_child(prev.clone(), tree.clone())),
            None => panic!("NullPointerException: DocTreePathScanner.scan without a current path"),
        };
        self.set_current_path(Some(path));
        let result = dispatch_doc_tree_path_scanner(self, tree.as_ref(), p);
        self.set_current_path(prev);
        result
    }

    /// Java: scan(DocTree tree, P p) with a possibly-null tree.
    fn scan_opt(&mut self, tree: Option<Rc<dyn DocTree>>, p: P) -> Option<R> {
        match tree {
            Some(tree) => self.scan(tree, p),
            None => None,
        }
    }

    /// Scans a sequence of nodes.
    fn scan_all(&mut self, nodes: &[Rc<dyn DocTree>], p: P) -> Option<R> {
        let mut r: Option<R> = None;
        let mut first = true;
        for node in nodes {
            r = if first {
                self.scan(node.clone(), p.clone())
            } else {
                self.scan_and_reduce(node.clone(), p.clone(), r)
            };
            first = false;
        }
        r
    }

    /// Java: private R scanAndReduce(DocTree node, P p, R r).
    fn scan_and_reduce(&mut self, node: Rc<dyn DocTree>, p: P, r: Option<R>) -> Option<R> {
        let scanned = self.scan(node, p);
        self.reduce(scanned, r)
    }

    /// Reduces two results into a combined result (Java: returns the first).
    fn reduce(&mut self, r1: Option<R>, _r2: Option<R>) -> Option<R> {
        r1
    }

    fn visit_attribute(
        &mut self,
        node: &dyn crate::doctree::attribute_tree::AttributeTree,
        p: P,
    ) -> Option<R> {
        match node.get_value() {
            Some(nodes) => {
                let mut r: Option<R> = None;
                for child in nodes {
                    let child: Rc<dyn DocTree> = child;
                    r = self.scan_and_reduce(child, p.clone(), r);
                }
                r
            }
            None => None,
        }
    }

    fn visit_author(
        &mut self,
        node: &dyn crate::doctree::author_tree::AuthorTree,
        p: P,
    ) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_name() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_comment(
        &mut self,
        _node: &dyn crate::doctree::comment_tree::CommentTree,
        _p: P,
    ) -> Option<R> {
        None
    }

    fn visit_deprecated(
        &mut self,
        node: &dyn crate::doctree::deprecated_tree::DeprecatedTree,
        p: P,
    ) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_body() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_doc_comment(
        &mut self,
        node: &dyn crate::doctree::doc_comment_tree::DocCommentTree,
        p: P,
    ) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_first_sentence() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        for child in node.get_body() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        for child in node.get_block_tags() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_doc_root(
        &mut self,
        _node: &dyn crate::doctree::doc_root_tree::DocRootTree,
        _p: P,
    ) -> Option<R> {
        None
    }

    fn visit_doc_type(
        &mut self,
        _node: &dyn crate::doctree::doc_type_tree::DocTypeTree,
        _p: P,
    ) -> Option<R> {
        None
    }

    fn visit_end_element(
        &mut self,
        _node: &dyn crate::doctree::end_element_tree::EndElementTree,
        _p: P,
    ) -> Option<R> {
        None
    }

    fn visit_entity(
        &mut self,
        _node: &dyn crate::doctree::entity_tree::EntityTree,
        _p: P,
    ) -> Option<R> {
        None
    }

    fn visit_erroneous(
        &mut self,
        _node: &dyn crate::doctree::erroneous_tree::ErroneousTree,
        _p: P,
    ) -> Option<R> {
        None
    }

    fn visit_hidden(
        &mut self,
        node: &dyn crate::doctree::hidden_tree::HiddenTree,
        p: P,
    ) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_body() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_identifier(
        &mut self,
        _node: &dyn crate::doctree::identifier_tree::IdentifierTree,
        _p: P,
    ) -> Option<R> {
        None
    }

    fn visit_index(&mut self, node: &dyn crate::doctree::index_tree::IndexTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_search_term(), p.clone());
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_inherit_doc(
        &mut self,
        _node: &dyn crate::doctree::inherit_doc_tree::InheritDocTree,
        _p: P,
    ) -> Option<R> {
        None
    }

    fn visit_link(&mut self, node: &dyn crate::doctree::link_tree::LinkTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_reference(), p.clone());
        for child in node.get_label() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_literal(
        &mut self,
        node: &dyn crate::doctree::literal_tree::LiteralTree,
        p: P,
    ) -> Option<R> {
        self.scan(node.get_body(), p.clone())
    }

    fn visit_param(&mut self, node: &dyn crate::doctree::param_tree::ParamTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_name(), p.clone());
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_provides(
        &mut self,
        node: &dyn crate::doctree::provides_tree::ProvidesTree,
        p: P,
    ) -> Option<R> {
        let mut r = self.scan(node.get_service_type(), p.clone());
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_reference(
        &mut self,
        _node: &dyn crate::doctree::reference_tree::ReferenceTree,
        _p: P,
    ) -> Option<R> {
        None
    }

    fn visit_return(
        &mut self,
        node: &dyn crate::doctree::return_tree::ReturnTree,
        p: P,
    ) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_see(&mut self, node: &dyn crate::doctree::see_tree::SeeTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_reference() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_serial(
        &mut self,
        node: &dyn crate::doctree::serial_tree::SerialTree,
        p: P,
    ) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_serial_data(
        &mut self,
        node: &dyn crate::doctree::serial_data_tree::SerialDataTree,
        p: P,
    ) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_serial_field(
        &mut self,
        node: &dyn crate::doctree::serial_field_tree::SerialFieldTree,
        p: P,
    ) -> Option<R> {
        let mut r = self.scan(node.get_name(), p.clone());
        r = self.scan_and_reduce(node.get_type(), p.clone(), r);
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_since(&mut self, node: &dyn crate::doctree::since_tree::SinceTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_body() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_start_element(
        &mut self,
        node: &dyn crate::doctree::start_element_tree::StartElementTree,
        p: P,
    ) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_attributes() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_summary(
        &mut self,
        node: &dyn crate::doctree::summary_tree::SummaryTree,
        p: P,
    ) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_summary() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_system_property(
        &mut self,
        _node: &dyn crate::doctree::system_property_tree::SystemPropertyTree,
        _p: P,
    ) -> Option<R> {
        None
    }

    fn visit_text(&mut self, _node: &dyn crate::doctree::text_tree::TextTree, _p: P) -> Option<R> {
        None
    }

    fn visit_throws(
        &mut self,
        node: &dyn crate::doctree::throws_tree::ThrowsTree,
        p: P,
    ) -> Option<R> {
        let mut r = self.scan(node.get_exception_name(), p.clone());
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_unknown_block_tag(
        &mut self,
        node: &dyn crate::doctree::unknown_block_tag_tree::UnknownBlockTagTree,
        p: P,
    ) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_content() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_unknown_inline_tag(
        &mut self,
        node: &dyn crate::doctree::unknown_inline_tag_tree::UnknownInlineTagTree,
        p: P,
    ) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_content() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_uses(&mut self, node: &dyn crate::doctree::uses_tree::UsesTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_service_type(), p.clone());
        for child in node.get_description() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_value(&mut self, node: &dyn crate::doctree::value_tree::ValueTree, p: P) -> Option<R> {
        self.scan(node.get_reference(), p.clone())
    }

    fn visit_version(
        &mut self,
        node: &dyn crate::doctree::version_tree::VersionTree,
        p: P,
    ) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_body() {
            let child: Rc<dyn DocTree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_other(&mut self, _node: &dyn DocTree, _p: P) -> Option<R> {
        None
    }
}
