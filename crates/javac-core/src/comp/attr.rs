#![allow(clippy::result_large_err)]
// Origin: com/sun/tools/javac/comp/Attr.java @ 982d53c (jdk-17.0.19-ga)
//
// Attr batch A (structure + type/identifier/expression entry points).
//
// §19 registration (batch A):
// Ported in this file: the Attr structure/instance equivalent (new with the
// ported dependencies), the allow* feature switches, ResultInfo (the checked
// subset), attribTree (the visitor dispatch frame), attribExpr (both
// overloads), attribType (both overloads), attribType(node, TypeSymbol),
// attribTypeVariables, attribAnnotationTypes, attribIdent (both overloads) and
// attribIdentAsEnumType, plus the visitor methods for the type trees
// (visitTypeIdent/visitTypeArray/visitTypeApply/visitWildcard/
// visitTypeParameter) and the basic expression leaves (visitLiteral/visitIdent/
// visitParens), with the exact upstream control flow.
//
// NOT PORTED IN THIS BATCH (registered; dependency batches not ported yet, no
// substitute in this file):
// - the CheckContext/CheckMode/checkType pipeline: ported in the dependency
//   batch (Check::check_type/CheckContext/CheckMode/NestedCheckContext/
//   BasicCheckContext; the compatible/report/checkWarner leaves that need
//   Types.isAssignable/convertWarner are registered);
// - Infer.InferenceContext: the context structure/free/asInstType/
//   addFreeTypeListener/notifyChange are ported (comp/infer.rs); the free-type
//   listener recursion inside check() remains registered (Infer batch);
// - DeferredAttr.DeferredAttrContext/AttrMode (DeferredAttr batch): the
//   attribTree deferred-mode/breakTree/matchBindings parts are registered;
// - Resolve (Resolve batch): attribIdent/visitIdent/visitSelect resolution is
//   registered;
// - Annotate/TypeAnnotations (Annotate batch): annotateTypeParameterSecondStage/
//   annotateTypeSecondStage are registered;
// - Analyzer/AttrRecover/ArgumentAttr/MatchBindingsComputer (their batches):
//   attribStat/attribTree glue registered;
// - Types: isAssignable/checkClassType call sites are registered until the
//   Check/Types batches provide them (Types.isConvertible/setBounds/erasure are
//   ported and used where upstream uses them);
// - visitApply/visitSelect/visitExec/... the remaining Attr visitors: registered
//   for Attr batch B/C (the default visitTree keeps the upstream behavior:
//   the tree is not attributed by batch A yet -> explicit unimplemented).
use std::cell::RefCell;
use std::rc::Rc;

use crate::code::deferred_lint_handler::DeferredLintHandler;
use crate::code::kinds::{Kind, KindSelector};
use crate::code::source::Feature;
use crate::code::symbol::CompletionFailure;
use crate::code::symtab::Symtab;
use crate::tree::tree_scanner::{tree_scanner_visitor_delegate, TreeScannerVisitor};
use crate::code::type_::{Type, TypeKindData};
use crate::code::type_tag::TypeTag;
use crate::code::types::Types;
use crate::comp::attr_context::AttrContext;
use crate::comp::check::Check;
use crate::comp::enter::Enter;
use crate::comp::env::Env;
use crate::comp::member_enter::MemberEnter;
use crate::comp::type_enter::TypeEnter;
use crate::comp::type_envs::TypeEnvs;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::tree::jc_tree::{Tag, TreeKind};
use crate::tree::tree_maker::TreeFactory;
use crate::code::dependencies::Dependencies;
use crate::util::jc_diagnostic::Factory;
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::names::Names;

/// Java: Attr.ResultInfo — the checked subset (see the file header for the
/// registered checkContext/checkMode fields).
#[derive(Clone)]
pub struct ResultInfo {
    /// Java: final KindSelector pkind
    pub pkind: KindSelector,
    /// Java: final Type pt (Java Type.noType -> None; ADR 0001).
    pub pt: Option<TypeId>,
    /// Java: final CheckContext checkContext
    pub check_context: Rc<dyn crate::comp::check::CheckContext>,
    /// Java: final CheckMode checkMode
    pub check_mode: crate::comp::check::CheckMode,
    /// Java: the ResultInfo subclass flavor (MethodAttrInfo is an inner subclass
    /// upstream; the port carries the flavor explicitly — registered §9).
    pub kind: ResultInfoKind,
    /// The identity of this ResultInfo (the upstream ArgumentType.speculativeTypes
    /// map is keyed by ResultInfo object identity; Rust needs an explicit id).
    /// §9 registered adapter: unique ids are assigned by new_with_mode, so every
    /// dup(...) call gets a fresh identity like the upstream constructors.
    pub id: u64,
}

/// Java: the ResultInfo subclass flavor (ResultInfo / Attr.MethodAttrInfo —
/// registered §9: Java subclassing is expressed by this enum).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ResultInfoKind {
    /// Java: plain Attr.ResultInfo
    #[default]
    Normal,
    /// Java: Attr.MethodAttrInfo
    MethodAttr,
}

/// Monotonic ResultInfo identity counter (§9 adapter, see ResultInfo::id).
static NEXT_RESULT_INFO_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl ResultInfo {
    /// Java: ResultInfo(KindSelector pkind, Type pt) — the upstream 2-arg
    /// constructor uses chk.basicHandler and CheckMode.NORMAL.
    pub fn new(
        pkind: KindSelector,
        pt: Option<TypeId>,
        check_context: Rc<dyn crate::comp::check::CheckContext>,
    ) -> ResultInfo {
        ResultInfo::new_with_mode(pkind, pt, check_context, crate::comp::check::CheckMode::NORMAL)
    }

    /// Java: protected ResultInfo(KindSelector pkind, Type pt, CheckContext
    /// checkContext, CheckMode checkMode)
    pub fn new_with_mode(
        pkind: KindSelector,
        pt: Option<TypeId>,
        check_context: Rc<dyn crate::comp::check::CheckContext>,
        check_mode: crate::comp::check::CheckMode,
    ) -> ResultInfo {
        ResultInfo {
            pkind,
            pt,
            check_context,
            check_mode,
            kind: ResultInfoKind::Normal,
            id: NEXT_RESULT_INFO_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }
    }

    /// Java: Attr.MethodAttrInfo(CheckContext checkContext) (Attr.java:568-571)
    /// — super(KindSelector.VAL, Infer.anyPoly, checkContext).
    pub fn method_attr_info(
        check_context: Rc<dyn crate::comp::check::CheckContext>,
        any_poly: TypeId,
    ) -> ResultInfo {
        let mut info = ResultInfo::new_with_mode(
            KindSelector::VAL,
            Some(any_poly),
            check_context,
            crate::comp::check::CheckMode::NORMAL,
        );
        info.kind = ResultInfoKind::MethodAttr;
        info
    }

    /// Java: protected boolean needsArgumentAttr(JCTree tree) — the base
    /// returns false; MethodAttrInfo overrides it to true (Attr.java:563-596).
    pub fn needs_argument_attr(&self, _tree: TreeId) -> bool {
        matches!(self.kind, ResultInfoKind::MethodAttr)
    }

    /// Java: protected Type check(final DiagnosticPosition pos, final Type
    /// found) — chk.checkType(pos, found, pt, checkContext)
    pub fn check(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        chk: &Check,
        pos: i32,
        found: TypeId,
    ) -> TypeId {
        let pt = match self.pt {
            Some(pt) => pt,
            None => crate::code::symtab::no_type_id(ctx),
        };
        chk.check_type(ctx, syms, pos, found, pt, &*self.check_context)
    }

    /// Java: public String toString() —
    /// (pt != null ? pt.toString() : "")
    pub fn to_string(&self, ctx: &CompilerContext) -> String {
        match self.pt {
            Some(pt) => type_display(ctx, pt),
            None => String::new(),
        }
    }

    /// Java: protected ResultInfo dup(CheckContext newContext) — ResultInfo.dup
    /// overload used by DeferredAttrNode.process (Attr.java:ResultInfo).
    pub fn dup_check_context(
        &self,
        check_context: Rc<dyn crate::comp::check::CheckContext>,
    ) -> ResultInfo {
        if matches!(self.kind, ResultInfoKind::MethodAttr) {
            // Java: MethodAttrInfo.dup(CheckContext) -> new MethodAttrInfo(newContext)
            let mut info = ResultInfo::new_with_mode(
                self.pkind,
                self.pt,
                check_context,
                self.check_mode,
            );
            info.kind = ResultInfoKind::MethodAttr;
            return info;
        }
        ResultInfo::new_with_mode(self.pkind, self.pt, check_context, self.check_mode)
    }

    /// Java: protected ResultInfo dup(Type newPt)
    pub fn dup(&self, new_pt: Option<TypeId>) -> ResultInfo {
        if matches!(self.kind, ResultInfoKind::MethodAttr) {
            // Java: MethodAttrInfo.dup(Type) throws IllegalStateException.
            panic!("IllegalStateException");
        }
        ResultInfo::new_with_mode(
            self.pkind,
            new_pt,
            Rc::clone(&self.check_context),
            self.check_mode,
        )
    }

    /// Java: protected ResultInfo dup(CheckContext newContext)
    pub fn dup_context(&self, new_context: Rc<dyn crate::comp::check::CheckContext>) -> ResultInfo {
        if matches!(self.kind, ResultInfoKind::MethodAttr) {
            // Java: MethodAttrInfo.dup(CheckContext) -> new MethodAttrInfo(newContext)
            let mut info = ResultInfo::new_with_mode(self.pkind, self.pt, new_context, self.check_mode);
            info.kind = ResultInfoKind::MethodAttr;
            return info;
        }
        ResultInfo::new_with_mode(
            self.pkind,
            self.pt,
            new_context,
            self.check_mode,
        )
    }

    /// Java: protected ResultInfo dup(Type newPt, CheckContext newContext)
    pub fn dup_pt_context(
        &self,
        new_pt: Option<TypeId>,
        new_context: Rc<dyn crate::comp::check::CheckContext>,
    ) -> ResultInfo {
        if matches!(self.kind, ResultInfoKind::MethodAttr) {
            panic!("IllegalStateException");
        }
        ResultInfo::new_with_mode(self.pkind, new_pt, new_context, self.check_mode)
    }

    /// Java: protected ResultInfo dup(Type newPt, CheckContext newContext,
    /// CheckMode newMode)
    pub fn dup_pt_context_mode(
        &self,
        new_pt: Option<TypeId>,
        new_context: Rc<dyn crate::comp::check::CheckContext>,
        new_mode: crate::comp::check::CheckMode,
    ) -> ResultInfo {
        if matches!(self.kind, ResultInfoKind::MethodAttr) {
            panic!("IllegalStateException");
        }
        ResultInfo::new_with_mode(self.pkind, new_pt, new_context, new_mode)
    }

    /// Java: protected ResultInfo dup(CheckMode newMode)
    pub fn dup_mode(&self, new_mode: crate::comp::check::CheckMode) -> ResultInfo {
        if matches!(self.kind, ResultInfoKind::MethodAttr) {
            panic!("IllegalStateException");
        }
        ResultInfo::new_with_mode(
            self.pkind,
            self.pt,
            Rc::clone(&self.check_context),
            new_mode,
        )
    }
}

/// Java: public class Attr extends JCTree.Visitor (the visitor is expressed as
/// the visit_* methods + the TreeKind dispatch in visit()).
pub struct Attr {
    pub names: Rc<Names>,
    pub log: Rc<RefCell<Log>>,
    pub chk: Rc<RefCell<Check>>,
    pub member_enter: Rc<RefCell<MemberEnter>>,
    pub type_enter: Rc<RefCell<TypeEnter>>,
    pub make: Rc<RefCell<crate::tree::tree_maker::TreeMaker>>,
    pub enter: Rc<RefCell<Enter>>,
    pub types: Rc<Types>,
    pub diags: Rc<Factory>,
    pub deferred_lint_handler: Rc<RefCell<DeferredLintHandler>>,
    pub type_envs: Rc<RefCell<TypeEnvs>>,
    pub dependencies: Rc<RefCell<dyn Dependencies>>,
    /// Java: boolean allowPoly
    pub allow_poly: bool,
    /// Java: boolean allowTypeAnnos
    pub allow_type_annos: bool,
    /// Java: boolean allowLambda
    pub allow_lambda: bool,
    /// Java: boolean allowDefaultMethods
    pub allow_default_methods: bool,
    /// Java: boolean allowStaticInterfaceMethods
    pub allow_static_interface_methods: bool,
    /// Java: boolean allowReifiableTypesInInstanceof
    pub allow_reifiable_types_in_instanceof: bool,
    /// Java: private final boolean allowRecords
    pub allow_records: bool,
    /// Java: private final boolean allowPatternSwitch
    pub allow_pattern_switch: bool,
    /// Java: boolean useBeforeDeclarationWarning
    pub use_before_declaration_warning: bool,
    /// Java: String sourceName
    pub source_name: String,
    /// Java: Env<AttrContext> env (the current environment)
    pub env: Option<Rc<Env<AttrContext>>>,
    /// Java: ResultInfo resultInfo (the current result info)
    pub result_info: Option<ResultInfo>,
    /// Java: CheckContext basicHandler (chk.basicHandler; injected explicitly
    /// like the other Check/Attr services, ADR 0001).
    pub basic_handler: Rc<dyn crate::comp::check::CheckContext>,
    /// Java: Type result
    pub result: Option<TypeId>,
    /// Java: private JCTree breakTree = null (Attr.java:448)
    pub break_tree: Option<TreeId>,
    /// Java: the predefined result infos (statInfo/unknownExprInfo/...)
    pub stat_info: ResultInfo,
    pub var_assignment_info: ResultInfo,
    pub unknown_expr_info: ResultInfo,
    pub unknown_type_info: ResultInfo,
    pub unknown_type_expr_info: ResultInfo,
    /// Java: final ResultInfo methodAttrInfo (Attr.java:629)
    pub method_attr_info: ResultInfo,
    /// Java: final ResultInfo recoveryInfo (Attr.java:633)
    pub recovery_info: ResultInfo,
    /// Java: private Map<ClassSymbol, MethodSymbol> clinits (Attr.java:3484)
    pub clinits: RefCell<std::collections::HashMap<SymbolId, SymbolId>>,
    /// Java: final Symtab syms (injected service, ADR 0001)
    pub syms: Rc<RefCell<Symtab>>,
    /// Java: final Infer infer (injected service, ADR 0001)
    pub infer: Rc<crate::comp::infer::Infer>,
}

