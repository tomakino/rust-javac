//! Factory for creating {@code DocTree} nodes.
//!
//! (§12/§14):
//! * Java interface -> Rust trait; the implementation is provided by the P6
//!   API layer (the DocTreeMaker behind com.sun.tools.javac.api.JavacTrees).
//! * a single `DocTree`/`Name`/... parameter becomes `Rc<dyn ...>` so that an
//!   implementation can embed the received node in the tree it creates (Java
//!   stores the reference as-is); `List<? extends DocTree>` parameters ->
//!   `&[Rc<dyn DocTree>]`, `List<DocTree>` results ->
//!   `Vec<Rc<dyn DocTree>>`; `String` -> `&JavaString`; `int pos` -> `i32`.
//! * Java overloads become distinct Rust names (documented per method):
//!   `newDocCommentTree(List, List)` -> `new_doc_comment_tree`,
//!   `newDocCommentTree(List, List, List, List)` ->
//!   `new_doc_comment_tree_with_preamble`, `newReturnTree(List)` ->
//!   `new_return_tree`, `newReturnTree(boolean, List)` ->
//!   `new_return_tree_with_inline`.
//! * the Java `default` methods keep their Java behaviour as Rust default
//!   methods: `newReturnTree(boolean, List)` panics with
//!   UnsupportedOperationException when `isInline`, and delegates to
//!   `newReturnTree(List)` otherwise; `newSummaryTree(List)` panics with
//!   UnsupportedOperationException("not implemented").
//! * `at(int)` returns `this` in Java, to facilitate method chaining.  A
//!   `dyn DocTreeFactory` cannot name the implementor's type, so `at` returns
//!   `()` and the implementation keeps the position in interior mutable state;
//!   this keeps `at` callable through the `Rc<dyn DocTreeFactory>` handed out
//!   by `DocTrees::get_doc_tree_factory` (registered language-shape
//!   difference).
//! * `Diagnostic<JavaFileObject> diag` of `newErroneousTree` may be null (its
//!   Javadoc says "or {@code null}") ->
//!   `Option<Rc<dyn Diagnostic<JavaFileObjectRef>>>`.
//!
//! @implNote The methods in an implementation of this interface may only accept
//! {@code DocTree} nodes that have been created by the same implementation.
//!
//! @since 9

use crate::doctree::attribute_tree::{AttributeTree, ValueKind};
use crate::doctree::author_tree::AuthorTree;
use crate::doctree::comment_tree::CommentTree;
use crate::doctree::deprecated_tree::DeprecatedTree;
use crate::doctree::doc_comment_tree::DocCommentTree;
use crate::doctree::doc_root_tree::DocRootTree;
use crate::doctree::doc_tree::DocTree;
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
use javac_strings::JavaString;
use javac_tools::lang::model::element::Name;
use javac_tools::tools::diagnostic::Diagnostic;
use javac_tools::tools::java_file_manager::JavaFileObjectRef;
use std::rc::Rc;

/// Factory for creating {@code DocTree} nodes.
///
/// @implNote The methods in an implementation of this interface may only accept
/// {@code DocTree} nodes that have been created by the same implementation.
///
/// @since 9
pub trait DocTreeFactory {
    /// Creates a new {@code AttributeTree} object, to represent an HTML attribute in an HTML tag.
    /// @param name  the name of the attribute
    /// @param vkind the kind of the attribute value
    /// @param value the value, if any, of the attribute
    /// @return an {@code AttributeTree} object
    fn new_attribute_tree(
        &self,
        name: Rc<dyn Name>,
        vkind: ValueKind,
        value: &[Rc<dyn DocTree>],
    ) -> Rc<dyn AttributeTree>;

    /// Creates a new {@code AuthorTree} object, to represent an {@code @author} tag.
    /// @param name the name of the author
    /// @return an {@code AuthorTree} object
    fn new_author_tree(&self, name: &[Rc<dyn DocTree>]) -> Rc<dyn AuthorTree>;

    /// Creates a new {@code LiteralTree} object, to represent a {@code {@code }} tag.
    /// @param text the content of the tag
    /// @return a {@code LiteralTree} object
    fn new_code_tree(&self, text: Rc<dyn TextTree>) -> Rc<dyn LiteralTree>;

