// Origin: com/sun/tools/javac/code/Kinds.java @ 982d53c (jdk-17.0.19-ga)

//! Internal symbol kinds which distinguish between elements of different
//! subclasses of Symbol. Symbol kinds are organized so they can be or'ed to
//! sets.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: the Java enum constructors store (category, kindName/
//! absentKind, selector) per constant; Rust expresses those as match-based
//! accessors. KindSelector/RequiresFlag-style EnumSet helpers are reproduced
//! with the upstream bit values. kindName(Symbol)/typeKindName(Type) read the
//! symbol kind/flags through the CompilerContext accessors (ADR 0001).

use javac_source::tree::member_reference_tree::ReferenceMode;
use javac_tools::lang::model::element::element_kind::ElementKind;

use crate::code::flags;
use crate::code::symbol::Symbol;
use crate::code::type_::Type;
use crate::code::type_tag::TypeTag;
use crate::compiler_context::CompilerContext;

/// Kind of symbols.
///
/// Java: Kinds.Kind. PORTING_RULES: the Java enum constructor stores
/// (category, kindName/absentKind, selector) per constant; Rust expresses
/// those as match-based accessors.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    NIL,
    PCK,
    TYP,
    VAR,
    MTH,
    POLY,
    MDL,
    ERR,
    AMBIGUOUS,
    HIDDEN,
    STATICERR,
    MISSING_ENCL,
    BAD_RESTRICTED_TYPE,
    ABSENT_VAR,
    WRONG_MTHS,
    WRONG_MTH,
    ABSENT_MTH,
    ABSENT_TYP,
}

/// Java: private enum Category
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Category {
    BASIC,
    ERROR,
    RESOLUTION,
    RESOLUTION_TARGET,
}

impl Kind {
    /// Java: enum ordinal() (the declaration order is the ordinal).
    pub fn ordinal(self) -> usize {
        self as usize
    }

    /// Java: private final Category category (constructor argument).
    fn category(self) -> Category {
        match self {
            Kind::NIL
            | Kind::PCK
            | Kind::TYP
            | Kind::VAR
            | Kind::MTH
            | Kind::POLY
            | Kind::MDL => Category::BASIC,
            Kind::ERR => Category::ERROR,
            Kind::MISSING_ENCL | Kind::BAD_RESTRICTED_TYPE => Category::RESOLUTION,
            Kind::AMBIGUOUS
            | Kind::HIDDEN
            | Kind::STATICERR
            | Kind::ABSENT_VAR
            | Kind::WRONG_MTHS
            | Kind::WRONG_MTH
            | Kind::ABSENT_MTH
            | Kind::ABSENT_TYP => Category::RESOLUTION_TARGET,
        }
    }

    /// Java: public KindSelector toSelector()
    pub fn to_selector(self) -> KindSelector {
        match self {
            Kind::NIL => KindSelector::NIL,
            Kind::PCK => KindSelector::PCK,
            Kind::TYP => KindSelector::TYP,
            Kind::VAR => KindSelector::VAR,
            Kind::MTH => KindSelector::MTH,
            Kind::POLY => KindSelector::POLY,
            Kind::MDL => KindSelector::MDL,
            Kind::ERR => KindSelector::ERR,
            _ => KindSelector::NIL,
        }
    }

    /// Java: public boolean matches(KindSelector kindSelectors)
    pub fn matches(self, kind_selectors: KindSelector) -> bool {
        self.to_selector().contains(kind_selectors)
    }

    /// Java: public boolean isResolutionError()
    pub fn is_resolution_error(self) -> bool {
        matches!(
            self.category(),
            Category::RESOLUTION | Category::RESOLUTION_TARGET
        )
    }

    /// Java: public boolean isResolutionTargetError()
    pub fn is_resolution_target_error(self) -> bool {
        self.category() == Category::RESOLUTION_TARGET
    }

    /// Java: public boolean isValid()
    pub fn is_valid(self) -> bool {
        self.category() == Category::BASIC
    }

    /// Java: public boolean betterThan(Kind other)
    pub fn better_than(self, other: Kind) -> bool {
        self.ordinal() < other.ordinal()
    }

    /// Java: public KindName kindName()
    pub fn kind_name(self) -> KindName {
        match self {
            Kind::PCK => KindName::PACKAGE,
            Kind::TYP => KindName::CLASS,
            Kind::VAR => KindName::VAR,
            Kind::MTH => KindName::METHOD,
            Kind::ABSENT_VAR => KindName::VAR,
            Kind::WRONG_MTHS | Kind::WRONG_MTH | Kind::ABSENT_MTH => KindName::METHOD,
            Kind::ABSENT_TYP => KindName::CLASS,
            _ => panic!("Unexpected kind: {self:?}"),
        }
    }

