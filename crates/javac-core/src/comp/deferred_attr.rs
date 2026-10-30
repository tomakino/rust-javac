// Origin: com/sun/tools/javac/comp/DeferredAttr.java @ 982d53c (jdk-17.0.19-ga)
//
// Registration mapping: AttributionMode / AttrMode / DeferredStuckPolicy /
// dummyStuckPolicy / SpeculativeCache / DeferredType operations /
// DeferredAttrContext / DeferredAttrNode / StuckNode / DeferredTypeMap /
// RecoveryDeferredTypeMap / FilterScanner / PolyScanner / LambdaReturnScanner /
// SwitchExpressionScanner / CheckStuckPolicy / OverloadStuckPolicy are ported
// here. The speculative attribution (attribSpeculative/
// attribSpeculativeLambda/DeferredAttrDiagHandler) is ported; the
// ArgumentAttr.LocalCacheContext parameter is wired (Analyzer/Attr pass
// argumentAttr.withLocalCacheContext()). The DeferredAttr service construction
// remains an ADR 0001 explicit-context REGISTERED. Historical dependency list:
// Attr.copyEnv/TreeCopier/ArgumentAttr.LocalCacheContext/Flow.analyzeLambda/
// Log.DeferredDiagnosticHandler/Annotate.Queues/DeferredCompletionFailureHandler;
// DeferredAttrNode.StructuralStuckChecker (needs Types.findDescriptorType/
// isFunctionalInterface, Attr.lambdaEnv/preFlow/unknownExprInfo/
// memberReferenceQualifierResult, ArgumentAttr, Resolve.resolveMemberReference);
// CheckStuckPolicy.visitLambda/visitReference (needs the FunctionDescriptor
//); RecoveryDeferredTypeMap.recover (needs Attr.RecoveryInfo and
// Check.checkNonVoid); DeferredAttr.CannotInfer errors live in Resolve (Resolve B
//). The DeferredAttr service construction is an ADR 0001 explicit-context
// REGISTERED.

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
use crate::util::names::Names;
use crate::tree::tree_scanner::TreeScannerVisitor;

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

/// /// Java: AttributionMode.recover() (DeferredAttr.java:1435-1450).
///
/// # Panics
///   - none.
///
/// Upstream: DeferredAttr.java:1435-1450.
///
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
    ctx: &mut CompilerContext,
    attr: &mut Attr,
    t: TypeId,
    msym: Option<SymbolId>,
    phase: MethodResolutionPhase,
) -> Option<TypeId> {
    let (_, _, _, _, cache, argument) = deferred_fields(ctx, t);
    if let Some(data) = argument {
        let argument_attr = attr
            .argument_attr
            .borrow()
            .clone()
            .expect("AssertionError: argumentAttr service must be installed");
        let argument_attr = argument_attr.borrow();
        let data = data.borrow();
        return argument_attr.argument_type_speculative_type(ctx, &data, t, msym, phase);
    }
    let entry = cache.borrow().get(msym, phase)?;
    ctx.tree(entry.speculative_tree).ty
}

/// Java: JCTree speculativeTree(DeferredAttrContext deferredAttrContext)
pub fn deferred_type_speculative_tree(
    ctx: &CompilerContext,
    attr: &mut Attr,
    t: TypeId,
    deferred_attr_context: &DeferredAttrContext,
) -> Option<TreeId> {
    let (_, _, _, _, cache, argument) = deferred_fields(ctx, t);
    if let Some(data) = argument {
        let argument_attr = attr
            .argument_attr
            .borrow()
            .clone()
            .expect("AssertionError: argumentAttr service must be installed");
        let argument_attr = argument_attr.borrow();
        let data = data.borrow();
        return argument_attr.argument_type_speculative_tree(ctx, &data, t, deferred_attr_context);
    }
    cache
        .borrow()
        .get(deferred_attr_context.msym, deferred_attr_context.phase)
        .map(|entry| entry.speculative_tree)
}

/// Java: DeferredType.complete base behavior (the ArgumentType override is in
/// deferred_type_complete below).
pub fn deferred_type_complete_base(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    attr: &mut Attr,
    t: TypeId,
    result_info: &ResultInfo,
    deferred_attr_context: &DeferredAttrContext,
) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
    match deferred_attr_context.mode {
        AttrMode::SPECULATIVE => {
            // Java: JCTree speculativeTree = attribSpeculative(tree, env, resultInfo);
            // speculativeCache.put(speculativeTree, resultInfo);
            // return speculativeTree.type;
            let (tree, env, mode, _, cache, _) = deferred_fields(ctx, t);
            // Java: Assert.check(mode == null || mode == AttrMode.SPECULATIVE);
            debug_assert!(mode.is_none() || mode == Some(AttrMode::SPECULATIVE));
            let deferred = attr
                .deferred_attr
                .borrow()
                .clone()
                .expect("AssertionError: deferredAttr service must be installed");
            let result_info_rc = Rc::new(result_info.clone());
            let speculative_tree = {
                let deferred = deferred.borrow();
                deferred
                    .attrib_speculative(ctx, syms, attr, tree, &env, result_info)
                    .map_err(|failure| match failure {
                        DeferredAttrFailure::Completion(failure) => failure,
                        other => panic!("unexpected speculative attribution failure: {:?}", other),
                    })?
            };
            cache.borrow_mut().put(speculative_tree, result_info_rc);
            Ok(ctx
                .tree(speculative_tree)
                .ty
                .unwrap_or_else(|| crate::code::symtab::no_type_id(ctx)))
        }
        AttrMode::CHECK => {
            let (tree, env, _, _, _, _) = deferred_fields(ctx, t);
            attr.attrib_tree(ctx, syms, tree, &env, result_info.clone())
        }
    }
}

