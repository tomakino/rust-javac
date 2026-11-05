//
// Registration:
// - the Analyzer class + the analyzerModes set + the -XDfind option decoding;
// - the five StatementAnalyzer subclasses (DiamondInitializer, LambdaAnalyzer,
// RedundantTypeArgAnalyzer, RedundantLocalVarTypeAnalyzer,
// RedundantLocalVarTypeAnalyzerForEach);
// - the StatementScanner/RewritingContext/deferred analysis helpers;
// - AnalyzerCopier/TreeRewriter (expressed as TreeCopyHook implementations, adapter);
// - the ArgumentAttr.LocalCacheContext of doAnalysis (outer context + the
//   attribSpeculative argument, Analyzer.java:555/571).
// Adapter: Java anonymous inner classes map to named structs/enums.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::code::flags;
use crate::code::kinds::Kind;
use crate::code::source::{Feature, Source};
use crate::code::types::Types;
use crate::comp::argument_attr::ArgumentAttr;
use crate::comp::attr::Attr;
use crate::comp::attr_context::AttrContext;
use crate::comp::deferred_attr::{AttributionMode, DeferredAttrService};
use crate::comp::env::Env;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId};
use crate::tree::jc_tree::{ParameterKind, Tag, TreeKind};
use crate::tree::tree_copier::{TreeCopier, TreeCopyHook};
use crate::tree::tree_maker::TreeMaker;
use crate::tree::tree_scanner::{TreeScannerVisitor, tree_scanner_visitor_delegate};
use crate::util::list::List;
use crate::util::log::{DiagnosticHandler, Log};
use crate::util::options::Options;

/// Java: Analyzer.AnalyzerMode (Analyzer.java:143-170).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalyzerMode {
    /// Java: DIAMOND("diamond", Feature.DIAMOND)
    DIAMOND,
    /// Java: LAMBDA("lambda", Feature.LAMBDA)
    LAMBDA,
    /// Java: METHOD("method", Feature.GRAPH_INFERENCE)
    METHOD,
    /// Java: LOCAL("local", Feature.LOCAL_VARIABLE_TYPE_INFERENCE)
    LOCAL,
}

impl AnalyzerMode {
    /// Java: AnalyzerMode.opt
    pub fn opt(self) -> &'static str {
        match self {
            AnalyzerMode::DIAMOND => "diamond",
            AnalyzerMode::LAMBDA => "lambda",
            AnalyzerMode::METHOD => "method",
            AnalyzerMode::LOCAL => "local",
        }
    }

    /// Java: AnalyzerMode.feature
    pub fn feature(self) -> Feature {
        match self {
            AnalyzerMode::DIAMOND => Feature::DIAMOND,
            AnalyzerMode::LAMBDA => Feature::LAMBDA,
            AnalyzerMode::METHOD => Feature::GRAPH_INFERENCE,
            AnalyzerMode::LOCAL => Feature::LOCAL_VARIABLE_TYPE_INFERENCE,
        }
    }

    /// Java: AnalyzerMode.getAnalyzerModes(String opt, Source source)
    /// (Analyzer.java:172-195).
    ///
    /// # Arguments
    ///   - opt: the -XDfind option value (None when absent);
    ///   - source: the source level.
    ///
    /// # Returns
    ///   - the enabled analyzer modes.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Analyzer.java:172-195.
    pub fn get_analyzer_modes(opt: Option<&'static str>, source: Source) -> AnalyzerModes {
        let mut res = AnalyzerModes {
            diamond: false,
            lambda: false,
            method: false,
            local: false,
        };
        let Some(opt) = opt else {
            return res;
        };
        let modes: Vec<&'static str> = {
            let mut v = Vec::new();
            for m in opt.split(',') {
                match m {
                    "diamond" => v.push("diamond"),
                    "lambda" => v.push("lambda"),
                    "method" => v.push("method"),
                    "local" => v.push("local"),
                    "all" => v.push("all"),
                    _ => {}
                }
            }
            v
        };
        if modes.contains(&"all") {
            res = AnalyzerModes {
                diamond: true,
                lambda: true,
                method: true,
                local: true,
            };
        }
        for mode in [
            AnalyzerMode::DIAMOND,
            AnalyzerMode::LAMBDA,
            AnalyzerMode::METHOD,
            AnalyzerMode::LOCAL,
        ] {
            let name = mode.opt();
            let disabled_name = format!("-{}", name);
            if modes.contains(&disabled_name.as_str()) || !mode.feature().allowed_in_source(source)
            {
                res.set(mode, false);
            } else if modes.contains(&name) {
                res.set(mode, true);
            }
        }
        res
    }
}

/// Java: EnumSet<AnalyzerMode> analyzerModes (Adapter: a boolean set).
#[derive(Clone, Copy, Debug, Default)]
pub struct AnalyzerModes {
    /// Java: AnalyzerMode.DIAMOND enabled
    pub diamond: bool,
    /// Java: AnalyzerMode.LAMBDA enabled
    pub lambda: bool,
    /// Java: AnalyzerMode.METHOD enabled
    pub method: bool,
    /// Java: AnalyzerMode.LOCAL enabled
    pub local: bool,
}

impl AnalyzerModes {
    /// Java: EnumSet.isEmpty()
    pub fn is_empty(&self) -> bool {
        !self.diamond && !self.lambda && !self.method && !self.local
    }

    /// Java: EnumSet.contains(mode)
    pub fn contains(&self, mode: AnalyzerMode) -> bool {
        match mode {
            AnalyzerMode::DIAMOND => self.diamond,
            AnalyzerMode::LAMBDA => self.lambda,
            AnalyzerMode::METHOD => self.method,
            AnalyzerMode::LOCAL => self.local,
        }
    }

    /// Java: EnumSet.add/remove
    pub fn set(&mut self, mode: AnalyzerMode, value: bool) {
        match mode {
            AnalyzerMode::DIAMOND => self.diamond = value,
            AnalyzerMode::LAMBDA => self.lambda = value,
            AnalyzerMode::METHOD => self.method = value,
            AnalyzerMode::LOCAL => self.local = value,
        }
    }
}

