import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.Function;

public class GenInfer {
    interface A<X> { X get(); }
    interface B<Y> extends A<List<Y>> {}

    static <T> T pick(T a, T b) { return a == null ? b : a; }
    static <T, R> R apply(Function<? super T, ? extends R> f, T value) { return f.apply(value); }
    static <K, V> Map<K, V> pair(K k, V v) { return Map.of(k, v); }

    static class Holder<Z> {
        final Z value;
        Holder(Z value) { this.value = value; }
        <U> Holder<U> map(Function<? super Z, ? extends U> f) { return new Holder<>(f.apply(value)); }
    }

    static class Node<T extends Comparable<? super T>> implements Comparable<Node<T>> {
        final T key;
        Node(T key) { this.key = key; }
        public int compareTo(Node<T> other) { return key.compareTo(other.key); }
    }

    static <T extends Comparable<? super T>> T max(List<? extends T> list) {
        T best = null;
        for (T item : list) {
            if (best == null || item.compareTo(best) > 0) best = item;
        }
        return best;
    }

    static <E> void drain(List<? super E> sink, Iterable<? extends E> source) {
        for (E item : source) sink.add(item);
    }

    static void wild(Set<? extends Map<String, ? extends List<Integer>>> set) {
        for (Map<String, ? extends List<Integer>> map : set) {
            for (Map.Entry<String, ? extends List<Integer>> entry : map.entrySet()) {
                List<Integer> list = entry.getValue();
                if (!list.isEmpty()) System.out.print(list.get(0));
            }
        }
    }

    public static void main(String[] args) {
        Holder<String> h = new Holder<>("x");
        Holder<Integer> i = h.map(String::length);
        Map<String, Integer> m = pair("k", i.value);
        List<Integer> sink = new ArrayList<>();
        drain(sink, List.of(1, 2, 3));
        System.out.println(pick(m.get("k"), 0) + max(List.of(3, 1, 2)) + sink.get(2));
        System.out.println(apply((Object o) -> String.valueOf(o), 42));
        B<String> b = null;
        A<List<String>> a = b;
        System.out.println(a == null);
        Node<Integer> n = new Node<>(1);
        System.out.println(n.compareTo(new Node<>(2)));
    }
}
