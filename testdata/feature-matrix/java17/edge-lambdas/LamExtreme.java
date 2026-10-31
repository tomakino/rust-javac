import java.util.function.BiFunction;
import java.util.function.Function;
import java.util.function.Supplier;

public class LamExtreme {
    interface Factory<T> { T create(); }
    interface Join { String apply(String... parts); }
    interface Tri<A, B, C, R> { R apply(A a, B b, C c); }

    static class Holder<T> {
        final T value;
        Holder(T value) { this.value = value; }
        Holder() { this(null); }
    }

    static String str() { return "s"; }
    static String str2(String a, String b) { return a + b; }
    static <T> T id(T t) { return t; }

    public static void main(String[] args) {
        Factory<String> f1 = () -> "x";
        Supplier<Holder<String>> f2 = () -> new Holder<>("h");
        BiFunction<String, String, String> b1 = LamExtreme::str2;
        BiFunction<String, String, String> b2 = (a, b) -> a + b;
        Join join = (String... parts) -> String.join(",", parts);
        Function<String, String> idFn = LamExtreme::id;
        Tri<Integer, Integer, Integer, Integer> tri = (a, b, c) -> a + b + c;
        int[] captured = {0};
        Runnable inc = () -> captured[0]++;
        inc.run();
        Supplier<Holder<String>> ctor = Holder::new;
        System.out.println(f1.create() + f2.get().value + b1.apply("a", "b") + b2.apply("c", "d"));
        System.out.println(join.apply("1", "2", "3") + idFn.apply("i") + tri.apply(1, 2, 3));
        System.out.println(captured[0] + ctor.get().value + str());
    }
}
