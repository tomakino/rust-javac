// PORTING ADAPTER (not a 1:1 port): test fixtures for the com.sun.source.util
// tests (compiled only under #[cfg(test)]).
#![allow(dead_code)]

use crate::doctree::doc_comment_tree::DocCommentTree;
use crate::doctree::doc_tree::{DocTree, Kind as DocKind};
use crate::doctree::doc_tree_visitor::DocTreeVisitor;
use crate::tree::compilation_unit_tree::CompilationUnitTree;
use crate::tree::identifier_tree::IdentifierTree;
use crate::tree::tree::{Kind, Tree};
use crate::tree::tree_visitor::TreeVisitor;
use javac_strings::JavaString;
use javac_tools::lang::model::element::Name;
use std::rc::Rc;

pub struct TestName;

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

pub struct IdentTree;

impl Tree for IdentTree {
    fn get_kind(&self) -> Kind {
        Kind::IDENTIFIER
    }
    fn accept<R, D>(&self, _visitor: &mut dyn TreeVisitor<R, D>, _data: D) -> R {
        panic!("UnsupportedOperationException: accept: test fixture accept")
    }
    fn as_identifier_tree(&self) -> Option<&dyn IdentifierTree> {
        Some(self)
    }
}

impl crate::tree::expression_tree::ExpressionTree for IdentTree {}
impl crate::tree::case_label_tree::CaseLabelTree for IdentTree {}

impl IdentifierTree for IdentTree {
    fn get_name(&self) -> Rc<dyn Name> {
        Rc::new(TestName)
    }
}

pub struct UnitTree;

impl Tree for UnitTree {
    fn get_kind(&self) -> Kind {
        Kind::COMPILATION_UNIT
    }
    fn accept<R, D>(&self, _visitor: &mut dyn TreeVisitor<R, D>, _data: D) -> R {
        panic!("UnsupportedOperationException: accept: test fixture accept")
    }
    fn as_compilation_unit_tree(&self) -> Option<&dyn CompilationUnitTree> {
        Some(self)
    }
}

impl CompilationUnitTree for UnitTree {
    fn get_package_annotations(
        &self,
    ) -> Option<Vec<Rc<dyn crate::tree::annotation_tree::AnnotationTree>>> {
        None
    }
    fn get_package_name(&self) -> Option<Rc<dyn crate::tree::expression_tree::ExpressionTree>> {
        None
    }
    fn get_package(&self) -> Option<Rc<dyn crate::tree::package_tree::PackageTree>> {
        None
    }
    fn get_imports(&self) -> Vec<Rc<dyn crate::tree::import_tree::ImportTree>> {
        Vec::new()
    }
    fn get_type_decls(&self) -> Vec<Rc<dyn Tree>> {
        Vec::new()
    }
    fn get_source_file(&self) -> Rc<dyn javac_tools::tools::java_file_object::JavaFileObject> {
        Rc::new(TestJavaFileObject)
    }
    fn get_line_map(&self) -> Option<Rc<dyn crate::tree::line_map::LineMap>> {
        None
    }
}

pub struct DocLeaf;

impl DocTree for DocLeaf {
    fn get_kind(&self) -> DocKind {
        DocKind::TEXT
    }
    fn accept<R, D>(&self, _visitor: &mut dyn DocTreeVisitor<R, D>, _data: D) -> R {
        panic!("UnsupportedOperationException: accept: test fixture accept")
    }
    fn as_text_tree(&self) -> Option<&dyn crate::doctree::text_tree::TextTree> {
        Some(self as &dyn crate::doctree::text_tree::TextTree)
    }
}

pub struct DocCommentLeaf;

impl DocTree for DocCommentLeaf {
    fn get_kind(&self) -> DocKind {
        DocKind::DOC_COMMENT
    }
    fn accept<R, D>(&self, _visitor: &mut dyn DocTreeVisitor<R, D>, _data: D) -> R {
        panic!("UnsupportedOperationException: accept: test fixture accept")
    }
    fn as_doc_comment_tree(&self) -> Option<&dyn DocCommentTree> {
        Some(self)
    }
}

impl crate::doctree::text_tree::TextTree for DocLeaf {
    fn get_body(&self) -> JavaString {
        JavaString::from("text")
    }
}

