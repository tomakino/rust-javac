//
// Registration mapping (): this file ports
// - ArgumentAttr (instance/constructor/setResult/checkSpeculative/
// withLocalCacheContext/LocalCacheContext/attribArg/visitTree/processArg/
// visitParens/visitConditional/visitSwitchExpression/visitReference/
// visitLambda/visitApply/visitNewClass),
// - the ArgumentType base class and its six concrete subclasses
// (ParensType/ConditionalType/SwitchExpressionType/ExplicitLambdaType/
// ResolvedMethodType/ResolvedConstructorType),
// - UniquePos.
// adapter registered adapters:
// - Java class inheritance (ArgumentType extends DeferredType, and the six
// subclasses) is expressed by the ArgumentTypeData payload carried by
// TypeKindData::Deferred plus kind dispatch in this file; the overridden
// DeferredType methods (complete/speculativeType/speculativeTree) dispatch on
// that payload in deferred_attr.rs;
// - Java Map<ResultInfo,Type> object identity is keyed by ResultInfo.id
// (attr.rs) in a Vec of (id, TypeId) pairs, preserving insertion order;
// - Java Optional fields are Option fields; Java HashMap/LinkedHashMap caches
// are Vec/RefCell maps preserving the port cache policy (entries are removed
// on completion, see LocalCacheContext);
// - the Java visitor dispatch becomes a match in ArgumentAttr::visit (the
// ported tree model has no visitor base class of its own);
// - explicit ctx/syms/env parameters follow ADR 0001;
// - UniquePos identity uses (pos, DiagnosticSource identity); the port keys it
// by the source file identity (Rc::as_ptr of the JavaFileObject of
// log.current_source) because DiagnosticSource is value-copied in the port.

use std::cell::RefCell;
use std::rc::Rc;

use crate::comp::attr::{Attr, ResultInfo};
use crate::comp::env::Env;
use crate::compiler_context::CompilerContext;
use crate::ids::SymbolId;
use crate::ids::TreeId;
use crate::ids::TypeId;
use crate::tree::tree_scanner::TreeScannerVisitor;
use crate::util::list::List;

/// Java: the six concrete ArgumentAttr.ArgumentType subclasses (ArgumentAttr.java:
/// ParensType 405-425, ConditionalType 430-456, SwitchExpressionType 461-513,
/// ExplicitLambdaType 518-640, ResolvedMemberType/ResolvedMethodType/
/// ResolvedConstructorType 645-735).
///
/// # Arguments
///   - (none: enum only).
///
/// # Returns
///   - the enum value identifying the concrete ArgumentType subclass.
///
/// # Panics
///   - none.
///
/// Upstream: ArgumentAttr.java:405-735 (ParensType/ConditionalType/SwitchExpressionType/ExplicitLambdaType/ResolvedMethodType/ResolvedConstructorType).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgumentKind {
    Parens,
    Conditional,
    SwitchExpression,
    ExplicitLambda,
    ResolvedMethod,
    ResolvedConstructor,
}

/// Java: ArgumentAttr.ArgumentType<T> state (ArgumentAttr.java:641-740), carried
/// by TypeKindData::Deferred::argument.
///
/// The parent DeferredType state (tree/env/mode/notPertinentToApplicability/
/// speculativeCache) stays in the Deferred variant; this payload adds the
/// speculative tree, the ResultInfo-keyed speculative types map and the lazily
/// populated scratch lists of SwitchExpressionType/ExplicitLambdaType.
///
/// # Fields
///   - kind: the concrete subclass (Porting rules enum dispatch);
///   - speculative_tree: the speculative pass tree (ArgumentAttr.java:330-350);
///   - speculative_types: ResultInfo->Type entries (ArgumentAttr.java:360-390);
///   - yield_expressions/argtypes/return_expressions: the lazily populated scratch lists.
///
/// # Panics
///   - none.
///
/// Upstream: ArgumentAttr.java:330-400, 480-500, 515-640.
#[derive(Clone)]
pub struct ArgumentTypeData {
    /// The concrete subclass.
    pub kind: ArgumentKind,
    /// Java: T speculativeTree
    pub speculative_tree: TreeId,
    /// Java: Map<ResultInfo, Type> speculativeTypes. Java keys by ResultInfo
    /// object identity and shares the map object across dup(...) calls; the
    /// port keeps (ResultInfo.id, ResultInfo, Type) triples behind a shared
    /// Rc<RefCell<...>> so the cached argument type and every dup observe the
    /// same entries (ArgumentAttr.java:343-348, 357, 419, 454, 510, 622, 687,
    /// 717).
    pub speculative_types: Rc<RefCell<Vec<(u64, Rc<ResultInfo>, TypeId)>>>,
    /// Java: SwitchExpressionType.yieldExpressions (Optional<List<JCYield>>)
    pub yield_expressions: RefCell<Option<List<TreeId>>>,
    /// Java: ExplicitLambdaType.argtypes (Optional<List<Type>>)
    pub argtypes: RefCell<Option<List<TypeId>>>,
    /// Java: ExplicitLambdaType.returnExpressions (Optional<List<JCReturn>>)
    pub return_expressions: RefCell<Option<List<TreeId>>>,
}

// ---------------------------------------------------------------------------
// ArgumentAttr (ArgumentAttr.java:96-790). Explicit ctx/syms/attr/
// deferred parameters per ADR 0001; the tree visitor dispatch becomes an
// ArgumentAttr::visit match (adapter, no visitor base class in the ported model).
// ---------------------------------------------------------------------------

/// Java: public class ArgumentAttr (ArgumentAttr.java:96-790)
///
/// # Fields
///   - deferred: DeferredAttr service cell (Porting rules, mutual recursion);
///   - diags: JCDiagnostic.Factory;
///   - env: the current attribution environment (AttribArg save/restore);
///   - result: the attribution result;
///   - argument_type_cache: UniquePos -> ArgumentType cache;
///   - flow: Flow service cell (Porting rules).
///
/// # Panics
///   - none.
///
/// Upstream: ArgumentAttr.java:96-130.
#[derive(Clone)]
pub struct ArgumentAttr {
    /// Java: final DeferredAttr deferredAttr (cell: DeferredAttr and
    /// ArgumentAttr are mutually recursive services, Porting rules).
    pub deferred: Rc<RefCell<Option<Rc<RefCell<crate::comp::deferred_attr::DeferredAttrService>>>>>,
    /// Java: final JCDiagnostic.Factory diags
    pub diags: Rc<crate::util::jc_diagnostic::Factory>,
    /// Java: private Env<AttrContext> env
    pub env: Option<Rc<Env<crate::comp::attr_context::AttrContext>>>,
    /// Java: Type result
    pub result: Option<TypeId>,
    /// Java: Map<UniquePos, ArgumentType<?>> argumentTypeCache
    ///
    /// Port adapter (A3k): the cache is shared by the per-attribution session
    /// clones, so completions started from the canonical service observe the
    /// same entries (upstream has a single ArgumentAttr object).
    pub argument_type_cache: Rc<RefCell<std::collections::HashMap<UniquePos, Rc<ArgumentTypeData>>>>,
    /// Java: final Flow flow (cell: the wiring installs it; Porting rules). Used by
    /// attribSpeculativeLambda's flow.analyzeLambda call.
    pub flow: Rc<RefCell<Option<Rc<crate::comp::flow::Flow>>>>,
}

/// Java: class ArgumentAttr.UniquePos (ArgumentAttr.java:745-775)
///
/// # Fields
///   - pos: source char offset;
///   - source: source-file identity (the file object pointer, Porting rules).
///
/// # Panics
///   - none.
///
/// Upstream: ArgumentAttr.java:745-775.
#[derive(Clone, Debug)]
pub struct UniquePos {
    /// Java: int pos
    pub pos: i32,
    /// Java: DiagnosticSource source (identity by file pointer, Porting rules)
    pub source: Option<usize>,
}

