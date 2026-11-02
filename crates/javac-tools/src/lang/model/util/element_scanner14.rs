//! A scanning visitor of program elements with default behavior appropriate for
//! the RELEASE_14 source version.
//!
//! Java `ElementScanner14 extends ElementScanner9` overrides
//! `visitType` and `visitExecutable` to scan the type parameters before the
//! enclosed elements / parameters, and `visitRecordComponent`.  The Rust trait
//! re-declares those methods; calling them on a concrete type implementing both
//! requires a fully qualified call (registered language-shape limitation).  The
//! Java `private createScanningList` helper has no trait-form conflict and is a
//! module-private function here.

use super::element_scanner9::ElementScanner9;
use crate::lang::model::element::{
    Element, ExecutableElement, Parameterizable, RecordComponentElement, TypeElement,
};
use std::rc::Rc;

/// A scanning visitor of program elements with default behavior
/// appropriate for the RELEASE_14 source version.
///
/// @param <R> the return type of this visitor's methods.  Use Void
///            for visitors that do not need to return results.
/// @param <P> the type of the additional parameter to this visitor's
///            methods.  Use Void for visitors that do not need an
///            additional parameter.
///
/// @see <a href="ElementScanner6.html#note_for_subclasses"><strong>Compatibility
/// note for subclasses</strong></a>
/// @see ElementScanner6
/// @see ElementScanner7
/// @see ElementScanner8
/// @see ElementScanner9
/// @since 16
pub trait ElementScanner14<R: Clone, P: Clone>: ElementScanner9<R, P> {
    /// {@inheritDoc}
    ///
    /// @implSpec This implementation scans the type parameters, if
    /// any, and then the enclosed elements.
    fn visit_type(&mut self, e: &dyn TypeElement, p: P) -> R {
        let scanning_list = create_scanning_list(e, e.get_enclosed_elements());
        self.scan_all(&scanning_list, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation first scans the type parameters, if any, and then
    /// the parameters.
    fn visit_executable(&mut self, e: &dyn ExecutableElement, p: P) -> R {
        let parameters: Vec<Rc<dyn Element>> = e
            .get_parameters()
            .into_iter()
            .map(|parameter| parameter as Rc<dyn Element>)
            .collect();
        let scanning_list = create_scanning_list(e, parameters);
        self.scan_all(&scanning_list, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation scans the enclosed elements.
    fn visit_record_component(&mut self, e: &dyn RecordComponentElement, p: P) -> R {
        self.scan_all(&e.get_enclosed_elements(), p)
    }
}

/// Java: `private List<? extends Element> createScanningList(Parameterizable
/// element, List<? extends Element> toBeScanned)` of ElementScanner14.
fn create_scanning_list(
    element: &dyn Parameterizable,
    to_be_scanned: Vec<Rc<dyn Element>>,
) -> Vec<Rc<dyn Element>> {
    let type_parameters = element.get_type_parameters();
    if type_parameters.is_empty() {
        to_be_scanned
    } else {
        let mut scanning_list: Vec<Rc<dyn Element>> = type_parameters
            .into_iter()
            .map(|parameter| parameter as Rc<dyn Element>)
            .collect();
        scanning_list.extend(to_be_scanned);
        scanning_list
    }
}
