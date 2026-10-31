public class RecordsPatterns {
    record Box<T>(T value) {
    }

    static String describe(Object candidate) {
        if (candidate instanceof Box<?> box && box.value() instanceof String text) {
            return text;
        }
        return "other";
    }

    public static void main(String[] args) {
        System.out.println(describe(new Box<>("hi")) + describe(new Box<>(1)));
    }
}
