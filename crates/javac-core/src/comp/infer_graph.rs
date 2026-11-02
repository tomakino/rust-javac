//! Graph-based inference: strategies, the inference graph and its solver.

use std::cell::RefCell;
use std::rc::Rc;

use crate::code::symtab::Symtab;
use crate::code::type_::{InferenceBound, TypeKindData};
use crate::code::type_tag::TypeTag;
use crate::code::types::Types;
use crate::resources::compiler_properties::fragments;
use crate::resources::diagnostic_contract::TypeArg;
use crate::comp::infer::{contains_any_list, new_captured_type_ctx, Infer, InferenceContext, InferenceException};
use crate::compiler_context::CompilerContext;
use crate::ids::TypeId;
use crate::util::list::List;
use crate::util::warner::Warner;

/// Java: enum GraphInferenceSteps / LegacyInferenceSteps step sets
/// (Infer.java:1563-1598).
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InferenceStep {
    EQ,
    LOWER,
    THROWS,
    UPPER,
    UPPER_LEGACY,
    CAPTURED,
}

/// Java: LegacyInferenceSteps.EQ_LOWER / EQ_UPPER
pub fn legacy_inference_steps_eq_lower() -> Vec<InferenceStep> {
    vec![InferenceStep::EQ, InferenceStep::LOWER]
}

/// Java: LegacyInferenceSteps.EQ_UPPER
pub fn legacy_inference_steps_eq_upper() -> Vec<InferenceStep> {
    vec![InferenceStep::EQ, InferenceStep::UPPER_LEGACY]
}

/// Java: GraphInferenceSteps (Infer.java:1580-1598), in declaration order.
pub fn graph_inference_steps() -> Vec<Vec<InferenceStep>> {
    vec![
        vec![InferenceStep::EQ],
        vec![InferenceStep::EQ, InferenceStep::LOWER],
        vec![
            InferenceStep::EQ,
            InferenceStep::LOWER,
            InferenceStep::UPPER,
            InferenceStep::THROWS,
            InferenceStep::CAPTURED,
        ],
    ]
}

impl InferenceStep {
    /// Java: InferenceStep.ib
    pub fn ib(self) -> InferenceBound {
        match self {
            InferenceStep::EQ => InferenceBound::EQ,
            InferenceStep::LOWER => InferenceBound::LOWER,
            InferenceStep::THROWS => InferenceBound::UPPER,
            InferenceStep::UPPER => InferenceBound::UPPER,
            InferenceStep::UPPER_LEGACY => InferenceBound::UPPER,
            InferenceStep::CAPTURED => InferenceBound::UPPER,
        }
    }

    /// Java: public boolean accepts(UndetVar t, InferenceContext
    /// inferenceContext) (Infer.java:1546-1548 plus the overrides at
    /// 1453-1464/1494-1500/1509-1515)
    pub fn accepts(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        uv: TypeId,
        inference_context: &InferenceContext,
    ) -> Result<bool, InferenceException> {
        match self {
            InferenceStep::THROWS => {
                if !types.undet_var_is_throws(ctx, uv) {
                    return Ok(false);
                }
                let bounds =
                    types.undet_var_get_bounds(ctx, uv, &[InferenceBound::UPPER]);
                for b in &bounds {
                if !inference_context.free(ctx, syms, types, b)
                    && !types.is_subtype(
                        ctx,
                        syms,
                        syms.runtime_exception_type,
                        b,
                    )
                {
                    return Ok(false);
                }
                }
                Ok(true)
            }
            InferenceStep::UPPER_LEGACY => {
                let bounds =
                    types.undet_var_get_bounds(ctx, uv, &[self.ib()]);
                Ok(!inference_context.free_list(ctx, syms, types, bounds)
                    && !types.undet_var_is_captured(ctx, uv))
            }
            InferenceStep::CAPTURED => {
                if !types.undet_var_is_captured(ctx, uv) {
                    return Ok(false);
                }
                let bounds = types.undet_var_get_bounds(
                    ctx,
                    uv,
                    &[InferenceBound::UPPER, InferenceBound::LOWER],
                );
                Ok(!inference_context.free_list(ctx, syms, types, bounds))
            }
            _ => {
                let filtered = self.filter_bounds(ctx, syms, types, uv, inference_context)?;
                Ok(!filtered.is_empty() && !types.undet_var_is_captured(ctx, uv))
            }
        }
    }

