// Origin: com/sun/source/tree/ModuleTree.java @ 982d53c (jdk-17.0.19-ga)

//! A tree node for a module declaration.
//!
//! PORTING_RULES: Java interface -> Rust trait; the nested enum ModuleKind
//! lives in this file.

use super::annotation_tree::AnnotationTree;
use super::directive_tree::DirectiveTree;
use super::expression_tree::ExpressionTree;
use super::tree::Tree;
use javac_strings::JavaString;
use std::rc::Rc;

/// A tree node for a module declaration.
///
/// For example:
/// <pre>
///    <em>annotations</em>
///    [open] module <em>module-name</em> {
///        <em>directives</em>
///    }
/// </pre>
///
/// @since 9
pub trait ModuleTree: Tree {
    /// Returns the annotations associated with this module declaration.
    /// @return the annotations
    fn get_annotations(&self) -> Vec<Rc<dyn AnnotationTree>>;

    /// Returns the type of this module.
    /// @return the type of this module
    fn get_module_type(&self) -> ModuleKind;

    /// Returns the name of the module.
    /// @return the name of the module
    fn get_name(&self) -> Rc<dyn ExpressionTree>;

    /// Returns the directives in the module declaration.
    /// @return the directives in the module declaration
    fn get_directives(&self) -> Vec<Rc<dyn DirectiveTree>>;
}

/// The kind of the module.
///
/// PORTING_RULES: Java nested enum ModuleTree.ModuleKind ->
/// module_tree::ModuleKind.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleKind {
    /// Open module.
    OPEN,
    /// Strong module.
    STRONG,
}

impl ModuleKind {
    /// Java: enum#name() (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            ModuleKind::OPEN => "OPEN",
            ModuleKind::STRONG => "STRONG",
        })
    }

    /// Java: enum#values().
    pub fn values() -> [ModuleKind; 2] {
        [ModuleKind::OPEN, ModuleKind::STRONG]
    }
}

impl std::fmt::Display for ModuleKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}
