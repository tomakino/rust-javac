public class Nestmates {
    private int secret = 42;

    class Inner {
        int peek() {
            return secret;
        }
    }

    static class Nested {
        int peek(Nestmates outer) {
            return outer.secret;
        }
    }

    public static void main(String[] args) {
        Nestmates outer = new Nestmates();
        System.out.println(outer.new Inner().peek() + new Nested().peek(outer));
    }
}
