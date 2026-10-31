import java.util.function.Supplier;

public class LambdaCapture {
    private final String field = "f";

    Supplier<String> capture() {
        int local = 3;
        String text = "t";
        return () -> text + local + field + this.field;
    }

    public static void main(String[] args) {
        System.out.println(new LambdaCapture().capture().get());
    }
}