impl UniquePos {
    /// Java: UniquePos(JCTree tree)
    ///
    /// # Arguments
    ///   - ctx: the compiler context (tree position);
    ///   - log: the Log (current source identity);
    ///   - tree: the tree whose position is captured.
    ///
    /// # Returns
    ///   - a UniquePos for the tree.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: ArgumentAttr.java:745-760.
    pub fn new(
        ctx: &crate::compiler_context::CompilerContext,
        log: &crate::util::log::Log,
        tree: TreeId,
    ) -> UniquePos {
        let source = log
            .current_source()
            .map(|source| Rc::as_ptr(&source) as usize);
        UniquePos {
            pos: ctx.tree(tree).pos,
            source,
        }
    }
}

impl PartialEq for UniquePos {
    fn eq(&self, other: &UniquePos) -> bool {
        self.pos == other.pos && self.source == other.source
    }
}

impl Eq for UniquePos {}

impl std::hash::Hash for UniquePos {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.pos.hash(state);
        self.source.hash(state);
    }
}

/// Java: class ArgumentAttr.LocalCacheContext (ArgumentAttr.java:213-230).
///
/// # Fields
///   - prev_cache: the cache of the enclosing context restored by leave().
///
/// # Panics
///   - none.
///
/// Upstream: ArgumentAttr.java:213-230.
pub struct LocalCacheContext {
    /// Java: Map<UniquePos, ArgumentType<?>> prevCache
    pub prev_cache: Option<std::collections::HashMap<UniquePos, Rc<ArgumentTypeData>>>,
}

impl LocalCacheContext {
    /// Java: public void leave()
    ///
    /// # Arguments
    ///   - argument_attr: the service whose cache is restored.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: ArgumentAttr.java:226-230.
    pub fn leave(&mut self, argument_attr: &ArgumentAttr) {
        if let Some(prev) = self.prev_cache.take() {
            *argument_attr.argument_type_cache.borrow_mut() = prev;
        }
    }
}

impl ArgumentAttr {
    /// Java: public static ArgumentAttr instance(Context) — explicit
    /// construction per ADR 0001.
    ///
    /// # Arguments
    ///   - deferred: the DeferredAttr service cell;
    ///   - diags: the diagnostic factory.
    ///
    /// # Returns
    ///   - the shared ArgumentAttr cell (Porting rules cell injection).
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: ArgumentAttr.java:112-130.
    pub fn new(
        deferred: Rc<RefCell<Option<Rc<RefCell<crate::comp::deferred_attr::DeferredAttrService>>>>>,
        diags: Rc<crate::util::jc_diagnostic::Factory>,
    ) -> Rc<RefCell<ArgumentAttr>> {
        Rc::new(RefCell::new(ArgumentAttr {
            deferred,
            diags,
            env: None,
            result: None,
            argument_type_cache: Rc::new(RefCell::new(std::collections::HashMap::new())),
            flow: Rc::new(RefCell::new(None)),
        }))
    }

    /// Java: void setResult(JCExpression tree, Type type) (ArgumentAttr.java:135-142)
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - tree: the attributed expression;
    ///   - ty: the attribution result.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: ArgumentAttr.java:135-142.
    pub fn set_result(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: TreeId,
        ty: TypeId,
    ) {
        self.result = Some(ty);
        let speculative = self
            .env
            .as_ref()
            .map(|env| {
                env.info.borrow().attribution_mode
                    == crate::comp::deferred_attr::AttributionMode::SPECULATIVE
            })
            .unwrap_or(false);
        if speculative {
            // if we are in a speculative branch we can save the type in the tree
            // itself as there is no risk of polluting the original tree.
            ctx.tree_mut(tree).ty = Some(ty);
        }
    }

    /// Java: Type checkSpeculative(DiagnosticPosition pos, Type t,
    /// ResultInfo resultInfo) (ArgumentAttr.java:157-166)
    ///
    /// # Arguments
    ///   - ctx/syms/types/attr: explicit services (ADR 0001);
    ///   - t: the candidate type;
    ///   - result_info: the target result.
    ///
    /// # Returns
    ///   - Ok(Some(type)) or Ok(None); a deferred type is checked through DeferredAttr.
    ///
    /// # Panics
    ///   - none (failures are returned).
    ///
    /// Upstream: ArgumentAttr.java:157-166.
    pub fn check_speculative_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut Attr,
        t: TypeId,
        result_info: &ResultInfo,
    ) -> Result<Option<TypeId>, crate::code::symbol::CompletionFailure> {
        if ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::DEFERRED) {
            // ((DeferredType)t).check(resultInfo)
            let checked = crate::comp::deferred_attr::deferred_type_check(
                ctx, syms, types, attr, t, result_info,
            );
            match checked {
                Ok(ty) => Ok(Some(ty)),
                Err(failure) => Err(deferred_failure_to_completion(failure)),
            }
        } else {
            let chk_rc = Rc::clone(&attr.chk);
            let chk = Rc::clone(&chk_rc);
            Ok(Some(result_info.check(ctx, syms, &chk, 0, t)))
        }
    }

    /// Java: Type checkSpeculative(JCTree expr, ResultInfo resultInfo)
    ///
    /// # Arguments
    ///   - ctx/syms/types/attr: explicit services;
    ///   - expr: the expression (its tree type is checked);
    ///   - result_info: the target result.
    ///
    /// # Returns
    ///   - Ok(Some(type)) or Ok(None) when the tree has no type.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: ArgumentAttr.java:150-155.
    pub fn check_speculative(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        attr: &mut Attr,
        expr: TreeId,
        result_info: &ResultInfo,
    ) -> Result<Option<TypeId>, crate::code::symbol::CompletionFailure> {
        match ctx.tree(expr).ty {
            Some(ty) => self.check_speculative_type(ctx, syms, types, attr, ty, result_info),
            None => Ok(None),
        }
    }

    /// Java: LocalCacheContext withLocalCacheContext() (ArgumentAttr.java:196-198)
    ///
    /// # Arguments
    ///   - (none).
    ///
    /// # Returns
    ///   - a LocalCacheContext that must be left after the speculative pass.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: ArgumentAttr.java:196-198.
    pub fn with_local_cache_context(&self) -> LocalCacheContext {
        let mut cache = self.argument_type_cache.borrow_mut();
        let prev = std::mem::take(&mut *cache);
        LocalCacheContext {
            prev_cache: Some(prev),
        }
    }

    /// Java: Type attribArg(JCTree tree, Env<AttrContext> env)
    /// (ArgumentAttr.java:232-242)
    ///
    /// # Arguments
    ///   - ctx/syms/attr/log: explicit services;
    ///   - tree: the argument tree;
    ///   - env: the attribution environment.
    ///
    /// # Returns
    ///   - Ok(Some(type)) with the attribution result.
    ///
    /// # Panics
    ///   - none (completion failures are returned).
    ///
    /// Port adapter: the Log is passed as its Rc<Log> because the
    /// argument attribution may report diagnostics while the caller holds no
    /// Log borrow.
    pub fn attrib_arg(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        log: &Rc<crate::util::log::Log>,
        tree: TreeId,
        env: &Rc<Env<crate::comp::attr_context::AttrContext>>,
    ) -> Result<Option<TypeId>, crate::code::symbol::CompletionFailure> {
        let prev_env = self.env.clone();
        self.env = Some(Rc::clone(env));
        let result = self.visit(ctx, syms, attr, log, tree);
        self.env = prev_env;
        result
    }

    /// Java: the JCTree visitor dispatch; visitTree delegates to Attr (Porting rules match).
    fn visit(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        log: &Rc<crate::util::log::Log>,
        tree: TreeId,
    ) -> Result<Option<TypeId>, crate::code::symbol::CompletionFailure> {
        use crate::tree::jc_tree::Tag;
        match ctx.tree(tree).tag {
            Tag::PARENS => self.visit_parens(ctx, syms, attr, log, tree),
            Tag::CONDEXPR => self.visit_conditional(ctx, syms, attr, log, tree),
            Tag::SWITCH_EXPRESSION => {
                self.visit_switch_expression(ctx, syms, attr, log, tree)
            }
            Tag::REFERENCE => self.visit_reference(ctx, syms, attr, log, tree),
            Tag::LAMBDA => self.visit_lambda(ctx, syms, attr, log, tree),
            Tag::APPLY => self.visit_apply(ctx, syms, attr, log, tree),
            Tag::NEWCLASS => self.visit_new_class(ctx, syms, attr, log, tree),
            _ => {
                // Java: visitTree(JCTree that): that.accept(attr);
                // result = attr.result;
                let result = attr.attrib_tree(
                    ctx,
                    syms,
                    tree,
                    self.env.as_ref().expect("argument attr env"),
                    attr.unknown_expr_info.clone(),
                )?;
                self.result = attr.result.or(Some(result));
                Ok(self.result)
            }
        }
    }
}