    /// Java: abstract Type solve(UndetVar uv, InferenceContext
    /// inferenceContext) (Infer.java:1425-1531)
    pub fn solve(
        &self,
        infer: &Infer,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        uv: TypeId,
        inference_context: &InferenceContext,
    ) -> Result<TypeId, InferenceException> {
        match self {
            InferenceStep::EQ => {
                let filtered = self.filter_bounds(ctx, syms, types, uv, inference_context)?;
                Ok(*filtered.head())
            }
            InferenceStep::LOWER => {
                let lobounds = self.filter_bounds(ctx, syms, types, uv, inference_context)?;
                let owntype = if lobounds.tail().is_empty() {
                    *lobounds.head()
                } else {
                    let names = Rc::clone(&infer.names);
                    types.lub_list(ctx, syms, &names, lobounds)
                };
                if ctx.typ(owntype).is_primitive() || ctx.typ(owntype).has_tag(TypeTag::ERROR) {
                    let diag = infer.diags.fragment(
                        fragments::no_unique_minimal_instance_exists(
                            TypeArg::placeholder("type"),
                            vec![TypeArg::placeholder("type")],
                        )
                        .into(),
                    );
                    Err(infer.error(Some(diag)))
                } else {
                    Ok(owntype)
                }
            }
            InferenceStep::THROWS => Ok(syms.runtime_exception_type),
            InferenceStep::UPPER | InferenceStep::UPPER_LEGACY => {
                let hibounds = self.filter_bounds(ctx, syms, types, uv, inference_context)?;
                let owntype = if hibounds.tail().is_empty() {
                    *hibounds.head()
                } else {
                    types.glb_list(ctx, syms, hibounds)
                };
                if ctx.typ(owntype).is_primitive() || ctx.typ(owntype).has_tag(TypeTag::ERROR) {
                    let diag = infer.diags.fragment(
                        fragments::no_unique_maximal_instance_exists(
                            TypeArg::placeholder("type"),
                            vec![TypeArg::placeholder("type")],
                        )
                        .into(),
                    );
                    Err(infer.error(Some(diag)))
                } else {
                    Ok(owntype)
                }
            }
            InferenceStep::CAPTURED => {
                let upper_filtered =
                    InferenceStep::UPPER.filter_bounds(ctx, syms, types, uv, inference_context)?;
                let upper = if !upper_filtered.is_empty() {
                    InferenceStep::UPPER.solve(infer, ctx, syms, types, uv, inference_context)?
                } else {
                    syms.object_type
                };
                let lower_filtered =
                    InferenceStep::LOWER.filter_bounds(ctx, syms, types, uv, inference_context)?;
                let lower = if !lower_filtered.is_empty() {
                    InferenceStep::LOWER.solve(infer, ctx, syms, types, uv, inference_context)?
                } else {
                    syms.bot_type
                };
                let qtype = types.undet_var_qtype(ctx, uv);
                let (name, owner, wildcard) = match &ctx.typ(qtype).kind {
                    TypeKindData::CapturedType { wildcard, .. } => {
                        let qtype_tsym = ctx.typ(qtype).tsym.expect("captured symbol");
                        (
                            ctx.sym(qtype_tsym).name.clone(),
                            ctx.sym(qtype_tsym).owner,
                            *wildcard,
                        )
                    }
                    _ => panic!("AssertionError: CAPTURED on a non-captured type"),
                };
                Ok(new_captured_type_ctx(
                    ctx, syms, name, owner, upper, lower, wildcard,
                ))
            }
        }
    }

    /// Java: List<Type> filterBounds(UndetVar uv, InferenceContext
    /// inferenceContext) (Infer.java:1553-1556)
    fn filter_bounds(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        uv: TypeId,
        inference_context: &InferenceContext,
    ) -> Result<List<TypeId>, InferenceException> {
        let bounds = types.undet_var_get_bounds(ctx, uv, &[self.ib()]);
        let mut result: List<TypeId> = List::nil();
        let current = bounds;
        for t in &current {
        if bound_filter(ctx, syms, types, inference_context, t) {
            result = result.append(t);
        }
        }
        Ok(result)
    }
}

