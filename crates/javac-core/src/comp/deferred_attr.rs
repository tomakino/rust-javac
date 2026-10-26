// Origin: com/sun/tools/javac/comp/DeferredAttr.java @ 982d53c (jdk-17.0.19-ga)
//
// PORTING_RULES §19 mapping: AttributionMode / AttrMode / DeferredStuckPolicy /
// dummyStuckPolicy / SpeculativeCache / DeferredType operations /
// DeferredAttrContext / DeferredAttrNode / StuckNode / DeferredTypeMap /
// RecoveryDeferredTypeMap / FilterScanner / PolyScanner / LambdaReturnScanner /
// SwitchExpressionScanner / CheckStuckPolicy / OverloadStuckPolicy are ported
// here. NOT PORTED (registered): the DeferredAttr service and
// attribSpeculative/ex attribSpeculativeLambda/DeferredAttrDiagHandler (need
// Attr.copyEnv[ported]/TreeCopier/ArgumentAttr.LocalCacheContext/Flow.analyzeLambda/
// Log.DeferredDiagnosticHandler/Annotate.Queues/DeferredCompletionFailureHandler);
// DeferredAttrNode.StructuralStuckChecker (needs Types.findDescriptorType/
// isFunctionalInterface, Attr.lambdaEnv/preFlow/unknownExprInfo/
// memberReferenceQualifierResult, ArgumentAttr, Resolve.resolveMemberReference);
// CheckStuckPolicy.visitLambda/visitReference (needs the FunctionDescriptor
// batch); RecoveryDeferredTypeMap.recover (needs Attr.RecoveryInfo and
// Check.checkNonVoid); DeferredAttr.CannotInfer errors live in Resolve (Resolve B
// batch). The DeferredAttr service construction is an ADR 0001 explicit-context
// TODO.

#![allow(non_camel_case_types)]

use std::cell::RefCell;
use std::rc::Rc;

use crate::code::symtab::Symtab;
use crate::code::type_::TypeKindData;
use crate::code::type_tag::TypeTag;
use crate::code::types::Types;
use crate::comp::attr::Attr;
use crate::comp::attr::ResultInfo;
use crate::comp::env::Env;
use crate::comp::infer::{InferenceContext, InferenceException};
use crate::comp::resolve::MethodResolutionPhase;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::tree::jc_tree::{Tag, TreeKind};
use crate::util::list::List;
use crate::util::warner::Warner;

/// Java: enum DeferredAttr.AttributionMode
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AttributionMode {
    /// Normal, non-speculative, attribution.
    #[default]
    FULL,
    /// Speculative attribution on behalf of an Analyzer.
    ATTRIB_TO_TREE,
    /// Speculative attribution on behalf of an Analyzer.
    ANALYZER,
    /// Speculative attribution.
    SPECULATIVE,
}

impl AttributionMode {
    /// Java: AttributionMode(boolean isSpeculative, boolean recover)
    pub fn is_speculative(self) -> bool {
        !matches!(self, AttributionMode::FULL)
    }

    pub fn recover(self) -> bool {
        matches!(self, AttributionMode::FULL | AttributionMode::ATTRIB_TO_TREE)
    }
}

/// Java: public enum DeferredAttr.AttrMode
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttrMode {
    /// Java: SPECULATIVE
    SPECULATIVE,
    /// Java: CHECK
    CHECK,
}

/// Java: DeferredType.SpeculativeCache.Entry
#[derive(Clone)]
pub struct SpeculativeEntry {
    /// Java: JCTree speculativeTree
    pub speculative_tree: TreeId,
    /// Java: ResultInfo resultInfo
    pub result_info: Rc<ResultInfo>,
}

impl SpeculativeEntry {
    /// Java: boolean matches(MethodResolutionPhase phase)
    pub fn matches(&self, phase: MethodResolutionPhase) -> bool {
        self.result_info
            .check_context
            .deferred_attr_context()
            .phase
            == phase
    }
}

/// Java: DeferredType.SpeculativeCache (Map<Symbol,List<Entry>>; WeakHashMap
/// upstream, the port keeps a Vec of pairs — registered adapter)
#[derive(Clone, Default)]
pub struct SpeculativeCache {
    pub cache: Vec<(Option<SymbolId>, Vec<SpeculativeEntry>)>,
}

impl SpeculativeCache {
    /// Java: Entry get(Symbol msym, MethodResolutionPhase phase)
    pub fn get(
        &self,
        msym: Option<SymbolId>,
        phase: MethodResolutionPhase,
    ) -> Option<SpeculativeEntry> {
        let entries = self
            .cache
            .iter()
            .find(|(key, _)| *key == msym)
            .map(|(_, entries)| entries)?;
        entries.iter().find(|entry| entry.matches(phase)).cloned()
    }

    /// Java: void put(JCTree speculativeTree, ResultInfo resultInfo)
    pub fn put(&mut self, speculative_tree: TreeId, result_info: Rc<ResultInfo>) {
        let msym = result_info.check_context.deferred_attr_context().msym;
        let entry = SpeculativeEntry {
            speculative_tree,
            result_info,
        };
        if let Some((_, entries)) = self.cache.iter_mut().find(|(key, _)| *key == msym) {
            entries.insert(0, entry);
        } else {
            self.cache.push((msym, vec![entry]));
        }
    }
}

/// Java: public class DeferredType extends Type (tag DEFERRED)
///
/// PORTING_RULES: the upstream DeferredType instance is a Type object; the port
/// stores the fields in TypeKindData::Deferred (code/type_.rs) and the methods
/// are free functions taking the arena context.
pub fn deferred_type_new(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    attr: &mut Attr,
    tree: TreeId,
    env: &Rc<Env<crate::comp::attr_context::AttrContext>>,
) -> TypeId {
    let copied_env = attr.copy_env(ctx, syms, types, env);
    ctx.alloc_type(crate::code::type_::Type::new(
        TypeTag::DEFERRED,
        None,
        TypeKindData::Deferred {
            tree,
            env: copied_env,
            mode: None,
            not_pertinent: Vec::new(),
            speculative_cache: Rc::new(RefCell::new(SpeculativeCache::default())),
            argument: None,
        },
    ))
}

type DeferredFields = (
    TreeId,
    Rc<Env<crate::comp::attr_context::AttrContext>>,
    Option<AttrMode>,
    Vec<SymbolId>,
    Rc<RefCell<SpeculativeCache>>,
    Option<Rc<RefCell<crate::comp::argument_attr::ArgumentTypeData>>>,
);

fn deferred_fields(ctx: &CompilerContext, t: TypeId) -> DeferredFields {
    match &ctx.typ(t).kind {
        TypeKindData::Deferred {
            tree,
            env,
            mode,
            not_pertinent,
            speculative_cache,
            argument,
        } => (
            *tree,
            Rc::clone(env),
            *mode,
            not_pertinent.clone(),
            Rc::clone(speculative_cache),
            argument.clone(),
        ),
        _ => panic!("ClassCastException: not a DeferredType"),
    }
}

/// Java: DeferredType.getTag() -> DEFERRED
pub fn deferred_type_tag(ctx: &CompilerContext, t: TypeId) -> TypeTag {
    ctx.typ(t).get_tag()
}

/// Java: DeferredType.speculativeType(Symbol msym, MethodResolutionPhase phase)
pub fn deferred_type_speculative_type(
    ctx: &CompilerContext,
    t: TypeId,
    msym: Option<SymbolId>,
    phase: MethodResolutionPhase,
) -> Option<TypeId> {
    let (_, _, _, _, cache, _) = deferred_fields(ctx, t);
    let entry = cache.borrow().get(msym, phase)?;
    ctx.tree(entry.speculative_tree).ty
}

/// Java: JCTree speculativeTree(DeferredAttrContext deferredAttrContext)
pub fn deferred_type_speculative_tree(
    ctx: &CompilerContext,
    t: TypeId,
    deferred_attr_context: &DeferredAttrContext,
) -> Option<TreeId> {
    let (_, _, _, _, cache, _) = deferred_fields(ctx, t);
    cache
        .borrow()
        .get(deferred_attr_context.msym, deferred_attr_context.phase)
        .map(|entry| entry.speculative_tree)
}

