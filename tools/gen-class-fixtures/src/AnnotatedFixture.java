import java.lang.annotation.*;

@Retention(RetentionPolicy.RUNTIME)
@Target({ElementType.TYPE, ElementType.METHOD, ElementType.PARAMETER, ElementType.FIELD})
@interface MyAnno {
    int value();
    String name() default "x";
    Class<?> type() default Object.class;
    ElementType kind() default ElementType.TYPE;
}

@MyAnno(value = 7, name = "n")
public class AnnotatedFixture {
    @MyAnno(1)
    int field;

    @MyAnno(2)
    void method(@MyAnno(3) int param) {}
}

@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.TYPE_USE)
@interface MyTypeAnno {
}

class TypeAnnotated {
    @MyTypeAnno String field;
    java.util.List<@MyTypeAnno String> list;

    @MyTypeAnno String method(@MyTypeAnno String parameter) {
        return parameter;
    }
}
