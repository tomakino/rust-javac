//! Common interface for all nodes in a documentation syntax tree.
//!
//! (docs/porting adapter.md §14):
//! * Java interface -> Rust trait; the nested DocTree.Kind enum is declared in
//!   this module (39 constants, Java order) and its Java
//!   `public final String tagName` field is expressed by `tag_name()`, with
//!   `None` reproducing the null of the no-tag constructor.
//! * `DocTree.accept(DocTreeVisitor, D)` is generic and cannot be called on
//!   `&dyn DocTree`; it keeps `where Self: Sized` and the util layer
//!   reproduces the dispatch from getKind() plus the object-conversion
//!   accessors (as_xxx, §13 contract).
//! * the Java `@since` tags are kept as comment text.

/// Common interface for all nodes in a documentation syntax tree.
///
/// @since 1.8
use super::doc_tree_visitor::DocTreeVisitor;
use javac_strings::JavaString;
use super::attribute_tree::AttributeTree;
use super::author_tree::AuthorTree;
use super::block_tag_tree::BlockTagTree;
use super::comment_tree::CommentTree;
use super::deprecated_tree::DeprecatedTree;
use super::doc_comment_tree::DocCommentTree;
use super::doc_root_tree::DocRootTree;
use super::doc_type_tree::DocTypeTree;
use super::end_element_tree::EndElementTree;
use super::entity_tree::EntityTree;
use super::erroneous_tree::ErroneousTree;
use super::hidden_tree::HiddenTree;
use super::identifier_tree::IdentifierTree;
use super::index_tree::IndexTree;
use super::inherit_doc_tree::InheritDocTree;
use super::inline_tag_tree::InlineTagTree;
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

pub trait DocTree {
    /// Returns the kind of this tree.
    ///
    /// @return the kind of this tree
    fn get_kind(&self) -> Kind;

    /// Accept method used to implement the visitor pattern.  The
    /// visitor pattern is used to implement operations on trees.
    ///
    /// @param <R> the result type of this operation
    /// @param <D> the type of additional data
    /// @param visitor the visitor to be called
    /// @param data a parameter value to be passed to the visitor method
    /// @return the value returned from the visitor method
    ///
    /// `where Self: Sized` keeps DocTree object safe.
    fn accept<R, D>(&self, visitor: &mut dyn DocTreeVisitor<R, D>, data: D) -> R
    where
        Self: Sized;

    /// object conversion for `instanceof AttributeTree` (see file header).
    fn as_attribute_tree(&self) -> Option<&dyn AttributeTree> {
        None
    }

    /// object conversion for `instanceof AuthorTree` (see file header).
    fn as_author_tree(&self) -> Option<&dyn AuthorTree> {
        None
    }

    /// object conversion for `instanceof BlockTagTree` (see file header).
    fn as_block_tag_tree(&self) -> Option<&dyn BlockTagTree> {
        None
    }

    /// object conversion for `instanceof CommentTree` (see file header).
    fn as_comment_tree(&self) -> Option<&dyn CommentTree> {
        None
    }

    /// object conversion for `instanceof DeprecatedTree` (see file header).
    fn as_deprecated_tree(&self) -> Option<&dyn DeprecatedTree> {
        None
    }

    /// object conversion for `instanceof DocCommentTree` (see file header).
    fn as_doc_comment_tree(&self) -> Option<&dyn DocCommentTree> {
        None
    }

    /// object conversion for `instanceof DocRootTree` (see file header).
    fn as_doc_root_tree(&self) -> Option<&dyn DocRootTree> {
        None
    }

    /// object conversion for `instanceof DocTypeTree` (see file header).
    fn as_doc_type_tree(&self) -> Option<&dyn DocTypeTree> {
        None
    }

    /// object conversion for `instanceof EndElementTree` (see file header).
    fn as_end_element_tree(&self) -> Option<&dyn EndElementTree> {
        None
    }

    /// object conversion for `instanceof EntityTree` (see file header).
    fn as_entity_tree(&self) -> Option<&dyn EntityTree> {
        None
    }

    /// object conversion for `instanceof ErroneousTree` (see file header).
    fn as_erroneous_tree(&self) -> Option<&dyn ErroneousTree> {
        None
    }

    /// object conversion for `instanceof HiddenTree` (see file header).
    fn as_hidden_tree(&self) -> Option<&dyn HiddenTree> {
        None
    }