/// Java: public Type complete(ResultInfo resultInfo, DeferredAttrContext
/// deferredAttrContext) (DeferredAttr.java:309-330)
pub fn deferred_type_complete(
    ctx: &mut CompilerContext,
    syms: &mut Symtab,
    attr: &mut Attr,
    t: TypeId,
    result_info: &ResultInfo,
    deferred_attr_context: &DeferredAttrContext,
) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
    match deferred_attr_context.mode {
        AttrMode::SPECULATIVE => {
            // Java: attribSpeculative(tree, env, resultInfo); speculativeCache.put(...)
            unimplemented!(
                "TODO(Attr/ArgumentAttr/Flow batch): DeferredAttr.attribSpeculative (DeferredAttr.java:466-518)"
            )
        }
        AttrMode::CHECK => {
            let (tree, env, _, _, _, _) = deferred_fields(ctx, t);
            attr.attrib_tree(ctx, syms, tree, &env, result_info.clone())
        }
    }
}

/// The failure type of the deferred attribution machinery: upstream propagates
/// both CompletionFailure and InferenceException; the port keeps both.
#[derive(Debug)]
pub enum DeferredAttrFailure {
    Inference(InferenceException),
    Completion(crate::code::symbol::CompletionFailure),
}

impl From<InferenceException> for DeferredAttrFailure {
    fn from(value: InferenceException) -> DeferredAttrFailure {
        DeferredAttrFailure::Inference(value)
    }
}

impl From<crate::code::symbol::CompletionFailure> for DeferredAttrFailure {
    fn from(value: crate::code::symbol::CompletionFailure) -> DeferredAttrFailure {
        DeferredAttrFailure::Completion(value)
    }
}

/// Java: interface DeferredStuckPolicy (DeferredAttr.java:372-390)
pub trait DeferredStuckPolicy {
    /// Java: boolean isStuck();
    fn is_stuck(&self) -> bool;
    /// Java: Set<Type> stuckVars();
    fn stuck_vars(&self) -> Vec<TypeId>;
    /// Java: Set<Type> depVars();
    fn dep_vars(&self) -> Vec<TypeId>;
}

/// Java: DeferredAttr.dummyStuckPolicy
pub struct DummyStuckPolicy;

impl DeferredStuckPolicy for DummyStuckPolicy {
    fn is_stuck(&self) -> bool {
        false
    }

    fn stuck_vars(&self) -> Vec<TypeId> {
        Vec::new()
    }

    fn dep_vars(&self) -> Vec<TypeId> {
        Vec::new()
    }
}

/// Java: class DeferredAttr.DeferredAttrContext
pub struct DeferredAttrContext {
    /// Java: final AttrMode mode
    pub mode: AttrMode,
    /// Java: final Symbol msym (null -> None)
    pub msym: Option<SymbolId>,
    /// Java: final Resolve.MethodResolutionPhase phase
    pub phase: MethodResolutionPhase,
    /// Java: final InferenceContext inferenceContext
    pub inference_context: Rc<InferenceContext>,
    /// Java: final DeferredAttrContext parent
    pub parent: Option<Rc<DeferredAttrContext>>,
    /// Java: final Warner warn (shared mutable object upstream; RefCell keeps
    /// the mutable warning state — registered: warning flags are not
    /// propagated back to the caller, Lint batch)
    pub warn: Option<RefCell<Warner>>,
    /// Java: ArrayList<DeferredAttrNode> deferredAttrNodes
    pub deferred_attr_nodes: RefCell<Vec<Rc<DeferredAttrNode>>>,
    /// True for the emptyDeferredAttrContext (upstream anonymous subclass
    /// asserts in addDeferredAttrNode/complete; the port keeps the flag because
    /// the singleton identity is not preserved — registered adapter)
    pub empty: bool,
}

impl DeferredAttrContext {
    /// Java: DeferredAttrContext(AttrMode mode, Symbol msym,
    /// MethodResolutionPhase phase, InferenceContext inferenceContext,
    /// DeferredAttrContext parent, Warner warn)
    pub fn new(
        mode: AttrMode,
        msym: Option<SymbolId>,
        phase: MethodResolutionPhase,
        inference_context: Rc<InferenceContext>,
        parent: Option<Rc<DeferredAttrContext>>,
        warn: Option<Warner>,
    ) -> DeferredAttrContext {
        DeferredAttrContext {
            mode,
            msym,
            phase,
            inference_context,
            parent,
            warn: warn.map(RefCell::new),
            deferred_attr_nodes: RefCell::new(Vec::new()),
            empty: false,
        }
    }

    /// Java: DeferredAttr.emptyDeferredAttrContext
    pub fn empty() -> DeferredAttrContext {
        DeferredAttrContext {
            mode: AttrMode::CHECK,
            msym: None,
            phase: MethodResolutionPhase::BOX,
            inference_context: crate::comp::infer::empty_context(),
            parent: None,
            warn: None,
            deferred_attr_nodes: RefCell::new(Vec::new()),
            empty: true,
        }
    }

    /// Java: void addDeferredAttrNode(DeferredType dt, ResultInfo resultInfo,
    /// DeferredStuckPolicy deferredStuckPolicy)
    pub fn add_deferred_attr_node(&self, node: Rc<DeferredAttrNode>) {
        if self.empty {
            panic!("AssertionError: Empty deferred context!");
        }
        self.deferred_attr_nodes.borrow_mut().push(node);
    }

    /// Java: public boolean insideOverloadPhase() (DeferredAttr.java:637-648)
    pub fn inside_overload_phase(&self) -> bool {
        if self.empty {
            return false;
        }
        if self.mode == AttrMode::SPECULATIVE {
            return true;
        }
        match &self.parent {
            Some(parent) => parent.inside_overload_phase(),
            None => false,
        }
    }
}

impl DeferredAttrContext {
    /// Java: void complete() (DeferredAttr.java:602-635)
    pub fn complete(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        types: &Types,
        attr: &mut Attr,
        infer: &crate::comp::infer::Infer,
    ) -> Result<(), DeferredAttrFailure> {
        if self.empty {
            panic!("AssertionError: Empty deferred context!");
        }
        loop {
            let nodes = self.deferred_attr_nodes.borrow().clone();
            if nodes.is_empty() {
                break;
            }
            let mut progress = false;
            for node in &nodes {
                if process_deferred_attr_node(ctx, syms, types, attr, infer, node, self)? {
                    let mut list = self.deferred_attr_nodes.borrow_mut();
                    if let Some(pos) = list.iter().position(|n| Rc::ptr_eq(n, node)) {
                        list.remove(pos);
                    }
                    progress = true;
                }
            }
            if !progress {
                if self.inside_overload_phase() {
                    for node in self.deferred_attr_nodes.borrow().iter() {
                        let tree = match &ctx.typ(node.dt).kind {
                            TypeKindData::Deferred { tree, .. } => *tree,
                            _ => panic!("ClassCastException: not a DeferredType"),
                        };
                        ctx.tree_mut(tree).ty = Some(crate::code::symtab::no_type_id(ctx));
                    }
                    return Ok(());
                }
                let to_unstuck = self.pick_deferred_node(ctx, syms, types, infer)?;
                let Some(to_unstuck) = to_unstuck else {
                    break;
                };
                let stuck_vars = to_unstuck.deferred_stuck_policy.stuck_vars();
                let context = Rc::clone(&self.inference_context);
                let mut default_warn = Warner::new(None);
                let mut warn_guard = self.warn.as_ref().map(|warn| warn.borrow_mut());
                let warn_ref: &mut Warner = match warn_guard.as_mut() {
                    Some(warn) => warn,
                    None => &mut default_warn,
                };
                context.solve_any(
                    ctx,
                    syms,
                    types,
                    List::from_iterable(stuck_vars),
                    warn_ref,
                )?;
                let context = Rc::clone(&self.inference_context);
                context.notify_change(ctx, syms, types)?;
            }
        }
        Ok(())
    }