    /// Java: public KindName absentKind()
    pub fn absent_kind(self) -> KindName {
        match self {
            Kind::ABSENT_VAR => KindName::VAR,
            Kind::WRONG_MTHS | Kind::WRONG_MTH | Kind::ABSENT_MTH => KindName::METHOD,
            Kind::ABSENT_TYP => KindName::CLASS,
            _ => panic!("Unexpected kind: {self:?}"),
        }
    }
}

/// Java: public static class KindSelector
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindSelector {
    data: u8,
}

impl KindSelector {
    // basic selectors
    pub const NIL: KindSelector = KindSelector { data: 0 };
    pub const PCK: KindSelector = KindSelector { data: 0x01 };
    pub const TYP: KindSelector = KindSelector { data: 0x02 };
    pub const VAR: KindSelector = KindSelector { data: 0x04 };
    pub const VAL: KindSelector = KindSelector { data: 0x0c };
    pub const MTH: KindSelector = KindSelector { data: 0x10 };
    pub const POLY: KindSelector = KindSelector { data: 0x20 };
    pub const MDL: KindSelector = KindSelector { data: 0x40 };
    pub const ERR: KindSelector = KindSelector { data: 0x7f };
    pub const ASG: KindSelector = KindSelector { data: 0x84 };

    // common derived selectors
    pub const TYP_PCK: KindSelector = KindSelector::of(&[KindSelector::TYP, KindSelector::PCK]);
    pub const VAL_MTH: KindSelector =
        KindSelector::of(&[KindSelector::VAL, KindSelector::MTH]);
    pub const VAL_POLY: KindSelector =
        KindSelector::of(&[KindSelector::VAL, KindSelector::POLY]);
    pub const VAL_TYP: KindSelector =
        KindSelector::of(&[KindSelector::VAL, KindSelector::TYP]);
    pub const VAL_TYP_PCK: KindSelector = KindSelector::of(&[
        KindSelector::VAL,
        KindSelector::TYP,
        KindSelector::PCK,
    ]);

    /// Java: public static KindSelector of(KindSelector... kindSelectors)
    pub const fn of(kind_selectors: &[KindSelector]) -> KindSelector {
        let mut new_data: u8 = 0;
        let mut index = 0;
        while index < kind_selectors.len() {
            new_data |= kind_selectors[index].data;
            index += 1;
        }
        KindSelector { data: new_data }
    }

    /// Java: public boolean subset(KindSelector other)
    pub fn subset(self, other: KindSelector) -> bool {
        (self.data & !other.data) == 0
    }

    /// Java: public boolean contains(KindSelector other)
    pub fn contains(self, other: KindSelector) -> bool {
        (self.data & other.data) != 0
    }

    /// Java: public Set<KindName> kindNames() (EnumSet iteration order).
    pub fn kind_names(self) -> Vec<KindName> {
        let mut kinds = Vec::new();
        if (self.data & KindSelector::VAL.data) != 0 {
            if (self.data & KindSelector::VAL.data) == KindSelector::VAR.data {
                kinds.push(KindName::VAR);
            } else {
                kinds.push(KindName::VAL);
            }
        }
        if (self.data & KindSelector::MTH.data) != 0 {
            kinds.push(KindName::METHOD);
        }
        if (self.data & KindSelector::TYP.data) != 0 {
            kinds.push(KindName::CLASS);
        }
        if (self.data & KindSelector::PCK.data) != 0 {
            kinds.push(KindName::PACKAGE);
        }
        if (self.data & KindSelector::MDL.data) != 0 {
            kinds.push(KindName::MODULE);
        }
        kinds
    }
}

/// Java: public enum KindName implements Formattable
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KindName {
    ANNOTATION,
    CONSTRUCTOR,
    INTERFACE,
    ENUM,
    STATIC,
    TYPEVAR,
    BOUND,
    VAR,
    VAL,
    METHOD,
    CLASS,
    STATIC_INIT,
    INSTANCE_INIT,
    PACKAGE,
    MODULE,
    RECORD_COMPONENT,
    RECORD,
}

