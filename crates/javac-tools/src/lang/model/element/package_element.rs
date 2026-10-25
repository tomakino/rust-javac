// Origin: javax/lang/model/element/PackageElement.java @ 982d53c (jdk-17.0.19-ga)

//! Represents a package program element.
//!
//! PORTING_RULES: `extends Element, QualifiedNameable` -> Rust supertraits.
//! The Java `@Override` re-declarations of `asType`, `getQualifiedName`,
//! `getSimpleName`, `getEnclosedElements` and `getEnclosingElement` only narrow
//! their documentation (the signatures are inherited verbatim); they are not
//! repeated in this trait (registered difference).

use super::element::Element;
use super::qualified_nameable::QualifiedNameable;

/// Represents a package program element.  Provides access to information
/// about the package and its members.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see javax.lang.model.util.Elements#getPackageOf
/// @since 1.6
pub trait PackageElement: Element + QualifiedNameable {
    /// {@return true if this is an unnamed package and false
    /// otherwise}
    ///
    /// @jls 7.4.2 Unnamed Packages
    fn is_unnamed(&self) -> bool;
}