/// Java: Infer.BoundFilter.test (Infer.java:1233-1245)
pub(crate) fn bound_filter(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    inference_context: &InferenceContext,
    t: TypeId,
) -> bool {
    !types.is_erroneous_type(ctx, syms, t)
        && !inference_context.free(ctx, syms, types, t)
        && !ctx.typ(t).has_tag(TypeTag::BOT)
}

/// Java: interface GraphStrategy + NodeNotFoundException
/// (Infer.java:1299-1326)
pub trait GraphStrategy {
    fn pick_node(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        graph: &InferenceGraph,
        inference_context: &InferenceContext,
    ) -> Result<usize, NodeNotFoundException>;

    fn done(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        inference_context: &InferenceContext,
    ) -> Result<bool, InferenceException>;
}

/// Java: public static class NodeNotFoundException extends RuntimeException
#[derive(Debug)]
pub struct NodeNotFoundException {
    pub node_count: usize,
}

/// Java: abstract class LeafSolver (Infer.java:1309-1341)
pub struct LeafSolver {
    pub context: std::rc::Weak<InferenceContext>,
}

impl GraphStrategy for LeafSolver {
    fn pick_node(
        &self,
        _ctx: &mut CompilerContext,
        _syms: &Symtab,
        _types: &Types,
        graph: &InferenceGraph,
        _inference_context: &InferenceContext,
    ) -> Result<usize, NodeNotFoundException> {
        // The node list drops deleted entries; the port keeps stable ids with
        // None holes, so the first live node is returned here.
        match graph.nodes.iter().position(|node| node.is_some()) {
            Some(index) => Ok(index),
            None => Err(NodeNotFoundException {
                node_count: graph.nodes.len(),
            }),
        }
    }

    fn done(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        inference_context: &InferenceContext,
    ) -> Result<bool, InferenceException> {
        let _ = self.context.upgrade();
        Ok(inference_context.restvars(ctx, syms, types).is_empty())
    }
}

/// Java: abstract class BestLeafSolver extends LeafSolver (Infer.java:1319-1341)
/// Java: Map<Node, Pair<List<Node>, Integer>> treeCache (Inference.java:1357)
pub type TreeCache = RefCell<Vec<(usize, (Vec<usize>, i32))>>;

/// /// Java: Infer.GraphSolver.BestLeafSolver (Infer.java:1290-1330).
///
/// # Panics
///   - none.
///
/// Upstream: Infer.java:1290-1330.
///
pub struct BestLeafSolver {
    pub context: std::rc::Weak<InferenceContext>,
    pub vars_to_solve: List<TypeId>,
    pub tree_cache: TreeCache,
}

impl BestLeafSolver {
    /// Java: Pair<List<Node>, Integer> computeTreeToLeafs(Node n)
    fn compute_tree_to_leafs(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        graph: &InferenceGraph,
        node: usize,
    ) -> (Vec<usize>, i32) {
        if let Some((_, cached)) = self.tree_cache.borrow().iter().find(|(n, _)| *n == node) {
            return cached.clone();
        }
        let node_data = graph.nodes[node].as_ref().expect("live node");
        let result = if node_data.is_leaf(graph) {
            (vec![node], node_data.data.length())
        } else {
            let mut path = (vec![node], node_data.data.length());
            for n2 in node_data.get_all_dependencies(graph) {
                if n2 == node {
                    continue;
                }
                let subpath = self.compute_tree_to_leafs(ctx, syms, types, graph, n2);
                // Java: path.fst.prependList(subpath.fst) (Infer.java:1371) puts
                // the subpath *before* this node, so the resulting list starts at
                // a leaf. BestLeafSolver.pickNode then returns bestPath.fst.head
                // (Infer.java:1409) — the leaf, not the node it was reached
                // from. Appending the subpath instead would make every pick the
                // dependent node, solving it before the variables in its bounds
                // were instantiated.
                let mut combined = subpath.0.clone();
                combined.extend(path.0.clone());
                path = (combined, path.1 + subpath.1);
            }
            path
        };
        self.tree_cache.borrow_mut().push((node, result.clone()));
        let _ = (ctx, syms, types);
        result
    }
}

