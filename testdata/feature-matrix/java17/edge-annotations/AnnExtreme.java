import java.lang.annotation.ElementType;
import java.lang.annotation.Repeatable;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;
import java.util.ArrayList;
import java.util.List;

@Retention(RetentionPolicy.RUNTIME)
@Target({ElementType.TYPE, ElementType.FIELD, ElementType.METHOD, ElementType.PARAMETER,
        ElementType.TYPE_USE, ElementType.CONSTRUCTOR})
@Repeatable(Annos.class)
@interface Anno {
    String value() default "d";
    int num() default 1;
    Class<?> type() default Object.class;
    ElementType e() default ElementType.TYPE;
    int[] arr() default {1, 2};
}

@Retention(RetentionPolicy.RUNTIME)
@Target({ElementType.TYPE, ElementType.FIELD, ElementType.METHOD, ElementType.PARAMETER, ElementType.TYPE_USE})
@interface Annos { Anno[] value(); }

@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.TYPE_USE)
@interface NonNull {}

@Anno
@Anno(value = "t", num = 2)
public class AnnExtreme {
    @Anno @Anno(value = "f", num = 3) private String field = "v";
    @NonNull private List<@NonNull String> list = new ArrayList<>();
    private @NonNull String[] array = new String[0];
    private @NonNull int[][] matrix = new int[0][0];
    private List<@NonNull ? extends @NonNull Number> numbers = new ArrayList<>();

    @Anno public AnnExtreme(@NonNull String p) { this.field = p; }

    @Anno public @NonNull String method(@NonNull @Anno String a) { return a; }

    @Anno(value = "m", type = String.class, e = ElementType.FIELD, arr = {3, 4})
    public void values() { }

    public static void main(String[] args) {
        AnnExtreme a = new AnnExtreme("x");
        System.out.println(a.field + a.list + a.array.length + a.matrix.length + a.numbers);
        System.out.println(a.method("m") + a.getClass().getAnnotations().length);
    }
}
