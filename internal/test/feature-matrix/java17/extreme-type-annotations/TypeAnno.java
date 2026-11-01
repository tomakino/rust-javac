import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;
import java.util.List;

@Retention(RetentionPolicy.RUNTIME)
@Target({ElementType.TYPE_USE, ElementType.TYPE_PARAMETER})
@interface Mark {}

public class TypeAnno {
    static class Box<@Mark T> {
        @Mark T value;
        @Mark Box(@Mark T value) { this.value = value; }
        @Mark T get() { return value; }
    }

    @Mark
    static class Outer<@Mark U extends @Mark Comparable<? super U>> {
        @Mark List<@Mark U> items;
        @Mark Outer(@Mark List<@Mark U> items) { this.items = items; }
    }

    static int count(@Mark List<@Mark ? extends @Mark Number> values) {
        int n = 0;
        for (@Mark Number value : values) {
            if (value != null) n++;
        }
        return n;
    }

    static <@Mark T extends @Mark Number> @Mark T choose(@Mark T a, @Mark T b) {
        return a.intValue() >= b.intValue() ? a : b;
    }

    static class Holder<@Mark A> {
        @Mark A first;
        @Mark A second;
        Holder(@Mark A first, @Mark A second) { this.first = first; this.second = second; }
        @Mark A pick() { return first != null ? first : second; }
    }

    public static void main(String[] args) {
        Box<String> box = new Box<>("x");
        Outer<Integer> outer = new Outer<>(List.of(1, 2, 3));
        System.out.println(box.get() + outer.items.size());
        System.out.println(count(List.of(1, 2)) + choose(1, 2));
        Holder<String> holder = new Holder<>("a", "b");
        System.out.println(holder.pick());
    }
}