/// Adapter: the five upstream StatementAnalyzer subclasses as one enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalyzerKind {
    /// Java: Analyzer.DiamondInitializer
    DiamondInitializer,
    /// Java: Analyzer.LambdaAnalyzer
    LambdaAnalyzer,
    /// Java: Analyzer.RedundantTypeArgAnalyzer
    RedundantTypeArgAnalyzer,
    /// Java: Analyzer.RedundantLocalVarTypeAnalyzer
    RedundantLocalVarTypeAnalyzer,
    /// Java: Analyzer.RedundantLocalVarTypeAnalyzerForEach
    RedundantLocalVarTypeAnalyzerForEach,
}

impl AnalyzerKind {
    /// Java: StatementAnalyzer.mode
    pub fn mode(self) -> AnalyzerMode {
        match self {
            AnalyzerKind::DiamondInitializer => AnalyzerMode::DIAMOND,
            AnalyzerKind::LambdaAnalyzer => AnalyzerMode::LAMBDA,
            AnalyzerKind::RedundantTypeArgAnalyzer => AnalyzerMode::METHOD,
            AnalyzerKind::RedundantLocalVarTypeAnalyzer => AnalyzerMode::LOCAL,
            AnalyzerKind::RedundantLocalVarTypeAnalyzerForEach => AnalyzerMode::LOCAL,
        }
    }

    /// Java: StatementAnalyzer.tag
    pub fn tag(self) -> Tag {
        match self {
            AnalyzerKind::DiamondInitializer => Tag::NEWCLASS,
            AnalyzerKind::LambdaAnalyzer => Tag::NEWCLASS,
            AnalyzerKind::RedundantTypeArgAnalyzer => Tag::APPLY,
            AnalyzerKind::RedundantLocalVarTypeAnalyzer => Tag::VARDEF,
            AnalyzerKind::RedundantLocalVarTypeAnalyzerForEach => Tag::FOREACHLOOP,
        }
    }
}

/// Java: Analyzer.RewritingContext (Analyzer.java:640-740).
pub struct RewritingContext {
    /// Java: the whole tree being analyzed
    pub original_tree: TreeId,
    /// Java: a subtree, old tree, that will be rewritten
    pub old_tree: TreeId,
    /// Java: the replacement for the old tree
    pub replacement: TreeId,
    /// Java: did the compiler find any error (the anonymous DiagnosticHandler flag)
    pub erroneous: Rc<Cell<bool>>,
    /// Java: the env
    pub env: Rc<Env<AttrContext>>,
    /// Java: the corresponding analyzer
    pub kind: AnalyzerKind,
}

/// Java: Analyzer.AnalyzerCopier.visitLambdaExpression/visitNewClass post-fixes
/// (Adapter: TreeCopier subclass overrides expressed as a copy hook).
pub struct AnalyzerCopierHook;

impl TreeCopyHook for AnalyzerCopierHook {
    fn before_copy(&self, _ctx: &mut CompilerContext, _old: TreeId) -> Option<TreeId> {
        None
    }

    fn after_copy(&self, ctx: &mut CompilerContext, old: TreeId, new: TreeId) -> TreeId {
        analyzer_copier_fix(ctx, old, new)
    }
}

/// Java: Analyzer.TreeRewriter (Analyzer.java:772-800) — the copy hook
/// replaces the matched tree and fixes the wrapped variable modifiers.
pub struct TreeRewriterHook {
    /// Java: JCTree rewriting.oldTree
    pub old_tree: TreeId,
    /// Java: JCTree rewriting.replacement
    pub replacement: TreeId,
    /// Java: JCTree wrappedTree
    pub wrapped_tree: Option<TreeId>,
}

impl TreeCopyHook for TreeRewriterHook {
    fn before_copy(&self, _ctx: &mut CompilerContext, old: TreeId) -> Option<TreeId> {
        if old == self.old_tree {
            return Some(self.replacement);
        }
        None
    }

    #[allow(clippy::collapsible_if, clippy::collapsible_match)]
    fn after_copy(&self, ctx: &mut CompilerContext, old: TreeId, new: TreeId) -> TreeId {
        let new = analyzer_copier_fix(ctx, old, new);
        if self.wrapped_tree == Some(old) {
            // Java: ((JCVariableDecl) result).mods.flags &= Flags.FINAL;
            if let TreeKind::VariableDecl { mods, .. } = &ctx.tree(new).kind {
                if let Some(mods) = *mods {
                    if let TreeKind::Modifiers { flags: slot, .. } = &mut ctx.tree_mut(mods).kind {
                        *slot &= flags::FINAL as i64;
                    }
                }
            }
        }
        new
    }
}

/// Java: AnalyzerCopier.visitLambdaExpression/visitNewClass (adapter helper).
#[allow(clippy::collapsible_if, clippy::collapsible_match)]
fn analyzer_copier_fix(ctx: &mut CompilerContext, old: TreeId, new: TreeId) -> TreeId {
    if ctx.tree(old).tag == Tag::LAMBDA && ctx.tree(new).tag == Tag::LAMBDA {
        let old_implicit = matches!(
            &ctx.tree(old).kind,
            TreeKind::Lambda {
                param_kind: ParameterKind::IMPLICIT,
                ..
            }
        );
        if old_implicit {
            let params = match &ctx.tree(new).kind {
                TreeKind::Lambda { params, .. } => params.clone(),
                _ => List::nil(),
            };
            if let TreeKind::Lambda {
                param_kind,
                params: slot,
                ..
            } = &mut ctx.tree_mut(new).kind
            {
                *param_kind = ParameterKind::IMPLICIT;
                *slot = params.clone();
            }
            let mut current = params;
            while current.non_empty() {
                let param = *current.head();
                if let TreeKind::VariableDecl { vartype, .. } = &mut ctx.tree_mut(param).kind {
                    *vartype = None;
                }
                current = current.tail();
            }
        }
    }
    if ctx.tree(old).tag == Tag::NEWCLASS && ctx.tree(new).tag == Tag::NEWCLASS {
        let old_args = match &ctx.tree(old).kind {
            TreeKind::NewClass { args, .. } => args.clone(),
            _ => List::nil(),
        };
        let first_is_nullchk =
            old_args.non_empty() && ctx.tree(*old_args.head()).tag == Tag::NULLCHK;
        if first_is_nullchk {
            let first_arg = match &ctx.tree(*old_args.head()).kind {
                TreeKind::Unary { arg, .. } => arg.expect("NULLCHK arg"),
                _ => crate::util::assert::error(),
            };
            let rest = old_args.tail();
            if let TreeKind::NewClass { encl, args, .. } = &mut ctx.tree_mut(new).kind {
                *encl = Some(first_arg);
                *args = rest;
            }
        }
    }
    new
}