/// Java: public Type complete(ResultInfo resultInfo, DeferredAttrContext
/// deferredAttrContext) (DeferredAttr.java:309-330) — dispatches the
/// ArgumentType override (ArgumentAttr.java:345-365) when present.
pub fn deferred_type_complete(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    attr: &mut Attr,
    t: TypeId,
    result_info: &ResultInfo,
    deferred_attr_context: &DeferredAttrContext,
) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
    let argument = deferred_fields(ctx, t).5;
    match argument {
        Some(data) => {
            let argument_attr = attr
                .argument_attr
                .borrow()
                .clone()
                .expect("AssertionError: argumentAttr service must be installed");
            // Port adapter (A3k): run the completion on a session clone; the
            // surrounding speculative attribution may re-enter Attr::attribTree
            // and clone the canonical service again (upstream has one Java
            // object). The ArgumentType cache is a shared Rc.
            let mut argument_attr = argument_attr.borrow().clone();
            argument_attr.argument_type_complete(
                ctx,
                syms,
                attr,
                t,
                &data,
                result_info,
                deferred_attr_context,
            )
        }
        None => deferred_type_complete_base(ctx, syms, attr, t, result_info, deferred_attr_context),
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
    /// propagated back to the caller, Lint)
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
        syms: &Symtab,
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
        syms: &Symtab,
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
        syms: &Symtab,
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
/// Tarjan translation — adapter adapter)
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
    syms: &Symtab,
    types: &Types,
    attr: &mut Attr,
    infer: &crate::comp::infer::Infer,
    node: &DeferredAttrNode,
    deferred_attr_context: &DeferredAttrContext,
) -> Result<bool, DeferredAttrFailure> {
    match deferred_attr_context.mode {
        AttrMode::SPECULATIVE => {
            if node.deferred_stuck_policy.is_stuck() {
                // Java: new StructuralStuckChecker().check(dt, resultInfo, deferredAttrContext)
                let svc_rc = attr
                    .deferred_attr
                    .borrow()
                    .clone()
                    .expect("AssertionError: deferredAttr service must be installed");
                let mut svc = svc_rc.borrow_mut();
                let result = svc
                    .structural_stuck_check(ctx, syms, attr, node.dt, &node.result_info, deferred_attr_context);
                match result {
                    Ok(()) => Ok(true),
                    Err(failure) => Err(failure),
                }
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
    syms: &Symtab,
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
    syms: &Symtab,
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
        let chk_rc = Rc::clone(&attr.chk);
        let chk = Rc::clone(&chk_rc);
        Rc::new(overload_stuck_policy_new(
            ctx,
            syms,
            types,
            &attr.diags,
            &chk,
            result_info,
            t,
        ))
    } else {
        let chk_rc = Rc::clone(&attr.chk);
        let chk = Rc::clone(&chk_rc);
        Rc::new(check_stuck_policy_new(
            ctx,
            syms,
            types,
            &attr.diags,
            &chk,
            result_info,
            t,
        ))
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
    syms: &Symtab,
    _types: &Types,
    attr: &mut Attr,
    t: TypeId,
    result_info: &ResultInfo,
    deferred_attr_context: &Rc<DeferredAttrContext>,
    policy: &Rc<dyn DeferredStuckPolicy>,
) -> Result<TypeId, DeferredAttrFailure> {
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
/// (adapter adapter registered).
/// Java: class DeferredAttr.DeferredTypeMap<T> / RecoveryDeferredTypeMap
///
/// PORTING_RULES adapter adapters:
///   - the upstream generic parameter T (Void for the plain mapping, Type for the
///     recovery mapping) is unified to Option<TypeId> (the Java nullable Type / the
///     Void payload is unused); the two concrete classes are one struct plus the
///     recovery flag, and the RecoveryDeferredTypeMap override of typeOf is the
///     flag-dispatched recovery branch in type_of;
///   - the upstream StructuralTypeMapping visitor dispatch is reproduced
///     explicitly (as before); the recovery path needs the Attr/Check services
///     explicitly (ADR 0001) through DeferredMapCtx.
pub struct DeferredTypeMap {
    pub deferred_attr_context: Rc<DeferredAttrContext>,
    /// Java: RecoveryDeferredTypeMap overrides typeOf to synthesize a type for a
    /// deferred type that has not yet been reduced (adapter flag).
    pub recovery: bool,
    /// Java: Infer.ImplicitArgType extends DeferredTypeMap<Void> and overrides
    /// visitType/visitClassType (adapter flag with no payload: the mode behavior is
    /// fixed by the upstream subclass).
    pub implicit_arg: bool,
}

/// The explicit context required by the recovery-capable mapping (adapter ADR 0001).
pub struct DeferredMapCtx<'a> {
    pub attr: &'a mut Attr,
    pub syms: &'a Symtab,
    pub types: &'a Types,
    pub chk: &'a crate::comp::check::Check,
    pub diags: &'a crate::util::jc_diagnostic::Factory,
}

impl DeferredTypeMap {
    /// Java: protected DeferredTypeMap(AttrMode mode, Symbol msym,
    /// MethodResolutionPhase phase) (DeferredAttr.java:1000-1010)
    pub fn new(mode: AttrMode, msym: Option<SymbolId>, phase: MethodResolutionPhase) -> DeferredTypeMap {
        DeferredTypeMap {
            deferred_attr_context: Rc::new(DeferredAttrContext::new(
                mode,
                msym,
                phase,
                crate::comp::infer::empty_context(),
                Some(crate::comp::deferred_attr::empty_deferred_attr_context()),
                None,
            )),
            recovery: false,
            implicit_arg: false,
        }
    }

    /// Java: new Infer.ImplicitArgType(Symbol msym, MethodResolutionPhase phase)
    /// — the mapping behavior is fixed (adapter flag).
    pub fn new_implicit_arg(_msym: Option<SymbolId>, _phase: MethodResolutionPhase) -> DeferredTypeMap {
        let mut map = DeferredTypeMap::new(AttrMode::SPECULATIVE, None, MethodResolutionPhase::BOX);
        map.implicit_arg = true;
        map
    }

    /// Java: new RecoveryDeferredTypeMap(...) — the same mapping with the
    /// recovery override enabled (adapter).
    pub fn new_recovery(
        mode: AttrMode,
        msym: Option<SymbolId>,
        phase: MethodResolutionPhase,
    ) -> DeferredTypeMap {
        let mut map = DeferredTypeMap::new(mode, msym, phase);
        map.recovery = true;
        map
    }

    /// Java: public Type visitType(Type t, Type pt)
    pub fn visit(
        &mut self,
        ctx: &mut CompilerContext,
        dctx: &mut DeferredMapCtx,
        t: TypeId,
        pt: &Option<TypeId>,
    ) -> TypeId {
        if self.implicit_arg {
            // Java: Infer.ImplicitArgType.visitType:
            // if (t.hasTag(DEFERRED)) return visit(super.visitType(t, null));
            // else if (t.hasTag(BOT)) t = boxedClass(voidType).type;
            // return t;
            // and visitClassType: return types.erasure(t) (adapter: the class-type
            // erasure branch is folded into implicit_arg_value).
            let mapped = if ctx.typ(t).has_tag(TypeTag::DEFERRED) {
                self.type_of_no_recovery(ctx, dctx.attr, t)
                    .unwrap_or_else(|| crate::code::symtab::no_type_id(ctx))
            } else {
                t
            };
            return self.implicit_arg_value(ctx, dctx, mapped);
        }
        if !ctx.typ(t).has_tag(TypeTag::DEFERRED) {
            self.visit_structural(ctx, dctx, t, pt)
        } else {
            self.type_of(ctx, dctx, t, pt)
                .unwrap_or_else(|| crate::code::symtab::no_type_id(ctx))
        }
    }

    /// Java: ImplicitArgType.visitType/visitClassType value mapping.
    fn implicit_arg_value(
        &self,
        ctx: &mut CompilerContext,
        dctx: &DeferredMapCtx,
        t: TypeId,
    ) -> TypeId {
        use crate::code::type_tag::TypeTag;
        if ctx.typ(t).has_tag(TypeTag::BOT) {
            // infer as java.lang.Void for now
            let boxed = dctx.types.boxed_class_ro(ctx, dctx.syms, dctx.syms.void_type);
            return ctx.sym(boxed).ty.expect("boxed void type");
        }
        if ctx.typ(t).get_tag() == TypeTag::CLASS {
            return dctx.types.erasure(ctx, dctx.syms, t);
        }
        t
    }

    /// Java: StructuralTypeMapping's structural variants (the traversal order
    /// follows Type.StructuralTypeMapping).
    fn visit_structural(
        &mut self,
        ctx: &mut CompilerContext,
        dctx: &mut DeferredMapCtx,
        t: TypeId,
        pt: &Option<TypeId>,
    ) -> TypeId {
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
                let outer1 = outer.map(|outer| self.visit(ctx, dctx, outer, pt));
                let typarams1 = self.map(ctx, dctx, &typarams, pt);
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
                let elem1 = self.visit(ctx, dctx, elem, pt);
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
            TypeKindData::WildcardType {
                type_,
                kind,
                bound,
                ..
            } => {
                let Some(type_) = *type_ else {
                    return t;
                };
                let type1 = self.visit(ctx, dctx, type_, pt);
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
                            bound: *bound, is_printing_bound: std::cell::Cell::new(false), },
                    ))
                }
            }
            TypeKindData::MethodType {
                argtypes,
                restype,
                thrown,
                ..
            } => {
                let argtypes1 = self.map(ctx, dctx, argtypes, pt);
                let restype1 = restype.map(|r| self.visit(ctx, dctx, r, pt));
                let thrown1 = self.map(ctx, dctx, thrown, pt);
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
                let qtype1 = self.visit(ctx, dctx, qtype, pt);
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
    pub fn map(
        &mut self,
        ctx: &mut CompilerContext,
        dctx: &mut DeferredMapCtx,
        ts: &List<TypeId>,
        pt: &Option<TypeId>,
    ) -> List<TypeId> {
        let mut result: List<TypeId> = List::nil();
        for t in ts.iter() {
            result = result.append(self.visit(ctx, dctx, t, pt));
        }
        result
    }

    /// Java: protected Type typeOf(DeferredType dt, T p)
    pub fn type_of(
        &mut self,
        ctx: &mut CompilerContext,
        dctx: &mut DeferredMapCtx,
        t: TypeId,
        pt: &Option<TypeId>,
    ) -> Option<TypeId> {
        let owntype = self.type_of_no_recovery(ctx, dctx.attr, t);
        if self.recovery
            && owntype
                .map(|ty| ty == crate::code::symtab::no_type_id(ctx))
                .unwrap_or(true)
        {
            return self.recover(ctx, dctx, t, pt);
        }
        owntype
    }

    /// Java: DeferredTypeMap.typeOf (the base implementation).
    fn type_of_no_recovery(
        &mut self,
        ctx: &mut CompilerContext,
        attr: &mut Attr,
        t: TypeId,
    ) -> Option<TypeId> {
        let (tree, _, _, _, _, _) = deferred_fields(ctx, t);
        match self.deferred_attr_context.mode {
            AttrMode::CHECK => ctx.tree(tree).ty,
            AttrMode::SPECULATIVE => deferred_type_speculative_type(
                ctx,
                attr,
                t,
                self.deferred_attr_context.msym,
                self.deferred_attr_context.phase,
            ),
        }
    }

    /// Java: private Type recover(DeferredType dt, Type pt)
    /// (DeferredAttr.java:1073-1085)
    fn recover(
        &mut self,
        ctx: &mut CompilerContext,
        dctx: &mut DeferredMapCtx,
        dt: TypeId,
        pt: &Option<TypeId>,
    ) -> Option<TypeId> {
        let tree = deferred_fields(ctx, dt).0;
        let is_lambda_or_member_ref = matches!(
            ctx.tree(tree).tag,
            crate::tree::jc_tree::Tag::REFERENCE | crate::tree::jc_tree::Tag::LAMBDA
        );
        let is_functional_interface = pt
            .map(|pt| {
                let tsym = ctx.typ(pt).tsym.expect("pt tsym");
                dctx.types
                    .is_functional_interface(ctx, dctx.syms, dctx.diags, dctx.chk, tsym)
            })
            .unwrap_or(false);
        let needs_recovery_type =
            pt.is_none() || (is_lambda_or_member_ref && !is_functional_interface);
        let pt_recovery = if needs_recovery_type {
            dctx.syms.recovery_type
        } else {
            pt.expect("pt")
        };
        // Java: dt.check(attr.new RecoveryInfo(deferredAttrContext, ptRecovery) {
        // check(pos, found) => chk.checkNonVoid(pos, super.check(pos, found)) });
        let recovery_info = {
            let mut info = crate::comp::attr::ResultInfo::new_with_mode(
                crate::code::kinds::KindSelector::VAL,
                Some(pt_recovery),
                Rc::new(crate::comp::attr::RecoveryCheckContext {
                    enclosing: Rc::clone(&dctx.chk.basic_handler),
                    deferred_attr_context: Rc::clone(&self.deferred_attr_context),
                }),
                crate::comp::check::CheckMode::NORMAL,
            );
            info.check_non_void = true;
            info
        };
        let _ = deferred_type_check(ctx, dctx.syms, dctx.types, dctx.attr, dt, &recovery_info);
        // Java: return super.visit(dt);
        let owntype = self.type_of_no_recovery(ctx, dctx.attr, dt);
        if owntype
            .map(|ty| ty == crate::code::symtab::no_type_id(ctx))
            .unwrap_or(true)
        {
            Some(crate::code::symtab::no_type_id(ctx))
        } else {
            owntype
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
/// DeferredTypeMap<Type> (DeferredAttr.java:1035-1090)
///
/// adapter: the Java subclass is the ported DeferredTypeMap with recovery=true (the
/// typeOf override is dispatched in DeferredTypeMap::type_of); the
/// visitMethodType override is reproduced here.
pub struct RecoveryDeferredTypeMap {
    pub base: DeferredTypeMap,
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
            base: DeferredTypeMap::new_recovery(mode, msym, phase),
        }
    }

    /// Java: public Type visitType(Type t, Type pt)
    pub fn visit(
        &mut self,
        ctx: &mut CompilerContext,
        dctx: &mut DeferredMapCtx,
        t: TypeId,
        pt: &Option<TypeId>,
    ) -> TypeId {
        self.base.visit(ctx, dctx, t, pt)
    }

    /// Java: protected Type typeOf(DeferredType dt, Type pt)
    pub fn type_of(
        &mut self,
        ctx: &mut CompilerContext,
        dctx: &mut DeferredMapCtx,
        t: TypeId,
        pt: &Option<TypeId>,
    ) -> Option<TypeId> {
        self.base.type_of(ctx, dctx, t, pt)
    }

    /// Java: public Type visitMethodType(Type.MethodType t, Type pt)
    /// (DeferredAttr.java:1092-1120)
    #[allow(clippy::collapsible_if)]
    pub fn visit_method_type(
        &mut self,
        ctx: &mut CompilerContext,
        dctx: &mut DeferredMapCtx,
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
                            _ => crate::util::assert::error(),
                        };
                        let (m_argtypes, m_restype, m_thrown) = match &ctx.typ(mtype).kind {
                            TypeKindData::MethodType {
                                argtypes,
                                restype,
                                thrown,
                                ..
                            } => (argtypes.clone(), *restype, thrown.clone()),
                            _ => crate::util::assert::error(),
                        };
                        let argtypes1 = self.map_paired(ctx, dctx, &argtypes, &m_argtypes);
                        let restype1 = restype.map(|r| self.visit(ctx, dctx, r, &m_restype));
                        let thrown1 = self.map_paired(ctx, dctx, &thrown, &m_thrown);
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
        self.base.visit(ctx, dctx, t, pt)
    }

    fn map_paired(
        &mut self,
        ctx: &mut CompilerContext,
        dctx: &mut DeferredMapCtx,
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
            result = result.append(self.visit(ctx, dctx, *ts_cur.head(), &p));
            ts_cur = ts_cur.tail();
            if ps_cur.non_empty() {
                ps_cur = ps_cur.tail();
            }
        }
        result
    }
}

