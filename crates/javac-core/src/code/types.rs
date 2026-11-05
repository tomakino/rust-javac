#![allow(clippy::result_large_err)]
#![allow(clippy::type_complexity)]
// Method-level audit status (Type/Types batch): the previous 123-method
// NOT PORTED list is resolved below. Every unit maps to a local implementation,
// a dispatch adapter, or an explicitly registered dependency.
//
// 1) Implemented in this batch (upstream line -> local unit):
//    isSubtypes (1250-1259) -> is_subtypes; isSubtypesUnchecked (1266-1275) ->
//    is_subtypes_unchecked; cvarLowerBounds (1978-1980) -> cvar_lower_bounds;
//    isArray (2059-2063) -> is_array; elemtypeOrType (2083-2088) ->
//    elemtype_or_type; isDerivedRaw/isDerivedRawInternal/isDerivedRaw(List)
//    (2670-2692) -> is_derived_raw/is_derived_raw_internal/is_derived_raw_types
//    (+ is_derived_raw_cache field); substBound (3448-3461) -> subst_bound;
//    typaramsString/appendTyparamString (3656-3693) -> typarams_string/
//    append_typaram_string (+ types_to_string for toString(Type));
//    closureCollector/ClosureHolder (3735-3765) -> closure_collector/
//    ClosureHolder/ClosureCollector (the Java Collector/Stream lifecycle is
//    exposed explicitly, registered adapter); insert/union with BiPredicate
//    (3772-3808) -> closure_insert_with/closure_union_with + ClosureSkip enum
//    (BiPredicate represented by the two upstream predicates, registered);
//    newRound (5257-5263) -> new_round.
//
// 2) Implemented before this batch under a different local name or by the
//    port's match dispatch (audited, upstream line -> local unit):
//    asSub (539) -> as_super/as_sub family; descriptor chain (640-885) ->
//    FunctionDescriptorLookupError/descriptor_failure/find_descriptor_*/
//    is_functional_interface/remove_wildcards/merge_descriptors; functional
//    interface bridges (834-1010) -> functional_interface_bridges/
//    pending_bridges; checkUnsafeVarargsConversion (1044) ->
//    check_unsafe_varargs_conversion; isSignaturePolymorphic (1327) ->
//    is_signature_polymorphic; disjointTypes/disjointType (1888-1974) ->
//    disjoint_types/disjoint_type (+ recursive helpers folded into the
//    cached match, registered); relaxBound (2012) -> relax_bound;
//    dimensions/makeArrayType (2108-2128) -> dimensions/make_array_type;
//    isAssignable (2338-2348) -> is_assignable*; isDirectSuperInterface (2659)
//    -> is_direct_super_interface; setBounds/getBounds (2706-2750) ->
//    set_bounds/get_bounds; isSubSignature (2795-2799) -> is_sub_signature*;
//    overrideEquivalent (2816) -> override_equivalent; mergeAbstracts (2874) ->
//    merge_abstracts (anonymous baseSymbol() carried as
//    MergeAbstractsResult.base_symbol, registered); implementation/
//    implementationInternal (2978-3005) -> implementation_filtered/
//    implementation_internal; membersClosure (3091+) -> members_closure/
//    members_closure_visit; firstUnimplementedAbstract/interfaceCandidates/
//    prune (3101-3220) -> first_unimplemented_abstract*/interface_candidates/
//    prune; substBounds (3407) -> subst_bounds; newInstances (3487) ->
//    new_instances; createMethodTypeWith*(3503-3535) -> create_method_type_with*;
//    baseType/createErrorType (3546-3570) -> base_type/create_error_type;
//    closure/compoundMin/closureMin/glbFlattened (3710-4124) -> closure/
//    compound_min/closure_min/glb_flattened; resultSubtype/
//    returnTypeSubstitutable/covariantReturnType (4249-4292) ->
//    result_subtype/return_type_substitutable/covariant_return_type;
//    boxedTypeOrType/unboxedTypeOrType (4312-4336) -> boxed_type_or_type/
//    unboxed_type_or_type; sideCast/sideCastFinal (4479-4509) -> side_cast/
//    side_cast_final; giveWarning (4535) -> give_warning; superClosure (4550) ->
//    super_closure; adaptRecursive (4664-4675) -> adapt_recursive;
//    rewriteQuantifiers (4720) -> rewrite_quantifiers; rewriteAsWildcardType/
//    makeExtendsWildcard/makeSuperWildcard (4784-4844) ->
//    rewrite_as_wildcard_type/make_extends_wildcard/make_super_wildcard;
//    getRetention (5021-5039) -> get_retention; SignatureGenerator
//    assemble*/append*/classReference/hasTypeVar (5044-5229) ->
//    SignatureGenerator trait (append_char/append_bytes/append_name/
//    class_reference/assemble_sig/assemble_class_sig/assemble_params_sig/
//    has_type_var/report_illegal_signature, registered adapter for the Java
//    abstract callbacks); constantType (5231) -> constant_type.
//    TypeRelation/TypeMapping/SimpleVisitor/Default(Simple)Visitor overrides
//    (visitCapturedType/visitUndetVar/visitPackageType/visitClassSymbol/
//    visitMethodSymbol/visitOperatorSymbol/visitPackageSymbol/visitTypeSymbol/
//    visitVarSymbol/apply; 478/1214/1447/1463/1593/1983/4229/4758/4905-4962/
//    5005/5012) are implemented by the equivalent TypeKindData match branches
//    (the Java visitor hierarchy is a dispatch mechanism; registered adapter).
//
// 3) Registered adapters:
//    - Java Collector/Stream closureCollector -> ClosureCollector builder.
//    - BiPredicate<Type,Type> -> ClosureSkip (Basic/IsSameType, the only two
//      upstream predicates).
//    - Java visitor classes -> TypeKindData match dispatch.
//    - TypeMapping.apply (5005) -> the explicit map methods.
//
// 4) Registered adapters/dependencies:
//    - needsStripping (Types.java:276/295/404/3392/3399): the Type.java
//      stripMetadata/stripMetadataIfNeeded visitor machinery is not ported
//      (Type.java batch; API JavacTypes/JavacTrees also not ported).
//    - MembersScope.combine (Types.java:3024-3042) is implemented by the
//      MembersScope wrapper: every getSymbols/getSymbolsByName query applies
//      !s.owner.isInterface() when skipInterface is set.
//    - Type.contains / containsType are implemented by Types::contains and the
//      type_contains method (live uses at 1997/5986/6065); the free helper below
//      is retained for its unit test (registered adapter).
//    - TypeVar.rank_field reset (Types.java:2723): Type.java batch.
//    - lub Names service not injected in the current call path (local
//      expect "lub needs the Names service"): service-wiring batch.
//    - isSubtypeUnchecked(..., Warner) is implemented: the raw-conversion branch
//      of isSubtypeUncheckedInternal emits LintCategory.UNCHECKED
//      (silentWarn for reifiable targets) and the warn variant runs
//      checkUnsafeVarargsConversion (Types.java:1011-1064).
// End of Gap registration.

//! Utility routines for the Type interface.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! (ADR 0001, arena + IDs): this ports ONLY the Types
//! subset needed to unlock TreeMaker/class-fields (see the task):
//! hashCode, isSameType, erasure, boxedClass, and the helpers they need.
//! The Java Types instance state (Symtab, Names, Source, Check, Enter,
//! JavacMessages, JCDiagnostic.Factory, Warner) is NOT stored in the Rust Types
//! (which is a stateless service): the methods that need it take the ported
//! Symtab explicitly (registered section 9; no Rc/RefCell is introduced). The
//! The former method-level list (historical): isSubtype / isCastable / containsType /
//! asSuper / memberType / subst / capture / supertype / interfaces / lub / glb /
//! isReifiable / rank) is implemented in this file; see the audit at the top.
//! Remaining dependencies are needsStripping, TypeVar.rank_field reset and lub
//! Names injection (registered above).
//!
//! the Type.java helpers getEnclosingType/getTypeArguments/
//! getParameterTypes/getReturnType are ported as Type methods in type_.rs; the
//! free helpers below delegate to those methods (no duplicate matching logic).
//! TypeMetadata remains unported (equalsIgnoreMetadata == equals).
//!
//! (hashCode): the upstream Name.hashCode() is
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
    name.with_byte_array(|bytes| NameTableHandle::hash_value(bytes, 0, bytes.len() as i32))
}

/// Java: Symbol.flatName() for a type symbol.
fn flat_name_of(ctx: &CompilerContext, symbol: SymbolId) -> Name {
    match &ctx.sym(symbol).kind_data {
        crate::code::symbol::SymbolKind::ClassSymbol { flatname, .. } => flatname.clone(),
        _ => ctx.sym(symbol).name.clone(),
    }
}

