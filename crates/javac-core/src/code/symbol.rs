// Origin: com/sun/tools/javac/code/Symbol.java @ 982d53c (jdk-17.0.19-ga)
// PORTING_RULES §19 gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P3 semantic services (Enter/Attr/Resolve/Check or ClassReader; see docs/PLAN.md).
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: getRawAttributes (Symbol.java:146)
// - NOT PORTED: getRawTypeAttributes (Symbol.java:156)
// - NOT PORTED: annotationsPendingCompletion (Symbol.java:170)
// - NOT PORTED: appendAttributes (Symbol.java:174)
// - NOT PORTED: appendClassInitTypeAttributes (Symbol.java:180)
// - NOT PORTED: appendInitTypeAttributes (Symbol.java:186)
// - NOT PORTED: appendUniqueTypeAttributes (Symbol.java:192)
// - NOT PORTED: getClassInitTypeAttributes (Symbol.java:198)
// - NOT PORTED: getInitTypeAttributes (Symbol.java:204)
// - NOT PORTED: setInitTypeAttributes (Symbol.java:210)
// - NOT PORTED: setClassInitTypeAttributes (Symbol.java:214)
// - NOT PORTED: getDeclarationAttributes (Symbol.java:218)
// - NOT PORTED: hasAnnotations (Symbol.java:224)
// - NOT PORTED: hasTypeAnnotations (Symbol.java:228)
// - NOT PORTED: isCompleted (Symbol.java:232)
// - NOT PORTED: prependAttributes (Symbol.java:236)
// - NOT PORTED: resetAnnotations (Symbol.java:242)
// - NOT PORTED: setAttributes (Symbol.java:246)
// - NOT PORTED: setDeclarationAttributes (Symbol.java:252)
// - NOT PORTED: setTypeAttributes (Symbol.java:258)
// - NOT PORTED: initedMetadata (Symbol.java:266)
// - NOT PORTED: getMetadata (Symbol.java:273)
// - NOT PORTED: isDirectlyOrIndirectlyLocal (Symbol.java:443)
// - NOT PORTED: isConstructor (Symbol.java:458)
// - NOT PORTED: isInner (Symbol.java:490)
// - NOT PORTED: hasOuterInstance (Symbol.java:502)
// - NOT PORTED: hiddenIn (Symbol.java:567)
// - NOT PORTED: hiddenInInternal (Symbol.java:582)
// - NOT PORTED: isAccessibleIn (Symbol.java:616)
// - NOT PORTED: isInheritedIn (Symbol.java:651)
// - NOT PORTED: asMemberOf (Symbol.java:659)
// - NOT PORTED: overrides (Symbol.java:672)
// - NOT PORTED: apiComplete (Symbol.java:686)
// - NOT PORTED: exists (Symbol.java:696)
// - NOT PORTED: asType (Symbol.java:701)
// - NOT PORTED: getEnclosingElement (Symbol.java:706)
// - NOT PORTED: getModifiers (Symbol.java:716)
// - NOT PORTED: getSimpleName (Symbol.java:722)
// - NOT PORTED: getAnnotationMirrors (Symbol.java:731)
// - NOT PORTED: getTypeParameters (Symbol.java:743)
// - NOT PORTED: isDirectlyOrIndirectlyLocal (Symbol.java:764)
// - NOT PORTED: isConstructor (Symbol.java:765)
// - NOT PORTED: isInner (Symbol.java:769)
// - NOT PORTED: hasOuterInstance (Symbol.java:770)
// - NOT PORTED: isInheritedIn (Symbol.java:777)
// - NOT PORTED: asMemberOf (Symbol.java:778)
// - NOT PORTED: getUnderlyingSymbol (Symbol.java:790)
// - NOT PORTED: formFullName (Symbol.java:803)
// - NOT PORTED: formFlatName (Symbol.java:818)
// - NOT PORTED: precedes (Symbol.java:835)
// - NOT PORTED: getEnclosedElements (Symbol.java:852)
// - NOT PORTED: getAnnotationTypeMetadata (Symbol.java:867)
// - NOT PORTED: isAnnotationType (Symbol.java:872)
// - NOT PORTED: getGenericElement (Symbol.java:896)
// - NOT PORTED: getBounds (Symbol.java:901)
// - NOT PORTED: getAnnotationMirrors (Symbol.java:917)
// - NOT PORTED: getAttribute (Symbol.java:933)
// - NOT PORTED: isCurrentSymbolsAnnotation (Symbol.java:948)
// - NOT PORTED: create (Symbol.java:993)
// - NOT PORTED: getSimpleName (Symbol.java:1015)
// - NOT PORTED: isOpen (Symbol.java:1020)
// - NOT PORTED: isUnnamed (Symbol.java:1025)
// - NOT PORTED: isNoModule (Symbol.java:1034)
// - NOT PORTED: completeUsesProvides (Symbol.java:1050)
// - NOT PORTED: getEnclosedElements (Symbol.java:1079)
// - NOT PORTED: reset (Symbol.java:1088)
// - NOT PORTED: isUnnamed (Symbol.java:1171)
// - NOT PORTED: getRawAttributes (Symbol.java:1191)
// - NOT PORTED: mergeAttributes (Symbol.java:1200)
// - NOT PORTED: exists (Symbol.java:1211)
// - NOT PORTED: getEnclosingElement (Symbol.java:1221)
// - NOT PORTED: reset (Symbol.java:1235)
// - NOT PORTED: getRawAttributes (Symbol.java:1341)
// - NOT PORTED: getRawTypeAttributes (Symbol.java:1347)
// - NOT PORTED: getEnclosedElements (Symbol.java:1374)
// - NOT PORTED: getInterfaces (Symbol.java:1421)
// - NOT PORTED: getSuperclass (Symbol.java:1435)
// - NOT PORTED: getSuperClassToSearchForAnnotations (Symbol.java:1453)
// - NOT PORTED: getInheritedAnnotations (Symbol.java:1465)
// - NOT PORTED: getModifiers (Symbol.java:1491)
// - NOT PORTED: getRecordComponent (Symbol.java:1497)
// - NOT PORTED: findRecordComponentToRemove (Symbol.java:1506)
// - NOT PORTED: createRecordComponent (Symbol.java:1522)
// - NOT PORTED: setRecordComponents (Symbol.java:1539)
// - NOT PORTED: getNestingKind (Symbol.java:1544)
// - NOT PORTED: getAttribute (Symbol.java:1557)
// - NOT PORTED: markAbstractIfNeeded (Symbol.java:1580)
// - NOT PORTED: reset (Symbol.java:1591)
// - NOT PORTED: clearAnnotationMetadata (Symbol.java:1608)
// - NOT PORTED: getAnnotationTypeMetadata (Symbol.java:1614)
// - NOT PORTED: isAnnotationType (Symbol.java:1619)
// - NOT PORTED: setAnnotationTypeMetadata (Symbol.java:1623)
// - NOT PORTED: isRecord (Symbol.java:1629)
// - NOT PORTED: getPermittedSubclasses (Symbol.java:1634)
// - NOT PORTED: asMethodHandle (Symbol.java:1670)
// - NOT PORTED: poolKey (Symbol.java:1684)
// - NOT PORTED: asMemberOf (Symbol.java:1699)
// - NOT PORTED: getConstantValue (Symbol.java:1731)
// - NOT PORTED: setLazyConstValue (Symbol.java:1735)
// - NOT PORTED: isExceptionParameter (Symbol.java:1751)
// - NOT PORTED: isResourceVariable (Symbol.java:1755)
// - NOT PORTED: getConstValue (Symbol.java:1759)
// - NOT PORTED: setData (Symbol.java:1777)
// - NOT PORTED: getOriginalAnnos (Symbol.java:1826)
// - NOT PORTED: getAccessor (Symbol.java:1838)
// - NOT PORTED: getSimpleName (Symbol.java:1854)
// - NOT PORTED: isAliasFor (Symbol.java:1880)
// - NOT PORTED: aliases (Symbol.java:1884)
// - NOT PORTED: preserveBinding (Symbol.java:1888)
// - NOT PORTED: isPreserved (Symbol.java:1892)
// - NOT PORTED: poolKey (Symbol.java:1936)
// - NOT PORTED: getModifiers (Symbol.java:1945)
// - NOT PORTED: isHandle (Symbol.java:1974)
// - NOT PORTED: asHandle (Symbol.java:1979)
// - NOT PORTED: implemented (Symbol.java:1987)
// - NOT PORTED: implementedIn (Symbol.java:2000)
// - NOT PORTED: binaryOverrides (Symbol.java:2017)
// - NOT PORTED: binaryImplementation (Symbol.java:2042)
// - NOT PORTED: overrides (Symbol.java:2068)
// - NOT PORTED: overrides (Symbol.java:2085)
// - NOT PORTED: isOverridableIn (Symbol.java:2120)
// - NOT PORTED: isInheritedIn (Symbol.java:2142)
// - NOT PORTED: isLambdaMethod (Symbol.java:2153)
// - NOT PORTED: originalEnclosingMethod (Symbol.java:2160)
// - NOT PORTED: implementation (Symbol.java:2168)
// - NOT PORTED: implementation (Symbol.java:2175)
// - NOT PORTED: asMemberOf (Symbol.java:2205)
// - NOT PORTED: isStaticOrInstanceInit (Symbol.java:2221)
// - NOT PORTED: getDefaultValue (Symbol.java:2227)
// - NOT PORTED: getParameters (Symbol.java:2232)
// - NOT PORTED: isVarArgs (Symbol.java:2237)
// - NOT PORTED: isDefault (Symbol.java:2242)
// - NOT PORTED: getReceiverType (Symbol.java:2256)
// - NOT PORTED: getReturnType (Symbol.java:2262)
// - NOT PORTED: getThrownTypes (Symbol.java:2267)
// - NOT PORTED: staticArgs (Symbol.java:2291)
// - NOT PORTED: bootstrapMethod (Symbol.java:2296)
// - NOT PORTED: dynamicType (Symbol.java:2306)
// - NOT PORTED: dynamicType (Symbol.java:2329)
// - NOT PORTED: staticArgs (Symbol.java:2334)
// - NOT PORTED: bootstrapMethod (Symbol.java:2339)
// - NOT PORTED: referenceKind (Symbol.java:2369)
// - NOT PORTED: poolKey (Symbol.java:2404)
// - NOT PORTED: asHandle (Symbol.java:2409)
// - NOT PORTED: isHandle (Symbol.java:2420)
// - NOT PORTED: getAccessCode (Symbol.java:2442)
// - NOT PORTED: getFromCode (Symbol.java:2481)
// - NOT PORTED: isTerminal (Symbol.java:2525)
// - NOT PORTED: isTerminal (Symbol.java:2539)
// - NOT PORTED: getDiagnostic (Symbol.java:2562)
// - NOT PORTED: getMessage (Symbol.java:2570)
// - NOT PORTED: getDetailValue (Symbol.java:2574)
// - NOT PORTED: initCause (Symbol.java:2579)
// - NOT PORTED: resetDiagnostic (Symbol.java:2584)
// - NOT PORTED: visitClassSymbol (Symbol.java:2606)
// - NOT PORTED: visitMethodSymbol (Symbol.java:2607)
// - NOT PORTED: visitPackageSymbol (Symbol.java:2608)
// - NOT PORTED: visitOperatorSymbol (Symbol.java:2609)
// - NOT PORTED: visitVarSymbol (Symbol.java:2610)
// - NOT PORTED: visitTypeSymbol (Symbol.java:2611)
// - NOT PORTED: visitSymbol (Symbol.java:2612)
// End of §19 gap registration.
//         (the Kinds.Kind/KindSelector/KindName part comes from
//          com/sun/tools/javac/code/Kinds.java; see the module note below)

