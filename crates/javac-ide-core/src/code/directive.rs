//! Root class for the directives that may appear in module compilation units.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! the upstream abstract Directive class is represented by the
//! Directive enum (one variant per subclass); Symbol references are
//! Option<SymbolId>; List<ModuleSymbol> is Option<List<SymbolId>> (the field is
//! nullable upstream); Set<RequiresFlag>/Set<ExportsFlag>/Set<OpensFlag>
//! (EnumSet) are Vec<Flag> built in enum order (the upstream EnumSet iteration
//! order; registered section 9); getKind() returns the javax.lang.model
//! DirectiveKind from the ported javac-tools API layer.
//!
//! NOT PORTED IN THIS STEP (see docs/STATUS.md; no placeholder logic):
//! - toString(): needs the Symbol printer;
//! - equals/hashCode of ProvidesDirective/UsesDirective: the upstream equals is
//!   reference equality for the other directives and field equality for these
//!   two; Directive values are not arena values here, so the reference-equality
//!   part has no direct Rust mapping (needs the directive arena design);
//! - accept(ModuleElement.DirectiveVisitor): the API dispatch is part of the
//!   javax.lang.model API layer (javac-tools has the visitor interface; the
//!   implementation dispatch belongs to the API layer, like the com.sun.source
//!   tree view).

use crate::ids::SymbolId;
use crate::util::list::List;
use javac_tools::lang::model::element::module_element::DirectiveKind;

/// Flags for RequiresDirective.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RequiresFlag {
    TRANSITIVE,
    STATIC_PHASE,
    SYNTHETIC,
    MANDATED,
    EXTRA,
}

impl RequiresFlag {
    /// Java: public final int value
    pub const fn value(self) -> i32 {
        match self {
            RequiresFlag::TRANSITIVE => 0x0020,
            RequiresFlag::STATIC_PHASE => 0x0040,
            RequiresFlag::SYNTHETIC => 0x1000,
            RequiresFlag::MANDATED => 0x8000,
            RequiresFlag::EXTRA => 0x10000,
        }
    }

    /// Java: enum values() (declaration order).
    pub fn values() -> Vec<RequiresFlag> {
        vec![
            RequiresFlag::TRANSITIVE,
            RequiresFlag::STATIC_PHASE,
            RequiresFlag::SYNTHETIC,
            RequiresFlag::MANDATED,
            RequiresFlag::EXTRA,
        ]
    }

    /// Java: public static int value(Set<RequiresFlag> s)
    pub fn value_of(flags: &[RequiresFlag]) -> i32 {
        let mut v = 0;
        for flag in flags {
            v |= flag.value();
        }
        v
    }
}

/// Flags for ExportsDirective.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExportsFlag {
    SYNTHETIC,
    MANDATED,
}

impl ExportsFlag {
    pub const fn value(self) -> i32 {
        match self {
            ExportsFlag::SYNTHETIC => 0x1000,
            ExportsFlag::MANDATED => 0x8000,
        }
    }

    pub fn values() -> Vec<ExportsFlag> {
        vec![ExportsFlag::SYNTHETIC, ExportsFlag::MANDATED]
    }

    /// Java: public static int value(Set<ExportsFlag> s)
    pub fn value_of(flags: &[ExportsFlag]) -> i32 {
        let mut v = 0;
        for flag in flags {
            v |= flag.value();
        }
        v
    }
}

/// Flags for OpensDirective.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OpensFlag {
    SYNTHETIC,
    MANDATED,
}

impl OpensFlag {
    pub const fn value(self) -> i32 {
        match self {
            OpensFlag::SYNTHETIC => 0x1000,
            OpensFlag::MANDATED => 0x8000,
        }
    }

    pub fn values() -> Vec<OpensFlag> {
        vec![OpensFlag::SYNTHETIC, OpensFlag::MANDATED]
    }

    /// Java: public static int value(Set<OpensFlag> s)
    pub fn value_of(flags: &[OpensFlag]) -> i32 {
        let mut v = 0;
        for flag in flags {
            v |= flag.value();
        }
        v
    }
}

/// Java: public abstract class Directive (one variant per subclass).
pub enum Directive {
    /// 'exports' Package ';' / 'exports' Package 'to' ModuleList ';'
    Exports {
        /// The exported package.
        packge: Option<SymbolId>,
        /// The target modules (null = unqualified exports).
        modules: Option<List<SymbolId>>,
        /// The directive flags.
        flags: Vec<ExportsFlag>,
    },
    /// 'opens' Package ';' / 'opens' Package 'to' ModuleList ';'
    Opens {
        /// The opened package.
        packge: Option<SymbolId>,
        /// The target modules (null = unqualified opens).
        modules: Option<List<SymbolId>>,
        /// The directive flags.
        flags: Vec<OpensFlag>,
    },
    /// 'provides' ServiceName 'with' QualifiedIdentifier ';'
    Provides {
        /// The provided service.
        service: Option<SymbolId>,
        /// The implementations.
        impls: List<SymbolId>,
    },
    /// 'requires' ('static' | 'transitive')* ModuleName ';'
    Requires {
        /// The required module.
        module: Option<SymbolId>,
        /// The directive flags.
        flags: Vec<RequiresFlag>,
    },
    /// 'uses' ServiceName ';'
    Uses {
        /// The used service.
        service: Option<SymbolId>,
    },
}

