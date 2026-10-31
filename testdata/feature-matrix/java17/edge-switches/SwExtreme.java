public class SwExtreme {
    enum E { A, B, C }

    static int stringSwitch(String s) {
        switch (s) {
            case "Aa": return 1;
            case "BB": return 2;
            case "": return 3;
            default: return 0;
        }
    }

    static int enumSwitch(E e) {
        return switch (e) {
            case A -> 1;
            case B -> { yield 2; }
            case C -> 3;
        };
    }

    static int nested(int x) {
        return switch (x) {
            case 1 -> switch (x * 2) { case 2 -> 10; default -> 20; };
            case 2, 3 -> 30;
            default -> { int y = x + 1; yield y; }
        };
    }

    static String yieldBlock(int x) {
        return switch (x) {
            case 1, 2 -> "low";
            default -> {
                String s = "high";
                yield s + x;
            }
        };
    }

    static int classic(int x) {
        int r = 0;
        switch (x) {
            case 0: r += 1;
            case 1: r += 2; break;
            default: r = -1;
        }
        return r;
    }

    public static void main(String[] args) {
        System.out.println(stringSwitch("Aa") + stringSwitch("BB") + stringSwitch("") + stringSwitch("z"));
        System.out.println(enumSwitch(E.A) + enumSwitch(E.B) + enumSwitch(E.C));
        System.out.println(nested(1) + nested(2) + nested(5) + yieldBlock(1) + yieldBlock(9) + classic(0));
    }
}