impl Attr {
    /// Java: protected Attr(Context context) — injected dependencies.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        names: Rc<Names>,
        log: Rc<RefCell<Log>>,
        chk: Rc<RefCell<Check>>,
        member_enter: Rc<RefCell<MemberEnter>>,
        type_enter: Rc<RefCell<TypeEnter>>,
        make: Rc<RefCell<crate::tree::tree_maker::TreeMaker>>,
        enter: Rc<RefCell<Enter>>,
        types: Rc<Types>,
        diags: Rc<Factory>,
        deferred_lint_handler: Rc<RefCell<DeferredLintHandler>>,
        type_envs: Rc<RefCell<TypeEnvs>>,
        dependencies: Rc<RefCell<dyn Dependencies>>,
        basic_handler: Rc<dyn crate::comp::check::CheckContext>,
        syms: Rc<RefCell<Symtab>>,
        infer: Rc<crate::comp::infer::Infer>,
        source: crate::code::source::Source,
        options: &crate::util::options::Options,
        preview: &crate::code::preview::Preview,
    ) -> Attr {
        let allow_poly = Feature::POLY.allowed_in_source(source);
        let allow_type_annos = Feature::TYPE_ANNOTATIONS.allowed_in_source(source);
        let allow_lambda = Feature::LAMBDA.allowed_in_source(source);
        let allow_default_methods = Feature::DEFAULT_METHODS.allowed_in_source(source);
        let allow_static_interface_methods =
            Feature::STATIC_INTERFACE_METHODS.allowed_in_source(source);
        let allow_reifiable_types_in_instanceof =
            Feature::REIFIABLE_TYPES_INSTANCEOF.allowed_in_source(source);
        let allow_records = Feature::RECORDS.allowed_in_source(source);
        let allow_pattern_switch = (preview.is_enabled() || !preview.is_preview(Feature::PATTERN_SWITCH))
            && Feature::PATTERN_SWITCH.allowed_in_source(source);
        let recovery_type = syms.borrow().recovery_type;
        Attr {
            names,
            log,
            chk,
            member_enter,
            type_enter,
            make,
            enter,
            types,
            diags,
            deferred_lint_handler,
            type_envs,
            dependencies,
            allow_poly,
            allow_type_annos,
            allow_lambda,
            allow_default_methods,
            allow_static_interface_methods,
            allow_reifiable_types_in_instanceof,
            allow_records,
            allow_pattern_switch,
            use_before_declaration_warning: options.is_set("useBeforeDeclarationWarning"),
            source_name: source.name().to_string(),
            env: None,
            result_info: None,
            result: None,
            break_tree: None,
            basic_handler: Rc::clone(&basic_handler),
            stat_info: ResultInfo::new(KindSelector::NIL, None, Rc::clone(&basic_handler)),
            var_assignment_info: ResultInfo::new(KindSelector::ASG, None, Rc::clone(&basic_handler)),
            unknown_expr_info: ResultInfo::new(KindSelector::VAL, None, Rc::clone(&basic_handler)),
            unknown_type_info: ResultInfo::new(KindSelector::TYP, None, Rc::clone(&basic_handler)),
            unknown_type_expr_info: ResultInfo::new(KindSelector::VAL_TYP, None, Rc::clone(&basic_handler)),
            method_attr_info: ResultInfo::method_attr_info(Rc::clone(&basic_handler), infer.any_poly),
            recovery_info: ResultInfo::new_with_mode(
                KindSelector::VAL,
                Some(recovery_type),
                Rc::new(RecoveryCheckContext {
                    enclosing: Rc::clone(&basic_handler),
                    deferred_attr_context: crate::comp::deferred_attr::empty_deferred_attr_context(),
                }),
                crate::comp::check::CheckMode::NORMAL,
            ),
            clinits: RefCell::new(std::collections::HashMap::new()),
            syms,
            infer,
        }
    }

    // ------------------------------------------------------------------
    // Java: Attr.ResultInfo / the check pipeline (registered: Check batch).
    // ------------------------------------------------------------------

    /// Java: Type check(final JCTree tree, final Type found, final KindSelector
    /// ownkind, final ResultInfo resultInfo)
    fn check(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        found: TypeId,
        ownkind: KindSelector,
        result_info: &ResultInfo,
    ) -> TypeId {
        let inference_context = Rc::clone(result_info.check_context.inference_context());
        let found_is_error = ctx.typ(found).has_tag(TypeTag::ERROR);
        let pt_is_method_or_forall = result_info
            .pt
            .map(|pt| {
                let tag = ctx.typ(pt).get_tag();
                tag == TypeTag::METHOD || tag == TypeTag::FORALL
            })
            .unwrap_or(false);
        let should_check = !found_is_error && !pt_is_method_or_forall;
        let owntype = if should_check && !ownkind.subset(result_info.pkind) {
            // TODO(KindNames/printing batch): log.error(tree.pos(),
            //     Errors.UnexpectedType(resultInfo.pkind.kindNames(),
            //     ownkind.kindNames())) — the ported DiagnosticArg has no
            //     KindNameArg value constructor yet; registered.
            unimplemented!("TODO(KindNames batch): Attr.check unexpected type")
        } else if self.allow_poly
            && inference_context.free(ctx, syms, &self.types, found)
        {
            // TODO(Infer batch): owntype = shouldCheck ? resultInfo.pt : found;
            //     if (resultInfo.checkMode.installPostInferenceHook())
            //     inferenceContext.addFreeTypeListener(...) { check(...) };
            //     the listener recursion needs the mutable Attr reference;
            //     registered.
            unimplemented!("TODO(Infer batch): Attr.check free type listener")
        } else if should_check {
            let chk = self.chk.borrow();
            result_info.check(ctx, syms, &chk, ctx.tree(tree).pos, found)
        } else {
            found
        };
        if result_info.check_mode.update_tree_type() {
            ctx.tree_mut(tree).ty = Some(owntype);
        }
        owntype
    }

    // ------------------------------------------------------------------
    // Java: attribTree + the derived entry points (Attrs.java:660-850).
    // ------------------------------------------------------------------

    /// Java: Type attribTree(JCTree tree, Env<AttrContext> env, ResultInfo
    /// resultInfo)
    pub fn attrib_tree(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
        result_info: ResultInfo,
    ) -> Result<TypeId, CompletionFailure> {
        let prev_env = self.env.clone();
        let prev_result_info = self.result_info.clone();
        let needs_argument_attr = result_info.needs_argument_attr(tree);
        self.env = Some(Rc::clone(env));
        self.result_info = Some(result_info);
        let result = if needs_argument_attr {
            // TODO(ArgumentAttr batch): result = argumentAttr.attribArg(tree,
            //     env) (Attr.java:674-676); registered.
            unimplemented!("TODO(ArgumentAttr batch): Attr.attribTree argumentAttr.attribArg")
        } else {
            self.visit(ctx, syms, tree)
        };
        // TODO(MatchBindingsComputer batch): matchBindings =
        //     matchBindingsComputer.finishBindings(tree, matchBindings);
        // TODO(DeferredAttr batch): if (tree == breakTree &&
        //     resultInfo.checkContext.deferredAttrContext().mode ==
        //     AttrMode.CHECK) breakTreeFound(copyEnv(env));
        self.env = prev_env;
        self.result_info = prev_result_info;
        let value = match result {
            Ok(value) => value,
            Err(failure) => {
                ctx.tree_mut(tree).ty = Some(syms.err_type);
                return Err(failure);
            }
        };
        self.result = Some(value);
        Ok(value)
    }

    /// Java: Env<AttrContext> copyEnv(Env<AttrContext> env) (Attr.java:699-708)
    pub fn copy_env(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &crate::code::types::Types,
        env: &Rc<Env<AttrContext>>,
    ) -> Rc<Env<AttrContext>> {
        let (tree, info_scope) = {
            let info = env.info.borrow();
            (env.tree, info.scope)
        };
        let new_scope = self.copy_scope(ctx, syms, types, info_scope);
        let new_info = env.info.borrow().dup(new_scope);
        let new_outer = env
            .outer
            .as_ref()
            .map(|outer| self.copy_env(ctx, syms, types, outer));
        env.dup_with_outer(tree, new_info, new_outer)
    }

    /// Java: WriteableScope copyScope(WriteableScope sc) (Attr.java:710-718)
    fn copy_scope(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &crate::code::types::Types,
        scope: Option<crate::ids::ScopeId>,
    ) -> Option<crate::ids::ScopeId> {
        let scope_id = scope?;
        // The upstream collects with prepend and then enters in list order,
        // which reproduces the same shadowing chain; the port reverses the
        // collected order before entering.
        let symbols: Vec<crate::ids::SymbolId> = {
            let scope_ref = ctx.scope(scope_id).clone();
            scope_ref.get_symbols_full(
                scope_id,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
                types,
                syms,
                ctx,
            )
        };
        let owner = ctx.scope(scope_id).owner;
        let new_scope_id = ctx.alloc_scope(crate::code::scope::Scope::new_scope_impl(owner));
        for sym in symbols.into_iter().rev() {
            let (scopes, symbols_arena) = (&mut ctx.scopes, &ctx.symbols);
            scopes
                .get_mut(new_scope_id.index())
                .enter(new_scope_id, sym, symbols_arena);
        }
        Some(new_scope_id)
    }

    /// Java: the tree.accept(this) dispatch (JCTree.Visitor -> TreeKind match;
    /// registered adapter). The visitor methods not ported in batch A are
    /// explicit unimplemented registrations (Attr batches B/C).
    fn visit(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        match &ctx.tree(tree).tag {
            Tag::LITERAL => self.visit_literal(ctx, syms, tree),
            Tag::IDENT => self.visit_ident(ctx, syms, tree),
            Tag::PARENS => self.visit_parens(ctx, syms, tree),
            Tag::TYPEIDENT => self.visit_type_ident(ctx, syms, tree),
            Tag::TYPEARRAY => self.visit_type_array(ctx, syms, tree),
            Tag::TYPEAPPLY => self.visit_type_apply(ctx, syms, tree),
            Tag::WILDCARD => self.visit_wildcard(ctx, syms, tree),
            Tag::TYPEPARAMETER => self.visit_type_parameter(ctx, syms, tree),
            Tag::ERRONEOUS => self.visit_erroneous(ctx, syms, tree),
            Tag::SKIP => self.visit_skip(ctx, syms, tree),
            Tag::EXEC => self.visit_exec(ctx, syms, tree),
            Tag::BREAK => self.visit_break(ctx, syms, tree),
            Tag::CONTINUE => self.visit_continue(ctx, syms, tree),
            Tag::YIELD => self.visit_yield(ctx, syms, tree),
            Tag::RETURN => self.visit_return(ctx, syms, tree),
            Tag::THROW => self.visit_throw(ctx, syms, tree),
            Tag::INDEXED => self.visit_indexed(ctx, syms, tree),
            Tag::ASSIGN => self.visit_assign(ctx, syms, tree),
            Tag::LABELLED => self.visit_labeled(ctx, syms, tree),
            other => {
                let _ = other;
                self.visit_tree(ctx, tree)
            }
        }
    }

    /// Java: public void visitLiteral(JCLiteral tree) (4894-4898)
    fn visit_literal(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let (typetag, value) = match &ctx.tree(tree).kind {
            TreeKind::Literal { typetag, value } => (*typetag, value.clone()),
            _ => panic!("ClassCastException: JCLiteral expected"),
        };
        let lit_type = self.lit_type(syms, typetag);
        let constant = ctx.type_const_type(lit_type, literal_value_to_constant(value));
        let result_info = self.result_info.clone().expect("resultInfo");
        Ok(self.check(ctx, syms, tree, constant, KindSelector::VAL, &result_info))
    }

    /// Java: Type litType(TypeTag tag) —
    /// (tag == CLASS) ? syms.stringType : syms.typeOfTag[tag.ordinal()]
    fn lit_type(&self, syms: &Symtab, tag: TypeTag) -> TypeId {
        if tag == TypeTag::CLASS {
            return syms.string_type;
        }
        syms.type_of_tag[tag as usize].expect("predefined literal type")
    }

    /// Java: public void visitTypeIdent(JCPrimitiveTypeTree tree) (4899-4901)
    fn visit_type_ident(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let typetag = match &ctx.tree(tree).kind {
            TreeKind::PrimitiveTypeTree { typetag } => *typetag,
            _ => panic!("ClassCastException: JCPrimitiveTypeTree expected"),
        };
        let predefined = syms.type_of_tag[typetag as usize].expect("predefined type");
        let result_info = self.result_info.clone().expect("resultInfo");
        Ok(self.check(ctx, syms, tree, predefined, KindSelector::TYP, &result_info))
    }

    /// Java: public void visitTypeArray(JCArrayTypeTree tree) (4903-4908)
    fn visit_type_array(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let elemtype = match &ctx.tree(tree).kind {
            TreeKind::ArrayTypeTree { elemtype } => elemtype.expect("array element type"),
            _ => panic!("ClassCastException: JCArrayTypeTree expected"),
        };
        let env = self.env.clone().expect("env");
        let etype = self.attrib_type_use(ctx, syms, elemtype, &env);
        let ty = Type::new_array_type(etype, syms.array_class);
        let array_type = ctx.alloc_type(ty);
        let result_info = self.result_info.clone().expect("resultInfo");
        Ok(self.check(ctx, syms, tree, array_type, KindSelector::TYP, &result_info))
    }

    /// Java: public void visitTypeApply(JCTypeApply tree) (Attr.java:4913-4960)
    fn visit_type_apply(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        use crate::code::type_::TypeKindData;
        let owntype_error = self
            .types
            .create_error_type_original(ctx, ctx.tree(tree).ty, syms);

        // Attribute functor part of application and make sure it's a class.
        let clazz = type_apply_clazz(ctx, tree).expect("type apply clazz");
        let env = self.env.clone().expect("env");
        let clazz_type = self.attrib_type_use(ctx, syms, clazz, &env);
        let clazz_pos = ctx.tree(clazz).pos;
        let clazztype = self
            .chk
            .borrow()
            .check_class_type(ctx, clazz_pos, clazz_type);

        // Attribute type parameters
        let arguments = type_apply_arguments(ctx, tree);
        let mut actuals = self.attrib_types(ctx, syms, arguments, &env);

        let mut owntype = owntype_error;
        if ctx.typ(clazztype).has_tag(TypeTag::CLASS) {
            let clazz_tsym = ctx.typ(clazztype).tsym.expect("class type symbol");
            let clazz_sym_type = ctx.sym(clazz_tsym).ty.expect("class symbol type");
            let formals = type_arguments(ctx, clazz_sym_type);
            if actuals.is_empty() {
                // diamond
                actuals = formals.clone();
            }
            if actuals.length() == formals.length() {
                let mut a = actuals.clone();
                let mut f = formals.clone();
                let mut actuals_vec: Vec<TypeId> = Vec::new();
                while a.non_empty() {
                    let a_head = *a.head();
                    let f_head = *f.head();
                    actuals_vec.push(self.types.with_type_var(ctx, a_head, f_head));
                    a = a.tail();
                    f = f.tail();
                }
                actuals = List::from_iterable(actuals_vec);
                // Compute the proper generic outer
                let mut clazz_outer = ctx.typ(clazztype).get_enclosing_type();
                if let Some(outer) = clazz_outer
                    .filter(|outer| ctx.typ(*outer).has_tag(TypeTag::CLASS))
                {
                    {
                        let type_in = crate::tree::tree_info::type_in(ctx, clazz);
                        let site = match &ctx.tree(type_in).tag {
                            Tag::IDENT => {
                                let encl_class = env.encl_class.expect("enclClass");
                                let encl_sym = class_tree_symbol(ctx, encl_class).expect("enclClass sym");
                                ctx.sym(encl_sym).ty.expect("enclClass type")
                            }
                            Tag::SELECT => match &ctx.tree(type_in).kind {
                                TreeKind::FieldAccess { selected, .. } => {
                                    let selected = selected.expect("select qualifier");
                                    ctx.tree(selected).ty.expect("selected type")
                                }
                                _ => panic!("ClassCastException: JCFieldAccess expected"),
                            },
                            _ => panic!("AssertionError: {:?}", ctx.tree(type_in).tag),
                        };
                        if site != outer {
                            let mut site = site;
                            if ctx.typ(site).has_tag(TypeTag::CLASS) {
                                let outer_tsym = ctx.typ(outer).tsym.expect("outer type symbol");
                                site = match self.types.as_outer_super(ctx, syms, site, outer_tsym) {
                                    Some(super_type) => super_type,
                                    None => self.types.erasure(ctx, syms, outer),
                                };
                            }
                            clazz_outer = Some(site);
                        }
                    }
                }
                let class_type = TypeKindData::ClassType {
                    outer_field: clazz_outer,
                    typarams_field: Some(actuals),
                    allparams_field: None,
                    supertype_field: None,
                    interfaces_field: None,
                    all_interfaces_field: None,
                };
                owntype = ctx.alloc_type(crate::code::type_::Type::new(
                    TypeTag::CLASS,
                    Some(clazz_tsym),
                    class_type,
                ));
            } else {
                // TODO(Check/Types + printing batch): log.error(tree.pos(),
                //     Errors.WrongNumberTypeArgs(Integer.toString(formals.length())))
                //     / Errors.TypeDoesntTakeParams(clazztype.tsym)); the
                //     DiagnosticArg/SymbolArg mapping needs the printing layer;
                //     registered. Keeps the upstream error type.
                unimplemented!("TODO(Check/Types + printing batch): Attr.visitTypeApply wrong number of type args")
            }
        }
        let result_info = self.result_info.clone().expect("resultInfo");
        Ok(self.check(ctx, syms, tree, owntype, KindSelector::TYP, &result_info))
    }

    /// Java: List<Type> attribTypes(List<JCExpression> trees, Env<AttrContext>
    /// env) (Attr.java:800-810)
    fn attrib_types(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        trees: List<TreeId>,
        env: &Rc<Env<AttrContext>>,
    ) -> List<TypeId> {
        let mut result: List<TypeId> = List::nil();
        let mut current = trees;
        while current.non_empty() {
            let tree = *current.head();
            let ty = self.attrib_type_use(ctx, syms, tree, env);
            result = result.append(ty);
            current = current.tail();
        }
        result
    }

    /// Java: public void visitWildcard(JCWildcard tree) (5099-5109)
    fn visit_wildcard(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let (kind_tree, inner) = match &ctx.tree(tree).kind {
            TreeKind::Wildcard { kind, inner } => (kind.expect("wildcard kind"), *inner),
            _ => panic!("ClassCastException: JCWildcard expected"),
        };
        let bound_kind = match &ctx.tree(kind_tree).kind {
            TreeKind::TypeBoundKind { kind } => *kind,
            _ => panic!("ClassCastException: TypeBoundKind expected"),
        };
        let ty = if bound_kind == crate::code::bound_kind::BoundKind::UNBOUND {
            syms.object_type
        } else {
            let inner = inner.expect("wildcard inner type");
            let env = self.env.clone().expect("env");
            self.attrib_type_use(ctx, syms, inner, &env)
        };
        // TODO(Check batch): chk.checkRefType(tree.pos(), type)
        let wildcard = ctx.alloc_type(crate::code::type_::Type::new_wildcard_type(
            ty,
            bound_kind,
            syms.bound_class,
            None,
        ));
        let result_info = self.result_info.clone().expect("resultInfo");
        Ok(self.check(ctx, syms, tree, wildcard, KindSelector::TYP, &result_info))
    }

    /// Java: public void visitTypeParameter(JCTypeParameter tree) (5017-5029)
    fn visit_type_parameter(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        // TODO(Annotate/Types batch): annotateTypeParameterSecondStage +
        // checkIntersection + TypeVar.setUpperBound; registered.
        unimplemented!("TODO(Annotate/Types batch): Attr.visitTypeParameter")
    }

    /// Java: public void visitIdent(JCIdent tree) (Attr.java:4182-4260)
    fn visit_ident(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let env = self.env.clone().expect("env");
        let name = ident_name(ctx, tree);
        let pt = self.pt();
        let sym: SymbolId;
        if pt
            .map(|pt| {
                let tag = ctx.typ(pt).get_tag();
                tag == TypeTag::METHOD || tag == TypeTag::FORALL
            })
            .unwrap_or(false)
        {
            // TODO(Resolve batch): rs.resolveMethod(tree.pos(), env, tree.name,
            //     pt().getParameterTypes(), pt().getTypeArguments());
            unimplemented!("TODO(Resolve batch): Attr.visitIdent rs.resolveMethod")
        } else if let Some(tree_sym) = ident_symbol(ctx, tree) {
            if ctx.sym(tree_sym).kind != Kind::VAR {
                sym = tree_sym;
            } else {
                unimplemented!("TODO(Resolve batch): Attr.visitIdent rs.resolveIdent (VAR)")
            }
        } else {
            let _ = name;
            unimplemented!("TODO(Resolve batch): Attr.visitIdent rs.resolveIdent")
        }
        set_ident_symbol(ctx, tree, sym);
        let sym_kind = ctx.sym(sym).kind;
        let kind_selector = sym_kind.to_selector();
        let mut sym_env = Rc::clone(&env);
        let encl_class = env.encl_class.expect("enclClass");
        let encl_sym = class_tree_symbol(ctx, encl_class).expect("enclClass sym");
        let encl_owner = ctx.sym(encl_sym).owner.expect("enclClass owner");
        let names = Rc::clone(&self.names);
        let mut no_outer_this_path = false;
        let is_val_mth = kind_selector.contains(KindSelector::VAL);
        if ctx.sym(encl_owner).kind != Kind::PCK
            && is_val_mth
            && ctx.sym(sym).owner.map(|owner| ctx.sym(owner).kind) == Some(Kind::TYP)
            && !name.content_equals(&names._this.to_java_string())
            && !name.content_equals(&names._super.to_java_string())
        {
            while let Some(outer) = sym_env.outer.clone() {
                let encl_class = sym_env.encl_class.expect("symEnv enclClass");
                let encl_sym = class_tree_symbol(ctx, encl_class).expect("symEnv enclClass sym");
                // TODO(Symbol/Types batch): sym.isMemberOf(symEnv.enclClass.sym,
                //     types) needs Symbol.isSubClass/isInheritedIn/hiddenIn.
                let is_member = symbol_is_member_of(ctx, &self.types, sym, encl_sym);
                if is_member {
                    break;
                }
                let flags_field = ctx.sym(encl_sym).flags_field;
                if (flags_field & crate::code::flags::NOOUTERTHIS as i64) != 0 {
                    no_outer_this_path = false;
                }
                sym_env = outer;
            }
        }
        if sym_kind == Kind::VAR {
            unimplemented!("TODO(Resolve batch): Attr.visitIdent variable checks")
        }
        let sym_owner = ctx.sym(sym).owner;
        let sym_owner_is_typ = sym_owner.map(|owner| ctx.sym(owner).kind) == Some(Kind::TYP);
        let sym_is_static = (ctx.sym(sym).flags_field & crate::code::flags::STATIC as i64) != 0;
        if (sym_env.info.borrow().is_self_call || no_outer_this_path)
            && is_val_mth
            && sym_owner_is_typ
            && !sym_is_static
        {
            unimplemented!("TODO(Check batch): Attr.visitIdent early ref error")
        }
        let env1 = Rc::clone(&env);
        if sym_kind != Kind::ERR
            && sym_kind != Kind::TYP
            && sym_owner.is_some()
            && sym_owner != Some(encl_sym)
        {
            unimplemented!("TODO(Resolve batch): Attr.visitIdent rs.isAccessible")
        }
        if env.info.borrow().is_serializable {
            unimplemented!("TODO(Check batch): Attr.visitIdent serializable access")
        }
        let site = ctx.sym(encl_sym).ty.expect("enclClass type");
        let _ = &env1;
        let result_info = self.result_info.clone().expect("resultInfo");
        Ok(self.check_id(ctx, syms, tree, site, sym, &env, &result_info))
    }
    /// Java: Type checkId(JCTree tree, Type site, Symbol sym,
    /// Env<AttrContext> env, ResultInfo resultInfo) (Attr.java:4490-4496)
    #[allow(clippy::too_many_arguments)]
    fn check_id(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        site: TypeId,
        sym: SymbolId,
        env: &Rc<Env<AttrContext>>,
        result_info: &ResultInfo,
    ) -> TypeId {
        let pt = result_info.pt;
        let is_forall_or_method = pt
            .map(|pt| {
                let tag = ctx.typ(pt).get_tag();
                tag == TypeTag::FORALL || tag == TypeTag::METHOD
            })
            .unwrap_or(false);
        if is_forall_or_method {
            return self.check_method_id_internal(ctx, syms, tree, site, sym, env, result_info);
        }
        self.check_id_internal(ctx, syms, tree, site, sym, pt, env, result_info)
    }

    /// Java: Type checkMethodIdInternal(...) (Attr.java:4498-4518)
    #[allow(clippy::too_many_arguments)]
    fn check_method_id_internal(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _tree: TreeId,
        _site: TypeId,
        _sym: SymbolId,
        _env: &Rc<Env<AttrContext>>,
        _result_info: &ResultInfo,
    ) -> TypeId {
        // TODO(AttrRecover/Resolve batch): poly method recovery + checkIdInternal
        // with the method prototype; registered.
        unimplemented!("TODO(AttrRecover/Resolve batch): Attr.checkMethodIdInternal")
    }

    /// Java: Type checkIdInternal(JCTree tree, Type site, Symbol sym, Type pt,
    /// Env<AttrContext> env, ResultInfo resultInfo) (Attr.java:4520-4620)
    #[allow(clippy::too_many_arguments)]
    fn check_id_internal(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        site: TypeId,
        sym: SymbolId,
        pt: Option<TypeId>,
        env: &Rc<Env<AttrContext>>,
        result_info: &ResultInfo,
    ) -> TypeId {
        let pt_is_erroneous = pt
            .map(|pt| ctx.typ(pt).has_tag(TypeTag::ERROR))
            .unwrap_or(false);
        if pt_is_erroneous {
            return self.types.create_error_type_original(ctx, Some(site), syms);
        }
        let owntype: TypeId;
        match ctx.sym(sym).kind {
            Kind::TYP => {
                let mut own = ctx.sym(sym).ty.expect("type symbol type");
                if ctx.typ(own).has_tag(TypeTag::CLASS) {
                    // TODO(Lint/Resolve batch): chk.checkForBadAuxiliaryClassAccess(
                    //     tree.pos(), env, (ClassSymbol)sym);
                    let own_outer = ctx.typ(own).get_enclosing_type();
                    let sym_type = ctx.sym(sym).ty.expect("symbol type");
                    let sym_type_args = type_arguments(ctx, sym_type);
                    if !sym_type_args.is_empty() {
                        own = self.types.erasure(ctx, syms, own);
                    } else if let Some(own_outer) = own_outer.filter(|own_outer| {
                        ctx.typ(*own_outer).has_tag(TypeTag::CLASS) && site != *own_outer
                    }) {
                        {
                            let mut norm_outer = site;
                            if ctx.typ(norm_outer).has_tag(TypeTag::CLASS) {
                                // TODO(Types batch): types.asEnclosingSuper(site,
                                //     ownOuter.tsym) is not ported.
                                unimplemented!(
                                    "TODO(Types batch): Attr.checkIdInternal types.asEnclosingSuper"
                                )
                            }
                            if ctx.typ(norm_outer).has_tag(TypeTag::ERROR) {
                                norm_outer = self.types.erasure(ctx, syms, own_outer);
                            }
                            if norm_outer != own_outer {
                                own = ctx.alloc_type(crate::code::type_::Type::new(
                                    TypeTag::CLASS,
                                    ctx.typ(own).tsym,
                                    crate::code::type_::TypeKindData::ClassType {
                                        outer_field: Some(norm_outer),
                                        typarams_field: Some(List::nil()),
                                        allparams_field: None,
                                        supertype_field: None,
                                        interfaces_field: None,
                                        all_interfaces_field: None,
                                    },
                                ));
                            }
                        }
                    }
                }
                owntype = own;
            }
            Kind::VAR => {
                unimplemented!("TODO(Resolve/Check batch): Attr.checkIdInternal VAR")
            }
            Kind::MTH => {
                unimplemented!("TODO(Resolve/Check batch): Attr.checkIdInternal MTH")
            }
            Kind::PCK | Kind::ERR => {
                owntype = ctx.sym(sym).ty.expect("symbol type");
            }
            other => panic!("AssertionError: unexpected kind: {:?} in tree", other),
        }
        // TODO(Check batch): chk.checkDeprecated/checkSunAPI/checkProfile/
        //     checkPreview(pos, ...) are not ported yet (registered).
        let _ = (env, ctx.sym(sym).name.clone());
        self.check(
            ctx,
            syms,
            tree,
            owntype,
            ctx.sym(sym).kind.to_selector(),
            result_info,
        )
    }
    /// Java: public void visitParens(JCParens tree) (Attr.java:3922-3928)
    fn visit_parens(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Parens { expr } => expr.expect("parens expression"),
            _ => panic!("ClassCastException: JCParens expected"),
        };
        let env = self.env.clone().expect("env");
        let result_info = self.result_info.clone().expect("resultInfo");
        let owntype = self.attrib_tree_with(ctx, syms, expr, &env, result_info.clone());
        let pkind = result_info.pkind;
        let owntype = self.check(ctx, syms, tree, owntype, pkind, &result_info);
        let symbol = crate::tree::tree_info::symbol(ctx, tree);
        if let Some(symbol) = symbol {
            let kind = ctx.sym(symbol).kind;
            if kind.matches(crate::code::kinds::KindSelector::TYP_PCK)
                && kind != crate::code::kinds::Kind::ERR
            {
                self.attr_error(
                    ctx.tree(tree).pos,
                    crate::resources::compiler_properties::errors::illegal_parenthesized_expression(),
                );
            }
        }
        Ok(owntype)
    }

    /// Java: public void visitErroneous(JCErroneous tree) (5123-5131)
    fn visit_erroneous(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let errs = match &ctx.tree(tree).kind {
            TreeKind::Erroneous { errs } => errs.clone(),
            _ => panic!("ClassCastException: JCErroneous expected"),
        };
        let mut current = errs;
        while current.non_empty() {
            let err = *current.head();
            let env = self.env.clone().expect("env");
            let error_info = ResultInfo::new(KindSelector::ERR, self.pt(), Rc::clone(&self.basic_handler));
            let _ = self.attrib_tree_with(ctx, syms, err, &env, error_info);
            current = current.tail();
        }
        ctx.tree_mut(tree).ty = Some(syms.err_type);
        Ok(syms.err_type)
    }

    // ------------------------------------------------------------------
    // Attr batch B (first group): statement visitors + the entry points that
    // do not need the Analyzer/Flow machinery (Attr.java:1420-1600, 1871-2470,
    // 3922-4182). Methods whose upstream body needs a not-yet-ported service are
    // registered with unimplemented! + TODO at the exact call site.
    // ------------------------------------------------------------------

    /// Java: log.error(DiagnosticPosition, JCDiagnostic.Error) helper — the
    /// ported DiagnosticArg mapping and report call (ADR 0002 P4 placeholders).
    fn attr_error(&self, pos: i32, info: crate::resources::diagnostic_contract::ErrorInfo) {
        use crate::util::jc_diagnostic::DiagnosticType;
        let args: Vec<crate::util::jc_diagnostic::DiagnosticArg> = info
            .info()
            .args
            .iter()
            .map(|arg| {
                std::rc::Rc::new(javac_strings::JavaString::from(arg.0.as_str()))
                    as crate::util::jc_diagnostic::DiagnosticArg
            })
            .collect();
        let source = self.log.borrow().current_source();
        let diagnostic = self.diags.create(
            DiagnosticType::ERROR,
            source,
            Some(std::rc::Rc::new(
                crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
            )),
            info.info().code,
            args,
        );
        self.log.borrow_mut().report(diagnostic);
    }

    /// Display placeholder for a Type diagnostic argument (ADR 0002 P4).
    fn type_arg_placeholder(
        ctx: &CompilerContext,
        ty: TypeId,
    ) -> crate::resources::diagnostic_contract::TypeArg {
        let text = ctx
            .typ(ty)
            .tsym
            .map(|tsym| crate::comp::resolve::symbol_display_name(ctx, tsym))
            .unwrap_or_else(|| format!("{:?}", ctx.typ(ty).get_tag()));
        crate::resources::diagnostic_contract::TypeArg::placeholder(text)
    }

    /// Java: public Type attribStat(JCTree tree, Env<AttrContext> env)
    /// (Attr.java:749-756)
    pub fn attrib_stat(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _tree: TreeId,
        _env: &Rc<Env<AttrContext>>,
    ) -> TypeId {
        // Java: Env<AttrContext> analyzeEnv = analyzer.copyEnvIfNeeded(tree, env);
        //       Type result = attribTree(tree, env, statInfo);
        //       analyzer.analyzeIfNeeded(tree, analyzeEnv);
        //       attrRecover.doRecovery();
        // The Analyzer (flow analysis) and AttrRecover are registered with the
        // Flow/AttrRecover batch.
        unimplemented!(
            "TODO(Analyzer/AttrRecover batch): Attr.attribStat (Attr.java:749-756)"
        )
    }

    /// Java: <T extends JCTree> void attribStats(List<T> trees,
    /// Env<AttrContext> env) (Attr.java:770-774)
    pub fn attrib_stats(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        trees: List<TreeId>,
        env: &Rc<Env<AttrContext>>,
    ) {
        let mut current = trees;
        while current.non_empty() {
            let tree = *current.head();
            self.attrib_stat(ctx, syms, tree, env);
            current = current.tail();
        }
    }

    /// Java: List<Type> attribExprs(List<JCExpression> trees,
    /// Env<AttrContext> env, Type pt) (Attr.java:759-768)
    pub fn attrib_exprs(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        trees: List<TreeId>,
        env: &Rc<Env<AttrContext>>,
        pt: Option<TypeId>,
    ) -> List<TypeId> {
        let mut result: List<TypeId> = List::nil();
        let mut current = trees;
        while current.non_empty() {
            let tree = *current.head();
            result = result.append(self.attrib_expr_with_type(ctx, syms, tree, env, pt));
            current = current.tail();
        }
        result
    }

    /// Java: List<Type> attribAnyTypes(List<JCExpression> trees,
    /// Env<AttrContext> env) (Attr.java:790-798)
    pub fn attrib_any_types(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        trees: List<TreeId>,
        env: &Rc<Env<AttrContext>>,
    ) -> List<TypeId> {
        let mut result: List<TypeId> = List::nil();
        let mut current = trees;
        while current.non_empty() {
            let tree = *current.head();
            result = result.append(self.attrib_type(ctx, syms, tree, env));
            current = current.tail();
        }
        result
    }

    /// Java: public void visitSkip(JCSkip tree) (Attr.java:1420-1422)
    fn visit_skip(
        &mut self,
        ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        // Java: result = null; the ported visitor returns Type.noType for the
        // null statement result (§9 adapter, registered).
        Ok(self.no_type(ctx))
    }

    /// Java: public void visitExec(JCExpressionStatement tree)
    /// (Attr.java:2282-2288)
    fn visit_exec(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::ExpressionStatement { expr } => expr.expect("exec expression"),
            _ => panic!("ClassCastException: JCExpressionStatement expected"),
        };
        let env = self.env.clone().expect("env");
        // Java: Env<AttrContext> localEnv = env.dup(tree); (shares env.info)
        let local_env = env.dup_shared(Some(tree));
        self.attrib_expr(ctx, syms, expr, &local_env);
        Ok(self.no_type(ctx))
    }

    /// Java: public void visitBreak(JCBreak tree) (Attr.java:2290-2294)
    fn visit_break(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let label = match &ctx.tree(tree).kind {
            TreeKind::Break { label, .. } => label.clone(),
            _ => panic!("ClassCastException: JCBreak expected"),
        };
        let env = self.env.clone().expect("env");
        let pos = ctx.tree(tree).pos;
        let target = self.find_jump_target(ctx, syms, pos, Tag::BREAK, label, &env);
        if let TreeKind::Break { target: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = target;
        }
        Ok(self.no_type(ctx))
    }

    /// Java: public void visitContinue(JCContinue tree) (Attr.java:2308-2312)
    fn visit_continue(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let label = match &ctx.tree(tree).kind {
            TreeKind::Continue { label, .. } => label.clone(),
            _ => panic!("ClassCastException: JCContinue expected"),
        };
        let env = self.env.clone().expect("env");
        let pos = ctx.tree(tree).pos;
        let target = self.find_jump_target(ctx, syms, pos, Tag::CONTINUE, label, &env);
        if let TreeKind::Continue { target: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = target;
        }
        Ok(self.no_type(ctx))
    }

    /// Java: public void visitYield(JCYield tree) (Attr.java:2295-2307)
    fn visit_yield(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, .. } => *value,
            _ => panic!("ClassCastException: JCYield expected"),
        };
        let env = self.env.clone().expect("env");
        let yield_result = env.info.borrow().yield_result.clone();
        if let Some(result_info) = yield_result {
            if let Some(value) = value {
                self.attrib_tree_with(ctx, syms, value, &env, result_info);
            }
            let pos = ctx.tree(tree).pos;
            let target = self.find_jump_target(ctx, syms, pos, Tag::YIELD, None, &env);
            if let TreeKind::Yield { target: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = target;
            }
        } else {
            let pos = ctx.tree(tree).pos;
            let is_parens = value
                .map(|value| ctx.tree(value).has_tag(Tag::PARENS))
                .unwrap_or(false);
            if is_parens {
                self.attr_error(
                    pos,
                    crate::resources::compiler_properties::errors::no_switch_expression_qualify(),
                );
            } else {
                self.attr_error(
                    pos,
                    crate::resources::compiler_properties::errors::no_switch_expression(),
                );
            }
            if let Some(value) = value {
                let info = self.unknown_expr_info.clone();
                self.attrib_tree_with(ctx, syms, value, &env, info);
            }
        }
        Ok(self.no_type(ctx))
    }

    /// Java: public void visitLabelled(JCLabeledStatement tree)
    /// (Attr.java:1581-1597)
    fn visit_labeled(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let (label, body) = match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement { label, body } => (label.clone(), *body),
            _ => panic!("ClassCastException: JCLabeledStatement expected"),
        };
        let env = self.env.clone().expect("env");
        // Check that label is not used in an enclosing statement.
        let mut env1 = Rc::clone(&env);
        while let Some(env1_tree) = env1.tree {
            if ctx.tree(env1_tree).has_tag(Tag::CLASSDEF) {
                break;
            }
            if ctx.tree(env1_tree).has_tag(Tag::LABELLED) {
                let outer_label = match &ctx.tree(env1_tree).kind {
                    TreeKind::LabeledStatement { label, .. } => label.clone(),
                    _ => panic!("ClassCastException: JCLabeledStatement expected"),
                };
                if outer_label == label {
                    self.attr_error(
                        ctx.tree(tree).pos,
                        crate::resources::compiler_properties::errors::label_already_in_use(
                            label.clone(),
                        ),
                    );
                    break;
                }
            }
            match env1.next.clone() {
                Some(next) => env1 = next,
                None => break,
            }
        }
        let body_env = env.dup_shared(Some(tree));
        if let Some(body) = body {
            self.attrib_stat(ctx, syms, body, &body_env);
        }
        Ok(self.no_type(ctx))
    }

    /// Java: public void visitReturn(JCReturn tree) (Attr.java:2414-2443)
    #[allow(clippy::unnecessary_unwrap, clippy::collapsible_if)]
    fn visit_return(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Return { expr } => *expr,
            _ => panic!("ClassCastException: JCReturn expected"),
        };
        let env = self.env.clone().expect("env");
        let pos = ctx.tree(tree).pos;
        let return_result = env.info.borrow().return_result.clone();
        if return_result.is_none() {
            self.attr_error(
                pos,
                crate::resources::compiler_properties::errors::ret_outside_meth(),
            );
        } else if env.info.borrow().yield_result.is_some() {
            self.attr_error(
                pos,
                crate::resources::compiler_properties::errors::return_outside_switch_expression(),
            );
        } else if !env.info.borrow().is_lambda
            && !env.info.borrow().is_new_class
            && env
                .encl_method
                .map(|encl_method| {
                    crate::tree::tree_info::is_compact_constructor(ctx, encl_method)
                })
                .unwrap_or(false)
        {
            // Java: log.error(env.enclMethod,
            //     Errors.InvalidCanonicalConstructorInRecord(Fragments.Compact,
            //         env.enclMethod.sym.name,
            //         Fragments.CanonicalCantHaveReturnStatement));
            // Registered with the Records batch (Fragments.Compact/Canonical
            // fragment helpers need the argument plumbing).
            unimplemented!(
                "TODO(Records batch): Attr.visitReturn invalid canonical constructor (Attr.java:2425-2428)"
            )
        } else {
            let result_info = return_result.expect("returnResult");
            if let Some(expr) = expr {
                let pt = result_info.pt;
                if pt
                    .map(|pt| ctx.typ(pt).has_tag(TypeTag::VOID))
                    .unwrap_or(false)
                {
                    let fragment = self
                        .diags
                        .fragment(
                            crate::resources::compiler_properties::fragments::unexpected_ret_val()
                                .into(),
                        );
                    result_info
                        .check_context
                        .report(ctx, ctx.tree(expr).pos, &fragment);
                }
                self.attrib_tree_with(ctx, syms, expr, &env, result_info);
            } else if let Some(pt) = result_info.pt {
                if !ctx.typ(pt).has_tag(TypeTag::VOID) && !ctx.typ(pt).has_tag(TypeTag::NONE) {
                    let fragment = self
                        .diags
                        .fragment(
                            crate::resources::compiler_properties::fragments::missing_ret_val(
                                Self::type_arg_placeholder(ctx, pt),
                            )
                            .into(),
                        );
                    result_info.check_context.report(ctx, pos, &fragment);
                }
            }
        }
        Ok(self.no_type(ctx))
    }

    /// Java: public void visitThrow(JCThrow tree) (Attr.java:2445-2451)
    fn visit_throw(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Throw { expr } => expr.expect("throw expression"),
            _ => panic!("ClassCastException: JCThrow expected"),
        };
        let env = self.env.clone().expect("env");
        let pt = if self.allow_poly {
            None
        } else {
            Some(syms.throwable_type)
        };
        let owntype = self.attrib_expr_with_type(ctx, syms, expr, &env, pt);
        if self.allow_poly {
            let pos = ctx.tree(tree).pos;
            let chk = self.chk.borrow();
            chk.check_type(
                ctx,
                syms,
                pos,
                owntype,
                syms.throwable_type,
                &*self.basic_handler,
            );
        }
        Ok(self.no_type(ctx))
    }

    /// Java: public void visitIndexed(JCArrayAccess tree)
    /// (Attr.java:4169-4180)
    fn visit_indexed(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let (indexed, index) = match &ctx.tree(tree).kind {
            TreeKind::ArrayAccess { indexed, index } => (
                indexed.expect("array access indexed expression"),
                index.expect("array access index"),
            ),
            _ => panic!("ClassCastException: JCArrayAccess expected"),
        };
        let env = self.env.clone().expect("env");
        let mut owntype = self
            .types
            .create_error_type_original(ctx, ctx.tree(tree).ty, syms);
        let atype = self.attrib_expr(ctx, syms, indexed, &env);
        self.attrib_expr_with_type(ctx, syms, index, &env, Some(syms.int_type));
        if matches!(
            ctx.typ(atype).kind,
            crate::code::type_::TypeKindData::ArrayType { .. }
        ) {
            owntype = self.types.elemtype(ctx, syms, atype);
        } else if !ctx.typ(atype).has_tag(TypeTag::ERROR) {
            self.attr_error(
                ctx.tree(tree).pos,
                crate::resources::compiler_properties::errors::array_req_but_found(
                    Self::type_arg_placeholder(ctx, atype),
                ),
            );
        }
        let pkind = self
            .result_info
            .as_ref()
            .expect("resultInfo")
            .pkind;
        if !pkind.contains(crate::code::kinds::KindSelector::VAL) {
            owntype = self.types.capture(ctx, syms, owntype);
        }
        let result_info = self.result_info.clone().expect("resultInfo");
        Ok(self.check(ctx, syms, tree, owntype, crate::code::kinds::KindSelector::VAR, &result_info))
    }

    /// Java: public void visitAssign(JCAssign tree) (Attr.java:3930-3936)
    fn visit_assign(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Assign { lhs, rhs } => (
                lhs.expect("assignment lhs"),
                rhs.expect("assignment rhs"),
            ),
            _ => panic!("ClassCastException: JCAssign expected"),
        };
        let env = self.env.clone().expect("env");
        let lhs_env = env.dup_shared(Some(tree));
        let assignment_info = self.var_assignment_info.clone();
        let owntype = self.attrib_tree_with(ctx, syms, lhs, &lhs_env, assignment_info);
        let captured = self.types.capture(ctx, syms, owntype);
        self.attrib_expr_with_type(ctx, syms, rhs, &env, Some(owntype));
        let result_info = self.result_info.clone().expect("resultInfo");
        Ok(self.check(
            ctx,
            syms,
            tree,
            captured,
            crate::code::kinds::KindSelector::VAL,
            &result_info,
        ))
    }

    /// Java: private JCTree findJumpTarget(DiagnosticPosition pos,
    /// JCTree.Tag tag, Name label, Env<AttrContext> env)
    /// (Attr.java:2317-2328)
    fn find_jump_target(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        pos: i32,
        tag: Tag,
        label: Option<crate::util::name::Name>,
        env: &Rc<Env<AttrContext>>,
    ) -> Option<TreeId> {
        let (target, error) = self.find_jump_target_no_error(ctx, syms, tag, &label, env);
        if let Some(error) = error {
            self.attr_error(pos, error);
        }
        target
    }

    /// Java: private Pair<JCTree, JCDiagnostic.Error>
    /// findJumpTargetNoError(Tag tag, Name label, Env<AttrContext> env)
    /// (Attr.java:2339-2408)
    fn find_jump_target_no_error(
        &self,
        ctx: &CompilerContext,
        _syms: &mut Symtab,
        tag: Tag,
        label: &Option<crate::util::name::Name>,
        env: &Rc<Env<AttrContext>>,
    ) -> (
        Option<TreeId>,
        Option<crate::resources::diagnostic_contract::ErrorInfo>,
    ) {
        use crate::resources::compiler_properties::errors;
        let mut env1 = Rc::clone(env);
        let mut pending_error: Option<crate::resources::diagnostic_contract::ErrorInfo> = None;
        while let Some(env1_tree) = env1.tree {
            match ctx.tree(env1_tree).tag {
                Tag::LABELLED => {
                    let (labelled_label, labelled_body) = match &ctx.tree(env1_tree).kind {
                        TreeKind::LabeledStatement { label, body } => {
                            (label.clone(), *body)
                        }
                        _ => panic!("ClassCastException: JCLabeledStatement expected"),
                    };
                    if label.as_ref() == Some(&labelled_label) {
                        if tag == Tag::CONTINUE {
                            let is_loop = labelled_body
                                .map(|body| {
                                    matches!(
                                        ctx.tree(body).tag,
                                        Tag::DOLOOP
                                            | Tag::WHILELOOP
                                            | Tag::FORLOOP
                                            | Tag::FOREACHLOOP
                                    )
                                })
                                .unwrap_or(false);
                            if !is_loop {
                                pending_error = Some(errors::not_loop_label(labelled_label));
                            }
                            return (
                                Some(crate::tree::tree_info::referenced_statement(
                                    ctx, env1_tree,
                                )),
                                pending_error,
                            );
                        } else {
                            return (Some(env1_tree), pending_error);
                        }
                    }
                }
                Tag::DOLOOP | Tag::WHILELOOP | Tag::FORLOOP | Tag::FOREACHLOOP => {
                    if label.is_none() {
                        return (Some(env1_tree), pending_error);
                    }
                }
                Tag::SWITCH => {
                    if label.is_none() && tag == Tag::BREAK {
                        return (Some(env1_tree), None);
                    }
                }
                Tag::SWITCH_EXPRESSION => {
                    if tag == Tag::YIELD {
                        return (Some(env1_tree), None);
                    } else if tag == Tag::BREAK {
                        pending_error = Some(errors::break_outside_switch_expression());
                    } else {
                        pending_error = Some(errors::continue_outside_switch_expression());
                    }
                }
                Tag::LAMBDA | Tag::METHODDEF | Tag::CLASSDEF => break,
                _ => {}
            }
            match env1.next.clone() {
                Some(next) => env1 = next,
                None => break,
            }
        }
        if let Some(label) = label {
            return (None, Some(errors::undef_label(label.clone())));
        }
        if let Some(error) = pending_error {
            return (None, Some(error));
        }
        if tag == Tag::CONTINUE {
            (None, Some(errors::cont_outside_loop()))
        } else {
            (None, Some(errors::break_outside_switch_loop()))
        }
    }

    /// Java: public void visitTree(JCTree tree) (default: assert error)
    fn visit_tree(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) -> Result<TypeId, CompletionFailure> {
        unimplemented!("TODO(Attr batch B/C): Attr.visitTree default")
    }

    // ------------------------------------------------------------------
    // Derived entry points (Attr.java:722-850).
    // ------------------------------------------------------------------

    /// Java: public Type attribExpr(JCTree tree, Env<AttrContext> env)
    pub fn attrib_expr(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) -> TypeId {
        let info = self.unknown_expr_info.clone();
        self.attrib_tree_use(ctx, syms, tree, env, info)
    }

    /// Java: public Type attribExpr(JCTree tree, Env<AttrContext> env, Type pt)
    pub fn attrib_expr_with_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
        pt: Option<TypeId>,
    ) -> TypeId {
        let pt = match pt {
            Some(pt) if !ctx.typ(pt).has_tag(TypeTag::ERROR) => pt,
            _ => {
                // Java: Type.noType
                self.no_type(ctx)
            }
        };
        let info = ResultInfo::new(KindSelector::VAL, Some(pt), Rc::clone(&self.basic_handler));
        self.attrib_tree_use(ctx, syms, tree, env, info)
    }

    /// Java: public Type attribType(JCTree tree, Env<AttrContext> env)
    pub fn attrib_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) -> TypeId {
        let info = self.unknown_type_info.clone();
        self.attrib_tree_use(ctx, syms, tree, env, info)
    }

    /// Java: Type attribType(JCTree tree, Env<AttrContext> env, Type pt)
    pub fn attrib_type_with(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
        pt: Option<TypeId>,
    ) -> TypeId {
        let info = ResultInfo::new(KindSelector::TYP, pt, Rc::clone(&self.basic_handler));
        self.attrib_tree_use(ctx, syms, tree, env, info)
    }

    /// Java: public Type attribType(JCTree node, TypeSymbol sym)
    pub fn attrib_type_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        node: TreeId,
        sym: SymbolId,
    ) -> TypeId {
        let env = self.type_envs.borrow().get(sym).expect("typeEnvs entry");
        // Java: Env<AttrContext> localEnv = env.dup(node, env.info.dup());
        // The local AttrContext is a copy: modifications inside the local env
        // must not leak into the class env (dup_shared would share info).
        let info_copy = {
            let info = env.info.borrow();
            info.dup(info.scope)
        };
        let local_env = env.dup(Some(node), info_copy);
        let info = self.unknown_type_info.clone();
        self.attrib_tree_use(ctx, syms, node, &local_env, info)
    }

    /// Java: public Type attribImportQualifier(JCImport tree, Env<AttrContext>
    /// env)
    pub fn attrib_import_qualifier(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) -> TypeId {
        let qualid = import_qualid(ctx, tree).expect("import qualid");
        let selected = match &ctx.tree(qualid).kind {
            TreeKind::FieldAccess { selected, .. } => selected.expect("import selected"),
            _ => panic!("ClassCastException: JCFieldAccess expected"),
        };
        let static_import = import_static_import(ctx, tree);
        let pkind = if static_import {
            KindSelector::TYP
        } else {
            KindSelector::TYP_PCK
        };
        let info = ResultInfo::new(pkind, Some(self.no_type(ctx)), Rc::clone(&self.basic_handler));
        self.attrib_tree_use(ctx, syms, selected, env, info)
    }

    /// Java: void attribAnnotationTypes(List<JCAnnotation> annotations,
    /// Env<AttrContext> env) (838-844)
    pub fn attrib_annotation_types(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        annotations: List<TreeId>,
        env: &Rc<Env<AttrContext>>,
    ) {
        let mut current = annotations;
        while current.non_empty() {
            let annotation = *current.head();
            let annotation_type = match &ctx.tree(annotation).kind {
                TreeKind::Annotation { annotation_type, .. } => {
                    annotation_type.expect("annotation type")
                }
                _ => panic!("ClassCastException: JCAnnotation expected"),
            };
            self.attrib_type_use(ctx, syms, annotation_type, env);
            current = current.tail();
        }
    }

    /// Java: void attribTypeVariables(List<JCTypeParameter> typarams,
    /// Env<AttrContext> env, boolean checkCyclic) (811-836)
    pub fn attrib_type_variables(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        typarams: List<TreeId>,
        env: &Rc<Env<AttrContext>>,
        check_cyclic: bool,
    ) {
        let mut current = typarams.clone();
        while current.non_empty() {
            let tvar = *current.head();
            let ty = ctx.tree(tvar).ty;
            if let Some(type_var) = ty {
                let Some(tsym) = ctx.typ(type_var).tsym else {
                    panic!("type parameter tsym");
                };
                ctx.sym_mut(tsym).flags_field |= crate::code::flags::UNATTRIBUTED as i64;
                let no_type = self.no_type(ctx);
                set_upper_bound(ctx, type_var, no_type);
                let bounds = type_parameter_bounds(ctx, tvar);
                if !bounds.is_empty() {
                    let mut result: List<TypeId> = List::nil();
                    let mut bounds_current = bounds;
                    while bounds_current.non_empty() {
                        let bound = *bounds_current.head();
                        let bound_type = self.attrib_type_use(ctx, syms, bound, env);
                        result = result.prepend(bound_type);
                        bounds_current = bounds_current.tail();
                    }
                    // Java: types.setBounds(a, bounds.reverse())
                    let reversed = list_reverse(result);
                    self.types.set_bounds(ctx, syms, &self.names, type_var, reversed, false);
                } else {
                    // if no bounds are given, assume a single bound of
                    // java.lang.Object.
                    self.types.set_bounds(
                        ctx,
                        syms,
                        &self.names,
                        type_var,
                        List::of(syms.object_type),
                        false,
                    );
                }
                ctx.sym_mut(tsym).flags_field &= !(crate::code::flags::UNATTRIBUTED as i64);
            }
            current = current.tail();
        }
        if check_cyclic {
            let mut current = typarams;
            while current.non_empty() {
                let tvar = *current.head();
                // TODO(Check batch): chk.checkNonCyclic(tvar.pos(), (TypeVar)tvar.type)
                let _ = tvar;
                current = current.tail();
            }
        }
    }

    /// Java: public Symbol attribIdent(JCTree tree, JCCompilationUnit topLevel)
    /// (357-366)
    pub fn attrib_ident_toplevel(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        top_level: TreeId,
    ) -> SymbolId {
        let local_env = self.enter.borrow().get_top_level_env(ctx, top_level);
        // Java: localEnv.enclClass = make.ClassDef(make.Modifiers(0),
        //     syms.errSymbol.name, null, null, null, null);
        //     localEnv.enclClass.sym = syms.errSymbol;
        let err_symbol = syms.err_symbol;
        let err_name = ctx.sym(err_symbol).name.clone();
        let class_def = {
            let mut make = self.make.borrow_mut();
            let mods = make.modifiers(ctx, 0);
            make.class_def_permitting(
                ctx,
                mods,
                err_name,
                List::nil(),
                None,
                List::nil(),
                List::nil(),
                List::nil(),
            )
        };
        if let TreeKind::ClassDecl { sym, .. } = &mut ctx.tree_mut(class_def).kind {
            *sym = Some(err_symbol);
        }
        let env = Rc::new(Env {
            next: local_env.next.clone(),
            outer: local_env.outer.clone(),
            tree: local_env.tree,
            toplevel: local_env.toplevel,
            encl_class: Some(class_def),
            encl_method: local_env.encl_method,
            info: Rc::clone(&local_env.info),
            base_clause: local_env.base_clause,
        });
        self.attrib_ident(ctx, syms, tree, &env)
    }

    /// Java: public Symbol attribIdent(JCTree tree, Env<AttrContext> env)
    /// (370-375)
    pub fn attrib_ident(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _tree: TreeId,
        _env: &Rc<Env<AttrContext>>,
    ) -> SymbolId {
        // TODO(Resolve batch): the IdentAttributer visitor (rs.findIdent/
        // findIdentInPackage/findMemberType); registered.
        unimplemented!("TODO(Resolve batch): Attr.attribIdent IdentAttributer")
    }

    /// Java: Type attribIdentAsEnumType(Env<AttrContext> env, JCIdent id) (941-947)
    pub fn attrib_ident_as_enum_type(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _env: &Rc<Env<AttrContext>>,
        _id: TreeId,
    ) -> TypeId {
        // TODO(Resolve batch): rs.findIdentInPackage(..., names._class?) and the
        // enum class literal type; registered.
        unimplemented!("TODO(Resolve batch): Attr.attribIdentAsEnumType")
    }

    // ------------------------------------------------------------------
    // Private helpers.
    // ------------------------------------------------------------------

    fn attrib_tree_use(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
        result_info: ResultInfo,
    ) -> TypeId {
        match self.attrib_tree(ctx, syms, tree, env, result_info) {
            Ok(value) => value,
            Err(_failure) => {
                // Java: catch (CompletionFailure ex) { tree.type = syms.errType;
                //     return chk.completionError(tree.pos(), ex); }
                // The attribTree implementation sets the tree type and returns
                // the failure; the completion error reporting is Check's
                // completionError (Check batch).
                unimplemented!("TODO(Check batch): Attr.attribTree CompletionFailure completionError")
            }
        }
    }

    fn attrib_tree_with(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
        result_info: ResultInfo,
    ) -> TypeId {
        self.attrib_tree_use(ctx, syms, tree, env, result_info)
    }

    fn attrib_type_use(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) -> TypeId {
        let info = ResultInfo::new(KindSelector::TYP, Some(self.no_type(ctx)), Rc::clone(&self.basic_handler));
        self.attrib_tree_use(ctx, syms, tree, env, info)
    }

    fn pt(&self) -> Option<TypeId> {
        self.result_info.as_ref().and_then(|info| info.pt)
    }

    fn no_type(&mut self, ctx: &mut CompilerContext) -> TypeId {
        // Java: Type.noType — the existing ported helper (symtab::no_type_id)
        // builds the TypeTag::NONE instance with the same shape as the other
        // call sites (registered adapter).
        crate::code::symtab::no_type_id(ctx)
    }
}

