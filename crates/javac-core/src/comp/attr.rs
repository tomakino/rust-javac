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
    pub argument_attr_exclude: Option<crate::comp::argument_attr::UniquePos>,
    /// Java: Attr.RecoveryInfo overrides check() to wrap the result in
    /// chk.checkNonVoid (Attr.java:598-620); expressed as a flag (§9 registered).
    pub check_non_void: bool,
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
            check_non_void: false,
            kind: ResultInfoKind::Normal,
            argument_attr_exclude: None,
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
    pub fn needs_argument_attr(
        &self,
        ctx: &CompilerContext,
        tree: TreeId,
    ) -> bool {
        if !matches!(self.kind, ResultInfoKind::MethodAttr) {
            return false;
        }
        match &self.argument_attr_exclude {
            Some(exclude) => exclude.pos != ctx.tree(tree).pos,
            None => true,
        }
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
        let result = chk.check_type(ctx, syms, pos, found, pt, &*self.check_context);
        if self.check_non_void {
            // Java: RecoveryInfo overrides check() to wrap the result with
            // chk.checkNonVoid (Attr.java:598-620); §9 flag-based expression.
            chk.check_non_void(ctx, pos, result)
        } else {
            result
        }
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
            info.check_non_void = self.check_non_void;
            return info;
        }
        let mut result = ResultInfo::new_with_mode(self.pkind, self.pt, check_context, self.check_mode);
        result.check_non_void = self.check_non_void;
        result
    }

    /// Java: protected ResultInfo dup(Type newPt)
    pub fn dup(&self, new_pt: Option<TypeId>) -> ResultInfo {
        if matches!(self.kind, ResultInfoKind::MethodAttr) {
            // Java: MethodAttrInfo.dup(Type) throws IllegalStateException.
            panic!("IllegalStateException");
        }
        let mut result = ResultInfo::new_with_mode(
            self.pkind,
            new_pt,
            Rc::clone(&self.check_context),
            self.check_mode,
        );
        result.check_non_void = self.check_non_void;
        result
    }

    /// Java: protected ResultInfo dup(CheckContext newContext)
    pub fn dup_context(&self, new_context: Rc<dyn crate::comp::check::CheckContext>) -> ResultInfo {
        if matches!(self.kind, ResultInfoKind::MethodAttr) {
            // Java: MethodAttrInfo.dup(CheckContext) -> new MethodAttrInfo(newContext)
            let mut info = ResultInfo::new_with_mode(self.pkind, self.pt, new_context, self.check_mode);
            info.kind = ResultInfoKind::MethodAttr;
            info.check_non_void = self.check_non_void;
            return info;
        }
        let mut result = ResultInfo::new_with_mode(
            self.pkind,
            self.pt,
            new_context,
            self.check_mode,
        );
        result.check_non_void = self.check_non_void;
        result
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
        let mut result = ResultInfo::new_with_mode(self.pkind, new_pt, new_context, self.check_mode);
        result.check_non_void = self.check_non_void;
        result
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
        let mut result = ResultInfo::new_with_mode(self.pkind, new_pt, new_context, new_mode);
        result.check_non_void = self.check_non_void;
        result
    }

    /// Java: protected ResultInfo dup(CheckMode newMode)
    pub fn dup_mode(&self, new_mode: crate::comp::check::CheckMode) -> ResultInfo {
        if matches!(self.kind, ResultInfoKind::MethodAttr) {
            panic!("IllegalStateException");
        }
        let mut result = ResultInfo::new_with_mode(
            self.pkind,
            self.pt,
            Rc::clone(&self.check_context),
            new_mode,
        );
        result.check_non_void = self.check_non_void;
        result
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
    /// Java: final AttrRecover attrRecover (Attr.java:629 area) — installed
    /// lazily through a cell to break the Attr/AttrRecover cycle (§9).
    pub attr_recover: Rc<RefCell<Option<Rc<RefCell<crate::comp::attr_recover::AttrRecover>>>>>,

    /// Java: final Symtab syms (injected service, ADR 0001)
    pub syms: Rc<RefCell<Symtab>>,
    /// Java: final Infer infer (injected service, ADR 0001)
    pub infer: Rc<crate::comp::infer::Infer>,
    /// Java: final Resolve rs (cell: Resolve and Attr are mutually recursive
    /// services, §9; installed by the wiring).
    pub rs: Rc<RefCell<Option<Rc<RefCell<crate::comp::resolve::Resolve>>>>>,
    /// Java: final ArgumentAttr argumentAttr (§9 cell, installed by the wiring).
    pub argument_attr: Rc<RefCell<Option<Rc<RefCell<crate::comp::argument_attr::ArgumentAttr>>>>>,
    /// Java: final DeferredAttr deferredAttr (§9 cell, installed by the wiring).
    pub deferred_attr: Rc<RefCell<Option<Rc<RefCell<crate::comp::deferred_attr::DeferredAttrService>>>>>,
    /// Java: MatchBindings matchBindings (Attr.java:451) — the current flow
    /// bindings, rewritten by the pattern-matching visitors.
    pub match_bindings: crate::comp::match_bindings::MatchBindings,
    /// Java: final MatchBindingsComputer matchBindingsComputer (Attr.java:632).
    pub match_bindings_computer: crate::comp::match_bindings::MatchBindingsComputer,
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
        let mb_log = Rc::clone(&log);
        let mb_diags = Rc::clone(&diags);
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
            rs: Rc::new(RefCell::new(None)),
            argument_attr: Rc::new(RefCell::new(None)),
            deferred_attr: Rc::new(RefCell::new(None)),
            match_bindings: crate::comp::match_bindings::MatchBindings::empty(),
            match_bindings_computer: crate::comp::match_bindings::MatchBindingsComputer::new(
                mb_log, mb_diags,
            ),
            attr_recover: Rc::new(RefCell::new(None)),
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
            // Java: log.error(tree.pos(), Errors.UnexpectedType(
            //     resultInfo.pkind.kindNames(), ownkind.kindNames()));
            // (Attr.java:518-525)
            let expected = result_info
                .pkind
                .kind_names()
                .into_iter()
                .map(|kind| {
                    crate::resources::diagnostic_contract::KindNameArg::placeholder(
                        kind.name().to_string(),
                    )
                })
                .collect();
            let found_kinds = ownkind
                .kind_names()
                .into_iter()
                .map(|kind| {
                    crate::resources::diagnostic_contract::KindNameArg::placeholder(
                        kind.name().to_string(),
                    )
                })
                .collect();
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                ctx.tree(tree).pos,
                crate::util::jc_diagnostic::Error::from(
                    crate::resources::compiler_properties::errors::unexpected_type(expected, found_kinds),
                ),
            );
            self.types.create_error_type_original(ctx, Some(found), syms)
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
        let needs_argument_attr = result_info.needs_argument_attr(ctx, tree);
        self.env = Some(Rc::clone(env));
        self.result_info = Some(result_info);
        let result = if needs_argument_attr {
            // Java: result = argumentAttr.attribArg(tree, env); (Attr.java:674-676)
            let argument_attr = self
                .argument_attr
                .borrow()
                .clone()
                .expect("AssertionError: argumentAttr service must be installed");
            let mut argument_attr = argument_attr.borrow_mut();
            let log_rc = Rc::clone(&self.log);
            let log = log_rc.borrow();
            match argument_attr.attrib_arg(ctx, syms, self, &log, tree, env) {
                Ok(result) => Ok(result.expect("AssertionError: argument attribution result")),
                Err(failure) => Err(failure),
            }
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
            Tag::APPLY => self.visit_apply(ctx, syms, tree),
            Tag::SELECT => self.visit_select(ctx, syms, tree),
            Tag::NEWARRAY => self.visit_new_array(ctx, syms, tree),
            Tag::CONDEXPR => self.visit_conditional(ctx, syms, tree),
            Tag::ASSERT => self.visit_assert(ctx, syms, tree),
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
                // Java: log.error(tree.pos(), Errors.WrongNumberTypeArgs(
                //     Integer.toString(formals.length()))) / Errors.TypeDoesntTakeParams(
                //     clazztype.tsym); (Attr.java:2125-2135)
                let formals_len = list_len_type(&formals);
                let diag = crate::resources::compiler_properties::errors::wrong_number_type_args(
                    crate::util::java_string::JavaString::from(formals_len.to_string().as_str()),
                );
                crate::util::abstract_log::AbstractLog::error_at(
                    &mut *self.log.borrow_mut(),
                    ctx.tree(tree).pos,
                    crate::util::jc_diagnostic::Error::from(diag),
                );
                let clazz_tsym = ctx.typ(clazztype).tsym;
                if let Some(clazz_tsym) = clazz_tsym {
                    let diag = crate::resources::compiler_properties::errors::type_doesnt_take_params(
                        crate::resources::diagnostic_contract::SymbolArg::placeholder(
                            ctx.sym(clazz_tsym).name.to_java_string().to_string_lossy().to_string(),
                        ),
                    );
                    crate::util::abstract_log::AbstractLog::error_at(
                        &mut *self.log.borrow_mut(),
                        ctx.tree(tree).pos,
                        crate::util::jc_diagnostic::Error::from(diag),
                    );
                }
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
            // Java: env.info.pendingResolutionPhase = null;
            // sym = rs.resolveMethod(tree.pos(), env, tree.name,
            //     pt().getParameterTypes(), pt().getTypeArguments());
            env.info.borrow_mut().pending_resolution_phase = None;
            let pt = pt.expect("method prototype");
            let argtypes = self.types.get_parameter_types_public(ctx, pt);
            let typeargtypes = self.types.get_type_arguments_public(ctx, pt);
            let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
            let mut rs = rs_rc.borrow_mut();
            let chk_rc = Rc::clone(&self.chk);
            let chk = chk_rc.borrow();
            let infer_rc = Rc::clone(&self.infer);
            let types_rc = Rc::clone(&self.types);
            let pos = simple_diag_pos(ctx.tree(tree).pos);
            sym = rs.resolve_method(
                ctx,
                syms,
                &types_rc,
                self,
                &chk,
                &infer_rc,
                pos,
                &env,
                name.clone(),
                argtypes,
                Some(typeargtypes),
            );
        } else if let Some(tree_sym) = ident_symbol(ctx, tree) {
            if ctx.sym(tree_sym).kind != Kind::VAR {
                sym = tree_sym;
            } else {
                // Java: sym = rs.resolveIdent(tree.pos(), env, tree.name, pkind());
                let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
                let mut rs = rs_rc.borrow_mut();
                let pos = simple_diag_pos(ctx.tree(tree).pos);
                sym = rs.resolve_ident(ctx, syms, pos, &env, name.clone(), self.pkind());
            }
        } else {
            // Java: sym = rs.resolveIdent(tree.pos(), env, tree.name, pkind());
            let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
            let mut rs = rs_rc.borrow_mut();
            let pos = simple_diag_pos(ctx.tree(tree).pos);
            sym = rs.resolve_ident(ctx, syms, pos, &env, name.clone(), self.pkind());
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
            // Java: checkInit(tree, env, v, false);
            self.check_init(ctx, syms, tree, &env, sym, false);
            // Java: if (KindSelector.ASG.subset(pkind())) checkAssignable(tree.pos(), v, null, env);
            if crate::code::kinds::KindSelector::ASG.subset(self.pkind()) {
                self.check_assignable(ctx, &env, ctx.tree(tree).pos, sym, None);
            }
        }
        let sym_owner = ctx.sym(sym).owner;
        let sym_owner_is_typ = sym_owner.map(|owner| ctx.sym(owner).kind) == Some(Kind::TYP);
        let sym_is_static = (ctx.sym(sym).flags_field & crate::code::flags::STATIC as i64) != 0;
        if (sym_env.info.borrow().is_self_call || no_outer_this_path)
            && is_val_mth
            && sym_owner_is_typ
            && !sym_is_static
        {
            // Java: chk.earlyRefError(tree.pos(), sym.kind == VAR ? sym :
            //     thisSym(tree.pos(), env));
            let err_sym = if sym_kind == Kind::VAR {
                sym
            } else {
                self.this_sym(ctx, syms, ctx.tree(tree).pos, &env)
            };
            let chk_rc = Rc::clone(&self.chk);
            let chk = chk_rc.borrow();
            chk.early_ref_error(ctx, &self.log, ctx.tree(tree).pos, err_sym);
        }
        let mut env1 = Rc::clone(&env);
        if sym_kind != Kind::ERR
            && sym_kind != Kind::TYP
            && sym_owner.is_some()
            && sym_owner != Some(encl_sym)
        {
            // If the found symbol is inaccessible, then it is accessed through
            // an enclosing instance. Locate this enclosing instance.
            loop {
                if env1.outer.is_none() {
                    break;
                }
                let encl1 = env1.encl_class.expect("env1 enclClass");
                let encl1_sym = class_tree_symbol(ctx, encl1).expect("env1 enclClass sym");
                let site1 = ctx.sym(encl1_sym).ty.expect("env1 enclClass type");
                let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
                let mut rs = rs_rc.borrow_mut();
                if rs.is_accessible_member(ctx, syms, &env, site1, sym, false) {
                    break;
                }
                env1 = Rc::clone(env1.outer.as_ref().expect("env1 outer"));
            }
        }
        if env.info.borrow().is_serializable {
            // NOT PORTED: chk.checkAccessFromSerializableElement(tree,
            // env.info.isSerializableLambda) (Check.java:2745-2800, lint/serial
            // batch); registered.
        }
        let site = env1
            .encl_class
            .and_then(|encl| class_tree_symbol(ctx, encl))
            .and_then(|encl_sym| ctx.sym(encl_sym).ty)
            .expect("env1 enclClass type");
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
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        site: TypeId,
        sym: SymbolId,
        env: &Rc<Env<AttrContext>>,
        result_info: &ResultInfo,
    ) -> TypeId {
        // Java: if (resultInfo.pkind.contains(KindSelector.POLY))
        //   return attrRecover.recoverMethodInvocation(tree, site, sym, env, resultInfo);
        // else return checkIdInternal(tree, site, sym, resultInfo.pt, env, resultInfo);
        if result_info.pkind.contains(crate::code::kinds::KindSelector::POLY) {
            let recover_rc = self.attr_recover.borrow().clone();
            let recover_rc = recover_rc.expect(
                "AttrRecover instance is not installed on this Attr (service wiring pending)",
            );
            let chk_rc = Rc::clone(&self.chk);
            let diags = Rc::clone(&self.diags);
            let chk_guard = chk_rc.borrow();
            let recover = recover_rc.borrow();
            return recover.recover_method_invocation(
                ctx,
                syms,
                self,
                &chk_guard,
                &diags,
                tree,
                site,
                sym,
                env,
                result_info,
            );
        }
        self.check_id_internal(ctx, syms, tree, site, sym, result_info.pt, env, result_info)
    }

    /// Java: Type checkIdInternal(JCTree tree, Type site, Symbol sym, Type pt,
    /// Env<AttrContext> env, ResultInfo resultInfo) (Attr.java:4520-4620)
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::needless_late_init)]
    pub(crate) fn check_id_internal(
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
                            // Java: normOuter = types.asEnclosingSuper(site,
                            //     ownOuter.tsym); (Attr.java:4555-4565)
                            let mut norm_outer = site;
                            if ctx.typ(norm_outer).has_tag(TypeTag::CLASS) {
                                let own_outer_tsym = ctx
                                    .typ(own_outer)
                                    .tsym
                                    .expect("enclosing type symbol");
                                norm_outer = match self.types.as_enclosing_super(
                                    ctx,
                                    syms,
                                    norm_outer,
                                    own_outer_tsym,
                                ) {
                                    Some(super_type) => super_type,
                                    None => {
                                        // perhaps from an import
                                        norm_outer = self.types.erasure(ctx, syms, own_outer);
                                        norm_outer
                                    }
                                };
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
                // Java: VarSymbol v = (VarSymbol)sym; (Attr.java:4560-4600)
                // NOT PORTED: VarSymbol.getConstValue (Symbol.java:1190-1210,
                // ConstValue batch) — the constant-value sub-branch is skipped
                // and registered; everything else follows the upstream body.
                let sym_ty = ctx.sym(sym).ty.expect("symbol type");
                let encl_var = env.info.borrow().encl_var;
                if encl_var.is_some() && ctx.typ(sym_ty).has_tag(TypeTag::NONE) {
                    // self reference to implicitly typed variable declaration
                    let diag = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                        crate::resources::compiler_properties::fragments::local_self_ref(),
                    ));
                    crate::util::abstract_log::AbstractLog::error_at(
                        &mut *self.log.borrow_mut(),
                        ctx.tree(tree).pos,
                        crate::util::jc_diagnostic::Error::from(
                            crate::resources::compiler_properties::errors::cant_infer_local_var_type(
                                ctx.sym(sym).name.clone(),
                                crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(
                                    Rc::new(diag),
                                ),
                            ),
                        ),
                    );
                    let error = self.types.create_error_type_original(ctx, Some(sym_ty), syms);
                    ctx.sym_mut(sym).ty = Some(error);
                    return error;
                }
                // Test (4): if symbol is an instance field of a raw type,
                // which is being assigned to, issue an unchecked warning if
                // its type changes under erasure.
                let pkind = result_info.pkind;
                let owner = ctx.sym(sym).owner;
                let site_is_class_or_typevar = ctx.typ(site).has_tag(TypeTag::CLASS)
                    || ctx.typ(site).has_tag(TypeTag::TYPEVAR);
                if crate::code::kinds::KindSelector::ASG.subset(pkind)
                    && owner
                        .map(|owner| ctx.sym(owner).kind == crate::code::kinds::Kind::TYP)
                        .unwrap_or(false)
                    && ctx.sym(sym).flags_field & (crate::code::flags::STATIC as i64) == 0
                    && site_is_class_or_typevar
                {
                    let owner = owner.expect("variable owner");
                    let s = self.types.as_outer_super(ctx, syms, site, owner);
                    if let Some(s) = s {
                        let sym_type = ctx.sym(sym).ty.expect("symbol type");
                        let erased = self.types.erasure(ctx, syms, sym_type);
                        if self.types.is_raw_type_public(ctx, s)
                            && !self.types.is_same_type(ctx, syms, sym_type, erased)
                        {
                            let warning =
                                crate::resources::compiler_properties::warnings::unchecked_assign_to_var(
                                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                        ctx.sym(sym).name.to_java_string().to_string_lossy().to_string(),
                                    ),
                                    crate::resources::diagnostic_contract::TypeArg::placeholder(
                                        crate::comp::attr::type_display_of(ctx, s),
                                    ),
                                );
                            self.chk.borrow().warn_unchecked(ctx.tree(tree).pos, warning);
                        }
                    }
                }
                let name = ctx.sym(sym).name.clone();
                let mut own = if ctx
                    .sym(sym)
                    .owner
                    .map(|owner| ctx.sym(owner).kind == crate::code::kinds::Kind::TYP)
                    .unwrap_or(false)
                    && name != self.names._this
                    && name != self.names._super
                {
                    self.types.member_type(ctx, syms, site, sym)
                } else {
                    ctx.sym(sym).ty.expect("symbol type")
                };
                // NOT PORTED: if (v.getConstValue() != null &&
                // isStaticReference(tree)) owntype = owntype.constType(...)
                // (Symbol/ConstValue batch; registered).
                if pkind == crate::code::kinds::KindSelector::VAL {
                    own = self.types.capture(ctx, syms, own);
                }
                owntype = own;
            }
            Kind::MTH => {
                // Java: owntype = checkMethod(site, sym, new ResultInfo(
                //     resultInfo.pkind, resultInfo.pt.getReturnType(),
                //     resultInfo.checkContext, resultInfo.checkMode),
                //     env, TreeInfo.args(env.tree), resultInfo.pt.getParameterTypes(),
                //     resultInfo.pt.getTypeArguments()); (Attr.java:4630-4640)
                let pt = result_info.pt.expect("method result type");
                let restype = self.types.get_return_type_public(ctx, pt);
                let argtypes = self.types.get_parameter_types_public(ctx, pt);
                let targs = self.types.get_type_arguments_public(ctx, pt);
                let new_ri = ResultInfo::new_with_mode(
                    result_info.pkind,
                    Some(restype),
                    Rc::clone(&result_info.check_context),
                    result_info.check_mode,
                );
                let rs_rc = self
                    .rs
                    .borrow()
                    .clone()
                    .expect("AssertionError: rs service must be installed");
                let mut rs = rs_rc.borrow_mut();
                let infer_rc = Rc::clone(&self.infer);
                let argtrees = env
                    .tree
                    .and_then(|tree| crate::tree::tree_info::args(ctx, tree));
                let value = self.check_method(
                    ctx,
                    syms,
                    &mut rs,
                    &infer_rc,
                    site,
                    sym,
                    &new_ri,
                    env,
                    argtrees.as_ref(),
                    argtypes,
                    Some(targs),
                );
                owntype = value;
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
                std::rc::Rc::new(javac_strings::JavaString::from(arg.to_text().as_str()))
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
        panic!("AssertionError")
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
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) -> SymbolId {
        // Java: return tree.accept(identAttributer, env); (Attr.java:370-375)
        // IdentAttributer (Attr.java:377-410):
        use crate::code::kinds::Kind;
        match ctx.tree(tree).tag {
            Tag::SELECT => {
                let (selected, name) = match &ctx.tree(tree).kind {
                    TreeKind::FieldAccess { selected, name, .. } => (*selected, name.clone()),
                    _ => panic!("ClassCastException: JCFieldAccess expected"),
                };
                let selected = selected.expect("member select qualifier");
                let site = self.attrib_ident(ctx, syms, selected, env);
                let site_kind = ctx.sym(site).kind;
                if site_kind == Kind::ERR
                    || site_kind == Kind::ABSENT_TYP
                    || site_kind == Kind::HIDDEN
                {
                    return site;
                }
                if site_kind == Kind::PCK {
                    // Java: env.toplevel.packge = (PackageSymbol)site;
                    let toplevel = env.toplevel.expect("env.toplevel");
                    if let TreeKind::CompilationUnit { packge, .. } =
                        &mut ctx.tree_mut(toplevel).kind
                    {
                        *packge = Some(site);
                    }
                    let rs_rc = self
                        .rs
                        .borrow()
                        .clone()
                        .expect("AssertionError: rs service must be installed");
                    let mut rs = rs_rc.borrow_mut();
                    return rs.find_ident_in_package(
                        ctx,
                        syms,
                        None,
                        env,
                        site,
                        &name,
                        crate::code::kinds::KindSelector::TYP_PCK,
                    );
                }
                // Java: env.enclClass.sym = (ClassSymbol)site;
                //       return rs.findMemberType(env, site.asType(), name,
                //           (TypeSymbol)site);
                let encl_class = env.encl_class.expect("env.enclClass");
                if let TreeKind::ClassDecl { sym, .. } = &mut ctx.tree_mut(encl_class).kind {
                    *sym = Some(site);
                }
                let site_type = ctx.sym(site).ty.expect("site type");
                let rs_rc = self
                    .rs
                    .borrow()
                    .clone()
                    .expect("AssertionError: rs service must be installed");
                let mut rs = rs_rc.borrow_mut();
                rs.find_member_type_pub(ctx, syms, env, site_type, &name, site)
            }
            Tag::IDENT => {
                let name = ident_name(ctx, tree);
                let rs_rc = self
                    .rs
                    .borrow()
                    .clone()
                    .expect("AssertionError: rs service must be installed");
                let mut rs = rs_rc.borrow_mut();
                rs.find_ident(
                    ctx,
                    syms,
                    None,
                    env,
                    &name,
                    crate::code::kinds::KindSelector::TYP_PCK,
                )
            }
            _ => panic!("AssertionError: unexpected tree kind"),
        }
    }

    /// Java: Type attribIdentAsEnumType(Env<AttrContext> env, JCIdent id) (941-947)
    pub fn attrib_ident_as_enum_type(
        &mut self,
        ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
        id: TreeId,
    ) -> TypeId {
        // Java: Assert.check((env.enclClass.sym.flags() & ENUM) != 0);
        //       id.type = env.info.scope.owner.enclClass().type;
        //       id.sym = env.info.scope.owner.enclClass();
        //       return id.type; (Attr.java:941-947)
        let scope_owner = env
            .info
            .borrow()
            .scope
            .and_then(|scope| ctx.scope(scope).owner)
            .expect("AssertionError: enclosing scope owner");
        let encl_class = crate::code::symbol::symbol_encl_class(ctx, scope_owner)
            .expect("AssertionError: enclosing class");
        let encl_type = ctx.sym(encl_class).ty.expect("enclosing class type");
        ctx.tree_mut(id).ty = Some(encl_type);
        if let TreeKind::Ident { sym, .. } = &mut ctx.tree_mut(id).kind {
            *sym = Some(encl_class);
        }
        encl_type
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
            Err(failure) => {
                // Java: catch (CompletionFailure ex) { tree.type = syms.errType;
                //     return chk.completionError(tree.pos(), ex); }
                // (Attr.java:688-695)
                ctx.tree_mut(tree).ty = Some(syms.err_type);
                let chk_rc = Rc::clone(&self.chk);
                let chk = chk_rc.borrow();
                chk.completion_error(ctx, ctx.tree(tree).pos, &failure, syms)
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
/// # Arguments
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Attr.java:3335-3350 (conditionalContext).
///
    pub fn conditional_context(&self) -> Rc<dyn crate::comp::check::CheckContext> {
        Rc::new(ConditionalCheckContext {
            enclosing: Rc::clone(&self.basic_handler),
            diags: Rc::clone(&self.diags),
        })
    }

    /// Java: CheckContext diamondContext(JCNewClass clazz, TypeSymbol tsym,
    /// CheckContext checkContext) (Attr.java:3007-3016)
/// # Arguments
///   - clazz: the diamond new-class tree
///   - tsym: the site class symbol
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Attr.java:3360-3380 (diamondContext).
///
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
/// # Arguments
///   - ctx: compiler context
///   - that: the lambda tree
///   - descriptor: the functional descriptor
///   - result_info: the target result
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Attr.java:3438-3453 (lambdaBodyResult).
///
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
/// # Arguments
///   - ctx/syms: explicit services
///   - tree: the lambda tree
///   - descriptor: the functional descriptor
///   - check_context: the compatibility context
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Attr.java:3460-3490 (checkLambdaCompatible).
///
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
/// # Arguments
///   - ctx/syms: explicit services
///   - that: the functional expression
///   - result_info: the target result
///   - explicit_param_types: the explicit parameter types (if any)
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Attr.java:3390-3435 (getTargetInfo).
///
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

/// /// Java: class Attr.PostAttrAnalyzer extends TreeScanner (Attr.java:5824-5930).
///
/// # Panics
///   - none.
///
/// Upstream: Attr.java:5824-5930.
///
pub struct PostAttrAnalyzer<'a> {
    pub syms: &'a Symtab,
    pub names: &'a Names,
    /// Java: preFlow's anonymous PostAttrAnalyzer override: None for the plain
    /// postAttr analyzer; Some(stuckType) skips stuck subtrees, class decls and
    /// lambdas (§9: the override is this field).
    pub pre_flow_stuck_type: Option<TypeId>,
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
            if let Some(restype_tree) = restype_tree
                && ctx.tree(restype_tree).tag == Tag::TYPEIDENT
                && let TreeKind::PrimitiveTypeTree { typetag } = ctx.tree(restype_tree).kind
                && typetag == crate::code::type_tag::TypeTag::VOID
            {
                restype = self.syms.void_type;
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
            if let Some(stuck_type) = self.pre_flow_stuck_type {
                // Java: preFlow's scanner: skip stuck expressions, class decls
                // and lambdas.
                if ctx.tree(tree).ty == Some(stuck_type)
                    || ctx.tree(tree).tag == Tag::CLASSDEF
                    || ctx.tree(tree).tag == Tag::LAMBDA
                {
                    return;
                }
            }
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
        if constructor_type.is_none()
            && let TreeKind::NewClass {
                constructor_type: slot,
                ..
            } = &mut ctx.tree_mut(tree).kind
        {
            *slot = Some(self.syms.unknown_type);
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
            pre_flow_stuck_type: None,
        };
        crate::tree::tree_scanner::TreeScannerVisitor::scan(&mut analyzer, ctx, Some(tree));
    }
}



// ---------------------------------------------------------------------------
// Batch 5 prerequisite: Attr.checkMethod (Attr.java:4774-4895).
// §9 adapters: explicit ctx/syms/rs/infer parameters (ADR 0001);
// Sym.erasure(types) -> types.erasure(sym.type); the anonymous
// InapplicableSymbolError subclass -> the ported ResolveErrorData::Inapplicable
// symbol + resolve_error_diagnostic.
// ---------------------------------------------------------------------------
impl Attr {
    /// Java: public Type checkMethod(Type site, final Symbol sym, ResultInfo
    /// resultInfo, Env<AttrContext> env, final List<JCExpression> argtrees,
    /// List<Type> argtypes, List<Type> typeargtypes) (Attr.java:4774-4895)
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::drop_non_drop)] // the drops release borrows early (Attr/Check)
/// # Arguments
///   - ctx/syms: explicit services
///   - rs/infer: the resolve and inference services
///   - site/sym: the method and its site
///   - result_info/env: the attribution target and environment
///   - argtrees/argtypes/typeargtypes: the actual arguments
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Attr.java:4774-4895 (checkMethod).
///
    pub fn check_method(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        rs: &mut crate::comp::resolve::Resolve,
        infer: &Rc<crate::comp::infer::Infer>,
        site: TypeId,
        sym: SymbolId,
        result_info: &ResultInfo,
        env: &Rc<Env<crate::comp::attr_context::AttrContext>>,
        argtrees: Option<&List<TreeId>>,
        mut argtypes: List<TypeId>,
        typeargtypes: Option<List<TypeId>>,
    ) -> TypeId {
        use crate::code::flags;
        use crate::code::type_tag::TypeTag;
        use crate::resources::diagnostic_contract::{SymbolArg, TypeArg};
        let types = Rc::clone(&self.types);
        let chk_rc = Rc::clone(&self.chk);
        let diags = Rc::clone(&self.diags);
        let names = Rc::clone(&self.names);
        let env_tree = env.tree.expect("env tree");
        let tree_pos = ctx.tree(env_tree).pos;
        let sym_flags = ctx.sym(sym).flags_field;
        // Test (5): if symbol is an instance method of a raw type, issue an
        // unchecked warning if its argument types change under erasure.
        if sym_flags & (flags::STATIC as i64) == 0
            && (ctx.typ(site).has_tag(TypeTag::CLASS) || ctx.typ(site).has_tag(TypeTag::TYPEVAR))
        {
            let owner = ctx.sym(sym).owner.expect("method owner");
            if let Some(s) = types.as_outer_super(ctx, syms, site, owner)
                && types.is_raw_type_public(ctx, s)
            {
                    let sym_type = ctx.sym(sym).ty.expect("method type");
                    let erased = types.erasure(ctx, syms, sym_type);
                    let sym_params = types.get_parameter_types_public(ctx, sym_type);
                    let erased_params = types.get_parameter_types_public(ctx, erased);
                    if !types.is_same_types(ctx, syms, &sym_params, &erased_params) {
                        chk_rc.borrow().warn_unchecked(
                            tree_pos,
                            crate::resources::compiler_properties::warnings::unchecked_call_mbr_of_raw_type(
                                SymbolArg::placeholder(
                                    crate::comp::resolve::symbol_display_name(ctx, sym),
                                ),
                                TypeArg::placeholder(type_display_of(ctx, s)),
                            ),
                        );
                    }
            }
        }
        // Java: if (env.info.defaultSuperCallSite != null) { ... }
        let default_super_call_site = env.info.borrow().default_super_call_site;
        if let Some(default_super) = default_super_call_site {
            let encl_tree = env.encl_class.expect("enclClass");
            let encl_type = ctx.tree(encl_tree).ty.expect("enclosing class type");
            let supers = types
                .interfaces(ctx, syms, encl_type)
                .prepend(types.supertype(ctx, syms, encl_type));
            for sup in supers.iter() {
                let sup_tsym = ctx.typ(sup).tsym.expect("supertype symbol");
                let sym_encl = crate::code::symbol::symbol_encl_class(ctx, sym)
                    .expect("method enclosing class");
                if !crate::code::symbol::is_sub_class(ctx, syms, &types, sup_tsym, sym_encl) {
                    continue;
                }
                if types.is_same_type(ctx, syms, sup, default_super) {
                    continue;
                }
                let cand = types.interface_candidates(ctx, syms, sup, sym);
                if let Some(head) = cand.first().copied()
                    && head != sym
                    && crate::code::symbol::overrides(ctx, syms, &types, head, sym, sup_tsym, true)
                {
                        let diag = diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                            crate::resources::compiler_properties::fragments::overridden_default(
                                SymbolArg::placeholder(crate::comp::resolve::symbol_display_name(
                                    ctx, sym,
                                )),
                                TypeArg::placeholder(type_display_of(ctx, sup)),
                            ),
                        ));
                        let err = crate::resources::compiler_properties::errors::illegal_default_super_call(
                            crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                                type_display_of(ctx, default_super),
                            ),
                            crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                                diag.get_message(None).to_string(),
                            ),
                        );
                        crate::util::abstract_log::AbstractLog::error_at(
                            &mut *self.log.borrow_mut(),
                            tree_pos,
                            crate::util::jc_diagnostic::Error::from(err),
                        );
                        break;
                }
            }
            env.info.borrow_mut().default_super_call_site = None;
        }
        // Java: if (sym.isStatic() && site.isInterface() && env.tree.hasTag(APPLY)) ...
        let env_tag = ctx.tree(env_tree).tag;
        let site_is_interface = ctx
            .typ(site)
            .tsym
            .map(|tsym| ctx.sym(tsym).is_interface())
            .unwrap_or(false);
        if sym_flags & (flags::STATIC as i64) != 0
            && site_is_interface
            && env_tag == Tag::APPLY
        {
            let meth = match &ctx.tree(env_tree).kind {
                crate::tree::jc_tree::TreeKind::MethodInvocation { meth, .. } => *meth,
                _ => panic!("ClassCastException: JCMethodInvocation expected"),
            };
            if let Some(meth) = meth
                && ctx.tree(meth).tag == Tag::SELECT
            {
                    let selected = match &ctx.tree(meth).kind {
                        crate::tree::jc_tree::TreeKind::FieldAccess { selected, .. } => *selected,
                        _ => panic!("ClassCastException: JCFieldAccess expected"),
                    };
                    if !crate::tree::tree_info::is_static_selector(ctx, selected, &names) {
                        let err = crate::resources::compiler_properties::errors::illegal_static_intf_meth_call(
                            TypeArg::placeholder(type_display_of(ctx, site)),
                        );
                        crate::util::abstract_log::AbstractLog::error_at(
                            &mut *self.log.borrow_mut(),
                            tree_pos,
                            crate::util::jc_diagnostic::Error::from(err),
                        );
                }
            }
        }
        // Compute the identifier's instantiated type (Resolve.checkMethod).
        let mut note_warner = crate::util::warner::Warner::new(None);
        let chk_guard = chk_rc.borrow();
        let resolution = rs.check_method(
            ctx,
            syms,
            &types,
            self,
            &chk_guard,
            infer,
            env,
            site,
            sym,
            result_info,
            argtypes.clone(),
            typeargtypes.clone(),
            &mut note_warner,
        );
        drop(chk_guard);
        let owntype = match resolution {
            Ok(owntype) => owntype,
            Err(crate::comp::resolve::MethodCheckFailure::Inference(ex)) => {
                if let Some(diag) = ex.get_diagnostic() {
                    result_info.check_context.report(ctx, tree_pos, &diag);
                }
                return types.create_error_type_original(ctx, Some(site), syms);
            }
            Err(crate::comp::resolve::MethodCheckFailure::Inapplicable(ex)) => {
                let diag = ex.get_diagnostic().clone();
                let err_symbol = crate::code::symbol::symbol_new_resolve_error(
                    ctx,
                    crate::code::kinds::Kind::ERR,
                    names.empty.clone(),
                    "inapplicable method",
                    crate::code::symbol::ResolveErrorData::Inapplicable {
                        kind: crate::code::kinds::Kind::MTH,
                        sym,
                        details: Some(diag),
                    },
                );
                // Java: argtypes2 = argtypes.map(new ResolveDeferredRecoveryMap(...));
                let phase = env
                    .info
                    .borrow()
                    .pending_resolution_phase
                    .unwrap_or(crate::comp::resolve::MethodResolutionPhase::BOX);
                let chk_guard = chk_rc.borrow();
                let mut dctx = crate::comp::deferred_attr::DeferredMapCtx {
                    attr: self,
                    syms,
                    types: &types,
                    chk: &chk_guard,
                    diags: &diags,
                };
                let mut recovery_map = crate::comp::deferred_attr::ResolveDeferredRecoveryMap::new(
                    crate::comp::deferred_attr::AttrMode::CHECK,
                    Some(sym),
                    phase,
                );
                let mut argtypes2: List<TypeId> = List::nil();
                for t in argtypes.iter() {
                    argtypes2 = argtypes2.append(recovery_map.visit(ctx, &mut dctx, t, &None));
                }
                drop(dctx);
                drop(chk_guard);
                let err_diag = rs.resolve_error_diagnostic(
                    ctx,
                    syms,
                    err_symbol,
                    crate::util::jc_diagnostic::DiagnosticType::ERROR,
                    Some(Rc::new(crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(
                        tree_pos,
                    ))),
                    Some(sym),
                    Some(site),
                    Some(&ctx.sym(sym).name.clone()),
                    Some(argtypes2),
                    typeargtypes.clone(),
                );
                if let Some(err_diag) = err_diag {
                    self.log.borrow_mut().report(err_diag);
                }
                return types.create_error_type_original(ctx, Some(site), syms);
            }
        };
        // Java: DeferredTypeMap checkDeferredMap = ...; argtypes = argtypes.map(checkDeferredMap);
        let phase = env
            .info
            .borrow()
            .pending_resolution_phase
            .unwrap_or(crate::comp::resolve::MethodResolutionPhase::BOX);
        let chk_guard = chk_rc.borrow();
        let mut dctx = crate::comp::deferred_attr::DeferredMapCtx {
            attr: self,
            syms,
            types: &types,
            chk: &chk_guard,
            diags: &diags,
        };
        let mut check_deferred_map = crate::comp::deferred_attr::DeferredTypeMap::new(
            crate::comp::deferred_attr::AttrMode::CHECK,
            Some(sym),
            phase,
        );
        argtypes = check_deferred_map.map(ctx, &mut dctx, &argtypes, &None);
        drop(dctx);
        drop(chk_guard);
        let owntype = if note_warner
            .has_non_silent_lint(crate::code::lint::LintCategory::UNCHECKED)
        {
            let sym_type = ctx.sym(sym).ty.expect("method type");
            let sym_params = types.get_parameter_types_public(ctx, sym_type);
            let kind = crate::code::kinds::kind_name(ctx.sym(sym), ctx);
            let method_name = ctx.sym(sym).name.clone();
            let location = ctx.sym(sym).owner.expect("location");
            let location_kind = crate::code::kinds::kind_name(ctx.sym(location), ctx);
            chk_rc.borrow().warn_unchecked(
                tree_pos,
                crate::resources::compiler_properties::warnings::unchecked_meth_invocation_applied(
                    crate::resources::diagnostic_contract::KindNameArg::placeholder(kind.to_string()),
                    method_name,
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                        crate::comp::resolve::method_arguments_display_pub(ctx, Some(sym_params)),
                    ),
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                        crate::comp::resolve::method_arguments_display_pub(ctx, Some(argtypes.clone())),
                    ),
                    crate::resources::diagnostic_contract::KindNameArg::placeholder(
                        location_kind.to_string(),
                    ),
                    SymbolArg::placeholder(crate::comp::resolve::symbol_display_name(ctx, location)),
                ),
            );
            let any_poly = result_info.pt == Some(infer.any_poly);
            let is_partial = ctx.typ(owntype).is_partial();
            if !any_poly || ctx.typ(owntype).get_tag() != TypeTag::METHOD || !is_partial {
                let params = types.get_parameter_types_public(ctx, owntype);
                let ret = types.get_return_type_public(ctx, owntype);
                let thrown = ctx.typ(owntype).get_thrown_types(ctx);
                let erased_ret = types.erasure(ctx, syms, ret);
                let mut erased_thrown: List<TypeId> = List::nil();
                for t in thrown.iter() {
                    erased_thrown = erased_thrown.append(types.erasure(ctx, syms, t));
                }
                let ty = crate::code::type_::Type::new_method_type(
                    params,
                    erased_ret,
                    erased_thrown,
                    syms.method_class,
                );
                ctx.alloc_type(ty)
            } else {
                owntype
            }
        } else {
            owntype
        };
        // Java: PolyKind pkind = ...; TreeInfo.setPolyKind(env.tree, pkind);
        let sym_type = ctx.sym(sym).ty.expect("method type");
        let sym_ret = types.get_return_type_public(ctx, sym_type);
        let pkind = if ctx.typ(sym_type).has_tag(TypeTag::FORALL) {
            let tvars = match &ctx.typ(sym_type).kind {
                crate::code::type_::TypeKindData::ForAll { tvars, .. } => tvars.clone(),
                _ => List::nil(),
            };
            if types.contains_any(ctx, syms, sym_ret, &tvars) {
                crate::tree::jc_tree::PolyKind::POLY
            } else {
                crate::tree::jc_tree::PolyKind::STANDALONE
            }
        } else {
            crate::tree::jc_tree::PolyKind::STANDALONE
        };
        crate::tree::tree_info::set_poly_kind(ctx, env_tree, pkind);
        if result_info.pt == Some(infer.any_poly) {
            return owntype;
        }
        let last_varargs = env.info.borrow().last_resolve_varargs();
        chk_rc.borrow().check_method(
            ctx,
            syms,
            owntype,
            sym,
            env,
            argtrees,
            &argtypes,
            last_varargs,
            result_info.check_context.inference_context(),
        )
    }
}