impl ArgumentAttr {
    /// Porting rules: MethodAttrInfo with the processArg needsArgumentAttr override
    /// (the current argument position is excluded).
    fn argument_result_info(
        &self,
        attr: &Attr,
        ctx: &crate::compiler_context::CompilerContext,
        log: &Rc<crate::util::log::Log>,
        tree: TreeId,
    ) -> ResultInfo {
        let pos = UniquePos::new(ctx, &*log, tree);
        // Java: ArgumentAttr.processArg uses attr.new MethodAttrInfo() — the
        // Attr.MethodAttrInfo() no-arg constructor calls
        // MethodAttrInfo(Attr.this.basicHandler) (Attr.java:563-596), NOT the
        // enclosing methodAttrInfo. The basic handler's inference context is the
        // empty context; cloning methodAttrInfo would carry the enclosing
        // method's MethodCheckContext and make shouldPropagate() see a generic
        // enclosing context that does not exist for this argument.
        let mut info = crate::comp::attr::ResultInfo::method_attr_info(
            Rc::clone(&attr.basic_handler),
            attr.infer.any_poly,
        );
        info.argument_attr_exclude = Some(pos);
        info
    }

    /// Java: void visitParens(JCParens that) (ArgumentAttr.java:268-271)
    fn visit_parens(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        log: &Rc<crate::util::log::Log>,
        tree: TreeId,
    ) -> Result<Option<TypeId>, crate::code::symbol::CompletionFailure> {
        self.process_arg_with_speculation(ctx, syms, attr, log, tree, |speculative_tree| {
            ArgumentTypeData {
                kind: ArgumentKind::Parens,
                speculative_tree,
                speculative_types: Rc::new(RefCell::new(Vec::new())),
                yield_expressions: RefCell::new(None),
                argtypes: RefCell::new(None),
                return_expressions: RefCell::new(None),
            }
        })
    }

    /// Java: void visitConditional(JCConditional that) (ArgumentAttr.java:273-276)
    fn visit_conditional(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        log: &Rc<crate::util::log::Log>,
        tree: TreeId,
    ) -> Result<Option<TypeId>, crate::code::symbol::CompletionFailure> {
        self.process_arg_with_speculation(ctx, syms, attr, log, tree, |speculative_tree| {
            ArgumentTypeData {
                kind: ArgumentKind::Conditional,
                speculative_tree,
                speculative_types: Rc::new(RefCell::new(Vec::new())),
                yield_expressions: RefCell::new(None),
                argtypes: RefCell::new(None),
                return_expressions: RefCell::new(None),
            }
        })
    }

    /// Java: void visitSwitchExpression(JCSwitchExpression that)
    /// (ArgumentAttr.java:278-280)
    fn visit_switch_expression(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        log: &Rc<crate::util::log::Log>,
        tree: TreeId,
    ) -> Result<Option<TypeId>, crate::code::symbol::CompletionFailure> {
        self.process_arg_with_speculation(ctx, syms, attr, log, tree, |speculative_tree| {
            ArgumentTypeData {
                kind: ArgumentKind::SwitchExpression,
                speculative_tree,
                speculative_types: Rc::new(RefCell::new(Vec::new())),
                yield_expressions: RefCell::new(None),
                argtypes: RefCell::new(None),
                return_expressions: RefCell::new(None),
            }
        })
    }