fn import_qualid(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::Import { qualid, .. } => *qualid,
        _ => None,
    }
}

fn import_static_import(ctx: &CompilerContext, tree: TreeId) -> bool {
    match &ctx.tree(tree).kind {
        TreeKind::Import { static_import, .. } => *static_import,
        _ => false,
    }
}

fn type_parameter_bounds(ctx: &CompilerContext, tvar: TreeId) -> List<TreeId> {
    match &ctx.tree(tvar).kind {
        TreeKind::TypeParameter { bounds, .. } => bounds.clone(),
        _ => List::nil(),
    }
}

fn set_upper_bound(ctx: &mut CompilerContext, type_var: TypeId, bound: TypeId) {
    if let TypeKindData::TypeVar { bound: slot, .. } = &mut ctx.typ_mut(type_var).kind {
        *slot = Some(bound);
    }
}

fn list_reverse(list: List<TypeId>) -> List<TypeId> {
    let mut result: List<TypeId> = List::nil();
    let mut current = list;
    while current.non_empty() {
        result = result.prepend(*current.head());
        current = current.tail();
    }
    result
}

fn literal_value_to_constant(
    value: Option<crate::tree::jc_tree::LiteralValue>,
) -> crate::util::constants::ConstantValue {
    use crate::tree::jc_tree::LiteralValue;
    use crate::util::constants::ConstantValue;
    match value.expect("literal value") {
        LiteralValue::Integer(value) => ConstantValue::Integer(value),
        LiteralValue::Long(value) => ConstantValue::Long(value),
        LiteralValue::Float(value) => ConstantValue::Float(value),
        LiteralValue::Double(value) => ConstantValue::Double(value),
        LiteralValue::Character(value) => ConstantValue::Character(value),
        LiteralValue::Boolean(value) => ConstantValue::Boolean(value),
        LiteralValue::String(value) => ConstantValue::String(value),
    }
}