//! A class representing Java symbols: packages, classes, methods, variables,
//! types, packages. Each subclass is represented as a static inner class
//! inside Symbol.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: ADR 0001 (arena + IDs) defines the representation:
//! - a Symbol is a value in the symbol arena (SymbolId is the arena index);
//! - the owner, its type and the erasure cache are Option<SymbolId> /
//!   Option<TypeId>; the members scope is Option<ScopeId>;
//! - the upstream concrete subclasses map 1:1 to SymbolKind variants;
//! - the Kinds.Kind enum is needed by the Symbol.kind field and is declared in
//!   the kinds module of this file for now (the parser/compiler need it before
//!   the Kinds.java batch lands; it will move to code/kinds.rs then, registered
//!   in PORTING_RULES section 9).
//!
//! NOT PORTED IN THIS STEP (see docs/STATUS.md; no placeholder logic):
//! - SymbolMetadata/AnnoConstruct attribute methods, completer (Completer is an
//!   interface over Symbol mutation and needs the compiler context), the
//!   metadata field, poolTag/accept(Symbol.Visitor), toString/location/
//!   baseSymbol (the "this" result needs the arena ID), erasure/externalType
//!   (Types), the owner-chain methods (enclClass/outermostClass/packge) and the
//!   Types-dependent methods (isSubClass/isMemberOf/isEnclosedBy/...);
//! - the DynamicMethodSymbol/DynamicVarSymbol variants (LoadableConstant),
//!   MethodSymbol.code (jvm/Code) and MethodSymbol.defaultValue (Attribute),
//!   CompletionFailure and the Symbol.Visitor interface;
//! - ClassSymbol.annotationTypeMetadata, VarSymbol.data (only the ElementKind
//!   marker subset is represented as VarSymbol.data_kind; the Callable constant
//!   value stored in data is not ported yet) and MethodHandleSymbol private
//!   fields are not represented (dependencies not ported yet; reported);
//! - RootPackageSymbol.missingInfoHandler (MissingInfoHandler is not ported;
//!   the variant carries allowPrivateInvokeVirtual only).