    /// Java: void visitApply(JCMethodInvocation that) (ArgumentAttr.java:313-321)
    fn visit_apply(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        log: &Rc<crate::util::log::Log>,
        tree: TreeId,
    ) -> Result<Option<TypeId>, crate::code::symbol::CompletionFailure> {
        let typeargs_empty = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::MethodInvocation { typeargs, .. } => typeargs.is_empty(),
            _ => panic!("ClassCastException: JCMethodInvocation expected"),
        };
        if typeargs_empty {
            self.process_arg_with_speculation(ctx, syms, attr, log, tree, |speculative_tree| {
                ArgumentTypeData {
                    kind: ArgumentKind::ResolvedMethod,
                    speculative_tree,
                    speculative_types: Rc::new(RefCell::new(Vec::new())),
                    yield_expressions: RefCell::new(None),
                    argtypes: RefCell::new(None),
                    return_expressions: RefCell::new(None),
                }
            })
        } else {
            // not a poly expression, just call Attr
            let result = attr.attrib_tree(
                ctx,
                syms,
                tree,
                self.env.as_ref().expect("argument attr env"),
                attr.unknown_expr_info.clone(),
            )?;
            self.set_result(ctx, tree, result);
            Ok(Some(result))
        }
    }

    /// Java: void visitNewClass(JCNewClass that) (ArgumentAttr.java:324-332)
    fn visit_new_class(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        log: &Rc<crate::util::log::Log>,
        tree: TreeId,
    ) -> Result<Option<TypeId>, crate::code::symbol::CompletionFailure> {
        if crate::tree::tree_info::is_diamond(ctx, tree) {
            self.process_arg_with_speculation(ctx, syms, attr, log, tree, |speculative_tree| {
                ArgumentTypeData {
                    kind: ArgumentKind::ResolvedConstructor,
                    speculative_tree,
                    speculative_types: Rc::new(RefCell::new(Vec::new())),
                    yield_expressions: RefCell::new(None),
                    argtypes: RefCell::new(None),
                    return_expressions: RefCell::new(None),
                }
            })
        } else {
            // not a poly expression, just call Attr
            let result = attr.attrib_tree(
                ctx,
                syms,
                tree,
                self.env.as_ref().expect("argument attr env"),
                attr.unknown_expr_info.clone(),
            )?;
            self.set_result(ctx, tree, result);
            Ok(Some(result))
        }
    }

    /// Java: void visitLambda(JCLambda that) (ArgumentAttr.java:294-310)
    fn visit_lambda(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        log: &Rc<crate::util::log::Log>,
        tree: TreeId,
    ) -> Result<Option<TypeId>, crate::code::symbol::CompletionFailure> {
        let explicit = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::Lambda { param_kind, .. } => {
                *param_kind == crate::tree::jc_tree::ParameterKind::EXPLICIT
            }
            _ => panic!("ClassCastException: JCLambda expected"),
        };

        if explicit {
            self.process_arg_with_lambda_speculation(ctx, syms, attr, log, tree)
        } else {
            // otherwise just use a deferred type
            let env = self.env.clone().expect("env");
            let deferred_type = self.new_plain_deferred(ctx, syms, attr, tree, &env);
            self.set_result(ctx, tree, deferred_type);
            Ok(Some(deferred_type))
        }
    }

    /// Java: void visitReference(JCMemberReference tree) (ArgumentAttr.java:282-311)
    fn visit_reference(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        _log: &Rc<crate::util::log::Log>,
        tree: TreeId,
    ) -> Result<Option<TypeId>, crate::code::symbol::CompletionFailure> {
        // perform arity-based check
        let env = self.env.clone().expect("argument attr env");
        // Java: env.dup(tree) keeps the enclosing scope (ArgumentAttr.java:266);
        // clearing it made method parameters invisible while speculating the
        // member-reference qualifier.
        let local_env = env.dup(Some(tree), env.info.borrow().dup_ctx());
        let (expr, name) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::MemberReference { expr, name, .. } => {
                (*expr, name.clone())
            }
            _ => panic!("ClassCastException: JCMemberReference expected"),
        };
        let expr = expr.expect("member reference qualifier");
        let qualifier_result = attr.member_reference_qualifier_result(
            ctx,
            &local_env,
            tree,
        );
        let deferred = self.deferred.borrow().clone().expect("deferred attr service");
        let mut local_cache = self.with_local_cache_context();
        let expr_tree = {
            let deferred = deferred.borrow();
            deferred
                .attrib_speculative(ctx, syms, attr, expr, &local_env, &qualifier_result)
                .map_err(deferred_failure_to_completion)?
        };
        local_cache.leave(self);
        // JCMemberReference mref2 = new TreeCopier<Void>(attr.make).copy(tree);
        // mref2.expr = exprTree;
        let mref2 = {
            let copier = crate::tree::tree_copier::TreeCopier::new(Rc::clone(&attr.make));
            copier.copy(ctx, Some(tree)).expect("copied member reference")
        };
        if let crate::tree::jc_tree::TreeKind::MemberReference { expr: mref_expr, .. } =
            &mut ctx.tree_mut(mref2).kind
        {
            *mref_expr = Some(expr_tree);
        }
        let lhs_sym = crate::tree::tree_info::symbol(ctx, expr_tree);
        let is_super = lhs_sym
            .map(|sym| ctx.sym(sym).name == attr.names._super)
            .unwrap_or(false);
        local_env.info.borrow_mut().select_super = is_super;
        let expr_type = ctx.tree(expr_tree).ty.expect("qualifier type");
        let rs_rc = attr.rs.get().cloned().expect("resolve service");
        let rs = &*rs_rc;
        let chk_rc = Rc::clone(&attr.chk);
        let chk = Rc::clone(&chk_rc);
        let types_rc = Rc::clone(&attr.types);
        let infer_rc = Rc::clone(&attr.infer);
        let res = rs.get_member_reference(
            ctx,
            syms,
            &types_rc,
            attr,
            &chk,
            &infer_rc,
            ctx.tree(tree).pos,
            &local_env,
            mref2,
            expr_type,
            name.clone(),
        );
        drop(chk);
        drop(rs);
        if !ctx.sym(res).kind.is_resolution_error()
            && let crate::tree::jc_tree::TreeKind::MemberReference { sym, .. } =
                &mut ctx.tree_mut(tree).kind
        {
            *sym = Some(res);
        }
        let overload_kind = {
            let res_type = ctx.sym(res).ty;
            let forall = res_type
                .map(|t| ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::FORALL))
                .unwrap_or(false);
            let varargs = ctx.sym(res).flags_field & crate::code::flags::VARARGS != 0;
            let static_selector = crate::tree::tree_info::is_static_selector(
                ctx,
                Some(expr_tree),
                &attr.names,
            );
            let raw_type = utils_is_raw_type(ctx, &attr.types, expr_type);
            let is_array = ctx.typ(expr_type).has_tag(crate::code::type_tag::TypeTag::ARRAY);
            if ctx.sym(res).kind.is_resolution_target_error() {
                crate::tree::jc_tree::OverloadKind::ERROR
            } else if forall || varargs || (static_selector && raw_type && !is_array) {
                crate::tree::jc_tree::OverloadKind::OVERLOADED
            } else {
                crate::tree::jc_tree::OverloadKind::UNOVERLOADED
            }
        };
        if let crate::tree::jc_tree::TreeKind::MemberReference { overload_kind: ok, .. } =
            &mut ctx.tree_mut(tree).kind
        {
            *ok = Some(overload_kind);
        }
        // return a plain old deferred type for this
        let env_for_deferred = self.env.clone().expect("env");
        let deferred_type = self.new_plain_deferred(ctx, syms, attr, tree, &env_for_deferred);
        self.set_result(ctx, tree, deferred_type);
        Ok(Some(deferred_type))
    }
}

/// Porting rules helper: Type.isRaw() for the member-reference overload-kind check.
fn utils_is_raw_type(
    ctx: &crate::compiler_context::CompilerContext,
    types: &crate::code::types::Types,
    t: TypeId,
) -> bool {
    types.is_raw_type_public(ctx, t)
}

impl ArgumentAttr {
    /// Java: processArg(T that, Function<T, Z> argumentTypeFactory)
    /// (ArgumentAttr.java:252-266) — the speculative pass + factory entry.
    fn process_arg_with_speculation(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        log: &Rc<crate::util::log::Log>,
        tree: TreeId,
        factory: impl FnOnce(TreeId) -> ArgumentTypeData,
    ) -> Result<Option<TypeId>, crate::code::symbol::CompletionFailure> {
        let pos = UniquePos::new(ctx, &*log, tree);
        if let Some(finished) = self.process_arg_cached(ctx, syms, attr, tree, pos.clone())? {
            return Ok(Some(finished));
        }
        let result_info = self.argument_result_info(attr, ctx, log, tree);
        let env = self.env.clone().expect("argument attr env");
        let deferred = self.deferred.borrow().clone().expect("deferred attr service");
        let speculative_tree = {
            let deferred = deferred.borrow();
            deferred
                .tree_copier
                .copy(ctx, Some(tree))
                .expect("copied speculative tree")
        };
        let speculative_tree = {
            let deferred = deferred.borrow();
            deferred
                .attrib_speculative(ctx, syms, attr, speculative_tree, &env, &result_info)
                .map_err(deferred_failure_to_completion)?
        };
        let data = factory(speculative_tree);
        let deferred_type = self.new_argument_deferred(ctx, syms, attr, tree, &env, data.clone());
        self.argument_type_cache
            .borrow_mut()
            .insert(pos, Rc::new(data));
        self.set_result(ctx, tree, deferred_type);
        Ok(Some(deferred_type))
    }

