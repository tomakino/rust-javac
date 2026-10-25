// Origin: com/sun/source/doctree/DocTypeTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a {@code doctype} declaration.
///
/// <pre>
///    &lt;!doctype text&gt;
/// </pre>
///
/// For HTML5 documents, the correct form is {@code <!doctype html>}.
///
/// @since 10
use javac_strings::JavaString;
use super::doc_tree::DocTree;

pub trait DocTypeTree: DocTree {
    /// Returns the text of the doctype declaration.
    /// @return text
    fn get_text(&self) -> JavaString;

}
