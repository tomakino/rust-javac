public class InitExtreme {
    static final int CONST = 1 + 2 * 3;
    static final String SCONST = "a" + "b" + 3;
    static final int SHIFTED = 1 << 4;
    static int staticField = compute();
    static { staticField += 10; }

    final int a;
    final int b;
    { a = 1; }

    InitExtreme() { this(2); }
    InitExtreme(int b) { this.b = b; }

    static int compute() { return 42; }

    static class Sub extends InitExtreme {
        Sub() { super(7); }
    }

    public static void main(String[] args) {
        InitExtreme e = new InitExtreme();
        Sub s = new Sub();
        System.out.println(CONST + SCONST + SHIFTED + staticField + e.a + e.b + s.a + s.b);
    }
}
