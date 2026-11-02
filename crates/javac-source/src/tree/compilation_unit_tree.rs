//! Represents the abstract syntax tree for ordinary compilation units and
//! modular compilation units.
//!
//! Java interface -> Rust trait; the Java `default` method
//! `getModule` throws UnsupportedOperationException and is expressed as a
//! panicking Rust default method; nullable returns -> Option.

use super::annotation_tree::AnnotationTree;
use super::expression_tree::ExpressionTree;
use super::import_tree::ImportTree;
use super::line_map::LineMap;
use super::module_tree::ModuleTree;
use super::package_tree::PackageTree;
use super::tree::Tree;
use javac_tools::tools::java_file_object::JavaFileObject;
use std::rc::Rc;

/// Represents the abstract syntax tree for ordinary compilation units
/// and modular compilation units.
///
/// @jls 7.3 Compilation Units
/// @jls 7.4 Package Declarations
/// @jls 7.7 Module Declarations
///
/// @author Peter von der Ah&eacute;
/// @since 1.6
pub trait CompilationUnitTree: Tree {
    /// Returns the module tree associated with this compilation unit,
    /// or {@code null} if there is no module declaration.
    /// @return the module tree
    /// @implSpec This implementation throws {@code UnsupportedOperationException}
    /// @since 17
    fn get_module(&self) -> Option<Rc<dyn ModuleTree>> {
        panic!("UnsupportedOperationException: CompilationUnitTree.getModule")
    }

    /// Returns the annotations listed on any package declaration
    /// at the head of this compilation unit, or {@code null} if there
    /// is no package declaration.
    /// @return the package annotations
    fn get_package_annotations(&self) -> Option<Vec<Rc<dyn AnnotationTree>>>;

    /// Returns the name contained in any package declaration
    /// at the head of this compilation unit, or {@code null} if there
    /// is no package declaration.
    /// @return the package name
    fn get_package_name(&self) -> Option<Rc<dyn ExpressionTree>>;

    /// Returns the package tree associated with this compilation unit,
    /// or {@code null} if there is no package declaration.
    /// @return the package tree
    /// @since 9
    fn get_package(&self) -> Option<Rc<dyn PackageTree>>;

    /// Returns the import declarations appearing in this compilation unit,
    /// or an empty list if there are no import declarations.
    /// @return the import declarations
    fn get_imports(&self) -> Vec<Rc<dyn ImportTree>>;

    /// Returns the type declarations appearing in this compilation unit,
    /// or an empty list if there are no type declarations.
    /// The list may also include empty statements resulting from
    /// extraneous semicolons.
    /// A modular compilation unit does not contain any type declarations.
    /// @return the type declarations
    fn get_type_decls(&self) -> Vec<Rc<dyn Tree>>;

    /// Returns the file object containing the source for this compilation unit.
    /// @return the file object
    fn get_source_file(&self) -> Rc<dyn JavaFileObject>;

    /// Returns the line map for this compilation unit, if available,
    /// or {@code null} if the line map is not available.
    /// @return the line map for this compilation unit
    fn get_line_map(&self) -> Option<Rc<dyn LineMap>>;
}