/// Java: Analyzer.DeferredAnalysisHelper (Adapter: the two anonymous helper
/// implementations become this enum).
pub enum DeferredAnalysisHelper {
    /// Java: Analyzer.flushDeferredHelper
    Flush,
    /// Java: Analyzer.queueDeferredHelper
    Queue {
        /// Java: Map<ClassSymbol, Queue<RewritingContext>> Q
        queue: HashMap<SymbolId, Vec<RewritingContext>>,
    },
}

/// Java: public class Analyzer (Analyzer.java:102-820).
pub struct Analyzer {
    /// Java: final Types types
    pub types: Rc<Types>,
    /// Java: final Log log
    pub log: Rc<Log>,
    /// Java: final TreeMaker make
    pub make: Rc<TreeMaker>,
    /// Java: final Symtab (injected, ADR 0001)
    pub syms: Rc<crate::code::symtab::Symtab>,
    /// Java: final Attr attr (adapter cell: Attr and Analyzer are mutually recursive)
    pub attr: Rc<RefCell<Option<Rc<RefCell<Attr>>>>>,
    /// Java: final DeferredAttr deferredAttr (adapter cell)
    pub deferred_attr: Rc<RefCell<Option<Rc<RefCell<DeferredAttrService>>>>>,
    /// Java: final ArgumentAttr argumentAttr (adapter cell)
    pub argument_attr: Rc<RefCell<Option<Rc<RefCell<ArgumentAttr>>>>>,
    /// Java: final AnalyzerCopier copier
    pub copier: Rc<TreeCopier>,
    /// Java: private final boolean allowDiamondWithAnonymousClassCreation
    pub allow_diamond_with_anonymous_class_creation: bool,
    /// Java: final EnumSet<AnalyzerMode> analyzerModes
    pub analyzer_modes: AnalyzerModes,
    /// Java: DeferredAnalysisHelper deferredAnalysisHelper = queueDeferredHelper
    pub deferred_analysis_helper: DeferredAnalysisHelper,
}

