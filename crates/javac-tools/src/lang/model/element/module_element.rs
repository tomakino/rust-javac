// Origin: javax/lang/model/element/ModuleElement.java @ 982d53c (jdk-17.0.19-ga)

//! Represents a module program element.
//!
//! PORTING_RULES:
//! * The nested types of the Java interface (`DirectiveKind`, `Directive`,
//!   `DirectiveVisitor`, `RequiresDirective`, `ExportsDirective`,
//!   `OpensDirective`, `ProvidesDirective`, `UsesDirective`) become types of
//!   this module (Rust traits cannot be nested inside traits); the
//!   `ModuleElement.X` names are mapped to `module_element::X`.
//! * `List<? extends X>` -> `Vec<Rc<dyn X>>`; nullable target-module lists ->
//!   `Option<Vec<...>>`.
//! * `Directive`/`DirectiveVisitor` take `&mut self` for the visitor and keep
//!   the `Accept` generic method `where Self: Sized` (object safety).
//! * The two Java `DirectiveVisitor` default methods `visit(Directive)` and
//!   `visit(Directive, P)` dispatch through `d.accept(this, p)`; a generic
//!   `accept` cannot be called on `dyn Directive` in Rust, so those two
//!   convenience methods are not ported (registered difference).  Callers use
//!   the concrete `visitRequires`/`visitExports`/... methods.
//! * `DirectiveVisitor.visitUnknown` throws `UnknownDirectiveException`
//!   upstream; the Rust default implementation panics with the same message
//!   (Rust `dyn` cannot be stored in the exception; see
//!   unknown_directive_exception.rs).
//! * The Java `@Override` re-declarations of `asType`, `getQualifiedName`,
//!   `getSimpleName`, `getEnclosedElements` and `getEnclosingElement` are
//!   inherited from `Element`/`QualifiedNameable` and not repeated.

use super::element::Element;
use super::package_element::PackageElement;
use super::qualified_nameable::QualifiedNameable;
use super::type_element::TypeElement;
use javac_strings::JavaString;
use std::rc::Rc;

/// Represents a module program element.  Provides access to
/// information about the module, its directives, and its members.
///
/// @see javax.lang.model.util.Elements#getModuleOf
/// @since 9
/// @jls 7.7 Module Declarations
pub trait ModuleElement: Element + QualifiedNameable {
    /// {@return true if this is an open module and false otherwise}
    fn is_open(&self) -> bool;

    /// {@return true if this is an unnamed module and false otherwise}
    ///
    /// @jls 7.7.5 Unnamed Modules
    fn is_unnamed(&self) -> bool;

    /// Returns the directives contained in the declaration of this module.
    /// @return  the directives in the declaration of this module
    fn get_directives(&self) -> Vec<Rc<dyn Directive>>;
}

/// The kind of a directive.
///
/// <p>Note that it is possible additional directive kinds will be added
/// to accommodate new, currently unknown, language structures added to
/// future versions of the Java programming language.
///
/// @since 9
///
/// PORTING_RULES: Java enum constants keep their Java names and order.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DirectiveKind {
    /// A "requires (static|transitive)* module-name" directive.
    REQUIRES,
    /// An "exports package-name [to module-name-list]" directive.
    EXPORTS,
    /// An "opens package-name [to module-name-list]" directive.
    OPENS,
    /// A "uses service-name" directive.
    USES,
    /// A "provides service-name with implementation-name" directive.
    PROVIDES,
}

impl DirectiveKind {
    /// Java: enum#name() (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            DirectiveKind::REQUIRES => "REQUIRES",
            DirectiveKind::EXPORTS => "EXPORTS",
            DirectiveKind::OPENS => "OPENS",
            DirectiveKind::USES => "USES",
            DirectiveKind::PROVIDES => "PROVIDES",
        })
    }

    /// Java: enum#values().
    pub fn values() -> [DirectiveKind; 5] {
        [
            DirectiveKind::REQUIRES,
            DirectiveKind::EXPORTS,
            DirectiveKind::OPENS,
            DirectiveKind::USES,
            DirectiveKind::PROVIDES,
        ]
    }
}

impl std::fmt::Display for DirectiveKind {
    /// Java: Enum.toString() = name().
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}

