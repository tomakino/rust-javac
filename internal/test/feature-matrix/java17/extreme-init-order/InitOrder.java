public class InitOrder {
    interface Defaults {
        int BASE = InitOrder.bump("interface-field");
        default String label() { return "default"; }
        static String staticLabel() { return "static"; }
    }

    enum Color implements Defaults {
        RED("r") {
            @Override public String label() { return "red:" + key; }
        },
        GREEN("g") {
            @Override public String label() { return "green:" + key; }
        };
        final String key;
        Color(String key) { this.key = key; }
        @Override public String label() { return "color:" + key; }
    }

    static final int A = bump("static-A");
    static final int B;
    static {
        B = bump("static-B");
    }
    final int C = bump("instance-C");
    final int D;
    {
        D = bump("instance-D");
    }

    static int bump(String tag) {
        System.out.println("init " + tag);
        return tag.length();
    }

    InitOrder() {
        System.out.println("ctor C=" + C + " D=" + D);
    }

    static class Base {
        static { System.out.println("base-static"); }
        final int x = bump("base-x");
        Base() { System.out.println("base-ctor x=" + x + " y=" + y()); }
        int y() { return 1; }
    }

    static class Derived extends Base {
        static { System.out.println("derived-static"); }
        final int z = bump("derived-z");
        @Override int y() { return z; }
        Derived() { System.out.println("derived-ctor z=" + z); }
    }

    public static void main(String[] args) {
        System.out.println("main A=" + A + " B=" + B);
        new InitOrder();
        new Derived();
        System.out.println(Color.RED.label() + " " + Color.GREEN.label());
        System.out.println(Defaults.staticLabel() + " " + Color.RED.label());
    }
}