/// Java: class Resolve.ResolveDeferredRecoveryMap extends
/// DeferredAttr.RecoveryDeferredTypeMap (Resolve.java:2639-2660)
///
/// adapter: the Java subclass overrides typeOf; the port exposes the same override as
/// type_of with the base mapping embedded.
pub struct ResolveDeferredRecoveryMap {
    pub base: RecoveryDeferredTypeMap,
}

impl ResolveDeferredRecoveryMap {
    /// Java: public ResolveDeferredRecoveryMap(AttrMode mode, Symbol msym,
    /// MethodResolutionPhase step)
    pub fn new(
        mode: AttrMode,
        msym: Option<SymbolId>,
        phase: MethodResolutionPhase,
    ) -> ResolveDeferredRecoveryMap {
        ResolveDeferredRecoveryMap {
            base: RecoveryDeferredTypeMap::new(mode, msym, phase),
        }
    }

    /// Java: public Type visitType(Type t, Type pt)
    pub fn visit(
        &mut self,
        ctx: &mut CompilerContext,
        dctx: &mut DeferredMapCtx,
        t: TypeId,
        pt: &Option<TypeId>,
    ) -> TypeId {
        self.base.visit(ctx, dctx, t, pt)
    }

    /// Java: protected Type typeOf(DeferredType dt, Type pt)
    /// (Resolve.java:2646-2658)
    pub fn type_of(
        &mut self,
        ctx: &mut CompilerContext,
        dctx: &mut DeferredMapCtx,
        dt: TypeId,
        pt: &Option<TypeId>,
    ) -> Option<TypeId> {
        let res = self.base.type_of(ctx, dctx, dt, pt)?;
        if !dctx.types.is_erroneous_type(ctx, dctx.syms, res) {
            let tree = deferred_fields(ctx, dt).0;
            match ctx.tree(tree).tag {
                crate::tree::jc_tree::Tag::LAMBDA | crate::tree::jc_tree::Tag::REFERENCE => {
                    return Some(dt);
                }
                crate::tree::jc_tree::Tag::CONDEXPR => {
                    let recovery_type = dctx.syms.recovery_type;
                    return if res == recovery_type { Some(dt) } else { Some(res) };
                }
                _ => {}
            }
        }
        Some(res)
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
/// port) — adapter adapter registered. The traversal order follows the upstream
/// TreeScanner methods for the visited tags.
struct FilterScanner;

/// adapter helper: the scanner services the upstream inner class reaches through the
/// enclosing DeferredAttr instance (explicit parameter bundle).
///
/// # Fields
///   - syms/diags/chk: the services the upstream inner class reaches through DeferredAttr.
///
/// # Panics
///   - none.
///
/// Upstream: DeferredAttr.java:1050-1330 (inner-class service access, adapter bundle).
pub struct StuckScannerEnv<'a> {
    syms: &'a Symtab,
    diags: &'a crate::util::jc_diagnostic::Factory,
    chk: &'a crate::comp::check::Check,
}

impl FilterScanner {
    /// Java: public void scan(JCTree tree)
    fn scan(
        ctx: &mut CompilerContext,
        tree: Option<TreeId>,
        policy: &mut CheckStuckPolicy,
        types: &Types,
        env: &mut StuckScannerEnv,
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
                Self::scan(ctx, truepart, policy, types, env);
                Self::scan(ctx, falsepart, policy, types, env);
            }
            Tag::PARENS => {
                let expr = match &ctx.tree(tree).kind {
                    TreeKind::Parens { expr } => *expr,
                    _ => panic!("expected parens"),
                };
                Self::scan(ctx, expr, policy, types, env);
            }
            Tag::LAMBDA => policy.visit_lambda(ctx, tree, types, env),
            Tag::REFERENCE => policy.visit_reference(ctx, tree, types, env),
            Tag::SWITCH_EXPRESSION => policy.visit_switch_expression(ctx, tree, types, env),
            _ => {
                // Java: void skip(JCTree tree) {} — the node is discarded.
            }
        }
    }

    /// LambdaReturnScanner's statement traversal (DeferredAttr.java:1100-1105)
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    fn scan_lambda_return(
        ctx: &mut CompilerContext,
        tree: Option<TreeId>,
        policy: &mut CheckStuckPolicy,
        types: &Types,
        env: &mut StuckScannerEnv,
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
                        Self::scan_lambda_return(ctx, Some(stat), policy, types, env, pt);
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
                Self::scan_lambda_return(ctx, thenpart, policy, types, env, pt);
                Self::scan_lambda_return(ctx, elsepart, policy, types, env, pt);
            }
            Tag::RETURN => {
                let expr = match &ctx.tree(tree).kind {
                    TreeKind::Return { expr } => *expr,
                    _ => panic!("expected a return"),
                };
                if let Some(expr) = expr {
                    let prev_pt = policy.pt;
                    policy.pt = pt;
                    Self::scan(ctx, Some(expr), policy, types, env);
                    policy.pt = prev_pt;
                }
            }
            Tag::DOLOOP => {
                if let TreeKind::DoWhileLoop { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_lambda_return(ctx, body, policy, types, env, pt);
                }
            }
            Tag::WHILELOOP => {
                if let TreeKind::WhileLoop { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_lambda_return(ctx, body, policy, types, env, pt);
                }
            }
            Tag::FORLOOP => {
                if let TreeKind::ForLoop { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_lambda_return(ctx, body, policy, types, env, pt);
                }
            }
            Tag::FOREACHLOOP => {
                if let TreeKind::EnhancedForLoop { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_lambda_return(ctx, body, policy, types, env, pt);
                }
            }
            Tag::SWITCH => {
                if let TreeKind::Switch { cases, .. } = &ctx.tree(tree).kind {
                    let cases = cases.clone();
                    for case in cases.iter() {
                        Self::scan_lambda_return(ctx, Some(case), policy, types, env, pt);
                    }
                }
            }
            Tag::CASE => {
                if let TreeKind::Case { stats, body, .. } = &ctx.tree(tree).kind {
                    let stats = stats.clone();
                    let body = *body;
                    for stat in stats.iter() {
                        Self::scan_lambda_return(ctx, Some(stat), policy, types, env, pt);
                    }
                    Self::scan_lambda_return(ctx, body, policy, types, env, pt);
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
                Self::scan_lambda_return(ctx, body, policy, types, env, pt);
                for catcher in catchers.iter() {
                    Self::scan_lambda_return(ctx, Some(catcher), policy, types, env, pt);
                }
                Self::scan_lambda_return(ctx, finalizer, policy, types, env, pt);
            }
            Tag::CATCH => {
                if let TreeKind::Catch { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_lambda_return(ctx, body, policy, types, env, pt);
                }
            }
            Tag::SYNCHRONIZED => {
                if let TreeKind::Synchronized { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_lambda_return(ctx, body, policy, types, env, pt);
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
        env: &mut StuckScannerEnv,
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
                    FilterScanner::scan(ctx, Some(value), policy, types, env);
                    policy.pt = prev_pt;
                }
            }
            Tag::BLOCK => {
                if let TreeKind::Block { stats, .. } = &ctx.tree(tree).kind {
                    let stats = stats.clone();
                    for stat in stats.iter() {
                        Self::scan_switch_expression(ctx, Some(stat), policy, types, env, pt);
                    }
                }
            }
            Tag::CASE => {
                if let TreeKind::Case { stats, body, .. } = &ctx.tree(tree).kind {
                    let stats = stats.clone();
                    let body = *body;
                    for stat in stats.iter() {
                        Self::scan_switch_expression(ctx, Some(stat), policy, types, env, pt);
                    }
                    Self::scan_switch_expression(ctx, body, policy, types, env, pt);
                }
            }
            Tag::IF => {
                let (thenpart, elsepart) = match &ctx.tree(tree).kind {
                    TreeKind::If {
                        thenpart, elsepart, ..
                    } => (*thenpart, *elsepart),
                    _ => panic!("expected an if"),
                };
                Self::scan_switch_expression(ctx, thenpart, policy, types, env, pt);
                Self::scan_switch_expression(ctx, elsepart, policy, types, env, pt);
            }
            Tag::DOLOOP => {
                if let TreeKind::DoWhileLoop { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_switch_expression(ctx, body, policy, types, env, pt);
                }
            }
            Tag::WHILELOOP => {
                if let TreeKind::WhileLoop { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_switch_expression(ctx, body, policy, types, env, pt);
                }
            }
            Tag::FORLOOP => {
                if let TreeKind::ForLoop { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_switch_expression(ctx, body, policy, types, env, pt);
                }
            }
            Tag::FOREACHLOOP => {
                if let TreeKind::EnhancedForLoop { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_switch_expression(ctx, body, policy, types, env, pt);
                }
            }
            Tag::SWITCH => {
                if let TreeKind::Switch { cases, .. } = &ctx.tree(tree).kind {
                    let cases = cases.clone();
                    for case in cases.iter() {
                        Self::scan_switch_expression(ctx, Some(case), policy, types, env, pt);
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
                Self::scan_switch_expression(ctx, body, policy, types, env, pt);
                for catcher in catchers.iter() {
                    Self::scan_switch_expression(ctx, Some(catcher), policy, types, env, pt);
                }
                Self::scan_switch_expression(ctx, finalizer, policy, types, env, pt);
            }
            Tag::CATCH => {
                if let TreeKind::Catch { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_switch_expression(ctx, body, policy, types, env, pt);
                }
            }
            Tag::SYNCHRONIZED => {
                if let TreeKind::Synchronized { body, .. } = &ctx.tree(tree).kind {
                    let body = *body;
                    Self::scan_switch_expression(ctx, body, policy, types, env, pt);
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
        syms: &Symtab,
        types: &Types,
        diags: &crate::util::jc_diagnostic::Factory,
        chk: &crate::comp::check::Check,
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
        {
            let mut local_env = StuckScannerEnv { syms, diags, chk };
            FilterScanner::scan(ctx, Some(tree), &mut policy, types, &mut local_env);
        }
        if !policy.stuck_vars.borrow().is_empty() {
            let stuck_vars = policy.stuck_vars.borrow().clone();
            let listener = Rc::new(CheckStuckPolicyListener {
                stuck_vars: Rc::clone(&policy.stuck_vars),
            });
            let context = Rc::clone(&policy.inference_context);
            context.add_free_type_listener(
                ctx,
                syms,
                types,
                List::from_iterable(stuck_vars),
                listener,
            );
        }
        policy
    }

    /// Java: public void scan(JCTree tree) (FilterScanner.scan)
    pub fn scan(
        &mut self,
        ctx: &mut CompilerContext,
        types: &Types,
        env: &mut StuckScannerEnv,
        tree: Option<TreeId>,
    ) {
        FilterScanner::scan(ctx, tree, self, types, env);
    }

    /// Java: public void visitLambda(JCLambda tree) (DeferredAttr.java:1195-1220)
    fn visit_lambda(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        types: &Types,
        env: &mut StuckScannerEnv,
    ) {
        let Some(pt) = self.pt else {
            return;
        };
        if self.inference_context.inference_vars().contains(&pt) {
            self.stuck_vars.borrow_mut().push(pt);
        }
        let Some(pt_tsym) = ctx.typ(pt).tsym else {
            return;
        };
        if !types.is_functional_interface(ctx, env.syms, env.diags, env.chk, pt_tsym) {
            return;
        }
        let descriptor_type = match types.find_descriptor_type(ctx, env.syms, env.diags, env.chk, pt) {
            Ok(descriptor) => descriptor,
            Err(_) => return,
        };
        let desc_params = types.get_parameter_types_public(ctx, descriptor_type);
        let free_arg_vars = self.free_vars_of(ctx, types, env, desc_params);
        let param_kind = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::Lambda { param_kind, .. } => *param_kind,
            _ => return,
        };
        if param_kind == crate::tree::jc_tree::ParameterKind::IMPLICIT && !free_arg_vars.is_empty() {
            self.stuck_vars.borrow_mut().extend(free_arg_vars);
            let ret = types.get_return_type_public(ctx, descriptor_type);
            let thrown = ctx.typ(descriptor_type).get_thrown_types(ctx);
            let dep_return = self.free_vars_of(ctx, types, env, List::of(ret));
            let dep_thrown = self.free_vars_of(ctx, types, env, thrown);
            self.dep_vars.borrow_mut().extend(dep_return);
            self.dep_vars.borrow_mut().extend(dep_thrown);
        }
        let ret = types.get_return_type_public(ctx, descriptor_type);
        self.scan_lambda_body(ctx, tree, types, env, Some(ret));
    }

    /// adapter helper: InferenceContext.freeVarsIn(List<Type>) (DeferredAttr.java:1228-1240).
    fn free_vars_of(
        &self,
        ctx: &mut CompilerContext,
        types: &Types,
        env: &StuckScannerEnv,
        ts: crate::util::list::List<TypeId>,
    ) -> Vec<TypeId> {
        let mut result: Vec<TypeId> = Vec::new();
        let mut current = ts;
        while current.non_empty() {
            let t = *current.head();
            let vars = self
                .inference_context
                .free_vars_in(ctx, env.syms, types, t);
            result.extend(vars.iter());
            current = current.tail();
        }
        result
    }

    /// Java: public void visitReference(JCMemberReference tree) (DeferredAttr.java:1222-1245)
    fn visit_reference(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        types: &Types,
        env: &mut StuckScannerEnv,
    ) {
        // Java: scan(tree.expr)
        let expr = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::MemberReference { expr, .. } => *expr,
            _ => return,
        };
        FilterScanner::scan(ctx, expr, self, types, env);
        let Some(pt) = self.pt else {
            return;
        };
        if self.inference_context.inference_vars().contains(&pt) {
            self.stuck_vars.borrow_mut().push(pt);
            return;
        }
        let Some(pt_tsym) = ctx.typ(pt).tsym else {
            return;
        };
        if !types.is_functional_interface(ctx, env.syms, env.diags, env.chk, pt_tsym) {
            return;
        }
        let descriptor_type = match types.find_descriptor_type(ctx, env.syms, env.diags, env.chk, pt) {
            Ok(descriptor) => descriptor,
            Err(_) => return,
        };
        let desc_params = types.get_parameter_types_public(ctx, descriptor_type);
        let free_arg_vars = self.free_vars_of(ctx, types, env, desc_params);
        let overload_kind = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::MemberReference { overload_kind, .. } => *overload_kind,
            _ => None,
        };
        if !free_arg_vars.is_empty()
            && overload_kind != Some(crate::tree::jc_tree::OverloadKind::UNOVERLOADED)
        {
            self.stuck_vars.borrow_mut().extend(free_arg_vars);
            let ret = types.get_return_type_public(ctx, descriptor_type);
            let thrown = ctx.typ(descriptor_type).get_thrown_types(ctx);
            let dep_return = self.free_vars_of(ctx, types, env, List::of(ret));
            let dep_thrown = self.free_vars_of(ctx, types, env, thrown);
            self.dep_vars.borrow_mut().extend(dep_return);
            self.dep_vars.borrow_mut().extend(dep_thrown);
        }
    }

    /// Java: void scanLambdaBody(JCLambda lambda, final Type pt)
    /// (DeferredAttr.java:1247-1270)
    #[allow(dead_code)]
    fn scan_lambda_body(
        &mut self,
        ctx: &mut CompilerContext,
        lambda: TreeId,
        types: &Types,
        env: &mut StuckScannerEnv,
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
            self.scan(ctx, types, env, Some(body));
            self.pt = prev_pt;
        } else {
            FilterScanner::scan_lambda_return(ctx, Some(body), self, types, env, lambda_pt);
        }
    }

    /// Java: public void visitSwitchExpression(JCSwitchExpression expr)
    /// (DeferredAttr.java:1272-1285)
    fn visit_switch_expression(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        types: &Types,
        env: &mut StuckScannerEnv,
    ) {
        let cases = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression { cases, .. } => cases.clone(),
            _ => panic!("expected a switch expression"),
        };
        let pt = self.pt;
        for case in cases.iter() {
            FilterScanner::scan_switch_expression(ctx, Some(case), self, types, env, pt);
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
        _syms: &Symtab,
        _types: &Types,
        _inference_context: &InferenceContext,
    ) -> Result<(), InferenceException> {
        self.stuck_vars.borrow_mut().clear();
        Ok(())
    }
}

