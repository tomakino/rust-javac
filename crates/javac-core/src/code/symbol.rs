// Origin: com/sun/tools/javac/code/Symbol.java @ 982d53c (jdk-17.0.19-ga)
// Gap registration (method-level audit of the ported upstream class):
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
// - isCompleted is implemented as CompilerContext::is_symbol_completed (registered adapter).
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
// - apiComplete is implemented as CompilerContext::api_complete_symbol (registered adapter).
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
// End of Gap registration.
// (the Kinds.Kind/KindSelector/KindName part comes from
// com/sun/tools/javac/code/Kinds.java; see the module note below)

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
//!   the Kinds.java lands; it will move to code/kinds.rs then, registered
//!   in PORTING_RULES section 9).
//!
//! NOT PORTED IN THIS STEP (see docs/STATUS.md; no placeholder logic):
//! - SymbolMetadata/AnnoConstruct attribute methods, the metadata field,
//!   poolTag/accept(Symbol.Visitor), toString/location/baseSymbol (the "this"
//!   result needs the arena ID), erasure/externalType (Types), the owner-chain
//!   methods (enclClass/outermostClass/packge) and the Types-dependent methods
//!   (isSubClass/isMemberOf/isEnclosedBy/...);
//! - the Symbol.complete()/apiComplete() methods are implemented as
//!   CompilerContext::complete_symbol/complete_class_symbol (the completer needs
//!   &mut CompilerContext + Symtab, ADR 0001; registered adapter);
//! - the DynamicMethodSymbol/DynamicVarSymbol variants (LoadableConstant),
//!   MethodSymbol.code (jvm/Code) and MethodSymbol.defaultValue (Attribute) and
//!   the Symbol.Visitor interface;
//! - ClassSymbol.annotationTypeMetadata, VarSymbol.data (only the ElementKind
//!   marker subset is represented as VarSymbol.data_kind; the Callable constant
//!   value stored in data is not ported yet) and MethodHandleSymbol private
//!   fields are not represented (dependencies not ported yet; reported);
//! - RootPackageSymbol.missingInfoHandler (MissingInfoHandler is not ported;
//!   the variant carries allowPrivateInvokeVirtual only).

use std::collections::HashMap;
use std::rc::Rc;

pub use javac_tools::lang::model::element::element_kind::ElementKind;
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
/// node-specific fields (SymbolKind). The completer is represented (the metadata
/// fields are not, see the module doc).
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
    /// Java: public Symbol.Completer completer (NULL_COMPLETER -> None)
    pub completer: Option<CompleterRef>,
    /// Java: protected SymbolMetadata metadata (lazily created by
    /// initedMetadata(); Java null -> None).
    pub metadata: Option<crate::code::symbol_metadata::SymbolMetadata>,
    /// The symbol arena id (Java: the Symbol object identity; the port needs the
    /// id to create the SymbolMetadata back-reference). The field is filled by the
    /// unique allocation entry point CompilerContext::alloc_symbol; Clone does not
    /// copy it (a clone is only usable after it enters an arena).
    pub id: std::cell::Cell<Option<crate::ids::SymbolId>>,
    /// Java: the anonymous MethodSymbol baseSymbol() override used by the
    /// polymorphic-signature instance symbols (Resolve.java:2800-2810); Porting rules:
    /// the override is stored as a field and honored by base_symbol.
    pub base_symbol_override: Option<SymbolId>,
    /// The node-specific fields (one variant per upstream concrete subclass).
    pub kind_data: SymbolKind,
}

