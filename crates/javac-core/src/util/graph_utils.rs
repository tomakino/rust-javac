// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P2/P3 utility-service.
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: nodeAttributes (GraphUtils.java:84)
// - NOT PORTED: dependencyAttributes (GraphUtils.java:88)
// - NOT PORTED: formatProperties (GraphUtils.java:252)
// - NOT PORTED: wrap (GraphUtils.java:258)
// End of Gap registration.

//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: the interface/abstract-class hierarchy of GraphUtils (DependencyKind, Node,
//! NodeVisitor, AbstractNode, TarjanNode) maps to traits, as recorded in PLAN.md section 5.
//! Java's mutable TarjanNode fields (int index = -1, int lowlink, boolean active) are read and
//! written by Tarjan through shared node references; they are modelled with Cell in TarjanState
//! so that the algorithm stays a literal translation while nodes are shared.
//! PORTING_RULES: AbstractNode.accept's shared implementation is the free function
//! accept_abstract_node because Rust does not allow a default trait method to turn &Self into &N
//! (the Java cast (N)this). Ported node classes delegate their Node::accept implementation to it
//! exactly like they inherit AbstractNode.accept in Java.
//! PORTING_RULES: the Collection/Iterable parameters of NodeVisitor.visit and
//! TarjanNode.getDependenciesByKind map to slices/Vec; the defensive new ArrayList(nodes) copy in
//! visit is not reproduced because javac nodes cannot be mutated while visited under Rust's
//! borrow rules.
//! PORTING_RULES: Java compares dependencies with List.contains, i.e. Object.equals. Tarjan
//! therefore requires N: PartialEq, and tarjan's n != v reference comparison is expressed with
//! std::ptr::eq.
//! PORTING_RULES: TRIMMED and registered in docs/STATUS.md and docs/SUPPORT_MATRIX.md:
//! DottableNode, DotVisitor and toDot are not ported. Their output depends on Object.hashCode()
//! (identity hash) and java.util.Properties.toString() (Hashtable iteration order), which cannot
//! be reproduced byte-for-byte; they only feed debugging -X output. Re-evaluate if that
//! debugging output ever becomes observable behavior in scope.

use std::cell::Cell;
use std::marker::PhantomData;

use super::list::List;
use super::list_buffer::ListBuffer;

/// Basic interface for defining various dependency kinds.
pub trait DependencyKind {}

/// Common superinterfaces to all graph nodes.
pub trait Node<D, N: Node<D, N>> {
    /// visitor method.
    fn accept<A>(&self, visitor: &mut dyn NodeVisitor<D, N, A>, arg: &mut A);
}

/// Visitor for graph nodes.
pub trait NodeVisitor<D, N: Node<D, N>, A> {
    /// Visitor action for nodes.
    fn visit_node(&mut self, node: &N, arg: &mut A);

    /// Visitor action for a dependency between 'from' and 'to' with given kind.
    fn visit_dependency(&mut self, dk: &dyn DependencyKind, from: &N, to: &N, arg: &mut A);

    /// Visitor entry point.
    ///
    /// Java: public void visit(Collection<? extends N> nodes, A arg) {
    ///     for (N n : new ArrayList<>(nodes)) { n.accept(this, arg); }
    /// }
    ///
    /// PORTING_RULES: Collection<? extends N> maps to an iterator of node references, because
    /// javac nodes are shared objects held elsewhere (arena entries in the port).
    fn visit<'a>(&mut self, nodes: impl IntoIterator<Item = &'a N>, arg: &mut A)
    where
        Self: Sized,
        N: 'a,
    {
        // PORTING_RULES: the Java defensive copy is not needed, see the file comment.
        for n in nodes {
            n.accept(self, arg);
        }
    }
}

/// This class is a basic abstract class for representing a node.
/// A node is associated with a given data.
pub trait AbstractNode<D, N: AbstractNode<D, N>>: Node<D, N> {
    /// Java: public final D data;
    fn data(&self) -> &D;

    // Java: public String toString() { return data.toString(); }
    // PORTING_RULES: implementors provide Display; a default method would conflict with the
    // Display impl required by the Rust idiom.

    /// Get an array of the dependency kinds supported by this node.
    fn get_supported_dependency_kinds(&self) -> Vec<&'static dyn DependencyKind>;

    /// Get all dependencies of a given kind
    fn get_dependencies_by_kind(&self, dk: &dyn DependencyKind) -> Vec<&N>;
}