impl Analyzer {
    /// Java: public static Analyzer instance(Context) / protected Analyzer(Context)
    /// (Analyzer.java:120-140).
    ///
    /// # Arguments
    ///   - types/log/make/syms: the shared services;
    ///   - attr: the adapter cell for the mutually recursive Attr service;
    ///   - options/source: the -XDfind decoding inputs.
    ///
    /// The deferred/argument service cells start empty and are populated by
    /// the wiring after those services are constructed.
    ///
    /// # Returns
    ///   - the analyzer service.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Analyzer.java:120-140.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        types: Rc<Types>,
        log: Rc<Log>,
        make: Rc<TreeMaker>,
        syms: Rc<crate::code::symtab::Symtab>,
        attr: Rc<RefCell<Option<Rc<RefCell<Attr>>>>>,
        options: &Options,
        source: Source,
    ) -> Analyzer {
        let allow = Feature::DIAMOND_WITH_ANONYMOUS_CLASS_CREATION.allowed_in_source(source);
        let find_opt = options.get("find");
        let find_opt: Option<&'static str> = match find_opt {
            None => None,
            Some(value) => {
                let value = Box::leak(value.into_boxed_str());
                Some(&*value)
            }
        };
        let modes = AnalyzerMode::get_analyzer_modes(find_opt, source);
        let copier = Rc::new(TreeCopier::with_hook(
            Rc::clone(&make),
            Rc::new(AnalyzerCopierHook),
        ));
        Analyzer {
            types,
            log,
            make,
            syms,
            attr,
            deferred_attr: Rc::new(RefCell::new(None)),
            argument_attr: Rc::new(RefCell::new(None)),
            copier,
            allow_diamond_with_anonymous_class_creation: allow,
            analyzer_modes: modes,
            deferred_analysis_helper: DeferredAnalysisHelper::Queue {
                queue: HashMap::new(),
            },
        }
    }

    fn attr_service(&self) -> Rc<RefCell<Attr>> {
        self.attr
            .borrow()
            .clone()
            .expect("AssertionError: attr service must be installed")
    }

    fn deferred_service(&self) -> Rc<RefCell<DeferredAttrService>> {
        self.deferred_attr
            .borrow()
            .clone()
            .expect("AssertionError: deferredAttr service must be installed")
    }

    /// Java: Env<AttrContext> copyEnvIfNeeded(JCTree tree, Env<AttrContext> env)
    /// (Analyzer.java:456-472).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - tree: the statement tree;
    ///   - env: the current environment.
    ///
    /// # Returns
    ///   - the copied analysis env, or None when no analysis is needed.
    ///
    /// # Panics
    ///   - the attr service must be installed.
    ///
    /// Upstream: Analyzer.java:456-472.
    pub fn copy_env_if_needed(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) -> Option<Rc<Env<AttrContext>>> {
        if !self.analyzer_modes.is_empty()
            && !env.info.borrow().attribution_mode.is_speculative()
            && crate::tree::tree_info::is_statement(ctx, tree)
            && ctx.tree(tree).tag != Tag::LABELLED
        {
            let scope = env.info.borrow().scope.expect("env scope");
            let owner = ctx.scope(scope).owner.expect("scope owner");
            let new_scope = ctx.scope_dup_unshared(scope, Some(owner));
            let info = env.info.borrow().dup(Some(new_scope));
            let analyze_env = env.dup(env.tree, info);
            let return_result = analyze_env.info.borrow().return_result.clone();
            let new_return_result = match return_result {
                None => None,
                Some(result_info) => {
                    // The caller may already hold the Attr RefCell mutably;
                    // reuse the current ResultInfo's handler instead of
                    // borrowing the Attr service (Context.get(Attr.class)).
                    Some(crate::comp::attr::ResultInfo::new(
                        result_info.pkind,
                        result_info.pt,
                        Rc::clone(&result_info.check_context),
                    ))
                }
            };
            analyze_env.info.borrow_mut().return_result = new_return_result;
            Some(analyze_env)
        } else {
            None
        }
    }

    /// Java: void analyzeIfNeeded(JCTree tree, Env<AttrContext> env)
    /// (Analyzer.java:474-479).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - tree: the statement;
    ///   - env: the analysis env (None when no analysis is needed).
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Analyzer.java:474-479.
    pub fn analyze_if_needed(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        tree: TreeId,
        env: Option<&Rc<Env<AttrContext>>>,
    ) {
        if let Some(env) = env {
            self.analyze(ctx, syms, tree, env);
        }
    }

    /// analysis entry for Attr.attribStat, where the caller already holds the
    /// Attr object mutably (Java passes `this` directly).
    pub fn analyze_if_needed_with_attr(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        tree: TreeId,
        env: Option<&Rc<Env<AttrContext>>>,
        attr: &mut Attr,
    ) {
        let Some(env) = env else {
            return;
        };
        let rewritings = {
            let mut scanner = StatementScanner::new_with_attr(self, ctx, syms, tree, env, attr);
            scanner.scan_statement(ctx);
            std::mem::take(&mut scanner.rewritings)
        };
        if !rewritings.is_empty() {
            for rewriting in rewritings {
                self.queue(ctx, rewriting);
            }
        }
    }

    /// Java: protected void analyze(JCStatement statement, Env<AttrContext> env)
    /// (Analyzer.java:481-492).
    fn analyze(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        statement: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) {
        let rewritings = {
            let mut scanner = StatementScanner::new(self, ctx, syms, statement, env);
            scanner.scan_statement(ctx);
            std::mem::take(&mut scanner.rewritings)
        };
        if !rewritings.is_empty() {
            for rewriting in rewritings {
                self.queue(ctx, rewriting);
            }
        }
    }

    /// Java: void flush(Env<AttrContext> flushEnv) (Analyzer.java:585-587).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - flush_env: the environment of the class being flushed.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - the queue entries are analyzed with the registered services.
    ///
    /// Upstream: Analyzer.java:585-587.
    pub fn flush(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        flush_env: &Rc<Env<AttrContext>>,
    ) {
        let outermost = self.outermost_of_env(ctx, flush_env);
        let queued = match &mut self.deferred_analysis_helper {
            DeferredAnalysisHelper::Flush => None,
            DeferredAnalysisHelper::Queue { queue } => queue.remove(&outermost),
        };
        let Some(mut queued) = queued else {
            return;
        };
        let prev_helper = std::mem::replace(
            &mut self.deferred_analysis_helper,
            DeferredAnalysisHelper::Flush,
        );
        while !queued.is_empty() {
            let rewriting = queued.remove(0);
            self.do_analysis(ctx, syms, rewriting);
        }
        self.deferred_analysis_helper = prev_helper;
    }

    /// Java: ClassSymbol outermostClass (Symbol.java) — the adapter helper: climb
    /// the enclosing class chain from the env scope owner.
    fn outermost_of_env(&self, ctx: &CompilerContext, env: &Rc<Env<AttrContext>>) -> SymbolId {
        let scope = env.info.borrow().scope.expect("env scope");
        let mut current = ctx.scope(scope).owner.expect("scope owner");
        while let Some(owner) = ctx.sym(current).owner {
            if ctx.sym(owner).kind == Kind::TYP {
                current = owner;
            } else {
                break;
            }
        }
        current
    }

    /// Java: void queue(RewritingContext rewriting) (Analyzer.java:530-535).
    fn queue(&mut self, ctx: &mut CompilerContext, rewriting: RewritingContext) {
        let outermost = self.outermost_of_env(ctx, &rewriting.env);
        match &mut self.deferred_analysis_helper {
            DeferredAnalysisHelper::Flush => {}
            DeferredAnalysisHelper::Queue { queue } => {
                queue.entry(outermost).or_default().push(rewriting);
            }
        }
    }

    /// Java: void doAnalysis(RewritingContext rewriting) (Analyzer.java:553-583).
    fn do_analysis(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        rewriting: RewritingContext,
    ) {
        // Java: LocalCacheContext localCacheContext =
        //     argumentAttr.withLocalCacheContext(); (Analyzer.java:555);
        // the finally leave is modelled by the Drop guard below.
        let argument_attr = self.argument_attr.borrow().clone();
        struct CacheLeave {
            cache: Option<crate::comp::argument_attr::LocalCacheContext>,
            argument_attr: Option<Rc<RefCell<ArgumentAttr>>>,
        }
        impl Drop for CacheLeave {
            fn drop(&mut self) {
                if let (Some(cache), Some(argument_attr)) =
                    (self.cache.as_mut(), &self.argument_attr)
                {
                    cache.leave(&argument_attr.borrow());
                }
            }
        }
        let _outer_cache = CacheLeave {
            cache: argument_attr
                .as_ref()
                .map(|argument_attr| argument_attr.borrow().with_local_cache_context()),
            argument_attr: argument_attr.clone(),
        };
        let prev_source = {
            let toplevel = rewriting.env.toplevel.expect("env toplevel");
            let sourcefile = match &ctx.tree(toplevel).kind {
                TreeKind::CompilationUnit { sourcefile, .. } => sourcefile.clone(),
                _ => None,
            };
            crate::util::abstract_log::AbstractLog::use_source(&*self.log, sourcefile)
        };
        let mut tree_to_analyze = rewriting.original_tree;
        let mut wrapped_tree: Option<TreeId> = None;
        let scope_owner_is_type = {
            let scope = rewriting.env.info.borrow().scope.expect("env scope");
            ctx.scope(scope)
                .owner
                .map(|owner| ctx.sym(owner).kind == Kind::TYP)
                .unwrap_or(false)
        };
        if scope_owner_is_type {
            let block = {
                let make = &*self.make;
                make.at(crate::util::position::NOPOS);
                make.block_pub(
                    ctx,
                    flags::SYNTHETIC as i64,
                    List::of(rewriting.original_tree),
                )
            };
            tree_to_analyze = block;
            wrapped_tree = Some(rewriting.original_tree);
        }
        let erroneous = Rc::clone(&rewriting.erroneous);
        let rewriter = TreeCopier::with_hook(
            Rc::clone(&self.make),
            Rc::new(TreeRewriterHook {
                old_tree: rewriting.old_tree,
                replacement: rewriting.replacement,
                wrapped_tree,
            }),
        );
        let attr_rc = self.attr_service();
        let mut attr = attr_rc.borrow_mut();
        let stat_info = attr.stat_info.clone();
        let deferred_rc = self.deferred_service();
        let deferred = deferred_rc.borrow_mut();
        let diag_creator: Box<dyn Fn() -> DiagnosticHandler> =
            Box::new(move || DiagnosticHandler::Analyzer {
                erroneous: Rc::clone(&erroneous),
            });
        // Java: deferredAttr.attribSpeculative(..., AttributionMode.ANALYZER,
        //     argumentAttr.withLocalCacheContext()); (Analyzer.java:570-571).
        let local_cache = argument_attr
            .as_ref()
            .map(|argument_attr| argument_attr.borrow().with_local_cache_context());
        let result = deferred.attrib_speculative_with_rewriter(
            ctx,
            syms,
            &mut attr,
            tree_to_analyze,
            &rewriting.env,
            &stat_info,
            &rewriter,
            Some(diag_creator),
            AttributionMode::ANALYZER,
            local_cache,
        );
        let has_errors = rewriting.erroneous.get();
        drop(deferred);
        drop(attr);
        if result.is_ok() {
            let kind = rewriting.kind;
            let old_tree = rewriting.old_tree;
            let replacement = rewriting.replacement;
            self.process(ctx, syms, kind, old_tree, replacement, has_errors);
        }
        crate::util::abstract_log::AbstractLog::use_source(&*self.log, prev_source);
    }

    /// Java: boolean isImplicitlyTyped(JCVariableDecl decl) (Analyzer.java:300-303).
    fn is_implicitly_typed(&self, ctx: &CompilerContext, decl: TreeId) -> bool {
        match &ctx.tree(decl).kind {
            TreeKind::VariableDecl {
                vartype: Some(vartype),
                ..
            } => ctx.tree(*vartype).pos == crate::util::position::NOPOS,
            TreeKind::VariableDecl { vartype: None, .. } => true,
            _ => false,
        }
    }

    /// Java: JCVariableDecl rewriteVarType(JCVariableDecl oldTree)
    fn rewrite_var_type(&mut self, ctx: &mut CompilerContext, old_tree: TreeId) -> TreeId {
        let new_tree = self.copier.copy(ctx, Some(old_tree)).expect("copied tree");
        if let TreeKind::VariableDecl {
            vartype,
            declared_using_var,
            ..
        } = &mut ctx.tree_mut(new_tree).kind
        {
            // Java: newTree.vartype = null makes the declaration implicitly
            // typed. The port also carries the parser's declared-using-var
            // flag, so set it for the inferred type to be computed.
            *vartype = None;
            *declared_using_var = true;
        }
        new_tree
    }

    /// Java: List<JCTree> decls(JCClassDecl decl) (Analyzer.java:270-285).
    fn decls(&self, ctx: &CompilerContext, def: TreeId) -> List<TreeId> {
        let defs = match &ctx.tree(def).kind {
            TreeKind::ClassDecl { defs, .. } => defs.clone(),
            _ => List::nil(),
        };
        let mut result: List<TreeId> = List::nil();
        for t in defs.iter() {
            if ctx.tree(t).tag == Tag::METHODDEF {
                let flags_value = match &ctx.tree(t).kind {
                    TreeKind::MethodDecl {
                        mods: Some(mods), ..
                    } => match &ctx.tree(*mods).kind {
                        TreeKind::Modifiers { flags, .. } => *flags,
                        _ => 0,
                    },
                    _ => 0,
                };
                if flags_value & flags::GENERATEDCONSTR == 0 {
                    result = result.append(t);
                }
            } else {
                result = result.append(t);
            }
        }
        result
    }

    /// Java: StatementAnalyzer.isEnabled/analyzer.match dispatch.
    fn matches(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        kind: AnalyzerKind,
        tree: TreeId,
        active_attr: Option<&mut Attr>,
    ) -> bool {
        if !self.analyzer_modes.contains(kind.mode()) {
            return false;
        }
        match kind {
            AnalyzerKind::DiamondInitializer => {
                let clazz = match &ctx.tree(tree).kind {
                    TreeKind::NewClass { clazz, .. } => clazz.expect("new class clazz"),
                    _ => return false,
                };
                if ctx.tree(clazz).tag != Tag::TYPEAPPLY
                    || crate::tree::tree_info::is_diamond(ctx, tree)
                {
                    return false;
                }
                let def = match &ctx.tree(tree).kind {
                    TreeKind::NewClass { def, .. } => *def,
                    _ => None,
                };
                def.is_none() || self.allow_diamond_with_anonymous_class_creation
            }
            AnalyzerKind::LambdaAnalyzer => {
                let (def, clazz) = match &ctx.tree(tree).kind {
                    TreeKind::NewClass { def, clazz, .. } => {
                        (*def, clazz.expect("new class clazz"))
                    }
                    _ => return false,
                };
                let Some(def) = def else {
                    return false;
                };
                let clazz_type = ctx.tree(clazz).ty;
                let Some(clazz_type) = clazz_type else {
                    return false;
                };
                if !ctx
                    .typ(clazz_type)
                    .has_tag(crate::code::type_tag::TypeTag::CLASS)
                {
                    return false;
                }
                let Some(tsym) = ctx.typ(clazz_type).tsym else {
                    return false;
                };
                let is_fi = match active_attr {
                    Some(attr) => {
                        let chk = Rc::clone(&attr.chk);
                        let diags = Rc::clone(&attr.diags);
                        self.types
                            .is_functional_interface(ctx, syms, &diags, &chk, tsym)
                    }
                    None => {
                        let attr_rc = self.attr_service();
                        let attr = attr_rc.borrow();
                        let chk = Rc::clone(&attr.chk);
                        let diags = Rc::clone(&attr.diags);
                        self.types
                            .is_functional_interface(ctx, syms, &diags, &chk, tsym)
                    }
                };
                if !is_fi {
                    return false;
                }
                let mut count = 0usize;
                let mut current = self.decls(ctx, def);
                while current.non_empty() {
                    count += 1;
                    current = current.tail();
                }
                count == 1
            }
            AnalyzerKind::RedundantTypeArgAnalyzer => match &ctx.tree(tree).kind {
                TreeKind::MethodInvocation { typeargs, .. } => !typeargs.is_empty(),
                _ => false,
            },
            AnalyzerKind::RedundantLocalVarTypeAnalyzer => {
                let (sym, init) = match &ctx.tree(tree).kind {
                    TreeKind::VariableDecl { sym, init, .. } => (*sym, *init),
                    _ => return false,
                };
                let Some(sym) = sym else {
                    return false;
                };
                let owner_is_mth = ctx
                    .sym(sym)
                    .owner
                    .map(|owner| ctx.sym(owner).kind == Kind::MTH)
                    .unwrap_or(false);
                if !owner_is_mth || init.is_none() || self.is_implicitly_typed(ctx, tree) {
                    return false;
                }
                let inferable = match active_attr {
                    Some(attr) => attr.can_infer_local_var_type(ctx, tree).is_none(),
                    None => {
                        let attr_rc = self.attr_service();
                        let attr = attr_rc.borrow();
                        attr.can_infer_local_var_type(ctx, tree).is_none()
                    }
                };
                inferable
            }
            AnalyzerKind::RedundantLocalVarTypeAnalyzerForEach => {
                let var = match &ctx.tree(tree).kind {
                    TreeKind::EnhancedForLoop { var, .. } => *var,
                    _ => return false,
                };
                match var {
                    Some(var) => !self.is_implicitly_typed(ctx, var),
                    None => false,
                }
            }
        }
    }
}