    /// object conversion for `instanceof IdentifierTree` (see file header).
    fn as_identifier_tree(&self) -> Option<&dyn IdentifierTree> {
        None
    }

    /// object conversion for `instanceof IndexTree` (see file header).
    fn as_index_tree(&self) -> Option<&dyn IndexTree> {
        None
    }

    /// object conversion for `instanceof InheritDocTree` (see file header).
    fn as_inherit_doc_tree(&self) -> Option<&dyn InheritDocTree> {
        None
    }

    /// object conversion for `instanceof InlineTagTree` (see file header).
    fn as_inline_tag_tree(&self) -> Option<&dyn InlineTagTree> {
        None
    }

    /// object conversion for `instanceof LinkTree` (see file header).
    fn as_link_tree(&self) -> Option<&dyn LinkTree> {
        None
    }

    /// object conversion for `instanceof LiteralTree` (see file header).
    fn as_literal_tree(&self) -> Option<&dyn LiteralTree> {
        None
    }

    /// object conversion for `instanceof ParamTree` (see file header).
    fn as_param_tree(&self) -> Option<&dyn ParamTree> {
        None
    }

    /// object conversion for `instanceof ProvidesTree` (see file header).
    fn as_provides_tree(&self) -> Option<&dyn ProvidesTree> {
        None
    }

    /// object conversion for `instanceof ReferenceTree` (see file header).
    fn as_reference_tree(&self) -> Option<&dyn ReferenceTree> {
        None
    }

    /// object conversion for `instanceof ReturnTree` (see file header).
    fn as_return_tree(&self) -> Option<&dyn ReturnTree> {
        None
    }

    /// object conversion for `instanceof SeeTree` (see file header).
    fn as_see_tree(&self) -> Option<&dyn SeeTree> {
        None
    }

    /// object conversion for `instanceof SerialDataTree` (see file header).
    fn as_serial_data_tree(&self) -> Option<&dyn SerialDataTree> {
        None
    }

    /// object conversion for `instanceof SerialFieldTree` (see file header).
    fn as_serial_field_tree(&self) -> Option<&dyn SerialFieldTree> {
        None
    }

    /// object conversion for `instanceof SerialTree` (see file header).
    fn as_serial_tree(&self) -> Option<&dyn SerialTree> {
        None
    }

    /// object conversion for `instanceof SinceTree` (see file header).
    fn as_since_tree(&self) -> Option<&dyn SinceTree> {
        None
    }

    /// object conversion for `instanceof StartElementTree` (see file header).
    fn as_start_element_tree(&self) -> Option<&dyn StartElementTree> {
        None
    }

    /// object conversion for `instanceof SummaryTree` (see file header).
    fn as_summary_tree(&self) -> Option<&dyn SummaryTree> {
        None
    }

    /// object conversion for `instanceof SystemPropertyTree` (see file header).
    fn as_system_property_tree(&self) -> Option<&dyn SystemPropertyTree> {
        None
    }

    /// object conversion for `instanceof TextTree` (see file header).
    fn as_text_tree(&self) -> Option<&dyn TextTree> {
        None
    }

    /// object conversion for `instanceof ThrowsTree` (see file header).
    fn as_throws_tree(&self) -> Option<&dyn ThrowsTree> {
        None
    }

    /// object conversion for `instanceof UnknownBlockTagTree` (see file header).
    fn as_unknown_block_tag_tree(&self) -> Option<&dyn UnknownBlockTagTree> {
        None
    }

    /// object conversion for `instanceof UnknownInlineTagTree` (see file header).
    fn as_unknown_inline_tag_tree(&self) -> Option<&dyn UnknownInlineTagTree> {
        None
    }

    /// object conversion for `instanceof UsesTree` (see file header).
    fn as_uses_tree(&self) -> Option<&dyn UsesTree> {
        None
    }

    /// object conversion for `instanceof ValueTree` (see file header).
    fn as_value_tree(&self) -> Option<&dyn ValueTree> {
        None
    }

    /// object conversion for `instanceof VersionTree` (see file header).
    fn as_version_tree(&self) -> Option<&dyn VersionTree> {
        None
    }
}