/// Java: Type.toString() — the display layer is not ported yet (registered).
fn type_display(_ctx: &CompilerContext, _ty: TypeId) -> String {
    unimplemented!("TODO(Types/Symbol printing batch): Type.toString")
}

/// Java: Feature::allowed_in_source helper is the ported Source feature check
/// (code::source::Feature); this function documents the dependency for §19.
fn _feature_allowed(feature: Feature, source: crate::code::source::Source) -> bool {
    feature.allowed_in_source(source)
}


/// Java: Symbol.isMemberOf(TypeSymbol clazz, Types types) (Symbol.java:552)
/// TODO(Symbol/Types batch): isSubClass/isInheritedIn/hiddenIn are not ported;
/// the call site in visitIdent only runs for inner classes (registered).
fn symbol_is_member_of(
    _ctx: &mut CompilerContext,
    _types: &crate::code::types::Types,
    _sym: SymbolId,
    _clazz: SymbolId,
) -> bool {
    unimplemented!("TODO(Symbol/Types batch): Symbol.isMemberOf")
}

/// Java: JCIdent.name
fn ident_name(ctx: &CompilerContext, tree: TreeId) -> crate::util::name::Name {
    match &ctx.tree(tree).kind {
        TreeKind::Ident { name, .. } => name.clone(),
        _ => panic!("ClassCastException: JCIdent expected"),
    }
}

/// Java: JCIdent.sym
fn ident_symbol(ctx: &CompilerContext, tree: TreeId) -> Option<SymbolId> {
    match &ctx.tree(tree).kind {
        TreeKind::Ident { sym, .. } => *sym,
        _ => None,
    }
}

/// Java: tree.sym = sym
fn set_ident_symbol(ctx: &mut CompilerContext, tree: TreeId, sym: SymbolId) {
    if let TreeKind::Ident { sym: slot, .. } = &mut ctx.tree_mut(tree).kind {
        *slot = Some(sym);
    }
}
/// Java: JCTypeApply.clazz
fn type_apply_clazz(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::TypeApply { clazz, .. } => *clazz,
        _ => None,
    }
}

/// Java: JCTypeApply.arguments
fn type_apply_arguments(ctx: &CompilerContext, tree: TreeId) -> List<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::TypeApply { arguments, .. } => arguments.clone(),
        _ => List::nil(),
    }
}

/// Java: Type.getTypeArguments() for a class type (the ported ClassType stores
/// Option<List<TypeId>>; Union/Intersection inherit the field).
pub(crate) fn type_arguments(ctx: &CompilerContext, t: TypeId) -> List<TypeId> {
    match &ctx.typ(t).kind {
        TypeKindData::ClassType { typarams_field, .. }
        | TypeKindData::UnionClassType { typarams_field, .. }
        | TypeKindData::IntersectionClassType { typarams_field, .. } => {
            typarams_field.clone().unwrap_or_else(List::nil)
        }
        _ => List::nil(),
    }
}

fn class_tree_symbol(ctx: &CompilerContext, tree: TreeId) -> Option<SymbolId> {
    match &ctx.tree(tree).kind {
        TreeKind::ClassDecl { sym, .. } => *sym,
        _ => None,
    }
}
// ---------------------------------------------------------------------------
// Batch 5: Attr API subset used by ArgumentAttr/DeferredAttr (Attr.java:
// conditionalContext 2092-2106, diamondContext 3007-3016, FunctionalReturn
// context 3380-3435, lambdaBodyResult 3438-3453, checkLambdaCompatible
// 3455-3480). §9 registered adapters: the anonymous Java CheckContext
// subclasses are named structs; the explicit ctx/syms parameters follow
// ADR 0001.
// ---------------------------------------------------------------------------

/// Java: Attr.conditionalContext(CheckContext) (Attr.java:2092-2106)
pub struct ConditionalCheckContext {
    pub enclosing: Rc<dyn crate::comp::check::CheckContext>,
    pub diags: Rc<crate::util::jc_diagnostic::Factory>,
}

