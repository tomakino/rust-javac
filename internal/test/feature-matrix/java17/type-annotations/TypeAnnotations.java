import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

public class TypeAnnotations {
    @Target(ElementType.TYPE_USE)
    @Retention(RetentionPolicy.RUNTIME)
    @interface NonNull {
    }

    @NonNull
    String field = "x";

    static @NonNull String pick(@NonNull String first, String second) {
        return first.isEmpty() ? second : first;
    }

    public static void main(String[] args) {
        System.out.println(pick("a", "b") + new TypeAnnotations().field);
    }
}
