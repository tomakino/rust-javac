public class Fixtures {
    public static final int CONST = 42;
    public static final String NAME = "inner";
    public static final long LONG_CONST = 1234567890123L;
    public static final boolean BOOL_CONST = true;

    class Member {
        int x;
    }

    static class Nested {
        int y;
    }

    void methodWithLocal() {
        class Local {
            int z;
        }
        Runnable r = new Runnable() {
            public void run() {}
        };
    }
}
