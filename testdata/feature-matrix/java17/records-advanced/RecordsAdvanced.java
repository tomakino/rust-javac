import java.util.List;

public class RecordsAdvanced {
    interface Named {
        String name();
    }

    record Pair<A, B>(A first, B second) implements Named {
        @Override
        public String name() {
            return "pair";
        }
    }

    record Varargs(int... values) {
        int total() {
            int sum = 0;
            for (int value : values) {
                sum += value;
            }
            return sum;
        }
    }

    record Empty() {
    }

    static <A, B> Pair<A, B> of(A first, B second) {
        return new Pair<>(first, second);
    }

    public static void main(String[] args) {
        Pair<String, Integer> pair = of("a", 1);
        System.out.println(pair.name() + pair.first() + pair.second()
                + new Varargs(1, 2, 3).total() + new Empty().toString()
                + List.of(pair).size());
    }
}