// ---------------------------------------------------------------------------
// Batch 5: Attr.lambdaEnv (Attr.java:3484-3560) + Attr.preFlow
// (Attr.java:3296-3315). §9: the anonymous PostAttrAnalyzer subclass of
// preFlow is the PostAttrAnalyzer pre_flow flag; explicit ctx/syms parameters
// per ADR 0001.
// ---------------------------------------------------------------------------
impl Attr {
    /// Java: ResultInfo memberReferenceQualifierResult(JCMemberReference tree)
    /// (Attr.java:3755-3761)
/// # Arguments
///   - ctx: compiler context
///   - env: the attribution environment
///   - tree: the member reference
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Attr.java:3755-3761 (memberReferenceQualifierResult).
///
    pub fn member_reference_qualifier_result(
        &self,
        ctx: &CompilerContext,
        env: &Rc<Env<AttrContext>>,
        tree: TreeId,
    ) -> ResultInfo {
        let mode = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::MemberReference { mode, .. } => *mode,
            _ => panic!("ClassCastException: JCMemberReference expected"),
        };
        let _ = env;
        let pkind = if mode == crate::tree::jc_tree::ReferenceMode::INVOKE {
            KindSelector::VAL_TYP
        } else {
            KindSelector::TYP
        };
        let _ = ctx;
        ResultInfo::new(pkind, None, Rc::clone(&self.basic_handler))
    }

    /// Java: public Env<AttrContext> lambdaEnv(JCLambda that,
    /// Env<AttrContext> env) (Attr.java:3484-3560)
