public class InstanceofPattern {
    static int length(Object value) {
        if (value instanceof String text && !text.isEmpty()) {
            return text.length();
        }
        return 0;
    }

    public static void main(String[] args) {
        System.out.println(length("abc") + length("") + length(42));
    }
}