    /// Java: DeferredAttrNode pickDeferredNode() (DeferredAttr.java:657-662)
    fn pick_deferred_node(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        types: &Types,
        infer: &crate::comp::infer::Infer,
    ) -> Result<Option<Rc<DeferredAttrNode>>, DeferredAttrFailure> {
        let stuck_graph = self.build_stuck_graph(ctx, syms, types, infer)?;
        let deps: Vec<Vec<usize>> = stuck_graph.iter().map(|n| n.deps.clone()).collect();
        let sccs = crate::util::graph_utils::tarjan_scc_indexed(&deps);
        let Some(first) = sccs.first() else {
            panic!("java.util.NoSuchElementException");
        };
        if first.len() == 1 {
            return Ok(Some(Rc::clone(&stuck_graph[first[0]].data)));
        }
        let nodes = self.deferred_attr_nodes.borrow();
        if nodes.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Rc::clone(&nodes[0])))
        }
    }

    /// Java: List<StuckNode> buildStuckGraph() (DeferredAttr.java:664-683)
    fn build_stuck_graph(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        types: &Types,
        infer: &crate::comp::infer::Infer,
    ) -> Result<Vec<StuckNode>, DeferredAttrFailure> {
        let mut default_warn = Warner::new(None);
        let mut warn_guard = self.warn.as_ref().map(|warn| warn.borrow_mut());
        let warn_ref: &mut Warner = match warn_guard.as_mut() {
            Some(warn) => warn,
            None => &mut default_warn,
        };
        infer.do_incorporation(ctx, syms, types, &self.inference_context, warn_ref)?;
        let graph =
            crate::comp::infer::InferenceGraph::new(ctx, syms, types, &self.inference_context);
        let mut nodes: Vec<StuckNode> = self
            .deferred_attr_nodes
            .borrow()
            .iter()
            .map(|node| StuckNode {
                data: Rc::clone(node),
                deps: Vec::new(),
            })
            .collect();
        let count = nodes.len();
        for sn1 in 0..count {
            for sn2 in 0..count {
                if sn1 != sn2 && can_influence(&graph, &nodes[sn2], &nodes[sn1]) {
                    nodes[sn1].deps.push(sn2);
                }
            }
        }
        Ok(nodes)
    }
}