    /// Java: the lambda branch of processArg (ArgumentAttr.java:294-310)
    /// using deferredAttr.attribSpeculativeLambda.
    fn process_arg_with_lambda_speculation(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        log: &Rc<crate::util::log::Log>,
        tree: TreeId,
    ) -> Result<Option<TypeId>, crate::code::symbol::CompletionFailure> {
        let pos = UniquePos::new(ctx, &*log, tree);
        if let Some(finished) = self.process_arg_cached(ctx, syms, attr, tree, pos.clone())? {
            return Ok(Some(finished));
        }
        let result_info = attr.method_attr_info.clone();
        let env = self.env.clone().expect("argument attr env");
        let deferred = self.deferred.borrow().clone().expect("deferred attr service");
        let attr_recover_rc = attr.attr_recover.borrow().clone().expect("attr recover service");
        let rs_rc = attr.rs.get().cloned().expect("resolve service");
        let rs = &*rs_rc;
        let chk_rc = Rc::clone(&attr.chk);
        let chk = Rc::clone(&chk_rc);
        let diags = Rc::clone(&attr.diags);
        let flow_rc = self.flow.borrow().clone().expect("flow service");
        let speculative_lambda = {
            let deferred = deferred.borrow();
            deferred.attrib_speculative_lambda(
                ctx,
                syms,
                attr,
                &attr_recover_rc,
                &rs,
                &chk,
                &diags,
                &flow_rc,
                tree,
                &env,
                &result_info,
            )
            .map_err(deferred_failure_to_completion)?
        };
        let data = ArgumentTypeData {
            kind: ArgumentKind::ExplicitLambda,
            speculative_tree: speculative_lambda,
            speculative_types: Rc::new(RefCell::new(Vec::new())),
            yield_expressions: RefCell::new(None),
            argtypes: RefCell::new(None),
            return_expressions: RefCell::new(None),
        };
        let deferred_type = self.new_argument_deferred(ctx, syms, attr, tree, &env, data.clone());
        self.argument_type_cache
            .borrow_mut()
            .insert(pos, Rc::new(data));
        self.set_result(ctx, tree, deferred_type);
        Ok(Some(deferred_type))
    }

    /// Java: processArg(T that, Supplier<Z> argumentTypeFactory) cached path
    /// (ArgumentAttr.java:246-260). The factory body is inlined by both callers.
    fn process_arg_cached(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        tree: TreeId,
        pos: UniquePos,
    ) -> Result<Option<TypeId>, crate::code::symbol::CompletionFailure> {
        let env = self.env.clone().expect("argument attr env");
        let cached_entry = self.argument_type_cache.borrow().get(&pos).cloned();
        if let Some(cached) = cached_entry {
            let cached = &cached;
            let data = ArgumentTypeData {
                kind: cached.kind,
                speculative_tree: cached.speculative_tree,
                speculative_types: Rc::clone(&cached.speculative_types),
                yield_expressions: RefCell::new(cached.yield_expressions.borrow().clone()),
                argtypes: RefCell::new(cached.argtypes.borrow().clone()),
                return_expressions: RefCell::new(cached.return_expressions.borrow().clone()),
            };
            let deferred_type = self.new_argument_deferred(ctx, syms, attr, tree, &env, data);
            self.set_result(ctx, tree, deferred_type);
            Ok(Some(deferred_type))
        } else {
            Ok(None)
        }
    }

    /// Porting rules helper: plain DeferredType (no ArgumentType payload).
    fn new_plain_deferred(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        tree: TreeId,
        env: &Rc<Env<crate::comp::attr_context::AttrContext>>,
    ) -> TypeId {
        // Port adapter (A3k): the caller's Symtab borrow is forwarded instead of
        // re-borrowing attr.syms (which is already mutably borrowed while the
        // enclosing Attr::attrib_tree runs). Upstream has a single Symtab
        // service, so no re-borrow is involved.
        let types_rc = Rc::clone(&attr.types);
        crate::comp::deferred_attr::deferred_type_new(
            ctx,
            syms,
            &types_rc,
            attr,
            tree,
            env,
        )
    }

    /// Porting rules helper: DeferredType with the ArgumentType payload.
    fn new_argument_deferred(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        tree: TreeId,
        env: &Rc<Env<crate::comp::attr_context::AttrContext>>,
        data: ArgumentTypeData,
    ) -> TypeId {
        // Port adapter (A3k): use the caller's Symtab borrow; see
        // new_plain_deferred.
        let types_rc = Rc::clone(&attr.types);
        let deferred_type = crate::comp::deferred_attr::deferred_type_new(
            ctx,
            syms,
            &types_rc,
            attr,
            tree,
            env,
        );
        if let crate::code::type_::TypeKindData::Deferred { argument, .. } =
            &mut ctx.typ_mut(deferred_type).kind
        {
            *argument = Some(Rc::new(data));
        }
        deferred_type
    }
}

/// Porting rules helper: DeferredAttrFailure -> CompletionFailure (the upstream
/// attribSpeculative throws CompletionFailure).
fn deferred_failure_to_completion(
    failure: crate::comp::deferred_attr::DeferredAttrFailure,
) -> crate::code::symbol::CompletionFailure {
    match failure {
        crate::comp::deferred_attr::DeferredAttrFailure::Completion(failure) => failure,
        other => panic!("unexpected speculative attribution failure: {:?}", other),
    }
}