use std::collections::HashMap;
use std::rc::Rc;

use javac_tools::lang::model::element::element_kind::ElementKind;
use javac_tools::tools::java_file_manager::Location;
use javac_tools::tools::java_file_object::JavaFileObject;

use crate::code::flags;
use crate::code::kinds::Kind;
use crate::compiler_context::CompilerContext;
use crate::ids::{DirectiveId, ScopeId, SymbolId, TreeId, TypeId};
use crate::util::list::List;
use crate::util::name::Name;


/// A class representing Java symbols.
///
/// PORTING_RULES: the shared fields of the upstream Symbol class plus the
/// node-specific fields (SymbolKind). The completer and the metadata fields are
/// not represented yet (see the module doc).
pub struct Symbol {
    /// The kind of this symbol.
    pub kind: Kind,
    /// The flags of this symbol.
    pub flags_field: i64,
    /// The name of this symbol in Utf8 representation.
    pub name: Name,
    /// The type of this symbol (upstream field type; null -> None).
    pub ty: Option<TypeId>,
    /// The owner of this symbol.
    pub owner: Option<SymbolId>,
    /// A cache for the type erasure of this symbol.
    pub erasure_field: Option<TypeId>,
    /// The node-specific fields (one variant per upstream concrete subclass).
    pub kind_data: SymbolKind,
}

impl Symbol {
    /// Adapter constructor: the upstream constructor also installs
    /// Completer.NULL_COMPLETER (the completer interface is not ported yet).
    pub fn new(
        kind: Kind,
        flags: i64,
        name: Name,
        ty: Option<TypeId>,
        owner: Option<SymbolId>,
        kind_data: SymbolKind,
    ) -> Symbol {
        Symbol {
            kind,
            flags_field: flags,
            name,
            ty,
            owner,
            erasure_field: None,
            kind_data,
        }
    }

    /// An accessor method for the flags of this symbol.
    pub fn flags(&self) -> i64 {
        self.flags_field
    }

    /// Java: public boolean isDeprecated()
    pub fn is_deprecated(&self) -> bool {
        (self.flags_field & flags::DEPRECATED as i64) != 0
    }

    /// Java: public boolean hasDeprecatedAnnotation()
    pub fn has_deprecated_annotation(&self) -> bool {
        (self.flags_field & flags::DEPRECATED_ANNOTATION) != 0
    }

    /// Java: public boolean isDeprecatedForRemoval()
    pub fn is_deprecated_for_removal(&self) -> bool {
        (self.flags_field & flags::DEPRECATED_REMOVAL) != 0
    }

    /// Java: public boolean isPreviewApi()
    pub fn is_preview_api(&self) -> bool {
        (self.flags_field & flags::PREVIEW_API) != 0
    }

    /// Java: public boolean isAbstract()
    pub fn is_abstract(&self) -> bool {
        (self.flags_field & flags::ABSTRACT as i64) != 0
    }

    /// Java: public boolean isPrivate()
    pub fn is_private(&self) -> bool {
        (self.flags_field & flags::AccessFlags as i64) == flags::PRIVATE as i64
    }