    /// Creates a new {@code CommentTree}, to represent an HTML comment.
    /// @param text the content of the comment
    /// @return a {@code CommentTree} object
    fn new_comment_tree(&self, text: &JavaString) -> Rc<dyn CommentTree>;

    /// Creates a new {@code DeprecatedTree} object, to represent an {@code @deprecated} tag.
    /// @param text the content of the tag
    /// @return a {@code DeprecatedTree} object
    fn new_deprecated_tree(&self, text: &[Rc<dyn DocTree>]) -> Rc<dyn DeprecatedTree>;

    /// Creates a new {@code DocCommentTree} object, to represent a complete doc comment.
    /// @param full_body the entire body of the doc comment
    /// @param tags the block tags in the doc comment
    /// @return a {@code DocCommentTree} object
    fn new_doc_comment_tree(
        &self,
        full_body: &[Rc<dyn DocTree>],
        tags: &[Rc<dyn DocTree>],
    ) -> Rc<dyn DocCommentTree>;

    /// Creates a new {@code DocCommentTree} object, to represent the entire doc comment.
    /// (Java: newDocCommentTree(List, List, List, List) overload)
    /// @param full_body the entire body of the doc comment
    /// @param tags the block tags in the doc comment
    /// @param preamble the meta content of an html file including the body tag
    /// @param postamble the meta content of an html including the closing body tag
    /// @return a {@code DocCommentTree} object
    /// @since 10
    fn new_doc_comment_tree_with_preamble(
        &self,
        full_body: &[Rc<dyn DocTree>],
        tags: &[Rc<dyn DocTree>],
        preamble: &[Rc<dyn DocTree>],
        postamble: &[Rc<dyn DocTree>],
    ) -> Rc<dyn DocCommentTree>;

    /// Creates a new {@code DocRootTree} object, to represent an {@code {@docRoot}} tag.
    /// @return a {@code DocRootTree} object
    fn new_doc_root_tree(&self) -> Rc<dyn DocRootTree>;

    /// Creates a new {@code DocTypeTree}, to represent a {@code DOCTYPE} HTML declaration.
    /// @param text the content of the declaration
    /// @return a {@code DocTypeTree} object
    /// @since 10
    fn new_doc_type_tree(&self, text: &JavaString) -> Rc<dyn DocTypeTree>;

    /// Creates a new {@code EndElement} object, to represent the end of an HTML element.
    /// @param name the name of the HTML element
    /// @return an {@code EndElementTree} object
    fn new_end_element_tree(&self, name: Rc<dyn Name>) -> Rc<dyn EndElementTree>;

    /// Creates a new {@code EntityTree} object, to represent an HTML entity.
    /// @param name the name of the entity, representing the characters between '&amp;' and ';'
    /// in the representation of the entity in an HTML document
    /// @return an {@code EntityTree} object
    fn new_entity_tree(&self, name: Rc<dyn Name>) -> Rc<dyn EntityTree>;

    /// Creates a new {@code ErroneousTree} object, to represent some unparseable input.
    /// @param text the unparseable text
    /// @param diag a diagnostic associated with the unparseable text, or {@code null}
    /// @return an {@code ErroneousTree} object
    fn new_erroneous_tree(
        &self,
        text: &JavaString,
        diag: Option<Rc<dyn Diagnostic<JavaFileObjectRef>>>,
    ) -> Rc<dyn ErroneousTree>;

    /// Creates a new {@code ThrowsTree} object, to represent an {@code @exception} tag.
    /// @param name the name of the exception
    /// @param description a description of why the exception might be thrown
    /// @return an {@code ThrowsTree} object
    fn new_exception_tree(
        &self,
        name: Rc<dyn ReferenceTree>,
        description: &[Rc<dyn DocTree>],
    ) -> Rc<dyn ThrowsTree>;

    /// Creates a new {@code HiddenTree} object, to represent an {@code @hidden} tag.
    /// @param text the content of the tag
    /// @return a {@code HiddenTree} object
    fn new_hidden_tree(&self, text: &[Rc<dyn DocTree>]) -> Rc<dyn HiddenTree>;