/// # Arguments
///   - ctx/syms: explicit services
///   - that: the lambda tree
///   - env: the enclosing attribution environment
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Attr.java:3484-3560 (lambdaEnv).
///
    pub fn lambda_env(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        that: TreeId,
        env: &Rc<Env<crate::comp::attr_context::AttrContext>>,
    ) -> Rc<Env<crate::comp::attr_context::AttrContext>> {
        use crate::code::flags;
        let owner = {
            let info = env.info.borrow();
            let scope = info.scope.expect("env scope");
            ctx.scope(scope).owner.expect("scope owner")
        };
        let owner_kind = ctx.sym(owner).kind;
        let owner_owner_kind = ctx
            .sym(owner)
            .owner
            .map(|owner| ctx.sym(owner).kind)
            .expect("symbol owner");
        let lambda_env = if owner_kind == crate::code::kinds::Kind::VAR
            && owner_owner_kind == crate::code::kinds::Kind::TYP
        {
            // field initializer
            let encl_class = crate::code::symbol::symbol_encl_class(ctx, owner)
                .expect("field enclosing class");
            let mut new_scope_owner = owner;
            if ctx.sym(owner).flags_field & (flags::STATIC as i64) == 0 {
                let members = ctx.sym(encl_class).members().expect("class members");
                let inits = ctx.scope_get_symbols_by_name(
                    members,
                    &self.names.init,
                    crate::code::scope::LookupKind::NON_RECURSIVE,
                    &|_| true,
                );
                if let Some(first) = inits.first() {
                    new_scope_owner = *first;
                }
            } else {
                let mut clinits = self.clinits.borrow_mut();
                let clinit = match clinits.get(&encl_class) {
                    Some(clinit) => *clinit,
                    None => {
                        let clinit_type = ctx.alloc_type(
                            crate::code::type_::Type::new_method_type(
                                List::nil(),
                                syms.void_type,
                                List::nil(),
                                syms.method_class,
                            ),
                        );
                        let clinit = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
                            crate::code::kinds::Kind::MTH,
                            (flags::STATIC as i64)
                                | (flags::SYNTHETIC as i64)
                                | (flags::PRIVATE as i64),
                            self.names.clinit.clone(),
                            Some(clinit_type),
                            Some(encl_class),
                            crate::code::symbol::SymbolKind::MethodSymbol {
                                extra_params: List::nil(),
                                captured_locals: List::nil(),
                                params: List::nil(),
                                default_value: None,
                            },
                        ));
                        clinits.insert(encl_class, clinit);
                        clinit
                    }
                };
                new_scope_owner = clinit;
            }
            let new_scope = ctx.scope_dup_unshared(
                env.info.borrow().scope.expect("env scope"),
                Some(new_scope_owner),
            );
            let info = env.info.borrow().dup(Some(new_scope));
            env.dup(Some(that), info)
        } else {
            // Java: env.dup(that, env.info.dup(env.info.scope.dup()))
            let scope = ctx.scope_dup(env.info.borrow().scope.expect("env scope"), None);
            let info = env.info.borrow().dup(Some(scope));
            env.dup(Some(that), info)
        };
        {
            let mut info = lambda_env.info.borrow_mut();
            info.yield_result = None;
            info.is_lambda = true;
        }
        lambda_env
    }

    /// Java: void preFlow(JCLambda tree) (Attr.java:3296-3315)
    #[allow(clippy::too_many_arguments)]
