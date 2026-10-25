// Origin: com/sun/tools/javac/code/Types.java @ 982d53c (jdk-17.0.19-ga)

//! Utility routines for the Type interface.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES (ADR 0001, arena + IDs): this batch ports ONLY the Types
//! subset needed to unlock TreeMaker/class-fields (see the task):
//! hashCode, isSameType, erasure, boxedClass, and the helpers they need.
//! The Java Types instance state (Symtab, Names, Source, Check, Enter,
//! JavacMessages, JCDiagnostic.Factory, Warner) is NOT stored in the Rust Types
//! (which is a stateless service): the methods that need it take the ported
//! Symtab explicitly (registered section 9; no Rc/RefCell is introduced). The
//! remaining Types methods are NOT PORTED (TODO Types batch):
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
//! type_.rs with the Type batch, registered): getEnclosingType,
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
/// PORTING_RULES: the Java instance fields (syms/names/...) are not stored; the
/// ported methods take them as parameters.
#[derive(Debug, Default, Clone, Copy)]
pub struct Types;

impl Types {
    /// Get the Types instance for this context.
    ///
    /// PORTING_RULES: the Java Types instance is registered in the Context; the
    /// Rust Types is stateless, so this constructor does not need the Context
    /// (Context registration is TODO with the Types batch).
    pub fn new() -> Types {
        Types
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
            TypeKindData::ArrayType { elemtype } => {
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
                //     thisargs.tail != null; thisargs = thisargs.tail)
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
        }
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
            // violation (visit(null, ...)); do not invent a value.
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
            TypeKindData::ArrayType { elemtype } => elemtype.expect("array element type"),
            _ => unreachable!(),
        };
        let elemtype_s = match &ctx.typ(s).kind {
            TypeKindData::ArrayType { elemtype } => elemtype.expect("array element type"),
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
                // NOT PORTED: hasSameBounds/subst (TODO Types batch).
                panic!("unimplemented: hasSameArgs on ForAll types");
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
            //                               List.nil(), this, type.getMetadata());
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
                // batch TODO); no approximation is done.
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
                TypeKindData::ArrayType { elemtype: t_elem },
                TypeKindData::ArrayType { elemtype: s_elem },
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
                TypeKindData::ArrayType { elemtype } => elemtype.expect("array element type"),
                _ => unreachable!(),
            };
            let elem_is_primitive = ctx.typ(elem).is_primitive();
            let elem_is_object = self.is_same_type(ctx, syms, elem, syms.object_type);
            if elem_is_primitive || elem_is_object {
                panic!("unimplemented: arraySuperType (needs makeIntersectionType)");
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
            let outer1 = match outer {
                Some(outer) => self.subst_visit(ctx, syms, outer, from, to),
                None => panic!("unimplemented: subst of a class type without enclosing type"),
            };
            let typarams1 = self.subst(ctx, syms, &typarams, from, to);
            let unchanged = outer1 == outer.expect("outer") && list_same(&typarams, &typarams1);
            if unchanged {
                return t;
            }
            let tsym = ctx.typ(t).tsym;
            return new_class_type(ctx, Some(outer1), Some(typarams1), tsym);
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
                TypeKindData::ArrayType { elemtype } => elemtype.expect("array element type"),
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
            panic!("unimplemented: subst on ForAll types");
        }
        t
    }

    /// Return the interfaces implemented by this class.
    ///
    /// Java: public List<Type> interfaces(Type t)

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
            let components = ctx.typ(t).get_explicit_components();
            components
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

    /// Java: Subst.visit / StructuralTypeMapping behavior.

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
                    if ctx.typ(st).has_tag(TypeTag::CLASS)
                        || ctx.typ(st).has_tag(TypeTag::TYPEVAR)
                    {
                        if let Some(x) = self.as_super_visit(ctx, syms, st, sym, seen) {
                            return Some(x);
                        }
                    }
                    if (ctx.sym(sym).flags() & crate::code::flags::INTERFACE as i64) != 0 {
                        for i in self.interfaces(ctx, syms, t).iter() {
                            if !ctx.typ(i).has_tag(TypeTag::ERROR) {
                                if let Some(x) = self.as_super_visit(ctx, syms, i, sym, seen) {
                                    return Some(x);
                                }
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
    ///
    /// NOT PORTED: the upstream non-enclosing fallback uses
    /// Symbol.owner.enclClass() (Symbol batch TODO); no approximation is done.
    pub fn as_enclosing_super(
        &self,
        _ctx: &mut CompilerContext,
        _syms: &Symtab,
        _t: TypeId,
        _sym: SymbolId,
    ) -> Option<TypeId> {
        panic!("unimplemented: asEnclosingSuper (needs Symbol.owner.enclClass)");
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
                        match base {
                            Some(base) => Some(self.capture(ctx, syms, base)),
                            None => None,
                        }
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
    /// glb (TODO Types batch): a type that does not need capture conversion is
    /// returned as-is (the early returns of the upstream method), while the part
    /// that needs fresh captured variables panics with an explicit
    /// unimplemented message (no approximation).
    pub fn capture(&self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) -> TypeId {
        if !ctx.typ(t).has_tag(TypeTag::CLASS) {
            return t;
        }
        let no_type = no_type_of(ctx, syms);
        if let Some(encl) = enclosing_of(ctx, t) {
            if encl != no_type {
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
        }
        if is_raw_type(ctx, t) || !is_parameterized_type(ctx, t) {
            return t;
        }
        // The wildcard-capturing loop needs freshTypeVariables (new
        // CapturedType symbols) and glb (TODO Types batch).
        panic!("unimplemented: capture of a parameterized type with wildcards");
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
                panic!("unimplemented: containedBy on UndetVar (not ported)")
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
            TypeKindData::TypeVar { .. } | TypeKindData::CapturedType { .. } => {
                let upper = ctx.typ(t).get_upper_bound().expect("type var upper bound");
                self.is_subtype_no_capture(ctx, syms, upper, s)
            }
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
                let Some(s_tsym) = ctx.typ(s).tsym else {
                    return false;
                };
                let Some(sup) = self.as_super(ctx, syms, t, s_tsym) else {
                    return false;
                };
                if !ctx.typ(sup).has_tag(TypeTag::CLASS) {
                    return self.is_subtype_no_capture(ctx, syms, sup, s);
                }
                if ctx.typ(sup).tsym != ctx.typ(s).tsym {
                    return false;
                }
                if is_parameterized_type(ctx, s) {
                    panic!("unimplemented: containsTypeRecursive (needs adaptSelf/rewriteSupers)");
                }
                let sup_encl = enclosing_of(ctx, sup);
                let s_encl = enclosing_of(ctx, s);
                match (sup_encl, s_encl) {
                    (Some(sup_encl), Some(s_encl)) => {
                        self.is_subtype_no_capture(ctx, syms, sup_encl, s_encl)
                    }
                    (None, None) => true,
                    _ => panic!("unimplemented: isSubtype of class types without enclosing type"),
                }
            }
            TypeKindData::ErrorType { .. } => true,
            TypeKindData::ArrayType { .. } => {
                let t_elem = match &ctx.typ(t).kind {
                    TypeKindData::ArrayType { elemtype } => elemtype.expect("array element"),
                    _ => unreachable!(),
                };
                if ctx.typ(s).has_tag(TypeTag::ARRAY) {
                    let s_elem = match &ctx.typ(s).kind {
                        TypeKindData::ArrayType { elemtype } => elemtype.expect("array element"),
                        _ => unreachable!(),
                    };
                    if ctx.typ(t_elem).is_primitive() {
                        self.is_same_type(ctx, syms, t_elem, s_elem)
                    } else {
                        self.is_subtype_no_capture(ctx, syms, t_elem, s_elem)
                    }
                } else if ctx.typ(s).has_tag(TypeTag::CLASS) {
                    let s_name = ctx.typ(s).tsym.map(|tsym| ctx.sym(tsym).name.clone());
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
                panic!("unimplemented: isSuperType on UndetVar (not ported)")
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
                TypeKindData::ArrayType { elemtype } => elemtype.expect("array element type"),
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
    /// PORTING_RULES: the upstream rank_field caches (Type batch, not ported)
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

    /// Java: public boolean isConvertible(Type t, Type s)
    pub fn is_convertible(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
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
        if ctx.typ(t).has_tag(TypeTag::UNDETVAR) || ctx.typ(s).has_tag(TypeTag::UNDETVAR) {
            panic!("unimplemented: isConvertible on UndetVar (not ported)");
        }
        if t_primitive {
            let boxed = self.boxed_class(ctx, syms, t);
            let boxed_type = ctx.sym(boxed).ty.expect("boxed type");
            return self.is_subtype(ctx, syms, boxed_type, s);
        }
        panic!("unimplemented: isConvertible from a boxed type (needs unboxedType)");
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
    /// emitted here (the lint wiring is TODO with the Attr batch); the boolean
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

    /// Java: public boolean isCastable(Type t, Type s)
    pub fn is_castable(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        t: TypeId,
        s: TypeId,
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
                let boxed = self.boxed_class(ctx, syms, s);
                let boxed_type = ctx.sym(boxed).ty.expect("boxed type");
                return self.is_subtype(ctx, syms, boxed_type, t);
            }
            return false;
        }
        let result = self.is_castable_visit(ctx, syms, t, s);
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
        syms: &mut Symtab,
        t: TypeId,
        s: TypeId,
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
                self.is_castable(ctx, syms, upper, s)
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
                    return self.is_castable(ctx, syms, upper, s);
                }
                self.is_castable(ctx, syms, upper, s)
            }
            TypeKindData::ArrayType { .. } => {
                let tag = ctx.typ(s).get_tag();
                if tag == TypeTag::ERROR || tag == TypeTag::BOT {
                    return true;
                }
                if tag == TypeTag::TYPEVAR {
                    return self.is_castable(ctx, syms, s, t);
                }
                if tag == TypeTag::CLASS {
                    return self.is_subtype(ctx, syms, t, s);
                }
                if tag == TypeTag::ARRAY {
                    let t_elem = match &ctx.typ(t).kind {
                        TypeKindData::ArrayType { elemtype } => elemtype.expect("array element"),
                        _ => unreachable!(),
                    };
                    let s_elem = match &ctx.typ(s).kind {
                        TypeKindData::ArrayType { elemtype } => elemtype.expect("array element"),
                        _ => unreachable!(),
                    };
                    if ctx.typ(t_elem).is_primitive() || ctx.typ(s_elem).is_primitive() {
                        return ctx.typ(t_elem).has_tag(ctx.typ(s_elem).get_tag());
                    }
                    return self.is_castable_visit(ctx, syms, t_elem, s_elem);
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
                    let upper = ctx.typ(s).get_upper_bound().expect("type var upper bound");
                    return self.is_castable(ctx, syms, t, upper);
                }
                if self.is_compound(ctx, t) || self.is_compound(ctx, s) {
                    return self.visit_compound_type(ctx, syms, t, s, false);
                }
                if tag == TypeTag::CLASS || tag == TypeTag::ARRAY {
                    panic!("unimplemented: isCastable between class/array types");
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
    fn visit_compound_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        ct: TypeId,
        s: TypeId,
        reverse: bool,
    ) -> bool {
        for c in self.direct_supertypes(ctx, syms, ct).iter() {
            let ok = if reverse {
                self.is_castable(ctx, syms, s, c)
            } else {
                self.is_castable(ctx, syms, c, s)
            };
            if !ok {
                return false;
            }
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
/// yet (TODO Type batch; registered).
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
/// ported, so the nil default is used (TODO Symbol batch; registered).
fn get_type_arguments(ctx: &CompilerContext, t: TypeId) -> List<TypeId> {
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
/// Type batch; registered).
fn get_parameter_types(ctx: &CompilerContext, t: TypeId) -> List<TypeId> {
    match &ctx.typ(t).kind {
        TypeKindData::MethodType { argtypes, .. } => argtypes.clone(),
        _ => List::nil(),
    }
}

/// Java: Type.getReturnType() (MethodType: restype; other types: null).
///
/// PORTING_RULES: reproduced here (TODO Type batch; registered).
fn get_return_type(ctx: &CompilerContext, t: TypeId) -> TypeId {
    match &ctx.typ(t).kind {
        TypeKindData::MethodType { restype, .. } => restype.expect("method return type"),
        _ => panic!("unimplemented: getReturnType (TODO Type batch)"),
    }

}


// ---------------------------------------------------------------------------
// Types batch 2 helpers.
//
// PORTING_RULES: upstream these helpers are methods of Type.java
// (getThrownTypes/allparams/isParameterized/isRaw/hasErasedSupertypes/
// getSuperclass/getInterfaces/classBound/cvarUpperBound/cvarLowerBound/elemtype)
// or of Types.java (classBound/isUnbounded); they are reproduced here as private
// helpers because type_.rs/symbol.rs have not ported them yet (TODO Type/Symbol
// batch; registered). Symbol.complete() is not ported, so the lazy completion
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
fn allparams_of(ctx: &CompilerContext, t: TypeId) -> List<TypeId> {
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
        TypeKindData::ArrayType { elemtype } => match elemtype {
            Some(elem) => allparams_of(ctx, *elem),
            None => List::nil(),
        },
        _ => List::nil(),
    }
}

/// Java: Type.isParameterized() (ClassType: allparams().tail != null).
fn is_parameterized_type(ctx: &CompilerContext, t: TypeId) -> bool {
    match &ctx.typ(t).kind {
        TypeKindData::ClassType { .. }
        | TypeKindData::ErasedClassType { .. }
        | TypeKindData::UnionClassType { .. }
        | TypeKindData::IntersectionClassType { .. }
        | TypeKindData::ErrorType { .. } => allparams_of(ctx, t).length() > 1,
        _ => false,
    }
}

/// Java: Type.isRaw() (ClassType: this != tsym.type && tsym.type.allparams()
/// non-empty && this.allparams() empty; ArrayType: elemtype.isRaw()).
fn is_raw_type(ctx: &CompilerContext, t: TypeId) -> bool {
    match &ctx.typ(t).kind {
        TypeKindData::ArrayType { elemtype } => match elemtype {
            Some(elem) => is_raw_type(ctx, *elem),
            None => false,
        },
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

/// Java: Type.getThrownTypes() (MethodType: thrown; other types: List.nil()).
///
/// TODO(Types/FunctionDescriptor batch): upstream call sites are Types.java
/// 769/774/799 (FunctionDescriptor; not ported yet). Keep until that batch
/// wires it; do not delete.
#[allow(dead_code)]
fn get_thrown_types(ctx: &CompilerContext, t: TypeId) -> List<TypeId> {
    match &ctx.typ(t).kind {
        TypeKindData::MethodType { thrown, .. } => thrown.clone(),
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
    (ctx.sym(symbol).flags() & crate::code::flags::SEALED as i64) != 0
}

/// Java: Symbol.ClassSymbol.getSuperclass() (Symbol.complete() is not ported: a
/// null supertype_field is returned as None; an interface has no superclass).
fn class_symbol_get_superclass(ctx: &CompilerContext, symbol: SymbolId) -> Option<TypeId> {
    let Some(tsym_type) = ctx.sym(symbol).ty else {
        return None;
    };
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
        TypeKindData::ArrayType { elemtype } => elemtype.expect("array element type"),
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
/// TODO(Types/notSoftSubtype batch): upstream uses are Types.java:149-156 (def)
/// and Types.notSoftSubtype (Types.java:2007-2010; not ported yet). Keep until
/// that batch wires it; do not delete.
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
/// TODO(Types/newTypeVar batch): upstream constructor use (Types.newTypeVar)
/// is not ported yet. Keep until that batch wires it; do not delete.
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
/// TODO(Types capture batch): upstream use is Types.capture (Types.java:4388-4470);
/// the local capture() wildcard path is unimplemented. Keep until that batch
/// wires it; do not delete.
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

/// Java: Type.contains(elem) (Type class method; upstream use is in Attr and
/// other not-yet-ported callers, not in Types.java).
///
/// TODO(Attr/Type.contains batch): not wired yet. Keep until that batch wires
/// it; do not delete.
#[allow(dead_code)]
fn type_contains(ctx: &mut CompilerContext, syms: &Symtab, t: TypeId, elem: TypeId) -> bool {
    if type_equals_ignore_metadata(ctx, t, elem) {
        return true;
    }
    if is_parameterized_type(ctx, t) {
        if let Some(outer) = enclosing_of(ctx, t) {
            if type_contains(ctx, syms, outer, elem) {
                return true;
            }
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
    if let TypeKindData::WildcardType { bound, .. } = &mut ctx.typ_mut(t).kind {
        if *bound != Some(tv) {
            *bound = Some(tv);
        }
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
            },
        ));
        assert_eq!(types.hash_code(&mut ctx, int_array), 5 + 12);
        let int_array_2d = ctx.alloc_type(Type::new(
            TypeTag::ARRAY,
            Some(symtab.array_class),
            TypeKindData::ArrayType {
                elemtype: Some(int_array),
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
            },
        ));
        let string_array2 = ctx.alloc_type(Type::new(
            TypeTag::ARRAY,
            Some(symtab.array_class),
            TypeKindData::ArrayType {
                elemtype: Some(symtab.string_type),
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

}