    /// Creates a new {@code IdentifierTree} object, to represent an identifier, such as in a
    /// {@code @param} tag.
    /// @param name the name of the identifier
    /// @return an {@code IdentifierTree} object
    fn new_identifier_tree(&self, name: Rc<dyn Name>) -> Rc<dyn IdentifierTree>;

    /// Creates a new {@code IndexTree} object, to represent an {@code {@index }} tag.
    /// @param term the search term
    /// @param description an optional description of the search term
    /// @return an {@code IndexTree} object
    fn new_index_tree(
        &self,
        term: Rc<dyn DocTree>,
        description: &[Rc<dyn DocTree>],
    ) -> Rc<dyn IndexTree>;

    /// Creates a new {@code InheritDocTree} object, to represent an {@code {@inheritDoc}} tag.
    /// @return a {@code InheritDocTree} object
    fn new_inherit_doc_tree(&self) -> Rc<dyn InheritDocTree>;

    /// Creates a new {@code LinkTree} object, to represent a {@code {@link }} tag.
    /// @param ref_ the API element being referenced
    /// @param label an optional label for the link
    /// @return a {@code LinkTree} object
    fn new_link_tree(
        &self,
        ref_: Rc<dyn ReferenceTree>,
        label: &[Rc<dyn DocTree>],
    ) -> Rc<dyn LinkTree>;

    /// Creates a new {@code LinkTree} object, to represent a {@code {@linkplain }} tag.
    /// @param ref_ the API element being referenced
    /// @param label an optional label for the link
    /// @return a {@code LinkTree} object
    fn new_link_plain_tree(
        &self,
        ref_: Rc<dyn ReferenceTree>,
        label: &[Rc<dyn DocTree>],
    ) -> Rc<dyn LinkTree>;

    /// Creates a new {@code LiteralTree} object, to represent a {@code {@literal }} tag.
    /// @param text the content of the tag
    /// @return a {@code LiteralTree} object
    fn new_literal_tree(&self, text: Rc<dyn TextTree>) -> Rc<dyn LiteralTree>;

    /// Creates a new {@code ParamTree} object, to represent a {@code @param} tag.
    /// @param is_type_parameter {@code true} if this is a type parameter, and {@code false} otherwise
    /// @param name the parameter being described
    /// @param description the description of the parameter
    /// @return a {@code ParamTree} object
    fn new_param_tree(
        &self,
        is_type_parameter: bool,
        name: Rc<dyn IdentifierTree>,
        description: &[Rc<dyn DocTree>],
    ) -> Rc<dyn ParamTree>;

    /// Creates a new {@code ProvidesTree} object, to represent a {@code @provides} tag.
    /// @param name the name of the service type
    /// @param description a description of the service being provided
    /// @return a {@code ProvidesTree} object
    fn new_provides_tree(
        &self,
        name: Rc<dyn ReferenceTree>,
        description: &[Rc<dyn DocTree>],
    ) -> Rc<dyn ProvidesTree>;

    /// Creates a new {@code ReferenceTree} object, to represent a reference to an API element.
    ///
    /// @param signature the doc comment signature of the reference
    /// @return a {@code ReferenceTree} object
    fn new_reference_tree(&self, signature: &JavaString) -> Rc<dyn ReferenceTree>;

    /// Creates a new {@code ReturnTree} object, to represent a {@code @return} tag.
    /// @param description the description of the return value of a method
    /// @return a {@code ReturnTree} object
    fn new_return_tree(&self, description: &[Rc<dyn DocTree>]) -> Rc<dyn ReturnTree>;

    /// Creates a new {@code ReturnTree} object, to represent a {@code @return} tag
    /// or {@code {@return}} tag.
    /// (Java: newReturnTree(boolean, List) overload)
    ///
    /// @param is_inline    {@code true} if this instance is as an inline tag,
    ///                    and {@code false} otherwise
    /// @param description the description of the return value of a method
    ///
    /// @return a {@code ReturnTree} object
    /// @throws UnsupportedOperationException if inline {@code {@return}} tags are
    ///                                       not supported
    ///
    /// @implSpec This implementation throws {@code UnsupportedOperationException} if
    /// {@code isInline} is {@code true}, and calls {@link #new_return_tree(List)}
    /// otherwise.
    ///
    /// @since 16
    fn new_return_tree_with_inline(
        &self,
        is_inline: bool,
        description: &[Rc<dyn DocTree>],
    ) -> Rc<dyn ReturnTree> {
        if is_inline {
            // Java: throw new UnsupportedOperationException();
            panic!("UnsupportedOperationException: DocTreeFactory.newReturnTree");
        }
        self.new_return_tree(description)
    }

