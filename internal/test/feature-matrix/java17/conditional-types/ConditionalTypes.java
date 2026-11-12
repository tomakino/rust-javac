import java.util.function.Supplier;

/**
 * Conditional expression typing (JLS 15.25): binary numeric promotion, unboxing,
 * the null-and-reference cases, the null-and-primitive boxing case, reference joins,
 * constant folding, nesting, and poly expressions with a target type.
 */
public class ConditionalTypes {
    static int byteShort(byte b, short s) {
        return true ? b : s;
    }

    static int charInt(char c, int i) {
        return false ? c : i;
    }

    static long intLong(int i, long l) {
        return i > 0 ? i : l;
    }

    static double floatDouble(float f, double d) {
        return f > 0 ? f : d;
    }

    static int unboxing(Integer boxed, int value) {
        return boxed != null ? boxed : value;
    }

    static Integer nullInt(boolean flag) {
        return flag ? null : 42;
    }

    static String nullRef(boolean flag) {
        return flag ? null : "x";
    }

    static Number numericJoin(Integer i, Double d, boolean flag) {
        return flag ? i : d;
    }

    static int constantFold(boolean flag) {
        int a = flag ? 1 : 2;
        int b = true ? 3 : 4;
        return a + b;
    }

    static int nested(boolean a, boolean b) {
        return a ? (b ? 1 : 2) : 3;
    }

    static Supplier<String> lambda(boolean flag) {
        return flag ? () -> "yes" : () -> "no";
    }
}