impl DocCommentTree for DocCommentLeaf {
    fn get_first_sentence(&self) -> Vec<Rc<dyn DocTree>> {
        Vec::new()
    }
    fn get_body(&self) -> Vec<Rc<dyn DocTree>> {
        Vec::new()
    }
    fn get_block_tags(&self) -> Vec<Rc<dyn DocTree>> {
        Vec::new()
    }
}

/// A SourcePositions returning NOPOS (-1).
pub struct NoPositions;

impl crate::util::source_positions::SourcePositions for NoPositions {
    fn get_start_position(
        &self,
        _file: &dyn CompilationUnitTree,
        _tree: &dyn Tree,
    ) -> i64 {
        -1
    }
    fn get_end_position(&self, _file: &dyn CompilationUnitTree, _tree: &dyn Tree) -> i64 {
        -1
    }
}

/// A DocSourcePositions returning NOPOS (-1).
pub struct DocNoPositions;

impl crate::util::source_positions::SourcePositions for DocNoPositions {
    fn get_start_position(
        &self,
        _file: &dyn CompilationUnitTree,
        _tree: &dyn Tree,
    ) -> i64 {
        -1
    }
    fn get_end_position(&self, _file: &dyn CompilationUnitTree, _tree: &dyn Tree) -> i64 {
        -1
    }
}

impl crate::util::doc_source_positions::DocSourcePositions for DocNoPositions {
    fn get_start_position(
        &self,
        _file: &dyn CompilationUnitTree,
        _comment: &dyn DocCommentTree,
        _tree: &dyn DocTree,
    ) -> i64 {
        -1
    }
    fn get_end_position(
        &self,
        _file: &dyn CompilationUnitTree,
        _comment: &dyn DocCommentTree,
        _tree: &dyn DocTree,
    ) -> i64 {
        -1
    }
}

/// A minimal JavaFileObject used by the TaskEvent tests.
pub struct TestJavaFileObject;

impl javac_tools::tools::file_object::FileObject for TestJavaFileObject {
    fn to_uri(&self) -> javac_tools::uri::URI {
        panic!("UnsupportedOperationException: to_uri: test fixture has no uri")
    }
    fn get_name(&self) -> JavaString {
        JavaString::from("Test.java")
    }
    fn open_input_stream(&self) -> Result<Box<dyn std::io::Read>, javac_tools::error::Error> {
        panic!("UnsupportedOperationException: open_input_stream: test fixture")
    }
    fn open_output_stream(&self) -> Result<Box<dyn std::io::Write>, javac_tools::error::Error> {
        panic!("UnsupportedOperationException: open_output_stream: test fixture")
    }
    fn open_reader(
        &self,
        _ignore_encoding_errors: bool,
    ) -> Result<Box<dyn javac_tools::java_io::Reader>, javac_tools::error::Error> {
        panic!("UnsupportedOperationException: open_reader: test fixture")
    }
    fn get_char_content(
        &self,
        _ignore_encoding_errors: bool,
    ) -> Result<Option<JavaString>, javac_tools::error::Error> {
        Ok(None)
    }
    fn open_writer(&self) -> Result<Box<dyn javac_tools::java_io::Writer>, javac_tools::error::Error> {
        panic!("UnsupportedOperationException: open_writer: test fixture")
    }
    fn get_last_modified(&self) -> i64 {
        0
    }
    fn delete(&self) -> bool {
        false
    }
}

impl javac_tools::tools::java_file_object::JavaFileObject for TestJavaFileObject {
    fn get_kind(&self) -> javac_tools::tools::java_file_object::Kind {
        javac_tools::tools::java_file_object::Kind::SOURCE
    }
    fn is_name_compatible(
        &self,
        _simple_name: &JavaString,
        _kind: javac_tools::tools::java_file_object::Kind,
    ) -> bool {
        false
    }
    fn get_nesting_kind(&self) -> Option<javac_tools::lang::model::element::NestingKind> {
        None
    }
    fn get_access_level(&self) -> Option<javac_tools::lang::model::element::Modifier> {
        None
    }
}

/// A binary tree node fixture (kind BINARY) with two identifier children.
pub struct BinaryTreeFixture {
    pub left: Rc<dyn crate::tree::expression_tree::ExpressionTree>,
    pub right: Rc<dyn crate::tree::expression_tree::ExpressionTree>,
}