    /// Creates a new {@code SeeTree} object, to represent a {@code @see} tag.
    /// @param reference the reference
    /// @return a {@code SeeTree} object
    fn new_see_tree(&self, reference: &[Rc<dyn DocTree>]) -> Rc<dyn SeeTree>;

    /// Creates a new {@code SerialTree} object, to represent a {@code @serial} tag.
    /// @param description the description for the tag
    /// @return a {@code SerialTree} object
    fn new_serial_tree(&self, description: &[Rc<dyn DocTree>]) -> Rc<dyn SerialTree>;

    /// Creates a new {@code SerialDataTree} object, to represent a {@code @serialData} tag.
    /// @param description the description for the tag
    /// @return a {@code SerialDataTree} object
    fn new_serial_data_tree(&self, description: &[Rc<dyn DocTree>]) -> Rc<dyn SerialDataTree>;

    /// Creates a new {@code SerialFieldTree} object, to represent a {@code @serialField} tag.
    /// @param name the name of the field
    /// @param type_ the type of the field
    /// @param description the description of the field
    /// @return a {@code SerialFieldTree} object
    fn new_serial_field_tree(
        &self,
        name: Rc<dyn IdentifierTree>,
        type_: Rc<dyn ReferenceTree>,
        description: &[Rc<dyn DocTree>],
    ) -> Rc<dyn SerialFieldTree>;

    /// Creates a new {@code SinceTree} object, to represent a {@code @since} tag.
    /// @param text the content of the tag
    /// @return a {@code SinceTree} object
    fn new_since_tree(&self, text: &[Rc<dyn DocTree>]) -> Rc<dyn SinceTree>;

    /// Creates a new {@code StartElementTree} object, to represent the start of an HTML element.
    /// @param name the name of the HTML element
    /// @param attrs the attributes
    /// @param self_closing {@code true} if the start element is marked as self-closing; otherwise {@code false}
    /// @return a {@code StartElementTree} object
    fn new_start_element_tree(
        &self,
        name: Rc<dyn Name>,
        attrs: &[Rc<dyn DocTree>],
        self_closing: bool,
    ) -> Rc<dyn StartElementTree>;

    /// Creates a new {@code SummaryTree} object, to represent a {@code {@summary }} tag.
    ///
    /// @implSpec This implementation throws {@code UnsupportedOperationException}.
    ///
    /// @param summary the content of the tag
    /// @return a {@code SummaryTree} object
    /// @since 10
    fn new_summary_tree(&self, _summary: &[Rc<dyn DocTree>]) -> Rc<dyn SummaryTree> {
        // Java: throw new UnsupportedOperationException("not implemented");
        panic!("UnsupportedOperationException: not implemented")
    }

    /// Creates a new {@code SystemPropertyTree} object, to represent a {@code {@systemProperty }} tag.
    ///
    /// @param property_name the system property name
    /// @return a {@code SystemPropertyTree} object
    /// @since 12
    fn new_system_property_tree(&self, property_name: Rc<dyn Name>) -> Rc<dyn SystemPropertyTree>;

    /// Creates a new {@code TextTree} object, to represent some plain text.
    /// @param text the text
    /// @return a {@code TextTree} object
    fn new_text_tree(&self, text: &JavaString) -> Rc<dyn TextTree>;

    /// Creates a new {@code ThrowsTree} object, to represent a {@code @throws} tag.
    /// @param name the name of the exception
    /// @param description a description of why the exception might be thrown
    /// @return a {@code ThrowsTree} object
    fn new_throws_tree(
        &self,
        name: Rc<dyn ReferenceTree>,
        description: &[Rc<dyn DocTree>],
    ) -> Rc<dyn ThrowsTree>;

    /// Creates a new {@code UnknownBlockTagTree} object, to represent an unrecognized block tag.
    /// @param name the name of the block tag
    /// @param content the content
    /// @return an {@code UnknownBlockTagTree} object
    fn new_unknown_block_tag_tree(
        &self,
        name: Rc<dyn Name>,
        content: &[Rc<dyn DocTree>],
    ) -> Rc<dyn UnknownBlockTagTree>;