impl Analyzer {
    /// Java: StatementAnalyzer.rewrite dispatch (Analyzer.java:220-430).
    fn rewrite(
        &mut self,
        ctx: &mut CompilerContext,
        kind: AnalyzerKind,
        old_tree: TreeId,
    ) -> List<TreeId> {
        match kind {
            AnalyzerKind::DiamondInitializer => {
                let clazz = match &ctx.tree(old_tree).kind {
                    TreeKind::NewClass { clazz, .. } => clazz.expect("new class clazz"),
                    _ => return List::nil(),
                };
                if ctx.tree(clazz).tag == Tag::TYPEAPPLY {
                    let new_tree = self.copier.copy(ctx, Some(old_tree)).expect("copied tree");
                    if let TreeKind::NewClass { clazz, .. } = &ctx.tree(new_tree).kind {
                        let clazz = clazz.expect("new class clazz");
                        if let TreeKind::TypeApply { arguments, .. } = &mut ctx.tree_mut(clazz).kind
                        {
                            *arguments = List::nil();
                        }
                    }
                    List::of(new_tree)
                } else {
                    List::of(old_tree)
                }
            }
            AnalyzerKind::LambdaAnalyzer => {
                let def = match &ctx.tree(old_tree).kind {
                    TreeKind::NewClass { def, .. } => def.expect("anonymous class def"),
                    _ => return List::nil(),
                };
                let decls = self.decls(ctx, def);
                let first = *decls.head();
                let md = self.copier.copy(ctx, Some(first)).expect("copied method");
                let (params, body) = match &ctx.tree(md).kind {
                    TreeKind::MethodDecl { params, body, .. } => {
                        (params.clone(), body.expect("method body"))
                    }
                    _ => return List::nil(),
                };
                let pos = ctx.tree(old_tree).pos;
                let lambda = {
                    let make = &*self.make;
                    make.at(pos).lambda_pub(ctx, params, body)
                };
                List::of(lambda)
            }
            AnalyzerKind::RedundantTypeArgAnalyzer => {
                let new_tree = self.copier.copy(ctx, Some(old_tree)).expect("copied tree");
                if let TreeKind::MethodInvocation { typeargs, .. } =
                    &mut ctx.tree_mut(new_tree).kind
                {
                    *typeargs = List::nil();
                }
                List::of(new_tree)
            }
            AnalyzerKind::RedundantLocalVarTypeAnalyzer => {
                let new_tree = self.rewrite_var_type(ctx, old_tree);
                List::of(new_tree)
            }
            AnalyzerKind::RedundantLocalVarTypeAnalyzerForEach => {
                let new_tree = self.copier.copy(ctx, Some(old_tree)).expect("copied tree");
                let old_var = match &ctx.tree(old_tree).kind {
                    TreeKind::EnhancedForLoop { var, .. } => var.expect("foreach var"),
                    _ => return List::nil(),
                };
                let new_var = self.rewrite_var_type(ctx, old_var);
                let body_pos = ctx.tree(new_tree).pos;
                let empty_block = {
                    let make = &*self.make;
                    make.at(body_pos).block_pub(ctx, 0, List::nil())
                };
                if let TreeKind::EnhancedForLoop { var, body, .. } =
                    &mut ctx.tree_mut(new_tree).kind
                {
                    *var = Some(new_var);
                    *body = Some(empty_block);
                }
                List::of(new_tree)
            }
        }
    }