/// # Arguments
///   - ctx/syms: explicit services
///   - attr_recover/rs/infer/chk/diags: explicit services
///   - env: the attribution environment
///   - tree: the lambda tree
///
/// # Returns
///   - the upstream result (see the summary).
///
/// # Panics
///   - none beyond the registered unimplemented dependencies.
///
/// Upstream: Attr.java:3296-3315 (preFlow).
///
    pub fn pre_flow(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        attr_recover: &mut crate::comp::attr_recover::AttrRecover,
        rs: &mut crate::comp::resolve::Resolve,
        infer: &Rc<crate::comp::infer::Infer>,
        chk: &crate::comp::check::Check,
        diags: &crate::util::jc_diagnostic::Factory,
        env: &Rc<Env<crate::comp::attr_context::AttrContext>>,
        tree: TreeId,
    ) {
        attr_recover.do_recovery(ctx, syms, self, rs, infer, chk, diags);
        let body = match &ctx.tree(tree).kind {
            TreeKind::Lambda { body, .. } => *body,
            _ => panic!("ClassCastException: JCLambda expected"),
        };
        let stuck_type = syms.stuck_type;
        let mut analyzer = PostAttrAnalyzer {
            syms,
            names: &self.names,
            pre_flow_stuck_type: Some(stuck_type),
        };
        let _ = env;
        crate::tree::tree_scanner::TreeScannerVisitor::scan(&mut analyzer, ctx, body);
    }
}

impl Attr {
    #[allow(dead_code)]
    /// Java: boolean isAssignableAsBlankFinal(VarSymbol v, Env<AttrContext> env)
    /// (Attr.java:276-300).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - env: the attribution environment;
    ///   - v: the blank final variable.
    ///
    /// # Returns
    ///   - true when the blank final can be assigned in env.
    ///
    /// # Panics
    ///   - the environment scope must have an owner.
    ///
    /// Upstream: Attr.java:276-300.
    fn is_assignable_as_blank_final(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        env: &Rc<Env<AttrContext>>,
        v: SymbolId,
    ) -> bool {
        let owner = env
            .info
            .borrow()
            .scope
            .and_then(|scope| ctx.scope(scope).owner)
            .expect("AssertionError: enclosing scope owner");
        let owner_name = ctx.sym(owner).name.clone();
        let owner_kind = ctx.sym(owner).kind;
        let owner_flags = ctx.sym(owner).flags_field;
        let v_owner = ctx.sym(v).owner;
        let v_static = ctx.sym(v).flags_field & (crate::code::flags::STATIC as i64) != 0;
        let is_assignable = v_owner == Some(owner)
            || ((owner_name == self.names.init
                || owner_kind == crate::code::kinds::Kind::VAR
                || (owner_flags & (crate::code::flags::BLOCK as i64)) != 0)
                && v_owner == ctx.sym(owner).owner
                && v_static == self.resolve_is_static(ctx, env));
        let inside_compact_constructor = env.encl_method.map(|method| {
            crate::tree::tree_info::is_compact_constructor(ctx, method)
        }).unwrap_or(false);
        is_assignable && !inside_compact_constructor
    }

    #[allow(dead_code)]
    /// §9 helper: Resolve.isStatic(env) without a Resolve instance (the
    /// upstream Attr calls its own static Resolve.isStatic).
    fn resolve_is_static(
        &self,
        _ctx: &crate::compiler_context::CompilerContext,
        env: &Rc<Env<AttrContext>>,
    ) -> bool {
        let outer = env.outer.as_ref().expect("env.outer");
        env.info.borrow().static_level > outer.info.borrow().static_level
    }

    #[allow(dead_code)]
    /// Java: boolean isStaticReference(JCTree tree) (Attr.java:322-333).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - tree: the SELECT/IDENT tree.
    ///
    /// # Returns
    ///   - true when the tree is a static reference (a non-type select or an ident).
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Attr.java:322-333.
    fn is_static_reference(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        tree: TreeId,
    ) -> bool {
        if ctx.tree(tree).tag == crate::tree::jc_tree::Tag::SELECT {
            let selected = match &ctx.tree(tree).kind {
                crate::tree::jc_tree::TreeKind::FieldAccess { selected, .. } => *selected,
                _ => None,
            };
            if let Some(selected) = selected
                && let Some(lsym) = crate::tree::tree_info::symbol(ctx, selected)
                && ctx.sym(lsym).kind != crate::code::kinds::Kind::TYP
            {
                return false;
            }
        }
        true
    }

    #[allow(dead_code)]
    /// Java: void checkAssignable(DiagnosticPosition pos, VarSymbol v,
    /// JCTree base, Env<AttrContext> env) (Attr.java:308-330).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - env: the attribution environment;
    ///   - pos: the diagnostic position;
    ///   - v: the assigned variable;
    ///   - base: the select base (null when not a select).
    ///
    /// # Returns
    ///   - none (errors are reported).
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Attr.java:308-330.
    fn check_assignable(
        &mut self,
        ctx: &crate::compiler_context::CompilerContext,
        env: &Rc<Env<AttrContext>>,
        pos: i32,
        v: SymbolId,
        base: Option<TreeId>,
    ) {
        let v_flags = ctx.sym(v).flags_field;
        if ctx.sym(v).name == self.names._this {
            let info = crate::resources::compiler_properties::errors::cant_assign_val_to_this();
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
        } else if (v_flags & (crate::code::flags::FINAL as i64)) != 0
            && ((v_flags & (crate::code::flags::HASINIT as i64)) != 0
                || !(match base {
                    None => true,
                    Some(base) => crate::tree::tree_info::is_this_qualifier(ctx, base),
                } && self.is_assignable_as_blank_final(ctx, env, v)))
        {
            let info = crate::resources::compiler_properties::errors::cant_assign_val_to_final_var(
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    ctx.sym(v).name.to_java_string().to_string_lossy().to_string(),
                ),
            );
            crate::util::abstract_log::AbstractLog::error_at(
                &mut *self.log.borrow_mut(),
                pos,
                crate::util::jc_diagnostic::Error::from(info),
            );
        }
    }

}


/// §9 helper: the length of a List<Type> (used by the type-argument arity error).
fn list_len_type(list: &List<TypeId>) -> usize {
    let mut current = list.clone();
    let mut n = 0;
    while current.non_empty() {
        n += 1;
        current = current.tail();
    }
    n
}
impl Attr {
    /// Java: Env<AttrContext> enclosingInitEnv(Env<AttrContext> env)
    /// (Attr.java:4692-4718).
    ///
    /// # Arguments
    ///   - env: the current attribution environment.
    ///
    /// # Returns
    ///   - the enclosing field/initializer environment, or None.
    ///
    /// # Panics
    ///   - the enclosing environment chain is asserted non-null (upstream Assert).
    ///
    /// Upstream: Attr.java:4692-4718.
    #[allow(dead_code)]
    fn enclosing_init_env(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        env: &Rc<Env<AttrContext>>,
    ) -> Option<Rc<Env<AttrContext>>> {
        let mut current = Rc::clone(env);
        loop {
            let tag = current.tree.map(|tree| ctx.tree(tree).tag);
            match tag {
                Some(crate::tree::jc_tree::Tag::VARDEF) => {
                    // field?
                    let tree = current.tree.expect("vardef tree");
                    let sym = match &ctx.tree(tree).kind {
                        crate::tree::jc_tree::TreeKind::VariableDecl { sym, .. } => *sym,
                        _ => None,
                    };
                    let is_field = sym
                        .and_then(|sym| ctx.sym(sym).owner)
                        .map(|owner| ctx.sym(owner).kind == crate::code::kinds::Kind::TYP)
                        .unwrap_or(false);
                    if is_field {
                        return Some(Rc::clone(&current));
                    }
                }
                Some(crate::tree::jc_tree::Tag::BLOCK) => {
                    let next_is_class = current
                        .next
                        .as_ref()
                        .and_then(|next| next.tree)
                        .map(|tree| ctx.tree(tree).tag == crate::tree::jc_tree::Tag::CLASSDEF)
                        .unwrap_or(false);
                    if next_is_class {
                        return Some(Rc::clone(&current));
                    }
                }
                Some(crate::tree::jc_tree::Tag::METHODDEF)
                | Some(crate::tree::jc_tree::Tag::CLASSDEF)
                | Some(crate::tree::jc_tree::Tag::TOPLEVEL) => return None,
                _ => {}
            }
            current = match current.next.as_ref() {
                Some(next) => Rc::clone(next),
                None => panic!("AssertionError: env.next"),
            };
        }
    }

    /// Java: private boolean isStaticEnumField(VarSymbol v) (Attr.java:4764-4770).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - v: the variable.
    ///
    /// # Returns
    ///   - true when v is a static non-constant enum field.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Attr.java:4764-4770.
    #[allow(dead_code)]
    fn is_static_enum_field(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        v: SymbolId,
    ) -> bool {
        let owner = ctx.sym(v).owner;
        let data_is_some = match &ctx.sym(v).kind_data {
            crate::code::symbol::SymbolKind::VarSymbol { data, .. } => data.is_some(),
            _ => false,
        };
        owner
            .map(|owner| ctx.sym(owner).flags_field & (crate::code::flags::ENUM as i64) != 0)
            .unwrap_or(false)
            && ctx.sym(v).flags_field & (crate::code::flags::STATIC as i64) != 0
            && !(ctx.sym(v).flags_field & (crate::code::flags::FINAL as i64) != 0 && data_is_some)
            && ctx.sym(v).name != self.names._class
    }

    /// Java: private void checkEnumInitializer(JCTree tree, Env<AttrContext> env,
    /// VarSymbol v) (Attr.java:4728-4762).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - env: the attribution environment;
    ///   - tree: the reference tree;
    ///   - v: the referenced variable.
    ///
    /// # Returns
    ///   - none (errors are reported).
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Attr.java:4728-4762.
    #[allow(dead_code)]
    fn check_enum_initializer(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        env: &Rc<Env<AttrContext>>,
        tree: TreeId,
        v: SymbolId,
    ) {
        if !self.is_static_enum_field(ctx, v) {
            return;
        }
        let encl_class = env
            .info
            .borrow()
            .scope
            .and_then(|scope| ctx.scope(scope).owner)
            .and_then(|owner| crate::code::symbol::symbol_encl_class(ctx, owner));
        let Some(encl_class) = encl_class else {
            return;
        };
        if ctx.sym(encl_class).owner.is_none() {
            return;
        }
        let v_owner = ctx.sym(v).owner.expect("variable owner");
        let encl_type = ctx.sym(encl_class).ty.expect("enclosing class type");
        let v_owner_type = ctx.sym(v_owner).ty.expect("owner type");
        if v_owner != encl_class
            && !self
                .types
                .is_subtype(ctx, syms, encl_type, v_owner_type)
        {
            return;
        }
        if !self.resolve_is_initializer(ctx, env) {
            return;
        }
        let info = crate::resources::compiler_properties::errors::illegal_enum_static_ref();
        crate::util::abstract_log::AbstractLog::error_at(
            &mut *self.log.borrow_mut(),
            ctx.tree(tree).pos,
            crate::util::jc_diagnostic::Error::from(info),
        );
    }

    /// §9 helper: Resolve.isInitializer(env) without a Resolve instance.
    #[allow(dead_code)]
    fn resolve_is_initializer(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        env: &Rc<Env<AttrContext>>,
    ) -> bool {
        let scope = env.info.borrow().scope.expect("env scope");
        let owner = ctx.scope(scope).owner.expect("scope owner");
        let owner_kind = ctx.sym(owner).kind;
        let owner_flags = ctx.sym(owner).flags_field;
        let owner_owner_kind = ctx
            .sym(owner)
            .owner
            .map(|owner| ctx.sym(owner).kind)
            .expect("symbol owner");
        ctx.sym(owner).is_constructor(&self.names)
            || (owner_owner_kind == crate::code::kinds::Kind::TYP
                && (owner_kind == crate::code::kinds::Kind::VAR
                    || (owner_kind == crate::code::kinds::Kind::MTH
                        && (owner_flags & (crate::code::flags::BLOCK as i64)) != 0))
                && (owner_flags & (crate::code::flags::STATIC as i64)) == 0)
    }