    /// Creates a new {@code UnknownInlineTagTree} object, to represent an unrecognized inline tag.
    /// @param name the name of the inline tag
    /// @param content the content
    /// @return an {@code UnknownInlineTagTree} object
    fn new_unknown_inline_tag_tree(
        &self,
        name: Rc<dyn Name>,
        content: &[Rc<dyn DocTree>],
    ) -> Rc<dyn UnknownInlineTagTree>;

    /// Creates a new {@code UsesTree} object, to represent a {@code @uses} tag.
    /// @param name the name of the service type
    /// @param description a description of how the service will be used
    /// @return a {@code UsesTree} object
    fn new_uses_tree(
        &self,
        name: Rc<dyn ReferenceTree>,
        description: &[Rc<dyn DocTree>],
    ) -> Rc<dyn UsesTree>;

    /// Creates a new {@code ValueTree} object, to represent a {@code {@value }} tag.
    /// @param ref_ a reference to the value
    /// @return a {@code ValueTree} object
    fn new_value_tree(&self, ref_: Rc<dyn ReferenceTree>) -> Rc<dyn ValueTree>;

    /// Creates a new {@code VersionTree} object, to represent a {@code {@version }} tag.
    /// @param text the content of the tag
    /// @return a {@code VersionTree} object
    fn new_version_tree(&self, text: &[Rc<dyn DocTree>]) -> Rc<dyn VersionTree>;

    /// Sets the position to be recorded in subsequent tree nodes created by this factory.
    /// The position should be a character offset relative to the beginning of the source file
    /// or {@link javax.tools.Diagnostic#NOPOS NOPOS}.
    /// @param pos the position
    /// @return this object, to facilitate method chaining
    ///
    /// returns {@code ()} instead of {@code this} (see file header).
    fn at(&self, pos: i32);

    /// Gets the first sentence contained in a list of content.
    /// The determination of the first sentence is implementation specific, and may
    /// involve the use of a locale-specific {@link java.text.BreakIterator BreakIterator}
    /// and other heuristics.
    /// The resulting list may share a common set of initial items with the input list.
    /// @param list the list
    /// @return a list containing the first sentence of the list
    fn get_first_sentence(&self, list: &[Rc<dyn DocTree>]) -> Vec<Rc<dyn DocTree>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctree::block_tag_tree::BlockTagTree;
    use crate::doctree::doc_tree::Kind;
    use crate::doctree::doc_tree_visitor::DocTreeVisitor;
    use crate::doctree::inline_tag_tree::InlineTagTree;
    use std::cell::Cell;

    struct StubText;

    impl DocTree for StubText {
        fn get_kind(&self) -> Kind {
            Kind::TEXT
        }
        fn accept<R, D>(&self, _visitor: &mut dyn DocTreeVisitor<R, D>, _data: D) -> R {
            panic!("UnsupportedOperationException: accept: test fixture accept")
        }
    }

    impl TextTree for StubText {
        fn get_body(&self) -> JavaString {
            JavaString::from("text")
        }
    }

    struct StubReturnTree {
        inline: bool,
    }

    impl DocTree for StubReturnTree {
        fn get_kind(&self) -> Kind {
            Kind::RETURN
        }
        fn accept<R, D>(&self, _visitor: &mut dyn DocTreeVisitor<R, D>, _data: D) -> R {
            panic!("UnsupportedOperationException: accept: test fixture accept")
        }
    }

    impl BlockTagTree for StubReturnTree {
        fn get_tag_name(&self) -> JavaString {
            JavaString::from("return")
        }
    }

    impl InlineTagTree for StubReturnTree {
        fn get_tag_name(&self) -> JavaString {
            JavaString::from("return")
        }
    }

    impl ReturnTree for StubReturnTree {
        fn is_inline(&self) -> bool {
            self.inline
        }
        fn get_description(&self) -> Vec<Rc<dyn DocTree>> {
            Vec::new()
        }
    }

