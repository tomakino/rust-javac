import java.util.ArrayList;
import java.util.List;

public class Generics {
    static <T extends Comparable<T>> T max(List<T> values) {
        T best = values.get(0);
        for (T value : values) {
            if (value.compareTo(best) > 0) {
                best = value;
            }
        }
        return best;
    }

    static void addAll(List<? super Integer> target, List<Integer> source) {
        target.addAll(source);
    }

    public static void main(String[] args) {
        List<Integer> values = new ArrayList<>();
        values.add(3);
        values.add(1);
        List<Number> target = new ArrayList<>();
        addAll(target, values);
        System.out.println(max(values) + " " + target.size());
    }
}
