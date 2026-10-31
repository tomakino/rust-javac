public sealed interface SealExtreme permits SealExtreme.Leaf, SealExtreme.Node, SealExtreme.Kind, SealExtreme.Open {
    record Leaf(int v) implements SealExtreme { }
    final class Node implements SealExtreme {
        final int x;
        Node(int x) { this.x = x; }
    }
    enum Kind implements SealExtreme { A, B }
    non-sealed interface Open extends SealExtreme { }

    static int size(SealExtreme s) {
        if (s instanceof Leaf leaf) return leaf.v();
        if (s instanceof Node node) return node.x;
        if (s instanceof Kind) return 0;
        return -1;
    }

    static final class Impl implements Open { }
}