    /// A factory fixture: only the methods exercised by the tests have a body;
    /// the remaining required methods stand for the P6 implementation and are
    /// never called here.
    struct MockFactory {
        pos: Cell<i32>,
        created_at: Cell<Option<i32>>,
        return_tree_calls: Cell<u32>,
    }

    impl MockFactory {
        fn new() -> Self {
            MockFactory {
                pos: Cell::new(0),
                created_at: Cell::new(None),
                return_tree_calls: Cell::new(0),
            }
        }
    }

    impl DocTreeFactory for MockFactory {
        fn new_attribute_tree(
            &self,
            _name: Rc<dyn Name>,
            _vkind: ValueKind,
            _value: &[Rc<dyn DocTree>],
        ) -> Rc<dyn AttributeTree> {
            panic!("UnsupportedOperationException: new_attribute_tree: mock DocTreeFactory")
        }

        fn new_author_tree(&self, _name: &[Rc<dyn DocTree>]) -> Rc<dyn AuthorTree> {
            panic!("UnsupportedOperationException: new_author_tree: mock DocTreeFactory")
        }

        fn new_code_tree(&self, _text: Rc<dyn TextTree>) -> Rc<dyn LiteralTree> {
            panic!("UnsupportedOperationException: new_code_tree: mock DocTreeFactory")
        }

        fn new_comment_tree(&self, _text: &JavaString) -> Rc<dyn CommentTree> {
            panic!("UnsupportedOperationException: new_comment_tree: mock DocTreeFactory")
        }

        fn new_deprecated_tree(&self, _text: &[Rc<dyn DocTree>]) -> Rc<dyn DeprecatedTree> {
            panic!("UnsupportedOperationException: new_deprecated_tree: mock DocTreeFactory")
        }

        fn new_doc_comment_tree(
            &self,
            _full_body: &[Rc<dyn DocTree>],
            _tags: &[Rc<dyn DocTree>],
        ) -> Rc<dyn DocCommentTree> {
            panic!("UnsupportedOperationException: new_doc_comment_tree: mock DocTreeFactory")
        }

        fn new_doc_comment_tree_with_preamble(
            &self,
            _full_body: &[Rc<dyn DocTree>],
            _tags: &[Rc<dyn DocTree>],
            _preamble: &[Rc<dyn DocTree>],
            _postamble: &[Rc<dyn DocTree>],
        ) -> Rc<dyn DocCommentTree> {
            panic!(
                "UnsupportedOperationException: new_doc_comment_tree_with_preamble: mock DocTreeFactory"
            )
        }

        fn new_doc_root_tree(&self) -> Rc<dyn DocRootTree> {
            panic!("UnsupportedOperationException: new_doc_root_tree: mock DocTreeFactory")
        }

        fn new_doc_type_tree(&self, _text: &JavaString) -> Rc<dyn DocTypeTree> {
            panic!("UnsupportedOperationException: new_doc_type_tree: mock DocTreeFactory")
        }

        fn new_end_element_tree(&self, _name: Rc<dyn Name>) -> Rc<dyn EndElementTree> {
            panic!("UnsupportedOperationException: new_end_element_tree: mock DocTreeFactory")
        }

        fn new_entity_tree(&self, _name: Rc<dyn Name>) -> Rc<dyn EntityTree> {
            panic!("UnsupportedOperationException: new_entity_tree: mock DocTreeFactory")
        }

        fn new_erroneous_tree(
            &self,
            _text: &JavaString,
            _diag: Option<Rc<dyn Diagnostic<JavaFileObjectRef>>>,
        ) -> Rc<dyn ErroneousTree> {
            panic!("UnsupportedOperationException: new_erroneous_tree: mock DocTreeFactory")
        }

        fn new_exception_tree(
            &self,
            _name: Rc<dyn ReferenceTree>,
            _description: &[Rc<dyn DocTree>],
        ) -> Rc<dyn ThrowsTree> {
            panic!("UnsupportedOperationException: new_exception_tree: mock DocTreeFactory")
        }

        fn new_hidden_tree(&self, _text: &[Rc<dyn DocTree>]) -> Rc<dyn HiddenTree> {
            panic!("UnsupportedOperationException: new_hidden_tree: mock DocTreeFactory")
        }

        fn new_identifier_tree(&self, _name: Rc<dyn Name>) -> Rc<dyn IdentifierTree> {
            panic!("UnsupportedOperationException: new_identifier_tree: mock DocTreeFactory")
        }