/// Java: AbstractNode.accept(NodeVisitor<D, N, A> visitor, A arg)
pub fn accept_abstract_node<D, N, A>(
    node: &N,
    visitor: &mut dyn NodeVisitor<D, N, A>,
    arg: &mut A,
) where
    N: AbstractNode<D, N>,
{
    visitor.visit_node(node, arg);
    for dk in node.get_supported_dependency_kinds() {
        // Java: for (N dep : new ArrayList<>(getDependenciesByKind(dk)))
        for dep in node.get_dependencies_by_kind(dk) {
            visitor.visit_dependency(dk, node, dep, arg);
        }
    }
}

/// The Tarjan computation state of a node.
///
/// PORTING_RULES: Java keeps these three fields on the node object; the algorithm mutates them
/// through shared references, hence Cell.
pub struct TarjanState {
    /// Java: int index = -1;
    pub index: Cell<i32>,

    /// Java: int lowlink;
    pub lowlink: Cell<i32>,

    /// Java: boolean active;
    pub active: Cell<bool>,
}

impl TarjanState {
    pub fn new() -> TarjanState {
        TarjanState {
            index: Cell::new(-1),
            lowlink: Cell::new(0),
            active: Cell::new(false),
        }
    }
}

impl Default for TarjanState {
    fn default() -> Self {
        TarjanState::new()
    }
}

/// This class specialized Node, by adding elements that are required in order
/// to perform Tarjan computation of strongly connected components.
pub trait TarjanNode<D, N: TarjanNode<D, N>>: AbstractNode<D, N> {
    /// Java: (package private fields) int index = -1; int lowlink; boolean active;
    fn tarjan_state(&self) -> &TarjanState;

    /// Java: public abstract Iterable<? extends N> getAllDependencies();
    fn get_all_dependencies(&self) -> Vec<&N>;

    /// Java: public int compareTo(N o) {
    ///     return (index < o.index) ? -1 : (index == o.index) ? 0 : 1;
    /// }
    fn compare_to(&self, o: &N) -> i32 {
        let index = self.tarjan_state().index.get();
        let o_index = o.tarjan_state().index.get();
        if index < o_index {
            -1
        } else if index == o_index {
            0
        } else {
            1
        }
    }
}

/// Tarjan's algorithm to determine strongly connected components of a
/// directed graph in linear time. Works on TarjanNode.
///
/// Java: public static <D, N extends TarjanNode<D, N>> List<? extends List<? extends N>>
/// tarjan(Iterable<? extends N> nodes)
pub fn tarjan<'a, D, N>(nodes: impl IntoIterator<Item = &'a N>) -> List<List<&'a N>>
where
    D: 'a,
    N: TarjanNode<D, N> + PartialEq + 'a,
{
    let mut tarjan: Tarjan<'a, D, N> = Tarjan::new();
    tarjan.find_scc(nodes)
}

//where
/// Java: private static class Tarjan<D, N extends TarjanNode<D, N>>
struct Tarjan<'a, D, N: TarjanNode<D, N>> {
    /// Unique node identifier.
    ///
    /// Java: int index = 0;
    index: i32,

    /// List of SCCs found fso far.
    ///
    /// Java: ListBuffer<List<N>> sccs = new ListBuffer<>();
    sccs: ListBuffer<List<&'a N>>,

    /// Stack of all reacheable nodes from given root.
    ///
    /// Java: ListBuffer<N> stack = new ListBuffer<>();
    stack: ListBuffer<&'a N>,

    marker: PhantomData<D>,
}