impl ArgumentAttr {
    /// Java: ArgumentType.complete(ResultInfo, DeferredAttrContext)
    /// (ArgumentAttr.java:355-370).
    ///
    /// # Arguments
    ///   - ctx/syms/attr: explicit services;
    ///   - t/data: the deferred type and its ArgumentType payload;
    ///   - result_info/deferred_attr_context: the check target and mode.
    ///
    /// # Returns
    ///   - the computed type (speculative) or the plain attribution result (CHECK).
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: ArgumentAttr.java:355-370.
    #[allow(clippy::too_many_arguments)]
    pub fn argument_type_complete(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        t: TypeId,
        data: &Rc<ArgumentTypeData>,
        result_info: &ResultInfo,
        deferred_attr_context: &crate::comp::deferred_attr::DeferredAttrContext,
    ) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
        if deferred_attr_context.mode == crate::comp::deferred_attr::AttrMode::SPECULATIVE {
            // Port adapter (A3k): use the caller's Symtab borrow; attr.syms is
            // already mutably borrowed by the enclosing attribution.
            let recovery_type = syms.recovery_type;
            let tt = if result_info.pt == Some(recovery_type) {
                crate::comp::deferred_attr::deferred_type_complete_base(
                    ctx,
                    syms,
                    attr,
                    t,
                    result_info,
                    deferred_attr_context,
                )?
            } else {
                self.overload_check(ctx, syms, attr, data, result_info, deferred_attr_context)?
            };
            let key = result_info.id;
            {
                let data_ref = &data;
                let mut types = data_ref.speculative_types.borrow_mut();
                if let Some(entry) = types.iter_mut().find(|(k, _, _)| *k == key) {
                    entry.2 = tt;
                } else {
                    types.push((key, Rc::new(result_info.clone()), tt));
                }
            }
            Ok(tt)
        } else {
            let env = match &ctx.typ(t).kind {
                crate::code::type_::TypeKindData::Deferred { env, .. } => Rc::clone(env),
                _ => panic!("ClassCastException: DeferredType expected"),
            };
            if !env.info.borrow().attribution_mode.is_speculative() {
                let log = &*attr.log;
                let pos = UniquePos::new(ctx, &log, data.speculative_tree);
                drop(log);
                self.argument_type_cache.borrow_mut().remove(&pos);
            }
            crate::comp::deferred_attr::deferred_type_complete_base(
                ctx,
                syms,
                attr,
                t,
                result_info,
                deferred_attr_context,
            )
        }
    }

    /// Java: ArgumentType.speculativeType(Symbol, MethodResolutionPhase)
    /// (ArgumentAttr.java:372-390).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - data: the ArgumentType payload;
    ///   - t: the deferred type;
    ///   - msym/phase: the resolution round.
    ///
    /// # Returns
    ///   - the recorded speculative type, or noType (Some(no_type_id)) when absent.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: ArgumentAttr.java:372-390.
    pub fn argument_type_speculative_type(
        &self,
        ctx: &mut CompilerContext,
        data: &ArgumentTypeData,
        t: TypeId,
        msym: Option<SymbolId>,
        phase: crate::comp::resolve::MethodResolutionPhase,
    ) -> Option<TypeId> {
        let not_pertinent = match &ctx.typ(t).kind {
            crate::code::type_::TypeKindData::Deferred {
                not_pertinent, ..
            } => not_pertinent.clone(),
            _ => Vec::new(),
        };
        if msym.map(|msym| not_pertinent.contains(&msym)).unwrap_or(false) {
            // Java: super.speculativeType(msym, phase)
            let cache = match &ctx.typ(t).kind {
                crate::code::type_::TypeKindData::Deferred {
                    speculative_cache, ..
                } => Rc::clone(speculative_cache),
                _ => return None,
            };
            let entry = cache.get(msym, phase)?;
            return ctx.tree(entry.speculative_tree).ty;
        }
        let speculative_types = data.speculative_types.borrow();
        for (_key, result_info, ty) in speculative_types.iter() {
            let deferred_attr_context = result_info.check_context.deferred_attr_context();
            if deferred_attr_context.phase == phase
                && deferred_attr_context.msym == msym
            {
                return Some(*ty);
            }
        }
        Some(crate::code::symtab::no_type_id(ctx))
    }

    /// Java: ArgumentType.speculativeTree(DeferredAttrContext)
    /// (ArgumentAttr.java:390-400).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - data: the ArgumentType payload;
    ///   - t: the deferred type;
    ///   - deferred_attr_context: the resolution round.
    ///
    /// # Returns
    ///   - the speculative tree of the recorded round.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: ArgumentAttr.java:390-400.
    pub fn argument_type_speculative_tree(
        &self,
        ctx: &CompilerContext,
        data: &ArgumentTypeData,
        t: TypeId,
        deferred_attr_context: &crate::comp::deferred_attr::DeferredAttrContext,
    ) -> Option<TreeId> {
        let not_pertinent = match &ctx.typ(t).kind {
            crate::code::type_::TypeKindData::Deferred {
                not_pertinent, ..
            } => not_pertinent.clone(),
            _ => Vec::new(),
        };
        if deferred_attr_context
            .msym
            .map(|msym| not_pertinent.contains(&msym))
            .unwrap_or(false)
        {
            // Java: super.speculativeTree(deferredAttrContext)
            let cache = match &ctx.typ(t).kind {
                crate::code::type_::TypeKindData::Deferred {
                    speculative_cache, ..
                } => Rc::clone(speculative_cache),
                _ => return None,
            };
            return cache
                .get(deferred_attr_context.msym, deferred_attr_context.phase)
                .map(|entry| entry.speculative_tree);
        }
        Some(data.speculative_tree)
    }

    /// Java: ArgumentType.overloadCheck(ResultInfo, DeferredAttrContext)
    /// (the six subclass bodies, ArgumentAttr.java:405-735).
    #[allow(clippy::too_many_arguments)]
    fn overload_check(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        data: &Rc<ArgumentTypeData>,
        result_info: &ResultInfo,
        _deferred_attr_context: &crate::comp::deferred_attr::DeferredAttrContext,
    ) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
        let kind = data.kind;
        let speculative_tree = data.speculative_tree;
        let types_rc = Rc::clone(&attr.types);
        match kind {
            ArgumentKind::Parens => {
                // Java: ParensType.overloadCheck: checkSpeculative(speculativeTree.expr, resultInfo)
                let expr = match &ctx.tree(speculative_tree).kind {
                    crate::tree::jc_tree::TreeKind::Parens { expr } => *expr,
                    _ => panic!("ClassCastException: JCParens expected"),
                };
                let expr = expr.expect("parenthesized expression");
                match self.check_speculative(ctx, syms, &types_rc, attr, expr, result_info)? {
                    Some(t) => Ok(t),
                    None => Ok(no_type_of(ctx)),
                }
            }
            ArgumentKind::Conditional => {
                // Java: ConditionalType.overloadCheck (ArgumentAttr.java:430-456)
                let cond_ctx = attr.conditional_context(Rc::clone(&result_info.check_context));
                let local_info = result_info.dup_context(Rc::clone(&cond_ctx));
                let (is_standalone, truepart, falsepart) = match &ctx.tree(speculative_tree).kind {
                    crate::tree::jc_tree::TreeKind::Conditional {
                        truepart, falsepart, poly_kind, ..
                    } => (
                        *poly_kind == crate::tree::jc_tree::PolyKind::STANDALONE,
                        *truepart,
                        *falsepart,
                    ),
                    _ => panic!("ClassCastException: JCConditional expected"),
                };
                if is_standalone {
                    let ty = ctx.tree(speculative_tree).ty.unwrap_or_else(|| no_type_of(ctx));
                    return Ok(local_info.check(ctx, syms, &attr.chk, 0, ty));
                }
                let pt = result_info.pt.unwrap_or_else(|| no_type_of(ctx));
                if ctx.typ(pt).has_tag(crate::code::type_tag::TypeTag::VOID) {
                    // this means we are returning a poly conditional from
                    // void-compatible lambda expression
                    let diag = attr.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                        crate::resources::compiler_properties::fragments::conditional_target_cant_be_void(),
                    ));
                    result_info.check_context.report(ctx, 0, &diag);
                    return Ok(types_rc.create_error_type_original(ctx, Some(pt), syms));
                }
                // poly
                if let Some(truepart) = truepart {
                    self.check_speculative(ctx, syms, &types_rc, attr, truepart, &local_info)?;
                }
                if let Some(falsepart) = falsepart {
                    self.check_speculative(ctx, syms, &types_rc, attr, falsepart, &local_info)?;
                }
                Ok(local_info.pt.unwrap_or_else(|| no_type_of(ctx)))
            }
            ArgumentKind::SwitchExpression => {
                // Java: SwitchExpressionType.overloadCheck (ArgumentAttr.java:480-500)
                let cond_ctx = attr.conditional_context(Rc::clone(&result_info.check_context));
                let local_info = result_info.dup_context(Rc::clone(&cond_ctx));
                let pt = result_info.pt.unwrap_or_else(|| no_type_of(ctx));
                if ctx.typ(pt).has_tag(crate::code::type_tag::TypeTag::VOID) {
                    let diag = attr.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                        crate::resources::compiler_properties::fragments::switch_expression_target_cant_be_void(),
                    ));
                    result_info.check_context.report(ctx, 0, &diag);
                    return Ok(types_rc.create_error_type_original(ctx, Some(pt), syms));
                }
                let yields = self.switch_expression_yields(ctx, data, speculative_tree);
                let mut yield_list = yields;
                for yield_tree in &yield_list {
                let value = match &ctx.tree(yield_tree).kind {
                    crate::tree::jc_tree::TreeKind::Yield { value, .. } => *value,
                    _ => panic!("ClassCastException: JCYield expected"),
                };
                if let Some(value) = value {
                    self.check_speculative(ctx, syms, &types_rc, attr, value, result_info)?;
                }
                }
                Ok(local_info.pt.unwrap_or_else(|| no_type_of(ctx)))
            }
            ArgumentKind::ExplicitLambda => {
                // Java: ExplicitLambdaType.overloadCheck (ArgumentAttr.java:645-675)
                let argtypes = {
                    let mut argtypes = data.argtypes.borrow_mut();
                    if argtypes.is_none() {
                        let params = match &ctx.tree(speculative_tree).kind {
                            crate::tree::jc_tree::TreeKind::Lambda { params, .. } => params.clone(),
                            _ => panic!("ClassCastException: JCLambda expected"),
                        };
                        let mut result: List<TypeId> = List::nil();
                        let mut current = params;
                        while current.non_empty() {
                            let param = *current.head();
                            if let Some(ty) = ctx.tree(param).ty {
                                result = result.append(ty);
                            }
                            current = current.tail();
                        }
                        *argtypes = Some(result);
                    }
                    argtypes.clone().expect("argtypes")
                };
                let target_info = attr.
                    get_target_info(ctx, syms, speculative_tree, result_info, Some(argtypes));
                let target_info = match target_info {
                    Ok(target_info) => target_info,
                    Err(error) => {
                        // Java: resultInfo.checkContext.report(null,
                        // ex.getDiagnostic()); return null;
                        if let Some(diagnostic) = error.get_diagnostic() {
                            result_info.check_context.report(ctx, 0, diagnostic);
                        }
                        return Ok(no_type_of(ctx));
                    }
                };
                let lambda_type = target_info.descriptor;
                let current_target = target_info.target;
                // check compatibility
                let body_result_info =
                    attr.lambda_body_result(ctx, speculative_tree, lambda_type, result_info);
                let body_is_expression = match &ctx.tree(speculative_tree).kind {
                    crate::tree::jc_tree::TreeKind::Lambda { body, .. } => {
                        let body = body.expect("lambda body");
                        ctx.tree(body).tag != crate::tree::jc_tree::Tag::BLOCK
                    }
                    _ => panic!("ClassCastException: JCLambda expected"),
                };
                if body_is_expression {
                    let body = match &ctx.tree(speculative_tree).kind {
                        crate::tree::jc_tree::TreeKind::Lambda { body, .. } => {
                            body.expect("lambda body")
                        }
                        _ => crate::util::assert::error(),
                    };
                    let body_ty = ctx.tree(body).ty.unwrap_or_else(|| no_type_of(ctx));
                    self.check_speculative_type(
                        ctx,
                        syms,
                        &types_rc,
                        attr,
                        body_ty,
                        &body_result_info,
                    )?;
                } else {
                    let returns = self.lambda_return_expressions(ctx, data, speculative_tree);
                    let mut return_list = returns;
                    for ret in &return_list {
                    self.check_return_in_statement_lambda(
                        ctx,
                        syms,
                        attr,
                        &types_rc,
                        speculative_tree,
                        ret,
                        &body_result_info,
                    )?;
                    }
                }
                let check_context = Rc::clone(&result_info.check_context);
                attr.check_lambda_compatible(
                    ctx,
                    syms,
                    speculative_tree,
                    lambda_type,
                    &check_context,
                );
                Ok(current_target)
            }
            ArgumentKind::ResolvedMethod | ArgumentKind::ResolvedConstructor => {
                // Java: ResolvedMemberType.overloadCheck (ArgumentAttr.java:635-649)
                let mtype = match kind {
                    ArgumentKind::ResolvedMethod => match &ctx.tree(speculative_tree).kind {
                        crate::tree::jc_tree::TreeKind::MethodInvocation { meth, .. } => {
                            meth.and_then(|meth| ctx.tree(meth).ty)
                        }
                        _ => panic!("ClassCastException: JCMethodInvocation expected"),
                    },
                    _ => match &ctx.tree(speculative_tree).kind {
                        crate::tree::jc_tree::TreeKind::NewClass {
                            constructor_type, ..
                        } => *constructor_type,
                        _ => panic!("ClassCastException: JCNewClass expected"),
                    },
                };
                if kind == ArgumentKind::ResolvedConstructor
                    && mtype
                        .map(|m| {
                            let tag = ctx.typ(m).get_tag();
                            tag == crate::code::type_tag::TypeTag::ERROR
                                || tag == crate::code::type_tag::TypeTag::NONE
                        })
                        .unwrap_or(true)
                {
                    // The speculative diamond inference against this candidate's
                    // target already failed; the error type is not evidence that
                    // the candidate is applicable.
                    crate::comp::resolve::throw_inapplicable_method_exception(None);
                }
                let local_info = match kind {
                    ArgumentKind::ResolvedMethod => {
                        // Java: ResolvedMethodType.resultInfo(resultInfo) returns
                        // the same ResultInfo object (ArgumentAttr.java:675-678),
                        // so the speculativeTypes key is resultInfo's identity.
                        let mut info = result_info.clone();
                        info.id = result_info.id;
                        info
                    }
                    _ => {
                        let clazz = match &ctx.tree(speculative_tree).kind {
                            crate::tree::jc_tree::TreeKind::NewClass { clazz, .. } => {
                                clazz.expect("new class clazz")
                            }
                            _ => panic!("ClassCastException: JCNewClass expected"),
                        };
                        let clazz_type = ctx.tree(clazz).ty.expect("new class clazz type");
                        let tsym = ctx.typ(clazz_type).tsym.expect("clazz type symbol");
                        // Java: ResolvedConstructorType.resultInfo(resultInfo)
                        // (ArgumentAttr.java:704-707) returns
                        // resultInfo.dup(attr.diamondContext(...)); the target
                        // pt is kept unchanged (including type/inference vars).
                        result_info.dup_context(attr.diamond_context(
                            clazz,
                            tsym,
                            Rc::clone(&result_info.check_context),
                        ))
                    }
                };
                let t = match mtype {
                    Some(mtype)
                        if {
                            let tag = ctx.typ(mtype).get_tag();
                            tag == crate::code::type_tag::TypeTag::METHOD
                                || tag == crate::code::type_tag::TypeTag::FORALL
                        } && ctx.typ(mtype).is_partial() =>
                    {
                        // Java: ((PartiallyInferredMethodType) mtype).check(localInfo)
                        let chk_rc = Rc::clone(&attr.chk);
                        let chk = Rc::clone(&chk_rc);
                        match attr.infer.partially_inferred_method_type_check(
                            ctx,
                            syms,
                            &types_rc,
                            &chk,
                            mtype,
                            &local_info,
                        ) {
                            Ok(t) => t,
                            Err(exception) => match kind {
                                // A constructor candidate that fails its
                                // diamond inference is inapplicable; Resolve
                                // tries the next overload.
                                ArgumentKind::ResolvedConstructor => {
                                    crate::comp::resolve::throw_inapplicable_method_exception(
                                        exception.get_diagnostic().as_ref(),
                                    )
                                }
                                // Method inference failures keep their
                                // upstream meaning.
                                _ => crate::comp::resolve::throw_inference_exception(exception),
                            }
                        }
                    }
                    _ => {
                        // Java: localInfo.check(tree.pos(), speculativeTree.type)
                        //
                        // For a method candidate localInfo is the Resolve.MethodResultInfo
                        // itself (ArgumentAttr.java:675-678), and its check() replaces the
                        // found type with U(found) and re-captures the upper bound before
                        // the compatibility test (Resolve.java:1077-1098). The port's
                        // ResultInfo is a single structure, so the same normalisation has
                        // to be applied here; without it a captured variable such as
                        // "capture of ? extends Class<X>" reaches compatible() unexpanded
                        // and is rejected even though its upper bound satisfies the formal.
                        let chk_rc = Rc::clone(&attr.chk);
                        let chk = Rc::clone(&chk_rc);
                        let speculative_type = ctx
                            .tree(speculative_tree)
                            .ty
                            .unwrap_or_else(|| no_type_of(ctx));
                        let normalized = if kind == ArgumentKind::ResolvedMethod {
                            let types_rc2 = Rc::clone(&types_rc);
                            let u_result = match local_info.pt {
                                Some(pt) if speculative_type != pt => {
                                    types_rc2.cvar_upper_bound_of(ctx, speculative_type)
                                }
                                _ => speculative_type,
                            };
                            let inference_context = local_info.check_context.inference_context();
                            inference_context.cached_capture(
                                ctx,
                                syms,
                                &types_rc2,
                                speculative_tree,
                                u_result,
                                true,
                            )
                        } else {
                            speculative_type
                        };
                        local_info.check(
                            ctx,
                            syms,
                            &chk,
                            ctx.tree(speculative_tree).pos,
                            normalized,
                        )
                    }
                };
                if kind == ArgumentKind::ResolvedConstructor
                    && ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::ERROR)
                {
                    crate::comp::resolve::throw_inapplicable_method_exception(None);
                }
                if std::env::var_os("DSH_DEBUG_OVERLOAD").is_some()
                    && kind == ArgumentKind::ResolvedConstructor
                {
                    let mtype_tag = mtype.map(|m| ctx.typ(m).get_tag());
                    let target_display = local_info
                        .pt
                        .map(|pt| crate::comp::attr::type_display_of(ctx, pt));
                    let t_display = crate::comp::attr::type_display_of(ctx, t);
                    eprintln!(
                        "[DIAMOND-CHECK] target={target_display:?} t={t_display} mtype={mtype_tag:?}",
                    );
                }
                let local_info = Rc::new(local_info);
                let key = local_info.id;
                let data_ref = &data;
                data_ref.speculative_types.borrow_mut()
                    .push((key, local_info, t));
                Ok(t)
            }
        }
    }

    /// Java: ExplicitLambdaType.checkReturnInStatementLambda(JCReturn, ResultInfo)
    #[allow(clippy::too_many_arguments)]
    fn check_return_in_statement_lambda(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut Attr,
        types: &crate::code::types::Types,
        speculative_lambda: TreeId,
        ret: TreeId,
        result_info: &ResultInfo,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let pt = result_info.pt.unwrap_or_else(|| no_type_of(ctx));
        let expr = match &ctx.tree(ret).kind {
            crate::tree::jc_tree::TreeKind::Return { expr } => *expr,
            _ => panic!("ClassCastException: JCReturn expected"),
        };
        if ctx.typ(pt).has_tag(crate::code::type_tag::TypeTag::VOID) && expr.is_some() {
            // fail - if the function type's result is void, the lambda body must
            // be a void-compatible block.
            let diag = attr.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::unexpected_ret_val(),
            ));
            result_info
                .check_context
                .report(ctx, ctx.tree(speculative_lambda).pos, &diag);
        } else if !ctx.typ(pt).has_tag(crate::code::type_tag::TypeTag::VOID) {
            if expr.is_none() {
                // fail - if the function type's result is non-void, the lambda
                // body must be a value-compatible block.
                let arg = crate::resources::diagnostic_contract::TypeArg::placeholder(
                    crate::comp::attr::type_display_of(ctx, pt),
                );
                let diag = attr.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                    crate::resources::compiler_properties::fragments::missing_ret_val(arg),
                ));
                result_info
                    .check_context
                    .report(ctx, ctx.tree(speculative_lambda).pos, &diag);
            }
            if let Some(expr) = expr {
                self.check_speculative(ctx, syms, types, attr, expr, result_info)?;
            }
        }
        Ok(())
    }

    /// Java: ExplicitLambdaType.returnExpressions() lazily populated list.
    fn lambda_return_expressions(
        &mut self,
        ctx: &mut CompilerContext,
        data: &Rc<ArgumentTypeData>,
        speculative_lambda: TreeId,
    ) -> List<TreeId> {
        if data.return_expressions.borrow().is_none() {
            let body = match &ctx.tree(speculative_lambda).kind {
                crate::tree::jc_tree::TreeKind::Lambda { body, .. } => {
                    body.expect("lambda body")
                }
                _ => panic!("ClassCastException: JCLambda expected"),
            };
            let mut collector = LambdaReturnCollector { buf: Vec::new() };
            crate::tree::tree_scanner::TreeScannerVisitor::scan(&mut collector, ctx, Some(body));
            let mut list: List<TreeId> = List::nil();
            for tree in collector.buf {
                list = list.append(tree);
            }
            *data.return_expressions.borrow_mut() = Some(list);
        }
        data.return_expressions
            .borrow()
            .clone()
            .expect("return expressions")
    }

    /// Java: SwitchExpressionType.yieldExpressions() lazily populated list.
    fn switch_expression_yields(
        &mut self,
        ctx: &mut CompilerContext,
        data: &Rc<ArgumentTypeData>,
        speculative_switch: TreeId,
    ) -> List<TreeId> {
        if data.yield_expressions.borrow().is_none() {
            let cases = match &ctx.tree(speculative_switch).kind {
                crate::tree::jc_tree::TreeKind::SwitchExpression { cases, .. } => cases.clone(),
                _ => panic!("ClassCastException: JCSwitchExpression expected"),
            };
            let mut collector = SwitchYieldCollector {
                speculative_switch,
                buf: Vec::new(),
            };
            let mut current = cases;
            while current.non_empty() {
                crate::tree::tree_scanner::TreeScannerVisitor::scan(
                    &mut collector,
                    ctx,
                    Some(*current.head()),
                );
                current = current.tail();
            }
            let mut list: List<TreeId> = List::nil();
            for tree in collector.buf {
                list = list.append(tree);
            }
            *data.yield_expressions.borrow_mut() = Some(list);
        }
        data.yield_expressions
            .borrow()
            .clone()
            .expect("yield expressions")
    }
}

