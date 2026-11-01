import java.util.Arrays;
import java.util.Objects;

public record RecExtreme<T extends Comparable<T>>(T value, int... counts) implements Comparable<RecExtreme<T>> {
    public RecExtreme {
        Objects.requireNonNull(value, "value");
        if (counts == null) counts = new int[0];
    }

    public static <T extends Comparable<T>> RecExtreme<T> of(T value, int... counts) {
        return new RecExtreme<>(value, counts);
    }

    public int compareTo(RecExtreme<T> other) { return value.compareTo(other.value); }

    public int total() {
        int sum = 0;
        for (int count : counts) sum += count;
        return sum;
    }

    public static record Inner(int x, String y) {
        public Inner { if (x < 0) throw new IllegalArgumentException(); }
    }

    public static record Empty() { }

    public static void main(String[] args) {
        RecExtreme<String> r = RecExtreme.of("a", 1, 2, 3);
        System.out.println(r.value() + r.total() + Arrays.toString(r.counts()));
        System.out.println(r.compareTo(new RecExtreme<>("b")));
        System.out.println("" + new Inner(1, "i") + new Empty() + r.equals(r.hashCode() == 0));
    }
}
