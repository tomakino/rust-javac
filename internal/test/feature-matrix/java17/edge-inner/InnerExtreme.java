public class InnerExtreme {
    private int value = 1;
    private static int staticValue = 2;

    class Member { int get() { return value + staticValue; } }

    static class Nested { int get() { return staticValue; } }

    interface Iface {
        default String d() { return "d"; }
        static String s() { return "s"; }
        private String p() { return "p"; }
        default String dp() { return p() + d(); }
    }

    static class Impl implements Iface { }

    void local() {
        class Local { int get() { return value; } }
        Runnable r = new Runnable() {
            @Override public void run() { value++; }
        };
        Object o = new Object() {
            @Override public String toString() { return "anon" + value; }
        };
        r.run();
        System.out.println(new Local().get() + "" + o + new Member().get());
    }

    public static void main(String[] args) {
        InnerExtreme outer = new InnerExtreme();
        outer.local();
        System.out.println(new Nested().get() + new Impl().dp() + Iface.s());
        System.out.println(outer.new Member().get());
    }
}