impl crate::comp::check::CheckContext for ConditionalCheckContext {
    fn compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        found: TypeId,
        req: TypeId,
        warn: &mut crate::util::warner::Warner,
    ) -> bool {
        self.enclosing.compatible(ctx, syms, types, found, req, warn)
    }

    fn report(&self, ctx: &mut CompilerContext, pos: i32, details: &crate::util::jc_diagnostic::JCDiagnostic) {
        // Java: enclosingContext.report(pos,
        //   diags.fragment(Fragments.IncompatibleTypeInConditional(details)));
        let message = details.get_message(None);
        let fragment = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
            crate::resources::compiler_properties::fragments::incompatible_type_in_conditional(
                crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                    message.to_string(),
                ),
            ),
        ));
        self.enclosing.report(ctx, pos, &fragment);
    }

    fn check_warner(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> crate::util::warner::Warner {
        self.enclosing.check_warner(ctx, syms, types, pos, found, req)
    }

    fn inference_context(&self) -> &Rc<crate::comp::infer::InferenceContext> {
        self.enclosing.inference_context()
    }

    fn deferred_attr_context(&self) -> &Rc<crate::comp::deferred_attr::DeferredAttrContext> {
        self.enclosing.deferred_attr_context()
    }
}

/// Java: Attr.diamondContext(JCNewClass, TypeSymbol, CheckContext)
/// (Attr.java:3007-3016)
pub struct DiamondCheckContext {
    pub enclosing: Rc<dyn crate::comp::check::CheckContext>,
    pub diags: Rc<crate::util::jc_diagnostic::Factory>,
    pub clazz: TreeId,
    pub tsym: SymbolId,
}

impl crate::comp::check::CheckContext for DiamondCheckContext {
    fn compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        found: TypeId,
        req: TypeId,
        warn: &mut crate::util::warner::Warner,
    ) -> bool {
        self.enclosing.compatible(ctx, syms, types, found, req, warn)
    }

    fn report(&self, ctx: &mut CompilerContext, _pos: i32, details: &crate::util::jc_diagnostic::JCDiagnostic) {
        // Java: enclosingContext.report(clazz.clazz,
        //   diags.fragment(Fragments.CantApplyDiamond1(Fragments.Diamond(tsym), details)));
        let diamond_text = {
            let sym = ctx.sym(self.tsym);
            let display = crate::code::kinds::kind_name(sym, ctx);
            let fragment = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::diamond(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(display.to_string()),
                ),
            ));
            fragment.get_message(None).to_string()
        };
        let message = details.get_message(None);
        let key = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
            crate::resources::compiler_properties::fragments::cant_apply_diamond1(
                crate::resources::diagnostic_contract::DiagnosticArg::from_display(diamond_text),
                crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                    message.to_string(),
                ),
            ),
        ));
        let clazz_pos = ctx.tree(self.clazz).pos;
        self.enclosing.report(ctx, clazz_pos, &key);
    }

    fn check_warner(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> crate::util::warner::Warner {
        self.enclosing.check_warner(ctx, syms, types, pos, found, req)
    }

    fn inference_context(&self) -> &Rc<crate::comp::infer::InferenceContext> {
        self.enclosing.inference_context()
    }

    fn deferred_attr_context(&self) -> &Rc<crate::comp::deferred_attr::DeferredAttrContext> {
        self.enclosing.deferred_attr_context()
    }
}

/// Java: Attr.FunctionalReturnContext (Attr.java:3394-3411)
pub struct FunctionalReturnContext {
    pub enclosing: Rc<dyn crate::comp::check::CheckContext>,
    pub basic_handler: Rc<dyn crate::comp::check::CheckContext>,
    pub diags: Rc<crate::util::jc_diagnostic::Factory>,
}

impl crate::comp::check::CheckContext for FunctionalReturnContext {
    fn compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        found: TypeId,
        req: TypeId,
        warn: &mut crate::util::warner::Warner,
    ) -> bool {
        // Java: chk.basicHandler.compatible(inferenceContext().asUndetVar(found),
        //   inferenceContext().asUndetVar(req), warn)
        let inference_context = Rc::clone(self.enclosing.inference_context());
        let found = inference_context.as_undet_var(ctx, syms, types, found);
        let req = inference_context.as_undet_var(ctx, syms, types, req);
        self.basic_handler.compatible(ctx, syms, types, found, req, warn)
    }

    fn report(&self, ctx: &mut CompilerContext, pos: i32, details: &crate::util::jc_diagnostic::JCDiagnostic) {
        // Java: enclosingContext.report(pos,
        //   diags.fragment(Fragments.IncompatibleRetTypeInLambda(details)));
        let message = details.get_message(None);
        let fragment = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
            crate::resources::compiler_properties::fragments::incompatible_ret_type_in_lambda(
                crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                    message.to_string(),
                ),
            ),
        ));
        self.enclosing.report(ctx, pos, &fragment);
    }

    fn check_warner(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> crate::util::warner::Warner {
        self.enclosing.check_warner(ctx, syms, types, pos, found, req)
    }

    fn inference_context(&self) -> &Rc<crate::comp::infer::InferenceContext> {
        self.enclosing.inference_context()
    }

    fn deferred_attr_context(&self) -> &Rc<crate::comp::deferred_attr::DeferredAttrContext> {
        self.enclosing.deferred_attr_context()
    }
}

/// Java: Attr.ExpressionLambdaReturnContext (Attr.java:3413-3435)
pub struct ExpressionLambdaReturnContext {
    pub base: FunctionalReturnContext,
    pub expr: TreeId,
    pub exp_stmt_expected: std::cell::Cell<bool>,
}

impl crate::comp::check::CheckContext for ExpressionLambdaReturnContext {
    fn compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        found: TypeId,
        req: TypeId,
        warn: &mut crate::util::warner::Warner,
    ) -> bool {
        // Java: if (req.hasTag(VOID)) { expStmtExpected = true;
        //          return TreeInfo.isExpressionStatement(expr); }
        //        else return super.compatible(found, req, warn);
        if ctx.typ(req).has_tag(crate::code::type_tag::TypeTag::VOID) {
            self.exp_stmt_expected.set(true);
            return crate::tree::tree_info::is_expression_statement(ctx, self.expr);
        }
        self.base.compatible(ctx, syms, types, found, req, warn)
    }

    fn report(&self, ctx: &mut CompilerContext, pos: i32, details: &crate::util::jc_diagnostic::JCDiagnostic) {
        // Java: if (expStmtExpected) enclosingContext.report(pos,
        //          diags.fragment(Fragments.StatExprExpected));
        //        else super.report(pos, details);
        if self.exp_stmt_expected.get() {
            let fragment = self.base.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::stat_expr_expected(),
            ));
            self.base.enclosing.report(ctx, pos, &fragment);
        } else {
            self.base.report(ctx, pos, details);
        }
    }

    fn check_warner(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> crate::util::warner::Warner {
        self.base.check_warner(ctx, syms, types, pos, found, req)
    }

    fn inference_context(&self) -> &Rc<crate::comp::infer::InferenceContext> {
        self.base.inference_context()
    }

    fn deferred_attr_context(&self) -> &Rc<crate::comp::deferred_attr::DeferredAttrContext> {
        self.base.deferred_attr_context()
    }
}

impl Attr {
    /// Java: CheckContext conditionalContext(CheckContext checkContext)
    /// (Attr.java:2092-2106)
    pub fn conditional_context(&self) -> Rc<dyn crate::comp::check::CheckContext> {
        Rc::new(ConditionalCheckContext {
            enclosing: Rc::clone(&self.basic_handler),
            diags: Rc::clone(&self.diags),
        })
    }

    /// Java: CheckContext diamondContext(JCNewClass clazz, TypeSymbol tsym,
    /// CheckContext checkContext) (Attr.java:3007-3016)
    pub fn diamond_context(
        &self,
        clazz: TreeId,
        tsym: SymbolId,
    ) -> Rc<dyn crate::comp::check::CheckContext> {
        Rc::new(DiamondCheckContext {
            enclosing: Rc::clone(&self.basic_handler),
            diags: Rc::clone(&self.diags),
            clazz,
            tsym,
        })
    }

    /// Java: ResultInfo lambdaBodyResult(JCLambda that, Type descriptor,
    /// ResultInfo resultInfo) (Attr.java:3438-3453)
    pub fn lambda_body_result(
        &self,
        ctx: &mut CompilerContext,
        that: TreeId,
        descriptor: TypeId,
        result_info: &ResultInfo,
    ) -> ResultInfo {
        let (body, body_is_expression) = match &ctx.tree(that).kind {
            TreeKind::Lambda { body, .. } => {
                let body = body.expect("lambda body");
                // Java: that.getBodyKind() == EXPRESSION (JCLambda.getBodyKind
                // returns STATEMENT iff body.hasTag(BLOCK)).
                (body, ctx.tree(body).tag != Tag::BLOCK)
            }
            _ => panic!("ClassCastException: JCLambda expected"),
        };
        let func_context: Rc<dyn crate::comp::check::CheckContext> = if body_is_expression {
            Rc::new(ExpressionLambdaReturnContext {
                base: FunctionalReturnContext {
                    enclosing: Rc::clone(&result_info.check_context),
                    basic_handler: Rc::clone(&self.basic_handler),
                    diags: Rc::clone(&self.diags),
                },
                expr: body,
                exp_stmt_expected: std::cell::Cell::new(false),
            })
        } else {
            Rc::new(FunctionalReturnContext {
                enclosing: Rc::clone(&result_info.check_context),
                basic_handler: Rc::clone(&self.basic_handler),
                diags: Rc::clone(&self.diags),
            })
        };
        let return_type = self
            .types
            .get_return_type_public(ctx, descriptor);
        if Some(return_type) == Some(self.syms.borrow().recovery_type) {
            // Java: descriptor.getReturnType() == Type.recoveryType ? recoveryInfo: ...
            self.recovery_info.clone()
        } else {
            ResultInfo::new(KindSelector::VAL, Some(return_type), func_context)
        }
    }

    /// Java: void checkLambdaCompatible(JCLambda tree, Type descriptor,
    /// CheckContext checkContext) (Attr.java:3455-3480)
    pub fn check_lambda_compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        tree: TreeId,
        descriptor: TypeId,
        check_context: &Rc<dyn crate::comp::check::CheckContext>,
    ) {
        let inference_context = Rc::clone(check_context.inference_context());
        let return_type = inference_context.as_undet_var(
            ctx,
            syms,
            &self.types,
            self.types.get_return_type_public(ctx, descriptor),
        );
        let (body, _body_is_expression) = match &ctx.tree(tree).kind {
            TreeKind::Lambda { body, .. } => (
                body,
                body.map(|body| ctx.tree(body).tag != Tag::BLOCK)
                    .unwrap_or(false),
            ),
            _ => panic!("ClassCastException: JCLambda expected"),
        };
        let can_complete_normally = match &ctx.tree(tree).kind {
            TreeKind::Lambda {
                can_complete_normally,
                ..
            } => *can_complete_normally,
            _ => panic!("ClassCastException: JCLambda expected"),
        };
        let return_type_is_void =
            ctx.typ(return_type).has_tag(crate::code::type_tag::TypeTag::VOID);
        let return_type_is_recovery = return_type == self.syms.borrow().recovery_type;
        let body_is_statement = body
            .map(|body| ctx.tree(body).tag == Tag::BLOCK)
            .unwrap_or(true);
        if body_is_statement && can_complete_normally && !return_type_is_void && !return_type_is_recovery {
            let missing = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::missing_ret_val(
                    crate::resources::diagnostic_contract::TypeArg::placeholder(
                        type_display(ctx, return_type),
                    ),
                ),
            ));
            let message = missing.get_message(None);
            let outer = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::incompatible_ret_type_in_lambda(
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                        message.to_string(),
                    ),
                ),
            ));
            check_context.report(ctx, ctx.tree(tree).pos, &outer);
        }
        let arg_types = inference_context.as_undet_vars(
            ctx,
            syms,
            &self.types,
            self.types.get_parameter_types_public(ctx, descriptor),
        );
        let param_trees = match &ctx.tree(tree).kind {
            TreeKind::Lambda { params, .. } => params.clone(),
            _ => panic!("ClassCastException: JCLambda expected"),
        };
        let tree_types = crate::tree::tree_info::types(ctx, &param_trees);
        let mut same = arg_types.length() == tree_types.length();
        if same {
            let mut a = arg_types;
            let mut b = tree_types;
            while a.non_empty() {
                let at = *a.head();
                let bt = *b.head();
                if bt != Some(at) {
                    same = false;
                    break;
                }
                a = a.tail();
                b = b.tail();
            }
        }
        if !same {
            let fragment = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::incompatible_arg_types_in_lambda(),
            ));
            check_context.report(ctx, ctx.tree(tree).pos, &fragment);
        }
    }
}

/// Java: Attr.TargetInfo (Attr.java:3230-3238)
pub struct TargetInfo {
    pub target: TypeId,
    pub descriptor: TypeId,
}

impl Attr {
    /// Java: TargetInfo getTargetInfo(JCPolyExpression that, ResultInfo
    /// resultInfo, List<Type> explicitParamTypes) (Attr.java:3240-3295) —
    /// §9: explicit ctx/syms parameters; the upstream unchecked
    /// FunctionDescriptorLookupError is returned as Err.
    pub fn get_target_info(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        that: TreeId,
        result_info: &ResultInfo,
        explicit_param_types: Option<List<TypeId>>,
    ) -> Result<TargetInfo, crate::code::types::FunctionDescriptorLookupError> {
        use crate::code::type_tag::TypeTag;
        let recovery_type = self.syms.borrow().recovery_type;
        let mut current_target = result_info.pt.expect("resultInfo.pt");
        let lambda_type;
        if Some(current_target) != Some(recovery_type) {
            // We need to adjust the target. If the target is an intersection
            // type, for example: SAM & I1 & I2 ... the target will be updated
            // to SAM.
            current_target = self.target_checker(ctx, syms, current_target, ctx.tree(that).pos)?;
            if !ctx.typ(current_target).is_intersection() {
                if let Some(explicit) = &explicit_param_types {
                    let chk = self.chk.borrow();
                    current_target = self.infer.instantiate_functional_interface(
                        ctx,
                        syms,
                        &self.types,
                        &self.diags,
                        &chk,
                        ctx.tree(that).pos,
                        current_target,
                        explicit.clone(),
                        &result_info.check_context,
                    )?;
                }
                current_target = self.types.remove_wildcards(ctx, syms, current_target);
                let chk = self.chk.borrow();
                lambda_type = self
                    .types
                    .find_descriptor_type(ctx, syms, &self.diags, &chk, current_target)?;
            } else {
                let mut components: List<TypeId> = List::nil();
                let explicit_components = ctx.typ(current_target).get_explicit_components();
                for bound in explicit_components.iter() {
                    let mut bound = bound;
                    if let Some(explicit) = &explicit_param_types {
                        // Java: try { bound = infer.instantiateFunctionalInterface(that,
                        //     bound, explicitParamTypes, resultInfo.checkContext); }
                        //     catch (FunctionDescriptorLookupError t) { /* do nothing */ }
                        let chk = self.chk.borrow();
                        if let Ok(instantiated) = self.infer.instantiate_functional_interface(
                            ctx,
                            syms,
                            &self.types,
                            &self.diags,
                            &chk,
                            ctx.tree(that).pos,
                            bound,
                            explicit.clone(),
                            &result_info.check_context,
                        ) {
                            bound = instantiated;
                        }
                    }
                    bound = self.types.remove_wildcards(ctx, syms, bound);
                    components = components.append(bound);
                }
                current_target =
                    self.types
                        .make_intersection_type(ctx, syms, &self.names, components, false);
                // Java: currentTarget.tsym.flags_field |= INTERFACE;
                let tsym = ctx
                    .typ(current_target)
                    .tsym
                    .expect("intersection type tsym");
                let interface_flag = crate::code::flags::INTERFACE as i64;
                ctx.sym_mut(tsym).flags_field |= interface_flag;
                let chk = self.chk.borrow();
                lambda_type = self
                    .types
                    .find_descriptor_type(ctx, syms, &self.diags, &chk, current_target)?;
            }
        } else {
            current_target = recovery_type;
            lambda_type = self.fallback_descriptor_type(ctx, syms, that);
        }
        if ctx.tree(that).tag == Tag::LAMBDA && ctx.typ(lambda_type).has_tag(TypeTag::FORALL) {
            // lambda expression target desc cannot be a generic method
            let kind = crate::code::kinds::kind_name(ctx.sym(ctx.typ(current_target).tsym.expect("currentTarget tsym")), ctx);
            let sym = ctx.typ(current_target).tsym.expect("currentTarget tsym");
            let fragment = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::invalid_generic_lambda_target(
                    crate::resources::diagnostic_contract::TypeArg::placeholder(type_display(ctx, lambda_type)),
                    crate::resources::diagnostic_contract::KindNameArg::placeholder(kind.to_string()),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        crate::comp::resolve::symbol_display_name(ctx, sym),
                    ),
                ),
            ));
            result_info
                .check_context
                .report(ctx, ctx.tree(that).pos, &fragment);
            current_target =
                self.types
                    .create_error_type_original(ctx, result_info.pt, &self.syms.borrow());
        }
        Ok(TargetInfo {
            target: current_target,
            descriptor: lambda_type,
        })
    }

    /// Java: Types.MapVisitor<DiagnosticPosition> targetChecker (Attr.java:
    /// 3315-3348): visitClassType => intersection ? visitIntersectionClassType
    /// : t; visitIntersectionClassType validates the notional interface through
    /// findDescriptorSymbol and returns the intersection unchanged.
    fn target_checker(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        t: TypeId,
        pos: i32,
    ) -> Result<TypeId, crate::code::types::FunctionDescriptorLookupError> {
        if !ctx.typ(t).is_intersection() {
            return Ok(t);
        }
        let notional = self.make_notional_interface(ctx, syms, t, pos);
        let chk = self.chk.borrow();
        self.types
            .find_descriptor_symbol(ctx, syms, &self.diags, &chk, notional)?;
        Ok(t)
    }

    /// Java: private TypeSymbol makeNotionalInterface(IntersectionClassType ict,
    /// DiagnosticPosition pos) (Attr.java:3322-3338)
    fn make_notional_interface(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        ict: TypeId,
        _pos: i32,
    ) -> SymbolId {
        let interfaces = match &ctx.typ(ict).kind {
            crate::code::type_::TypeKindData::IntersectionClassType {
                interfaces_field, ..
            } => interfaces_field.clone().expect("intersection type has no interfaces"),
            _ => panic!("ClassCastException: IntersectionClassType expected"),
        };
        let mut targs: List<TypeId> = List::nil();
        let mut supertypes: List<TypeId> = List::nil();
        for i in interfaces.iter() {
            if self.types.is_parameterized_type_of(ctx, i) {
                let i_tsym = ctx.typ(i).tsym.expect("interface tsym");
                let i_tsym_type = ctx.sym(i_tsym).ty.expect("interface tsym type");
                let mut params = crate::code::types::allparams_of(ctx, i_tsym_type);
                while params.non_empty() {
                    targs = targs.append(*params.head());
                    params = params.tail();
                }
            }
            let i_tsym = ctx.typ(i).tsym.expect("interface tsym");
            let i_tsym_type = ctx.sym(i_tsym).ty.expect("interface tsym type");
            supertypes = supertypes.append(i_tsym_type);
        }
        let notional = self
            .types
            .make_intersection_type(ctx, syms, &self.names, supertypes, false);
        if let crate::code::type_::TypeKindData::IntersectionClassType {
            allparams_field, ..
        } = &mut ctx.typ_mut(notional).kind
        {
            *allparams_field = Some(targs);
        }
        let notional_tsym = ctx.typ(notional).tsym.expect("notional interface tsym");
        ctx.sym_mut(notional_tsym).flags_field |= crate::code::flags::INTERFACE as i64;
        notional_tsym
    }

    /// Java: private Type fallbackDescriptorType(JCExpression tree)
    /// (Attr.java:3350-3372)
    fn fallback_descriptor_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> TypeId {
        match ctx.tree(tree).tag {
            Tag::LAMBDA => {
                let params = match &ctx.tree(tree).kind {
                    TreeKind::Lambda { params, .. } => params.clone(),
                    _ => panic!("ClassCastException: JCLambda expected"),
                };
                let mut argtypes: List<TypeId> = List::nil();
                for param in params.iter() {
                    let vartype = match &ctx.tree(param).kind {
                        TreeKind::VariableDecl { vartype, .. } => *vartype,
                        _ => panic!("ClassCastException: JCVariableDecl expected"),
                    };
                    let argtype = match vartype {
                        Some(vartype) => match ctx.tree(vartype).ty {
                            Some(ty) => ty,
                            None => syms.err_type,
                        },
                        None => syms.err_type,
                    };
                    argtypes = argtypes.append(argtype);
                }
                let recovery_type = syms.recovery_type;
                let ty = crate::code::type_::Type::new_method_type(
                    argtypes,
                    recovery_type,
                    List::of(syms.throwable_type),
                    syms.method_class,
                );
                ctx.alloc_type(ty)
            }
            Tag::REFERENCE => {
                let recovery_type = syms.recovery_type;
                let ty = crate::code::type_::Type::new_method_type(
                    List::nil(),
                    recovery_type,
                    List::of(syms.throwable_type),
                    syms.method_class,
                );
                ctx.alloc_type(ty)
            }
            _ => panic!("AssertionError: Cannot get here!"),
        }
    }
}