    /// Java: private void checkInit(JCTree tree, Env<AttrContext> env,
    /// VarSymbol v, boolean onlyWarning) (Attr.java:4653-4690).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree/env: the reference context;
    ///   - v: the referenced variable;
    ///   - only_warning: the warning-only variant.
    ///
    /// # Returns
    ///   - none (errors/warnings are reported).
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Attr.java:4653-4690.
    #[allow(dead_code)]
    fn check_init(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
        v: SymbolId,
        only_warning: bool,
    ) {
        let init_env = self.enclosing_init_env(ctx, env);
        let v_pos = match &ctx.sym(v).kind_data {
            crate::code::symbol::SymbolKind::VarSymbol { pos, .. } => *pos,
            _ => 0,
        };
        let tree_pos = ctx.tree(tree).pos;
        if let Some(init_env) = init_env {
            let encl_var = init_env.info.borrow().encl_var;
            let v_owner = ctx.sym(v).owner;
            let owner_is_class = v_owner
                .map(|owner| ctx.sym(owner).kind == crate::code::kinds::Kind::TYP)
                .unwrap_or(false);
            let scope_owner = env
                .info
                .borrow()
                .scope
                .and_then(|scope| ctx.scope(scope).owner);
            let scope_owner_encl = scope_owner
                .and_then(|owner| crate::code::symbol::symbol_encl_class(ctx, owner));
            let same_staticness = (ctx.sym(v).flags_field & (crate::code::flags::STATIC as i64) != 0)
                == self.resolve_is_static(ctx, env);
            let assign_lhs_ok = if let Some(env_tree) = env.tree {
                if ctx.tree(env_tree).tag == crate::tree::jc_tree::Tag::ASSIGN {
                    let lhs = match &ctx.tree(env_tree).kind {
                        crate::tree::jc_tree::TreeKind::Assign { lhs, .. } => *lhs,
                        _ => None,
                    };
                    lhs.map(|lhs| crate::tree::tree_info::skip_parens(ctx, lhs) != tree)
                        .unwrap_or(true)
                } else {
                    true
                }
            } else {
                true
            };
            if (encl_var == Some(v) || v_pos > tree_pos)
                && owner_is_class
                && v_owner == scope_owner_encl
                && same_staticness
                && assign_lhs_ok
            {
                let is_self_ref = encl_var == Some(v);
                if !only_warning || self.is_static_enum_field(ctx, v) {
                    let info = if is_self_ref {
                        crate::resources::compiler_properties::errors::illegal_self_ref()
                    } else {
                        crate::resources::compiler_properties::errors::illegal_forward_ref()
                    };
                    crate::util::abstract_log::AbstractLog::error_at(
                        &mut *self.log.borrow_mut(),
                        tree_pos,
                        crate::util::jc_diagnostic::Error::from(info),
                    );
                } else if self.use_before_declaration_warning {
                    // Java: log.warning(tree.pos(), Warnings.SelfRef(v)/ForwardRef(v));
                    let warning = if is_self_ref {
                        crate::resources::compiler_properties::warnings::self_ref(
                            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                ctx.sym(v).name.to_java_string().to_string_lossy().to_string(),
                            ),
                        )
                    } else {
                        crate::resources::compiler_properties::warnings::forward_ref(
                            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                ctx.sym(v).name.to_java_string().to_string_lossy().to_string(),
                            ),
                        )
                    };
                    self.chk.borrow().warn_unchecked(tree_pos, warning);
                }
            }
        }
        // NOT PORTED: v.getConstValue() (Symbol.java:1759, ConstValue batch);
        // the constant-value evaluation side effect is registered.
        self.check_enum_initializer(ctx, syms, env, tree, v);
    }
}


impl Attr {
    /// Java: KindSelector pkind() — the current result-info kind selector.
    ///
    /// # Arguments
    ///   - (none).
    ///
    /// # Returns
    ///   - the current ResultInfo.pkind.
    ///
    /// # Panics
    ///   - resultInfo must be installed (upstream field access).
    ///
    /// Upstream: Attr.java:4130-4135 (ResultInfo.pkind accessor use).
    fn pkind(&self) -> crate::code::kinds::KindSelector {
        self.result_info
            .as_ref()
            .expect("AssertionError: resultInfo")
            .pkind
    }

    /// Java: Symbol thisSym(DiagnosticPosition pos, Env<AttrContext> env)
    /// (Attr.java:349-352).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - pos: the diagnostic position;
    ///   - env: the attribution environment.
    ///
    /// # Returns
    ///   - the current this symbol.
    ///
    /// # Panics
    ///   - the enclosing class must be present (upstream cast).
    ///
    /// Upstream: Attr.java:349-352.
    fn this_sym(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        pos: i32,
        env: &Rc<Env<AttrContext>>,
    ) -> SymbolId {
        let encl_class = env.encl_class.expect("AssertionError: enclClass");
        let encl_sym = class_tree_symbol(ctx, encl_class).expect("AssertionError: enclClass sym");
        let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
        let mut rs = rs_rc.borrow_mut();
        let types_rc = Rc::clone(&self.types);
        rs.resolve_self(
            ctx,
            syms,
            &types_rc,
            simple_diag_pos(pos),
            env,
            encl_sym,
            self.names._this.clone(),
        )
    }
}

/// §9 helper: a simple DiagnosticPosition for a source offset.
fn simple_diag_pos(
    pos: i32,
) -> Option<std::rc::Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>> {
    Some(std::rc::Rc::new(
        crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
    ))
}

impl Attr {
    /// Java: KindSelector attribArgs(KindSelector initialKind, List<JCExpression> trees,
    /// Env<AttrContext> env, ListBuffer<Type> argtypes) (Attr.java:772-787).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - initial_kind: the initial method kind selector;
    ///   - trees: the argument trees;
    ///   - env: the attribution environment.
    ///
    /// # Returns
    ///   - (kind, argtypes) as the upstream ListBuffer contract.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Attr.java:772-787.
    fn attrib_args(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        initial_kind: KindSelector,
        trees: List<TreeId>,
        env: &Rc<Env<AttrContext>>,
    ) -> (KindSelector, List<TypeId>) {
        let mut kind = initial_kind;
        let mut argtypes: List<TypeId> = List::nil();
        let mut current = trees;
        while current.non_empty() {
            let arg = *current.head();
            let info = if self.allow_poly {
                self.method_attr_info.clone()
            } else {
                self.unknown_expr_info.clone()
            };
            let argtype = self.attrib_tree_use(ctx, syms, arg, env, info);
            let chk_rc = Rc::clone(&self.chk);
            let chk = chk_rc.borrow();
            let argtype = chk.check_non_void(ctx, ctx.tree(arg).pos, argtype);
            drop(chk);
            if ctx.typ(argtype).has_tag(TypeTag::DEFERRED) {
                kind = KindSelector::of(&[KindSelector::POLY, kind]);
            }
            argtypes = argtypes.append(argtype);
            current = current.tail();
        }
        (kind, argtypes)
    }

    /// Java: Type newMethodTemplate(Type restype, List<Type> argtypes,
    /// List<Type> typeargtypes) (Attr.java:2621-2625).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - restype: the method return type;
    ///   - argtypes: the parameter types;
    ///   - typeargtypes: the type arguments (None for a plain MethodType).
    ///
    /// # Returns
    ///   - the method type (ForAll when type arguments are present).
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Attr.java:2621-2625.
    fn new_method_template(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        restype: TypeId,
        argtypes: List<TypeId>,
        typeargtypes: Option<List<TypeId>>,
    ) -> TypeId {
        let mt = crate::code::type_::Type::new_method_type(
            argtypes,
            restype,
            List::nil(),
            syms.method_class,
        );
        match typeargtypes {
            None => ctx.alloc_type(mt),
            Some(typeargtypes) => {
                let qtype = ctx.alloc_type(mt);
                ctx.alloc_type(crate::code::type_::Type::new_for_all(typeargtypes, qtype))
            }
        }
    }

