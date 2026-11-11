//! Navigation: definition, type definition, implementations, references,
//! document symbols.
//!
//! Implementation files:
//!
//! - `source`:     shared text/tree lookup helpers
//! - `definition`: go-to-definition, type definition, implementations
//! - `references`: references and prepare-rename
//! - `symbols`:    outline document symbols

mod definition;
mod references;
mod source;
mod super_methods;
mod symbols;

pub(crate) use super_methods::same_override_display;

use crate::compiler_context::CompilerContext;
use crate::ids::SymbolId;

/// Whether `class` is declared in a JDK system module (ct.sym / platform
/// modules), judged from the module of its package rather than a name prefix.
///
/// Shared by super-method navigation and constructor navigation: both choose
/// between the JDK `src.zip` and a class-path `-sources.jar`.
pub(crate) fn is_system_module_class(ctx: &mut CompilerContext, class: SymbolId) -> bool {
    let mut current = class;
    loop {
        let symbol = ctx.sym(current);
        if let crate::code::symbol::SymbolKind::PackageSymbol { modle, .. } = &symbol.kind_data {
            let Some(module) = *modle else {
                return false;
            };
            let name = ctx
                .sym(module)
                .name
                .to_java_string()
                .to_string_lossy()
                .to_string();
            return name.starts_with("java.") || name.starts_with("jdk.");
        }
        match symbol.owner {
            Some(owner) => current = owner,
            None => return false,
        }
    }
}
