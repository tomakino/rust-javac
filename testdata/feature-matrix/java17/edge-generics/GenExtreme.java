import java.util.ArrayList;
import java.util.List;
import java.util.Map;

public class GenExtreme {
    static <T extends Comparable<? super T>> T max(List<? extends T> list) {
        T best = list.get(0);
        for (T t : list) { if (t.compareTo(best) > 0) best = t; }
        return best;
    }

    static <T extends Number & Comparable<T>> T bounded(T a, T b) {
        return a.compareTo(b) <= 0 ? a : b;
    }

    static class Box<T> {
        final T value;
        Box(T value) { this.value = value; }
        <R> R map(java.util.function.Function<? super T, ? extends R> f) { return f.apply(value); }
    }

    @SafeVarargs
    static <T> List<T> listOf(T... items) { return new ArrayList<>(java.util.Arrays.asList(items)); }

    static Map<String, List<Map<Integer, ? extends Number>>> nested() { return Map.of(); }

    static List<? super Integer> superList() { return new ArrayList<Number>(); }

    static <T> T pick(T a, T b) { return a; }

    interface Visitor<R> { <T> R visit(Box<T> box, T value); }

    static void wild(List<?> list) { for (Object o : list) { System.out.print(o); } }

    public static void main(String[] args) {
        Box<String> box = new Box<>("x");
        String mapped = box.map(String::toUpperCase);
        Visitor<Integer> v = new Visitor<Integer>() {
            public <T> Integer visit(Box<T> b, T value) { return value.hashCode(); }
        };
        System.out.println(max(List.of(3, 1, 2)));
        System.out.println(bounded(5, 7));
        System.out.println(mapped + listOf(1, 2, 3) + nested() + superList() + pick("a", "b"));
        System.out.println(v.visit(box, "z"));
        wild(List.of(1, 2));
    }
}
