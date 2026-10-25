// Origin: com/sun/source/doctree/ParamTree.java @ 982d53c (jdk-17.0.19-ga)

/// A tree node for an {@code @param} block tag.
///
/// <pre>
///    &#064;param parameter-name description
/// </pre>
///
/// @since 1.8
use std::rc::Rc;
use super::block_tag_tree::BlockTagTree;
use super::doc_tree::DocTree;
use super::identifier_tree::IdentifierTree;

pub trait ParamTree: BlockTagTree {
    /// Returns {@code true} if this is documenting a type parameter.
    /// @return {@code true} if this is documenting a type parameter
    fn is_type_parameter(&self) -> bool;

    /// Returns the name of the parameter.
    /// @return the name of the parameter
    fn get_name(&self) -> Rc<dyn IdentifierTree>;

    /// Returns the description of the parameter.
    /// @return the description
    fn get_description(&self) -> Vec<Rc<dyn DocTree>>;

}
