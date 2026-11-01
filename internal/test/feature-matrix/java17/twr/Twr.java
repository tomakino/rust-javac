public class Twr {
    static class Resource implements AutoCloseable {
        private final String name;

        Resource(String name) {
            this.name = name;
        }

        @Override
        public void close() {
            System.out.println("close " + name);
        }
    }

    public static void main(String[] args) {
        try (Resource first = new Resource("a"); Resource second = new Resource("b")) {
            System.out.println(first.name + second.name);
        }
    }
}
