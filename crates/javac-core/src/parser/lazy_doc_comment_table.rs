//! A table giving the doc comment, if any, for any tree node, parsing the doc
//! comment on demand.
//!
//! PORTING_RULES: adapter notes:
//! - Java JCTree keys -> arena TreeId (ADR 0001); the table is a HashMap (no
//!   iteration order is observable in this class);
//! - Entry.tree is a lazily computed cache; the upstream non-final field is a
//!   RefCell<Option<Rc<DCTree>>> (registered RefCell whitelist: local single
//!   threaded cache, PORTING_RULES section 9);
//! - upstream LazyDocCommentTable(ParserFactory fac) reads fac.log.currentSource()
//!   and constructs DocCommentParser(fac, diagSource, comment). ParserFactory is
//!   ported; the same dependencies are injected:
//!   Names, Rc<RefCell<DocTreeMaker>> and the DiagnosticSource that the caller
//!   took from Log.currentSource() (ParserFactory is ported and injected);
//! - getCommentTree(JCTree) returns DCDocComment upstream; the port returns
//!   Rc<DCTree> with DCKind::DocComment (single node type, see tree/dc_tree.rs).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ids::TreeId;
use crate::parser::doc_comment_parser::DocCommentParser;
use crate::parser::tokens::Comment;
use crate::tree::dc_tree::DCTree;
use crate::tree::doc_comment_table::DocCommentTable;
use crate::tree::doc_tree_maker::DocTreeMaker;
use crate::util::diagnostic_source::DiagnosticSource;
use crate::util::java_string::JavaString;
use crate::util::names::Names;

struct Entry {
    /// Java: final Comment comment
    comment: Rc<dyn Comment>,
    /// Java: DCDocComment tree (lazily parsed)
    tree: RefCell<Option<Rc<DCTree>>>,
}

impl Entry {
    fn new(comment: Rc<dyn Comment>) -> Entry {
        Entry {
            comment,
            tree: RefCell::new(None),
        }
    }
}

/// A table giving the doc comment, if any, for any tree node, parsed lazily.
pub struct LazyDocCommentTable {
    /// Java: private final ParserFactory fac
    fac: Rc<crate::parser::parser_factory::ParserFactory>,
    /// Java: private final Names names (injected)
    names: Rc<Names>,
    doc_tree_maker: Rc<RefCell<DocTreeMaker>>,
    /// Java: private final DiagnosticSource diagSource = fac.log.currentSource()
    diag_source: Option<Rc<DiagnosticSource>>,
    /// Java: private final Map<JCTree, Entry> table
    table: HashMap<TreeId, Entry>,
}

impl LazyDocCommentTable {
    /// Creates the lazy table.
    ///
    /// PORTING_RULES: the upstream constructor takes ParserFactory and reads
    /// log.currentSource(); the port receives the same factory and dependencies.
    pub fn new(
        fac: Rc<crate::parser::parser_factory::ParserFactory>,
        names: Rc<Names>,
        doc_tree_maker: Rc<RefCell<DocTreeMaker>>,
        diag_source: Option<Rc<DiagnosticSource>>,
    ) -> LazyDocCommentTable {
        LazyDocCommentTable {
            fac,
            names,
            doc_tree_maker,
            diag_source,
            table: HashMap::new(),
        }
    }

    fn compute_tree(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        entry: &Entry,
    ) -> Rc<DCTree> {
        DocCommentParser::new(
            Rc::clone(&self.fac),
            Rc::clone(&self.names),
            Rc::clone(&self.doc_tree_maker),
            self.diag_source.clone(),
            Rc::clone(&entry.comment),
            false,
        )
        .parse(ctx)
    }
}

impl DocCommentTable for LazyDocCommentTable {
    fn has_comment(&self, tree: TreeId) -> bool {
        self.table.contains_key(&tree)
    }

    fn get_comment(&self, tree: TreeId) -> Option<Rc<dyn Comment>> {
        self.table.get(&tree).map(|entry| Rc::clone(&entry.comment))
    }

    fn get_comment_text(&self, tree: TreeId) -> Option<JavaString> {
        self.get_comment(tree)
            .and_then(|comment| comment.get_text())
    }

