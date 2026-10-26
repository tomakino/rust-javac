#![allow(clippy::result_large_err)]
#![allow(clippy::type_complexity)]

// Origin: com/sun/tools/javac/code/Types.java @ 982d53c (jdk-17.0.19-ga)
// Gap registration supplement (method-level audit):
// Dependency/phase: P3 semantic services (Enter/Attr/Resolve/Check or ClassReader; see docs/PLAN.md).
// - NOT PORTED: needsStripping (Types.java:276)
// - NOT PORTED: needsStripping (Types.java:295)
// - NOT PORTED: needsStripping (Types.java:404)
// - NOT PORTED: visitCapturedType (Types.java:478)
// - NOT PORTED: asSub (Types.java:539)
// - NOT PORTED: setMessage (Types.java:649)
// - NOT PORTED: getDiagnostic (Types.java:654)
// - NOT PORTED: getSymbol (Types.java:674)
// - NOT PORTED: findDescriptorInternal (Types.java:732)
// - NOT PORTED: mergeDescriptors (Types.java:793)
// - NOT PORTED: failure (Types.java:804)
// - NOT PORTED: failure (Types.java:808)
// - NOT PORTED: findDescriptorSymbol (Types.java:819)
// - NOT PORTED: findDescriptorType (Types.java:827)
// - NOT PORTED: isFunctionalInterface (Types.java:834)
// - NOT PORTED: isFunctionalInterface (Types.java:843)
// - NOT PORTED: removeWildcards (Types.java:852)
// - NOT PORTED: makeFunctionalInterfaceClass (Types.java:900)
// - NOT PORTED: functionalInterfaceBridges (Types.java:926)
// - NOT PORTED: pendingBridges (Types.java:959)
// - NOT PORTED: checkUnsafeVarargsConversion (Types.java:1044)
// - NOT PORTED: visitUndetVar (Types.java:1214)
// - NOT PORTED: isSubtypes (Types.java:1250)
// - NOT PORTED: isSubtypesUnchecked (Types.java:1266)
// - NOT PORTED: isSignaturePolymorphic (Types.java:1327)
// - NOT PORTED: visitPackageType (Types.java:1447)
// - NOT PORTED: visitUndetVar (Types.java:1463)
// - NOT PORTED: visitUndetVar (Types.java:1593)
// - NOT PORTED: disjointTypes (Types.java:1888)
// - NOT PORTED: disjointType (Types.java:1906)
// - NOT PORTED: isCastableRecursive (Types.java:1922)
// - NOT PORTED: notSoftSubtypeRecursive (Types.java:1935)
// - NOT PORTED: cvarLowerBounds (Types.java:1978)
// - NOT PORTED: visitCapturedType (Types.java:1983)
// - NOT PORTED: relaxBound (Types.java:2012)
// - NOT PORTED: isArray (Types.java:2059)
// - NOT PORTED: elemtypeOrType (Types.java:2083)
// - NOT PORTED: dimensions (Types.java:2108)
// - NOT PORTED: makeArrayType (Types.java:2123)
// - NOT PORTED: isAssignable (Types.java:2338)
// - NOT PORTED: isAssignable (Types.java:2348)
// - NOT PORTED: isDirectSuperInterface (Types.java:2659)
// - NOT PORTED: isDerivedRaw (Types.java:2670)
// - NOT PORTED: isDerivedRawInternal (Types.java:2679)
// - NOT PORTED: isDerivedRaw (Types.java:2688)
// - NOT PORTED: setBounds (Types.java:2706)
// - NOT PORTED: setBounds (Types.java:2719)
// - NOT PORTED: getBounds (Types.java:2731)
// - NOT PORTED: isSubSignature (Types.java:2795)
// - NOT PORTED: isSubSignature (Types.java:2799)
// - NOT PORTED: overrideEquivalent (Types.java:2816)
// - NOT PORTED: overridesObjectMethod (Types.java:2821)
// - NOT PORTED: mergeAbstracts (Types.java:2874)
// - NOT PORTED: baseSymbol (Types.java:2921)
// - NOT PORTED: implementationInternal (Types.java:2978)
// - NOT PORTED: implementation (Types.java:3003)
// - NOT PORTED: getSymbols (Types.java:3029)
// - NOT PORTED: getSymbolsByName (Types.java:3034)
// - NOT PORTED: getMark (Types.java:3039)
// - NOT PORTED: membersClosure (Types.java:3091)
// - NOT PORTED: firstUnimplementedAbstract (Types.java:3101)
// - NOT PORTED: firstUnimplementedAbstractImpl (Types.java:3110)
// - NOT PORTED: interfaceCandidates (Types.java:3187)
// - NOT PORTED: prune (Types.java:3206)
// - NOT PORTED: needsStripping (Types.java:3392)
// - NOT PORTED: needsStripping (Types.java:3399)
// - NOT PORTED: substBounds (Types.java:3407)
// - NOT PORTED: substBound (Types.java:3448)
// - NOT PORTED: newInstances (Types.java:3487)
// - NOT PORTED: createMethodTypeWithParameters (Types.java:3503)
// - NOT PORTED: createMethodTypeWithThrown (Types.java:3519)
// - NOT PORTED: createMethodTypeWithReturn (Types.java:3535)
// - NOT PORTED: baseType (Types.java:3546)
// - NOT PORTED: baseType (Types.java:3554)
// - NOT PORTED: createErrorType (Types.java:3562)
// - NOT PORTED: createErrorType (Types.java:3566)
// - NOT PORTED: createErrorType (Types.java:3570)
// - NOT PORTED: typaramsString (Types.java:3656)
// - NOT PORTED: appendTyparamString (Types.java:3668)
// - NOT PORTED: closure (Types.java:3710)
// - NOT PORTED: closureCollector (Types.java:3735)
// - NOT PORTED: merge (Types.java:3757)
// - NOT PORTED: closure (Types.java:3762)
// - NOT PORTED: merge (Types.java:3856)
// - NOT PORTED: compoundMin (Types.java:3900)
// - NOT PORTED: closureMin (Types.java:3915)
// - NOT PORTED: glbFlattened (Types.java:4124)
// - NOT PORTED: visitUndetVar (Types.java:4229)
// - NOT PORTED: resultSubtype (Types.java:4249)
// - NOT PORTED: returnTypeSubstitutable (Types.java:4261)
// - NOT PORTED: returnTypeSubstitutable (Types.java:4270)
// - NOT PORTED: covariantReturnType (Types.java:4292)
// - NOT PORTED: boxedTypeOrType (Types.java:4312)
// - NOT PORTED: unboxedTypeOrType (Types.java:4336)
// - NOT PORTED: sideCast (Types.java:4479)
// - NOT PORTED: sideCastFinal (Types.java:4509)
// - NOT PORTED: giveWarning (Types.java:4535)
// - NOT PORTED: superClosure (Types.java:4550)
// - NOT PORTED: adaptRecursive (Types.java:4664)
// - NOT PORTED: adaptRecursive (Types.java:4675)
// - NOT PORTED: rewriteQuantifiers (Types.java:4720)
// - NOT PORTED: visitCapturedType (Types.java:4758)
// - NOT PORTED: rewriteAsWildcardType (Types.java:4784)
// - NOT PORTED: makeExtendsWildcard (Types.java:4822)
// - NOT PORTED: makeSuperWildcard (Types.java:4844)
// - NOT PORTED: visitPackageType (Types.java:4905)
// - NOT PORTED: visitCapturedType (Types.java:4908)
// - NOT PORTED: visitUndetVar (Types.java:4910)
// - NOT PORTED: visitClassSymbol (Types.java:4928)
// - NOT PORTED: visitMethodSymbol (Types.java:4929)
// - NOT PORTED: visitOperatorSymbol (Types.java:4930)
// - NOT PORTED: visitPackageSymbol (Types.java:4931)
// - NOT PORTED: visitTypeSymbol (Types.java:4932)
// - NOT PORTED: visitVarSymbol (Types.java:4933)
// - NOT PORTED: visitCapturedType (Types.java:4951)
// - NOT PORTED: visitUndetVar (Types.java:4959)
// - NOT PORTED: apply (Types.java:5005)
// - NOT PORTED: visitCapturedType (Types.java:5012)
// - NOT PORTED: getRetention (Types.java:5021)
// - NOT PORTED: getRetention (Types.java:5025)
// - NOT PORTED: append (Types.java:5062)
// - NOT PORTED: append (Types.java:5063)
// - NOT PORTED: append (Types.java:5064)
// - NOT PORTED: classReference (Types.java:5065)
// - NOT PORTED: reportIllegalSignature (Types.java:5071)
// - NOT PORTED: assembleSig (Types.java:5078)
// - NOT PORTED: hasTypeVar (Types.java:5170)
// - NOT PORTED: assembleClassSig (Types.java:5180)
// - NOT PORTED: assembleParamsSig (Types.java:5207)
// - NOT PORTED: assembleSig (Types.java:5224)
// - NOT PORTED: constantType (Types.java:5231)
// - NOT PORTED: newRound (Types.java:5257)
// End of Gap registration supplement.
// Gap registration (Types service, method-level audit).
// Dependency/phase: P3 semantic services (Enter/Attr/Resolve/Check and the
// ClassReader/complete() symbol graph); the file currently ports the P1/P2
// subset needed by the parser/tree layers.
// Lead cross-check of reference code/Types.java against the local fn names
// reports 164 not-yet-ported semantic methods. Representative groups (the
// authoritative per-method list is maintained by the Lead in STATUS):
// - TypeRelation visitors: visitClassType/visitWildcardType/visitTypeVar/
// visitArrayType/visitErrorType dispatch for isSubtype/isSameType families;
// - type projection helpers: mapTypeArgument/TypeArgumentProjection/upward/
// downward/complement/captures/makeWildcard (TypeProjection);
// - lub/glb/capture fresh-type machinery and adapt/subst recursion paths;
// - memberType/asSuper recursion over completed symbol graphs (needs the
// ClassReader/Enter work).
// No substitutes are implemented here; these are registered for the P3.
// End of Gap registration.

//! Utility routines for the Type interface.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES (ADR 0001, arena + IDs): this ports ONLY the Types
//! subset needed to unlock TreeMaker/class-fields (see the task):
//! hashCode, isSameType, erasure, boxedClass, and the helpers they need.
//! The Java Types instance state (Symtab, Names, Source, Check, Enter,
//! JavacMessages, JCDiagnostic.Factory, Warner) is NOT stored in the Rust Types
//! (which is a stateless service): the methods that need it take the ported
//! Symtab explicitly (registered section 9; no Rc/RefCell is introduced). The
//! remaining Types methods are NOT PORTED (TODO Types):
//!   - isSubtype / isCastable / containsType / asSuper / memberType / subst /
//!     capture / supertype / interfaces / lub / glb / isReifiable / rank / ...;
//!   - containsTypeEquivalent's containsType(t, s) && containsType(s, t) branch
//!     is NOT PORTED (containsType needs isSubtype): the ported
//!     contains_type_equivalent* methods therefore panic with an explicit
//!     unimplemented message when isSameType is insufficient (no structural
//!     approximation is used);
//!   - isSameType ForAll branch: hasSameBounds + subst are NOT PORTED (panic);
//!   - isSameType compound (intersection/union) branch: supertype/interfaces
//!     are NOT PORTED (panic);
//!   - UndetVar: the TypeKindData variant is not ported, the branch is
//!     unreachable;
//!   - TypeMetadata (combineMetadata/cloneWithMetadata/typeNoMetadata) is not
//!     ported: equalsIgnoreMetadata == equals and combineMetadata(s, t) == s.
//!
//! PORTING_RULES (upstream helpers that live in Type.java but are not ported in
//! type_.rs yet; reproduced here as private helpers and to be moved into
//! type_.rs with the Type, registered): getEnclosingType,
//! getTypeArguments, getParameterTypes, getReturnType and the ArrayType
//! Type.equals override (equalsIgnoreMetadata).
//!
//! PORTING_RULES (hashCode): the upstream Name.hashCode() is
//! Object.identityHashCode (Name.java has no hashCode override), which is not
//! reproducible across processes; the port uses the javac name-table hash
//! (NameTable.hashValue over the name bytes) as a stable equivalent (any stable
//! value consistent with identity equality is equivalent for the HashMap usage;
//! registered). Likewise BoundKind.hashCode()/TypeVar identity hash are
//! enum/identity hashes upstream; the port uses BoundKind.ordinal and the TypeId
//! arena index (same instance gives the same value, different instances give
//! different values; registered).

use crate::code::bound_kind::BoundKind;
use crate::code::symtab::Symtab;
use crate::code::type_::{Type, TypeKindData};
use crate::code::type_tag::TypeTag;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TypeId};
use crate::util::list::List;
use crate::util::name::{Name, NameTableHandle};

/// The Java TypeTag ordinal (Java: type.getTag().ordinal()).
fn tag_ordinal(tag: TypeTag) -> i32 {
    // The Rust TypeTag enum declaration order is the Java TypeTag declaration
    // order (BYTE, CHAR, SHORT, LONG, FLOAT, INT, DOUBLE, BOOLEAN, VOID, CLASS,
    // ARRAY, METHOD, PACKAGE, MODULE, TYPEVAR, WILDCARD, FORALL, DEFERRED, BOT,
    // NONE, ERROR, UNKNOWN, UNDETVAR, UNINITIALIZED_THIS, UNINITIALIZED_OBJECT).
    tag as usize as i32
}

/// Java: Name.hashCode() (stable name-table hash; see the module doc).
fn name_hash(name: &Name) -> i32 {
    let bytes = name.get_byte_array();
    NameTableHandle::hash_value(bytes, 0, bytes.len() as i32)
}

/// Java: Symbol.flatName() for a type symbol.
fn flat_name_of(ctx: &CompilerContext, symbol: SymbolId) -> Name {
    match &ctx.sym(symbol).kind_data {
        crate::code::symbol::SymbolKind::ClassSymbol { flatname, .. } => flatname.clone(),
        _ => ctx.sym(symbol).name.clone(),
    }
}

/// Utility routines for the Type interface.
///
/// Java: public class Types
///
/// PORTING_RULES: the Java instance fields (syms/...) are not stored; the
/// ported methods take them as parameters. The Names service is stored (the
/// captured-name creation of capture() needs it, Types.java:94/101/122).
#[derive(Default, Clone)]
pub struct Types {
    /// Java: final Names names (None until the service construction wires it).
    names: Option<std::rc::Rc<crate::util::names::Names>>,
    /// Java: final boolean mapCapturesToBounds (Types.java:97; set from
    /// Feature.MAP_CAPTURES_TO_BOUNDS for the configured source level).
    map_captures_to_bounds: bool,
    /// Java: DescriptorCache._map (Types.java:665) / Entry cache.
    descriptor_cache: std::cell::RefCell<std::collections::HashMap<SymbolId, DescriptorEntry>>,
    /// Java: ImplementationCache._map (Types.java:2934).
    implementation_cache: std::cell::RefCell<
        std::collections::HashMap<
            SymbolId,
            std::collections::HashMap<SymbolId, ImplementationEntry>,
        >,
    >,
    /// Java: MembersClosureCache._map (Types.java:3012).
    members_closure_cache: std::cell::RefCell<
        std::collections::HashMap<SymbolId, crate::ids::ScopeId>,
    >,
    /// Java: MembersClosureCache.seenTypes (Types.java:3014).
    members_closure_seen: std::cell::RefCell<std::collections::HashSet<SymbolId>>,
    /// Java: CandidatesCache.cache (Types.java:3151); the upstream Entry
    /// equality uses isSameType(site) + symbol identity, the port keys by the
    /// nominal TypeId (a cache-miss-only difference, registered Porting rules).
    candidates_cache: std::cell::RefCell<
        std::collections::HashMap<(TypeId, SymbolId), Vec<SymbolId>>,
    >,
}

/// Java: ImplementationCache.Entry (Types.java:2936-2958).
#[derive(Clone)]
pub struct ImplementationEntry {
    pub cached_impl: Option<SymbolId>,
    pub check_result: bool,
    pub prev_mark: i32,
}

/// Java: DescriptorCache.Entry (Types.java:703-720).
#[derive(Clone)]
pub struct DescriptorEntry {
    pub cached_desc_res: crate::code::types::FunctionDescriptor,
    pub prev_mark: i32,
}

/// Java: Types.FunctionDescriptorLookupError (Types.java:640-658) — the port
/// carries the diagnostic.
#[derive(Clone, Debug, Default)]
pub struct FunctionDescriptorLookupError {
    pub diagnostic: Option<crate::util::jc_diagnostic::JCDiagnostic>,
}

impl FunctionDescriptorLookupError {
/// /// Java: FunctionDescriptorLookupError() (Types.java:4100-4120).
///
/// # Panics
///   - none.
///
/// Upstream: Types.java:4100-4120.
///
    pub fn new() -> FunctionDescriptorLookupError {
        FunctionDescriptorLookupError { diagnostic: None }
    }

    pub fn set_message(mut self, diag: crate::util::jc_diagnostic::JCDiagnostic) -> Self {
        self.diagnostic = Some(diag);
        self
    }

/// /// Java: FunctionDescriptorLookupError.getDiagnostic() (Types.java:4120-4140).
///
/// # Panics
///   - none.
///
/// Upstream: Types.java:4120-4140.
///
    pub fn get_diagnostic(&self) -> Option<&crate::util::jc_diagnostic::JCDiagnostic> {
        self.diagnostic.as_ref()
    }
}

/// Java: DescriptorCache.FunctionDescriptor (Types.java:667-700); the merged
/// descriptor (FunctionDescriptor.getType override) is expressed by the
/// optional merged_symbols/thrown override.
#[derive(Clone, Debug)]
pub struct FunctionDescriptor {
    pub desc_sym: SymbolId,
    /// Java: the anonymous FunctionDescriptor override in mergeDescriptors
    /// returns createMethodTypeWithThrown(memberType, bestSoFar.thrown); None
    /// means the plain getSymbol()::memberType path.
    pub merge: Option<FunctionDescriptorMerge>,
}

/// Java: DescriptorCache.mergeDescriptors' anonymous override payload
/// (Types.java:793-802).
#[derive(Clone, Debug)]
pub struct FunctionDescriptorMerge {
    pub best_so_far: SymbolId,
}

impl FunctionDescriptor {
/// /// Java: FunctionDescriptor(Symbol) (Types.java:3000-3020).
///
/// # Panics
///   - none.
///
/// Upstream: Types.java:3000-3020.
///
    pub fn new(desc_sym: SymbolId) -> FunctionDescriptor {
        FunctionDescriptor {
            desc_sym,
            merge: None,
        }
    }

    pub fn get_symbol(&self) -> SymbolId {
        self.desc_sym
    }
}

fn init_new_types_fields() -> (
    std::cell::RefCell<std::collections::HashMap<SymbolId, DescriptorEntry>>,
    std::cell::RefCell<
        std::collections::HashMap<
            SymbolId,
            std::collections::HashMap<SymbolId, ImplementationEntry>,
        >,
    >,
    std::cell::RefCell<std::collections::HashMap<SymbolId, crate::ids::ScopeId>>,
    std::cell::RefCell<std::collections::HashSet<SymbolId>>,
    std::cell::RefCell<std::collections::HashMap<(TypeId, SymbolId), Vec<SymbolId>>>,
) {
    (
        std::cell::RefCell::new(std::collections::HashMap::new()),
        std::cell::RefCell::new(std::collections::HashMap::new()),
        std::cell::RefCell::new(std::collections::HashMap::new()),
        std::cell::RefCell::new(std::collections::HashSet::new()),
        std::cell::RefCell::new(std::collections::HashMap::new()),
    )
}

impl Types {
    /// Get the Types instance for this context.
    ///
    /// PORTING_RULES: the Java Types instance is registered in the Context; the
    /// Rust Types keeps only the Names service (wired by the services that
    /// need capture(); see with_names).
    pub fn new() -> Types {
        let (
            descriptor_cache,
            implementation_cache,
            members_closure_cache,
            members_closure_seen,
            candidates_cache,
        ) = init_new_types_fields();
        Types {
            names: None,
            map_captures_to_bounds: false,
            descriptor_cache,
            implementation_cache,
            members_closure_cache,
            members_closure_seen,
            candidates_cache,
        }
    }

    /// Wires the Names service (the ported service construction; the upstream
    /// constructor reads Names.instance(context)).
    pub fn with_names(names: std::rc::Rc<crate::util::names::Names>) -> Types {
        let (
            descriptor_cache,
            implementation_cache,
            members_closure_cache,
            members_closure_seen,
            candidates_cache,
        ) = init_new_types_fields();
        Types {
            names: Some(names),
            map_captures_to_bounds: false,
            descriptor_cache,
            implementation_cache,
            members_closure_cache,
            members_closure_seen,
            candidates_cache,
        }
    }

    /// Wires the Names service and the source-dependent
    /// mapCapturesToBounds flag (the upstream Types constructor reads
    /// Feature.MAP_CAPTURES_TO_BOUNDS.allowedInSource(source)).
    pub fn with_names_and_source(
        names: std::rc::Rc<crate::util::names::Names>,
        source: crate::code::source::Source,
    ) -> Types {
        let (
            descriptor_cache,
            implementation_cache,
            members_closure_cache,
            members_closure_seen,
            candidates_cache,
        ) = init_new_types_fields();
        Types {
            names: Some(names),
            map_captures_to_bounds:
                crate::code::source::Feature::MAP_CAPTURES_TO_BOUNDS.allowed_in_source(source),
            descriptor_cache,
            implementation_cache,
            members_closure_cache,
            members_closure_seen,
            candidates_cache,
        }
    }

    /// Java: final boolean mapCapturesToBounds
    pub fn map_captures_to_bounds(&self) -> bool {
        self.map_captures_to_bounds
    }

    /// The captured type variable name (Types.java:122).
    fn captured_name(&self) -> crate::util::name::Name {
        let Some(names) = &self.names else {
            unimplemented!(
                "TODO(Types batch): Types names service not wired (Types::with_names)"
            );
        };
        names.from_utf(b"<captured wildcard>")
    }

    /// Compute a hash code on a type.
    ///
    /// Java: public int hashCode(Type t) { return hashCode(t, false); }
    pub fn hash_code(&self, ctx: &mut CompilerContext, t: TypeId) -> i32 {
        self.hash_code_impl(ctx, t, false)
    }

    /// Java: public int hashCode(Type t, boolean strict)
    pub fn hash_code_with_strict(&self, ctx: &mut CompilerContext, t: TypeId, strict: bool) -> i32 {
        self.hash_code_impl(ctx, t, strict)
    }

    // The upstream hashCode(t, strict) has two visitors whose only difference is
    // the visitTypeVar override, and both return identityHashCode (Types.java
    // hashCodeStrictVisitor simply overrides visitTypeVar with
    // System.identityHashCode, exactly like the base visitor), so the strict flag
    // currently changes nothing for the ported branches (the port keeps the
    // parameter for the 1:1 signature, registered).
    #[allow(clippy::only_used_in_recursion)]
    fn hash_code_impl(&self, ctx: &mut CompilerContext, t: TypeId, strict: bool) -> i32 {
        // Java: HashCodeVisitor.visit(t): the visitor dispatches on the concrete
        // type class; ErrorType overrides visitErrorType, TypeVar/CapturedType
        // use identityHashCode (strict mode also uses identityHashCode for
        // TypeVar), ForAll/CapturedType are handled by SimpleVisitor.
        match &ctx.typ(t).kind {
            TypeKindData::ErrorType { .. } => 0,
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => self.hash_class_type(ctx, t),
            TypeKindData::WildcardType { kind, type_, .. } => {
                let mut result = bound_kind_ordinal(*kind);
                if let Some(type_) = type_ {
                    result = result
                        .wrapping_mul(127)
                        .wrapping_add(self.hash_code_impl(ctx, *type_, strict));
                }
                result
            }
            TypeKindData::ArrayType { elemtype, .. } => {
                let elemtype = elemtype.expect("array element type");
                self.hash_code_impl(ctx, elemtype, strict).wrapping_add(12)
            }
            TypeKindData::MethodType {
                argtypes, restype, ..
            } => {
                let argtypes = argtypes.clone();
                let restype = restype.expect("method return type");
                let mut h = tag_ordinal(TypeTag::METHOD);
                // Java: for (List<Type> thisargs = t.argtypes;
                // thisargs.tail != null; thisargs = thisargs.tail)
                // - every argument but the last.
                let last = argtypes.length() - 1;
                for index in 0..last {
                    h = (h << 5).wrapping_add(self.hash_code_impl(ctx, argtypes.get(index), strict));
                }
                (h << 5).wrapping_add(self.hash_code_impl(ctx, restype, strict))
            }
            TypeKindData::TypeVar { .. } | TypeKindData::CapturedType { .. } => {
                // Java: System.identityHashCode(t) (strict and non-strict).
                type_identity_hash(t)
            }
            TypeKindData::UndetVar { .. } => {
                // Java: HashCodeVisitor.visitUndetVar -> System.identityHashCode(t)
                type_identity_hash(t)
            }
            TypeKindData::ForAll { qtype, .. } => {
                // Java: SimpleVisitor.visitForAll -> visit(qtype).
                let qtype = qtype.expect("for-all qtype");
                self.hash_code_impl(ctx, qtype, strict)
            }
            _ => tag_ordinal(ctx.typ(t).get_tag()),
        }
    }

    /// Java: HashCodeVisitor.visitClassType
    fn hash_class_type(&self, ctx: &mut CompilerContext, t: TypeId) -> i32 {
        let enclosing = get_enclosing_type(ctx, t).expect("class type enclosing type");
        let mut result = self.hash_code_impl(ctx, enclosing, false);
        result = result.wrapping_mul(127);
        let tsym = ctx.typ(t).tsym.expect("class type symbol");
        result = result.wrapping_add(name_hash(&flat_name_of(ctx, tsym)));
        for argument in get_type_arguments(ctx, t).iter() {
            result = result.wrapping_mul(127);
            result = result.wrapping_add(self.hash_code_impl(ctx, argument, false));
        }
        result
    }

    /// Is t the same type as s?
    ///
    /// Java: public boolean isSameType(Type t, Type s)
    pub fn is_same_type(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId, s: TypeId) -> bool {
        match &ctx.typ(t).kind {
            TypeKindData::JCPrimitiveType
            | TypeKindData::JCVoidType
            | TypeKindData::BottomType
            | TypeKindData::JCNoType
            | TypeKindData::TypeVar { .. }
            | TypeKindData::CapturedType { .. }
            | TypeKindData::UnknownType => self.is_same_type_visit_type(ctx, syms, t, s),
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
                self.is_same_type_visit_class(ctx, syms, t, s)
            }
            // Java: isSameTypeVisitor.visitErrorType -> true
            TypeKindData::ErrorType { .. } => true,
            TypeKindData::WildcardType { .. } => {
                self.is_same_type_visit_wildcard(ctx, syms, t, s)
            }
            TypeKindData::ArrayType { .. } => self.is_same_type_visit_array(ctx, syms, t, s),
            TypeKindData::MethodType { .. } => self.is_same_type_visit_method(ctx, syms, t, s),
            TypeKindData::PackageType => t == s,
            TypeKindData::ModuleType => {
                // Java: DefaultTypeVisitor.visitModuleType -> visitType, whose
                // default branch throws AssertionError for the MODULE tag.
                panic!("AssertionError: isSameType {:?}", ctx.typ(t).get_tag());
            }
            TypeKindData::ForAll { .. } => self.is_same_type_visit_for_all(ctx, syms, t, s),
            TypeKindData::UndetVar { .. } => {
                self.is_same_type_visit_undet_var(ctx, syms, t, s)
            }
            TypeKindData::Deferred { .. } => {
                // Java: isSameTypeVisitor.visitType default branch throws
                // AssertionError for DEFERRED (deferred types are mapped
                // through DeferredTypeMap before a relation is computed).
                panic!("AssertionError: isSameType DEFERRED")
            }
            TypeKindData::PartiallyInferredMethodType { .. } => {
                // Java: PartiallyInferredMethodType extends MethodType
                // (tag METHOD); the isSameType visitor dispatches to
                // visitMethodType.
                self.is_same_type_visit_method(ctx, syms, t, s)
            }
        }
    }

    /// Java: isSameTypeVisitor.visitUndetVar (Types.java:1463-1480)
    fn is_same_type_visit_undet_var(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        if ctx.typ(s).has_tag(TypeTag::WILDCARD) {
            // FIXME, this might be leftovers from before capture conversion
            return false;
        }
        let qtype = self.undet_var_qtype(ctx, t);
        if t == s
            || qtype == s
            || ctx.typ(s).has_tag(TypeTag::ERROR)
            || ctx.typ(s).has_tag(TypeTag::UNKNOWN)
        {
            return true;
        }
        self.undet_var_add_bound(
            ctx,
            syms,
            t,
            crate::code::type_::InferenceBound::EQ,
            s,
            false,
        );
        true
    }

    /// Java: isSubtype.visitUndetVar (Types.java:1214-1227)
    fn is_subtype_visit_undet_var(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        let qtype = self.undet_var_qtype(ctx, t);
        if t == s
            || qtype == s
            || ctx.typ(s).has_tag(TypeTag::ERROR)
            || ctx.typ(s).has_tag(TypeTag::UNKNOWN)
        {
            return true;
        } else if ctx.typ(s).has_tag(TypeTag::BOT) {
            // if 's' is 'null' there's no instantiated type U for which
            // U <: s (but 'null' itself, which is not a valid type)
            return false;
        }
        self.undet_var_add_bound(
            ctx,
            syms,
            t,
            crate::code::type_::InferenceBound::UPPER,
            s,
            false,
        );
        true
    }

    /// Java: isSameTypeVisitor.visitType
    fn is_same_type_visit_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        if self.equals_ignore_metadata(ctx, t, s) {
            return true;
        }
        if ctx.typ(s).is_partial() {
            return self.is_same_type(ctx, syms, s, t);
        }
        match ctx.typ(t).get_tag() {
            TypeTag::BYTE
            | TypeTag::CHAR
            | TypeTag::SHORT
            | TypeTag::INT
            | TypeTag::LONG
            | TypeTag::FLOAT
            | TypeTag::DOUBLE
            | TypeTag::BOOLEAN
            | TypeTag::VOID
            | TypeTag::BOT
            | TypeTag::NONE => ctx.typ(t).has_tag(ctx.typ(s).get_tag()),
            TypeTag::TYPEVAR => {
                if ctx.typ(s).has_tag(TypeTag::TYPEVAR) {
                    // type-substitution does not preserve type-var types; check
                    // that type var symbols and bounds are indeed the same.
                    t == s
                } else {
                    // special case for s == ? super X, where upper(s) = u; check
                    // that u == t, where u has been set by Type.withTypeVar.
                    ctx.typ(s).is_super_bound()
                        && !ctx.typ(s).is_extends_bound()
                        && {
                            let upper = self.wild_upper_bound(ctx, syms, s);
                            self.is_same_type(ctx, syms, t, upper)
                        }
                }
            }
            tag => panic!("AssertionError: isSameType {tag:?}"),
        }
    }

    /// Java: isSameTypeVisitor.visitWildcardType
    fn is_same_type_visit_wildcard(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        if !ctx.typ(s).has_tag(TypeTag::WILDCARD) {
            return false;
        }
        let (t_kind, t_type) = match &ctx.typ(t).kind {
            TypeKindData::WildcardType { kind, type_, .. } => (*kind, *type_),
            _ => unreachable!(),
        };
        let (s_kind, s_type) = match &ctx.typ(s).kind {
            TypeKindData::WildcardType { kind, type_, .. } => (*kind, *type_),
            _ => unreachable!(),
        };
        (t_kind == s_kind || (ctx.typ(t).is_extends_bound() && ctx.typ(s).is_extends_bound()))
            && self.is_same_type(
                ctx,
                syms,
                t_type.expect("wildcard bound type"),
                s_type.expect("wildcard bound type"),
            )
    }

    /// Java: isSameTypeVisitor.visitClassType
    fn is_same_type_visit_class(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        if t == s {
            return true;
        }
        if ctx.typ(s).is_partial() {
            return self.is_same_type(ctx, syms, s, t);
        }
        if ctx.typ(s).is_super_bound() && !ctx.typ(s).is_extends_bound() {
            let upper = self.wild_upper_bound(ctx, syms, s);
            let lower = self.wild_lower_bound(ctx, syms, s);
            return self.is_same_type(ctx, syms, t, upper)
                && self.is_same_type(ctx, syms, t, lower);
        }
        if self.is_compound(ctx, t) && self.is_compound(ctx, s) {
            let sup_t = self.supertype(ctx, syms, t);
            let sup_s = self.supertype(ctx, syms, s);
            if !self.is_same_type(ctx, syms, sup_t, sup_s) {
                return false;
            }
            let mut t_map: Vec<(SymbolId, TypeId)> = Vec::new();
            for ti in self.interfaces(ctx, syms, t).iter() {
                let tsym = ctx.typ(ti).tsym.expect("interface tsym");
                if t_map.iter().any(|(symbol, _)| *symbol == tsym) {
                    panic!("AssertionError: Malformed intersection");
                }
                t_map.push((tsym, ti));
            }
            for si in self.interfaces(ctx, syms, s).iter() {
                let s_tsym = ctx.typ(si).tsym.expect("interface tsym");
                let Some(position) = t_map.iter().position(|(symbol, _)| *symbol == s_tsym) else {
                    return false;
                };
                let ti = t_map.remove(position).1;
                if !self.is_same_type(ctx, syms, ti, si) {
                    return false;
                }
            }
            return t_map.is_empty();
        }
        ctx.typ(t).tsym == ctx.typ(s).tsym
            && self.is_same_type_enclosing(
                ctx,
                syms,
                get_enclosing_type(ctx, t),
                get_enclosing_type(ctx, s),
            )
            && self.contains_type_equivalent(
                ctx,
                syms,
                &get_type_arguments(ctx, t),
                &get_type_arguments(ctx, s),
            )
    }

    /// Java: visit(t.getEnclosingType(), s.getEnclosingType()) inside
    /// visitClassType (the upstream code only calls this for class types whose
    /// enclosing type is a type, never null).
    fn is_same_type_enclosing(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: Option<TypeId>,
        s: Option<TypeId>,
    ) -> bool {
        match (t, s) {
            (Some(t), Some(s)) => self.is_same_type(ctx, syms, t, s),
            // NOT PORTED: a null enclosing type is an upstream assertion
            // violation (visit(null,...)); do not invent a value.
            _ => panic!("unimplemented: null enclosing type in isSameType"),
        }
    }

    /// Java: isSameTypeVisitor.visitArrayType
    fn is_same_type_visit_array(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        if t == s {
            return true;
        }
        if ctx.typ(s).is_partial() {
            return self.is_same_type(ctx, syms, s, t);
        }
        if !ctx.typ(s).has_tag(TypeTag::ARRAY) {
            return false;
        }
        let elemtype = match &ctx.typ(t).kind {
            TypeKindData::ArrayType { elemtype, .. } => elemtype.expect("array element type"),
            _ => unreachable!(),
        };
        let elemtype_s = match &ctx.typ(s).kind {
            TypeKindData::ArrayType { elemtype, .. } => elemtype.expect("array element type"),
            _ => unreachable!(),
        };
        self.contains_type_equivalent_single(ctx, syms, elemtype, elemtype_s)
    }

    /// Java: isSameTypeVisitor.visitMethodType (thrown exceptions are ignored).
    fn is_same_type_visit_method(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        self.has_same_args(ctx, syms, t, s)
            && self.is_same_type(
                ctx,
                syms,
                get_return_type(ctx, t),
                get_return_type(ctx, s),
            )
    }

    /// Java: isSameTypeVisitor.visitForAll
    fn is_same_type_visit_for_all(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        if !ctx.typ(s).has_tag(TypeTag::FORALL) {
            return false;
        }
        let (t_qtype, t_tvars) = match &ctx.typ(t).kind {
            TypeKindData::ForAll { qtype, tvars } => {
                (qtype.expect("for-all qtype"), tvars.clone())
            }
            _ => unreachable!(),
        };
        let (s_qtype, s_tvars) = match &ctx.typ(s).kind {
            TypeKindData::ForAll { qtype, tvars } => {
                (qtype.expect("for-all qtype"), tvars.clone())
            }
            _ => unreachable!(),
        };
        self.has_same_bounds(ctx, syms, &t_tvars, &s_tvars) && {
            let substituted = self.subst_type(ctx, syms, s_qtype, &s_tvars, &t_tvars);
            self.is_same_type(ctx, syms, t_qtype, substituted)
        }
    }

    /// Java: public boolean containsTypeEquivalent(List<Type> ts, List<Type> ss)
    pub fn contains_type_equivalent(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts: &List<TypeId>,
        ss: &List<TypeId>,
    ) -> bool {
        let mut ts = ts.clone();
        let mut ss = ss.clone();
        while ts.non_empty()
            && ss.non_empty()
            && self.contains_type_equivalent_single(ctx, syms, *ts.head(), *ss.head())
        {
            ts = ts.tail();
            ss = ss.tail();
        }
        ts.is_empty() && ss.is_empty()
    }

    /// Java: private boolean containsTypeEquivalent(Type t, Type s)
    fn contains_type_equivalent_single(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        if self.is_same_type(ctx, syms, t, s) {
            // shortcut
            return true;
        }
        // Java: containsType(t, s) && containsType(s, t)
        self.contains_type(ctx, syms, t, s) && self.contains_type(ctx, syms, s, t)
    }

    /// Java: public boolean hasSameArgs(Type t, Type s)
    pub fn has_same_args(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId, s: TypeId) -> bool {
        // Java: hasSameArgs(t, s, true) -> HasSameArgs.visit(t, s).
        match &ctx.typ(t).kind {
            TypeKindData::MethodType { .. } => {
                ctx.typ(s).has_tag(TypeTag::METHOD)
                    && self.contains_type_equivalent(
                        ctx,
                        syms,
                        &get_parameter_types(ctx, t),
                        &get_parameter_types(ctx, s),
                    )
            }
            TypeKindData::ErrorType { .. } => false,
            TypeKindData::ForAll { .. } => {
                // Java: HasSameArgs.visitForAll (Types.java:2830-2840):
                // non-strict allows comparing a ForAll with its method type.
                self.has_same_args_strict(ctx, syms, t, s, true)
            }
            _ => panic!("AssertionError: hasSameArgs {:?}", ctx.typ(t).get_tag()),
        }
    }

    /// Java: public Type erasure(Type t) { return eraseNotNeeded(t) ? t :
    /// erasure(t, false); }
    pub fn erasure(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
        if self.erase_not_needed(ctx, syms, t) {
            t
        } else {
            self.erasure_recurse(ctx, syms, t, false)
        }
    }

    /// Java: private boolean eraseNotNeeded(Type t)
    fn erase_not_needed(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> bool {
        ctx.typ(t).is_primitive() || ctx.typ(syms.string_type).tsym == ctx.typ(t).tsym
    }

    /// Java: private Type erasure(Type t, boolean recurse)
    fn erasure_recurse(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        recurse: bool,
    ) -> TypeId {
        if ctx.typ(t).is_primitive() {
            // fast special case
            return t;
        }
        self.erasure_visit(ctx, syms, t, recurse)
    }

    /// Java: the anonymous StructuralTypeMapping Boolean instance "erasure"
    /// (visitType/visitWildcardType/visitClassType/visitTypeVar; the default
    /// MapVisitor behavior is the identity mapping).
    fn erasure_visit(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        recurse: bool,
    ) -> TypeId {
        match &ctx.typ(t).kind {
            TypeKindData::WildcardType { .. } => {
                let upper = self.wild_upper_bound(ctx, syms, t);
                self.erasure_recurse(ctx, syms, upper, recurse)
                // combineMetadata(erased, t) == erased (no metadata support).
            }
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. }
            | TypeKindData::ErrorType { .. } => {
                let tsym = ctx.typ(t).tsym.expect("class type symbol");
                let erased = self.symbol_erasure(ctx, syms, tsym);
                if recurse {
                    // Java: new ErasedClassType(erased.getEnclosingType(),
                    // erased.tsym, metadata).
                    let outer = get_enclosing_type(ctx, erased);
                    let erased_tsym = ctx.typ(erased).tsym;
                    ctx.alloc_type(Type::new(
                        TypeTag::CLASS,
                        erased_tsym,
                        TypeKindData::ErasedClassType {
                            outer_field: outer,
                            typarams_field: Some(List::nil()),
                            allparams_field: None,
                            supertype_field: None,
                            interfaces_field: None,
                            all_interfaces_field: None,
                        },
                    ))
                } else {
                    // combineMetadata(erased, t) == erased (no metadata).
                    erased
                }
            }
            TypeKindData::TypeVar { .. } | TypeKindData::CapturedType { .. } => {
                let upper = ctx
                    .typ(t)
                    .get_upper_bound()
                    .expect("type variable upper bound");
                self.erasure_recurse(ctx, syms, upper, recurse)
            }
            TypeKindData::ForAll { qtype, .. } => {
                // Java: SimpleVisitor.visitForAll -> visit(t.qtype).
                let qtype = qtype.expect("for-all qtype");
                self.erasure_recurse(ctx, syms, qtype, recurse)
            }
            _ => {
                // Java: visitType - not primitive, "other cases already handled"
                // -> combineMetadata(t, t) == t.
                t
            }
        }
    }

    /// Java: Symbol.TypeSymbol.erasure(Types) / Symbol.ClassSymbol.erasure(Types)
    fn symbol_erasure(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        symbol: SymbolId,
    ) -> TypeId {
        if let Some(erasure_field) = ctx.sym(symbol).erasure_field {
            return erasure_field;
        }
        let ty = ctx.sym(symbol).ty.expect("symbol type");
        let erased = if matches!(
            ctx.sym(symbol).kind_data,
            crate::code::symbol::SymbolKind::ClassSymbol { .. }
        ) {
            // Java: Symbol.ClassSymbol.erasure(Types):
            // erasure_field = new ClassType(types.erasure(type.getEnclosingType()),
            // List.nil(), this, type.getMetadata());
            let enclosing = get_enclosing_type(ctx, ty).expect("class type enclosing type");
            let erased_enclosing = self.erasure_recurse(ctx, syms, enclosing, false);
            ctx.alloc_type(Type::new(
                TypeTag::CLASS,
                Some(symbol),
                TypeKindData::ClassType {
                    outer_field: Some(erased_enclosing),
                    typarams_field: Some(List::nil()),
                    allparams_field: None,
                    supertype_field: None,
                    interfaces_field: None,
                    all_interfaces_field: None,
                },
            ))
        } else {
            // Java: Symbol.TypeSymbol.erasure(Types):
            // erasure_field = types.erasure(type);
            self.erasure_recurse(ctx, syms, ty, false)
        };
        ctx.sym_mut(symbol).erasure_field = Some(erased);
        erased
    }

    /// Return the class that boxes the given primitive.
    ///
    /// Java: public ClassSymbol boxedClass(Type t)
    pub fn boxed_class(&self, ctx: &mut CompilerContext, syms: &mut Symtab, t: TypeId) -> SymbolId {
        let tag = ctx.typ(t).get_tag();
        let name = syms.boxed_name[tag_ordinal(tag) as usize]
            .clone()
            .expect("boxed class name");
        let java_base = syms.java_base;
        syms.enter_class(ctx, java_base, name, None)
    }

    /// Get a wildcard's upper bound, returning non-wildcards unchanged.
    ///
    /// Java: public Type wildUpperBound(Type t)
    pub fn wild_upper_bound(&self, ctx: &CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
        if ctx.typ(t).has_tag(TypeTag::WILDCARD) {
            if ctx.typ(t).is_super_bound() {
                let bound = match &ctx.typ(t).kind {
                    TypeKindData::WildcardType { bound, .. } => *bound,
                    _ => unreachable!(),
                };
                match bound {
                    None => syms.object_type,
                    Some(bound) => ctx
                        .typ(bound)
                        .get_upper_bound()
                        .expect("wildcard bound upper bound"),
                }
            } else {
                let type_ = match &ctx.typ(t).kind {
                    TypeKindData::WildcardType { type_, .. } => *type_,
                    _ => unreachable!(),
                };
                self.wild_upper_bound(ctx, syms, type_.expect("wildcard bound type"))
            }
        } else {
            t
        }
    }

    /// Get a wildcard's lower bound, returning non-wildcards unchanged.
    ///
    /// Java: public Type wildLowerBound(Type t)
    pub fn wild_lower_bound(&self, ctx: &CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
        if ctx.typ(t).has_tag(TypeTag::WILDCARD) {
            if ctx.typ(t).is_extends_bound() {
                syms.bot_type
            } else {
                let type_ = match &ctx.typ(t).kind {
                    TypeKindData::WildcardType { type_, .. } => *type_,
                    _ => unreachable!(),
                };
                self.wild_lower_bound(ctx, syms, type_.expect("wildcard bound type"))
            }
        } else {
            t
        }
    }

    /// Java: Type.isCompound() (Type: false; IntersectionClassType: true;
    /// UnionClassType: getLub().isCompound()).
    fn is_compound(&self, ctx: &mut CompilerContext, t: TypeId) -> bool {
        match &ctx.typ(t).kind {
            TypeKindData::IntersectionClassType { .. } => true,
            TypeKindData::UnionClassType { .. } => {
                // NOT PORTED: UnionClassType.isCompound() calls getLub() (Types
                // TODO); no approximation is done.
                panic!("unimplemented: UnionClassType.isCompound (needs getLub)");
            }
            _ => false,
        }
    }

    /// Java: Type.equalsIgnoreMetadata (Type.equals is identity except the
    /// ArrayType override).
    ///
    /// PORTING_RULES: Type.equals is "this == t" (Type.java) and ArrayType
    /// overrides it as "(this == arrayType || elemtype.equals(arrayType.elemtype))";
    /// both are reproduced here (TypeMetadata is not ported).
    fn equals_ignore_metadata(&self, ctx: &CompilerContext, t: TypeId, s: TypeId) -> bool {
        if t == s {
            return true;
        }
        match (
            &ctx.typ(t).kind,
            &ctx.typ(s).kind,
        ) {
            (
                TypeKindData::ArrayType { elemtype: t_elem, .. },
                TypeKindData::ArrayType { elemtype: s_elem, .. },
            ) => match (t_elem, s_elem) {
                (Some(t_elem), Some(s_elem)) => self.equals_ignore_metadata(ctx, *t_elem, *s_elem),
                (None, None) => true,
                _ => false,
            },
            _ => false,
        }
    }

    /// Return the supertype of given type.
    ///
    /// Java: public Type supertype(Type t)
    pub fn supertype(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
        let tag = ctx.typ(t).get_tag();
        if tag == TypeTag::ERROR {
            return no_type_of(ctx, syms);
        }
        let is_class_family = matches!(
            &ctx.typ(t).kind,
            TypeKindData::ClassType { .. }
                | TypeKindData::ErasedClassType { .. }
                | TypeKindData::UnionClassType { .. }
                | TypeKindData::IntersectionClassType { .. }
        );
        if is_class_family {
            let existing = match &ctx.typ(t).kind {
                TypeKindData::ClassType { supertype_field, .. }
                | TypeKindData::ErasedClassType { supertype_field, .. }
                | TypeKindData::UnionClassType { supertype_field, .. }
                | TypeKindData::IntersectionClassType { supertype_field, .. } => *supertype_field,
                _ => unreachable!(),
            };
            if let Some(sup) = existing {
                return sup;
            }
            let tsym = ctx.typ(t).tsym.expect("class type symbol");
            let mut supertype_opt = class_symbol_get_superclass(ctx, tsym);
            if symbol_is_interface(ctx, tsym) {
                let tsym_type = ctx.sym(tsym).ty.expect("symbol type");
                supertype_opt = match &ctx.typ(tsym_type).kind {
                    TypeKindData::ClassType { supertype_field, .. }
                    | TypeKindData::ErasedClassType { supertype_field, .. }
                    | TypeKindData::UnionClassType { supertype_field, .. }
                    | TypeKindData::IntersectionClassType { supertype_field, .. } => {
                        *supertype_field
                    }
                    _ => None,
                };
            }
            let supertype = supertype_opt.unwrap_or_else(|| no_type_of(ctx, syms));
            let tsym_type = ctx.sym(tsym).ty.expect("symbol type");
            let class_bound_type = class_bound(ctx, syms, t);
            let actuals = allparams_of(ctx, class_bound_type);
            let formals = allparams_of(ctx, tsym_type);
            let result = if has_erased_supertypes(ctx, t) {
                self.erasure_recursive(ctx, syms, supertype)
            } else if formals.non_empty() {
                self.subst_type(ctx, syms, supertype, &formals, &actuals)
            } else {
                supertype
            };
            set_class_supertype_field(ctx, t, Some(result));
            return result;
        }
        if tag == TypeTag::TYPEVAR {
            let upper = ctx.typ(t).get_upper_bound().expect("type var upper bound");
            let upper_is_compound = self.is_compound(ctx, upper);
            let upper_is_interface = type_is_interface(ctx, upper);
            if ctx.typ(upper).has_tag(TypeTag::TYPEVAR)
                || (!upper_is_compound && !upper_is_interface)
            {
                return upper;
            }
            return self.supertype(ctx, syms, upper);
        }
        if tag == TypeTag::ARRAY {
            let elem = match &ctx.typ(t).kind {
                TypeKindData::ArrayType { elemtype, .. } => elemtype.expect("array element type"),
                _ => unreachable!(),
            };
            let elem_is_primitive = ctx.typ(elem).is_primitive();
            let elem_is_object = self.is_same_type(ctx, syms, elem, syms.object_type);
            if elem_is_primitive || elem_is_object {
                return self.array_super_type(ctx, syms);
            }
            let sup = self.supertype(ctx, syms, elem);
            let tsym = ctx.typ(t).tsym;
            return new_array_type(ctx, sup, tsym);
        }
        no_type_of(ctx, syms)
    }

    pub fn interfaces(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> List<TypeId> {
        let is_class_family = matches!(
            &ctx.typ(t).kind,
            TypeKindData::ClassType { .. }
                | TypeKindData::ErasedClassType { .. }
                | TypeKindData::UnionClassType { .. }
                | TypeKindData::IntersectionClassType { .. }
        );
        if is_class_family {
            let existing = match &ctx.typ(t).kind {
                TypeKindData::ClassType { interfaces_field, .. }
                | TypeKindData::ErasedClassType { interfaces_field, .. }
                | TypeKindData::UnionClassType { interfaces_field, .. }
                | TypeKindData::IntersectionClassType { interfaces_field, .. } => {
                    interfaces_field.clone()
                }
                _ => unreachable!(),
            };
            if let Some(list) = existing {
                return list;
            }
            let tsym = ctx.typ(t).tsym.expect("class type symbol");
            let tsym_type = ctx.sym(tsym).ty.expect("symbol type");
            let declared = class_symbol_get_interfaces_option(ctx, tsym);
            let result = match declared {
                Some(list) => list,
                None => {
                    if t == tsym_type {
                        List::nil()
                    } else {
                        let actuals = allparams_of(ctx, t);
                        let formals = allparams_of(ctx, tsym_type);
                        if formals.non_empty() {
                            let interfaces = class_symbol_get_interfaces(ctx, tsym);
                            self.subst_types(ctx, syms, &interfaces, &formals, &actuals)
                        } else {
                            List::nil()
                        }
                    }
                }
            };
            set_class_interfaces_field(ctx, t, Some(result.clone()));
            return result;
        }
        let tag = ctx.typ(t).get_tag();
        if tag == TypeTag::ERROR {
            let existing = match &ctx.typ(t).kind {
                TypeKindData::ErrorType { interfaces_field, .. } => interfaces_field.clone(),
                _ => None,
            };
            if let Some(list) = existing {
                return list;
            }
            let tsym = ctx.typ(t).tsym.expect("error type symbol");
            let declared = class_symbol_get_interfaces_option(ctx, tsym);
            let result = declared.unwrap_or_else(List::nil);
            set_class_interfaces_field(ctx, t, Some(result.clone()));
            return result;
        }
        if tag == TypeTag::TYPEVAR {
            let upper = ctx.typ(t).get_upper_bound().expect("type var upper bound");
            let upper_is_compound = self.is_compound(ctx, upper);
            let upper_is_interface = type_is_interface(ctx, upper);
            if upper_is_compound {
                return self.interfaces(ctx, syms, upper);
            }
            if upper_is_interface {
                return List::of(upper);
            }
            return List::nil();
        }
        List::nil()
    }

    fn subst_visit(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        from: &List<TypeId>,
        to: &List<TypeId>,
    ) -> TypeId {
        let tag = ctx.typ(t).get_tag();
        if tag == TypeTag::TYPEVAR {
            let mut from = from.clone();
            let mut to = to.clone();
            while from.non_empty() {
                if type_equals_ignore_metadata(ctx, t, *from.head()) {
                    return with_type_var(ctx, *to.head(), t);
                }
                from = from.tail();
                to = to.tail();
            }
            return t;
        }
        let is_class_family = matches!(
            &ctx.typ(t).kind,
            TypeKindData::ClassType { .. }
                | TypeKindData::ErasedClassType { .. }
                | TypeKindData::UnionClassType { .. }
                | TypeKindData::IntersectionClassType { .. }
        );
        if is_class_family {
            if self.is_compound(ctx, t) {
                panic!("unimplemented: subst on compound types (needs makeIntersectionType)");
            }
            let (outer, typarams) = match &ctx.typ(t).kind {
                TypeKindData::ClassType { outer_field, typarams_field, .. }
                | TypeKindData::ErasedClassType { outer_field, typarams_field, .. }
                | TypeKindData::UnionClassType { outer_field, typarams_field, .. }
                | TypeKindData::IntersectionClassType { outer_field, typarams_field, .. } => {
                    (*outer_field, typarams_field.clone().unwrap_or_else(List::nil))
                }
                _ => unreachable!(),
            };
            let no_type = no_type_of(ctx, syms);
            let outer1 = match outer {
                Some(outer) => self.subst_visit(ctx, syms, outer, from, to),
                // Java: a class type may have Type.noType as its enclosing
                // type; the structural mapping visits it unchanged.
                None => no_type,
            };
            let typarams1 = self.subst(ctx, syms, &typarams, from, to);
            let outer_unchanged = match outer {
                Some(outer) => outer1 == outer,
                None => outer1 == no_type,
            };
            let unchanged = outer_unchanged && list_same(&typarams, &typarams1);
            if unchanged {
                return t;
            }
            let tsym = ctx.typ(t).tsym;
            let outer_field = if outer1 == no_type { None } else { Some(outer1) };
            return new_class_type(ctx, outer_field, Some(typarams1), tsym);
        }
        if matches!(&ctx.typ(t).kind, TypeKindData::WildcardType { .. }) {
            let (type_, kind, bound) = match &ctx.typ(t).kind {
                TypeKindData::WildcardType { type_, kind, bound } => (*type_, *kind, *bound),
                _ => unreachable!(),
            };
            let Some(type_) = type_ else {
                return t;
            };
            let visited = self.subst_visit(ctx, syms, type_, from, to);
            let t2 = if visited == type_ {
                t
            } else {
                let tsym = ctx.typ(t).tsym;
                new_wildcard_type(ctx, Some(visited), kind, tsym, bound)
            };
            if t2 != t && ctx.typ(t).is_extends_bound() && ctx.typ(visited).is_extends_bound() {
                let upper = self.wild_upper_bound(ctx, syms, visited);
                if let TypeKindData::WildcardType { type_, .. } = &mut ctx.typ_mut(t2).kind {
                    *type_ = Some(upper);
                }
            }
            return t2;
        }
        if tag == TypeTag::ARRAY {
            let elem = match &ctx.typ(t).kind {
                TypeKindData::ArrayType { elemtype, .. } => elemtype.expect("array element type"),
                _ => unreachable!(),
            };
            let elem1 = self.subst_visit(ctx, syms, elem, from, to);
            if elem1 == elem {
                return t;
            }
            let tsym = ctx.typ(t).tsym;
            return new_array_type(ctx, elem1, tsym);
        }
        if tag == TypeTag::METHOD {
            let (argtypes, restype, thrown) = match &ctx.typ(t).kind {
                TypeKindData::MethodType { argtypes, restype, thrown, .. } => {
                    (argtypes.clone(), *restype, thrown.clone())
                }
                _ => unreachable!(),
            };
            let argtypes1 = self.subst(ctx, syms, &argtypes, from, to);
            let restype = restype.expect("method return type");
            let restype1 = self.subst_visit(ctx, syms, restype, from, to);
            let thrown1 = self.subst(ctx, syms, &thrown, from, to);
            if list_same(&argtypes, &argtypes1)
                && restype1 == restype
                && list_same(&thrown, &thrown1)
            {
                return t;
            }
            let tsym = ctx.typ(t).tsym;
            return new_method_type_2(ctx, argtypes1, Some(restype1), thrown1, tsym);
        }
        if tag == TypeTag::FORALL {
            // Java: Subst.visitForAll (Types.java:3375-3400)
            let (tvars, qtype) = match &ctx.typ(t).kind {
                TypeKindData::ForAll { tvars, qtype } => (tvars.clone(), *qtype),
                _ => unreachable!(),
            };
            let qtype = qtype.expect("for-all qtype");
            let mut tvars_cur = tvars.clone();
            let mut qtype_cur = qtype;
            if self.type_list_contains_any(ctx, syms, to, &tvars, &mut Vec::new()) {
                let freevars = self.new_instances(ctx, syms, &tvars);
                let substituted =
                    self.subst_type(ctx, syms, qtype_cur, &tvars, &freevars);
                qtype_cur = substituted;
                tvars_cur = freevars;
            }
            let tvars1 = self.subst_bounds(ctx, syms, &tvars_cur, from, to);
            let qtype1 = self.subst_visit(ctx, syms, qtype_cur, from, to);
            if list_same(&tvars_cur, &tvars1) && qtype1 == qtype_cur {
                return t;
            }
            let new_qtype = if list_same(&tvars_cur, &tvars1) {
                qtype1
            } else {
                self.subst_type(ctx, syms, qtype1, &tvars_cur, &tvars1)
            };
            let qtype_tsym = ctx.typ(new_qtype).tsym;
            return ctx.alloc_type(crate::code::type_::Type::new(
                TypeTag::FORALL,
                qtype_tsym,
                TypeKindData::ForAll {
                    tvars: tvars1,
                    qtype: Some(new_qtype),
                },
            ));
        }
        if self.is_compound(ctx, t) {
            // Java: Subst.visitClassType for compound types (Types.java:3355-3365)
            let supertype = self.supertype(ctx, syms, t);
            let st = self.subst_visit(ctx, syms, supertype, from, to);
            let interfaces = self.interfaces(ctx, syms, t);
            let mut is: List<TypeId> = List::nil();
            let mut current = interfaces.clone();
            while current.non_empty() {
                let interface = *current.head();
                let substituted = self.subst_visit(ctx, syms, interface, from, to);
                is = is.append(substituted);
                current = current.tail();
            }
            if st == supertype && list_same(&interfaces, &is) {
                return t;
            }
            let Some(names) = self.names.clone() else {
                unimplemented!(
                    "TODO(Types batch): subst on compound types needs the Names service (Types::with_names)"
                );
            };
            return self.make_intersection_type(ctx, syms, &names, is.prepend(st), false);
        }
        t
    }

    /// Java: public List<Type> newInstances(List<Type> tvars)
    /// (Types.java:3487-3496)
    pub fn new_instances(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        tvars: &List<TypeId>,
    ) -> List<TypeId> {
        let mut tvars1: List<TypeId> = List::nil();
        let mut current = tvars.clone();
        while current.non_empty() {
            let tvar = *current.head();
            let (bound, lower) = match &ctx.typ(tvar).kind {
                TypeKindData::TypeVar { bound, lower } => (*bound, *lower),
                _ => (None, None),
            };
            let tsym = ctx.typ(tvar).tsym;
            let new_tvar = ctx.alloc_type(crate::code::type_::Type::new(
                TypeTag::TYPEVAR,
                tsym,
                TypeKindData::TypeVar { bound, lower },
            ));
            tvars1 = tvars1.append(new_tvar);
            current = current.tail();
        }
        let mut result: List<TypeId> = List::nil();
        let mut current = tvars1.clone();
        while current.non_empty() {
            let tvar = *current.head();
            let upper = ctx.typ(tvar).get_upper_bound().expect("type var upper bound");
            let substituted = self.subst_type(ctx, syms, upper, tvars, &tvars1);
            if let TypeKindData::TypeVar { bound, .. } = &mut ctx.typ_mut(tvar).kind {
                *bound = Some(substituted);
            }
            result = result.append(tvar);
            current = current.tail();
        }
        result
    }

    /// Java: public Type createMethodTypeWithReturn(Type original,
    /// Type newReturn) (Types.java:3535-3565; the anonymous baseType override
    /// is registered: the local Type has no baseType concept, so the returned
    /// MethodType does not carry a base type)
    pub fn create_method_type_with_return(
        &self,
        ctx: &mut CompilerContext,
        original: TypeId,
        new_return: TypeId,
    ) -> TypeId {
        let kind = ctx.typ(original).kind.clone();
        let tsym = ctx.typ(original).tsym;
        let tag = ctx.typ(original).get_tag();
        match &kind {
            TypeKindData::MethodType {
                argtypes,
                thrown,
                recvtype,
                ..
            } => ctx.alloc_type(crate::code::type_::Type::new(
                TypeTag::METHOD,
                tsym,
                TypeKindData::MethodType {
                    argtypes: argtypes.clone(),
                    restype: Some(new_return),
                    thrown: thrown.clone(),
                    recvtype: *recvtype,
                },
            )),
            TypeKindData::ForAll { tvars, qtype } => {
                let qtype = qtype.expect("for-all qtype");
                let new_qtype = self.create_method_type_with_return(ctx, qtype, new_return);
                ctx.alloc_type(crate::code::type_::Type::new(
                    TypeTag::FORALL,
                    tsym,
                    TypeKindData::ForAll {
                        tvars: tvars.clone(),
                        qtype: Some(new_qtype),
                    },
                ))
            }
            _ => panic!("IllegalArgumentException: Not a method type: {tag:?}"),
        }
    }

    /// Java: public List<Type> substBounds(List<Type> tvars, List<Type> from,
    /// List<Type> to) (Types.java:3407-3445)
    pub(crate) fn subst_bounds(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        tvars: &List<TypeId>,
        from: &List<TypeId>,
        to: &List<TypeId>,
    ) -> List<TypeId> {
        if tvars.is_empty() {
            return tvars.clone();
        }
        let mut new_bounds: Vec<TypeId> = Vec::new();
        let mut changed = false;
        let mut current = tvars.clone();
        while current.non_empty() {
            let tvar = *current.head();
            let upper = ctx.typ(tvar).get_upper_bound().expect("type var upper bound");
            let bound = self.subst_type(ctx, syms, upper, from, to);
            if bound != upper {
                changed = true;
            }
            new_bounds.push(bound);
            current = current.tail();
        }
        if !changed {
            return tvars.clone();
        }
        let mut new_tvars: List<TypeId> = List::nil();
        let mut current = tvars.clone();
        while current.non_empty() {
            let tvar = *current.head();
            let tsym = ctx.typ(tvar).tsym;
            let new_tvar = ctx.alloc_type(crate::code::type_::Type::new(
                TypeTag::TYPEVAR,
                tsym,
                TypeKindData::TypeVar {
                    bound: None,
                    lower: Some(syms.bot_type),
                },
            ));
            new_tvars = new_tvars.append(new_tvar);
            current = current.tail();
        }
        let mut substituted_bounds: Vec<TypeId> = Vec::new();
        for bound in &new_bounds {
            let substituted = self.subst_type(ctx, syms, *bound, tvars, &new_tvars);
            substituted_bounds.push(substituted);
        }
        let mut current = new_tvars.clone();
        let mut index = 0;
        while current.non_empty() {
            let tvar = *current.head();
            let bound = substituted_bounds[index];
            if let TypeKindData::TypeVar { bound: bound_field, .. } =
                &mut ctx.typ_mut(tvar).kind
            {
                *bound_field = Some(bound);
            }
            index += 1;
            current = current.tail();
        }
        new_tvars
    }

    /// Java: Type.containsAny(List<Type> ts, List<Type> tvs)
    /// (Type.java containsAny/contains helper for Subst.visitForAll)
    fn type_list_contains_any(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts: &List<TypeId>,
        tvs: &List<TypeId>,
        cache: &mut Vec<(TypeId, TypeId)>,
    ) -> bool {
        let mut current = ts.clone();
        while current.non_empty() {
            let t = *current.head();
            if self.type_list_contains(ctx, syms, tvs, t, cache) {
                return true;
            }
            current = current.tail();
        }
        false
    }

    /// Java: Type.contains(List<Type> ts, Type elem)
    fn type_list_contains(
        &self,
        ctx: &mut CompilerContext,
        _syms: &Symtab,
        ts: &List<TypeId>,
        elem: TypeId,
        _cache: &mut Vec<(TypeId, TypeId)>,
    ) -> bool {
        let mut current = ts.clone();
        while current.non_empty() {
            let t = *current.head();
            if self.type_contains(ctx, _syms, t, elem) {
                return true;
            }
            current = current.tail();
        }
        false
    }

    /// The direct supertypes of a type.
    ///
    /// Java: public List<Type> directSupertypes(Type t)
    pub fn direct_supertypes(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> List<TypeId> {
        if !ctx.typ(t).is_intersection() {
            let sup = self.supertype(ctx, syms, t);
            let no_type = no_type_of(ctx, syms);
            let interfaces = self.interfaces(ctx, syms, t);
            if sup == no_type || sup == t {
                interfaces
            } else {
                interfaces.prepend(sup)
            }
        } else {
            ctx.typ(t).get_explicit_components()
        }
    }

    /// Java: public List<Type> erasureRecursive(Type t)
    pub fn erasure_recursive(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> TypeId {
        self.erasure_recurse(ctx, syms, t, true)
    }

    /// Java: public List<Type> erasure(List<Type> ts)
    pub fn erasure_list(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts: &List<TypeId>,
    ) -> List<TypeId> {
        let mut elements = Vec::new();
        for t in ts.iter() {
            elements.push(self.erasure(ctx, syms, t));
        }
        let mut result = List::nil();
        for t in elements.into_iter().rev() {
            result = result.prepend(t);
        }
        result
    }

    /// Java: public List<Type> subst(List<Type> ts, List<Type> from,
    /// List<Type> to)
    pub fn subst(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts: &List<TypeId>,
        from: &List<TypeId>,
        to: &List<TypeId>,
    ) -> List<TypeId> {
        let (from, to) = self.align_subst_lists(from, to);
        let mut elements = Vec::new();
        for t in ts.iter() {
            elements.push(self.subst_visit(ctx, syms, t, &from, &to));
        }
        let mut result = List::nil();
        for t in elements.into_iter().rev() {
            result = result.prepend(t);
        }
        result
    }

    /// Substitute all occurrences of a type in from with the corresponding type
    /// in to in t.
    ///
    /// Java: public Type subst(Type t, List<Type> from, List<Type> to)
    pub fn subst_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        from: &List<TypeId>,
        to: &List<TypeId>,
    ) -> TypeId {
        let (from, to) = self.align_subst_lists(from, to);
        self.subst_visit(ctx, syms, t, &from, &to)
    }

    /// Java: subst(List<Type> ts, from, to) for a list of types.
    pub fn subst_types(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts: &List<TypeId>,
        from: &List<TypeId>,
        to: &List<TypeId>,
    ) -> List<TypeId> {
        self.subst(ctx, syms, ts, from, to)
    }

    /// Java: Subst constructor: match lists from right, discarding leading
    /// elements of the longer list.
    fn align_subst_lists(
        &self,
        from: &List<TypeId>,
        to: &List<TypeId>,
    ) -> (List<TypeId>, List<TypeId>) {
        let mut from = from.clone();
        let mut to = to.clone();
        while from.length() > to.length() {
            from = from.tail();
        }
        while from.length() < to.length() {
            to = to.tail();
        }
        (from, to)
    }

    /// Return the base type of t or any of its supertypes that starts with the
    /// given symbol.
    ///
    /// Java: public Type asSuper(Type t, Symbol sym)
    pub fn as_super(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        sym: SymbolId,
    ) -> Option<TypeId> {
        // optimization: sym.type == syms.objectType
        if ctx.sym(sym).ty == Some(syms.object_type) {
            return Some(syms.object_type);
        }
        let mut seen: Vec<SymbolId> = Vec::new();
        self.as_super_visit(ctx, syms, t, sym, &mut seen)
    }

    fn as_super_visit(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        sym: SymbolId,
        seen: &mut Vec<SymbolId>,
    ) -> Option<TypeId> {
        match &ctx.typ(t).kind {
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
                if ctx.typ(t).tsym == Some(sym) {
                    return Some(t);
                }
                let c = ctx.typ(t).tsym.expect("class type symbol");
                if seen.contains(&c) {
                    return None;
                }
                seen.push(c);
                let result = (|| {
                    let st = self.supertype(ctx, syms, t);
                    if (ctx.typ(st).has_tag(TypeTag::CLASS)
                        || ctx.typ(st).has_tag(TypeTag::TYPEVAR))
                        && let Some(x) = self.as_super_visit(ctx, syms, st, sym, seen)
                    {
                        return Some(x);
                    }
                    if (ctx.sym(sym).flags() & crate::code::flags::INTERFACE as i64) != 0 {
                        for i in self.interfaces(ctx, syms, t).iter() {
                            if !ctx.typ(i).has_tag(TypeTag::ERROR)
                                && let Some(x) = self.as_super_visit(ctx, syms, i, sym, seen)
                            {
                                return Some(x);
                            }
                        }
                    }
                    None
                })();
                seen.pop();
                result
            }
            TypeKindData::ArrayType { .. } => {
                let sym_type = ctx.sym(sym).ty.expect("symbol type");
                if self.is_subtype_no_capture(ctx, syms, t, sym_type) {
                    Some(sym_type)
                } else {
                    None
                }
            }
            TypeKindData::TypeVar { .. } | TypeKindData::CapturedType { .. } => {
                if ctx.typ(t).tsym == Some(sym) {
                    Some(t)
                } else {
                    let upper = ctx.typ(t).get_upper_bound().expect("type var upper bound");
                    self.as_super_visit(ctx, syms, upper, sym, seen)
                }
            }
            TypeKindData::ErrorType { .. } => Some(t),
            _ => None,
        }
    }

    /// Return the base type of t or any of its outer types that starts with the
    /// given symbol.
    ///
    /// Java: public Type asOuterSuper(Type t, Symbol sym)
    pub fn as_outer_super(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        sym: SymbolId,
    ) -> Option<TypeId> {
        match ctx.typ(t).get_tag() {
            TypeTag::CLASS => {
                let mut t = t;
                loop {
                    if let Some(s) = self.as_super(ctx, syms, t, sym) {
                        return Some(s);
                    }
                    match enclosing_of(ctx, t) {
                        Some(outer) if ctx.typ(outer).has_tag(TypeTag::CLASS) => t = outer,
                        _ => return None,
                    }
                }
            }
            TypeTag::ARRAY => {
                let sym_type = ctx.sym(sym).ty.expect("symbol type");
                if self.is_subtype_no_capture(ctx, syms, t, sym_type) {
                    Some(sym_type)
                } else {
                    None
                }
            }
            TypeTag::TYPEVAR => self.as_super(ctx, syms, t, sym),
            TypeTag::ERROR => Some(t),
            _ => None,
        }
    }

    /// Return the base type of t or any of its enclosing types that starts with
    /// the given symbol.
    ///
    /// Java: public Type asEnclosingSuper(Type t, Symbol sym)
    /// (Types.java:2251-2273)
    pub fn as_enclosing_super(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        sym: SymbolId,
    ) -> Option<TypeId> {
        match ctx.typ(t).get_tag() {
            TypeTag::CLASS => {
                let mut t = t;
                loop {
                    if let Some(s) = self.as_super(ctx, syms, t, sym) {
                        return Some(s);
                    }
                    let outer = get_enclosing_type(ctx, t);
                    let next = match outer {
                        Some(outer) if ctx.typ(outer).has_tag(TypeTag::CLASS) => outer,
                        _ => {
                            let t_tsym = ctx.typ(t).tsym.expect("class type symbol");
                            let owner = ctx.sym(t_tsym).owner?;
                            match crate::comp::resolve::symbols_encl_class(ctx, owner) {
                                Some(encl_class) => ctx
                                    .sym(encl_class)
                                    .ty
                                    .expect("enclosing class type"),
                                None => break,
                            }
                        }
                    };
                    t = next;
                    if !ctx.typ(t).has_tag(TypeTag::CLASS) {
                        return None;
                    }
                }
                None
            }
            TypeTag::ARRAY => {
                let sym_type = ctx.sym(sym).ty.expect("symbol type");
                if self.is_subtype_no_capture(ctx, syms, t, sym_type) {
                    Some(sym_type)
                } else {
                    None
                }
            }
            TypeTag::TYPEVAR => self.as_super(ctx, syms, t, sym),
            TypeTag::ERROR => Some(t),
            _ => None,
        }
    }

    /// The type of given symbol, seen as a member of t.
    ///
    /// Java: public Type memberType(Type t, Symbol sym)
    pub fn member_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        sym: SymbolId,
    ) -> TypeId {
        if (ctx.sym(sym).flags() & crate::code::flags::STATIC as i64) != 0 {
            return ctx.sym(sym).ty.expect("member type");
        }
        self.member_type_visit(ctx, syms, t, sym)
    }

    fn member_type_visit(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        sym: SymbolId,
    ) -> TypeId {
        match &ctx.typ(t).kind {
            TypeKindData::WildcardType { .. } => {
                let upper = self.wild_upper_bound(ctx, syms, t);
                self.member_type(ctx, syms, upper, sym)
            }
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
                let owner = ctx.sym(sym).owner;
                let flags = ctx.sym(sym).flags();
                if (flags & crate::code::flags::STATIC as i64) == 0
                    && owner
                        .and_then(|owner| ctx.sym(owner).ty)
                        .map(|owner_type| is_parameterized_type(ctx, owner_type))
                        .unwrap_or(false)
                {
                    let owner = owner.expect("member owner");
                    let base = self.as_outer_super(ctx, syms, t, owner);
                    let base = if self.is_compound(ctx, t) {
                        // Java: base = t.isCompound() ? capture(base) : base;
                        base.map(|base| self.capture(ctx, syms, base))
                    } else {
                        base
                    };
                    if let Some(base) = base {
                        let sym_type = ctx.sym(sym).ty.expect("member type");
                        let owner_type = ctx.sym(owner).ty.expect("owner type");
                        let owner_params = allparams_of(ctx, owner_type);
                        let base_params = allparams_of(ctx, base);
                        if owner_params.non_empty() {
                            if base_params.is_empty() {
                                // then base is a raw type
                                return self.erasure(ctx, syms, sym_type);
                            } else {
                                return self.subst_type(
                                    ctx,
                                    syms,
                                    sym_type,
                                    &owner_params,
                                    &base_params,
                                );
                            }
                        }
                    }
                }
                ctx.sym(sym).ty.expect("member type")
            }
            TypeKindData::TypeVar { .. } | TypeKindData::CapturedType { .. } => {
                let upper = ctx.typ(t).get_upper_bound().expect("type var upper bound");
                self.member_type(ctx, syms, upper, sym)
            }
            _ => ctx.sym(sym).ty.expect("member type"),
        }
    }

    /// Java: public Type capture(Type t)
    ///
    /// PORTING_RULES: the freshTypeVariables/glb parts need CapturedType plus
    /// glb (TODO Types): a type that does not need capture conversion is
    /// returned as-is (the early returns of the upstream method), while the part
    /// that needs fresh captured variables panics with an explicit
    /// unimplemented message (no approximation).
    pub fn capture(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
        if !ctx.typ(t).has_tag(TypeTag::CLASS) {
            return t;
        }
        let no_type = no_type_of(ctx, syms);
        if let Some(encl) = enclosing_of(ctx, t)
            && encl != no_type
        {
            let captured_encl = self.capture(ctx, syms, encl);
            if captured_encl != encl {
                let tsym = ctx.typ(t).tsym.expect("class type symbol");
                let type1 = self.member_type(ctx, syms, captured_encl, tsym);
                let tsym_type = ctx.sym(tsym).ty.expect("symbol type");
                let formals = get_type_arguments(ctx, tsym_type);
                let actuals = get_type_arguments(ctx, t);
                return self.subst_type(ctx, syms, type1, &formals, &actuals);
            }
        }
        if is_raw_type(ctx, t) || !is_parameterized_type(ctx, t) {
            return t;
        }
        let cls = t;
        let tsym = ctx.typ(cls).tsym.expect("class type symbol");
        let tsym_type = ctx.sym(tsym).ty.expect("class symbol type");
        let a_list = get_type_arguments(ctx, tsym_type);
        let t_list = get_type_arguments(ctx, cls);
        let s_list = self.fresh_type_variables(ctx, syms, t_list.clone());
        let mut current_a = a_list.clone();
        let mut current_t = t_list;
        let mut current_s = s_list.clone();
        let mut captured = false;
        while current_a.non_empty() && current_t.non_empty() && current_s.non_empty() {
            let head_s = *current_s.head();
            let head_t = *current_t.head();
            if head_s != head_t {
                captured = true;
                let head_a = *current_a.head();
                let wildcard = head_t;
                // Ui = A.head.getUpperBound() (or Object when absent)
                let ui = ctx
                    .typ(head_a)
                    .get_upper_bound()
                    .unwrap_or(syms.object_type);
                let kind = match &ctx.typ(wildcard).kind {
                    TypeKindData::WildcardType { kind, .. } => *kind,
                    _ => crate::code::bound_kind::BoundKind::UNBOUND,
                };
                let (upper, lower) = match kind {
                    crate::code::bound_kind::BoundKind::UNBOUND => {
                        let upper = self.subst_type(ctx, syms, ui, &a_list, &s_list);
                        (upper, syms.bot_type)
                    }
                    crate::code::bound_kind::BoundKind::EXTENDS => {
                        let extends_bound = match &ctx.typ(wildcard).kind {
                            TypeKindData::WildcardType { type_, .. } => {
                                type_.expect("extends bound")
                            }
                            _ => unreachable!(),
                        };
                        let ui_subst = self.subst_type(ctx, syms, ui, &a_list, &s_list);
                        let upper = self.glb(ctx, syms, extends_bound, ui_subst);
                        (upper, syms.bot_type)
                    }
                    crate::code::bound_kind::BoundKind::SUPER => {
                        let upper = self.subst_type(ctx, syms, ui, &a_list, &s_list);
                        let super_bound = match &ctx.typ(wildcard).kind {
                            TypeKindData::WildcardType { type_, .. } => {
                                type_.expect("super bound")
                            }
                            _ => unreachable!(),
                        };
                        (upper, super_bound)
                    }
                };
                // Java: Si.setUpperBound(upper); Si.lower = lower;
                if let TypeKindData::CapturedType { bound: b, lower: l, .. } =
                    &mut ctx.typ_mut(head_s).kind
                {
                    *b = Some(upper);
                    *l = Some(lower);
                }
                let upper = ctx
                    .typ(head_s)
                    .get_upper_bound()
                    .expect("captured upper bound");
                let lower = match &ctx.typ(head_s).kind {
                    TypeKindData::CapturedType { lower, .. } => {
                        lower.expect("captured lower bound")
                    }
                    _ => unreachable!(),
                };
                let tmp_bound_err = ctx.typ(upper).has_tag(TypeTag::ERROR);
                let tmp_lower_err = ctx.typ(lower).has_tag(TypeTag::ERROR);
                if !tmp_bound_err
                    && !tmp_lower_err
                    && self.is_same_type(ctx, syms, upper, lower)
                {
                    current_s = current_s.tail().prepend(upper);
                }
            }
            current_a = current_a.tail();
            current_t = current_t.tail();
            current_s = current_s.tail();
        }
        if !current_a.is_empty() || !current_t.is_empty() || !current_s.is_empty() {
            // some "rare" type involved
            return self.erasure(ctx, syms, t);
        }
        if captured {
            let no_type = no_type_of(ctx, syms);
            let enclosing = ctx
                .typ(cls)
                .get_enclosing_type()
                .filter(|enclosing| *enclosing != no_type);
            return ctx.alloc_type(crate::code::type_::Type::new(
                TypeTag::CLASS,
                Some(tsym),
                TypeKindData::ClassType {
                    outer_field: enclosing,
                    typarams_field: Some(s_list),
                    allparams_field: None,
                    supertype_field: None,
                    interfaces_field: None,
                    all_interfaces_field: None,
                },
            ));
        }
        t
    }

    /// Java: public List<Type> freshTypeVariables(List<Type> types)
    /// (Types.java:4458-4475)
    fn fresh_type_variables(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: List<TypeId>,
    ) -> List<TypeId> {
        let captured_name = self.captured_name();
        let mut result: List<TypeId> = List::nil();
        let mut current = types;
        while current.non_empty() {
            let t = *current.head();
            if ctx.typ(t).has_tag(TypeTag::WILDCARD) {
                let bound = match &ctx.typ(t).kind {
                    TypeKindData::WildcardType { type_, .. } => *type_,
                    _ => None,
                }
                .unwrap_or(syms.object_type);
                let captured = self.new_captured_type_with_symbol(
                    ctx,
                    syms,
                    captured_name.clone(),
                    bound,
                    syms.bot_type,
                    t,
                );
                result = result.append(captured);
            } else {
                result = result.append(t);
            }
            current = current.tail();
        }
        result
    }

    /// Java: new CapturedType(capturedName, syms.noSymbol, bound, lower,
    /// wildcard) (Type.java CapturedType constructor): creates the captured
    /// TypeVariableSymbol and its CapturedType.
    fn new_captured_type_with_symbol(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        name: crate::util::name::Name,
        bound: TypeId,
        lower: TypeId,
        wildcard: TypeId,
    ) -> TypeId {
        let symbol = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            crate::code::kinds::Kind::TYP,
            0,
            name,
            None,
            Some(syms.no_symbol),
            crate::code::symbol::SymbolKind::TypeVariableSymbol,
        ));
        let type_id = ctx.alloc_type(crate::code::type_::Type::new(
            TypeTag::TYPEVAR,
            Some(symbol),
            TypeKindData::CapturedType {
                bound: Some(bound),
                lower: Some(lower),
                wildcard: Some(wildcard),
            },
        ));
        ctx.sym_mut(symbol).ty = Some(type_id);
        type_id
    }

    /// Java: public boolean containedBy(Type t, Type s)
    pub fn contained_by(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        match ctx.typ(t).get_tag() {
            TypeTag::UNDETVAR => {
                if ctx.typ(s).has_tag(TypeTag::WILDCARD) {
                    let kind = match &ctx.typ(s).kind {
                        TypeKindData::WildcardType { kind, .. } => *kind,
                        _ => unreachable!(),
                    };
                    match kind {
                        crate::code::bound_kind::BoundKind::UNBOUND => {}
                        crate::code::bound_kind::BoundKind::EXTENDS => {
                            let bound = self.wild_upper_bound(ctx, syms, s);
                            self.undet_var_add_bound(
                                ctx,
                                syms,
                                t,
                                crate::code::type_::InferenceBound::UPPER,
                                bound,
                                false,
                            );
                        }
                        crate::code::bound_kind::BoundKind::SUPER => {
                            let bound = self.wild_lower_bound(ctx, syms, s);
                            self.undet_var_add_bound(
                                ctx,
                                syms,
                                t,
                                crate::code::type_::InferenceBound::LOWER,
                                bound,
                                false,
                            );
                        }
                    }
                    true
                } else {
                    self.is_same_type(ctx, syms, t, s)
                }
            }
            TypeTag::ERROR => true,
            _ => self.contains_type(ctx, syms, s, t),
        }
    }

    /// Java: public boolean containsType(Type t, Type s)
    pub fn contains_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        match &ctx.typ(t).kind {
            TypeKindData::WildcardType { .. } => {
                if ctx.typ(s).is_partial() {
                    self.contained_by(ctx, syms, s, t)
                } else {
                    let t_ext = ctx.typ(t).is_extends_bound();
                    let t_sup = ctx.typ(t).is_super_bound();
                    let lower_t = self.wild_lower_bound(ctx, syms, t);
                    let lower_s = self.wild_lower_bound(ctx, syms, s);
                    let upper_s = self.wild_upper_bound(ctx, syms, s);
                    let upper_t = self.wild_upper_bound(ctx, syms, t);
                    self.is_same_wildcard(ctx, syms, t, s)
                        || self.is_capture_of(ctx, syms, s, t)
                        || ((t_ext || self.is_subtype_no_capture(ctx, syms, lower_t, lower_s))
                            && (t_sup || self.is_subtype_no_capture(ctx, syms, upper_s, upper_t)))
                }
            }
            TypeKindData::ErrorType { .. } => true,
            TypeKindData::UndetVar { .. } => {
                // Java: ContainsType.visitUndetVar (Types.java:1593)
                if !ctx.typ(s).has_tag(TypeTag::WILDCARD) {
                    self.is_same_type(ctx, syms, t, s)
                } else {
                    false
                }
            }
            TypeKindData::ForAll { qtype, .. } => {
                let qtype = qtype.expect("for-all qtype");
                self.contains_type(ctx, syms, qtype, s)
            }
            _ => {
                if ctx.typ(s).is_partial() {
                    self.contained_by(ctx, syms, s, t)
                } else {
                    self.is_same_type(ctx, syms, t, s)
                }
            }
        }
    }

    /// Java: boolean containsType(List<Type> ts, List<Type> ss)
    pub fn contains_type_list(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts: &List<TypeId>,
        ss: &List<TypeId>,
    ) -> bool {
        let mut ts = ts.clone();
        let mut ss = ss.clone();
        while ts.non_empty()
            && ss.non_empty()
            && self.contains_type(ctx, syms, *ts.head(), *ss.head())
        {
            ts = ts.tail();
            ss = ss.tail();
        }
        ts.is_empty() && ss.is_empty()
    }

    /// Java: public boolean isCaptureOf(Type s, WildcardType t)
    pub fn is_capture_of(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        s: TypeId,
        t: TypeId,
    ) -> bool {
        if !ctx.typ(s).has_tag(TypeTag::TYPEVAR) || !ctx.typ(s).is_captured() {
            return false;
        }
        let wildcard = match &ctx.typ(s).kind {
            TypeKindData::CapturedType { wildcard, .. } => *wildcard,
            _ => None,
        };
        match wildcard {
            Some(wildcard) => self.is_same_wildcard(ctx, syms, t, wildcard),
            None => false,
        }
    }

    /// Java: public boolean isSameWildcard(WildcardType t, Type s)
    pub fn is_same_wildcard(
        &self,
        ctx: &mut CompilerContext,
        _syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        if !ctx.typ(s).has_tag(TypeTag::WILDCARD) {
            return false;
        }
        let (t_kind, t_type) = match &ctx.typ(t).kind {
            TypeKindData::WildcardType { kind, type_, .. } => (*kind, *type_),
            _ => return false,
        };
        let (s_kind, s_type) = match &ctx.typ(s).kind {
            TypeKindData::WildcardType { kind, type_, .. } => (*kind, *type_),
            _ => return false,
        };
        s_kind == t_kind && s_type == t_type
    }

    /// Java: public boolean isSubtype(Type t, Type s)
    pub fn is_subtype(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        self.is_subtype_with_capture(ctx, syms, t, s, true)
    }

    /// Java: public final boolean isSubtypeNoCapture(Type t, Type s)
    pub fn is_subtype_no_capture(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        self.is_subtype_with_capture(ctx, syms, t, s, false)
    }

    /// Java: public boolean isSubtype(Type t, Type s, boolean capture)
    pub fn is_subtype_with_capture(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        capture: bool,
    ) -> bool {
        if type_equals_ignore_metadata(ctx, t, s) {
            return true;
        }
        if ctx.typ(s).is_partial() {
            return self.is_super_type(ctx, syms, s, t);
        }
        if self.is_compound(ctx, s) {
            let components = self
                .interfaces(ctx, syms, s)
                .prepend(self.supertype(ctx, syms, s));
            for s2 in components.iter() {
                if !self.is_subtype_with_capture(ctx, syms, t, s2, capture) {
                    return false;
                }
            }
            return true;
        }
        if !ctx.typ(t).has_tag(TypeTag::UNDETVAR) && !self.is_compound(ctx, t) {
            let lower = cvar_lower_bound(ctx, self.wild_lower_bound(ctx, syms, s));
            if s != lower && !ctx.typ(lower).has_tag(TypeTag::BOT) {
                let t2 = if capture { self.capture(ctx, syms, t) } else { t };
                return self.is_subtype_with_capture(ctx, syms, t2, lower, false);
            }
        }
        let t2 = if capture { self.capture(ctx, syms, t) } else { t };
        self.is_subtype_visit(ctx, syms, t2, s)
    }

    /// Java: isSubtype.visit(t, s) (the TypeRelation dispatch).
    fn is_subtype_visit(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        match &ctx.typ(t).kind {
            TypeKindData::JCPrimitiveType
            | TypeKindData::JCVoidType
            | TypeKindData::BottomType
            | TypeKindData::JCNoType
            | TypeKindData::UnknownType => match ctx.typ(t).get_tag() {
                TypeTag::BYTE => {
                    !ctx.typ(s).has_tag(TypeTag::CHAR)
                        && ctx.typ(t).get_tag().is_sub_range_of(ctx.typ(s).get_tag())
                }
                TypeTag::CHAR => {
                    !ctx.typ(s).has_tag(TypeTag::SHORT)
                        && ctx.typ(t).get_tag().is_sub_range_of(ctx.typ(s).get_tag())
                }
                TypeTag::SHORT | TypeTag::INT | TypeTag::LONG | TypeTag::FLOAT
                | TypeTag::DOUBLE => ctx.typ(t).get_tag().is_sub_range_of(ctx.typ(s).get_tag()),
                TypeTag::BOOLEAN | TypeTag::VOID => ctx.typ(t).has_tag(ctx.typ(s).get_tag()),
                TypeTag::BOT => {
                    ctx.typ(s).has_tag(TypeTag::BOT)
                        || ctx.typ(s).has_tag(TypeTag::CLASS)
                        || ctx.typ(s).has_tag(TypeTag::ARRAY)
                        || ctx.typ(s).has_tag(TypeTag::TYPEVAR)
                }
                TypeTag::WILDCARD | TypeTag::NONE => false,
                TypeTag::TYPEVAR => {
                    let upper = ctx.typ(t).get_upper_bound().expect("type var upper bound");
                    self.is_subtype_no_capture(ctx, syms, upper, s)
                }
                tag => panic!("AssertionError: isSubtype {tag:?}"),
            },
            TypeKindData::UndetVar { .. } => {
                self.is_subtype_visit_undet_var(ctx, syms, t, s)
            }
            TypeKindData::Deferred { .. } => {
                // Java: IsSubtype default branch throws AssertionError for
                // DEFERRED (deferred types are mapped first).
                panic!("AssertionError: isSubtype DEFERRED")
            }
            TypeKindData::TypeVar { .. } | TypeKindData::CapturedType { .. } => {
                let upper = ctx.typ(t).get_upper_bound().expect("type var upper bound");
                self.is_subtype_no_capture(ctx, syms, upper, s)
            }
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
                // Java: IsSubtype.visitClassType (Types.java:1159-1172)
                let Some(s_tsym) = ctx.typ(s).tsym else {
                    return false;
                };
                let Some(sup) = self.as_super(ctx, syms, t, s_tsym) else {
                    return false;
                };
                // If t is an intersection, sup might not be a class type
                if !ctx.typ(sup).has_tag(TypeTag::CLASS) {
                    return self.is_subtype_no_capture(ctx, syms, sup, s);
                }
                ctx.typ(sup).tsym == ctx.typ(s).tsym
                    && (!is_parameterized_type(ctx, s)
                        || self.contains_type_recursive(ctx, syms, s, sup))
                    && {
                        let sup_encl = enclosing_or_no_type(ctx, sup);
                        let s_encl = enclosing_or_no_type(ctx, s);
                        self.is_subtype_no_capture(ctx, syms, sup_encl, s_encl)
                    }
            }
            TypeKindData::ErrorType { .. } => true,
            TypeKindData::ArrayType { .. } => {
                let t_elem = match &ctx.typ(t).kind {
                    TypeKindData::ArrayType { elemtype, .. } => elemtype.expect("array element"),
                    _ => unreachable!(),
                };
                if ctx.typ(s).has_tag(TypeTag::ARRAY) {
                    let s_elem = match &ctx.typ(s).kind {
                        TypeKindData::ArrayType { elemtype, .. } => elemtype.expect("array element"),
                        _ => unreachable!(),
                    };
                    if ctx.typ(t_elem).is_primitive() {
                        self.is_same_type(ctx, syms, t_elem, s_elem)
                    } else {
                        self.is_subtype_no_capture(ctx, syms, t_elem, s_elem)
                    }
                } else if ctx.typ(s).has_tag(TypeTag::CLASS) {
                    // Java: s.tsym.getQualifiedName() compared with the
                    // java.lang.Object/java.lang.Cloneable/java.io.Serializable
                    // names (Types.java IsSubtype.visitArrayType).
                    let s_name = ctx
                        .typ(s)
                        .tsym
                        .map(|tsym| self.type_qualified_name(ctx, tsym));
                    let names = syms.names();
                    match s_name {
                        Some(name) => {
                            name.to_java_string() == names.std.java_lang_Object.to_java_string()
                                || name.to_java_string()
                                    == names.std.java_lang_Cloneable.to_java_string()
                                || name.to_java_string()
                                    == names.std.java_io_Serializable.to_java_string()
                        }
                        None => false,
                    }
                } else {
                    false
                }
            }
            TypeKindData::MethodType { .. }
            | TypeKindData::PartiallyInferredMethodType { .. }
            | TypeKindData::PackageType
            | TypeKindData::ModuleType => {
                panic!("AssertionError: isSubtype {:?}", ctx.typ(t).get_tag())
            }
            TypeKindData::ForAll { .. } | TypeKindData::WildcardType { .. } => {
                panic!("AssertionError: isSubtype {:?}", ctx.typ(t).get_tag())
            }
        }
    }

    /// Java: public boolean isSuperType(Type t, Type s)
    pub fn is_super_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        match ctx.typ(t).get_tag() {
            TypeTag::ERROR => true,
            TypeTag::UNDETVAR => {
                let qtype = self.undet_var_qtype(ctx, t);
                if t == s
                    || qtype == s
                    || ctx.typ(s).has_tag(TypeTag::ERROR)
                    || ctx.typ(s).has_tag(TypeTag::BOT)
                {
                    return true;
                }
                self.undet_var_add_bound(
                    ctx,
                    syms,
                    t,
                    crate::code::type_::InferenceBound::LOWER,
                    s,
                    false,
                );
                true
            }
            _ => self.is_subtype(ctx, syms, s, t),
        }
    }

    /// Java: public boolean isSameTypes(List<Type> ts, List<Type> ss)
    pub fn is_same_types(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts: &List<TypeId>,
        ss: &List<TypeId>,
    ) -> bool {
        let mut ts = ts.clone();
        let mut ss = ss.clone();
        while ts.non_empty()
            && ss.non_empty()
            && self.is_same_type(ctx, syms, *ts.head(), *ss.head())
        {
            ts = ts.tail();
            ss = ss.tail();
        }
        ts.is_empty() && ss.is_empty()
    }


    /// Java: public boolean hasSameBounds(ForAll t, ForAll s) (the bounds slice
    /// form used by isSameType's ForAll branch).
    fn has_same_bounds(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t_tvars: &List<TypeId>,
        s_tvars: &List<TypeId>,
    ) -> bool {
        let mut l1 = t_tvars.clone();
        let mut l2 = s_tvars.clone();
        while l1.non_empty() && l2.non_empty() {
            let b1 = ctx
                .typ(*l1.head())
                .get_upper_bound()
                .expect("type var upper bound");
            let b2 = ctx
                .typ(*l2.head())
                .get_upper_bound()
                .expect("type var upper bound");
            let substituted = self.subst_type(ctx, syms, b2, s_tvars, t_tvars);
            if !self.is_same_type(ctx, syms, b1, substituted) {
                return false;
            }
            l1 = l1.tail();
            l2 = l2.tail();
        }
        l1.is_empty() && l2.is_empty()
    }

    /// Java: public boolean isReifiable(Type t)
    ///
    /// PORTING_RULES: upstream isReifiable takes only Type; syms is threaded
    /// through recursion for the Rust arena/context API (expression adapter).
    #[allow(clippy::only_used_in_recursion)]
    pub fn is_reifiable(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> bool {
        let tag = ctx.typ(t).get_tag();
        if tag == TypeTag::CLASS {
            if self.is_compound(ctx, t) {
                return false;
            }
            if !is_parameterized_type(ctx, t) {
                return true;
            }
            for param in allparams_of(ctx, t).iter() {
                if !ctx.typ(param).is_unbound() {
                    return false;
                }
            }
            return true;
        }
        if tag == TypeTag::ARRAY {
            let elem = match &ctx.typ(t).kind {
                TypeKindData::ArrayType { elemtype, .. } => elemtype.expect("array element type"),
                _ => unreachable!(),
            };
            return self.is_reifiable(ctx, syms, elem);
        }
        if tag == TypeTag::TYPEVAR {
            return false;
        }
        true
    }

    /// Java: public int rank(Type t)
    ///
    /// PORTING_RULES: the upstream rank_field caches (Type, not ported)
    /// are not used; the rank is recomputed each time (same result).
    pub fn rank(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> i32 {
        let tag = ctx.typ(t).get_tag();
        if tag == TypeTag::CLASS {
            let tsym = ctx.typ(t).tsym.expect("class type symbol");
            let names = syms.names();
            let name = flat_name_of(ctx, tsym);
            if name.to_java_string() == names.std.java_lang_Object.to_java_string() {
                return 0;
            }
            let sup = self.supertype(ctx, syms, t);
            let mut r = self.rank(ctx, syms, sup);
            for l in self.interfaces(ctx, syms, t).iter() {
                let rank_l = self.rank(ctx, syms, l);
                if rank_l > r {
                    r = rank_l;
                }
            }
            return r + 1;
        }
        if tag == TypeTag::TYPEVAR {
            let sup = self.supertype(ctx, syms, t);
            let mut r = self.rank(ctx, syms, sup);
            for l in self.interfaces(ctx, syms, t).iter() {
                let rank_l = self.rank(ctx, syms, l);
                if rank_l > r {
                    r = rank_l;
                }
            }
            return r + 1;
        }
        if tag == TypeTag::ERROR || tag == TypeTag::NONE {
            return 0;
        }
        panic!("AssertionError: rank {:?}", ctx.typ(t).get_tag())
    }

    /// Java: public boolean isUnbounded(Type t)
    pub fn is_unbounded(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> bool {
        let is_class_family = matches!(
            &ctx.typ(t).kind,
            TypeKindData::ClassType { .. }
                | TypeKindData::ErasedClassType { .. }
                | TypeKindData::UnionClassType { .. }
                | TypeKindData::IntersectionClassType { .. }
        );
        if !is_class_family {
            return true;
        }
        let tsym = ctx.typ(t).tsym.expect("class type symbol");
        let tsym_type = ctx.sym(tsym).ty.expect("symbol type");
        let mut parms = allparams_of(ctx, tsym_type);
        let mut args = allparams_of(ctx, t);
        while parms.non_empty() {
            let bound = *parms.head();
            let unb = new_wildcard_type(
                ctx,
                Some(syms.object_type),
                BoundKind::UNBOUND,
                Some(syms.bound_class),
                Some(bound),
            );
            if !self.contains_type(ctx, syms, *args.head(), unb) {
                return false;
            }
            parms = parms.tail();
            args = args.tail();
        }
        true
    }

    /// Java: public Type cvarUpperBound(Type t) (Types.java)
    pub fn cvar_upper_bound(&self, ctx: &mut CompilerContext, t: TypeId) -> TypeId {
        if ctx.typ(t).has_tag(TypeTag::TYPEVAR) {
            let captured = ctx.typ(t).is_captured();
            if captured {
                let bound = match &ctx.typ(t).kind {
                    crate::code::type_::TypeKindData::TypeVar { bound, .. } => *bound,
                    _ => None,
                }
                .expect("type variable bound");
                return self.cvar_upper_bound(ctx, bound);
            }
            return t;
        }
        t
    }

    /// Java: public Type elemtype(Type t) (Types.java)
    pub fn elemtype(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
        match ctx.typ(t).get_tag() {
            TypeTag::WILDCARD => {
                let upper = self.wild_upper_bound(ctx, syms, t);
                self.elemtype(ctx, syms, upper)
            }
            TypeTag::ARRAY => match &ctx.typ(t).kind {
                crate::code::type_::TypeKindData::ArrayType { elemtype, .. } => {
                    elemtype.expect("array element type")
                }
                _ => unreachable!(),
            },
            TypeTag::FORALL => {
                let qtype = match &ctx.typ(t).kind {
                    crate::code::type_::TypeKindData::ForAll { qtype, .. } => *qtype,
                    _ => None,
                }
                .expect("forall qtype");
                self.elemtype(ctx, syms, qtype)
            }
            _ => t,
        }
    }

    /// Java: HasSameArgs(strict).visit(t, s) (Types.java:2810-2860)
    pub fn has_same_args_strict(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        strict: bool,
    ) -> bool {
        match &ctx.typ(t).kind {
            TypeKindData::MethodType { .. } => {
                ctx.typ(s).has_tag(TypeTag::METHOD)
                    && self.contains_type_equivalent(
                        ctx,
                        syms,
                        &get_parameter_types(ctx, t),
                        &get_parameter_types(ctx, s),
                    )
            }
            TypeKindData::ErrorType { .. } => false,
            TypeKindData::ForAll { .. } => {
                let (t_tvars, t_qtype) = match &ctx.typ(t).kind {
                    TypeKindData::ForAll { tvars, qtype } => (tvars.clone(), *qtype),
                    _ => unreachable!(),
                };
                if !ctx.typ(s).has_tag(TypeTag::FORALL) {
                    return if strict {
                        false
                    } else {
                        let t_method = t_qtype.expect("for-all qtype");
                        self.has_same_args_strict(ctx, syms, t_method, s, strict)
                    };
                }
                let (s_tvars, s_qtype) = match &ctx.typ(s).kind {
                    TypeKindData::ForAll { tvars, qtype } => (tvars.clone(), *qtype),
                    _ => unreachable!(),
                };
                let s_qtype = s_qtype.expect("for-all qtype");
                self.has_same_bounds(ctx, syms, &t_tvars, &s_tvars)
                    && {
                        let substituted = self.subst_type(ctx, syms, s_qtype, &s_tvars, &t_tvars);
                        let t_qtype = t_qtype.expect("for-all qtype");
                        self.has_same_args_strict(ctx, syms, t_qtype, substituted, strict)
                    }
            }
            _ => panic!("AssertionError: hasSameArgs {:?}", ctx.typ(t).get_tag()),
        }
    }

    /// Java: public boolean isSubSignature(Type t, Type s) —
    /// hasSameArgs(t, s) || hasSameArgs(t, erasure(s)) (Types.java:2799).
    pub fn is_sub_signature(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        if self.has_same_args(ctx, syms, t, s) {
            return true;
        }
        let erased = self.erasure(ctx, syms, s);
        self.has_same_args(ctx, syms, t, erased)
    }

    /// Java: public boolean isSubSignature(Type t, Type s, boolean strict) —
    /// TODO(Types): the strict=false HasSameArgs variant is not ported.
    pub fn is_sub_signature_strict(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        strict: bool,
    ) -> bool {
        if self.has_same_args_strict(ctx, syms, t, s, strict) {
            return true;
        }
        let erased = self.erasure(ctx, syms, s);
        self.has_same_args_strict(ctx, syms, t, erased, strict)
    }
    /// Java: public boolean isConvertible(Type t, Type s)
    /// Java: public boolean Type.contains(Type t) — the base Type implementation
    /// is equalsIgnoreMetadata (the port maps it to isSameType); WildcardType
    /// overrides it (kind != UNBOUND && type.contains(t)).
    pub fn contains(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        if let crate::code::type_::TypeKindData::WildcardType {
            type_, kind, ..
        } = &ctx.typ(t).kind
        {
            if *kind != crate::code::bound_kind::BoundKind::UNBOUND {
                let inner = type_.expect("wildcard bound type");
                return self.contains(ctx, syms, inner, s);
            }
            return false;
        }
        self.is_same_type(ctx, syms, t, s)
    }

    /// Java: public boolean Type.containsAny(List<Type> ts)
    pub fn contains_any(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        ts: &List<TypeId>,
    ) -> bool {
        let mut current = ts.clone();
        while current.non_empty() {
            let s = *current.head();
            if self.contains(ctx, syms, t, s) {
                return true;
            }
            current = current.tail();
        }
        false
    }

    /// Java: public Type Type.withTypeVar(Type t) — the base implementation
    /// returns this; WildcardType sets its bound (the port mutates the arena
    /// value and returns the same id, like the upstream in-place mutation).
    pub fn with_type_var(
        &self,
        ctx: &mut CompilerContext,
        t: TypeId,
        tvar: TypeId,
    ) -> TypeId {
        if let crate::code::type_::TypeKindData::WildcardType { bound, .. } =
            &mut ctx.typ_mut(t).kind
        {
            // Java: if (bound == t) return this; bound = (TypeVar)t; return this;
            *bound = Some(tvar);
        }
        t
    }

    /// Java: public boolean isConvertible(Type t, Type s, Warner warn)
    /// (Types.java:601-625)
    pub fn is_convertible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        if ctx.typ(t).has_tag(TypeTag::ERROR) {
            return true;
        }
        let t_primitive = ctx.typ(t).is_primitive();
        let s_primitive = ctx.typ(s).is_primitive();
        if t_primitive == s_primitive {
            return self.is_subtype_unchecked(ctx, syms, t, s);
        }
        let t_undet = ctx.typ(t).has_tag(TypeTag::UNDETVAR);
        let s_undet = ctx.typ(s).has_tag(TypeTag::UNDETVAR);
        if t_undet || s_undet {
            return if t_undet {
                let boxed = self.boxed_type_or_type_ro(ctx, syms, s);
                self.is_subtype(ctx, syms, t, boxed)
            } else {
                let boxed = self.boxed_type_or_type_ro(ctx, syms, t);
                self.is_subtype(ctx, syms, boxed, s)
            };
        }
        if t_primitive {
            let boxed = self.boxed_class_ro(ctx, syms, t);
            let boxed_type = ctx.sym(boxed).ty.expect("boxed type");
            self.is_subtype(ctx, syms, boxed_type, s)
        } else {
            let unboxed = self.unboxed_type_ro(ctx, syms, t);
            self.is_subtype(ctx, syms, unboxed, s)
        }
    }

    /// The read-only counterpart of boxedClass: the boxed class symbols are
    /// pre-entered by Symtab (syms.boxed_class_ids, Porting rules adapter), so relation
    /// code that only holds &Symtab can obtain the symbol exactly like the
    /// upstream syms.enterClass(java_base, boxedName) call.
    pub fn boxed_class_ro(
        &self,
        ctx: &CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> SymbolId {
        let tag = ctx.typ(t).get_tag();
        syms.boxed_class_ids[tag as usize].unwrap_or_else(|| {
            panic!("AssertionError: boxed class not entered for {:?}", tag)
        })
    }

    /// The read-only counterpart of boxedTypeOrType (Types.java:4312-4317).
    pub fn boxed_type_or_type_ro(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> TypeId {
        if ctx.typ(t).is_primitive() {
            let boxed = self.boxed_class_ro(ctx, syms, t);
            ctx.sym(boxed).ty.expect("boxed type")
        } else {
            t
        }
    }

    /// The read-only counterpart of unboxedType (same upstream body, using the
    /// pre-entered boxed class symbols; Porting rules adapter).
    #[allow(clippy::collapsible_if)]
    pub fn unboxed_type_ro(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> TypeId {
        if ctx.typ(t).has_tag(TypeTag::ERROR) {
            return no_type_of(ctx, syms);
        }
        let boxed_ids = syms.boxed_class_ids.clone();
        for (index, boxed) in boxed_ids.iter().enumerate() {
            if let Some(boxed) = boxed {
                if self.as_super(ctx, syms, t, *boxed).is_some() {
                    return syms.type_of_tag[index].expect("typeOfTag entry");
                }
            }
        }
        no_type_of(ctx, syms)
    }

    /// Java: public Type boxedTypeOrType(Type t) (Types.java:4312-4317)
    pub fn boxed_type_or_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        t: TypeId,
    ) -> TypeId {
        if ctx.typ(t).is_primitive() {
            let boxed = self.boxed_class(ctx, syms, t);
            ctx.sym(boxed).ty.expect("boxed type")
        } else {
            t
        }
    }

    /// Java: public Type unboxedType(Type t) (Types.java:4321-4334)
    pub fn unboxed_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        t: TypeId,
    ) -> TypeId {
        if ctx.typ(t).has_tag(TypeTag::ERROR) {
            return no_type_of(ctx, syms);
        }
        let boxed_names = syms.boxed_name.clone();
        let java_base = syms.java_base;
        let no_symbol = syms.no_symbol;
        for (index, boxed) in boxed_names.iter().enumerate() {
            if let Some(boxed) = boxed {
                let boxed_tsym =
                    syms.enter_class(ctx, java_base, boxed.clone(), Some(no_symbol));
                if self.as_super(ctx, syms, t, boxed_tsym).is_some() {
                    return syms.type_of_tag[index].expect("typeOfTag entry");
                }
            }
        }
        no_type_of(ctx, syms)
    }

    /// Java: public Type unboxedTypeOrType(Type t) (Types.java:4336-4340)
    pub fn unboxed_type_or_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        t: TypeId,
    ) -> TypeId {
        let unboxed = self.unboxed_type(ctx, syms, t);
        if ctx.typ(unboxed).has_tag(TypeTag::NONE) {
            t
        } else {
            unboxed
        }
    }

    /// Java: public boolean isSubtypeUnchecked(Type t, Type s)
    pub fn is_subtype_unchecked(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        self.is_subtype_unchecked_internal(ctx, syms, t, s, true)
    }

    /// Java: private boolean isSubtypeUncheckedInternal(Type t, Type s, boolean
    /// capture, Warner warn)
    ///
    /// PORTING_RULES: the lint warnings (Warner/LintCategory.UNCHECKED) are not
    /// emitted here (the lint wiring is TODO with the Attr); the boolean
    /// relation is 1:1.
    fn is_subtype_unchecked_internal(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        capture: bool,
    ) -> bool {
        if ctx.typ(t).has_tag(TypeTag::ARRAY) && ctx.typ(s).has_tag(TypeTag::ARRAY) {
            // Java: isSameType(elemtype(t), elemtype(s)) /
            // isSubtypeUncheckedInternal(elemtype(t), elemtype(s), false, warn)
            let t_elem = elemtype_of(ctx, syms, t);
            let s_elem = elemtype_of(ctx, syms, s);
            if ctx.typ(t_elem).is_primitive() {
                return self.is_same_type(ctx, syms, t_elem, s_elem);
            }
            return self.is_subtype_unchecked_internal(ctx, syms, t_elem, s_elem, false);
        }
        if self.is_subtype_with_capture(ctx, syms, t, s, capture) {
            return true;
        }
        if ctx.typ(t).has_tag(TypeTag::TYPEVAR) {
            let upper = ctx.typ(t).get_upper_bound().expect("type var upper bound");
            return self.is_subtype_unchecked_internal(ctx, syms, upper, s, false);
        }
        if !is_raw_type(ctx, s) {
            let Some(s_tsym) = ctx.typ(s).tsym else {
                return false;
            };
            if let Some(t2) = self.as_super(ctx, syms, t, s_tsym)
                && is_raw_type(ctx, t2)
            {
                return true;
            }
        }
        false
    }

    /// Java: private Type skipTypeVars(Type site, boolean capture)
    fn skip_type_vars(&self, ctx: &mut CompilerContext, site: TypeId) -> TypeId {
        let mut site = site;
        while ctx.typ(site).has_tag(TypeTag::TYPEVAR) {
            site = ctx.typ(site).get_upper_bound().expect("type var upper bound");
        }
        site
    }

    /// Java: public boolean isCastable(Type t, Type s) (Types.java:1631-1633)
    pub fn is_castable(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        let mut no_warnings = crate::util::warner::Warner::new(None);
        self.is_castable_with_warn(ctx, syms, t, s, &mut no_warnings)
    }

    /// Java: public boolean isCastable(Type t, Type s, Warner warn)
    /// (Types.java:1640-1670)
    ///
    /// PORTING_RULES Porting rules adapter: the upstream TypeRelation.warnStack (a stack
    /// pushed around visit) is expressed by passing the Warner explicitly; the
    /// recursive calls that upstream makes with noWarnings construct a local
    /// Warner (noWarnings has no observable state). checkUnsafeVarargsConversion
    /// (Types.java:1663) is not ported yet (Varargs, registered).
    pub fn is_castable_with_warn(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        warn: &mut crate::util::warner::Warner,
    ) -> bool {
        if t == s {
            return true;
        }
        if ctx.typ(t).is_primitive() != ctx.typ(s).is_primitive() {
            let t = self.skip_type_vars(ctx, t);
            if self.is_convertible(ctx, syms, t, s) {
                return true;
            }
            if ctx.typ(s).is_primitive() {
                let boxed = self.boxed_class_ro(ctx, syms, s);
                let boxed_type = ctx.sym(boxed).ty.expect("boxed type");
                return self.is_subtype(ctx, syms, boxed_type, t);
            }
            return false;
        }
        // Java: if (warn != warnStack.head) { push; checkUnsafeVarargsConversion;
        // result = isCastable.visit(t,s); pop; } else visit(t,s)
        let result = self.is_castable_visit(ctx, syms, t, s, warn);
        if result && ctx.typ(t).has_tag(TypeTag::CLASS) && ctx.typ(s).has_tag(TypeTag::CLASS) {
            let tsym = ctx.typ(t).tsym.expect("class type symbol");
            let ssym = ctx.typ(s).tsym.expect("class type symbol");
            if symbol_is_sealed(ctx, tsym) || symbol_is_sealed(ctx, ssym) {
                return if self.is_compound(ctx, t) || self.is_compound(ctx, s) {
                    true
                } else {
                    !self.are_disjoint(ctx, syms, tsym, ssym)
                };
            }
        }
        result
    }

    /// Java: private boolean isCastable.visit(t, s) (the TypeRelation dispatch).
    fn is_castable_visit(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        warn: &mut crate::util::warner::Warner,
    ) -> bool {
        match &ctx.typ(t).kind {
            TypeKindData::JCPrimitiveType
            | TypeKindData::JCVoidType
            | TypeKindData::BottomType
            | TypeKindData::JCNoType
            | TypeKindData::UnknownType => {
                if ctx.typ(s).has_tag(TypeTag::ERROR) || ctx.typ(t).has_tag(TypeTag::NONE) {
                    return true;
                }
                match ctx.typ(t).get_tag() {
                    TypeTag::BYTE
                    | TypeTag::CHAR
                    | TypeTag::SHORT
                    | TypeTag::INT
                    | TypeTag::LONG
                    | TypeTag::FLOAT
                    | TypeTag::DOUBLE => ctx.typ(s).is_numeric(),
                    TypeTag::BOOLEAN => ctx.typ(s).has_tag(TypeTag::BOOLEAN),
                    TypeTag::VOID => false,
                    TypeTag::BOT => self.is_subtype(ctx, syms, t, s),
                    _ => panic!("AssertionError: isCastable {:?}", ctx.typ(t).get_tag()),
                }
            }
            TypeKindData::WildcardType { .. } => {
                let upper = self.wild_upper_bound(ctx, syms, t);
                self.is_castable_with_warn(ctx, syms, upper, s, warn)
            }
            TypeKindData::TypeVar { .. } | TypeKindData::CapturedType { .. } => {
                let tag = ctx.typ(s).get_tag();
                if tag == TypeTag::ERROR || tag == TypeTag::BOT {
                    return true;
                }
                let upper = ctx.typ(t).get_upper_bound().expect("type var upper bound");
                if tag == TypeTag::TYPEVAR {
                    if self.is_subtype(ctx, syms, t, s) {
                        return true;
                    }
                    // Java: isCastable(t.getUpperBound(), s, noWarnings) then warn.
                    let mut no_warnings = crate::util::warner::Warner::new(None);
                    if self.is_castable_with_warn(ctx, syms, upper, s, &mut no_warnings) {
                        warn.warn(crate::code::lint::LintCategory::UNCHECKED);
                        return true;
                    }
                    return false;
                }
                self.is_castable_with_warn(ctx, syms, upper, s, warn)
            }
            TypeKindData::ArrayType { .. } => {
                let tag = ctx.typ(s).get_tag();
                if tag == TypeTag::ERROR || tag == TypeTag::BOT {
                    return true;
                }
                if tag == TypeTag::TYPEVAR {
                    // Java: isCastable(s, t, noWarnings) then warn.
                    let mut no_warnings = crate::util::warner::Warner::new(None);
                    if self.is_castable_with_warn(ctx, syms, s, t, &mut no_warnings) {
                        warn.warn(crate::code::lint::LintCategory::UNCHECKED);
                        return true;
                    }
                    return false;
                }
                if tag == TypeTag::CLASS {
                    return self.is_subtype(ctx, syms, t, s);
                }
                if tag == TypeTag::ARRAY {
                    let t_elem = match &ctx.typ(t).kind {
                        TypeKindData::ArrayType { elemtype, .. } => elemtype.expect("array element"),
                        _ => unreachable!(),
                    };
                    let s_elem = match &ctx.typ(s).kind {
                        TypeKindData::ArrayType { elemtype, .. } => elemtype.expect("array element"),
                        _ => unreachable!(),
                    };
                    if ctx.typ(t_elem).is_primitive() || ctx.typ(s_elem).is_primitive() {
                        return ctx.typ(t_elem).has_tag(ctx.typ(s_elem).get_tag());
                    }
                    return self.is_castable_visit(ctx, syms, t_elem, s_elem, warn);
                }
                false
            }
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
                let tag = ctx.typ(s).get_tag();
                if tag == TypeTag::ERROR || tag == TypeTag::BOT {
                    return true;
                }
                if tag == TypeTag::TYPEVAR {
                    // Java: if (isCastable(t, s.getUpperBound(), noWarnings)) {
                    // warnStack.head.warn(UNCHECKED); return true; } else false;
                    let upper = ctx.typ(s).get_upper_bound().expect("type var upper bound");
                    let mut no_warnings = crate::util::warner::Warner::new(None);
                    if self.is_castable_with_warn(ctx, syms, t, upper, &mut no_warnings) {
                        warn.warn(crate::code::lint::LintCategory::UNCHECKED);
                        return true;
                    }
                    return false;
                }
                if self.is_compound(ctx, t) || self.is_compound(ctx, s) {
                    return if !self.is_compound(ctx, t) {
                        self.visit_compound_type(ctx, syms, s, t, true, warn)
                    } else {
                        self.visit_compound_type(ctx, syms, t, s, false, warn)
                    };
                }
                if tag == TypeTag::CLASS || tag == TypeTag::ARRAY {
                    let t_erasure = self.erasure(ctx, syms, t);
                    let s_erasure = self.erasure(ctx, syms, s);
                    let upcast = self.is_subtype(ctx, syms, t_erasure, s_erasure);
                    if upcast || self.is_subtype(ctx, syms, s_erasure, t_erasure) {
                        if !upcast && tag == TypeTag::ARRAY {
                            if !self.is_reifiable(ctx, syms, s) {
                                warn.warn(crate::code::lint::LintCategory::UNCHECKED);
                            }
                            return true;
                        } else if is_raw_type(ctx, s) {
                            return true;
                        } else if is_raw_type(ctx, t) {
                            if !self.is_unbounded(ctx, syms, s) {
                                warn.warn(crate::code::lint::LintCategory::UNCHECKED);
                            }
                            return true;
                        }
                        let (a, b) = if upcast { (t, s) } else { (s, t) };
                        let high = true;
                        let low = false;
                        let no_rewrite = false;
                        let mut a_high =
                            self.rewrite_quantifiers(ctx, syms, a, high, no_rewrite);
                        let mut a_low =
                            self.rewrite_quantifiers(ctx, syms, a, low, no_rewrite);
                        let mut b_high =
                            self.rewrite_quantifiers(ctx, syms, b, high, no_rewrite);
                        let mut b_low =
                            self.rewrite_quantifiers(ctx, syms, b, low, no_rewrite);
                        let mut low_sub = {
                            let a_low_tsym = ctx.typ(a_low).tsym.expect("rewritten symbol");
                            self.as_sub(ctx, syms, b_low, a_low_tsym)
                        };
                        let mut high_sub = low_sub.and_then(|low_sub| {
                            let a_high_tsym = ctx.typ(a_high).tsym.expect("rewritten symbol");
                            let _ = low_sub;
                            self.as_sub(ctx, syms, b_high, a_high_tsym)
                        });
                        if high_sub.is_none() {
                            let rewrite = true;
                            a_high = self.rewrite_quantifiers(ctx, syms, a, high, rewrite);
                            a_low = self.rewrite_quantifiers(ctx, syms, a, low, rewrite);
                            b_high = self.rewrite_quantifiers(ctx, syms, b, high, rewrite);
                            b_low = self.rewrite_quantifiers(ctx, syms, b, low, rewrite);
                            low_sub = {
                                let a_low_tsym = ctx.typ(a_low).tsym.expect("rewritten symbol");
                                self.as_sub(ctx, syms, b_low, a_low_tsym)
                            };
                            high_sub = low_sub.and_then(|_unused| {
                                let a_high_tsym = ctx.typ(a_high).tsym.expect("rewritten symbol");
                                self.as_sub(ctx, syms, b_high, a_high_tsym)
                            });
                        }
                        if let (Some(low_sub), Some(high_sub)) = (low_sub, high_sub) {
                            let a_high_params = self.class_allparams(ctx, a_high);
                            let a_low_params = self.class_allparams(ctx, a_low);
                            let high_params = self.class_allparams(ctx, high_sub);
                            let low_params = self.class_allparams(ctx, low_sub);
                            let disjoint = self.disjoint_types(
                                ctx,
                                syms,
                                a_high_params.clone(),
                                high_params.clone(),
                            ) || self.disjoint_types(
                                ctx,
                                syms,
                                a_high_params,
                                low_params.clone(),
                            ) || self.disjoint_types(
                                ctx,
                                syms,
                                a_low_params.clone(),
                                high_params,
                            ) || self.disjoint_types(
                                ctx,
                                syms,
                                a_low_params,
                                low_params,
                            );
                            if !disjoint {
                                let give_warning = if upcast {
                                    self.give_warning(ctx, syms, a, b)
                                } else {
                                    self.give_warning(ctx, syms, b, a)
                                };
                                if give_warning {
                                    warn.warn(crate::code::lint::LintCategory::UNCHECKED);
                                }
                                return true;
                            }
                        }
                        if self.is_reifiable(ctx, syms, s) {
                            return self.is_subtype_unchecked(ctx, syms, a, b);
                        }
                        // Java: isSubtypeUnchecked(a, b, warn) — the warn parameter
                        // is registered with the Varargs (checkUnsafeVarargsConversion
                        // is not ported yet); the boolean relation is unchanged.
                        return self.is_subtype_unchecked(ctx, syms, a, b);
                    }
                    // Sidecast
                    if tag == TypeTag::CLASS {
                        let s_tsym = ctx.typ(s).tsym.expect("cast target symbol");
                        let t_tsym = ctx.typ(t).tsym.expect("cast source symbol");
                        let s_interface =
                            (ctx.sym(s_tsym).flags_field & crate::code::flags::INTERFACE as i64)
                                != 0;
                        let t_interface =
                            (ctx.sym(t_tsym).flags_field & crate::code::flags::INTERFACE as i64)
                                != 0;
                        let s_final =
                            (ctx.sym(s_tsym).flags_field & crate::code::flags::FINAL as i64) != 0;
                        let t_final =
                            (ctx.sym(t_tsym).flags_field & crate::code::flags::FINAL as i64) != 0;
                        if s_interface {
                            return if !t_final {
                                self.side_cast(ctx, syms, t, s, warn)
                            } else {
                                self.side_cast_final(ctx, syms, t, s, warn)
                            };
                        } else if t_interface {
                            return if !s_final {
                                self.side_cast(ctx, syms, t, s, warn)
                            } else {
                                self.side_cast_final(ctx, syms, t, s, warn)
                            };
                        }
                        return false;
                    }
                }
                false
            }
            TypeKindData::ErrorType { .. } => true,
            TypeKindData::MethodType { .. } | TypeKindData::ForAll { .. } => {
                panic!("AssertionError: isCastable {:?}", ctx.typ(t).get_tag())
            }
            _ => false,
        }
    }

    /// Java: boolean visitCompoundType(ClassType ct, Type s, boolean reverse)
    /// (Types.java:1790-1803)
    fn visit_compound_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ct: TypeId,
        s: TypeId,
        reverse: bool,
        warn: &mut crate::util::warner::Warner,
    ) -> bool {
        // Java: Warner warn = noWarnings; (per-component cleared)
        let mut compound_warn = crate::util::warner::Warner::new(None);
        for c in self.direct_supertypes(ctx, syms, ct).iter() {
            compound_warn.clear();
            let ok = if reverse {
                self.is_castable_with_warn(ctx, syms, s, c, &mut compound_warn)
            } else {
                self.is_castable_with_warn(ctx, syms, c, s, &mut compound_warn)
            };
            if !ok {
                return false;
            }
        }
        if compound_warn.has_lint(crate::code::lint::LintCategory::UNCHECKED) {
            warn.warn(crate::code::lint::LintCategory::UNCHECKED);
        }
        true
    }

    /// Java: private boolean areDisjoint(ClassSymbol ts, ClassSymbol ss)
    fn are_disjoint(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts: SymbolId,
        ss: SymbolId,
    ) -> bool {
        let ts_type = ctx.sym(ts).ty.expect("class type");
        let ss_type = ctx.sym(ss).ty.expect("class type");
        let ts_erasure = self.erasure(ctx, syms, ts_type);
        let ss_erasure = self.erasure(ctx, syms, ss_type);
        if self.is_subtype(ctx, syms, ts_erasure, ss_erasure) {
            return false;
        }
        let ts_interface = symbol_is_interface(ctx, ts);
        let ss_interface = symbol_is_interface(ctx, ss);
        if ts_interface == ss_interface && self.is_subtype(ctx, syms, ss_erasure, ts_erasure) {
            return false;
        }
        if ts_interface && !ss_interface {
            return self.are_disjoint(ctx, syms, ss, ts);
        }
        if !ts_interface && symbol_is_final(ctx, ts) {
            return true;
        }
        if symbol_is_sealed(ctx, ts) || symbol_is_sealed(ctx, ss) {
            let sealed_one = if symbol_is_sealed(ctx, ts) { ts } else { ss };
            let other = if sealed_one == ts { ss } else { ts };
            let permitted = match &ctx.sym(sealed_one).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { permitted, .. } => permitted.clone(),
                _ => List::nil(),
            };
            for sub in permitted.iter() {
                if !self.are_disjoint(ctx, syms, sub, other) {
                    return false;
                }
            }
            return true;
        }
        false
    }
}

/// Java: BoundKind.hashCode() (an enum identity hash upstream; the port uses the
/// declaration order as a stable equivalent, registered).
fn bound_kind_ordinal(kind: BoundKind) -> i32 {
    match kind {
        BoundKind::EXTENDS => 0,
        BoundKind::SUPER => 1,
        BoundKind::UNBOUND => 2,
    }
}

/// Java: System.identityHashCode(t) equivalent (stable arena index; registered).
fn type_identity_hash(t: TypeId) -> i32 {
    t.index() as i32
}

/// Java: Type.getEnclosingType() (ClassType: outer_field; other types: null).
///
/// PORTING_RULES: reproduced here because type_.rs has not ported getEnclosingType
/// yet (TODO Type; registered).
fn get_enclosing_type(ctx: &CompilerContext, t: TypeId) -> Option<TypeId> {
    match &ctx.typ(t).kind {
        TypeKindData::ClassType { outer_field, .. }
        | TypeKindData::ErasedClassType { outer_field, .. }
        | TypeKindData::UnionClassType { outer_field, .. }
        | TypeKindData::IntersectionClassType { outer_field, .. }
        | TypeKindData::ErrorType { outer_field, .. } => *outer_field,
        _ => None,
    }
}

/// Java: Type.getTypeArguments() (ClassType: typarams_field, lazily nil after
/// complete(); other types: List.nil()).
///
/// PORTING_RULES: the upstream ClassType.getTypeArguments() calls complete()
/// before defaulting typarams_field to List.nil(); the Symbol completer is not
/// ported, so the nil default is used (TODO Symbol; registered).
pub(crate) fn get_type_arguments(ctx: &CompilerContext, t: TypeId) -> List<TypeId> {
    match &ctx.typ(t).kind {
        TypeKindData::ClassType { typarams_field, .. }
        | TypeKindData::ErasedClassType { typarams_field, .. }
        | TypeKindData::UnionClassType { typarams_field, .. }
        | TypeKindData::IntersectionClassType { typarams_field, .. }
        | TypeKindData::ErrorType { typarams_field, .. } => {
            typarams_field.clone().unwrap_or_else(List::nil)
        }
        _ => List::nil(),
    }
}

/// Java: Type.getParameterTypes() (MethodType: argtypes; other types: List.nil()).
///
/// PORTING_RULES: reproduced here because type_.rs has not ported it yet (TODO
/// Type; registered).
pub(crate) fn get_parameter_types(ctx: &CompilerContext, t: TypeId) -> List<TypeId> {
    match &ctx.typ(t).kind {
        TypeKindData::MethodType { argtypes, .. } => argtypes.clone(),
        _ => List::nil(),
    }
}

/// Java: Type.getReturnType() (MethodType: restype; other types: null).
///
/// PORTING_RULES: reproduced here (TODO Type; registered).
fn get_return_type(ctx: &CompilerContext, t: TypeId) -> TypeId {
    match &ctx.typ(t).kind {
        TypeKindData::MethodType { restype, .. } => restype.expect("method return type"),
        _ => panic!("unimplemented: getReturnType (TODO Type batch)"),
    }

}


// ---------------------------------------------------------------------------
// Types helpers.
//
// PORTING_RULES: upstream these helpers are methods of Type.java
// (getThrownTypes/allparams/isParameterized/isRaw/hasErasedSupertypes/
// getSuperclass/getInterfaces/classBound/cvarUpperBound/cvarLowerBound/elemtype)
// or of Types.java (classBound/isUnbounded); they are reproduced here as private
// helpers because type_.rs/symbol.rs have not ported them yet (TODO Type/Symbol
// ; registered). Symbol.complete() is not ported, so the lazy completion
// calls are skipped (fields are used as-is); a null supertype_field/
// interfaces_field is returned as "not computed yet".
// ---------------------------------------------------------------------------

/// Java: Type.noType for this symbol table (the type of syms.noSymbol).
fn no_type_of(ctx: &CompilerContext, syms: &Symtab) -> TypeId {
    ctx.sym(syms.no_symbol).ty.expect("no type")
}

/// Java: Type.allparams() (ClassType: getTypeArguments().prependList(
/// getEnclosingType().allparams()); ArrayType: elemtype.allparams(); other
/// types: List.nil()).
pub(crate) fn allparams_of(ctx: &CompilerContext, t: TypeId) -> List<TypeId> {
    match &ctx.typ(t).kind {
        TypeKindData::ClassType { outer_field, .. }
        | TypeKindData::ErasedClassType { outer_field, .. }
        | TypeKindData::UnionClassType { outer_field, .. }
        | TypeKindData::IntersectionClassType { outer_field, .. }
        | TypeKindData::ErrorType { outer_field, .. } => {
            let params = get_type_arguments(ctx, t);
            match outer_field {
                Some(outer) => params.prepend_list(&allparams_of(ctx, *outer)),
                None => params,
            }
        }
        TypeKindData::ArrayType { elemtype: Some(elem), .. } => allparams_of(ctx, *elem),
        TypeKindData::ArrayType { elemtype: None, .. } => List::nil(),
        _ => List::nil(),
    }
}

/// Java: Type.isParameterized() (ClassType: allparams().tail != null).
fn is_parameterized_type(ctx: &CompilerContext, t: TypeId) -> bool {
    // Java: ClassType.isParameterized() is allparams().tail != null
    // (Type.java:1116-1121); in javac List.nil() is a non-null EMPTY_LIST whose
    // tail is null, so the predicate is "the allparams list is non-empty".
    match &ctx.typ(t).kind {
        TypeKindData::ClassType {
            allparams_field,
            typarams_field,
            ..
        }
        | TypeKindData::ErasedClassType {
            allparams_field,
            typarams_field,
            ..
        }
        | TypeKindData::UnionClassType {
            allparams_field,
            typarams_field,
            ..
        }
        | TypeKindData::IntersectionClassType {
            allparams_field,
            typarams_field,
            ..
        } => allparams_field
            .clone()
            .or_else(|| typarams_field.clone())
            .map(|params| params.non_empty())
            .unwrap_or(false),
        _ => false,
    }
}

/// Java: Type.isRaw() (ClassType: this != tsym.type && tsym.type.allparams()
/// non-empty && this.allparams() empty; ArrayType: elemtype.isRaw()).
fn is_raw_type(ctx: &CompilerContext, t: TypeId) -> bool {
    match &ctx.typ(t).kind {
        TypeKindData::ArrayType { elemtype: Some(elem), .. } => is_raw_type(ctx, *elem),
        TypeKindData::ArrayType { elemtype: None, .. } => false,
        TypeKindData::ClassType { .. }
        | TypeKindData::ErasedClassType { .. }
        | TypeKindData::UnionClassType { .. }
        | TypeKindData::IntersectionClassType { .. }
        | TypeKindData::ErrorType { .. } => {
            let Some(tsym) = ctx.typ(t).tsym else {
                return false;
            };
            let Some(tsym_type) = ctx.sym(tsym).ty else {
                return false;
            };
            t != tsym_type
                && allparams_of(ctx, tsym_type).non_empty()
                && allparams_of(ctx, t).is_empty()
        }
        _ => false,
    }
}

/// Java: Type.hasErasedSupertypes() (ErasedClassType: true; ClassType: isRaw()).
fn has_erased_supertypes(ctx: &CompilerContext, t: TypeId) -> bool {
    matches!(&ctx.typ(t).kind, TypeKindData::ErasedClassType { .. })
        || is_raw_type(ctx, t)
}

/// Java: Type.getThrownTypes() (Type.java:549 default -> List.nil();
/// Type.java:1491 MethodType -> thrown; Type.java:1808 ForAll -> qtype).
///
/// Used by the ported Types relations (isSubSignature/checkMethod paths) and
/// by the FunctionDescriptor when it lands (Types.java:769/774/799).
#[allow(dead_code)]
fn get_thrown_types(ctx: &CompilerContext, t: TypeId) -> List<TypeId> {
    match &ctx.typ(t).kind {
        TypeKindData::MethodType { thrown, .. } => thrown.clone(),
        // Java: Type.ForAll.getThrownTypes() { return qtype.getThrownTypes(); }
        TypeKindData::ForAll { qtype, .. } => {
            let qtype = qtype.expect("for-all qtype");
            get_thrown_types(ctx, qtype)
        }
        _ => List::nil(),
    }
}

/// Java: Symbol.isInterface()
fn symbol_is_interface(ctx: &CompilerContext, symbol: SymbolId) -> bool {
    (ctx.sym(symbol).flags() & crate::code::flags::INTERFACE as i64) != 0
}

/// Java: Symbol.isFinal()
fn symbol_is_final(ctx: &CompilerContext, symbol: SymbolId) -> bool {
    (ctx.sym(symbol).flags() & crate::code::flags::FINAL as i64) != 0
}

/// Java: Symbol.isSealed()
fn symbol_is_sealed(ctx: &CompilerContext, symbol: SymbolId) -> bool {
    (ctx.sym(symbol).flags() & crate::code::flags::SEALED) != 0
}

/// Java: Symbol.ClassSymbol.getSuperclass() (Symbol.complete() is not ported: a
/// null supertype_field is returned as None; an interface has no superclass).
fn class_symbol_get_superclass(ctx: &CompilerContext, symbol: SymbolId) -> Option<TypeId> {
    let tsym_type = ctx.sym(symbol).ty?;
    match &ctx.typ(tsym_type).kind {
        TypeKindData::ClassType { supertype_field, .. }
        | TypeKindData::ErasedClassType { supertype_field, .. }
        | TypeKindData::UnionClassType { supertype_field, .. }
        | TypeKindData::IntersectionClassType { supertype_field, .. } => {
            if symbol_is_interface(ctx, symbol) {
                None
            } else {
                *supertype_field
            }
        }
        _ => None,
    }
}

/// Java: Symbol.ClassSymbol.getInterfaces() (Symbol.complete() is not ported; a
/// null interfaces_field is returned as an empty list).
fn class_symbol_get_interfaces(ctx: &CompilerContext, symbol: SymbolId) -> List<TypeId> {
    let Some(tsym_type) = ctx.sym(symbol).ty else {
        return List::nil();
    };
    match &ctx.typ(tsym_type).kind {
        TypeKindData::ClassType { interfaces_field, .. }
        | TypeKindData::ErasedClassType { interfaces_field, .. }
        | TypeKindData::UnionClassType { interfaces_field, .. }
        | TypeKindData::IntersectionClassType { interfaces_field, .. } => {
            interfaces_field.clone().unwrap_or_else(List::nil)
        }
        _ => List::nil(),
    }
}

/// Java: Types.classBound(Type t)
fn class_bound(ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
    match &ctx.typ(t).kind {
        TypeKindData::ClassType { outer_field, .. }
        | TypeKindData::ErasedClassType { outer_field, .. }
        | TypeKindData::UnionClassType { outer_field, .. }
        | TypeKindData::IntersectionClassType { outer_field, .. }
        | TypeKindData::ErrorType { outer_field, .. } => {
            let Some(outer) = *outer_field else {
                return t;
            };
            let outer1 = class_bound(ctx, syms, outer);
            if outer1 == outer {
                t
            } else {
                let args = get_type_arguments(ctx, t);
                let tsym = ctx.typ(t).tsym;
                new_class_type(ctx, Some(outer1), Some(args), tsym)
            }
        }
        TypeKindData::TypeVar { .. } => {
            let sup = Types::new().supertype(ctx, syms, t);
            class_bound(ctx, syms, sup)
        }
        _ => t,
    }
}

/// Java: Types.elemtype(Type t)
fn elemtype_of(ctx: &CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
    match &ctx.typ(t).kind {
        TypeKindData::WildcardType { .. } => {
            let upper = Types::new().wild_upper_bound(ctx, syms, t);
            elemtype_of(ctx, syms, upper)
        }
        TypeKindData::ArrayType { elemtype, .. } => elemtype.expect("array element type"),
        TypeKindData::ForAll { qtype, .. } => {
            let qtype = qtype.expect("for-all qtype");
            elemtype_of(ctx, syms, qtype)
        }
        TypeKindData::ErrorType { .. } => t,
        _ => panic!("unimplemented: elemtype of a non-array type"),
    }
}

/// Java: Types.cvarUpperBound(Type t)
///
/// Upstream definition: Types.java:149-156; used by notSoftSubtype
/// (Types.java:2007) and the isCastable/disjointType relations (ported).
#[allow(dead_code)]
fn cvar_upper_bound(ctx: &CompilerContext, t: TypeId) -> TypeId {
    if ctx.typ(t).has_tag(TypeTag::TYPEVAR) {
        if ctx.typ(t).is_captured() {
            let upper = ctx.typ(t).get_upper_bound().expect("captured type upper bound");
            cvar_upper_bound(ctx, upper)
        } else {
            t
        }
    } else {
        t
    }
}

/// Java: Types.cvarLowerBound(Type t)
fn cvar_lower_bound(ctx: &CompilerContext, t: TypeId) -> TypeId {
    if ctx.typ(t).has_tag(TypeTag::TYPEVAR) && ctx.typ(t).is_captured() {
        let lower = ctx.typ(t).get_lower_bound().expect("captured type lower bound");
        cvar_lower_bound(ctx, lower)
    } else {
        t
    }
}

/// Java: new Type.ClassType(outer, typarams, tsym)
fn new_class_type(
    ctx: &mut CompilerContext,
    outer: Option<TypeId>,
    typarams: Option<List<TypeId>>,
    tsym: Option<SymbolId>,
) -> TypeId {
    ctx.alloc_type(Type::new(
        TypeTag::CLASS,
        tsym,
        TypeKindData::ClassType {
            outer_field: outer,
            typarams_field: typarams,
            allparams_field: None,
            supertype_field: None,
            interfaces_field: None,
            all_interfaces_field: None,
        },
    ))
}

/// Java: new Type.WildcardType(type, kind, tsym, bound)
fn new_wildcard_type(
    ctx: &mut CompilerContext,
    type_: Option<TypeId>,
    kind: BoundKind,
    tsym: Option<SymbolId>,
    bound: Option<TypeId>,
) -> TypeId {
    ctx.alloc_type(Type::new(
        TypeTag::WILDCARD,
        tsym,
        TypeKindData::WildcardType { type_, kind, bound },
    ))
}

/// Java: new Type.ArrayType(elemtype, tsym)
fn new_array_type(ctx: &mut CompilerContext, elemtype: TypeId, tsym: Option<SymbolId>) -> TypeId {
    ctx.alloc_type(Type::new(
        TypeTag::ARRAY,
        tsym,
        TypeKindData::ArrayType {
            elemtype: Some(elemtype),
                varargs: false,
        },
    ))
}

/// Java: new Type.MethodType(argtypes, restype, thrown, tsym)
fn new_method_type_2(
    ctx: &mut CompilerContext,
    argtypes: List<TypeId>,
    restype: Option<TypeId>,
    thrown: List<TypeId>,
    tsym: Option<SymbolId>,
) -> TypeId {
    ctx.alloc_type(Type::new(
        TypeTag::METHOD,
        tsym,
        TypeKindData::MethodType {
            argtypes,
            restype,
            thrown,
            recvtype: None,
        },
    ))
}

/// Java: new Type.TypeVar(tsym, bound, lower)
///
/// Used by substBounds/newInstances (Types.java:3427: new TypeVar(t.tsym,
/// null, syms.botType, ...)); ported with the Types relations.
#[allow(dead_code)]
fn new_type_var_type(
    ctx: &mut CompilerContext,
    tsym: Option<SymbolId>,
    bound: Option<TypeId>,
    lower: Option<TypeId>,
) -> TypeId {
    ctx.alloc_type(Type::new(
        TypeTag::TYPEVAR,
        tsym,
        TypeKindData::TypeVar { bound, lower },
    ))
}

/// Java: new Type.CapturedType(name, owner, bound, lower, wildcard)
///
/// Used by capture()/freshTypeVariables (Types.java:4465: new CapturedType
/// (capturedName, syms.noSymbol, bound, syms.botType, wildcard)); ported with
/// the Types relations.
#[allow(dead_code)]
fn new_captured_type(
    ctx: &mut CompilerContext,
    tsym: Option<SymbolId>,
    bound: Option<TypeId>,
    lower: Option<TypeId>,
    wildcard: Option<TypeId>,
) -> TypeId {
    ctx.alloc_type(Type::new(
        TypeTag::TYPEVAR,
        tsym,
        TypeKindData::CapturedType {
            bound,
            lower,
            wildcard,
        },
    ))
}

/// Java: Type.equalsIgnoreMetadata -> Type.equals (identity except ArrayType).
fn type_equals_ignore_metadata(ctx: &CompilerContext, t: TypeId, s: TypeId) -> bool {
    Types::new().equals_ignore_metadata(ctx, t, s)
}

/// Java: Type.contains(elem) (Type.java:1148/1408/1504/1854 overrides; the
/// upstream uses are in Attr and other not-yet-ported callers, not Types.java).
///
/// TODO(Attr/Type.contains): not wired yet. Keep until that wires
/// it; do not delete.
#[allow(dead_code)]
fn type_contains(ctx: &mut CompilerContext, syms: &Symtab, t: TypeId, elem: TypeId) -> bool {
    if type_equals_ignore_metadata(ctx, t, elem) {
        return true;
    }
    if is_parameterized_type(ctx, t) {
        if let Some(outer) = enclosing_of(ctx, t)
            && type_contains(ctx, syms, outer, elem)
        {
            return true;
        }
        for arg in get_type_arguments(ctx, t).iter() {
            if type_contains(ctx, syms, arg, elem) {
                return true;
            }
        }
    }
    if Types::new().is_compound(ctx, t) {
        let sup = Types::new().supertype(ctx, syms, t);
        if type_contains(ctx, syms, sup, elem) {
            return true;
        }
        for i in Types::new().interfaces(ctx, syms, t).iter() {
            if type_contains(ctx, syms, i, elem) {
                return true;
            }
        }
    }
    false
}


/// Java: Type.getEnclosingType() for the class-type family (None == null).
fn enclosing_of(ctx: &CompilerContext, t: TypeId) -> Option<TypeId> {
    get_enclosing_type(ctx, t)
}

/// Java: Type.isInterface() (ClassType/TypeVar: tsym.flags() & INTERFACE; other
/// types: false).
fn type_is_interface(ctx: &CompilerContext, t: TypeId) -> bool {
    match ctx.typ(t).tsym {
        Some(tsym) => symbol_is_interface(ctx, tsym),
        None => false,
    }
}

/// Java: Type.withTypeVar(Type t) (Type: return this; WildcardType: set bound).
fn with_type_var(ctx: &mut CompilerContext, t: TypeId, tv: TypeId) -> TypeId {
    if let TypeKindData::WildcardType { bound, .. } = &mut ctx.typ_mut(t).kind
        && *bound != Some(tv)
    {
        *bound = Some(tv);
    }
    t
}

/// Java: List<Type> reference equality (outer1 == outer && typarams1 ==
/// typarams) as used by the mapping classes: same length and same element
/// identities.
fn list_same(a: &List<TypeId>, b: &List<TypeId>) -> bool {
    if a.length() != b.length() {
        return false;
    }
    let mut ai = a.iter();
    let mut bi = b.iter();
    loop {
        match (ai.next(), bi.next()) {
            (Some(a), Some(b)) => {
                if a != b {
                    return false;
                }
            }
            (None, None) => return true,
            _ => return false,
        }
    }
}

/// Set the ClassType.supertype_field of the given type.
fn set_class_supertype_field(ctx: &mut CompilerContext, t: TypeId, value: Option<TypeId>) {
    match &mut ctx.typ_mut(t).kind {
        TypeKindData::ClassType { supertype_field, .. }
        | TypeKindData::ErasedClassType { supertype_field, .. }
        | TypeKindData::UnionClassType { supertype_field, .. }
        | TypeKindData::IntersectionClassType { supertype_field, .. } => *supertype_field = value,
        _ => panic!("not a class type"),
    }
}

/// Set the ClassType.interfaces_field of the given type.
fn set_class_interfaces_field(ctx: &mut CompilerContext, t: TypeId, value: Option<List<TypeId>>) {
    match &mut ctx.typ_mut(t).kind {
        TypeKindData::ClassType { interfaces_field, .. }
        | TypeKindData::ErasedClassType { interfaces_field, .. }
        | TypeKindData::UnionClassType { interfaces_field, .. }
        | TypeKindData::IntersectionClassType { interfaces_field, .. } => {
            *interfaces_field = value
        }
        _ => panic!("not a class type"),
    }
}

/// Java: ClassSymbol.getInterfaces() as an Option (None == null field).
fn class_symbol_get_interfaces_option(
    ctx: &CompilerContext,
    symbol: SymbolId,
) -> Option<List<TypeId>> {
    let tsym_type = ctx.sym(symbol).ty?;
    match &ctx.typ(tsym_type).kind {
        TypeKindData::ClassType { interfaces_field, .. }
        | TypeKindData::ErasedClassType { interfaces_field, .. }
        | TypeKindData::UnionClassType { interfaces_field, .. }
        | TypeKindData::IntersectionClassType { interfaces_field, .. } => {
            interfaces_field.clone()
        }
        _ => None,
    }
}


impl Types {

    /// Java: public boolean isSignaturePolymorphic(MethodSymbol msym)
    pub fn is_signature_polymorphic(
        &self,
        ctx: &CompilerContext,
        syms: &Symtab,
        m: SymbolId,
    ) -> bool {
        let flags = ctx.sym(m).flags_field;
        if (flags & crate::code::flags::NATIVE as i64) == 0 {
            return false;
        }
        let owner_ty = ctx.sym(m).owner.and_then(|o| ctx.sym(o).ty);
        if owner_ty != Some(syms.method_handle_type) && owner_ty != Some(syms.var_handle_type) {
            return false;
        }
        let Some(ty) = ctx.sym(m).ty else {
            return false;
        };
        let argtypes = ctx.typ(ty).get_parameter_types(ctx);
        if argtypes.length() != 1 {
            return false;
        }
        let head = *argtypes.head();
        if !ctx.typ(head).has_tag(crate::code::type_tag::TypeTag::ARRAY) {
            return false;
        }
        let elem = match &ctx.typ(head).kind {
            crate::code::type_::TypeKindData::ArrayType { elemtype, .. } => *elemtype,
            _ => None,
        };
        let elem_tsym = elem.and_then(|e| ctx.typ(e).tsym);
        elem_tsym == ctx.typ(syms.object_type).tsym
    }
}


// ----------------------------------------------------------------------
// Java: Types.makeIntersectionType/setBounds (Signature attribute).
// ----------------------------------------------------------------------
impl Types {
    /// Java: public IntersectionClassType makeIntersectionType(List<Type> bounds,
    /// boolean allInterfaces)
    pub fn make_intersection_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        names: &crate::util::names::Names,
        bounds: List<TypeId>,
        all_interfaces: bool,
    ) -> TypeId {
        assert!(bounds.non_empty(), "AssertionError: bounds must be non-empty");
        let first_explicit_bound = *bounds.head();
        let bounds = if all_interfaces {
            bounds.prepend(syms.object_type)
        } else {
            bounds
        };
        // Java: ClassSymbol bc = new ClassSymbol(ABSTRACT|PUBLIC|SYNTHETIC|COMPOUND|ACYCLIC,
        // names.empty, null, syms.noSymbol); bc.type = intersectionType;
        let flags = crate::code::flags::ABSTRACT as i64
            | crate::code::flags::PUBLIC as i64
            | crate::code::flags::SYNTHETIC as i64
            | crate::code::flags::COMPOUND as i64
            | crate::code::flags::ACYCLIC as i64;
        let bc = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            crate::code::kinds::Kind::TYP,
            flags,
            names.empty.clone(),
            None,
            Some(syms.no_symbol),
            crate::code::symbol::SymbolKind::ClassSymbol {
                members_field: None,
                fullname: names.empty.clone(),
                flatname: names.empty.clone(),
                sourcefile: None,
                classfile: None,
                trans_local: List::nil(),
                record_components: List::nil(),
                permitted: List::nil(),
                is_permitted_explicit: false,
                annotation_type_metadata: Default::default(),
            },
        ));
        let intersection = ctx.alloc_type(
            crate::code::type_::Type::new_intersection_class_type(bounds, bc, all_interfaces),
        );
        ctx.sym_mut(bc).ty = Some(intersection);
        let erasure = if ctx.typ(first_explicit_bound).has_tag(crate::code::type_tag::TypeTag::TYPEVAR) {
            syms.object_type
        } else {
            self.erasure(ctx, syms, first_explicit_bound)
        };
        ctx.sym_mut(bc).erasure_field = Some(erasure);
        let members = ctx.scope_create(Some(bc));
        if let crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } =
            &mut ctx.sym_mut(bc).kind_data
        {
            *members_field = Some(members);
        }
        intersection
    }

    /// Java: public void setBounds(TypeVar t, List<Type> bounds, boolean allInterfaces)
    pub fn set_bounds(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        names: &crate::util::names::Names,
        tvar: TypeId,
        bounds: List<TypeId>,
        all_interfaces: bool,
    ) {
        assert!(bounds.non_empty(), "AssertionError: bounds must be non-empty");
        let upper = if bounds.tail().is_empty() {
            *bounds.head()
        } else {
            self.make_intersection_type(ctx, syms, names, bounds, all_interfaces)
        };
        if let crate::code::type_::TypeKindData::TypeVar { bound, .. } =
            &mut ctx.typ_mut(tvar).kind
        {
            *bound = Some(upper);
        } else {
            panic!("ClassCastException: not a TypeVar");
        }
        // Java: t.rank_field = -1; — TypeVar.rank_field is not ported yet
        // (registered representation gap; rank is a cache for type-variable
        // ordering and has no observable classfile-read behavior).
    }
}

// ----------------------------------------------------------------------
// Java: Types.createErrorType overloads (P3/Enter). The three Java
// overloads get distinct Rust names (PORTING_RULES section 3).
// ----------------------------------------------------------------------

impl Types {
    /// Java: public ArrayType makeArrayType(Type t) (Types.java:1333-1336)
    pub fn make_array_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        t: TypeId,
    ) -> TypeId {
        let array = crate::code::type_::Type::new_array_type(t, syms.array_class);
        ctx.alloc_type(array)
    }

    /// Java: public Type createErrorType(Type originalType)
    pub fn create_error_type_original(
        &self,
        ctx: &mut CompilerContext,
        original_type: Option<TypeId>,
        syms: &crate::code::symtab::Symtab,
    ) -> TypeId {
        // Java: new ErrorType(originalType, syms.errSymbol)
        let original = match original_type {
            Some(original) => original,
            None => self.no_type(ctx),
        };
        ctx.alloc_type(Type::new(
            crate::code::type_tag::TypeTag::ERROR,
            Some(syms.err_symbol),
            TypeKindData::ErrorType {
                outer_field: None,
                typarams_field: None,
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
                original_type: Some(original),
                candidate_symbol: None,
            },
        ))
    }

    /// Java: public Type createErrorType(ClassSymbol c, Type originalType)
    pub fn create_error_type_class(
        &self,
        ctx: &mut CompilerContext,
        class: crate::ids::SymbolId,
        original_type: Option<TypeId>,
    ) -> TypeId {
        let no_type = self.no_type(ctx);
        let error_type = ctx.alloc_type(Type::new(
            crate::code::type_tag::TypeTag::ERROR,
            Some(class),
            TypeKindData::ErrorType {
                outer_field: None,
                typarams_field: None,
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
                original_type: Some(original_type.unwrap_or(no_type)),
                candidate_symbol: None,
            },
        ));
        ctx.sym_mut(class).ty = Some(error_type);
        ctx.sym_mut(class).kind = crate::code::kinds::Kind::ERR;
        let scope = ctx.scope_create_error(Some(class));
        if let crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } =
            &mut ctx.sym_mut(class).kind_data
        {
            *members_field = Some(scope);
        }
        error_type
    }

    /// Java: public Type createErrorType(Name name, TypeSymbol container,
    /// Type originalType)
    pub fn create_error_type(
        &self,
        ctx: &mut CompilerContext,
        name: Name,
        container: crate::ids::SymbolId,
        original_type: Option<TypeId>,
    ) -> TypeId {
        let no_type = self.no_type(ctx);
        let class = crate::code::symtab::new_class_symbol(
            ctx,
            (crate::code::flags::PUBLIC | crate::code::flags::STATIC) as i64
                | crate::code::flags::ACYCLIC as i64,
            name,
            Some(container),
            no_type,
        );
        self.create_error_type_class(ctx, class, original_type)
    }

    /// The shared Type.noType value (the arena has no global singleton; the
    /// port allocates a JCNoType value like Symtab.noTypeId).
    fn no_type(&self, ctx: &mut CompilerContext) -> TypeId {
        ctx.alloc_type(Type::new(
            crate::code::type_tag::TypeTag::NONE,
            None,
            TypeKindData::JCNoType,
        ))
    }
}

/// Java: Types.AdaptFailure (thrown when a mapping conflicts).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdaptFailure;

/// Java: Type.getEnclosingType() with Type.noType for the absent case.
fn enclosing_or_no_type(ctx: &mut CompilerContext, t: TypeId) -> TypeId {
    ctx.typ(t)
        .get_enclosing_type()
        .unwrap_or_else(|| crate::code::symtab::no_type_id(ctx))
}

/// Java: Type.getTypeArguments() as a List (the empty list when absent).
fn type_arguments_list(ctx: &CompilerContext, t: TypeId) -> List<TypeId> {
    match &ctx.typ(t).kind {
        TypeKindData::ClassType { typarams_field, .. }
        | TypeKindData::ErasedClassType { typarams_field, .. }
        | TypeKindData::UnionClassType { typarams_field, .. }
        | TypeKindData::IntersectionClassType { typarams_field, .. } => typarams_field
            .clone()
            .unwrap_or_else(List::nil),
        _ => List::nil(),
    }
}

impl Types {
   // ------------------------------------------------------------------
    // Types relations : adapt/adaptSelf/rewriteSupers/containsTypeRecursive
    // (Types.java:4577-4700/1133-1180) — the core needed by isSubtype's
    // parameterized class-type branch.
    // ------------------------------------------------------------------

    /// Java: ClassType.allparams() — the cached all type parameters.
    pub fn class_allparams(&self, ctx: &CompilerContext, t: TypeId) -> List<TypeId> {
        match &ctx.typ(t).kind {
            TypeKindData::ClassType {
                allparams_field,
                typarams_field,
                ..
            }
            | TypeKindData::ErasedClassType {
                allparams_field,
                typarams_field,
                ..
            }
            | TypeKindData::UnionClassType {
                allparams_field,
                typarams_field,
                ..
            }
            | TypeKindData::IntersectionClassType {
                allparams_field,
                typarams_field,
                ..
            } => allparams_field
                .clone()
                .or_else(|| typarams_field.clone())
                .unwrap_or_else(List::nil),
            _ => List::nil(),
        }
    }

    /// Java: Type.isExtendsBound() for WildcardType
    fn wildcard_is_extends_bound(&self, ctx: &CompilerContext, t: TypeId) -> bool {
        match &ctx.typ(t).kind {
            TypeKindData::WildcardType { kind, .. } => {
                *kind == crate::code::bound_kind::BoundKind::EXTENDS
            }
            _ => false,
        }
    }

    /// Java: Type.isSuperBound() for WildcardType
    fn wildcard_is_super_bound(&self, ctx: &CompilerContext, t: TypeId) -> bool {
        match &ctx.typ(t).kind {
            TypeKindData::WildcardType { kind, .. } => {
                *kind == crate::code::bound_kind::BoundKind::SUPER
            }
            _ => false,
        }
    }

    /// Java: public void adapt(Type source, Type target, ListBuffer<Type> from,
    /// ListBuffer<Type> to) (Types.java:4577-4594) — the Adapter mapping is
    /// computed by adapt_visit; the returned pair is the from/to substitution.
    pub fn adapt(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        source: TypeId,
        target: TypeId,
    ) -> Result<(Vec<TypeId>, Vec<TypeId>), AdaptFailure> {
        let mut from: Vec<TypeId> = Vec::new();
        let mut to: Vec<TypeId> = Vec::new();
        let mut mapping: Vec<(SymbolId, TypeId)> = Vec::new();
        let mut cache: Vec<(TypeId, TypeId)> = Vec::new();
        self.adapt_visit(
            ctx,
            syms,
            source,
            target,
            &mut from,
            &mut to,
            &mut mapping,
            &mut cache,
        )?;
        // Java: for each from entry replace to with the current mapping value.
        for (index, from_ty) in from.iter().enumerate() {
            let tsym = ctx.typ(*from_ty).tsym.expect("type variable symbol");
            if let Some((_, value)) = mapping.iter().find(|(sym, _)| *sym == tsym) {
                to[index] = *value;
            }
        }
        Ok((from, to))
    }

    /// Java: Adapter.adapt(Type source, Type target)
    #[allow(clippy::too_many_arguments)]
    fn adapt_visit(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        source: TypeId,
        target: TypeId,
        from: &mut Vec<TypeId>,
        to: &mut Vec<TypeId>,
        mapping: &mut Vec<(SymbolId, TypeId)>,
        cache: &mut Vec<(TypeId, TypeId)>,
    ) -> Result<(), AdaptFailure> {
        match &ctx.typ(source).kind {
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
                if ctx.typ(target).has_tag(TypeTag::CLASS) {
                    let source_params = self.class_allparams(ctx, source);
                    let target_params = self.class_allparams(ctx, target);
                    self.adapt_recursive(
                        ctx, syms, source_params, target_params, from, to, mapping, cache,
                    )?;
                }
            }
            TypeKindData::ArrayType { .. } => {
                if ctx.typ(target).has_tag(TypeTag::ARRAY) {
                    let source_elem = match &ctx.typ(source).kind {
                        TypeKindData::ArrayType { elemtype, .. } => {
                            elemtype.expect("array element")
                        }
                        _ => unreachable!(),
                    };
                    let target_elem = match &ctx.typ(target).kind {
                        TypeKindData::ArrayType { elemtype, .. } => {
                            elemtype.expect("array element")
                        }
                        _ => unreachable!(),
                    };
                    self.adapt_recursive(
                        ctx,
                        syms,
                        List::of(source_elem),
                        List::of(target_elem),
                        from,
                        to,
                        mapping,
                        cache,
                    )?;
                }
            }
            TypeKindData::WildcardType { .. } => {
                if self.wildcard_is_extends_bound(ctx, source) {
                    let source_upper = self.wild_upper_bound(ctx, syms, source);
                    let target_upper = self.wild_upper_bound(ctx, syms, target);
                    self.adapt_recursive(
                        ctx,
                        syms,
                        List::of(source_upper),
                        List::of(target_upper),
                        from,
                        to,
                        mapping,
                        cache,
                    )?;
                } else if self.wildcard_is_super_bound(ctx, source) {
                    let source_lower = self.wild_lower_bound(ctx, syms, source);
                    let target_lower = self.wild_lower_bound(ctx, syms, target);
                    self.adapt_recursive(
                        ctx,
                        syms,
                        List::of(source_lower),
                        List::of(target_lower),
                        from,
                        to,
                        mapping,
                        cache,
                    )?;
                }
            }
            TypeKindData::TypeVar { .. } | TypeKindData::CapturedType { .. } => {
                let source_tsym = ctx.typ(source).tsym.expect("type variable symbol");
                let existing = mapping
                    .iter()
                    .find(|(sym, _)| *sym == source_tsym)
                    .map(|(_, value)| *value);
                let value = match existing {
                    Some(value) => {
                        let value_super = self.wildcard_is_super_bound(ctx, value);
                        let target_super = self.wildcard_is_super_bound(ctx, target);
                        if value_super && target_super {
                            let value_lower = self.wild_lower_bound(ctx, syms, value);
                            let target_lower = self.wild_lower_bound(ctx, syms, target);
                            if self.is_subtype(ctx, syms, value_lower, target_lower) {
                                target
                            } else {
                                value
                            }
                        } else {
                            let value_extends = self.wildcard_is_extends_bound(ctx, value);
                            let target_extends = self.wildcard_is_extends_bound(ctx, target);
                            if value_extends && target_extends {
                                let value_upper = self.wild_upper_bound(ctx, syms, value);
                                let target_upper = self.wild_upper_bound(ctx, syms, target);
                                if self.is_subtype(ctx, syms, value_upper, target_upper) {
                                    value
                                } else {
                                    target
                                }
                            } else if !self.is_same_type(ctx, syms, value, target) {
                                return Err(AdaptFailure);
                            } else {
                                value
                            }
                        }
                    }
                    None => {
                        from.push(source);
                        to.push(target);
                        target
                    }
                };
                if let Some(entry) = mapping
                    .iter_mut()
                    .find(|(sym, _)| *sym == source_tsym)
                {
                    entry.1 = value;
                } else {
                    mapping.push((source_tsym, value));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Java: Adapter.adaptRecursive(Type source, Type target)
    #[allow(clippy::too_many_arguments)]
    fn adapt_recursive(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        source: List<TypeId>,
        target: List<TypeId>,
        from: &mut Vec<TypeId>,
        to: &mut Vec<TypeId>,
        mapping: &mut Vec<(SymbolId, TypeId)>,
        cache: &mut Vec<(TypeId, TypeId)>,
    ) -> Result<(), AdaptFailure> {
        let mut source_list = source;
        let mut target_list = target;
        while source_list.non_empty() && target_list.non_empty() {
            let source_head = *source_list.head();
            let target_head = *target_list.head();
            let pair = (source_head, target_head);
            if !cache.contains(&pair) {
                cache.push(pair);
                let result = self.adapt_visit(
                    ctx,
                    syms,
                    source_head,
                    target_head,
                    from,
                    to,
                    mapping,
                    cache,
                );
                cache.pop();
                result?;
            }
            source_list = source_list.tail();
            target_list = target_list.tail();
        }
        Ok(())
    }

    /// Java: private void adaptSelf(Type t, ListBuffer<Type> from,
    /// ListBuffer<Type> to) (Types.java:4690-4699)
    pub fn adapt_self(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> (Vec<TypeId>, Vec<TypeId>) {
        let tsym = ctx.typ(t).tsym.expect("class type symbol");
        let tsym_type = ctx.sym(tsym).ty.expect("class symbol type");
        match self.adapt(ctx, syms, tsym_type, t) {
            Ok(pair) => pair,
            Err(failure) => panic!("AssertionError: {:?}", failure),
        }
    }

    /// Java: private Type rewriteSupers(Type t) (Types.java:1148-1180)
    pub fn rewrite_supers(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> TypeId {
        if !is_parameterized_type(ctx, t) {
            return t;
        }
        let (from, to) = self.adapt_self(ctx, syms, t);
        if from.is_empty() {
            return t;
        }
        let mut rewrite: Vec<TypeId> = Vec::new();
        let mut changed = false;
        for orig in &to {
            let mut s = self.rewrite_supers(ctx, syms, *orig);
            if self.wildcard_is_super_bound(ctx, s) && !self.wildcard_is_extends_bound(ctx, s) {
                s = ctx.alloc_type(crate::code::type_::Type::new(
                    TypeTag::WILDCARD,
                    Some(syms.bound_class),
                    TypeKindData::WildcardType {
                        type_: Some(syms.object_type),
                        kind: crate::code::bound_kind::BoundKind::UNBOUND,
                        bound: None,
                    },
                ));
                changed = true;
            } else if s != *orig {
                let upper = self.wild_upper_bound(ctx, syms, s);
                s = ctx.alloc_type(crate::code::type_::Type::new(
                    TypeTag::WILDCARD,
                    Some(syms.bound_class),
                    TypeKindData::WildcardType {
                        type_: Some(upper),
                        kind: crate::code::bound_kind::BoundKind::EXTENDS,
                        bound: None,
                    },
                ));
                changed = true;
            }
            rewrite.push(s);
        }
        if changed {
            let tsym = ctx.typ(t).tsym.expect("class type symbol");
            let tsym_type = ctx.sym(tsym).ty.expect("class symbol type");
            let from_list = List::from_iterable(from);
            let to_list = List::from_iterable(rewrite);
            self.subst_type(ctx, syms, tsym_type, &from_list, &to_list)
        } else {
            t
        }
    }

    /// Java: IsSubtype.containsTypeRecursive(Type t, Type s)
    /// (Types.java:1133-1146)
    pub fn contains_type_recursive(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        let mut cache: Vec<(TypeId, TypeId)> = Vec::new();
        self.contains_type_recursive_cached(ctx, syms, t, s, &mut cache)
    }

    fn contains_type_recursive_cached(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        cache: &mut Vec<(TypeId, TypeId)>,
    ) -> bool {
        let pair = (t, s);
        if !cache.contains(&pair) {
            cache.push(pair);
            let result = self.contains_type_list(
                ctx,
                syms,
                &type_arguments_list(ctx, t),
                &type_arguments_list(ctx, s),
            );
            cache.pop();
            result
        } else {
            let rewritten = self.rewrite_supers(ctx, syms, s);
            self.contains_type_list(
                ctx,
                syms,
                &type_arguments_list(ctx, t),
                &type_arguments_list(ctx, rewritten),
            )
        }
    }

}


// ---------------------------------------------------------------------------
// Types relations : Symbol.precedes, closure/insert/union/closureMin,
// glb and the complete capture (Types.java:835-852 Symbol.precedes,
// 3700-3810 closure/union, 3915-3950 closureMin, 4088-4160 glb, 4388-4475
// capture/freshTypeVariables).
// ---------------------------------------------------------------------------
impl Types {
    /// Java: TypeSymbol.getQualifiedName()
    fn type_qualified_name(
        &self,
        ctx: &CompilerContext,
        symbol: SymbolId,
    ) -> crate::util::name::Name {
        match &ctx.sym(symbol).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { fullname, .. } => fullname.clone(),
            _ => ctx.sym(symbol).name.clone(),
        }
    }

    /// Java: public boolean isInterface() (Type.java:611: INTERFACE flag)
    fn is_interface(&self, ctx: &CompilerContext, t: TypeId) -> bool {
        ctx.typ(t)
            .tsym
            .map(|tsym| {
                (ctx.sym(tsym).flags_field & crate::code::flags::INTERFACE as i64) != 0
            })
            .unwrap_or(false)
    }

    /// Java: public final boolean precedes(TypeSymbol that, Types types)
    /// (Symbol.java:835-852)
    pub fn symbol_precedes(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        this_sym: SymbolId,
        that_sym: SymbolId,
    ) -> bool {
        if this_sym == that_sym {
            return false;
        }
        let this_type = ctx.sym(this_sym).ty.expect("this type");
        let that_type = ctx.sym(that_sym).ty.expect("that type");
        if ctx.typ(this_type).get_tag() == ctx.typ(that_type).get_tag() {
            if ctx.typ(this_type).has_tag(TypeTag::CLASS) {
                let that_rank = self.rank(ctx, syms, that_type);
                let this_rank = self.rank(ctx, syms, this_type);
                if that_rank != this_rank {
                    return that_rank < this_rank;
                }
                return self
                    .type_qualified_name(ctx, that_sym)
                    .compare_to(&self.type_qualified_name(ctx, this_sym))
                    < 0;
            } else if ctx.typ(this_type).has_tag(TypeTag::TYPEVAR) {
                return self.is_subtype(ctx, syms, this_type, that_type);
            }
        }
        ctx.typ(this_type).has_tag(TypeTag::TYPEVAR)
    }

    /// Java: public List<Type> insert(List<Type> cl, Type t)
    /// (Types.java:3749-3763; basicClosureSkip = same tsym)
    fn closure_insert(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        cl: List<TypeId>,
        t: TypeId,
    ) -> List<TypeId> {
        if cl.is_empty() {
            return cl.prepend(t);
        }
        let head = *cl.head();
        let t_tsym = ctx.typ(t).tsym.expect("insert type symbol");
        let head_tsym = ctx.typ(head).tsym.expect("closure head symbol");
        if t_tsym == head_tsym {
            return cl;
        }
        if self.symbol_precedes(ctx, syms, t_tsym, head_tsym) {
            return cl.prepend(t);
        }
        self.closure_insert(ctx, syms, cl.tail(), t).prepend(head)
    }

    /// Java: public List<Type> union(List<Type> cl1, List<Type> cl2)
    /// (Types.java:3765-3810; basicClosureSkip = same tsym)
    pub fn closure_union(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        cl1: List<TypeId>,
        cl2: List<TypeId>,
    ) -> List<TypeId> {
        if cl1.is_empty() {
            return cl2;
        }
        if cl2.is_empty() {
            return cl1;
        }
        let head1 = *cl1.head();
        let head2 = *cl2.head();
        let head1_tsym = ctx.typ(head1).tsym.expect("union head symbol");
        let head2_tsym = ctx.typ(head2).tsym.expect("union head symbol");
        if head1_tsym == head2_tsym {
            return self
                .closure_union(ctx, syms, cl1.tail(), cl2.tail())
                .prepend(head1);
        }
        if self.symbol_precedes(ctx, syms, head2_tsym, head1_tsym) {
            return self
                .closure_union(ctx, syms, cl1, cl2.tail())
                .prepend(head2);
        }
        self.closure_union(ctx, syms, cl1.tail(), cl2).prepend(head1)
    }

    /// Java: public List<Type> closure(Type t) (Types.java:3710-3736; the
    /// upstream cache is a performance optimization, omitted as equivalent)
    pub fn closure(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> List<TypeId> {
        let mut cl: List<TypeId>;
        let st = self.supertype(ctx, syms, t);
        if !self.is_compound(ctx, t) {
            if ctx.typ(st).has_tag(TypeTag::CLASS) {
                let super_closure = self.closure(ctx, syms, st);
                cl = self.closure_insert(ctx, syms, super_closure, t);
            } else if ctx.typ(st).has_tag(TypeTag::TYPEVAR) {
                let super_closure = self.closure(ctx, syms, st);
                cl = super_closure.prepend(t);
            } else {
                cl = List::of(t);
            }
        } else {
            cl = self.closure(ctx, syms, st);
        }
        let mut interfaces = self.interfaces(ctx, syms, t);
        while interfaces.non_empty() {
            let interface = *interfaces.head();
            let interface_closure = self.closure(ctx, syms, interface);
            cl = self.closure_union(ctx, syms, cl, interface_closure);
            interfaces = interfaces.tail();
        }
        cl
    }

    /// Java: private List<Type> closureMin(List<Type> cl)
    /// (Types.java:3915-3950)
    fn closure_min(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        cl: List<TypeId>,
    ) -> List<TypeId> {
        let mut classes: List<TypeId> = List::nil();
        let mut interfaces: List<TypeId> = List::nil();
        let mut to_skip: Vec<TypeId> = Vec::new();
        let mut cl = cl;
        while cl.non_empty() {
            let current = *cl.head();
            let mut keep = !to_skip.contains(&current);
            if keep && ctx.typ(current).has_tag(TypeTag::TYPEVAR) {
                let mut tail = cl.tail();
                while tail.non_empty() {
                    let t = *tail.head();
                    if self.is_subtype_no_capture(ctx, syms, t, current) {
                        keep = false;
                        break;
                    }
                    tail = tail.tail();
                }
            }
            if keep {
                if self.is_interface(ctx, current) {
                    interfaces = interfaces.append(current);
                } else {
                    classes = classes.append(current);
                }
                let mut tail = cl.tail();
                while tail.non_empty() {
                    let t = *tail.head();
                    if self.is_subtype_no_capture(ctx, syms, current, t) {
                        to_skip.push(t);
                    }
                    tail = tail.tail();
                }
            }
            cl = cl.tail();
        }
        classes.append_list(&interfaces)
    }

    /// Java: public Type glb(List<Type> ts) (Types.java:4092-4100)
    pub fn glb_list(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts: List<TypeId>,
    ) -> TypeId {
        assert!(!ts.is_empty(), "AssertionError: glb of an empty list");
        let mut t1 = *ts.head();
        let mut tail = ts.tail();
        while tail.non_empty() {
            if ctx.typ(t1).has_tag(TypeTag::ERROR) {
                return t1;
            }
            let t2 = *tail.head();
            t1 = self.glb(ctx, syms, t1, t2);
            tail = tail.tail();
        }
        t1
    }

    /// Java: public Type glb(Type t, Type s) (Types.java:4102-4113)
    pub fn glb(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> TypeId {
        if ctx.typ(t).is_primitive() || ctx.typ(s).is_primitive() {
            return syms.err_type;
        }
        if self.is_subtype_no_capture(ctx, syms, t, s) {
            return t;
        }
        if self.is_subtype_no_capture(ctx, syms, s, t) {
            return s;
        }
        let t_closure = self.closure(ctx, syms, t);
        let s_closure = self.closure(ctx, syms, s);
        let closure = self.closure_union(ctx, syms, t_closure, s_closure);
        self.glb_flattened(ctx, syms, closure, t)
    }

    /// Java: private Type glbFlattened(List<Type> flatBounds, Type errT)
    /// (Types.java:4124-4155)
    fn glb_flattened(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        flat_bounds: List<TypeId>,
        err_t: TypeId,
    ) -> TypeId {
        let bounds = self.closure_min(ctx, syms, flat_bounds);
        if bounds.is_empty() {
            return syms.object_type;
        }
        if bounds.tail().is_empty() {
            return *bounds.head();
        }
        let mut class_count = 0;
        let mut cvars: Vec<TypeId> = Vec::new();
        let mut lowers: Vec<TypeId> = Vec::new();
        let mut current = bounds.clone();
        while current.non_empty() {
            let bound = *current.head();
            if !self.is_interface(ctx, bound) {
                class_count += 1;
                let lower = cvar_lower_bound(ctx, bound);
                if bound != lower && !ctx.typ(lower).has_tag(TypeTag::BOT) {
                    cvars.push(bound);
                    lowers.push(lower);
                }
            }
            current = current.tail();
        }
        if class_count > 1 {
            if lowers.is_empty() {
                return self.create_error_type_original(ctx, Some(err_t), syms);
            }
            let mut new_bounds: Vec<TypeId> = Vec::new();
            let mut current = bounds.clone();
            while current.non_empty() {
                let bound = *current.head();
                if !cvars.contains(&bound) {
                    new_bounds.push(bound);
                }
                current = current.tail();
            }
            new_bounds.extend(lowers);
            let mut list = List::nil();
            for bound in new_bounds.into_iter().rev() {
                list = list.prepend(bound);
            }
            return self.glb_list(ctx, syms, list);
        }
        let Some(names) = self.names.clone() else {
            unimplemented!(
                "TODO(Types batch): glb makeIntersectionType needs the Names service (Types::with_names)"
            );
        };
        self.make_intersection_type(ctx, syms, &names, bounds, false)
    }
}


// ---------------------------------------------------------------------------
// Types relations (isCastable support): asSub, Type.contains,
// rewriteQuantifiers, notSoftSubtype (Types.java:539-600, 1130-1155,
// 4720-4800, 1989-2020).
// ---------------------------------------------------------------------------
impl Types {
    /// Java: ClassType.contains(Type elem) (Type.java:1143-1155)
    fn type_contains(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        elem: TypeId,
    ) -> bool {
        if type_equals_ignore_metadata(ctx, t, elem) {
            return true;
        }
        if is_parameterized_type(ctx, t) {
            if let Some(enclosing) = get_enclosing_type(ctx, t)
                && self.type_contains(ctx, syms, enclosing, elem)
            {
                return true;
            }
            let arguments = get_type_arguments(ctx, t);
            if type_list_contains(ctx, arguments, elem) {
                return true;
            }
        }
        if self.is_compound(ctx, t) {
            let supertype = self.supertype(ctx, syms, t);
            if type_equals_ignore_metadata(ctx, supertype, elem) {
                return true;
            }
            let interfaces = self.interfaces(ctx, syms, t);
            if type_list_contains(ctx, interfaces, elem) {
                return true;
            }
        }
        false
    }

    /// Java: public Type asSub(Type t, Symbol sym) (Types.java:539-600)
    pub fn as_sub(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        sym: SymbolId,
    ) -> Option<TypeId> {
        // Java: SimpleVisitor.visitType returns null for every non-class type.
        if !ctx.typ(t).has_tag(TypeTag::CLASS) {
            return None;
        }
        if ctx.typ(t).tsym == Some(sym) {
            return Some(t);
        }
        let sym_type = ctx.sym(sym).ty.expect("symbol type");
        let base = self.as_super(ctx, syms, sym_type, ctx.typ(t).tsym.expect("type symbol"))?;
        let (from, to) = match self.adapt(ctx, syms, base, t) {
            Ok(pair) => pair,
            Err(_failure) => return None,
        };
        let from_list = List::from_iterable(from);
        let to_list = List::from_iterable(to);
        let mut res = self.subst_type(ctx, syms, sym_type, &from_list, &to_list);
        if !self.is_subtype(ctx, syms, res, t) {
            return None;
        }
        let mut open_vars: Vec<TypeId> = Vec::new();
        let mut params = self.class_allparams(ctx, sym_type);
        while params.non_empty() {
            let param = *params.head();
            if self.type_contains(ctx, syms, res, param) && !self.type_contains(ctx, syms, t, param) {
                open_vars.push(param);
            }
            params = params.tail();
        }
        if !open_vars.is_empty() {
            if is_raw_type(ctx, t) {
                res = self.erasure(ctx, syms, res);
            } else {
                let opens = List::from_iterable(open_vars.clone());
                let mut wildcards: List<TypeId> = List::nil();
                for open in &open_vars {
                    let wildcard = ctx.alloc_type(crate::code::type_::Type::new(
                        TypeTag::WILDCARD,
                        Some(syms.bound_class),
                        TypeKindData::WildcardType {
                            type_: Some(syms.object_type),
                            kind: crate::code::bound_kind::BoundKind::UNBOUND,
                            bound: Some(*open),
                        },
                    ));
                    wildcards = wildcards.append(wildcard);
                }
                res = self.subst_type(ctx, syms, res, &opens, &wildcards);
            }
        }
        Some(res)
    }

    /// Java: private Type rewriteQuantifiers(Type t, boolean high,
    /// boolean rewriteTypeVars) (Types.java:4720-4800; the Rewriter visitor)
    pub fn rewrite_quantifiers(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        high: bool,
        rewrite_type_vars: bool,
    ) -> TypeId {
        match &ctx.typ(t).kind {
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
                let allparams = self.class_allparams(ctx, t);
                let mut rewritten: List<TypeId> = List::nil();
                let mut changed = false;
                let mut current = allparams;
                while current.non_empty() {
                    let arg = *current.head();
                    let bound =
                        self.rewrite_quantifiers(ctx, syms, arg, high, rewrite_type_vars);
                    if arg != bound {
                        changed = true;
                    }
                    rewritten = rewritten.append(bound);
                    current = current.tail();
                }
                if changed {
                    let tsym = ctx.typ(t).tsym.expect("class type symbol");
                    let tsym_type = ctx.sym(tsym).ty.expect("class symbol type");
                    let tsym_params = self.class_allparams(ctx, tsym_type);
                    self.subst_type(ctx, syms, tsym_type, &tsym_params, &rewritten)
                } else {
                    t
                }
            }
            TypeKindData::CapturedType { .. } => {
                let wildcard = match &ctx.typ(t).kind {
                    TypeKindData::CapturedType { wildcard, .. } => {
                        wildcard.expect("captured wildcard")
                    }
                    _ => unreachable!(),
                };
                let w_bound = match &ctx.typ(wildcard).kind {
                    TypeKindData::WildcardType { type_, .. } => {
                        type_.expect("wildcard bound")
                    }
                    _ => unreachable!(),
                };
                let bound = if self.type_contains(ctx, syms, w_bound, t) {
                    self.erasure(ctx, syms, w_bound)
                } else {
                    self.rewrite_quantifiers(ctx, syms, w_bound, high, rewrite_type_vars)
                };
                let bound = self.rewrite_quantifiers(ctx, syms, bound, high, rewrite_type_vars);
                let (formal, kind) = match &ctx.typ(wildcard).kind {
                    TypeKindData::WildcardType { bound, kind, .. } => (*bound, *kind),
                    _ => unreachable!(),
                };
                self.rewrite_as_wildcard_type(ctx, syms, bound, formal, kind, high)
            }
            TypeKindData::TypeVar { .. } => {
                if rewrite_type_vars {
                    let upper = ctx.typ(t).get_upper_bound().expect("type var upper bound");
                    let bound = if self.type_contains(ctx, syms, upper, t) {
                        self.erasure(ctx, syms, upper)
                    } else {
                        self.rewrite_quantifiers(ctx, syms, upper, high, rewrite_type_vars)
                    };
                    self.rewrite_as_wildcard_type(
                        ctx,
                        syms,
                        bound,
                        Some(t),
                        crate::code::bound_kind::BoundKind::EXTENDS,
                        high,
                    )
                } else {
                    t
                }
            }
            TypeKindData::WildcardType { .. } => {
                let (bound, formal, kind) = match &ctx.typ(t).kind {
                    TypeKindData::WildcardType {
                        type_: Some(type_),
                        bound,
                        kind,
                    } => (*type_, *bound, *kind),
                    _ => return t,
                };
                let bound2 = self.rewrite_quantifiers(ctx, syms, bound, high, rewrite_type_vars);
                if bound == bound2 {
                    t
                } else {
                    self.rewrite_as_wildcard_type(ctx, syms, bound2, formal, kind, high)
                }
            }
            _ => t,
        }
    }

    /// Java: Rewriter.rewriteAsWildcardType + makeExtendsWildcard/
    /// makeSuperWildcard (Types.java:4785-4860)
    fn rewrite_as_wildcard_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        bound: TypeId,
        formal: Option<TypeId>,
        bk: crate::code::bound_kind::BoundKind,
        high: bool,
    ) -> TypeId {
        use crate::code::bound_kind::BoundKind;
        match bk {
            BoundKind::EXTENDS => {
                if high {
                    let b = self.rewrite_b(ctx, syms, bound, true);
                    self.make_extends_wildcard(ctx, syms, b, formal)
                } else {
                    self.make_extends_wildcard(ctx, syms, syms.object_type, formal)
                }
            }
            BoundKind::SUPER => {
                if high {
                    self.make_super_wildcard(ctx, syms, syms.bot_type, formal)
                } else {
                    let b = self.rewrite_b(ctx, syms, bound, false);
                    self.make_super_wildcard(ctx, syms, b, formal)
                }
            }
            BoundKind::UNBOUND => {
                self.make_extends_wildcard(ctx, syms, syms.object_type, formal)
            }
        }
    }

    /// Java: Rewriter.B(Type t)
    fn rewrite_b(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        high: bool,
    ) -> TypeId {
        let mut t = t;
        while ctx.typ(t).has_tag(TypeTag::WILDCARD) {
            let (type_, kind) = match &ctx.typ(t).kind {
                TypeKindData::WildcardType { type_, kind, .. } => (*type_, *kind),
                _ => break,
            };
            t = match type_ {
                Some(bound) => bound,
                None => {
                    if high {
                        syms.object_type
                    } else {
                        syms.bot_type
                    }
                }
            };
            let _ = kind;
        }
        t
    }

    fn make_extends_wildcard(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        bound: TypeId,
        formal: Option<TypeId>,
    ) -> TypeId {
        let kind = if bound == syms.object_type {
            crate::code::bound_kind::BoundKind::UNBOUND
        } else {
            crate::code::bound_kind::BoundKind::EXTENDS
        };
        ctx.alloc_type(crate::code::type_::Type::new(
            TypeTag::WILDCARD,
            Some(syms.bound_class),
            TypeKindData::WildcardType {
                type_: Some(bound),
                kind,
                bound: formal,
            },
        ))
    }

    fn make_super_wildcard(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        bound: TypeId,
        formal: Option<TypeId>,
    ) -> TypeId {
        let kind = if ctx.typ(bound).has_tag(TypeTag::BOT) {
            crate::code::bound_kind::BoundKind::UNBOUND
        } else {
            crate::code::bound_kind::BoundKind::SUPER
        };
        let type_ = if kind == crate::code::bound_kind::BoundKind::UNBOUND {
            syms.object_type
        } else {
            bound
        };
        ctx.alloc_type(crate::code::type_::Type::new(
            TypeTag::WILDCARD,
            Some(syms.bound_class),
            TypeKindData::WildcardType {
                type_: Some(type_),
                kind,
                bound: formal,
            },
        ))
    }

    /// Java: private Type arraySuperType() (Types.java:4080-4087):
    /// all arrays implement Cloneable and Serializable.
    fn array_super_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
    ) -> TypeId {
        let Some(names) = self.names.clone() else {
            unimplemented!(
                "TODO(Types batch): arraySuperType needs the Names service (Types::with_names)"
            );
        };
        let bounds = List::of(syms.serializable_type).append(syms.cloneable_type);
        self.make_intersection_type(ctx, syms, &names, bounds, true)
    }

    /// Java: public boolean notSoftSubtype(Type t, Type s)
    /// (Types.java:1998-2010)
    pub fn not_soft_subtype(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        if t == s {
            return false;
        }
        if ctx.typ(t).has_tag(TypeTag::TYPEVAR) {
            let upper = ctx.typ(t).get_upper_bound().expect("type var upper bound");
            let relaxed = self.relax_bound(ctx, syms, s);
            return !self.is_castable(ctx, syms, upper, relaxed);
        }
        let s = if !ctx.typ(s).has_tag(TypeTag::WILDCARD) {
            self.cvar_upper_bound(ctx, s)
        } else {
            s
        };
        let relaxed = self.relax_bound(ctx, syms, s);
        !self.is_subtype(ctx, syms, t, relaxed)
    }

    /// Java: private Type relaxBound(Type t) (Types.java:2012-2016)
    fn relax_bound(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> TypeId {
        if ctx.typ(t).has_tag(TypeTag::TYPEVAR) {
            let skipped = self.skip_type_vars(ctx, t);
            self.rewrite_quantifiers(ctx, syms, skipped, true, true)
        } else {
            t
        }
    }
}

/// Java: contains(List<Type> ts, Type elem) helper for ClassType.contains.
fn type_list_contains(
    ctx: &mut CompilerContext,
    ts: List<TypeId>,
    elem: TypeId,
) -> bool {
    let mut current = ts;
    while current.non_empty() {
        let t = *current.head();
        if type_equals_ignore_metadata(ctx, t, elem) {
            return true;
        }
        current = current.tail();
    }
    false
}


// ---------------------------------------------------------------------------
// Types relations (isCastable support): disjointTypes/disjointType,
// superClosure, sideCast/sideCastFinal, giveWarning (Types.java:1888-1975,
// 4479-4560).
// ---------------------------------------------------------------------------
impl Types {
    /// Java: public boolean disjointTypes(List<Type> ts, List<Type> ss)
    /// (Types.java:1888-1896)
    pub fn disjoint_types(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts: List<TypeId>,
        ss: List<TypeId>,
    ) -> bool {
        let mut ts = ts;
        let mut ss = ss;
        // Java: while (ts.tail != null && ss.tail != null) — in javac List.nil()
        // is a non-null EMPTY_LIST with a null tail, so the condition means
        // "both lists are non-empty".
        while ts.non_empty() && ss.non_empty() {
            let t = *ts.head();
            let s = *ss.head();
            if self.disjoint_type(ctx, syms, t, s) {
                return true;
            }
            ts = ts.tail();
            ss = ss.tail();
        }
        false
    }

    /// Java: public boolean disjointType(Type t, Type s) (Types.java:1906-1974)
    pub fn disjoint_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        let mut cache: Vec<(TypeId, TypeId)> = Vec::new();
        self.disjoint_type_cached(ctx, syms, t, s, &mut cache)
    }

    fn disjoint_type_cached(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        cache: &mut Vec<(TypeId, TypeId)>,
    ) -> bool {
        if ctx.typ(s).has_tag(TypeTag::WILDCARD) && !ctx.typ(t).has_tag(TypeTag::WILDCARD) {
            return self.disjoint_type_cached(ctx, syms, s, t, cache);
        }
        if ctx.typ(t).has_tag(TypeTag::WILDCARD) {
            let (type_, kind) = match &ctx.typ(t).kind {
                TypeKindData::WildcardType { type_, kind, .. } => (*type_, *kind),
                _ => return false,
            };
            if kind == crate::code::bound_kind::BoundKind::UNBOUND {
                return false;
            }
            let Some(type_) = type_ else {
                return false;
            };
            if !ctx.typ(s).has_tag(TypeTag::WILDCARD) {
                return if kind == crate::code::bound_kind::BoundKind::EXTENDS {
                    self.not_soft_subtype_cached(ctx, syms, s, type_, cache)
                } else {
                    self.not_soft_subtype_cached(ctx, syms, type_, s, cache)
                };
            }
            let (s_type, s_kind) = match &ctx.typ(s).kind {
                TypeKindData::WildcardType { type_, kind, .. } => (*type_, *kind),
                _ => return false,
            };
            if s_kind == crate::code::bound_kind::BoundKind::UNBOUND {
                return false;
            }
            let Some(s_type) = s_type else {
                return false;
            };
            if kind == crate::code::bound_kind::BoundKind::EXTENDS {
                if s_kind == crate::code::bound_kind::BoundKind::EXTENDS {
                    let upper_s = self.wild_upper_bound(ctx, syms, s);
                    let pair = (type_, upper_s);
                    if cache.contains(&pair) {
                        return true;
                    }
                    cache.push(pair);
                    let result = self.is_castable(ctx, syms, type_, upper_s);
                    cache.pop();
                    return !result;
                } else if s_kind == crate::code::bound_kind::BoundKind::SUPER {
                    let lower_s = self.wild_lower_bound(ctx, syms, s);
                    return self.not_soft_subtype_cached(ctx, syms, lower_s, type_, cache);
                }
            } else if kind == crate::code::bound_kind::BoundKind::SUPER
                && s_kind == crate::code::bound_kind::BoundKind::EXTENDS
            {
                let upper_s = self.wild_upper_bound(ctx, syms, s);
                return self.not_soft_subtype_cached(ctx, syms, type_, upper_s, cache);
            }
            let _ = s_type;
            false
        } else {
            let mut result = self.not_soft_subtype_cached(ctx, syms, t, s, cache);
            if !result {
                result = self.not_soft_subtype_cached(ctx, syms, s, t, cache);
            }
            result
        }
    }

    fn not_soft_subtype_cached(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        cache: &mut Vec<(TypeId, TypeId)>,
    ) -> bool {
        let pair = (t, s);
        if cache.contains(&pair) {
            return false;
        }
        cache.push(pair);
        let result = self.not_soft_subtype(ctx, syms, t, s);
        cache.pop();
        result
    }

    /// Java: private List<Type> superClosure(Type t, Type s)
    /// (Types.java:4550-4560)
    fn super_closure(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        erasure_s: TypeId,
    ) -> List<TypeId> {
        let mut cl: List<TypeId> = List::nil();
        let mut interfaces = self.interfaces(ctx, syms, t);
        while interfaces.non_empty() {
            let interface = *interfaces.head();
            let interface_erasure = self.erasure(ctx, syms, interface);
            if self.is_subtype(ctx, syms, erasure_s, interface_erasure) {
                cl = self.closure_insert(ctx, syms, cl, interface);
            } else {
                let sub_closure = self.super_closure(ctx, syms, interface, erasure_s);
                cl = self.closure_union(ctx, syms, cl, sub_closure);
            }
            interfaces = interfaces.tail();
        }
        cl
    }

    /// Java: private boolean giveWarning(Type from, Type to)
    /// (Types.java:4532-4548)
    fn give_warning(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        from: TypeId,
        to: TypeId,
    ) -> bool {
        let bounds = if self.is_compound(ctx, to) {
            self.direct_supertypes(ctx, syms, to)
        } else {
            List::of(to)
        };
        let mut current = bounds;
        while current.non_empty() {
            let b = *current.head();
            let b_tsym = ctx.typ(b).tsym.expect("bound symbol");
            let sub_from = self.as_sub(ctx, syms, from, b_tsym);
            if is_parameterized_type(ctx, b)
                && !(self.is_unbounded(ctx, syms, b)
                    || self.is_subtype(ctx, syms, from, b)
                    || sub_from
                        .map(|sub_from| {
                            self.contains_type_equivalent(
                                ctx,
                                syms,
                                &self.class_allparams(ctx, b),
                                &self.class_allparams(ctx, sub_from),
                            )
                        })
                        .unwrap_or(false))
            {
                return true;
            }
            current = current.tail();
        }
        false
    }

    /// Java: private boolean sideCast(Type from, Type to, Warner warn)
    /// (Types.java:4479-4520; the unchecked-lint warnings are registered for
    /// the Attr/Lint)
    fn side_cast(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        from: TypeId,
        to: TypeId,
        warn: &mut crate::util::warner::Warner,
    ) -> bool {
        let mut reverse = false;
        let mut from = from;
        let mut to = to;
        let target = to;
        let to_tsym = ctx.typ(to).tsym.expect("cast target symbol");
        if (ctx.sym(to_tsym).flags_field & crate::code::flags::INTERFACE as i64) == 0 {
            let from_tsym = ctx.typ(from).tsym.expect("cast source symbol");
            assert!(
                (ctx.sym(from_tsym).flags_field & crate::code::flags::INTERFACE as i64) != 0,
                "AssertionError: sideCast expects an interface"
            );
            reverse = true;
            to = from;
            from = target;
        }
        let from_erasure = self.erasure(ctx, syms, from);
        let common_supers = self.super_closure(ctx, syms, to, from_erasure);
        let mut give_warning = common_supers.is_empty();
        let mut common_supers = common_supers;
        while common_supers.non_empty() {
            let common = *common_supers.head();
            let common_tsym = ctx.typ(common).tsym.expect("common super symbol");
            let t1 = self.as_super(ctx, syms, from, common_tsym);
            let t2 = common;
            if let Some(t1) = t1 {
                let t1_args = get_type_arguments(ctx, t1);
                let t2_args = get_type_arguments(ctx, t2);
                if self.disjoint_types(ctx, syms, t1_args, t2_args) {
                    return false;
                }
                give_warning = give_warning
                    || if reverse {
                        self.give_warning(ctx, syms, t2, t1)
                    } else {
                        self.give_warning(ctx, syms, t1, t2)
                    };
            }
            common_supers = common_supers.tail();
        }
        if give_warning {
            warn.warn(crate::code::lint::LintCategory::UNCHECKED);
        }
        true
    }

    /// Java: private boolean sideCastFinal(Type from, Type to, Warner warn)
    /// (Types.java:4522-4548)
    fn side_cast_final(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        from: TypeId,
        to: TypeId,
        warn: &mut crate::util::warner::Warner,
    ) -> bool {
        let mut reverse = false;
        let mut from = from;
        let mut to = to;
        let target = to;
        let to_tsym = ctx.typ(to).tsym.expect("cast target symbol");
        if (ctx.sym(to_tsym).flags_field & crate::code::flags::INTERFACE as i64) == 0 {
            let from_tsym = ctx.typ(from).tsym.expect("cast source symbol");
            assert!(
                (ctx.sym(from_tsym).flags_field & crate::code::flags::INTERFACE as i64) != 0,
                "AssertionError: sideCastFinal expects an interface"
            );
            reverse = true;
            to = from;
            from = target;
        }
        let from_tsym = ctx.typ(from).tsym.expect("final class symbol");
        assert!(
            (ctx.sym(from_tsym).flags_field & crate::code::flags::FINAL as i64) != 0,
            "AssertionError: sideCastFinal expects a final class"
        );
        let to_tsym = ctx.typ(to).tsym.expect("cast target symbol");
        let Some(t1) = self.as_super(ctx, syms, from, to_tsym) else {
            return false;
        };
        let t2 = to;
        let t1_args = get_type_arguments(ctx, t1);
        let t2_args = get_type_arguments(ctx, t2);
        if self.disjoint_types(ctx, syms, t1_args, t2_args) {
            return false;
        }
        // Java: if (giveWarning(t2, t1) || giveWarning(t1, t2))
        // warn.warn(LintCategory.UNCHECKED); return true;
        let _ = (reverse, target);
        if self.give_warning(ctx, syms, t2, t1) || self.give_warning(ctx, syms, t1, t2) {
            warn.warn(crate::code::lint::LintCategory::UNCHECKED);
        }
        true
    }

}


// ---------------------------------------------------------------------------
// Infer: Type.UndetVar operations (Type.java:1892-2250) and the
// UndetVar branches of the Types relations (Types.java:1214/1282/1463/1494/
// 1593/4229/2731).
// ---------------------------------------------------------------------------
impl Types {
    /// Java: public List<Type> getBounds(TypeVar t) (Types.java:2731-2742)
    pub fn get_bounds(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> List<TypeId> {
        let upper = ctx.typ(t).get_upper_bound().expect("type var upper bound");
        if ctx.typ(upper).has_tag(TypeTag::NONE) {
            return List::nil();
        }
        if self.is_erroneous(ctx, syms, upper) || !self.is_compound(ctx, upper) {
            return List::of(upper);
        }
        let erasure = self.erasure(ctx, syms, t);
        let erasure_tsym = ctx.typ(erasure).tsym.expect("erasure symbol");
        if (ctx.sym(erasure_tsym).flags_field & crate::code::flags::INTERFACE as i64) == 0 {
            let interfaces = self.interfaces(ctx, syms, t);
            let supertype = self.supertype(ctx, syms, t);
            interfaces.prepend(supertype)
        } else {
            self.interfaces(ctx, syms, t)
        }
    }

    /// Java: public boolean isErroneous() (Type.java:564/1111/1377/1493/1811/
    /// 1850/2397)
    #[allow(clippy::only_used_in_recursion)]
    pub(crate) fn is_erroneous(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> bool {
        let kind = ctx.typ(t).kind.clone();
        let tsym_opt = ctx.typ(t).tsym;
        match &kind {
            TypeKindData::ErrorType { .. } => true,
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
                if let Some(enclosing) = get_enclosing_type(ctx, t)
                    && self.is_erroneous(ctx, syms, enclosing)
                {
                    return true;
                }
                let mut arguments = get_type_arguments(ctx, t);
                while arguments.non_empty() {
                    if self.is_erroneous(ctx, syms, *arguments.head()) {
                        return true;
                    }
                    arguments = arguments.tail();
                }
                let Some(tsym) = tsym_opt else {
                    return false;
                };
                let Some(tsym_type) = ctx.sym(tsym).ty else {
                    return false;
                };
                tsym_type != t && self.is_erroneous(ctx, syms, tsym_type)
            }
            TypeKindData::ArrayType { elemtype, .. } => elemtype
                .map(|elem| self.is_erroneous(ctx, syms, elem))
                .unwrap_or(false),
            TypeKindData::MethodType {
                argtypes, restype, ..
            } => {
                let mut arguments = argtypes.clone();
                while arguments.non_empty() {
                    if self.is_erroneous(ctx, syms, *arguments.head()) {
                        return true;
                    }
                    arguments = arguments.tail();
                }
                restype
                    .map(|restype| self.is_erroneous(ctx, syms, restype))
                    .unwrap_or(false)
            }
            TypeKindData::ForAll { qtype, .. } => qtype
                .map(|qtype| self.is_erroneous(ctx, syms, qtype))
                .unwrap_or(false),
            _ => false,
        }
    }

    /// The origin type variable of an UndetVar (DelegatedType.qtype).
    pub fn undet_var_qtype(&self, ctx: &CompilerContext, uv: TypeId) -> TypeId {
        match &ctx.typ(uv).kind {
            TypeKindData::UndetVar { qtype, .. } => *qtype,
            _ => panic!("ClassCastException: not an UndetVar"),
        }
    }

    fn undet_var_kind(&self, ctx: &CompilerContext, uv: TypeId) -> crate::code::type_::UndetVarKind {
        match &ctx.typ(uv).kind {
            TypeKindData::UndetVar { kind, .. } => *kind,
            _ => panic!("ClassCastException: not an UndetVar"),
        }
    }

    /// Java: public UndetVar(TypeVar origin, UndetVarListener listener,
    /// Types types) (Type.java:1965-2000)
    pub fn undet_var_new(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        origin: TypeId,
        listener: Option<std::rc::Rc<dyn crate::comp::infer::UndetVarListener>>,
    ) -> TypeId {
        let captured = ctx.typ(origin).is_captured();
        let kind = if captured {
            crate::code::type_::UndetVarKind::CAPTURED
        } else {
            crate::code::type_::UndetVarKind::NORMAL
        };
        let declared_bounds = self.get_bounds(ctx, syms, origin);
        let declared_count = declared_bounds.length() as usize;
        let tsym = ctx.typ(origin).tsym;
        let uv = ctx.alloc_type(Type::new(
            TypeTag::UNDETVAR,
            tsym,
            TypeKindData::UndetVar {
                qtype: origin,
                inst: None,
                bounds: [List::nil(), List::nil(), List::nil()],
                declared_count,
                kind,
                listener,
                incorporation_actions: std::collections::VecDeque::new(),
            },
        ));
        // Java: for (Type t : declaredBounds.reverse()) addBound(UPPER, t, types, true)
        let mut reversed: Vec<TypeId> = Vec::new();
        let mut current = declared_bounds;
        while current.non_empty() {
            reversed.push(*current.head());
            current = current.tail();
        }
        for t in reversed.into_iter().rev() {
            self.undet_var_add_bound(
                ctx,
                syms,
                uv,
                crate::code::type_::InferenceBound::UPPER,
                t,
                true,
            );
        }
        if captured {
            let lower = ctx.typ(origin).get_lower_bound().expect("captured lower bound");
            if !ctx.typ(lower).has_tag(TypeTag::BOT) {
                self.undet_var_add_bound(
                    ctx,
                    syms,
                    uv,
                    crate::code::type_::InferenceBound::LOWER,
                    lower,
                    true,
                );
            }
        }
        uv
    }

    /// Java: public List<Type> getBounds(InferenceBound... ibs)
    pub fn undet_var_get_bounds(
        &self,
        ctx: &CompilerContext,
        uv: TypeId,
        ibs: &[crate::code::type_::InferenceBound],
    ) -> List<TypeId> {
        let bounds = match &ctx.typ(uv).kind {
            TypeKindData::UndetVar { bounds, .. } => bounds.clone(),
            _ => panic!("ClassCastException: not an UndetVar"),
        };
        let mut result: List<TypeId> = List::nil();
        for ib in ibs {
            let mut current = bounds[ib.ordinal()].clone();
            while current.non_empty() {
                result = result.append(*current.head());
                current = current.tail();
            }
        }
        result
    }

    /// Java: public List<Type> getDeclaredBounds()
    pub fn undet_var_get_declared_bounds(
        &self,
        ctx: &CompilerContext,
        uv: TypeId,
    ) -> List<TypeId> {
        let declared_count = match &ctx.typ(uv).kind {
            TypeKindData::UndetVar { declared_count, .. } => *declared_count,
            _ => panic!("ClassCastException: not an UndetVar"),
        };
        let mut result: List<TypeId> = List::nil();
        let mut count = 0;
        let mut current = self.undet_var_get_bounds(
            ctx,
            uv,
            &[crate::code::type_::InferenceBound::UPPER],
        );
        while current.non_empty() {
            if count == declared_count {
                break;
            }
            result = result.append(*current.head());
            count += 1;
            current = current.tail();
        }
        result
    }

    /// Java: public void setBounds(InferenceBound ib, List<Type> newBounds)
    pub fn undet_var_set_bounds(
        &self,
        ctx: &mut CompilerContext,
        uv: TypeId,
        ib: crate::code::type_::InferenceBound,
        new_bounds: List<TypeId>,
    ) {
        if let TypeKindData::UndetVar { bounds, .. } = &mut ctx.typ_mut(uv).kind {
            bounds[ib.ordinal()] = new_bounds;
        } else {
            panic!("ClassCastException: not an UndetVar");
        }
    }

    /// Java: public Type getInst()
    pub fn undet_var_get_inst(&self, ctx: &CompilerContext, uv: TypeId) -> Option<TypeId> {
        match &ctx.typ(uv).kind {
            TypeKindData::UndetVar { inst, .. } => *inst,
            _ => panic!("ClassCastException: not an UndetVar"),
        }
    }

    /// Java: public void setInst(Type inst)
    pub fn undet_var_set_inst(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        uv: TypeId,
        inst: TypeId,
    ) {
        let listener = match &ctx.typ(uv).kind {
            TypeKindData::UndetVar { listener, .. } => listener.clone(),
            _ => panic!("ClassCastException: not an UndetVar"),
        };
        if let TypeKindData::UndetVar { inst: slot, .. } = &mut ctx.typ_mut(uv).kind {
            *slot = Some(inst);
        } else {
            panic!("ClassCastException: not an UndetVar");
        }
        if let Some(listener) = listener {
            listener.var_instantiated(ctx, syms, self, uv);
        }
    }

    /// Java: public final boolean isCaptured()
    pub fn undet_var_is_captured(&self, ctx: &CompilerContext, uv: TypeId) -> bool {
        self.undet_var_kind(ctx, uv) == crate::code::type_::UndetVarKind::CAPTURED
    }

    /// Java: public final boolean isThrows()
    pub fn undet_var_is_throws(&self, ctx: &CompilerContext, uv: TypeId) -> bool {
        self.undet_var_kind(ctx, uv) == crate::code::type_::UndetVarKind::THROWS
    }

    /// Java: public void setThrow()
    pub fn undet_var_set_throw(&self, ctx: &mut CompilerContext, uv: TypeId) {
        if self.undet_var_kind(ctx, uv) == crate::code::type_::UndetVarKind::CAPTURED {
            panic!("IllegalStateException: invalid state transition");
        }
        if let TypeKindData::UndetVar { kind, .. } = &mut ctx.typ_mut(uv).kind {
            *kind = crate::code::type_::UndetVarKind::THROWS;
        } else {
            panic!("ClassCastException: not an UndetVar");
        }
    }

    /// Java: private void notifyBoundChange(InferenceBound ib, Type bound,
    /// boolean update)
    fn undet_var_notify_bound_change(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        uv: TypeId,
        ib: crate::code::type_::InferenceBound,
        bound: TypeId,
        update: bool,
    ) {
        let listener = match &ctx.typ(uv).kind {
            TypeKindData::UndetVar { listener, .. } => listener.clone(),
            _ => panic!("ClassCastException: not an UndetVar"),
        };
        if let Some(listener) = listener {
            listener.var_bound_changed(ctx, syms, self, uv, ib, bound, update);
        }
    }

    /// Java: public final void addBound(InferenceBound ib, Type bound,
    /// Types types) (Type.java:2090-2110) — the public entry with the
    /// mapCapturesToBounds rewriting.
    pub fn undet_var_add_bound(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        uv: TypeId,
        ib: crate::code::type_::InferenceBound,
        bound: TypeId,
        update: bool,
    ) {
        if self.map_captures_to_bounds && !update {
            let mut bound = bound;
            match ib {
                crate::code::type_::InferenceBound::LOWER => {
                    bound = self.cvar_upper_bound(ctx, bound);
                }
                crate::code::type_::InferenceBound::UPPER => {
                    let alt_bound = cvar_lower_bound(ctx, bound);
                    if !ctx.typ(alt_bound).has_tag(TypeTag::BOT) {
                        bound = alt_bound;
                    }
                }
                crate::code::type_::InferenceBound::EQ => {}
            }
            self.undet_var_add_bound_internal(ctx, syms, uv, ib, bound, update);
        } else {
            self.undet_var_add_bound_internal(ctx, syms, uv, ib, bound, update);
        }
    }

    /// Java: private void addBound(InferenceBound ib, Type bound, Types types,
    /// boolean update) (Type.java:2110-2143)
    fn undet_var_add_bound_internal(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        uv: TypeId,
        ib: crate::code::type_::InferenceBound,
        bound: TypeId,
        update: bool,
    ) {
        let kind = self.undet_var_kind(ctx, uv);
        if kind == crate::code::type_::UndetVarKind::CAPTURED && !update {
            // If the new incoming bound is itself a (regular) inference
            // variable, then we are allowed to propagate this inference
            // variable bounds to it.
            if ctx.typ(bound).has_tag(TypeTag::UNDETVAR)
                && !self.undet_var_is_captured(ctx, bound)
            {
                self.undet_var_add_bound_internal(
                    ctx,
                    syms,
                    bound,
                    ib.complement(),
                    uv,
                    false,
                );
            }
            return;
        }
        let mapped = self.map_undet_vars_to_typevars(ctx, syms, bound);
        let bound2 = self.base_type_of(ctx, mapped);
        let qtype = self.undet_var_qtype(ctx, uv);
        if bound == qtype {
            return;
        }
        let prev_bounds = self.undet_var_get_bounds(ctx, uv, &[ib]);
        let mut current = prev_bounds;
        while current.non_empty() {
            if self.is_same_type(ctx, syms, *current.head(), bound2) {
                return;
            }
            current = current.tail();
        }
        let new_bounds = self
            .undet_var_get_bounds(ctx, uv, &[ib])
            .prepend(bound2);
        self.undet_var_set_bounds(ctx, uv, ib, new_bounds);
        self.undet_var_notify_bound_change(ctx, syms, uv, ib, bound2, false);
    }

    /// Java: TypeMapping toTypeVarMap (Type.java:2144-2151): replace every
    /// UndetVar by its inst (or qtype) in a structural mapping.
    fn map_undet_vars_to_typevars(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> TypeId {
        let kind = ctx.typ(t).kind.clone();
        let tsym_opt = ctx.typ(t).tsym;
        match &kind {
            TypeKindData::UndetVar { inst, qtype, .. } => inst.unwrap_or(*qtype),
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
                let outer = get_enclosing_type(ctx, t);
                let outer1 = outer.map(|outer| self.map_undet_vars_to_typevars(ctx, syms, outer));
                let typarams = get_type_arguments(ctx, t);
                let mut typarams1: List<TypeId> = List::nil();
                let mut changed = false;
                let mut current = typarams.clone();
                while current.non_empty() {
                    let arg = *current.head();
                    let mapped = self.map_undet_vars_to_typevars(ctx, syms, arg);
                    if mapped != arg {
                        changed = true;
                    }
                    typarams1 = typarams1.append(mapped);
                    current = current.tail();
                }
                let outer_changed = match (outer, outer1) {
                    (Some(original), Some(mapped)) => original != mapped,
                    _ => false,
                };
                if !changed && !outer_changed {
                    t
                } else {
                    new_class_type(ctx, outer1, Some(typarams1), tsym_opt)
                }
            }
            TypeKindData::ArrayType { elemtype, .. } => {
                let elemtype = elemtype.expect("array element type");
                let elemtype1 = self.map_undet_vars_to_typevars(ctx, syms, elemtype);
                if elemtype1 == elemtype {
                    t
                } else {
                    new_array_type(ctx, elemtype1, tsym_opt)
                }
            }
            TypeKindData::WildcardType { type_, kind, bound } => {
                let Some(type_) = *type_ else {
                    return t;
                };
                let type1 = self.map_undet_vars_to_typevars(ctx, syms, type_);
                if type1 == type_ {
                    t
                } else {
                    new_wildcard_type(ctx, Some(type1), *kind, tsym_opt, *bound)
                }
            }
            TypeKindData::MethodType {
                argtypes,
                restype,
                thrown,
                ..
            } => {
                let argtypes1 = self.map_undet_vars_list(ctx, syms, argtypes.clone());
                let restype1 = restype.map(|restype| self.map_undet_vars_to_typevars(ctx, syms, restype));
                let thrown1 = self.map_undet_vars_list(ctx, syms, thrown.clone());
                let changed = !list_same(argtypes, &argtypes1)
                    || restype1 != *restype
                    || !list_same(thrown, &thrown1);
                if changed {
                    new_method_type_2(ctx, argtypes1, restype1, thrown1, tsym_opt)
                } else {
                    t
                }
            }
            TypeKindData::ForAll { tvars, qtype } => {
                let qtype = qtype.expect("for-all qtype");
                let qtype1 = self.map_undet_vars_to_typevars(ctx, syms, qtype);
                if qtype1 == qtype {
                    t
                } else {
                    ctx.alloc_type(Type::new(
                        TypeTag::FORALL,
                        tsym_opt,
                        TypeKindData::ForAll {
                            tvars: tvars.clone(),
                            qtype: Some(qtype1),
                        },
                    ))
                }
            }
            _ => t,
        }
    }

    fn map_undet_vars_list(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts: List<TypeId>,
    ) -> List<TypeId> {
        let mut result: List<TypeId> = List::nil();
        let mut current = ts;
        while current.non_empty() {
            result = result.append(self.map_undet_vars_to_typevars(ctx, syms, *current.head()));
            current = current.tail();
        }
        result
    }

    /// Java: UndetVar.baseType() (Type.java:2043-2048)
    pub(crate) fn base_type_of(&self, ctx: &mut CompilerContext, t: TypeId) -> TypeId {
        match &ctx.typ(t).kind {
            TypeKindData::UndetVar { .. } => match self.undet_var_get_inst(ctx, t) {
                Some(inst) => self.base_type_of(ctx, inst),
                None => t,
            },
            _ => t,
        }
    }

    /// Java: public UndetVar dup(Types types) (Type.java:2025-2030)
    pub fn undet_var_dup(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        uv: TypeId,
    ) -> TypeId {
        let qtype = self.undet_var_qtype(ctx, uv);
        let listener = match &ctx.typ(uv).kind {
            TypeKindData::UndetVar { listener, .. } => listener.clone(),
            _ => panic!("ClassCastException: not an UndetVar"),
        };
        let uv2 = self.undet_var_new(ctx, syms, qtype, listener);
        self.undet_var_dup_to(ctx, syms, uv, uv2);
        uv2
    }

    /// Java: public void dupTo(UndetVar uv2, Types types)
    /// (Type.java:2032-2050)
    pub fn undet_var_dup_to(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        uv: TypeId,
        uv2: TypeId,
    ) {
        // uv2.listener = null
        if let TypeKindData::UndetVar { listener, .. } = &mut ctx.typ_mut(uv2).kind {
            *listener = None;
        }
        for ib in crate::code::type_::InferenceBound::values() {
            self.undet_var_set_bounds(ctx, uv2, ib, List::nil());
            let mut bounds = self.undet_var_get_bounds(ctx, uv, &[ib]);
            while bounds.non_empty() {
                self.undet_var_add_bound(ctx, syms, uv2, ib, *bounds.head(), true);
                bounds = bounds.tail();
            }
        }
        let inst = self.undet_var_get_inst(ctx, uv);
        if let TypeKindData::UndetVar { inst: slot, .. } = &mut ctx.typ_mut(uv2).kind {
            *slot = inst;
        }
        let listener = match &ctx.typ(uv).kind {
            TypeKindData::UndetVar { listener, .. } => listener.clone(),
            _ => panic!("ClassCastException: not an UndetVar"),
        };
        let kind = self.undet_var_kind(ctx, uv);
        let actions = match &ctx.typ(uv).kind {
            TypeKindData::UndetVar {
                incorporation_actions,
                ..
            } => incorporation_actions.clone(),
            _ => panic!("ClassCastException: not an UndetVar"),
        };
        let mut new_actions = std::collections::VecDeque::new();
        for action in actions {
            new_actions.push_back(action.dup(uv2));
        }
        if let TypeKindData::UndetVar {
            listener: listener_slot,
            incorporation_actions: actions_slot,
            kind: kind_slot,
            ..
        } = &mut ctx.typ_mut(uv2).kind
        {
            *listener_slot = listener;
            *actions_slot = new_actions;
            *kind_slot = kind;
        }
    }

    /// Java: public void substBounds(List<Type> from, List<Type> to,
    /// Types types) (Type.java:2153-2190)
    pub fn undet_var_subst_bounds(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        uv: TypeId,
        from: &List<TypeId>,
        to: &List<TypeId>,
    ) {
        let previous_listener = match &ctx.typ(uv).kind {
            TypeKindData::UndetVar { listener, .. } => listener.clone(),
            _ => panic!("ClassCastException: not an UndetVar"),
        };
        let recorder = std::rc::Rc::new(BoundsChangeRecorder::default());
        if let TypeKindData::UndetVar { listener, .. } = &mut ctx.typ_mut(uv).kind {
            *listener = Some(recorder.clone());
        }
        for ib in crate::code::type_::InferenceBound::values() {
            let prev_bounds = self.undet_var_get_bounds(ctx, uv, &[ib]);
            let mut new_bounds: List<TypeId> = List::nil();
            let mut deps: Vec<TypeId> = Vec::new();
            let mut current = prev_bounds;
            while current.non_empty() {
                let t = *current.head();
                if !self.contains_any(ctx, syms, t, from) {
                    new_bounds = new_bounds.append(t);
                } else {
                    deps.push(t);
                }
                current = current.tail();
            }
            self.undet_var_set_bounds(ctx, uv, ib, new_bounds);
            for dep in deps {
                let substituted = self.subst_type(ctx, syms, dep, from, to);
                self.undet_var_add_bound(ctx, syms, uv, ib, substituted, true);
            }
        }
        if let TypeKindData::UndetVar { listener, .. } = &mut ctx.typ_mut(uv).kind {
            *listener = previous_listener;
        }
        let changes = recorder.changes.borrow().clone();
        for (ib, bound) in changes {
            self.undet_var_notify_bound_change(ctx, syms, uv, ib, bound, true);
        }
    }
}

/// The UndetVarListener used by UndetVar.substBounds to record bound changes
/// (Type.java:2156-2160).
#[derive(Default)]
struct BoundsChangeRecorder {
    changes: std::cell::RefCell<Vec<(crate::code::type_::InferenceBound, TypeId)>>,
}

impl crate::comp::infer::UndetVarListener for BoundsChangeRecorder {
    fn var_bound_changed(
        &self,
        _ctx: &mut CompilerContext,
        _syms: &Symtab,
        _types: &Types,
        _uv: TypeId,
        ib: crate::code::type_::InferenceBound,
        bound: TypeId,
        _update: bool,
    ) {
        self.changes.borrow_mut().push((ib, bound));
    }

    fn var_instantiated(
        &self,
        _ctx: &mut CompilerContext,
        _syms: &Symtab,
        _types: &Types,
        _uv: TypeId,
    ) {
        unimplemented!("TODO(Infer batch): BoundsChangeRecorder.varInstantiated");
    }
}


// ---------------------------------------------------------------------------
// Infer: the least-upper-bound machinery used by InferenceStep.LOWER
// (Types.java:3792-3960/3988-4090: intersect/merge/compoundMin/
// erasedSupertypes/lub). 
// ---------------------------------------------------------------------------
impl Types {
    /// Java: public List<Type> intersect(List<Type> cl1, List<Type> cl2)
    /// (Types.java:3811-3834)
    pub fn intersect(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        cl1: List<TypeId>,
        cl2: List<TypeId>,
    ) -> List<TypeId> {
        let mut merges: Vec<(TypeId, TypeId)> = Vec::new();
        self.intersect_internal(ctx, syms, cl1, cl2, &mut merges)
    }

    fn intersect_internal(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        cl1: List<TypeId>,
        cl2: List<TypeId>,
        merges: &mut Vec<(TypeId, TypeId)>,
    ) -> List<TypeId> {
        if list_same(&cl1, &cl2) {
            return cl1;
        }
        if cl1.is_empty() || cl2.is_empty() {
            return List::nil();
        }
        let head1 = *cl1.head();
        let head2 = *cl2.head();
        let sym1 = ctx.typ(head1).tsym.expect("closure head symbol");
        let sym2 = ctx.typ(head2).tsym.expect("closure head symbol");
        if self.symbol_precedes(ctx, syms, sym1, sym2) {
            return self.intersect_internal(ctx, syms, cl1.tail(), cl2, merges);
        }
        if self.symbol_precedes(ctx, syms, sym2, sym1) {
            return self.intersect_internal(ctx, syms, cl1, cl2.tail(), merges);
        }
        if self.is_same_type(ctx, syms, head1, head2) {
            return self
                .intersect_internal(ctx, syms, cl1.tail(), cl2.tail(), merges)
                .prepend(head1);
        }
        if sym1 == sym2
            && ctx.typ(head1).has_tag(TypeTag::CLASS)
            && ctx.typ(head2).has_tag(TypeTag::CLASS)
        {
            if is_parameterized_type(ctx, head1) && is_parameterized_type(ctx, head2) {
                let merge = self.merge_types(ctx, syms, head1, head2, merges);
                return self
                    .intersect_internal(ctx, syms, cl1.tail(), cl2.tail(), merges)
                    .prepend(merge);
            }
            if is_raw_type(ctx, head1) || is_raw_type(ctx, head2) {
                let erased = self.erasure(ctx, syms, head1);
                return self
                    .intersect_internal(ctx, syms, cl1.tail(), cl2.tail(), merges)
                    .prepend(erased);
            }
        }
        self.intersect_internal(ctx, syms, cl1.tail(), cl2.tail(), merges)
    }

    /// Java: private Type merge(Type c1, Type c2) (Types.java:3864-3900)
    fn merge_types(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        c1: TypeId,
        c2: TypeId,
        merges: &mut Vec<(TypeId, TypeId)>,
    ) -> TypeId {
        let act1 = get_type_arguments(ctx, c1);
        let act2 = get_type_arguments(ctx, c2);
        let c1_tsym = ctx.typ(c1).tsym.expect("class type symbol");
        let typarams_type = ctx.sym(c1_tsym).ty.expect("class symbol type");
        let mut typarams = get_type_arguments(ctx, typarams_type);
        let mut merged: List<TypeId> = List::nil();
        let mut act1 = act1;
        let mut act2 = act2;
        while act1.non_empty() && act2.non_empty() && typarams.non_empty() {
            let a1 = *act1.head();
            let a2 = *act2.head();
            let m = if self.contains_type(ctx, syms, a1, a2) {
                a1
            } else if self.contains_type(ctx, syms, a2, a1) {
                a2
            } else {
                let pair = (c1, c2);
                let already = merges.iter().any(|(x, y)| {
                    self.is_same_type(ctx, syms, *x, c1)
                        && self.is_same_type(ctx, syms, *y, c2)
                });
                if !already {
                    merges.push(pair);
                    let upper1 = self.wild_upper_bound(ctx, syms, a1);
                    let upper2 = self.wild_upper_bound(ctx, syms, a2);
                    let lub = self.lub_pair(ctx, syms, upper1, upper2);
                    merges.pop();
                    ctx.alloc_type(crate::code::type_::Type::new(
                        TypeTag::WILDCARD,
                        Some(syms.bound_class),
                        TypeKindData::WildcardType {
                            type_: Some(lub),
                            kind: crate::code::bound_kind::BoundKind::EXTENDS,
                            bound: None,
                        },
                    ))
                } else {
                    ctx.alloc_type(crate::code::type_::Type::new(
                        TypeTag::WILDCARD,
                        Some(syms.bound_class),
                        TypeKindData::WildcardType {
                            type_: Some(syms.object_type),
                            kind: crate::code::bound_kind::BoundKind::UNBOUND,
                            bound: None,
                        },
                    ))
                }
            };
            let formal = *typarams.head();
            let with_var = with_type_var(ctx, m, formal);
            merged = merged.append(with_var);
            act1 = act1.tail();
            act2 = act2.tail();
            typarams = typarams.tail();
        }
        assert!(
            act1.is_empty() && act2.is_empty() && typarams.is_empty(),
            "AssertionError: merge arity"
        );
        let outer = get_enclosing_type(ctx, c1);
        new_class_type(ctx, outer, Some(merged), Some(c1_tsym))
    }

    /// Java: private Type compoundMin(List<Type> cl) (Types.java:3902-3913)
    fn compound_min(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        names: &crate::util::names::Names,
        cl: List<TypeId>,
    ) -> TypeId {
        if cl.is_empty() {
            return syms.object_type;
        }
        let compound = self.closure_min(ctx, syms, cl);
        if compound.is_empty() {
            panic!("AssertionError: compoundMin on an empty minimal closure");
        } else if compound.tail().is_empty() {
            *compound.head()
        } else {
            self.make_intersection_type(ctx, syms, names, compound, false)
        }
    }

    /// Java: List<Type> erasedSupertypes(Type t) (Types.java:4040-4052)
    fn erased_supertypes(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> List<TypeId> {
        let mut result: List<TypeId> = List::nil();
        let mut sup = self.closure(ctx, syms, t);
        while sup.non_empty() {
            let sup_t = *sup.head();
            if ctx.typ(sup_t).has_tag(TypeTag::TYPEVAR) {
                result = result.append(sup_t);
            } else {
                result = result.append(self.erasure(ctx, syms, sup_t));
            }
            sup = sup.tail();
        }
        result
    }

    /// Java: public Type lub(List<Type> ts) (Types.java:3957-3959)
    pub fn lub_list(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        names: &crate::util::names::Names,
        ts: List<TypeId>,
    ) -> TypeId {
        let mut args: Vec<TypeId> = Vec::new();
        let mut current = ts;
        while current.non_empty() {
            args.push(*current.head());
            current = current.tail();
        }
        self.lub(ctx, syms, names, &args)
    }

    /// Java: public Type lub(Type... ts) (Types.java:3965-4035)
    pub fn lub(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        names: &crate::util::names::Names,
        ts: &[TypeId],
    ) -> TypeId {
        const UNKNOWN_BOUND: i32 = 0;
        const ARRAY_BOUND: i32 = 1;
        const CLASS_BOUND: i32 = 2;
        let mut kinds: Vec<i32> = vec![UNKNOWN_BOUND; ts.len()];
        let mut boundkind = UNKNOWN_BOUND;
        for (index, t) in ts.iter().enumerate() {
            let mut t = *t;
            match ctx.typ(t).get_tag() {
                TypeTag::CLASS => {
                    kinds[index] = CLASS_BOUND;
                    boundkind |= CLASS_BOUND;
                }
                TypeTag::ARRAY => {
                    kinds[index] = ARRAY_BOUND;
                    boundkind |= ARRAY_BOUND;
                }
                TypeTag::TYPEVAR => {
                    while ctx.typ(t).has_tag(TypeTag::TYPEVAR) {
                        t = ctx.typ(t).get_upper_bound().expect("type var upper bound");
                    }
                    if ctx.typ(t).has_tag(TypeTag::ARRAY) {
                        kinds[index] = ARRAY_BOUND;
                        boundkind |= ARRAY_BOUND;
                    } else {
                        kinds[index] = CLASS_BOUND;
                        boundkind |= CLASS_BOUND;
                    }
                }
                _ => {
                    kinds[index] = UNKNOWN_BOUND;
                    if ctx.typ(t).is_primitive() {
                        return syms.err_type;
                    }
                }
            }
        }
        match boundkind {
            0 => syms.bot_type,
            ARRAY_BOUND => {
                // calculate lub(A[], B[])
                let mut elements: Vec<TypeId> = Vec::new();
                for t in ts {
                    let elem = elemtype_of(ctx, syms, *t);
                    if ctx.typ(elem).is_primitive() {
                        let first = ts[0];
                        for other in &ts[1..] {
                            if !self.is_same_type(ctx, syms, first, *other) {
                                // lub(int[], B[]) is Cloneable & Serializable
                                return self.array_super_type(ctx, syms);
                            }
                        }
                        // all the array types are the same, return one
                        return first;
                    }
                    elements.push(elem);
                }
                // lub(A[], B[]) is lub(A, B)[]
                let lub = self.lub(ctx, syms, names, &elements);
                new_array_type(ctx, lub, Some(syms.array_class))
            }
            CLASS_BOUND => {
                // calculate lub(A, B)
                let mut start_idx = 0;
                for t in ts {
                    if ctx.typ(*t).has_tag(TypeTag::CLASS)
                        || ctx.typ(*t).has_tag(TypeTag::TYPEVAR)
                    {
                        break;
                    }
                    start_idx += 1;
                }
                assert!(start_idx < ts.len(), "AssertionError: lub startIdx");
                // step 1 - compute erased candidate set (EC)
                let mut cl = self.erased_supertypes(ctx, syms, ts[start_idx]);
                for t in &ts[start_idx + 1..] {
                    if ctx.typ(*t).has_tag(TypeTag::CLASS)
                        || ctx.typ(*t).has_tag(TypeTag::TYPEVAR)
                    {
                        let erased = self.erased_supertypes(ctx, syms, *t);
                        cl = self.intersect(ctx, syms, cl, erased);
                    }
                }
                // step 2 - compute minimal erased candidate set (MEC)
                let mec = self.closure_min(ctx, syms, cl);
                // step 3 - for each element G in MEC, compute lci(Inv(G))
                let mut candidates: List<TypeId> = List::nil();
                let mut mec_current = mec;
                while mec_current.non_empty() {
                    let erased_supertype = *mec_current.head();
                    let erased_tsym = ctx
                        .typ(erased_supertype)
                        .tsym
                        .expect("erased supertype symbol");
                    let mut lci: List<TypeId> = List::nil();
                    if let Some(first) = self.as_super(ctx, syms, ts[start_idx], erased_tsym) {
                        lci = List::of(first);
                    }
                    for t in &ts[start_idx + 1..] {
                        let super_type = self.as_super(ctx, syms, *t, erased_tsym);
                        let super_list = super_type.map(List::of).unwrap_or_else(List::nil);
                        lci = self.intersect(ctx, syms, lci, super_list);
                    }
                    let mut lci_current = lci;
                    while lci_current.non_empty() {
                        candidates = candidates.append(*lci_current.head());
                        lci_current = lci_current.tail();
                    }
                    mec_current = mec_current.tail();
                }
                // step 4 - let MEC be { G1, G2... Gn }, then we have that
                // lub = lci(Inv(G1)) & lci(Inv(G2)) &... & lci(Inv(Gn))
                self.compound_min(ctx, syms, names, candidates)
            }
            _ => {
                // calculate lub(A, B[])
                let mut classes: List<TypeId> =
                    List::of(self.array_super_type(ctx, syms));
                for (index, t) in ts.iter().enumerate() {
                    if kinds[index] != ARRAY_BOUND {
                        // Filter out any arrays
                        classes = classes.prepend(*t);
                    }
                }
                // lub(A, B[]) is lub(A, arraySuperType)
                self.lub_list(ctx, syms, names, classes)
            }
        }
    }

    /// Java: private Type lub(Type t, Type s) — the two-argument form used by
    /// merge (a convenience over lub(Type...)).
    fn lub_pair(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> TypeId {
        let names = self
            .names
            .clone()
            .expect("TODO(Types batch): lub needs the Names service (Types::with_names)");
        self.lub(ctx, syms, &names, &[t, s])
    }
}


// ---------------------------------------------------------------------------
// Infer: public wrappers used by comp/infer.rs (the UndetVar engine and
// the Infer incorporation actions), and the closureCollector helper used by
// Infer.CheckUpperBounds (Types.java:3738-3750).
// ---------------------------------------------------------------------------
impl Types {
    /// Java: MethodType.getParameterTypes() view (also for the ported
    /// PartiallyInferredMethodType, which extends MethodType upstream).
    pub fn get_parameter_types_public(
        &self,
        ctx: &CompilerContext,
        t: TypeId,
    ) -> List<TypeId> {
        match &ctx.typ(t).kind {
            TypeKindData::MethodType { argtypes, .. }
            | TypeKindData::PartiallyInferredMethodType { argtypes, .. } => argtypes.clone(),
            _ => List::nil(),
        }
    }

    /// Java: MethodType.getReturnType() view (see above).
    pub fn get_return_type_public(&self, ctx: &CompilerContext, t: TypeId) -> TypeId {
        match &ctx.typ(t).kind {
            TypeKindData::MethodType {
                restype: Some(restype),
                ..
            }
            | TypeKindData::PartiallyInferredMethodType {
                restype: Some(restype),
                ..
            } => *restype,
            _ => panic!("AssertionError: no return type"),
        }
    }

    /// Java: boolean isRaw() view (Types.isRaw is a private helper).
    pub fn is_raw_type_public(&self, ctx: &CompilerContext, t: TypeId) -> bool {
        is_raw_type(ctx, t)
    }

    /// Java: public Type cvarUpperBound(Type t) — public wrapper used by
    /// Resolve.MethodResultInfo.U(found).
    pub fn cvar_upper_bound_of(&self, ctx: &CompilerContext, t: TypeId) -> TypeId {
        cvar_upper_bound(ctx, t)
    }

    /// Java: boolean isParameterized() view used by Infer.getParameterizedSupers.
    pub fn is_parameterized_type_of(&self, ctx: &CompilerContext, t: TypeId) -> bool {
        is_parameterized_type(ctx, t)
    }

    /// Java: Type.getTypeArguments() view used by Infer's ReachabilityVisitor.
    pub fn get_type_arguments_public(
        &self,
        ctx: &CompilerContext,
        t: TypeId,
    ) -> List<TypeId> {
        get_type_arguments(ctx, t)
    }

    /// Java: Type.isErroneous() view used by Infer.instantiateAsUninferredVars.
    pub fn is_erroneous_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> bool {
        self.is_erroneous(ctx, syms, t)
    }

    /// Java: public List<Type> getBounds(TypeVar t) — the public name used by
    /// Infer.instantiateAsUninferredVars.
    pub fn get_bounds_of_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> List<TypeId> {
        self.get_bounds(ctx, syms, t)
    }

    /// The UndetVar incorporation action queue (upstream
    /// UndetVar.incorporationActions); the engine mutates it through these
    /// accessors because the type lives in the arena.
    pub fn undet_var_actions(
        &self,
        ctx: &CompilerContext,
        uv: TypeId,
    ) -> Vec<crate::comp::infer::IncorporationAction> {
        match &ctx.typ(uv).kind {
            TypeKindData::UndetVar {
                incorporation_actions,
                ..
            } => incorporation_actions.iter().cloned().collect(),
            _ => panic!("ClassCastException: not an UndetVar"),
        }
    }

    pub fn undet_var_add_actions(
        &self,
        ctx: &mut CompilerContext,
        uv: TypeId,
        actions: Vec<crate::comp::infer::IncorporationAction>,
    ) {
        if let TypeKindData::UndetVar {
            incorporation_actions,
            ..
        } = &mut ctx.typ_mut(uv).kind
        {
            for action in actions {
                incorporation_actions.push_back(action);
            }
        }
    }

    pub fn undet_var_add_action_front(
        &self,
        ctx: &mut CompilerContext,
        uv: TypeId,
        action: crate::comp::infer::IncorporationAction,
    ) {
        if let TypeKindData::UndetVar {
            incorporation_actions,
            ..
        } = &mut ctx.typ_mut(uv).kind
        {
            incorporation_actions.push_front(action);
        }
    }

    pub fn undet_var_pop_action(
        &self,
        ctx: &mut CompilerContext,
        uv: TypeId,
    ) -> Option<crate::comp::infer::IncorporationAction> {
        match &mut ctx.typ_mut(uv).kind {
            TypeKindData::UndetVar {
                incorporation_actions,
                ..
            } => incorporation_actions.pop_front(),
            _ => panic!("ClassCastException: not an UndetVar"),
        }
    }

    /// Java: Collector closureCollector(true, types::isSameType) as used by
    /// Infer.CheckUpperBounds (Types.java:3738-3750 + 922-925): merge the
    /// closures of ts with the isSameType skip predicate and minimize.
    pub fn closure_collect(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts: List<TypeId>,
    ) -> List<TypeId> {
        let mut closure: List<TypeId> = List::nil();
        let mut current = ts;
        while current.non_empty() {
            let t = *current.head();
            let t_closure = self.closure(ctx, syms, t);
            let mut c = t_closure;
            while c.non_empty() {
                let x = *c.head();
                closure = self.closure_insert_same_type(ctx, syms, closure, x);
                c = c.tail();
            }
            current = current.tail();
        }
        self.closure_min(ctx, syms, closure)
    }

    /// Java: closureCollector insert(cl, t, (t1,t2) -> isSameType(t1,t2)).
    fn closure_insert_same_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        cl: List<TypeId>,
        t: TypeId,
    ) -> List<TypeId> {
        if cl.is_empty() {
            return cl.prepend(t);
        }
        let head = *cl.head();
        if self.is_same_type(ctx, syms, t, head) {
            return cl;
        }
        let t_tsym = ctx.typ(t).tsym.expect("closure type symbol");
        let head_tsym = ctx.typ(head).tsym.expect("closure head symbol");
        if self.symbol_precedes(ctx, syms, t_tsym, head_tsym) {
            return cl.prepend(t);
        }
        self.closure_insert_same_type(ctx, syms, cl.tail(), t)
            .prepend(head)
    }
}


// ---------------------------------------------------------------------------
// Types : isAssignable/covariantReturnType/resultSubtype/
// returnTypeSubstitutable/isDerivedRaw/overrideEquivalent/overridesObjectMethod
// (Types.java:2338-2365, 2670-2700, 2810-2832, 4249-4300), 1:1.
// ---------------------------------------------------------------------------
impl Types {
    /// Java: public boolean isAssignable(Type t, Type s) (Types.java:2338-2340)
    pub fn is_assignable(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        let mut no_warnings = crate::util::warner::Warner::new(None);
        self.is_assignable_with_warn(ctx, syms, t, s, &mut no_warnings)
    }

    /// Java: public boolean isAssignable(Type t, Type s, Warner warn)
    /// (Types.java:2348-2365)
    pub fn is_assignable_with_warn(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        warn: &mut crate::util::warner::Warner,
    ) -> bool {
        if ctx.typ(t).has_tag(TypeTag::ERROR) {
            return true;
        }
        let t_tag = ctx.typ(t).get_tag();
        if t_tag.is_sub_range_of(TypeTag::INT)
            && let Some(const_value) = ctx.typ(t).const_value()
        {
            let value = match const_value {
                crate::util::constants::ConstantValue::Integer(v) => *v,
                crate::util::constants::ConstantValue::Character(v) => *v as i32,
                crate::util::constants::ConstantValue::Short(v) => *v as i32,
                crate::util::constants::ConstantValue::Byte(v) => *v as i32,
                _ => 0,
            };
            match ctx.typ(s).get_tag() {
                TypeTag::BYTE | TypeTag::CHAR | TypeTag::SHORT | TypeTag::INT => {
                    if ctx.typ(s).get_tag().check_range(value) {
                        return true;
                    }
                }
                TypeTag::CLASS => {
                    // Java: unboxedType(s) — read-only variant (unboxed_type_ro
                    // returns noType when there is no unboxed type).
                    let unboxed = self.unboxed_type_ro(ctx, syms, s);
                    let unboxed = if ctx
                        .typ(unboxed)
                        .has_tag(crate::code::type_tag::TypeTag::NONE)
                    {
                        s
                    } else {
                        unboxed
                    };
                    match ctx.typ(unboxed).get_tag() {
                        TypeTag::BYTE | TypeTag::CHAR | TypeTag::SHORT => {
                            return self.is_assignable_with_warn(ctx, syms, t, unboxed, warn);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        // Java: isConvertible(t, s, warn) — the warn parameter is not yet wired
        // in the ported isConvertible (Lint, registered); the boolean
        // relation is unchanged.
        let _ = warn;
        self.is_convertible(ctx, syms, t, s)
    }

    /// Java: public boolean covariantReturnType(Type t, Type s, Warner warner)
    /// (Types.java:4296-4303)
    pub fn covariant_return_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        t: TypeId,
        s: TypeId,
        warn: &mut crate::util::warner::Warner,
    ) -> bool {
        self.is_same_type(ctx, syms, t, s)
            || (!ctx.typ(t).is_primitive()
                && !ctx.typ(s).is_primitive()
                && self.is_assignable_with_warn(ctx, syms, t, s, warn))
    }

    /// Java: public boolean resultSubtype(Type t, Type s, Warner warner)
    /// (Types.java:4249-4257)
    pub fn result_subtype(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        t: TypeId,
        s: TypeId,
        warner: &mut crate::util::warner::Warner,
    ) -> bool {
        let tvars = crate::code::types::get_type_arguments(ctx, t);
        let svars = crate::code::types::get_type_arguments(ctx, s);
        let tres = method_return_type(ctx, t);
        let sres_full = method_return_type(ctx, s);
        let sres = {
            let substituted = self.subst(ctx, syms, &List::of(sres_full), &svars, &tvars);
            *substituted.head()
        };
        self.covariant_return_type(ctx, syms, tres, sres, warner)
    }

    /// Java: public boolean returnTypeSubstitutable(Type r1, Type r2)
    /// (Types.java:4261-4270)
    pub fn return_type_substitutable(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        r1: TypeId,
        r2: TypeId,
    ) -> bool {
        let mut no_warnings = crate::util::warner::Warner::new(None);
        if self.has_same_args(ctx, syms, r1, r2) {
            self.result_subtype(ctx, syms, r1, r2, &mut no_warnings)
        } else {
            let r1_res = method_return_type(ctx, r1);
            let r2_erasure = {
                let r2_res = method_return_type(ctx, r2);
                self.erasure(ctx, syms, r2_res)
            };
            self.covariant_return_type(ctx, syms, r1_res, r2_erasure, &mut no_warnings)
        }
    }

    /// Java: public Type returnTypeSubstitutable(Type r1, Type r2, Type r2res,
    /// Warner warner) (Types.java:4272-4290)
    pub fn return_type_substitutable_with(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        r1: TypeId,
        r2: TypeId,
        r2res: TypeId,
        warner: &mut crate::util::warner::Warner,
    ) -> bool {
        let r1_res = method_return_type(ctx, r1);
        if self.is_same_type(ctx, syms, r1_res, r2res) {
            return true;
        }
        if ctx.typ(r1_res).is_primitive() || ctx.typ(r2res).is_primitive() {
            return false;
        }
        if self.has_same_args(ctx, syms, r1, r2) {
            return self.covariant_return_type(ctx, syms, r1_res, r2res, warner);
        }
        // Java: isSubtypeUnchecked(r1.getReturnType(), r2res, warner) — the warn
        // parameter is registered with the Varargs (see is_castable_with_warn).
        if self.is_subtype_unchecked(ctx, syms, r1_res, r2res) {
            return true;
        }
        let r2_erasure = self.erasure(ctx, syms, r2res);
        if !self.is_subtype(ctx, syms, r1_res, r2_erasure) {
            return false;
        }
        warner.warn(crate::code::lint::LintCategory::UNCHECKED);
        true
    }

    /// Java: public boolean isDerivedRaw(Type t) (Types.java:2670-2700)
    pub fn is_derived_raw(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        t: TypeId,
    ) -> bool {
        if self.is_erroneous(ctx, syms, t) {
            return false;
        }
        if is_raw_type(ctx, t) {
            return true;
        }
        let super_type = self.supertype(ctx, syms, t);
        let no_type = ctx.sym(syms.no_symbol).ty.expect("no type");
        if super_type != no_type && self.is_derived_raw(ctx, syms, super_type) {
            return true;
        }
        let mut interfaces = self.interfaces(ctx, syms, t);
        while interfaces.non_empty() {
            let interface = *interfaces.head();
            if self.is_derived_raw(ctx, syms, interface) {
                return true;
            }
            interfaces = interfaces.tail();
        }
        false
    }

    /// Java: public boolean overrideEquivalent(Type t, Type s) (Types.java:2816-2820)
    pub fn override_equivalent(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
        if self.has_same_args(ctx, syms, t, s) {
            return true;
        }
        let s_erasure = self.erasure(ctx, syms, s);
        if self.has_same_args(ctx, syms, t, s_erasure) {
            return true;
        }
        let t_erasure = self.erasure(ctx, syms, t);
        self.has_same_args(ctx, syms, t_erasure, s)
    }

    /// Java: public boolean overridesObjectMethod(TypeSymbol origin, Symbol msym)
    /// (Types.java:2822-2830)
    pub fn overrides_object_method(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        origin: SymbolId,
        msym: SymbolId,
    ) -> bool {
        let object_tsym = ctx.typ(syms.object_type).tsym.expect("object type symbol");
        let members = match &ctx.sym(object_tsym).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
            _ => None,
        };
        if let Some(members) = members {
            let name = ctx.sym(msym).name.clone();
            let found = ctx.scope_get_symbols_by_name(
                members,
                &name,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            );
            for sym in found {
                if crate::code::symbol::overrides(ctx, syms, self, msym, sym, origin, true) {
                    return true;
                }
            }
        }
        false
    }
}

/// Java: Type.getReturnType() for method types (Types.java:4253/4266/4279).
fn method_return_type(ctx: &CompilerContext, t: TypeId) -> TypeId {
    match &ctx.typ(t).kind {
        TypeKindData::MethodType { restype, .. } => restype.expect("method return type"),
        TypeKindData::ForAll { qtype, .. } => {
            let qtype = qtype.expect("forall qtype");
            method_return_type(ctx, qtype)
        }
        _ => panic!("IllegalArgumentException: Not a method type"),
    }
}


// ---------------------------------------------------------------------------
// Types : membersClosure/MembersClosureCache/implementation/
// ImplementationCache/interfaceCandidates/prune (Types.java:2931-3095,
// 3150-3260), 1:1. The auxiliary Java inner classes are expressed as methods +
// explicit caches on the Types value (registered adapter).
// ---------------------------------------------------------------------------
impl Types {
    /// Java: public CompoundScope membersClosure(Type site, boolean skipInterface)
    /// (Types.java:3091-3095).
    ///
    /// PORTING_RULES Porting rules adapter: the upstream MembersScope view (which filters
    /// out interface-owned members for skipInterface) is expressed by passing the
    /// filter to the scope query at the call sites (implementationInternal).
    pub fn members_closure(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        site: TypeId,
    ) -> crate::ids::ScopeId {
        self.members_closure_visit(ctx, syms, site)
    }

    /// Java: MembersClosureCache.visit (Types.java:3040-3089)
    fn members_closure_visit(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        t: TypeId,
    ) -> crate::ids::ScopeId {
        use crate::code::type_::TypeKindData;
        match &ctx.typ(t).kind {
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
                let tsym = ctx.typ(t).tsym.expect("class symbol");
                if !self.members_closure_seen.borrow_mut().insert(tsym) {
                    // Java: return new CompoundScope(t.tsym); (empty scope for a
                    // circle/multi-path visit)
                    let scope = ctx.alloc_scope(crate::code::scope::Scope::new_compound_scope(Some(tsym)));
                    return scope;
                }
                let cached = self.members_closure_cache.borrow().get(&tsym).copied();
                if let Some(cached) = cached {
                    self.members_closure_seen.borrow_mut().remove(&tsym);
                    return cached;
                }
                let closure_scope =
                    ctx.alloc_scope(crate::code::scope::Scope::new_compound_scope(Some(tsym)));
                let interfaces = self.interfaces(ctx, syms, t);
                let mut current = interfaces;
                while current.non_empty() {
                    let iface = *current.head();
                    let sub = self.members_closure_visit(ctx, syms, iface);
                    ctx.scope_prepend_sub_scope(closure_scope, sub);
                    current = current.tail();
                }
                let super_type = self.supertype(ctx, syms, t);
                let super_scope = self.members_closure_visit(ctx, syms, super_type);
                ctx.scope_prepend_sub_scope(closure_scope, super_scope);
                let members = match &ctx.sym(tsym).kind_data {
                    crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => {
                        *members_field
                    }
                    _ => None,
                };
                if let Some(members) = members {
                    ctx.scope_prepend_sub_scope(closure_scope, members);
                }
                self.members_closure_cache
                    .borrow_mut()
                    .insert(tsym, closure_scope);
                self.members_closure_seen.borrow_mut().remove(&tsym);
                closure_scope
            }
            TypeKindData::TypeVar { .. } | TypeKindData::CapturedType { .. } => {
                let upper = ctx.typ(t).get_upper_bound().expect("type var upper bound");
                self.members_closure_visit(ctx, syms, upper)
            }
            _ => {
                let no_symbol = syms.no_symbol;
                ctx.alloc_scope(crate::code::scope::Scope::new_compound_scope(Some(no_symbol)))
            }
        }
    }

    /// Java: public MethodSymbol implementation(MethodSymbol ms, TypeSymbol
    /// origin, boolean checkResult, Predicate<Symbol> implFilter)
    /// (Types.java:3003-3005).
    ///
    /// PORTING_RULES Porting rules adapter: the only upstream implFilter is
    /// MethodSymbol.implementation_filter (non-synthetic methods), so the port
    /// applies that filter directly (see implementation_internal).
    pub fn implementation(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        ms: SymbolId,
        origin: SymbolId,
        check_result: bool,
    ) -> Option<SymbolId> {
        self.implementation_filtered(ctx, syms, ms, origin, check_result)
    }

    /// Java: ImplementationCache.get/implementationInternal
    /// (Types.java:2960-3001)
    pub fn implementation_filtered(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        ms: SymbolId,
        origin: SymbolId,
        check_result: bool,
    ) -> Option<SymbolId> {
        let origin_ty = ctx.sym(origin).ty.expect("origin type");
        let members = self.members_closure(ctx, syms, origin_ty);
        let mark = ctx.scope_get_mark(members);
        if let Some(entry) = self
            .implementation_cache
            .borrow()
            .get(&ms)
            .and_then(|per_origin| per_origin.get(&origin))
            .cloned()
            && entry.check_result == check_result
            && entry.prev_mark == mark
        {
            return entry.cached_impl;
        }
        let result = self.implementation_internal(ctx, syms, ms, origin, check_result);
        self.implementation_cache
            .borrow_mut()
            .entry(ms)
            .or_default()
            .insert(
                origin,
                ImplementationEntry {
                    cached_impl: result,
                    check_result,
                    prev_mark: mark,
                },
            );
        result
    }

    /// Java: ImplementationCache.implementationInternal (Types.java:2980-3001)
    fn implementation_internal(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        ms: SymbolId,
        origin: SymbolId,
        check_result: bool,
    ) -> Option<SymbolId> {
        let ms_name = ctx.sym(ms).name.clone();
        let synthetic_flag = crate::code::flags::SYNTHETIC as i64;
        let mut t = ctx.sym(origin).ty.expect("origin type");
        loop {
            let tag = ctx.typ(t).get_tag();
            if tag != crate::code::type_tag::TypeTag::CLASS
                && tag != crate::code::type_tag::TypeTag::TYPEVAR
            {
                break;
            }
            t = self.skip_type_vars(ctx, t);
            let c = ctx.typ(t).tsym.expect("class symbol");
            let members = match &ctx.sym(c).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
                _ => None,
            };
            let mut best_so_far: Option<SymbolId> = None;
            if let Some(members) = members {
                let found = ctx.scope_get_symbols_by_name(
                    members,
                    &ms_name,
                    crate::code::scope::LookupKind::NON_RECURSIVE,
                    &|sym| (ctx.sym(sym).flags_field & synthetic_flag) == 0,
                );
                for sym in found {
                    if crate::code::symbol::overrides(ctx, syms, self, sym, ms, origin, check_result) {
                        best_so_far = Some(sym);
                        let abstract_flag = crate::code::flags::ABSTRACT as i64;
                        if (ctx.sym(sym).flags_field & abstract_flag) == 0 {
                            break;
                        }
                    }
                }
            }
            if best_so_far.is_some() {
                return best_so_far;
            }
            let super_type = self.supertype(ctx, syms, t);
            t = super_type;
        }
        None
    }

    /// Java: public List<MethodSymbol> interfaceCandidates(Type site, MethodSymbol ms)
    /// (Types.java:3187-3202)
    pub fn interface_candidates(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        site: TypeId,
        ms: SymbolId,
    ) -> Vec<SymbolId> {
        if let Some(cached) = self.candidates_cache.borrow().get(&(site, ms)) {
            return cached.clone();
        }
        let site_tsym = ctx.typ(site).tsym.expect("site symbol");
        let ms_name = ctx.sym(ms).name.clone();
        let synthetic_flag = crate::code::flags::SYNTHETIC as i64;
        let mut candidates2: Vec<SymbolId> = Vec::new();
        let members = self.members_closure(ctx, syms, site);
        let filter = |ctx: &mut CompilerContext, syms2: &mut Symtab, s: SymbolId| -> bool {
            if ctx.sym(s).kind != crate::code::kinds::Kind::MTH
                || ctx.sym(s).name.to_java_string() != ms_name.to_java_string()
                || (ctx.sym(s).flags_field & synthetic_flag) != 0
            {
                return false;
            }
            if !crate::code::symbol::is_inherited_in(ctx, syms2, self, s, site_tsym) {
                return false;
            }
            let s_ty = ctx.sym(s).ty.expect("method type");
            let ms_ty = ctx.sym(ms).ty.expect("method type");
            let s_mt = self.member_type(ctx, syms2, site, s);
            let ms_mt = self.member_type(ctx, syms2, site, ms);
            let _ = (s_ty, ms_ty);
            self.override_equivalent(ctx, syms2, s_mt, ms_mt)
        };
        // Java: for (Symbol s : membersClosure(site, false).getSymbols(filter))
        // The closure filter needs &mut ctx; collect candidates by iterating the
        // closure symbols through the context query.
        let all = ctx.scope_get_symbols(
            members,
            crate::code::scope::LookupKind::RECURSIVE,
            &|_| true,
        );
        for s in all {
            if !filter(ctx, syms, s) {
                continue;
            }
            if ctx.typ(site).tsym.map(|sym| sym != site_tsym).unwrap_or(true) {
                let _ = site_tsym;
            }
            let site_is_interface =
                (ctx.sym(site_tsym).flags_field & crate::code::flags::INTERFACE as i64) != 0;
            let owner_is_interface = ctx
                .sym(s)
                .owner
                .map(|owner| (ctx.sym(owner).flags_field & crate::code::flags::INTERFACE as i64) != 0)
                .unwrap_or(false);
            if !site_is_interface && !owner_is_interface {
                let result = vec![s];
                self.candidates_cache
                    .borrow_mut()
                    .insert((site, ms), result.clone());
                return result;
            }
            if !candidates2.contains(&s) {
                candidates2.insert(0, s);
            }
        }
        let result = self.prune(ctx, syms, candidates2);
        self.candidates_cache
            .borrow_mut()
            .insert((site, ms), result.clone());
        result
    }

    /// Java: public List<MethodSymbol> prune(List<MethodSymbol> methods)
    /// (Types.java:3205-3219)
    pub fn prune(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        methods: Vec<SymbolId>,
    ) -> Vec<SymbolId> {
        let mut methods_min: Vec<SymbolId> = Vec::new();
        for m1 in methods.iter() {
            let m1_owner = ctx.sym(*m1).owner.expect("method owner");
            let mut is_min_m1 = true;
            for m2 in methods.iter() {
                if m1 == m2 {
                    continue;
                }
                let m2_owner = ctx.sym(*m2).owner.expect("method owner");
                if m2_owner != m1_owner {
                    let m2_owner_ty = ctx.sym(m2_owner).ty.expect("owner type");
                    if self.as_super(ctx, syms, m2_owner_ty, m1_owner).is_some() {
                        is_min_m1 = false;
                        break;
                    }
                }
            }
            if is_min_m1 {
                methods_min.push(*m1);
            }
        }
        methods_min
    }
}


// ---------------------------------------------------------------------------
// Types : createMethodTypeWithThrown / MostSpecificReturnCheck /
// mergeAbstracts (Types.java:2834-2930, 3519-3535), 1:1.
// adapter adapters: Types has no Check service field in the port (explicit services,
// ADR 0001) so mergeAbstracts takes the Check; the merged MethodSymbol's
// anonymous baseSymbol() override is carried as MergeAbstractsResult.base_symbol.
// ---------------------------------------------------------------------------

/// Java: Types.MostSpecificReturnCheck (Types.java:2834-2870).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MostSpecificReturnCheck {
    BASIC,
    RTS,
}

impl MostSpecificReturnCheck {
    /// Java: MostSpecificReturnCheck.test(Type mt1, Type mt2, Types types)
    pub fn test(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        types: &Types,
        mt1: TypeId,
        mt2: TypeId,
    ) -> bool {
        match self {
            MostSpecificReturnCheck::BASIC => {
                let tvars = crate::code::types::get_type_arguments(ctx, mt1);
                let svars = crate::code::types::get_type_arguments(ctx, mt2);
                let t = method_return_type(ctx, mt1);
                let sres = method_return_type(ctx, mt2);
                let s = {
                    let substituted = types.subst(ctx, syms, &List::of(sres), &svars, &tvars);
                    *substituted.head()
                };
                types.is_same_type(ctx, syms, t, s)
                    || (!ctx.typ(t).is_primitive()
                        && !ctx.typ(s).is_primitive()
                        && types.is_subtype(ctx, syms, t, s))
            }
            MostSpecificReturnCheck::RTS => {
                types.return_type_substitutable(ctx, syms, mt1, mt2)
            }
        }
    }
}

/// Java: Optional<Symbol> mergeAbstracts(...) result. base_symbol is the
/// upstream anonymous MethodSymbol.baseSymbol() override (see Porting rules note above).
#[derive(Clone, Debug)]
pub struct MergeAbstractsResult {
    pub symbol: SymbolId,
    pub base_symbol: SymbolId,
    pub thrown: List<TypeId>,
}

impl Types {
    /// Java: public Type createMethodTypeWithThrown(Type original,
    /// List<Type> newThrown) (Types.java:3519-3535)
    pub fn create_method_type_with_thrown(
        &self,
        ctx: &mut CompilerContext,
        original: TypeId,
        new_thrown: List<TypeId>,
    ) -> TypeId {
        match &ctx.typ(original).kind {
            TypeKindData::MethodType {
                argtypes, restype, ..
            } => {
                let argtypes = argtypes.clone();
                let restype = restype.expect("method return type");
                let method_class = ctx.typ(original).tsym.expect("method class");
                let mt = crate::code::type_::Type::new_method_type(
                    argtypes,
                    restype,
                    new_thrown,
                    method_class,
                );
                ctx.alloc_type(mt)
            }
            TypeKindData::ForAll { tvars, qtype } => {
                let tvars = tvars.clone();
                let qtype = qtype.expect("forall qtype");
                let inner = self.create_method_type_with_thrown(ctx, qtype, new_thrown);
                ctx.alloc_type(crate::code::type_::Type::new_for_all(tvars, inner))
            }
            _ => panic!("IllegalArgumentException: Not a method type"),
        }
    }

    /// Java: public Optional<Symbol> mergeAbstracts(List<Symbol> ambiguousInOrder,
    /// Type site, boolean sigCheck) (Types.java:2874-2930)
    pub fn merge_abstracts(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        chk: &crate::comp::check::Check,
        ambiguous_in_order: &[SymbolId],
        site: TypeId,
        sig_check: bool,
    ) -> Option<MergeAbstractsResult> {
        let head = *ambiguous_in_order.first().expect("ambiguous methods");
        let head_ty = ctx.sym(head).ty.expect("method type");
        let head_erasure = self.erasure(ctx, syms, head_ty);
        let erased_params = crate::code::types::get_parameter_types(ctx, head_erasure);
        let mut should_erase = false;
        for s in ambiguous_in_order.iter() {
            let s_ty = ctx.sym(*s).ty.expect("method type");
            let s_erasure = self.erasure(ctx, syms, s_ty);
            let s_params = crate::code::types::get_parameter_types(ctx, s_erasure);
            if (ctx.sym(*s).flags_field & crate::code::flags::ABSTRACT as i64) == 0
                || (sig_check && !self.is_same_types(ctx, syms, &erased_params, &s_params))
            {
                return None;
            }
            if ctx
                .typ(s_ty)
                .has_tag(crate::code::type_tag::TypeTag::FORALL)
            {
                should_erase = true;
            }
        }
        for msc in [MostSpecificReturnCheck::BASIC, MostSpecificReturnCheck::RTS] {
            'outer: for s in ambiguous_in_order.iter() {
                let s_ty = ctx.sym(*s).ty.expect("method type");
                let mt = self.member_type(ctx, syms, site, *s);
                let mut all_thrown = crate::code::types::get_thrown_types(ctx, mt);
                for s2 in ambiguous_in_order.iter() {
                    if s != s2 {
                        let mt2 = self.member_type(ctx, syms, site, *s2);
                        if !self.is_sub_signature(ctx, syms, mt, mt2)
                            || !msc.test(ctx, syms, self, mt, mt2)
                        {
                            continue 'outer;
                        }
                        let mut thrown2 = crate::code::types::get_thrown_types(ctx, mt2);
                        if !ctx
                            .typ(mt)
                            .has_tag(crate::code::type_tag::TypeTag::FORALL)
                            && should_erase
                        {
                            thrown2 = self.erasure_list(ctx, syms, &thrown2);
                        } else if ctx
                            .typ(mt)
                            .has_tag(crate::code::type_tag::TypeTag::FORALL)
                        {
                            assert!(
                                ctx.typ(mt2)
                                    .has_tag(crate::code::type_tag::TypeTag::FORALL),
                                "AssertionError: generic merge"
                            );
                            let mt2_args = crate::code::types::get_type_arguments(ctx, mt2);
                            let mt_args = crate::code::types::get_type_arguments(ctx, mt);
                            thrown2 = self.subst(ctx, syms, &thrown2, &mt2_args, &mt_args);
                        }
                        all_thrown = chk.intersect(ctx, syms, &all_thrown, &thrown2);
                    }
                }
                if all_thrown == crate::code::types::get_thrown_types(ctx, mt) {
                    return Some(MergeAbstractsResult {
                        symbol: *s,
                        base_symbol: *s,
                        thrown: all_thrown,
                    });
                }
                // Java: new MethodSymbol(s.flags(), s.name,
                // createMethodTypeWithThrown(s.type, allThrown), s.owner)
                // { baseSymbol() { return s; } }
                let flags = ctx.sym(*s).flags_field;
                let name = ctx.sym(*s).name.clone();
                let owner = ctx.sym(*s).owner;
                let new_ty = self.create_method_type_with_thrown(ctx, s_ty, all_thrown.clone());
                let merged = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
                    crate::code::kinds::Kind::MTH,
                    flags,
                    name,
                    Some(new_ty),
                    owner,
                    crate::code::symbol::SymbolKind::MethodSymbol {
                        extra_params: List::nil(),
                        captured_locals: List::nil(),
                        params: List::nil(),
                        default_value: None,
                    },
                ));
                let _ = (s, s_ty);
                return Some(MergeAbstractsResult {
                    symbol: merged,
                    base_symbol: *s,
                    thrown: all_thrown,
                });
            }
        }
        None
    }
}


// ---------------------------------------------------------------------------
// Types : DescriptorCache/DescriptorFilter/findDescriptorSymbol/
// findDescriptorType/isFunctionalInterface/removeWildcards
// (Types.java:640-885, 984-1010), 1:1. adapter adapters: the facade services
// (diags/chk/enter) are explicit parameters (ADR 0001); the upstream
// FunctionDescriptor.getType inner override is carried by
// FunctionDescriptorMerge (see mergeDescriptors).
// ---------------------------------------------------------------------------
impl Types {
    /// Java: private FunctionDescriptorLookupError failure(String msg, Object...)
    /// / failure(JCDiagnostic) (Types.java:804-810)
    fn descriptor_failure(
        &self,
        diags: &crate::util::jc_diagnostic::Factory,
        fragment: crate::resources::diagnostic_contract::FragmentInfo,
    ) -> FunctionDescriptorLookupError {
        let diag = diags.fragment(crate::util::jc_diagnostic::Fragment::from(fragment));
        FunctionDescriptorLookupError::new().set_message(diag)
    }

    /// Symbol display placeholder for the ADR 0002 P4 args.
    fn symbol_arg_for(
        &self,
        ctx: &CompilerContext,
        sym: SymbolId,
    ) -> crate::resources::diagnostic_contract::SymbolArg {
        crate::resources::diagnostic_contract::SymbolArg::placeholder(
            crate::comp::resolve::symbol_display_name(ctx, sym),
        )
    }

    /// Java: class DescriptorFilter implements Predicate<Symbol>
    /// (Types.java:984-1000)
    fn descriptor_filter(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        origin: SymbolId,
        sym: SymbolId,
    ) -> bool {
        if ctx.sym(sym).kind != crate::code::kinds::Kind::MTH {
            return false;
        }
        let flags_field = ctx.sym(sym).flags_field;
        let abstract_flag = crate::code::flags::ABSTRACT as i64;
        let default_flag = crate::code::flags::DEFAULT;
        if (flags_field & (abstract_flag | default_flag)) != abstract_flag {
            return false;
        }
        if self.overrides_object_method(ctx, syms, origin, sym) {
            return false;
        }
        let origin_ty = ctx.sym(origin).ty.expect("interface type");
        let candidates = self.interface_candidates(ctx, syms, origin_ty, sym);
        if candidates.is_empty() {
            return true;
        }
        let head = candidates[0];
        (ctx.sym(head).flags_field & default_flag) == 0
    }

    /// Java: DescriptorCache.get(TypeSymbol origin) (Types.java:715-730)
    pub fn desc_cache_get(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        diags: &crate::util::jc_diagnostic::Factory,
        chk: &crate::comp::check::Check,
        origin: SymbolId,
    ) -> Result<FunctionDescriptor, FunctionDescriptorLookupError> {
        let origin_ty = ctx.sym(origin).ty.expect("type symbol type");
        let members = self.members_closure(ctx, syms, origin_ty);
        let mark = ctx.scope_get_mark(members);
        if let Some(entry) = self.descriptor_cache.borrow().get(&origin).cloned()
            && entry.prev_mark == mark
        {
            return Ok(entry.cached_desc_res);
        }
        let desc_res = self.find_descriptor_internal(ctx, syms, diags, chk, origin, members)?;
        self.descriptor_cache.borrow_mut().insert(
            origin,
            DescriptorEntry {
                cached_desc_res: desc_res.clone(),
                prev_mark: mark,
            },
        );
        Ok(desc_res)
    }

    /// Java: DescriptorCache.findDescriptorInternal(TypeSymbol origin,
    /// CompoundScope membersCache) (Types.java:732-790)
    #[allow(clippy::too_many_arguments)]
    pub fn find_descriptor_internal(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        diags: &crate::util::jc_diagnostic::Factory,
        chk: &crate::comp::check::Check,
        origin: SymbolId,
        members: crate::ids::ScopeId,
    ) -> Result<FunctionDescriptor, FunctionDescriptorLookupError> {
        let origin_flags = ctx.sym(origin).flags_field;
        let is_interface = (origin_flags & crate::code::flags::INTERFACE as i64) != 0;
        let is_annotation = (origin_flags & crate::code::flags::ANNOTATION as i64) != 0;
        let is_sealed = is_interface && (origin_flags & crate::code::flags::SEALED) != 0;
        if !is_interface || is_annotation || is_sealed {
            // Java: throw failure("not.a.functional.intf", origin);
            return Err(self.descriptor_failure(
                diags,
                crate::resources::compiler_properties::fragments::not_a_functional_intf(
                    self.symbol_arg_for(ctx, origin),
                ),
            ));
        }
        let mut abstracts: Vec<SymbolId> = Vec::new();
        let origin_ty = ctx.sym(origin).ty.expect("interface type");
        let all = ctx.scope_get_symbols(
            members,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        for sym in all {
            if !self.descriptor_filter(ctx, syms, origin, sym) {
                continue;
            }
            let mtype = self.member_type(ctx, syms, origin_ty, sym);
            if abstracts.is_empty() {
                abstracts.push(sym);
            } else {
                let first = abstracts[0];
                let first_mtype = self.member_type(ctx, syms, origin_ty, first);
                if ctx.sym(sym).name.to_java_string()
                    == ctx.sym(first).name.to_java_string()
                    && self.override_equivalent(ctx, syms, mtype, first_mtype)
                {
                    // Java: if (!abstracts.stream().filter(msym ->
                    // msym.owner.isSubClass(sym.enclClass(), Types.this))
                    //.map(...).anyMatch(abstractMType -> isSubSignature(...)))
                    let sym_encl = crate::code::symbol::symbol_encl_class(ctx, sym)
                        .or_else(|| ctx.sym(sym).owner)
                        .expect("enclosing class");
                    let mut found_sub = false;
                    for msym in abstracts.iter() {
                        let msym_owner = ctx.sym(*msym).owner.expect("method owner");
                        if crate::code::symbol::is_sub_class(ctx, syms, self, msym_owner, sym_encl) {
                            let abstract_m_type =
                                self.member_type(ctx, syms, origin_ty, *msym);
                            if self.is_sub_signature(ctx, syms, abstract_m_type, mtype) {
                                found_sub = true;
                                break;
                            }
                        }
                    }
                    if !found_sub {
                        abstracts.push(sym);
                    }
                } else {
                    // Java: throw failure("not.a.functional.intf.1", origin,
                    // diags.fragment(Fragments.IncompatibleAbstracts(...)))
                    let kind_name = crate::code::kinds::kind_name(ctx.sym(origin), ctx); 
                    let fragment =
                        crate::resources::compiler_properties::fragments::incompatible_abstracts(
                            crate::resources::diagnostic_contract::KindNameArg::placeholder(
                                format!("{kind_name:?}"),
                            ),
                            self.symbol_arg_for(ctx, origin),
                        );
                    let diag = diags.fragment(crate::util::jc_diagnostic::Fragment::from(fragment));
                    return Err(FunctionDescriptorLookupError::new().set_message(diag));
                }
            }
        }
        if abstracts.is_empty() {
            // Java: throw failure("not.a.functional.intf.1", origin,
            // diags.fragment(Fragments.NoAbstracts(...)))
            let kind_name = crate::code::kinds::kind_name(ctx.sym(origin), ctx);
            let fragment =
                crate::resources::compiler_properties::fragments::no_abstracts(
                    crate::resources::diagnostic_contract::KindNameArg::placeholder(format!(
                        "{kind_name:?}"
                    )),
                    self.symbol_arg_for(ctx, origin),
                );
            let diag = diags.fragment(crate::util::jc_diagnostic::Fragment::from(fragment));
            return Err(FunctionDescriptorLookupError::new().set_message(diag));
        }
        if abstracts.len() == 1 {
            return Ok(FunctionDescriptor::new(abstracts[0]));
        }
        // Java: mergeDescriptors (Types.java:793-802 + 2930 note): the merged
        // descriptor uses baseSymbol() as its symbol and overrides getType to
        // reuse bestSoFar's thrown types (see FunctionDescriptorMerge).
        match self.merge_descriptors(ctx, syms, chk, origin, &abstracts) {
            Some(merge) => Ok(FunctionDescriptor {
                desc_sym: merge.base_symbol,
                merge: Some(FunctionDescriptorMerge {
                    best_so_far: merge.symbol,
                }),
            }),
            None => {
                // Java: build IncompatibleDescsInFunctionalIntf with the
                // descriptor.throws/descriptor fragment lines (JCDiagnostic.
                // MultilineDiagnostic). The ported JCDiagnostic has no
                // MultiLine form yet (registered adapter): the error is reported with
                // the primary diagnostic only, same key.
                let kind_name = crate::code::kinds::kind_name(ctx.sym(origin), ctx);
                let fragment =
                    crate::resources::compiler_properties::fragments::incompatible_descs_in_functional_intf(
                        crate::resources::diagnostic_contract::KindNameArg::placeholder(format!(
                            "{kind_name:?}"
                        )),
                        self.symbol_arg_for(ctx, origin),
                    );
                Err(self.descriptor_failure(diags, fragment))
            }
        }
    }

    /// Java: DescriptorCache.mergeDescriptors(TypeSymbol origin,
    /// List<Symbol> methodSyms) (Types.java:793-802)
    fn merge_descriptors(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        chk: &crate::comp::check::Check,
        origin: SymbolId,
        method_syms: &[SymbolId],
    ) -> Option<MergeAbstractsResult> {
        let origin_ty = ctx.sym(origin).ty.expect("interface type");
        self.merge_abstracts(ctx, syms, chk, method_syms, origin_ty, false)
    }

    /// Java: public Symbol findDescriptorSymbol(TypeSymbol origin)
    /// (Types.java:819-821)
    pub fn find_descriptor_symbol(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        diags: &crate::util::jc_diagnostic::Factory,
        chk: &crate::comp::check::Check,
        origin: SymbolId,
    ) -> Result<SymbolId, FunctionDescriptorLookupError> {
        Ok(self
            .desc_cache_get(ctx, syms, diags, chk, origin)?
            .get_symbol())
    }

    /// Java: public Type findDescriptorType(Type origin) (Types.java:827-829)
    pub fn find_descriptor_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        diags: &crate::util::jc_diagnostic::Factory,
        chk: &crate::comp::check::Check,
        origin: TypeId,
    ) -> Result<TypeId, FunctionDescriptorLookupError> {
        let origin_tsym = ctx.typ(origin).tsym.expect("origin type symbol");
        let descriptor = self.desc_cache_get(ctx, syms, diags, chk, origin_tsym)?;
        descriptor.get_type(ctx, syms, self, chk, origin)
    }

    /// Java: public boolean isFunctionalInterface(TypeSymbol tsym)
    /// (Types.java:834-841) / isFunctionalInterface(Type site) (843-850)
    pub fn is_functional_interface(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        diags: &crate::util::jc_diagnostic::Factory,
        chk: &crate::comp::check::Check,
        origin: SymbolId,
    ) -> bool {
        self.find_descriptor_symbol(ctx, syms, diags, chk, origin)
            .is_ok()
    }

    /// Java: public Type removeWildcards(Type site) (Types.java:852-885)
    pub fn remove_wildcards(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        site: TypeId,
    ) -> TypeId {
        let actuals = crate::code::types::get_type_arguments(ctx, site);
        let wildcard_tag = crate::code::type_tag::TypeTag::WILDCARD;
        let has_wildcard = actuals
            .iter()
            .any(|t| ctx.typ(t).has_tag(wildcard_tag));
        if !has_wildcard {
            return site;
        }
        let site_tsym = ctx.typ(site).tsym.expect("site symbol");
        let site_tsym_ty = ctx.sym(site_tsym).ty.expect("site symbol type");
        let formals = crate::code::types::get_type_arguments(ctx, site_tsym_ty);
        let mut args: List<TypeId> = List::nil();
        let mut current = actuals;
        let mut formals_iter = formals.clone();
        while current.non_empty() {
            let actual = *current.head();
            let formal = *formals_iter.head();
            let bound = ctx
                .typ(formal)
                .get_upper_bound()
                .expect("formal upper bound");
            if ctx.typ(actual).has_tag(wildcard_tag) {
                // Java: WildcardType wt = (WildcardType)actual;
                let (wt_type, wt_kind) = match &ctx.typ(actual).kind {
                    TypeKindData::WildcardType { type_, kind, .. } => (*type_, *kind),
                    _ => (None, crate::code::bound_kind::BoundKind::UNBOUND),
                };
                let contains_formals = self.contains_any(ctx, syms, bound, &formals);
                if contains_formals {
                    // Java: targs.add(wt.type); (may be null)
                    if let Some(wt_type) = wt_type {
                        args = args.append(wt_type);
                    }
                } else {
                    match wt_kind {
                        crate::code::bound_kind::BoundKind::UNBOUND => {
                            args = args.append(bound);
                        }
                        crate::code::bound_kind::BoundKind::EXTENDS => {
                            let wt_type = wt_type.expect("wildcard bound type");
                            let glb = self.glb(ctx, syms, bound, wt_type);
                            args = args.append(glb);
                        }
                        crate::code::bound_kind::BoundKind::SUPER => {
                            args = args.append(wt_type.expect("wildcard lower type"));
                        }
                    }
                }
            } else {
                args = args.append(actual);
            }
            current = current.tail();
            formals_iter = formals_iter.tail();
        }
        let substituted = self.subst(ctx, syms, &List::of(site_tsym_ty), &formals, &args);
        *substituted.head()
    }
}

impl FunctionDescriptor {
    /// Java: public Type getType(Type site) (Types.java:680-700)
    pub fn get_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        types: &Types,
        chk: &crate::comp::check::Check,
        site: TypeId,
    ) -> Result<TypeId, FunctionDescriptorLookupError> {
        let site = types.remove_wildcards(ctx, syms, site);
        if ctx.typ(site).is_intersection() {
            let components = ctx.typ(site).get_explicit_components();
            for component in components.iter() {
                if !chk.check_valid_generic_type(ctx, syms, component) {
                    // Java: throw failure(diags.fragment(
                    // Fragments.NoSuitableFunctionalIntfInst(site)))
                    return Err(FunctionDescriptorLookupError::new());
                }
            }
        } else if !chk.check_valid_generic_type(ctx, syms, site) {
            return Err(FunctionDescriptorLookupError::new());
        }
        let mt = types.member_type(ctx, syms, site, self.desc_sym);
        match &self.merge {
            None => Ok(mt),
            Some(merge) => {
                let best_so_far_ty = ctx.sym(merge.best_so_far).ty.expect("merged type");
                let best_thrown = crate::code::types::get_thrown_types(ctx, best_so_far_ty);
                Ok(types.create_method_type_with_thrown(ctx, mt, best_thrown))
            }
        }
    }
}


// ---------------------------------------------------------------------------
// Types : makeFunctionalInterfaceClass / functionalInterfaceBridges /
// pendingBridges (Types.java:890-975), 1:1. adapter adapters: the Enter service and
// the enclosing Env are explicit parameters (ADR 0001); the synthetic class
// symbol construction mirrors the upstream ClassSymbol constructor fields.
// ---------------------------------------------------------------------------
impl Types {
    /// Java: public ClassSymbol makeFunctionalInterfaceClass(Env<AttrContext> env,
    /// Name name, Type target, long cflags) (Types.java:890-916)
    #[allow(clippy::too_many_arguments)]
    pub fn make_functional_interface_class(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        diags: &crate::util::jc_diagnostic::Factory,
        chk: &crate::comp::check::Check,
        env: &std::rc::Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        name: crate::util::name::Name,
        target: TypeId,
        cflags: i64,
    ) -> Option<SymbolId> {
        let unknown_type = syms.unknown_type;
        if target == unknown_type {
            return None;
        }
        let target_tsym = ctx.typ(target).tsym.expect("target type symbol");
        let desc_sym = self
            .find_descriptor_symbol(ctx, syms, diags, chk, target_tsym)
            .ok()?;
        let desc_type = self
            .find_descriptor_type(ctx, syms, diags, chk, target)
            .ok()?;
        // Java: env.enclClass.sym.outermostClass()
        let encl_class = env.encl_class?;
        let encl_sym = match &ctx.tree(encl_class).kind {
            crate::tree::jc_tree::TreeKind::ClassDecl { sym, .. } => (*sym)?,
            _ => return None,
        };
        let mut outermost = encl_sym;
        loop {
            let owner = ctx.sym(outermost).owner;
            match owner {
                Some(owner) if ctx.sym(owner).kind != crate::code::kinds::Kind::PCK => {
                    outermost = owner;
                }
                _ => break,
            }
        }
        let no_type = no_type_of(ctx, syms);
        // Java: new ClassSymbol(cflags, name, syms.noSymbol) computes
        // fullname/flatname from (name, owner) in the constructor.
        let fullname = crate::code::symtab::form_full_name(ctx, &name, Some(outermost));
        let flatname = crate::code::symtab::form_flat_name(ctx, &name, Some(outermost));
        let mut class_symbol = crate::code::symbol::Symbol::new(
            crate::code::kinds::Kind::TYP,
            cflags,
            name,
            Some(no_type),
            Some(outermost),
            crate::code::symbol::SymbolKind::ClassSymbol {
                members_field: None,
                fullname,
                flatname,
                sourcefile: None,
                classfile: None,
                trans_local: List::nil(),
                record_components: List::nil(),
                permitted: List::nil(),
                is_permitted_explicit: false,
                annotation_type_metadata: Default::default(),
            },
        );
        class_symbol.completer = None;
        let csym = ctx.alloc_symbol(class_symbol);
        let members = ctx.scope_create(Some(csym));
        if let crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } =
            &mut ctx.sym_mut(csym).kind_data
        {
            *members_field = Some(members);
        }
        let desc_flags = ctx.sym(desc_sym).flags_field;
        let desc_name = ctx.sym(desc_sym).name.clone();
        let inst_desc = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            crate::code::kinds::Kind::MTH,
            desc_flags,
            desc_name,
            Some(desc_type),
            Some(csym),
            crate::code::symbol::SymbolKind::MethodSymbol {
                extra_params: List::nil(),
                captured_locals: List::nil(),
                params: List::nil(),
                default_value: None,
            },
        ));
        ctx.scope_enter(members, inst_desc);
        let ctype = ctx.alloc_type(crate::code::type_::Type::new_class_type(
            Some(no_type),
            Some(List::nil()),
            csym,
        ));
        let object_type = syms.object_type;
        let interfaces = if ctx.typ(target).is_intersection() {
            self.direct_supertypes(ctx, syms, target)
        } else {
            List::of(target)
        };
        if let crate::code::type_::TypeKindData::ClassType {
            supertype_field,
            interfaces_field,
            ..
        } = &mut ctx.typ_mut(ctype).kind
        {
            *supertype_field = Some(object_type);
            *interfaces_field = Some(interfaces);
        }
        ctx.sym_mut(csym).ty = Some(ctype);
        // Java: csym.sourcefile = ((ClassSymbol)csym.owner).sourcefile;
        let owner_sourcefile = match &ctx.sym(outermost).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile.clone(),
            _ => None,
        };
        if let crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } =
            &mut ctx.sym_mut(csym).kind_data
        {
            *sourcefile = owner_sourcefile;
        }
        Some(csym)
    }

    /// Java: private Predicate<Symbol> bridgeFilter (Types.java:948-955)
    fn bridge_filter(
        &self,
        ctx: &CompilerContext,
        sym: SymbolId,
    ) -> bool {
        let name = ctx.sym(sym).name.to_java_string();
        name != ctx.sym(sym).name.table.names().init.to_java_string()
            && name != ctx.sym(sym).name.table.names().clinit.to_java_string()
            && (ctx.sym(sym).flags_field & crate::code::flags::SYNTHETIC as i64) == 0
    }

    /// Java: public List<Symbol> functionalInterfaceBridges(TypeSymbol origin)
    /// (Types.java:925-947)
    pub fn functional_interface_bridges(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        diags: &crate::util::jc_diagnostic::Factory,
        chk: &crate::comp::check::Check,
        enter: &crate::comp::enter::Enter,
        origin: SymbolId,
    ) -> Vec<SymbolId> {
        assert!(
            self.is_functional_interface(ctx, syms, diags, chk, origin),
            "AssertionError: isFunctionalInterface(origin)"
        );
        let desc_sym = self
            .find_descriptor_symbol(ctx, syms, diags, chk, origin)
            .expect("descriptor symbol");
        let origin_ty = ctx.sym(origin).ty.expect("interface type");
        let members = self.members_closure(ctx, syms, origin_ty);
        let desc_name = ctx.sym(desc_sym).name.clone();
        let candidates = ctx.scope_get_symbols_by_name(
            members,
            &desc_name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        let mut overridden: Vec<SymbolId> = Vec::new();
        for m2 in candidates {
            if !self.bridge_filter(ctx, m2) {
                continue;
            }
            if m2 == desc_sym {
                continue;
            }
            if !crate::code::symbol::overrides(ctx, syms, self, desc_sym, m2, origin, false) {
                continue;
            }
            let m2_encl = crate::code::symbol::symbol_encl_class(ctx, m2)
                .or_else(|| ctx.sym(m2).owner)
                .expect("enclosing class");
            let mut skip = false;
            for m3 in overridden.iter() {
                let m3_ty = ctx.sym(*m3).ty.expect("bridge type");
                let m2_ty = ctx.sym(m2).ty.expect("bridge type");
                let m3_erasure = self.erasure(ctx, syms, m3_ty);
                let m2_erasure = self.erasure(ctx, syms, m2_ty);
                let same_erasure = self.is_same_type(ctx, syms, m3_erasure, m2_erasure);
                let overrides_m2 =
                    crate::code::symbol::overrides(ctx, syms, self, *m3, m2, origin, false);
                if same_erasure
                    || (overrides_m2
                        && {
                            let pending = self.pending_bridges(ctx, syms, enter, origin, m2_encl);
                            let binary = {
                                let m3_owner = ctx.sym(*m3).owner.expect("m3 owner");
                                crate::code::symbol::binary_implementation(
                                    ctx, syms, self, m2, m3_owner,
                                )
                                .is_some()
                            };
                            pending || binary
                        })
                {
                    skip = true;
                    break;
                }
            }
            if skip {
                continue;
            }
            overridden.push(m2);
        }
        overridden
    }

    /// Java: private boolean pendingBridges(ClassSymbol origin, TypeSymbol s)
    /// (Types.java:957-975)
    fn pending_bridges(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        enter: &crate::comp::enter::Enter,
        origin: SymbolId,
        s: SymbolId,
    ) -> bool {
        let class_file = match &ctx.sym(origin).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } => classfile.clone(),
            _ => None,
        };
        if let Some(class_file) = class_file
            && class_file.get_kind() == javac_tools::tools::java_file_object::Kind::CLASS
            && enter.get_env(origin).is_none()
        {
            return false;
        }
        if origin == s {
            return true;
        }
        let origin_ty = ctx.sym(origin).ty.expect("class type");
        let interfaces = self.interfaces(ctx, syms, origin_ty);
        let mut current = interfaces;
        while current.non_empty() {
            let iface = *current.head();
            let iface_tsym = ctx.typ(iface).tsym.expect("interface symbol");
            if self.pending_bridges(ctx, syms, enter, iface_tsym, s) {
                return true;
            }
            current = current.tail();
        }
        false
    }
}

impl Types {
    /// Java: private boolean isSubtypeUncheckedInternal(Type t, Type s,
    /// boolean capture, Warner warn) (Types.java:1020-1055) — the warning
    /// carrying variant; the boolean relation is identical to
    /// is_subtype_unchecked_internal.
    #[allow(clippy::too_many_arguments)]
    fn is_subtype_unchecked_internal_warn(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        capture: bool,
        warn: &mut crate::util::warner::Warner,
    ) -> bool {
        if ctx.typ(t).has_tag(TypeTag::ARRAY) && ctx.typ(s).has_tag(TypeTag::ARRAY) {
            let t_elem = elemtype_of(ctx, syms, t);
            let s_elem = elemtype_of(ctx, syms, s);
            if ctx.typ(t_elem).is_primitive() {
                return self.is_same_type(ctx, syms, t_elem, s_elem);
            }
            return self.is_subtype_unchecked_internal_warn(
                ctx, syms, t_elem, s_elem, false, warn,
            );
        }
        if self.is_subtype_with_capture(ctx, syms, t, s, capture) {
            return true;
        }
        if ctx.typ(t).has_tag(TypeTag::TYPEVAR) {
            let upper = ctx.typ(t).get_upper_bound().expect("type var upper bound");
            return self.is_subtype_unchecked_internal_warn(ctx, syms, upper, s, false, warn);
        }
        if !is_raw_type(ctx, s) {
            let Some(s_tsym) = ctx.typ(s).tsym else {
                return false;
            };
            if let Some(t2) = self.as_super(ctx, syms, t, s_tsym)
                && is_raw_type(ctx, t2)
            {
                // Java: if (isReifiable(s)) warn.silentWarn(UNCHECKED);
                // else warn.warn(UNCHECKED);
                if self.is_reifiable(ctx, syms, s) {
                    warn.silent_warn(crate::code::lint::LintCategory::UNCHECKED);
                } else {
                    warn.warn(crate::code::lint::LintCategory::UNCHECKED);
                }
                return true;
            }
        }
        false
    }

    /// Java: public boolean isSubtypeUnchecked(Type t, Type s, Warner warn)
    /// (Types.java:1011-1018)
    pub fn is_subtype_unchecked_with_warn(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        warn: &mut crate::util::warner::Warner,
    ) -> bool {
        let result = self.is_subtype_unchecked_internal_warn(ctx, syms, t, s, true, warn);
        if result {
            self.check_unsafe_varargs_conversion(ctx, syms, t, s, warn);
        }
        result
    }

    /// Java: private void checkUnsafeVarargsConversion(Type t, Type s, Warner
    /// warn) (Types.java:1057-1080)
    fn check_unsafe_varargs_conversion(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        warn: &mut crate::util::warner::Warner,
    ) {
        if !ctx.typ(t).has_tag(TypeTag::ARRAY) || self.is_reifiable(ctx, syms, t) {
            return;
        }
        // ArrayType from = (ArrayType)t;
        let should_warn = match ctx.typ(s).get_tag() {
            TypeTag::ARRAY => {
                // Java: from.isVarargs() && !to.isVarargs() && !isReifiable(from)
                ctx.typ(t).is_varargs()
                    && !ctx.typ(s).is_varargs()
                    && !self.is_reifiable(ctx, syms, t)
            }
            TypeTag::CLASS => ctx.typ(t).is_varargs(),
            _ => false,
        };
        if should_warn {
            warn.warn(crate::code::lint::LintCategory::VARARGS);
        }
    }

    /// Java: public boolean isConvertible(Type t, Type s, Warner warn)
    /// (Types.java:601-625) — the warning carrying variant.
    pub fn is_convertible_with_warn(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        warn: &mut crate::util::warner::Warner,
    ) -> bool {
        if ctx.typ(t).has_tag(TypeTag::ERROR) {
            return true;
        }
        let t_primitive = ctx.typ(t).is_primitive();
        let s_primitive = ctx.typ(s).is_primitive();
        if t_primitive == s_primitive {
            return self.is_subtype_unchecked_with_warn(ctx, syms, t, s, warn);
        }
        let t_undet = ctx.typ(t).has_tag(TypeTag::UNDETVAR);
        let s_undet = ctx.typ(s).has_tag(TypeTag::UNDETVAR);
        if t_undet || s_undet {
            return if t_undet {
                let boxed = self.boxed_type_or_type_ro(ctx, syms, s);
                self.is_subtype(ctx, syms, t, boxed)
            } else {
                let boxed = self.boxed_type_or_type_ro(ctx, syms, t);
                self.is_subtype(ctx, syms, boxed, s)
            };
        }
        if t_primitive {
            let boxed = self.boxed_class_ro(ctx, syms, t);
            let boxed_type = ctx.sym(boxed).ty.expect("boxed type");
            self.is_subtype(ctx, syms, boxed_type, s)
        } else {
            let unboxed = self.unboxed_type_ro(ctx, syms, t);
            self.is_subtype(ctx, syms, unboxed, s)
        }
    }
}

impl Types {
    /// Java: public boolean isDirectSuperInterface(TypeSymbol isym,
    /// TypeSymbol origin) (Types.java:2659-2665).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - isym: the interface symbol;
    ///   - origin: the class symbol whose direct interfaces are checked.
    ///
    /// # Returns
    ///   - true when isym is a direct super interface of origin.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Types.java:2659-2665.
    pub fn is_direct_super_interface(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        isym: crate::ids::SymbolId,
        origin: crate::ids::SymbolId,
    ) -> bool {
        let origin_type = ctx.sym(origin).ty.expect("origin class type");
        let mut interfaces = self.interfaces(ctx, syms, origin_type);
        while interfaces.non_empty() {
            let i = *interfaces.head();
            if ctx.typ(i).tsym == Some(isym) {
                return true;
            }
            interfaces = interfaces.tail();
        }
        false
    }
}

impl Types {
    /// Java: public List<Type> captures(Type t) (Types.java:431-436).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - t: the type to scan.
    ///
    /// # Returns
    ///   - the captured/synthetic type variables mentioned in t.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Types.java:431-436.
    pub fn captures(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> List<TypeId> {
        let mut seen: Vec<TypeId> = Vec::new();
        let mut result: List<TypeId> = List::nil();
        self.capture_scan_collect(ctx, syms, t, &mut seen, &mut result);
        result
    }

    /// Java: CaptureScanner (Types.java:441-482).
    #[allow(clippy::collapsible_if, clippy::collapsible_match)]
    fn capture_scan_collect(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        seen: &mut Vec<TypeId>,
        result: &mut List<TypeId>,
    ) {
        let kind = ctx.typ(t).kind.clone();
        match &kind {
            crate::code::type_::TypeKindData::ClassType { .. } => {
                if self.is_compound(ctx, t) {
                    let mut current = self.direct_supertypes(ctx, syms, t);
                    while current.non_empty() {
                        let sup = *current.head();
                        self.capture_scan_collect(ctx, syms, sup, seen, result);
                        current = current.tail();
                    }
                } else {
                    let mut current = crate::code::types::allparams_of(ctx, t);
                    while current.non_empty() {
                        let param = *current.head();
                        self.capture_scan_collect(ctx, syms, param, seen, result);
                        current = current.tail();
                    }
                }
            }
            crate::code::type_::TypeKindData::ArrayType { elemtype, .. } => {
                if let Some(elemtype) = elemtype {
                    self.capture_scan_collect(ctx, syms, *elemtype, seen, result);
                }
            }
            crate::code::type_::TypeKindData::WildcardType { type_, .. } => {
                if let Some(type_) = type_ {
                    self.capture_scan_collect(ctx, syms, *type_, seen, result);
                }
            }
            crate::code::type_::TypeKindData::TypeVar { bound, .. } => {
                let synthetic = ctx
                    .typ(t)
                    .tsym
                    .map(|tsym| {
                        ctx.sym(tsym).flags_field & (crate::code::flags::SYNTHETIC as i64) != 0
                    })
                    .unwrap_or(false);
                if synthetic && !seen.contains(&t) {
                    seen.push(t);
                    *result = result.prepend(t);
                    if let Some(bound) = bound {
                        self.capture_scan_collect(ctx, syms, *bound, seen, result);
                    }
                }
            }
            crate::code::type_::TypeKindData::CapturedType { bound, lower, .. } => {
                if !seen.contains(&t) {
                    seen.push(t);
                    *result = result.prepend(t);
                    if let Some(bound) = bound {
                        self.capture_scan_collect(ctx, syms, *bound, seen, result);
                    }
                    if let Some(lower) = lower {
                        self.capture_scan_collect(ctx, syms, *lower, seen, result);
                    }
                }
            }
            _ => {}
        }
    }

    /// Java: public Type upward(Type t, List<Type> vars) (Types.java:420-422).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - t: the type to project;
    ///   - vars: the type variables to map.
    ///
    /// # Returns
    ///   - the upward projection.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Types.java:420-422.
    pub fn upward(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        vars: &List<TypeId>,
    ) -> TypeId {
        let mut seen: Vec<TypeId> = Vec::new();
        self.projection(ctx, syms, t, vars, ProjectionKind::UPWARDS, &mut seen)
    }

    /// Java: TypeProjection.visit dispatch (Types.java:240-430).
    fn projection(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        vars: &List<TypeId>,
        pkind: ProjectionKind,
        seen: &mut Vec<TypeId>,
    ) -> TypeId {
        let tag = ctx.typ(t).get_tag();
        match tag {
            crate::code::type_tag::TypeTag::CLASS => {
                let (outer_field, typarams_field) = match &ctx.typ(t).kind {
                    crate::code::type_::TypeKindData::ClassType {
                        outer_field,
                        typarams_field,
                        ..
                    } => (*outer_field, typarams_field.clone()),
                    _ => (None, None),
                };
                if self.is_compound(ctx, t) {
                    let components = self.direct_supertypes(ctx, syms, t);
                    let mut projected: List<TypeId> = List::nil();
                    let mut current = components.clone();
                    let mut changed = false;
                    while current.non_empty() {
                        let component = *current.head();
                        let projected_component =
                            self.projection(ctx, syms, component, vars, pkind, seen);
                        if projected_component != component {
                            changed = true;
                        }
                        projected = projected.append(projected_component);
                        current = current.tail();
                    }
                    if !changed {
                        return t;
                    }
                    let names = self
                        .names
                        .as_ref()
                        .expect("Types::with_names must be used for intersection projection");
                    return self.make_intersection_type(ctx, syms, names, projected, false);
                }
                let outer = outer_field;
                let outer1 = outer.map(|outer| self.projection(ctx, syms, outer, vars, pkind, seen));
                let typarams = typarams_field.unwrap_or_else(List::nil);
                let tsym = ctx.typ(t).tsym.expect("class type symbol");
                let formals_type = ctx.sym(tsym).ty.expect("class symbol type");
                let mut formals = ctx.typ(formals_type).get_type_arguments();
                let mut typarams1: List<TypeId> = List::nil();
                let mut changed = false;
                let mut current = typarams.clone();
                while current.non_empty() {
                    let actual = *current.head();
                    let declared_bound = if formals.non_empty() {
                        let formal = *formals.head();
                        ctx.typ(formal).get_upper_bound().unwrap_or(syms.object_type)
                    } else {
                        syms.object_type
                    };
                    let t2 = self.map_type_argument(ctx, syms, t, declared_bound, actual, vars, pkind, seen);
                    if ctx.typ(t2).has_tag(crate::code::type_tag::TypeTag::BOT) {
                        return syms.bot_type;
                    }
                    if t2 != actual {
                        changed = true;
                    }
                    typarams1 = typarams1.append(t2);
                    if formals.non_empty() {
                        formals = formals.tail();
                    }
                    current = current.tail();
                }
                if outer1 == outer && !changed {
                    return t;
                }
                ctx.alloc_type(crate::code::type_::Type::new(
                    crate::code::type_tag::TypeTag::CLASS,
                    Some(tsym),
                    crate::code::type_::TypeKindData::ClassType {
                        outer_field: outer1,
                        typarams_field: Some(typarams1),
                        allparams_field: None,
                        supertype_field: None,
                        interfaces_field: None,
                        all_interfaces_field: None,
                    },
                ))
            }
            crate::code::type_tag::TypeTag::ARRAY => {
                let elemtype = match &ctx.typ(t).kind {
                    crate::code::type_::TypeKindData::ArrayType { elemtype, .. } => {
                        elemtype.expect("array element type")
                    }
                    _ => unreachable!(),
                };
                let elemtype1 = self.projection(ctx, syms, elemtype, vars, pkind, seen);
                if elemtype1 == elemtype {
                    return t;
                }
                if ctx.typ(elemtype1).has_tag(crate::code::type_tag::TypeTag::BOT) {
                    return syms.bot_type;
                }
                ctx.alloc_type(crate::code::type_::Type::new_array_type(
                    elemtype1,
                    syms.array_class,
                ))
            }
            crate::code::type_tag::TypeTag::TYPEVAR => {
                let (bound, lower) = match &ctx.typ(t).kind {
                    crate::code::type_::TypeKindData::TypeVar { bound, lower } => (*bound, *lower),
                    _ => (None, None),
                };
                if !self.list_contains_type(ctx, syms, vars, t) {
                    return t;
                }
                if seen.contains(&t) {
                    // cycle
                    return if pkind == ProjectionKind::UPWARDS {
                        syms.object_type
                    } else {
                        syms.bot_type
                    };
                }
                seen.push(t);
                let bound_type = match pkind {
                    ProjectionKind::UPWARDS => bound.unwrap_or(syms.object_type),
                    ProjectionKind::DOWNWARDS => lower.unwrap_or(syms.bot_type),
                };
                let result = self.projection(ctx, syms, bound_type, vars, pkind, seen);
                seen.pop();
                result
            }
            _ => t,
        }
    }

    /// Java: TypeProjection.mapTypeArgument (Types.java:326-330).
    #[allow(clippy::too_many_arguments)]
    fn map_type_argument(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        site: TypeId,
        declared_bound: TypeId,
        t: TypeId,
        vars: &List<TypeId>,
        pkind: ProjectionKind,
        seen: &mut Vec<TypeId>,
    ) -> TypeId {
        if !self.contains_any(ctx, syms, t, vars) {
            return t;
        }
        self.type_argument_projection(ctx, syms, site, declared_bound, t, vars, pkind, seen)
    }

    /// Java: TypeArgumentProjection.visit (Types.java:340-400).
    #[allow(clippy::too_many_arguments)]
    fn type_argument_projection(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        site: TypeId,
        declared_bound: TypeId,
        t: TypeId,
        vars: &List<TypeId>,
        pkind: ProjectionKind,
        seen: &mut Vec<TypeId>,
    ) -> TypeId {
        if ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::WILDCARD) {
            let (wt_type, wt_kind) = match &ctx.typ(t).kind {
                crate::code::type_::TypeKindData::WildcardType { type_, kind, .. } => {
                    (*type_, *kind)
                }
                _ => return t,
            };
            let mut bound = syms.bot_type;
            let mut bk = wt_kind;
            match wt_kind {
                crate::code::bound_kind::BoundKind::EXTENDS => {
                    if let Some(wt_type) = wt_type {
                        bound = self.projection(ctx, syms, wt_type, vars, pkind, seen);
                    }
                    if ctx.typ(bound).has_tag(crate::code::type_tag::TypeTag::BOT) {
                        return syms.bot_type;
                    }
                }
                crate::code::bound_kind::BoundKind::SUPER => {
                    if let Some(wt_type) = wt_type {
                        bound = self.projection(ctx, syms, wt_type, vars, pkind.complement(), seen);
                    }
                    if ctx.typ(bound).has_tag(crate::code::type_tag::TypeTag::BOT) {
                        bound = syms.object_type;
                        bk = crate::code::bound_kind::BoundKind::UNBOUND;
                    }
                }
                _ => {}
            }
            return ctx.alloc_type(crate::code::type_::Type::new_wildcard_type(
                bound,
                bk,
                syms.bound_class,
                None,
            ));
        }
        // type argument is some type containing restricted vars
        if pkind == ProjectionKind::DOWNWARDS {
            return syms.bot_type;
        }
        let upper = self.projection(ctx, syms, t, vars, ProjectionKind::UPWARDS, seen);
        let lower = self.projection(ctx, syms, t, vars, ProjectionKind::DOWNWARDS, seen);
        let site_tsym = ctx.typ(site).tsym.expect("site type symbol");
        let formals_type = ctx.sym(site_tsym).ty.expect("site symbol type");
        let formals = ctx.typ(formals_type).get_type_arguments();
        let object = syms.object_type;
        let (bound, bk) = if !self.is_same_type(ctx, syms, upper, object)
            && (self.contains_any(ctx, syms, declared_bound, &formals)
                || !self.is_subtype(ctx, syms, declared_bound, upper))
        {
            (upper, crate::code::bound_kind::BoundKind::EXTENDS)
        } else if !ctx.typ(lower).has_tag(crate::code::type_tag::TypeTag::BOT) {
            (lower, crate::code::bound_kind::BoundKind::SUPER)
        } else {
            (object, crate::code::bound_kind::BoundKind::UNBOUND)
        };
        ctx.alloc_type(crate::code::type_::Type::new_wildcard_type(
            bound,
            bk,
            syms.bound_class,
            None,
        ))
    }

    fn list_contains_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        list: &List<TypeId>,
        t: TypeId,
    ) -> bool {
        let mut current = list.clone();
        while current.non_empty() {
            if self.is_same_type(ctx, syms, *current.head(), t) {
                return true;
            }
            current = current.tail();
        }
        false
    }
}

/// Java: Types.ProjectionKind (Types.java:198-215).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionKind {
    /// Java: UPWARDS
    UPWARDS,
    /// Java: DOWNWARDS
    DOWNWARDS,
}

impl ProjectionKind {
    /// Java: ProjectionKind.complement()
    pub fn complement(self) -> ProjectionKind {
        match self {
            ProjectionKind::UPWARDS => ProjectionKind::DOWNWARDS,
            ProjectionKind::DOWNWARDS => ProjectionKind::UPWARDS,
        }
    }
}

impl Types {
    /// Java: public MethodSymbol firstUnimplementedAbstract(ClassSymbol sym)
    /// (Types.java:3095-3110).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - envs: the type environments (Enter.getEnv);
    ///   - allow_default_methods: the DEFAULT_METHODS feature flag;
    ///   - sym: the class symbol to inspect.
    ///
    /// # Returns
    ///   - the first unimplemented abstract method, if any.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Adapter: the upstream try/catch CompletionFailure body is not represented
    /// (the ported symbol layer reports completion failures explicitly).
    /// Upstream: Types.java:3095-3155.
    pub fn first_unimplemented_abstract(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        envs: &crate::comp::type_envs::TypeEnvs,
        allow_default_methods: bool,
        sym: SymbolId,
    ) -> Option<SymbolId> {
        let _ = envs;
        self.first_unimplemented_abstract_impl(ctx, syms, allow_default_methods, sym, sym)
    }

    /// Java: private MethodSymbol firstUnimplementedAbstractImpl(ClassSymbol impl,
    /// ClassSymbol c) (Types.java:3112-3155).
    #[allow(clippy::collapsible_if)]
    fn first_unimplemented_abstract_impl(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        allow_default_methods: bool,
        impl_sym: SymbolId,
        c: SymbolId,
    ) -> Option<SymbolId> {
        let mut undef: Option<SymbolId> = None;
        let c_flags = ctx.sym(c).flags_field;
        let is_abstract_or_interface = (c_flags
            & ((crate::code::flags::ABSTRACT as i64) | (crate::code::flags::INTERFACE as i64)))
            != 0;
        if c == impl_sym || is_abstract_or_interface {
            let members = match &ctx.sym(c).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
                _ => None,
            };
            if let Some(members) = members {
                let symbols = ctx.scope_get_symbols(members, crate::code::scope::LookupKind::NON_RECURSIVE, &|_| true);
                for absmeth in symbols {
                    if ctx.sym(absmeth).kind != crate::code::kinds::Kind::MTH {
                        continue;
                    }
                    let flags_value = ctx.sym(absmeth).flags_field;
                    let abstract_default_private = crate::code::flags::ABSTRACT as i64
                        | crate::code::flags::DEFAULT
                        | crate::code::flags::PRIVATE as i64;
                    if (flags_value & abstract_default_private) != crate::code::flags::ABSTRACT as i64 {
                        continue;
                    }
                    let mut implmeth = crate::code::symbol::implementation(
                        ctx,
                        syms,
                        self,
                        absmeth,
                        impl_sym,
                        true,
                    );
                    if implmeth.is_none() || implmeth == Some(absmeth) {
                        if allow_default_methods {
                            let impl_type = ctx.sym(impl_sym).ty.expect("implementation type");
                            let candidates =
                                self.interface_candidates(ctx, syms, impl_type, absmeth);
                            if let Some(prov) = candidates.first().copied() {
                                if crate::code::symbol::overrides(
                                    ctx, syms, self, prov, absmeth, impl_sym, true,
                                ) {
                                    implmeth = Some(prov);
                                }
                            }
                        }
                    }
                    if implmeth.is_none() || implmeth == Some(absmeth) {
                        undef = Some(absmeth);
                        break;
                    }
                }
            }
        }
        undef
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::source::Source;
    use crate::jvm::target::Target;
    use crate::util::names::Names;
    use std::rc::Rc;

    fn build() -> (CompilerContext, Symtab) {
        let mut ctx = CompilerContext::new();
        let names = Rc::new(Names::new(false));
        let symtab = Symtab::new(&mut ctx, names, Target::DEFAULT, Source::JDK17);
        (ctx, symtab)
    }

    fn build_with_types() -> (CompilerContext, Symtab, Types) {
        let mut ctx = CompilerContext::new();
        let names = Rc::new(Names::new(false));
        let symtab = Symtab::new(&mut ctx, Rc::clone(&names), Target::DEFAULT, Source::JDK17);
        let types = Types::with_names(names);
        (ctx, symtab, types)
    }

    fn class_type(ctx: &mut CompilerContext, tsym: SymbolId, args: &[TypeId]) -> TypeId {
        let symbol_type = ctx.sym(tsym).ty.expect("symbol type");
        let outer = get_enclosing_type(ctx, symbol_type);
        let mut list = List::nil();
        for arg in args.iter().rev() {
            list = list.prepend(*arg);
        }
        ctx.alloc_type(Type::new(
            TypeTag::CLASS,
            Some(tsym),
            TypeKindData::ClassType {
                outer_field: outer,
                typarams_field: Some(list),
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
            },
        ))
    }

    #[test]
    fn hash_code_matches_the_java_tag_hash_for_primitives() {
        let (mut ctx, symtab) = build();
        let types = Types::new();
        assert_eq!(types.hash_code(&mut ctx, symtab.int_type), 5);
        assert_eq!(types.hash_code(&mut ctx, symtab.byte_type), 0);
        assert_eq!(types.hash_code(&mut ctx, symtab.char_type), 1);
        assert_eq!(types.hash_code(&mut ctx, symtab.short_type), 2);
        assert_eq!(types.hash_code(&mut ctx, symtab.long_type), 3);
        assert_eq!(types.hash_code(&mut ctx, symtab.float_type), 4);
        assert_eq!(types.hash_code(&mut ctx, symtab.double_type), 6);
        assert_eq!(types.hash_code(&mut ctx, symtab.boolean_type), 7);
        assert_eq!(types.hash_code(&mut ctx, symtab.void_type), 8);
        assert_eq!(types.hash_code(&mut ctx, symtab.bot_type), 18);
        assert_eq!(types.hash_code(&mut ctx, symtab.err_type), 0);
        assert_eq!(types.hash_code(&mut ctx, symtab.unknown_type), 21);
        let no_type = ctx.sym(symtab.no_symbol).ty.expect("no type");
        assert_eq!(types.hash_code(&mut ctx, no_type), 19);
    }

    #[test]
    fn hash_code_of_arrays_adds_12() {
        let (mut ctx, symtab) = build();
        let types = Types::new();
        let int_array = ctx.alloc_type(Type::new(
            TypeTag::ARRAY,
            Some(symtab.array_class),
            TypeKindData::ArrayType {
                elemtype: Some(symtab.int_type),
                varargs: false,
            },
        ));
        assert_eq!(types.hash_code(&mut ctx, int_array), 5 + 12);
        let int_array_2d = ctx.alloc_type(Type::new(
            TypeTag::ARRAY,
            Some(symtab.array_class),
            TypeKindData::ArrayType {
                elemtype: Some(int_array),
                varargs: false,
            },
        ));
        assert_eq!(types.hash_code(&mut ctx, int_array_2d), 5 + 12 + 12);
    }

    #[test]
    fn is_same_type_covers_primitives_arrays_and_class_types() {
        let (mut ctx, symtab) = build();
        let types = Types::new();
        assert!(types.is_same_type(&mut ctx, &symtab, symtab.int_type, symtab.int_type));
        assert!(!types.is_same_type(&mut ctx, &symtab, symtab.int_type, symtab.long_type));
        assert!(!types.is_same_type(&mut ctx, &symtab, symtab.int_type, symtab.string_type));
        let string_tsym = ctx.typ(symtab.string_type).tsym.expect("string symbol");
        let string1 = class_type(&mut ctx, string_tsym, &[]);
        let string2 = class_type(&mut ctx, string_tsym, &[]);
        assert!(types.is_same_type(&mut ctx, &symtab, string1, string2));
        assert!(types.is_same_type(&mut ctx, &symtab, symtab.string_type, symtab.string_type));
        assert!(!types.is_same_type(&mut ctx, &symtab, symtab.string_type, symtab.object_type));
        let string_array = ctx.alloc_type(Type::new(
            TypeTag::ARRAY,
            Some(symtab.array_class),
            TypeKindData::ArrayType {
                elemtype: Some(symtab.string_type),
                varargs: false,
            },
        ));
        let string_array2 = ctx.alloc_type(Type::new(
            TypeTag::ARRAY,
            Some(symtab.array_class),
            TypeKindData::ArrayType {
                elemtype: Some(symtab.string_type),
                varargs: false,
            },
        ));        assert!(types.is_same_type(&mut ctx, &symtab, string_array, string_array2));
    }

    #[test]
    fn erasure_keeps_primitives_strings_and_arrays() {
        let (mut ctx, symtab) = build();
        let types = Types::new();
        assert_eq!(types.erasure(&mut ctx, &symtab, symtab.int_type), symtab.int_type);
        assert_eq!(
            types.erasure(&mut ctx, &symtab, symtab.string_type),
            symtab.string_type
        );
        let int_array = ctx.alloc_type(Type::new(
            TypeTag::ARRAY,
            Some(symtab.array_class),
            TypeKindData::ArrayType {
                elemtype: Some(symtab.int_type),
                varargs: false,
            },
        ));
        assert_eq!(types.erasure(&mut ctx, &symtab, int_array), int_array);
    }

    #[test]
    fn erasure_of_a_parameterized_class_type_returns_an_erased_class_type() {
        let (mut ctx, symtab) = build();
        let types = Types::new();
        let list_tsym = ctx.typ(symtab.list_type).tsym.expect("List symbol");
        let list_string = class_type(&mut ctx, list_tsym, &[symtab.string_type]);
        let erased = types.erasure(&mut ctx, &symtab, list_string);
        assert_ne!(erased, list_string);
        assert_eq!(ctx.typ(erased).get_tag(), TypeTag::CLASS);
        assert_eq!(ctx.typ(erased).tsym, Some(list_tsym));
        let args = get_type_arguments(&ctx, erased);
        assert!(args.is_empty());
        // the erasure is cached on the symbol (Symbol.erasure_field).
        let erased_again = types.erasure(&mut ctx, &symtab, list_string);
        assert_eq!(erased_again, erased);
        assert_eq!(ctx.sym(list_tsym).erasure_field, Some(erased));
    }

    #[test]
    fn boxed_class_enters_the_primitive_wrapper() {
        let (mut ctx, mut symtab) = build();
        let types = Types::new();
        for (ty, expected) in [
            (symtab.int_type, "java.lang.Integer"),
            (symtab.void_type, "java.lang.Void"),
            (symtab.boolean_type, "java.lang.Boolean"),
            (symtab.byte_type, "java.lang.Byte"),
        ] {
            let boxed = types.boxed_class(&mut ctx, &mut symtab, ty);
            let flatname = flat_name_of(&ctx, boxed);
            assert_eq!(flatname.to_java_string().to_string_lossy(), expected);
        }
    }

    #[test]
    fn symtab_class_field_is_cached_and_has_the_jvm_shape() {
        let (mut ctx, mut symtab) = build();
        let types = Types::new();
        let int_type = symtab.int_type;
        let field = symtab.get_class_field(&mut ctx, &types, int_type);
        let symbol = ctx.sym(field);
        assert_eq!(
            symbol.flags()
                & (crate::code::flags::STATIC | crate::code::flags::PUBLIC | crate::code::flags::FINAL)
                    as i64,
            (crate::code::flags::STATIC | crate::code::flags::PUBLIC | crate::code::flags::FINAL)
                as i64
        );
        assert_eq!(symbol.name.to_java_string().to_string_lossy(), "class");
        // cached: the second call returns the same VarSymbol.
        let field_again = symtab.get_class_field(&mut ctx, &types, int_type);
        assert_eq!(field_again, field);
        let field_type = ctx.sym(field).ty.expect("field type");
        assert_eq!(ctx.typ(field_type).get_tag(), TypeTag::CLASS);
        let args = get_type_arguments(&ctx, field_type);
        assert_eq!(args.length(), 1);
        let boxed = types.boxed_class(&mut ctx, &mut symtab, int_type);
        let boxed_type = ctx.sym(boxed).ty.expect("boxed type");
        assert_eq!(args.head(), &boxed_type);
    }

    #[test]
    fn contains_type_equivalent_matches_lists() {
        let (mut ctx, symtab) = build();
        let types = Types::new();
        assert!(types.contains_type_equivalent(
            &mut ctx,
            &symtab,
            &List::of(symtab.int_type),
            &List::of(symtab.int_type)
        ));
        let int_long = List::of(symtab.long_type).prepend(symtab.int_type);
        assert!(!types.contains_type_equivalent(
            &mut ctx,
            &symtab,
            &List::of(symtab.int_type),
            &int_long
        ));
    }

    #[test]
    fn elemtype_of_unwraps_arrays_and_wildcards() {
        let (mut ctx, symtab) = build();
        let int_array = new_array_type(&mut ctx, symtab.int_type, Some(symtab.array_class));
        assert_eq!(elemtype_of(&ctx, &symtab, int_array), symtab.int_type);
        // Java: WildcardType.type is the (upper) bound; kind=EXTENDS.
        let wildcard = new_wildcard_type(
            &mut ctx,
            Some(int_array),
            BoundKind::EXTENDS,
            None,
            None,
        );
        assert_eq!(elemtype_of(&ctx, &symtab, wildcard), symtab.int_type);
    }

    #[test]
    fn cvar_upper_bound_unwraps_captured_types() {
        let (mut ctx, symtab) = build();
        let captured = new_captured_type(&mut ctx, None, Some(symtab.string_type), None, None);
        assert_eq!(cvar_upper_bound(&ctx, captured), symtab.string_type);
        assert_eq!(cvar_upper_bound(&ctx, symtab.int_type), symtab.int_type);
    }

    #[test]
    fn get_thrown_types_reads_the_method_thrown_list() {
        let (mut ctx, symtab) = build();
        let method =
            new_method_type_2(&mut ctx, List::nil(), None, List::of(symtab.err_type), None);
        assert_eq!(get_thrown_types(&ctx, method), List::of(symtab.err_type));
        // Java: Type.ForAll.getThrownTypes() -> qtype.getThrownTypes().
        let for_all = ctx.alloc_type(Type::new(
            TypeTag::FORALL,
            None,
            TypeKindData::ForAll {
                tvars: List::of(TypeId::new(1)),
                qtype: Some(method),
            },
        ));
        assert_eq!(get_thrown_types(&ctx, for_all), List::of(symtab.err_type));
        assert_eq!(get_thrown_types(&ctx, symtab.int_type), List::nil());
    }

    #[test]
    fn type_var_and_captured_constructors_preserve_their_fields() {
        let (mut ctx, symtab) = build();
        let type_var = new_type_var_type(&mut ctx, None, Some(symtab.string_type), None);
        match &ctx.typ(type_var).kind {
            TypeKindData::TypeVar { bound, lower } => {
                assert_eq!(*bound, Some(symtab.string_type));
                assert_eq!(*lower, None);
            }
            _ => panic!("expected TypeVar"),
        }
        let wildcard = new_wildcard_type(&mut ctx, None, BoundKind::SUPER, None, None);
        let captured = new_captured_type(
            &mut ctx,
            None,
            Some(symtab.string_type),
            None,
            Some(wildcard),
        );
        match &ctx.typ(captured).kind {
            TypeKindData::CapturedType {
                bound,
                lower,
                wildcard: wc,
            } => {
                assert_eq!(*bound, Some(symtab.string_type));
                assert_eq!(*lower, None);
                assert_eq!(*wc, Some(wildcard));
            }
            _ => panic!("expected CapturedType"),
        }
    }

    #[test]
    fn type_contains_recognizes_parameterized_arguments() {
        let (mut ctx, symtab) = build();
        let list_tsym = ctx.typ(symtab.list_type).tsym.expect("list symbol");
        let parameterized = class_type(
            &mut ctx,
            list_tsym,
            &[symtab.string_type, symtab.int_type],
        );
        assert!(type_contains(
            &mut ctx,
            &symtab,
            parameterized,
            symtab.string_type
        ));
        assert!(!type_contains(
            &mut ctx,
            &symtab,
            parameterized,
            symtab.long_type
        ));
        assert!(type_contains(
            &mut ctx,
            &symtab,
            parameterized,
            parameterized
        ));
    }


    /// JDK 17 oracle (tools/gen-types-oracle/GenTypesOracle.java): isSameType
    /// and erasure results for the field types of TypesOracleSample, collected
    /// at development time (no JVM at test time).
    #[test]
    fn types_oracle_is_same_type_and_erasure() {
        let data = include_str!("../../tests/data/types-oracle.txt");
        let (mut ctx, mut symtab) = build();
        let types = Types::new();
        let list_tsym = ctx.typ(symtab.list_type).tsym.expect("list symbol");
        let int_type = symtab.int_type;
        let boxed_sym = types.boxed_class(&mut ctx, &mut symtab, int_type);
        let boxed = ctx.sym(boxed_sym).ty.expect("boxed type");
        let int_array = new_array_type(&mut ctx, symtab.int_type, Some(symtab.array_class));
        let string_array = new_array_type(&mut ctx, symtab.string_type, Some(symtab.array_class));
        let long_array = new_array_type(&mut ctx, symtab.long_type, Some(symtab.array_class));
        let long_array_2 = new_array_type(&mut ctx, long_array, Some(symtab.array_class));
        let list_string = class_type(&mut ctx, list_tsym, &[symtab.string_type]);
        let list_integer = class_type(&mut ctx, list_tsym, &[boxed]);
        let mut map: std::collections::HashMap<&str, TypeId> =
            std::collections::HashMap::new();
        map.insert("i", symtab.int_type);
        map.insert("z", symtab.boolean_type);
        map.insert("s", symtab.string_type);
        map.insert("o", symtab.object_type);
        map.insert("box", boxed);
        map.insert("ia", int_array);
        map.insert("sa", string_array);
        map.insert("la", long_array_2);
        map.insert("ls", list_string);
        map.insert("li", list_integer);
        let mut same_checked = 0usize;
        let mut erase_checked = 0usize;
        for line in data.lines() {
            let mut parts = line.split(' ');
            match parts.next().expect("oracle kind") {
                "SAME" => {
                    let a = parts.next().expect("type a");
                    let b = parts.next().expect("type b");
                    let expected: bool = parts.next().expect("expected").parse().expect("bool");
                    let ta = *map.get(a).expect("mapped type a");
                    let tb = *map.get(b).expect("mapped type b");
                    let got = types.is_same_type(&mut ctx, &symtab, ta, tb);
                    assert_eq!(got, expected, "isSameType({a}, {b})");
                    same_checked += 1;
                }
                "ERASE" => {
                    let a = parts.next().expect("type");
                    let expected = parts.next().expect("erased name");
                    let ta = *map.get(a).expect("mapped type");
                    let erased = types.erasure(&mut ctx, &symtab, ta);
                    let erased_tsym = ctx.typ(erased).tsym;
                    match expected {
                        "int" => assert_eq!(erased, symtab.int_type),
                        "boolean" => assert_eq!(erased, symtab.boolean_type),
                        "java.lang.String" => assert_eq!(erased, symtab.string_type),
                        "java.lang.Object" => {
                            let object_tsym =
                                ctx.typ(symtab.object_type).tsym.expect("object symbol");
                            assert_eq!(erased_tsym, Some(object_tsym));
                        }
                        "java.lang.Integer" => assert_eq!(erased_tsym, Some(boxed_sym)),
                        "int[]" => {
                            assert_eq!(ctx.typ(erased).get_tag(), TypeTag::ARRAY);
                            assert_eq!(elemtype_of(&ctx, &symtab, erased), symtab.int_type);
                        }
                        "java.lang.String[]" => {
                            assert_eq!(ctx.typ(erased).get_tag(), TypeTag::ARRAY);
                            assert_eq!(elemtype_of(&ctx, &symtab, erased), symtab.string_type);
                        }
                        "long[][]" => {
                            assert_eq!(ctx.typ(erased).get_tag(), TypeTag::ARRAY);
                            let elem = elemtype_of(&ctx, &symtab, erased);
                            assert_eq!(ctx.typ(elem).get_tag(), TypeTag::ARRAY);
                            assert_eq!(elemtype_of(&ctx, &symtab, elem), symtab.long_type);
                        }
                        "java.util.List" => assert_eq!(erased_tsym, Some(list_tsym)),
                        other => panic!("unexpected erasure name {other}"),
                    }
                    erase_checked += 1;
                }
                other => panic!("unexpected oracle kind {other}"),
            }
        }
        assert_eq!(same_checked, 100, "SAME oracle rows");
        assert_eq!(erase_checked, 10, "ERASE oracle rows");
    }


    /// Sets java.util.List<E> type parameters (E = Object upper bound) so that
    /// the parameterized-type relation tests can build List<...> instances.
    fn install_list_type_parameter(
        ctx: &mut CompilerContext,
        symtab: &Symtab,
    ) -> (SymbolId, TypeId) {
        let list_type = symtab.list_type;
        let list_tsym = ctx.typ(list_type).tsym.expect("list symbol");
        let type_param = new_type_var_type(ctx, Some(list_tsym), Some(symtab.object_type), None);
        if let TypeKindData::ClassType {
            typarams_field,
            allparams_field,
            ..
        } = &mut ctx.typ_mut(list_type).kind
        {
            *typarams_field = Some(List::of(type_param));
            *allparams_field = Some(List::of(type_param));
        }
        (list_tsym, type_param)
    }

    #[test]
    fn adapt_self_maps_the_class_definition_type_variables() {
        let (mut ctx, symtab, types) = build_with_types();
        let (list_tsym, type_param) = install_list_type_parameter(&mut ctx, &symtab);
        let list_string = class_type(&mut ctx, list_tsym, &[symtab.string_type]);
        let (from, to) = types.adapt_self(&mut ctx, &symtab, list_string);
        assert_eq!(from, vec![type_param]);
        assert_eq!(to, vec![symtab.string_type]);
    }

    #[test]
    fn rewrite_supers_rewrites_super_bound_wildcards_to_unbounded() {
        let (mut ctx, symtab, types) = build_with_types();
        let (list_tsym, _type_param) = install_list_type_parameter(&mut ctx, &symtab);
        let super_wildcard = new_wildcard_type(
            &mut ctx,
            Some(symtab.string_type),
            BoundKind::SUPER,
            Some(symtab.bound_class),
            None,
        );
        let list_wildcard = class_type(&mut ctx, list_tsym, &[super_wildcard]);
        let rewritten = types.rewrite_supers(&mut ctx, &symtab, list_wildcard);
        let args = type_arguments_list(&ctx, rewritten);
        assert_eq!(args.length(), 1);
        match &ctx.typ(*args.head()).kind {
            TypeKindData::WildcardType { kind, .. } => {
                assert_eq!(*kind, BoundKind::UNBOUND);
            }
            _ => panic!("expected a rewritten wildcard"),
        }
    }

    #[test]
    fn is_subtype_parameterized_types_use_contains_type_recursive() {
        let (mut ctx, symtab, types) = build_with_types();
        let (list_tsym, _type_param) = install_list_type_parameter(&mut ctx, &symtab);
        let list_string = class_type(&mut ctx, list_tsym, &[symtab.string_type]);
        let list_object = class_type(&mut ctx, list_tsym, &[symtab.object_type]);
        let unbounded = new_wildcard_type(
            &mut ctx,
            Some(symtab.object_type),
            BoundKind::UNBOUND,
            Some(symtab.bound_class),
            None,
        );
        let list_unbounded = class_type(&mut ctx, list_tsym, &[unbounded]);
        // List<String> <: List<String>
        assert!(types.is_subtype(&mut ctx, &symtab, list_string, list_string));
        // List<String> <: List<Object> is false (invariance)
        assert!(!types.is_subtype(&mut ctx, &symtab, list_string, list_object));
        // List<String> <: List<?> (contains the unbounded wildcard)
        assert!(types.is_subtype(&mut ctx, &symtab, list_string, list_unbounded));
        // List<String> <: Object (asSuper objectType optimization)
        assert!(types.is_subtype(&mut ctx, &symtab, list_string, symtab.object_type));
        // List<String> <: List<Integer> is false
        let list_int = class_type(&mut ctx, list_tsym, &[symtab.int_type]);
        assert!(!types.is_subtype(&mut ctx, &symtab, list_string, list_int));
    }


    /// JDK 17 internal-API oracle (tools/gen-types-relations-oracle/
    /// GenTypesRelationsOracle.java): isSubtype/isConvertible/isCastable/
    /// isParameterized results for the RelSample field types, collected at
    /// development time (no JVM at test time).
    #[test]
    fn types_relations_match_the_internal_api_oracle() {
        use crate::code::bound_kind::BoundKind;
        let data = include_str!("../../tests/data/types-relations-oracle.txt");
        let (mut ctx, symtab, types) = build_with_types();
        let (list_tsym, _type_param) = install_list_type_parameter(&mut ctx, &symtab);
        let unbounded = new_wildcard_type(
            &mut ctx,
            Some(symtab.object_type),
            BoundKind::UNBOUND,
            Some(symtab.bound_class),
            None,
        );
        let extends_string = new_wildcard_type(
            &mut ctx,
            Some(symtab.string_type),
            BoundKind::EXTENDS,
            Some(symtab.bound_class),
            None,
        );
        let super_string = new_wildcard_type(
            &mut ctx,
            Some(symtab.string_type),
            BoundKind::SUPER,
            Some(symtab.bound_class),
            None,
        );
        let list_string = class_type(&mut ctx, list_tsym, &[symtab.string_type]);
        let list_object = class_type(&mut ctx, list_tsym, &[symtab.object_type]);
        let list_unbounded = class_type(&mut ctx, list_tsym, &[unbounded]);
        let list_extends_string = class_type(&mut ctx, list_tsym, &[extends_string]);
        let list_super_string = class_type(&mut ctx, list_tsym, &[super_string]);
        let string_array = new_array_type(&mut ctx, symtab.string_type, Some(symtab.array_class));
        let int_array = new_array_type(&mut ctx, symtab.int_type, Some(symtab.array_class));
        let object_array = new_array_type(&mut ctx, symtab.object_type, Some(symtab.array_class));
        let mut by_name = std::collections::HashMap::new();
        by_name.insert("i", symtab.int_type);
        by_name.insert("l", symtab.long_type);
        by_name.insert("s", symtab.string_type);
        by_name.insert("o", symtab.object_type);
        by_name.insert("ls", list_string);
        by_name.insert("lo", list_object);
        by_name.insert("lu", list_unbounded);
        by_name.insert("les", list_extends_string);
        by_name.insert("lss", list_super_string);
        by_name.insert("sa", string_array);
        by_name.insert("ia", int_array);
        by_name.insert("oa", object_array);
        let mut checked = 0;
        let mut skipped = 0;
        for line in data.lines() {
            let parts: Vec<&str> = line.split(' ').collect();
            if parts.len() < 3 {
                continue;
            }
            let kind = parts[0];
            let left = parts[1];
            let right = parts[2];
            if kind == "PARAM" {
                if left == "inner" {
                    skipped += 1;
                    continue;
                }
                let ty = *by_name.get(left).unwrap_or_else(|| panic!("unknown {left}"));
                let expected = parts[2] == "true";
                assert_eq!(
                    is_parameterized_type(&ctx, ty),
                    expected,
                    "PARAM {left}"
                );
                checked += 1;
                let _ = right;
                continue;
            }
            if left == "inner" || right == "inner" {
                // SKIP: the sample's inner class type needs the Enter/Attr
                // integration (types unit tests build symbols without Enter);
                // registered, covered by the Attr harness tests later.
                skipped += 1;
                continue;
            }
            let left_ty = *by_name.get(left).unwrap_or_else(|| panic!("unknown {left}"));
            let right_ty = *by_name.get(right).unwrap_or_else(|| panic!("unknown {right}"));
            let expected = parts[3] == "true";
            let actual = match kind {
                "SUB" => types.is_subtype(&mut ctx, &symtab, left_ty, right_ty),
                "CONV" => types.is_convertible(&mut ctx, &symtab, left_ty, right_ty),
                "CAST" => types.is_castable(&mut ctx, &symtab, left_ty, right_ty),
                _ => continue,
            };
            assert_eq!(actual, expected, "{kind} {left} {right}");
            checked += 1;
        }
        assert_eq!(checked, 43, "checked relation rows");
        assert_eq!(skipped, 3, "inner-class rows skipped");
    }

    // ------------------------------------------------------------------
    // functional interface descriptors / castable warner /
    // checkUnique MTH.
    // ------------------------------------------------------------------

    #[allow(clippy::type_complexity, clippy::too_many_arguments)]
    fn build_services() -> (
        CompilerContext,
        Rc<std::cell::RefCell<Symtab>>,
        Types,
        Rc<std::cell::RefCell<crate::util::log::Log>>,
        Rc<crate::util::jc_diagnostic::Factory>,
        Rc<crate::code::lint::Lint>,
        Rc<std::cell::RefCell<crate::code::deferred_lint_handler::DeferredLintHandler>>,
    ) {
        use crate::code::deferred_lint_handler::DeferredLintHandler;
        use crate::util::context::Context;
        use crate::util::javac_messages::JavacMessages;
        let mut ctx = CompilerContext::new();
        let mut context = Context::new();
        let log = crate::util::log::Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = JavacMessages::instance(&mut context);
        let diags = Rc::new(crate::util::jc_diagnostic::Factory::new(messages, "compiler"));
        let lint = crate::code::lint::Lint::instance(&mut context);
        let deferred = Rc::new(std::cell::RefCell::new(DeferredLintHandler::new()));
        let symtab = Symtab::new(&mut ctx, Rc::clone(&names), Target::DEFAULT, Source::JDK17);
        let symtab = Rc::new(std::cell::RefCell::new(symtab));
        let types = Types::with_names(names);
        (ctx, symtab, types, log, diags, lint, deferred)
    }

    /// Build a synthetic interface class symbol with the given abstract method
    /// into the context (a local test fixture for the SAM tests; the upstream
    /// tests use full Enter/Analyze, which needs the Attr).
    #[allow(clippy::too_many_arguments)]
    fn make_interface_with_method(
        ctx: &mut CompilerContext,
        symtab: &mut Symtab,
        names: &Rc<Names>,
        method_name: &str,
        param_types: &[TypeId],
        return_type: TypeId,
        thrown: &[TypeId],
        owner_outermost: SymbolId,
        extra_method: Option<(&str, &[TypeId], TypeId)>,
    ) -> (SymbolId, TypeId, SymbolId) {
        let names = Rc::clone(names);
        let object_type = symtab.object_type;
        let method_class = symtab.method_class;
        let no_type = ctx.sym(symtab.no_symbol).ty.expect("no type");
        let flags = (crate::code::flags::PUBLIC
            | crate::code::flags::INTERFACE
            | crate::code::flags::ABSTRACT) as i64;
        let flat = names.from_string(&javac_strings::JavaString::from("Sam"));
        let class = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            crate::code::kinds::Kind::TYP,
            flags,
            flat.clone(),
            Some(no_type),
            Some(owner_outermost),
            crate::code::symbol::SymbolKind::ClassSymbol {
                members_field: None,
                fullname: flat.clone(),
                flatname: flat.clone(),
                sourcefile: None,
                classfile: None,
                trans_local: List::nil(),
                record_components: List::nil(),
                permitted: List::nil(),
                is_permitted_explicit: false,
                annotation_type_metadata: Default::default(),
            },
        ));
        let class_type = ctx.alloc_type(Type::new_class_type(
            Some(no_type),
            Some(List::nil()),
            class,
        ));
        ctx.sym_mut(class).ty = Some(class_type);
        let members = ctx.scope_create(Some(class));
        if let crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } =
            &mut ctx.sym_mut(class).kind_data
        {
            *members_field = Some(members);
        }
        if let TypeKindData::ClassType {
            supertype_field,
            interfaces_field,
            ..
        } = &mut ctx.typ_mut(class_type).kind
        {
            *supertype_field = Some(object_type);
            *interfaces_field = Some(List::nil());
        }
        let mut args = List::nil();
        for param in param_types.iter().rev() {
            args = args.prepend(*param);
        }
        let mut thrown_list = List::nil();
        for throw in thrown.iter().rev() {
            thrown_list = thrown_list.prepend(*throw);
        }
        let mtype = ctx.alloc_type(Type::new_method_type(
            args,
            return_type,
            thrown_list,
            method_class,
        ));
        let mname = names.from_string(&javac_strings::JavaString::from(method_name));
        let mflags = (crate::code::flags::PUBLIC | crate::code::flags::ABSTRACT) as i64;
        let method = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            crate::code::kinds::Kind::MTH,
            mflags,
            mname,
            Some(mtype),
            Some(class),
            crate::code::symbol::SymbolKind::MethodSymbol {
                extra_params: List::nil(),
                captured_locals: List::nil(),
                params: List::nil(),
                default_value: None,
            },
        ));
        ctx.scope_enter(members, method);
        if let Some((extra_name, extra_params, extra_return)) = extra_method {
            let mut extra_args = List::nil();
            for param in extra_params.iter().rev() {
                extra_args = extra_args.prepend(*param);
            }
            let extra_type = ctx.alloc_type(Type::new_method_type(
                extra_args,
                extra_return,
                List::nil(),
                method_class,
            ));
            let extra_symbol_name =
                names.from_string(&javac_strings::JavaString::from(extra_name));
            let extra = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
                crate::code::kinds::Kind::MTH,
                mflags,
                extra_symbol_name,
                Some(extra_type),
                Some(class),
                crate::code::symbol::SymbolKind::MethodSymbol {
                    extra_params: List::nil(),
                    captured_locals: List::nil(),
                    params: List::nil(),
                    default_value: None,
                },
            ));
            ctx.scope_enter(members, extra);
        }
        (class, class_type, method)
    }


    fn check_for_tests(
        names: &Rc<Names>,
        log: &Rc<std::cell::RefCell<crate::util::log::Log>>,
        diags: &Rc<crate::util::jc_diagnostic::Factory>,
        types: &Types,
        lint: &Rc<crate::code::lint::Lint>,
        deferred: &Rc<std::cell::RefCell<crate::code::deferred_lint_handler::DeferredLintHandler>>,
        syms: &Rc<std::cell::RefCell<Symtab>>,
    ) -> crate::comp::check::Check {
        crate::comp::check::Check::new(
            Rc::clone(names),
            Rc::clone(log),
            Rc::clone(diags),
            Source::JDK17,
            '$',
            Rc::new(types.clone()),
            Rc::clone(lint),
            Rc::clone(deferred),
            Rc::clone(syms),
        )
    }

    #[test]
    fn functional_interface_descriptor_identifies_the_sam() {
        let (mut ctx, syms_rc, types, log, diags, lint, deferred) = build_services();
        let names = Rc::new(Names::new(false));
        let mut syms = syms_rc.borrow_mut();
        let int_type = syms.int_type;
        let java_base = syms.java_base;
        let (_, _, method) = make_interface_with_method(
            &mut ctx,
            &mut syms,
            &names,
            "apply",
            &[int_type],
            int_type,
            &[],
            java_base,
            None,
        );
        let chk = check_for_tests(&names, &log, &diags, &types, &lint, &deferred, &syms_rc);
        let interface_tsym = ctx.sym(method).owner.expect("interface owner");
        let desc_sym = types
            .find_descriptor_symbol(&mut ctx, &mut syms, &diags, &chk, interface_tsym)
            .expect("SAM descriptor");
        assert_eq!(desc_sym, method, "the single abstract method is the descriptor");
        assert!(types.is_functional_interface(&mut ctx, &mut syms, &diags, &chk, interface_tsym));
        let interface_ty = ctx.sym(interface_tsym).ty.expect("interface type");
        let desc_ty = types
            .find_descriptor_type(&mut ctx, &mut syms, &diags, &chk, interface_ty)
            .expect("descriptor type");
        match &ctx.typ(desc_ty).kind {
            TypeKindData::MethodType {
                argtypes, restype, ..
            } => {
                assert_eq!(argtypes.length(), 1);
                assert_eq!(*argtypes.head(), int_type);
                assert_eq!(restype.expect("return type"), int_type);
            }
            _ => panic!("method type expected"),
        }
    }

    #[test]
    fn functional_interface_rejects_two_abstract_methods() {
        let (mut ctx, syms_rc, types, log, diags, lint, deferred) = build_services();
        let names = Rc::new(Names::new(false));
        let mut syms = syms_rc.borrow_mut();
        let int_type = syms.int_type;
        let void_type = syms.void_type;
        let java_base = syms.java_base;
        let (_, _, method) = make_interface_with_method(
            &mut ctx,
            &mut syms,
            &names,
            "apply",
            &[int_type],
            int_type,
            &[],
            java_base,
            Some(("other", &[], void_type)),
        );
        let chk = check_for_tests(&names, &log, &diags, &types, &lint, &deferred, &syms_rc);
        let interface_tsym = ctx.sym(method).owner.expect("interface owner");
        let result = types.find_descriptor_symbol(&mut ctx, &mut syms, &diags, &chk, interface_tsym);
        assert!(result.is_err(), "two abstract methods are not a SAM");
        assert!(!types.is_functional_interface(&mut ctx, &mut syms, &diags, &chk, interface_tsym));
    }

    #[test]
    fn descriptor_type_keeps_the_sam_thrown_clause() {
        let (mut ctx, syms_rc, types, log, diags, lint, deferred) = build_services();
        let names = Rc::new(Names::new(false));
        let mut syms = syms_rc.borrow_mut();
        let int_type = syms.int_type;
        let exception_type = syms.exception_type;
        let java_base = syms.java_base;
        let (_, _, method) = make_interface_with_method(
            &mut ctx,
            &mut syms,
            &names,
            "call",
            &[],
            int_type,
            &[exception_type],
            java_base,
            None,
        );
        let chk = check_for_tests(&names, &log, &diags, &types, &lint, &deferred, &syms_rc);
        let interface_tsym = ctx.sym(method).owner.expect("interface owner");
        let interface_ty = ctx.sym(interface_tsym).ty.expect("interface type");
        let desc_ty = types
            .find_descriptor_type(&mut ctx, &mut syms, &diags, &chk, interface_ty)
            .expect("descriptor type");
        let thrown = crate::code::types::get_thrown_types(&ctx, desc_ty);
        assert_eq!(thrown.length(), 1);
        assert_eq!(*thrown.head(), exception_type);
    }

    #[test]
    fn is_castable_with_warn_covers_the_primitive_relation() {
        let (mut ctx, syms, types) = build_with_types();
        let int_type = syms.int_type;
        let long_type = syms.long_type;
        let boolean_type = syms.boolean_type;
        let mut no_warn = crate::util::warner::Warner::new(None);
        assert!(types.is_castable_with_warn(
            &mut ctx,
            &syms,
            int_type,
            long_type,
            &mut no_warn
        ));
        assert!(!types.is_castable_with_warn(
            &mut ctx,
            &syms,
            boolean_type,
            int_type,
            &mut no_warn
        ));
        assert!(types.is_castable_with_warn(
            &mut ctx,
            &syms,
            int_type,
            int_type,
            &mut no_warn
        ));
        assert!(!no_warn.has_lint(crate::code::lint::LintCategory::UNCHECKED));
    }

    #[test]
    fn check_unique_mth_uses_has_same_args() {
        let (mut ctx, syms_rc, types, log, diags, lint, deferred) = build_services();
        let names = Rc::new(Names::new(false));
        let chk = check_for_tests(&names, &log, &diags, &types, &lint, &deferred, &syms_rc);
        let (class, int_type, java_base) = {
            let mut syms = syms_rc.borrow_mut();
            let int_type = syms.int_type;
            let java_base = syms.java_base;
            let (class, _, _) = make_interface_with_method(
                &mut ctx,
                &mut syms,
                &names,
                "apply",
                &[int_type],
                int_type,
                &[],
                java_base,
                Some(("apply", &[], int_type)),
            );
            (class, int_type, java_base)
        };
        let members = match &ctx.sym(class).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => {
                members_field.expect("members")
            }
            _ => panic!("class expected"),
        };
        let apply_name = names.from_string(&javac_strings::JavaString::from("apply"));
        let symbols = ctx.scope_get_symbols_by_name(
            members,
            &apply_name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        assert_eq!(symbols.len(), 2);
        assert!(
            chk.check_unique(&mut ctx, crate::util::position::NOPOS, symbols[0], members),
            "different signatures are unique"
        );
        let duplicate_type = ctx.sym(symbols[0]).ty.expect("method type");
        let dup = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            crate::code::kinds::Kind::MTH,
            (crate::code::flags::PUBLIC | crate::code::flags::ABSTRACT) as i64,
            apply_name,
            Some(duplicate_type),
            Some(class),
            crate::code::symbol::SymbolKind::MethodSymbol {
                extra_params: List::nil(),
                captured_locals: List::nil(),
                params: List::nil(),
                default_value: None,
            },
        ));
        ctx.scope_enter(members, dup);
        assert!(
            !chk.check_unique(&mut ctx, crate::util::position::NOPOS, dup, members),
            "same signature is a duplicate"
        );
        let _ = (int_type, java_base);
    }


}