/// Porting rules helper: noType.
fn no_type_of(ctx: &mut CompilerContext) -> TypeId {
    crate::code::symtab::no_type_id(ctx)
}

/// Java: LambdaReturnScanner (DeferredAttr.java:1080-1110): collects the
/// JCReturn nodes of a lambda body, skipping nested lambdas/classes.
struct LambdaReturnCollector {
    buf: Vec<TreeId>,
}

impl crate::tree::tree_scanner::TreeScannerVisitor for LambdaReturnCollector {
    fn scan(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) {
        if let Some(tree) = tree
            && self.allowed(ctx.tree(tree).tag)
        {
            crate::tree::jc_tree::accept(ctx, tree, self);
        }
    }
}

impl LambdaReturnCollector {
    fn allowed(&self, tag: crate::tree::jc_tree::Tag) -> bool {
        use crate::tree::jc_tree::Tag;
        // Java: LambdaReturnScanner overrides visitLambda/visitClassDef to not
        // descend; everything else is navigated.
        tag != Tag::LAMBDA && tag != Tag::CLASSDEF
    }
}

crate::tree::tree_scanner::tree_scanner_visitor_delegate!(LambdaReturnCollector);

/// Java: SwitchExpressionScanner (DeferredAttr.java:1140-1155): a FilterScanner
/// that only descends into the allowed target-type dependent nodes.
struct SwitchYieldCollector {
    speculative_switch: TreeId,
    buf: Vec<TreeId>,
}

