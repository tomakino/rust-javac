//! A tree node for an attribute in an HTML element.
//!
//! Java interface -> Rust trait; the nested enum ValueKind lives
//! in this file; `Name getName()` -> Rc<dyn Name>.

use super::doc_tree::DocTree;
use javac_strings::JavaString;
use javac_tools::lang::model::element::Name;
use std::rc::Rc;

/// A tree node for an attribute in an HTML element.
///
/// @since 1.8
pub trait AttributeTree: DocTree {
    /// Returns the name of the attribute.
    /// @return the name of the attribute
    fn get_name(&self) -> Rc<dyn Name>;

    /// Returns the kind of the attribute value.
    /// @return the kind of the attribute value
    fn get_value_kind(&self) -> ValueKind;

    /// Returns the value of the attribute, or {@code null} if the
    /// {@linkplain #getValueKind() kind of this attribute} is {@code EMPTY}.
    /// @return the value of the attribute
    fn get_value(&self) -> Option<Vec<Rc<dyn DocTree>>>;
}

/// The kind of an attribute value.
///
/// Java nested enum AttributeTree.ValueKind ->
/// attribute_tree::ValueKind.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueKind {
    /// The attribute value is empty.
    EMPTY,
    /// The attribute value is not enclosed in quotes.
    UNQUOTED,
    /// The attribute value is enclosed in single quotation marks.
    SINGLE,
    /// The attribute value is enclosed in double quotation marks.
    DOUBLE,
}

impl ValueKind {
    /// Java: enum#name() (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            ValueKind::EMPTY => "EMPTY",
            ValueKind::UNQUOTED => "UNQUOTED",
            ValueKind::SINGLE => "SINGLE",
            ValueKind::DOUBLE => "DOUBLE",
        })
    }

    /// Java: enum#values().
    pub fn values() -> [ValueKind; 4] {
        [ValueKind::EMPTY, ValueKind::UNQUOTED, ValueKind::SINGLE, ValueKind::DOUBLE]
    }
}

impl std::fmt::Display for ValueKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}