    /// Java: Type adjustMethodReturnType(Symbol msym, Type qualifierType,
    /// Name methodName, List<Type> argtypes, Type restype) (Attr.java:2627-2649).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - msym: the resolved method symbol;
    ///   - qualifier_type: the qualifier static type;
    ///   - method_name: the method name;
    ///   - argtypes: the actual argument types;
    ///   - restype: the declared return type.
    ///
    /// # Returns
    ///   - the adjusted return type (getClass/array clone special cases).
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Attr.java:2627-2649.
    #[allow(clippy::too_many_arguments)]
    fn adjust_method_return_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        msym: SymbolId,
        qualifier_type: TypeId,
        method_name: &crate::util::name::Name,
        argtypes: &List<TypeId>,
        restype: TypeId,
    ) -> TypeId {
        let object_tsym = ctx.typ(syms.object_type).tsym.expect("object symbol");
        if ctx.sym(msym).owner == Some(object_tsym)
            && method_name == &self.names.getClass
            && argtypes.is_empty()
        {
            // x.getClass() has type Class<? extends |X|>
            let encl = ctx.typ(restype).get_enclosing_type();
            let tsym = ctx.typ(restype).tsym.expect("Class symbol");
            let erased = self.types.erasure(ctx, syms, qualifier_type);
            let wildcard = ctx.alloc_type(Type::new_wildcard_type(
                erased,
                crate::code::bound_kind::BoundKind::EXTENDS,
                syms.bound_class,
                None,
            ));
            return ctx.alloc_type(Type::new(
                TypeTag::CLASS,
                Some(tsym),
                TypeKindData::ClassType {
                    outer_field: encl,
                    typarams_field: Some(List::of(wildcard)),
                    allparams_field: None,
                    supertype_field: None,
                    interfaces_field: None,
                    all_interfaces_field: None,
                },
            ));
        }
        let array_tsym = Some(syms.array_class);
        if ctx.sym(msym).owner == array_tsym
            && method_name == &self.names.clone
            && ctx.typ(qualifier_type).has_tag(TypeTag::ARRAY)
        {
            // array.clone() has a result that is the same as the static type
            // of the array being cloned
            return qualifier_type;
        }
        restype
    }

    /// Java: boolean checkFirstConstructorStat(JCMethodInvocation tree,
    /// JCMethodDecl enclMethod, boolean error) (Attr.java:2653-2671).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - tree: the application node;
    ///   - encl_method: the enclosing method (None for an initializer);
    ///   - error: whether an error is issued.
    ///
    /// # Returns
    ///   - true when the call is the first statement of the constructor.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Attr.java:2653-2671.
    #[allow(clippy::collapsible_if)]
    fn check_first_constructor_stat(
        &mut self,
        ctx: &CompilerContext,
        tree: TreeId,
        encl_method: Option<TreeId>,
        error: bool,
    ) -> bool {
        if let Some(encl_method) = encl_method {
            let (name, body) = match &ctx.tree(encl_method).kind {
                TreeKind::MethodDecl { name, body, .. } => (name.clone(), *body),
                _ => (self.names.empty.clone(), None),
            };
            if name == self.names.init {
                if let Some(body) = body {
                    if let TreeKind::Block { stats, .. } = &ctx.tree(body).kind {
                        if stats.non_empty() {
                            let head = *stats.head();
                            if ctx.tree(head).tag == Tag::EXEC {
                                let expr = match &ctx.tree(head).kind {
                                    TreeKind::ExpressionStatement { expr } => *expr,
                                    _ => None,
                                };
                                if expr == Some(tree) {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }
        if error {
            let name = match &ctx.tree(tree).kind {
                TreeKind::MethodInvocation { meth, .. } => meth
                    .and_then(|meth| crate::tree::tree_info::name(ctx, meth))
                    .unwrap_or_else(|| self.names.empty.clone()),
                _ => self.names.empty.clone(),
            };
            self.attr_error(
                ctx.tree(tree).pos,
                crate::resources::compiler_properties::errors::call_must_be_first_stmt_in_ctor(name),
            );
        }
        false
    }
}

impl Attr {
    /// Java: public void visitApply(JCMethodInvocation tree) (Attr.java:2465-2601).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the method invocation.
    ///
    /// # Returns
    ///   - the attributed invocation type.
    ///
    /// # Panics
    ///   - the resolved method type is asserted non-wildcard (upstream AssertionError).
    ///
    /// Upstream: Attr.java:2465-2601.
    #[allow(unused_assignments, clippy::collapsible_if)]
    fn visit_apply(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        // Java: Env<AttrContext> localEnv = env.dup(tree, env.info.dup());
        let env = self.env.clone().expect("env");
        let info_copy = {
            let info = env.info.borrow();
            info.dup(info.scope)
        };
        let local_env = env.dup(Some(tree), info_copy);
        let (meth, args, typeargs) = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation {
                meth, args, typeargs, ..
            } => (*meth, args.clone(), typeargs.clone()),
            _ => panic!("ClassCastException: JCMethodInvocation expected"),
        };
        let meth = meth.expect("method invocation meth");
        let meth_name = crate::tree::tree_info::name(ctx, meth).unwrap_or_else(|| self.names.empty.clone());
        let is_constructor_call =
            meth_name == self.names._this || meth_name == self.names._super;
        let mut argtypes: List<TypeId> = List::nil();
        let mut typeargtypes: Option<List<TypeId>> = None;
        if is_constructor_call {
            // We are seeing a ...this(...) or ...super(...) call.
            // Check that this is the first statement in a constructor.
            self.check_first_constructor_stat(ctx, tree, env.encl_method, true);
            // Record the fact that this is a constructor call (using isSelfCall).
            local_env.info.borrow_mut().is_self_call = true;
            // Attribute arguments, yielding list of argument types.
            let (kind, attributed) = self.attrib_args(
                ctx,
                syms,
                KindSelector::MTH,
                args.clone(),
                &local_env,
            );
            argtypes = attributed;
            typeargtypes = Some(self.attrib_types(
                ctx,
                syms,
                typeargs.clone(),
                &local_env,
            ));
            // Variable `site` points to the class in which the called constructor is defined.
            let encl_class = env.encl_class.expect("env.enclClass");
            let encl_sym = class_tree_symbol(ctx, encl_class).expect("enclClass sym");
            let mut site = ctx.sym(encl_sym).ty.expect("enclClass type");
            if meth_name == self.names._super {
                if site == syms.object_type {
                    self.attr_error(
                        ctx.tree(meth).pos,
                        crate::resources::compiler_properties::errors::no_superclass(
                            Self::type_arg_placeholder(ctx, site),
                        ),
                    );
                    site = self.types.create_error_type_original(ctx, Some(syms.object_type), syms);
                } else {
                    site = self.types.supertype(ctx, syms, site);
                }
            }
            if ctx.typ(site).has_tag(TypeTag::CLASS) {
                let mut encl = ctx.typ(site).get_enclosing_type();
                while let Some(encl_type) = encl {
                    if !ctx.typ(encl_type).has_tag(TypeTag::TYPEVAR) {
                        break;
                    }
                    encl = ctx.typ(encl_type).get_upper_bound();
                }
                if encl
                    .map(|encl| ctx.typ(encl).has_tag(TypeTag::CLASS))
                    .unwrap_or(false)
                {
                    // we are calling a nested class
                    if ctx.tree(meth).tag == Tag::SELECT {
                        let qualifier = match &ctx.tree(meth).kind {
                            TreeKind::FieldAccess { selected, .. } => {
                                selected.expect("select qualifier")
                            }
                            _ => panic!("ClassCastException: JCFieldAccess expected"),
                        };
                        // We are seeing a prefixed call, of the form <expr>.super(...).
                        // Check that the prefix expression conforms to the outer
                        // instance type of the class.
                        let encl = encl.expect("enclosing type");
                        let qual_type = self.attrib_expr_with_type(
                            ctx,
                            syms,
                            qualifier,
                            &local_env,
                            Some(encl),
                        );
                        let chk_rc = Rc::clone(&self.chk);
                        let chk = chk_rc.borrow();
                        chk.check_ref_type(ctx, ctx.tree(qualifier).pos, qual_type);
                    } else if meth_name == self.names._super {
                        // qualifier omitted; check for existence of an appropriate
                        // implicit qualifier.
                        let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
                        let mut rs = rs_rc.borrow_mut();
                        let types_rc = Rc::clone(&self.types);
                        rs.resolve_implicit_this(
                            ctx,
                            syms,
                            &types_rc,
                            simple_diag_pos(ctx.tree(meth).pos),
                            &local_env,
                            site,
                            true,
                        );
                    }
                } else if ctx.tree(meth).tag == Tag::SELECT {
                    let site_tsym = ctx.typ(site).tsym.expect("site type symbol");
                    self.attr_error(
                        ctx.tree(meth).pos,
                        crate::resources::compiler_properties::errors::illegal_qual_not_icls(
                            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                ctx.sym(site_tsym)
                                    .name
                                    .to_java_string()
                                    .to_string_lossy()
                                    .to_string(),
                            ),
                        ),
                    );
                    let selected = match &ctx.tree(meth).kind {
                        TreeKind::FieldAccess { selected, .. } => selected.expect("select qualifier"),
                        _ => panic!("ClassCastException: JCFieldAccess expected"),
                    };
                    self.attrib_expr_with_type(ctx, syms, selected, &local_env, Some(site));
                }
                // if we are calling a java.lang.Enum constructor, prefix the
                // implicit String and int parameters
                let enum_tsym = Some(syms.enum_sym);
                if ctx.typ(site).tsym == enum_tsym {
                    argtypes = argtypes.prepend(syms.int_type).prepend(syms.string_type);
                }
                // Resolve the called constructor under the assumption that we are
                // referring to a superclass instance of the current instance.
                let select_super_prev = local_env.info.borrow().select_super;
                local_env.info.borrow_mut().select_super = true;
                local_env.info.borrow_mut().pending_resolution_phase = None;
                let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
                let mut rs = rs_rc.borrow_mut();
                let chk_rc = Rc::clone(&self.chk);
                let chk = chk_rc.borrow();
                let infer_rc = Rc::clone(&self.infer);
                let types_rc = Rc::clone(&self.types);
                let sym = rs.resolve_constructor(
                    ctx,
                    syms,
                    &types_rc,
                    self,
                    &chk,
                    &infer_rc,
                    simple_diag_pos(ctx.tree(meth).pos),
                    &local_env,
                    site,
                    argtypes.clone(),
                    typeargtypes.clone(),
                );
                drop(chk);
                drop(rs);
                local_env.info.borrow_mut().select_super = select_super_prev;
                // Set method symbol to resolved constructor...
                crate::tree::tree_info::set_symbol(ctx, meth, sym);
                // ...and check that it is legal in the current context.
                // (this will also set the tree's type)
                let pt = self.pt().unwrap_or_else(|| self.no_type(ctx));
                let mpt = self.new_method_template(ctx, syms, pt, argtypes.clone(), typeargtypes.clone());
                let result_info = self.result_info.clone().expect("resultInfo");
                let new_info = ResultInfo::new_with_mode(
                    kind,
                    Some(mpt),
                    Rc::clone(&result_info.check_context),
                    result_info.check_mode,
                );
                self.check_id(ctx, syms, meth, site, sym, &local_env, &new_info);
            } else if ctx.typ(site).has_tag(TypeTag::ERROR)
                && ctx.tree(meth).tag == Tag::SELECT
            {
                let selected = match &ctx.tree(meth).kind {
                    TreeKind::FieldAccess { selected, .. } => selected.expect("select qualifier"),
                    _ => panic!("ClassCastException: JCFieldAccess expected"),
                };
                self.attrib_expr_with_type(ctx, syms, selected, &local_env, Some(site));
            }
            // Otherwise, `site' is an error type and we do nothing
            ctx.tree_mut(tree).ty = Some(syms.void_type);
            self.result = Some(syms.void_type);
        } else {
            // Otherwise, we are seeing a regular method call.
            // Attribute the arguments, yielding list of argument types, ...
            let (kind, attributed) = self.attrib_args(
                ctx,
                syms,
                KindSelector::VAL,
                args.clone(),
                &local_env,
            );
            argtypes = attributed;
            typeargtypes = Some(self.attrib_any_types(
                ctx,
                syms,
                typeargs.clone(),
                &local_env,
            ));
            // ... and attribute the method using as a prototype a methodtype whose
            // formal argument types are exactly the list of actual arguments
            // (this will also set the method symbol).
            let pt = self.pt().unwrap_or_else(|| self.no_type(ctx));
            let mpt = self.new_method_template(ctx, syms, pt, argtypes.clone(), typeargtypes.clone());
            local_env.info.borrow_mut().pending_resolution_phase = None;
            let result_info = self.result_info.clone().expect("resultInfo");
            let new_info = ResultInfo::new_with_mode(
                kind,
                Some(mpt),
                Rc::clone(&result_info.check_context),
                result_info.check_mode,
            );
            let mtype = self.attrib_tree_use(ctx, syms, meth, &local_env, new_info);
            // Compute the result type.
            let mut restype = self.types.get_return_type_public(ctx, mtype);
            if ctx.typ(restype).has_tag(TypeTag::WILDCARD) {
                panic!("AssertionError: {:?}", mtype);
            }
            let qualifier = if ctx.tree(meth).tag == Tag::SELECT {
                let selected = match &ctx.tree(meth).kind {
                    TreeKind::FieldAccess { selected, .. } => selected.expect("select qualifier"),
                    _ => panic!("ClassCastException: JCFieldAccess expected"),
                };
                ctx.tree(selected).ty.unwrap_or_else(|| self.no_type(ctx))
            } else {
                let encl_class = env.encl_class.expect("env.enclClass");
                let encl_sym = class_tree_symbol(ctx, encl_class).expect("enclClass sym");
                ctx.sym(encl_sym).ty.expect("enclClass type")
            };
            let msym = crate::tree::tree_info::symbol(ctx, meth);
            restype = self.adjust_method_return_type(
                ctx,
                syms,
                msym.unwrap_or(syms.err_symbol),
                qualifier,
                &meth_name,
                &argtypes,
                restype,
            );
            let chk_rc = Rc::clone(&self.chk);
            let chk = chk_rc.borrow();
            chk.check_ref_types(
                ctx,
                syms,
                typeargs.clone(),
                typeargtypes.clone().unwrap_or_else(List::nil),
            );
            drop(chk);
            // Check that value of resulting type is admissible in the current
            // context. Also, capture the return type.
            let captured = result_info
                .check_context
                .inference_context()
                .cached_capture(ctx, syms, &self.types, tree, restype, true);
            let result_info2 = self.result_info.clone().expect("resultInfo");
            self.result = Some(self.check(ctx, syms, tree, captured, KindSelector::VAL, &result_info2));
        }
        // Java: chk.validate(tree.typeargs, localEnv);
        // NOT PORTED: Check.validate/Validator (Check.java:1397-1525, Check batch);
        // registered.
        Ok(self.result.expect("attribution result"))
    }
}

impl Attr {
    /// Java: public void visitSelect(JCFieldAccess tree) (Attr.java:4262-4400).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the field access.
    ///
    /// # Returns
    ///   - the attributed select type.
    ///
    /// # Panics
    ///   - none (the upstream AssertionError cases are registered).
    ///
    /// Upstream: Attr.java:4262-4400.
    fn visit_select(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        use crate::code::flags;
        let (selected, name) = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { selected, name, .. } => {
                (selected.expect("select qualifier"), name.clone())
            }
            _ => panic!("ClassCastException: JCFieldAccess expected"),
        };
        // Determine the expected kind of the qualifier expression.
        let mut skind = KindSelector::NIL;
        if name == self.names._this || name == self.names._super || name == self.names._class {
            skind = KindSelector::TYP;
        } else {
            let pkind = self.pkind();
            if pkind.contains(KindSelector::PCK) {
                skind = KindSelector::of(&[skind, KindSelector::PCK]);
            }
            if pkind.contains(KindSelector::TYP) {
                skind = KindSelector::of(&[skind, KindSelector::TYP, KindSelector::PCK]);
            }
            if pkind.contains(KindSelector::VAL_MTH) {
                skind = KindSelector::of(&[skind, KindSelector::VAL, KindSelector::TYP]);
            }
        }
        // Attribute the qualifier expression, and determine its symbol (if any).
        let env = self.env.clone().expect("env");
        let no_type = self.no_type(ctx);
        let qualifier_info = ResultInfo::new(skind, Some(no_type), Rc::clone(&self.basic_handler));
        let mut site = self.attrib_tree_use(ctx, syms, selected, &env, qualifier_info);
        if !self.pkind().contains(KindSelector::TYP_PCK) {
            site = self.types.capture(ctx, syms, site);
        }
        // don't allow T.class T[].class, etc
        if skind == KindSelector::TYP {
            let mut elt = site;
            while ctx.typ(elt).has_tag(TypeTag::ARRAY) {
                elt = self.types.elemtype(ctx, syms, elt);
            }
            if ctx.typ(elt).has_tag(TypeTag::TYPEVAR) {
                self.attr_error(
                    ctx.tree(tree).pos,
                    crate::resources::compiler_properties::errors::type_var_cant_be_deref(),
                );
                let site_name = ctx.typ(site).tsym.map(|tsym| ctx.sym(tsym).name.clone()).unwrap_or_else(|| self.names.empty.clone());
                let error = self.types.create_error_type(ctx, site_name, ctx.typ(site).tsym.expect("site symbol"), Some(site));
                ctx.tree_mut(tree).ty = Some(error);
                let error_tsym = ctx.typ(error).tsym;
                if let TreeKind::FieldAccess { sym: slot, .. } = &mut ctx.tree_mut(tree).kind {
                    *slot = error_tsym;
                }
                self.result = Some(error);
                return Ok(error);
            }
        }
        // If qualifier symbol is a type or `super', assert `selectSuper' for the
        // selection. This is relevant for determining whether protected symbols
        // are accessible.
        let sitesym = crate::tree::tree_info::symbol(ctx, selected);
        let select_super_prev = env.info.borrow().select_super;
        let is_super_qualifier = sitesym
            .map(|sitesym| ctx.sym(sitesym).name == self.names._super)
            .unwrap_or(false);
        env.info.borrow_mut().select_super = is_super_qualifier;
        // Determine the symbol represented by the selection.
        env.info.borrow_mut().pending_resolution_phase = None;
        let result_info = self.result_info.clone().expect("resultInfo");
        let mut sym = self.select_sym(ctx, syms, tree, sitesym, site, &env, &result_info);
        if ctx.sym(sym).kind == crate::code::kinds::Kind::VAR
            && ctx.sym(sym).name != self.names._super
            && env.info.borrow().default_super_call_site.is_some()
        {
            let site_tsym = ctx.typ(site).tsym.expect("site type symbol");
            self.attr_error(
                ctx.tree(selected).pos,
                crate::resources::compiler_properties::errors::not_encl_class(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        ctx.sym(site_tsym).name.to_java_string().to_string_lossy().to_string(),
                    ),
                ),
            );
            sym = syms.err_symbol;
        }
        let sym_exists = crate::code::symbol::symbol_exists(ctx, sym);
        if sym_exists
            && ctx.sym(sym).kind != crate::code::kinds::Kind::TYP
            && self.pkind().contains(KindSelector::TYP_PCK)
        {
            site = self.types.capture(ctx, syms, site);
            sym = self.select_sym(ctx, syms, tree, sitesym, site, &env, &result_info);
        }
        let var_args = env.info.borrow().last_resolve_varargs();
        let _ = var_args;
        // Java: tree.sym = sym;
        if let TreeKind::FieldAccess { sym: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(sym);
        }
        if ctx.typ(site).has_tag(TypeTag::TYPEVAR)
            && ctx.sym(sym).kind != crate::code::kinds::Kind::TYP
            && ctx.sym(sym).kind != crate::code::kinds::Kind::ERR
        {
            // site = types.skipTypeVars(site, true)
            let mut current = site;
            while ctx.typ(current).has_tag(TypeTag::TYPEVAR) {
                current = ctx.typ(current).get_upper_bound().expect("typevar upper bound");
            }
            site = self.types.capture(ctx, syms, current);
        }
        // If that symbol is a variable, ...
        if ctx.sym(sym).kind == crate::code::kinds::Kind::VAR {
            // ... evaluate its initializer, if it has one, and check for
            // illegal forward reference.
            self.check_init(ctx, syms, tree, &env, sym, true);
            // If we are expecting a variable (as opposed to a value), check
            // that the variable is assignable in the current environment.
            if crate::code::kinds::KindSelector::ASG.subset(self.pkind()) {
                self.check_assignable(ctx, &env, ctx.tree(tree).pos, sym, Some(selected));
            }
        }
        // Java: try-with-resources explicit close lint (SERIAL batch) — the
        // upstream branch needs LintCategory.TRY + autoCloseableClose;
        // ported below with the ported Lint service.
        if let Some(sitesym) = sitesym {
            let is_resource = match &ctx.sym(sitesym).kind_data {
                crate::code::symbol::SymbolKind::VarSymbol { data_kind, .. } => {
                    *data_kind == Some(crate::code::symbol::ElementKind::RESOURCE_VARIABLE)
                }
                _ => false,
            };
            if is_resource
                && ctx.sym(sym).kind == crate::code::kinds::Kind::MTH
                && ctx.sym(sym).name == self.names.close
            {
                let sitesym_type = ctx.sym(sitesym).ty.expect("resource type");
                let sitesym_tsym = ctx.typ(sitesym_type).tsym.expect("resource type symbol");
                if crate::code::symbol::overrides(
                    ctx,
                    syms,
                    &self.types,
                    sym,
                    syms.auto_closeable_close,
                    sitesym_tsym,
                    true,
                ) {
                    let try_enabled = env
                        .info
                        .borrow()
                        .lint
                        .as_ref()
                        .map(|lint| lint.is_enabled(crate::code::lint::LintCategory::TRY))
                        .unwrap_or(false);
                    if try_enabled {
                        let warning = crate::resources::compiler_properties::warnings::try_explicit_close_call();
                        let warning = crate::util::jc_diagnostic::Warning::from(warning);
                        crate::util::mandatory_warning_handler::MandatoryWarningLog::warning(
                            &mut *self.log.borrow_mut(),
                            Some(crate::code::lint::LintCategory::TRY),
                            Some(std::rc::Rc::new(
                                crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(ctx.tree(tree).pos),
                            )),
                            warning,
                        );
                    }
                }
            }
        }
        // Disallow selecting a type from an expression
        let sitesym_is_type = sitesym
            .map(|sitesym| ctx.sym(sitesym).kind == crate::code::kinds::Kind::TYP)
            .unwrap_or(false);
        let sym_is_type = ctx.sym(sym).kind == crate::code::kinds::Kind::TYP;
        if sym_is_type
            && !sitesym
                .map(|sitesym| {
                    ctx.sym(sitesym)
                        .kind
                        .matches(KindSelector::TYP_PCK)
                })
                .unwrap_or(false)
        {
            let pt = self.pt().unwrap_or_else(|| self.no_type(ctx));
            let check_kind = match sitesym {
                None => KindSelector::VAL,
                Some(sitesym) => ctx.sym(sitesym).kind.to_selector(),
            };
            let new_info = ResultInfo::new(
                KindSelector::TYP_PCK,
                Some(pt),
                Rc::clone(&self.basic_handler),
            );
            let site_type = self.check(ctx, syms, selected, pt, check_kind, &new_info);
            ctx.tree_mut(tree).ty = Some(site_type);
        }
        if sitesym_is_type {
            if ctx.sym(sym).name == self.names._this {
                // If `C' is the currently compiled class, check that C.this does
                // not appear in a call to a super(...)
                let encl_class = env.encl_class.expect("enclClass");
                let encl_sym = class_tree_symbol(ctx, encl_class).expect("enclClass sym");
                if env.info.borrow().is_self_call && ctx.typ(site).tsym == Some(encl_sym) {
                    let chk_rc = Rc::clone(&self.chk);
                    let chk = chk_rc.borrow();
                    chk.early_ref_error(ctx, &self.log, ctx.tree(tree).pos, sym);
                }
            } else {
                // Check if type-qualified fields or methods are static (JLS)
                let sym_flags = ctx.sym(sym).flags_field;
                if (sym_flags & (flags::STATIC as i64)) == 0
                    && ctx.sym(sym).name != self.names._super
                    && (ctx.sym(sym).kind == crate::code::kinds::Kind::VAR
                        || ctx.sym(sym).kind == crate::code::kinds::Kind::MTH)
                {
                    let static_error = {
                        let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
                        let mut rs = rs_rc.borrow_mut();
                        rs.new_static_error_pub(ctx, sym)
                    };
                    let pos = simple_diag_pos(ctx.tree(tree).pos);
                    let site_opt = Some(site);
                    let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
                    let mut rs = rs_rc.borrow_mut();
                    let sym_name = ctx.sym(sym).name.clone();
                    rs.access_base(
                        ctx,
                        syms,
                        static_error,
                        pos,
                        None,
                        site_opt,
                        &sym_name,
                        true,
                    );
                }
            }
            // Java: if (!allowStaticInterfaceMethods && sitesym.isInterface() &&
            //     sym.isStatic() && sym.kind == MTH) log.error(DiagnosticFlag.SOURCE_LEVEL,
            //     tree.pos(), Feature.STATIC_INTERFACE_METHODS_INVOKE.error(sourceName));
            // NOT PORTED: the source-level feature error logging (Features batch);
            // registered.
        } else if ctx.sym(sym).kind != crate::code::kinds::Kind::ERR
            && (ctx.sym(sym).flags_field & (flags::STATIC as i64)) != 0
            && ctx.sym(sym).name != self.names._class
        {
            // If the qualified item is not a type and the selected item is
            // static, report a warning.
            let kind_name = crate::code::kinds::kind_name(ctx.sym(sym), ctx);
            let owner = ctx.sym(sym).owner.expect("symbol owner");
            let warning = crate::resources::compiler_properties::warnings::static_not_qualified_by_type(
                crate::resources::diagnostic_contract::KindNameArg::placeholder(kind_name.to_string()),
                crate::resources::diagnostic_contract::SymbolArg::placeholder(
                    ctx.sym(owner).name.to_java_string().to_string_lossy().to_string(),
                ),
            );
            let chk_rc = Rc::clone(&self.chk);
            let chk = chk_rc.borrow();
            chk.warn_static(ctx, &self.log, ctx.tree(tree).pos, warning);
        }
        // If we are selecting an instance member via a `super', ...
        if env.info.borrow().select_super && (ctx.sym(sym).flags_field & (flags::STATIC as i64)) == 0 {
            // Check that super-qualified symbols are not abstract (JLS)
            let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
            let mut rs = rs_rc.borrow_mut();
            rs.check_non_abstract(ctx, simple_diag_pos(ctx.tree(tree).pos), sym);
            drop(rs);
            if self.types.is_raw_type_public(ctx, site) {
                // Determine argument types for site.
                let encl_class = env.encl_class.expect("enclClass");
                let encl_sym = class_tree_symbol(ctx, encl_class).expect("enclClass sym");
                let encl_type = ctx.sym(encl_sym).ty.expect("encl class type");
                let site_tsym = ctx.typ(site).tsym.expect("site type symbol");
                if let Some(site1) = self.types.as_super(ctx, syms, encl_type, site_tsym) {
                    site = site1;
                }
            }
        }
        if env.info.borrow().is_serializable {
            // NOT PORTED: chk.checkAccessFromSerializableElement(tree,
            // env.info.isSerializableLambda) (Check.java:2745-2800, lint/serial
            // batch); registered.
        }
        env.info.borrow_mut().select_super = select_super_prev;
        let result = self.check_id(ctx, syms, tree, site, sym, &env, &result_info);
        self.result = Some(result);
        Ok(result)
    }

    /// Java: private Symbol selectSym(JCFieldAccess tree, Symbol location,
    /// Type site, Env<AttrContext> env, ResultInfo resultInfo)
    /// (Attr.java:4420-4478).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the select tree;
    ///   - location: the qualifier symbol;
    ///   - site: the qualifier type;
    ///   - env/result_info: the attribution context.
    ///
    /// # Returns
    ///   - the selected symbol.
    ///
    /// # Panics
    ///   - wildcard qualifiers assert as in the upstream code.
    ///
    /// Upstream: Attr.java:4420-4478.
    #[allow(clippy::too_many_arguments)]
    fn select_sym(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        location: Option<SymbolId>,
        site: TypeId,
        env: &Rc<Env<AttrContext>>,
        result_info: &ResultInfo,
    ) -> SymbolId {
        let name = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { name, .. } => name.clone(),
            _ => panic!("ClassCastException: JCFieldAccess expected"),
        };
        let pos = simple_diag_pos(ctx.tree(tree).pos);
        match ctx.typ(site).get_tag() {
            TypeTag::PACKAGE => {
                let site_tsym = ctx.typ(site).tsym.expect("package symbol");
                let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
                let mut rs = rs_rc.borrow_mut();
                let found = rs.find_ident_in_package(
                    ctx,
                    syms,
                    pos.clone(),
                    env,
                    site_tsym,
                    &name,
                    result_info.pkind,
                );
                rs.access_base(ctx, syms, found, pos, location, Some(site), &name, true)
            }
            TypeTag::ARRAY | TypeTag::CLASS => {
                let pt_is_method = result_info
                    .pt
                    .map(|pt| {
                        let tag = ctx.typ(pt).get_tag();
                        tag == TypeTag::METHOD || tag == TypeTag::FORALL
                    })
                    .unwrap_or(false);
                if pt_is_method {
                    let pt = result_info.pt.expect("method prototype");
                    let argtypes = self.types.get_parameter_types_public(ctx, pt);
                    let typeargtypes = self.types.get_type_arguments_public(ctx, pt);
                    let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
                    let mut rs = rs_rc.borrow_mut();
                    let types_rc = Rc::clone(&self.types);
                    let chk_rc = Rc::clone(&self.chk);
                    let chk = chk_rc.borrow();
                    let infer_rc = Rc::clone(&self.infer);
                    rs.resolve_qualified_method(
                        ctx,
                        syms,
                        &types_rc,
                        self,
                        &chk,
                        &infer_rc,
                        pos.clone(),
                        env,
                        location.unwrap_or(syms.no_symbol),
                        site,
                        name.clone(),
                        argtypes,
                        Some(typeargtypes),
                    )
                } else if name == self.names._this || name == self.names._super {
                    let site_tsym = ctx.typ(site).tsym.expect("site type symbol");
                    let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
                    let mut rs = rs_rc.borrow_mut();
                    let types_rc = Rc::clone(&self.types);
                    rs.resolve_self(ctx, syms, &types_rc, pos, env, site_tsym, name)
                } else if name == self.names._class {
                    let types_rc = Rc::clone(&self.types);
                    let syms_guard = syms;
                    syms_guard.get_class_field(ctx, &types_rc, site)
                } else {
                    let found = {
                        let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
                        let mut rs = rs_rc.borrow_mut();
                        rs.find_ident_in_type(ctx, syms, pos.clone(), env, site, &name, result_info.pkind)
                    };
                    let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
                    let mut rs = rs_rc.borrow_mut();
                    rs.access_base(ctx, syms, found, pos, location, Some(site), &name, true)
                }
            }
            TypeTag::WILDCARD => panic!("AssertionError: wildcard select"),
            TypeTag::TYPEVAR => {
                let upper = ctx.typ(site).get_upper_bound();
                let mut sym: Option<SymbolId> = None;
                if let Some(upper) = upper {
                    let captured = self.types.capture(ctx, syms, upper);
                    sym = Some(self.select_sym(ctx, syms, tree, location, captured, env, result_info));
                }
                let Some(mut sym) = sym else {
                    self.attr_error(
                        ctx.tree(tree).pos,
                        crate::resources::compiler_properties::errors::type_var_cant_be_deref(),
                    );
                    return syms.err_symbol;
                };
                if ctx.sym(sym).flags_field & (crate::code::flags::PRIVATE as i64) != 0 {
                    // Java: sym2 = rs.new AccessError(env, site, sym);
                    let env_snapshot = crate::comp::resolve::resolve_env_snapshot(ctx, env);
                    let access_error = {
                        let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
                        let mut rs = rs_rc.borrow_mut();
                        let data = crate::code::symbol::ResolveErrorData::Access {
                            env: env_snapshot,
                            site: Some(site),
                            site_inaccessible: false,
                            sym,
                        };
                        rs.new_resolve_error(
                            ctx,
                            crate::code::kinds::Kind::HIDDEN,
                            "access error",
                            data,
                        )
                    };
                    sym = access_error;
                }
                let rs_rc = self.rs.borrow().clone().expect("AssertionError: rs service must be installed");
                let mut rs = rs_rc.borrow_mut();
                rs.access_base(ctx, syms, sym, pos, location, Some(site), &name, true);
                sym
            }
            TypeTag::ERROR => {
                let site_tsym = ctx.typ(site).tsym.expect("error type symbol");
                let error = self.types.create_error_type(ctx, name, site_tsym, Some(site));
                ctx.typ(error).tsym.expect("error type symbol")
            }
            _ => {
                // The qualifier expression is of a primitive type -- only
                // .class is allowed for these.
                if name == self.names._class {
                    let types_rc = Rc::clone(&self.types);
                    syms.get_class_field(ctx, &types_rc, site)
                } else {
                    self.attr_error(
                        ctx.tree(tree).pos,
                        crate::resources::compiler_properties::errors::cant_deref(
                            Self::type_arg_placeholder(ctx, site),
                        ),
                    );
                    syms.err_symbol
                }
            }
        }
    }
}