/// Represents a directive within the declaration of this module.  The
/// directives of a module declaration configure the module in the Java
/// Platform Module System.
///
/// @since 9
///
/// PORTING_RULES: Java nested interface ModuleElement.Directive ->
/// module_element::Directive; `Object.toString()` -> Display supertrait.
pub trait Directive: std::fmt::Display {
    /// {@return the kind of this directive}
    /// <ul>
    ///
    /// <li> The kind of a requires directive is REQUIRES.
    ///
    /// <li> The kind of an exports directive is EXPORTS.
    ///
    /// <li> The kind of an opens directive is OPENS.
    ///
    /// <li> The kind of a uses directive is USES.
    ///
    /// <li> The kind of a provides directive is PROVIDES.
    ///
    /// </ul>
    fn get_kind(&self) -> DirectiveKind;

    /// Applies a visitor to this directive.
    ///
    /// @param <R> the return type of the visitor's methods
    /// @param <P> the type of the additional parameter to the visitor's methods
    /// @param v   the visitor operating on this directive
    /// @param p   additional parameter to the visitor
    /// @return a visitor-specified result
    ///
    /// PORTING_RULES: `where Self: Sized` keeps Directive object safe.
    fn accept<R, P>(&self, v: &mut dyn DirectiveVisitor<R, P>, p: P) -> R
    where
        Self: Sized;

    // PORTING_RULES: Object conversion contract (same rationale as
    // Element::as_*): Java DirectiveFilter code casts a Directive to its
    // subinterface after checking getKind(); Rust needs an object-safe accessor.
    // Default None, overridden by concrete implementations.
    fn as_requires_directive(&self) -> Option<&dyn RequiresDirective> {
        None
    }

    fn as_exports_directive(&self) -> Option<&dyn ExportsDirective> {
        None
    }

    fn as_opens_directive(&self) -> Option<&dyn OpensDirective> {
        None
    }

    fn as_provides_directive(&self) -> Option<&dyn ProvidesDirective> {
        None
    }

    fn as_uses_directive(&self) -> Option<&dyn UsesDirective> {
        None
    }
}

/// A visitor of module directives, in the style of the visitor design
/// pattern.  Classes implementing this interface are used to operate
/// on a directive when the kind of directive is unknown at compile time.
///
/// <p> Classes implementing this interface may or may not throw a
/// NullPointerException if the additional parameter p
/// is null; see documentation of the implementing class for
/// details.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @since 9
pub trait DirectiveVisitor<R, P> {
    /// Visits a requires directive.
    /// @param d  the directive to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    fn visit_requires(&mut self, d: &dyn RequiresDirective, p: P) -> R;

    /// Visits an exports directive.
    /// @param d  the directive to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    fn visit_exports(&mut self, d: &dyn ExportsDirective, p: P) -> R;

    /// Visits an opens directive.
    /// @param d  the directive to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    fn visit_opens(&mut self, d: &dyn OpensDirective, p: P) -> R;

    /// Visits a uses directive.
    /// @param d  the directive to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    fn visit_uses(&mut self, d: &dyn UsesDirective, p: P) -> R;

    /// Visits a provides directive.
    /// @param d  the directive to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    fn visit_provides(&mut self, d: &dyn ProvidesDirective, p: P) -> R;

    /// Visits an unknown directive.
    /// This can occur if the language evolves and new kinds of directive are added.
    /// @param d  the directive to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    /// @throws UnknownDirectiveException a visitor implementation may optionally throw this exception
    /// @implSpec The default implementation throws new UnknownDirectiveException(d, p).
    ///
    /// PORTING_RULES: the upstream default throws; the Rust default panics with
    /// the same message (the exception object cannot hold a `&dyn Directive`;
    /// see unknown_directive_exception.rs).
    fn visit_unknown(&mut self, d: &dyn Directive, p: P) -> R {
        let _ = p;
        panic!("Unknown directive: {}", d);
    }
}

/// A dependency of a module.
/// @since 9
pub trait RequiresDirective: Directive {
    /// {@return whether or not this is a static dependency}
    fn is_static(&self) -> bool;

    /// {@return whether or not this is a transitive dependency}
    fn is_transitive(&self) -> bool;

    /// {@return the module that is required}
    fn get_dependency(&self) -> Rc<dyn ModuleElement>;
}

