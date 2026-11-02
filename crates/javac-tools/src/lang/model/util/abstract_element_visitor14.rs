//! A skeletal visitor of program elements with default behavior appropriate
//! for the RELEASE_14 source version.
//!
//! PORTING_RULES: Java abstract class `AbstractElementVisitor14 extends
//! AbstractElementVisitor6` adds only constructors; in addition it makes visitRecordComponent abstract (Rust cannot cancel a default method in a subtrait, registered difference); the Rust trait is
//! an empty subtrait of `AbstractElementVisitor6`, which keeps the same default
//! behavior.  Implementations implement each level of the Java class hierarchy
//! explicitly (registered language-shape limitation).

use super::abstract_element_visitor6::AbstractElementVisitor6;

/// A skeletal visitor of program elements with default behavior
/// appropriate for the RELEASE_14 source version.
///
/// @param <R> the return type of this visitor's methods.  Use Void
///            for visitors that do not need to return results.
/// @param <P> the type of the additional parameter to this visitor's
///            methods.  Use Void for visitors that do not need an
///            additional parameter.
///
/// @see <a href="AbstractElementVisitor6.html#note_for_subclasses">
/// <strong>Compatibility note for subclasses</strong></a>
/// @see AbstractElementVisitor6
/// @since 16
pub trait AbstractElementVisitor14<R, P>: AbstractElementVisitor6<R, P> {}
