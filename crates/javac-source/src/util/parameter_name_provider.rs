//! A provider for parameter names when the parameter names are not determined
//! from a reliable source, like a classfile.
//!
//! PORTING_RULES: Java interface -> Rust trait; `CharSequence` (may be null) ->
//! `Option<JavaString>`.

use javac_strings::JavaString;
use javac_tools::lang::model::element::VariableElement;

/// A provider for parameter names when the parameter names are not determined from
/// a reliable source, like a classfile.
///
/// @since 13
pub trait ParameterNameProvider {
    /// Infer a parameter name for the given parameter. The implementations of this method
    /// should infer parameter names in such a way that the parameter names are distinct
    /// for any given owning method.
    ///
    /// If the implementation of this method returns null, an automatically synthesized name is used.
    ///
    /// @param parameter the parameter for which the name should be inferred
    /// @return a user-friendly name for the parameter, or null if unknown
    fn get_parameter_name(&self, parameter: &dyn VariableElement) -> Option<JavaString>;
}
