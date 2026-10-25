// Origin: com/sun/source/doctree/ReferenceTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for a reference to a Java language element.
///
/// <pre>
///    package.class#field
///    package.class#method(<i>arg-types</i>)
/// </pre>
///
/// @since 1.8
use javac_strings::JavaString;
use super::doc_tree::DocTree;

pub trait ReferenceTree: DocTree {
    /// Returns the signature of the Java language element being referenced,
    /// as found in {@code @see} and similar nodes.
    /// @return the signature
    fn get_signature(&self) -> JavaString;

}
