import java.util.function.IntSupplier;
import java.util.function.Supplier;

public class DesugarHell {
    static int deepFinally(int x) {
        try {
            try {
                try {
                    try {
                        try {
                            if (x == 0) return 1;
                            if (x < 0) throw new IllegalStateException("x");
                        } finally {
                            x += 1;
                        }
                    } finally {
                        x += 2;
                    }
                } finally {
                    x += 3;
                }
            } finally {
                x += 4;
            }
        } finally {
            x += 5;
        }
        return x;
    }

    interface Fn { int apply(int x); }

    static class Outer {
        int v = 1;

        Fn make() {
            return new Fn() {
                public int apply(int x) {
                    Runnable r = () -> { v += x; };
                    r.run();
                    Object anon = new Object() {
                        @Override public String toString() { return "a" + v; }
                    };
                    return anon.hashCode() + v;
                }
            };
        }
    }

    static int nestedLambda() {
        Supplier<IntSupplier> s = () -> {
            int a = 1;
            return () -> a + 2;
        };
        return s.get().getAsInt();
    }

    static int loopResolved() {
        int acc = 0;
        for (int i = 0; i < 3; i++) {
            try {
                if (i == 1) continue;
                acc += i;
            } finally {
                acc += 10;
            }
        }
        return acc;
    }

    public static void main(String[] args) {
        System.out.println(deepFinally(1) + deepFinally(0) + new Outer().make().apply(2)
            + nestedLambda() + loopResolved());
    }
}