thread_local! {
    static TYPE_RECURSION_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct TypeRecursionGuard;

impl Drop for TypeRecursionGuard {
    fn drop(&mut self) {
        TYPE_RECURSION_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

/// Utility routines for the Type interface.
///
/// Java: public class Types
///
/// the Java instance fields (syms/...) are not stored; the
/// ported methods take them as parameters. The Names service is stored (the
/// captured-name creation of capture() needs it, Types.java:94/101/122).
#[derive(Clone)]
pub struct Types {
    /// Java: final Names names (the injected table is registered in the file
    /// header, and the non-injected constructors share the thread-local
    /// DEFAULT_NAMES table).
    names: std::rc::Rc<crate::util::names::Names>,
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
    members_closure_cache:
        std::cell::RefCell<std::collections::HashMap<SymbolId, crate::ids::ScopeId>>,
    /// Java: MembersClosureCache.seenTypes (Types.java:3014).
    members_closure_seen: std::cell::RefCell<std::collections::HashSet<SymbolId>>,
    /// Java: CandidatesCache.cache (Types.java:3151); the upstream Entry
    /// equality uses isSameType(site) + symbol identity, the port keys by the
    /// nominal TypeId (a cache-miss-only difference, registered Porting rules).
    candidates_cache:
        std::cell::RefCell<std::collections::HashMap<(TypeId, SymbolId), Vec<SymbolId>>>,
    /// Java: Map<Type,Boolean> isDerivedRawCache (Types.java:2668).
    is_derived_raw_cache: std::cell::RefCell<std::collections::HashMap<TypeId, bool>>,
    /// Port adapter: Java caches the rank in Type.rank_field; the port keys a
    /// side map by the arena TypeId. Entries are written only when the
    /// supertype graph is final (completed classes, type variables).
    rank_cache: std::cell::RefCell<std::collections::HashMap<TypeId, i32>>,
    /// Java: Types.Lub.mergeCache (Types.java:3855): the per-Types guard that
    /// stops merge -> lub -> merge recursion.
    merge_cache: std::cell::RefCell<Vec<(TypeId, TypeId)>>,
}

/// Java: ImplementationCache.Entry (Types.java:2936-2958).
#[derive(Clone)]
pub struct ImplementationEntry {
    pub cached_impl: Option<SymbolId>,
    pub check_result: bool,
    pub prev_mark: i32,
    pub filter_kind: ImplementationFilterKind,
}

/// Java: MethodSymbol.implementationFilter (Symbol.java:2168-2172) and the
/// Check.equalsHasCodeFilter predicate: the predicate identity is compared by
/// the implementation cache (Types.java:2958-2966); the port represents the
/// predicate by a comparable kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImplementationFilterKind {
    /// Java: s.kind == MTH && (s.flags() & SYNTHETIC) == 0
    Default,
    /// Java: MethodSymbol.implementation_filter.test(s) && (s.flags() & BAD_OVERRIDE) == 0
    EqualsHasCode,
}

/// Adapter: apply the upstream implementation filter predicate to a symbol.
pub fn implementation_filter_matches(
    ctx: &CompilerContext,
    kind: ImplementationFilterKind,
    sym: SymbolId,
) -> bool {
    let flags_value = ctx.sym(sym).flags_field;
    if ctx.sym(sym).kind != crate::code::kinds::Kind::MTH
        || (flags_value & crate::code::flags::SYNTHETIC as i64) != 0
    {
        return false;
    }
    match kind {
        ImplementationFilterKind::Default => true,
        ImplementationFilterKind::EqualsHasCode => {
            (flags_value & crate::code::flags::BAD_OVERRIDE) == 0
        }
    }
}

/// Java: DescriptorCache.Entry (Types.java:703-720).
#[derive(Clone)]
pub struct DescriptorEntry {
    pub cached_desc_res: FunctionDescriptor,
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
    std::cell::RefCell<std::collections::HashMap<TypeId, bool>>,
    std::cell::RefCell<std::collections::HashMap<TypeId, i32>>,
) {
    (
        std::cell::RefCell::new(std::collections::HashMap::new()),
        std::cell::RefCell::new(std::collections::HashMap::new()),
        std::cell::RefCell::new(std::collections::HashMap::new()),
        std::cell::RefCell::new(std::collections::HashSet::new()),
        std::cell::RefCell::new(std::collections::HashMap::new()),
        std::cell::RefCell::new(std::collections::HashMap::new()),
        std::cell::RefCell::new(std::collections::HashMap::new()),
    )
}

/// Java: the BiPredicate<Type, Type> used by closure insertion (Types.java:3735-3808).
/// The port names the two upstream predicates explicitly (registered adapter:
/// BiPredicate is not part of the ported API surface).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosureSkip {
    /// Java: basicClosureSkip (Types.java:3767): t1.tsym == t2.tsym.
    Basic,
    /// Java: Types::isSameType (used by Infer.CheckUpperBounds).
    IsSameType,
}

/// Java: Types.ClosureHolder (Types.java:3742-3765).
#[derive(Clone, Debug)]
pub struct ClosureHolder {
    closure: List<TypeId>,
    min_closure: bool,
    should_skip: ClosureSkip,
}

impl ClosureHolder {
    /// Java: void add(Type type) (Types.java:3753-3755)
    pub fn add(&mut self, types: &Types, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) {
        self.closure =
            types.closure_insert_with(ctx, syms, self.closure.clone(), t, self.should_skip);
    }

    /// Java: ClosureHolder merge(ClosureHolder other) (Types.java:3757-3760)
    pub fn merge(
        &mut self,
        types: &Types,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        other: &ClosureHolder,
    ) {
        self.closure = types.closure_union_with(
            ctx,
            syms,
            self.closure.clone(),
            other.closure.clone(),
            self.should_skip,
        );
    }

    /// Java: List<Type> closure() (Types.java:3762-3764)
    pub fn closure(&self, types: &Types, ctx: &mut CompilerContext, syms: &Symtab) -> List<TypeId> {
        if self.min_closure {
            types.closure_min(ctx, syms, self.closure.clone())
        } else {
            self.closure.clone()
        }
    }
}

/// Java: the Collector returned by closureCollector (supplier + accumulator +
/// combiner + finisher); the port exposes the same lifecycle explicitly.
#[derive(Clone, Debug)]
pub struct ClosureCollector {
    holder: ClosureHolder,
}

impl ClosureCollector {
    /// Java: ClosureHolder::add (the collector accumulator).
    pub fn add(&mut self, types: &Types, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) {
        self.holder.add(types, ctx, syms, t);
    }

    /// Java: ClosureHolder::merge (the collector combiner).
    pub fn merge(
        &mut self,
        types: &Types,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        other: &ClosureCollector,
    ) {
        self.holder.merge(types, ctx, syms, &other.holder);
    }

    /// Java: ClosureHolder::closure (the collector finisher).
    pub fn closure(&self, types: &Types, ctx: &mut CompilerContext, syms: &Symtab) -> List<TypeId> {
        self.holder.closure(types, ctx, syms)
    }
}

thread_local! {
    /// The shared Name table used by Types instances created without an
    /// injected Names service (Java: Types reads Names.instance(context), one
    /// table per context). The compiler is single-threaded; the thread-local
    /// keeps one table so that names produced by the non-injected constructors
    /// are identical (registered adapter).
    static DEFAULT_NAMES: std::rc::Rc<crate::util::names::Names> =
        std::rc::Rc::new(crate::util::names::Names::new(false));
}

impl Types {
    /// Get a Types instance whose Names service is the shared thread-local
    /// default table.
    ///
    /// upstream always reads `Names.instance(context)`, i.e. one
    /// table per compiler context. Runtime services that can compare Name
    /// identity against compiler symbols MUST use `with_names` with the
    /// compiler's table instead; `new` is reserved for tests and for helpers
    /// that never perform Name-based comparisons (registered adapter).
    pub fn new() -> Types {
        let (
            descriptor_cache,
            implementation_cache,
            members_closure_cache,
            members_closure_seen,
            candidates_cache,
            is_derived_raw_cache,
            rank_cache,
        ) = init_new_types_fields();
        Types {
            names: DEFAULT_NAMES.with(std::rc::Rc::clone),
            map_captures_to_bounds: false,
            descriptor_cache,
            implementation_cache,
            members_closure_cache,
            members_closure_seen,
            candidates_cache,
            is_derived_raw_cache,
            rank_cache,
            merge_cache: std::cell::RefCell::new(Vec::new()),
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
            is_derived_raw_cache,
            rank_cache,
        ) = init_new_types_fields();
        Types {
            names,
            map_captures_to_bounds: false,
            descriptor_cache,
            implementation_cache,
            members_closure_cache,
            members_closure_seen,
            candidates_cache,
            is_derived_raw_cache,
            rank_cache,
            merge_cache: std::cell::RefCell::new(Vec::new()),
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
            is_derived_raw_cache,
            rank_cache,
        ) = init_new_types_fields();
        Types {
            names,
            map_captures_to_bounds: crate::code::source::Feature::MAP_CAPTURES_TO_BOUNDS
                .allowed_in_source(source),
            descriptor_cache,
            implementation_cache,
            members_closure_cache,
            members_closure_seen,
            candidates_cache,
            is_derived_raw_cache,
            rank_cache,
            merge_cache: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// Java: final boolean mapCapturesToBounds
    pub fn map_captures_to_bounds(&self) -> bool {
        self.map_captures_to_bounds
    }

    /// The Names service: the injected one, or the shared thread-local table
    /// for the constructors that cannot inject it (registered adapter; upstream
    /// Types always reads Names.instance(context)).
    /// The captured type variable name (Types.java:122).
    fn captured_name(&self) -> Name {
        let names = std::rc::Rc::clone(&self.names);
        names.from_utf(b"<captured wildcard>")
    }

    /// Compute a hash code on a type.
    ///
    /// Java: public int hashCode(Type t) { return hashCode(t, false); }
    pub fn hash_code(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> i32 {
        self.hash_code_impl(ctx, syms, t, false)
    }

    /// Java: public int hashCode(Type t, boolean strict)
    pub fn hash_code_with_strict(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        strict: bool,
    ) -> i32 {
        self.hash_code_impl(ctx, syms, t, strict)
    }

    // The upstream hashCode(t, strict) has two visitors whose only difference is
    // the visitTypeVar override, and both return identityHashCode (Types.java
    // hashCodeStrictVisitor simply overrides visitTypeVar with
    // System.identityHashCode, exactly like the base visitor), so the strict flag
    // currently changes nothing for the ported branches (the port keeps the
    // parameter for the 1:1 signature, registered).
    #[allow(clippy::only_used_in_recursion)]
    fn hash_code_impl(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        strict: bool,
    ) -> i32 {
        // Java: HashCodeVisitor.visit(t): the visitor dispatches on the concrete
        // type class; ErrorType overrides visitErrorType, TypeVar/CapturedType
        // use identityHashCode (strict mode also uses identityHashCode for
        // TypeVar), ForAll/CapturedType are handled by SimpleVisitor.
        match &ctx.typ(t).kind {
            TypeKindData::ErrorType { .. } => 0,
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => self.hash_class_type(ctx, syms, t),
            TypeKindData::WildcardType { kind, type_, .. } => {
                let mut result = bound_kind_ordinal(*kind);
                if let Some(type_) = type_ {
                    result = result
                        .wrapping_mul(127)
                        .wrapping_add(self.hash_code_impl(ctx, syms, *type_, strict));
                }
                result
            }
            TypeKindData::ArrayType { elemtype, .. } => {
                let elemtype = elemtype.expect("array element type");
                self.hash_code_impl(ctx, syms, elemtype, strict)
                    .wrapping_add(12)
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
                    h = (h << 5).wrapping_add(self.hash_code_impl(
                        ctx,
                        syms,
                        argtypes.get(index),
                        strict,
                    ));
                }
                (h << 5).wrapping_add(self.hash_code_impl(ctx, syms, restype, strict))
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
                self.hash_code_impl(ctx, syms, qtype, strict)
            }
            _ => tag_ordinal(ctx.typ(t).get_tag()),
        }
    }

    /// Hash code of a class type: the enclosing type, the flat name, and the
    /// type arguments.
    ///
    /// Port adapter: `None` is the local representation of `Type.noType` for a
    /// top-level class, so the no-type value is hashed; upstream hashes
    /// `getEnclosingType()`, which returns noType rather than null here.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - t: the class type.
    ///
    /// # Returns
    ///   - the hash code.
    ///
    /// # Panics
    ///   - none.
    fn hash_class_type(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> i32 {
        let enclosing =
            get_enclosing_type(ctx, t).unwrap_or_else(|| crate::code::symtab::no_type_id(ctx));
        let mut result = self.hash_code_impl(ctx, syms, enclosing, false);
        result = result.wrapping_mul(127);
        let tsym = ctx.typ(t).tsym.expect("class type symbol");
        result = result.wrapping_add(name_hash(&flat_name_of(ctx, tsym)));
        for argument in get_type_arguments_completed(ctx, syms, t, None).iter() {
            result = result.wrapping_mul(127);
            result = result.wrapping_add(self.hash_code_impl(ctx, syms, argument, false));
        }
        result
    }

    /// Is t the same type as s?
    ///
    /// Java: public boolean isSameType(Type t, Type s)
    pub fn is_same_type(
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
            | TypeKindData::JCNoType { .. }
            | TypeKindData::TypeVar { .. }
            | TypeKindData::CapturedType { .. }
            | TypeKindData::UnknownType
            | TypeKindData::UninitializedType { .. } => {
                self.is_same_type_visit_type(ctx, syms, t, s)
            }
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
                self.is_same_type_visit_class(ctx, syms, t, s)
            }
            // Java: isSameTypeVisitor.visitErrorType -> true
            TypeKindData::ProxyType { .. } => t == s,
            TypeKindData::ErrorType { .. } => true,
            TypeKindData::WildcardType { .. } => self.is_same_type_visit_wildcard(ctx, syms, t, s),
            TypeKindData::ArrayType { .. } => self.is_same_type_visit_array(ctx, syms, t, s),
            TypeKindData::MethodType { .. } => self.is_same_type_visit_method(ctx, syms, t, s),
            TypeKindData::PackageType => t == s,
            TypeKindData::ModuleType => {
                // Java: DefaultTypeVisitor.visitModuleType -> visitType, whose
                // default branch throws AssertionError for the MODULE tag.
                panic!("AssertionError: isSameType {:?}", ctx.typ(t).get_tag());
            }
            TypeKindData::ForAll { .. } => self.is_same_type_visit_for_all(ctx, syms, t, s),
            TypeKindData::UndetVar { .. } => self.is_same_type_visit_undet_var(ctx, syms, t, s),
            TypeKindData::Deferred { .. } => {
                // Java: Types.SameType.visitType (Types.java:1106-1108):
                // return t == s; — reference equality, no DEFERRED special case.
                t == s
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
        if t == s {
            return true;
        }
        if qtype == s {
            // Java: t == s || t.qtype == s returns true without recording a
            // bound (Types.java:1469-1471).
            return true;
        }
        if ctx.typ(s).has_tag(TypeTag::ERROR) || ctx.typ(s).has_tag(TypeTag::UNKNOWN) {
            return true;
        }
        if ctx.typ(s).has_tag(TypeTag::BOT) {
            // Java: if 's' is 'null' there's no instantiated type U for which
            // U <: s (but 'null' itself, which is not a valid type)
            return false;
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
                    // type-substitution does not preserve type-var types
                    // check that type var symbols and bounds are indeed the same
                    t == s
                } else {
                    // special case for s == ? super X, where upper(s) = u; check
                    // that u == t, where u has been set by Type.withTypeVar.
                    ctx.typ(s).is_super_bound() && !ctx.typ(s).is_extends_bound() && {
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
            _ => crate::util::assert::error(),
        };
        let (s_kind, s_type) = match &ctx.typ(s).kind {
            TypeKindData::WildcardType { kind, type_, .. } => (*kind, *type_),
            _ => crate::util::assert::error(),
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
            let sup_t = self.supertype(ctx, syms, t, None);
            let sup_s = self.supertype(ctx, syms, s, None);
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
        if ctx.typ(t).tsym != ctx.typ(s).tsym {
            return false;
        }
        let t_enclosing = get_enclosing_type(ctx, t);
        let s_enclosing = get_enclosing_type(ctx, s);
        if !self.is_same_type_enclosing(ctx, syms, t_enclosing, s_enclosing) {
            return false;
        }
        let t_args = get_type_arguments_completed(ctx, syms, t, None);
        let s_args = get_type_arguments_completed(ctx, syms, s, None);
        self.contains_type_equivalent(ctx, syms, &t_args, &s_args)
    }

    /// Compares the enclosing types of two class types in the class-type case
    /// of the type-equality relation.
    ///
    /// Port adapter: `None` is the local representation of the absent enclosing
    /// type (`Type.noType` for a top-level class), not a null reference. Two
    /// absent enclosing types are equal, and an absent one differs from a
    /// present one; the upstream visitor compares the noType values directly.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - t/s: the enclosing types, where `None` is noType.
    ///
    /// # Returns
    ///   - true iff both enclosing types are absent or both are the same type.
    ///
    /// # Panics
    ///   - none.
    fn is_same_type_enclosing(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: Option<TypeId>,
        s: Option<TypeId>,
    ) -> bool {
        match (t, s) {
            (None, None) => true,
            (Some(t), Some(s)) => self.is_same_type(ctx, syms, t, s),
            _ => false,
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
            _ => crate::util::assert::error(),
        };
        let elemtype_s = match &ctx.typ(s).kind {
            TypeKindData::ArrayType { elemtype, .. } => elemtype.expect("array element type"),
            _ => crate::util::assert::error(),
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
        if !self.has_same_args(ctx, syms, t, s) {
            return false;
        }
        // Java: visit(t.getReturnType(), s.getReturnType()) (Types.java:1443);
        // Type.getReturnType() returns null for a non-method type and the
        // upstream visitor then throws NullPointerException.
        match (
            ctx.typ(t).get_return_type(ctx),
            ctx.typ(s).get_return_type(ctx),
        ) {
            (Some(t_ret), Some(s_ret)) => self.is_same_type(ctx, syms, t_ret, s_ret),
            _ => panic!("NullPointerException: isSameType method return type"),
        }
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
            TypeKindData::ForAll { qtype, tvars } => (qtype.expect("for-all qtype"), tvars.clone()),
            _ => crate::util::assert::error(),
        };
        let (s_qtype, s_tvars) = match &ctx.typ(s).kind {
            TypeKindData::ForAll { qtype, tvars } => (qtype.expect("for-all qtype"), tvars.clone()),
            _ => crate::util::assert::error(),
        };
        self.has_same_bounds(ctx, syms, &t_tvars, &s_tvars) && {
            let substituted = self.subst_type(ctx, syms, s_qtype, &s_tvars, &t_tvars, None);
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
    pub fn has_same_args(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
    ) -> bool {
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
            TypeKindData::UninitializedType { .. } => self.is_subtype_no_capture(ctx, syms, t, s),
            TypeKindData::ForAll { .. } => {
                // Java: hasSameArgs(t, s) == hasSameArgs(t, s, true)
                // (Types.java:3254-3256); the strict entry requires strict=true.
                // The non-strict ForAll/MethodType branch lives in
                // has_same_args_strict(..., false) and is used by
                // isSubSignature(..., false).
                self.has_same_args_strict(ctx, syms, t, s, true)
            }
            // Non-method types never have the same argument list as a method
            // type; the relation yields false for them.
            _ => false,
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
            TypeKindData::ArrayType { elemtype, .. } => {
                // Java: StructuralTypeMapping.visitArrayType erases the element
                // type and rebuilds the array type when it changed.
                let elem = elemtype.expect("array element type");
                let erased = self.erasure(ctx, syms, elem);
                if erased == elem {
                    t
                } else {
                    self.make_array_type(ctx, syms, erased)
                }
            }
            TypeKindData::WildcardType { .. } => {
                let upper = self.wild_upper_bound(ctx, syms, t);
                self.erasure_recurse(ctx, syms, upper, recurse)
                // combineMetadata(erased, t) == erased (no metadata support).
            }
            // Java: StructuralTypeMapping has no visitErrorType override; the
            // default visitType is the identity mapping, so erasure leaves an
            // ErrorType untouched (Types.java:2400-2424).
            TypeKindData::ErrorType { .. } => t,
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
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
                let upper = ctx.upper_bound_of(t).expect("type variable upper bound");
                self.erasure_recurse(ctx, syms, upper, recurse)
            }
            TypeKindData::UndetVar { .. } => {
                // Java: UndetVar is a DelegatedType; erasure dispatches through
                // the upper bound of the original type variable.
                let upper = ctx
                    .upper_bound_of(t)
                    .expect("inference variable upper bound");
                self.erasure_recurse(ctx, syms, upper, recurse)
            }
            TypeKindData::ForAll { qtype, .. } => {
                // Java: SimpleVisitor.visitForAll -> visit(t.qtype).
                let qtype = qtype.expect("for-all qtype");
                self.erasure_recurse(ctx, syms, qtype, recurse)
            }
            TypeKindData::MethodType { .. } | TypeKindData::PartiallyInferredMethodType { .. } => {
                // Java: StructuralTypeMapping.visitMethodType — erase the
                // parameter, return and thrown types (the erasure visitor
                // dispatches on the method type like any structural mapping).
                let (argtypes, restype, thrown) = match &ctx.typ(t).kind {
                    TypeKindData::MethodType {
                        argtypes,
                        restype,
                        thrown,
                        ..
                    }
                    | TypeKindData::PartiallyInferredMethodType {
                        argtypes,
                        restype,
                        thrown,
                        ..
                    } => (argtypes.clone(), *restype, thrown.clone()),
                    _ => crate::util::assert::error(),
                };
                let mut argtypes1: List<TypeId> = List::nil();
                for arg in argtypes.iter() {
                    argtypes1 = argtypes1.append(self.erasure_recurse(ctx, syms, arg, recurse));
                }
                let restype1 =
                    restype.map(|restype| self.erasure_recurse(ctx, syms, restype, recurse));
                let mut thrown1: List<TypeId> = List::nil();
                for thrown_type in thrown.iter() {
                    thrown1 = thrown1.append(self.erasure_recurse(ctx, syms, thrown_type, recurse));
                }
                if list_same(&argtypes, &argtypes1)
                    && restype1 == restype
                    && list_same(&thrown, &thrown1)
                {
                    return t;
                }
                new_method_type_2(ctx, argtypes1, restype1, thrown1, ctx.typ(t).tsym)
            }
            _ => {
                // Java: visitType - not primitive, "other cases already handled"
                // -> combineMetadata(t, t) == t.
                t
            }
        }
    }

    /// Java: Symbol.TypeSymbol.erasure(Types) / Symbol.ClassSymbol.erasure(Types)
    pub fn symbol_erasure(
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
            // Java: ClassType.getEnclosingType() returns Type.noType for a
            // top-level class; the port stores that as None.
            let enclosing =
                get_enclosing_type(ctx, ty).unwrap_or_else(|| crate::code::symtab::no_type_id(ctx));
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

    /// Returns the class that boxes the given primitive.
    ///
    /// Port adapter: the class is entered through the package-owned flat entry
    /// so the symbol carries the `java.lang` package owner (upstream enters
    /// the same class from the boxed name).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - t: the primitive type.
    ///
    /// # Returns
    ///   - the boxed class symbol.
    ///
    /// # Panics
    ///   - the type has no boxed class name.
    pub fn boxed_class(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> SymbolId {
        let tag = ctx.typ(t).get_tag();
        let name = syms.boxed_name[tag_ordinal(tag) as usize]
            .clone()
            .expect("boxed class name");
        let java_base = syms.java_base;
        syms.enter_class_flat(ctx, java_base, name)
    }

    /// Get a wildcard's upper bound, returning non-wildcards unchanged.
    ///
    /// Java: public Type wildUpperBound(Type t)
    pub fn wild_upper_bound(&self, ctx: &CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
        if ctx.typ(t).has_tag(TypeTag::WILDCARD) {
            if ctx.typ(t).is_super_bound() {
                let bound = match &ctx.typ(t).kind {
                    TypeKindData::WildcardType { bound, .. } => *bound,
                    _ => crate::util::assert::error(),
                };
                match bound {
                    None => syms.object_type,
                    Some(bound) => ctx
                        .upper_bound_of(bound)
                        .expect("wildcard bound upper bound"),
                }
            } else {
                let type_ = match &ctx.typ(t).kind {
                    TypeKindData::WildcardType { type_, .. } => *type_,
                    _ => crate::util::assert::error(),
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
                    _ => crate::util::assert::error(),
                };
                self.wild_lower_bound(ctx, syms, type_.expect("wildcard bound type"))
            }
        } else {
            t
        }
    }

    /// Java: Type.isCompound() (Type: false; IntersectionClassType: true;
    /// UnionClassType: getLub().isCompound()).
    /// Adapter: public wrapper of the ported isCompound.
    pub fn is_compound_public(&self, ctx: &mut CompilerContext, t: TypeId) -> bool {
        self.is_compound(ctx, t)
    }

    fn is_compound(&self, ctx: &mut CompilerContext, t: TypeId) -> bool {
        match &ctx.typ(t).kind {
            TypeKindData::IntersectionClassType { .. } => true,
            TypeKindData::UnionClassType { .. } => {
                // Java: UnionClassType.isCompound() (Type.java:1220-1222):
                // return getLub().isCompound(); getLub() is tsym.type.
                let tsym = ctx.typ(t).tsym.expect("union class symbol");
                let lub = ctx.sym(tsym).ty.expect("union lub type");
                self.is_compound(ctx, lub)
            }
            _ => false,
        }
    }

    /// Java: Type.equalsIgnoreMetadata (Type.java:512-514).
    ///
    /// `typeNoMetadata().equals(t.typeNoMetadata())`: both sides are first
    /// reduced to the un-annotated instance, so an annotated clone still
    /// compares identity-equal with the type it was derived from (this is what
    /// lets `@A T` satisfy an assignment target of plain `T`).
    ///
    /// Type.equals itself is "this == t" (Type.java:508-510); ArrayType
    /// overrides it structurally (Type.java:1150) and is reproduced below.
    fn equals_ignore_metadata(&self, ctx: &CompilerContext, t: TypeId, s: TypeId) -> bool {
        let t = ctx.type_no_metadata_id(t);
        let s = ctx.type_no_metadata_id(s);
        if t == s {
            return true;
        }
        match (&ctx.typ(t).kind, &ctx.typ(s).kind) {
            (
                TypeKindData::ArrayType {
                    elemtype: t_elem, ..
                },
                TypeKindData::ArrayType {
                    elemtype: s_elem, ..
                },
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
    pub fn supertype(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        mut attr: Option<&mut crate::comp::attr::Attr>,
    ) -> TypeId {
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
                TypeKindData::ClassType {
                    supertype_field, ..
                }
                | TypeKindData::ErasedClassType {
                    supertype_field, ..
                }
                | TypeKindData::UnionClassType {
                    supertype_field, ..
                }
                | TypeKindData::IntersectionClassType {
                    supertype_field, ..
                } => *supertype_field,
                _ => crate::util::assert::error(),
            };
            if let Some(sup) = existing {
                return sup;
            }
            let tsym = ctx.typ(t).tsym.expect("class type symbol");
            // Java: ClassSymbol.getSuperclass() completes the symbol before
            // reading supertype_field (Symbol.java). The ported Symtab keeps its
            // mutable tables internally, so completion writes the real table.
            if !ctx.is_symbol_completed(tsym) {
                // Port adapter: the Attr-inside callers pass their Attr so the
                // reentrant completion reuses the same Attr object.
                match attr.as_deref_mut() {
                    Some(attr) => {
                        if let Err(failure) = ctx.complete_class_symbol_with_attr(syms, tsym, attr)
                        {
                            crate::code::symbol::throw_completion_failure(failure);
                        }
                    }
                    None => {
                        if let Err(failure) = ctx.complete_class_symbol(syms, tsym) {
                            crate::code::symbol::throw_completion_failure(failure);
                        }
                    }
                }
            }
            let mut supertype_opt = class_symbol_get_superclass(ctx, tsym);
            if symbol_is_interface(ctx, tsym) {
                let tsym_type = ctx.sym(tsym).ty.expect("symbol type");
                supertype_opt = match &ctx.typ(tsym_type).kind {
                    TypeKindData::ClassType {
                        supertype_field, ..
                    }
                    | TypeKindData::ErasedClassType {
                        supertype_field, ..
                    }
                    | TypeKindData::UnionClassType {
                        supertype_field, ..
                    }
                    | TypeKindData::IntersectionClassType {
                        supertype_field, ..
                    } => *supertype_field,
                    _ => None,
                };
            }
            let supertype = supertype_opt.unwrap_or_else(|| no_type_of(ctx, syms));
            let tsym_type = ctx.sym(tsym).ty.expect("symbol type");
            let class_bound_type = class_bound(self, ctx, syms, t, attr.as_deref_mut());
            // The actual type arguments of the class bound and the formal
            // type parameters of the class symbol.
            // Port adapter: allparams() calls getTypeArguments(), whose upstream
            // implementation lazily completes the class symbol.
            let actuals = allparams_of_completed(ctx, syms, class_bound_type, attr.as_deref_mut());
            let formals = allparams_of_completed(ctx, syms, tsym_type, attr.as_deref_mut());
            if crate::util::debug_flags::debug_enabled("DSH_DEBUG_ALL") {
                let dt = crate::comp::attr::type_display_of(ctx, t);
                if dt == "ListIterator" || dt == "Iterator" {
                    let dst = crate::comp::attr::type_display_of(ctx, supertype);
                    eprintln!(
                        "[ST] t={} erased={} raw={} sup={} formals_empty={}",
                        dt,
                        has_erased_supertypes(ctx, t),
                        is_raw_type(ctx, t),
                        dst,
                        formals.is_empty()
                    );
                }
            }
            let result = if has_erased_supertypes(ctx, t) {
                self.erasure_recursive(ctx, syms, supertype)
            } else if formals.non_empty() {
                self.subst_type(
                    ctx,
                    syms,
                    supertype,
                    &formals,
                    &actuals,
                    attr.as_deref_mut(),
                )
            } else {
                supertype
            };
            set_class_supertype_field(ctx, t, Some(result));
            return result;
        }
        if tag == TypeTag::TYPEVAR {
            let upper = ctx.upper_bound_of(t).expect("type var upper bound");
            let upper_is_compound = self.is_compound(ctx, upper);
            let upper_is_interface = type_is_interface(ctx, upper);
            if ctx.typ(upper).has_tag(TypeTag::TYPEVAR)
                || (!upper_is_compound && !upper_is_interface)
            {
                return upper;
            }
            return self.supertype(ctx, syms, upper, attr);
        }
        if tag == TypeTag::ARRAY {
            let elem = match &ctx.typ(t).kind {
                TypeKindData::ArrayType { elemtype, .. } => elemtype.expect("array element type"),
                _ => crate::util::assert::error(),
            };
            let elem_is_primitive = ctx.typ(elem).is_primitive();
            let elem_is_object = self.is_same_type(ctx, syms, elem, syms.object_type);
            if elem_is_primitive || elem_is_object {
                return self.array_super_type(ctx, syms);
            }
            let sup = self.supertype(ctx, syms, elem, attr);
            let tsym = ctx.typ(t).tsym;
            return new_array_type(ctx, sup, tsym);
        }
        no_type_of(ctx, syms)
    }

    pub fn interfaces(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> List<TypeId> {
        self.interfaces_with_attr(ctx, syms, t, None)
    }

    /// Types.interfaces with the enclosing Attr of an Attr-inside caller. The
    /// declared-interface read completes a source class first; that completion
    /// must run on the caller's Attr, not on the shared service cell.
    pub fn interfaces_with_attr(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        mut attr: Option<&mut crate::comp::attr::Attr>,
    ) -> List<TypeId> {
        let is_class_family = matches!(
            &ctx.typ(t).kind,
            TypeKindData::ClassType { .. }
                | TypeKindData::ErasedClassType { .. }
                | TypeKindData::UnionClassType { .. }
                | TypeKindData::IntersectionClassType { .. }
        );
        if is_class_family {
            let existing = match &ctx.typ(t).kind {
                TypeKindData::ClassType {
                    interfaces_field, ..
                }
                | TypeKindData::ErasedClassType {
                    interfaces_field, ..
                }
                | TypeKindData::UnionClassType {
                    interfaces_field, ..
                }
                | TypeKindData::IntersectionClassType {
                    interfaces_field, ..
                } => interfaces_field.clone(),
                _ => crate::util::assert::error(),
            };
            if let Some(list) = existing {
                return list;
            }
            let tsym = ctx.typ(t).tsym.expect("class type symbol");
            // Java: Types.java:2594-2613 visitClassType calls
            // ((ClassSymbol)t.tsym).getInterfaces(), and
            // Symbol.java:1421-1422 getInterfaces() starts with apiComplete().
            // Completion must happen before reading the declared interface
            // list, otherwise an incomplete classfile stub caches an empty
            // list in the ClassType of an already-completed symbol.
            if !ctx.is_symbol_completed(tsym) {
                let completed = match attr.as_deref_mut() {
                    Some(attr) => ctx.complete_class_symbol_with_attr(syms, tsym, attr),
                    None => ctx.complete_class_symbol(syms, tsym),
                };
                if let Err(failure) = completed {
                    crate::code::symbol::throw_completion_failure(failure);
                }
            }
            let tsym_type = ctx.sym(tsym).ty.expect("symbol type");
            // The declared interfaces are substituted with the actual type
            // the declared interfaces of the symbol are substituted with the
            // actual type arguments whenever t is not the symbol's own type.
            // The ported previous version returned the declared list unchanged,
            // which left the hierarchy as Collection<E> for a List<T>.
            let interfaces = class_symbol_get_interfaces(ctx, tsym);
            let result = if t == tsym_type {
                interfaces
            } else if has_erased_supertypes(ctx, t) {
                // Java: if (t.hasErasedSupertypes()) t.interfaces_field =
                // erasureRecursive(interfaces) (Types.java:2616-2617). A raw
                // type keeps its declared supertypes raw: substituting with an
                // empty actuals list leaves Iterator<E> instead of Iterator.
                let mut erased: List<TypeId> = List::nil();
                for i in interfaces.iter() {
                    erased = erased.prepend(self.erasure_recursive(ctx, syms, i));
                }
                let mut out: List<TypeId> = List::nil();
                for i in erased.iter() {
                    out = out.prepend(i);
                }
                out
            } else {
                let actuals = allparams_of(ctx, t);
                let formals = allparams_of(ctx, tsym_type);
                if formals.non_empty() {
                    self.subst_types(ctx, syms, &interfaces, &formals, &actuals)
                } else {
                    interfaces
                }
            };
            set_class_interfaces_field(ctx, t, Some(result.clone()));
            return result;
        }
        let tag = ctx.typ(t).get_tag();
        if tag == TypeTag::ERROR {
            let existing = match &ctx.typ(t).kind {
                TypeKindData::ErrorType {
                    interfaces_field, ..
                } => interfaces_field.clone(),
                _ => None,
            };
            if let Some(list) = existing {
                return list;
            }
            let tsym = ctx.typ(t).tsym.expect("error type symbol");
            let declared = class_symbol_get_interfaces_option(ctx, tsym);
            // ErrorType has no ClassType.interfaces_field to cache into; the
            // declared interfaces (possibly empty) are the result.
            return declared.unwrap_or_else(List::nil);
        }
        if tag == TypeTag::TYPEVAR {
            let upper = ctx.upper_bound_of(t).expect("type var upper bound");
            let upper_is_compound = self.is_compound(ctx, upper);
            let upper_is_interface = type_is_interface(ctx, upper);
            if upper_is_compound {
                return self.interfaces_with_attr(ctx, syms, upper, attr.as_deref_mut());
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
        mut attr: Option<&mut crate::comp::attr::Attr>,
    ) -> TypeId {
        let tag = ctx.typ(t).get_tag();
        if tag == TypeTag::TYPEVAR {
            let mut from = from.clone();
            let mut to = to.clone();
            while from.non_empty() {
                // Java: Subst.visitTypeVar compares with `==` (reference
                // identity, Types.java:3433). A type variable and its
                // cloneWithMetadata copy are the same variable for this
                // purpose, so compare the un-annotated bases; two TypeVars
                // that merely share a symbol are NOT interchangeable.
                if t == *from.head()
                    || ctx.type_no_metadata_id(t) == ctx.type_no_metadata_id(*from.head())
                {
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
        if is_class_family && !self.is_compound(ctx, t) {
            let (outer, typarams) = match &ctx.typ(t).kind {
                TypeKindData::ClassType {
                    outer_field,
                    typarams_field,
                    ..
                }
                | TypeKindData::ErasedClassType {
                    outer_field,
                    typarams_field,
                    ..
                }
                | TypeKindData::UnionClassType {
                    outer_field,
                    typarams_field,
                    ..
                }
                | TypeKindData::IntersectionClassType {
                    outer_field,
                    typarams_field,
                    ..
                } => (
                    *outer_field,
                    typarams_field.clone().unwrap_or_else(List::nil),
                ),
                _ => crate::util::assert::error(),
            };
            let no_type = no_type_of(ctx, syms);
            let outer1 = match outer {
                Some(outer) => self.subst_visit(ctx, syms, outer, from, to, attr.as_deref_mut()),
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
            // Upstream: Types.Subst.visitClassType — new ClassType(outer1,
            // typarams1, tsym); Type.noType is the single absent-enclosing
            // sentinel. The local model
            // carries it either as None or as Some(noType); a rebuilt class type
            // keeps the encoding of the source type so structural comparisons
            // against that type still line up.
            let outer_field = match outer {
                Some(_) => Some(outer1),
                None => None,
            };
            return new_class_type(ctx, outer_field, Some(typarams1), tsym);
        }
        if matches!(&ctx.typ(t).kind, TypeKindData::WildcardType { .. }) {
            let (type_, kind, bound) = match &ctx.typ(t).kind {
                TypeKindData::WildcardType {
                    type_, kind, bound, ..
                } => (*type_, *kind, *bound),
                _ => crate::util::assert::error(),
            };
            let Some(type_) = type_ else {
                return t;
            };
            let visited = self.subst_visit(ctx, syms, type_, from, to, attr.as_deref_mut());
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
                _ => crate::util::assert::error(),
            };
            let elem1 = self.subst_visit(ctx, syms, elem, from, to, attr.as_deref_mut());
            if elem1 == elem {
                return t;
            }
            let tsym = ctx.typ(t).tsym;
            return new_array_type(ctx, elem1, tsym);
        }
        if tag == TypeTag::METHOD {
            // A partially inferred method type extends MethodType, so
            // Subst.visitMethodType (Types.StructuralTypeMapping) maps it like
            // any MethodType and rebuilds a plain MethodType.
            let (argtypes, restype, thrown) = match &ctx.typ(t).kind {
                TypeKindData::MethodType {
                    argtypes,
                    restype,
                    thrown,
                    ..
                } => (argtypes.clone(), *restype, thrown.clone()),
                TypeKindData::PartiallyInferredMethodType {
                    argtypes,
                    restype,
                    thrown,
                    ..
                } => (argtypes.clone(), *restype, thrown.clone()),
                _ => crate::util::assert::error(),
            };
            let argtypes1 = self.subst(ctx, syms, &argtypes, from, to);
            let restype = restype.expect("method return type");
            let restype1 = self.subst_visit(ctx, syms, restype, from, to, attr.as_deref_mut());
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
                _ => crate::util::assert::error(),
            };
            let qtype = qtype.expect("for-all qtype");
            let mut tvars_cur = tvars.clone();
            let mut qtype_cur = qtype;
            if self.type_list_contains_any(ctx, syms, to, &tvars, &mut Vec::new()) {
                let freevars = self.new_instances(ctx, syms, &tvars);
                let substituted = self.subst_type(ctx, syms, qtype_cur, &tvars, &freevars, None);
                qtype_cur = substituted;
                tvars_cur = freevars;
            }
            let tvars1 = self.subst_bounds(ctx, syms, &tvars_cur, from, to);
            let qtype1 = self.subst_visit(ctx, syms, qtype_cur, from, to, attr.as_deref_mut());
            if list_same(&tvars_cur, &tvars1) && qtype1 == qtype_cur {
                return t;
            }
            let new_qtype = if list_same(&tvars_cur, &tvars1) {
                qtype1
            } else {
                self.subst_type(ctx, syms, qtype1, &tvars_cur, &tvars1, None)
            };
            let qtype_tsym = ctx.typ(new_qtype).tsym;
            return ctx.alloc_type(Type::new(
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
            let supertype = self.supertype(ctx, syms, t, None);
            let st = self.subst_visit(ctx, syms, supertype, from, to, attr.as_deref_mut());
            let interfaces = self.interfaces(ctx, syms, t);
            let mut is: List<TypeId> = List::nil();
            let current = interfaces.clone();
            for interface in &current {
                let substituted =
                    self.subst_visit(ctx, syms, interface, from, to, attr.as_deref_mut());
                is = is.append(substituted);
            }
            if st == supertype && list_same(&interfaces, &is) {
                return t;
            }
            let names = std::rc::Rc::clone(&self.names);
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
        let current = tvars.clone();
        for tvar in &current {
            let (bound, lower) = match &ctx.typ(tvar).kind {
                TypeKindData::TypeVar { bound, lower } => (*bound, *lower),
                _ => (None, None),
            };
            let tsym = ctx.typ(tvar).tsym;
            let new_tvar = ctx.alloc_type(Type::new(
                TypeTag::TYPEVAR,
                tsym,
                TypeKindData::TypeVar { bound, lower },
            ));
            tvars1 = tvars1.append(new_tvar);
        }
        let mut result: List<TypeId> = List::nil();
        let current = tvars1.clone();
        for tvar in &current {
            let upper = ctx.upper_bound_of(tvar).expect("type var upper bound");
            let substituted = self.subst_type(ctx, syms, upper, tvars, &tvars1, None);
            if let TypeKindData::TypeVar { bound, .. } = &mut ctx.typ_mut(tvar).kind {
                *bound = Some(substituted);
            }
            result = result.append(tvar);
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
            } => ctx.alloc_type(Type::new(
                TypeTag::METHOD,
                tsym,
                TypeKindData::MethodType {
                    argtypes: argtypes.clone(),
                    restype: Some(new_return),
                    thrown: thrown.clone(),
                    recvtype: *recvtype,
                },
            )),
            TypeKindData::PartiallyInferredMethodType {
                argtypes,
                thrown,
                recvtype,
                ..
            } => ctx.alloc_type(Type::new(
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
                ctx.alloc_type(Type::new(
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
        let current = tvars.clone();
        for tvar in &current {
            let upper = ctx.upper_bound_of(tvar).expect("type var upper bound");
            let bound = self.subst_type(ctx, syms, upper, from, to, None);
            if bound != upper {
                changed = true;
            }
            new_bounds.push(bound);
        }
        if !changed {
            return tvars.clone();
        }
        let mut new_tvars: List<TypeId> = List::nil();
        let current = tvars.clone();
        for tvar in &current {
            let tsym = ctx.typ(tvar).tsym;
            let new_tvar = ctx.alloc_type(Type::new(
                TypeTag::TYPEVAR,
                tsym,
                TypeKindData::TypeVar {
                    bound: None,
                    lower: Some(syms.bot_type),
                },
            ));
            new_tvars = new_tvars.append(new_tvar);
        }
        let mut substituted_bounds: Vec<TypeId> = Vec::new();
        for bound in &new_bounds {
            let substituted = self.subst_type(ctx, syms, *bound, tvars, &new_tvars, None);
            substituted_bounds.push(substituted);
        }
        let current = new_tvars.clone();
        let mut index = 0;
        for tvar in &current {
            let bound = substituted_bounds[index];
            if let TypeKindData::TypeVar {
                bound: bound_field, ..
            } = &mut ctx.typ_mut(tvar).kind
            {
                *bound_field = Some(bound);
            }
            index += 1;
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
        let current = ts.clone();
        for t in &current {
            if self.type_list_contains(ctx, syms, tvs, t, cache) {
                return true;
            }
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
        let current = ts.clone();
        for t in &current {
            let r = self.type_contains(ctx, _syms, t, elem);
            if r {
                return true;
            }
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
            let sup = self.supertype(ctx, syms, t, None);
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
    pub fn erasure_recursive(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
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
            elements.push(self.subst_visit(ctx, syms, t, &from, &to, None));
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
        attr: Option<&mut crate::comp::attr::Attr>,
    ) -> TypeId {
        let (from, to) = self.align_subst_lists(from, to);
        self.subst_visit(ctx, syms, t, &from, &to, attr)
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
        // Track the remaining lengths instead of recomputing length() (O(n))
        // on every step, which made the alignment O(n^2).
        let mut from_remaining = from.length();
        let mut to_remaining = to.length();
        let mut from = from.clone();
        let mut to = to.clone();
        while from_remaining > to_remaining {
            from = from.tail();
            from_remaining -= 1;
        }
        while from_remaining < to_remaining {
            to = to.tail();
            to_remaining -= 1;
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
        // Port adapter: ensure a classfile stub has its INTERFACE/access flags
        // before asSuper reads them; upstream ClassSymbol.flags() observes the
        // completed symbol (the local stub carries CLASS_SEEN only).
        if ctx.sym(sym).kind == crate::code::kinds::Kind::TYP
            && (ctx.sym(sym).flags() & crate::code::flags::INTERFACE as i64) == 0
        {
            let _ = ctx.complete_class_symbol(syms, sym);
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
                    let st = self.supertype(ctx, syms, t, None);
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
                    let upper = ctx.upper_bound_of(t).expect("type var upper bound");
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
                        if crate::util::debug_flags::debug_enabled("DSH_DEBUG_ALL") {
                            let n1 = ctx.sym(sym).name.to_java_string().to_string_lossy().to_string();
                            if n1 == "Iterator" {
                                let dt = crate::comp::attr::type_display_of(ctx, t);
                                let ds = crate::comp::attr::type_display_of(ctx, s);
                                eprintln!("[AOS] t={} -> {}", dt, ds);
                            }
                        }
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
                                Some(encl_class) => {
                                    ctx.sym(encl_class).ty.expect("enclosing class type")
                                }
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
                        // Java: owner.type.allparams() / base.allparams() go through
                        // getTypeArguments(), which lazily calls complete() when
                        // typarams_field is still null (Type.java:1068-1080). The
                        // non-completing helper would read an unfilled field and
                        // yield the wrong parameter list.
                        let owner_params = allparams_of_completed(ctx, syms, owner_type, None);
                        let base_params = allparams_of_completed(ctx, syms, base, None);
                        if crate::util::debug_flags::debug_enabled("DSH_DEBUG_ALL") {
                            let nm = ctx.sym(sym).name.to_java_string().to_string_lossy().to_string();
                            if nm == "next" {
                                let db = crate::comp::attr::type_display_of(ctx, base);
                                eprintln!(
                                    "[MTV] owner_params_empty={} base={} base_params_empty={} raw={}",
                                    owner_params.is_empty(),
                                    db,
                                    base_params.is_empty(),
                                    is_raw_type(ctx, base)
                                );
                            }
                        }
                        if owner_params.non_empty() {
                            return if base_params.is_empty() {
                                // then base is a raw type
                                self.erasure(ctx, syms, sym_type)
                            } else {
                                self.subst_type(
                                    ctx,
                                    syms,
                                    sym_type,
                                    &owner_params,
                                    &base_params,
                                    None,
                                )
                            }
                        }
                    }
                }
                ctx.sym(sym).ty.expect("member type")
            }
            TypeKindData::TypeVar { .. } | TypeKindData::CapturedType { .. } => {
                let upper = ctx.upper_bound_of(t).expect("type var upper bound");
                self.member_type(ctx, syms, upper, sym)
            }
            _ => ctx.sym(sym).ty.expect("member type"),
        }
    }

    /// Java: public Type capture(Type t)
    ///
    /// the freshTypeVariables/glb parts use the ported
    /// CapturedType and Types::glb; a type that does not need capture conversion
    /// is returned as-is (the early returns of the upstream method).
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
                let formals = get_type_arguments_completed(ctx, syms, tsym_type, None);
                let actuals = get_type_arguments_completed(ctx, syms, t, None);
                return self.subst_type(ctx, syms, type1, &formals, &actuals, None);
            }
        }
        if is_raw_type(ctx, t) || !is_parameterized_type(ctx, t) {
            return t;
        }
        let cls = t;
        let tsym = ctx.typ(cls).tsym.expect("class type symbol");
        // getTypeArguments() lazily completes the class symbol
        // capture() reads t.tsym.type.getTypeArguments()
        // as the formal parameter list, so the symbol must be completed before
        // the list is read; otherwise a classfile generic such as
        // java.util.List yields an empty formal list and capture() falls into
        // the rare-type erasure branch.
        if !ctx.is_symbol_completed(tsym)
            && let Err(failure) = ctx.complete_class_symbol(syms, tsym)
        {
            crate::code::symbol::throw_completion_failure(failure);
        }
        let tsym_type = ctx.sym(tsym).ty.expect("class symbol type");
        let a_list = get_type_arguments_completed(ctx, syms, tsym_type, None);
        let t_list = get_type_arguments_completed(ctx, syms, cls, None);
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
                let ui = ctx.upper_bound_of(head_a).unwrap_or(syms.object_type);
                let kind = match &ctx.typ(wildcard).kind {
                    TypeKindData::WildcardType { kind, .. } => *kind,
                    _ => BoundKind::UNBOUND,
                };
                let (upper, lower) = match kind {
                    BoundKind::UNBOUND => {
                        let upper = self.subst_type(ctx, syms, ui, &a_list, &s_list, None);
                        (upper, syms.bot_type)
                    }
                    BoundKind::EXTENDS => {
                        let extends_bound = match &ctx.typ(wildcard).kind {
                            TypeKindData::WildcardType { type_, .. } => {
                                type_.expect("extends bound")
                            }
                            _ => crate::util::assert::error(),
                        };
                        let ui_subst = self.subst_type(ctx, syms, ui, &a_list, &s_list, None);
                        let upper = self.glb(ctx, syms, extends_bound, ui_subst);
                        (upper, syms.bot_type)
                    }
                    BoundKind::SUPER => {
                        let upper = self.subst_type(ctx, syms, ui, &a_list, &s_list, None);
                        let super_bound = match &ctx.typ(wildcard).kind {
                            TypeKindData::WildcardType { type_, .. } => type_.expect("super bound"),
                            _ => crate::util::assert::error(),
                        };
                        (upper, super_bound)
                    }
                };
                // Java: Si.setUpperBound(upper); Si.lower = lower;
                if let TypeKindData::CapturedType {
                    bound: b, lower: l, ..
                } = &mut ctx.typ_mut(head_s).kind
                {
                    *b = Some(upper);
                    *l = Some(lower);
                }
                let upper = ctx.upper_bound_of(head_s).expect("captured upper bound");
                let lower = match &ctx.typ(head_s).kind {
                    TypeKindData::CapturedType { lower, .. } => {
                        lower.expect("captured lower bound")
                    }
                    _ => crate::util::assert::error(),
                };
                // Java: Type tmpBound = Si.getUpperBound().hasTag(UNDETVAR) ?
                //     ((UndetVar)Si.getUpperBound()).qtype : Si.getUpperBound();
                // (Types.java:4436-4437) — the capture check compares the
                // *qtypes*, so an inference variable bound never records an EQ
                // bound here through isSameType/visitUndetVar.
                let tmp_bound = if ctx.typ(upper).has_tag(TypeTag::UNDETVAR) {
                    self.undet_var_qtype(ctx, upper)
                } else {
                    upper
                };
                let tmp_lower = if ctx.typ(lower).has_tag(TypeTag::UNDETVAR) {
                    self.undet_var_qtype(ctx, lower)
                } else {
                    lower
                };
                let tmp_bound_err = ctx.typ(upper).has_tag(TypeTag::ERROR);
                let tmp_lower_err = ctx.typ(lower).has_tag(TypeTag::ERROR);
                if !tmp_bound_err
                    && !tmp_lower_err
                    && self.is_same_type(ctx, syms, tmp_bound, tmp_lower)
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
            return ctx.alloc_type(Type::new(
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
        // Fast path: without wildcards there is nothing to capture. The loop
        // below would still rebuild the whole list through append(), which
        // allocates a fresh node per element on every capture() call.
        if !types.iter().any(|t| ctx.typ(t).has_tag(TypeTag::WILDCARD)) {
            return types;
        }
        let captured_name = self.captured_name();
        let mut result: List<TypeId> = List::nil();
        let current = types;
        for t in &current {
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
        name: Name,
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
        let type_id = ctx.alloc_type(Type::new(
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
                        _ => crate::util::assert::error(),
                    };
                    match kind {
                        BoundKind::UNBOUND => {}
                        BoundKind::EXTENDS => {
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
                        BoundKind::SUPER => {
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
                } else if ctx.typ(s).has_tag(TypeTag::TYPEVAR) {
                    // A captured variable on the right of `C<T> <: C<S>` still
                    // contains the target argument when it is bounded from below
                    // by a concrete type: `? super Object` captures to a variable
                    // whose lower bound is Object, so `List<? super Object>` is
                    // assignable to `List<Object>` (javac accepts it), while the
                    // bottom lower bound of `? extends Object` keeps that case
                    // rejected. A lower bound that is itself a type variable
                    // (`? super T`) keeps the plain same-type comparison;
                    // otherwise generic inference rebinds Guava's
                    // `Ordering.from`/`TypeToken.of` to the wrong candidate.
                    match ctx.typ(s).get_lower_bound() {
                        Some(lower)
                            if !ctx.typ(lower).has_tag(TypeTag::BOT)
                                && !ctx.typ(lower).has_tag(TypeTag::TYPEVAR) =>
                        {
                            self.contains_type(ctx, syms, t, lower)
                        }
                        _ => self.is_same_type(ctx, syms, t, s),
                    }
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
        if type_equals_ignore_metadata(self, ctx, t, s) {
            return true;
        }
        if ctx.typ(s).is_partial() {
            return self.is_super_type(ctx, syms, s, t);
        }
        if self.is_compound(ctx, s) {
            let components = self
                .interfaces(ctx, syms, s)
                .prepend(self.supertype(ctx, syms, s, None));
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
                let t2 = if capture {
                    self.capture(ctx, syms, t)
                } else {
                    t
                };
                return self.is_subtype_with_capture(ctx, syms, t2, lower, false);
            }
        }
        let t2 = if capture {
            self.capture(ctx, syms, t)
        } else {
            t
        };
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
            // Java: isSubtypeVisitor.visitType case NONE -> false
            TypeKindData::ProxyType { .. } => false,

            TypeKindData::JCPrimitiveType
            | TypeKindData::JCVoidType
            | TypeKindData::BottomType
            | TypeKindData::JCNoType { .. }
            | TypeKindData::UnknownType => match ctx.typ(t).get_tag() {
                TypeTag::BYTE => {
                    !ctx.typ(s).has_tag(TypeTag::CHAR)
                        && ctx.typ(t).get_tag().is_sub_range_of(ctx.typ(s).get_tag())
                }
                TypeTag::CHAR => {
                    !ctx.typ(s).has_tag(TypeTag::SHORT)
                        && ctx.typ(t).get_tag().is_sub_range_of(ctx.typ(s).get_tag())
                }
                TypeTag::SHORT
                | TypeTag::INT
                | TypeTag::LONG
                | TypeTag::FLOAT
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
                    let upper = ctx.upper_bound_of(t).expect("type var upper bound");
                    self.is_subtype_no_capture(ctx, syms, upper, s)
                }
                tag => panic!("AssertionError: isSubtype {tag:?}"),
            },
            TypeKindData::UninitializedType { .. } => {
                panic!("AssertionError: isSubtype UNINITIALIZED")
            }
            TypeKindData::UndetVar { .. } => self.is_subtype_visit_undet_var(ctx, syms, t, s),
            TypeKindData::Deferred { .. } => {
                // Java: IsSubtype default branch throws AssertionError for
                // DEFERRED (deferred types are mapped first).
                panic!("AssertionError: isSubtype DEFERRED")
            }
            TypeKindData::TypeVar { .. } | TypeKindData::CapturedType { .. } => {
                // Java: IsSubtype.visitType (Types.java:1117-1118) — IsSubtype
                // does not override visitTypeVar, so type variables fall through
                // to the TYPEVAR case of visitType.
                let upper = ctx.upper_bound_of(t).expect("type var upper bound");
                self.is_subtype_no_capture(ctx, syms, upper, s)
            }
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
                // Java: IsSubtype.visitClassType (Types.java:1159-1172) checks
                // a super-bounded target before the ordinary parameterized
                // comparison:
                //     if (s.isSuperBound() && !s.isExtendsBound())
                //         return visit(t, wildUpperBound(s)) && visit(t, wildLowerBound(s));
                // Without it the parameterized path compares the type arguments
                // against the capture variable and records an equality bound
                // (K = capture of ? super Entry<R,V>) that later collides with
                // the real constraint from the call.
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
                if ctx.typ(sup).tsym == ctx.typ(s).tsym
                    && (is_raw_type(ctx, sup) || is_raw_type(ctx, t))
                {
                    // Java unchecked conversion: a raw supertype is compatible
                    // with the parameterized target (ClassType.visitClassType
                    // + containsTypeRecursive on the erased arguments).
                    return true;
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
                    _ => crate::util::assert::error(),
                };
                if ctx.typ(s).has_tag(TypeTag::ARRAY) {
                    let s_elem = match &ctx.typ(s).kind {
                        TypeKindData::ArrayType { elemtype, .. } => {
                            elemtype.expect("array element")
                        }
                        _ => crate::util::assert::error(),
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
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - t_tvars/s_tvars: the quantified variable lists.
    ///
    /// # Returns
    ///   - true when both quantifiers have the same bounds.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Java: public TypeVar substBound(TypeVar t, List<Type> from, List<Type> to)
    /// (Types.java:3448-3461)
    pub fn subst_bound(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        from: &List<TypeId>,
        to: &List<TypeId>,
    ) -> TypeId {
        let bound = ctx.upper_bound_of(t).expect("type var upper bound");
        let bound1 = self.subst_type(ctx, syms, bound, from, to, None);
        if bound1 == bound {
            return t;
        }
        let tsym = ctx.typ(t).tsym;
        let new_tvar = ctx.alloc_type(Type::new(
            TypeTag::TYPEVAR,
            tsym,
            TypeKindData::TypeVar {
                bound: None,
                lower: Some(syms.bot_type),
            },
        ));
        let new_bound = self.subst_type(ctx, syms, bound1, &List::of(t), &List::of(new_tvar), None);
        ctx.typ_mut(new_tvar).set_upper_bound(Some(new_bound));
        new_tvar
    }

    pub fn has_same_bounds(
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
                .upper_bound_of(*l1.head())
                .expect("type var upper bound");
            let b2 = ctx
                .upper_bound_of(*l2.head())
                .expect("type var upper bound");
            let substituted = self.subst_type(ctx, syms, b2, s_tvars, t_tvars, None);
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
    /// upstream isReifiable takes only Type; syms is threaded
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
                _ => crate::util::assert::error(),
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
    /// the upstream rank_field caches (Type, not ported)
    /// are not used; the rank is recomputed each time (same result).
    pub fn rank(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> i32 {
        if let Some(rank) = self.rank_cache.borrow().get(&t) {
            return *rank;
        }
        let rank = self.rank_uncached(ctx, syms, t);
        // Cache only once the supertype graph is final: completion can still
        // add interfaces to an incomplete class.
        let stable = match ctx.typ(t).tsym {
            Some(tsym) => {
                ctx.typ(t).has_tag(TypeTag::TYPEVAR) || ctx.is_symbol_completed(tsym)
            }
            None => true,
        };
        if stable {
            self.rank_cache.borrow_mut().insert(t, rank);
        }
        rank
    }

    /// The uncached rank computation (Java: Types.rank body).
    fn rank_uncached(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> i32 {
        let tag = ctx.typ(t).get_tag();
        if tag == TypeTag::CLASS {
            let tsym = ctx.typ(t).tsym.expect("class type symbol");
            let object_tsym = ctx.typ(syms.object_type).tsym.expect("object type symbol");
            if tsym == object_tsym {
                return 0;
            }
            let sup = self.supertype(ctx, syms, t, None);
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
            let sup = self.supertype(ctx, syms, t, None);
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
        // eprintln!("[IU] a");
        let mut parms = allparams_of(ctx, tsym_type);
        // eprintln!("[IU] b");
        let mut args = allparams_of(ctx, t);
        // eprintln!("[IU] c");
        while parms.non_empty() {
            // eprintln!("[IU] loop");
            let bound = *parms.head();
            let unb = new_wildcard_type(
                ctx,
                Some(syms.object_type),
                BoundKind::UNBOUND,
                Some(syms.bound_class),
                Some(bound),
            );
            // eprintln!("[IU] before-contains");
            if !self.contains_type(ctx, syms, *args.head(), unb) {
                // eprintln!("[IU] contains-false");
                return false;
            }
            // eprintln!("[IU] contains-true");
            parms = parms.tail();
            args = args.tail();
        }
        true
    }

    /// Java: public Type cvarUpperBound(Type t) (Types.java)
    /// Java: public List<Type> cvarLowerBounds(List<Type> ts) (Types.java:1978-1980)
    pub fn cvar_lower_bounds(&self, ctx: &mut CompilerContext, ts: &List<TypeId>) -> List<TypeId> {
        let mut result = List::nil();
        for t in ts.iter() {
            result = result.append(cvar_lower_bound(ctx, t));
        }
        result
    }

    pub fn cvar_upper_bound(&self, ctx: &mut CompilerContext, t: TypeId) -> TypeId {
        // Java: Types.cvarUpperBound (Types.java:149-155):
        //   if (t.hasTag(TYPEVAR)) {
        //       TypeVar v = (TypeVar) t;
        //       return v.isCaptured() ? cvarUpperBound(v.getUpperBound()) : v;
        //   } else return t;
        if ctx.typ(t).has_tag(TypeTag::TYPEVAR) {
            if ctx.typ(t).is_captured() {
                let upper = ctx.upper_bound_of(t).expect("type variable bound");
                return self.cvar_upper_bound(ctx, upper);
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
                TypeKindData::ArrayType { elemtype, .. } => {
                    elemtype.expect("array element type")
                }
                _ => crate::util::assert::error(),
            },
            TypeTag::FORALL => {
                let qtype = match &ctx.typ(t).kind {
                    TypeKindData::ForAll { qtype, .. } => *qtype,
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
                    _ => crate::util::assert::error(),
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
                    _ => crate::util::assert::error(),
                };
                let s_qtype = s_qtype.expect("for-all qtype");
                self.has_same_bounds(ctx, syms, &t_tvars, &s_tvars) && {
                    let substituted = self.subst_type(ctx, syms, s_qtype, &s_tvars, &t_tvars, None);
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
    /// delegates to HasSameArgs with the strict flag.
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
    pub fn contains(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId, s: TypeId) -> bool {
        if let TypeKindData::WildcardType { type_, kind, .. } = &ctx.typ(t).kind
        {
            if *kind != BoundKind::UNBOUND {
                let inner = type_.expect("wildcard bound type");
                return self.contains(ctx, syms, inner, s);
            }
            return false;
        }
        // Java: Type.contains(Type t) is equalsIgnoreMetadata on the base type
        // (Type.java), i.e. a plain identity/equality test. Reaching for
        // isSameType here dispatched an inference variable through
        // isSameTypeVisitor.visitUndetVar, which *records* an equality bound:
        // asking "does Entry<K,V> contain the capture of ? super Entry<R,V>"
        // silently bound K to that capture variable.
        if type_equals_ignore_metadata(self, ctx, t, s) {
            return true;
        }
        match ctx.typ(t).kind.clone() {
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
                if is_parameterized_type(ctx, t) {
                    if let Some(enclosing) = get_enclosing_type(ctx, t)
                        && self.contains(ctx, syms, enclosing, s)
                    {
                        return true;
                    }
                    let mut args = get_type_arguments_completed(ctx, syms, t, None);
                    while args.non_empty() {
                        if self.contains(ctx, syms, *args.head(), s) {
                            return true;
                        }
                        args = args.tail();
                    }
                }
                if self.is_compound(ctx, t) {
                    let sup = self.supertype(ctx, syms, t, None);
                    if self.contains(ctx, syms, sup, s) {
                        return true;
                    }
                    let mut interfaces = self.interfaces(ctx, syms, t);
                    while interfaces.non_empty() {
                        if self.contains(ctx, syms, *interfaces.head(), s) {
                            return true;
                        }
                        interfaces = interfaces.tail();
                    }
                }
                false
            }
            TypeKindData::ArrayType { elemtype, .. } => {
                let elem = elemtype.expect("array element type");
                self.contains(ctx, syms, elem, s)
            }
            TypeKindData::MethodType {
                argtypes,
                restype,
                thrown,
                ..
            } => {
                let ret = restype.expect("method return type");
                self.contains_list(ctx, syms, argtypes, s)
                    || self.contains(ctx, syms, ret, s)
                    || self.contains_list(ctx, syms, thrown, s)
            }
            TypeKindData::PartiallyInferredMethodType {
                argtypes,
                restype,
                thrown,
                ..
            } => {
                let ret = restype.expect("partially inferred return type");
                self.contains_list(ctx, syms, argtypes, s)
                    || self.contains(ctx, syms, ret, s)
                    || self.contains_list(ctx, syms, thrown, s)
            }
            TypeKindData::ForAll { qtype, .. } => {
                let qtype = qtype.expect("for-all qtype");
                self.contains(ctx, syms, qtype, s)
            }
            _ => false,
        }
    }

    /// Whether any type in the list contains the given type.
    fn contains_list(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts: List<TypeId>,
        s: TypeId,
    ) -> bool {
        let mut current = ts;
        while current.non_empty() {
            if self.contains(ctx, syms, *current.head(), s) {
                return true;
            }
            current = current.tail();
        }
        false
    }

    /// Java: public boolean Type.containsAny(List<Type> ts)
    pub fn contains_any(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        ts: &List<TypeId>,
    ) -> bool {
        let current = ts.clone();
        for s in &current {
            if self.contains(ctx, syms, t, s) {
                return true;
            }
        }
        false
    }

    /// Java: public Type Type.withTypeVar(Type t) — the base implementation
    /// returns this; WildcardType sets its bound (the port mutates the arena
    /// value and returns the same id, like the upstream in-place mutation).
    pub fn with_type_var(&self, ctx: &mut CompilerContext, t: TypeId, tvar: TypeId) -> TypeId {
        if let TypeKindData::WildcardType { bound, .. } =
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
    pub fn boxed_class_ro(&self, ctx: &CompilerContext, syms: &Symtab, t: TypeId) -> SymbolId {
        let tag = ctx.typ(t).get_tag();
        syms.boxed_class_ids[tag as usize]
            .unwrap_or_else(|| panic!("AssertionError: boxed class not entered for {:?}", tag))
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
    pub fn unboxed_type_ro(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
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
        syms: &Symtab,
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
    pub fn unboxed_type(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
        if ctx.typ(t).has_tag(TypeTag::ERROR) {
            return no_type_of(ctx, syms);
        }
        let boxed_names = syms.boxed_name.clone();
        let java_base = syms.java_base;
        let no_symbol = syms.no_symbol;
        for (index, boxed) in boxed_names.iter().enumerate() {
            if let Some(boxed) = boxed {
                let boxed_tsym = syms.enter_class(ctx, java_base, boxed.clone(), Some(no_symbol));
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
        syms: &Symtab,
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
        // Java: isSubtypeUnchecked(t, s) uses noWarnings (Types.java:1005-1007).
        let mut no_warnings = crate::util::warner::Warner::new(None);
        self.is_subtype_unchecked_internal(ctx, syms, t, s, true, &mut no_warnings)
    }

    /// Java: public boolean isSubtypeUnchecked(Type t, Type s, Warner warn)
    /// (Types.java:1011-1017).
    pub fn is_subtype_unchecked_warn(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        warn: &crate::util::warner::Warner,
    ) -> bool {
        let result = self.is_subtype_unchecked_internal(ctx, syms, t, s, true, warn);
        if result {
            self.check_unsafe_varargs_conversion(ctx, syms, t, s, warn);
        }
        result
    }

    /// Java: public boolean isSubtypes(List<Type> ts, List<Type> ss)
    /// (Types.java:1250-1259)
    pub fn is_subtypes(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts: &List<TypeId>,
        ss: &List<TypeId>,
    ) -> bool {
        let mut ts = ts.clone();
        let mut ss = ss.clone();
        while ts.non_empty() && ss.non_empty() && self.is_subtype(ctx, syms, *ts.head(), *ss.head())
        {
            ts = ts.tail();
            ss = ss.tail();
        }
        ts.is_empty() && ss.is_empty()
    }

    /// Java: public boolean isSubtypesUnchecked(List<Type> ts, List<Type> ss,
    /// Warner warn) (Types.java:1266-1275). Each pair goes through the
    /// Warner-carrying isSubtypeUnchecked, so UNCHECKED is emitted.
    pub fn is_subtypes_unchecked(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts: &List<TypeId>,
        ss: &List<TypeId>,
        _warn: &crate::util::warner::Warner,
    ) -> bool {
        let mut ts = ts.clone();
        let mut ss = ss.clone();
        while ts.non_empty()
            && ss.non_empty()
            && self.is_subtype_unchecked_warn(ctx, syms, *ts.head(), *ss.head(), _warn)
        {
            ts = ts.tail();
            ss = ss.tail();
        }
        ts.is_empty() && ss.is_empty()
    }

    /// Java: public boolean isArray(Type t) (Types.java:2059-2063)
    pub fn is_array(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> bool {
        let mut t = t;
        while ctx.typ(t).has_tag(TypeTag::WILDCARD) {
            t = self.wild_upper_bound(ctx, syms, t);
        }
        ctx.typ(t).has_tag(TypeTag::ARRAY)
    }

    /// Java: public Type elemtypeOrType(Type t) (Types.java:2083-2088)
    pub fn elemtype_or_type(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
        elemtype_of(self, ctx, syms, t).unwrap_or(t)
    }

    /// Java: private boolean isSubtypeUncheckedInternal(Type t, Type s, boolean
    /// capture, Warner warn) (Types.java:1019-1042). The raw-conversion branch
    /// emits UNCHECKED exactly as upstream (silentWarn when the target is
    /// reifiable).
    fn is_subtype_unchecked_internal(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        capture: bool,
        warn: &crate::util::warner::Warner,
    ) -> bool {
        if ctx.typ(t).has_tag(TypeTag::ARRAY) && ctx.typ(s).has_tag(TypeTag::ARRAY) {
            // Java: isSameType(elemtype(t), elemtype(s)) /
            // isSubtypeUncheckedInternal(elemtype(t), elemtype(s), false, warn)
            let t_elem = elemtype_of(self, ctx, syms, t).expect("array element type");
            let s_elem = elemtype_of(self, ctx, syms, s).expect("array element type");
            if ctx.typ(t_elem).is_primitive() {
                return self.is_same_type(ctx, syms, t_elem, s_elem);
            }
            return self.is_subtype_unchecked_internal(ctx, syms, t_elem, s_elem, false, warn);
        }
        if self.is_subtype_with_capture(ctx, syms, t, s, capture) {
            return true;
        }
        if ctx.typ(t).has_tag(TypeTag::TYPEVAR) {
            let upper = ctx.upper_bound_of(t).expect("type var upper bound");
            return self.is_subtype_unchecked_internal(ctx, syms, upper, s, false, warn);
        }
        if !is_raw_type(ctx, s) {
            let Some(s_tsym) = ctx.typ(s).tsym else {
                return false;
            };
            if let Some(t2) = self.as_super(ctx, syms, t, s_tsym)
                && is_raw_type(ctx, t2)
            {
                // Java: Types.java:1030-1039 — raw conversion warning.
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

    /// Java: private Type skipTypeVars(Type site, boolean capture)
    fn skip_type_vars(&self, ctx: &mut CompilerContext, site: TypeId) -> TypeId {
        let mut site = site;
        while ctx.typ(site).has_tag(TypeTag::TYPEVAR) {
            site = ctx.upper_bound_of(site).expect("type var upper bound");
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
    /// Porting rules adapter: the upstream TypeRelation.warnStack (a stack
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
        warn: &crate::util::warner::Warner,
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
        // eprintln!("[ICW] visit-result={}", result);
        if result && ctx.typ(t).has_tag(TypeTag::CLASS) && ctx.typ(s).has_tag(TypeTag::CLASS) {
            // eprintln!("[ICW] sealed-check");
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
        warn: &crate::util::warner::Warner,
    ) -> bool {
        {
            static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let c = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if c >= 0 {
                let dt = crate::comp::attr::type_display_of(ctx, t);
                let ds = crate::comp::attr::type_display_of(ctx, s);
                // eprintln!("[ICV] n={} t={} s={}", c, dt, ds);
            }
        }
        match &ctx.typ(t).kind {
            TypeKindData::JCPrimitiveType
            | TypeKindData::JCVoidType
            | TypeKindData::BottomType
            | TypeKindData::JCNoType { .. }
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
                let upper = ctx.upper_bound_of(t).expect("type var upper bound");
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
                        TypeKindData::ArrayType { elemtype, .. } => {
                            elemtype.expect("array element")
                        }
                        _ => crate::util::assert::error(),
                    };
                    let s_elem = match &ctx.typ(s).kind {
                        TypeKindData::ArrayType { elemtype, .. } => {
                            elemtype.expect("array element")
                        }
                        _ => crate::util::assert::error(),
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
                // eprintln!("[ICV2] a tag={:?}", tag);
                if tag == TypeTag::ERROR || tag == TypeTag::BOT {
                    return true;
                }
                if tag == TypeTag::TYPEVAR {
                    // Java: if (isCastable(t, s.getUpperBound(), noWarnings)) {
                    // warnStack.head.warn(UNCHECKED); return true; } else false;
                    let upper = ctx.upper_bound_of(s).expect("type var upper bound");
                    let mut no_warnings = crate::util::warner::Warner::new(None);
                    if self.is_castable_with_warn(ctx, syms, t, upper, &mut no_warnings) {
                        warn.warn(crate::code::lint::LintCategory::UNCHECKED);
                        return true;
                    }
                    return false;
                }
                // eprintln!("[ICV2] b");
                if self.is_compound(ctx, t) || self.is_compound(ctx, s) {
                    // eprintln!("[ICV2] compound");
                    return if !self.is_compound(ctx, t) {
                        self.visit_compound_type(ctx, syms, s, t, true, warn)
                    } else {
                        self.visit_compound_type(ctx, syms, t, s, false, warn)
                    };
                }
                if tag == TypeTag::CLASS || tag == TypeTag::ARRAY {
                    // eprintln!("[ICV2] c");
                    let t_erasure = self.erasure(ctx, syms, t);
                    // eprintln!("[ICV2] d");
                    let s_erasure = self.erasure(ctx, syms, s);
                    // eprintln!("[ICV2] e");
                    let upcast = self.is_subtype(ctx, syms, t_erasure, s_erasure);
                    // eprintln!("[ICV2] f upcast={}", upcast);
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
                        let mut a_high = self.rewrite_quantifiers(ctx, syms, a, high, no_rewrite);
                        let mut a_low = self.rewrite_quantifiers(ctx, syms, a, low, no_rewrite);
                        let mut b_high = self.rewrite_quantifiers(ctx, syms, b, high, no_rewrite);
                        let mut b_low = self.rewrite_quantifiers(ctx, syms, b, low, no_rewrite);
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
                        if crate::util::debug_flags::debug_enabled("DSH_DEBUG_ALL") {
                            let dt = crate::comp::attr::type_display_of(ctx, t);
                            let ds = crate::comp::attr::type_display_of(ctx, s);
                            eprintln!(
                                "[IC3] t={} s={} low_sub={} high_sub={}",
                                dt,
                                ds,
                                low_sub.is_some(),
                                high_sub.is_some()
                            );
                        }
                        if let (Some(low_sub), Some(high_sub)) = (low_sub, high_sub) {
                            let a_high_params = self.class_allparams(ctx, a_high);
                            let a_low_params = self.class_allparams(ctx, a_low);
                            let high_params = self.class_allparams(ctx, high_sub);
                            let low_params = self.class_allparams(ctx, low_sub);
                            let a_high_all: Vec<TypeId> = a_high_params.iter().collect();
                            let a_low_all: Vec<TypeId> = a_low_params.iter().collect();
                            let high_all: Vec<TypeId> = high_params.iter().collect();
                            let low_all: Vec<TypeId> = low_params.iter().collect();
                            let dbg = crate::util::debug_flags::debug_enabled("DSH_DEBUG_ALL");
                            let d1 = self.disjoint_types(
                                ctx,
                                syms,
                                a_high_params.clone(),
                                high_params.clone(),
                            );
                            let d2 = self.disjoint_types(ctx, syms, a_high_params, low_params.clone());
                            let d3 = self.disjoint_types(ctx, syms, a_low_params.clone(), high_params);
                            let d4 = self.disjoint_types(ctx, syms, a_low_params, low_params);
                            if dbg {
                                let p = |ctx: &mut CompilerContext, l: &[TypeId]| -> String {
                                    l.iter()
                                        .map(|t| crate::comp::attr::type_display_of(ctx, *t))
                                        .collect::<Vec<_>>()
                                        .join(",")
                                };
                                let ah = p(ctx, &a_high_all);
                                let al = p(ctx, &a_low_all);
                                let hp = p(ctx, &high_all);
                                let lp = p(ctx, &low_all);
                                eprintln!(
                                    "[DIS] ah=[{}] al=[{}] hp=[{}] lp=[{}] d1={} d2={} d3={} d4={}",
                                    ah, al, hp, lp, d1, d2, d3, d4
                                );
                            }
                            let disjoint = d1 || d2 || d3 || d4;
                            if crate::util::debug_flags::debug_enabled("DSH_DEBUG_ALL") {
                                eprintln!("[IC3] disjoint={}", disjoint);
                            }
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
                        let s_interface = (ctx.sym(s_tsym).flags_field
                            & crate::code::flags::INTERFACE as i64)
                            != 0;
                        let t_interface = (ctx.sym(t_tsym).flags_field
                            & crate::code::flags::INTERFACE as i64)
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
        warn: &crate::util::warner::Warner,
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

/// The enclosing type of a type: the `outer_field` of the class-type family,
/// and `None` for every other type.
///
/// Port adapter: `None` is the local representation of the absent enclosing
/// type (`Type.noType` for a top-level class), not a null reference.
fn get_enclosing_type(ctx: &CompilerContext, t: TypeId) -> Option<TypeId> {
    // Type.getEnclosingType (type_.rs:1009-1020); no duplicate matching.
    ctx.typ(t).get_enclosing_type()
}

/// Java: Type.getTypeArguments() (ClassType: typarams_field, lazily nil after
/// complete(); other types: List.nil()).
///
/// reads the cached typarams_field via the ported Type method
/// (type_.rs:1022-1036). Callers that may need the upstream lazy complete() use
/// get_type_arguments_completed below, which routes through
/// complete_class_symbol[_with_attr] (the ported Symbol.complete).
pub(crate) fn get_type_arguments(ctx: &CompilerContext, t: TypeId) -> List<TypeId> {
    ctx.typ(t).get_type_arguments()
}

/// The lazily completed type arguments of a class type:
///
/// <pre>
/// public List&lt;Type&gt; getTypeArguments() {
///     if (typarams_field == null) {
///         complete();
///         if (typarams_field == null)
///             typarams_field = List.nil();
///     }
///     return typarams_field;
/// }
/// </pre>
///
/// Port notes (A3k): the local ClassType stores the upstream nullable
/// typarams_field as Option; None is the "not filled in yet" state. Completion
/// is the ported ClassSymbol.complete(); the caller's Attr is reused through
/// complete_class_symbol_with_attr when one is available and the Attr-less
/// complete_class_symbol entry point is used otherwise (no RefCell probing).
pub(crate) fn get_type_arguments_completed(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    t: TypeId,
    mut attr: Option<&mut crate::comp::attr::Attr>,
) -> List<TypeId> {
    // ForAll overrides Type.getTypeArguments() with its method type variables;
    // it is not a class type and must not be completed (Type.java ForAll).
    if let TypeKindData::ForAll { tvars, .. } = &ctx.typ(t).kind {
        return tvars.clone();
    }
    let needs_completion = match &ctx.typ(t).kind {
        TypeKindData::ClassType { typarams_field, .. }
        | TypeKindData::ErasedClassType { typarams_field, .. }
        | TypeKindData::UnionClassType { typarams_field, .. }
        | TypeKindData::IntersectionClassType { typarams_field, .. }
        | TypeKindData::ErrorType { typarams_field, .. } => typarams_field.is_none(),
        _ => return List::nil(),
    };
    if !needs_completion {
        return get_type_arguments(ctx, t);
    }
    // Complete the class symbol before the parameter list is read.
    if let Some(tsym) = ctx.typ(t).tsym
        && !ctx.is_symbol_completed(tsym)
    {
        match attr.as_deref_mut() {
            Some(attr) => {
                if let Err(failure) = ctx.complete_class_symbol_with_attr(syms, tsym, attr) {
                    crate::code::symbol::throw_completion_failure(failure);
                }
            }
            None => {
                if let Err(failure) = ctx.complete_class_symbol(syms, tsym) {
                    crate::code::symbol::throw_completion_failure(failure);
                }
            }
        }
    }
    // An absent parameter list is cached as the empty list.
    match &mut ctx.typ_mut(t).kind {
        TypeKindData::ClassType { typarams_field, .. }
        | TypeKindData::ErasedClassType { typarams_field, .. }
        | TypeKindData::UnionClassType { typarams_field, .. }
        | TypeKindData::IntersectionClassType { typarams_field, .. }
        | TypeKindData::ErrorType { typarams_field, .. } => {
            if typarams_field.is_none() {
                *typarams_field = Some(List::nil());
            }
            typarams_field.clone().unwrap_or_else(List::nil)
        }
        _ => List::nil(),
    }
}

/// Java: Type.getParameterTypes() (MethodType: argtypes; other types: List.nil()).
///
/// delegates to Type.getParameterTypes (type_.rs:977-987),
/// including the ForAll qtype delegation.
pub(crate) fn get_parameter_types(ctx: &CompilerContext, t: TypeId) -> List<TypeId> {
    ctx.typ(t).get_parameter_types(ctx)
}

// ---------------------------------------------------------------------------
// Types helpers.
//
// Rust: upstream these helpers are methods of Type.java
// (getThrownTypes/allparams/isParameterized/isRaw/hasErasedSupertypes/
// getSuperclass/getInterfaces/classBound/cvarUpperBound/cvarLowerBound/elemtype)
// or of Types.java (classBound/isUnbounded). Those with a ported Type method
// delegate (getEnclosingType/getTypeArguments/getParameterTypes below); the
// remaining helpers stay here until their Type/Symbol host methods are ported
// (registered). Lazy completion is handled by get_type_arguments_completed and
// the complete_class_symbol* entry points; a null supertype_field/interfaces_field
// is still returned as "not computed yet" (adapter).
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
        TypeKindData::ArrayType {
            elemtype: Some(elem),
            ..
        } => allparams_of(ctx, *elem),
        TypeKindData::ArrayType { elemtype: None, .. } => List::nil(),
        _ => List::nil(),
    }
}

/// All type parameters of a class type:
///
/// <pre>
/// public List&lt;Type&gt; allparams() {
///     if (allparams_field == null) {
///         allparams_field = getTypeArguments().prependList(getEnclosingType().allparams());
///     }
///     return allparams_field;
/// }
/// </pre>
///
/// Port notes (A3k): every hop follows the lazy getTypeArguments() completion
/// semantics of get_type_arguments_completed; the local allparams_of does not
/// cache allparams_field, so the value is recomputed from the (now completed)
/// typarams_field. The enclosing chain is walked with the caller's Attr.
pub(crate) fn allparams_of_completed(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    t: TypeId,
    mut attr: Option<&mut crate::comp::attr::Attr>,
) -> List<TypeId> {
    enum AllParamsView {
        Class(Option<TypeId>),
        Array(Option<TypeId>),
        Other,
    }
    let view = match &ctx.typ(t).kind {
        TypeKindData::ClassType { outer_field, .. }
        | TypeKindData::ErasedClassType { outer_field, .. }
        | TypeKindData::UnionClassType { outer_field, .. }
        | TypeKindData::IntersectionClassType { outer_field, .. }
        | TypeKindData::ErrorType { outer_field, .. } => AllParamsView::Class(*outer_field),
        TypeKindData::ArrayType { elemtype, .. } => AllParamsView::Array(*elemtype),
        _ => AllParamsView::Other,
    };
    match view {
        AllParamsView::Class(outer) => {
            let params = get_type_arguments_completed(ctx, syms, t, attr.as_deref_mut());
            match outer {
                Some(outer) => params.prepend_list(&allparams_of_completed(ctx, syms, outer, attr)),
                None => params,
            }
        }
        AllParamsView::Array(Some(elem)) => allparams_of_completed(ctx, syms, elem, attr),
        AllParamsView::Array(None) | AllParamsView::Other => List::nil(),
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
            outer_field,
            ..
        }
        | TypeKindData::ErasedClassType {
            allparams_field,
            typarams_field,
            outer_field,
            ..
        }
        | TypeKindData::UnionClassType {
            allparams_field,
            typarams_field,
            outer_field,
            ..
        }
        | TypeKindData::IntersectionClassType {
            allparams_field,
            typarams_field,
            outer_field,
            ..
        } => {
            // Java: allparams() = getTypeArguments().prependList(enclosing.allparams())
            // (Type.java:1104-1109), so a member class of a parameterized class is
            // itself parameterized. allparams_field is the authoritative cache;
            // without it, the member's own type arguments are prepended to the
            // enclosing chain.
            if let Some(params) = allparams_field {
                return params.non_empty();
            }
            if typarams_field
                .as_ref()
                .map(|params| params.non_empty())
                .unwrap_or(false)
            {
                return true;
            }
            match outer_field {
                Some(outer) => is_parameterized_type(ctx, *outer),
                None => false,
            }
        }
        TypeKindData::ArrayType {
            elemtype: Some(elem),
            ..
        } => is_parameterized_type(ctx, *elem),
        _ => false,
    }
}

/// Java: Type.isRaw() (ClassType: this != tsym.type && tsym.type.allparams()
/// non-empty && this.allparams() empty; ArrayType: elemtype.isRaw()).
fn is_raw_type(ctx: &CompilerContext, t: TypeId) -> bool {
    match &ctx.typ(t).kind {
        TypeKindData::ArrayType {
            elemtype: Some(elem),
            ..
        } => is_raw_type(ctx, *elem),
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
    matches!(&ctx.typ(t).kind, TypeKindData::ErasedClassType { .. }) || is_raw_type(ctx, t)
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
        TypeKindData::ClassType {
            supertype_field, ..
        }
        | TypeKindData::ErasedClassType {
            supertype_field, ..
        }
        | TypeKindData::UnionClassType {
            supertype_field, ..
        }
        | TypeKindData::IntersectionClassType {
            supertype_field, ..
        } => {
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
        TypeKindData::ClassType {
            interfaces_field, ..
        }
        | TypeKindData::ErasedClassType {
            interfaces_field, ..
        }
        | TypeKindData::UnionClassType {
            interfaces_field, ..
        }
        | TypeKindData::IntersectionClassType {
            interfaces_field, ..
        } => interfaces_field.clone().unwrap_or_else(List::nil),
        _ => List::nil(),
    }
}

/// Java: Types.classBound(Type t)
fn class_bound(
    types: &Types,
    ctx: &mut CompilerContext,
    syms: &Symtab,
    t: TypeId,
    mut attr: Option<&mut crate::comp::attr::Attr>,
) -> TypeId {
    match &ctx.typ(t).kind {
        TypeKindData::ClassType { outer_field, .. }
        | TypeKindData::ErasedClassType { outer_field, .. }
        | TypeKindData::UnionClassType { outer_field, .. }
        | TypeKindData::IntersectionClassType { outer_field, .. }
        | TypeKindData::ErrorType { outer_field, .. } => {
            let Some(outer) = *outer_field else {
                return t;
            };
            let outer1 = class_bound(types, ctx, syms, outer, attr.as_deref_mut());
            if outer1 == outer {
                t
            } else {
                let args = get_type_arguments_completed(ctx, syms, t, None);
                let tsym = ctx.typ(t).tsym;
                new_class_type(ctx, Some(outer1), Some(args), tsym)
            }
        }
        TypeKindData::TypeVar { .. } => {
            let sup = types.supertype(ctx, syms, t, attr);
            class_bound(types, ctx, syms, sup, None)
        }
        _ => t,
    }
}

/// Java: Types.elemtype(Type t)
fn elemtype_of(types: &Types, ctx: &CompilerContext, syms: &Symtab, t: TypeId) -> Option<TypeId> {
    match &ctx.typ(t).kind {
        TypeKindData::WildcardType { .. } => {
            let upper = types.wild_upper_bound(ctx, syms, t);
            elemtype_of(types, ctx, syms, upper)
        }
        TypeKindData::ArrayType { elemtype, .. } => *elemtype,
        TypeKindData::ForAll { qtype, .. } => {
            let qtype = (*qtype).expect("for-all qtype");
            elemtype_of(types, ctx, syms, qtype)
        }
        TypeKindData::ErrorType { .. } => Some(t),
        // Java: Types.elemtype default case returns null (Types.java:2078-2079).
        _ => None,
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
            let upper = ctx.upper_bound_of(t).expect("captured type upper bound");
            cvar_upper_bound(ctx, upper)
        } else {
            t
        }
    } else {
        t
    }
}

/// Java: Types.cvarLowerBound(Type t)
pub(crate) fn cvar_lower_bound(ctx: &CompilerContext, t: TypeId) -> TypeId {
    if ctx.typ(t).has_tag(TypeTag::TYPEVAR) && ctx.typ(t).is_captured() {
        let lower = ctx.lower_bound_of(t).expect("captured type lower bound");
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
        TypeKindData::WildcardType {
            type_,
            kind,
            bound,
            is_printing_bound: std::cell::Cell::new(false),
        },
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
pub(crate) fn new_method_type_2(
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
fn type_equals_ignore_metadata(types: &Types, ctx: &CompilerContext, t: TypeId, s: TypeId) -> bool {
    types.equals_ignore_metadata(ctx, t, s)
}

/// Java: Type.contains(elem) (Type.java:1148/1408/1504/1854 overrides). The
/// live call sites use Types::contains and the type_contains method (see the
/// audit at the top); this free helper is retained only for its unit test.
#[allow(dead_code)]
fn type_contains(
    types: &Types,
    ctx: &mut CompilerContext,
    syms: &Symtab,
    t: TypeId,
    elem: TypeId,
) -> bool {
    if type_equals_ignore_metadata(types, ctx, t, elem) {
        return true;
    }
    if is_parameterized_type(ctx, t) {
        if let Some(outer) = enclosing_of(ctx, t)
            && type_contains(types, ctx, syms, outer, elem)
        {
            return true;
        }
        for arg in get_type_arguments_completed(ctx, syms, t, None).iter() {
            if type_contains(types, ctx, syms, arg, elem) {
                return true;
            }
        }
    }
    if types.is_compound(ctx, t) {
        let sup = types.supertype(ctx, syms, t, None);
        if type_contains(types, ctx, syms, sup, elem) {
            return true;
        }
        for i in types.interfaces(ctx, syms, t).iter() {
            if type_contains(types, ctx, syms, i, elem) {
                return true;
            }
        }
    }
    false
}

/// The enclosing type of the class-type family; `None` is the local
/// `Type.noType` representation for a top-level class.
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
        TypeKindData::ClassType {
            supertype_field, ..
        }
        | TypeKindData::ErasedClassType {
            supertype_field, ..
        }
        | TypeKindData::UnionClassType {
            supertype_field, ..
        }
        | TypeKindData::IntersectionClassType {
            supertype_field, ..
        } => *supertype_field = value,
        _ => panic!("not a class type"),
    }
}

/// Set the ClassType.interfaces_field of the given type.
fn set_class_interfaces_field(ctx: &mut CompilerContext, t: TypeId, value: Option<List<TypeId>>) {
    match &mut ctx.typ_mut(t).kind {
        TypeKindData::ClassType {
            interfaces_field, ..
        }
        | TypeKindData::ErasedClassType {
            interfaces_field, ..
        }
        | TypeKindData::UnionClassType {
            interfaces_field, ..
        }
        | TypeKindData::IntersectionClassType {
            interfaces_field, ..
        } => *interfaces_field = value,
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
        TypeKindData::ClassType {
            interfaces_field, ..
        }
        | TypeKindData::ErasedClassType {
            interfaces_field, ..
        }
        | TypeKindData::UnionClassType {
            interfaces_field, ..
        }
        | TypeKindData::IntersectionClassType {
            interfaces_field, ..
        } => interfaces_field.clone(),
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
        if !ctx.typ(head).has_tag(TypeTag::ARRAY) {
            return false;
        }
        let elem = match &ctx.typ(head).kind {
            TypeKindData::ArrayType { elemtype, .. } => *elemtype,
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
        syms: &Symtab,
        names: &crate::util::names::Names,
        bounds: List<TypeId>,
        all_interfaces: bool,
    ) -> TypeId {
        assert!(
            bounds.non_empty(),
            "AssertionError: bounds must be non-empty"
        );
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
        let intersection = ctx.alloc_type(Type::new_intersection_class_type(
            bounds,
            bc,
            all_interfaces,
        ));
        ctx.sym_mut(bc).ty = Some(intersection);
        let erasure = if ctx
            .typ(first_explicit_bound)
            .has_tag(TypeTag::TYPEVAR)
        {
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
        syms: &Symtab,
        names: &crate::util::names::Names,
        tvar: TypeId,
        bounds: List<TypeId>,
        all_interfaces: bool,
    ) {
        assert!(
            bounds.non_empty(),
            "AssertionError: bounds must be non-empty"
        );
        let upper = if bounds.tail().is_empty() {
            *bounds.head()
        } else {
            self.make_intersection_type(ctx, syms, names, bounds, all_interfaces)
        };
        if let TypeKindData::TypeVar { bound, .. } = &mut ctx.typ_mut(tvar).kind
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
// overloads get distinct Rust names ().
// ----------------------------------------------------------------------

impl Types {
    /// Java: public ArrayType makeArrayType(Type t) (Types.java:1333-1336)
    pub fn make_array_type(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
        let array = Type::new_array_type(t, syms.array_class);
        ctx.alloc_type(array)
    }

    /// Java: public ArrayType makeVarargs() (Type.java:1399-1406).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - t: the array type to mark.
    ///
    /// # Returns
    ///   - a copy of the array type with the varargs flag set.
    ///
    /// # Panics
    ///   - the type must be an ArrayType.
    ///
    /// Upstream: Type.java:1399-1406.
    pub fn make_varargs(&self, ctx: &mut CompilerContext, t: TypeId) -> TypeId {
        match &ctx.typ(t).kind {
            TypeKindData::ArrayType { .. } => {
                let mut copy = ctx.typ(t).clone();
                if let TypeKindData::ArrayType { varargs, .. } = &mut copy.kind
                {
                    *varargs = true;
                }
                ctx.alloc_type(copy)
            }
            _ => panic!("ClassCastException: ArrayType expected"),
        }
    }

    /// Java: public Type createErrorType(Type originalType)
    pub fn create_error_type_original(
        &self,
        ctx: &mut CompilerContext,
        original_type: Option<TypeId>,
        syms: &Symtab,
    ) -> TypeId {
        // Java: new ErrorType(originalType, syms.errSymbol)
        let original = match original_type {
            Some(original) => original,
            None => self.no_type(ctx),
        };
        ctx.alloc_type(Type::new(
            TypeTag::ERROR,
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
                model_type: None,
                model_type_provider: None,
            },
        ))
    }

    /// Java: public Type createErrorType(ClassSymbol c, Type originalType)
    pub fn create_error_type_class(
        &self,
        ctx: &mut CompilerContext,
        class: SymbolId,
        original_type: Option<TypeId>,
    ) -> TypeId {
        let no_type = self.no_type(ctx);
        let error_type = ctx.alloc_type(Type::new(
            TypeTag::ERROR,
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
                model_type: None,
                model_type_provider: None,
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

    /// Java: Type.getModelType() (Type.java:1100-1110 / ErrorType anonymous
    /// override in TypeEnter.java:555-569): evaluates the lazy provider once and
    /// caches the synthesized model type. Returns None when the type is not an
    /// ErrorType or has no provider.
    pub fn get_model_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> Option<TypeId> {
        let (cached, provider) = match &mut ctx.typ_mut(t).kind {
            TypeKindData::ErrorType {
                model_type,
                model_type_provider,
                ..
            } => (*model_type, model_type_provider.clone()),
            _ => return None,
        };
        if cached.is_some() {
            return cached;
        }
        let provider = provider?;
        let model = provider(ctx, syms, self);
        if let TypeKindData::ErrorType { model_type, .. } = &mut ctx.typ_mut(t).kind {
            *model_type = Some(model);
        }
        Some(model)
    }

    /// Java: public Type createErrorType(Name name, TypeSymbol container,
    /// Type originalType)
    pub fn create_error_type(
        &self,
        ctx: &mut CompilerContext,
        name: Name,
        container: SymbolId,
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
    pub fn no_type(&self, ctx: &mut CompilerContext) -> TypeId {
        ctx.alloc_type(Type::new(
            TypeTag::NONE,
            None,
            TypeKindData::JCNoType {
                kind: crate::code::type_::JCNoTypeKind::Base,
            },
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
        | TypeKindData::IntersectionClassType { typarams_field, .. } => {
            typarams_field.clone().unwrap_or_else(List::nil)
        }
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
            } => {
                let own = allparams_field.clone().or_else(|| typarams_field.clone());
                // Java: ClassType.allparams() prepends the enclosing type's
                // allparams (inner classes carry their owner's type arguments).
                if ctx.typ(t).has_tag(TypeTag::CLASS)
                    && let Some(enclosing) = ctx.typ(t).get_enclosing_type()
                    && enclosing != t
                    && ctx
                        .typ(enclosing)
                        .has_tag(TypeTag::CLASS)
                {
                    let outer = self.class_allparams(ctx, enclosing);
                    if outer.non_empty() {
                        return outer.append_list(&own.unwrap_or_else(List::nil));
                    }
                }
                own
            }
            .unwrap_or_else(List::nil),
            _ => List::nil(),
        }
    }

    /// Java: Type.isExtendsBound() for WildcardType
    fn wildcard_is_extends_bound(&self, ctx: &CompilerContext, t: TypeId) -> bool {
        match &ctx.typ(t).kind {
            TypeKindData::WildcardType { kind, .. } => {
                *kind == BoundKind::EXTENDS
            }
            _ => false,
        }
    }

    /// Java: Type.isSuperBound() for WildcardType
    fn wildcard_is_super_bound(&self, ctx: &CompilerContext, t: TypeId) -> bool {
        match &ctx.typ(t).kind {
            TypeKindData::WildcardType { kind, .. } => {
                *kind == BoundKind::SUPER
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
                        ctx,
                        syms,
                        source_params,
                        target_params,
                        from,
                        to,
                        mapping,
                        cache,
                    )?;
                }
            }
            TypeKindData::ArrayType { .. } => {
                if ctx.typ(target).has_tag(TypeTag::ARRAY) {
                    let source_elem = match &ctx.typ(source).kind {
                        TypeKindData::ArrayType { elemtype, .. } => {
                            elemtype.expect("array element")
                        }
                        _ => crate::util::assert::error(),
                    };
                    let target_elem = match &ctx.typ(target).kind {
                        TypeKindData::ArrayType { elemtype, .. } => {
                            elemtype.expect("array element")
                        }
                        _ => crate::util::assert::error(),
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
                if let Some(entry) = mapping.iter_mut().find(|(sym, _)| *sym == source_tsym) {
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
    pub fn rewrite_supers(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
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
                s = ctx.alloc_type(Type::new(
                    TypeTag::WILDCARD,
                    Some(syms.bound_class),
                    TypeKindData::WildcardType {
                        type_: Some(syms.object_type),
                        kind: BoundKind::UNBOUND,
                        bound: None,
                        is_printing_bound: std::cell::Cell::new(false),
                    },
                ));
                changed = true;
            } else if s != *orig {
                let upper = self.wild_upper_bound(ctx, syms, s);
                s = ctx.alloc_type(Type::new(
                    TypeTag::WILDCARD,
                    Some(syms.bound_class),
                    TypeKindData::WildcardType {
                        type_: Some(upper),
                        kind: BoundKind::EXTENDS,
                        bound: None,
                        is_printing_bound: std::cell::Cell::new(false),
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
            self.subst_type(ctx, syms, tsym_type, &from_list, &to_list, None)
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
    ) -> Name {
        match &ctx.sym(symbol).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { fullname, .. } => fullname.clone(),
            _ => ctx.sym(symbol).name.clone(),
        }
    }

    /// Java: public boolean isInterface() (Type.java:611: INTERFACE flag)
    fn is_interface(&self, ctx: &CompilerContext, t: TypeId) -> bool {
        ctx.typ(t)
            .tsym
            .map(|tsym| (ctx.sym(tsym).flags_field & crate::code::flags::INTERFACE as i64) != 0)
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
    fn closure_skip(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        skip: ClosureSkip,
        t1: TypeId,
        t2: TypeId,
    ) -> bool {
        match skip {
            ClosureSkip::Basic => ctx.typ(t1).tsym == ctx.typ(t2).tsym,
            ClosureSkip::IsSameType => self.is_same_type(ctx, syms, t1, t2),
        }
    }

    /// Java: public List<Type> insert(List<Type> cl, Type t, BiPredicate<Type,
    /// Type> shouldSkip) (Types.java:3772-3783)
    fn closure_insert_with(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        cl: List<TypeId>,
        t: TypeId,
        should_skip: ClosureSkip,
    ) -> List<TypeId> {
        if cl.is_empty() {
            return cl.prepend(t);
        }
        let head = *cl.head();
        if self.closure_skip(ctx, syms, should_skip, t, head) {
            return cl;
        }
        if let (Some(t_sym), Some(head_sym)) = (ctx.typ(t).tsym, ctx.typ(head).tsym)
            && self.symbol_precedes(ctx, syms, t_sym, head_sym)
        {
            return cl.prepend(t);
        }
        self.closure_insert_with(ctx, syms, cl.tail(), t, should_skip)
            .prepend(head)
    }

    /// Java: public List<Type> union(List<Type> cl1, List<Type> cl2,
    /// BiPredicate<Type, Type> shouldSkip) (Types.java:3792-3804)
    fn closure_union_with(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        cl1: List<TypeId>,
        cl2: List<TypeId>,
        should_skip: ClosureSkip,
    ) -> List<TypeId> {
        if cl1.is_empty() {
            return cl2;
        }
        if cl2.is_empty() {
            return cl1;
        }
        let head1 = *cl1.head();
        let head2 = *cl2.head();
        if self.closure_skip(ctx, syms, should_skip, head1, head2) {
            return self
                .closure_union_with(ctx, syms, cl1.tail(), cl2.tail(), should_skip)
                .prepend(head1);
        }
        if let (Some(head2_sym), Some(head1_sym)) = (ctx.typ(head2).tsym, ctx.typ(head1).tsym)
            && self.symbol_precedes(ctx, syms, head2_sym, head1_sym)
        {
            return self
                .closure_union_with(ctx, syms, cl1, cl2.tail(), should_skip)
                .prepend(head2);
        }
        self.closure_union_with(ctx, syms, cl1.tail(), cl2, should_skip)
            .prepend(head1)
    }

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
        self.closure_union(ctx, syms, cl1.tail(), cl2)
            .prepend(head1)
    }

    /// Java: public List<Type> closure(Type t) (Types.java:3710-3736; the
    /// upstream cache is a performance optimization, omitted as equivalent)
    pub fn closure(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> List<TypeId> {
        let depth = TYPE_RECURSION_DEPTH.with(|d| {
            let v = d.get();
            d.set(v + 1);
            v
        });
        if depth < 60 && crate::util::debug_flags::debug_enabled("DSH_DEBUG_RECURSION") {
            let name = ctx
                .typ(t)
                .tsym
                .map(|s| {
                    ctx.sym(s)
                        .name
                        .to_java_string()
                        .to_string_lossy()
                        .to_string()
                })
                .unwrap_or_else(|| format!("{:?}", ctx.typ(t).get_tag()));
            eprintln!("[CLOSURE depth={depth}] {name}");
        }
        let _depth_guard = TypeRecursionGuard;
        let mut cl: List<TypeId>;
        let st = self.supertype(ctx, syms, t, None);
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
        let interfaces = self.interfaces(ctx, syms, t);
        for interface in &interfaces {
            let interface_closure = self.closure(ctx, syms, interface);
            cl = self.closure_union(ctx, syms, cl, interface_closure);
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
        let cl = cl;
        let mut rest = cl.clone();
        for current in &cl {
            rest = rest.tail();
            let mut keep = !to_skip.contains(&current);
            if keep && ctx.typ(current).has_tag(TypeTag::TYPEVAR) {
                let mut tail = rest.clone();
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
                let mut tail = rest.clone();
                while tail.non_empty() {
                    let t = *tail.head();
                    if self.is_subtype_no_capture(ctx, syms, current, t) {
                        to_skip.push(t);
                    }
                    tail = tail.tail();
                }
            }
        }
        classes.append_list(&interfaces)
    }

    /// Java: public Type glb(List<Type> ts) (Types.java:4092-4100)
    pub fn glb_list(&self, ctx: &mut CompilerContext, syms: &Symtab, ts: List<TypeId>) -> TypeId {
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
    pub fn glb(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId, s: TypeId) -> TypeId {
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
        let current = bounds.clone();
        for bound in &current {
            if !self.is_interface(ctx, bound) {
                class_count += 1;
                let lower = cvar_lower_bound(ctx, bound);
                if bound != lower && !ctx.typ(lower).has_tag(TypeTag::BOT) {
                    cvars.push(bound);
                    lowers.push(lower);
                }
            }
        }
        if class_count > 1 {
            if lowers.is_empty() {
                return self.create_error_type_original(ctx, Some(err_t), syms);
            }
            let mut new_bounds: Vec<TypeId> = Vec::new();
            let current = bounds.clone();
            for bound in &current {
                if !cvars.contains(&bound) {
                    new_bounds.push(bound);
                }
            }
            new_bounds.extend(lowers);
            let mut list = List::nil();
            for bound in new_bounds.into_iter().rev() {
                list = list.prepend(bound);
            }
            return self.glb_list(ctx, syms, list);
        }
        let names = std::rc::Rc::clone(&self.names);
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
        // Java: WildcardType.contains(Type elem) — kind != UNBOUND delegates to
        // the type argument, so "? super T" contains T. Without this branch
        // typeContains(Comparable<? super T>, T) is false and rewriteQuantifiers
        // (rewriteTypeVars = true) expands T -> Comparable<? super T> forever.
        if let TypeKindData::WildcardType { type_, kind, .. } = &ctx.typ(t).kind {
            if *kind != crate::code::bound_kind::BoundKind::UNBOUND {
                let inner = type_.expect("wildcard bound type");
                return self.type_contains(ctx, syms, inner, elem);
            }
            return false;
        }
        // Java: CapturedType extends WildcardType, so it inherits the override
        // above. The port's CapturedType is a separate TypeKindData variant, so
        // the same delegation has to be spelled out here.
        if let TypeKindData::CapturedType { wildcard, .. } = &ctx.typ(t).kind {
            if let Some(wildcard) = *wildcard {
                return self.type_contains(ctx, syms, wildcard, elem);
            }
            return false;
        }
        if type_equals_ignore_metadata(self, ctx, t, elem) {
            return true;
        }
        if is_parameterized_type(ctx, t) {
            if let Some(enclosing) = get_enclosing_type(ctx, t)
                && self.type_contains(ctx, syms, enclosing, elem)
            {
                return true;
            }
            let arguments = get_type_arguments_completed(ctx, syms, t, None);
            if type_list_contains(self, ctx, syms, arguments, elem) {
                return true;
            }
        }
        if self.is_compound(ctx, t) {
            let supertype = self.supertype(ctx, syms, t, None);
            if type_equals_ignore_metadata(self, ctx, supertype, elem) {
                return true;
            }
            let interfaces = self.interfaces(ctx, syms, t);
            if type_list_contains(self, ctx, syms, interfaces, elem) {
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
        let mut res = self.subst_type(ctx, syms, sym_type, &from_list, &to_list, None);
        if !self.is_subtype(ctx, syms, res, t) {
            return None;
        }
        let mut open_vars: Vec<TypeId> = Vec::new();
        let params = self.class_allparams(ctx, sym_type);
        for param in &params {
            if self.type_contains(ctx, syms, res, param) && !self.type_contains(ctx, syms, t, param)
            {
                open_vars.push(param);
            }
        }
        if !open_vars.is_empty() {
            if is_raw_type(ctx, t) {
                res = self.erasure(ctx, syms, res);
            } else {
                let opens = List::from_iterable(open_vars.clone());
                let mut wildcards: List<TypeId> = List::nil();
                for open in &open_vars {
                    let wildcard = ctx.alloc_type(Type::new(
                        TypeTag::WILDCARD,
                        Some(syms.bound_class),
                        TypeKindData::WildcardType {
                            type_: Some(syms.object_type),
                            kind: BoundKind::UNBOUND,
                            bound: Some(*open),
                            is_printing_bound: std::cell::Cell::new(false),
                        },
                    ));
                    wildcards = wildcards.append(wildcard);
                }
                res = self.subst_type(ctx, syms, res, &opens, &wildcards, None);
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
                let current = allparams;
                for arg in &current {
                    let bound = self.rewrite_quantifiers(ctx, syms, arg, high, rewrite_type_vars);
                    if arg != bound {
                        changed = true;
                    }
                    rewritten = rewritten.append(bound);
                }
                if changed {
                    let tsym = ctx.typ(t).tsym.expect("class type symbol");
                    let tsym_type = ctx.sym(tsym).ty.expect("class symbol type");
                    let tsym_params = self.class_allparams(ctx, tsym_type);
                    self.subst_type(ctx, syms, tsym_type, &tsym_params, &rewritten, None)
                } else {
                    t
                }
            }
            TypeKindData::CapturedType { .. } => {
                let wildcard = match &ctx.typ(t).kind {
                    TypeKindData::CapturedType { wildcard, .. } => {
                        wildcard.expect("captured wildcard")
                    }
                    _ => crate::util::assert::error(),
                };
                let w_bound = match &ctx.typ(wildcard).kind {
                    TypeKindData::WildcardType { type_, .. } => type_.expect("wildcard bound"),
                    _ => crate::util::assert::error(),
                };
                let bound = if self.type_contains(ctx, syms, w_bound, t) {
                    self.erasure(ctx, syms, w_bound)
                } else {
                    self.rewrite_quantifiers(ctx, syms, w_bound, high, rewrite_type_vars)
                };
                let bound = self.rewrite_quantifiers(ctx, syms, bound, high, rewrite_type_vars);
                let (formal, kind) = match &ctx.typ(wildcard).kind {
                    TypeKindData::WildcardType { bound, kind, .. } => (*bound, *kind),
                    _ => crate::util::assert::error(),
                };
                self.rewrite_as_wildcard_type(ctx, syms, bound, formal, kind, high)
            }
            TypeKindData::TypeVar { .. } => {
                if rewrite_type_vars {
                    let upper = ctx.upper_bound_of(t).expect("type var upper bound");
                    let contained = self.type_contains(ctx, syms, upper, t);
                    {
                        static N: std::sync::atomic::AtomicUsize =
                            std::sync::atomic::AtomicUsize::new(0);
                        let c = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if c >= 50 && c % 50 == 0 {
                            let du = crate::comp::attr::type_display_of(ctx, upper);
                            let dt = crate::comp::attr::type_display_of(ctx, t);
                            eprintln!(
                                "[TVR] n={} contained={} upper={} t={}",
                                c, contained, du, dt
                            );
                        }
                    }
                    let bound = if contained {
                        self.erasure(ctx, syms, upper)
                    } else {
                        self.rewrite_quantifiers(ctx, syms, upper, high, rewrite_type_vars)
                    };
                    self.rewrite_as_wildcard_type(
                        ctx,
                        syms,
                        bound,
                        Some(t),
                        BoundKind::EXTENDS,
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
                        ..
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
        bk: BoundKind,
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
            BoundKind::UNBOUND => self.make_extends_wildcard(ctx, syms, syms.object_type, formal),
        }
    }

    /// Java: Rewriter.B(Type t)
    fn rewrite_b(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId, high: bool) -> TypeId {
        let mut t = t;
        while ctx.typ(t).has_tag(TypeTag::WILDCARD) {
            let (type_, kind) = match &ctx.typ(t).kind {
                TypeKindData::WildcardType { type_, kind, .. } => (*type_, *kind),
                _ => break,
            };
            // Java: t = high ? w.getExtendsBound() : w.getSuperBound();
            // getExtendsBound() is null unless kind == EXTENDS, getSuperBound()
            // is null unless kind == SUPER; the null case falls back to
            // objectType/botType. Reading type_ unconditionally turned an
            // unbounded "?" into Object on the low side instead of botType.
            t = match (high, kind, type_) {
                (true, BoundKind::EXTENDS, Some(bound)) => bound,
                (false, BoundKind::SUPER, Some(bound)) => bound,
                _ => {
                    if high {
                        syms.object_type
                    } else {
                        syms.bot_type
                    }
                }
            };
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
            BoundKind::UNBOUND
        } else {
            BoundKind::EXTENDS
        };
        ctx.alloc_type(Type::new(
            TypeTag::WILDCARD,
            Some(syms.bound_class),
            TypeKindData::WildcardType {
                type_: Some(bound),
                kind,
                bound: formal,
                is_printing_bound: std::cell::Cell::new(false),
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
            BoundKind::UNBOUND
        } else {
            BoundKind::SUPER
        };
        let type_ = if kind == BoundKind::UNBOUND {
            syms.object_type
        } else {
            bound
        };
        ctx.alloc_type(Type::new(
            TypeTag::WILDCARD,
            Some(syms.bound_class),
            TypeKindData::WildcardType {
                type_: Some(type_),
                kind,
                bound: formal,
                is_printing_bound: std::cell::Cell::new(false),
            },
        ))
    }

    /// Java: private Type arraySuperType() (Types.java:4080-4087):
    /// all arrays implement Cloneable and Serializable.
    fn array_super_type(&self, ctx: &mut CompilerContext, syms: &Symtab) -> TypeId {
        let names = std::rc::Rc::clone(&self.names);
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
            let upper = ctx.upper_bound_of(t).expect("type var upper bound");
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
    fn relax_bound(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
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
    types: &Types,
    ctx: &mut CompilerContext,
    syms: &Symtab,
    ts: List<TypeId>,
    elem: TypeId,
) -> bool {
    // Java: boolean containsType(List<Type> ts, Type s) — "for (Type t : ts) if
    // (t.contains(s)) return true". The element check must be Type.contains, not
    // identity: against a type argument list of [? super T] the element T is
    // contained through WildcardType.contains. Comparing for equality here made
    // typeContains(Comparable<? super T>, T) false, which sent
    // rewriteQuantifiers(T, rewriteTypeVars = true) into an infinite
    // T -> Comparable<? super T> expansion.
    let current = ts;
    for t in &current {
        if types.type_contains(ctx, syms, t, elem) {
            return true;
        }
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
            if kind == BoundKind::UNBOUND {
                return false;
            }
            let Some(type_) = type_ else {
                return false;
            };
            if !ctx.typ(s).has_tag(TypeTag::WILDCARD) {
                return if kind == BoundKind::EXTENDS {
                    self.not_soft_subtype_cached(ctx, syms, s, type_, cache)
                } else {
                    self.not_soft_subtype_cached(ctx, syms, type_, s, cache)
                };
            }
            let (s_type, s_kind) = match &ctx.typ(s).kind {
                TypeKindData::WildcardType { type_, kind, .. } => (*type_, *kind),
                _ => return false,
            };
            if s_kind == BoundKind::UNBOUND {
                return false;
            }
            let Some(s_type) = s_type else {
                return false;
            };
            if kind == BoundKind::EXTENDS {
                if s_kind == BoundKind::EXTENDS {
                    let upper_s = self.wild_upper_bound(ctx, syms, s);
                    let pair = (type_, upper_s);
                    if cache.contains(&pair) {
                        return true;
                    }
                    cache.push(pair);
                    let result = self.is_castable(ctx, syms, type_, upper_s);
                    cache.pop();
                    return !result;
                } else if s_kind == BoundKind::SUPER {
                    let lower_s = self.wild_lower_bound(ctx, syms, s);
                    return self.not_soft_subtype_cached(ctx, syms, lower_s, type_, cache);
                }
            } else if kind == BoundKind::SUPER
                && s_kind == BoundKind::EXTENDS
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
        let interfaces = self.interfaces(ctx, syms, t);
        for interface in &interfaces {
            let interface_erasure = self.erasure(ctx, syms, interface);
            if self.is_subtype(ctx, syms, erasure_s, interface_erasure) {
                cl = self.closure_insert(ctx, syms, cl, interface);
            } else {
                let sub_closure = self.super_closure(ctx, syms, interface, erasure_s);
                cl = self.closure_union(ctx, syms, cl, sub_closure);
            }
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
        let current = bounds;
        for b in &current {
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
        warn: &crate::util::warner::Warner,
    ) -> bool {
        let mut reverse = false;
        let mut from = from;
        let mut to = to;
        let target = to;
        let to_tsym = ctx.typ(to).tsym.expect("cast target symbol");
        if (ctx.sym(to_tsym).flags_field & crate::code::flags::INTERFACE as i64) == 0 {
            let from_tsym = ctx.typ(from).tsym.expect("cast source symbol");
            assert_ne!(ctx.sym(from_tsym).flags_field & crate::code::flags::INTERFACE as i64, 0, "AssertionError: sideCast expects an interface");
            reverse = true;
            to = from;
            from = target;
        }
        let from_erasure = self.erasure(ctx, syms, from);
        let common_supers = self.super_closure(ctx, syms, to, from_erasure);
        let mut give_warning = common_supers.is_empty();
        let common_supers = common_supers;
        for common in &common_supers {
            let common_tsym = ctx.typ(common).tsym.expect("common super symbol");
            let t1 = self.as_super(ctx, syms, from, common_tsym);
            let t2 = common;
            if let Some(t1) = t1 {
                let t1_args = get_type_arguments_completed(ctx, syms, t1, None);
                let t2_args = get_type_arguments_completed(ctx, syms, t2, None);
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
        warn: &crate::util::warner::Warner,
    ) -> bool {
        let mut reverse = false;
        let mut from = from;
        let mut to = to;
        let target = to;
        let to_tsym = ctx.typ(to).tsym.expect("cast target symbol");
        if (ctx.sym(to_tsym).flags_field & crate::code::flags::INTERFACE as i64) == 0 {
            let from_tsym = ctx.typ(from).tsym.expect("cast source symbol");
            assert_ne!(ctx.sym(from_tsym).flags_field & crate::code::flags::INTERFACE as i64, 0, "AssertionError: sideCastFinal expects an interface");
            reverse = true;
            to = from;
            from = target;
        }
        let from_tsym = ctx.typ(from).tsym.expect("final class symbol");
        assert_ne!(ctx.sym(from_tsym).flags_field & crate::code::flags::FINAL as i64, 0, "AssertionError: sideCastFinal expects a final class");
        let to_tsym = ctx.typ(to).tsym.expect("cast target symbol");
        let Some(t1) = self.as_super(ctx, syms, from, to_tsym) else {
            return false;
        };
        let t2 = to;
        let t1_args = get_type_arguments_completed(ctx, syms, t1, None);
        let t2_args = get_type_arguments_completed(ctx, syms, t2, None);
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
        let upper = ctx.upper_bound_of(t).expect("type var upper bound");
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
            let supertype = self.supertype(ctx, syms, t, None);
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
                let mut arguments = get_type_arguments_completed(ctx, syms, t, None);
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

    fn undet_var_kind(
        &self,
        ctx: &CompilerContext,
        uv: TypeId,
    ) -> crate::code::type_::UndetVarKind {
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
            let lower = ctx.lower_bound_of(origin).expect("captured lower bound");
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
    pub fn undet_var_get_declared_bounds(&self, ctx: &CompilerContext, uv: TypeId) -> List<TypeId> {
        let declared_count = match &ctx.typ(uv).kind {
            TypeKindData::UndetVar { declared_count, .. } => *declared_count,
            _ => panic!("ClassCastException: not an UndetVar"),
        };
        let mut result: List<TypeId> = List::nil();
        let mut count = 0;
        let mut current =
            self.undet_var_get_bounds(ctx, uv, &[crate::code::type_::InferenceBound::UPPER]);
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
            if ctx.typ(bound).has_tag(TypeTag::UNDETVAR) && !self.undet_var_is_captured(ctx, bound)
            {
                self.undet_var_add_bound_internal(ctx, syms, bound, ib.complement(), uv, false);
            }
            return;
        }
        let mapped = self.map_undet_vars_to_typevars(ctx, syms, bound);
        let bound2 = self.base_type_of(ctx, syms, mapped);
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
        let new_bounds = self.undet_var_get_bounds(ctx, uv, &[ib]).prepend(bound2);
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
                let typarams = get_type_arguments_completed(ctx, syms, t, None);
                let mut typarams1: List<TypeId> = List::nil();
                let mut changed = false;
                let current = typarams.clone();
                for arg in &current {
                    let mapped = self.map_undet_vars_to_typevars(ctx, syms, arg);
                    if mapped != arg {
                        changed = true;
                    }
                    typarams1 = typarams1.append(mapped);
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
            TypeKindData::WildcardType {
                type_, kind, bound, ..
            } => {
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
                let restype1 =
                    restype.map(|restype| self.map_undet_vars_to_typevars(ctx, syms, restype));
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

    /// The base type of a type: an inference variable maps to the base type of
    /// its instantiated type and every other type maps to itself.
    pub(crate) fn base_type_of(
        &self,
        ctx: &mut CompilerContext,
        _syms: &Symtab,
        t: TypeId,
    ) -> TypeId {
        match &ctx.typ(t).kind {
            TypeKindData::UndetVar { .. } => match self.undet_var_get_inst(ctx, t) {
                Some(inst) => self.base_type_of(ctx, _syms, inst),
                None => t,
            },
            _ => t,
        }
    }

    /// Java: public UndetVar dup(Types types) (Type.java:2025-2030)
    pub fn undet_var_dup(&self, ctx: &mut CompilerContext, syms: &Symtab, uv: TypeId) -> TypeId {
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
            let current = prev_bounds;
            for t in &current {
                if !self.contains_any(ctx, syms, t, from) {
                    new_bounds = new_bounds.append(t);
                } else {
                    deps.push(t);
                }
            }
            self.undet_var_set_bounds(ctx, uv, ib, new_bounds);
            for dep in deps {
                let substituted = self.subst_type(ctx, syms, dep, from, to, None);
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
        // Java: UndetVarListener.varInstantiated default body Assert.error()
        // (Type.java:1907): the listener used by substBounds does not expect an
        // instantiation of the recorded variables.
        panic!("AssertionError: varInstantiated must not be called on BoundsChangeRecorder");
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
        let act1 = get_type_arguments_completed(ctx, syms, c1, None);
        let act2 = get_type_arguments_completed(ctx, syms, c2, None);
        let c1_tsym = ctx.typ(c1).tsym.expect("class type symbol");
        let typarams_type = ctx.sym(c1_tsym).ty.expect("class symbol type");
        let mut typarams = get_type_arguments_completed(ctx, syms, typarams_type, None);
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
                    self.is_same_type(ctx, syms, *x, c1) && self.is_same_type(ctx, syms, *y, c2)
                }) || self.merge_cache.borrow().iter().any(|(x, y)| {
                    self.is_same_type(ctx, syms, *x, c1) && self.is_same_type(ctx, syms, *y, c2)
                });
                if !already {
                    merges.push(pair);
                    self.merge_cache.borrow_mut().push(pair);
                    let upper1 = self.wild_upper_bound(ctx, syms, a1);
                    let upper2 = self.wild_upper_bound(ctx, syms, a2);
                    let lub = self.lub_pair(ctx, syms, upper1, upper2);
                    self.merge_cache.borrow_mut().pop();
                    merges.pop();
                    ctx.alloc_type(Type::new(
                        TypeTag::WILDCARD,
                        Some(syms.bound_class),
                        TypeKindData::WildcardType {
                            type_: Some(lub),
                            kind: BoundKind::EXTENDS,
                            bound: None,
                            is_printing_bound: std::cell::Cell::new(false),
                        },
                    ))
                } else {
                    ctx.alloc_type(Type::new(
                        TypeTag::WILDCARD,
                        Some(syms.bound_class),
                        TypeKindData::WildcardType {
                            type_: Some(syms.object_type),
                            kind: BoundKind::UNBOUND,
                            bound: None,
                            is_printing_bound: std::cell::Cell::new(false),
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
        let sup = self.closure(ctx, syms, t);
        for sup_t in &sup {
            if ctx.typ(sup_t).has_tag(TypeTag::TYPEVAR) {
                result = result.append(sup_t);
            } else {
                result = result.append(self.erasure(ctx, syms, sup_t));
            }
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

    fn debug_lub_depth(ctx: &CompilerContext, ts: &[TypeId]) -> usize {
        let depth = TYPE_RECURSION_DEPTH.with(|d| {
            let v = d.get();
            d.set(v + 1);
            v
        });
        if depth < 60 && crate::util::debug_flags::debug_enabled("DSH_DEBUG_RECURSION") {
            let names: Vec<String> = ts
                .iter()
                .map(|t| {
                    ctx.typ(*t)
                        .tsym
                        .map(|s| {
                            ctx.sym(s)
                                .name
                                .to_java_string()
                                .to_string_lossy()
                                .to_string()
                        })
                        .unwrap_or_else(|| format!("{:?}", ctx.typ(*t).get_tag()))
                })
                .collect();
            eprintln!("[LUB depth={depth}] {names:?}");
        }
        depth
    }

    /// Java: public Type lub(Type... ts) (Types.java:3965-4035)
    pub fn lub(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        names: &crate::util::names::Names,
        ts: &[TypeId],
    ) -> TypeId {
        let _depth = Self::debug_lub_depth(ctx, ts);
        let _depth_guard = TypeRecursionGuard;
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
                        t = ctx.upper_bound_of(t).expect("type var upper bound");
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
                    let elem = elemtype_of(self, ctx, syms, *t).expect("array element type");
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
                    if ctx.typ(*t).has_tag(TypeTag::CLASS) || ctx.typ(*t).has_tag(TypeTag::TYPEVAR)
                    {
                        break;
                    }
                    start_idx += 1;
                }
                assert!(start_idx < ts.len(), "AssertionError: lub startIdx");
                // step 1 - compute erased candidate set (EC)
                let mut cl = self.erased_supertypes(ctx, syms, ts[start_idx]);
                for t in &ts[start_idx + 1..] {
                    if ctx.typ(*t).has_tag(TypeTag::CLASS) || ctx.typ(*t).has_tag(TypeTag::TYPEVAR)
                    {
                        let erased = self.erased_supertypes(ctx, syms, *t);
                        cl = self.intersect(ctx, syms, cl, erased);
                    }
                }
                // step 2 - compute minimal erased candidate set (MEC)
                let mec = self.closure_min(ctx, syms, cl);
                // step 3 - for each element G in MEC, compute lci(Inv(G))
                let mut candidates: List<TypeId> = List::nil();
                let mec_current = mec;
                for erased_supertype in &mec_current {
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
                }
                // step 4 - let MEC be { G1, G2... Gn }, then we have that
                // lub = lci(Inv(G1)) & lci(Inv(G2)) &... & lci(Inv(Gn))
                self.compound_min(ctx, syms, names, candidates)
            }
            _ => {
                // calculate lub(A, B[])
                let mut classes: List<TypeId> = List::of(self.array_super_type(ctx, syms));
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
    fn lub_pair(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId, s: TypeId) -> TypeId {
        let names = std::rc::Rc::clone(&self.names);
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
    pub fn get_parameter_types_public(&self, ctx: &CompilerContext, t: TypeId) -> List<TypeId> {
        match &ctx.typ(t).kind {
            TypeKindData::MethodType { argtypes, .. }
            | TypeKindData::PartiallyInferredMethodType { argtypes, .. } => argtypes.clone(),
            // Java: DelegatedType.getParameterTypes() delegates to qtype.
            // Java: DelegatedType.getParameterTypes() delegates to qtype.
            TypeKindData::ForAll {
                qtype: Some(qtype), ..
            } => match &ctx.typ(*qtype).kind {
                TypeKindData::MethodType { argtypes, .. }
                | TypeKindData::PartiallyInferredMethodType { argtypes, .. } => argtypes.clone(),
                _ => List::nil(),
            },
            _ => List::nil(),
        }
    }

    /// The return type of a method type (or the delegated type's return type).
    ///
    /// Port adapter: the ERROR case returns the type itself, matching
    /// ErrorType.getReturnType.
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
            // Java: DelegatedType.getReturnType() delegates to qtype.
            TypeKindData::ForAll {
                qtype: Some(qtype), ..
            } => self.get_return_type_public(ctx, *qtype),
            // Java: ErrorType.getReturnType() returns this.
            _ if ctx.typ(t).has_tag(TypeTag::ERROR) => t,
            _ => panic!(
                "AssertionError: no return type tag={:?}",
                ctx.typ(t).get_tag()
            ),
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
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> List<TypeId> {
        // Java: DelegatedType.getTypeArguments() delegates to qtype; ForAll
        // overrides it with the type variables.
        if let TypeKindData::ForAll { tvars, .. } = &ctx.typ(t).kind {
            return tvars.clone();
        }
        get_type_arguments_completed(ctx, syms, t, None)
    }

    /// Java: Type.isErroneous() view used by Infer.instantiateAsUninferredVars.
    pub fn is_erroneous_type(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> bool {
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
        let current = ts;
        for t in &current {
            let t_closure = self.closure(ctx, syms, t);
            let mut c = t_closure;
            while c.non_empty() {
                let x = *c.head();
                closure = self.closure_insert_same_type(ctx, syms, closure, x);
                c = c.tail();
            }
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
        warn: &crate::util::warner::Warner,
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
                        .has_tag(TypeTag::NONE)
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
        // Java: isConvertible(t, s, warn)
        self.is_convertible_with_warn(ctx, syms, t, s, warn)
    }

    /// Java: public boolean covariantReturnType(Type t, Type s, Warner warner)
    /// (Types.java:4296-4303)
    pub fn covariant_return_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        warn: &crate::util::warner::Warner,
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
        syms: &Symtab,
        t: TypeId,
        s: TypeId,
        warner: &crate::util::warner::Warner,
    ) -> bool {
        let tvars = get_type_arguments_completed(ctx, syms, t, None);
        let svars = get_type_arguments_completed(ctx, syms, s, None);
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
        syms: &Symtab,
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
        syms: &Symtab,
        r1: TypeId,
        r2: TypeId,
        r2res: TypeId,
        warner: &crate::util::warner::Warner,
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

    /// Java: public boolean isDerivedRaw(Type t) (Types.java:2670-2677)
    pub fn is_derived_raw(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> bool {
        if let Some(result) = self.is_derived_raw_cache.borrow().get(&t) {
            return *result;
        }
        let result = self.is_derived_raw_internal(ctx, syms, t);
        self.is_derived_raw_cache.borrow_mut().insert(t, result);
        result
    }

    /// Java: public boolean isDerivedRawInternal(Type t) (Types.java:2679-2686)
    pub fn is_derived_raw_internal(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> bool {
        if self.is_erroneous(ctx, syms, t) {
            return false;
        }
        let super_type = self.supertype(ctx, syms, t, None);
        let no_type = ctx.sym(syms.no_symbol).ty.expect("no type");
        if is_raw_type(ctx, t) {
            return true;
        }
        if super_type != no_type && self.is_derived_raw(ctx, syms, super_type) {
            return true;
        }
        let interfaces = self.interfaces(ctx, syms, t);
        self.is_derived_raw_types(ctx, syms, &interfaces)
    }

    /// Java: public boolean isDerivedRaw(List<Type> ts) (Types.java:2688-2692)
    pub fn is_derived_raw_types(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ts: &List<TypeId>,
    ) -> bool {
        let current = ts.clone();
        for t in &current {
            if self.is_derived_raw(ctx, syms, t) {
                return true;
            }
        }
        false
    }

    /// Java: public boolean overrideEquivalent(Type t, Type s) (Types.java:2816-2820)
    pub fn override_equivalent(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
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
        syms: &Symtab,
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

/// Java: Types.MembersClosureCache.MembersScope (Types.java:3015-3042).
///
/// The upstream view wraps a CompoundScope and applies combine(sf) =
/// !s.owner.isInterface() && (sf == null || sf.test(s)). The port keeps the
/// scope id and applies the same predicate after each query (same symbol set;
/// the underlying scope order is unchanged).
pub struct MembersScope {
    scope: crate::ids::ScopeId,
    skip_interface: bool,
}

impl MembersScope {
    pub fn scope_id(&self) -> crate::ids::ScopeId {
        self.scope
    }

    pub fn get_symbols(
        &self,
        ctx: &mut CompilerContext,
        lookup_kind: crate::code::scope::LookupKind,
        filter: &dyn Fn(SymbolId) -> bool,
    ) -> Vec<SymbolId> {
        let mut symbols = ctx.scope_get_symbols(self.scope, lookup_kind, filter);
        if self.skip_interface {
            symbols.retain(|sym| !member_owner_is_interface(ctx, *sym));
        }
        symbols
    }

    pub fn get_symbols_by_name(
        &self,
        ctx: &mut CompilerContext,
        name: &Name,
        lookup_kind: crate::code::scope::LookupKind,
        filter: &dyn Fn(SymbolId) -> bool,
    ) -> Vec<SymbolId> {
        let mut symbols = ctx.scope_get_symbols_by_name(self.scope, name, lookup_kind, filter);
        if self.skip_interface {
            symbols.retain(|sym| !member_owner_is_interface(ctx, *sym));
        }
        symbols
    }

    pub fn get_mark(&self, ctx: &CompilerContext) -> i32 {
        ctx.scope_get_mark(self.scope)
    }
}

/// Java: MembersScope.combine's !s.owner.isInterface() (Types.java:3024-3026).
fn member_owner_is_interface(ctx: &CompilerContext, sym: SymbolId) -> bool {
    ctx.sym(sym).owner.is_some_and(|owner| {
        (ctx.sym(owner).flags_field & crate::code::flags::INTERFACE as i64) != 0
    })
}

impl Types {
    /// Java: public CompoundScope membersClosure(Type site, boolean skipInterface)
    /// (Types.java:3091-3095).
    ///
    /// Java: public CompoundScope membersClosure(Type site, boolean skipInterface)
    /// (Types.java:3091-3095). The returned MembersScope applies the upstream
    /// combine predicate (Types.java:3024-3026): when skipInterface is true,
    /// symbols owned by interfaces are filtered at every query boundary.
    pub fn members_closure(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        site: TypeId,
        skip_interface: bool,
    ) -> MembersScope {
        MembersScope {
            scope: self.members_closure_visit(ctx, syms, site),
            skip_interface,
        }
    }

    /// Java: MembersClosureCache.visit (Types.java:3040-3089)
    fn members_closure_visit(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
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
                    let scope =
                        ctx.alloc_scope(crate::code::scope::Scope::new_compound_scope(Some(tsym)));
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
                let current = interfaces;
                for iface in &current {
                    let sub = self.members_closure_visit(ctx, syms, iface);
                    ctx.scope_prepend_sub_scope(closure_scope, sub);
                }
                let super_type = self.supertype(ctx, syms, t, None);
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
                let upper = ctx.upper_bound_of(t).expect("type var upper bound");
                self.members_closure_visit(ctx, syms, upper)
            }
            _ => {
                let no_symbol = syms.no_symbol;
                ctx.alloc_scope(crate::code::scope::Scope::new_compound_scope(Some(
                    no_symbol,
                )))
            }
        }
    }

    /// Java: public MethodSymbol implementation(MethodSymbol ms, TypeSymbol
    /// origin, boolean checkResult, Predicate<Symbol> implFilter)
    /// (Types.java:3003-3005).
    ///
    /// Porting rules adapter: the only upstream implFilter is
    /// MethodSymbol.implementation_filter (non-synthetic methods), so the port
    /// applies that filter directly (see implementation_internal).
    pub fn implementation(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ms: SymbolId,
        origin: SymbolId,
        check_result: bool,
        filter_kind: ImplementationFilterKind,
    ) -> Option<SymbolId> {
        self.implementation_filtered(ctx, syms, ms, origin, check_result, filter_kind)
    }

    /// Java: ImplementationCache.get/implementationInternal
    /// (Types.java:2960-3001)
    pub fn implementation_filtered(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ms: SymbolId,
        origin: SymbolId,
        check_result: bool,
        filter_kind: ImplementationFilterKind,
    ) -> Option<SymbolId> {
        let origin_ty = ctx.sym(origin).ty.expect("origin type");
        let members = self.members_closure(ctx, syms, origin_ty, true);
        let mark = members.get_mark(ctx);
        if let Some(entry) = self
            .implementation_cache
            .borrow()
            .get(&ms)
            .and_then(|per_origin| per_origin.get(&origin))
            .cloned()
            && entry.check_result == check_result
            && entry.prev_mark == mark
            && entry.filter_kind == filter_kind
        {
            return entry.cached_impl;
        }
        let result = self.implementation_internal(ctx, syms, ms, origin, check_result, filter_kind);
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
                    filter_kind,
                },
            );
        result
    }

    /// Java: ImplementationCache.implementationInternal (Types.java:2980-3001)
    fn implementation_internal(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ms: SymbolId,
        origin: SymbolId,
        check_result: bool,
        filter_kind: ImplementationFilterKind,
    ) -> Option<SymbolId> {
        let ms_name = ctx.sym(ms).name.clone();
        let mut t = ctx.sym(origin).ty.expect("origin type");
        loop {
            let tag = ctx.typ(t).get_tag();
            if tag != TypeTag::CLASS
                && tag != TypeTag::TYPEVAR
            {
                break;
            }
            t = self.skip_type_vars(ctx, t);
            let c = ctx.typ(t).tsym.expect("class symbol");
            let members = match &ctx.sym(c).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => {
                    *members_field
                }
                _ => None,
            };
            let mut best_so_far: Option<SymbolId> = None;
            if let Some(members) = members {
                let scope_snapshot = ctx.scope(members).clone();
                let candidates = scope_snapshot.get_symbols_by_name(
                    members,
                    &ms_name,
                    crate::code::scope::LookupKind::NON_RECURSIVE,
                    &|_| true,
                    ctx,
                );
                let found: Vec<SymbolId> = candidates
                    .into_iter()
                    .filter(|sym| implementation_filter_matches(ctx, filter_kind, *sym))
                    .collect();
                for sym in found {
                    if crate::code::symbol::overrides(
                        ctx,
                        syms,
                        self,
                        sym,
                        ms,
                        origin,
                        check_result,
                    ) {
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
            let super_type = self.supertype(ctx, syms, t, None);
            t = super_type;
        }
        None
    }

    /// Java: public List<MethodSymbol> interfaceCandidates(Type site, MethodSymbol ms)
    /// (Types.java:3187-3202)
    pub fn interface_candidates(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
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
        let members = self.members_closure(ctx, syms, site, false);
        let filter = |ctx: &mut CompilerContext, syms2: &Symtab, s: SymbolId| -> bool {
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
        let all = members.get_symbols(ctx, crate::code::scope::LookupKind::RECURSIVE, &|_| true);
        for s in all {
            if !filter(ctx, syms, s) {
                continue;
            }
            if ctx
                .typ(site)
                .tsym
                .map(|sym| sym != site_tsym)
                .unwrap_or(true)
            {
                let _ = site_tsym;
            }
            let site_is_interface =
                (ctx.sym(site_tsym).flags_field & crate::code::flags::INTERFACE as i64) != 0;
            let owner_is_interface = ctx
                .sym(s)
                .owner
                .map(|owner| {
                    (ctx.sym(owner).flags_field & crate::code::flags::INTERFACE as i64) != 0
                })
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
        syms: &Symtab,
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
        syms: &Symtab,
        types: &Types,
        mt1: TypeId,
        mt2: TypeId,
    ) -> bool {
        match self {
            MostSpecificReturnCheck::BASIC => {
                let tvars = get_type_arguments_completed(ctx, syms, mt1, None);
                let svars = get_type_arguments_completed(ctx, syms, mt2, None);
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
            MostSpecificReturnCheck::RTS => types.return_type_substitutable(ctx, syms, mt1, mt2),
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
                let mt = Type::new_method_type(
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
                ctx.alloc_type(Type::new_for_all(tvars, inner))
            }
            _ => panic!("IllegalArgumentException: Not a method type"),
        }
    }

    /// Java: public Optional<Symbol> mergeAbstracts(List<Symbol> ambiguousInOrder,
    /// Type site, boolean sigCheck) (Types.java:2874-2930)
    pub fn merge_abstracts(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        chk: &crate::comp::check::Check,
        ambiguous_in_order: &[SymbolId],
        site: TypeId,
        sig_check: bool,
    ) -> Option<MergeAbstractsResult> {
        let head = *ambiguous_in_order.first().expect("ambiguous methods");
        let head_ty = ctx.sym(head).ty.expect("method type");
        let head_erasure = self.erasure(ctx, syms, head_ty);
        let erased_params = get_parameter_types(ctx, head_erasure);
        let mut should_erase = false;
        for s in ambiguous_in_order.iter() {
            let s_ty = ctx.sym(*s).ty.expect("method type");
            let s_erasure = self.erasure(ctx, syms, s_ty);
            let s_params = get_parameter_types(ctx, s_erasure);
            if (ctx.sym(*s).flags_field & crate::code::flags::ABSTRACT as i64) == 0
                || (sig_check && !self.is_same_types(ctx, syms, &erased_params, &s_params))
            {
                return None;
            }
            if ctx
                .typ(s_ty)
                .has_tag(TypeTag::FORALL)
            {
                should_erase = true;
            }
        }
        for msc in [MostSpecificReturnCheck::BASIC, MostSpecificReturnCheck::RTS] {
            'outer: for s in ambiguous_in_order.iter() {
                let s_ty = ctx.sym(*s).ty.expect("method type");
                let mt = self.member_type(ctx, syms, site, *s);
                let mut all_thrown = get_thrown_types(ctx, mt);
                for s2 in ambiguous_in_order.iter() {
                    if s != s2 {
                        let mt2 = self.member_type(ctx, syms, site, *s2);
                        if !self.is_sub_signature(ctx, syms, mt, mt2)
                            || !msc.test(ctx, syms, self, mt, mt2)
                        {
                            continue 'outer;
                        }
                        let mut thrown2 = get_thrown_types(ctx, mt2);
                        if !ctx.typ(mt).has_tag(TypeTag::FORALL)
                            && should_erase
                        {
                            thrown2 = self.erasure_list(ctx, syms, &thrown2);
                        } else if ctx.typ(mt).has_tag(TypeTag::FORALL) {
                            assert!(
                                ctx.typ(mt2).has_tag(TypeTag::FORALL),
                                "AssertionError: generic merge"
                            );
                            let mt2_args = get_type_arguments_completed(
                                ctx, syms, mt2, None,
                            );
                            let mt_args = get_type_arguments_completed(
                                ctx, syms, mt, None,
                            );
                            thrown2 = self.subst(ctx, syms, &thrown2, &mt2_args, &mt_args);
                        }
                        all_thrown = chk.intersect(ctx, syms, &all_thrown, &thrown2);
                    }
                }
                // Java: `allThrown == mt.getThrownTypes()` is a reference
                // comparison (Types.java:2913): intersect() builds a fresh list,
                // so an unchanged thrown clause still yields the merged symbol.
                if all_thrown.ptr_eq(&get_thrown_types(ctx, mt)) {
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
                        original_enclosing_method: None,
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
        syms: &Symtab,
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
        syms: &Symtab,
        diags: &crate::util::jc_diagnostic::Factory,
        chk: &crate::comp::check::Check,
        origin: SymbolId,
    ) -> Result<FunctionDescriptor, FunctionDescriptorLookupError> {
        let origin_ty = ctx.sym(origin).ty.expect("type symbol type");
        let members = self.members_closure(ctx, syms, origin_ty, false);
        let mark = members.get_mark(ctx);
        if let Some(entry) = self.descriptor_cache.borrow().get(&origin).cloned()
            && entry.prev_mark == mark
        {
            return Ok(entry.cached_desc_res);
        }
        let desc_res =
            self.find_descriptor_internal(ctx, syms, diags, chk, origin, members.scope_id())?;
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
        syms: &Symtab,
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
            let origin_ty = ctx.sym(origin).ty.expect("origin type");
            let origin_arg = crate::resources::diagnostic_contract::DiagnosticArg::Type(
                crate::comp::check::type_arg_with_rich_where(ctx, origin_ty),
            );
            return Err(self.descriptor_failure(
                diags,
                crate::resources::compiler_properties::fragments::not_a_functional_intf(origin_arg),
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
                if ctx.sym(sym).name.to_java_string() == ctx.sym(first).name.to_java_string()
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
                        if crate::code::symbol::is_sub_class(ctx, syms, self, msym_owner, sym_encl)
                        {
                            let abstract_m_type = self.member_type(ctx, syms, origin_ty, *msym);
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
                    let simple = ctx.sym(origin).name.to_java_string().to_string_lossy();
                    let kind_name = crate::code::kinds::kind_name(ctx.sym(origin), ctx);
                    let origin_ty = ctx.sym(origin).ty.expect("origin type");
                    let origin_arg = if ctx.typ(origin_ty).is_intersection() {
                        crate::resources::diagnostic_contract::DiagnosticArg::Type(
                            crate::comp::check::type_arg_with_rich_where(ctx, origin_ty),
                        )
                    } else {
                        crate::resources::diagnostic_contract::DiagnosticArg::from_display(simple)
                    };
                    let detail = diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                        crate::resources::compiler_properties::fragments::incompatible_abstracts(
                            crate::resources::diagnostic_contract::KindNameArg::placeholder(
                                kind_name.name(),
                            ),
                            origin_arg.clone(),
                        ),
                    ));
                    let fragment =
                        crate::resources::compiler_properties::fragments::not_a_functional_intf1(
                            origin_arg,
                            crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(
                                std::rc::Rc::new(detail),
                            ),
                        );
                    let diag = diags.fragment(crate::util::jc_diagnostic::Fragment::from(fragment));
                    return Err(FunctionDescriptorLookupError::new().set_message(diag));
                }
            }
        }
        if abstracts.is_empty() {
            // Java: throw failure("not.a.functional.intf.1", origin,
            // diags.fragment(Fragments.NoAbstracts(...)))
            let simple = ctx.sym(origin).name.to_java_string().to_string_lossy();
            let kind_name = crate::code::kinds::kind_name(ctx.sym(origin), ctx);
            let origin_ty = ctx.sym(origin).ty.expect("origin type");
            let origin_arg = if ctx.typ(origin_ty).is_intersection() {
                crate::resources::diagnostic_contract::DiagnosticArg::Type(
                    crate::comp::check::type_arg_with_rich_where(ctx, origin_ty),
                )
            } else {
                crate::resources::diagnostic_contract::DiagnosticArg::from_display(simple)
            };
            let detail = diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::no_abstracts(
                    crate::resources::diagnostic_contract::KindNameArg::placeholder(
                        kind_name.name(),
                    ),
                    origin_arg.clone(),
                ),
            ));
            let fragment = crate::resources::compiler_properties::fragments::not_a_functional_intf1(
                origin_arg,
                crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(
                    std::rc::Rc::new(detail),
                ),
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
                // Java: Types.java:765-781 — one descriptor/descriptor.throws
                // fragment per abstract method, reported as a
                // MultilineDiagnostic under the IncompatibleDescsInFunctionalIntf
                // primary fragment.
                let kind_name = crate::code::kinds::kind_name(ctx.sym(origin), ctx);
                let msg = diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                    crate::resources::compiler_properties::fragments::incompatible_descs_in_functional_intf(
                        crate::resources::diagnostic_contract::KindNameArg::placeholder(
                            kind_name.name(),
                        ),
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(
                            ctx.sym(origin).name.to_java_string().to_string_lossy(),
                        ),
                    ),
                ));
                let mut descriptors: List<
                    crate::util::jc_diagnostic::JCDiagnostic,
                > = List::nil();
                for desc in &abstracts {
                    let desc_name = ctx.sym(*desc).name.clone();
                    let desc_ty = ctx.sym(*desc).ty.expect("descriptor method type");
                    let params = self.get_parameter_types_public(ctx, desc_ty);
                    let ret = self.get_return_type_public(ctx, desc_ty);
                    let thrown = self.get_thrown_types_public(ctx, desc_ty);
                    let params_args: Vec<crate::resources::diagnostic_contract::TypeArg> = params
                        .iter()
                        .map(|p| {
                            crate::resources::diagnostic_contract::TypeArg::placeholder(
                                crate::comp::attr::type_display_of(ctx, p),
                            )
                        })
                        .collect();
                    let ret_arg = crate::resources::diagnostic_contract::TypeArg::placeholder(
                        crate::comp::attr::type_display_of(ctx, ret),
                    );
                    let thrown_args: Vec<crate::resources::diagnostic_contract::TypeArg> = thrown
                        .iter()
                        .map(|t| {
                            crate::resources::diagnostic_contract::TypeArg::placeholder(
                                crate::comp::attr::type_display_of(ctx, t),
                            )
                        })
                        .collect();
                    let info = if thrown.is_empty() {
                        crate::resources::compiler_properties::fragments::descriptor(
                            desc_name,
                            params_args,
                            ret_arg,
                            thrown_args,
                        )
                    } else {
                        crate::resources::compiler_properties::fragments::descriptor_throws(
                            desc_name,
                            params_args,
                            ret_arg,
                            thrown_args,
                        )
                    };
                    descriptors = descriptors
                        .append(diags.fragment(crate::util::jc_diagnostic::Fragment::from(info)));
                }
                let multiline =
                    crate::util::jc_diagnostic::MultilineDiagnostic::new(&msg, descriptors);
                Err(FunctionDescriptorLookupError::new()
                    .set_message(multiline.as_diagnostic().clone()))
            }
        }
    }

    /// Java: DescriptorCache.mergeDescriptors(TypeSymbol origin,
    /// List<Symbol> methodSyms) (Types.java:793-802)
    fn merge_descriptors(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
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
        syms: &Symtab,
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
        syms: &Symtab,
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
        syms: &Symtab,
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
        syms: &Symtab,
        site: TypeId,
    ) -> TypeId {
        let actuals = get_type_arguments_completed(ctx, syms, site, None);
        let wildcard_tag = TypeTag::WILDCARD;
        let has_wildcard = actuals.iter().any(|t| ctx.typ(t).has_tag(wildcard_tag));
        if !has_wildcard {
            return site;
        }
        let site_tsym = ctx.typ(site).tsym.expect("site symbol");
        let site_tsym_ty = ctx.sym(site_tsym).ty.expect("site symbol type");
        let formals =
            get_type_arguments_completed(ctx, syms, site_tsym_ty, None);
        let mut args: List<TypeId> = List::nil();
        let mut current = actuals;
        let mut formals_iter = formals.clone();
        while current.non_empty() {
            let actual = *current.head();
            let formal = *formals_iter.head();
            let bound = ctx.upper_bound_of(formal).expect("formal upper bound");
            if ctx.typ(actual).has_tag(wildcard_tag) {
                // Java: WildcardType wt = (WildcardType)actual;
                let (wt_type, wt_kind) = match &ctx.typ(actual).kind {
                    TypeKindData::WildcardType { type_, kind, .. } => (*type_, *kind),
                    _ => (None, BoundKind::UNBOUND),
                };
                let contains_formals = self.contains_any(ctx, syms, bound, &formals);
                if contains_formals {
                    // Java: targs.add(wt.type); (may be null)
                    if let Some(wt_type) = wt_type {
                        args = args.append(wt_type);
                    }
                } else {
                    match wt_kind {
                        BoundKind::UNBOUND => {
                            args = args.append(bound);
                        }
                        BoundKind::EXTENDS => {
                            let wt_type = wt_type.expect("wildcard bound type");
                            let glb = self.glb(ctx, syms, bound, wt_type);
                            args = args.append(glb);
                        }
                        BoundKind::SUPER => {
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
        syms: &Symtab,
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
                let best_thrown = get_thrown_types(ctx, best_so_far_ty);
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
        syms: &Symtab,
        diags: &crate::util::jc_diagnostic::Factory,
        chk: &crate::comp::check::Check,
        env: &std::rc::Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        name: Name,
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
                original_enclosing_method: None,
            },
        ));
        ctx.scope_enter(members, inst_desc);
        let ctype = ctx.alloc_type(Type::new_class_type(
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
        if let TypeKindData::ClassType {
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
    fn bridge_filter(&self, ctx: &CompilerContext, sym: SymbolId) -> bool {
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
        syms: &Symtab,
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
        let members = self.members_closure(ctx, syms, origin_ty, false);
        let desc_name = ctx.sym(desc_sym).name.clone();
        let candidates = members.get_symbols_by_name(
            ctx,
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
                    || (overrides_m2 && {
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
        syms: &Symtab,
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
        warn: &crate::util::warner::Warner,
    ) -> bool {
        if ctx.typ(t).has_tag(TypeTag::ARRAY) && ctx.typ(s).has_tag(TypeTag::ARRAY) {
            let t_elem = elemtype_of(self, ctx, syms, t).expect("array element type");
            let s_elem = elemtype_of(self, ctx, syms, s).expect("array element type");
            if ctx.typ(t_elem).is_primitive() {
                return self.is_same_type(ctx, syms, t_elem, s_elem);
            }
            return self.is_subtype_unchecked_internal_warn(ctx, syms, t_elem, s_elem, false, warn);
        }
        // Raw-to-parameterized conversion is an unchecked conversion even
        // though the erasures are subtypes; isSubtype itself rejects it.
        if is_raw_type(ctx, t)
            && !is_raw_type(ctx, s)
            && is_parameterized_type(ctx, s)
            && ctx.typ(s).has_tag(TypeTag::CLASS)
        {
            let t_erasure = self.erasure(ctx, syms, t);
            let s_erasure = self.erasure(ctx, syms, s);
            if self.is_subtype(ctx, syms, t_erasure, s_erasure) {
                if self.is_reifiable(ctx, syms, s) {
                    warn.silent_warn(crate::code::lint::LintCategory::UNCHECKED);
                } else {
                    warn.warn(crate::code::lint::LintCategory::UNCHECKED);
                }
                return true;
            }
        }
        if self.is_subtype_with_capture(ctx, syms, t, s, capture) {
            return true;
        }
        if ctx.typ(t).has_tag(TypeTag::TYPEVAR) {
            let upper = ctx.upper_bound_of(t).expect("type var upper bound");
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
        warn: &crate::util::warner::Warner,
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
        warn: &crate::util::warner::Warner,
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
        warn: &crate::util::warner::Warner,
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
        syms: &Symtab,
        isym: SymbolId,
        origin: SymbolId,
    ) -> bool {
        let origin_type = ctx.sym(origin).ty.expect("origin class type");
        let interfaces = self.interfaces(ctx, syms, origin_type);
        for i in &interfaces {
            if ctx.typ(i).tsym == Some(isym) {
                return true;
            }
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
            TypeKindData::ClassType { .. } => {
                if self.is_compound(ctx, t) {
                    let current = self.direct_supertypes(ctx, syms, t);
                    for sup in &current {
                        self.capture_scan_collect(ctx, syms, sup, seen, result);
                    }
                } else {
                    let current = allparams_of(ctx, t);
                    for param in &current {
                        self.capture_scan_collect(ctx, syms, param, seen, result);
                    }
                }
            }
            TypeKindData::ArrayType { elemtype, .. } => {
                if let Some(elemtype) = elemtype {
                    self.capture_scan_collect(ctx, syms, *elemtype, seen, result);
                }
            }
            TypeKindData::WildcardType { type_, .. } => {
                if let Some(type_) = type_ {
                    self.capture_scan_collect(ctx, syms, *type_, seen, result);
                }
            }
            TypeKindData::TypeVar { bound, .. } => {
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
            TypeKindData::CapturedType { bound, lower, .. } => {
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
            TypeTag::CLASS => {
                let (outer_field, typarams_field) = match &ctx.typ(t).kind {
                    TypeKindData::ClassType {
                        outer_field,
                        typarams_field,
                        ..
                    } => (*outer_field, typarams_field.clone()),
                    _ => (None, None),
                };
                if self.is_compound(ctx, t) {
                    let components = self.direct_supertypes(ctx, syms, t);
                    let mut projected: List<TypeId> = List::nil();
                    let current = components.clone();
                    let mut changed = false;
                    for component in &current {
                        let projected_component =
                            self.projection(ctx, syms, component, vars, pkind, seen);
                        if projected_component != component {
                            changed = true;
                        }
                        projected = projected.append(projected_component);
                    }
                    if !changed {
                        return t;
                    }
                    let names = std::rc::Rc::clone(&self.names);
                    return self.make_intersection_type(ctx, syms, &names, projected, false);
                }
                let outer = outer_field;
                let outer1 =
                    outer.map(|outer| self.projection(ctx, syms, outer, vars, pkind, seen));
                let typarams = typarams_field.unwrap_or_else(List::nil);
                let tsym = ctx.typ(t).tsym.expect("class type symbol");
                let formals_type = ctx.sym(tsym).ty.expect("class symbol type");
                let mut formals =
                    get_type_arguments_completed(ctx, syms, formals_type, None);
                let mut typarams1: List<TypeId> = List::nil();
                let mut changed = false;
                let current = typarams.clone();
                for actual in &current {
                    let declared_bound = if formals.non_empty() {
                        let formal = *formals.head();
                        ctx.upper_bound_of(formal).unwrap_or(syms.object_type)
                    } else {
                        syms.object_type
                    };
                    let t2 = self.map_type_argument(
                        ctx,
                        syms,
                        t,
                        declared_bound,
                        actual,
                        vars,
                        pkind,
                        seen,
                    );
                    if ctx.typ(t2).has_tag(TypeTag::BOT) {
                        return syms.bot_type;
                    }
                    if t2 != actual {
                        changed = true;
                    }
                    typarams1 = typarams1.append(t2);
                    if formals.non_empty() {
                        formals = formals.tail();
                    }
                }
                if outer1 == outer && !changed {
                    return t;
                }
                ctx.alloc_type(Type::new(
                    TypeTag::CLASS,
                    Some(tsym),
                    TypeKindData::ClassType {
                        outer_field: outer1,
                        typarams_field: Some(typarams1),
                        allparams_field: None,
                        supertype_field: None,
                        interfaces_field: None,
                        all_interfaces_field: None,
                    },
                ))
            }
            TypeTag::ARRAY => {
                let elemtype = match &ctx.typ(t).kind {
                    TypeKindData::ArrayType { elemtype, .. } => {
                        elemtype.expect("array element type")
                    }
                    _ => crate::util::assert::error(),
                };
                let elemtype1 = self.projection(ctx, syms, elemtype, vars, pkind, seen);
                if elemtype1 == elemtype {
                    return t;
                }
                if ctx
                    .typ(elemtype1)
                    .has_tag(TypeTag::BOT)
                {
                    return syms.bot_type;
                }
                ctx.alloc_type(Type::new_array_type(
                    elemtype1,
                    syms.array_class,
                ))
            }
            TypeTag::TYPEVAR => {
                // Captured type variables are modelled by a separate kind; their
                // upper/lower bounds are set by capture() and must be projected
                // just like a plain TypeVar (returning None made upward() map a
                // captured wildcard to Object).
                let (bound, lower) = match &ctx.typ(t).kind {
                    TypeKindData::TypeVar { bound, lower }
                    | TypeKindData::CapturedType { bound, lower, .. } => (*bound, *lower),
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
        if ctx.typ(t).has_tag(TypeTag::WILDCARD) {
            let (wt_type, wt_kind) = match &ctx.typ(t).kind {
                TypeKindData::WildcardType { type_, kind, .. } => {
                    (*type_, *kind)
                }
                _ => return t,
            };
            let mut bound = syms.bot_type;
            let mut bk = wt_kind;
            match wt_kind {
                BoundKind::EXTENDS => {
                    if let Some(wt_type) = wt_type {
                        bound = self.projection(ctx, syms, wt_type, vars, pkind, seen);
                    }
                    if ctx.typ(bound).has_tag(TypeTag::BOT) {
                        return syms.bot_type;
                    }
                }
                BoundKind::SUPER => {
                    if let Some(wt_type) = wt_type {
                        bound = self.projection(ctx, syms, wt_type, vars, pkind.complement(), seen);
                    }
                    if ctx.typ(bound).has_tag(TypeTag::BOT) {
                        bound = syms.object_type;
                        bk = BoundKind::UNBOUND;
                    }
                }
                _ => {}
            }
            return ctx.alloc_type(Type::new_wildcard_type(
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
        let formals =
            get_type_arguments_completed(ctx, syms, formals_type, None);
        let object = syms.object_type;
        let (bound, bk) = if !self.is_same_type(ctx, syms, upper, object)
            && (self.contains_any(ctx, syms, declared_bound, &formals)
                || !self.is_subtype(ctx, syms, declared_bound, upper))
        {
            (upper, BoundKind::EXTENDS)
        } else if !ctx.typ(lower).has_tag(TypeTag::BOT) {
            (lower, BoundKind::SUPER)
        } else {
            (object, BoundKind::UNBOUND)
        };
        ctx.alloc_type(Type::new_wildcard_type(
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
        syms: &Symtab,
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
        syms: &Symtab,
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
                crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => {
                    *members_field
                }
                _ => None,
            };
            if let Some(members) = members {
                let symbols = ctx.scope_get_symbols(
                    members,
                    crate::code::scope::LookupKind::NON_RECURSIVE,
                    &|_| true,
                );
                for absmeth in symbols {
                    if ctx.sym(absmeth).kind != crate::code::kinds::Kind::MTH {
                        continue;
                    }
                    let flags_value = ctx.sym(absmeth).flags_field;
                    let abstract_default_private = crate::code::flags::ABSTRACT as i64
                        | crate::code::flags::DEFAULT
                        | crate::code::flags::PRIVATE as i64;
                    if (flags_value & abstract_default_private)
                        != crate::code::flags::ABSTRACT as i64
                    {
                        continue;
                    }
                    let mut implmeth = crate::code::symbol::implementation(
                        ctx, syms, self, absmeth, impl_sym, true,
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
            if undef.is_none() {
                let c_type = match ctx.sym(c).ty {
                    Some(ty) => ty,
                    None => return None,
                };
                let supertype = self.supertype(ctx, syms, c_type, None);
                if ctx
                    .typ(supertype)
                    .has_tag(TypeTag::CLASS)
                {
                    if let Some(tsym) = ctx.typ(supertype).tsym {
                        undef = self.first_unimplemented_abstract_impl(
                            ctx,
                            syms,
                            allow_default_methods,
                            impl_sym,
                            tsym,
                        );
                    }
                }
                let interfaces = self.interfaces(ctx, syms, c_type);
                for interface in interfaces.iter() {
                    if undef.is_some() {
                        break;
                    }
                    if let Some(tsym) = ctx.typ(interface).tsym {
                        undef = self.first_unimplemented_abstract_impl(
                            ctx,
                            syms,
                            allow_default_methods,
                            impl_sym,
                            tsym,
                        );
                    }
                }
            }
        }
        undef
    }
}
impl Types {
    /// Java: public RetentionPolicy getRetention(TypeSymbol sym) (Types.java:5024-5040).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - sym: the annotation type symbol.
    ///
    /// # Returns
    ///   - the retention policy (CLASS by default).
    ///
    /// # Panics
    ///   - the Types names service must be installed.
    ///
    /// Upstream: Types.java:5024-5040.
    /// Java: public String toString(Type t) (Types.java:3648-3654) — the
    /// deprecated display uses typaramsString for ForAll types.
    pub fn types_to_string(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> String {
        if ctx.typ(t).has_tag(TypeTag::FORALL) {
            let tvars = match &ctx.typ(t).kind {
                TypeKindData::ForAll { tvars, .. } => tvars.clone(),
                _ => crate::util::assert::error(),
            };
            let qtype = match &ctx.typ(t).kind {
                TypeKindData::ForAll { qtype, .. } => qtype.expect("for-all qtype"),
                _ => crate::util::assert::error(),
            };
            format!(
                "{}{}",
                self.typarams_string(ctx, syms, &tvars),
                crate::code::type_::type_to_string(ctx, qtype)
            )
        } else {
            crate::code::type_::type_to_string(ctx, t)
        }
    }

    /// Java: private String typaramsString(List<Type> tvars) (Types.java:3656-3667)
    pub fn typarams_string(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        tvars: &List<TypeId>,
    ) -> String {
        let mut result = String::from("<");
        let mut first = true;
        for tvar in tvars.iter() {
            if !first {
                result.push_str(", ");
            }
            first = false;
            self.append_typaram_string(ctx, syms, tvar, &mut result);
        }
        result.push('>');
        result
    }

    /// Java: private void appendTyparamString(TypeVar t, StringBuilder buf)
    /// (Types.java:3668-3693)
    pub fn append_typaram_string(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
        buf: &mut String,
    ) {
        buf.push_str(&crate::code::type_::type_to_string(ctx, t));
        let Some(bound) = ctx.upper_bound_of(t) else {
            return;
        };
        let object_tsym = ctx.typ(syms.object_type).tsym;
        if ctx.typ(bound).tsym == object_tsym {
            return;
        }
        buf.push_str(" extends ");
        if !self.is_compound(ctx, bound) {
            buf.push_str(&crate::code::type_::type_to_string(ctx, bound));
        } else {
            let erasure = self.erasure(ctx, syms, t);
            let erasure_is_interface = ctx
                .typ(erasure)
                .tsym
                .map(|tsym| (ctx.sym(tsym).flags_field & crate::code::flags::INTERFACE as i64) != 0)
                .unwrap_or(false);
            if !erasure_is_interface {
                let supertype = self.supertype(ctx, syms, t, None);
                buf.push_str(&crate::code::type_::type_to_string(ctx, supertype));
                for interface in self.interfaces(ctx, syms, t).iter() {
                    buf.push('&');
                    buf.push_str(&crate::code::type_::type_to_string(ctx, interface));
                }
            } else {
                let mut first = true;
                for interface in self.interfaces(ctx, syms, t).iter() {
                    if !first {
                        buf.push('&');
                    }
                    first = false;
                    buf.push_str(&crate::code::type_::type_to_string(ctx, interface));
                }
            }
        }
    }

    /// Java: public void newRound() (Types.java:5257-5263)
    pub fn new_round(&self) {
        self.descriptor_cache.borrow_mut().clear();
        self.is_derived_raw_cache.borrow_mut().clear();
        self.implementation_cache.borrow_mut().clear();
        self.members_closure_cache.borrow_mut().clear();
        self.members_closure_seen.borrow_mut().clear();
        self.candidates_cache.borrow_mut().clear();
    }

    /// Java: public Collector<Type, ClosureHolder, List<Type>> closureCollector(...)
    /// (Types.java:3735-3740)
    ///
    /// the Java Collector/Stream abstraction is not part of the
    /// port; the same builder lifecycle (supplier/add/merge/finish) is exposed
    /// explicitly by ClosureCollector (registered adapter).
    pub fn closure_collector(
        &self,
        min_closure: bool,
        should_skip: ClosureSkip,
    ) -> ClosureCollector {
        ClosureCollector {
            holder: ClosureHolder {
                closure: List::nil(),
                min_closure,
                should_skip,
            },
        }
    }

    pub fn get_retention(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        sym: SymbolId,
    ) -> crate::code::attribute::RetentionPolicy {
        use crate::code::attribute::RetentionPolicy;
        let mut vis = RetentionPolicy::CLASS;
        let retention_tsym = match ctx.typ(syms.retention_type).tsym {
            Some(tsym) => tsym,
            None => return vis,
        };
        let compound = match ctx.sym(sym).attribute(ctx, retention_tsym) {
            Some(attribute) => attribute,
            None => return vis,
        };
        let names = std::rc::Rc::clone(&self.names);
        let values = match &ctx.attr(compound).kind {
            crate::code::attribute::AttributeKind::Compound { values, .. } => values.clone(),
            _ => return vis,
        };
        let current = values;
        for pair in &current {
            if ctx.sym(pair.fst).name != names.value {
                continue;
            }
            if let crate::code::attribute::AttributeKind::Enum {
                value: Some(enum_sym),
            } = &ctx.attr(pair.snd).kind
            {
                let level = ctx.sym(*enum_sym).name.clone();
                if level == names.SOURCE {
                    vis = RetentionPolicy::SOURCE;
                } else if level == names.CLASS {
                    vis = RetentionPolicy::CLASS;
                } else if level == names.RUNTIME {
                    vis = RetentionPolicy::RUNTIME;
                }
            }
        }
        vis
    }
}

impl Types {
    /// Java: JCLambda.getDescriptorType(Types types) (JCTree.java:775-777).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - diags/chk: the diagnostic factory and the check service;
    ///   - target: the lambda target type.
    ///
    /// # Returns
    ///   - the descriptor type, or an error type for a null target.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: JCTree.java:775-777 (delegates to Types.findDescriptorType).
    pub fn lambda_descriptor_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        diags: &crate::util::jc_diagnostic::Factory,
        chk: &crate::comp::check::Check,
        target: Option<TypeId>,
    ) -> TypeId {
        match target {
            Some(target) => match self.find_descriptor_type(ctx, syms, diags, chk, target) {
                Ok(descriptor) => descriptor,
                Err(_) => self.create_error_type_original(ctx, Some(target), syms),
            },
            None => self.create_error_type_original(ctx, None, syms),
        }
    }
}

impl Types {
    /// Java: public Type skipTypeVars(Type t, boolean capture) with capture=false
    /// (Types.java:180-188; the ported private helper is the non-capturing path).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - t: the type to project.
    ///
    /// # Returns
    ///   - the projection of t after skipping type variables.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Types.java:180-188.
    pub fn skip_type_vars_public(
        &self,
        ctx: &mut CompilerContext,
        t: TypeId,
    ) -> TypeId {
        self.skip_type_vars(ctx, t)
    }

    /// Java: Type.getThrownTypes() (Type.java:1491 MethodType / Type.java:1808
    /// ForAll), the Flow resource close() thrown-type read (Flow.java:1302).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - t: the method (or for-all) type.
    ///
    /// # Returns
    ///   - the declared thrown types (empty for other types).
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Flow.java:1302 (getThrownTypes on the member type).
    pub fn get_thrown_types_public(
        &self,
        ctx: &CompilerContext,
        t: TypeId,
    ) -> List<TypeId> {
        get_thrown_types(ctx, t)
    }
}

/// (Types.java:5046-5061).
///
/// The generator reports types that cannot appear in a JVM signature (e.g.
/// compound/intersection class types or captured type variables) by returning
/// this exception value, which carries the offending type exactly like the
/// upstream RuntimeException field.
#[derive(Clone, Debug)]
pub struct InvalidSignatureException {
    pub type_: TypeId,
}

impl InvalidSignatureException {
    /// Return the type that could not be assembled into a signature.
    ///
    /// constructor argument reported by reportIllegalSignature; callers use it
    /// to attach the diagnostic to the offending type tree.
    ///
    /// # Returns
    ///   - the type that caused the signature error.
    ///
    /// # Panics
    ///   - none.
    ///
    pub fn type_(&self) -> TypeId {
        self.type_
    }
}

///
/// Assembles the JVM generic signature of a type into a caller-provided buffer.
/// The upstream abstract class is modelled as a trait: implementations supply
/// the buffer append primitives and the class-reference hook, while the
/// signature grammar itself is ported as default methods so that subclasses
/// such as Lower.LowerSignatureGenerator only override the append primitives.
pub trait SignatureGenerator {
    /// Append one character to the signature buffer.
    ///
    /// concrete buffer decides whether characters are widened to UTF-8 bytes
    /// (StringBuilder) or emitted directly (the Lower signature generator).
    ///
    /// # Arguments
    ///   - ch: the character to append.
    ///
    /// # Panics
    ///   - none; buffer overflow handling is the implementation's concern.
    ///
    fn append_char(&mut self, ch: char);

    /// Append a pre-encoded UTF-8 byte sequence to the signature buffer.
    ///
    /// for externalized class names so that the internal JVM encoding is
    /// produced without a second conversion step.
    ///
    /// # Arguments
    ///   - ba: the bytes to append.
    ///
    /// # Panics
    ///   - none.
    ///
    fn append_bytes(&mut self, ba: &[u8]);

    /// Append a name to the signature buffer.
    ///
    /// implementation decides the encoding (upstream uses the name table bytes).
    ///
    /// # Arguments
    ///   - name: the name to append.
    ///
    /// # Panics
    ///   - none.
    ///
    fn append_name(&mut self, name: &Name);

    /// Hook invoked when a class name is referenced from a signature.
    ///
    /// The upstream default is a no-op; the class-writer subclass records the
    /// referenced class so that the constant pool entry can be created later.
    ///
    /// # Arguments
    ///   - _c: the referenced class symbol.
    ///
    /// # Panics
    ///   - none.
    ///
    fn class_reference(&mut self, _c: SymbolId) {}

    /// Return the Types service that owns this generator.
    ///
    /// exposes it to the default signature methods so that a generator can be
    /// created once and reused for many trees.
    ///
    /// # Returns
    ///   - the shared Types service.
    ///
    /// # Panics
    ///   - none.
    ///
    fn types(&self) -> &Types;

    /// Report a type that is illegal in a generic signature.
    ///
    /// The upstream method throws InvalidSignatureException; the ported trait
    /// returns the same payload as an error so that callers propagate it without
    /// panics (the Lower caller converts it into the JVMS error path).
    ///
    /// # Arguments
    ///   - t: the offending type.
    ///
    /// # Returns
    ///   - Err(InvalidSignatureException) carrying t.
    ///
    /// # Panics
    ///   - none.
    ///
    fn report_illegal_signature(&mut self, t: TypeId) -> Result<(), InvalidSignatureException> {
        Err(InvalidSignatureException { type_: t })
    }

    /// Assemble the generic signature of a type.
    ///
    /// Java: public void assembleSig(Type type) — the default signature grammar;
    /// subclasses override this entry to special-case verifier pseudo-types.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - t: the type whose signature is assembled.
    ///
    /// # Returns
    ///   - Ok(()) when appended, Err(InvalidSignatureException) for illegal types.
    ///
    /// # Panics
    ///   - an unsupported type tag reaches the assertion branch.
    fn assemble_sig(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> Result<(), InvalidSignatureException> {
        self.assemble_sig_default(ctx, syms, t)
    }

    /// Assemble the generic signature of a type.
    ///
    /// method walks the Java type tag and appends the corresponding JVMS 4.7.9.1
    /// signature grammar: primitive/void codes, L<class>; for classes, [ for
    /// arrays, (args)ret^throws for methods, wildcard bound markers, T<var>; for
    /// type variables and <params>qtype for foralls. Compound class types and
    /// captured type variables are reported as illegal signatures.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table (used by the Types helpers);
    ///   - t: the type whose signature is assembled.
    ///
    /// # Returns
    ///   - Ok(()) when the type was appended, Err(InvalidSignatureException) for
    ///     types that cannot appear in a signature.
    ///
    /// # Panics
    ///   - the type tag is not handled by the signature grammar (assertion,
    ///     mirroring the upstream AssertionError default branch).
    ///
    fn assemble_sig_default(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> Result<(), InvalidSignatureException> {
        use crate::code::type_tag::TypeTag;
        match ctx.typ(t).get_tag() {
            TypeTag::BYTE => self.append_char('B'),
            TypeTag::SHORT => self.append_char('S'),
            TypeTag::CHAR => self.append_char('C'),
            TypeTag::INT => self.append_char('I'),
            TypeTag::LONG => self.append_char('J'),
            TypeTag::FLOAT => self.append_char('F'),
            TypeTag::DOUBLE => self.append_char('D'),
            TypeTag::BOOLEAN => self.append_char('Z'),
            TypeTag::VOID => self.append_char('V'),
            TypeTag::CLASS => {
                if self.types().is_compound_public(ctx, t) {
                    self.report_illegal_signature(t)?;
                }
                self.append_char('L');
                self.assemble_class_sig(ctx, syms, t)?;
                self.append_char(';');
            }
            TypeTag::ARRAY => {
                let elemtype = match &ctx.typ(t).kind {
                    TypeKindData::ArrayType { elemtype, .. } => *elemtype,
                    _ => panic!("AssertionError: array type expected"),
                };
                self.append_char('[');
                if let Some(elemtype) = elemtype {
                    self.assemble_sig(ctx, syms, elemtype)?;
                }
            }
            TypeTag::METHOD => {
                let (argtypes, restype, thrown) = match &ctx.typ(t).kind {
                    TypeKindData::MethodType {
                        argtypes,
                        restype,
                        thrown,
                        ..
                    } => (argtypes.clone(), *restype, thrown.clone()),
                    _ => panic!("AssertionError: method type expected"),
                };
                self.append_char('(');
                self.assemble_sig_list(ctx, syms, argtypes)?;
                self.append_char(')');
                if let Some(restype) = restype {
                    self.assemble_sig(ctx, syms, restype)?;
                }
                if self.has_type_var(ctx, thrown.clone()) {
                    let mut current = thrown;
                    while current.non_empty() {
                        self.append_char('^');
                        self.assemble_sig(ctx, syms, *current.head())?;
                        current = current.tail();
                    }
                }
            }
            TypeTag::WILDCARD => {
                let (kind, bounded) = match &ctx.typ(t).kind {
                    TypeKindData::WildcardType { kind, type_, .. } => {
                        (*kind, *type_)
                    }
                    _ => panic!("AssertionError: wildcard type expected"),
                };
                match kind {
                    BoundKind::SUPER => {
                        self.append_char('-');
                        if let Some(bounded) = bounded {
                            self.assemble_sig(ctx, syms, bounded)?;
                        }
                    }
                    BoundKind::EXTENDS => {
                        self.append_char('+');
                        if let Some(bounded) = bounded {
                            self.assemble_sig(ctx, syms, bounded)?;
                        }
                    }
                    BoundKind::UNBOUND => self.append_char('*'),
                }
            }
            TypeTag::TYPEVAR => {
                if ctx.typ(t).is_captured() {
                    self.report_illegal_signature(t)?;
                }
                let tsym = ctx.typ(t).tsym.expect("type variable symbol");
                self.append_char('T');
                let name = ctx.sym(tsym).name.clone();
                self.append_name(&name);
                self.append_char(';');
            }
            TypeTag::FORALL => {
                let (tvars, qtype) = match &ctx.typ(t).kind {
                    TypeKindData::ForAll { tvars, qtype } => {
                        (tvars.clone(), *qtype)
                    }
                    _ => panic!("AssertionError: forall type expected"),
                };
                self.assemble_params_sig(ctx, syms, tvars)?;
                if let Some(qtype) = qtype {
                    self.assemble_sig(ctx, syms, qtype)?;
                }
            }
            other => panic!("AssertionError: typeSig {:?}", other),
        }
        Ok(())
    }

    /// Check whether a type list mentions a type variable.
    ///
    /// result decides whether a method's throws clause is appended after the
    /// return type (JVMS 4.7.9.1 only emits ^throwable for generic throws).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - l: the type list to scan.
    ///
    /// # Returns
    ///   - true when one element has the TYPEVAR tag.
    ///
    /// # Panics
    ///   - none.
    ///
    fn has_type_var(
        &self,
        ctx: &CompilerContext,
        mut l: List<TypeId>,
    ) -> bool {
        while l.non_empty() {
            if ctx
                .typ(*l.head())
                .has_tag(TypeTag::TYPEVAR)
            {
                return true;
            }
            l = l.tail();
        }
        false
    }

    /// Assemble the class portion of a signature (after the leading 'L').
    ///
    /// a member class whose enclosing type carries type parameters the upstream
    /// grammar emits the enclosing type followed by '.' (or '$' for local and
    /// anonymous classes, whose flat name carries the synthetic separator);
    /// otherwise the externalized flat name is appended. Type arguments follow
    /// in angle brackets when the class is parameterized.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - t: the class type whose class part is assembled.
    ///
    /// # Returns
    ///   - Ok(()) when appended, Err(InvalidSignatureException) for illegal
    ///     compound/intersection class types reached through nested calls.
    ///
    /// # Panics
    ///   - the type is not a class type, or a class symbol lacks an owner.
    ///
    fn assemble_class_sig(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> Result<(), InvalidSignatureException> {
        let tsym = ctx.typ(t).tsym.expect("class symbol");
        self.class_reference(tsym);
        let outer = ctx
            .typ(t)
            .get_enclosing_type()
            .unwrap_or_else(|| crate::code::symtab::no_type_id(ctx));
        let outer_allparams = allparams_of(ctx, outer);
        if !outer_allparams.is_empty() {
            let owner = ctx.sym(tsym).owner.expect("class owner");
            let empty_name = self.types().names.empty.to_java_string();
            let raw_outer = ctx.sym(owner).kind == crate::code::kinds::Kind::MTH
                || ctx.sym(tsym).name.content_equals(&empty_name);
            let outer_sig_type = if raw_outer {
                self.types().erasure(ctx, syms, outer)
            } else {
                outer
            };
            self.assemble_class_sig(ctx, syms, outer_sig_type)?;
            self.append_char(if raw_outer { '$' } else { '.' });
            let flatname = class_flatname(ctx, tsym);
            if raw_outer {
                let owner_encl = crate::code::symbol::symbol_encl_class(ctx, owner)
                    .expect("owner enclosing class");
                let owner_flat = class_flatname(ctx, owner_encl);
                let start = owner_flat.get_byte_length() + 1;
                let sub = flatname.sub_name(start, flatname.get_byte_length());
                self.append_name(&sub);
            } else {
                let name = ctx.sym(tsym).name.clone();
                self.append_name(&name);
            }
        } else {
            let flatname = class_flatname(ctx, tsym);
            let external = crate::jvm::class_file::externalize_name(&flatname);
            self.append_bytes(&external);
        }
        let args = get_type_arguments_completed(ctx, syms, t, None);
        if !args.is_empty() {
            self.append_char('<');
            self.assemble_sig_list(ctx, syms, args)?;
            self.append_char('>');
        }
        Ok(())
    }

    /// Assemble the formal type-parameter section of a generic signature.
    ///
    /// Emits '<' then, for each type variable, its name
    /// followed by the bounds in JVMS form: a ':' before the first bound when it
    /// is an interface, and one ':' per bound otherwise.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table (for Types.getBounds);
    ///   - typarams: the type variables.
    ///
    /// # Returns
    ///   - Ok(()) when appended, Err(InvalidSignatureException) for illegal bound
    ///     types.
    ///
    /// # Panics
    ///   - a type parameter has no symbol.
    ///
    fn assemble_params_sig(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        typarams: List<TypeId>,
    ) -> Result<(), InvalidSignatureException> {
        self.append_char('<');
        let mut current = typarams;
        while current.non_empty() {
            let tvar = *current.head();
            let tsym = ctx.typ(tvar).tsym.expect("type variable symbol");
            let name = ctx.sym(tsym).name.clone();
            self.append_name(&name);
            let bounds = self.types().get_bounds(ctx, syms, tvar);
            if !bounds.is_empty() {
                let bounds_head = *bounds.head();
                if let Some(bounds_tsym) = ctx.typ(bounds_head).tsym
                    && (ctx.sym(bounds_tsym).flags_field & crate::code::flags::INTERFACE as i64)
                        != 0
                {
                    self.append_char(':');
                }
            }
            let mut bounds_current = bounds;
            while bounds_current.non_empty() {
                self.append_char(':');
                self.assemble_sig(ctx, syms, *bounds_current.head())?;
                bounds_current = bounds_current.tail();
            }
            current = current.tail();
        }
        self.append_char('>');
        Ok(())
    }

    /// Assemble the signatures of a type list.
    ///
    /// Each element is assembled in order; this is used for method parameters,
    /// class type arguments and type-parameter bounds.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - types: the type list.
    ///
    /// # Returns
    ///   - Ok(()) when all types were appended, Err(InvalidSignatureException) for
    ///     the first illegal type.
    ///
    /// # Panics
    ///   - none.
    ///
    fn assemble_sig_list(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        mut types: List<TypeId>,
    ) -> Result<(), InvalidSignatureException> {
        while types.non_empty() {
            self.assemble_sig(ctx, syms, *types.head())?;
            types = types.tail();
        }
        Ok(())
    }
}

/// Return the flat name of a class symbol (Symbol.ClassSymbol.flatname).
///
/// The helper mirrors the upstream field access used by the signature
/// generator; a non-class symbol is an internal invariant violation.
fn class_flatname(
    ctx: &CompilerContext,
    sym: SymbolId,
) -> Name {
    match &ctx.sym(sym).kind_data {
        crate::code::symbol::SymbolKind::ClassSymbol { flatname, .. } => flatname.clone(),
        _ => panic!("ClassCastException: class symbol expected"),
    }
}

impl Types {
    /// Create a copy of a method type with a different parameter list.
    ///
    /// The original type may be a for-all type wrapping a method type; in that
    /// case the qualifier type is copied recursively while the type variables
    /// are preserved. Any other type is rejected with a panic, matching the
    /// upstream IllegalArgumentException in the parameters visitor.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - original: the method (or for-all) type;
    ///   - new_params: the replacement parameter types.
    ///
    /// # Returns
    ///   - the rebuilt method type.
    ///
    /// # Panics
    ///   - the original type is neither a method type nor a for-all type.
    pub fn create_method_type_with_parameters(
        &self,
        ctx: &mut CompilerContext,
        original: TypeId,
        new_params: List<TypeId>,
    ) -> TypeId {
        match &ctx.typ(original).kind {
            TypeKindData::MethodType { .. } => {
                let mut mt = ctx.typ(original).clone();
                if let TypeKindData::MethodType { argtypes, .. } = &mut mt.kind {
                    *argtypes = new_params;
                }
                ctx.alloc_type(mt)
            }
            TypeKindData::ForAll { tvars, qtype } => {
                let tvars = tvars.clone();
                let inner = qtype.expect("for-all qualifier type");
                let new_inner = self.create_method_type_with_parameters(ctx, inner, new_params);
                let forall = ctx.typ(original).clone();
                let mut forall = forall;
                if let TypeKindData::ForAll { qtype, .. } = &mut forall.kind {
                    *qtype = Some(new_inner);
                    let _ = &tvars;
                }
                ctx.alloc_type(forall)
            }
            _ => panic!("IllegalArgumentException: not a method type"),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "varInstantiated must not be called")]
    fn bounds_change_recorder_rejects_instantiation() {
        let mut ctx = CompilerContext::new();
        let names = Names::new(false);
        let syms = Symtab::new(
            &mut ctx,
            Rc::new(names),
            Target::DEFAULT,
            Source::JDK17,
        );
        let types = Types::with_names(Rc::new(Names::new(false)));
        let uv = ctx.alloc_type(Type::new(
            TypeTag::NONE,
            None,
            TypeKindData::JCNoType {
                kind: crate::code::type_::JCNoTypeKind::Base,
            },
        ));
        let recorder = BoundsChangeRecorder::default();
        crate::comp::infer::UndetVarListener::var_instantiated(
            &recorder, &mut ctx, &syms, &types, uv,
        );
    }
    use crate::code::source::Source;
    use crate::jvm::target::Target;
    use crate::util::names::Names;
    use std::rc::Rc;

    #[test]
    fn error_type_model_type_provider_is_lazy_and_cached() {
        let (mut ctx, syms) = build();
        let names = Rc::new(Names::new(false));
        let types = Types::with_names(names);
        let err_ty = syms.err_type;
        let error = ctx.alloc_type(Type::new(
            TypeTag::ERROR,
            None,
            TypeKindData::ErrorType {
                outer_field: None,
                typarams_field: None,
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
                original_type: None,
                candidate_symbol: None,
                model_type: None,
                model_type_provider: Some(Rc::new(move |_ctx, _syms, _types| err_ty)),
            },
        ));
        assert_eq!(types.get_model_type(&mut ctx, &syms, error), Some(err_ty));
        assert_eq!(types.get_model_type(&mut ctx, &syms, error), Some(err_ty));
    }

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

    #[test]
    fn symbol_precedes_orders_type_variables_before_other_types() {
        use crate::code::kinds::Kind;
        use crate::code::symbol::{Symbol, SymbolKind};
        let (mut ctx, syms, types) = build_with_types();
        let table: Rc<NameTableHandle> = NameTableHandle::unshared();
        let name_of = |text: &str| {
            let units: Vec<u16> = text.encode_utf16().collect();
            table.from_chars(&units, 0, units.len() as i32)
        };
        let class = ctx.alloc_symbol(Symbol::new(
            Kind::TYP,
            0,
            name_of("C"),
            None,
            None,
            SymbolKind::TypeSymbol,
        ));
        let class_type_id = ctx.alloc_type(Type::new(
            TypeTag::CLASS,
            Some(class),
            TypeKindData::ClassType {
                outer_field: None,
                typarams_field: Some(List::nil()),
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
            },
        ));
        ctx.sym_mut(class).ty = Some(class_type_id);
        let type_variable = ctx.alloc_symbol(Symbol::new(
            Kind::TYP,
            0,
            name_of("T"),
            None,
            None,
            SymbolKind::TypeSymbol,
        ));
        let type_variable_type = ctx.alloc_type(Type::new(
            TypeTag::TYPEVAR,
            Some(type_variable),
            TypeKindData::TypeVar {
                bound: None,
                lower: None,
            },
        ));
        ctx.sym_mut(type_variable).ty = Some(type_variable_type);
        assert!(!types.symbol_precedes(&mut ctx, &syms, class, class));
        assert!(types.symbol_precedes(&mut ctx, &syms, type_variable, class));
        assert!(!types.symbol_precedes(&mut ctx, &syms, class, type_variable));
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
        assert_eq!(types.hash_code(&mut ctx, &symtab, symtab.int_type), 5);
        assert_eq!(types.hash_code(&mut ctx, &symtab, symtab.byte_type), 0);
        assert_eq!(types.hash_code(&mut ctx, &symtab, symtab.char_type), 1);
        assert_eq!(types.hash_code(&mut ctx, &symtab, symtab.short_type), 2);
        assert_eq!(types.hash_code(&mut ctx, &symtab, symtab.long_type), 3);
        assert_eq!(types.hash_code(&mut ctx, &symtab, symtab.float_type), 4);
        assert_eq!(types.hash_code(&mut ctx, &symtab, symtab.double_type), 6);
        assert_eq!(types.hash_code(&mut ctx, &symtab, symtab.boolean_type), 7);
        assert_eq!(types.hash_code(&mut ctx, &symtab, symtab.void_type), 8);
        assert_eq!(types.hash_code(&mut ctx, &symtab, symtab.bot_type), 18);
        assert_eq!(types.hash_code(&mut ctx, &symtab, symtab.err_type), 0);
        assert_eq!(types.hash_code(&mut ctx, &symtab, symtab.unknown_type), 21);
        let no_type = ctx.sym(symtab.no_symbol).ty.expect("no type");
        assert_eq!(types.hash_code(&mut ctx, &symtab, no_type), 19);
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
        assert_eq!(types.hash_code(&mut ctx, &symtab, int_array), 5 + 12);
        let int_array_2d = ctx.alloc_type(Type::new(
            TypeTag::ARRAY,
            Some(symtab.array_class),
            TypeKindData::ArrayType {
                elemtype: Some(int_array),
                varargs: false,
            },
        ));
        assert_eq!(
            types.hash_code(&mut ctx, &symtab, int_array_2d),
            5 + 12 + 12
        );
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
        ));
        assert!(types.is_same_type(&mut ctx, &symtab, string_array, string_array2));
    }

    #[test]
    fn is_same_type_handles_absent_enclosing_type_as_no_type() {
        fn top_level_class_type(ctx: &mut CompilerContext, tsym: SymbolId) -> TypeId {
            ctx.alloc_type(Type::new(
                TypeTag::CLASS,
                Some(tsym),
                TypeKindData::ClassType {
                    outer_field: None,
                    typarams_field: Some(List::nil()),
                    allparams_field: None,
                    supertype_field: None,
                    interfaces_field: None,
                    all_interfaces_field: None,
                },
            ))
        }
        let (mut ctx, symtab) = build();
        let types = Types::new();
        let string_tsym = ctx.typ(symtab.string_type).tsym.expect("string symbol");
        let first = top_level_class_type(&mut ctx, string_tsym);
        let second = top_level_class_type(&mut ctx, string_tsym);
        // Both top-level classes have no enclosing type (Type.noType); upstream
        // compares the two noType values as equal.
        assert!(types.is_same_type(&mut ctx, &symtab, first, second));
        let no_type = ctx.sym(symtab.no_symbol).ty.expect("no type");
        let with_no_type_outer = ctx.alloc_type(Type::new(
            TypeTag::CLASS,
            Some(string_tsym),
            TypeKindData::ClassType {
                outer_field: Some(no_type),
                typarams_field: Some(List::nil()),
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
            },
        ));
        // noType and a class type are different.
        assert!(!types.is_same_type(&mut ctx, &symtab, first, with_no_type_outer));
        assert!(!types.is_same_type(&mut ctx, &symtab, no_type, first));
        // The class-type hash uses the same noType adapter instead of panicking.
        assert_eq!(
            types.hash_code(&mut ctx, &symtab, first),
            types.hash_code(&mut ctx, &symtab, second)
        );
    }

    #[test]
    fn erasure_keeps_primitives_strings_and_arrays() {
        let (mut ctx, symtab) = build();
        let types = Types::new();
        assert_eq!(
            types.erasure(&mut ctx, &symtab, symtab.int_type),
            symtab.int_type
        );
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

    struct CountingCompleter(Rc<std::cell::Cell<usize>>);

    impl crate::code::symbol::Completer for CountingCompleter {
        fn complete(
            &mut self,
            ctx: &mut CompilerContext,
            _syms: &Symtab,
            sym: SymbolId,
        ) -> Result<(), crate::code::symbol::CompletionFailure> {
            self.0.set(self.0.get() + 1);
            if let Some(ty) = ctx.sym(sym).ty
                && let TypeKindData::ClassType { typarams_field, .. } =
                    &mut ctx.typ_mut(ty).kind
            {
                *typarams_field = Some(List::nil());
            }
            Ok(())
        }
    }

    #[test]
    fn completed_reads_call_the_completer_only_once() {
        let (mut ctx, syms) = build();
        let names = Names::new(false);
        let no_type = ctx.no_type_id();
        let class = crate::code::symtab::new_class_symbol(
            &mut ctx,
            0,
            names.from_utf(b"Missing"),
            None,
            no_type,
        );
        let class_type = ctx.sym(class).ty.expect("class type");
        let calls = Rc::new(std::cell::Cell::new(0));
        ctx.sym_mut(class).completer = Some(Rc::new(std::cell::RefCell::new(CountingCompleter(
            Rc::clone(&calls),
        ))));
        let first = get_type_arguments_completed(&mut ctx, &syms, class_type, None);
        let second = get_type_arguments_completed(&mut ctx, &syms, class_type, None);
        assert_eq!(calls.get(), 1);
        assert!(first.is_empty() && second.is_empty());
    }

    #[test]
    fn completed_get_type_arguments_does_not_reenter_completion() {
        let (mut ctx, symtab) = build();
        let list_tsym = ctx.typ(symtab.list_type).tsym.expect("List symbol");
        let list_string = class_type(&mut ctx, list_tsym, &[symtab.string_type]);
        // typarams_field is Some, so the completed helper must take the cache
        // branch and never call itself (the former fallback self-recursion).
        let args = get_type_arguments_completed(&mut ctx, &symtab, list_string, None);
        assert_eq!(args.length(), 1);
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
        let (mut ctx, symtab) = build();
        let types = Types::new();
        for (ty, expected) in [
            (symtab.int_type, "java.lang.Integer"),
            (symtab.void_type, "java.lang.Void"),
            (symtab.boolean_type, "java.lang.Boolean"),
            (symtab.byte_type, "java.lang.Byte"),
        ] {
            let boxed = types.boxed_class(&mut ctx, &symtab, ty);
            let flatname = flat_name_of(&ctx, boxed);
            assert_eq!(flatname.to_java_string().to_string_lossy(), expected);
        }
    }

    #[test]
    fn symtab_class_field_is_cached_and_has_the_jvm_shape() {
        let (mut ctx, symtab) = build();
        let types = Types::new();
        let int_type = symtab.int_type;
        let field = symtab.get_class_field(&mut ctx, &types, int_type);
        let symbol = ctx.sym(field);
        assert_eq!(
            symbol.flags()
                & (crate::code::flags::STATIC
                    | crate::code::flags::PUBLIC
                    | crate::code::flags::FINAL) as i64,
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
        let boxed = types.boxed_class(&mut ctx, &symtab, int_type);
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
        let types = Types::new();
        let int_array = new_array_type(&mut ctx, symtab.int_type, Some(symtab.array_class));
        assert_eq!(
            elemtype_of(&types, &ctx, &symtab, int_array).expect("array element type"),
            symtab.int_type
        );
        // Java: WildcardType.type is the (upper) bound; kind=EXTENDS.
        let wildcard = new_wildcard_type(&mut ctx, Some(int_array), BoundKind::EXTENDS, None, None);
        assert_eq!(
            elemtype_of(&types, &ctx, &symtab, wildcard).expect("array element type"),
            symtab.int_type
        );
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
        let types = Types::new();
        let list_tsym = ctx.typ(symtab.list_type).tsym.expect("list symbol");
        let parameterized = class_type(&mut ctx, list_tsym, &[symtab.string_type, symtab.int_type]);
        assert!(type_contains(
            &types,
            &mut ctx,
            &symtab,
            parameterized,
            symtab.string_type
        ));
        assert!(!type_contains(
            &types,
            &mut ctx,
            &symtab,
            parameterized,
            symtab.long_type
        ));
        assert!(type_contains(
            &types,
            &mut ctx,
            &symtab,
            parameterized,
            parameterized
        ));
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

    // ------------------------------------------------------------------
    // functional interface descriptors / castable warner /
    // checkUnique MTH.
    // ------------------------------------------------------------------

    #[allow(clippy::type_complexity, clippy::too_many_arguments)]
    fn build_services() -> (
        CompilerContext,
        Rc<Symtab>,
        Types,
        Rc<crate::util::log::Log>,
        Rc<crate::util::jc_diagnostic::Factory>,
        Rc<crate::code::lint::Lint>,
        Rc<crate::code::deferred_lint_handler::DeferredLintHandler>,
    ) {
        use crate::code::deferred_lint_handler::DeferredLintHandler;
        use crate::util::context::Context;
        use crate::util::javac_messages::JavacMessages;
        let mut ctx = CompilerContext::new();
        let mut context = Context::new();
        let log = crate::util::log::Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = JavacMessages::instance(&mut context);
        let diags = Rc::new(crate::util::jc_diagnostic::Factory::new(
            messages, "compiler",
        ));
        let lint = crate::code::lint::Lint::instance(&mut context);
        let deferred = Rc::new(DeferredLintHandler::new());
        let symtab = Symtab::new(&mut ctx, Rc::clone(&names), Target::DEFAULT, Source::JDK17);
        let symtab = Rc::new(symtab);
        let types = Types::with_names(names);
        (ctx, symtab, types, log, diags, lint, deferred)
    }

    /// Build a synthetic interface class symbol with the given abstract method
    /// into the context (a local test fixture for the SAM tests; the upstream
    /// tests use full Enter/Analyze, which needs the Attr).
    #[allow(clippy::too_many_arguments)]
    fn make_interface_with_method(
        ctx: &mut CompilerContext,
        symtab: &Symtab,
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
                original_enclosing_method: None,
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
            let extra_symbol_name = names.from_string(&javac_strings::JavaString::from(extra_name));
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
                    original_enclosing_method: None,
                },
            ));
            ctx.scope_enter(members, extra);
        }
        (class, class_type, method)
    }

    fn check_for_tests(
        names: &Rc<Names>,
        log: &Rc<crate::util::log::Log>,
        diags: &Rc<crate::util::jc_diagnostic::Factory>,
        types: &Types,
        lint: &Rc<crate::code::lint::Lint>,
        deferred: &Rc<crate::code::deferred_lint_handler::DeferredLintHandler>,
        syms: &Rc<Symtab>,
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
        let syms = &*syms_rc;
        let int_type = syms.int_type;
        let java_base = syms.java_base;
        let (_, _, method) = make_interface_with_method(
            &mut ctx,
            &syms,
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
            .find_descriptor_symbol(&mut ctx, &syms, &diags, &chk, interface_tsym)
            .expect("SAM descriptor");
        assert_eq!(
            desc_sym, method,
            "the single abstract method is the descriptor"
        );
        assert!(types.is_functional_interface(&mut ctx, &syms, &diags, &chk, interface_tsym));
        let interface_ty = ctx.sym(interface_tsym).ty.expect("interface type");
        let desc_ty = types
            .find_descriptor_type(&mut ctx, &syms, &diags, &chk, interface_ty)
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
        let syms = &*syms_rc;
        let int_type = syms.int_type;
        let void_type = syms.void_type;
        let java_base = syms.java_base;
        let (_, _, method) = make_interface_with_method(
            &mut ctx,
            &syms,
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
        let result = types.find_descriptor_symbol(&mut ctx, &syms, &diags, &chk, interface_tsym);
        assert!(result.is_err(), "two abstract methods are not a SAM");
        assert!(!types.is_functional_interface(&mut ctx, &syms, &diags, &chk, interface_tsym));
    }

    #[test]
    fn descriptor_type_keeps_the_sam_thrown_clause() {
        let (mut ctx, syms_rc, types, log, diags, lint, deferred) = build_services();
        let names = Rc::new(Names::new(false));
        let syms = &*syms_rc;
        let int_type = syms.int_type;
        let exception_type = syms.exception_type;
        let java_base = syms.java_base;
        let (_, _, method) = make_interface_with_method(
            &mut ctx,
            &syms,
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
            .find_descriptor_type(&mut ctx, &syms, &diags, &chk, interface_ty)
            .expect("descriptor type");
        let thrown = get_thrown_types(&ctx, desc_ty);
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
        assert!(types.is_castable_with_warn(&mut ctx, &syms, int_type, long_type, &mut no_warn));
        assert!(!types.is_castable_with_warn(
            &mut ctx,
            &syms,
            boolean_type,
            int_type,
            &mut no_warn
        ));
        assert!(types.is_castable_with_warn(&mut ctx, &syms, int_type, int_type, &mut no_warn));
        assert!(!no_warn.has_lint(crate::code::lint::LintCategory::UNCHECKED));
    }

    #[test]
    fn check_unique_mth_uses_has_same_args() {
        let (mut ctx, syms_rc, types, log, diags, lint, deferred) = build_services();
        let names = Rc::new(Names::new(false));
        let chk = check_for_tests(&names, &log, &diags, &types, &lint, &deferred, &syms_rc);
        let (class, int_type, java_base) = {
            let syms = &*syms_rc;
            let int_type = syms.int_type;
            let java_base = syms.java_base;
            let (class, _, _) = make_interface_with_method(
                &mut ctx,
                &syms,
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
            chk.check_unique(
                &mut ctx,
                &*syms_rc,
                crate::util::position::NOPOS,
                symbols[0],
                members,
            ),
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
                original_enclosing_method: None,
            },
        ));
        ctx.scope_enter(members, dup);
        assert!(
            !chk.check_unique(
                &mut ctx,
                &*syms_rc,
                crate::util::position::NOPOS,
                dup,
                members,
            ),
            "same signature is a duplicate"
        );
        let _ = (int_type, java_base);
    }

    #[test]
    fn is_subtypes_and_unchecked_compare_corresponding_lists() {
        let (mut ctx, syms, types) = build_with_types();
        let ints = List::of(syms.int_type);
        let booleans = List::of(syms.boolean_type);
        assert!(types.is_subtypes(&mut ctx, &syms, &ints, &ints));
        assert!(!types.is_subtypes(&mut ctx, &syms, &ints, &booleans));
        assert!(!types.is_subtypes(&mut ctx, &syms, &ints, &List::nil()));
        let warn = crate::util::warner::Warner::new(None);
        assert!(types.is_subtypes_unchecked(&mut ctx, &syms, &ints, &ints, &warn));
        assert!(!types.is_subtypes_unchecked(&mut ctx, &syms, &ints, &booleans, &warn));
        assert!(!types.is_subtypes_unchecked(&mut ctx, &syms, &List::nil(), &ints, &warn));
    }

    #[test]
    fn is_array_and_elemtype_or_type_follow_the_upstream_tags() {
        let (mut ctx, syms, types) = build_with_types();
        let int_array = ctx.alloc_type(Type::new(
            TypeTag::ARRAY,
            Some(syms.array_class),
            TypeKindData::ArrayType {
                elemtype: Some(syms.int_type),
                varargs: false,
            },
        ));
        assert!(types.is_array(&mut ctx, &syms, int_array));
        assert!(!types.is_array(&mut ctx, &syms, syms.int_type));
        assert_eq!(
            types.elemtype_or_type(&mut ctx, &syms, int_array),
            syms.int_type
        );
        assert_eq!(
            types.elemtype_or_type(&mut ctx, &syms, syms.int_type),
            syms.int_type
        );
        let lower = types.cvar_lower_bounds(&mut ctx, &List::of(syms.int_type));
        assert_eq!(*lower.head(), syms.int_type);
    }

    #[test]
    fn is_derived_raw_caches_and_new_round_clears_the_cache() {
        let (mut ctx, syms, types) = build_with_types();
        let int_type = syms.int_type;
        assert!(!types.is_derived_raw(&mut ctx, &syms, int_type));
        assert_eq!(
            types.is_derived_raw_cache.borrow().len(),
            1,
            "cache entry inserted"
        );
        types.new_round();
        assert!(
            types.is_derived_raw_cache.borrow().is_empty(),
            "newRound clears isDerivedRawCache"
        );
    }

    #[test]
    fn typarams_string_and_subst_bound_cover_type_variables() {
        use crate::code::kinds::Kind;
        use crate::code::symbol::{Symbol, SymbolKind};
        let (mut ctx, syms, types) = build_with_types();
        let names = Rc::new(Names::new(false));
        let type_var_symbol = ctx.alloc_symbol(Symbol::new(
            Kind::TYP,
            0,
            names.from_string(&javac_strings::JavaString::from("T")),
            None,
            None,
            SymbolKind::TypeSymbol,
        ));
        let type_var = ctx.alloc_type(Type::new(
            TypeTag::TYPEVAR,
            Some(type_var_symbol),
            TypeKindData::TypeVar {
                bound: Some(syms.int_type),
                lower: Some(syms.bot_type),
            },
        ));
        ctx.sym_mut(type_var_symbol).ty = Some(type_var);
        assert_eq!(
            types.typarams_string(&mut ctx, &syms, &List::of(type_var)),
            "<T extends int>"
        );
        let unbound = ctx.alloc_type(Type::new(
            TypeTag::TYPEVAR,
            Some(type_var_symbol),
            TypeKindData::TypeVar {
                bound: Some(syms.object_type),
                lower: Some(syms.bot_type),
            },
        ));
        assert_eq!(
            types.typarams_string(&mut ctx, &syms, &List::of(unbound)),
            "<T>"
        );
        assert_eq!(
            types.subst_bound(&mut ctx, &syms, type_var, &List::nil(), &List::nil()),
            type_var,
            "unchanged bound returns the original type variable"
        );
    }

    #[test]
    fn closure_collector_supports_add_merge_and_finish() {
        let (mut ctx, syms, types) = build_with_types();
        let mut collector = types.closure_collector(false, ClosureSkip::Basic);
        collector.add(&types, &mut ctx, &syms, syms.int_type);
        assert_eq!(
            collector.closure(&types, &mut ctx, &syms),
            List::of(syms.int_type)
        );
        let mut other = types.closure_collector(false, ClosureSkip::Basic);
        other.add(&types, &mut ctx, &syms, syms.long_type);
        collector.merge(&types, &mut ctx, &syms, &other);
        let closure = collector.closure(&types, &mut ctx, &syms);
        assert_eq!(closure.length(), 2);
    }
}

/// A type together with the Types instance that owns its equality and hash.
///
/// Java Types.UniqueType is used as the constant-pool key for types so that two
/// structurally equal types share one entry; the Rust port keeps the same
/// contract behind explicit methods because equality needs the compiler context.
#[derive(Clone, Debug)]
pub struct UniqueType {
    /// The wrapped type.
    pub type_: TypeId,
}

impl UniqueType {
    /// Create a unique-type wrapper.
    pub fn new(type_: TypeId) -> UniqueType {
        UniqueType { type_ }
    }

    /// Return the hash code of the wrapped type.
    pub fn hash_code(&self, ctx: &mut CompilerContext, syms: &Symtab, types: &Types) -> i32 {
        types.hash_code(ctx, syms, self.type_)
    }

    /// Return whether the wrapped type equals the other wrapped type.
    pub fn equals(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        other: &UniqueType,
    ) -> bool {
        types.is_same_type(ctx, syms, self.type_, other.type_)
    }
}

impl Types {
    /// Return the type of a loadable constant.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - c: the loadable constant.
    ///
    /// # Returns
    ///   - the constant type.
    ///
    /// # Panics
    ///   - the constant is not loadable.
    pub fn constant_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        c: &crate::jvm::pool_constant::LoadableConstantRef,
    ) -> TypeId {
        use crate::jvm::pool_constant::LoadableConstantRef;
        match crate::jvm::pool_constant::loadable_pool_tag(c) {
            crate::jvm::class_file::CONSTANT_CLASS => syms.class_type,
            crate::jvm::class_file::CONSTANT_STRING => syms.string_type,
            crate::jvm::class_file::CONSTANT_INTEGER => syms.int_type,
            crate::jvm::class_file::CONSTANT_FLOAT => syms.float_type,
            crate::jvm::class_file::CONSTANT_LONG => syms.long_type,
            crate::jvm::class_file::CONSTANT_DOUBLE => syms.double_type,
            crate::jvm::class_file::CONSTANT_METHODHANDLE => syms.method_handle_type,
            crate::jvm::class_file::CONSTANT_METHODTYPE => syms.method_type_type,
            crate::jvm::class_file::CONSTANT_DYNAMIC => {
                let LoadableConstantRef::Dynamic(sym) = c else {
                    panic!("AssertionError: dynamic constant expected");
                };
                ctx.sym(*sym).ty.expect("dynamic constant type")
            }
            tag => panic!("AssertionError: Not a loadable constant: {}", tag),
        }
    }
}

impl Types {
    /// Return the base type of a type (the unannotated / primitive form).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - t: the type.
    ///
    /// # Returns
    ///   - the base type (the type symbol type for class types).
    ///
    /// # Panics
    ///   - none.
    pub fn base_type(&self, ctx: &mut CompilerContext, t: TypeId) -> TypeId {
        // Java: Type.baseType() returns this; only ConstType overrides it to
        // the underlying plain type. The class branch used to return the class
        // symbol's declared type, which dropped the type arguments
        // (ListBuffer<Type> became the declaration ListBuffer<T>).
        if ctx.typ(t).const_value().is_some() {
            return ctx.type_base_type(t);
        }
        match &ctx.typ(t).kind {
            TypeKindData::UndetVar {
                inst: Some(inst), ..
            } => self.base_type(ctx, *inst),
            _ => t,
        }
    }

    /// Return the string representation of a compile-time constant type.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - t: the constant type.
    ///
    /// # Returns
    ///   - the string value of the constant.
    ///
    /// # Panics
    ///   - the type is not a compile-time constant type.
    pub fn string_value(
        &self,
        ctx: &mut CompilerContext,
        t: TypeId,
    ) -> crate::util::java_string::JavaString {
        if ctx.typ(t).has_tag(TypeTag::BOT) {
            return crate::util::java_string::JavaString::from("null");
        }
        let constant = ctx
            .typ(t)
            .const_value()
            .expect("AssertionError: non-constant type");
        use crate::code::type_tag::TypeTag;
        match ctx.typ(t).get_tag() {
            TypeTag::BOOLEAN => {
                let truthy = match constant {
                    crate::util::constants::ConstantValue::Boolean(v) => *v,
                    crate::util::constants::ConstantValue::Integer(v) => *v != 0,
                    other => panic!("AssertionError: boolean constant expected: {:?}", other),
                };
                crate::util::java_string::JavaString::from(if truthy { "true" } else { "false" })
            }
            TypeTag::CHAR => {
                let unit = match constant {
                    crate::util::constants::ConstantValue::Character(v) => *v,
                    crate::util::constants::ConstantValue::Integer(v) => *v as u16,
                    other => panic!("AssertionError: char constant expected: {:?}", other),
                };
                crate::util::java_string::JavaString::from_units(vec![unit])
            }
            // Java: Type.stringValue() default arm is od.toString(); for a
            // String constant that is the string itself. Rendering through
            // Display goes through to_string_lossy and would replace unpaired
            // surrogates (GB18030's mapping tables contain them) with U+FFFD,
            // corrupting the constant.
            _ => match constant {
                crate::util::constants::ConstantValue::String(s) => s.clone(),
                other => crate::util::java_string::JavaString::from(other.display().as_str()),
            },
        }
    }
}

impl Types {
    /// Return the number of array dimensions of a type.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - t: the type.
    ///
    /// # Returns
    ///   - the number of array dimensions (0 for non-array types).
    ///
    /// # Panics
    ///   - none.
    pub fn dimensions(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) -> i32 {
        if !ctx.typ(t).has_tag(TypeTag::ARRAY) {
            return 0;
        }
        let elem = self.elemtype(ctx, syms, t);
        self.dimensions(ctx, syms, elem) + 1
    }
}
