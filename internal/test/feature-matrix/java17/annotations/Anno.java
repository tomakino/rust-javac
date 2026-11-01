import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

@Retention(RetentionPolicy.RUNTIME)
@Target({ElementType.TYPE, ElementType.METHOD, ElementType.FIELD, ElementType.PARAMETER})
@interface Mark {
    String value() default "default";

    int number() default 1;
}

@Mark("type")
public class Anno {
    @Mark(value = "field", number = 2)
    private String field = "x";

    @Mark("method")
    public String method(@Mark("param") String input) {
        return input + field;
    }

    public static void main(String[] args) {
        Mark mark = Anno.class.getAnnotation(Mark.class);
        System.out.println(mark.value() + mark.number() + new Anno().method("m"));
    }
}
