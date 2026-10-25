// Origin: javax/lang/model/element/Name.java @ 982d53c (jdk-17.0.19-ga)

//! An immutable sequence of characters.
//!
//! PORTING_RULES — signature mapping:
//! * Java `Name extends CharSequence`: the `CharSequence` methods used by this
//!   API are represented explicitly with Java String semantics
//!   (`javac_strings::JavaString`): `length`, `char_at`, `to_java_string`
//!   (Java `toString`); `subSequence(int, int)` -> `sub_sequence` returning
//!   the minimal sequence representation `JavaString`.
//! * `contentEquals(CharSequence)` takes `&JavaString`: every CharSequence used
//!   by the ported compiler is a Java String.
//! * Java `equals(Object)`/`hashCode()` are Object methods; Rust object
//!   identity is carried by the `Rc<dyn Name>` references and content equality
//!   by `content_equals`.  This keeps the trait object safe.

use javac_strings::JavaString;

/// An immutable sequence of characters.  When created by the same
/// implementation, objects implementing this interface must obey the
/// general {@linkplain Object#equals equals contract} when compared
/// with each other.  Therefore, Name objects from the same
/// implementation are usable in collections while Names from
/// different implementations may not work properly in collections.
///
/// <p id="empty_name">An {@linkplain CharSequence#isEmpty() empty}
/// Name has a {@linkplain CharSequence#length() length} of
/// zero.
///
/// <p>In the context of annotation processing, the guarantees for "the
/// same" implementation must include contexts where the API mediated side
/// effects of processors could be visible to each other, including
/// successive annotation processing rounds.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @see javax.lang.model.util.Elements#getName
/// @since 1.6
pub trait Name {
    /// Java: `CharSequence.length()`.
    fn length(&self) -> usize;

    /// Java: `CharSequence.charAt(int index)`.
    fn char_at(&self, index: usize) -> u16;

    /// Java: `CharSequence.subSequence(int start, int end)`.
    ///
    /// PORTING_RULES: returns the minimal CharSequence representation
    /// (`JavaString`); see the file header.
    fn sub_sequence(&self, start: usize, end: usize) -> JavaString;

    /// Java: `CharSequence.toString()` (the text of the name).
    fn to_java_string(&self) -> JavaString;

    /// Returns true if the argument represents the same
    /// name as this, and false otherwise.
    ///
    /// <p>Note that the identity of a Name is a function both
    /// of its content in terms of a sequence of characters as well as
    /// the implementation which created it.
    ///
    /// @param cs The sequence to compare this name against
    /// @return true if this name represents the same sequence
    /// of char values as the specified sequence, false
    /// otherwise
    /// @see String#contentEquals(CharSequence)
    fn content_equals(&self, cs: &JavaString) -> bool;
}