/// Java: CheckStuckPolicy constructor wrapper used by DeferredType.check
/// (the upstream new CheckStuckPolicy(resultInfo, dt)).
///
/// # Arguments
///   - ctx/syms/types/diags/chk/result_info/dt: explicit services.
///
/// # Returns
///   - a fresh CheckStuckPolicy.
///
/// # Panics
///   - none.
///
/// Upstream: DeferredAttr.java:1160-1190 (new CheckStuckPolicy(resultInfo, dt)).
pub fn check_stuck_policy_new(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    diags: &crate::util::jc_diagnostic::Factory,
    chk: &crate::comp::check::Check,
    result_info: &ResultInfo,
    dt: TypeId,
) -> CheckStuckPolicy {
    CheckStuckPolicy::new(ctx, syms, types, diags, chk, result_info, dt)
}

/// Java: class OverloadStuckPolicy extends CheckStuckPolicy implements
/// DeferredStuckPolicy
///
/// PORTING_RULES: the upstream constructor's scan dispatches to the overridden
/// visitLambda/visitReference (Java virtual dispatch). The ported
/// visitLambda/visitReference are registered as unimplemented (FunctionDescriptor
///), so the overload-specific stuck flag is only produced by those two
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
        syms: &Symtab,
        types: &Types,
        diags: &crate::util::jc_diagnostic::Factory,
        chk: &crate::comp::check::Check,
        result_info: &ResultInfo,
        dt: TypeId,
    ) -> OverloadStuckPolicy {
        let base = CheckStuckPolicy::new(ctx, syms, types, diags, chk, result_info, dt);
        // Java: OverloadStuckPolicy.visitLambda (DeferredAttr.java:1313-1318):
        // a poly lambda is stuck during the overload phase.
        let (tree, _, _, _, _, _) = deferred_fields(ctx, dt);
        let mut stuck = false;
        let mut stack = vec![tree];
        while let Some(node) = stack.pop() {
            let is_implicit_lambda = match &ctx.tree(node).kind {
                crate::tree::jc_tree::TreeKind::Lambda { param_kind, .. } => {
                    *param_kind == crate::tree::jc_tree::ParameterKind::IMPLICIT
                }
                _ => false,
            };
            if is_implicit_lambda {
                stuck = true;
            }
            for child in crate::tree::tree_scanner::tree_children(ctx, node) {
                stack.push(child);
            }
        }
        OverloadStuckPolicy { base, stuck }
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
///
/// # Arguments
///   - ctx/syms/types/diags/chk/result_info/dt: explicit services.
///
/// # Returns
///   - a fresh OverloadStuckPolicy.
///
/// # Panics
///   - none.
///
/// Upstream: DeferredAttr.java:1300-1330.
pub fn overload_stuck_policy_new(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    diags: &crate::util::jc_diagnostic::Factory,
    chk: &crate::comp::check::Check,
    result_info: &ResultInfo,
    dt: TypeId,
) -> OverloadStuckPolicy {
    OverloadStuckPolicy::new(ctx, syms, types, diags, chk, result_info, dt)
}

// ---------------------------------------------------------------------------
// the DeferredAttr service (DeferredAttr.java:96-560): the service
// instance/constructor, the attribSpeculative overloads, DeferredAttrDiagHandler
// and its PosScanner. adapter adapters: the upstream service fields are explicit
// construction parameters (ADR 0001); the Position scanner is a precomputed
// position set of the copied tree (same tree, same positions, registered);
// the LocalCacheContext overload is registered as depending on ArgumentAttr
// (implemented with ArgumentAttr in the next stage).
// ---------------------------------------------------------------------------

/// Java: public class DeferredAttr (DeferredAttr.java:96-560)
///
/// # Fields
///   - annotate/diags/enter/infer/log/make/types/syms/names/dcfh: injected services;
///   - tree_copier/stuck_tree/empty_deferred_attr_context: the upstream fields.
///
/// # Panics
///   - none.
///
/// Upstream: DeferredAttr.java:96-230.
pub struct DeferredAttrService {
    /// Java: final Annotate annotate (lazily installed, adapter cell).
    pub annotate: Rc<RefCell<Option<Rc<crate::comp::annotate::Annotate>>>>,
    /// Java: final JCDiagnostic.Factory diags
    pub diags: Rc<crate::util::jc_diagnostic::Factory>,
    /// Java: final Enter enter
    pub enter: Rc<RefCell<crate::comp::enter::Enter>>,
    /// Java: final Infer infer
    pub infer: Rc<crate::comp::infer::Infer>,
    /// Java: final Log log
    pub log: Rc<RefCell<crate::util::log::Log>>,
    /// Java: final TreeMaker make
    pub make: Rc<RefCell<crate::tree::tree_maker::TreeMaker>>,
    /// Java: final Types types
    pub types: Rc<Types>,
    /// Java: final Symtab syms
    pub syms: Rc<RefCell<Symtab>>,
    /// Java: final Names names
    pub names: Rc<Names>,
    /// Java: final DeferredCompletionFailureHandler dcfh
    pub dcfh: Rc<crate::code::deferred_completion_failure_handler::DeferredCompletionFailureHandler>,
    /// Java: final TreeCopier<Void> treeCopier
    pub tree_copier: crate::tree::tree_copier::TreeCopier,
    /// Java: shared tree for stuck expressions
    pub stuck_tree: TreeId,
    /// Java: emptyDeferredAttrContext
    pub empty_deferred_attr_context: Rc<DeferredAttrContext>,
}

/// adapter: the positions of a copied tree, scanned once when the speculative
/// DeferredAttrDiagHandler is installed (equivalent to the upstream PosScanner,
/// which scans the unchanged tree per diagnostic).
pub struct PositionScannerVisitor {
    pub positions: std::collections::HashSet<i32>,
}

impl crate::tree::tree_scanner::TreeScannerVisitor for PositionScannerVisitor {
    /// Java: PosScanner.scan(JCTree): record the position then navigate.
    fn scan(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) {
        if let Some(tree) = tree {
            self.positions.insert(ctx.tree(tree).pos);
            crate::tree::jc_tree::accept(ctx, tree, self);
        }
    }
}

crate::tree::tree_scanner::tree_scanner_visitor_delegate!(PositionScannerVisitor);

impl DeferredAttrService {
    /// Java: protected DeferredAttr(Context context) (DeferredAttr.java:112-200)
    /// — the Ported construction with explicit services (ADR 0001).
    /// 
    /// # Arguments
    ///   - ctx: compiler context used to allocate the shared stuck tree;
    ///   - the service parameters mirror the upstream constructor fields.
    /// 
    ///    # Returns
    ///   - the shared DeferredAttrService handle.
    /// 
    ///    # Panics
    ///   - none.
    /// 
    ///    Upstream: DeferredAttr.java:112-230.
    /// 
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: &mut CompilerContext,
        annotate: Rc<RefCell<Option<Rc<crate::comp::annotate::Annotate>>>>,
        diags: Rc<crate::util::jc_diagnostic::Factory>,
        enter: Rc<RefCell<crate::comp::enter::Enter>>,
        infer: Rc<crate::comp::infer::Infer>,
        log: Rc<RefCell<crate::util::log::Log>>,
        make: Rc<RefCell<crate::tree::tree_maker::TreeMaker>>,
        types: Rc<Types>,
        syms: Rc<RefCell<Symtab>>,
        names: Rc<Names>,
        dcfh: Rc<crate::code::deferred_completion_failure_handler::DeferredCompletionFailureHandler>,
    ) -> Rc<DeferredAttrService> {
        // Java: stuckTree = make.Ident(names.empty).setType(Type.stuckType);
        let stuck_type = syms.borrow().stuck_type;
        let stuck_tree = ctx.alloc_tree(crate::tree::jc_tree::JCTree {
            pos: crate::util::position::NOPOS,
            ty: Some(stuck_type),
            tag: crate::tree::jc_tree::Tag::IDENT,
            kind: crate::tree::jc_tree::TreeKind::Ident {
                name: names.empty.clone(),
                sym: None,
            },
        });
        let tree_copier = crate::tree::tree_copier::TreeCopier::new_speculative(Rc::clone(&make));
        Rc::new(DeferredAttrService {
            annotate,
            diags,
            enter,
            infer,
            log,
            make,
            types,
            syms,
            names,
            dcfh,
            tree_copier,
            stuck_tree,
            empty_deferred_attr_context: crate::comp::deferred_attr::empty_deferred_attr_context(),
        })
    }

    /// Java: JCTree attribSpeculative(JCTree, Env, ResultInfo)
    /// (DeferredAttr.java:470-472)
    /// 
    /// # Arguments
    ///   - ctx/syms/attr: explicit services;
    ///   - tree/env/result_info: the speculative target.
    /// 
    ///    # Returns
    ///   - Ok(the speculative tree with its type).
    /// 
    ///    # Panics
    ///   - the copied tree must be a valid JCExpression tree (upstream Assert).
    /// 
    ///    Upstream: DeferredAttr.java:470-472.
    pub fn attrib_speculative(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        attr: &mut Attr,
        tree: TreeId,
        env: &Rc<Env<crate::comp::attr_context::AttrContext>>,
        result_info: &ResultInfo,
    ) -> Result<TreeId, DeferredAttrFailure> {
        self.attrib_speculative_with_diag(
            ctx,
            syms,
            attr,
            tree,
            env,
            result_info,
            None,
            AttributionMode::SPECULATIVE,
            None,
        )
    }

    /// Java: JCTree attribSpeculative(JCTree tree, Env, ResultInfo,
    /// TreeCopier<Z> deferredCopier, Supplier<DiagnosticHandler>,
    /// AttributionMode, LocalCacheContext) (DeferredAttr.java:481-489)
    /// 
    /// # Arguments
    ///   - as above plus diag_handler_creator/attribution_mode; the tree is copied first with tree_copier.
    /// 
    ///    # Returns
    ///   - Ok(the speculative tree) after attributing the copy.
    /// 
    ///    # Panics
    ///   - none beyond the registered dependency failures.
    /// 
    ///    Upstream: DeferredAttr.java:481-489.
    /// 
    /// 
    ///    # Arguments
    ///   - as above; Porting rules: the LocalCacheContext overload is registered with the ArgumentAttr stage.
    /// 
    ///    # Returns
    ///   - Ok(the speculative tree) after the attribution round.
    /// 
    ///    # Panics
    ///   - none.
    /// 
    ///    Upstream: DeferredAttr.java:496-520.
    /// 
    #[allow(clippy::too_many_arguments)]
    pub fn attrib_speculative_with_copier(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        attr: &mut Attr,
        tree: TreeId,
        env: &Rc<Env<crate::comp::attr_context::AttrContext>>,
        result_info: &ResultInfo,
        diag_handler_creator: Option<Box<dyn Fn() -> crate::util::log::DiagnosticHandler>>,
        attribution_mode: AttributionMode,
    ) -> Result<TreeId, DeferredAttrFailure> {
        let new_tree = self.tree_copier.copy(ctx, Some(tree)).expect("copied tree");
        self.attrib_speculative_with_diag(
            ctx,
            syms,
            attr,
            new_tree,
            env,
            result_info,
            diag_handler_creator,
            attribution_mode,
            None,
        )
    }

    /// Java: JCTree attribSpeculative(JCTree tree, Env, ResultInfo,
    /// Supplier<DiagnosticHandler>, AttributionMode, LocalCacheContext)
    /// (DeferredAttr.java:489-511); the LocalCacheContext is left by the Drop
    /// guard below, mirroring the upstream finally (DeferredAttr.java:508-510).
    #[allow(clippy::too_many_arguments)]
    pub fn attrib_speculative_with_diag(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        attr: &mut Attr,
        tree: TreeId,
        env: &Rc<Env<crate::comp::attr_context::AttrContext>>,
        result_info: &ResultInfo,
        diag_handler_creator: Option<Box<dyn Fn() -> crate::util::log::DiagnosticHandler>>,
        attribution_mode: AttributionMode,
        local_cache: Option<crate::comp::argument_attr::LocalCacheContext>,
    ) -> Result<TreeId, DeferredAttrFailure> {
        use crate::util::log::DiagnosticHandler;
        // Java: finally { ...; if (localCache != null) localCache.leave(); }
        // (DeferredAttr.java:502-510); the Drop guard also covers panic paths.
        struct LocalCacheLeave {
            cache: Option<crate::comp::argument_attr::LocalCacheContext>,
            argument_attr: Option<Rc<RefCell<crate::comp::argument_attr::ArgumentAttr>>>,
        }
        impl Drop for LocalCacheLeave {
            fn drop(&mut self) {
                if let (Some(cache), Some(argument_attr)) =
                    (self.cache.as_mut(), &self.argument_attr)
                {
                    cache.leave(&argument_attr.borrow());
                }
            }
        }
        let _local_cache_leave = LocalCacheLeave {
            cache: local_cache,
            argument_attr: attr.argument_attr.borrow().clone(),
        };
        // The immediate env of a deferred attribution can be a synthetic env
        // without a scope; walk the enclosing envs (upstream Env.outer is always
        // populated during attribution) and fall back to the Attr's current env.
        let mut scope = None;
        let mut current = Some(Rc::clone(env));
        while let Some(candidate) = current {
            if let Some(candidate_scope) = candidate.info.borrow().scope {
                scope = Some(candidate_scope);
                break;
            }
            current = candidate.outer.clone();
        }
        if scope.is_none()
            && let Some(attr_env) = &attr.env
        {
            scope = attr_env.info.borrow().scope;
        }
        let scope = scope.expect("env scope");
        let owner = ctx.scope(scope).owner;
        let new_scope = ctx.scope_dup_unshared(scope, owner);
        let new_info = env.info.borrow().dup(Some(new_scope));
        let speculative_env = env.dup(Some(tree), new_info);
        speculative_env.info.borrow_mut().attribution_mode = attribution_mode;
        let deferred_diagnostic_handler = match diag_handler_creator {
            Some(creator) => creator(),
            None => {
                let mut visitor = PositionScannerVisitor {
                    positions: std::collections::HashSet::new(),
                };
                crate::tree::tree_scanner::TreeScannerVisitor::scan(&mut visitor, ctx, Some(tree));
                let positions = Rc::new(visitor.positions);
                DiagnosticHandler::Deferred {
                    prev: None,
                    deferred: std::collections::VecDeque::new(),
                    filter: Some(Box::new(move |diag| {
                        positions.contains(&diag.get_int_position())
                    })),
                }
            }
        };
        let prev_cf_handler = self.dcfh.set_handler(
            ctx,
            Box::new(crate::code::deferred_completion_failure_handler::SpeculativeCodeHandler::new()),
        );
        let annotate_rc = self.annotate.borrow().clone();
        let prev_queues = annotate_rc.as_ref().map(|annotate| {
            annotate.set_queues(crate::comp::annotate::AnnotateQueues::new())
        });
        let nwarnings = self.log.borrow().nwarnings;
        self.log.borrow_mut().nwarnings = 0;
        self.log
            .borrow_mut()
            .install_diagnostic_handler(deferred_diagnostic_handler);
        let result = attr.attrib_tree(ctx, syms, tree, &speculative_env, result_info.clone());
        if let Some(annotate) = &annotate_rc {
            annotate.set_queues(prev_queues.expect("prev queues"));
        }
        self.dcfh.set_handler(ctx, prev_cf_handler);
        {
            let mut log = self.log.borrow_mut();
            log.nwarnings += nwarnings;
        }
        if let Some(toplevel) = env.toplevel {
            self.enter.borrow_mut().unenter(ctx, syms, toplevel, tree);
        }
        self.log.borrow_mut().pop_diagnostic_handler();
        match result {
            Ok(_) => Ok(tree),
            Err(failure) => Err(DeferredAttrFailure::from(failure)),
        }
    }
}

