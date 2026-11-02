//! A scanning visitor of program elements with default behavior appropriate for
//! the RELEASE_8 source version.
//!
//! PORTING_RULES: Java `ElementScanner8 extends ElementScanner7` adds only
//! constructors; the Rust trait is an empty subtrait that keeps the inherited
//! default behavior.

use super::element_scanner7::ElementScanner7;

/// A scanning visitor of program elements with default behavior
/// appropriate for the RELEASE_8 source version.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @see <a href="ElementScanner6.html#note_for_subclasses"><strong>Compatibility
/// note for subclasses</strong></a>
/// @see ElementScanner6
/// @see ElementScanner7
/// @see ElementScanner9
/// @see ElementScanner14
/// @since 1.8
pub trait ElementScanner8<R: Clone, P: Clone>: ElementScanner7<R, P> {}