/// Enumerates all kinds of trees.
///
/// Java enum constants keep their Java names (UPPER_SNAKE_CASE)
/// and declaration order.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Used for instances of {@link AttributeTree}
    /// representing an HTML attribute.
    ATTRIBUTE,

    /// Used for instances of {@link AuthorTree}
    /// representing an {@code @author} tag.
    AUTHOR,

    /// Used for instances of {@link LiteralTree}
    /// representing an {@code @code} tag.
    CODE,

    /// Used for instances of {@link CommentTree}
    /// representing an HTML comment.
    COMMENT,

    /// Used for instances of {@link DeprecatedTree}
    /// representing an {@code @deprecated} tag.
    DEPRECATED,

    /// Used for instances of {@link DocCommentTree}
    /// representing a complete doc comment.
    DOC_COMMENT,

    /// Used for instances of {@link DocRootTree}
    /// representing an {@code @docRoot} tag.
    DOC_ROOT,

    /// Used for instances of {@link DocTypeTree}
    /// representing an HTML DocType declaration.
    DOC_TYPE,

    /// Used for instances of {@link EndElementTree}
    /// representing the end of an HTML element.
    END_ELEMENT,

    /// Used for instances of {@link EntityTree}
    /// representing an HTML entity.
    ENTITY,

    /// Used for instances of {@link ErroneousTree}
    /// representing some invalid text.
    ERRONEOUS,

    /// Used for instances of {@link ThrowsTree}
    /// representing an {@code @exception} tag.
    EXCEPTION,

    /// Used for instances of {@link HiddenTree}
    /// representing an {@code @hidden} tag.
    HIDDEN,

    /// Used for instances of {@link IdentifierTree}
    /// representing an identifier.
    IDENTIFIER,

    /// Used for instances of {@link IndexTree}
    /// representing an {@code @index} tag.
    INDEX,

    /// Used for instances of {@link InheritDocTree}
    /// representing an {@code @inheritDoc} tag.
    INHERIT_DOC,

    /// Used for instances of {@link LinkTree}
    /// representing an {@code @link} tag.
    LINK,

    /// Used for instances of {@link LinkTree}
    /// representing an {@code @linkplain} tag.
    LINK_PLAIN,

    /// Used for instances of {@link LiteralTree}
    /// representing an {@code @literal} tag.
    LITERAL,

    /// Used for instances of {@link ParamTree}
    /// representing an {@code @param} tag.
    PARAM,

    /// Used for instances of {@link ProvidesTree}
    /// representing an {@code @provides} tag.
    PROVIDES,

    /// Used for instances of {@link ReferenceTree}
    /// representing a reference to an element in the
    /// Java programming language.
    REFERENCE,

    /// Used for instances of {@link ReturnTree}
    /// representing an {@code @return} tag.
    RETURN,

    /// Used for instances of {@link SeeTree}
    /// representing an {@code @see} tag.
    SEE,

    /// Used for instances of {@link SerialTree}
    /// representing an {@code @serial} tag.
    SERIAL,

    /// Used for instances of {@link SerialDataTree}
    /// representing an {@code @serialData} tag.
    SERIAL_DATA,

    /// Used for instances of {@link SerialFieldTree}
    /// representing an {@code @serialField} tag.
    SERIAL_FIELD,

    /// Used for instances of {@link SinceTree}
    /// representing an {@code @since} tag.
    SINCE,

    /// Used for instances of {@link EndElementTree}
    /// representing the start of an HTML element.
    START_ELEMENT,

    /// Used for instances of {@link SystemPropertyTree}
    /// representing an {@code @systemProperty} tag.
    SYSTEM_PROPERTY,

    /// Used for instances of {@link SummaryTree}
    /// representing an {@code @summary} tag.
    SUMMARY,

    /// Used for instances of {@link TextTree}
    /// representing some documentation text.
    TEXT,

    /// Used for instances of {@link ThrowsTree}
    /// representing an {@code @throws} tag.
    THROWS,

    /// Used for instances of {@link UnknownBlockTagTree}
    /// representing an unknown block tag.
    UNKNOWN_BLOCK_TAG,

    /// Used for instances of {@link UnknownInlineTagTree}
    /// representing an unknown inline tag.
    UNKNOWN_INLINE_TAG,

    /// Used for instances of {@link UsesTree}
    /// representing an {@code @uses} tag.
    USES,

    /// Used for instances of {@link ValueTree}
    /// representing an {@code @value} tag.
    VALUE,

    /// Used for instances of {@link VersionTree}
    /// representing an {@code @version} tag.
    VERSION,

    /// An implementation-reserved node. This is the not the node
    /// you are looking for.
    OTHER,

}