/// Batch 5 helper: diagnostic display text of a type for the Infer/Attr
/// adapter argument placeholders (§9, the type printer batch is pending).
pub(crate) fn type_display_of(ctx: &CompilerContext, t: TypeId) -> String {
    type_display(ctx, t)
}

/// Java: Attr.RecoveryInfo's anonymous CheckContext (Attr.java:598-620): a
/// NestedCheckContext whose deferredAttrContext is the empty deferred context.
/// Registered §9: the anonymous subclass is a named struct.
pub struct RecoveryCheckContext {
    pub enclosing: Rc<dyn crate::comp::check::CheckContext>,
    pub deferred_attr_context: Rc<crate::comp::deferred_attr::DeferredAttrContext>,
}

impl crate::comp::check::CheckContext for RecoveryCheckContext {
    fn compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        found: TypeId,
        req: TypeId,
        warn: &mut crate::util::warner::Warner,
    ) -> bool {
        self.enclosing.compatible(ctx, syms, types, found, req, warn)
    }

    fn report(&self, ctx: &mut CompilerContext, pos: i32, details: &crate::util::jc_diagnostic::JCDiagnostic) {
        self.enclosing.report(ctx, pos, details);
    }

    fn check_warner(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> crate::util::warner::Warner {
        self.enclosing.check_warner(ctx, syms, types, pos, found, req)
    }

    fn inference_context(&self) -> &Rc<crate::comp::infer::InferenceContext> {
        self.enclosing.inference_context()
    }

    fn deferred_attr_context(&self) -> &Rc<crate::comp::deferred_attr::DeferredAttrContext> {
        &self.deferred_attr_context
    }
}



// ---------------------------------------------------------------------------
// Batch 5 prerequisite: Attr.PostAttrAnalyzer (Attr.java:5824-5930) + postAttr.
// §9 registered: Java TreeScanner subclassing -> TreeScannerVisitor impl; the
// "tree instanceof JCExpression" test is ports::tree_info::is_expression (tag
// based class-hierarchy test); symbols are created through the arena with the
// upstream constructor arguments.
// ---------------------------------------------------------------------------

pub struct PostAttrAnalyzer<'a> {
    pub syms: &'a Symtab,
    pub names: &'a Names,
}

impl PostAttrAnalyzer<'_> {
    /// Java: private void initTypeIfNeeded(JCTree that)
    fn init_type_if_needed(&self, ctx: &mut CompilerContext, tree: TreeId) {
        if ctx.tree(tree).ty.is_none() {
            let ty = if ctx.tree(tree).tag == Tag::METHODDEF {
                self.dummy_method_type_for(ctx, Some(tree))
            } else {
                self.syms.unknown_type
            };
            ctx.tree_mut(tree).ty = Some(ty);
        }
    }

    /// Java: private Type dummyMethodType(JCMethodDecl md)
    fn dummy_method_type_for(&self, ctx: &mut CompilerContext, md: Option<TreeId>) -> TypeId {
        let mut restype = self.syms.unknown_type;
        if let Some(md) = md {
            let restype_tree = match &ctx.tree(md).kind {
                TreeKind::MethodDecl { restype, .. } => *restype,
                _ => None,
            };
            if let Some(restype_tree) = restype_tree {
                if ctx.tree(restype_tree).tag == Tag::TYPEIDENT
                    && let TreeKind::PrimitiveTypeTree { typetag } = ctx.tree(restype_tree).kind
                    && typetag == crate::code::type_tag::TypeTag::VOID
                {
                    restype = self.syms.void_type;
                }
            }
        }
        let ty = crate::code::type_::Type::new_method_type(
            List::nil(),
            restype,
            List::nil(),
            self.syms.method_class,
        );
        ctx.alloc_type(ty)
    }

    fn dummy_method_type(&self, ctx: &mut CompilerContext) -> TypeId {
        self.dummy_method_type_for(ctx, None)
    }

    fn new_symbol(
        &self,
        ctx: &mut CompilerContext,
        kind: crate::code::kinds::Kind,
        flags: i64,
        name: crate::util::name::Name,
        ty: Option<TypeId>,
        kind_data: crate::code::symbol::SymbolKind,
    ) -> SymbolId {
        ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            kind,
            flags,
            name,
            ty,
            Some(self.syms.no_symbol),
            kind_data,
        ))
    }
    fn operation_symbol(&self, ctx: &mut CompilerContext) -> SymbolId {
        let ty = self.dummy_method_type(ctx);
        self.new_symbol(
            ctx,
            crate::code::kinds::Kind::MTH,
            0,
            self.names.empty.clone(),
            Some(ty),
            crate::code::symbol::SymbolKind::OperatorSymbol {
                opcode: -1,
                access_code: 0,
            },
        )
    }
}

impl crate::tree::tree_scanner::TreeScannerVisitor for PostAttrAnalyzer<'_> {
    /// Java: public void scan(JCTree tree)
    fn scan(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) {
        if let Some(tree) = tree {
            if crate::tree::tree_info::is_expression(ctx, tree) {
                self.init_type_if_needed(ctx, tree);
            }
            crate::tree::jc_tree::accept(ctx, tree, self);
        }
    }

    /// Java: public void visitIdent(JCIdent that)
    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let TreeKind::Ident { sym, .. } = &mut ctx.tree_mut(tree).kind
            && sym.is_none()
        {
            *sym = Some(self.syms.unknown_symbol);
        }
    }

    /// Java: public void visitSelect(JCFieldAccess that)
    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let TreeKind::FieldAccess { sym, .. } = &mut ctx.tree_mut(tree).kind
            && sym.is_none()
        {
            *sym = Some(self.syms.unknown_symbol);
        }
        let selected = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { selected, .. } => *selected,
            _ => panic!("expected a field access"),
        };
        self.scan(ctx, selected);
    }

    /// Java: public void visitClassDef(JCClassDecl that)
    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.init_type_if_needed(ctx, tree);
        let needs_sym = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { sym, .. } => sym.is_none(),
            _ => panic!("expected a class declaration"),
        };
        if needs_sym {
            let (name, ty) = match &ctx.tree(tree).kind {
                TreeKind::ClassDecl { name, .. } => (name.clone(), ctx.tree(tree).ty),
                _ => unreachable!(),
            };
            let fullname = crate::code::symtab::form_full_name(ctx, &name, Some(self.syms.no_symbol));
            let flatname = crate::code::symtab::form_flat_name(ctx, &name, Some(self.syms.no_symbol));
            let sym = self.new_symbol(
                ctx,
                crate::code::kinds::Kind::TYP,
                0,
                name,
                ty,
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
            if let TreeKind::ClassDecl { sym: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(sym);
            }
        }
        let (mods, typarams, extending, implementing, permitting, defs) =
            match &ctx.tree(tree).kind {
                TreeKind::ClassDecl {
                    mods,
                    typarams,
                    extending,
                    implementing,
                    permitting,
                    defs,
                    ..
                } => (
                    *mods,
                    typarams.clone(),
                    *extending,
                    implementing.clone(),
                    permitting.clone(),
                    defs.clone(),
                ),
                _ => panic!("expected a class declaration"),
            };
        self.scan(ctx, mods);
        self.scan_list(ctx, &typarams);
        self.scan(ctx, extending);
        self.scan_list(ctx, &implementing);
        self.scan_list(ctx, &permitting);
        self.scan_list(ctx, &defs);
    }

    /// Java: public void visitMethodDef(JCMethodDecl that)
    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.init_type_if_needed(ctx, tree);
        let needs_sym = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl { sym, .. } => sym.is_none(),
            _ => panic!("expected a method declaration"),
        };
        if needs_sym {
            let (name, ty) = match &ctx.tree(tree).kind {
                TreeKind::MethodDecl { name, .. } => (name.clone(), ctx.tree(tree).ty),
                _ => unreachable!(),
            };
            let sym = self.new_symbol(
                ctx,
                crate::code::kinds::Kind::MTH,
                0,
                name,
                ty,
                crate::code::symbol::SymbolKind::MethodSymbol {
                    extra_params: List::nil(),
                    captured_locals: List::nil(),
                    params: List::nil(),
                    default_value: None,
                },
            );
            if let TreeKind::MethodDecl { sym: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(sym);
            }
        }
        let (mods, restype, typarams, recvparam, params, thrown, default_value, body) =
            match &ctx.tree(tree).kind {
                TreeKind::MethodDecl {
                    mods,
                    restype,
                    typarams,
                    recvparam,
                    params,
                    thrown,
                    default_value,
                    body,
                    ..
                } => (
                    *mods,
                    *restype,
                    typarams.clone(),
                    *recvparam,
                    params.clone(),
                    thrown.clone(),
                    *default_value,
                    *body,
                ),
                _ => panic!("expected a method declaration"),
            };
        self.scan(ctx, mods);
        self.scan(ctx, restype);
        self.scan_list(ctx, &typarams);
        self.scan(ctx, recvparam);
        self.scan_list(ctx, &params);
        self.scan_list(ctx, &thrown);
        self.scan(ctx, default_value);
        self.scan(ctx, body);
    }

    /// Java: public void visitVarDef(JCVariableDecl that)
    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.init_type_if_needed(ctx, tree);
        let (name, ty, needs_sym, vartype) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl {
                name,
                vartype,
                sym,
                ..
            } => (name.clone(), ctx.tree(tree).ty, sym.is_none(), *vartype),
            _ => panic!("expected a variable declaration"),
        };
        if needs_sym {
            let sym = self.new_symbol(
                ctx,
                crate::code::kinds::Kind::VAR,
                0,
                name,
                ty,
                crate::code::symbol::SymbolKind::VarSymbol {
                    pos: 0,
                    adr: 0,
                    data_kind: None,
                    data: None,
                },
            );
            if let TreeKind::VariableDecl { sym: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(sym);
            }
        }
        if vartype.is_none() {
            // Java: that.vartype = make.at(Position.NOPOS).Erroneous();
            let erroneous = ctx.alloc_tree(crate::tree::jc_tree::JCTree {
                pos: crate::util::position::NOPOS,
                ty: None,
                tag: Tag::ERRONEOUS,
                kind: TreeKind::Erroneous { errs: List::nil() },
            });
            if let TreeKind::VariableDecl { vartype: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(erroneous);
            }
        }
        let (mods, vartype, nameexpr, init) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl {
                mods,
                vartype,
                nameexpr,
                init,
                ..
            } => (*mods, *vartype, *nameexpr, *init),
            _ => panic!("expected a variable declaration"),
        };
        self.scan(ctx, mods);
        self.scan(ctx, vartype);
        self.scan(ctx, nameexpr);
        self.scan(ctx, init);
    }

    /// Java: public void visitBindingPattern(JCBindingPattern that)
    fn visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.init_type_if_needed(ctx, tree);
        let var = match &ctx.tree(tree).kind {
            TreeKind::BindingPattern { var } => *var,
            _ => panic!("expected a binding pattern"),
        };
        if let Some(var) = var {
            self.init_type_if_needed(ctx, var);
            let (name, ty, needs_sym) = match &ctx.tree(var).kind {
                TreeKind::VariableDecl { name, sym, .. } => {
                    (name.clone(), ctx.tree(var).ty, sym.is_none())
                }
                _ => panic!("expected a variable declaration"),
            };
            if needs_sym {
                let sym = self.new_symbol(
                    ctx,
                    crate::code::kinds::Kind::VAR,
                    0,
                    name,
                    ty,
                    crate::code::symbol::SymbolKind::BindingSymbol {
                        aliases: List::nil(),
                    },
                );
                if let TreeKind::VariableDecl { sym: slot, .. } = &mut ctx.tree_mut(var).kind {
                    *slot = Some(sym);
                }
            }
        }
        self.scan(ctx, var);
    }

    /// Java: public void visitNewClass(JCNewClass that)
    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (constructor, constructor_type) = match &ctx.tree(tree).kind {
            TreeKind::NewClass {
                constructor,
                constructor_type,
                ..
            } => (*constructor, *constructor_type),
            _ => panic!("expected a new class"),
        };
        if constructor.is_none() {
            let ctor_type = self.dummy_method_type(ctx);
            let sym = self.new_symbol(
                ctx,
                crate::code::kinds::Kind::MTH,
                0,
                self.names.init.clone(),
                Some(ctor_type),
                crate::code::symbol::SymbolKind::MethodSymbol {
                    extra_params: List::nil(),
                    captured_locals: List::nil(),
                    params: List::nil(),
                    default_value: None,
                },
            );
            if let TreeKind::NewClass { constructor: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(sym);
            }
        }
        if constructor_type.is_none() {
            if let TreeKind::NewClass {
                constructor_type: slot,
                ..
            } = &mut ctx.tree_mut(tree).kind
            {
                *slot = Some(self.syms.unknown_type);
            }
        }
        let (encl, typeargs, clazz, args, def) = match &ctx.tree(tree).kind {
            TreeKind::NewClass {
                encl,
                typeargs,
                clazz,
                args,
                def,
                ..
            } => (*encl, typeargs.clone(), *clazz, args.clone(), *def),
            _ => panic!("expected a new class"),
        };
        self.scan(ctx, encl);
        self.scan_list(ctx, &typeargs);
        self.scan(ctx, clazz);
        self.scan_list(ctx, &args);
        self.scan(ctx, def);
    }



    /// Java: public void visitAssignop(JCAssignOp that)
    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let needs_operator = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { operator, .. } => operator.is_none(),
            _ => panic!("expected a compound assignment"),
        };
        if needs_operator {
            let sym = self.operation_symbol(ctx);
            if let TreeKind::AssignOp { operator: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(sym);
            }
        }
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a compound assignment"),
        };
        self.scan(ctx, lhs);
        self.scan(ctx, rhs);
    }

    /// Java: public void visitBinary(JCBinary that)
    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let needs_operator = match &ctx.tree(tree).kind {
            TreeKind::Binary { operator, .. } => operator.is_none(),
            _ => panic!("expected a binary expression"),
        };
        if needs_operator {
            let sym = self.operation_symbol(ctx);
            if let TreeKind::Binary { operator: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(sym);
            }
        }
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Binary { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a binary expression"),
        };
        self.scan(ctx, lhs);
        self.scan(ctx, rhs);
    }

    /// Java: public void visitUnary(JCUnary that)
    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let needs_operator = match &ctx.tree(tree).kind {
            TreeKind::Unary { operator, .. } => operator.is_none(),
            _ => panic!("expected a unary expression"),
        };
        if needs_operator {
            let sym = self.operation_symbol(ctx);
            if let TreeKind::Unary { operator: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(sym);
            }
        }
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, .. } => *arg,
            _ => panic!("expected a unary expression"),
        };
        self.scan(ctx, arg);
    }

    /// Java: public void visitReference(JCMemberReference that)
    fn visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        // Java: super.visitReference(that);
        let (expr, typeargs) = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { expr, typeargs, .. } => (*expr, typeargs.clone()),
            _ => panic!("expected a member reference"),
        };
        self.scan(ctx, expr);
        self.scan_list_opt(ctx, typeargs);
        let needs_sym = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { sym, .. } => sym.is_none(),
            _ => panic!("expected a member reference"),
        };
        if needs_sym {
            let ty = self.dummy_method_type(ctx);
            let sym = self.new_symbol(
                ctx,
                crate::code::kinds::Kind::MTH,
                0,
                self.names.empty.clone(),
                Some(ty),
                crate::code::symbol::SymbolKind::MethodSymbol {
                    extra_params: List::nil(),
                    captured_locals: List::nil(),
                    params: List::nil(),
                    default_value: None,
                },
            );
            if let TreeKind::MemberReference { sym: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(sym);
            }
        }
    }
}

tree_scanner_visitor_delegate!(PostAttrAnalyzer<'a>, 'a);