impl<'a, D, N: TarjanNode<D, N> + PartialEq + 'a> Tarjan<'a, D, N> {
    fn new() -> Tarjan<'a, D, N> {
        Tarjan {
            index: 0,
            sccs: ListBuffer::new(),
            stack: ListBuffer::new(),
            marker: PhantomData,
        }
    }

    /// Java: private List<? extends List<? extends N>> findSCC(Iterable<? extends N> nodes)
    fn find_scc(&mut self, nodes: impl IntoIterator<Item = &'a N>) -> List<List<&'a N>> {
        for node in nodes {
            if node.tarjan_state().index.get() == -1 {
                self.find_scc_node(node);
            }
        }
        self.sccs.to_list()
    }

    /// Java: private void findSCC(N v)
    fn find_scc_node(&mut self, v: &'a N) {
        self.visit_node(v);
        for n in v.get_all_dependencies() {
            if n.tarjan_state().index.get() == -1 {
                //it's the first time we see this node
                self.find_scc_node(n);
                let lowlink = v
                    .tarjan_state()
                    .lowlink
                    .get()
                    .min(n.tarjan_state().lowlink.get());
                v.tarjan_state().lowlink.set(lowlink);
            } else if self.stack.contains(&n) {
                //this node is already reachable from current root
                let lowlink = v
                    .tarjan_state()
                    .lowlink
                    .get()
                    .min(n.tarjan_state().index.get());
                v.tarjan_state().lowlink.set(lowlink);
            }
        }
        if v.tarjan_state().lowlink.get() == v.tarjan_state().index.get() {
            //v is the root of a SCC
            self.add_scc(v);
        }
    }

    /// Java: private void visitNode(N n)
    fn visit_node(&mut self, n: &'a N) {
        let state = n.tarjan_state();
        state.index.set(self.index);
        state.lowlink.set(self.index);
        self.index += 1;
        self.stack.prepend(n);
        state.active.set(true);
    }

    /// Java: private void addSCC(N v)
    fn add_scc(&mut self, v: &'a N) {
        let mut cycle: ListBuffer<&'a N> = ListBuffer::new();
        loop {
            // Java: n = stack.remove();
            let n = self.stack.poll().expect("java.util.NoSuchElementException");
            n.tarjan_state().active.set(false);
            cycle.add(n);
            // Java: } while (n != v); — reference comparison, hence ptr::eq.
            if std::ptr::eq(n, v) {
                break;
            }
        }
        self.sccs.add(cycle.to_list());
    }
}


/// Tarjan SCC over an index graph (Porting rules adapter): the ported generic tarjan above
/// operates on &N references; the inference/deferred graphs keep their nodes in
/// index arenas (stable node ids), so this faithful translation of the same
/// algorithm (GraphUtils.java:171-235) works on dependency index lists.
pub fn tarjan_scc_indexed(deps: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let count = deps.len();
    let mut index_counter = 0i32;
    let mut stack: Vec<usize> = Vec::new();
    let mut sccs: Vec<Vec<usize>> = Vec::new();
    let mut index = vec![-1i32; count];
    let mut lowlink = vec![0i32; count];
    for start in 0..count {
        if index[start] == -1 {
            tarjan_scc_visit(
                start,
                deps,
                &mut index_counter,
                &mut index,
                &mut lowlink,
                &mut stack,
                &mut sccs,
            );
        }
    }
    sccs
}