impl Symbol {
    /// Adapter constructor: the upstream constructor installs
    /// Completer.NULL_COMPLETER; locally None represents NULL_COMPLETER.
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
            completer: None,
            metadata: None,
            id: std::cell::Cell::new(None),
            base_symbol_override: None,
            kind_data,
        }
    }




    /// Java: public List<Attribute.Compound> getRawAttributes()
    pub fn get_raw_attributes(&self) -> List<crate::ids::AttributeId> {
        match &self.metadata {
            None => List::nil(),
            Some(metadata) => metadata.get_declaration_attributes(),
        }
    }

    /// Java: public List<Attribute.TypeCompound> getRawTypeAttributes()
    pub fn get_raw_type_attributes(&self) -> List<crate::ids::AttributeId> {
        match &self.metadata {
            None => List::nil(),
            Some(metadata) => metadata.get_type_attributes(),
        }
    }

    /// Java: public Attribute.Compound attribute(Symbol anno)
    pub fn attribute(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        anno: SymbolId,
    ) -> Option<crate::ids::AttributeId> {
        let attributes = self.get_raw_attributes();
        let mut current = attributes;
        while current.non_empty() {
            let attribute = *current.head();
            let attribute_type = ctx.attr(attribute).ty;
            if attribute_type.and_then(|ty| ctx.typ(ty).tsym) == Some(anno) {
                return Some(attribute);
            }
            current = current.tail();
        }
        None
    }

    /// Java: public boolean annotationsPendingCompletion()
    pub fn annotations_pending_completion(&self) -> bool {
        match &self.metadata {
            None => false,
            Some(metadata) => metadata.pending_completion(),
        }
    }

    /// Java: public void appendAttributes(List<Attribute.Compound> l)
    pub fn append_attributes(&mut self, attributes: List<crate::ids::AttributeId>) {
        if !attributes.is_empty() {
            self.inited_metadata().append(attributes);
        }
    }

    /// Java: public void appendClassInitTypeAttributes(List<Attribute.TypeCompound> l)
    pub fn append_class_init_type_attributes(&mut self, attributes: List<crate::ids::AttributeId>) {
        if !attributes.is_empty() {
            self.inited_metadata().append_class_init_type_attributes(attributes);
        }
    }

    /// Java: public void appendInitTypeAttributes(List<Attribute.TypeCompound> l)
    pub fn append_init_type_attributes(&mut self, attributes: List<crate::ids::AttributeId>) {
        if !attributes.is_empty() {
            self.inited_metadata().append_init_type_attributes(attributes);
        }
    }

    /// Java: public void setTypeAttributes(List<Attribute.TypeCompound> l)
    pub fn set_type_attributes(&mut self, attributes: List<crate::ids::AttributeId>) {
        self.inited_metadata().set_type_attributes(attributes);
    }

    /// Java: public void appendUniqueTypeAttributes(List<Attribute.TypeCompound> l)
    pub fn append_unique_type_attributes(&mut self, attributes: List<crate::ids::AttributeId>) {
        if !attributes.is_empty() {
            self.inited_metadata().append_unique_types(attributes);
        }
    }

    /// Java: public List<Attribute.TypeCompound> getClassInitTypeAttributes()
    pub fn get_class_init_type_attributes(&self) -> List<crate::ids::AttributeId> {
        match &self.metadata {
            None => List::nil(),
            Some(metadata) => metadata.get_class_init_type_attributes(),
        }
    }

    /// Java: public List<Attribute.TypeCompound> getInitTypeAttributes()
    pub fn get_init_type_attributes(&self) -> List<crate::ids::AttributeId> {
        match &self.metadata {
            None => List::nil(),
            Some(metadata) => metadata.get_init_type_attributes(),
        }
    }

    /// Java: public void setInitTypeAttributes(List<Attribute.TypeCompound> l)
    pub fn set_init_type_attributes(&mut self, attributes: List<crate::ids::AttributeId>) {
        self.inited_metadata().set_init_type_attributes(attributes);
    }

    /// Java: public void setClassInitTypeAttributes(List<Attribute.TypeCompound> l)
    pub fn set_class_init_type_attributes(&mut self, attributes: List<crate::ids::AttributeId>) {
        self.inited_metadata().set_class_init_type_attributes(attributes);
    }

    /// Java: public List<Attribute.Compound> getDeclarationAttributes()
    pub fn get_declaration_attributes(&self) -> List<crate::ids::AttributeId> {
        match &self.metadata {
            None => List::nil(),
            Some(metadata) => metadata.get_declaration_attributes(),
        }
    }

    /// Java: public boolean hasAnnotations()
    pub fn has_annotations(&self) -> bool {
        match &self.metadata {
            None => false,
            Some(metadata) => !metadata.is_empty(),
        }
    }

    /// Java: public boolean hasTypeAnnotations()
    pub fn has_type_annotations(&self) -> bool {
        match &self.metadata {
            None => false,
            Some(metadata) => !metadata.is_types_empty(),
        }
    }

    /// Java: public void prependAttributes(List<Attribute.Compound> l)
    pub fn prepend_attributes(&mut self, attributes: List<crate::ids::AttributeId>) {
        if !attributes.is_empty() {
            self.inited_metadata().prepend(attributes);
        }
    }

    /// Java: public void setDeclarationAttributes(List<Attribute.Compound> a)
    pub fn set_declaration_attributes(&mut self, attributes: List<crate::ids::AttributeId>) {
        self.inited_metadata().set_declaration_attributes(attributes);
    }

    /// Java: public void resetAnnotations()
    pub fn reset_annotations(&mut self) {
        self.inited_metadata().reset();
    }

    /// Java: public void setAttributes(Symbol other)
    pub fn set_attributes(
        &mut self,
        ctx: &crate::compiler_context::CompilerContext,
        other: &Symbol,
    ) {
        match &other.metadata {
            None => {}
            Some(other_metadata) => {
                self.inited_metadata().set_attributes(ctx, other_metadata);
            }
        }
    }

    /// Java: protected SymbolMetadata initedMetadata() (Java: lazily creates the
    /// SymbolMetadata bound to this symbol).
    fn inited_metadata(&mut self) -> &mut crate::code::symbol_metadata::SymbolMetadata {
        if self.metadata.is_none() {
            let sym = self
                .id
                .get()
                .expect("AssertionError: symbol is not allocated");
            self.metadata = Some(crate::code::symbol_metadata::SymbolMetadata::new(sym));
        }
        self.metadata.as_mut().expect("metadata")
    }

    /// Java: ModuleSymbol.reset() / ClassSymbol.reset()
    pub fn reset(&mut self) {
        match &mut self.kind_data {
            SymbolKind::ModuleSymbol {
                directives,
                requires,
                exports,
                opens,
                provides,
                uses,
                visible_packages,
                ..
            } => {
                *directives = List::nil();
                *requires = List::nil();
                *exports = List::nil();
                *opens = List::nil();
                *provides = List::nil();
                *uses = List::nil();
                visible_packages.clear();
            }
            SymbolKind::ClassSymbol { .. } => {
                // Java: ClassSymbol.reset() resets metadata (metadata = null).
                // TODO(Annotate): SymbolMetadata is not ported yet; there is
                // no metadata state to reset (registered gap).
            }
            _ => {}
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

impl Clone for Symbol {
    /// Clones the symbol fields; the arena id is not copied because a clone
    /// is only usable after it enters the symbol arena through the unique
    /// allocation entry point (CompilerContext::alloc_symbol records the id and
    /// rebinds a copied metadata). Registered adapter: Java Symbols have no clone
    /// path; the ported phases clone-and-allocate synthetic symbols.
    fn clone(&self) -> Symbol {
        Symbol {
            kind: self.kind,
            flags_field: self.flags_field,
            name: self.name.clone(),
            ty: self.ty,
            owner: self.owner,
            erasure_field: self.erasure_field,
            completer: self.completer.clone(),
            metadata: self.metadata.clone(),
            base_symbol_override: self.base_symbol_override,
            kind_data: self.kind_data.clone(),
            id: std::cell::Cell::new(None),
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
/// Java: the AttrContext/Env state captured by resolution errors for the
/// deferred diagnostics (Resolve.InvisibleSymbolError/AccessError keep the
/// Env<AttrContext>; the port stores the fields the diagnostics read, a
/// registered Porting rules adapter for the Rc<Env> reference).
#[derive(Clone, Debug)]
pub struct ResolveEnvSnapshot {
    pub tree: Option<TreeId>,
    pub toplevel: Option<TreeId>,
    pub encl_class: Option<TreeId>,
    pub encl_class_sym: Option<SymbolId>,
    pub toplevel_package: Option<SymbolId>,
    pub toplevel_module: Option<SymbolId>,
    pub select_super: bool,
    pub visiting_service_implementation: bool,
}

/// Java: the Resolve.ResolveError subclass payload (Resolve.java:3909-4700).
/// The hierarchical Java classes map to this tagged payload stored in the
/// SymbolKind::ResolveError variant (the error object stays a Symbol, as
/// upstream).
#[derive(Clone, Debug)]
pub enum ResolveErrorData {
    /// Resolve.ResolveError base
    SymbolNotFound,
    /// Resolve.InvalidSymbolError (the found symbol is invalid)
    /// Resolve.AccessError (the symbol is not accessible from the site)
    Access {
        env: ResolveEnvSnapshot,
        site: Option<TypeId>,
        /// Java: AccessError.this.site != null && !isAccessible(env, this.site)
        /// frozen at construction (the diagnostic reads it later).
        site_inaccessible: bool,
        sym: SymbolId,
    },
    /// Resolve.InvisibleSymbolError
    Invisible {
        env: ResolveEnvSnapshot,
        suppress_error: bool,
        sym: SymbolId,
    },
    /// Resolve.StaticError
    Static { sym: SymbolId },
    /// Resolve.AmbiguityError (the maximally specific symbols, reverse order)
    Ambiguity { syms: Vec<SymbolId> },
    /// Resolve.BadRestrictedTypeError
    BadRestrictedType { type_name: Name },
    /// Resolve.InapplicableSymbolError (Resolve.java:4104-4183)
    Inapplicable {
        kind: Kind,
        sym: SymbolId,
        details: Option<crate::util::jc_diagnostic::JCDiagnostic>,
    },
    /// Resolve.InapplicableSymbolsError (Resolve.java:4183-4300)
    InapplicableSymbols {
        kind: Kind,
        candidates: Vec<(SymbolId, Option<crate::util::jc_diagnostic::JCDiagnostic>)>,
    },
    /// Resolve.DiamondError (Resolve.java:4313-4350)
    Diamond {
        kind: Kind,
        sym: SymbolId,
        details: Option<crate::util::jc_diagnostic::JCDiagnostic>,
    },
    /// Resolve.BadVarargsMethod (Resolve.java:4617-4650)
    BadVarargs { delegated: SymbolId },
    /// Resolve.BadMethodReferenceError (Resolve.java:4645-4675) extends
    /// StaticError: the found method reference symbol has the wrong staticness.
    BadMethodReference { sym: SymbolId, unbound_lookup: bool },
    /// Resolve.BadConstructorReferenceError (Resolve.java:4680-4690) extends
    /// InvalidSymbolError: an enclosing instance is not available.
    BadConstructorReference { sym: SymbolId },
}

/// Java: public enum SymbolKind (Resolve.ResolveError hierarchy lives here).
#[derive(Clone)]
pub enum SymbolKind {
    /// Java: Resolve.ResolveError (the resolution error is a Symbol subclass)
    ResolveError {
        /// The debug name of the Java error class.
        debug_name: String,
        /// The Java subclass payload.
        data: Box<ResolveErrorData>,
    },
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
        /// Java: private AnnotationTypeMetadata annotationTypeMetadata
        /// (Symbol.java:1290; the not-an-annotation default).
        annotation_type_metadata: crate::comp::annotate::AnnotationTypeMetadata,
    },
    /// Java: Symbol.BindingSymbol (Symbol.java:1861-1902) — a VarSymbol
    /// subclass introduced by a pattern match binding.
    BindingSymbol {
        /// Java: List<BindingSymbol> aliases() — empty means List.of(this).
        aliases: List<SymbolId>,
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
        /// Java: VarSymbol.data (Object) — the ConstantValue attribute value
        /// (Attribute.Constant's value). The Callable form of data is not
        /// ported yet (registered in the module doc).
        data: Option<crate::code::attribute::ConstValue>,
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
        /// Java: public Attribute defaultValue (Symbol.java:1780; null -> None)
        /// — the annotation element default value (AttributeId, Porting rules arena).
        default_value: Option<crate::ids::AttributeId>,
    },
    /// Java: Symbol.MethodHandleSymbol
    MethodHandleSymbol {
        /// The referenced symbol (private field refSym).
        ref_sym: Option<SymbolId>,
        /// Whether this is a getter handle (private field getter).
        getter: bool,
    },
    /// Java: Symbol.DynamicMethodSymbol (an invokedynamic method symbol).
    ///
    /// The symbol carries its bootstrap method handle and the static argument
    /// constants; the ported loadable-constant array is an enum so it can live
    /// inside the arena.
    DynamicMethodSymbol {
        /// The bootstrap method handle.
        bsm: SymbolId,
        /// The static arguments passed to the bootstrap method.
        static_args: Vec<crate::jvm::pool_constant::LoadableConstantRef>,
    },
    /// Java: Symbol.OperatorSymbol
    /// Java: Symbol.DynamicVarSymbol (a constant-dynamic variable symbol).
    ///
    /// The symbol carries the VarSymbol state plus the bootstrap method handle
    /// and the static argument constants of the constant dynamic entry.
    DynamicVarSymbol {
        /// The variable position (declaration order).
        pos: i32,
        /// The variable address (set during code generation).
        adr: i32,
        /// The ElementKind marker of the upstream Object data field.
        data_kind: Option<ElementKind>,
        /// Java: VarSymbol.data (the ConstantValue attribute value).
        data: Option<crate::code::attribute::ConstValue>,
        /// The bootstrap method handle.
        bsm: SymbolId,
        /// The static arguments passed to the bootstrap method.
        static_args: Vec<crate::jvm::pool_constant::LoadableConstantRef>,
    },
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


/// Java: Symbol.Completer reference (the upstream interface is shared by the
/// ClassFinder instance; the shared, mutable reference is kept in Symbol.completer).
pub type CompleterRef = Rc<std::cell::RefCell<dyn Completer>>;

/// Creates a fresh Completer reference (the port creates one wrapper per symbol
/// so nested completion does not borrow the same RefCell twice; registered
/// adapter for the upstream shared ClassFinder instance).
pub type CompleterFactory = Rc<dyn Fn() -> CompleterRef>;

/// Java: Symbol.Completer
///
/// PORTING_RULES: Java throws CompletionFailure; the Rust port returns
/// Result<(), CompletionFailure> from complete(). The compiler context and the
/// symbol table are passed explicitly because symbols are arena values and there
/// is no global Context (ADR 0001).
pub trait Completer {
    /// Java: void complete(Symbol sym) throws CompletionFailure
    fn complete(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        sym: SymbolId,
    ) -> Result<(), CompletionFailure>;

    /// Java: default boolean isTerminal() { return false; }
    fn is_terminal(&self) -> bool {
        false
    }

    /// Java: instanceof DeferredCompleter (adapter: Rust has no instanceof).
    fn is_deferred_completer(&self) -> bool {
        false
    }
}

/// Java: Symbol.Completer.NULL_COMPLETER (a terminal completer that does nothing).
pub struct NullCompleter;

impl Completer for NullCompleter {
    fn complete(
        &mut self,
        _ctx: &mut crate::compiler_context::CompilerContext,
        _syms: &mut crate::code::symtab::Symtab,
        _sym: SymbolId,
    ) -> Result<(), CompletionFailure> {
        Ok(())
    }

    fn is_terminal(&self) -> bool {
        true
    }
}

/// Java: public static class Symbol.CompletionFailure extends RuntimeException
///
/// PORTING_RULES: Rust has no unchecked exceptions, so CompletionFailure is a
/// plain error value returned in Result; diagSupplier -> a boxed closure that is
/// evaluated lazily (RefCell cache like the upstream lazy diag field).
pub struct CompletionFailure {
    /// Java: public final transient DeferredCompletionFailureHandler dcfh
    pub dcfh: Option<Rc<crate::code::deferred_completion_failure_handler::DeferredCompletionFailureHandler>>,
    /// Java: the ClassFinder.BadClassFile subtype marker (ClassFinder.fillIn only
    /// purges partially initialized symbols for BadClassFile; registered adapter
    /// because the Rust error value has no subtype information).
    pub bad_class_file: bool,
    /// Java: public transient Symbol sym
    pub sym: Option<SymbolId>,
    /// Java: private transient JCDiagnostic diag
    ///
    /// PORTING_RULES: boxed so that CompletionFailure stays a small error
    /// value (upstream Java has no error-size concern; the field semantics are
    /// unchanged).
    diag: std::cell::RefCell<Option<Box<crate::util::jc_diagnostic::JCDiagnostic>>>,
    /// Java: private transient Supplier<JCDiagnostic> diagSupplier
    diag_supplier: Option<Box<dyn Fn() -> crate::util::jc_diagnostic::JCDiagnostic>>,
}

impl CompletionFailure {
    /// Java: CompletionFailure(Symbol sym, Supplier<JCDiagnostic> diagSupplier,
    /// DeferredCompletionFailureHandler dcfh)
    pub fn new(
        sym: Option<SymbolId>,
        diag_supplier: Box<dyn Fn() -> crate::util::jc_diagnostic::JCDiagnostic>,
        dcfh: Option<Rc<crate::code::deferred_completion_failure_handler::DeferredCompletionFailureHandler>>,
    ) -> CompletionFailure {
        CompletionFailure {
            dcfh,
            bad_class_file: false,
            sym,
            diag: std::cell::RefCell::new(None),
            diag_supplier: Some(diag_supplier),
        }
    }

    /// Java: public JCDiagnostic getDiagnostic()
    pub fn get_diagnostic(&self) -> Option<crate::util::jc_diagnostic::JCDiagnostic> {
        {
            let mut cache = self.diag.borrow_mut();
            if cache.is_none() && let Some(supplier) = &self.diag_supplier {
                *cache = Some(Box::new(supplier()));
            }
        }
        self.diag.borrow().as_ref().map(|diag| (**diag).clone())
    }

    /// Java: public String getMessage()
    pub fn get_message(&self) -> crate::util::java_string::JavaString {
        match self.get_diagnostic() {
            Some(diag) => diag.get_message(None),
            None => crate::util::java_string::JavaString::new(),
        }
    }

    /// Java: public JCDiagnostic getDetailValue()
    pub fn get_detail_value(&self) -> Option<crate::util::jc_diagnostic::JCDiagnostic> {
        self.get_diagnostic()
    }

    /// Java: public void resetDiagnostic(Supplier<JCDiagnostic> diagSupplier)
    pub fn reset_diagnostic(
        &mut self,
        diag_supplier: Box<dyn Fn() -> crate::util::jc_diagnostic::JCDiagnostic>,
    ) {
        self.diag_supplier = Some(diag_supplier);
        *self.diag.borrow_mut() = None;
    }
}

impl std::fmt::Display for CompletionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get_message().to_string_lossy())
    }
}

impl std::fmt::Debug for CompletionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CompletionFailure: {}", self.get_message().to_string_lossy())
    }
}

impl std::error::Error for CompletionFailure {}


// ----------------------------------------------------------------------
// Java: Symbol subclass constructors (P2 ClassReader). The ClassSymbol
// constructor is produced by Symtab.enterClass (fullname/flatname); these
// helpers cover the member symbols.
// ----------------------------------------------------------------------
impl Symbol {
    /// Java: public VarSymbol(long flags, Name name, Type type, Symbol owner)
    pub fn new_var_symbol(flags: i64, name: Name, ty: TypeId, owner: SymbolId) -> Symbol {
        Symbol::new(
            Kind::VAR,
            flags,
            name,
            Some(ty),
            Some(owner),
            SymbolKind::VarSymbol {
                pos: crate::util::position::NOPOS,
                adr: -1,
                data_kind: None,
                data: None,
            },
        )
    }

    /// Java: public MethodSymbol(long flags, Name name, Type type, Symbol owner)
    pub fn new_method_symbol(flags: i64, name: Name, ty: TypeId, owner: SymbolId) -> Symbol {
        Symbol::new(
            Kind::MTH,
            flags,
            name,
            Some(ty),
            Some(owner),
            SymbolKind::MethodSymbol {
                extra_params: List::nil(),
                captured_locals: List::nil(),
                params: List::nil(),
                default_value: None,
            },
        )
    }

    /// Java: public TypeVariableSymbol(long flags, Name name, Type type, Symbol owner)
    pub fn new_type_variable_symbol(
        flags: i64,
        name: Name,
        ty: TypeId,
        owner: SymbolId,
    ) -> Symbol {
        Symbol::new(
            Kind::TYP,
            flags,
            name,
            Some(ty),
            Some(owner),
            SymbolKind::TypeVariableSymbol,
        )
    }
}


// ----------------------------------------------------------------------
// Java: Symbol accessor methods needed by the ClassReader.
// ----------------------------------------------------------------------
impl Symbol {
    /// Java: public boolean hasOuterInstance()
    pub fn has_outer_instance(&self, ctx: &crate::compiler_context::CompilerContext) -> bool {
        let enclosing_is_class = self
            .ty
            .and_then(|ty| ctx.typ(ty).get_enclosing_type())
            .is_some_and(|outer| ctx.typ(outer).has_tag(crate::code::type_tag::TypeTag::CLASS));
        let flags = self.flags_field;
        enclosing_is_class
            && (flags & (crate::code::flags::INTERFACE as i64 | crate::code::flags::NOOUTERTHIS as i64)) == 0
    }

    /// Java: public boolean isConstructor() { return name == name.table.names.init; }
    pub fn is_constructor(&self, names: &crate::util::names::Names) -> bool {
        self.name.to_java_string() == names.init.to_java_string()
    }