    fn get_comment_tree(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: TreeId,
    ) -> Option<Rc<DCTree>> {
        let entry = self.table.get(&tree)?;
        if let Some(tree) = entry.tree.borrow().as_ref() {
            return Some(Rc::clone(tree));
        }
        let parsed = self.compute_tree(ctx, entry);
        *entry.tree.borrow_mut() = Some(Rc::clone(&parsed));
        Some(parsed)
    }

    fn put_comment(&mut self, tree: TreeId, comment: Rc<dyn Comment>) {
        self.table.insert(tree, Entry::new(comment));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::tokens::CommentStyle;
    use crate::util::java_string::JavaString;
    use crate::util::jc_diagnostic::Factory;
    use crate::util::javac_messages::{DEFAULT_BUNDLE_NAME, JavacMessages};
    use crate::util::list::List;
    use std::cell::RefCell;

    struct TestComment {
        text: JavaString,
    }

    impl Comment for TestComment {
        fn get_text(&self) -> Option<JavaString> {
            Some(self.text.clone())
        }

        fn get_source_pos(&self, index: i32) -> i32 {
            index
        }

        fn get_style(&self) -> CommentStyle {
            CommentStyle::JAVADOC
        }

        fn is_deprecated(&self) -> bool {
            false
        }
    }

    fn factory() -> Rc<crate::parser::parser_factory::ParserFactory> {
        let mut context = crate::util::context::Context::new();
        let names = Rc::new(Names::new(false));
        let source = crate::code::source::Source::JDK17;
        let scanner_factory = crate::parser::scanner_factory::ScannerFactory::new(
            &mut context,
            Rc::clone(&names),
            source,
        );
        Rc::new(crate::parser::parser_factory::ParserFactory::new(
            &mut context,
            names,
            source,
            scanner_factory,
            None,
        ))
    }

    fn setup() -> (LazyDocCommentTable, TreeId) {
        let fac = factory();
        let names = Rc::clone(&fac.names);
        let messages = Rc::new(JavacMessages::new(DEFAULT_BUNDLE_NAME));
        let maker = DocTreeMaker::new(Rc::new(Factory::new(messages, "compiler")));
        let table = LazyDocCommentTable::new(
            fac,
            names,
            Rc::new(RefCell::new(maker)),
            None,
        );
        (table, TreeId::new(1))
    }

    fn comment(text: &str) -> Rc<dyn Comment> {
        Rc::new(TestComment {
            text: JavaString::from(text),
        })
    }

    #[test]
    fn missing_tree_has_nothing() {
        let (table, tree) = setup();
        assert!(!table.has_comment(tree));
        assert!(table.get_comment(tree).is_none());
        assert!(table.get_comment_text(tree).is_none());
        assert!(table.get_comment_tree(&mut crate::compiler_context::CompilerContext::new(), tree).is_none());
    }

    #[test]
    fn comment_text_is_available_without_parsing() {
        let (mut table, tree) = setup();
        table.put_comment(tree, comment("Hello world."));
        assert!(table.has_comment(tree));
        assert_eq!(
            table.get_comment_text(tree),
            Some(JavaString::from("Hello world."))
        );
        // The parsed tree has not been requested yet; parsing is lazy and is
        // triggered only by get_comment_tree (covered in doc_comment_parser
        // tests, which own the parser).
    }

    #[test]
    fn put_comment_replaces_the_entry() {
        let (mut table, tree) = setup();
        table.put_comment(tree, comment("first"));
        table.put_comment(tree, comment("second"));
        assert_eq!(
            table.get_comment_text(tree),
            Some(JavaString::from("second"))
        );
    }

    #[test]
    fn comment_style_and_text_round_trip() {
        let comment = comment("doc");
        assert_eq!(comment.get_style(), CommentStyle::JAVADOC);
        assert!(!comment.is_deprecated());
        assert_eq!(comment.get_text(), Some(JavaString::from("doc")));
        // The list type of parsed trees is the ported immutable list.
        let empty: List<Rc<DCTree>> = List::nil();
        assert!(empty.is_empty());
    }
}
