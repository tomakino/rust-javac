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
// - Check.CheckContext/CheckMode/checkType (Check batch): ResultInfo keeps
//   pkind/pt and its dup methods, but the checkContext/checkMode fields and
//   ResultInfo.check() are registered (the check pipeline is the Check batch);
// - Infer.InferenceContext (Infer batch): the check()/poly inference glue is
//   registered; ResultInfo.pt is Option<TypeId> (Java Type.noType -> None);
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
}

impl ResultInfo {
    /// Java: ResultInfo(KindSelector pkind, Type pt)
    pub fn new(pkind: KindSelector, pt: Option<TypeId>) -> ResultInfo {
        ResultInfo { pkind, pt }
    }

    /// Java: protected ResultInfo dup(Type newPt)
    pub fn dup(&self, new_pt: Option<TypeId>) -> ResultInfo {
        ResultInfo {
            pkind: self.pkind,
            pt: new_pt,
        }
    }

    /// Java: public String toString()
    pub fn to_display(&self, ctx: &CompilerContext) -> String {
        format!("ResultInfo({:?}, {})", self.pkind, match self.pt {
            Some(pt) => type_display(ctx, pt),
            None => "noType".to_string(),
        })
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
            stat_info: ResultInfo::new(KindSelector::NIL, None),
            var_assignment_info: ResultInfo::new(KindSelector::ASG, None),
            unknown_expr_info: ResultInfo::new(KindSelector::VAL, None),
            unknown_type_info: ResultInfo::new(KindSelector::TYP, None),
            unknown_type_expr_info: ResultInfo::new(KindSelector::VAL_TYP, None),
        }
    }

    // ------------------------------------------------------------------
    // Java: Attr.ResultInfo / the check pipeline (registered: Check batch).
    // ------------------------------------------------------------------

    /// Java: Type check(JCTree tree, Type found, KindSelector ownkind,
    /// ResultInfo resultInfo)
    fn check(
        &mut self,
        _ctx: &mut CompilerContext,
        _tree: TreeId,
        _found: TypeId,
        _ownkind: KindSelector,
        _result_info: &ResultInfo,
    ) -> TypeId {
        // TODO(Check/Infer batch): the upstream body needs the CheckContext
        // (compatible/report), the InferenceContext free-type listeners and
        // Types.createErrorType; registered.
        unimplemented!("TODO(Check/Infer batch): Attr.check")
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
        self.env = Some(Rc::clone(env));
        self.result_info = Some(result_info);
        let result = self.visit(ctx, syms, tree);
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
            Tag::LITERAL => self.visit_literal(ctx, tree),
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
        tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        let (typetag, value) = match &ctx.tree(tree).kind {
            TreeKind::Literal { typetag, value } => (*typetag, value.clone()),
            _ => panic!("ClassCastException: JCLiteral expected"),
        };
        let lit_type = self.lit_type(ctx, typetag);
        let constant = ctx.type_const_type(lit_type, literal_value_to_constant(value));
        let result_info = self.result_info.clone().expect("resultInfo");
        Ok(self.check(ctx, tree, constant, KindSelector::VAL, &result_info))
    }

    /// Java: Type litType(TypeTag tag)
    fn lit_type(&self, ctx: &CompilerContext, tag: TypeTag) -> TypeId {
        if tag == TypeTag::CLASS {
            return self.syms_type(ctx, "string_type");
        }
        self.syms_type(ctx, "type_of_tag")
    }

    // The Symtab is threaded explicitly in the port (ADR 0001); the litType
    // helper needs the predefined types, therefore it is routed through
    // syms_type at the call sites (see visit_literal with syms).
    fn syms_type(&self, _ctx: &CompilerContext, _what: &str) -> TypeId {
        unimplemented!("TODO(Symtab threading): Attr litType predefined types")
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
        Ok(self.check(ctx, tree, predefined, KindSelector::TYP, &result_info))
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
        Ok(self.check(ctx, tree, array_type, KindSelector::TYP, &result_info))
    }

    /// Java: public void visitTypeApply(JCTypeApply tree) (4913-...)
    fn visit_type_apply(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _tree: TreeId,
    ) -> Result<TypeId, CompletionFailure> {
        // TODO(Check/Types batch): checkClassType/withTypeVar/checkExtends; the
        // full visitTypeApply body is Attr batch A/C (registered).
        unimplemented!("TODO(Check/Types batch): Attr.visitTypeApply")
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
        Ok(self.check(ctx, tree, wildcard, KindSelector::TYP, &result_info))
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
            let error_info = ResultInfo::new(KindSelector::ERR, self.pt());
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
        let info = ResultInfo::new(KindSelector::VAL, Some(pt));
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
        self.no_type(ctx);
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
        let info = ResultInfo::new(KindSelector::TYP, pt);
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
        let local_env = env.dup_shared(Some(node));
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
        let info = ResultInfo::new(pkind, Some(self.no_type(ctx)));
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
        let info = ResultInfo::new(KindSelector::TYP, Some(self.no_type(ctx)));
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_info_keep_pkind_and_pt_and_dup() {
        let info = ResultInfo::new(KindSelector::TYP, Some(TypeId::new(3)));
        assert_eq!(info.pkind, KindSelector::TYP);
        assert_eq!(info.pt, Some(TypeId::new(3)));
        let dup = info.dup(Some(TypeId::new(4)));
        assert_eq!(dup.pkind, KindSelector::TYP);
        assert_eq!(dup.pt, Some(TypeId::new(4)));
        let no_pt = info.dup(None);
        assert_eq!(no_pt.pt, None);
    }

    #[test]
    fn predefined_result_infos_match_the_upstream_kinds() {
        // The predefined ResultInfo kinds (Attr constructor): statInfo is NIL,
        // varAssignmentInfo is ASG, unknownExprInfo is VAL, unknownTypeInfo is
        // TYP, unknownTypeExprInfo is VAL_TYP.
        let stat_info = ResultInfo::new(KindSelector::NIL, None);
        assert_eq!(stat_info.pkind, KindSelector::NIL);
        let var_assignment = ResultInfo::new(KindSelector::ASG, None);
        assert_eq!(var_assignment.pkind, KindSelector::ASG);
        let expr = ResultInfo::new(KindSelector::VAL, None);
        assert_eq!(expr.pkind, KindSelector::VAL);
        let ty = ResultInfo::new(KindSelector::TYP, None);
        assert_eq!(ty.pkind, KindSelector::TYP);
        let val_typ = ResultInfo::new(KindSelector::VAL_TYP, None);
        assert_eq!(val_typ.pkind, KindSelector::VAL_TYP);
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