impl DeferredAttrService {
    /// Java: JCLambda attribSpeculativeLambda(JCLambda that, Env<AttrContext> env,
    /// ResultInfo resultInfo) (DeferredAttr.java:434-468)
    /// 
    /// # Arguments
    ///   - ctx/syms/attr/attr_recover/rs/chk/diags/flow: explicit services;
    ///   - that/env/result_info: the lambda and its target.
    /// 
    ///    # Returns
    ///   - Ok(the speculative lambda tree).
    /// 
    ///    # Panics
    ///   - the lambda body / return tree shapes are asserted as in the upstream casts.
    /// 
    ///    Upstream: DeferredAttr.java:434-468.
    /// 
    #[allow(clippy::too_many_arguments)]
    pub fn attrib_speculative_lambda(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        attr: &mut Attr,
        attr_recover_rc: &Rc<RefCell<crate::comp::attr_recover::AttrRecover>>,
        rs: &crate::comp::resolve::Resolve,
        chk: &crate::comp::check::Check,
        diags: &crate::util::jc_diagnostic::Factory,
        flow: &crate::comp::flow::Flow,
        that: TreeId,
        env: &Rc<Env<crate::comp::attr_context::AttrContext>>,
        result_info: &ResultInfo,
    ) -> Result<TreeId, DeferredAttrFailure> {
        use crate::tree::tree_maker::TreeFactory;
        let (params, body, pos) = match &ctx.tree(that).kind {
            crate::tree::jc_tree::TreeKind::Lambda { params, body, .. } => {
                (params.clone(), *body, ctx.tree(that).pos)
            }
            _ => panic!("ClassCastException: JCLambda expected"),
        };
        let body = body.expect("lambda body");
        let mut stats = params.clone();
        let body_is_expression = ctx.tree(body).tag != crate::tree::jc_tree::Tag::BLOCK;
        if body_is_expression {
            let ret = {
                let mut make = self.make.borrow_mut();
                make.at(pos);
                make.return_(ctx, Some(body))
            };
            stats = stats.append(ret);
        } else {
            stats = stats.append(body);
        }
        let lambda_block = {
            let mut make = self.make.borrow_mut();
            make.at(pos);
            make.block(ctx, 0, stats)
        };
        let local_env = attr.lambda_env(ctx, syms, that, env);
        local_env.info.borrow_mut().return_result = Some(result_info.clone());
        let outcome = (|| -> Result<TreeId, DeferredAttrFailure> {
            let speculative_tree = self.attrib_speculative(
                ctx,
                syms,
                attr,
                lambda_block,
                &local_env,
                result_info,
            )?;
            let spec_stats = match &ctx.tree(speculative_tree).kind {
                crate::tree::jc_tree::TreeKind::Block { stats, .. } => stats.clone(),
                _ => panic!("ClassCastException: JCBlock expected"),
            };
            let mut args: crate::util::list::List<TreeId> = crate::util::list::List::nil();
            let mut all = spec_stats;
            let mut last: TreeId = speculative_tree;
            while all.non_empty() {
                let stat = *all.head();
                last = stat;
                if ctx.tree(stat).tag == crate::tree::jc_tree::Tag::VARDEF {
                    args = args.append(stat);
                }
                all = all.tail();
            }
            let lambda_body = if ctx.tree(last).tag == crate::tree::jc_tree::Tag::RETURN {
                match &ctx.tree(last).kind {
                    crate::tree::jc_tree::TreeKind::Return { expr } => expr.expect("return expr"),
                    _ => last,
                }
            } else {
                last
            };
            let speculative_lambda = {
                let mut make = self.make.borrow_mut();
                make.at(pos);
                make.lambda(ctx, args, lambda_body)
            };
            {
                // The recover service is re-entrant: attribution inside
                // preFlow may call checkId -> recoverMethodInvocation again.
                // Hold the RefCell borrow only for the doRecovery call.
                let mut attr_recover = attr_recover_rc.borrow_mut();
                attr.pre_flow(ctx, syms, &mut attr_recover, rs, &self.infer, chk, diags, env, speculative_lambda);
            }
            flow.analyze_lambda(ctx, syms, env, speculative_lambda, &self.make, false);
            Ok(speculative_lambda)
        })();
        if let Some(scope) = local_env.info.borrow().scope {
            ctx.scope_leave(scope);
        }
        outcome
    }
}