    /// Java: public boolean isRecord() { return (flags_field & RECORD) != 0; }
    pub fn is_record(&self) -> bool {
        (self.flags_field & crate::code::flags::RECORD) != 0
    }
}

// ----------------------------------------------------------------------
// Java: ParamSymbol/MethodSymbol parameter wiring needed by ClassReader.
// ----------------------------------------------------------------------
impl Symbol {
    /// Java: public ParamSymbol(long flags, Name name, Type type, Symbol owner)
    pub fn new_param_symbol(
        flags: i64,
        name: Name,
        ty: crate::ids::TypeId,
        owner: SymbolId,
    ) -> Symbol {
        Symbol::new(
            Kind::VAR,
            flags,
            name,
            Some(ty),
            Some(owner),
            SymbolKind::ParamSymbol,
        )
    }

    /// Java: sym.params = params (MethodSymbol.params field setter).
    pub fn set_params(&mut self, params: List<SymbolId>) {
        if let SymbolKind::MethodSymbol { params: slot, .. } = &mut self.kind_data {
            *slot = params;
        } else {
            panic!("ClassCastException: not a method symbol");
        }
    }
}

// ----------------------------------------------------------------------
// Java: RecordComponent constructor + ClassSymbol.setRecordComponents
// (Record attribute).
// ----------------------------------------------------------------------
impl Symbol {
    /// Java: public RecordComponent(Name name, Type type, Symbol owner)
    pub fn new_record_component(
        name: Name,
        ty: crate::ids::TypeId,
        owner: SymbolId,
    ) -> Symbol {
        Symbol::new(
            Kind::VAR,
            crate::code::flags::PUBLIC as i64,
            name,
            Some(ty),
            Some(owner),
            SymbolKind::RecordComponent {
                accessor: None,
                accessor_meth: None,
                original_annos: List::nil(),
                pos: crate::util::position::NOPOS,
                is_varargs: false,
            },
        )
    }

    /// Java: ClassSymbol.setRecordComponents(List<RecordComponent>)
    pub fn set_record_components(&mut self, components: List<SymbolId>) {
        if let SymbolKind::ClassSymbol {
            record_components, ..
        } = &mut self.kind_data
        {
            *record_components = components;
        } else {
            panic!("ClassCastException: not a class symbol");
        }
    }

    /// Java: public void setData(Object data)
    pub fn set_data(&mut self, value: Option<crate::code::attribute::ConstValue>) {
        if let SymbolKind::VarSymbol { data: slot, .. } = &mut self.kind_data {
            *slot = value;
        } else {
            panic!("ClassCastException: not a variable symbol");
        }
    }

    /// Java: public Object getData()
    pub fn get_data(&self) -> Option<&crate::code::attribute::ConstValue> {
        match &self.kind_data {
            SymbolKind::VarSymbol { data, .. } => data.as_ref(),
            _ => None,
        }
    }
}

impl Symbol {
    /// Java: public Symbol location()
    pub fn location(&self) -> Option<SymbolId> {
        let owner = self.owner?;
        Some(owner)
    }
}
// ----------------------------------------------------------------------
// Java: Symbol owner-chain helpers (P3/Enter). The upstream methods use
// the implicit `this`; the arena port takes the SymbolId and the context
// (ADR 0001 explicit-context adapter).
// ----------------------------------------------------------------------

/// Java: public Symbol location()
pub fn symbol_location(ctx: &CompilerContext, symbol: SymbolId) -> Option<SymbolId> {
    let owner = ctx.sym(symbol).owner?;
    // Java: owner.name == null is not representable (Name is never null here).
    if ctx.sym(owner).name.is_empty()
        && (ctx.sym(owner).flags_field & flags::BLOCK as i64) == 0
        && ctx.sym(owner).kind != Kind::PCK
        && ctx.sym(owner).kind != Kind::TYP
    {
        return None;
    }
    Some(owner)
}

/// Java: public ClassSymbol enclClass()
pub fn symbol_encl_class(ctx: &CompilerContext, symbol: SymbolId) -> Option<SymbolId> {
    let mut current = Some(symbol);
    while let Some(c) = current {
        let is_class = ctx.sym(c).kind == Kind::TYP
            && ctx
                .sym(c)
                .ty
                .map(|ty| ctx.typ(ty).has_tag(crate::code::type_tag::TypeTag::CLASS))
                .unwrap_or(false);
        if is_class {
            return Some(c);
        }
        current = ctx.sym(c).owner;
    }
    None
}

/// Java: public PackageSymbol packge()
pub fn symbol_packge(ctx: &CompilerContext, symbol: SymbolId) -> Option<SymbolId> {
    let mut current = Some(symbol);
    while let Some(c) = current {
        if ctx.sym(c).kind == Kind::PCK {
            return Some(c);
        }
        current = ctx.sym(c).owner;
    }
    None
}

/// Java: public boolean isDirectlyOrIndirectlyLocal()
pub fn symbol_is_directly_or_indirectly_local(ctx: &CompilerContext, symbol: SymbolId) -> bool {
    let Some(owner) = ctx.sym(symbol).owner else {
        return false;
    };
    let owner_kind = ctx.sym(owner).kind;
    if owner_kind == Kind::VAR || owner_kind == Kind::MTH {
        return true;
    }
    if owner_kind == Kind::TYP {
        return symbol_is_directly_or_indirectly_local(ctx, owner);
    }
    false
}

/// Java: MethodSymbol.isStaticOrInstanceInit()
pub fn symbol_is_static_or_instance_init(ctx: &CompilerContext, symbol: SymbolId) -> bool {
    matches!(
        ctx.sym(symbol).get_kind(ctx),
        javac_tools::lang::model::element::element_kind::ElementKind::STATIC_INIT
            | javac_tools::lang::model::element::element_kind::ElementKind::INSTANCE_INIT
    )
}


/// Java: creates a resolution-error symbol (Resolve.ResolveError)
pub fn symbol_new_resolve_error(
    ctx: &mut CompilerContext,
    kind: crate::code::kinds::Kind,
    name: Name,
    debug_name: &str,
    data: ResolveErrorData,
) -> SymbolId {
    let symbol = Symbol::new(
        kind,
        0,
        name,
        None,
        None,
        SymbolKind::ResolveError {
            debug_name: debug_name.to_string(),
            data: Box::new(data),
        },
    );
    ctx.alloc_symbol(symbol)
}

/// The ResolveError payload of a symbol, if any.
pub fn symbol_resolve_error_data(ctx: &CompilerContext, symbol: SymbolId) -> Option<&ResolveErrorData> {
    match &ctx.sym(symbol).kind_data {
        SymbolKind::ResolveError { data, .. } => Some(data),
        _ => None,
    }
}

/// Java: public boolean exists() — ErrorSymbol (ErrSymbol) is the only symbol
/// that does not exist; the port keys on the symbol kind (registered adapter).
pub fn symbol_exists(ctx: &CompilerContext, symbol: SymbolId) -> bool {
    match &ctx.sym(symbol).kind_data {
        SymbolKind::ResolveError { data, .. } => matches!(
            **data,
                | ResolveErrorData::Access { .. }
                | ResolveErrorData::Invisible { .. }
                | ResolveErrorData::Static { .. }
                | ResolveErrorData::Ambiguity { .. }
                | ResolveErrorData::BadMethodReference { .. }
                | ResolveErrorData::BadConstructorReference { .. }
        ),
        _ => ctx.sym(symbol).kind != crate::code::kinds::Kind::ERR,
    }
}

/// Java: public ClassSymbol outermostClass() (Symbol.java:523)
pub fn symbol_outermost_class(ctx: &CompilerContext, symbol: SymbolId) -> Option<SymbolId> {
    let mut sym = symbol;
    let mut previous: Option<SymbolId> = None;
    while ctx.sym(sym).kind != crate::code::kinds::Kind::PCK {
        previous = Some(sym);
        match ctx.sym(sym).owner {
            Some(owner) => sym = owner,
            None => break,
        }
    }
    previous
}

/// Java: public boolean isEnclosedBy(ClassSymbol clazz) (Symbol.java:559)
pub fn symbol_is_enclosed_by(ctx: &CompilerContext, symbol: SymbolId, clazz: SymbolId) -> bool {
    let mut sym = symbol;
    while ctx.sym(sym).kind != crate::code::kinds::Kind::PCK {
        if sym == clazz {
            return true;
        }
        match ctx.sym(sym).owner {
            Some(owner) => sym = owner,
            None => return false,
        }
    }
    false
}

/// Java: public boolean isSubClass(Symbol base, Types types) — the ClassSymbol
/// implementation (Symbol.java:1389); the base Symbol implementation is an
/// AssertionError (only ClassSymbols are queried).
pub fn symbol_is_sub_class(
    ctx: &mut CompilerContext,
    types: &crate::code::types::Types,
    syms: &crate::code::symtab::Symtab,
    symbol: SymbolId,
    base: SymbolId,
) -> bool {
    if ctx.sym(symbol).kind != crate::code::kinds::Kind::TYP {
        panic!("AssertionError: isSubClass");
    }
    if symbol == base {
        return true;
    }
    let base_interface =
        (ctx.sym(base).flags_field & crate::code::flags::INTERFACE as i64) != 0;
    let self_ty = ctx.sym(symbol).ty.expect("class symbol type");
    if base_interface {
        let mut t = self_ty;
        while ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::CLASS) {
            let interfaces = types.interfaces(ctx, syms, t);
            let mut current = interfaces;
            while current.non_empty() {
                let interface = *current.head();
                let reachable = ctx
                    .typ(interface)
                    .tsym
                    .map(|tsym| symbol_is_sub_class(ctx, types, syms, tsym, base))
                    .unwrap_or(false);
                if reachable {
                    return true;
                }
                current = current.tail();
            }
            t = types.supertype(ctx, syms, t);
        }
        false
    } else {
        let mut t = self_ty;
        while ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::CLASS) {
            if ctx.typ(t).tsym == Some(base) {
                return true;
            }
            t = types.supertype(ctx, syms, t);
        }
        false
    }
}

/// Java: public final boolean isAccessibleIn(Symbol clazz, Types types)
/// (Symbol.java:616)
pub fn symbol_is_accessible_in(
    ctx: &mut CompilerContext,
    types: &crate::code::types::Types,
    syms: &crate::code::symtab::Symtab,
    symbol: SymbolId,
    clazz: SymbolId,
) -> bool {
    let access = ctx.sym(symbol).flags_field & crate::code::flags::AccessFlags as i64;
    use crate::code::flags;
    if access == flags::PUBLIC as i64 {
        return true;
    }
    if access == flags::PRIVATE as i64 {
        return ctx.sym(symbol).owner == Some(clazz);
    }
    if access == flags::PROTECTED as i64 {
        // we model interfaces as extending Object
        return (ctx.sym(clazz).flags_field & flags::INTERFACE as i64) == 0;
    }
    if access != 0 {
        return true;
    }
    let this_package = symbol_packge(ctx, symbol).expect("symbol package");
    let mut sup = Some(clazz);
    while let Some(mut current) = sup {
        if Some(current) == ctx.sym(symbol).owner {
            break;
        }
        // while (sup.type.hasTag(TYPEVAR)) sup = sup.type.getUpperBound().tsym;
        loop {
            let current_ty = ctx.sym(current).ty.expect("super type");
            if !ctx.typ(current_ty).has_tag(crate::code::type_tag::TypeTag::TYPEVAR) {
                break;
            }
            let bound = match &ctx.typ(current_ty).kind {
                crate::code::type_::TypeKindData::TypeVar { bound, .. } => *bound,
                _ => None,
            }
            .expect("type variable bound");
            current = ctx.typ(bound).tsym.expect("type variable bound symbol");
        }
        let current_ty = ctx.sym(current).ty.expect("super type");
        if ctx.typ(current_ty).has_tag(crate::code::type_tag::TypeTag::ERROR) {
            return true; // error recovery
        }
        if (ctx.sym(current).flags_field & flags::COMPOUND as i64) != 0 {
            sup = ctx.typ(current_ty).tsym;
            continue;
        }
        if symbol_packge(ctx, current) != Some(this_package) {
            return false;
        }
        let next = types.supertype(ctx, syms, current_ty);
        sup = ctx.typ(next).tsym;
    }
    (ctx.sym(clazz).flags_field & flags::INTERFACE as i64) == 0
}

/// Java: public boolean isInheritedIn(Symbol clazz, Types types)
/// (Symbol.java:651)
pub fn symbol_is_inherited_in(
    ctx: &mut CompilerContext,
    types: &crate::code::types::Types,
    syms: &crate::code::symtab::Symtab,
    symbol: SymbolId,
    clazz: SymbolId,
) -> bool {
    symbol_is_accessible_in(ctx, types, syms, symbol, clazz)
}

