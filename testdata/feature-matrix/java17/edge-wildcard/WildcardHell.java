import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.List;

public class WildcardHell {
    static <T> void sink(List<? super T> s, T t) { s.add(t); }

    static <T extends Comparable<? super T>> T max(Collection<? extends T> c) {
        T best = null;
        for (T t : c) {
            if (best == null || t.compareTo(best) > 0) best = t;
        }
        return best;
    }

    static List<? extends Number> nums() { return List.of(1, 2.5, 3L); }

    static List<? super Integer> supers() { return new ArrayList<Number>(); }

    static class Builder<T> {
        final List<T> xs = new ArrayList<>();

        <U> Builder<U> map(java.util.function.Function<? super T, ? extends U> f) {
            Builder<U> b = new Builder<>();
            for (T t : xs) b.xs.add(f.apply(t));
            return b;
        }
    }

    static <T> Builder<T> of(T... xs) {
        Builder<T> b = new Builder<>();
        b.xs.addAll(Arrays.asList(xs));
        return b;
    }

    interface Sink<A, B> { B apply(A a); }

    static <A, B> B run(Sink<A, B> f, A a) { return f.apply(a); }

    public static void main(String[] args) {
        sink(supers(), 1);
        System.out.println(max(List.of(3, 1, 2)));
        Builder<String> b = WildcardHell.<String>of("a", "b")
            .map(String::toUpperCase)
            .map(s -> s + "!");
        System.out.println("" + b.xs + nums() + supers());
        System.out.println(run((Integer n) -> n + 1, 41));
    }
}
