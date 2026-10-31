public class SealedAdvanced {
    sealed interface Node permits Leaf, Branch, NonSealedNode {
    }

    record Leaf(int value) implements Node {
    }

    record Branch(Node left, Node right) implements Node {
    }

    non-sealed interface NonSealedNode extends Node {
    }

    static int sum(Node node) {
        if (node instanceof Leaf leaf) {
            return leaf.value();
        }
        if (node instanceof Branch branch) {
            return sum(branch.left()) + sum(branch.right());
        }
        return 0;
    }

    public static void main(String[] args) {
        Node tree = new Branch(new Leaf(1), new Branch(new Leaf(2), new Leaf(3)));
        System.out.println(sum(tree));
    }
}