impl Attr {
    /// Java: public void visitNewArray(JCNewArray tree) (Attr.java:3034-3075).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the array creation tree.
    ///
    /// # Returns
    ///   - the attributed array type.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Attr.java:3034-3075.
    fn visit_new_array(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let (elemtype_tree, dims, elems) = match &ctx.tree(tree).kind {
            TreeKind::NewArray {
                elemtype, dims, elems, ..
            } => (*elemtype, dims.clone(), elems.clone()),
            _ => panic!("ClassCastException: JCNewArray expected"),
        };
        let mut owntype = self
            .types
            .create_error_type_original(ctx, ctx.tree(tree).ty, syms);
        let env = self.env.clone().expect("env");
        let local_env = env.dup(Some(tree), env.info.borrow().dup(env.info.borrow().scope));
        let elemtype: TypeId;
        if let Some(elemtype_tree) = elemtype_tree {
            elemtype = self.attrib_type_use(ctx, syms, elemtype_tree, &local_env);
            // Java: chk.validate(tree.elemtype, localEnv);
            // NOT PORTED: Check.validate/Validator (Check.java:1397-1525, Check batch);
            // registered.
            owntype = elemtype;
            let mut l = dims.clone();
            while l.non_empty() {
                let dim = *l.head();
                self.attrib_expr_with_type(ctx, syms, dim, &local_env, Some(syms.int_type));
                owntype = ctx.alloc_type(crate::code::type_::Type::new_array_type(
                    owntype,
                    syms.array_class,
                ));
                l = l.tail();
            }
        } else {
            // we are seeing an untyped aggregate { ... }
            // this is allowed only if the prototype is an array
            let pt = self.pt().unwrap_or_else(|| self.no_type(ctx));
            if ctx.typ(pt).has_tag(TypeTag::ARRAY) {
                elemtype = self.types.elemtype(ctx, syms, pt);
            } else {
                let encl_var_ok = {
                    let info = env.info.borrow();
                    match info.encl_var {
                        None => true,
                        Some(encl_var) => {
                            let encl_type = ctx.sym(encl_var).ty;
                            !encl_type
                                .map(|t| ctx.typ(t).has_tag(TypeTag::ERROR))
                                .unwrap_or(false)
                        }
                    }
                };
                if !ctx.typ(pt).has_tag(TypeTag::ERROR) && encl_var_ok {
                    self.attr_error(
                        ctx.tree(tree).pos,
                        crate::resources::compiler_properties::errors::illegal_initializer_for_type(
                            Self::type_arg_placeholder(ctx, pt),
                        ),
                    );
                }
                elemtype = self
                    .types
                    .create_error_type_original(ctx, Some(pt), syms);
            }
        }
        if let Some(elems) = elems {
            self.attrib_exprs(ctx, syms, elems, &local_env, Some(elemtype));
            owntype = ctx.alloc_type(crate::code::type_::Type::new_array_type(
                elemtype,
                syms.array_class,
            ));
        }
        if !self.types.is_reifiable(ctx, syms, elemtype) {
            self.attr_error(
                ctx.tree(tree).pos,
                crate::resources::compiler_properties::errors::generic_array_creation(),
            );
        }
        let result_info = self.result_info.clone().expect("resultInfo");
        let result = self.check(ctx, syms, tree, owntype, KindSelector::VAL, &result_info);
        self.result = Some(result);
        Ok(result)
    }