// ---------------------------------------------------------------------------
// DeferredAttrNode.StructuralStuckChecker (DeferredAttr.java:800-990)
// and LambdaBodyStructChecker (DeferredAttr.java:955-990). adapter: the inner-
// class field access is explicit service parameters; the navigational body is
// tree_children (explicit recursion).
// ---------------------------------------------------------------------------

impl DeferredAttrService {
    /// Java: StructuralStuckChecker.check(DeferredType dt, ResultInfo,
    /// DeferredAttrContext) (DeferredAttr.java:808-816)
    /// 
    /// # Arguments
    ///   - ctx/syms/attr/dt/result_info/deferred_attr_context: the stuck node and its target.
    /// 
    ///    # Returns
    ///   - Ok(()) after recording the stuck tree in the speculative cache.
    /// 
    ///    # Panics
    ///   - none.
    /// 
    ///    Upstream: DeferredAttr.java:808-816.
    pub fn structural_stuck_check(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        attr: &mut Attr,
        dt: TypeId,
        result_info: &Rc<ResultInfo>,
        deferred_attr_context: &DeferredAttrContext,
    ) -> Result<(), DeferredAttrFailure> {
        let (tree, env, _, _, cache, _) = deferred_fields(ctx, dt);
        let state = StructuralState {
            result_info: Rc::clone(result_info),
            inference_context: Rc::clone(&deferred_attr_context.inference_context),
            env,
        };
        self.structural_check_tree(ctx, syms, attr, &state, tree)?;
        cache
            .borrow_mut()
            .put(self.stuck_tree, Rc::clone(result_info));
        Ok(())
    }

