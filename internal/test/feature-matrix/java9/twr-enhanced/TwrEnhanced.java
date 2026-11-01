public class TwrEnhanced {
    static class Resource implements AutoCloseable {
        final String name;

        Resource(String name) {
            this.name = name;
        }

        @Override
        public void close() {
        }
    }

    public static void main(String[] args) {
        Resource resource = new Resource("r");
        try (resource) {
            System.out.println(resource.name);
        }
    }
}
