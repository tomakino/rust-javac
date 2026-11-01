public class InterfacePrivate {
    interface Accumulator {
        private int twice(int x) {
            return x * 2;
        }

        default int apply(int x) {
            return twice(x);
        }

        static int unit() {
            return 1;
        }
    }

    static class Impl implements Accumulator {
    }

    public static void main(String[] args) {
        System.out.println(new Impl().apply(21) + Accumulator.unit());
    }
}