/// Java: private boolean hiddenIn(ClassSymbol clazz, Types types)
/// (Symbol.java:567)
pub fn symbol_hidden_in(
    ctx: &mut CompilerContext,
    types: &crate::code::types::Types,
    syms: &crate::code::symtab::Symtab,
    symbol: SymbolId,
    clazz: SymbolId,
) -> bool {
    let hidden = symbol_hidden_in_internal(ctx, types, syms, symbol, clazz);
    let hidden = hidden.expect("hiddenInInternal result");
    hidden != symbol
}

/// Java: private Symbol hiddenInInternal(ClassSymbol currentClass, Types types)
/// (Symbol.java:582)
fn symbol_hidden_in_internal(
    ctx: &mut CompilerContext,
    types: &crate::code::types::Types,
    syms: &crate::code::symtab::Symtab,
    symbol: SymbolId,
    current_class: SymbolId,
) -> Option<SymbolId> {
    if ctx.sym(symbol).owner == Some(current_class) {
        return Some(symbol);
    }
    let name = ctx.sym(symbol).name.clone();
    let kind = ctx.sym(symbol).kind;
    if let Some(scope) = ctx.sym(current_class).members() {
        let symbols = ctx.scope_get_symbols_by_name(
            scope,
            &name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        for candidate in symbols {
            if ctx.sym(candidate).kind == kind
                && (kind != crate::code::kinds::Kind::MTH
                    || ((ctx.sym(candidate).flags_field & crate::code::flags::STATIC as i64) != 0
                        && types.is_sub_signature(
                            ctx,
                            syms,
                            ctx.sym(candidate).ty.expect("candidate type"),
                            ctx.sym(symbol).ty.expect("symbol type"),
                        )))
            {
                return Some(candidate);
            }
        }
    }
    let current_ty = ctx.sym(current_class).ty.expect("current class type");
    let supertype = types.supertype(ctx, syms, current_ty);
    let mut supers = types.interfaces(ctx, syms, current_ty).prepend(supertype);
    let mut hidden_sym: Option<SymbolId> = None;
    while supers.non_empty() {
        let st = *supers.head();
        supers = supers.tail();
        let tsym = match (ctx.typ(st).has_tag(crate::code::type_tag::TypeTag::CLASS), ctx.typ(st).tsym) {
            (true, Some(tsym)) => tsym,
            _ => continue,
        };
        let result = symbol_hidden_in_internal(ctx, types, syms, symbol, tsym);
        if result == Some(symbol) {
            return Some(symbol);
        } else if let Some(other) = result {
            hidden_sym = Some(other);
        }
    }
    hidden_sym
}

/// Java: public boolean isMemberOf(TypeSymbol clazz, Types types)
/// (Symbol.java:552)
pub fn symbol_is_member_of(
    ctx: &mut CompilerContext,
    types: &crate::code::types::Types,
    syms: &crate::code::symtab::Symtab,
    symbol: SymbolId,
    clazz: SymbolId,
) -> bool {
    if ctx.sym(symbol).owner == Some(clazz) {
        return true;
    }
    let owner = ctx.sym(symbol).owner.expect("symbol owner");
    symbol_is_sub_class(ctx, types, syms, clazz, owner)
        && symbol_is_inherited_in(ctx, types, syms, symbol, clazz)
        && !symbol_hidden_in(ctx, types, syms, symbol, clazz)
}

/// Java: public Symbol location(Type site, Types types) (Symbol.java:330)
pub fn symbol_location_site(
    ctx: &mut CompilerContext,
    types: &crate::code::types::Types,
    syms: &crate::code::symtab::Symtab,
    symbol: SymbolId,
    site: TypeId,
) -> Option<SymbolId> {
    let owner = ctx.sym(symbol).owner?;
    let owner_name = ctx.sym(owner).name.clone();
    if owner_name.is_empty() {
        return symbol_location(ctx, symbol);
    }
    let Some(owner_ty) = ctx.sym(owner).ty else {
        return Some(owner);
    };
    if !ctx.typ(owner_ty).has_tag(crate::code::type_tag::TypeTag::CLASS) {
        return Some(owner);
    }
    let Some(ownertype) = types.as_outer_super(ctx, syms, site, owner) else {
        return Some(owner);
    };
    match ctx.typ(ownertype).tsym {
        Some(tsym) => Some(tsym),
        None => Some(owner),
    }
}


/// Java: PackageSymbol.fullname (Symbol.java PackageSymbol field)
pub fn package_fullname(ctx: &CompilerContext, pkg: SymbolId) -> Option<crate::util::name::Name> {
    match &ctx.sym(pkg).kind_data {
        SymbolKind::PackageSymbol { fullname, .. }
        | SymbolKind::RootPackageSymbol { fullname, .. } => Some(fullname.clone()),
        _ => None,
    }
}

/// Java: PackageSymbol.modle (Symbol.java PackageSymbol field)
pub fn package_module(ctx: &CompilerContext, pkg: SymbolId) -> Option<SymbolId> {
    match &ctx.sym(pkg).kind_data {
        SymbolKind::PackageSymbol { modle, .. }
        | SymbolKind::RootPackageSymbol { modle, .. } => *modle,
        _ => None,
    }
}

/// Java: public BindingSymbol(long flags, Name name, Type type, Symbol
/// owner) (Symbol.java:1874-1876): super(flags | HASINIT | MATCH_BINDING, ...).
pub fn new_binding_symbol(
    flags: i64,
    name: Name,
    ty: Option<TypeId>,
    owner: Option<SymbolId>,
) -> Symbol {
    Symbol::new(
        Kind::VAR,
        flags | crate::code::flags::HASINIT as i64 | crate::code::flags::MATCH_BINDING,
        name,
        ty,
        owner,
        SymbolKind::BindingSymbol {
            aliases: List::nil(),
        },
    )
}

/// Java: BindingSymbol.aliases() — empty aliases field means List.of(this)
/// (Symbol.java:1880-1883).
pub fn binding_symbol_aliases(
    ctx: &CompilerContext,
    sym: SymbolId,
) -> List<SymbolId> {
    match &ctx.sym(sym).kind_data {
        SymbolKind::BindingSymbol { aliases } if !aliases.is_empty() => aliases.clone(),
        _ => List::of(sym),
    }
}

/// Java: BindingSymbol.isAliasFor(BindingSymbol b) (Symbol.java:1878-1880).
pub fn binding_symbol_is_alias_for(
    ctx: &CompilerContext,
    sym: SymbolId,
    other: SymbolId,
) -> bool {
    let aliases = binding_symbol_aliases(ctx, sym);
    let mut current = binding_symbol_aliases(ctx, other);
    while current.non_empty() {
        if !aliases.iter().any(|alias| alias == *current.head()) {
            return false;
        }
        current = current.tail();
    }
    true
}

/// Java: BindingSymbol.preserveBinding() (Symbol.java:1885-1887).
pub fn binding_symbol_preserve(ctx: &mut CompilerContext, sym: SymbolId) {
    ctx.sym_mut(sym).flags_field |= crate::code::flags::MATCH_BINDING_TO_OUTER;
}

/// Java: BindingSymbol.isPreserved() (Symbol.java:1889-1891).
pub fn binding_symbol_is_preserved(ctx: &CompilerContext, sym: SymbolId) -> bool {
    (ctx.sym(sym).flags_field & crate::code::flags::MATCH_BINDING_TO_OUTER) != 0
}



impl Symbol {
    /// Java: public AnnotationTypeMetadata getAnnotationTypeMetadata()
    /// (Symbol.java:867/1614; the base Symbol asserts, only ClassSymbol has it).
    pub fn get_annotation_type_metadata(
        &self,
    ) -> Option<&crate::comp::annotate::AnnotationTypeMetadata> {
        match &self.kind_data {
            SymbolKind::ClassSymbol {
                annotation_type_metadata,
                ..
            } => Some(annotation_type_metadata),
            _ => None,
        }
    }

    /// Java: ClassSymbol.setAnnotationTypeMetadata (Symbol.java:1614 area;
    /// the upstream ClassSymbol constructor installs the metadata).
    pub fn set_annotation_type_metadata(
        &mut self,
        metadata: crate::comp::annotate::AnnotationTypeMetadata,
    ) {
        if let SymbolKind::ClassSymbol {
            annotation_type_metadata,
            ..
        } = &mut self.kind_data
        {
            *annotation_type_metadata = metadata;
        }
    }

    /// Mutable accessor (the upstream AnnotationTypeMetadata setters mutate the
    /// metadata object through the ClassSymbol field).
    pub fn get_annotation_type_metadata_mut(
        &mut self,
    ) -> Option<&mut crate::comp::annotate::AnnotationTypeMetadata> {
        match &mut self.kind_data {
            SymbolKind::ClassSymbol {
                annotation_type_metadata,
                ..
            } => Some(annotation_type_metadata),
            _ => None,
        }
    }

    /// Java: ClassSymbol.isAnnotationType() (Symbol.java:1618-1620).
    pub fn is_annotation_type(&self) -> bool {
        (self.flags_field & crate::code::flags::ANNOTATION as i64) != 0
    }
}

// ---------------------------------------------------------------------------
// Symbol : isSubClass/isMemberOf/isAccessibleIn/isInheritedIn/overrides/
// implementation (Symbol.java:341/545/620-760/1389/1995-2200), 1:1.
// The Java class hierarchy is expressed by dispatching on SymbolKind (registered
// adapter adapter); the free functions carry the explicit Types/Symtab/CompilerContext
// services (ADR 0001).
// ---------------------------------------------------------------------------

/// Java: public Symbol baseSymbol() (Symbol.java:341-343). The upstream override
/// is materialized by Symbol.clone() subclasses; the port has no clone, so a
/// symbol is its own base symbol (registered Porting rules).
pub fn base_symbol(ctx: &crate::compiler_context::CompilerContext, sym: SymbolId) -> SymbolId {
    if let Some(over) = ctx.sym(sym).base_symbol_override {
        return over;
    }
    let _ = ctx;
    sym
}

/// Java: public boolean isSubClass(Symbol base, Types types) —
/// ClassSymbol.isSubClass (Symbol.java:1389-1406).
pub fn is_sub_class(
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    sym: SymbolId,
    base: SymbolId,
) -> bool {
    assert!(
        matches!(ctx.sym(sym).kind, Kind::TYP | Kind::ERR),
        "AssertionError: isSubClass requires a class symbol"
    );
    if sym == base {
        return true;
    }
    let base_is_interface =
        (ctx.sym(base).flags_field & crate::code::flags::INTERFACE as i64) != 0;
    let sym_ty = ctx.sym(sym).ty.expect("class type");
    let class_tag = crate::code::type_tag::TypeTag::CLASS;
    if base_is_interface {
        let mut t = sym_ty;
        while ctx.typ(t).has_tag(class_tag) {
            let interfaces = types.interfaces(ctx, syms, t);
            let mut current = interfaces;
            while current.non_empty() {
                let iface = *current.head();
                let iface_tsym = ctx.typ(iface).tsym.expect("interface symbol");
                if is_sub_class(ctx, syms, types, iface_tsym, base) {
                    return true;
                }
                current = current.tail();
            }
            t = types.supertype(ctx, syms, t);
        }
        false
    } else {
        let mut t = sym_ty;
        while ctx.typ(t).has_tag(class_tag) {
            if ctx.typ(t).tsym == Some(base) {
                return true;
            }
            t = types.supertype(ctx, syms, t);
        }
        false
    }
}

/// Java: public final boolean isAccessibleIn(Symbol clazz, Types types)
/// (Symbol.java:620-655).
pub fn is_accessible_in(
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    sym: SymbolId,
    clazz: SymbolId,
) -> bool {
    let flags_field = ctx.sym(sym).flags_field;
    let access = flags_field & crate::code::flags::AccessFlags as i64;
    let public_flag = crate::code::flags::PUBLIC as i64;
    let private_flag = crate::code::flags::PRIVATE as i64;
    let protected_flag = crate::code::flags::PROTECTED as i64;
    if access == public_flag {
        return true;
    }
    if access == private_flag {
        return ctx.sym(sym).owner == Some(clazz);
    }
    if access == protected_flag {
        return (ctx.sym(clazz).flags_field & crate::code::flags::INTERFACE as i64) == 0;
    }
    if access == 0 {
        let this_package = packge(ctx, sym);
        let mut sup = Some(clazz);
        while let Some(sup_sym) = sup {
            if sup_sym == ctx.sym(sym).owner.unwrap_or(sym) {
                break;
            }
            // Java: while (sup.type.hasTag(TYPEVAR))
            // sup = sup.type.getUpperBound().tsym;
            let mut sup_ty = ctx.sym(sup_sym).ty.expect("supertype");
            while ctx
                .typ(sup_ty)
                .has_tag(crate::code::type_tag::TypeTag::TYPEVAR)
            {
                let upper = ctx
                    .typ(sup_ty)
                    .get_upper_bound()
                    .expect("type var upper bound");
                let Some(tsym) = ctx.typ(upper).tsym else { break };
                sup = Some(tsym);
                sup_ty = tsym_type(ctx, tsym);
            }
            let Some(sup_sym) = sup else { break };
            let sup_type = ctx.sym(sup_sym).ty.expect("supertype");
            if types.is_erroneous(ctx, syms, sup_type) {
                return true;
            }
            if (ctx.sym(sup_sym).flags_field & crate::code::flags::COMPOUND as i64) != 0 {
                sup = ctx.sym(sup_sym).owner;
                continue;
            }
            if packge(ctx, sup_sym) != this_package {
                return false;
            }
            let super_type = types.supertype(ctx, syms, sup_type);
            sup = ctx.typ(super_type).tsym;
        }
        return (ctx.sym(clazz).flags_field & crate::code::flags::INTERFACE as i64) == 0;
    }
    true
}

fn tsym_type(
    ctx: &crate::compiler_context::CompilerContext,
    tsym: SymbolId,
) -> crate::ids::TypeId {
    ctx.sym(tsym).ty.expect("symbol type")
}

/// Java: private PackageSymbol packge() / Symbol.packge (Symbol.java:305-320).
pub(crate) fn packge(
    ctx: &crate::compiler_context::CompilerContext,
    sym: SymbolId,
) -> Option<SymbolId> {
    let mut current = Some(sym);
    while let Some(sym) = current {
        if ctx.sym(sym).kind == Kind::PCK {
            return Some(sym);
        }
        current = ctx.sym(sym).owner;
    }
    None
}

/// Java: public boolean isInheritedIn(Symbol clazz, Types types)
/// (Symbol.java:666-668 default; MethodSymbol override 2160-2167).
pub fn is_inherited_in(
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    sym: SymbolId,
    clazz: SymbolId,
) -> bool {
    if ctx.sym(sym).kind == Kind::MTH {
        let flags_field = ctx.sym(sym).flags_field;
        let access = flags_field & crate::code::flags::AccessFlags as i64;
        if access == crate::code::flags::PUBLIC as i64 {
            let owner_is_interface = ctx
                .sym(sym)
                .owner
                .map(|owner| (ctx.sym(owner).flags_field & crate::code::flags::INTERFACE as i64) != 0)
                .unwrap_or(false);
            return !owner_is_interface
                || clazz == ctx.sym(sym).owner.unwrap_or(sym)
                || (flags_field & crate::code::flags::STATIC as i64) == 0;
        }
    }
    is_accessible_in(ctx, syms, types, sym, clazz)
}

/// Java: private boolean hiddenIn(ClassSymbol clazz, Types types)
/// (Symbol.java:573-581).
fn hidden_in(
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    sym: SymbolId,
    clazz: SymbolId,
) -> bool {
    let hidden = hidden_in_internal(ctx, syms, types, sym, clazz);
    let hidden = hidden.expect("hiddenInInternal cannot return null");
    hidden != sym
}

/// Java: private Symbol hiddenInInternal(ClassSymbol currentClass, Types types)
/// (Symbol.java:588-608).
fn hidden_in_internal(
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    sym: SymbolId,
    current_class: SymbolId,
) -> Option<SymbolId> {
    if Some(current_class) == ctx.sym(sym).owner {
        return Some(sym);
    }
    let sym_name = ctx.sym(sym).name.clone();
    let members = match &ctx.sym(current_class).kind_data {
        SymbolKind::ClassSymbol { members_field, .. } => *members_field,
        _ => None,
    };
    if let Some(members) = members {
        let found = ctx.scope_get_symbols_by_name(
            members,
            &sym_name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        for candidate in found {
            let candidate_kind = ctx.sym(candidate).kind;
            if candidate_kind == ctx.sym(sym).kind
                && (candidate_kind != Kind::MTH
                    || ((ctx.sym(candidate).flags_field & crate::code::flags::STATIC as i64) != 0
                        && {
                            let candidate_ty = ctx.sym(candidate).ty.expect("candidate type");
                            let sym_ty = ctx.sym(sym).ty.expect("symbol type");
                            types.is_sub_signature(ctx, syms, candidate_ty, sym_ty)
                        }))
            {
                return Some(candidate);
            }
        }
    }
    let current_ty = ctx.sym(current_class).ty.expect("class type");
    let mut supertypes =
        types
            .interfaces(ctx, syms, current_ty)
            .prepend(types.supertype(ctx, syms, current_ty));
    let mut hidden_sym: Option<SymbolId> = None;
    while supertypes.non_empty() {
        let st = *supertypes.head();
        supertypes = supertypes.tail();
        if !ctx
            .typ(st)
            .has_tag(crate::code::type_tag::TypeTag::CLASS)
        {
            continue;
        }
        let tsym = ctx.typ(st).tsym.expect("class symbol");
        let found = hidden_in_internal(ctx, syms, types, sym, tsym);
        if found == Some(sym) {
            return Some(sym);
        }
        if found.is_some() {
            hidden_sym = found;
        }
    }
    hidden_sym
}

/// Java: public boolean isMemberOf(TypeSymbol clazz, Types types)
/// (Symbol.java:558-563).
pub fn is_member_of(
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    sym: SymbolId,
    clazz: SymbolId,
) -> bool {
    if ctx.sym(sym).owner == Some(clazz) {
        return true;
    }
    let owner = ctx.sym(sym).owner.expect("member owner");
    is_sub_class(ctx, syms, types, clazz, owner)
        && is_inherited_in(ctx, syms, types, sym, clazz)
        && !hidden_in(ctx, syms, types, sym, clazz)
}

/// Java: public boolean isEnclosedBy(ClassSymbol clazz) (Symbol.java:568-573)
pub fn is_enclosed_by(
    ctx: &crate::compiler_context::CompilerContext,
    sym: SymbolId,
    clazz: SymbolId,
) -> bool {
    let mut current = Some(sym);
    while let Some(current_sym) = current {
        if ctx.sym(current_sym).kind == Kind::PCK {
            break;
        }
        if current_sym == clazz {
            return true;
        }
        current = ctx.sym(current_sym).owner;
    }
    false
}


/// Java: MethodSymbol.isOverridableIn(TypeSymbol origin) (Symbol.java:2143-2158)
fn is_overridable_in(
    ctx: &crate::compiler_context::CompilerContext,
    sym: SymbolId,
    origin: SymbolId,
) -> bool {
    let flags_field = ctx.sym(sym).flags_field;
    let access = flags_field & crate::code::flags::AccessFlags as i64;
    if access == crate::code::flags::PRIVATE as i64 {
        return false;
    }
    if access == crate::code::flags::PUBLIC as i64 {
        let owner_is_interface = ctx
            .sym(sym)
            .owner
            .map(|owner| (ctx.sym(owner).flags_field & crate::code::flags::INTERFACE as i64) != 0)
            .unwrap_or(false);
        return !owner_is_interface || (flags_field & crate::code::flags::STATIC as i64) == 0;
    }
    if access == crate::code::flags::PROTECTED as i64 {
        return (ctx.sym(origin).flags_field & crate::code::flags::INTERFACE as i64) == 0;
    }
    if access == 0 {
        return packge(ctx, sym) == packge(ctx, origin)
            && (ctx.sym(origin).flags_field & crate::code::flags::INTERFACE as i64) == 0;
    }
    false
}

/// Java: public boolean overrides(Symbol _other, TypeSymbol origin, Types types,
/// boolean checkResult) (Symbol.java:672-674 base false; MethodSymbol
/// 2068-2072/2085-2141)
pub fn overrides(
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &mut crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    sym: SymbolId,
    other: SymbolId,
    origin: SymbolId,
    check_result: bool,
) -> bool {
    overrides_full(ctx, syms, types, sym, other, origin, check_result, true)
}

/// Java: MethodSymbol.overrides(..., boolean requireConcreteIfInherited)
/// (Symbol.java:2085-2141)
#[allow(clippy::too_many_arguments)]
pub fn overrides_full(
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &mut crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    sym: SymbolId,
    other: SymbolId,
    origin: SymbolId,
    check_result: bool,
    require_concrete_if_inherited: bool,
) -> bool {
    if ctx.sym(sym).kind != Kind::MTH {
        return false;
    }
    if ctx.sym(sym).name.to_java_string() == ctx.sym(sym).name.table.names().init.to_java_string() {
        return false;
    }
    if ctx.sym(other).kind != Kind::MTH {
        return false;
    }
    if sym == other {
        return true;
    }
    let sym_owner = ctx.sym(sym).owner.expect("method owner");
    let other_owner = ctx.sym(other).owner.expect("method owner");
    let sym_flags = ctx.sym(sym).flags_field;
    let other_flags = ctx.sym(other).flags_field;
    let abstract_flag = crate::code::flags::ABSTRACT as i64;
    let default_flag = crate::code::flags::DEFAULT;
    // check for a direct implementation
    if is_overridable_in(ctx, other, sym_owner) {
        let owner_ty = ctx.sym(sym_owner).ty.expect("owner type");
        if types
            .as_super(ctx, syms, owner_ty, other_owner)
            .is_some()
        {
            let mt = types.member_type(ctx, syms, owner_ty, sym);
            let ot = types.member_type(ctx, syms, owner_ty, other);
            if types.is_sub_signature(ctx, syms, mt, ot) {
                if !check_result {
                    return true;
                }
                if types.return_type_substitutable(ctx, syms, mt, ot) {
                    return true;
                }
            }
        }
    }
    // check for an inherited implementation
    if ((sym_flags & abstract_flag) != 0 && require_concrete_if_inherited)
        || ((other_flags & abstract_flag) == 0 && (other_flags & default_flag) == 0)
        || !is_overridable_in(ctx, other, origin)
        || !is_member_of(ctx, syms, types, sym, origin)
    {
        return false;
    }
    let origin_ty = ctx.sym(origin).ty.expect("origin type");
    let mt = types.member_type(ctx, syms, origin_ty, sym);
    let ot = types.member_type(ctx, syms, origin_ty, other);
    if !types.is_sub_signature(ctx, syms, mt, ot) {
        return false;
    }
    if !check_result {
        return true;
    }
    let mut no_warnings = crate::util::warner::Warner::new(None);
    types.result_subtype(ctx, syms, mt, ot, &mut no_warnings)
}

/// Java: public boolean binaryOverrides(Symbol _other, TypeSymbol origin,
/// Types types) (Symbol.java:2020-2035)
pub fn binary_overrides(
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &mut crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    sym: SymbolId,
    other: SymbolId,
    origin: SymbolId,
) -> bool {
    if ctx.sym(sym).kind != Kind::MTH || ctx.sym(other).kind != Kind::MTH {
        return false;
    }
    if ctx.sym(sym).name.to_java_string() == ctx.sym(sym).name.table.names().init.to_java_string() {
        return false;
    }
    if sym == other {
        return true;
    }
    let owner = ctx.sym(sym).owner.expect("method owner");
    let other_owner = ctx.sym(other).owner.expect("other owner");
    let owner_ty = ctx.sym(owner).ty.expect("owner type");
    let origin_ty = ctx.sym(origin).ty.expect("origin type");
    let sym_ty = ctx.sym(sym).ty.expect("method type");
    let other_ty = ctx.sym(other).ty.expect("method type");
    if is_overridable_in(ctx, other, owner)
        && types.as_super(ctx, syms, owner_ty, other_owner).is_some()
    {
        let sym_erasure = types.erasure(ctx, syms, sym_ty);
        let other_erasure = types.erasure(ctx, syms, other_ty);
        if types.is_same_type(ctx, syms, sym_erasure, other_erasure) {
            return true;
        }
    }
    let sym_flags = ctx.sym(sym).flags_field;
    let abstract_flag = crate::code::flags::ABSTRACT as i64;
    if (sym_flags & abstract_flag) != 0
        || !is_overridable_in(ctx, other, origin)
        || !is_member_of(ctx, syms, types, sym, origin)
    {
        return false;
    }
    let sym_erasure = types.erasure(ctx, syms, sym_ty);
    let other_erasure = types.erasure(ctx, syms, other_ty);
    let _ = origin_ty;
    types.is_same_type(ctx, syms, sym_erasure, other_erasure)
}

/// Java: public MethodSymbol binaryImplementation(ClassSymbol origin, Types types)
/// (Symbol.java:2042-2056)
pub fn binary_implementation(
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &mut crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    sym: SymbolId,
    origin: SymbolId,
) -> Option<SymbolId> {
    let mut current = Some(origin);
    while let Some(current_sym) = current {
        let members = match &ctx.sym(current_sym).kind_data {
            SymbolKind::ClassSymbol { members_field, .. } => *members_field,
            _ => None,
        };
        if let Some(members) = members {
            let name = ctx.sym(sym).name.clone();
            let found = ctx.scope_get_symbols_by_name(
                members,
                &name,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            );
            for candidate in found {
                if ctx.sym(candidate).kind == Kind::MTH
                    && binary_overrides(ctx, syms, types, candidate, sym, origin)
                {
                    return Some(candidate);
                }
            }
        }
        let current_ty = ctx.sym(current_sym).ty.expect("class type");
        let super_type = types.supertype(ctx, syms, current_ty);
        current = ctx.typ(super_type).tsym;
    }
    None
}

/// Java: public MethodSymbol implementation(TypeSymbol origin, Types types,
/// boolean checkResult) / implementation(..., Predicate<Symbol>)
/// (Symbol.java:2168-2187)
pub fn implementation(
    ctx: &mut crate::compiler_context::CompilerContext,
    syms: &mut crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    sym: SymbolId,
    origin: SymbolId,
    check_result: bool,
) -> Option<SymbolId> {
    let result = types.implementation_filtered(
        ctx,
        syms,
        sym,
        origin,
        check_result,
        crate::code::types::ImplementationFilterKind::Default,
    );
    if result.is_some() {
        return result;
    }
    let origin_ty = ctx.sym(origin).ty.expect("origin type");
    let is_interface = (ctx.sym(origin).flags_field & crate::code::flags::INTERFACE as i64) != 0;
    if types.is_derived_raw(ctx, syms, origin_ty) && !is_interface {
        let super_type = types.supertype(ctx, syms, origin_ty);
        let super_tsym = ctx.typ(super_type).tsym.expect("supertype symbol");
        return implementation(ctx, syms, types, sym, super_tsym, check_result);
    }
    None
}


impl Symbol {
    /// Java: public Object getConstValue() (Symbol.java:1734-1765).
    ///
    /// # Arguments
    ///   - (none).
    ///
    /// # Returns
    ///   - the constant value of this variable (None for non-constants and for
    ///     exception/resource parameters).
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Symbol.java:1734-1765.
    pub fn get_const_value(&self) -> Option<crate::code::attribute::ConstValue> {
        let kind_data = match &self.kind_data {
            SymbolKind::VarSymbol { data_kind, data, .. } => (data_kind, data),
            _ => return None,
        };
        let (data_kind, data) = kind_data;
        if *data_kind == Some(crate::code::symbol::ElementKind::EXCEPTION_PARAMETER)
            || *data_kind == Some(crate::code::symbol::ElementKind::RESOURCE_VARIABLE)
        {
            return None;
        }
        // The Java Callable<?> lazy-initializer channel is not represented: the
        // constant is computed when the symbol is entered (Porting adapter).
        data.clone()
    }
}


impl Symbol {
    /// Java: public boolean isDirectlyOrIndirectlyLocal() (Symbol.java:438-446).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - sym: the symbol.
    ///
    /// # Returns
    ///   - true when the symbol is declared (directly or indirectly) local to a
    ///     method or variable initializer.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Symbol.java:438-446.
    pub fn is_directly_or_indirectly_local(ctx: &CompilerContext, sym: SymbolId) -> bool {
        let Some(owner) = ctx.sym(sym).owner else {
            return false;
        };
        let owner_kind = ctx.sym(owner).kind;
        if owner_kind == crate::code::kinds::Kind::VAR || owner_kind == crate::code::kinds::Kind::MTH {
            return true;
        }
        if owner_kind == crate::code::kinds::Kind::TYP {
            return Symbol::is_directly_or_indirectly_local(ctx, owner);
        }
        false
    }
}


impl Symbol {
    /// Java: boolean isPermittedExplicit / permitted (ClassSymbol fields,
    /// sealed classes; Symbol.java:1660-1680).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - sym: the class symbol.
    ///
    /// # Returns
    ///   - the permitted subclasses list (empty for non-sealed classes).
    ///
    /// # Panics
    ///   - none (non-class symbols return an empty list).
    ///
    /// Upstream: Symbol.java:1660-1680.
    pub fn permitted_subs(ctx: &CompilerContext, sym: SymbolId) -> List<SymbolId> {
        match &ctx.sym(sym).kind_data {
            SymbolKind::ClassSymbol { permitted, .. } => permitted.clone(),
            _ => List::nil(),
        }
    }

    /// Java: boolean isPermittedExplicit (ClassSymbol field, sealed classes).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - sym: the class symbol.
    ///
    /// # Returns
    ///   - the is_permitted_explicit flag.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Symbol.java:1660-1680.
    pub fn is_permitted_explicit(ctx: &CompilerContext, sym: SymbolId) -> bool {
        match &ctx.sym(sym).kind_data {
            SymbolKind::ClassSymbol { is_permitted_explicit, .. } => *is_permitted_explicit,
            _ => false,
        }
    }

    /// Adapter: the enclosing module of the package/symbol (Symbol.packge().modle).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - sym: the symbol.
    ///
    /// # Returns
    ///   - the module symbol when present.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Symbol.java:305-320 (PackageSymbol.modle).
    pub fn symbol_module(ctx: &CompilerContext, sym: SymbolId) -> Option<SymbolId> {
        match &ctx.sym(sym).kind_data {
            SymbolKind::PackageSymbol { modle, .. } => *modle,
            _ => None,
        }
    }
}

impl Symbol {
    /// Java: public void markAbstractIfNeeded(Types types) (Symbol.java:1568-1582).
    ///
    /// # Arguments
    ///   - ctx/syms/types: explicit services;
    ///   - envs: the type environments (Types.enter.getEnv);
    ///   - allow_default_methods: the DEFAULT_METHODS feature flag;
    ///   - sym: the class symbol.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Symbol.java:1568-1582.
    pub fn mark_abstract_if_needed(
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        envs: &crate::comp::type_envs::TypeEnvs,
        allow_default_methods: bool,
        sym: SymbolId,
    ) {
        let c_ty = match ctx.sym(sym).ty {
            Some(t) => t,
            None => return,
        };
        if envs.get(sym).is_none() {
            return;
        }
        let flags_value = ctx.sym(sym).flags_field;
        if (flags_value & (crate::code::flags::ENUM as i64)) == 0 {
            return;
        }
        let supertype = types.supertype(ctx, syms, c_ty);
        let super_tsym = ctx.typ(supertype).tsym;
        if super_tsym != Some(syms.enum_sym) {
            return;
        }
        if (flags_value & ((crate::code::flags::FINAL as i64) | (crate::code::flags::ABSTRACT as i64))) != 0 {
            return;
        }
        if types
            .first_unimplemented_abstract(ctx, syms, envs, allow_default_methods, sym)
            .is_some()
        {
            ctx.sym_mut(sym).flags_field |= crate::code::flags::ABSTRACT as i64;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
/// Classifies how a member is accessed by an operator expression.
///
/// Dereference is code 0, simple assignment is 2, pre/post increment and
/// decrement occupy the even codes 4 through 10, and FIRSTASGOP starts the
/// compound-assignment range; the backend uses the parity of the derived code
/// to choose between invokevirtual and invokespecial.
pub enum AccessCode {
    /// UNKNOWN(-1, Tag.NO_TAG)
    UNKNOWN,
    /// DEREF(0, Tag.NO_TAG)
    DEREF,
    /// ASSIGN(2, Tag.ASSIGN)
    ASSIGN,
    /// PREINC(4, Tag.PREINC)
    PREINC,
    /// PREDEC(6, Tag.PREDEC)
    PREDEC,
    /// POSTINC(8, Tag.POSTINC)
    POSTINC,
    /// POSTDEC(10, Tag.POSTDEC)
    POSTDEC,
    /// FIRSTASGOP(12, Tag.NO_TAG)
    FIRSTASGOP,
}

impl AccessCode {
/// Return the numeric access code assigned to this enum constant.
///
/// dereference is 0, assignment is 2, pre/post increment/decrement occupy the
/// even numbers 4..=10 and FIRSTASGOP starts the compound-assignment range at 12.
///
/// # Returns
///   - the upstream access code.
///
/// # Panics
///   - none.
///
    pub fn code(self) -> i32 {
        match self {
            AccessCode::UNKNOWN => -1,
            AccessCode::DEREF => 0,
            AccessCode::ASSIGN => 2,
            AccessCode::PREINC => 4,
            AccessCode::PREDEC => 6,
            AccessCode::POSTINC => 8,
            AccessCode::POSTDEC => 10,
            AccessCode::FIRSTASGOP => 12,
        }
    }

/// Return the operator tag associated with this access code.
///
/// the assignment and increment/decrement access codes map to their own tags.
///
/// # Returns
///   - the associated tree tag.
///
/// # Panics
///   - none.
///
    pub fn tag(self) -> crate::tree::jc_tree::Tag {
        match self {
            AccessCode::UNKNOWN | AccessCode::DEREF | AccessCode::FIRSTASGOP => {
                crate::tree::jc_tree::Tag::NO_TAG
            }
            AccessCode::ASSIGN => crate::tree::jc_tree::Tag::ASSIGN,
            AccessCode::PREINC => crate::tree::jc_tree::Tag::PREINC,
            AccessCode::PREDEC => crate::tree::jc_tree::Tag::PREDEC,
            AccessCode::POSTINC => crate::tree::jc_tree::Tag::POSTINC,
            AccessCode::POSTDEC => crate::tree::jc_tree::Tag::POSTDEC,
        }
    }

/// Return the number of access-code slots used by the backend tables.
///
/// the bytecode ranges exactly as upstream: binary operators, string_add, and the
/// shift-with-long-count opcodes each contribute two access codes.
///
/// # Returns
///   - the number of access-code slots.
///
/// # Panics
///   - none.
///
    pub fn number_of_access_codes() -> i32 {
        (crate::jvm::byte_codes::LUSHRL - crate::jvm::byte_codes::ISHLL
            + crate::jvm::byte_codes::LXOR
            + 2
            - crate::jvm::byte_codes::IADD)
            * 2
            + AccessCode::FIRSTASGOP.code()
            + 2
    }

/// Map a numeric access code back to its enum constant.
///
/// fixed enum values and returns UNKNOWN when the code does not correspond to a
/// declared constant, mirroring the upstream fallback.
///
/// # Arguments
///   - code: the numeric access code.
///
/// # Returns
///   - the matching access code or UNKNOWN.
///
/// # Panics
///   - none.
///
    pub fn get_from_code(code: i32) -> AccessCode {
        for a_code in [
            AccessCode::UNKNOWN,
            AccessCode::DEREF,
            AccessCode::ASSIGN,
            AccessCode::PREINC,
            AccessCode::PREDEC,
            AccessCode::POSTINC,
            AccessCode::POSTDEC,
            AccessCode::FIRSTASGOP,
        ] {
            if a_code.code() == code {
                return a_code;
            }
        }
        AccessCode::UNKNOWN
    }

/// Compute the access code for an operator tag and bytecode opcode.
///
/// their fixed codes; binary operators map to `(opcode - iadd) * 2 + FIRSTASGOP`;
/// string addition and the shift-with-long-count range use the corresponding
/// upstream offsets; any other opcode yields -1.
///
/// # Arguments
///   - tag: the operator tree tag;
///   - opcode: the backend bytecode opcode.
///
/// # Returns
///   - the derived access code, or -1 when the opcode is not an access operator.
///
/// # Panics
///   - none.
///
    pub fn from(tag: crate::tree::jc_tree::Tag, opcode: i32) -> i32 {
        match tag {
            crate::tree::jc_tree::Tag::PREINC => return AccessCode::PREINC.code(),
            crate::tree::jc_tree::Tag::PREDEC => return AccessCode::PREDEC.code(),
            crate::tree::jc_tree::Tag::POSTINC => return AccessCode::POSTINC.code(),
            crate::tree::jc_tree::Tag::POSTDEC => return AccessCode::POSTDEC.code(),
            _ => {}
        }
        use crate::jvm::byte_codes::{IADD, ISHLL, LUSHRL, LXOR, STRING_ADD};
        if (IADD..=LXOR).contains(&opcode) {
            return (opcode - IADD) * 2 + AccessCode::FIRSTASGOP.code();
        } else if opcode == STRING_ADD {
            return (LXOR + 1 - IADD) * 2 + AccessCode::FIRSTASGOP.code();
        } else if (ISHLL..=LUSHRL).contains(&opcode) {
            return (opcode - ISHLL + LXOR + 2 - IADD) * 2 + AccessCode::FIRSTASGOP.code();
        }
        -1
    }
}

impl Symbol {
/// Return the (cached) access code of this operator symbol for a tree tag.
///
/// AccessCode.from(tag, opcode) and caches it on the OperatorSymbol; cached values
/// are reused unless the tag is an increment/decrement unary operator, which must
/// recompute because its direction is encoded in the tag.
///
/// # Arguments
///   - tag: the operator tree tag.
///
/// # Returns
///   - the cached or newly derived access code.
///
/// # Panics
///   - the symbol is not an OperatorSymbol.
///
    pub fn get_access_code(&mut self, tag: crate::tree::jc_tree::Tag) -> i32 {
        let (opcode, access_code) = match &self.kind_data {
            SymbolKind::OperatorSymbol {
                opcode,
                access_code,
            } => (*opcode, *access_code),
            _ => panic!("AssertionError: not an OperatorSymbol"),
        };
        if access_code != i32::MIN && !tag.is_inc_or_dec_unary_op() {
            return access_code;
        }
        let new_code = AccessCode::from(tag, opcode);
        if let SymbolKind::OperatorSymbol { access_code, .. } = &mut self.kind_data {
            *access_code = new_code;
        }
        new_code
    }
}

/// Return the external erased type of a symbol.
///
/// Constructors of inner classes gain the enclosing instance class as their
/// first parameter; every other symbol uses its erased type. The helper is the
/// Rust form of Symbol.externalType, which the access-constructor synthesis
/// needs to describe the synthesized constructor.
///
/// # Arguments
///   - ctx: the compiler context;
///   - types: the type relations service;
///   - syms: the symbol table;
///   - names: the name table (for the constructor name);
///   - sym: the symbol whose external type is computed.
///
/// # Returns
///   - the erased external type.
///
/// # Panics
///   - the symbol or its owner has no type.
pub fn external_type(
    ctx: &mut crate::compiler_context::CompilerContext,
    types: &crate::code::types::Types,
    syms: &crate::code::symtab::Symtab,
    names: &crate::util::names::Names,
    sym: SymbolId,
) -> crate::ids::TypeId {
    let sym_ty = ctx.sym(sym).ty.expect("symbol type");
    let erased = types.erasure(ctx, syms, sym_ty);
    let is_init = ctx.sym(sym).name == names.init;
    let owner_has_outer = ctx
        .sym(sym)
        .owner
        .map(|owner| ctx.sym(owner).has_outer_instance(ctx))
        .unwrap_or(false);
    if is_init && owner_has_outer {
        let owner = ctx.sym(sym).owner.expect("constructor owner");
        let owner_ty = ctx.sym(owner).ty.expect("owner type");
        let enclosing = ctx
            .typ(owner_ty)
            .get_enclosing_type()
            .expect("enclosing type");
        let outer_this_type = types.erasure(ctx, syms, enclosing);
        let mut method_ty = ctx.typ(erased).clone();
        if let crate::code::type_::TypeKindData::MethodType { argtypes, .. } =
            &mut method_ty.kind
        {
            *argtypes = argtypes.prepend(outer_this_type);
        }
        ctx.alloc_type(method_ty)
    } else {
        erased
    }

}


/// Build a method-handle symbol for an accessed symbol.
///
/// The Java helper is VarSymbol.asMethodHandle(boolean) / MethodSymbol
/// equivalents; the handle kind and getter flag are recorded on the symbol so
/// the class writer can emit the reference kind.
///
/// # Arguments
///   - ctx: the compiler context;
///   - sym: the referenced symbol;
///   - getter: whether this handle reads a field.
///
/// # Returns
///   - the fresh method-handle symbol.
///
/// # Panics
///   - the referenced symbol has no type.
pub fn as_method_handle(
    ctx: &mut crate::compiler_context::CompilerContext,
    sym: SymbolId,
    getter: bool,
) -> SymbolId {
    let referenced = ctx.sym(sym).clone();
    let flags = referenced.flags_field;
    let name = referenced.name.clone();
    let ty = referenced.ty.expect("referenced symbol type");
    let owner = referenced.owner.unwrap_or(sym);
    let symbol = Symbol::new(
        Kind::MTH,
        flags,
        name,
        Some(ty),
        Some(owner),
        SymbolKind::MethodHandleSymbol {
            ref_sym: Some(sym),
            getter,
        },
    );
    ctx.alloc_symbol(symbol)
}

/// Build an invokedynamic method symbol.
///
/// # Arguments
///   - ctx: the compiler context;
///   - name: the invokedynamic method name;
///   - owner: the described symbol owner;
///   - bsm: the bootstrap method handle;
///   - ty: the method type;
///   - static_args: the bootstrap static arguments.
///
/// # Returns
///   - the fresh dynamic method symbol.
///
/// # Panics
///   - none.
pub fn new_dynamic_method_symbol(
    ctx: &mut crate::compiler_context::CompilerContext,
    name: Name,
    owner: SymbolId,
    bsm: SymbolId,
    ty: crate::ids::TypeId,
    static_args: Vec<crate::jvm::pool_constant::LoadableConstantRef>,
) -> SymbolId {
    let symbol = Symbol::new(
        Kind::MTH,
        0,
        name,
        Some(ty),
        Some(owner),
        SymbolKind::DynamicMethodSymbol { bsm, static_args },
    );
    ctx.alloc_symbol(symbol)
}


/// Return whether a symbol carries type annotations.
pub fn symbol_has_type_annotations(
    ctx: &crate::compiler_context::CompilerContext,
    sym: SymbolId,
) -> bool {
    ctx.sym(sym).has_type_annotations()
}

/// Return the raw type attributes of a symbol.
pub fn symbol_raw_type_attributes(
    ctx: &crate::compiler_context::CompilerContext,
    sym: SymbolId,
) -> List<crate::ids::AttributeId> {
    ctx.sym(sym).get_raw_type_attributes()
}

/// Return whether a variable symbol is an exception parameter.
pub fn symbol_is_exception_parameter(
    ctx: &crate::compiler_context::CompilerContext,
    sym: SymbolId,
) -> bool {
    matches!(
        &ctx.sym(sym).kind_data,
        SymbolKind::VarSymbol {
            data_kind: Some(ElementKind::EXCEPTION_PARAMETER),
            ..
        }
    )
}

/// Return whether a variable symbol is a resource variable.
pub fn symbol_is_resource_variable(
    ctx: &crate::compiler_context::CompilerContext,
    sym: SymbolId,
) -> bool {
    matches!(
        &ctx.sym(sym).kind_data,
        SymbolKind::VarSymbol {
            data_kind: Some(ElementKind::RESOURCE_VARIABLE),
            ..
        }
    )
}
/// Create a constant-dynamic variable symbol.
///
/// # Arguments
///   - ctx: the compiler context;
///   - name: the constant name;
///   - owner: the owner symbol;
///   - bsm: the bootstrap method handle;
///   - ty: the constant type;
///   - static_args: the bootstrap static arguments.
///
/// # Returns
///   - the fresh dynamic variable symbol.
///
/// # Panics
///   - none.
pub fn new_dynamic_var_symbol(
    ctx: &mut crate::compiler_context::CompilerContext,
    name: Name,
    owner: SymbolId,
    bsm: SymbolId,
    ty: crate::ids::TypeId,
    static_args: Vec<crate::jvm::pool_constant::LoadableConstantRef>,
) -> SymbolId {
    let symbol = Symbol::new(
        Kind::VAR,
        0,
        name,
        Some(ty),
        Some(owner),
        SymbolKind::DynamicVarSymbol {
            pos: crate::util::position::NOPOS,
            adr: -1,
            data_kind: None,
            data: None,
            bsm,
            static_args,
        },
    );
    ctx.alloc_symbol(symbol)
}

/// Return whether a symbol is a dynamic (invokedynamic/constant-dynamic)
/// symbol.
///
/// # Arguments
///   - ctx: the compiler context;
///   - sym: the symbol to test.
///
/// # Returns
///   - true for DynamicMethodSymbol and DynamicVarSymbol.
///
/// # Panics
///   - none.
pub fn symbol_is_dynamic(ctx: &crate::compiler_context::CompilerContext, sym: SymbolId) -> bool {
    matches!(
        ctx.sym(sym).kind_data,
        SymbolKind::DynamicMethodSymbol { .. } | SymbolKind::DynamicVarSymbol { .. }
    )
}

/// Return the static arguments of a dynamic symbol.
///
/// # Arguments
///   - ctx: the compiler context;
///   - sym: the dynamic symbol.
///
/// # Returns
///   - the static argument constants, or None for a non-dynamic symbol.
///
/// # Panics
///   - none.
pub fn dynamic_static_args(
    ctx: &crate::compiler_context::CompilerContext,
    sym: SymbolId,
) -> Option<Vec<crate::jvm::pool_constant::LoadableConstantRef>> {
    match &ctx.sym(sym).kind_data {
        SymbolKind::DynamicMethodSymbol { static_args, .. }
        | SymbolKind::DynamicVarSymbol { static_args, .. } => Some(static_args.clone()),
        _ => None,
    }
}

/// Return the bootstrap method handle of a dynamic symbol.
///
/// # Arguments
///   - ctx: the compiler context;
///   - sym: the dynamic symbol.
///
/// # Returns
///   - the bootstrap method symbol, or None for a non-dynamic symbol.
///
/// # Panics
///   - none.
pub fn dynamic_bootstrap_method(
    ctx: &crate::compiler_context::CompilerContext,
    sym: SymbolId,
) -> Option<SymbolId> {
    match &ctx.sym(sym).kind_data {
        SymbolKind::DynamicMethodSymbol { bsm, .. }
        | SymbolKind::DynamicVarSymbol { bsm, .. } => Some(*bsm),
        _ => None,
    }
}

/// Return the referenced symbol of a method-handle symbol.
///
/// # Arguments
///   - ctx: the compiler context;
///   - sym: the method-handle symbol.
///
/// # Returns
///   - the referenced symbol, or None for a non-handle symbol.
///
/// # Panics
///   - none.
pub fn method_handle_base_symbol(
    ctx: &crate::compiler_context::CompilerContext,
    sym: SymbolId,
) -> Option<SymbolId> {
    match &ctx.sym(sym).kind_data {
        SymbolKind::MethodHandleSymbol { ref_sym, .. } => *ref_sym,
        _ => None,
    }
}

/// Return the reference kind of a method-handle symbol.
///
/// # Arguments
///   - ctx: the compiler context;
///   - sym: the method-handle symbol.
///
/// # Returns
///   - the ClassFile REF_* reference kind, or None for a non-handle symbol.
///
/// # Panics
///   - none.
pub fn method_handle_reference_kind(
    ctx: &crate::compiler_context::CompilerContext,
    sym: SymbolId,
) -> Option<i32> {
    let (ref_sym, getter) = match &ctx.sym(sym).kind_data {
        SymbolKind::MethodHandleSymbol { ref_sym, getter } => ((*ref_sym)?, *getter),
        _ => return None,
    };
    let referenced = ctx.sym(ref_sym);
    let kind = referenced.kind;
    Some(if kind == Kind::VAR {
        if getter {
            if referenced.is_static(ctx) {
                crate::jvm::class_file::REF_GETSTATIC
            } else {
                crate::jvm::class_file::REF_GETFIELD
            }
        } else if referenced.is_static(ctx) {
            crate::jvm::class_file::REF_PUTSTATIC
        } else {
            crate::jvm::class_file::REF_PUTFIELD
        }
    } else if referenced_is_constructor(ctx, ref_sym) {
        crate::jvm::class_file::REF_NEWINVOKESPECIAL
    } else if referenced.is_static(ctx) {
        crate::jvm::class_file::REF_INVOKESTATIC
    } else if let Some(owner) = referenced.owner
        && ctx.sym(owner).kind == Kind::TYP
        && ctx.sym(owner).is_interface()
    {
        crate::jvm::class_file::REF_INVOKEINTERFACE
    } else {
        crate::jvm::class_file::REF_INVOKEVIRTUAL
    })
}

/// Return whether a symbol is a constructor.
///
/// The check needs the name table; it is kept local to the helper because the
/// method-handle reference kind needs it for the invoked-special case.
fn referenced_is_constructor(
    ctx: &crate::compiler_context::CompilerContext,
    sym: SymbolId,
) -> bool {
    let name = &ctx.sym(sym).name;
    name.to_java_string().to_string_lossy() == "<init>"
}

/// Return the constant-pool tag of a symbol.
///
/// # Arguments
///   - ctx: the compiler context;
///   - sym: the symbol.
///
/// # Returns
///   - the JVMS constant pool tag.
///
/// # Panics
///   - the symbol kind has no valid pool entry.
pub fn symbol_pool_tag(ctx: &crate::compiler_context::CompilerContext, sym: SymbolId) -> i32 {
    let symbol = ctx.sym(sym);
    match &symbol.kind_data {
        SymbolKind::MethodHandleSymbol { .. } => crate::jvm::class_file::CONSTANT_METHODHANDLE,
        SymbolKind::DynamicMethodSymbol { .. } => crate::jvm::class_file::CONSTANT_INVOKEDYNAMIC,
        SymbolKind::DynamicVarSymbol { .. } => crate::jvm::class_file::CONSTANT_DYNAMIC,
        _ => match symbol.kind {
            Kind::VAR => crate::jvm::class_file::CONSTANT_FIELDREF,
            Kind::MTH => {
                if let Some(owner) = symbol.owner
                    && ctx.sym(owner).kind == Kind::TYP
                    && ctx.sym(owner).is_interface()
                {
                    crate::jvm::class_file::CONSTANT_INTERFACEMETHODREF
                } else {
                    crate::jvm::class_file::CONSTANT_METHODREF
                }
            }
            Kind::MDL => crate::jvm::class_file::CONSTANT_MODULE,
            Kind::PCK => crate::jvm::class_file::CONSTANT_PACKAGE,
            _ => panic!("AssertionError: Invalid pool entry"),
        },
    }
}

/// Return the constant-pool key of a symbol.
///
/// # Arguments
///   - ctx: the compiler context;
///   - sym: the symbol.
///
/// # Returns
///   - the pool key carrying the symbol identity (or the handle identity for
///     method-handle symbols).
///
/// # Panics
///   - none.
pub fn symbol_pool_key(
    ctx: &crate::compiler_context::CompilerContext,
    sym: SymbolId,
) -> crate::jvm::pool_constant::PoolKey {
    match &ctx.sym(sym).kind_data {
        SymbolKind::MethodHandleSymbol { .. } => {
            crate::jvm::pool_constant::PoolKey::MethodHandle(sym)
        }
        SymbolKind::DynamicMethodSymbol { .. } | SymbolKind::DynamicVarSymbol { .. } => {
            crate::jvm::pool_constant::PoolKey::Dynamic(sym)
        }
        _ => crate::jvm::pool_constant::PoolKey::Symbol(sym),
    }
}

// ----------------------------------------------------------------------
// Java: Symbol.toString() and the subclass overrides (the display layer).
// ----------------------------------------------------------------------

/// Java: Symbol.toString() and subclass overrides.
///
/// # Arguments
///   - ctx: the compiler context;
///   - sym: the symbol to display.
///
/// # Returns
///   - the symbol display string (Symbol/ClassSymbol/MethodSymbol/VarSymbol/
///     PackageSymbol/ModuleSymbol toString rules).
///
/// # Panics
///   - a method symbol has no owner (upstream invariant).
pub fn symbol_to_string(
    ctx: &crate::compiler_context::CompilerContext,
    sym: SymbolId,
) -> String {
    let symbol = ctx.sym(sym);
    match &symbol.kind_data {
        SymbolKind::ClassSymbol { fullname, flatname, .. } => {
            if symbol.name.is_empty() {
                // Java: ClassSymbol.className (Symbol.java:1360-1366):
                // Log.getLocalizedString("anonymous.class", flatname).
                let messages = crate::util::javac_messages::JavacMessages::new(
                    crate::util::javac_messages::DEFAULT_BUNDLE_NAME,
                );
                messages.get_localized_string(
                    "compiler.misc.anonymous.class",
                    &[flatname.to_java_string().to_string_lossy().to_string()],
                )
            } else {
                fullname.to_java_string().to_string_lossy().to_string()
            }
        }
        SymbolKind::PackageSymbol { fullname, .. }
        | SymbolKind::RootPackageSymbol { fullname, .. } => {
            // Java: PackageSymbol.toString (Symbol.java:1161-1163).
            fullname.to_java_string().to_string_lossy().to_string()
        }
        SymbolKind::ModuleSymbol { .. } => {
            // Java: ModuleSymbol.toString (Symbol.java:1064-1071): null name is
            // "<unknown>", empty is "<unnamed>". The port has no null Name.
            if symbol.name.is_empty() {
                String::from("<unnamed>")
            } else {
                symbol.name.to_java_string().to_string_lossy().to_string()
            }
        }
        SymbolKind::MethodSymbol { .. } | SymbolKind::DynamicMethodSymbol { .. } => {
            method_symbol_to_string(ctx, sym)
        }
        _ => symbol.name.to_java_string().to_string_lossy().to_string(),
    }
}

/// Java: MethodSymbol.toString (Symbol.java:1952-1966).
fn method_symbol_to_string(
    ctx: &crate::compiler_context::CompilerContext,
    sym: SymbolId,
) -> String {
    let method = ctx.sym(sym);
    let flags = method.flags_field;
    if (flags & i64::from(crate::code::flags::BLOCK)) != 0 {
        return method
            .owner
            .map(|owner| {
                ctx.sym(owner)
                    .name
                    .to_java_string()
                    .to_string_lossy()
                    .to_string()
            })
            .expect("method owner");
    }
    let mut result = if method.name.content_equals(&javac_strings::JavaString::from("<init>")) {
        method
            .owner
            .map(|owner| {
                ctx.sym(owner)
                    .name
                    .to_java_string()
                    .to_string_lossy()
                    .to_string()
            })
            .expect("method owner")
    } else {
        method.name.to_java_string().to_string_lossy().to_string()
    };
    if let Some(method_ty) = method.ty {
        // Java: if (type.hasTag(FORALL)) s = "<" + getTypeArguments() + ">" + s.
        if ctx.typ(method_ty).has_tag(crate::code::type_tag::TypeTag::FORALL) {
            if let crate::code::type_::TypeKindData::ForAll { tvars, .. } =
                &ctx.typ(method_ty).kind
            {
                result = format!(
                    "<{}>{}",
                    crate::code::type_::type_list_to_string(ctx, tvars),
                    result
                );
            }
        }
        if let Some(params) = method_type_params(ctx, method_ty) {
            let varargs = (flags & i64::from(crate::code::flags::VARARGS)) != 0;
            result.push('(');
            result.push_str(&argtypes_to_string(ctx, &params, varargs));
            result.push(')');
        }
    }
    result
}

fn method_type_params(
    ctx: &crate::compiler_context::CompilerContext,
    ty: crate::ids::TypeId,
) -> Option<crate::util::list::List<crate::ids::TypeId>> {
    match &ctx.typ(ty).kind {
        crate::code::type_::TypeKindData::MethodType { argtypes, .. } => Some(argtypes.clone()),
        crate::code::type_::TypeKindData::ForAll { qtype: Some(qtype), .. } => {
            method_type_params(ctx, *qtype)
        }
        _ => None,
    }
}

fn argtypes_to_string(
    ctx: &crate::compiler_context::CompilerContext,
    params: &crate::util::list::List<crate::ids::TypeId>,
    varargs: bool,
) -> String {
    if !varargs {
        return crate::code::type_::type_list_to_string(ctx, params);
    }
    let mut result = String::new();
    let mut current = params.clone();
    while current.tail().non_empty() {
        result.push_str(&crate::code::type_::type_to_string(ctx, *current.head()));
        result.push(',');
        current = current.tail();
    }
    if current.non_empty() {
        let last = *current.head();
        if ctx.typ(last).has_tag(crate::code::type_tag::TypeTag::ARRAY) {
            if let crate::code::type_::TypeKindData::ArrayType { elemtype: Some(elemtype), .. } =
                &ctx.typ(last).kind
            {
                result.push_str(&crate::code::type_::type_to_string(ctx, elemtype.clone()));
                // Java: argtypes(varargs) appends the array type's annotations
                // before the ellipsis (Type.java:532-534).
                result.push_str(&crate::code::type_::annotation_mirrors_string(ctx, last));
            }
            result.push_str("...");
        } else {
            result.push_str(&crate::code::type_::type_to_string(ctx, last.clone()));
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::kinds::{KindName, KindSelector};
    use std::cell::Cell;
    use std::rc::Rc;

    fn test_diag() -> crate::util::jc_diagnostic::JCDiagnostic {
        let mut ctx = crate::util::context::Context::new();
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut ctx);
        let factory = crate::util::jc_diagnostic::Factory::new(messages, "compiler");
        factory.fragment(crate::resources::compiler_properties::fragments::illegal_start_of_class_file().into())
    }

    #[test]
    fn completion_failure_caches_lazy_diagnostic() {
        let calls = Rc::new(Cell::new(0));
        let calls_for_supplier = Rc::clone(&calls);
        let failure = CompletionFailure::new(
            None,
            Box::new(move || {
                calls_for_supplier.set(calls_for_supplier.get() + 1);
                test_diag()
            }),
            None,
        );
        assert_eq!(calls.get(), 0);
        let first = failure.get_diagnostic();
        assert!(first.is_some());
        assert_eq!(calls.get(), 1);
        let _second = failure.get_diagnostic();
        assert_eq!(calls.get(), 1, "the diagnostic is cached like the Java field");
        assert!(!failure.get_message().is_empty());
    }

    #[test]
    fn completion_failure_reset_diagnostic_recomputes() {
        let calls = Rc::new(Cell::new(0));
        let calls_for_supplier = Rc::clone(&calls);
        let mut failure = CompletionFailure::new(
            None,
            Box::new(move || {
                calls_for_supplier.set(calls_for_supplier.get() + 1);
                test_diag()
            }),
            None,
        );
        let _ = failure.get_diagnostic();
        let calls_for_reset = Rc::clone(&calls);
        failure.reset_diagnostic(Box::new(move || {
            calls_for_reset.set(calls_for_reset.get() + 1);
            test_diag()
        }));
        let _ = failure.get_diagnostic();
        assert_eq!(calls.get(), 2);
    }

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
                annotation_type_metadata: Default::default(),
        }
    }

    fn method_kind_data() -> SymbolKind {
        SymbolKind::MethodSymbol {
            extra_params: List::nil(),
            captured_locals: List::nil(),
            params: List::nil(),
            default_value: None,
        }
    }

    fn var_kind_data(data_kind: Option<ElementKind>) -> SymbolKind {
        SymbolKind::VarSymbol {
            pos: 0,
            adr: -1,
            data_kind,
            data: None,
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
                annotation_type_metadata: Default::default(),
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
                data: None,
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



impl Symbol {
    /// Java: public NestingKind getNestingKind() (Symbol.java:1548-1560).
    ///
    /// # Arguments
    ///   - ctx: the compiler context.
    ///
    /// # Returns
    ///   - the nesting kind.
    ///
    /// # Panics
    ///   - none.
    pub fn get_nesting_kind(&self, ctx: &CompilerContext) -> crate::code::kinds::NestingKind {
        let Some(owner) = self.owner else {
            return crate::code::kinds::NestingKind::TopLevel;
        };
        if ctx.sym(owner).kind == crate::code::kinds::Kind::PCK {
            crate::code::kinds::NestingKind::TopLevel
        } else if self.name.is_empty() {
            crate::code::kinds::NestingKind::Anonymous
        } else if ctx.sym(owner).kind == crate::code::kinds::Kind::MTH {
            crate::code::kinds::NestingKind::Local
        } else {
            crate::code::kinds::NestingKind::Member
        }
    }

    /// Java: public RecordComponent getRecordComponent(VarSymbol field)
    /// (Symbol.java:1534-1541).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - field: the record component field.
    ///
    /// # Returns
    ///   - the matching record component, if any.
    ///
    /// # Panics
    ///   - none.
    pub fn get_record_component(
        &self,
        ctx: &CompilerContext,
        field: SymbolId,
    ) -> Option<SymbolId> {
        let crate::code::symbol::SymbolKind::ClassSymbol {
            record_components, ..
        } = &self.kind_data
        else {
            return None;
        };
        let field_name = ctx.sym(field).name.clone();
        let mut current = record_components.clone();
        while current.non_empty() {
            let rc = *current.head();
            current = current.tail();
            if ctx.sym(rc).name == field_name {
                return Some(rc);
            }
        }
        None
    }

    /// Java: public RecordComponent findRecordComponentToRemove(JCVariableDecl var)
    /// (Symbol.java:1506-1520).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - var: the record component field declaration tree.
    ///
    /// # Returns
    ///   - the record component to replace, if any.
    ///
    /// # Panics
    ///   - none.
    pub fn find_record_component_to_remove(
        &self,
        ctx: &CompilerContext,
        var: TreeId,
    ) -> Option<SymbolId> {
        let crate::code::symbol::SymbolKind::ClassSymbol {
            record_components, ..
        } = &self.kind_data
        else {
            return None;
        };
        let (name, pos) = match &ctx.tree(var).kind {
            crate::tree::jc_tree::TreeKind::VariableDecl { name, .. } => {
                (name.clone(), ctx.tree(var).pos)
            }
            _ => return None,
        };
        let mut to_remove: Option<SymbolId> = None;
        let mut current = record_components.clone();
        while current.non_empty() {
            let rc = *current.head();
            current = current.tail();
            let (rc_pos, _rc_varargs) = match &ctx.sym(rc).kind_data {
                crate::code::symbol::SymbolKind::RecordComponent { pos, is_varargs, .. } => {
                    (*pos, *is_varargs)
                }
                _ => (crate::util::position::NOPOS, false),
            };
            if ctx.sym(rc).name == name && pos == rc_pos {
                to_remove = Some(rc);
            }
        }
        to_remove
    }
}