    /// Java: StructuralStuckChecker tree navigation (TreeScanner default bodies;
    /// the overridden visits are the special cases below).
    fn structural_check_tree(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        attr: &mut Attr,
        state: &StructuralState,
        tree: TreeId,
    ) -> Result<(), DeferredAttrFailure> {
        use crate::tree::jc_tree::Tag;
        match ctx.tree(tree).tag {
            Tag::LAMBDA => self.structural_check_lambda(ctx, syms, attr, state, tree),
            Tag::REFERENCE => self.structural_check_reference(ctx, syms, attr, state, tree),
            // Java: visitNewClass/visitApply: do nothing
            Tag::NEWCLASS | Tag::APPLY => Ok(()),
            Tag::CONDEXPR => {
                // Java: visitConditional: skip tree.cond
                let (truepart, falsepart) = match &ctx.tree(tree).kind {
                    crate::tree::jc_tree::TreeKind::Conditional {
                        truepart, falsepart, ..
                    } => (*truepart, *falsepart),
                    _ => panic!("ClassCastException: JCConditional expected"),
                };
                if let Some(truepart) = truepart {
                    self.structural_check_tree(ctx, syms, attr, state, truepart)?;
                }
                if let Some(falsepart) = falsepart {
                    self.structural_check_tree(ctx, syms, attr, state, falsepart)?;
                }
                Ok(())
            }
            Tag::SWITCH_EXPRESSION => {
                // Java: visitSwitchExpression: scan(tree.cases)
                let cases = match &ctx.tree(tree).kind {
                    crate::tree::jc_tree::TreeKind::SwitchExpression { cases, .. } => cases.clone(),
                    _ => panic!("ClassCastException: JCSwitchExpression expected"),
                };
                let mut current = cases;
                while current.non_empty() {
                    self.structural_check_tree(ctx, syms, attr, state, *current.head())?;
                    current = current.tail();
                }
                Ok(())
            }
            _ => {
                for child in crate::tree::tree_scanner::tree_children(ctx, tree) {
                    self.structural_check_tree(ctx, syms, attr, state, child)?;
                }
                Ok(())
            }
        }
    }

    /// Java: StructuralStuckChecker.visitLambda (DeferredAttr.java:820-880)
    fn structural_check_lambda(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        attr: &mut Attr,
        state: &StructuralState,
        tree: TreeId,
    ) -> Result<(), DeferredAttrFailure> {
        let check_context = Rc::clone(&state.result_info.check_context);
        let pt = state
            .result_info
            .pt
            .unwrap_or_else(|| crate::code::symtab::no_type_id(ctx));
        let inference_vars = state.inference_context.inference_vars();
        if inference_vars.contains(&pt) {
            return Ok(());
        }
        // must be a functional descriptor
        let descriptor_type = {
            let chk_rc = Rc::clone(&attr.chk);
            let chk = Rc::clone(&chk_rc);
            match self
                .types
                .find_descriptor_type(ctx, syms, &self.diags, &chk, pt)
            {
                Ok(descriptor) => Some(descriptor),
                Err(error) => {
                    if let Some(diag) = error.get_diagnostic() {
                        check_context.report(ctx, 0, diag);
                    }
                    None
                }
            }
        };
        let Some(descriptor_type) = descriptor_type else {
            return Ok(());
        };
        let params = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::Lambda { params, .. } => params.clone(),
            _ => panic!("ClassCastException: JCLambda expected"),
        };
        let descriptor_params = self.types.get_parameter_types_public(ctx, descriptor_type);
        if list_len(&descriptor_params) != list_len_trees(&params) {
            let diag = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                crate::resources::compiler_properties::fragments::incompatible_arg_types_in_lambda(),
            ));
            check_context.report(ctx, ctx.tree(tree).pos, &diag);
        }
        let current_return_type = self.types.get_return_type_public(ctx, descriptor_type);
        let return_type_is_void =
            ctx.typ(current_return_type).has_tag(TypeTag::VOID);
        let (body, body_is_expression) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::Lambda { body, .. } => {
                let body = body.expect("lambda body");
                (body, ctx.tree(body).tag != crate::tree::jc_tree::Tag::BLOCK)
            }
            _ => panic!("ClassCastException: JCLambda expected"),
        };
        if body_is_expression {
            let is_expression_compatible = !return_type_is_void
                || crate::tree::tree_info::is_expression_statement(ctx, body);
            if !is_expression_compatible {
                let missing = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                    crate::resources::compiler_properties::fragments::missing_ret_val(
                        crate::resources::diagnostic_contract::TypeArg::placeholder(
                            crate::comp::attr::type_display_of(ctx, current_return_type),
                        ),
                    ),
                ));
                let diag = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                    crate::resources::compiler_properties::fragments::incompatible_ret_type_in_lambda(
                        crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(
                            Rc::new(missing),
                        ),
                    ),
                ));
                check_context.report(ctx, ctx.tree(tree).pos, &diag);
            }
        } else {
            let (is_void_compatible, is_potentially_value_compatible) =
                self.lambda_body_struct_check(ctx, body);
            if return_type_is_void {
                if !is_void_compatible {
                    let diag = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                        crate::resources::compiler_properties::fragments::unexpected_ret_val(),
                    ));
                    check_context.report(ctx, ctx.tree(tree).pos, &diag);
                }
            } else {
                let can_complete_normally =
                    self.can_lambda_body_complete_normally(ctx, syms, attr, state, tree)?;
                let is_value_compatible =
                    is_potentially_value_compatible && !can_complete_normally;
                if !is_value_compatible && !is_void_compatible {
                    crate::util::abstract_log::AbstractLog::error_at(
                        &mut *self.log.borrow_mut(),
                        ctx.tree(body).pos,
                        crate::util::jc_diagnostic::Error::from(
                            crate::resources::compiler_properties::errors::
                                lambda_body_neither_value_nor_void_compatible(),
                        ),
                    );
                }
                if !is_value_compatible {
                    let missing = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                        crate::resources::compiler_properties::fragments::missing_ret_val(
                            crate::resources::diagnostic_contract::TypeArg::placeholder(
                                crate::comp::attr::type_display_of(ctx, current_return_type),
                            ),
                        ),
                    ));
                    let int_pos = ctx.tree(tree).pos;
                    let source = self.log.borrow().current_source();
                    // Java: the details diagnostic carries the lambda position
                    // (used by the final incompatible-types report).
                    let diag = self.diags.create(
                        crate::util::jc_diagnostic::DiagnosticType::FRAGMENT,
                        source,
                        Some(Rc::new(
                            crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(int_pos),
                        )),
                        "incompatible.ret.type.in.lambda",
                        vec![Rc::new(missing)],
                    );
                    check_context.report(ctx, int_pos, &diag);
                }
            }
        }
        Ok(())
    }

    /// Java: StructuralStuckChecker.visitReference (DeferredAttr.java:900-955)
    fn structural_check_reference(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        attr: &mut Attr,
        state: &StructuralState,
        tree: TreeId,
    ) -> Result<(), DeferredAttrFailure> {
        let check_context = Rc::clone(&state.result_info.check_context);
        let pt = state
            .result_info
            .pt
            .unwrap_or_else(|| crate::code::symtab::no_type_id(ctx));
        let inference_vars = state.inference_context.inference_vars();
        if inference_vars.contains(&pt) {
            return Ok(());
        }
        let descriptor = {
            let chk_rc = Rc::clone(&attr.chk);
            let chk = Rc::clone(&chk_rc);
            match self
                .types
                .find_descriptor_type(ctx, syms, &self.diags, &chk, pt)
            {
                Ok(descriptor) => Some(descriptor),
                Err(error) => {
                    if let Some(diag) = error.get_diagnostic() {
                        check_context.report(ctx, 0, diag);
                    }
                    None
                }
            }
        };
        let Some(descriptor) = descriptor else {
            return Ok(());
        };
        let (expr, name) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::MemberReference { expr, name, .. } => {
                (*expr, name.clone())
            }
            _ => panic!("ClassCastException: JCMemberReference expected"),
        };
        let expr = expr.expect("member reference qualifier");
        let local_env = state.env.dup(Some(tree), state.env.info.borrow().dup(None));
        let qualifier_result = attr.member_reference_qualifier_result(ctx, &local_env, tree);
        let expr_tree = self
            .attrib_speculative(ctx, syms, attr, expr, &local_env, &qualifier_result)
            .map_err(|failure| match failure {
                DeferredAttrFailure::Completion(failure) => failure,
                other => panic!("unexpected speculative attribution failure: {:?}", other),
            })?;
        let descriptor_params = self.types.get_parameter_types_public(ctx, descriptor);
        let mut argtypes: crate::util::list::List<TypeId> = crate::util::list::List::nil();
        let mut current = descriptor_params;
        while current.non_empty() {
            argtypes = argtypes.append(crate::code::symtab::no_type_id(ctx));
            current = current.tail();
        }
        let mref2 = {
            let copier = crate::tree::tree_copier::TreeCopier::new(Rc::clone(&self.make));
            copier.copy(ctx, Some(tree)).expect("copied member reference")
        };
        if let crate::tree::jc_tree::TreeKind::MemberReference { expr: mref_expr, .. } =
            &mut ctx.tree_mut(mref2).kind
        {
            *mref_expr = Some(expr_tree);
        }
        let expr_type = ctx.tree(expr_tree).ty.expect("qualifier type");
        let site = ctx.typ(expr_type).tsym.expect("qualifier type symbol");
        let rs_rc = attr.rs.borrow().clone().expect("resolve service");
        let rs = rs_rc.borrow();
        let chk_rc = Rc::clone(&attr.chk);
        let chk = Rc::clone(&chk_rc);
        let types_rc = Rc::clone(&attr.types);
        let infer_rc = Rc::clone(&attr.infer);
        let arity_check: Rc<RefCell<dyn crate::comp::resolve::MethodCheck>> =
            Rc::new(RefCell::new(crate::comp::resolve::ArityMethodCheck));
        let (lookup_sym, _) = rs.resolve_member_reference(
            ctx,
            syms,
            &types_rc,
            attr,
            &chk,
            &infer_rc,
            &local_env,
            ctx.tree(tree).pos,
            mref2,
            expr_type,
            name.clone(),
            argtypes.clone(),
            None,
            descriptor,
            arity_check,
            &state.inference_context,
            crate::comp::resolve::ReferenceChooserKind::Structural,
        );
        let kind = ctx.sym(lookup_sym).kind;
        match kind {
            crate::code::kinds::Kind::WRONG_MTH | crate::code::kinds::Kind::WRONG_MTHS => {
                // note: as argtypes are erroneous types, type-errors must have
                // been caused by arity mismatch
                let diag = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                    crate::resources::compiler_properties::fragments::incompatible_arg_types_in_mref(),
                ));
                check_context.report(ctx, ctx.tree(tree).pos, &diag);
            }
            crate::code::kinds::Kind::ABSENT_MTH | crate::code::kinds::Kind::STATICERR => {
                // Java: the structural check reports the descriptor/parameter
                // incompatibility; the ported lookup yields ABSENT_MTH for the
                // arity mismatch, so report the same fragment directly.
                let diag = self.diags.fragment(crate::util::jc_diagnostic::Fragment::from(
                    crate::resources::compiler_properties::fragments::incompatible_arg_types_in_mref(),
                ));
                let info = crate::resources::compiler_properties::errors::prob_found_req(
                    crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(
                        std::rc::Rc::new(diag),
                    ),
                );
                // Java: the compact diagnostic is flagged COMPRESSED, which
                // makes the driver print the "Some messages have been
                // simplified" note (JavaCompiler.java:1719-1721).
                crate::util::abstract_log::AbstractLog::error_at(
                    &mut *attr.log.borrow_mut(),
                    ctx.tree(tree).pos,
                    crate::util::jc_diagnostic::Error::from(info),
                );
                attr.log.borrow_mut().compressed_output = true;
            }
            _ => {}
        }
        drop(chk);
        drop(rs);
        Ok(())
    }
    /// Java: StructuralStuckChecker.canLambdaBodyCompleteNormally(JCLambda)
    /// (DeferredAttr.java:885-900)
    fn can_lambda_body_complete_normally(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        attr: &mut Attr,
        state: &StructuralState,
        tree: TreeId,
    ) -> Result<bool, DeferredAttrFailure> {
        let _ = state;
        let old_params = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::Lambda { params, .. } => params.clone(),
            _ => panic!("ClassCastException: JCLambda expected"),
        };
        let err_type = ctx
            .sym(syms.err_symbol)
            .ty
            .unwrap_or_else(|| crate::code::symtab::no_type_id(ctx));
        // Java: tree.params = tree.params.stream().map(vd -> make.VarDef(
        // vd.mods, vd.name, make.Erroneous(), null))
        let mut new_params: crate::util::list::List<TreeId> = crate::util::list::List::nil();
        let mut current = old_params.clone();
        while current.non_empty() {
            let vd = *current.head();
            let (mods, name) = match &ctx.tree(vd).kind {
                crate::tree::jc_tree::TreeKind::VariableDecl { mods, name, .. } => {
                    (*mods, name.clone())
                }
                _ => panic!("ClassCastException: JCVariableDecl expected"),
            };
            let new_vd = ctx.alloc_tree(crate::tree::jc_tree::JCTree {
                pos: ctx.tree(vd).pos,
                ty: Some(err_type),
                tag: crate::tree::jc_tree::Tag::VARDEF,
                kind: crate::tree::jc_tree::TreeKind::VariableDecl {
                    mods,
                    name,
                    nameexpr: None,
                    vartype: None,
                    init: None,
                    sym: None,
                    start_pos: 0,
                    declared_using_var: false,
                },
            });
            new_params = new_params.append(new_vd);
            current = current.tail();
        }
        if let crate::tree::jc_tree::TreeKind::Lambda { params, .. } =
            &mut ctx.tree_mut(tree).kind
        {
            *params = new_params;
        }
        let env = state.env.clone();
        let unknown_expr_info = attr.unknown_expr_info.clone();
        let attr_recover_rc = attr
            .attr_recover
            .borrow()
            .clone()
            .expect("attr recover service");
        let rs_rc = attr.rs.borrow().clone().expect("resolve service");
        let rs = rs_rc.borrow();
        let chk_rc = Rc::clone(&attr.chk);
        let chk = Rc::clone(&chk_rc);
        let diags = Rc::clone(&attr.diags);
        let flow_rc = attr
            .argument_attr
            .borrow()
            .clone()
            .and_then(|argument_attr| argument_attr.borrow().flow.borrow().clone())
            .expect("flow service");
        let result = self.attrib_speculative_lambda(
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
            &unknown_expr_info,
        );
        drop(chk);
        drop(rs);
        // Java: tree.params = oldParams (finally)
        if let crate::tree::jc_tree::TreeKind::Lambda { params, .. } =
            &mut ctx.tree_mut(tree).kind
        {
            *params = old_params;
        }
        let speculative_lambda = result.map_err(|failure| match failure {
            DeferredAttrFailure::Completion(failure) => failure,
            other => panic!("unexpected speculative lambda failure: {:?}", other),
        })?;
        Ok(match &ctx.tree(speculative_lambda).kind {
            crate::tree::jc_tree::TreeKind::Lambda {
                can_complete_normally,
                ..
            } => *can_complete_normally,
            _ => panic!("ClassCastException: JCLambda expected"),
        })
    }

    /// Java: LambdaBodyStructChecker (DeferredAttr.java:955-990)
    /// 
    /// # Fields
    ///   - is_void_compatible/is_potentially_value_compatible: the JLS value/void compatibility flags.
    /// 
    ///    # Panics
    ///   - none.
    /// 
    ///    Upstream: DeferredAttr.java:955-990.
    /// 
    fn lambda_body_struct_check(
        &self,
        ctx: &mut CompilerContext,
        tree: TreeId,
    ) -> (bool, bool) {
        let mut checker = LambdaBodyStructChecker {
            is_void_compatible: true,
            is_potentially_value_compatible: true,
        };
        checker.scan(ctx, Some(tree));
        (
            checker.is_void_compatible,
            checker.is_potentially_value_compatible,
        )
    }
}