    /// Java: public boolean isPublic()
    pub fn is_public(&self) -> bool {
        (self.flags_field & flags::AccessFlags as i64) == flags::PUBLIC as i64
    }

    /// Java: public boolean isEnum()
    pub fn is_enum(&self) -> bool {
        (self.flags_field & flags::ENUM as i64) != 0
    }

    /// Java: public boolean isSealed()
    pub fn is_sealed(&self) -> bool {
        (self.flags_field & flags::SEALED) != 0
    }

    /// Java: public boolean isNonSealed()
    pub fn is_non_sealed(&self) -> bool {
        (self.flags_field & flags::NON_SEALED) != 0
    }

    /// Java: public boolean isFinal()
    pub fn is_final(&self) -> bool {
        (self.flags_field & flags::FINAL as i64) != 0
    }

    /// Java: public boolean isInterface()
    pub fn is_interface(&self) -> bool {
        (self.flags_field & flags::INTERFACE as i64) != 0
    }

    /// Has this symbol an empty name? This includes anonymous inner classes.
    pub fn is_anonymous(&self) -> bool {
        self.name.is_empty()
    }

    /// Java: public boolean isDynamic() (overridden by the dynamic symbol
    /// subclasses, which are not ported yet; the base behavior is false).
    pub fn is_dynamic(&self) -> bool {
        false
    }

    /// Java: public ElementKind getKind() (the base Symbol returns OTHER; the
    /// subclasses override it). The upstream owner/names reads go through the
    /// CompilerContext in this adapter.
    pub fn get_kind(&self, ctx: &CompilerContext) -> ElementKind {
        match &self.kind_data {
            SymbolKind::ClassSymbol { .. } => {
                let symbol_flags = self.flags();
                if (symbol_flags & flags::ANNOTATION as i64) != 0 {
                    ElementKind::ANNOTATION_TYPE
                } else if (symbol_flags & flags::INTERFACE as i64) != 0 {
                    ElementKind::INTERFACE
                } else if (symbol_flags & flags::ENUM as i64) != 0 {
                    ElementKind::ENUM
                } else if (symbol_flags & flags::RECORD) != 0 {
                    ElementKind::RECORD
                } else {
                    ElementKind::CLASS
                }
            }
            SymbolKind::TypeVariableSymbol => ElementKind::TYPE_PARAMETER,
            SymbolKind::ModuleSymbol { .. } => ElementKind::MODULE,
            SymbolKind::PackageSymbol { .. } | SymbolKind::RootPackageSymbol { .. } => {
                ElementKind::PACKAGE
            }
            SymbolKind::VarSymbol { data_kind, .. } => {
                let symbol_flags = self.flags();
                if (symbol_flags & flags::PARAMETER) != 0 {
                    if *data_kind == Some(ElementKind::EXCEPTION_PARAMETER) {
                        return ElementKind::EXCEPTION_PARAMETER;
                    }
                    return ElementKind::PARAMETER;
                }
                if (symbol_flags & flags::ENUM as i64) != 0 {
                    return ElementKind::ENUM_CONSTANT;
                }
                let owner_kind = self.owner.map(|owner| ctx.symbol_kind(owner));
                if matches!(owner_kind, Some(Kind::TYP) | Some(Kind::ERR)) {
                    return ElementKind::FIELD;
                }
                if *data_kind == Some(ElementKind::RESOURCE_VARIABLE) {
                    return ElementKind::RESOURCE_VARIABLE;
                }
                if (symbol_flags & flags::MATCH_BINDING) != 0 {
                    return ElementKind::BINDING_VARIABLE;
                }
                ElementKind::LOCAL_VARIABLE
            }
            SymbolKind::RecordComponent { .. } => ElementKind::RECORD_COMPONENT,
            SymbolKind::MethodSymbol { .. }
            | SymbolKind::MethodHandleSymbol { .. }
            | SymbolKind::OperatorSymbol { .. } => {
                let names = self.name.table.names();
                if self.name.to_java_string() == names.init.to_java_string() {
                    return ElementKind::CONSTRUCTOR;
                }
                if self.name.to_java_string() == names.clinit.to_java_string() {
                    return ElementKind::STATIC_INIT;
                }
                if (self.flags() & flags::BLOCK as i64) != 0 {
                    if self.is_static(ctx) {
                        return ElementKind::STATIC_INIT;
                    }
                    return ElementKind::INSTANCE_INIT;
                }
                ElementKind::METHOD
            }
            _ => ElementKind::OTHER,
        }
    }

    /// Java: public boolean isStatic()
    pub fn is_static(&self, ctx: &CompilerContext) -> bool {
        if (self.flags() & flags::STATIC as i64) != 0 {
            return true;
        }
        let owner_flags = self
            .owner
            .map(|owner| ctx.sym(owner).flags())
            .unwrap_or(0);
        let owner_is_interface = (owner_flags & flags::INTERFACE as i64) != 0;
        let is_this_name =
            self.name.to_java_string() == self.name.table.names()._this.to_java_string();
        owner_is_interface && self.kind != Kind::MTH && !is_this_name
    }

    /// Java: public boolean isDeprecatableViaAnnotation()
    pub fn is_deprecatable_via_annotation(&self, ctx: &CompilerContext) -> bool {
        !matches!(
            self.get_kind(ctx),
            ElementKind::LOCAL_VARIABLE
                | ElementKind::PACKAGE
                | ElementKind::PARAMETER
                | ElementKind::RESOURCE_VARIABLE
                | ElementKind::EXCEPTION_PARAMETER
        )
    }

    /// Java: public WriteableScope members() (overridden by ClassSymbol and
    /// PackageSymbol). Returns null for the other symbols.
    pub fn members(&self) -> Option<ScopeId> {
        match &self.kind_data {
            SymbolKind::ClassSymbol { members_field, .. }
            | SymbolKind::PackageSymbol { members_field, .. }
            | SymbolKind::RootPackageSymbol { members_field, .. } => *members_field,
            _ => None,
        }
    }