    /// Java: StatementAnalyzer.process dispatch (Analyzer.java:230-430).
    fn process(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        kind: AnalyzerKind,
        old_tree: TreeId,
        new_tree: TreeId,
        has_errors: bool,
    ) {
        if has_errors {
            return;
        }
        match kind {
            AnalyzerKind::DiamondInitializer => {
                let old_clazz = match &ctx.tree(old_tree).kind {
                    TreeKind::NewClass { clazz, .. } => clazz.expect("new class clazz"),
                    _ => return,
                };
                let old_def = match &ctx.tree(old_tree).kind {
                    TreeKind::NewClass { def, .. } => *def,
                    _ => None,
                };
                let new_def = match &ctx.tree(new_tree).kind {
                    TreeKind::NewClass { def, .. } => *def,
                    _ => None,
                };
                let inferred_args;
                let explicit_args;
                if let Some(_old_def) = old_def {
                    let (new_impl, new_ext) = match &ctx.tree(new_def.expect("new def")).kind {
                        TreeKind::ClassDecl {
                            implementing,
                            extending,
                            ..
                        } => (implementing.clone(), *extending),
                        _ => (List::nil(), None),
                    };
                    let (old_impl, old_ext) = match &ctx.tree(_old_def).kind {
                        TreeKind::ClassDecl {
                            implementing,
                            extending,
                            ..
                        } => (implementing.clone(), *extending),
                        _ => (List::nil(), None),
                    };
                    let new_type_tree = if !new_impl.is_empty() {
                        *new_impl.head()
                    } else {
                        new_ext.expect("extends")
                    };
                    let old_type_tree = if !old_impl.is_empty() {
                        *old_impl.head()
                    } else {
                        old_ext.expect("extends")
                    };
                    inferred_args = ctx
                        .tree(new_type_tree)
                        .ty
                        .map(|t| crate::comp::attr::type_arguments(ctx, t))
                        .unwrap_or_else(List::nil);
                    explicit_args = ctx
                        .tree(old_type_tree)
                        .ty
                        .map(|t| crate::comp::attr::type_arguments(ctx, t))
                        .unwrap_or_else(List::nil);
                } else {
                    inferred_args = ctx
                        .tree(new_tree)
                        .ty
                        .map(|t| crate::comp::attr::type_arguments(ctx, t))
                        .unwrap_or_else(List::nil);
                    explicit_args = ctx
                        .tree(old_tree)
                        .ty
                        .map(|t| crate::comp::attr::type_arguments(ctx, t))
                        .unwrap_or_else(List::nil);
                }
                let mut explicit = explicit_args;
                let mut current = inferred_args;
                while current.non_empty() {
                    let t = *current.head();
                    if !explicit.non_empty() {
                        return;
                    }
                    let head = *explicit.head();
                    if !self.types.is_same_type(ctx, syms, t, head) {
                        return;
                    }
                    explicit = explicit.tail();
                    current = current.tail();
                }
                self.log_warning(
                    ctx,
                    ctx.tree(old_clazz).pos,
                    crate::resources::compiler_properties::warnings::diamond_redundant_args(),
                );
            }
            AnalyzerKind::LambdaAnalyzer => {
                let old_def = match &ctx.tree(old_tree).kind {
                    TreeKind::NewClass { def, .. } => def.expect("anonymous class def"),
                    _ => return,
                };
                self.log_warning(
                    ctx,
                    ctx.tree(old_def).pos,
                    crate::resources::compiler_properties::warnings::potential_lambda_found(),
                );
            }
            AnalyzerKind::RedundantTypeArgAnalyzer => {
                self.log_warning(
                    ctx,
                    ctx.tree(old_tree).pos,
                    crate::resources::compiler_properties::warnings::method_redundant_typeargs(),
                );
            }
            AnalyzerKind::RedundantLocalVarTypeAnalyzer => {
                self.process_var(ctx, syms, old_tree, new_tree);
            }
            AnalyzerKind::RedundantLocalVarTypeAnalyzerForEach => {
                let old_var = match &ctx.tree(old_tree).kind {
                    TreeKind::EnhancedForLoop { var, .. } => var.expect("foreach var"),
                    _ => return,
                };
                let new_var = match &ctx.tree(new_tree).kind {
                    TreeKind::EnhancedForLoop { var, .. } => var.expect("foreach var"),
                    _ => return,
                };
                self.process_var(ctx, syms, old_var, new_var);
            }
        }
    }

