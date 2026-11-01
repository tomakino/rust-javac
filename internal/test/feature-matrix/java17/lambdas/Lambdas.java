import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;
import java.util.function.Supplier;

public class Lambdas {
    public static void main(String[] args) {
        List<String> values = new ArrayList<>(List.of("b", "a"));
        values.sort(String::compareTo);
        Function<Integer, int[]> factory = int[]::new;
        Supplier<List<String>> supplier = ArrayList::new;
        Function<String, Integer> parse = Integer::parseInt;
        System.out.println(values + " " + factory.apply(2).length + " " + supplier.get().size() + " " + parse.apply("7"));
    }
}