impl KindName {
    /// Java: private final String name (the compiler.misc.* resource key).
    pub const fn name(self) -> &'static str {
        match self {
            KindName::ANNOTATION => "kindname.annotation",
            KindName::CONSTRUCTOR => "kindname.constructor",
            KindName::INTERFACE => "kindname.interface",
            KindName::ENUM => "kindname.enum",
            KindName::STATIC => "kindname.static",
            KindName::TYPEVAR => "kindname.type.variable",
            KindName::BOUND => "kindname.type.variable.bound",
            KindName::VAR => "kindname.variable",
            KindName::VAL => "kindname.value",
            KindName::METHOD => "kindname.method",
            KindName::CLASS => "kindname.class",
            KindName::STATIC_INIT => "kindname.static.init",
            KindName::INSTANCE_INIT => "kindname.instance.init",
            KindName::PACKAGE => "kindname.package",
            KindName::MODULE => "kindname.module",
            KindName::RECORD_COMPONENT => "kindname.record.component",
            KindName::RECORD => "kindname.record",
        }
    }

    /// Java: public String getKind() (Formattable)
    pub fn get_kind(self) -> &'static str {
        "Kindname"
    }
}

impl std::fmt::Display for KindName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Java: public static KindName kindName(MemberReferenceTree.ReferenceMode mode)
pub fn kind_name_for_reference_mode(mode: ReferenceMode) -> KindName {
    match mode {
        ReferenceMode::INVOKE => KindName::METHOD,
        ReferenceMode::NEW => KindName::CONSTRUCTOR,
    }
}

impl crate::api::formattable::Formattable for KindName {
    /// Java: public String toString(Locale locale, Messages messages)
    fn to_string_with_locale(
        &self,
        locale: &crate::util::javac_messages::Locale,
        messages: &dyn crate::api::messages::Messages,
    ) -> String {
        let s = self.name();
        messages.get_localized_string(Some(locale), &format!("compiler.misc.{s}"), &[])
    }

    /// Java: public String getKind()
    fn get_kind(&self) -> String {
        "Kindname".to_string()
    }
}

/// Java: public static KindName kindName(Symbol sym)
pub fn kind_name(sym: &Symbol, ctx: &CompilerContext) -> KindName {
    match sym.get_kind(ctx) {
        ElementKind::PACKAGE => KindName::PACKAGE,
        ElementKind::ENUM => KindName::ENUM,
        ElementKind::ANNOTATION_TYPE | ElementKind::CLASS => KindName::CLASS,
        ElementKind::RECORD => KindName::RECORD,
        ElementKind::INTERFACE => KindName::INTERFACE,
        ElementKind::TYPE_PARAMETER => KindName::TYPEVAR,
        ElementKind::BINDING_VARIABLE
        | ElementKind::ENUM_CONSTANT
        | ElementKind::PARAMETER
        | ElementKind::LOCAL_VARIABLE
        | ElementKind::EXCEPTION_PARAMETER
        | ElementKind::RESOURCE_VARIABLE => KindName::VAR,
        ElementKind::FIELD => {
            if (sym.flags_field & flags::RECORD) != 0 {
                KindName::RECORD_COMPONENT
            } else {
                KindName::VAR
            }
        }
        ElementKind::CONSTRUCTOR => KindName::CONSTRUCTOR,
        ElementKind::METHOD => KindName::METHOD,
        ElementKind::STATIC_INIT => KindName::STATIC_INIT,
        ElementKind::INSTANCE_INIT => KindName::INSTANCE_INIT,
        ElementKind::MODULE => KindName::MODULE,
        ElementKind::RECORD_COMPONENT => KindName::RECORD_COMPONENT,
        other => panic!("Unexpected kind: {other:?}"),
    }
}

