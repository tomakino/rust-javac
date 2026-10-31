public class TwrExtreme {
    static class Res implements AutoCloseable {
        final String name;
        Res(String name) { this.name = name; }
        @Override public void close() { System.out.print("c" + name); }
    }

    static void multi() {
        try (Res a = new Res("a"); Res b = new Res("b"); var c = new Res("c")) {
            System.out.print("body");
        }
    }

    static void effect(Res r) {
        try (r) { System.out.print("e"); }
    }

    static void nested() {
        try (Res a = new Res("x")) {
            try (Res b = new Res("y")) { System.out.print("n"); }
        }
    }

    static int finallyOrder() {
        int r = 0;
        try {
            try { r = 1; return r; } finally { r += 10; }
        } finally { r += 100; }
    }

    static void suppression() {
        try (Res a = new Res("s")) {
            throw new IllegalStateException("x");
        } catch (IllegalStateException e) {
            System.out.print("catch");
        }
    }

    public static void main(String[] args) {
        multi();
        effect(new Res("r"));
        nested();
        System.out.print(finallyOrder());
        suppression();
    }
}