impl crate::tree::tree_scanner::TreeScannerVisitor for SwitchYieldCollector {
    fn scan(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) {
        if let Some(tree) = tree {
            let tag = ctx.tree(tree).tag;
            if self.allowed(tag) {
                if tag == crate::tree::jc_tree::Tag::YIELD
                    && let crate::tree::jc_tree::TreeKind::Yield { target, .. } =
                        &ctx.tree(tree).kind
                    && *target == Some(self.speculative_switch)
                {
                    self.buf.push(tree);
                }
                crate::tree::jc_tree::accept(ctx, tree, self);
            }
        }
    }
}

impl SwitchYieldCollector {
    fn allowed(&self, tag: crate::tree::jc_tree::Tag) -> bool {
        use crate::tree::jc_tree::Tag;
        matches!(
            tag,
            Tag::BLOCK
                | Tag::CASE
                | Tag::CATCH
                | Tag::DOLOOP
                | Tag::FOREACHLOOP
                | Tag::FORLOOP
                | Tag::IF
                | Tag::SYNCHRONIZED
                | Tag::SWITCH
                | Tag::SWITCH_EXPRESSION
                | Tag::TRY
                | Tag::WHILELOOP
                | Tag::YIELD
        )
    }
}

crate::tree::tree_scanner::tree_scanner_visitor_delegate!(SwitchYieldCollector);