        fn new_index_tree(
            &self,
            _term: Rc<dyn DocTree>,
            _description: &[Rc<dyn DocTree>],
        ) -> Rc<dyn IndexTree> {
            panic!("UnsupportedOperationException: new_index_tree: mock DocTreeFactory")
        }

        fn new_inherit_doc_tree(&self) -> Rc<dyn InheritDocTree> {
            panic!("UnsupportedOperationException: new_inherit_doc_tree: mock DocTreeFactory")
        }

        fn new_link_tree(
            &self,
            _ref_: Rc<dyn ReferenceTree>,
            _label: &[Rc<dyn DocTree>],
        ) -> Rc<dyn LinkTree> {
            panic!("UnsupportedOperationException: new_link_tree: mock DocTreeFactory")
        }

        fn new_link_plain_tree(
            &self,
            _ref_: Rc<dyn ReferenceTree>,
            _label: &[Rc<dyn DocTree>],
        ) -> Rc<dyn LinkTree> {
            panic!("UnsupportedOperationException: new_link_plain_tree: mock DocTreeFactory")
        }

        fn new_literal_tree(&self, _text: Rc<dyn TextTree>) -> Rc<dyn LiteralTree> {
            panic!("UnsupportedOperationException: new_literal_tree: mock DocTreeFactory")
        }

        fn new_param_tree(
            &self,
            _is_type_parameter: bool,
            _name: Rc<dyn IdentifierTree>,
            _description: &[Rc<dyn DocTree>],
        ) -> Rc<dyn ParamTree> {
            panic!("UnsupportedOperationException: new_param_tree: mock DocTreeFactory")
        }

        fn new_provides_tree(
            &self,
            _name: Rc<dyn ReferenceTree>,
            _description: &[Rc<dyn DocTree>],
        ) -> Rc<dyn ProvidesTree> {
            panic!("UnsupportedOperationException: new_provides_tree: mock DocTreeFactory")
        }

        fn new_reference_tree(&self, _signature: &JavaString) -> Rc<dyn ReferenceTree> {
            panic!("UnsupportedOperationException: new_reference_tree: mock DocTreeFactory")
        }

        fn new_return_tree(&self, _description: &[Rc<dyn DocTree>]) -> Rc<dyn ReturnTree> {
            self.return_tree_calls
                .set(self.return_tree_calls.get().wrapping_add(1));
            Rc::new(StubReturnTree { inline: false })
        }

        fn new_see_tree(&self, _reference: &[Rc<dyn DocTree>]) -> Rc<dyn SeeTree> {
            panic!("UnsupportedOperationException: new_see_tree: mock DocTreeFactory")
        }

        fn new_serial_tree(&self, _description: &[Rc<dyn DocTree>]) -> Rc<dyn SerialTree> {
            panic!("UnsupportedOperationException: new_serial_tree: mock DocTreeFactory")
        }

        fn new_serial_data_tree(&self, _description: &[Rc<dyn DocTree>]) -> Rc<dyn SerialDataTree> {
            panic!("UnsupportedOperationException: new_serial_data_tree: mock DocTreeFactory")
        }

        fn new_serial_field_tree(
            &self,
            _name: Rc<dyn IdentifierTree>,
            _type_: Rc<dyn ReferenceTree>,
            _description: &[Rc<dyn DocTree>],
        ) -> Rc<dyn SerialFieldTree> {
            panic!("UnsupportedOperationException: new_serial_field_tree: mock DocTreeFactory")
        }

        fn new_since_tree(&self, _text: &[Rc<dyn DocTree>]) -> Rc<dyn SinceTree> {
            panic!("UnsupportedOperationException: new_since_tree: mock DocTreeFactory")
        }

        fn new_start_element_tree(
            &self,
            _name: Rc<dyn Name>,
            _attrs: &[Rc<dyn DocTree>],
            _self_closing: bool,
        ) -> Rc<dyn StartElementTree> {
            panic!("UnsupportedOperationException: new_start_element_tree: mock DocTreeFactory")
        }