impl GraphStrategy for BestLeafSolver {
    fn pick_node(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        graph: &InferenceGraph,
        inference_context: &InferenceContext,
    ) -> Result<usize, NodeNotFoundException> {
        let _ = inference_context;
        self.tree_cache.borrow_mut().clear();
        let mut best: Option<(Vec<usize>, i32)> = None;
        for (index, node) in graph.nodes.iter().enumerate() {
            let Some(node) = node else { continue };
            if !contains_any_list(ctx, syms, types, node.data.clone(), self.vars_to_solve.clone()) {
                continue;
            }
            let path = self.compute_tree_to_leafs(ctx, syms, types, graph, index);
            if best.as_ref().map(|(_, cost)| path.1 < *cost).unwrap_or(true) {
                best = Some(path);
            }
        }
        match best {
            Some((path, _)) => Ok(path[0]),
            None => Err(NodeNotFoundException {
                node_count: graph.nodes.len(),
            }),
        }
    }

    fn done(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        inference_context: &InferenceContext,
    ) -> Result<bool, InferenceException> {
        let _ = self.context.upgrade();
        let as_inst = inference_context.as_inst_types(ctx, syms, types, self.vars_to_solve.clone());
        Ok(!inference_context.free_list(ctx, syms, types, as_inst))
    }
}

/// Java: enum DependencyKind implements GraphUtils.DependencyKind
/// (Infer.java:1601-1610; kept for the Porting rules mapping of the graph API)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DependencyKind {
    BOUND,
    STUCK,
}

impl DependencyKind {
/// /// Java: Infer.DependencyKind.dotStyle (Infer.java:1350-1370).
///
/// # Panics
///   - none.
///
/// Upstream: Infer.java:1350-1370.
///
    pub fn dot_style(self) -> &'static str {
        match self {
            DependencyKind::BOUND => "dotted",
            DependencyKind::STUCK => "dashed",
        }
    }
}

/// Java: class GraphSolver (Infer.java:1623-1945)
pub struct GraphSolver<'a> {
    pub inference_context: &'a Rc<InferenceContext>,
}