    /// Java: public Name getQualifiedName() (ClassSymbol/PackageSymbol return
    /// their fullname).
    pub fn get_qualified_name(&self) -> Name {
        match &self.kind_data {
            SymbolKind::ClassSymbol { fullname, .. }
            | SymbolKind::PackageSymbol { fullname, .. }
            | SymbolKind::RootPackageSymbol { fullname, .. } => fullname.clone(),
            _ => self.name.clone(),
        }
    }

    /// Java: public Name flatName() (ClassSymbol returns its flatname).
    pub fn flat_name(&self) -> Name {
        match &self.kind_data {
            SymbolKind::ClassSymbol { flatname, .. } => flatname.clone(),
            _ => self.get_qualified_name(),
        }
    }
}

/// The node-specific fields of the Symbol subclasses.
///
/// PORTING_RULES: one variant per upstream concrete class whose fields can be
/// expressed with the ported types; inherited fields of TypeSymbol are not
/// repeated (TypeSymbol has no fields). See the module doc for the variants
/// that are not ported yet.
/// Flags for ModuleSymbol (Java: Symbol.ModuleSymbol.ModuleFlags, an EnumSet).
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModuleFlags {
    OPEN,
    SYNTHETIC,
    MANDATED,
}

impl ModuleFlags {
    /// Java: public final int value
    pub const fn value(self) -> i32 {
        match self {
            ModuleFlags::OPEN => 0x0020,
            ModuleFlags::SYNTHETIC => 0x1000,
            ModuleFlags::MANDATED => 0x8000,
        }
    }

    /// Java: enum values() (declaration order).
    pub fn values() -> Vec<ModuleFlags> {
        vec![
            ModuleFlags::OPEN,
            ModuleFlags::SYNTHETIC,
            ModuleFlags::MANDATED,
        ]
    }

    /// Java: public static int value(Set<ModuleFlags> s)
    pub fn value_of(flags: &[ModuleFlags]) -> i32 {
        let mut v = 0;
        for flag in flags {
            v |= flag.value();
        }
        v
    }
}

/// Flags for ModuleSymbol module resolution (Java:
/// Symbol.ModuleSymbol.ModuleResolutionFlags, an EnumSet).
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModuleResolutionFlags {
    DO_NOT_RESOLVE_BY_DEFAULT,
    WARN_DEPRECATED,
    WARN_DEPRECATED_REMOVAL,
    WARN_INCUBATING,
}

impl ModuleResolutionFlags {
    /// Java: public final int value
    pub const fn value(self) -> i32 {
        match self {
            ModuleResolutionFlags::DO_NOT_RESOLVE_BY_DEFAULT => 0x0001,
            ModuleResolutionFlags::WARN_DEPRECATED => 0x0002,
            ModuleResolutionFlags::WARN_DEPRECATED_REMOVAL => 0x0004,
            ModuleResolutionFlags::WARN_INCUBATING => 0x0008,
        }
    }

    /// Java: enum values() (declaration order).
    pub fn values() -> Vec<ModuleResolutionFlags> {
        vec![
            ModuleResolutionFlags::DO_NOT_RESOLVE_BY_DEFAULT,
            ModuleResolutionFlags::WARN_DEPRECATED,
            ModuleResolutionFlags::WARN_DEPRECATED_REMOVAL,
            ModuleResolutionFlags::WARN_INCUBATING,
        ]
    }

    /// Java: public static int value(Set<ModuleResolutionFlags> s)
    pub fn value_of(flags: &[ModuleResolutionFlags]) -> i32 {
        let mut v = 0;
        for flag in flags {
            v |= flag.value();
        }
        v
    }
}