/// An exported package of a module.
/// @since 9
pub trait ExportsDirective: Directive {
    /// {@return the package being exported}
    fn get_package(&self) -> Rc<dyn PackageElement>;

    /// Returns the specific modules to which the package is being exported,
    /// or null, if the package is exported to all modules which
    /// have readability to this module.
    /// @return the specific modules to which the package is being exported
    fn get_target_modules(&self) -> Option<Vec<Rc<dyn ModuleElement>>>;
}

/// An opened package of a module.
/// @since 9
pub trait OpensDirective: Directive {
    /// {@return the package being opened}
    fn get_package(&self) -> Rc<dyn PackageElement>;

    /// Returns the specific modules to which the package is being open
    /// or null, if the package is open all modules which
    /// have readability to this module.
    /// @return the specific modules to which the package is being opened
    fn get_target_modules(&self) -> Option<Vec<Rc<dyn ModuleElement>>>;
}

/// An implementation of a service provided by a module.
/// @since 9
pub trait ProvidesDirective: Directive {
    /// {@return the service being provided}
    fn get_service(&self) -> Rc<dyn TypeElement>;

    /// {@return the implementations of the service being provided}
    fn get_implementations(&self) -> Vec<Rc<dyn TypeElement>>;
}

/// A reference to a service used by a module.
/// @since 9
pub trait UsesDirective: Directive {
    /// {@return the service that is used}
    fn get_service(&self) -> Rc<dyn TypeElement>;
}

#[cfg(test)]
mod tests {
    use super::{DirectiveKind, DirectiveVisitor, Directive, RequiresDirective};
    use crate::lang::model::element::module_element::ModuleElement;
    use javac_strings::JavaString;

    #[test]
    fn directive_kind_shape() {
        assert_eq!(DirectiveKind::values().len(), 5);
        assert_eq!(DirectiveKind::values()[0], DirectiveKind::REQUIRES);
        assert_eq!(DirectiveKind::PROVIDES.name(), JavaString::from("PROVIDES"));
        assert_eq!(DirectiveKind::USES.to_string(), "USES");
    }

    // A minimal RequiresDirective used to exercise the dyn visitor.
    struct TestRequires;

    impl std::fmt::Display for TestRequires {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("requires java.base")
        }
    }

    impl Directive for TestRequires {
        fn get_kind(&self) -> DirectiveKind {
            DirectiveKind::REQUIRES
        }
        fn accept<R, P>(&self, v: &mut dyn DirectiveVisitor<R, P>, p: P) -> R {
            v.visit_requires(self, p)
        }
    }

    impl RequiresDirective for TestRequires {
        fn is_static(&self) -> bool {
            false
        }
        fn is_transitive(&self) -> bool {
            false
        }
        fn get_dependency(&self) -> std::rc::Rc<dyn ModuleElement> {
            panic!("UnsupportedOperationException: get_dependency: test fixture has no module")
        }
    }

    struct KindVisitor;

    impl DirectiveVisitor<DirectiveKind, ()> for KindVisitor {
        fn visit_requires(&mut self, _d: &dyn RequiresDirective, _p: ()) -> DirectiveKind {
            DirectiveKind::REQUIRES
        }
        fn visit_exports(
            &mut self,
            _d: &dyn super::ExportsDirective,
            _p: (),
        ) -> DirectiveKind {
            DirectiveKind::EXPORTS
        }
        fn visit_opens(&mut self, _d: &dyn super::OpensDirective, _p: ()) -> DirectiveKind {
            DirectiveKind::OPENS
        }
        fn visit_uses(&mut self, _d: &dyn super::UsesDirective, _p: ()) -> DirectiveKind {
            DirectiveKind::USES
        }
        fn visit_provides(&mut self, _d: &dyn super::ProvidesDirective, _p: ()) -> DirectiveKind {
            DirectiveKind::PROVIDES
        }
    }

    #[test]
    fn directive_accept_dispatches_through_dyn_visitor() {
        let directive = TestRequires;
        let kind = directive.accept(&mut KindVisitor, ());
        assert_eq!(kind, DirectiveKind::REQUIRES);
        // The directive itself can be used through dyn.
        let dyn_directive: &dyn Directive = &directive;
        assert_eq!(dyn_directive.get_kind(), DirectiveKind::REQUIRES);
        assert_eq!(dyn_directive.to_string(), "requires java.base");
    }
}