impl Directive {
    /// Java: public ModuleElement.DirectiveKind getKind()
    pub fn get_kind(&self) -> DirectiveKind {
        match self {
            Directive::Exports { .. } => DirectiveKind::EXPORTS,
            Directive::Opens { .. } => DirectiveKind::OPENS,
            Directive::Provides { .. } => DirectiveKind::PROVIDES,
            Directive::Requires { .. } => DirectiveKind::REQUIRES,
            Directive::Uses { .. } => DirectiveKind::USES,
        }
    }

    /// Java: public PackageSymbol getPackage() (Exports/Opens)
    pub fn get_package(&self) -> Option<SymbolId> {
        match self {
            Directive::Exports { packge, .. } | Directive::Opens { packge, .. } => *packge,
            _ => panic!("ClassCastException: not an exports/opens directive"),
        }
    }

    /// Java: public java.util.List<ModuleSymbol> getTargetModules()
    pub fn get_target_modules(&self) -> Option<List<SymbolId>> {
        match self {
            Directive::Exports { modules, .. } | Directive::Opens { modules, .. } => {
                modules.clone()
            }
            _ => panic!("ClassCastException: not an exports/opens directive"),
        }
    }

    /// Java: public boolean isStatic() (RequiresDirective)
    pub fn is_static(&self) -> bool {
        match self {
            Directive::Requires { flags, .. } => flags.contains(&RequiresFlag::STATIC_PHASE),
            _ => panic!("ClassCastException: not a requires directive"),
        }
    }

    /// Java: public boolean isTransitive() (RequiresDirective)
    pub fn is_transitive(&self) -> bool {
        match self {
            Directive::Requires { flags, .. } => flags.contains(&RequiresFlag::TRANSITIVE),
            _ => panic!("ClassCastException: not a requires directive"),
        }
    }

    /// Java: public ModuleSymbol getDependency() (RequiresDirective)
    pub fn get_dependency(&self) -> Option<SymbolId> {
        match self {
            Directive::Requires { module, .. } => *module,
            _ => panic!("ClassCastException: not a requires directive"),
        }
    }

    /// Java: public ClassSymbol getService() (Provides/Uses)
    pub fn get_service(&self) -> Option<SymbolId> {
        match self {
            Directive::Provides { service, .. } | Directive::Uses { service } => *service,
            _ => panic!("ClassCastException: not a provides/uses directive"),
        }
    }

    /// Java: public List<ClassSymbol> getImplementations() (ProvidesDirective)
    pub fn get_implementations(&self) -> List<SymbolId> {
        match self {
            Directive::Provides { impls, .. } => impls.clone(),
            _ => panic!("ClassCastException: not a provides directive"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_flags_and_values() {
        assert_eq!(RequiresFlag::TRANSITIVE.value(), 0x0020);
        assert_eq!(RequiresFlag::EXTRA.value(), 0x10000);
        assert_eq!(RequiresFlag::values().len(), 5);
        assert_eq!(
            RequiresFlag::value_of(&[RequiresFlag::TRANSITIVE, RequiresFlag::STATIC_PHASE]),
            0x0060
        );
        assert_eq!(ExportsFlag::value_of(&[ExportsFlag::MANDATED]), 0x8000);
        assert_eq!(OpensFlag::value_of(&[OpensFlag::SYNTHETIC]), 0x1000);
    }

    #[test]
    fn directive_kinds_and_fields() {
        let package = SymbolId::new(1);
        let module_a = SymbolId::new(2);
        let module_b = SymbolId::new(3);
        let exports = Directive::Exports {
            packge: Some(package),
            modules: Some(List::of2(module_a, module_b)),
            flags: vec![ExportsFlag::SYNTHETIC],
        };
        assert_eq!(exports.get_kind(), DirectiveKind::EXPORTS);
        assert_eq!(exports.get_package(), Some(package));
        assert_eq!(exports.get_target_modules().expect("modules").length(), 2);

        let requires = Directive::Requires {
            module: Some(module_a),
            flags: vec![RequiresFlag::TRANSITIVE, RequiresFlag::STATIC_PHASE],
        };
        assert_eq!(requires.get_kind(), DirectiveKind::REQUIRES);
        assert!(requires.is_transitive());
        assert!(requires.is_static());
        assert_eq!(requires.get_dependency(), Some(module_a));

        let provides = Directive::Provides {
            service: Some(module_a),
            impls: List::of(module_b),
        };
        assert_eq!(provides.get_kind(), DirectiveKind::PROVIDES);
        assert_eq!(provides.get_service(), Some(module_a));
        assert_eq!(provides.get_implementations().length(), 1);

        let uses = Directive::Uses {
            service: Some(module_b),
        };
        assert_eq!(uses.get_kind(), DirectiveKind::USES);
        assert_eq!(uses.get_service(), Some(module_b));
    }

    #[test]
    #[should_panic(expected = "not a requires directive")]
    fn requires_methods_panic_for_other_directives() {
        let uses = Directive::Uses { service: None };
        let _ = uses.is_static();
    }
}
