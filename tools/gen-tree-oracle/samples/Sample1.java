package samples;

import java.util.*;
import java.util.function.*;

public class Sample1 {
    public sealed interface Shape permits Circle, Square {}

    public record Circle(double radius) implements Shape {}

    public record Square(double side) implements Shape {}

    @SafeVarargs
    public static <T extends Comparable<T>> T max(T... values) {
        T best = values[0];
        for (T value : values) {
            if (value.compareTo(best) > 0) {
                best = value;
            }
        }
        return best;
    }

    public static double area(Shape shape) {
        if (shape instanceof Circle circle) {
            return Math.PI * circle.radius() * circle.radius();
        } else if (shape instanceof Square square) {
            return square.side() * square.side();
        }
        return 0.0;
    }

    public static String kind(int code) {
        return switch (code) {
            case 0 -> "zero";
            case 1, 2 -> "small";
            default -> "large";
        };
    }

    public static String describe(Object value) {
        if (value instanceof String text && !text.isBlank()) {
            return "string:" + text;
        }
        return String.valueOf(value);
    }

    public static void main(String[] args) {
        Function<Integer, Integer> twice = x -> x * 2;
        List<Integer> numbers = List.of(1, 2, 3);
        int total = numbers.stream().map(twice).reduce(0, Integer::sum);
        String block = """
                text block with " quotes and 'single' quotes
                and a trailing line
                """;
        var label = describe(total);
        System.out.println(label + block + kind(total) + area(new Circle(1.0)));
    }
}