#[allow(clippy::large_enum_variant)]
pub enum SymbolKind {
    /// Java: Symbol.DelegatedSymbol<T extends Symbol>
    DelegatedSymbol {
        /// The delegate symbol.
        other: Option<SymbolId>,
    },
    /// Java: Symbol.TypeSymbol (abstract, no fields).
    TypeSymbol,
    /// Java: Symbol.TypeVariableSymbol
    TypeVariableSymbol,
    /// Java: Symbol.PackageSymbol
    PackageSymbol {
        /// The members of this package.
        members_field: Option<ScopeId>,
        /// The fully qualified name.
        fullname: Name,
        /// The package-info class, if any.
        package_info: Option<SymbolId>,
        /// The module to which this package belongs.
        modle: Option<SymbolId>,
        /// The source file, if any.
        sourcefile: Option<Rc<dyn JavaFileObject>>,
    },
    /// Java: Symbol.ClassSymbol
    ClassSymbol {
        /// The members of this class.
        members_field: Option<ScopeId>,
        /// The fully qualified name.
        fullname: Name,
        /// The flat (binary) name.
        flatname: Name,
        /// The source file, if any.
        sourcefile: Option<Rc<dyn JavaFileObject>>,
        /// The class file, if any.
        classfile: Option<Rc<dyn JavaFileObject>>,
        /// Symbols of the current class's local classes (transitive).
        trans_local: List<SymbolId>,
        /// The record components (Java: List<RecordComponent>, private field).
        record_components: List<SymbolId>,
        /// The permitted subclasses (sealed classes).
        permitted: List<SymbolId>,
        /// Whether the permits clause was explicit.
        is_permitted_explicit: bool,
    },
    /// Java: Symbol.VarSymbol
    VarSymbol {
        /// The variable's position (declaration order).
        pos: i32,
        /// The variable's address (set during code generation).
        adr: i32,
        /// The ElementKind marker of the upstream Object data field
        /// (EXCEPTION_PARAMETER/RESOURCE_VARIABLE markers; the Callable constant
        /// value stored in data is not ported yet, see the module doc).
        data_kind: Option<ElementKind>,
    },
    /// Java: Symbol.RecordComponent
    RecordComponent {
        /// The accessor method of this record component.
        accessor: Option<SymbolId>,
        /// The accessor method tree of this record component.
        accessor_meth: Option<TreeId>,
        /// The original annotations (Java: List<JCAnnotation>, private field).
        original_annos: List<TreeId>,
        /// The record component position (private field).
        pos: i32,
        /// Whether this record component is a varargs component (private field).
        is_varargs: bool,
    },
    /// Java: Symbol.ParamSymbol (no fields).
    ParamSymbol,
    /// Java: Symbol.BindingSymbol (no fields).
    BindingSymbol,
    /// Java: Symbol.MethodSymbol
    ///
    /// PORTING_RULES: the upstream code field (jvm/Code) and defaultValue field
    /// (Attribute) are not ported yet (see the module doc).
    MethodSymbol {
        /// The extra parameter symbols.
        extra_params: List<SymbolId>,
        /// The captured local variables.
        captured_locals: List<SymbolId>,
        /// The value parameter symbols.
        params: List<SymbolId>,
    },
    /// Java: Symbol.MethodHandleSymbol
    MethodHandleSymbol {
        /// The referenced symbol (private field refSym).
        ref_sym: Option<SymbolId>,
        /// Whether this is a getter handle (private field getter).
        getter: bool,
    },
    /// Java: Symbol.OperatorSymbol
    OperatorSymbol {
        /// The operator opcode (a JCTree.Tag ordinal in the upstream code).
        opcode: i32,
        /// The operator access code (private field accessCode).
        access_code: i32,
    },
    /// Java: Symbol.ModuleSymbol
    ModuleSymbol {
        /// The module version, if any.
        version: Option<Name>,
        /// The location of the module sources.
        source_location: Option<Rc<dyn Location>>,
        /// The location of the module classes.
        class_location: Option<Rc<dyn Location>>,
        /// The location of the patched module.
        patch_location: Option<Rc<dyn Location>>,
        /// The output location of the patched module.
        patch_output_location: Option<Rc<dyn Location>>,
        /// All directives, in natural order (arena values).
        directives: List<DirectiveId>,
        /// The requires directives.
        requires: List<DirectiveId>,
        /// The exports directives.
        exports: List<DirectiveId>,
        /// The opens directives.
        opens: List<DirectiveId>,
        /// The provides directives.
        provides: List<DirectiveId>,
        /// The uses directives.
        uses: List<DirectiveId>,
        /// The module-info class symbol.
        module_info: Option<SymbolId>,
        /// The unnamed package of the module.
        unnamed_package: Option<SymbolId>,
        /// The visible packages of the module.
        visible_packages: HashMap<Name, SymbolId>,
        /// The modules read by this module.
        read_modules: Vec<SymbolId>,
        /// The packages enclosed by this module.
        enclosed_packages: List<SymbolId>,
        /// The module flags.
        flags: Vec<ModuleFlags>,
        /// The module resolution flags.
        resolution_flags: Vec<ModuleResolutionFlags>,
    },
    /// Java: Symbol.RootPackageSymbol
    RootPackageSymbol {
        /// The members of this package.
        members_field: Option<ScopeId>,
        /// The fully qualified name.
        fullname: Name,
        /// The package-info class, if any.
        package_info: Option<SymbolId>,
        /// The module to which this package belongs.
        modle: Option<SymbolId>,
        /// The source file, if any.
        sourcefile: Option<Rc<dyn JavaFileObject>>,
        /// Whether private invokevirtual is allowed.
        allow_private_invoke_virtual: bool,
    },
}

#[cfg(test)]
mod tests {
    use crate::code::kinds::{KindName, KindSelector};