        fn new_system_property_tree(
            &self,
            _property_name: Rc<dyn Name>,
        ) -> Rc<dyn SystemPropertyTree> {
            panic!("UnsupportedOperationException: new_system_property_tree: mock DocTreeFactory")
        }

        fn new_text_tree(&self, _text: &JavaString) -> Rc<dyn TextTree> {
            self.created_at.set(Some(self.pos.get()));
            Rc::new(StubText)
        }

        fn new_throws_tree(
            &self,
            _name: Rc<dyn ReferenceTree>,
            _description: &[Rc<dyn DocTree>],
        ) -> Rc<dyn ThrowsTree> {
            panic!("UnsupportedOperationException: new_throws_tree: mock DocTreeFactory")
        }

        fn new_unknown_block_tag_tree(
            &self,
            _name: Rc<dyn Name>,
            _content: &[Rc<dyn DocTree>],
        ) -> Rc<dyn UnknownBlockTagTree> {
            panic!("UnsupportedOperationException: new_unknown_block_tag_tree: mock DocTreeFactory")
        }

        fn new_unknown_inline_tag_tree(
            &self,
            _name: Rc<dyn Name>,
            _content: &[Rc<dyn DocTree>],
        ) -> Rc<dyn UnknownInlineTagTree> {
            panic!(
                "UnsupportedOperationException: new_unknown_inline_tag_tree: mock DocTreeFactory"
            )
        }

        fn new_uses_tree(
            &self,
            _name: Rc<dyn ReferenceTree>,
            _description: &[Rc<dyn DocTree>],
        ) -> Rc<dyn UsesTree> {
            panic!("UnsupportedOperationException: new_uses_tree: mock DocTreeFactory")
        }

        fn new_value_tree(&self, _ref_: Rc<dyn ReferenceTree>) -> Rc<dyn ValueTree> {
            panic!("UnsupportedOperationException: new_value_tree: mock DocTreeFactory")
        }

        fn new_version_tree(&self, _text: &[Rc<dyn DocTree>]) -> Rc<dyn VersionTree> {
            panic!("UnsupportedOperationException: new_version_tree: mock DocTreeFactory")
        }

        fn at(&self, pos: i32) {
            self.pos.set(pos);
        }

        fn get_first_sentence(&self, list: &[Rc<dyn DocTree>]) -> Vec<Rc<dyn DocTree>> {
            list.to_vec()
        }
    }

    #[test]
    fn doc_tree_factory_is_object_safe() {
        let factory: Rc<dyn DocTreeFactory> = Rc::new(MockFactory::new());
        // at and the factory methods are callable through the
        // Rc<dyn DocTreeFactory> returned by DocTrees.getDocTreeFactory().
        factory.at(1);
        let text = factory.new_text_tree(&JavaString::from("text"));
        assert_eq!(text.get_body().to_string_lossy(), "text");
        let description: Vec<Rc<dyn DocTree>> = vec![Rc::new(StubText)];
        let returned = factory.new_return_tree_with_inline(false, &description);
        assert!(!returned.is_inline());
        assert_eq!(factory.get_first_sentence(&description).len(), 1);
    }

    #[test]
    fn at_sets_the_position_recorded_in_subsequent_nodes() {
        let factory = MockFactory::new();
        let dyn_factory: &dyn DocTreeFactory = &factory;
        dyn_factory.at(42);
        let _ = dyn_factory.new_text_tree(&JavaString::from("text"));
        assert_eq!(factory.created_at.get(), Some(42));
    }

    #[test]
    fn new_return_tree_with_inline_delegates_to_new_return_tree() {
        let factory = MockFactory::new();
        let description: Vec<Rc<dyn DocTree>> = vec![Rc::new(StubText)];
        let tree = factory.new_return_tree_with_inline(false, &description);
        assert!(!tree.is_inline());
        assert_eq!(factory.return_tree_calls.get(), 1);
    }

    #[test]
    #[should_panic(expected = "UnsupportedOperationException")]
    fn new_return_tree_with_inline_panics_for_inline_form() {
        let factory = MockFactory::new();
        factory.new_return_tree_with_inline(true, &[]);
    }

    #[test]
    #[should_panic(expected = "not implemented")]
    fn new_summary_tree_has_no_default_implementation() {
        let factory = MockFactory::new();
        factory.new_summary_tree(&[]);
    }
}