impl<'a> GraphSolver<'a> {
/// /// Java: GraphSolver(InferenceContext) (Infer.java:1370-1390).
///
/// # Panics
///   - none.
///
/// Upstream: Infer.java:1370-1390.
///
    pub fn new(inference_context: &'a Rc<InferenceContext>) -> GraphSolver<'a> {
        GraphSolver { inference_context }
    }

    /// Java: void solve(GraphStrategy sstrategy) (Infer.java:1638-1666)
    pub fn solve(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        infer: &Infer,
        strategy: Rc<dyn GraphStrategy>,
        warn: &Warner,
    ) -> Result<(), InferenceException> {
        infer.do_incorporation(ctx, syms, types, self.inference_context, warn)?;
        let mut graph = InferenceGraph::new(ctx, syms, types, self.inference_context);
        let mut iterations = 0;
        while !strategy.done(ctx, syms, types, self.inference_context)? {
            iterations += 1;
            if iterations > 1_000_000 {
                return Err(infer.error(None));
            }
            let node_to_solve = match strategy.pick_node(
                ctx,
                syms,
                types,
                &graph,
                self.inference_context,
            ) {
                Ok(node) => node,
                Err(e) => {
                    if std::env::var_os("DSH_DEBUG_GS").is_some() {
                        eprintln!("[GS] pick_node failed: {e:?}");
                    }
                    return Err(infer.error(None));
                }
            };
            let vars_to_solve = graph.nodes[node_to_solve]
                .as_ref()
                .expect("live node")
                .data
                .clone();
            let saved_undet = self.inference_context.save(ctx, syms, types);
            // The failure is caught around the whole solve loop and recovered.
            //
            // Port adapter (A3k): the ported InferenceException travels as the
            // Err value of the solve calls, so the upstream catch is expressed
            // as one outcome variable covering the whole loop body. A failure
            // is not swallowed: the same rollback/recovery as upstream runs and
            // the recovery's own failures still propagate.
            let mut inference_failure: Option<InferenceException> = None;
            let mut solved_all = false;
            'outer: while !solved_all {
                let restvars = self.inference_context.restvars(ctx, syms, types);
                if !contains_any_list(ctx, syms, types, restvars, vars_to_solve.clone()) {
                    solved_all = true;
                    break;
                }
                let mut progressed = false;
                for steps in graph_inference_steps() {
                    let solved = match self.inference_context.solve_basic(
                        ctx,
                        syms,
                        types,
                        vars_to_solve.clone(),
                        &steps,
                    ) {
                        Ok(solved) => solved,
                        Err(ex) => {
                            inference_failure = Some(ex);
                            break 'outer;
                        }
                    };
                    if !solved.is_empty() {
                        if let Err(ex) = infer.do_incorporation(
                            ctx,
                            syms,
                            types,
                            self.inference_context,
                            warn,
                        ) {
                            inference_failure = Some(ex);
                            break 'outer;
                        }
                        progressed = true;
                        continue 'outer;
                    }
                }
                if !progressed {
                    // No progress: report an inference failure.
                    inference_failure = Some(infer.error(None));
                    break;
                }
            }
            if inference_failure.is_some() {
                self.inference_context
                    .rollback(ctx, syms, types, saved_undet);
                infer.instantiate_as_uninferred_vars(
                    ctx,
                    syms,
                    types,
                    vars_to_solve.clone(),
                    self.inference_context,
                )?;
                infer.do_incorporation(ctx, syms, types, self.inference_context, warn)?;
            }
            graph.delete_node(ctx, syms, types, self.inference_context, node_to_solve);
        }
        Ok(())
    }
}

/// Java: class InferenceGraph (Infer.java:1678-1917)
pub struct InferenceGraph {
    /// The nodes; None marks a deleted node (the graph keeps stable node ids so
    /// that dependency edges stay valid while nodes are deleted — Porting rules adapter).
    pub nodes: Vec<Option<GraphNode>>,
}

/// Java: InferenceGraph.Node (Infer.java:1685-1850)
#[derive(Clone)]
pub struct GraphNode {
    pub data: List<TypeId>,
    pub deps: Vec<usize>,
    pub index: i32,
    pub lowlink: i32,
    pub active: bool,
}

impl GraphNode {
    fn is_leaf(&self, _graph: &InferenceGraph) -> bool {
        self.deps.is_empty()
    }

    fn get_all_dependencies(&self, _graph: &InferenceGraph) -> Vec<usize> {
        self.deps.clone()
    }
}

