//
// Rust:
// - the Context singleton instance is not expressed (ADR 0001); Symtab creates
//   one handler and stores it on the RootPackageSymbol like upstream;
// - the upstream getParameterName(ParamSymbol) uses ParamSymbol's
//   VariableElement view; the port exposes the provider call on
//   VariableElement directly (the Symbol/Element API layer is not ported).

use std::rc::Rc;

use javac_source::util::parameter_name_provider::ParameterNameProvider;
use javac_tools::lang::model::element::VariableElement;

use crate::util::java_string::JavaString;
use crate::util::name::Name;
use crate::util::names::Names;

/// Java: public class MissingInfoHandler (MissingInfoHandler.java:43-75)
pub struct MissingInfoHandler {
    names: Rc<Names>,
    parameter_name_provider: Option<Rc<dyn ParameterNameProvider>>,
}

impl MissingInfoHandler {
    /// Adapter for MissingInfoHandler.instance(context): the Symtab constructor
    /// creates the handler with its Names (ADR 0001 explicit-context adapter).
    pub fn new(names: Rc<Names>) -> MissingInfoHandler {
        MissingInfoHandler {
            names,
            parameter_name_provider: None,
        }
    }

    /// Java: public Name getParameterName(ParamSymbol parameter)
    /// (MissingInfoHandler.java:61-70). The provider returns a CharSequence;
    /// null -> None.
    pub fn get_parameter_name(&self, parameter: &dyn VariableElement) -> Option<Name> {
        let name: Option<JavaString> = self
            .parameter_name_provider
            .as_ref()
            .and_then(|provider| provider.get_parameter_name(parameter));
        name.map(|name| self.names.from_string(&name))
    }

    /// Java: public void setDelegate(ParameterNameProvider delegate)
    /// (MissingInfoHandler.java:72-74).
    pub fn set_delegate(&mut self, delegate: Option<Rc<dyn ParameterNameProvider>>) {
        self.parameter_name_provider = delegate;
    }
}