    /// Java: void processVar(JCVariableDecl oldTree, JCVariableDecl newTree,
    /// boolean hasErrors) (Analyzer.java:310-318).
    fn process_var(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        old_tree: TreeId,
        new_tree: TreeId,
    ) {
        let (old_ty, new_ty) = (ctx.tree(old_tree).ty, ctx.tree(new_tree).ty);
        if let (Some(old_ty), Some(new_ty)) = (old_ty, new_ty)
            && self.types.is_same_type(ctx, &syms, old_ty, new_ty)
        {
            self.log_warning(
                ctx,
                ctx.tree(old_tree).pos,
                crate::resources::compiler_properties::warnings::local_redundant_type(),
            );
        }
    }

    /// adapter helper: log.warning(pos, Warnings.X).
    fn log_warning(
        &self,
        _ctx: &mut CompilerContext,
        pos: i32,
        warning: crate::resources::diagnostic_contract::WarningInfo,
    ) {
        let warning = crate::util::jc_diagnostic::Warning::from(warning);
        crate::util::mandatory_warning_handler::MandatoryWarningLog::warning(
            &*self.log,
            None,
            Some(Rc::new(
                crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
            )),
            warning,
        );
    }
}

/// Java: Analyzer.StatementScanner (Analyzer.java:590-740).
pub struct StatementScanner<'a> {
    /// the Analyzer (inner class access)
    pub analyzer: &'a mut Analyzer,
    /// the shared symbol table used by the scanner queries
    pub syms: &'a crate::code::symtab::Symtab,
    /// Java: JCTree originalTree
    pub original_tree: TreeId,
    /// Java: Env<AttrContext> env (attr.copyEnv(env))
    pub env: Rc<Env<AttrContext>>,
    /// Java: ListBuffer<RewritingContext> rewritings
    pub rewritings: Vec<RewritingContext>,
    /// The live Attr when the scanner is invoked from attribStat (the caller
    /// already holds the Attr RefCell mutably).
    pub active_attr: Option<&'a mut Attr>,
}