impl InferenceGraph {
    /// Java: InferenceGraph() { initNodes(); } (Infer.java:1837-1917)
    pub fn new(
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        inference_context: &InferenceContext,
    ) -> InferenceGraph {
        let mut graph = InferenceGraph { nodes: Vec::new() };
        // add nodes
        let restvars = inference_context.restvars(ctx, syms, types);
        let mut current = restvars;
        while current.non_empty() {
            graph.nodes.push(Some(GraphNode {
                data: List::of(*current.head()),
                deps: Vec::new(),
                index: -1,
                lowlink: 0,
                active: false,
            }));
            current = current.tail();
        }
        // add dependencies
        let count = graph.nodes.len();
        for i in 0..count {
            let i_type = *graph.nodes[i].as_ref().expect("live node").data.head();
            for j in 0..count {
                let j_type = *graph.nodes[j].as_ref().expect("live node").data.head();
                if i != j {
                    let uv_i = inference_context.as_undet_var(ctx, syms, types, i_type);
                    let bounds = types.undet_var_get_bounds(
                        ctx,
                        uv_i,
                        &InferenceBound::values(),
                    );
                    if contains_any_list(ctx, syms, types, bounds, List::of(j_type)) {
                        graph.nodes[i].as_mut().expect("live node").deps.push(j);
                    }
                }
            }
        }
        // merge cyclic nodes
        //
        // Java: InferenceGraph.initNodes (Infer.java:1880-1914). Upstream keeps
        // every node in the list while it merges the cyclic components and only
        // then swaps in the acyclic list of component heads. The port must not
        // remove a head from the list while later components are still being
        // merged: notifyUpdate() walks the live nodes, so a head that has
        // already been taken out of the list would keep stale dependency ids
        // (the merge of a later component cannot rewrite its edges), which in
        // turn changes the BestLeafSolver path costs and the solve order.
        // Take the final snapshot of the heads after all merges have been
        // applied, then remap the pre-merge ids to the compacted ids.
        let sccs = graph.tarjan_scc();
        let mut heads: Vec<usize> = Vec::new();
        let mut old_to_new: Vec<Option<usize>> = vec![None; count];
        for scc in &sccs {
            if scc.len() > 1 {
                let root = scc[0];
                let tail: Vec<usize> = scc[1..].to_vec();
                graph.merge_with(root, &tail, count);
                for n in scc {
                    graph.notify_update(*n, Some(root), count);
                }
            }
            let head = scc[0];
            let new_index = heads.len();
            for n in scc {
                old_to_new[*n] = Some(new_index);
            }
            heads.push(head);
        }
        let mut acyclic_nodes: Vec<Option<GraphNode>> = heads
            .iter()
            .map(|head| graph.nodes[*head].clone())
            .collect();
        for node in acyclic_nodes.iter_mut().flatten() {
            let mut new_deps: Vec<usize> = Vec::new();
            for dep in &node.deps {
                if let Some(Some(new_dep)) = old_to_new.get(*dep) {
                    if !new_deps.contains(new_dep) {
                        new_deps.push(*new_dep);
                    }
                }
            }
            node.deps = new_deps;
        }
        graph.nodes = acyclic_nodes;
        graph
    }

    /// Java: InferenceGraph.Node.closure() — the set of nodes reachable
    /// through dependencies (used by DeferredAttr.canInfluence).
    pub fn node_closure(&self, node: usize) -> Vec<usize> {
        let mut closure: Vec<usize> = Vec::new();
        self.collect_closure(node, &mut closure);
        closure
    }

    fn collect_closure(&self, node: usize, closure: &mut Vec<usize>) {
        if closure.contains(&node) {
            return;
        }
        closure.push(node);
        if let Some(data) = self.nodes.get(node).and_then(|n| n.as_ref()) {
            for dep in &data.deps {
                self.collect_closure(*dep, closure);
            }
        }
    }

    /// Java: Node findNode(Type t)
    #[allow(clippy::collapsible_if)]
    pub fn find_node(&self, t: TypeId) -> Option<usize> {
        for (index, node) in self.nodes.iter().enumerate() {
            if let Some(node) = node {
                if node.data.iter().any(|x| x == t) {
                    return Some(index);
                }
            }
        }
        None
    }

    /// Java: void deleteNode(Node n)
    pub fn delete_node(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        _inference_context: &InferenceContext,
        node: usize,
    ) {
        let _ = (ctx, syms, types);
        self.nodes[node] = None;
        self.notify_update(node, None, self.nodes.len());
    }

    /// Java: void notifyUpdate(Node from, Node to)
    fn notify_update(&mut self, from: usize, to: Option<usize>, count: usize) {
        for i in 0..count {
            if self.nodes.get(i).map(|n| n.is_some()).unwrap_or(false) {
                self.graph_changed(i, from, to);
            }
        }
    }

    /// Java: Node.graphChanged(Node from, Node to)
    fn graph_changed(&mut self, node: usize, from: usize, to: Option<usize>) {
        let Some(node_data) = self.nodes.get_mut(node).and_then(|n| n.as_mut()) else {
            return;
        };
        if let Some(position) = node_data.deps.iter().position(|d| *d == from) {
            node_data.deps.remove(position);
            if let Some(to) = to {
                node_data.deps.push(to);
            }
        }
    }