/// Java: public static KindName typeKindName(Type t)
pub fn type_kind_name(t: &Type, ctx: &CompilerContext) -> KindName {
    let tsym_flags = || -> i64 {
        let tsym = t.tsym.expect("tsym is null");
        ctx.sym(tsym).flags()
    };
    if t.has_tag(TypeTag::TYPEVAR)
        || (t.has_tag(TypeTag::CLASS) && (tsym_flags() & flags::COMPOUND as i64) != 0)
    {
        KindName::BOUND
    } else if t.has_tag(TypeTag::PACKAGE) {
        KindName::PACKAGE
    } else if (tsym_flags() & flags::ANNOTATION as i64) != 0 {
        KindName::ANNOTATION
    } else if (tsym_flags() & flags::INTERFACE as i64) != 0 {
        KindName::INTERFACE
    } else {
        KindName::CLASS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::symbol::SymbolKind;
    use crate::code::type_::{Type, TypeKindData};
    use crate::util::list::List;
    use crate::util::name::Name;
    use crate::util::name::NameTableHandle;
    use std::collections::HashMap;
    use std::rc::Rc;

    fn name(text: &[u16]) -> Name {
        let table: Rc<NameTableHandle> = NameTableHandle::unshared();
        table.from_chars(text, 0, text.len() as i32)
    }

    /// Creates a name whose table is bound to a Names instance (needed by the
    /// Symbol.getKind isStatic branch, which reads names._this). The Names
    /// instance is returned so the caller keeps it alive (Weak in the table).
    fn name_with_names(text: &str) -> (Rc<crate::util::names::Names>, Name) {
        let names = Rc::new(crate::util::names::Names::new(false));
        let table = names.table.clone();
        let units: Vec<u16> = text.encode_utf16().collect();
        let name = table.from_chars(&units, 0, units.len() as i32);
        (names, name)
    }

    fn class_data() -> SymbolKind {
        SymbolKind::ClassSymbol {
            members_field: None,
            fullname: name(&[0x43]),
            flatname: name(&[0x43]),
            sourcefile: None,
            classfile: None,
            trans_local: List::nil(),
            record_components: List::nil(),
            permitted: List::nil(),
            is_permitted_explicit: false,
                annotation_type_metadata: Default::default(),
        }
    }

    fn method_data() -> SymbolKind {
        SymbolKind::MethodSymbol {
            extra_params: List::nil(),
            captured_locals: List::nil(),
            params: List::nil(),
            default_value: None,
        }
    }

    fn var_data(data_kind: Option<ElementKind>) -> SymbolKind {
        SymbolKind::VarSymbol {
            lazy_const_value: None,
            pos: 0,
            adr: -1,
            data_kind,
            data: None,
        }
    }

    fn package_data() -> SymbolKind {
        SymbolKind::PackageSymbol {
            members_field: None,
            fullname: name(&[0x50]),
            package_info: None,
            modle: None,
            sourcefile: None,
        }
    }

    fn module_data() -> SymbolKind {
        SymbolKind::ModuleSymbol {
            version: None,
            source_location: None,
            class_location: None,
            patch_location: None,
            patch_output_location: None,
            directives: List::nil(),
            requires: List::nil(),
            exports: List::nil(),
            opens: List::nil(),
            provides: List::nil(),
            uses: List::nil(),
            module_info: None,
            unnamed_package: None,
            visible_packages: HashMap::new(),
            read_modules: Vec::new(),
            enclosed_packages: List::nil(),
            flags: Vec::new(),
            resolution_flags: Vec::new(),
            uses_provides_completer: None,
        }
    }

    fn symbol(flags: i64, data: SymbolKind) -> Symbol {
        Symbol::new(Kind::TYP, flags, name(&[0x41]), None, None, data)
    }

    fn kind_name_of(sym: Symbol) -> KindName {
        let mut ctx = CompilerContext::new();
        let id = ctx.alloc_symbol(sym);
        kind_name(ctx.sym(id), &ctx)
    }

    #[test]
    fn kind_name_covers_element_kinds() {
        assert_eq!(kind_name_of(symbol(0, class_data())), KindName::CLASS);
        assert_eq!(
            kind_name_of(symbol(flags::INTERFACE as i64, class_data())),
            KindName::INTERFACE
        );
        assert_eq!(
            kind_name_of(symbol(flags::ENUM as i64, class_data())),
            KindName::ENUM
        );
        assert_eq!(
            kind_name_of(symbol(flags::ANNOTATION as i64, class_data())),
            KindName::CLASS
        );
        assert_eq!(
            kind_name_of(symbol(flags::RECORD, class_data())),
            KindName::RECORD
        );
        let (_names, method_name) = name_with_names("method");
        assert_eq!(
            kind_name_of(Symbol::new(
                Kind::MTH,
                0,
                method_name,
                None,
                None,
                method_data()
            )),
            KindName::METHOD
        );
        assert_eq!(
            kind_name_of(Symbol::new(
                Kind::PCK,
                0,
                name(&[0x50]),
                None,
                None,
                package_data()
            )),
            KindName::PACKAGE
        );
        assert_eq!(
            kind_name_of(Symbol::new(
                Kind::MDL,
                0,
                name(&[0x4D]),
                None,
                None,
                module_data()
            )),
            KindName::MODULE
        );
        assert_eq!(
            kind_name_of(Symbol::new(
                Kind::TYP,
                0,
                name(&[0x54]),
                None,
                None,
                SymbolKind::TypeVariableSymbol
            )),
            KindName::TYPEVAR
        );
    }

    #[test]
    fn kind_name_maps_variable_element_kinds() {
        let mut ctx = CompilerContext::new();
        let owner_class = ctx.alloc_symbol(symbol(0, class_data()));
        let owner_method = ctx.alloc_symbol(Symbol::new(
            Kind::MTH,
            0,
            name(&[0x4D]),
            None,
            None,
            method_data(),
        ));
        let field = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            0,
            name(&[0x46]),
            None,
            Some(owner_class),
            var_data(None),
        ));
        assert_eq!(kind_name(ctx.sym(field), &ctx), KindName::VAR);
        let local = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            0,
            name(&[0x4C]),
            None,
            Some(owner_method),
            var_data(None),
        ));
        assert_eq!(kind_name(ctx.sym(local), &ctx), KindName::VAR);
        let parameter = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            flags::PARAMETER,
            name(&[0x50]),
            None,
            Some(owner_method),
            var_data(None),
        ));
        assert_eq!(kind_name(ctx.sym(parameter), &ctx), KindName::VAR);
        let enum_constant = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            flags::ENUM as i64,
            name(&[0x45]),
            None,
            None,
            var_data(None),
        ));
        assert_eq!(kind_name(ctx.sym(enum_constant), &ctx), KindName::VAR);
        let binding = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            flags::MATCH_BINDING,
            name(&[0x42]),
            None,
            Some(owner_method),
            var_data(None),
        ));
        assert_eq!(kind_name(ctx.sym(binding), &ctx), KindName::VAR);
    }

    #[test]
    fn type_kind_name_covers_type_tags_and_flags() {
        let mut ctx = CompilerContext::new();
        let class_sym = ctx.alloc_symbol(symbol(0, class_data()));
        let interface_sym = ctx.alloc_symbol(symbol(flags::INTERFACE as i64, class_data()));
        let annotation_sym = ctx.alloc_symbol(symbol(flags::ANNOTATION as i64, class_data()));
        let compound_sym = ctx.alloc_symbol(symbol(flags::COMPOUND as i64, class_data()));
        let package_sym = ctx.alloc_symbol(Symbol::new(
            Kind::PCK,
            0,
            name(&[0x50]),
            None,
            None,
            package_data(),
        ));
        let class_type = ctx.alloc_type(Type::new(
            TypeTag::CLASS,
            Some(class_sym),
            TypeKindData::ClassType {
                outer_field: None,
                typarams_field: None,
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
            },
        ));
        assert_eq!(type_kind_name(ctx.typ(class_type), &ctx), KindName::CLASS);
        let interface_type = ctx.alloc_type(Type::new(
            TypeTag::CLASS,
            Some(interface_sym),
            TypeKindData::ClassType {
                outer_field: None,
                typarams_field: None,
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
            },
        ));
        assert_eq!(
            type_kind_name(ctx.typ(interface_type), &ctx),
            KindName::INTERFACE
        );
        let annotation_type = ctx.alloc_type(Type::new(
            TypeTag::CLASS,
            Some(annotation_sym),
            TypeKindData::ClassType {
                outer_field: None,
                typarams_field: None,
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
            },
        ));
        assert_eq!(
            type_kind_name(ctx.typ(annotation_type), &ctx),
            KindName::ANNOTATION
        );
        let compound_type = ctx.alloc_type(Type::new(
            TypeTag::CLASS,
            Some(compound_sym),
            TypeKindData::ClassType {
                outer_field: None,
                typarams_field: None,
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
            },
        ));
        assert_eq!(
            type_kind_name(ctx.typ(compound_type), &ctx),
            KindName::BOUND
        );
        let package_type = ctx.alloc_type(Type::new(
            TypeTag::PACKAGE,
            Some(package_sym),
            TypeKindData::PackageType,
        ));
        assert_eq!(
            type_kind_name(ctx.typ(package_type), &ctx),
            KindName::PACKAGE
        );
        let type_var = ctx.alloc_type(Type::new(
            TypeTag::TYPEVAR,
            None,
            TypeKindData::TypeVar {
                bound: None,
                lower: None,
            },
        ));
        assert_eq!(type_kind_name(ctx.typ(type_var), &ctx), KindName::BOUND);
    }
}

/// Java: javax.lang.model.element.NestingKind (the Element API enum).
///
/// Adapter: the Java enum is part of the javax.lang.model interface layer (P6);
/// the internal Symbol.getNestingKind result is represented by this enum until
/// the P6 api adapter is wired.
pub enum NestingKind {
    /// Java: TOP_LEVEL
    TopLevel,
    /// Java: MEMBER
    Member,
    /// Java: LOCAL
    Local,
    /// Java: ANONYMOUS
    Anonymous,
}
