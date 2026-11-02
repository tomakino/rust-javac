//! A visitor of program elements, in the style of the visitor design pattern.
//!
//! — signature mapping:
//! * visit methods take `&mut self` (a Java visitor is a mutable object; the
//!   ported visitors keep state in `&mut self`), the visited element as
//!   `&dyn X`, and the parameter `P` by value.
//! * the single-argument convenience overloads (`visit(Element)`,
//!   `visitPackage(PackageElement)`, ...) only forward `null` as the parameter
//!   upstream; `null` has no Rust form for an unconstrained `P`, so the
//!   convenience overloads are not ported (registered difference).  Callers
//!   pass the `P` value explicitly.
//! * the Java `default` methods `visitModule`/`visitRecordComponent` that
//!   forward to `visitUnknown` become trait default methods, exactly as
//!   upstream.

use super::executable_element::ExecutableElement;
use super::element::Element;
use super::module_element::ModuleElement;
use super::package_element::PackageElement;
use super::record_component_element::RecordComponentElement;
use super::type_element::TypeElement;
use super::type_parameter_element::TypeParameterElement;
use super::variable_element::VariableElement;

/// A visitor of program elements, in the style of the visitor design
/// pattern.  Classes implementing this interface are used to operate
/// on an element when the kind of element is unknown at compile time.
/// When a visitor is passed to an element's {@link Element#accept
/// accept} method, the visitXyz method most applicable
/// to that element is invoked.
///
/// <p> Classes implementing this interface may or may not throw a
/// NullPointerException if the additional parameter p
/// is null; see documentation of the implementing class for
/// details.
///
/// <p>Methods to accommodate new language constructs are expected to
/// be added as default methods to provide strong source compatibility,
/// as done for {@link visitModule visitModule}. The implementations of
/// the default methods will in turn call {@link visitUnknown
/// visitUnknown}, behavior that will be overridden in concrete
/// visitors supporting the source version with the new language
/// construct.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @since 1.6
pub trait ElementVisitor<R, P> {
    /// Visits an element.
    /// @param e  the element to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    fn visit(&mut self, _e: &dyn Element, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit: Rust: visitor interface method not overridden")
    }

    /// Visits a package element.
    /// @param e  the element to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    fn visit_package(&mut self, _e: &dyn PackageElement, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_package: Rust: visitor interface method not overridden")
    }

    /// Visits a type element.
    /// @param e  the element to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    fn visit_type(&mut self, _e: &dyn TypeElement, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_type: Rust: visitor interface method not overridden")
    }

    /// Visits a variable element.
    /// @param e  the element to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    fn visit_variable(&mut self, _e: &dyn VariableElement, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_variable: Rust: visitor interface method not overridden")
    }

    /// Visits an executable element.
    /// @param e  the element to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    fn visit_executable(&mut self, _e: &dyn ExecutableElement, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_executable: Rust: visitor interface method not overridden")
    }

    /// Visits a type parameter element.
    /// @param e  the element to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    fn visit_type_parameter(&mut self, _e: &dyn TypeParameterElement, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_type_parameter: Rust: visitor interface method not overridden")
    }

    /// Visits an unknown kind of element.
    /// This can occur if the language evolves and new kinds
    /// of elements are added to the Element hierarchy.
    ///
    /// @param e  the element to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    /// @throws UnknownElementException
    ///  a visitor implementation may optionally throw this exception
    fn visit_unknown(&mut self, _e: &dyn Element, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_unknown: Rust: visitor interface method not overridden")
    }

    /// Visits a module element.
    ///
    /// @implSpec The default implementation visits a ModuleElement by calling
    /// visitUnknown(e, p).
    ///
    /// @param e  the element to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    /// @since 9
    fn visit_module(&mut self, e: &dyn ModuleElement, p: P) -> R {
        self.visit_unknown(e, p)
    }

    /// Visits a record component element.
    ///
    /// @implSpec The default implementation visits a RecordComponentElement by
    /// calling visitUnknown(e, p).
    ///
    /// @param e  the element to visit
    /// @param p  a visitor-specified parameter
    /// @return a visitor-specified result
    /// @since 16
    fn visit_record_component(&mut self, e: &dyn RecordComponentElement, p: P) -> R {
        self.visit_unknown(e, p)
    }
}
