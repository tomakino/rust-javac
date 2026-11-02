//! Utilities to assist in the processing of
//! {@linkplain javax.lang.model.element program elements} and
//! {@linkplain javax.lang.model.type types}.
//!
//! <p> Unless otherwise specified in a particular implementation, the
//! collections returned by methods in this package should be expected
//! to be unmodifiable by the caller and unsafe for concurrent access.
//!
//! <p> Unless otherwise specified, methods in this package will throw
//! a {@code NullPointerException} if given a {@code null} argument.
//!
//! @author Joseph D. Darcy
//! @author Scott Seligman
//! @author Peter von der Ah&eacute;
//! @since 1.6
//!
//! `javax.lang.model.util.Elements` and `Types` are deferred
//! (registered P6 item); this package ports `ElementFilter` and the visitor /
//! scanner families (46 files).

pub mod abstract_annotation_value_visitor6;
pub mod abstract_annotation_value_visitor7;
pub mod abstract_annotation_value_visitor8;
pub mod abstract_annotation_value_visitor9;
pub mod abstract_annotation_value_visitor14;
pub mod abstract_element_visitor6;
pub mod abstract_element_visitor7;
pub mod abstract_element_visitor8;
pub mod abstract_element_visitor9;
pub mod abstract_element_visitor14;
pub mod abstract_type_visitor6;
pub mod abstract_type_visitor7;
pub mod abstract_type_visitor8;
pub mod abstract_type_visitor9;
pub mod abstract_type_visitor14;
pub mod element_filter;
pub mod elements;
pub mod element_kind_visitor6;
pub mod element_kind_visitor7;
pub mod element_kind_visitor8;
pub mod element_kind_visitor9;
pub mod element_kind_visitor14;
pub mod element_scanner6;
pub mod element_scanner7;
pub mod element_scanner8;
pub mod element_scanner9;
pub mod element_scanner14;
pub mod simple_annotation_value_visitor6;
pub mod simple_annotation_value_visitor7;
pub mod simple_annotation_value_visitor8;
pub mod simple_annotation_value_visitor9;
pub mod simple_annotation_value_visitor14;
pub mod simple_element_visitor6;
pub mod simple_element_visitor7;
pub mod simple_element_visitor8;
pub mod simple_element_visitor9;
pub mod simple_element_visitor14;
pub mod simple_type_visitor6;
pub mod simple_type_visitor7;
pub mod simple_type_visitor8;
pub mod simple_type_visitor9;
pub mod simple_type_visitor14;
pub mod type_kind_visitor6;
pub mod types;
pub mod type_kind_visitor7;
pub mod type_kind_visitor8;
pub mod type_kind_visitor9;
pub mod type_kind_visitor14;

#[cfg(test)]
pub(crate) mod test_support;