    /// Java: public void visitAssert(JCAssert tree) (Attr.java:2453-2459).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the assert statement.
    ///
    /// # Returns
    ///   - Type.noType (statement result).
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Attr.java:2453-2459.
    fn visit_assert(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let (cond, detail) = match &ctx.tree(tree).kind {
            TreeKind::Assert { cond, detail } => (*cond, *detail),
            _ => panic!("ClassCastException: JCAssert expected"),
        };
        let env = self.env.clone().expect("env");
        if let Some(cond) = cond {
            self.attrib_expr_with_type(ctx, syms, cond, &env, Some(syms.boolean_type));
        }
        if let Some(detail) = detail {
            let detail_type = self.attrib_expr(ctx, syms, detail, &env);
            let chk_rc = Rc::clone(&self.chk);
            let chk = chk_rc.borrow();
            chk.check_non_void(ctx, ctx.tree(detail).pos, detail_type);
        }
        Ok(self.no_type(ctx))
    }
}

impl Attr {
    /// Java: public void visitConditional(JCConditional tree) (Attr.java:1985-2053).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the conditional expression.
    ///
    /// # Returns
    ///   - the conditional type.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Attr.java:1985-2053.
    fn visit_conditional(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let (cond, truepart, falsepart) = match &ctx.tree(tree).kind {
            TreeKind::Conditional {
                cond, truepart, falsepart, ..
            } => (
                cond.expect("conditional cond"),
                truepart.expect("conditional truepart"),
                falsepart.expect("conditional falsepart"),
            ),
            _ => panic!("ClassCastException: JCConditional expected"),
        };
        let env = self.env.clone().expect("env");
        self.attrib_expr_with_type(ctx, syms, cond, &env, Some(syms.boolean_type));
        let cond_bindings = self.match_bindings.clone();
        let pt = self.pt().unwrap_or_else(|| self.no_type(ctx));
        let recovery_type = syms.recovery_type;
        let any_poly = self.infer.any_poly;
        let poly_kind = if !self.allow_poly
            || (ctx.typ(pt).has_tag(TypeTag::NONE)
                && pt != recovery_type
                && pt != any_poly)
            || self.is_boolean_or_numeric(ctx, syms, tree)
        {
            crate::tree::jc_tree::PolyKind::STANDALONE
        } else {
            crate::tree::jc_tree::PolyKind::POLY
        };
        if let TreeKind::Conditional { poly_kind: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = poly_kind;
        }
        let result_info = self.result_info.clone().expect("resultInfo");
        if poly_kind == crate::tree::jc_tree::PolyKind::POLY
            && ctx.typ(pt).has_tag(TypeTag::VOID)
        {
            // this means we are returning a poly conditional from a
            // void-compatible lambda expression
            let fragment = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::conditional_target_cant_be_void(),
            ));
            result_info.check_context.report(ctx, ctx.tree(tree).pos, &fragment);
            let error = self.types.create_error_type_original(ctx, Some(pt), syms);
            ctx.tree_mut(tree).ty = Some(error);
            self.result = Some(error);
            return Ok(error);
        }
        let cond_info = if poly_kind == crate::tree::jc_tree::PolyKind::STANDALONE {
            self.unknown_expr_info.clone()
        } else {
            result_info.dup_context(self.conditional_context())
        };
        let cond_bindings_true = cond_bindings.bindings_when_true.clone();
        let cond_bindings_false = cond_bindings.bindings_when_false.clone();
        let true_env = self.binding_env(ctx, syms, &env, cond_bindings_true);
        let truetype = self.attrib_tree_use(ctx, syms, truepart, &true_env, cond_info.clone());
        let true_bindings = self.match_bindings.clone();
        let false_env = self.binding_env(ctx, syms, &env, cond_bindings_false);
        let falsetype = self.attrib_tree_use(ctx, syms, falsepart, &false_env, cond_info);
        let false_bindings = self.match_bindings.clone();
        let mut owntype = if poly_kind == crate::tree::jc_tree::PolyKind::STANDALONE {
            self.cond_type(
                ctx,
                syms,
                &[ctx.tree(truepart).pos, ctx.tree(falsepart).pos],
                &[truetype, falsetype],
            )
        } else {
            pt
        };
        // Java: constant folding (condtype/truetype/falsetype const values)
        let cond_tree_type = ctx.tree(cond).ty;
        let cond_tree_type = cond_tree_type.unwrap_or_else(|| self.no_type(ctx));
        let cond_const = ctx.typ(cond_tree_type).const_value().cloned();
        let true_const = ctx.typ(truetype).const_value().cloned();
        let false_const = ctx.typ(falsetype).const_value().cloned();
        if cond_const.is_some()
            && true_const.is_some()
            && false_const.is_some()
            && !ctx.typ(owntype).has_tag(TypeTag::NONE)
        {
            let cond_type = ctx.tree(cond).ty.unwrap_or_else(|| self.no_type(ctx));
            let chosen = if ctx.typ(cond_type).is_true() { truetype } else { falsetype };
            let cfolder = crate::comp::const_fold::ConstFold::new();
            owntype = cfolder.coerce(ctx, syms, chosen, owntype);
        }
        let result_info2 = self.result_info.clone().expect("resultInfo");
        let result = self.check(ctx, syms, tree, owntype, KindSelector::VAL, &result_info2);
        self.result = Some(result);
        self.match_bindings = self.match_bindings_computer.conditional(
            ctx,
            ctx.tree(tree).pos,
            &cond_bindings,
            &true_bindings,
            &false_bindings,
        );
        Ok(result)
    }

    /// Java: private boolean isBooleanOrNumeric(Env<AttrContext> env,
    /// JCExpression tree) (Attr.java:2055-2091).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the conditional tree.
    ///
    /// # Returns
    ///   - true when the condition and both arms are boolean/numeric.
    ///
    /// # Panics
    ///   - speculative attribution failures propagate as panics (upstream
    ///     unchecked CompletionFailure).
    ///
    /// Upstream: Attr.java:2055-2091.
    fn is_boolean_or_numeric(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> bool {
        let env = self.env.clone().expect("env");
        match ctx.tree(tree).tag {
            Tag::LITERAL => {
                let typetag = match &ctx.tree(tree).kind {
                    TreeKind::Literal { typetag, .. } => *typetag,
                    _ => return false,
                };
                typetag.is_sub_range_of(TypeTag::DOUBLE)
                    || typetag == TypeTag::BOOLEAN
                    || typetag == TypeTag::BOT
            }
            Tag::LAMBDA | Tag::REFERENCE => false,
            Tag::PARENS => {
                let expr = match &ctx.tree(tree).kind {
                    TreeKind::Parens { expr } => expr.expect("parens expr"),
                    _ => return false,
                };
                self.is_boolean_or_numeric(ctx, syms, expr)
            }
            Tag::CONDEXPR => {
                let (truepart, falsepart) = match &ctx.tree(tree).kind {
                    TreeKind::Conditional { truepart, falsepart, .. } => (
                        truepart.expect("conditional truepart"),
                        falsepart.expect("conditional falsepart"),
                    ),
                    _ => return false,
                };
                self.is_boolean_or_numeric(ctx, syms, truepart)
                    && self.is_boolean_or_numeric(ctx, syms, falsepart)
            }
            Tag::APPLY => {
                let speculative = self.attrib_speculative_for_boolean(ctx, syms, tree, &env);
                let meth = match &ctx.tree(speculative).kind {
                    TreeKind::MethodInvocation { meth, .. } => {
                        meth.expect("speculative meth")
                    }
                    _ => return false,
                };
                let msym = match crate::tree::tree_info::symbol(ctx, meth) {
                    Some(msym) => msym,
                    None => return false,
                };
                let receiver_type = if ctx.tree(meth).tag == Tag::IDENT {
                    let encl_class = env.encl_class.expect("enclClass");
                    let encl_sym = class_tree_symbol(ctx, encl_class).expect("enclClass sym");
                    ctx.sym(encl_sym).ty.expect("enclClass type")
                } else {
                    let selected = match &ctx.tree(meth).kind {
                        TreeKind::FieldAccess { selected, .. } => selected.expect("selected"),
                        _ => return false,
                    };
                    ctx.tree(selected).ty.unwrap_or_else(|| self.no_type(ctx))
                };
                let member_type = self.types.member_type(ctx, syms, receiver_type, msym);
                let owntype = self.types.get_return_type_public(ctx, member_type);
                self.primitive_or_boxed(ctx, syms, owntype)
            }
            Tag::NEWCLASS => {
                let clazz = match &ctx.tree(tree).kind {
                    TreeKind::NewClass { clazz, .. } => clazz.expect("new class clazz"),
                    _ => return false,
                };
                let class_name = remove_class_params(ctx, clazz);
                let speculative = {
                    let deferred = self.deferred_attr.borrow().clone().expect("deferred attr service");
                    let mut deferred = deferred.borrow_mut();
                    let unknown_type = self.unknown_type_info.clone();
                    match deferred.attrib_speculative(ctx, syms, self, class_name, &env, &unknown_type) {
                        Ok(t) => t,
                        Err(_) => return false,
                    }
                };
                let speculative_type = ctx.tree(speculative).ty.unwrap_or_else(|| self.no_type(ctx));
                self.primitive_or_boxed(ctx, syms, speculative_type)
            }
            _ => {
                let speculative = self.attrib_speculative_for_boolean(ctx, syms, tree, &env);
                let speculative_type = ctx.tree(speculative).ty.unwrap_or_else(|| self.no_type(ctx));
                self.primitive_or_boxed(ctx, syms, speculative_type)
            }
        }
    }

    /// §9 helper: speculative attribution for isBooleanOrNumeric.
    fn attrib_speculative_for_boolean(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) -> TreeId {
        let deferred = self.deferred_attr.borrow().clone().expect("deferred attr service");
        let mut deferred = deferred.borrow_mut();
        let unknown = self.unknown_expr_info.clone();
        deferred
            .attrib_speculative(ctx, syms, self, tree, env, &unknown)
            .map_err(|failure| match failure {
                crate::comp::deferred_attr::DeferredAttrFailure::Completion(failure) => failure,
                other => panic!("unexpected speculative attribution failure: {:?}", other),
            })
            .expect("speculative attribution result")
    }

    /// Java: boolean primitiveOrBoxed(Type t) (Attr.java:2082-2084).
    fn primitive_or_boxed(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        t: TypeId,
    ) -> bool {
        !ctx.typ(t).has_tag(TypeTag::TYPEVAR)
            && !ctx.typ(t).has_tag(TypeTag::ERROR)
            && {
                let unboxed = self.types.unboxed_type_or_type(ctx, syms, t);
                ctx.typ(unboxed).is_primitive()
            }
    }

    /// Java: Env<AttrContext> bindingEnv(Env, List<BindingSymbol>)
    /// (Attr.java:2190-2199).
    fn binding_env(
        &self,
        ctx: &mut CompilerContext,
        _syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
        bindings: List<SymbolId>,
    ) -> Rc<Env<AttrContext>> {
        let new_scope = ctx.scope_dup(env.info.borrow().scope.expect("env scope"), None);
        let info = env.info.borrow().dup(Some(new_scope));
        let env1 = env.dup(env.tree, info);
        let mut current = bindings;
        while current.non_empty() {
            ctx.scope_enter(new_scope, *current.head());
            current = current.tail();
        }
        env1
    }

    /// Java: Type condType(List<DiagnosticPosition> positions,
    /// List<Type> condTypes) (Attr.java:2100-2170).
    fn cond_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        positions: &[i32],
        cond_types: &[TypeId],
    ) -> TypeId {
        if cond_types.is_empty() {
            return syms.object_type;
        }
        let first = cond_types[0];
        let mut all_same = true;
        for t in &cond_types[1..] {
            if !self.types.is_same_type(ctx, syms, first, *t) {
                all_same = false;
                break;
            }
        }
        if all_same {
            return first;
        }
        let unboxed: Vec<TypeId> = cond_types
            .iter()
            .map(|t| {
                if ctx.typ(*t).is_primitive() {
                    *t
                } else {
                    self.types.unboxed_type_ro(ctx, syms, *t)
                }
            })
            .collect();
        if unboxed.iter().all(|t| ctx.typ(*t).is_primitive()) {
            for t in &unboxed {
                let tag = ctx.typ(*t).get_tag();
                if !tag.is_strict_sub_range_of(TypeTag::INT) {
                    continue;
                }
                let all_ok = unboxed.iter().filter(|o| *o != t).all(|o| {
                    ctx.typ(*o).get_tag() == TypeTag::INT
                        && self.types.is_assignable(ctx, syms, *o, *t)
                });
                if all_ok {
                    return *t;
                }
            }
            for tag in [TypeTag::BYTE, TypeTag::CHAR, TypeTag::SHORT, TypeTag::INT, TypeTag::LONG, TypeTag::FLOAT, TypeTag::DOUBLE, TypeTag::BOOLEAN] {
                let candidate = syms.type_of_tag[tag as usize].expect("predefined type");
                if unboxed
                    .iter()
                    .all(|t| self.types.is_subtype(ctx, syms, *t, candidate))
                {
                    return candidate;
                }
            }
        }
        let boxed: Vec<TypeId> = cond_types
            .iter()
            .map(|t| {
                if ctx.typ(*t).is_primitive() {
                    let boxed_sym = self.types.boxed_class(ctx, syms, *t);
                    ctx.sym(boxed_sym).ty.expect("boxed type")
                } else {
                    *t
                }
            })
            .collect();
        for t in &boxed {
            let all_ok = boxed.iter().filter(|o| *o != t).all(|o| {
                self.types.is_assignable(ctx, syms, *o, *t)
            });
            if all_ok {
                return *t;
            }
        }
        let mut checked: Vec<TypeId> = Vec::new();
        for (index, t) in boxed.iter().enumerate() {
            let pos = positions.get(index).copied().unwrap_or(0);
            let chk_rc = Rc::clone(&self.chk);
            let chk = chk_rc.borrow();
            checked.push(chk.check_non_void(ctx, pos, *t));
        }
        let mut lub_types: List<TypeId> = List::nil();
        for t in checked {
            if !ctx.typ(t).has_tag(TypeTag::BOT) {
                lub_types = lub_types.append(t);
            }
        }
        let lub_vec: Vec<TypeId> = {
            let mut v = Vec::new();
            let mut current = lub_types;
            while current.non_empty() {
                v.push(*current.head());
                current = current.tail();
            }
            v
        };
        self.types.lub(ctx, syms, &self.names, &lub_vec)
    }
}

/// §9 helper: TreeTranslator removeClassParams (Attr.java:2090-2095):
/// translate a type-apply to its clazz.
fn remove_class_params(ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
    if ctx.tree(tree).tag == Tag::TYPEAPPLY {
        let clazz = match &ctx.tree(tree).kind {
            TreeKind::TypeApply { clazz, .. } => clazz.expect("type apply clazz"),
            _ => tree,
        };
        return remove_class_params(ctx, clazz);
    }
    tree
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