    /// Java: mergeWith(List<? extends Node> nodes) (Infer.java:1755-1780)
    fn merge_with(&mut self, root: usize, tail: &[usize], count: usize) {
        for n in tail {
            let n_data = self.nodes[*n].as_ref().expect("live node").clone();
            let root_data = self.nodes[root].as_mut().expect("live node");
            assert!(
                n_data.data.length() == 1,
                "AssertionError: Attempt to merge a compound node!"
            );
            let mut data = root_data.data.clone();
            let mut current = n_data.data;
            while current.non_empty() {
                data = data.append(*current.head());
                current = current.tail();
            }
            root_data.data = data;
            for dep in n_data.deps {
                if !root_data.deps.contains(&dep) {
                    root_data.deps.push(dep);
                }
            }
        }
        // update deps: dependencies on merged members become self-dependencies
        let members: Vec<TypeId> = self.nodes[root]
            .as_ref()
            .expect("live node")
            .data
            .iter()
            .collect();
        let mut new_deps: Vec<usize> = Vec::new();
        let old_deps = self.nodes[root].as_ref().expect("live node").deps.clone();
        for dep in old_deps {
            let Some(dep_data) = self.nodes.get(dep).and_then(|n| n.as_ref()) else {
                continue;
            };
            let first = *dep_data.data.head();
            if members.contains(&first) {
                if !new_deps.contains(&root) {
                    new_deps.push(root);
                }
            } else if !new_deps.contains(&dep) {
                new_deps.push(dep);
            }
        }
        self.nodes[root].as_mut().expect("live node").deps = new_deps;
        let _ = count;
    }

    /// GraphUtils.tarjan over the live node ids (the faithful port of
    /// GraphUtils.java:171-235, adapted to the stable node ids; see the file
    /// header registration).
    fn tarjan_scc(&mut self) -> Vec<Vec<usize>> {
        let count = self.nodes.len();
        let mut sccs: Vec<Vec<usize>> = Vec::new();
        let mut index_counter = 0i32;
        let mut stack: Vec<usize> = Vec::new();
        for start in 0..count {
            if self.nodes[start].is_none() {
                continue;
            }
            if self.nodes[start].as_ref().expect("live node").index != -1 {
                continue;
            }
            self.tarjan_visit(start, &mut index_counter, &mut stack, &mut sccs);
        }
        sccs
    }

    fn tarjan_visit(
        &mut self,
        v: usize,
        index: &mut i32,
        stack: &mut Vec<usize>,
        sccs: &mut Vec<Vec<usize>>,
    ) {
        // visitNode
        {
            let state = self.nodes[v].as_mut().expect("live node");
            state.index = *index;
            state.lowlink = *index;
            *index += 1;
            state.active = true;
        }
        stack.push(v);
        let deps = self.nodes[v].as_ref().expect("live node").deps.clone();
        for n in deps {
            if self.nodes.get(n).map(|x| x.is_none()).unwrap_or(true) {
                continue;
            }
            let n_index = self.nodes[n].as_ref().expect("live node").index;
            if n_index == -1 {
                self.tarjan_visit(n, index, stack, sccs);
                let n_lowlink = self.nodes[n].as_ref().expect("live node").lowlink;
                let v_lowlink = self.nodes[v].as_ref().expect("live node").lowlink;
                self.nodes[v].as_mut().expect("live node").lowlink = v_lowlink.min(n_lowlink);
            } else if stack.contains(&n) {
                let n_index = self.nodes[n].as_ref().expect("live node").index;
                let v_lowlink = self.nodes[v].as_ref().expect("live node").lowlink;
                self.nodes[v].as_mut().expect("live node").lowlink = v_lowlink.min(n_index);
            }
        }
        let (lowlink, node_index) = {
            let state = self.nodes[v].as_ref().expect("live node");
            (state.lowlink, state.index)
        };
        if lowlink == node_index {
            // v is the root of a SCC
            let mut cycle: Vec<usize> = Vec::new();
            loop {
                let n = stack.pop().expect("java.util.NoSuchElementException");
                self.nodes[n].as_mut().expect("live node").active = false;
                cycle.push(n);
                if n == v {
                    break;
                }
            }
            sccs.push(cycle);
        }
    }
}

// =====================================================================
// ReachabilityVisitor + deferred listeners (InferenceContext.java:406-484,
// 248-284)
// =====================================================================