/// Java: DeferredAttrNode.StructuralStuckChecker state
struct StructuralState {
    result_info: Rc<ResultInfo>,
    inference_context: Rc<crate::comp::infer::InferenceContext>,
    env: Rc<Env<crate::comp::attr_context::AttrContext>>,
}

/// adapter helper: List length.
fn list_len(list: &crate::util::list::List<TypeId>) -> usize {
    let mut current = list.clone();
    let mut n = 0;
    while current.non_empty() {
        n += 1;
        current = current.tail();
    }
    n
}

fn list_len_trees(list: &crate::util::list::List<TreeId>) -> usize {
    let mut current = list.clone();
    let mut n = 0;
    while current.non_empty() {
        n += 1;
        current = current.tail();
    }
    n
}

/// Java: LambdaBodyStructChecker (DeferredAttr.java:955-990)
struct LambdaBodyStructChecker {
    is_void_compatible: bool,
    is_potentially_value_compatible: bool,
}

impl LambdaBodyStructChecker {
    fn scan(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) {
        let Some(tree) = tree else {
            return;
        };
        use crate::tree::jc_tree::Tag;
        match ctx.tree(tree).tag {
            // Java: visitClassDef/visitLambda/visitNewClass: do nothing
            Tag::CLASSDEF | Tag::LAMBDA | Tag::NEWCLASS => {}
            Tag::RETURN => {
                // Java: visitReturn
                let expr = match &ctx.tree(tree).kind {
                    crate::tree::jc_tree::TreeKind::Return { expr } => *expr,
                    _ => None,
                };
                if expr.is_some() {
                    self.is_void_compatible = false;
                } else {
                    self.is_potentially_value_compatible = false;
                }
            }
            _ => {
                for child in crate::tree::tree_scanner::tree_children(ctx, tree) {
                    self.scan(ctx, Some(child));
                }
            }
        }
    }
}


impl DeferredAttrService {

    /// Java: JCTree attribSpeculative(JCTree tree, Env, ResultInfo,
    /// TreeCopier<Z> deferredCopier, Supplier<DiagnosticHandler>,
    /// AttributionMode, LocalCacheContext) (DeferredAttr.java:496-520) —
    /// adapter: the caller-supplied copier (Analyzer.TreeRewriter) overload.
    ///
    /// # Arguments
    ///   - as above; copier is the analyzer TreeRewriter.
    ///
    /// # Returns
    ///   - Ok(the speculative tree).
    ///
    /// # Panics
    ///   - none beyond the registered dependencies.
    ///
    /// Upstream: DeferredAttr.java:496-520.
    #[allow(clippy::too_many_arguments)]
    pub fn attrib_speculative_with_rewriter(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        attr: &mut Attr,
        tree: TreeId,
        env: &Rc<Env<crate::comp::attr_context::AttrContext>>,
        result_info: &ResultInfo,
        copier: &crate::tree::tree_copier::TreeCopier,
        diag_handler_creator: Option<Box<dyn Fn() -> crate::util::log::DiagnosticHandler>>,
        attribution_mode: AttributionMode,
        local_cache: Option<crate::comp::argument_attr::LocalCacheContext>,
    ) -> Result<TreeId, DeferredAttrFailure> {
        let new_tree = copier.copy(ctx, Some(tree)).expect("copied tree");
        self.attrib_speculative_with_diag(
            ctx,
            syms,
            attr,
            new_tree,
            env,
            result_info,
            diag_handler_creator,
            attribution_mode,
            local_cache,
        )
    }
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
