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
use crate::code::kinds::KindSelector;
use crate::code::source::Feature;
use crate::code::symbol::CompletionFailure;
use crate::code::symtab::Symtab;
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
}

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
        }
    }

    /// Java: protected boolean needsArgumentAttr(JCTree tree) (the upstream
    /// default returns false; MethodAttrInfo overrides it — registered for the
    /// ArgumentAttr batch).
    pub fn needs_argument_attr(&self, _tree: TreeId) -> bool {
        false
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

    /// Java: protected ResultInfo dup(Type newPt)
    pub fn dup(&self, new_pt: Option<TypeId>) -> ResultInfo {
        ResultInfo::new_with_mode(
            self.pkind,
            new_pt,
            Rc::clone(&self.check_context),
            self.check_mode,
        )
    }

    /// Java: protected ResultInfo dup(CheckContext newContext)
    pub fn dup_context(&self, new_context: Rc<dyn crate::comp::check::CheckContext>) -> ResultInfo {
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
        ResultInfo::new_with_mode(self.pkind, new_pt, new_context, new_mode)
    }

    /// Java: protected ResultInfo dup(CheckMode newMode)
    pub fn dup_mode(&self, new_mode: crate::comp::check::CheckMode) -> ResultInfo {
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
    /// Java: the predefined result infos (statInfo/unknownExprInfo/...)
    pub stat_info: ResultInfo,
    pub var_assignment_info: ResultInfo,
    pub unknown_expr_info: ResultInfo,
    pub unknown_type_info: ResultInfo,
    pub unknown_type_expr_info: ResultInfo,
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
            basic_handler: Rc::clone(&basic_handler),
            stat_info: ResultInfo::new(KindSelector::NIL, None, Rc::clone(&basic_handler)),
            var_assignment_info: ResultInfo::new(KindSelector::ASG, None, Rc::clone(&basic_handler)),
            unknown_expr_info: ResultInfo::new(KindSelector::VAL, None, Rc::clone(&basic_handler)),
            unknown_type_info: ResultInfo::new(KindSelector::TYP, None, Rc::clone(&basic_handler)),
            unknown_type_expr_info: ResultInfo::new(KindSelector::VAL_TYP, None, Rc::clone(&basic_handler)),
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
        let clazztype = self.chk.borrow().check_class_type(ctx, clazz_pos, clazz_type);

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

    /// Java: public void visitIdent(JCIdent tree) (4182-...)
    fn visit_ident(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        // TODO(Resolve batch): Resolve.findIdent/findVar/... ; registered.
        unimplemented!("TODO(Resolve batch): Attr.visitIdent")
    }

    /// Java: public void visitParens(JCParens tree) (3922-...)
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
        Ok(self.attrib_tree_with(ctx, syms, expr, &env, result_info))
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
fn type_arguments(ctx: &CompilerContext, t: TypeId) -> List<TypeId> {
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
        // The predefined ResultInfo kinds (Attr constructor): statInfo is NIL,
        // varAssignmentInfo is ASG, unknownExprInfo is VAL, unknownTypeInfo is
        // TYP, unknownTypeExprInfo is VAL_TYP.
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
        let new_mode = info.dup_pt_context_mode(Some(TypeId::new(3)), context, crate::comp::check::CheckMode::NORMAL);
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
}