impl Kind {
    /// The name of the tag, if any, associated with this kind of node.
    ///
    /// Java: `public final String tagName` (null when the kind has no tag).
    pub fn tag_name(&self) -> Option<JavaString> {
        match self {
            Kind::ATTRIBUTE => None,
            Kind::AUTHOR => Some(JavaString::from("author")),
            Kind::CODE => Some(JavaString::from("code")),
            Kind::COMMENT => None,
            Kind::DEPRECATED => Some(JavaString::from("deprecated")),
            Kind::DOC_COMMENT => None,
            Kind::DOC_ROOT => Some(JavaString::from("docRoot")),
            Kind::DOC_TYPE => None,
            Kind::END_ELEMENT => None,
            Kind::ENTITY => None,
            Kind::ERRONEOUS => None,
            Kind::EXCEPTION => Some(JavaString::from("exception")),
            Kind::HIDDEN => Some(JavaString::from("hidden")),
            Kind::IDENTIFIER => None,
            Kind::INDEX => Some(JavaString::from("index")),
            Kind::INHERIT_DOC => Some(JavaString::from("inheritDoc")),
            Kind::LINK => Some(JavaString::from("link")),
            Kind::LINK_PLAIN => Some(JavaString::from("linkplain")),
            Kind::LITERAL => Some(JavaString::from("literal")),
            Kind::PARAM => Some(JavaString::from("param")),
            Kind::PROVIDES => Some(JavaString::from("provides")),
            Kind::REFERENCE => None,
            Kind::RETURN => Some(JavaString::from("return")),
            Kind::SEE => Some(JavaString::from("see")),
            Kind::SERIAL => Some(JavaString::from("serial")),
            Kind::SERIAL_DATA => Some(JavaString::from("serialData")),
            Kind::SERIAL_FIELD => Some(JavaString::from("serialField")),
            Kind::SINCE => Some(JavaString::from("since")),
            Kind::START_ELEMENT => None,
            Kind::SYSTEM_PROPERTY => Some(JavaString::from("systemProperty")),
            Kind::SUMMARY => Some(JavaString::from("summary")),
            Kind::TEXT => None,
            Kind::THROWS => Some(JavaString::from("throws")),
            Kind::UNKNOWN_BLOCK_TAG => None,
            Kind::UNKNOWN_INLINE_TAG => None,
            Kind::USES => Some(JavaString::from("uses")),
            Kind::VALUE => Some(JavaString::from("value")),
            Kind::VERSION => Some(JavaString::from("version")),
            Kind::OTHER => None,
        }
    }

