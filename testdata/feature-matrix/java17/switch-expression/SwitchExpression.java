public class SwitchExpression {
    enum Kind {
        A, B, C
    }

    static int classify(int value) {
        return switch (value) {
            case 0 -> 0;
            case 1, 2 -> 10;
            default -> {
                int rest = value % 2;
                yield rest == 0 ? 100 : -1;
            }
        };
    }

    static String name(Kind kind) {
        return switch (kind) {
            case A -> "a";
            case B -> "b";
            case C -> "c";
        };
    }

    static String text(String value) {
        return switch (value) {
            case "x" -> "ex";
            default -> "other";
        };
    }

    public static void main(String[] args) {
        System.out.println(classify(0) + classify(2) + classify(4) + classify(5) + name(Kind.B) + text("x"));
    }
}