    fn class_kind_data() -> SymbolKind {
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
        }
    }

    fn method_kind_data() -> SymbolKind {
        SymbolKind::MethodSymbol {
            extra_params: List::nil(),
            captured_locals: List::nil(),
            params: List::nil(),
        }
    }

    fn var_kind_data(data_kind: Option<ElementKind>) -> SymbolKind {
        SymbolKind::VarSymbol {
            pos: 0,
            adr: -1,
            data_kind,
        }
    }

    #[test]
    fn get_kind_maps_class_like_symbols() {
        let mut ctx = CompilerContext::new();
        let class = ctx.alloc_symbol(Symbol::new(Kind::TYP, 0, name(&[0x43]), None, None, class_kind_data()));
        assert_eq!(ctx.sym(class).get_kind(&ctx), ElementKind::CLASS);
        let interface = ctx.alloc_symbol(Symbol::new(Kind::TYP, flags::INTERFACE as i64, name(&[0x49]), None, None, class_kind_data()));
        assert_eq!(ctx.sym(interface).get_kind(&ctx), ElementKind::INTERFACE);
        let enum_symbol = ctx.alloc_symbol(Symbol::new(Kind::TYP, flags::ENUM as i64, name(&[0x45]), None, None, class_kind_data()));
        assert_eq!(ctx.sym(enum_symbol).get_kind(&ctx), ElementKind::ENUM);
        let annotation = ctx.alloc_symbol(Symbol::new(Kind::TYP, flags::ANNOTATION as i64, name(&[0x41]), None, None, class_kind_data()));
        assert_eq!(ctx.sym(annotation).get_kind(&ctx), ElementKind::ANNOTATION_TYPE);
        let record = ctx.alloc_symbol(Symbol::new(Kind::TYP, flags::RECORD, name(&[0x52]), None, None, class_kind_data()));
        assert_eq!(ctx.sym(record).get_kind(&ctx), ElementKind::RECORD);
    }

    #[test]
    fn get_kind_maps_variable_symbols() {
        let mut ctx = CompilerContext::new();
        let owner_class = ctx.alloc_symbol(Symbol::new(Kind::TYP, 0, name(&[0x43]), None, None, class_kind_data()));
        let owner_method = ctx.alloc_symbol(Symbol::new(Kind::MTH, 0, name(&[0x4D]), None, None, method_kind_data()));
        let field = ctx.alloc_symbol(Symbol::new(Kind::VAR, 0, name(&[0x46]), None, Some(owner_class), var_kind_data(None)));
        assert_eq!(ctx.sym(field).get_kind(&ctx), ElementKind::FIELD);
        let local = ctx.alloc_symbol(Symbol::new(Kind::VAR, 0, name(&[0x4C]), None, Some(owner_method), var_kind_data(None)));
        assert_eq!(ctx.sym(local).get_kind(&ctx), ElementKind::LOCAL_VARIABLE);
        let parameter = ctx.alloc_symbol(Symbol::new(Kind::VAR, flags::PARAMETER, name(&[0x50]), None, Some(owner_method), var_kind_data(None)));
        assert_eq!(ctx.sym(parameter).get_kind(&ctx), ElementKind::PARAMETER);
        let exception = ctx.alloc_symbol(Symbol::new(Kind::VAR, flags::PARAMETER, name(&[0x45]), None, Some(owner_method), var_kind_data(Some(ElementKind::EXCEPTION_PARAMETER))));
        assert_eq!(ctx.sym(exception).get_kind(&ctx), ElementKind::EXCEPTION_PARAMETER);
        let enum_constant = ctx.alloc_symbol(Symbol::new(Kind::VAR, flags::ENUM as i64, name(&[0x45]), None, None, var_kind_data(None)));
        assert_eq!(ctx.sym(enum_constant).get_kind(&ctx), ElementKind::ENUM_CONSTANT);
        let binding = ctx.alloc_symbol(Symbol::new(Kind::VAR, flags::MATCH_BINDING, name(&[0x42]), None, Some(owner_method), var_kind_data(None)));
        assert_eq!(ctx.sym(binding).get_kind(&ctx), ElementKind::BINDING_VARIABLE);
    }

    #[test]
    fn get_kind_maps_methods_packages_modules_and_type_parameters() {
        let mut ctx = CompilerContext::new();
        let (_names, method_name) = name_with_names("method");
        let method = ctx.alloc_symbol(Symbol::new(Kind::MTH, 0, method_name, None, None, method_kind_data()));
        assert_eq!(ctx.sym(method).get_kind(&ctx), ElementKind::METHOD);
        let (_names2, instance_name) = name_with_names("instance");
        let instance_init = ctx.alloc_symbol(Symbol::new(Kind::MTH, flags::BLOCK as i64, instance_name, None, None, method_kind_data()));
        assert_eq!(ctx.sym(instance_init).get_kind(&ctx), ElementKind::INSTANCE_INIT);
        let package = ctx.alloc_symbol(Symbol::new(Kind::PCK, 0, name(&[0x50]), None, None, SymbolKind::PackageSymbol {
            members_field: None,
            fullname: name(&[0x50]),
            package_info: None,
            modle: None,
            sourcefile: None,
        }));
        assert_eq!(ctx.sym(package).get_kind(&ctx), ElementKind::PACKAGE);
        let module = ctx.alloc_symbol(Symbol::new(Kind::MDL, 0, name(&[0x4D]), None, None, SymbolKind::ModuleSymbol {
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
        }));
        assert_eq!(ctx.sym(module).get_kind(&ctx), ElementKind::MODULE);
        let type_parameter = ctx.alloc_symbol(Symbol::new(Kind::TYP, 0, name(&[0x54]), None, None, SymbolKind::TypeVariableSymbol));
        assert_eq!(ctx.sym(type_parameter).get_kind(&ctx), ElementKind::TYPE_PARAMETER);
        let record_component = ctx.alloc_symbol(Symbol::new(Kind::VAR, flags::RECORD, name(&[0x52]), None, None, SymbolKind::RecordComponent {
            accessor: None,
            accessor_meth: None,
            original_annos: List::nil(),
            pos: 0,
            is_varargs: false,
        }));
        assert_eq!(ctx.sym(record_component).get_kind(&ctx), ElementKind::RECORD_COMPONENT);
    }
    use super::*;
    use crate::util::name::NameTableHandle;
    use javac_source::tree::member_reference_tree::ReferenceMode;

    fn name(text: &[u16]) -> Name {
        let table: Rc<NameTableHandle> = NameTableHandle::unshared();
        table.from_chars(text, 0, text.len() as i32)
    }

    /// Creates a name whose table is bound to a Names instance (needed by the
    /// Symbol.getKind CONSTRUCTOR/STATIC_INIT and isStatic branches, which read
    /// names.init/clinit/_this like the upstream code). The Names instance is
    /// returned so the caller keeps it alive (the name table holds a Weak).
    fn name_with_names(text: &str) -> (Rc<crate::util::names::Names>, Name) {
        let names = Rc::new(crate::util::names::Names::new(false));
        let table = names.table.clone();
        let units: Vec<u16> = text.encode_utf16().collect();
        let name = table.from_chars(&units, 0, units.len() as i32);
        (names, name)
    }

    fn class_symbol() -> Symbol {
        let class_name = name(&[0x43]);
        Symbol::new(
            Kind::TYP,
            flags::PUBLIC as i64,
            class_name.clone(),
            None,
            None,
            SymbolKind::ClassSymbol {
                members_field: Some(ScopeId::new(1)),
                fullname: class_name.clone(),
                flatname: class_name,
                sourcefile: None,
                classfile: None,
                trans_local: List::nil(),
                record_components: List::nil(),
                permitted: List::nil(),
                is_permitted_explicit: false,
            },
        )
    }

    #[test]
    fn kind_ordering_and_categories() {
        assert_eq!(Kind::NIL.ordinal(), 0);
        assert_eq!(Kind::PCK.ordinal(), 1);
        assert_eq!(Kind::TYP.ordinal(), 2);
        assert_eq!(Kind::ABSENT_TYP.ordinal(), 17);
        assert!(Kind::TYP.is_valid());
        assert!(!Kind::ERR.is_valid());
        assert!(Kind::AMBIGUOUS.is_resolution_target_error());
        assert!(Kind::MISSING_ENCL.is_resolution_error());
        assert!(!Kind::MISSING_ENCL.is_resolution_target_error());
        assert!(Kind::TYP.better_than(Kind::VAR));
        assert!(!Kind::VAR.better_than(Kind::TYP));
    }

    #[test]
    fn kind_selectors() {
        assert!(KindSelector::TYP.contains(KindSelector::TYP));
        assert!(!KindSelector::TYP.contains(KindSelector::PCK));
        assert!(KindSelector::TYP_PCK.contains(KindSelector::TYP));
        assert!(KindSelector::TYP_PCK.contains(KindSelector::PCK));
        assert!(KindSelector::TYP.subset(KindSelector::TYP_PCK));
        assert_eq!(
            KindSelector::TYP_PCK.kind_names(),
            vec![KindName::CLASS, KindName::PACKAGE]
        );
        assert!(Kind::TYP.matches(KindSelector::TYP_PCK));
        assert!(!Kind::MTH.matches(KindSelector::TYP_PCK));
        assert_eq!(Kind::TYP.to_selector(), KindSelector::TYP);
    }

    #[test]
    fn kind_names_and_display() {
        assert_eq!(Kind::TYP.kind_name(), KindName::CLASS);
        assert_eq!(Kind::PCK.kind_name(), KindName::PACKAGE);
        assert_eq!(Kind::WRONG_MTH.kind_name(), KindName::METHOD);
        assert_eq!(Kind::ABSENT_VAR.absent_kind(), KindName::VAR);
        assert_eq!(KindName::CLASS.name(), "kindname.class");
        assert_eq!(format!("{}", KindName::METHOD), "kindname.method");
        assert_eq!(
            crate::code::kinds::kind_name_for_reference_mode(ReferenceMode::NEW),
            KindName::CONSTRUCTOR
        );
    }

    #[test]
    #[should_panic(expected = "Unexpected kind")]
    fn kind_name_panics_for_invalid_kind() {
        let _ = Kind::AMBIGUOUS.kind_name();
    }

    #[test]
    fn symbol_flags_accessors() {
        let symbol = class_symbol();
        assert_eq!(symbol.flags(), flags::PUBLIC as i64);
        assert!(symbol.is_public());
        assert!(!symbol.is_private());
        assert!(!symbol.is_abstract());
        assert!(!symbol.is_deprecated());
        assert!(!symbol.is_preview_api());
        assert!(!symbol.is_sealed());
        assert!(!symbol.is_anonymous());
        assert!(!symbol.is_dynamic());
        assert_eq!(symbol.kind, Kind::TYP);
    }

    #[test]
    fn symbol_members_and_names() {
        let class = class_symbol();
        assert_eq!(class.members(), Some(ScopeId::new(1)));
        assert_eq!(
            class.get_qualified_name().to_java_string(),
            name(&[0x43]).to_java_string()
        );
        assert_eq!(
            class.flat_name().to_java_string(),
            name(&[0x43]).to_java_string()
        );

        let var = Symbol::new(
            Kind::VAR,
            flags::PRIVATE as i64,
            name(&[0x56]),
            None,
            Some(SymbolId::new(2)),
            SymbolKind::VarSymbol {
                pos: 3,
                adr: -1,
                data_kind: None,
            },
        );
        assert!(var.is_private());
        assert_eq!(var.members(), None);
        assert_eq!(
            var.get_qualified_name().to_java_string(),
            name(&[0x56]).to_java_string()
        );
        match var.kind_data {
            SymbolKind::VarSymbol { pos, adr, .. } => {
                assert_eq!(pos, 3);
                assert_eq!(adr, -1);
            }
            _ => panic!("expected a variable symbol"),
        }
    }

    #[test]
    fn record_component_and_operator_fields() {
        let component = Symbol::new(
            Kind::VAR,
            flags::RECORD,
            name(&[0x52]),
            None,
            None,
            SymbolKind::RecordComponent {
                accessor: Some(SymbolId::new(1)),
                accessor_meth: Some(TreeId::new(2)),
                original_annos: List::nil(),
                pos: 0,
                is_varargs: false,
            },
        );
        match component.kind_data {
            SymbolKind::RecordComponent {
                accessor,
                accessor_meth,
                ..
            } => {
                assert_eq!(accessor, Some(SymbolId::new(1)));
                assert_eq!(accessor_meth, Some(TreeId::new(2)));
            }
            _ => panic!("expected a record component"),
        }

        let operator = Symbol::new(
            Kind::MTH,
            0,
            name(&[0x50]),
            None,
            None,
            SymbolKind::OperatorSymbol {
                opcode: 20,
                access_code: 7,
            },
        );
        match operator.kind_data {
            SymbolKind::OperatorSymbol {
                opcode,
                access_code,
            } => {
                assert_eq!(opcode, 20);
                assert_eq!(access_code, 7);
            }
            _ => panic!("expected an operator symbol"),
        }
    }
}
