// Origin: javax/lang/model/util/ElementScanner9.java @ 982d53c (jdk-17.0.19-ga)

//! A scanning visitor of program elements with default behavior appropriate for
//! source versions RELEASE_9 through RELEASE_14.
//!
//! PORTING_RULES: Java `ElementScanner9 extends ElementScanner8` only overrides
//! `visitModule` (scans the enclosed elements).  The Rust trait is a subtrait
//! re-declaring that method; calling it on a concrete type implementing both
//! requires a fully qualified call (registered language-shape limitation).

use super::element_scanner8::ElementScanner8;
use crate::lang::model::element::ModuleElement;

/// A scanning visitor of program elements with default behavior
/// appropriate for source versions RELEASE_9 through RELEASE_14.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see <a href="ElementScanner6.html#note_for_subclasses"><strong>Compatibility
/// note for subclasses</strong></a>
/// @see ElementScanner6
/// @see ElementScanner7
/// @see ElementScanner8
/// @since 9
pub trait ElementScanner9<R: Clone, P: Clone>: ElementScanner8<R, P> {
    /// {@inheritDoc}
    ///
    /// @implSpec This implementation scans the enclosed elements.
    /// TODO(upstream): the Java source carries the same "Hmmm, this might not be
    /// right" comment; it is kept for the upstream diff.
    fn visit_module(&mut self, e: &dyn ModuleElement, p: P) -> R {
        // TODO: Hmmm, this might not be right
        self.scan_all(&e.get_enclosed_elements(), p)
    }
}