fn tarjan_scc_visit(
    v: usize,
    deps: &[Vec<usize>],
    index_counter: &mut i32,
    index: &mut [i32],
    lowlink: &mut [i32],
    stack: &mut Vec<usize>,
    sccs: &mut Vec<Vec<usize>>,
) {
    index[v] = *index_counter;
    lowlink[v] = *index_counter;
    *index_counter += 1;
    stack.push(v);
    for &n in &deps[v] {
        if index[n] == -1 {
            tarjan_scc_visit(n, deps, index_counter, index, lowlink, stack, sccs);
            lowlink[v] = lowlink[v].min(lowlink[n]);
        } else if stack.contains(&n) {
            lowlink[v] = lowlink[v].min(index[n]);
        }
    }
    if lowlink[v] == index[v] {
        let mut cycle: Vec<usize> = Vec::new();
        loop {
            let n = stack.pop().expect("java.util.NoSuchElementException");
            cycle.push(n);
            if n == v {
                break;
            }
        }
        sccs.push(cycle);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    struct TestKind;
    impl DependencyKind for TestKind {}
    static TEST_KIND: TestKind = TestKind;

    struct TestNode {
        data: i32,
        deps: RefCell<Vec<&'static TestNode>>,
        state: TarjanState,
    }

    fn node(data: i32) -> &'static TestNode {
        Box::leak(Box::new(TestNode {
            data,
            deps: RefCell::new(Vec::new()),
            state: TarjanState::new(),
        }))
    }

    fn link(from: &'static TestNode, to: &'static TestNode) {
        from.deps.borrow_mut().push(to);
    }

    impl Node<i32, TestNode> for TestNode {
        fn accept<A>(&self, visitor: &mut dyn NodeVisitor<i32, TestNode, A>, arg: &mut A) {
            accept_abstract_node(self, visitor, arg);
        }
    }

    impl AbstractNode<i32, TestNode> for TestNode {
        fn data(&self) -> &i32 {
            &self.data
        }

        fn get_supported_dependency_kinds(&self) -> Vec<&'static dyn DependencyKind> {
            vec![&TEST_KIND]
        }

        fn get_dependencies_by_kind(&self, _dk: &dyn DependencyKind) -> Vec<&TestNode> {
            self.deps.borrow().iter().copied().collect()
        }
    }

    impl TarjanNode<i32, TestNode> for TestNode {
        fn tarjan_state(&self) -> &TarjanState {
            &self.state
        }

        fn get_all_dependencies(&self) -> Vec<&TestNode> {
            self.deps.borrow().iter().copied().collect()
        }
    }

    impl PartialEq for TestNode {
        fn eq(&self, other: &Self) -> bool {
            std::ptr::eq(self, other)
        }
    }

    impl Eq for TestNode {}

    fn collected(sccs: &List<List<&'static TestNode>>) -> Vec<Vec<i32>> {
        let mut result = Vec::new();
        for scc in sccs.iter() {
            let mut ids = Vec::new();
            for n in scc.iter() {
                ids.push(n.data);
            }
            result.push(ids);
        }
        result
    }

    #[test]
    fn single_node_is_its_own_scc() {
        let a = node(1);
        let sccs = tarjan::<i32, TestNode>(vec![a]);
        assert_eq!(collected(&sccs), vec![vec![1]]);
    }

    #[test]
    fn chain_yields_singletons_in_completion_order() {
        let a = node(1);
        let b = node(2);
        let c = node(3);
        link(a, b);
        link(b, c);
        let sccs = tarjan::<i32, TestNode>(vec![a]);
        assert_eq!(collected(&sccs), vec![vec![3], vec![2], vec![1]]);
    }

    #[test]
    fn cycle_is_one_scc() {
        let a = node(1);
        let b = node(2);
        link(a, b);
        link(b, a);
        let sccs = tarjan::<i32, TestNode>(vec![a]);
        assert_eq!(collected(&sccs), vec![vec![2, 1]]);
    }

    #[test]
    fn three_node_cycle_is_one_scc() {
        let a = node(1);
        let b = node(2);
        let c = node(3);
        link(a, b);
        link(b, c);
        link(c, a);
        let sccs = tarjan::<i32, TestNode>(vec![a]);
        assert_eq!(collected(&sccs), vec![vec![3, 2, 1]]);
    }

    #[test]
    fn disconnected_graph_visits_all_nodes() {
        let a = node(1);
        let b = node(2);
        let sccs = tarjan::<i32, TestNode>(vec![a, b]);
        assert_eq!(collected(&sccs), vec![vec![1], vec![2]]);
    }

    #[test]
    fn tarjan_state_is_filled_in() {
        let a = node(1);
        let _ = tarjan::<i32, TestNode>(vec![a]);
        assert_eq!(a.state.index.get(), 0);
        assert_eq!(a.state.lowlink.get(), 0);
        assert!(!a.state.active.get());
    }

    struct RecordingVisitor {
        nodes: Vec<i32>,
        deps: Vec<(i32, i32)>,
    }

    impl NodeVisitor<i32, TestNode, i32> for RecordingVisitor {
        fn visit_node(&mut self, node: &TestNode, _arg: &mut i32) {
            self.nodes.push(node.data);
        }

        fn visit_dependency(
            &mut self,
            _dk: &dyn DependencyKind,
            from: &TestNode,
            to: &TestNode,
            _arg: &mut i32,
        ) {
            self.deps.push((from.data, to.data));
        }
    }

    #[test]
    fn accept_visits_node_and_dependencies() {
        let a = node(1);
        let b = node(2);
        link(a, b);
        let mut visitor = RecordingVisitor {
            nodes: Vec::new(),
            deps: Vec::new(),
        };
        let mut arg = 0;
        a.accept(&mut visitor, &mut arg);
        assert_eq!(visitor.nodes, vec![1]);
        assert_eq!(visitor.deps, vec![(1, 2)]);
    }

    #[test]
    fn visitor_visit_entry_point() {
        let a = node(1);
        let b = node(2);
        let mut visitor = RecordingVisitor {
            nodes: Vec::new(),
            deps: Vec::new(),
        };
        let mut arg = 0;
        visitor.visit(vec![a, b], &mut arg);
        assert_eq!(visitor.nodes, vec![1, 2]);
        assert!(visitor.deps.is_empty());
    }

    #[test]
    fn compare_to_uses_tarjan_index() {
        let a = node(1);
        let b = node(2);
        let _ = tarjan::<i32, TestNode>(vec![a, b]);
        assert_eq!(a.compare_to(b), -1);
        assert_eq!(b.compare_to(a), 1);
        assert_eq!(a.compare_to(a), 0);
    }
}