impl Tree for BinaryTreeFixture {
    fn get_kind(&self) -> Kind {
        Kind::MULTIPLY
    }
    fn accept<R, D>(&self, _visitor: &mut dyn TreeVisitor<R, D>, _data: D) -> R {
        panic!("UnsupportedOperationException: accept: test fixture accept")
    }
    fn as_binary_tree(&self) -> Option<&dyn crate::tree::binary_tree::BinaryTree> {
        Some(self)
    }
}

impl crate::tree::case_label_tree::CaseLabelTree for BinaryTreeFixture {}
impl crate::tree::expression_tree::ExpressionTree for BinaryTreeFixture {}

impl crate::tree::binary_tree::BinaryTree for BinaryTreeFixture {
    fn get_left_operand(&self) -> Rc<dyn crate::tree::expression_tree::ExpressionTree> {
        self.left.clone()
    }
    fn get_right_operand(&self) -> Rc<dyn crate::tree::expression_tree::ExpressionTree> {
        self.right.clone()
    }
}

/// A CompilationUnitTree fixture with type declarations (for TreePath searches).
pub struct UnitTreeWithDecls {
    pub decls: Vec<Rc<dyn Tree>>,
}

impl Tree for UnitTreeWithDecls {
    fn get_kind(&self) -> Kind {
        Kind::COMPILATION_UNIT
    }
    fn accept<R, D>(&self, _visitor: &mut dyn TreeVisitor<R, D>, _data: D) -> R {
        panic!("UnsupportedOperationException: accept: test fixture accept")
    }
    fn as_compilation_unit_tree(&self) -> Option<&dyn CompilationUnitTree> {
        Some(self)
    }
}

impl CompilationUnitTree for UnitTreeWithDecls {
    fn get_module(&self) -> Option<Rc<dyn crate::tree::module_tree::ModuleTree>> {
        None
    }
    fn get_package_annotations(
        &self,
    ) -> Option<Vec<Rc<dyn crate::tree::annotation_tree::AnnotationTree>>> {
        None
    }
    fn get_package_name(&self) -> Option<Rc<dyn crate::tree::expression_tree::ExpressionTree>> {
        None
    }
    fn get_package(&self) -> Option<Rc<dyn crate::tree::package_tree::PackageTree>> {
        None
    }
    fn get_imports(&self) -> Vec<Rc<dyn crate::tree::import_tree::ImportTree>> {
        Vec::new()
    }
    fn get_type_decls(&self) -> Vec<Rc<dyn Tree>> {
        self.decls.clone()
    }
    fn get_source_file(&self) -> Rc<dyn javac_tools::tools::java_file_object::JavaFileObject> {
        Rc::new(TestJavaFileObject)
    }
    fn get_line_map(&self) -> Option<Rc<dyn crate::tree::line_map::LineMap>> {
        None
    }
}

/// A DocCommentTree fixture with explicit body children (for DocTreePath tests).
pub struct DocCommentWithBody {
    pub body: Vec<Rc<dyn crate::doctree::doc_tree::DocTree>>,
}

impl crate::doctree::doc_tree::DocTree for DocCommentWithBody {
    fn get_kind(&self) -> crate::doctree::doc_tree::Kind {
        crate::doctree::doc_tree::Kind::DOC_COMMENT
    }
    fn accept<R, D>(
        &self,
        _visitor: &mut dyn crate::doctree::doc_tree_visitor::DocTreeVisitor<R, D>,
        _data: D,
    ) -> R {
        panic!("UnsupportedOperationException: accept: test fixture accept")
    }
    fn as_doc_comment_tree(&self) -> Option<&dyn DocCommentTree> {
        Some(self)
    }
}

impl DocCommentTree for DocCommentWithBody {
    fn get_first_sentence(&self) -> Vec<Rc<dyn crate::doctree::doc_tree::DocTree>> {
        Vec::new()
    }
    fn get_body(&self) -> Vec<Rc<dyn crate::doctree::doc_tree::DocTree>> {
        self.body.clone()
    }
    fn get_block_tags(&self) -> Vec<Rc<dyn crate::doctree::doc_tree::DocTree>> {
        Vec::new()
    }
}