impl Attr {
    /// Java: public void postAttr(JCTree tree) (Attr.java:5821-5823)
    pub fn post_attr(&mut self, ctx: &mut CompilerContext, syms: &Symtab, tree: TreeId) {
        let mut analyzer = PostAttrAnalyzer {
            syms,
            names: &self.names,
        };
        crate::tree::tree_scanner::TreeScannerVisitor::scan(&mut analyzer, ctx, Some(tree));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCheckContext {
        infer: Rc<crate::comp::infer::InferenceContext>,
        deferred: Rc<crate::comp::deferred_attr::DeferredAttrContext>,
    }

    impl crate::comp::check::CheckContext for TestCheckContext {
        fn compatible(
            &self,
            _ctx: &mut CompilerContext,
            _syms: &Symtab,
            _types: &Types,
            _found: TypeId,
            _req: TypeId,
            _warn: &mut crate::util::warner::Warner,
        ) -> bool {
            false
        }

        fn report(
            &self,
            _ctx: &mut CompilerContext,
            _pos: i32,
            _details: &crate::util::jc_diagnostic::JCDiagnostic,
        ) {
            unimplemented!("test check context report")
        }

        fn check_warner(
            &self,
            _ctx: &mut CompilerContext,
            _syms: &Symtab,
            _types: &Types,
            _pos: i32,
            _found: TypeId,
            _req: TypeId,
        ) -> crate::util::warner::Warner {
            crate::util::warner::Warner::new(None)
        }

        fn inference_context(&self) -> &Rc<crate::comp::infer::InferenceContext> {
            &self.infer
        }

        fn deferred_attr_context(&self) -> &Rc<crate::comp::deferred_attr::DeferredAttrContext> {
            &self.deferred
        }
    }

    fn test_check_context() -> Rc<dyn crate::comp::check::CheckContext> {
        Rc::new(TestCheckContext {
            infer: crate::comp::infer::empty_context(),
            deferred: Rc::new(crate::comp::deferred_attr::DeferredAttrContext::empty()),
        })
    }

    #[test]
    fn result_info_keep_pkind_and_pt_and_dup() {
        let info = ResultInfo::new(KindSelector::TYP, Some(TypeId::new(3)), test_check_context());
        assert_eq!(info.pkind, KindSelector::TYP);
        assert_eq!(info.pt, Some(TypeId::new(3)));
        let dup = info.dup(Some(TypeId::new(4)));
        assert_eq!(dup.pkind, KindSelector::TYP);
        assert_eq!(dup.pt, Some(TypeId::new(4)));
        let no_pt = info.dup(None);
        assert_eq!(no_pt.pt, None);
        let mode_dup = info.dup_mode(crate::comp::check::CheckMode::NO_TREE_UPDATE);
        assert_eq!(mode_dup.check_mode, crate::comp::check::CheckMode::NO_TREE_UPDATE);
    }

    #[test]
    fn predefined_result_infos_match_the_upstream_kinds() {
        let stat_info = ResultInfo::new(KindSelector::NIL, None, test_check_context());
        assert_eq!(stat_info.pkind, KindSelector::NIL);
        let var_assignment = ResultInfo::new(KindSelector::ASG, None, test_check_context());
        assert_eq!(var_assignment.pkind, KindSelector::ASG);
        let expr = ResultInfo::new(KindSelector::VAL, None, test_check_context());
        assert_eq!(expr.pkind, KindSelector::VAL);
        let ty = ResultInfo::new(KindSelector::TYP, None, test_check_context());
        assert_eq!(ty.pkind, KindSelector::TYP);
        let val_typ = ResultInfo::new(KindSelector::VAL_TYP, None, test_check_context());
        assert_eq!(val_typ.pkind, KindSelector::VAL_TYP);
    }

    #[test]
    fn result_info_to_string_matches_the_upstream_semantics() {
        // Java: public String toString() { return pt != null ? pt.toString() : ""; }
        let no_pt = ResultInfo::new(KindSelector::TYP, None, test_check_context());
        assert_eq!(no_pt.to_string(&CompilerContext::new()), "");
    }

    #[test]
    fn result_info_dup_overloads_keep_context_and_mode() {
        let context = test_check_context();
        let info = ResultInfo::new_with_mode(
            KindSelector::TYP,
            Some(TypeId::new(1)),
            Rc::clone(&context),
            crate::comp::check::CheckMode::NO_INFERENCE_HOOK,
        );
        let same_context = info.dup_context(Rc::clone(&context));
        assert!(Rc::ptr_eq(&same_context.check_context, &context));
        let new_pt = info.dup_pt_context(Some(TypeId::new(2)), Rc::clone(&context));
        assert_eq!(new_pt.pt, Some(TypeId::new(2)));
        let new_mode = info.dup_pt_context_mode(
            Some(TypeId::new(3)),
            context,
            crate::comp::check::CheckMode::NORMAL,
        );
        assert_eq!(new_mode.check_mode, crate::comp::check::CheckMode::NORMAL);
    }

    #[test]
    fn list_reverse_helpers() {
        let list = List::of(TypeId::new(1)).prepend(TypeId::new(2));
        let reversed = list_reverse(list);
        assert_eq!(*reversed.head(), TypeId::new(1));
        assert_eq!(*reversed.tail().head(), TypeId::new(2));
        assert!(list_reverse(List::nil()).is_empty());
    }

    #[test]
    fn literal_value_to_constant_maps_the_value_domain() {
        use crate::tree::jc_tree::LiteralValue;
        use crate::util::constants::ConstantValue;
        assert_eq!(
            literal_value_to_constant(Some(LiteralValue::Integer(5))),
            ConstantValue::Integer(5)
        );
        assert_eq!(
            literal_value_to_constant(Some(LiteralValue::Boolean(true))),
            ConstantValue::Boolean(true)
        );
        assert_eq!(
            literal_value_to_constant(Some(LiteralValue::Character(65))),
            ConstantValue::Character(65)
        );
    }
    use std::io::Read;

    use crate::parser::parser::Parser;
    use crate::util::abstract_log::AbstractLog;

    use javac_tools::error::Error as ToolsError;
    use javac_tools::tools::file_object::FileObject;
    use javac_tools::tools::java_file_manager::{
        FileObjectRef, JavaFileManager, JavaFileObjectRef, Location, LocationRef,
    };
    use javac_tools::tools::java_file_object::{JavaFileObject, Kind};
    use javac_tools::tools::option_checker::OptionChecker;
    use javac_tools::uri::URI;

    use crate::code::deferred_lint_handler::DeferredLintHandler;
    use crate::code::dependencies::DummyDependencies;
    use crate::code::preview::Preview;
    use crate::code::source::Source;
    use crate::comp::member_enter::MemberEnter;
    use crate::comp::type_enter::TypeEnter;
    use crate::comp::type_envs::TypeEnvs;
    use crate::ids::TreeId;
    use crate::jvm::target::Target;
    use crate::parser::parser_factory::ParserFactory;
    use crate::parser::scanner_factory::ScannerFactory;
    use crate::tree::jc_tree::{Tag, TreeKind};
    use crate::tree::tree_maker::TreeMaker;
    use crate::util::context::Context;
    use crate::util::javac_messages::JavacMessages;
    use crate::util::jc_diagnostic::Factory;
    use crate::util::list::List;
    use crate::util::names::Names;
    use crate::util::options::Options;

    /// The development-time Attr oracle (tools/gen-attr-oracle).
    const ATTR_ORACLE: &str =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/attr-oracle.txt"));

    /// The same source as tools/gen-attr-oracle/GenAttrOracle.java.
    const SAMPLE_SOURCE: &str = "package p;\nimport java.util.List;\nclass Sample {\n    int f1;\n    int[] f2;\n    String f3;\n    String[] f4;\n    List<String> f5;\n    List<String[]> f6;\n    List<List<String>> f7;\n}\n";

    struct TestSourceFile {
        name: javac_strings::JavaString,
        base: String,
        content: String,
    }

    impl FileObject for TestSourceFile {
        fn to_uri(&self) -> URI {
            URI::create(&javac_strings::JavaString::from(
                format!("string:///{}", self.name.to_string_lossy()).as_str(),
            ))
        }
        fn get_name(&self) -> javac_strings::JavaString {
            self.name.clone()
        }
        fn open_input_stream(&self) -> Result<Box<dyn Read>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn open_output_stream(&self) -> Result<Box<dyn std::io::Write>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn open_reader(
            &self,
            _ignore_encoding_errors: bool,
        ) -> Result<Box<dyn javac_tools::java_io::Reader>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn get_char_content(
            &self,
            _ignore_encoding_errors: bool,
        ) -> Result<Option<javac_strings::JavaString>, ToolsError> {
            Ok(Some(javac_strings::JavaString::from(self.content.as_str())))
        }
        fn open_writer(&self) -> Result<Box<dyn javac_tools::java_io::Writer>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn get_last_modified(&self) -> i64 {
            0
        }
        fn delete(&self) -> bool {
            false
        }
    }

    impl JavaFileObject for TestSourceFile {
        fn get_kind(&self) -> Kind {
            Kind::SOURCE
        }
        fn is_name_compatible(
            &self,
            simple_name: &javac_strings::JavaString,
            kind: Kind,
        ) -> bool {
            kind == Kind::SOURCE && simple_name.to_string_lossy() == self.base
        }
        fn get_nesting_kind(
            &self,
        ) -> Option<javac_tools::lang::model::element::nesting_kind::NestingKind> {
            None
        }
        fn get_access_level(
            &self,
        ) -> Option<javac_tools::lang::model::element::modifier::Modifier> {
            None
        }
    }

    struct UnusedFileManager;

    impl OptionChecker for UnusedFileManager {
        fn is_supported_option(&self, _option: &javac_strings::JavaString) -> i32 {
            0
        }
    }

    impl JavaFileManager for UnusedFileManager {
        fn get_class_loader(
            &self,
            _location: &dyn Location,
        ) -> Option<Rc<dyn javac_tools::jdk::ClassLoader>> {
            None
        }
        fn list(
            &self,
            _location: &dyn Location,
            _package_name: &javac_strings::JavaString,
            _kinds: &[Kind],
            _recurse: bool,
        ) -> Result<Vec<JavaFileObjectRef>, ToolsError> {
            Ok(Vec::new())
        }
        fn infer_binary_name(
            &self,
            _location: &dyn Location,
            _file: &JavaFileObjectRef,
        ) -> Option<javac_strings::JavaString> {
            None
        }
        fn is_same_file(&self, a: &FileObjectRef, b: &FileObjectRef) -> bool {
            a.get_name().to_string_lossy() == b.get_name().to_string_lossy()
        }
        fn handle_option(
            &self,
            _current: &javac_strings::JavaString,
            _remaining: &mut dyn Iterator<Item = javac_strings::JavaString>,
        ) -> bool {
            false
        }
        fn has_location(&self, _location: &dyn Location) -> bool {
            false
        }
        fn get_java_file_for_input(
            &self,
            _location: &dyn Location,
            _class_name: &javac_strings::JavaString,
            _kind: Kind,
        ) -> Result<Option<JavaFileObjectRef>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn get_java_file_for_output(
            &self,
            _location: &dyn Location,
            _class_name: &javac_strings::JavaString,
            _kind: Kind,
            _sibling: Option<&FileObjectRef>,
        ) -> Result<JavaFileObjectRef, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn get_file_for_input(
            &self,
            _location: &dyn Location,
            _package_name: &javac_strings::JavaString,
            _relative_name: &javac_strings::JavaString,
        ) -> Result<Option<FileObjectRef>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn get_file_for_output(
            &self,
            _location: &dyn Location,
            _package_name: &javac_strings::JavaString,
            _relative_name: &javac_strings::JavaString,
            _sibling: Option<&FileObjectRef>,
        ) -> Result<FileObjectRef, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn flush(&self) -> Result<(), ToolsError> {
            Ok(())
        }
        fn close(&self) -> Result<(), ToolsError> {
            Ok(())
        }
        fn list_locations_for_modules(
            &self,
            _location: &dyn Location,
        ) -> Result<Vec<Vec<LocationRef>>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
    }

    struct AttrHarness {
        ctx: CompilerContext,
        syms: Rc<RefCell<Symtab>>,
        names: Rc<Names>,
        log: Rc<RefCell<Log>>,
        diags: Rc<Factory>,
        options: Rc<Options>,
        class_tree: TreeId,
        env: Rc<Env<AttrContext>>,
        attr: Attr,
    }

    fn build_attr_harness() -> AttrHarness {
        build_attr_harness_with_source(SAMPLE_SOURCE)
    }

    fn build_attr_harness_with_source(source_text: &str) -> AttrHarness {
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = JavacMessages::instance(&mut context);
        let diags = Rc::new(Factory::new(messages, "compiler"));
        let options = Options::instance(&mut context);
        let sourcefile: JavaFileObjectRef = Rc::new(TestSourceFile {
            name: javac_strings::JavaString::from("Sample.java"),
            base: String::from("Sample"),
            content: source_text.to_string(),
        });
        log.borrow_mut().use_source(Some(Rc::clone(&sourcefile)));
        let scanner_factory =
            ScannerFactory::new(&mut context, Rc::clone(&names), Source::DEFAULT);
        let parser_factory = ParserFactory::new(
            &mut context,
            Rc::clone(&names),
            Source::DEFAULT,
            scanner_factory,
            None,
        );
        let mut ctx = CompilerContext::new();
        let mut parser = parser_factory.new_parser(
            &mut ctx,
            &javac_strings::JavaString::from(source_text),
            false,
            true,
            false,
        );
        let unit = parser.parse_compilation_unit(&mut ctx);
        let syms = Rc::new(RefCell::new(Symtab::new(
            &mut ctx,
            Rc::clone(&names),
            Target::JDK1_17,
            Source::JDK17,
        )));
        let java_base = syms.borrow().java_base;
        if let TreeKind::CompilationUnit {
            modle,
            sourcefile: slot,
            ..
        } = &mut ctx.tree_mut(unit).kind
        {
            *modle = Some(java_base);
            *slot = Some(Rc::clone(&sourcefile));
        }
        let types = Rc::new(Types::with_names(Rc::clone(&names)));
        let deferred = Rc::new(RefCell::new(DeferredLintHandler::new()));
        let lint = crate::code::lint::Lint::instance(&mut context);
        let chk = Rc::new(RefCell::new(Check::new(
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&diags),
            Source::JDK17,
            '$',
            Rc::clone(&types),
            lint,
            Rc::clone(&deferred),
            Rc::clone(&syms),
        )));
        let type_envs = Rc::new(RefCell::new(TypeEnvs::new()));
        let dependencies: Rc<RefCell<dyn crate::code::dependencies::Dependencies>> =
            Rc::new(RefCell::new(DummyDependencies));
        let make = Rc::new(RefCell::new(TreeMaker::new(Some(Rc::clone(&names)))));
        let manager: Rc<dyn JavaFileManager> = Rc::new(UnusedFileManager);
        let enter = Rc::new(RefCell::new(Enter::new(
            Rc::clone(&log),
            Rc::clone(&names),
            Rc::clone(&diags),
            Rc::clone(&chk),
            Rc::clone(&types),
            None,
            manager,
            &options,
        )));
        let member_enter = Rc::new(RefCell::new(MemberEnter::new(
            Rc::clone(&enter),
            Rc::clone(&log),
            Rc::clone(&chk),
            Rc::clone(&types),
            Rc::clone(&deferred),
            Rc::new(RefCell::new(None)),
        )));
        let type_enter = Rc::new(RefCell::new(TypeEnter::new(
            Rc::clone(&names),
            Rc::clone(&enter),
            Rc::clone(&member_enter),
            Rc::clone(&log),
            Rc::clone(&chk),
            Rc::clone(&types),
            Rc::clone(&make),
            Rc::clone(&deferred),
            None,
            Rc::clone(&type_envs),
            Rc::clone(&dependencies),
            Rc::new(RefCell::new(None)),
        )));
        let preview = Preview::new(false, false);
        let basic_handler = Rc::clone(&chk.borrow().basic_handler);
        let infer = crate::comp::infer::Infer::new(
            &mut ctx,
            crate::code::source::Feature::GRAPH_INFERENCE
                .allowed_in_source(crate::code::source::Source::JDK17),
            Rc::clone(&diags),
            Rc::clone(&names),
        );
        let attr = Attr::new(
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&chk),
            Rc::clone(&member_enter),
            Rc::clone(&type_enter),
            Rc::clone(&make),
            Rc::clone(&enter),
            Rc::clone(&types),
            Rc::clone(&diags),
            Rc::clone(&deferred),
            Rc::clone(&type_envs),
            Rc::clone(&dependencies),
            basic_handler,
            Rc::clone(&syms),
            Rc::clone(&infer),
            Source::JDK17,
            &options,
            &preview,
        );
        enter.borrow_mut().main(&mut ctx, &mut syms.borrow_mut(), List::of(unit));
        let class_tree = class_tree_in(&ctx, unit);
        let class_sym = class_tree_symbol(&ctx, class_tree).expect("class symbol");
        let top = enter.borrow().get_top_level_env(&mut ctx, unit);
        let members_scope = match &ctx.sym(class_sym).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol {
                members_field, ..
            } => *members_field,
            _ => None,
        };
        let mut info = crate::comp::attr_context::AttrContext::new();
        info.scope = members_scope;
        let env = Rc::new(Env {
            next: None,
            outer: top.outer.clone(),
            tree: Some(class_tree),
            toplevel: Some(unit),
            encl_class: Some(class_tree),
            encl_method: None,
            info: Rc::new(RefCell::new(info)),
            base_clause: false,
        });
        AttrHarness {
            ctx,
            syms,
            names,
            log,
            diags,
            options,
            class_tree,
            env,
            attr,
        }
    }

    fn class_tree_in(ctx: &CompilerContext, unit: TreeId) -> TreeId {
        let defs = match &ctx.tree(unit).kind {
            TreeKind::CompilationUnit { defs, .. } => defs.clone(),
            _ => List::nil(),
        };
        let mut current = defs;
        while current.non_empty() {
            let def = *current.head();
            if ctx.tree(def).has_tag(Tag::CLASSDEF) {
                return def;
            }
            current = current.tail();
        }
        panic!("class declaration not found")
    }

    fn class_defs(ctx: &CompilerContext, class_tree: TreeId) -> List<TreeId> {
        match &ctx.tree(class_tree).kind {
            TreeKind::ClassDecl { defs, .. } => defs.clone(),
            _ => List::nil(),
        }
    }

    fn var_decl_type(ctx: &CompilerContext, var: TreeId) -> TreeId {
        match &ctx.tree(var).kind {
            TreeKind::VariableDecl {
                vartype: Some(vartype),
                ..
            } => *vartype,
            _ => panic!("variable declaration without a type"),
        }
    }

    /// Collects the field type trees in member order (the oracle walks the
    /// ClassTree members in the same order).
    fn type_trees(ctx: &CompilerContext, class_tree: TreeId) -> Vec<TreeId> {
        let mut result = Vec::new();
        let mut current = class_defs(ctx, class_tree);
        while current.non_empty() {
            let def = *current.head();
            if ctx.tree(def).has_tag(Tag::VARDEF) {
                result.push(var_decl_type(ctx, def));
            }
            current = current.tail();
        }
        result
    }

    /// Pre-sets the java.lang/java.util symbols of the identifier type trees:
    /// Resolve is not ported yet (registered: Attr.visitIdent identifier
    /// resolution is the Resolve batch); the tree.sym fast path itself is the
    /// upstream Attr.java:4190-4192 branch.
    fn preset_type_idents(
        ctx: &mut CompilerContext,
        syms: &Symtab,
        names: &Names,
        tree: TreeId,
    ) {
        match ctx.tree(tree).tag {
            Tag::TYPEARRAY => {
                if let TreeKind::ArrayTypeTree {
                    elemtype: Some(elemtype),
                } = &ctx.tree(tree).kind
                {
                    let elemtype = *elemtype;
                    preset_type_idents(ctx, syms, names, elemtype);
                }
            }
            Tag::TYPEAPPLY => {
                let (clazz, arguments) = match &ctx.tree(tree).kind {
                    TreeKind::TypeApply { clazz, arguments } => (*clazz, arguments.clone()),
                    _ => unreachable!(),
                };
                if let Some(clazz) = clazz {
                    preset_type_idents(ctx, syms, names, clazz);
                }
                let mut current = arguments;
                while current.non_empty() {
                    let argument = *current.head();
                    preset_type_idents(ctx, syms, names, argument);
                    current = current.tail();
                }
            }
            Tag::IDENT => {
                let name = match &ctx.tree(tree).kind {
                    TreeKind::Ident { name, .. } => name.clone(),
                    _ => unreachable!(),
                };
                let string_name = names.from_utf(b"String");
                let object_name = names.from_utf(b"Object");
                let list_name = names.from_utf(b"List");
                let symbol = if name.content_equals(&string_name.to_java_string()) {
                    ctx.typ(syms.string_type).tsym
                } else if name.content_equals(&object_name.to_java_string()) {
                    ctx.typ(syms.object_type).tsym
                } else if name.content_equals(&list_name.to_java_string()) {
                    ctx.typ(syms.list_type).tsym
                } else {
                    panic!(
                        "unsupported identifier in the attr oracle source: {}",
                        name.to_java_string().to_string_lossy()
                    )
                };
                let symbol = symbol.expect("predefined type symbol");
                if let TreeKind::Ident { sym: slot, .. } = &mut ctx.tree_mut(tree).kind {
                    *slot = Some(symbol);
                }
            }
            _ => {}
        }
    }

    /// Gives java.util.List its declared type parameter (the real compiler fills
    /// the generic signature from the class file reader; the harness injects
    /// E = Object, the declared bound of List<E>; registered adapter).
    fn inject_list_type_parameters(ctx: &mut CompilerContext, syms: &mut Symtab) {
        let list_type = syms.list_type;
        let object_type = syms.object_type;
        if let crate::code::type_::TypeKindData::ClassType { typarams_field, .. } =
            &mut ctx.typ_mut(list_type).kind
        {
            *typarams_field = Some(List::of(object_type));
        }
    }

    fn encode_type(ctx: &CompilerContext, t: TypeId) -> String {
        let tag = ctx.typ(t).get_tag();
        match tag {
            crate::code::type_tag::TypeTag::ARRAY => {
                let elemtype = match &ctx.typ(t).kind {
                    crate::code::type_::TypeKindData::ArrayType { elemtype, .. } => {
                        elemtype.expect("array element type")
                    }
                    _ => panic!("array type expected"),
                };
                format!("ARRAY[{}]", encode_type(ctx, elemtype))
            }
            crate::code::type_tag::TypeTag::CLASS => {
                let tsym = ctx.typ(t).tsym.expect("class type symbol");
                let qualified = match &ctx.sym(tsym).kind_data {
                    crate::code::symbol::SymbolKind::ClassSymbol { fullname, .. } => {
                        fullname.to_java_string().to_string_lossy()
                    }
                    _ => ctx.sym(tsym).name.to_java_string().to_string_lossy(),
                };
                let arguments = type_arguments(ctx, t);
                if arguments.is_empty() {
                    format!("DECL:{qualified}")
                } else {
                    let mut encoded = String::from("DECL:");
                    encoded.push_str(&qualified);
                    encoded.push('<');
                    let mut current = arguments;
                    let mut first = true;
                    while current.non_empty() {
                        if !first {
                            encoded.push(',');
                        }
                        first = false;
                        let argument = *current.head();
                        encoded.push_str(&encode_type(ctx, argument));
                        current = current.tail();
                    }
                    encoded.push('>');
                    encoded
                }
            }
            crate::code::type_tag::TypeTag::ERROR => String::from("ERROR"),
            _ => {
                if ctx.typ(t).is_primitive() {
                    format!("PRIM:{tag:?}")
                } else {
                    format!("{tag:?}:unsupported")
                }
            }
        }
    }

    fn oracle_entries() -> Vec<String> {
        ATTR_ORACLE
            .lines()
            .filter(|line| line.starts_with("TYPE "))
            .map(|line| line.to_string())
            .collect()
    }

    #[test]
    fn attr_type_attribution_matches_the_javac_oracle() {
        let mut harness = build_attr_harness();
        inject_list_type_parameters(&mut harness.ctx, &mut harness.syms.borrow_mut());
        let trees = type_trees(&harness.ctx, harness.class_tree);
        let mut actual: Vec<String> = Vec::new();
        for (index, tree) in trees.iter().enumerate() {
            let tree = *tree;
            preset_type_idents(&mut harness.ctx, &harness.syms.borrow(), &harness.names, tree);
            let ty = harness
                .attr
                .attrib_type(&mut harness.ctx, &mut harness.syms.borrow_mut(), tree, &harness.env);
            actual.push(format!("TYPE {} {}", index, encode_type(&harness.ctx, ty)));
        }
        let expected = oracle_entries();
        assert_eq!(actual.len(), expected.len(), "type tree count");
        for (index, (actual_line, expected_line)) in actual.iter().zip(expected.iter()).enumerate() {
            assert_eq!(
                actual_line, expected_line,
                "type tree #{index} attribution differs from the javac oracle"
            );
        }
    }

    /// The same source as tools/gen-resolve-oracle/GenResolveOracle.java.
    const RESOLVE_SAMPLE_SOURCE: &str = "class Sample {\n    int f1;\n    String f2;\n    static int f3;\n    class Inner {}\n    int m() {\n        return f1 + f3;\n    }\n    Inner m2() {\n        return null;\n    }\n}\n";

    /// The development-time Resolve oracle (tools/gen-resolve-oracle).
    const RESOLVE_ORACLE: &str =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/resolve-oracle.txt"));

    /// Encodes a resolved symbol in the oracle line format
    /// "REF <name> <kind> <owner-qualified-name>".
    fn resolve_ref_line(ctx: &CompilerContext, symbol: SymbolId) -> String {
        let kind = match ctx.sym(symbol).kind {
            crate::code::kinds::Kind::VAR => "FIELD",
            crate::code::kinds::Kind::TYP => "CLASS",
            crate::code::kinds::Kind::MTH => "METHOD",
            other => return format!("REF {:?}", other),
        };
        let name = ctx.sym(symbol).name.to_java_string().to_string_lossy();
        let owner = ctx
            .sym(symbol)
            .owner
            .map(|owner| match &ctx.sym(owner).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { fullname, .. } => {
                    fullname.to_java_string().to_string_lossy()
                }
                _ => ctx.sym(owner).name.to_java_string().to_string_lossy(),
            })
            .unwrap_or_default();
        format!("REF {} {} {}", name, kind, owner)
    }

    /// Enters the field symbols of the class. The full MemberEnter.visitVarDef
    /// path calls Attr.attribType (TODO(Attr batch), not ported yet), so the
    /// test enters the VarSymbols with the upstream MemberEnter field
    /// construction (name/owner/flags; the type attribution is the registered
    /// Attr batch dependency).
    fn enter_member_fields(harness: &mut AttrHarness) {
        let class_sym = class_tree_symbol(&harness.ctx, harness.class_tree).expect("class symbol");
        let members = harness.ctx.sym(class_sym).members().expect("class members scope");
        let mut current = class_defs(&harness.ctx, harness.class_tree);
        while current.non_empty() {
            let def = *current.head();
            if harness.ctx.tree(def).has_tag(Tag::VARDEF) {
                let pos = harness.ctx.tree(def).pos;
                let (name, flags_value) = match &harness.ctx.tree(def).kind {
                    TreeKind::VariableDecl { name, mods, .. } => {
                        let flags_value = mods
                            .and_then(|mods| match &harness.ctx.tree(mods).kind {
                                TreeKind::Modifiers { flags, .. } => Some(*flags),
                                _ => None,
                            })
                            .unwrap_or(0);
                        (name.clone(), flags_value)
                    }
                    _ => panic!("variable decl expected"),
                };
                let symbol = harness.ctx.alloc_symbol(crate::code::symbol::Symbol::new(
                    crate::code::kinds::Kind::VAR,
                    flags_value,
                    name,
                    None,
                    Some(class_sym),
                    crate::code::symbol::SymbolKind::VarSymbol {
                        pos,
                        adr: 0,
                        data_kind: None,
                        data: None,
                    },
                ));
                harness.ctx.scope_enter(members, symbol);
            }
            current = current.tail();
        }
    }

    /// A method-body environment over the class environment (the Attr
    /// scope.dup()/leave() pattern modelled with an explicit scope).
    fn build_method_env(harness: &mut AttrHarness) -> Rc<Env<AttrContext>> {
        let class_env = Rc::clone(&harness.env);
        let class_tree = harness.class_tree;
        let class_sym = class_tree_symbol(&harness.ctx, class_tree).expect("class symbol");
        let method_scope = harness.ctx.scope_create(Some(class_sym));
        let mut info = crate::comp::attr_context::AttrContext::new();
        info.scope = Some(method_scope);
        Rc::new(Env {
            next: None,
            outer: Some(class_env),
            tree: Some(class_tree),
            toplevel: harness.env.toplevel,
            encl_class: Some(class_tree),
            encl_method: None,
            info: Rc::new(RefCell::new(info)),
            base_clause: false,
        })
    }

    #[test]
    fn resolve_find_ident_matches_the_javac_oracle() {
        use crate::code::kinds::KindSelector;
        use crate::comp::resolve::Resolve;
        let mut harness = build_attr_harness_with_source(RESOLVE_SAMPLE_SOURCE);
        enter_member_fields(&mut harness);
        let method_env = build_method_env(&mut harness);
        let mut resolve = Resolve::new(
            &mut harness.ctx,
            Rc::clone(&harness.names),
            Rc::clone(&harness.log),
            Rc::clone(&harness.attr.types),
            Rc::clone(&harness.diags),
            Source::JDK17,
            Rc::clone(&harness.options),
            None,
        );
        let class_sym = class_tree_symbol(&harness.ctx, harness.class_tree).expect("class symbol");
        let class_type = harness.ctx.sym(class_sym).ty.expect("class type");
        let mut actual = Vec::new();
        for line in RESOLVE_ORACLE.lines() {
            let fields: Vec<&str> = line.split(' ').collect();
            if fields.first() != Some(&"REF") {
                continue;
            }
            let name = harness.names.from_utf(fields[1].as_bytes());
            let selector = match fields[2] {
                "FIELD" => KindSelector::VAL,
                "CLASS" => KindSelector::TYP,
                other => panic!("unsupported oracle kind {}", other),
            };
            let symbol = resolve.find_ident_in_type(
                &mut harness.ctx,
                &mut harness.syms.borrow_mut(),
                None,
                &method_env,
                class_type,
                &name,
                selector,
            );
            actual.push(resolve_ref_line(&harness.ctx, symbol));
        }
        let expected: Vec<String> = RESOLVE_ORACLE
            .lines()
            .filter(|line| line.starts_with("REF "))
            .map(|line| line.to_string())
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn resolve_find_ident_finds_fields_and_member_types() {
        use crate::code::kinds::KindSelector;
        use crate::comp::resolve::Resolve;
        let mut harness = build_attr_harness_with_source(RESOLVE_SAMPLE_SOURCE);
        enter_member_fields(&mut harness);
        let method_env = build_method_env(&mut harness);
        let mut resolve = Resolve::new(
            &mut harness.ctx,
            Rc::clone(&harness.names),
            Rc::clone(&harness.log),
            Rc::clone(&harness.attr.types),
            Rc::clone(&harness.diags),
            Source::JDK17,
            Rc::clone(&harness.options),
            None,
        );
        // findIdent(VAL) goes through findVar/findField from the method env.
        let f1_name = harness.names.from_utf(b"f1");
        let f1 = resolve.find_ident(
            &mut harness.ctx,
            &mut harness.syms.borrow_mut(),
            None,
            &method_env,
            &f1_name,
            KindSelector::VAL,
        );
        assert_eq!(harness.ctx.sym(f1).name, f1_name);
        assert_eq!(harness.ctx.sym(f1).kind, crate::code::kinds::Kind::VAR);
        // findIdentInType(TYP) finds the member type Inner.
        let class_sym = class_tree_symbol(&harness.ctx, harness.class_tree).expect("class symbol");
        let class_type = harness.ctx.sym(class_sym).ty.expect("class type");
        let inner_name = harness.names.from_utf(b"Inner");
        let inner = resolve.find_ident_in_type(
            &mut harness.ctx,
            &mut harness.syms.borrow_mut(),
            None,
            &method_env,
            class_type,
            &inner_name,
            KindSelector::TYP,
        );
        assert_eq!(harness.ctx.sym(inner).name, inner_name);
        assert_eq!(harness.ctx.sym(inner).kind, crate::code::kinds::Kind::TYP);
        // Unknown member: the typeNotFound error symbol.
        let missing = harness.names.from_utf(b"missing");
        let not_found = resolve.find_ident_in_type(
            &mut harness.ctx,
            &mut harness.syms.borrow_mut(),
            None,
            &method_env,
            class_type,
            &missing,
            KindSelector::VAL,
        );
        assert!(crate::code::symbol::symbol_resolve_error_data(&harness.ctx, not_found).is_some());
    }


    /// The development-time Resolve error-key oracle
    /// (tools/gen-resolve-error-oracle).
    const RESOLVE_ERROR_ORACLE: &str =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/resolve-error-oracle.txt"));

    #[test]
    fn resolve_error_keys_match_the_javac_oracle() {
        use crate::comp::resolve::Resolve;
        use crate::util::jc_diagnostic::DiagnosticType;
        let mut harness = build_attr_harness_with_source(RESOLVE_SAMPLE_SOURCE);
        enter_member_fields(&mut harness);
        let class_sym = class_tree_symbol(&harness.ctx, harness.class_tree).expect("class symbol");
        let class_type = harness.ctx.sym(class_sym).ty.expect("class type");
        let mut resolve = Resolve::new(
            &mut harness.ctx,
            Rc::clone(&harness.names),
            Rc::clone(&harness.log),
            Rc::clone(&harness.attr.types),
            Rc::clone(&harness.diags),
            Source::JDK17,
            Rc::clone(&harness.options),
            None,
        );
        let unknown_var = harness.names.from_utf(b"unknownVar");
        let unknown_type = harness.names.from_utf(b"UnknownType");
        // E1: unknown variable (SymbolNotFoundError, ABSENT_VAR).
        let e1 = resolve
            .resolve_error_diagnostic(
                &mut harness.ctx,
                &harness.syms.borrow(),
                resolve.var_not_found,
                DiagnosticType::ERROR,
                None,
                Some(class_sym),
                Some(class_type),
                Some(&unknown_var),
                None,
                None,
            )
            .expect("E1 diagnostic");
        // E2: unknown type (SymbolNotFoundError, ABSENT_TYP).
        let e2 = resolve
            .resolve_error_diagnostic(
                &mut harness.ctx,
                &harness.syms.borrow(),
                resolve.type_not_found,
                DiagnosticType::ERROR,
                None,
                Some(class_sym),
                Some(class_type),
                Some(&unknown_type),
                None,
                None,
            )
            .expect("E2 diagnostic");
        // E3: instance field referenced from a static context (StaticError).
        let f1_name = harness.names.from_utf(b"f1");
        let members = harness.ctx.sym(class_sym).members().expect("class members");
        let f1 = harness
            .ctx
            .scope_get_symbols_by_name(
                members,
                &f1_name,
                crate::code::scope::LookupKind::RECURSIVE,
                &|_| true,
            )
            .into_iter()
            .next()
            .expect("f1 symbol");
        let static_error = crate::code::symbol::symbol_new_resolve_error(
            &mut harness.ctx,
            crate::code::kinds::Kind::STATICERR,
            harness.names.empty.clone(),
            "static error",
            crate::code::symbol::ResolveErrorData::Static { sym: f1 },
        );
        let e3 = resolve
            .resolve_error_diagnostic(
                &mut harness.ctx,
                &harness.syms.borrow(),
                static_error,
                DiagnosticType::ERROR,
                None,
                None,
                Some(class_type),
                Some(&f1_name),
                None,
                None,
            )
            .expect("E3 diagnostic");
        // E4: private field access (AccessError -> report.access).
        let private_field = harness.ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            crate::code::kinds::Kind::VAR,
            crate::code::flags::PRIVATE as i64,
            harness.names.from_utf(b"secret"),
            harness.ctx.sym(class_sym).ty,
            Some(class_sym),
            crate::code::symbol::SymbolKind::VarSymbol {
                pos: 0,
                adr: 0,
                data_kind: None,
                data: None,
            },
        ));
        let env_snapshot = crate::comp::resolve::resolve_env_snapshot(&harness.ctx, &harness.env);
        let access_error = crate::code::symbol::symbol_new_resolve_error(
            &mut harness.ctx,
            crate::code::kinds::Kind::HIDDEN,
            harness.names.empty.clone(),
            "access error",
            crate::code::symbol::ResolveErrorData::Access {
                env: env_snapshot,
                site: Some(class_type),
                site_inaccessible: false,
                sym: private_field,
            },
        );
        let secret_name = harness.names.from_utf(b"secret");
        let e4 = resolve
            .resolve_error_diagnostic(
                &mut harness.ctx,
                &harness.syms.borrow(),
                access_error,
                DiagnosticType::ERROR,
                None,
                Some(class_sym),
                Some(class_type),
                Some(&secret_name),
                None,
                None,
            )
            .expect("E4 diagnostic");
        let actual = vec![
            format!("E1 {}", e1.get_diagnostic_info().key()),
            format!("E2 {}", e2.get_diagnostic_info().key()),
            format!("E3 {}", e3.get_diagnostic_info().key()),
            format!("E4 {}", e4.get_diagnostic_info().key()),
        ];
        let expected: Vec<String> = RESOLVE_ERROR_ORACLE
            .lines()
            .filter(|line| line.starts_with("ERR "))
            .map(|line| line.replace("ERR ", ""))
            .collect();
        assert_eq!(actual, expected);
    }

    const ATTR_B_SOURCE: &str = "\
class Sample {
    void m() {
        label: while (true) {
            break label;
        }
    }
}
";
    fn first_class_defs(ctx: &CompilerContext, class_tree: TreeId) -> List<TreeId> {
        match &ctx.tree(class_tree).kind {
            TreeKind::ClassDecl { defs, .. } => defs.clone(),
            _ => List::nil(),
        }
    }
    fn method_body(ctx: &CompilerContext, method_tree: TreeId) -> Option<TreeId> {
        match &ctx.tree(method_tree).kind {
            TreeKind::MethodDecl { body, .. } => *body,
            _ => None,
        }
    }
    fn block_stats(ctx: &CompilerContext, block_tree: TreeId) -> List<TreeId> {
        match &ctx.tree(block_tree).kind {
            TreeKind::Block { stats, .. } => stats.clone(),
            _ => List::nil(),
        }
    }
    fn labeled_body(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
        match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement { body, .. } => *body,
            _ => None,
        }
    }
    fn while_body(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
        match &ctx.tree(tree).kind {
            TreeKind::WhileLoop { body, .. } => *body,
            _ => None,
        }
    }
    /// Locate the labelled/while/break trees of ATTR_B_SOURCE.
    fn attr_b_trees(harness: &AttrHarness) -> (TreeId, TreeId, TreeId) {
        let class_defs = first_class_defs(&harness.ctx, harness.class_tree);
        let mut current = class_defs;
        let mut method = None;
        while current.non_empty() {
            let def = *current.head();
            if harness.ctx.tree(def).has_tag(Tag::METHODDEF) {
                method = Some(def);
                break;
            }
            current = current.tail();
        }
        let method = method.expect("method definition");
        let body = method_body(&harness.ctx, method).expect("method body");
        let stats = block_stats(&harness.ctx, body);
        let labelled = *stats.head();
        let while_tree = labeled_body(&harness.ctx, labelled).expect("while loop");
        let while_body_tree = while_body(&harness.ctx, while_tree).expect("while body");
        let break_tree = *block_stats(&harness.ctx, while_body_tree).head();
        (labelled, while_tree, break_tree)
    }

    #[test]
    fn referenced_statement_skips_labels_to_the_loop() {
        let harness = build_attr_harness_with_source(ATTR_B_SOURCE);
        let (labelled, while_tree, _break_tree) = attr_b_trees(&harness);
        let referenced =
            crate::tree::tree_info::referenced_statement(&harness.ctx, labelled);
        assert_eq!(referenced, while_tree);
    }

    #[test]
    fn find_jump_target_resolves_an_unlabelled_break_to_the_loop() {
        let harness = build_attr_harness_with_source(ATTR_B_SOURCE);
        let (_labelled, while_tree, _break_tree) = attr_b_trees(&harness);
        let mut info = crate::comp::attr_context::AttrContext::new();
        info.scope = None;
        let env = Rc::new(Env {
            next: None,
            outer: None,
            tree: Some(while_tree),
            toplevel: None,
            encl_class: None,
            encl_method: None,
            info: Rc::new(RefCell::new(info)),
            base_clause: false,
        });
        let (target, error) = harness.attr.find_jump_target_no_error(
            &harness.ctx,
            &mut harness.syms.borrow_mut(),
            Tag::BREAK,
            &None,
            &env,
        );
        assert_eq!(target, Some(while_tree));
        assert!(error.is_none());
    }

    #[test]
    fn visit_break_sets_the_target_tree() {
        let mut harness = build_attr_harness_with_source(ATTR_B_SOURCE);
        let (labelled, while_tree, break_tree) = attr_b_trees(&harness);
        // The env chain models the enclosing statements from innermost out:
        // while (inner) -> label (outer) -> method/class environment.
        let outer_env = Rc::new(Env {
            next: None,
            outer: None,
            tree: Some(labelled),
            toplevel: None,
            encl_class: None,
            encl_method: None,
            info: Rc::new(RefCell::new(crate::comp::attr_context::AttrContext::new())),
            base_clause: false,
        });
        let env = Rc::new(Env {
            next: Some(outer_env),
            outer: None,
            tree: Some(while_tree),
            toplevel: None,
            encl_class: None,
            encl_method: None,
            info: Rc::new(RefCell::new(crate::comp::attr_context::AttrContext::new())),
            base_clause: false,
        });
        let stat_info = harness.attr.stat_info.clone();
        let result = harness
            .attr
            .attrib_tree(&mut harness.ctx, &mut harness.syms.borrow_mut(), break_tree, &env, stat_info)
            .expect("break attribution");
        let _ = result;
        let target = match &harness.ctx.tree(break_tree).kind {
            TreeKind::Break { target, .. } => *target,
            _ => panic!("expected break tree"),
        };
        // Upstream findJumpTargetNoError returns the labelled statement itself
        // for a labelled break (Attr.java:2352-2362).
        assert_eq!(target, Some(labelled));
    }

}