/// Java: boolean canInfluence(InferenceGraph graph, StuckNode sn1,
/// StuckNode sn2) (DeferredAttr.java:685-704)
#[allow(clippy::collapsible_if)]
fn can_influence(
    graph: &crate::comp::infer::InferenceGraph,
    sn1: &StuckNode,
    sn2: &StuckNode,
) -> bool {
    let output_vars = sn1.data.deferred_stuck_policy.dep_vars();
    for input_var in sn2.data.deferred_stuck_policy.stuck_vars() {
        if let Some(input_node) = graph.find_node(input_var) {
            let input_closure = graph.node_closure(input_node);
            for output_var in &output_vars {
                if let Some(output_node) = graph.find_node(*output_var) {
                    if input_closure.contains(&output_node) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Java: class StuckNode extends GraphUtils.TarjanNode (the node graph is kept
/// as an index list; util/graph_utils.rs tarjan_scc_indexed is the faithful
/// Tarjan translation — §9 adapter)
pub struct StuckNode {
    pub data: Rc<DeferredAttrNode>,
    pub deps: Vec<usize>,
}

/// Java: class DeferredAttrNode
pub struct DeferredAttrNode {
    /// Java: DeferredType dt
    pub dt: TypeId,
    /// Java: ResultInfo resultInfo
    pub result_info: Rc<ResultInfo>,
    /// Java: DeferredStuckPolicy deferredStuckPolicy
    pub deferred_stuck_policy: Rc<dyn DeferredStuckPolicy>,
}

/// Java: boolean process(final DeferredAttrContext deferredAttrContext)
/// (DeferredAttr.java:757-800)
#[allow(clippy::too_many_arguments)]
fn process_deferred_attr_node(
    ctx: &mut CompilerContext,
    syms: &mut Symtab,
    types: &Types,
    attr: &mut Attr,
    infer: &crate::comp::infer::Infer,
    node: &DeferredAttrNode,
    deferred_attr_context: &DeferredAttrContext,
) -> Result<bool, DeferredAttrFailure> {
    match deferred_attr_context.mode {
        AttrMode::SPECULATIVE => {
            if node.deferred_stuck_policy.is_stuck() {
                unimplemented!(
                    "TODO(FunctionDescriptor/Attr batch): DeferredAttrNode.StructuralStuckChecker (DeferredAttr.java:800-990)"
                )
            } else {
                panic!("AssertionError: Cannot get here");
            }
        }
        AttrMode::CHECK => {
            if node.deferred_stuck_policy.is_stuck() {
                let parent = deferred_attr_context.parent.clone();
                let propagate = match &parent {
                    Some(parent) => {
                        !parent.empty
                            && contains_any_inference_vars(
                                ctx,
                                syms,
                                types,
                                &parent.inference_context,
                                &node.deferred_stuck_policy.stuck_vars(),
                            )
                    }
                    None => false,
                };
                if propagate {
                    let parent = parent.expect("parent deferred context");
                    let nested = Rc::new(ParentCheckContext {
                        parent: Rc::clone(&parent),
                        enclosing: Rc::clone(&node.result_info.check_context),
                    });
                    let new_info = node.result_info.dup_check_context(nested);
                    parent.add_deferred_attr_node(Rc::new(DeferredAttrNode {
                        dt: node.dt,
                        result_info: Rc::new(new_info),
                        deferred_stuck_policy: Rc::clone(&node.deferred_stuck_policy),
                    }));
                    let tree = match &ctx.typ(node.dt).kind {
                        TypeKindData::Deferred { tree, .. } => *tree,
                        _ => panic!("ClassCastException: not a DeferredType"),
                    };
                    ctx.tree_mut(tree).ty = Some(syms.stuck_type);
                    Ok(true)
                } else {
                    Ok(false)
                }
            } else {
                assert!(
                    !deferred_attr_context.inside_overload_phase(),
                    "AssertionError: attribution shouldn't be happening here"
                );
                let inst_pt = {
                    let pt = node
                        .result_info
                        .pt
                        .unwrap_or_else(|| crate::code::symtab::no_type_id(ctx));
                    let context = Rc::clone(&deferred_attr_context.inference_context);
                    context.as_inst_type(ctx, syms, types, pt)
                };
                let inst_result_info = node.result_info.dup(Some(inst_pt));
                let info_context =
                    Rc::clone(inst_result_info.check_context.deferred_attr_context());
                let dummy: Rc<dyn DeferredStuckPolicy> = Rc::new(DummyStuckPolicy);
                check_with_policy(
                    ctx,
                    syms,
                    types,
                    attr,
                    node.dt,
                    &inst_result_info,
                    &info_context,
                    &dummy,
                )?;
                let _ = infer;
                Ok(true)
            }
        }
    }
}

/// The CheckContext used when a stuck node is propagated to the parent deferred
/// context (DeferredAttr.java:769-785, anonymous NestedCheckContext subclass).
struct ParentCheckContext {
    parent: Rc<DeferredAttrContext>,
    enclosing: Rc<dyn crate::comp::check::CheckContext>,
}

impl crate::comp::check::CheckContext for ParentCheckContext {
    fn compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        found: TypeId,
        req: TypeId,
        warn: &mut Warner,
    ) -> bool {
        self.enclosing.compatible(ctx, syms, types, found, req, warn)
    }

    fn report(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        details: &crate::util::jc_diagnostic::JCDiagnostic,
    ) {
        self.enclosing.report(ctx, pos, details)
    }

    fn check_warner(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> Warner {
        self.enclosing.check_warner(ctx, syms, types, pos, found, req)
    }

    fn inference_context(&self) -> &Rc<InferenceContext> {
        &self.parent.inference_context
    }

    fn deferred_attr_context(&self) -> &Rc<DeferredAttrContext> {
        &self.parent
    }
}

/// Java: Type.containsAny(deferredAttrContext.parent.inferenceContext.inferencevars,
/// List.from(stuckVars()))
fn contains_any_inference_vars(
    ctx: &mut CompilerContext,
    syms: &mut Symtab,
    types: &Types,
    inference_context: &InferenceContext,
    ts: &[TypeId],
) -> bool {
    let inference_vars = inference_context.inference_vars();
    for t in ts {
        let mut vars = inference_vars.clone();
        while vars.non_empty() {
            if types.contains(ctx, syms, *t, *vars.head()) {
                return true;
            }
            vars = vars.tail();
        }
    }
    false
}

/// Java: DeferredAttr.emptyDeferredAttrContext
pub fn empty_deferred_attr_context() -> Rc<DeferredAttrContext> {
    Rc::new(DeferredAttrContext::empty())
}

/// Java: Type check(ResultInfo resultInfo) (DeferredAttr.java:333-343)
pub fn deferred_type_check(
    ctx: &mut CompilerContext,
    syms: &mut Symtab,
    types: &Types,
    attr: &mut Attr,
    t: TypeId,
    result_info: &ResultInfo,
) -> Result<TypeId, DeferredAttrFailure> {
    let deferred_attr_context = Rc::clone(result_info.check_context.deferred_attr_context());
    let pt = result_info
        .pt
        .unwrap_or_else(|| crate::code::symtab::no_type_id(ctx));
    let policy: Rc<dyn DeferredStuckPolicy> = if ctx.typ(pt).has_tag(TypeTag::NONE)
        || types.is_erroneous_type(ctx, syms, pt)
    {
        Rc::new(DummyStuckPolicy)
    } else if deferred_attr_context.mode == AttrMode::SPECULATIVE
        || deferred_attr_context.inside_overload_phase()
    {
        Rc::new(overload_stuck_policy_new(ctx, syms, types, result_info, t))
    } else {
        Rc::new(check_stuck_policy_new(ctx, syms, types, result_info, t))
    };
    check_with_policy(
        ctx,
        syms,
        types,
        attr,
        t,
        result_info,
        &deferred_attr_context,
        &policy,
    )
}

/// Java: private Type check(ResultInfo resultInfo, DeferredStuckPolicy
/// deferredStuckPolicy) (DeferredAttr.java:346-369)
#[allow(clippy::collapsible_if, clippy::too_many_arguments)]
fn check_with_policy(
    ctx: &mut CompilerContext,
    syms: &mut Symtab,
    types: &Types,
    attr: &mut Attr,
    t: TypeId,
    result_info: &ResultInfo,
    deferred_attr_context: &Rc<DeferredAttrContext>,
    policy: &Rc<dyn DeferredStuckPolicy>,
) -> Result<TypeId, DeferredAttrFailure> {
    let _ = types;
    assert!(
        !deferred_attr_context.empty,
        "AssertionError: empty deferred context"
    );
    if policy.is_stuck() {
        deferred_attr_context.add_deferred_attr_node(Rc::new(DeferredAttrNode {
            dt: t,
            result_info: Rc::new(result_info.clone()),
            deferred_stuck_policy: Rc::clone(policy),
        }));
        if deferred_attr_context.mode == AttrMode::SPECULATIVE {
            let mut not_pertinent = match &ctx.typ(t).kind {
                TypeKindData::Deferred { not_pertinent, .. } => not_pertinent.clone(),
                _ => panic!("ClassCastException: not a DeferredType"),
            };
            if let Some(msym) = deferred_attr_context.msym {
                if !not_pertinent.contains(&msym) {
                    not_pertinent.push(msym);
                }
            }
            if let TypeKindData::Deferred {
                not_pertinent: slot,
                mode: mode_slot,
                ..
            } = &mut ctx.typ_mut(t).kind
            {
                *slot = not_pertinent;
                *mode_slot = Some(AttrMode::SPECULATIVE);
            }
        }
        Ok(crate::code::symtab::no_type_id(ctx))
    } else {
        let result = deferred_type_complete(
            ctx,
            syms,
            attr,
            t,
            result_info,
            deferred_attr_context,
        )?;
        if let TypeKindData::Deferred { mode: slot, .. } = &mut ctx.typ_mut(t).kind {
            *slot = Some(deferred_attr_context.mode);
        }
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// DeferredTypeMap / RecoveryDeferredTypeMap (DeferredAttr.java:998-1120)
// ---------------------------------------------------------------------------

/// Java: class DeferredAttr.DeferredTypeMap<T> extends StructuralTypeMapping<T>
///
/// PORTING_RULES: the upstream class extends the Type.StructuralTypeMapping
/// visitor (not ported as a trait); the port reproduces the visitor's dispatch
/// for the type variants (identity for non-deferred types, structural recursion
/// for class/array/wildcard/method/forall, visitTypeVar for captured types)
/// (§9 adapter registered).
pub struct DeferredTypeMap<T> {
    pub deferred_attr_context: DeferredAttrContext,
    pub marker: std::marker::PhantomData<T>,
}

impl<T> DeferredTypeMap<T> {
    /// Java: protected DeferredTypeMap(AttrMode mode, Symbol msym,
    /// MethodResolutionPhase phase)
    pub fn new(mode: AttrMode, msym: Option<SymbolId>, phase: MethodResolutionPhase) -> DeferredTypeMap<T> {
        DeferredTypeMap {
            deferred_attr_context: DeferredAttrContext::new(
                mode,
                msym,
                phase,
                crate::comp::infer::empty_context(),
                Some(crate::comp::deferred_attr::empty_deferred_attr_context()),
                None,
            ),
            marker: std::marker::PhantomData,
        }
    }

    /// Java: public Type visitType(Type t, T p)
    pub fn visit(&mut self, ctx: &mut CompilerContext, t: TypeId, p: &T) -> TypeId {
        if !ctx.typ(t).has_tag(TypeTag::DEFERRED) {
            self.visit_structural(ctx, t, p)
        } else {
            self.type_of(ctx, t, p)
                .unwrap_or_else(|| crate::code::symtab::no_type_id(ctx))
        }
    }

    /// Java: StructuralTypeMapping's structural variants (only the nodes that
    /// can contain a deferred type need to be re-created; the traversal order
    /// follows Type.StructuralTypeMapping).
    fn visit_structural(&mut self, ctx: &mut CompilerContext, t: TypeId, p: &T) -> TypeId {
        let kind = ctx.typ(t).kind.clone();
        let tsym_opt = ctx.typ(t).tsym;
        let tag_opt = ctx.typ(t).tag;
        match &kind {
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => {
                let outer = match &kind {
                    TypeKindData::ClassType { outer_field, .. }
                    | TypeKindData::ErasedClassType { outer_field, .. }
                    | TypeKindData::UnionClassType { outer_field, .. }
                    | TypeKindData::IntersectionClassType { outer_field, .. } => *outer_field,
                    _ => None,
                };
                let typarams = match &kind {
                    TypeKindData::ClassType { typarams_field, .. }
                    | TypeKindData::ErasedClassType { typarams_field, .. }
                    | TypeKindData::UnionClassType { typarams_field, .. }
                    | TypeKindData::IntersectionClassType { typarams_field, .. } => {
                        typarams_field.clone().unwrap_or_else(List::nil)
                    }
                    _ => List::nil(),
                };
                let outer1 = outer.map(|outer| self.visit(ctx, outer, p));
                let typarams1 = self.map(ctx, &typarams, p);
                if list_same_types(&typarams, &typarams1) {
                    t
                } else {
                    let tsym = tsym_opt;
                    let mut items: Vec<TypeId> = Vec::new();
                    for item in typarams1.iter() {
                        items.push(item);
                    }
                    let mut list: List<TypeId> = List::nil();
                    for item in items.into_iter().rev() {
                        list = list.prepend(item);
                    }
                    ctx.alloc_type(crate::code::type_::Type::new(
                        tag_opt,
                        tsym,
                        TypeKindData::ClassType {
                            outer_field: outer1,
                            typarams_field: Some(list),
                            allparams_field: None,
                            supertype_field: None,
                            interfaces_field: None,
                            all_interfaces_field: None,
                        },
                    ))
                }
            }
            TypeKindData::ArrayType { elemtype, .. } => {
                let elem = match elemtype {
                    Some(e) => *e,
                    None => return t,
                };
                let elem1 = self.visit(ctx, elem, p);
                if elem1 == elem {
                    t
                } else {
                    let tsym = tsym_opt;
                    ctx.alloc_type(crate::code::type_::Type::new(
                        TypeTag::ARRAY,
                        tsym,
                        TypeKindData::ArrayType {
                            elemtype: Some(elem1),
                varargs: false,
                        },
                    ))
                }
            }
            TypeKindData::WildcardType { type_, kind, bound } => {
                let Some(type_) = *type_ else {
                    return t;
                };
                let type1 = self.visit(ctx, type_, p);
                if type1 == type_ {
                    t
                } else {
                    let tsym = tsym_opt;
                    ctx.alloc_type(crate::code::type_::Type::new(
                        TypeTag::WILDCARD,
                        tsym,
                        TypeKindData::WildcardType {
                            type_: Some(type1),
                            kind: *kind,
                            bound: *bound,
                        },
                    ))
                }
            }
            TypeKindData::MethodType {
                argtypes,
                restype,
                thrown,
                ..
            } => {
                let argtypes1 = self.map(ctx, argtypes, p);
                let restype1 = restype.map(|r| self.visit(ctx, r, p));
                let thrown1 = self.map(ctx, thrown, p);
                if list_same_types(argtypes, &argtypes1)
                    && restype1 == *restype
                    && list_same_types(thrown, &thrown1)
                {
                    t
                } else {
                    let tsym = tsym_opt;
                    ctx.alloc_type(crate::code::type_::Type::new(
                        TypeTag::METHOD,
                        tsym,
                        TypeKindData::MethodType {
                            argtypes: argtypes1,
                            restype: restype1,
                            thrown: thrown1,
                            recvtype: None,
                        },
                    ))
                }
            }
            TypeKindData::ForAll { tvars, qtype } => {
                let Some(qtype) = *qtype else {
                    return t;
                };
                let qtype1 = self.visit(ctx, qtype, p);
                if qtype1 == qtype {
                    t
                } else {
                    let tsym = tsym_opt;
                    ctx.alloc_type(crate::code::type_::Type::new(
                        TypeTag::FORALL,
                        tsym,
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

    /// Java: List<Type> map(List<Type> ts, T p) (TypeMapping.visit(List))
    pub fn map(&mut self, ctx: &mut CompilerContext, ts: &List<TypeId>, p: &T) -> List<TypeId> {
        let mut result: List<TypeId> = List::nil();
        for t in ts.iter() {
            result = result.append(self.visit(ctx, t, p));
        }
        result
    }

    /// Java: protected Type typeOf(DeferredType dt, T p)
    pub fn type_of(&mut self, ctx: &mut CompilerContext, t: TypeId, _p: &T) -> Option<TypeId> {
        let (tree, _, _, _, _, _) = deferred_fields(ctx, t);
        match self.deferred_attr_context.mode {
            AttrMode::CHECK => ctx.tree(tree).ty,
            AttrMode::SPECULATIVE => deferred_type_speculative_type(
                ctx,
                t,
                self.deferred_attr_context.msym,
                self.deferred_attr_context.phase,
            ),
        }
    }
}

fn list_same_types(a: &List<TypeId>, b: &List<TypeId>) -> bool {
    if a.length() != b.length() {
        return false;
    }
    let mut a_cur = a.clone();
    let mut b_cur = b.clone();
    while a_cur.non_empty() && b_cur.non_empty() {
        if a_cur.head() != b_cur.head() {
            return false;
        }
        a_cur = a_cur.tail();
        b_cur = b_cur.tail();
    }
    true
}

/// Java: public class DeferredAttr.RecoveryDeferredTypeMap extends
/// DeferredTypeMap<Type>
pub struct RecoveryDeferredTypeMap {
    pub base: DeferredTypeMap<Option<TypeId>>,
}

impl RecoveryDeferredTypeMap {
    /// Java: public RecoveryDeferredTypeMap(AttrMode mode, Symbol msym,
    /// MethodResolutionPhase phase)
    pub fn new(
        mode: AttrMode,
        msym: Option<SymbolId>,
        phase: MethodResolutionPhase,
    ) -> RecoveryDeferredTypeMap {
        RecoveryDeferredTypeMap {
            base: DeferredTypeMap::new(mode, msym, phase),
        }
    }

    /// Java: public Type visitType(Type t, Type pt)
    pub fn visit(&mut self, ctx: &mut CompilerContext, t: TypeId, pt: &Option<TypeId>) -> TypeId {
        self.base.visit(ctx, t, pt)
    }

    /// Java: protected Type typeOf(DeferredType dt, Type pt)
    pub fn type_of(&mut self, ctx: &mut CompilerContext, t: TypeId, pt: &Option<TypeId>) -> Option<TypeId> {
        let owntype = self.base.type_of(ctx, t, pt);
        match owntype {
            Some(ty) if ty != crate::code::symtab::no_type_id(ctx) => Some(ty),
            _ => {
                // Java: recover(dt, pt) — registered (Attr/Check batch)
                unimplemented!(
                    "TODO(Attr/Check batch): RecoveryDeferredTypeMap.recover (DeferredAttr.java:1073-1085)"
                )
            }
        }
    }

    /// Java: public Type visitMethodType(Type.MethodType t, Type pt)
    #[allow(clippy::collapsible_if)]
    pub fn visit_method_type(
        &mut self,
        ctx: &mut CompilerContext,
        t: TypeId,
        pt: &Option<TypeId>,
    ) -> TypeId {
        if ctx.typ(t).has_tag(TypeTag::METHOD)
            && self.base.deferred_attr_context.mode == AttrMode::CHECK
        {
            if let Some(msym) = self.base.deferred_attr_context.msym {
                let mtype = ctx.sym(msym).ty;
                let mtype = mtype.and_then(|mtype| {
                    if ctx.typ(mtype).has_tag(TypeTag::ERROR) {
                        match &ctx.typ(mtype).kind {
                            TypeKindData::ErrorType { original_type, .. } => *original_type,
                            _ => None,
                        }
                    } else {
                        Some(mtype)
                    }
                });
                if let Some(mtype) = mtype {
                    if ctx.typ(mtype).has_tag(TypeTag::METHOD) {
                        let (argtypes, restype, thrown) = match &ctx.typ(t).kind {
                            TypeKindData::MethodType {
                                argtypes,
                                restype,
                                thrown,
                                ..
                            } => (argtypes.clone(), *restype, thrown.clone()),
                            _ => unreachable!(),
                        };
                        let m_argtypes = match &ctx.typ(mtype).kind {
                            TypeKindData::MethodType { argtypes, .. } => argtypes.clone(),
                            _ => unreachable!(),
                        };
                        let m_restype = match &ctx.typ(mtype).kind {
                            TypeKindData::MethodType { restype, .. } => *restype,
                            _ => unreachable!(),
                        };
                        let m_thrown = match &ctx.typ(mtype).kind {
                            TypeKindData::MethodType { thrown, .. } => thrown.clone(),
                            _ => unreachable!(),
                        };
                        let argtypes1 = self.map_paired(ctx, &argtypes, &m_argtypes);
                        let restype1 = restype.map(|r| self.visit(ctx, r, &m_restype));
                        let thrown1 = self.map_paired(ctx, &thrown, &m_thrown);
                        if list_same_types(&argtypes, &argtypes1)
                            && restype1 == restype
                            && list_same_types(&thrown, &thrown1)
                        {
                            return t;
                        }
                        let tsym = ctx.typ(t).tsym;
                        return ctx.alloc_type(crate::code::type_::Type::new(
                            TypeTag::METHOD,
                            tsym,
                            TypeKindData::MethodType {
                                argtypes: argtypes1,
                                restype: restype1,
                                thrown: thrown1,
                                recvtype: None,
                            },
                        ));
                    }
                }
            }
        }
        self.base.visit(ctx, t, pt)
    }

    fn map_paired(
        &mut self,
        ctx: &mut CompilerContext,
        ts: &List<TypeId>,
        ps: &List<TypeId>,
    ) -> List<TypeId> {
        let mut result: List<TypeId> = List::nil();
        let mut ts_cur = ts.clone();
        let mut ps_cur = ps.clone();
        while ts_cur.non_empty() {
            let p = if ps_cur.non_empty() {
                Some(*ps_cur.head())
            } else {
                None
            };
            result = result.append(self.visit(ctx, *ts_cur.head(), &p));
            ts_cur = ts_cur.tail();
            if ps_cur.non_empty() {
                ps_cur = ps_cur.tail();
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// FilterScanner / PolyScanner / LambdaReturnScanner / SwitchExpressionScanner /
// CheckStuckPolicy / OverloadStuckPolicy (DeferredAttr.java:1050-1330)
// ---------------------------------------------------------------------------

/// Java: abstract static class FilterScanner extends TreeScanner
///
/// PORTING_RULES: the upstream subclass overrides scan(JCTree) to visit only
/// the nodes whose tag is in validTags; the port reproduces the same dispatch
/// with explicit tag matching (TreeScanner has no virtual scan hook in the
/// port) — §9 adapter registered. The traversal order follows the upstream
/// TreeScanner methods for the visited tags.
struct FilterScanner;

impl FilterScanner {
    /// Java: public void scan(JCTree tree)
    fn scan(
        ctx: &mut CompilerContext,
        tree: Option<TreeId>,
        policy: &mut CheckStuckPolicy,
        types: &Types,
    ) {
        let Some(tree) = tree else {
            return;
        };
        match &ctx.tree(tree).tag {
            Tag::CONDEXPR => {
                let (truepart, falsepart) = match &ctx.tree(tree).kind {
                    TreeKind::Conditional {
                        truepart, falsepart, ..
                    } => (*truepart, *falsepart),
                    _ => panic!("expected a conditional"),
                };
                Self::scan(ctx, truepart, policy, types);
                Self::scan(ctx, falsepart, policy, types);
            }
            Tag::PARENS => {
                let expr = match &ctx.tree(tree).kind {
                    TreeKind::Parens { expr } => *expr,
                    _ => panic!("expected parens"),
                };
                Self::scan(ctx, expr, policy, types);
            }
            Tag::LAMBDA => policy.visit_lambda(ctx, tree, types),
            Tag::REFERENCE => policy.visit_reference(ctx, tree, types),
            Tag::SWITCH_EXPRESSION => policy.visit_switch_expression(ctx, tree, types),
            _ => {
                // Java: void skip(JCTree tree) {} — the node is discarded.
            }
        }
    }

    /// LambdaReturnScanner's statement traversal (DeferredAttr.java:1100-1105)
    #[allow(dead_code)]
    fn scan_lambda_return(
        ctx: &mut CompilerContext,
        tree: Option<TreeId>,
        policy: &mut CheckStuckPolicy,
        types: &Types,
        pt: Option<TypeId>,
    ) {
        let Some(tree) = tree else {
            return;
        };
        match &ctx.tree(tree).tag {
            Tag::BLOCK => {
                if let TreeKind::Block { stats, .. } = &ctx.tree(tree).kind {
                    let stats = stats.clone();
                    for stat in stats.iter() {
                        Self::scan_lambda_return(ctx, Some(stat), policy, types, pt);
                    }
                }
            }
            Tag::IF => {
                let (thenpart, elsepart) = match &ctx.tree(tree).kind {
                    TreeKind::If {
                        thenpart, elsepart, ..
                    } => (*thenpart, *elsepart),
                    _ => panic!("expected an if"),
                };
                Self::scan_lambda_return(ctx, thenpart, policy, types, pt);
                Self::scan_lambda_return(ctx, elsepart, policy, types, pt);
            }
            Tag::RETURN => {
                let expr = match &ctx.tree(tree).kind {
                    TreeKind::Return { expr } => *expr,
                    _ => panic!("expected a return"),
                };
                if let Some(expr) = expr {
                    let prev_pt = policy.pt;
                    policy.pt = pt;
                    Self::scan(ctx, Some(expr), policy, types);
                    policy.pt = prev_pt;
                }
            }
            Tag::DOLOOP => {
                if let TreeKind::DoWhileLoop { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_lambda_return(ctx, body, policy, types, pt);
                }
            }
            Tag::WHILELOOP => {
                if let TreeKind::WhileLoop { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_lambda_return(ctx, body, policy, types, pt);
                }
            }
            Tag::FORLOOP => {
                if let TreeKind::ForLoop { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_lambda_return(ctx, body, policy, types, pt);
                }
            }
            Tag::FOREACHLOOP => {
                if let TreeKind::EnhancedForLoop { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_lambda_return(ctx, body, policy, types, pt);
                }
            }
            Tag::SWITCH => {
                if let TreeKind::Switch { cases, .. } = &ctx.tree(tree).kind {
                    let cases = cases.clone();
                    for case in cases.iter() {
                        Self::scan_lambda_return(ctx, Some(case), policy, types, pt);
                    }
                }
            }
            Tag::CASE => {
                if let TreeKind::Case { stats, body, .. } = &ctx.tree(tree).kind {
                    let stats = stats.clone();
                    let body = *body;
                    for stat in stats.iter() {
                        Self::scan_lambda_return(ctx, Some(stat), policy, types, pt);
                    }
                    Self::scan_lambda_return(ctx, body, policy, types, pt);
                }
            }
            Tag::TRY => {
                let (body, catchers, finalizer) = match &ctx.tree(tree).kind {
                    TreeKind::Try {
                        body,
                        catchers,
                        finalizer,
                        ..
                    } => (*body, catchers.clone(), *finalizer),
                    _ => panic!("expected a try"),
                };
                Self::scan_lambda_return(ctx, body, policy, types, pt);
                for catcher in catchers.iter() {
                    Self::scan_lambda_return(ctx, Some(catcher), policy, types, pt);
                }
                Self::scan_lambda_return(ctx, finalizer, policy, types, pt);
            }
            Tag::CATCH => {
                if let TreeKind::Catch { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_lambda_return(ctx, body, policy, types, pt);
                }
            }
            Tag::SYNCHRONIZED => {
                if let TreeKind::Synchronized { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_lambda_return(ctx, body, policy, types, pt);
                }
            }
            _ => {}
        }
    }

    /// SwitchExpressionScanner's statement traversal (DeferredAttr.java:1112-1117)
    fn scan_switch_expression(
        ctx: &mut CompilerContext,
        tree: Option<TreeId>,
        policy: &mut CheckStuckPolicy,
        types: &Types,
        pt: Option<TypeId>,
    ) {
        let Some(tree) = tree else {
            return;
        };
        match &ctx.tree(tree).tag {
            Tag::YIELD => {
                let value = match &ctx.tree(tree).kind {
                    TreeKind::Yield { value, .. } => *value,
                    _ => panic!("expected a yield"),
                };
                if let Some(value) = value {
                    let prev_pt = policy.pt;
                    policy.pt = pt;
                    FilterScanner::scan(ctx, Some(value), policy, types);
                    policy.pt = prev_pt;
                }
            }
            Tag::BLOCK => {
                if let TreeKind::Block { stats, .. } = &ctx.tree(tree).kind {
                    let stats = stats.clone();
                    for stat in stats.iter() {
                        Self::scan_switch_expression(ctx, Some(stat), policy, types, pt);
                    }
                }
            }
            Tag::CASE => {
                if let TreeKind::Case { stats, body, .. } = &ctx.tree(tree).kind {
                    let stats = stats.clone();
                    let body = *body;
                    for stat in stats.iter() {
                        Self::scan_switch_expression(ctx, Some(stat), policy, types, pt);
                    }
                    Self::scan_switch_expression(ctx, body, policy, types, pt);
                }
            }
            Tag::IF => {
                let (thenpart, elsepart) = match &ctx.tree(tree).kind {
                    TreeKind::If {
                        thenpart, elsepart, ..
                    } => (*thenpart, *elsepart),
                    _ => panic!("expected an if"),
                };
                Self::scan_switch_expression(ctx, thenpart, policy, types, pt);
                Self::scan_switch_expression(ctx, elsepart, policy, types, pt);
            }
            Tag::DOLOOP => {
                if let TreeKind::DoWhileLoop { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_switch_expression(ctx, body, policy, types, pt);
                }
            }
            Tag::WHILELOOP => {
                if let TreeKind::WhileLoop { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_switch_expression(ctx, body, policy, types, pt);
                }
            }
            Tag::FORLOOP => {
                if let TreeKind::ForLoop { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_switch_expression(ctx, body, policy, types, pt);
                }
            }
            Tag::FOREACHLOOP => {
                if let TreeKind::EnhancedForLoop { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_switch_expression(ctx, body, policy, types, pt);
                }
            }
            Tag::SWITCH => {
                if let TreeKind::Switch { cases, .. } = &ctx.tree(tree).kind {
                    let cases = cases.clone();
                    for case in cases.iter() {
                        Self::scan_switch_expression(ctx, Some(case), policy, types, pt);
                    }
                }
            }
            Tag::TRY => {
                let (body, catchers, finalizer) = match &ctx.tree(tree).kind {
                    TreeKind::Try {
                        body,
                        catchers,
                        finalizer,
                        ..
                    } => (*body, catchers.clone(), *finalizer),
                    _ => panic!("expected a try"),
                };
                Self::scan_switch_expression(ctx, body, policy, types, pt);
                for catcher in catchers.iter() {
                    Self::scan_switch_expression(ctx, Some(catcher), policy, types, pt);
                }
                Self::scan_switch_expression(ctx, finalizer, policy, types, pt);
            }
            Tag::CATCH => {
                if let TreeKind::Catch { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_switch_expression(ctx, body, policy, types, pt);
                }
            }
            Tag::SYNCHRONIZED => {
                if let TreeKind::Synchronized { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_switch_expression(ctx, body, policy, types, pt);
                }
            }
            _ => {}
        }
    }
}

/// Java: static class PolyScanner extends FilterScanner
pub struct PolyScanner;

/// Java: static class LambdaReturnScanner extends FilterScanner
pub struct LambdaReturnScanner;

/// Java: static class SwitchExpressionScanner extends FilterScanner
pub struct SwitchExpressionScanner;

/// Java: class CheckStuckPolicy extends PolyScanner implements
/// DeferredStuckPolicy, Infer.FreeTypeListener
pub struct CheckStuckPolicy {
    /// Java: Type pt
    pub pt: Option<TypeId>,
    /// Java: InferenceContext inferenceContext
    pub inference_context: Rc<InferenceContext>,
    /// Java: Set<Type> stuckVars (shared with the listener; linked-set order)
    pub stuck_vars: Rc<RefCell<Vec<TypeId>>>,
    /// Java: Set<Type> depVars
    pub dep_vars: Rc<RefCell<Vec<TypeId>>>,
}

impl CheckStuckPolicy {
    /// Java: public CheckStuckPolicy(ResultInfo resultInfo, DeferredType dt)
    pub fn new(
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        types: &Types,
        result_info: &ResultInfo,
        dt: TypeId,
    ) -> CheckStuckPolicy {
        let mut policy = CheckStuckPolicy {
            pt: result_info.pt,
            inference_context: Rc::clone(result_info.check_context.inference_context()),
            stuck_vars: Rc::new(RefCell::new(Vec::new())),
            dep_vars: Rc::new(RefCell::new(Vec::new())),
        };
        let (tree, _, _, _, _, _) = deferred_fields(ctx, dt);
        FilterScanner::scan(ctx, Some(tree), &mut policy, types);
        if !policy.stuck_vars.borrow().is_empty() {
            let stuck_vars = policy.stuck_vars.borrow().clone();
            let listener = Rc::new(CheckStuckPolicyListener {
                stuck_vars: Rc::clone(&policy.stuck_vars),
            });
            let context = Rc::clone(&policy.inference_context);
            context.add_free_type_listener(
                ctx,
                &*syms,
                types,
                List::from_iterable(stuck_vars),
                listener,
            );
        }
        policy
    }

    /// Java: public void scan(JCTree tree) (FilterScanner.scan)
    pub fn scan(&mut self, ctx: &mut CompilerContext, types: &Types, tree: Option<TreeId>) {
        FilterScanner::scan(ctx, tree, self, types);
    }

    /// Java: public void visitLambda(JCLambda tree) — registered: needs
    /// Types.isFunctionalInterface/findDescriptorType (FunctionDescriptor batch).
    fn visit_lambda(&mut self, _ctx: &mut CompilerContext, _tree: TreeId, _types: &Types) {
        unimplemented!(
            "TODO(FunctionDescriptor batch): CheckStuckPolicy.visitLambda (DeferredAttr.java:1195-1220)"
        )
    }

    /// Java: public void visitReference(JCMemberReference tree) — registered:
    /// needs Types.isFunctionalInterface/findDescriptorType and the
    /// JCMemberReference.OverloadKind state (Resolve B/FunctionDescriptor batch).
    fn visit_reference(&mut self, _ctx: &mut CompilerContext, _tree: TreeId, _types: &Types) {
        unimplemented!(
            "TODO(FunctionDescriptor/Resolve B batch): CheckStuckPolicy.visitReference (DeferredAttr.java:1222-1245)"
        )
    }

    /// Java: void scanLambdaBody(JCLambda lambda, final Type pt)
    /// (DeferredAttr.java:1247-1270)
    #[allow(dead_code)]
    fn scan_lambda_body(
        &mut self,
        ctx: &mut CompilerContext,
        lambda: TreeId,
        types: &Types,
        lambda_pt: Option<TypeId>,
    ) {
        let (body, _param_kind) = match &ctx.tree(lambda).kind {
            TreeKind::Lambda { body, param_kind, .. } => (*body, *param_kind),
            _ => panic!("expected a lambda"),
        };
        let Some(body) = body else {
            return;
        };
        if !ctx.tree(body).has_tag(Tag::BLOCK) {
            // Java: BodyKind.EXPRESSION
            let prev_pt = self.pt;
            self.pt = lambda_pt;
            self.scan(ctx, types, Some(body));
            self.pt = prev_pt;
        } else {
            FilterScanner::scan_lambda_return(ctx, Some(body), self, types, lambda_pt);
        }
    }

    /// Java: public void visitSwitchExpression(JCSwitchExpression expr)
    /// (DeferredAttr.java:1272-1285)
    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId, types: &Types) {
        let cases = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression { cases, .. } => cases.clone(),
            _ => panic!("expected a switch expression"),
        };
        let pt = self.pt;
        for case in cases.iter() {
            FilterScanner::scan_switch_expression(ctx, Some(case), self, types, pt);
        }
    }
}

impl DeferredStuckPolicy for CheckStuckPolicy {
    fn is_stuck(&self) -> bool {
        !self.stuck_vars.borrow().is_empty()
    }

    fn stuck_vars(&self) -> Vec<TypeId> {
        self.stuck_vars.borrow().clone()
    }

    fn dep_vars(&self) -> Vec<TypeId> {
        self.dep_vars.borrow().clone()
    }
}

/// Java: CheckStuckPolicy.typesInferred(InferenceContext) (the policy is
/// registered as a FreeTypeListener; the shared stuck-vars set is cleared)
struct CheckStuckPolicyListener {
    stuck_vars: Rc<RefCell<Vec<TypeId>>>,
}

impl crate::comp::infer::FreeTypeListener for CheckStuckPolicyListener {
    fn types_inferred(
        &self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _types: &Types,
        _inference_context: &InferenceContext,
    ) -> Result<(), InferenceException> {
        self.stuck_vars.borrow_mut().clear();
        Ok(())
    }
}

/// Java: CheckStuckPolicy constructor wrapper used by DeferredType.check
/// (the upstream new CheckStuckPolicy(resultInfo, dt)).
pub fn check_stuck_policy_new(
    ctx: &mut CompilerContext,
    syms: &mut Symtab,
    types: &Types,
    result_info: &ResultInfo,
    dt: TypeId,
) -> CheckStuckPolicy {
    CheckStuckPolicy::new(ctx, syms, types, result_info, dt)
}

/// Java: class OverloadStuckPolicy extends CheckStuckPolicy implements
/// DeferredStuckPolicy
///
/// PORTING_RULES: the upstream constructor's scan dispatches to the overridden
/// visitLambda/visitReference (Java virtual dispatch). The ported
/// visitLambda/visitReference are registered as unimplemented (FunctionDescriptor
/// batch), so the overload-specific stuck flag is only produced by those two
/// methods; this difference is registered together with that dependency.
pub struct OverloadStuckPolicy {
    /// Java: the inherited CheckStuckPolicy state
    pub base: CheckStuckPolicy,
    /// Java: boolean stuck
    pub stuck: bool,
}

impl OverloadStuckPolicy {
    /// Java: public OverloadStuckPolicy(ResultInfo resultInfo, DeferredType dt)
    pub fn new(
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        types: &Types,
        result_info: &ResultInfo,
        dt: TypeId,
    ) -> OverloadStuckPolicy {
        let base = CheckStuckPolicy::new(ctx, syms, types, result_info, dt);
        OverloadStuckPolicy { base, stuck: false }
    }
}

impl DeferredStuckPolicy for OverloadStuckPolicy {
    fn is_stuck(&self) -> bool {
        self.base.is_stuck() || self.stuck
    }

    fn stuck_vars(&self) -> Vec<TypeId> {
        self.base.stuck_vars()
    }

    fn dep_vars(&self) -> Vec<TypeId> {
        self.base.dep_vars()
    }
}

/// Java: OverloadStuckPolicy constructor wrapper used by DeferredType.check.
pub fn overload_stuck_policy_new(
    ctx: &mut CompilerContext,
    syms: &mut Symtab,
    types: &Types,
    result_info: &ResultInfo,
    dt: TypeId,
) -> OverloadStuckPolicy {
    OverloadStuckPolicy::new(ctx, syms, types, result_info, dt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comp::check::CheckContext;
    use crate::comp::infer::empty_context;
    use crate::ids::TreeId;

    /// A CheckContext that only supplies the inference/deferred contexts needed
    /// by the SpeculativeCache tests.
    struct DummyCheckContext {
        deferred: Rc<DeferredAttrContext>,
        inference: Rc<InferenceContext>,
    }

    impl CheckContext for DummyCheckContext {
        fn compatible(
            &self,
            _ctx: &mut CompilerContext,
            _syms: &Symtab,
            _types: &Types,
            _found: TypeId,
            _req: TypeId,
            _warn: &mut Warner,
        ) -> bool {
            true
        }

        fn report(
            &self,
            _ctx: &mut CompilerContext,
            _pos: i32,
            _details: &crate::util::jc_diagnostic::JCDiagnostic,
        ) {
        }

        fn check_warner(
            &self,
            _ctx: &mut CompilerContext,
            _syms: &Symtab,
            _types: &Types,
            _pos: i32,
            _found: TypeId,
            _req: TypeId,
        ) -> Warner {
            Warner::new(None)
        }

        fn inference_context(&self) -> &Rc<InferenceContext> {
            &self.inference
        }

        fn deferred_attr_context(&self) -> &Rc<DeferredAttrContext> {
            &self.deferred
        }
    }

    fn result_info(
        phase: MethodResolutionPhase,
        pt: Option<TypeId>,
    ) -> (Rc<ResultInfo>, Rc<DeferredAttrContext>) {
        let deferred = Rc::new(DeferredAttrContext::new(
            AttrMode::CHECK,
            None,
            phase,
            empty_context(),
            None,
            None,
        ));
        let inference = empty_context();
        let check_context = Rc::new(DummyCheckContext {
            deferred: Rc::clone(&deferred),
            inference,
        });
        let info = Rc::new(ResultInfo::new(
            crate::code::kinds::KindSelector::VAL,
            pt,
            check_context,
        ));
        (info, deferred)
    }

    #[test]
    fn attribution_mode_values_match_the_upstream_flags() {
        assert!(!AttributionMode::FULL.is_speculative());
        assert!(AttributionMode::FULL.recover());
        assert!(AttributionMode::ATTRIB_TO_TREE.is_speculative());
        assert!(AttributionMode::ATTRIB_TO_TREE.recover());
        assert!(AttributionMode::ANALYZER.is_speculative());
        assert!(!AttributionMode::ANALYZER.recover());
        assert!(AttributionMode::SPECULATIVE.is_speculative());
        assert!(!AttributionMode::SPECULATIVE.recover());
    }

    #[test]
    fn empty_deferred_attr_context_keeps_the_upstream_defaults() {
        let context = DeferredAttrContext::empty();
        assert!(context.empty);
        assert_eq!(context.mode, AttrMode::CHECK);
        assert_eq!(context.phase, MethodResolutionPhase::BOX);
        assert_eq!(context.msym, None);
        assert!(!context.inside_overload_phase());
    }

    #[test]
    fn inside_overload_phase_walks_the_parent_chain() {
        let check = Rc::new(DeferredAttrContext::new(
            AttrMode::CHECK,
            None,
            MethodResolutionPhase::BOX,
            empty_context(),
            Some(Rc::new(DeferredAttrContext::empty())),
            None,
        ));
        assert!(!check.inside_overload_phase());
        let speculative = Rc::new(DeferredAttrContext::new(
            AttrMode::SPECULATIVE,
            None,
            MethodResolutionPhase::BOX,
            empty_context(),
            Some(Rc::clone(&check)),
            None,
        ));
        assert!(speculative.inside_overload_phase());
        let child = Rc::new(DeferredAttrContext::new(
            AttrMode::CHECK,
            None,
            MethodResolutionPhase::BOX,
            empty_context(),
            Some(Rc::clone(&check)),
            None,
        ));
        assert!(!child.inside_overload_phase());
        let child_of_speculative = Rc::new(DeferredAttrContext::new(
            AttrMode::CHECK,
            None,
            MethodResolutionPhase::BOX,
            empty_context(),
            Some(Rc::clone(&speculative)),
            None,
        ));
        assert!(child_of_speculative.inside_overload_phase());
        let child_of_empty = Rc::new(DeferredAttrContext::new(
            AttrMode::CHECK,
            None,
            MethodResolutionPhase::BOX,
            empty_context(),
            Some(Rc::new(DeferredAttrContext::empty())),
            None,
        ));
        assert!(!child_of_empty.inside_overload_phase());
    }

    #[test]
    #[should_panic(expected = "AssertionError: Empty deferred context!")]
    fn empty_context_rejects_add_deferred_attr_node() {
        let context = DeferredAttrContext::empty();
        let node = Rc::new(DeferredAttrNode {
            dt: TypeId::new(0),
            result_info: result_info(MethodResolutionPhase::BOX, None).0,
            deferred_stuck_policy: Rc::new(DummyStuckPolicy),
        });
        context.add_deferred_attr_node(node);
    }

    #[test]
    fn non_empty_context_accepts_deferred_attr_nodes() {
        let context = DeferredAttrContext::new(
            AttrMode::CHECK,
            None,
            MethodResolutionPhase::BOX,
            empty_context(),
            None,
            None,
        );
        let node = Rc::new(DeferredAttrNode {
            dt: TypeId::new(0),
            result_info: result_info(MethodResolutionPhase::BOX, None).0,
            deferred_stuck_policy: Rc::new(DummyStuckPolicy),
        });
        context.add_deferred_attr_node(node);
        assert_eq!(context.deferred_attr_nodes.borrow().len(), 1);
    }

    #[test]
    fn speculative_cache_put_and_get_by_phase() {
        let (box_info, _box_context) = result_info(MethodResolutionPhase::BOX, None);
        let (vararity_info, _var_context) = result_info(MethodResolutionPhase::VARARITY, None);
        let mut cache = SpeculativeCache::default();
        let box_tree = TreeId::new(7);
        cache.put(box_tree, Rc::clone(&box_info));
        let var_tree = TreeId::new(9);
        cache.put(var_tree, Rc::clone(&vararity_info));
        assert_eq!(
            cache
                .get(None, MethodResolutionPhase::BOX)
                .expect("box entry")
                .speculative_tree,
            box_tree
        );
        assert_eq!(
            cache
                .get(None, MethodResolutionPhase::VARARITY)
                .expect("vararity entry")
                .speculative_tree,
            var_tree
        );
        assert_eq!(
            cache
                .get(None, MethodResolutionPhase::BASIC)
                .map(|entry| entry.speculative_tree),
            None
        );
    }

    /// JDK 17 internal-API oracle (tools/gen-deferred-attr-oracle/
    /// GenDeferredAttrOracle.java): DeferredAttrContext defaults / overload
    /// phase / empty-context assertion behavior, collected at development time
    /// (no JVM at test time).
    #[test]
    fn deferred_attr_oracle_matches_the_javac_internal_api() {
        let data = include_str!("../../tests/data/deferred-attr-oracle.txt");
        let mut checked = 0;
        for line in data.lines() {
            let parts: Vec<&str> = line.split(' ').collect();
            if parts.is_empty() {
                continue;
            }
            match parts[0] {
                "EMPTY" => {
                    let context = DeferredAttrContext::empty();
                    assert_eq!(format!("{:?}", context.mode), parts[2]);
                    assert_eq!(format!("{:?}", context.phase), parts[4]);
                    assert_eq!(parts[6], "null");
                    assert_eq!(context.msym, None);
                    assert_eq!(!context.inside_overload_phase(), parts[8] == "false");
                    checked += 1;
                }
                "CHECK" => {
                    let context = DeferredAttrContext::new(
                        AttrMode::CHECK,
                        None,
                        MethodResolutionPhase::BOX,
                        empty_context(),
                        None,
                        None,
                    );
                    assert_eq!(!context.inside_overload_phase(), parts[2] == "false");
                    checked += 1;
                }
                "SPECULATIVE" => {
                    let context = DeferredAttrContext::new(
                        AttrMode::SPECULATIVE,
                        None,
                        MethodResolutionPhase::BOX,
                        empty_context(),
                        None,
                        None,
                    );
                    assert_eq!(!context.inside_overload_phase(), parts[2] == "false");
                    checked += 1;
                }
                "NESTED_CHECK" => {
                    let parent = Rc::new(DeferredAttrContext::new(
                        AttrMode::CHECK,
                        None,
                        MethodResolutionPhase::BOX,
                        empty_context(),
                        None,
                        None,
                    ));
                    let context = DeferredAttrContext::new(
                        AttrMode::CHECK,
                        None,
                        MethodResolutionPhase::BOX,
                        empty_context(),
                        Some(parent),
                        None,
                    );
                    assert_eq!(!context.inside_overload_phase(), parts[2] == "false");
                    checked += 1;
                }
                _ => {}
            }
        }
        assert_eq!(checked, 4, "oracle rows");
    }
}