    /// Java: enum#name() (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            Kind::ATTRIBUTE => "ATTRIBUTE",
            Kind::AUTHOR => "AUTHOR",
            Kind::CODE => "CODE",
            Kind::COMMENT => "COMMENT",
            Kind::DEPRECATED => "DEPRECATED",
            Kind::DOC_COMMENT => "DOC_COMMENT",
            Kind::DOC_ROOT => "DOC_ROOT",
            Kind::DOC_TYPE => "DOC_TYPE",
            Kind::END_ELEMENT => "END_ELEMENT",
            Kind::ENTITY => "ENTITY",
            Kind::ERRONEOUS => "ERRONEOUS",
            Kind::EXCEPTION => "EXCEPTION",
            Kind::HIDDEN => "HIDDEN",
            Kind::IDENTIFIER => "IDENTIFIER",
            Kind::INDEX => "INDEX",
            Kind::INHERIT_DOC => "INHERIT_DOC",
            Kind::LINK => "LINK",
            Kind::LINK_PLAIN => "LINK_PLAIN",
            Kind::LITERAL => "LITERAL",
            Kind::PARAM => "PARAM",
            Kind::PROVIDES => "PROVIDES",
            Kind::REFERENCE => "REFERENCE",
            Kind::RETURN => "RETURN",
            Kind::SEE => "SEE",
            Kind::SERIAL => "SERIAL",
            Kind::SERIAL_DATA => "SERIAL_DATA",
            Kind::SERIAL_FIELD => "SERIAL_FIELD",
            Kind::SINCE => "SINCE",
            Kind::START_ELEMENT => "START_ELEMENT",
            Kind::SYSTEM_PROPERTY => "SYSTEM_PROPERTY",
            Kind::SUMMARY => "SUMMARY",
            Kind::TEXT => "TEXT",
            Kind::THROWS => "THROWS",
            Kind::UNKNOWN_BLOCK_TAG => "UNKNOWN_BLOCK_TAG",
            Kind::UNKNOWN_INLINE_TAG => "UNKNOWN_INLINE_TAG",
            Kind::USES => "USES",
            Kind::VALUE => "VALUE",
            Kind::VERSION => "VERSION",
            Kind::OTHER => "OTHER",
        })
    }

    /// Java: enum#values().
    pub fn values() -> [Kind; 39] {
        [
            Kind::ATTRIBUTE,
            Kind::AUTHOR,
            Kind::CODE,
            Kind::COMMENT,
            Kind::DEPRECATED,
            Kind::DOC_COMMENT,
            Kind::DOC_ROOT,
            Kind::DOC_TYPE,
            Kind::END_ELEMENT,
            Kind::ENTITY,
            Kind::ERRONEOUS,
            Kind::EXCEPTION,
            Kind::HIDDEN,
            Kind::IDENTIFIER,
            Kind::INDEX,
            Kind::INHERIT_DOC,
            Kind::LINK,
            Kind::LINK_PLAIN,
            Kind::LITERAL,
            Kind::PARAM,
            Kind::PROVIDES,
            Kind::REFERENCE,
            Kind::RETURN,
            Kind::SEE,
            Kind::SERIAL,
            Kind::SERIAL_DATA,
            Kind::SERIAL_FIELD,
            Kind::SINCE,
            Kind::START_ELEMENT,
            Kind::SYSTEM_PROPERTY,
            Kind::SUMMARY,
            Kind::TEXT,
            Kind::THROWS,
            Kind::UNKNOWN_BLOCK_TAG,
            Kind::UNKNOWN_INLINE_TAG,
            Kind::USES,
            Kind::VALUE,
            Kind::VERSION,
            Kind::OTHER,
        ]
    }
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::{DocTree, Kind};
    use crate::doctree::doc_tree_visitor::DocTreeVisitor;

    struct Leaf {
        kind: Kind,
    }

    impl DocTree for Leaf {
        fn get_kind(&self) -> Kind {
            self.kind
        }
        fn accept<R, D>(&self, _visitor: &mut dyn DocTreeVisitor<R, D>, _data: D) -> R {
            panic!("UnsupportedOperationException: accept: test fixture accept")
        }
    }

    #[test]
    fn kind_enum_is_complete_and_ordered() {
        let values = Kind::values();
        assert_eq!(values.len(), 39);
        assert_eq!(values[0], Kind::ATTRIBUTE);
        assert_eq!(values[1], Kind::AUTHOR);
        assert_eq!(values[2], Kind::CODE);
        assert_eq!(values[values.len() - 1], Kind::OTHER);
        assert_eq!(Kind::AUTHOR.name().to_string_lossy(), "AUTHOR");
        assert_eq!(Kind::DOC_COMMENT.to_string(), "DOC_COMMENT");
    }

    #[test]
    fn kind_tag_names() {
        // Java: public final String tagName; null for kinds without a tag.
        assert_eq!(Kind::AUTHOR.tag_name().map(|s| s.to_string_lossy()), Some("author".to_string()));
        assert_eq!(Kind::CODE.tag_name().map(|s| s.to_string_lossy()), Some("code".to_string()));
        assert_eq!(Kind::DOC_ROOT.tag_name().map(|s| s.to_string_lossy()), Some("docRoot".to_string()));
        assert_eq!(Kind::ATTRIBUTE.tag_name(), None);
        assert_eq!(Kind::OTHER.tag_name(), None);
    }

    #[test]
    fn doc_tree_is_object_safe_with_conversion_accessors() {
        let leaf = Leaf { kind: Kind::ATTRIBUTE };
        let tree: &dyn DocTree = &leaf;
        assert_eq!(tree.get_kind(), Kind::ATTRIBUTE);
        assert!(tree.as_attribute_tree().is_none());
        assert!(tree.as_author_tree().is_none());
        let _ = tree.as_doc_comment_tree();
    }
}