impl<'a> StatementScanner<'a> {
    /// Java: StatementScanner(JCTree originalTree, Env<AttrContext> env).
    pub fn new(
        analyzer: &'a mut Analyzer,
        ctx: &mut CompilerContext,
        syms: &'a crate::code::symtab::Symtab,
        original_tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) -> StatementScanner<'a> {
        let copied_env = {
            let attr_rc = analyzer.attr_service();
            let mut attr = attr_rc.borrow_mut();
            attr.copy_env(ctx, syms, &analyzer.types, env)
        };
        StatementScanner {
            analyzer,
            syms,
            original_tree,
            env: copied_env,
            rewritings: Vec::new(),
            active_attr: None,
        }
    }

    /// StatementScanner constructor used while Attr is already mutably
    /// borrowed by its caller (attribStat); Java passes the live Attr object.
    pub fn new_with_attr(
        analyzer: &'a mut Analyzer,
        ctx: &mut CompilerContext,
        syms: &'a crate::code::symtab::Symtab,
        original_tree: TreeId,
        env: &Rc<Env<AttrContext>>,
        attr: &'a mut Attr,
    ) -> StatementScanner<'a> {
        let copied_env = attr.copy_env(ctx, syms, &analyzer.types, env);
        StatementScanner {
            analyzer,
            syms,
            original_tree,
            env: copied_env,
            rewritings: Vec::new(),
            active_attr: Some(attr),
        }
    }

    /// Java: public void scan() (StatementScanner scan entry).
    pub fn scan_statement(&mut self, ctx: &mut CompilerContext) {
        let original = self.original_tree;
        self.scan(ctx, Some(original));
    }
}

impl TreeScannerVisitor for StatementScanner<'_> {
    /// Java: public void scan(JCTree tree) (Analyzer.java:660-680).
    #[allow(clippy::collapsible_if)]
    fn scan(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) {
        let Some(tree) = tree else {
            return;
        };
        for kind in [
            AnalyzerKind::DiamondInitializer,
            AnalyzerKind::LambdaAnalyzer,
            AnalyzerKind::RedundantTypeArgAnalyzer,
            AnalyzerKind::RedundantLocalVarTypeAnalyzer,
            AnalyzerKind::RedundantLocalVarTypeAnalyzerForEach,
        ] {
            if !self.analyzer.analyzer_modes.contains(kind.mode()) {
                continue;
            }
            if ctx.tree(tree).tag != kind.tag() {
                continue;
            }
            let active_attr = self.active_attr.as_deref_mut();
            if !self
                .analyzer
                .matches(ctx, self.syms, kind, tree, active_attr)
            {
                continue;
            }
            let replacements = self.analyzer.rewrite(ctx, kind, tree);
            let mut current = replacements;
            while current.non_empty() {
                let replacement = *current.head();
                let env = {
                    let env = if let Some(attr) = self.active_attr.as_deref_mut() {
                        attr.copy_env(ctx, self.syms, &self.analyzer.types, &self.env)
                    } else {
                        let attr_rc = self.analyzer.attr_service();
                        let mut attr = attr_rc.borrow_mut();
                        attr.copy_env(ctx, self.syms, &self.analyzer.types, &self.env)
                    };
                    let env_scope = env.info.borrow().scope.expect("env scope");
                    if ctx.tree(tree).tag == Tag::VARDEF {
                        if let TreeKind::VariableDecl { sym: Some(sym), .. } = &ctx.tree(tree).kind
                        {
                            ctx.scope_remove(env_scope, *sym);
                        }
                    }
                    env
                };
                self.rewritings.push(RewritingContext {
                    original_tree: self.original_tree,
                    old_tree: tree,
                    replacement,
                    erroneous: Rc::new(Cell::new(false)),
                    env,
                    kind,
                });
                current = current.tail();
            }
            break;
        }
        crate::tree::jc_tree::accept(ctx, tree, self);
    }

    /// Java: public void visitClassDef(JCClassDecl tree) — do nothing.
    fn visit_class_def(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}
    /// Java: public void visitMethodDef(JCMethodDecl tree) — do nothing.
    fn visit_method_def(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}
    /// Java: public void visitBlock(JCBlock tree) — do nothing.
    fn visit_block(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}
    /// Java: public void visitDoLoop(JCDoWhileLoop tree) — scan(cond).
    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let cond = match &ctx.tree(tree).kind {
            TreeKind::DoWhileLoop { cond, .. } => *cond,
            _ => None,
        };
        self.scan(ctx, cond);
    }

    /// Java: public void visitWhileLoop(JCWhileLoop tree) — scan(cond).
    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let cond = match &ctx.tree(tree).kind {
            TreeKind::WhileLoop { cond, .. } => *cond,
            _ => None,
        };
        self.scan(ctx, cond);
    }

    /// Java: public void visitForLoop(JCForLoop tree) — scan(cond/update).
    fn visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, step) = match &ctx.tree(tree).kind {
            TreeKind::ForLoop { cond, step, .. } => (*cond, step.clone()),
            _ => (None, List::nil()),
        };
        self.scan(ctx, cond);
        let mut current = step;
        while current.non_empty() {
            let item = *current.head();
            self.scan(ctx, Some(item));
            current = current.tail();
        }
    }

    /// Java: public void visitForeachLoop(JCEnhancedForLoop tree) — scan(expr).
    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop { expr, .. } => *expr,
            _ => None,
        };
        self.scan(ctx, expr);
    }

    /// Java: public void visitSwitch(JCSwitch tree) — scan(selector).
    fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let selector = match &ctx.tree(tree).kind {
            TreeKind::Switch { selector, .. } => *selector,
            _ => None,
        };
        self.scan(ctx, selector);
    }

    /// Java: public void visitTry(JCTry tree) — scan(block/catches/finally).
    fn visit_try(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, catchers, finalizer) = match &ctx.tree(tree).kind {
            TreeKind::Try {
                body,
                catchers,
                finalizer,
                ..
            } => (*body, catchers.clone(), *finalizer),
            _ => (None, List::nil(), None),
        };
        self.scan(ctx, body);
        let mut current = catchers;
        while current.non_empty() {
            let item = *current.head();
            self.scan(ctx, Some(item));
            current = current.tail();
        }
        self.scan(ctx, finalizer);
    }

    /// Java: public void visitIf(JCIf tree) — scan(cond).
    fn visit_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let cond = match &ctx.tree(tree).kind {
            TreeKind::If { cond, .. } => *cond,
            _ => None,
        };
        self.scan(ctx, cond);
    }

    /// Java: public void visitLambda(JCLambda tree) — do nothing.
    fn visit_lambda(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}
}

tree_scanner_visitor_delegate!(StatementScanner<'a>, 'a);
