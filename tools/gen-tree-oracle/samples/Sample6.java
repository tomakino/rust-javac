package samples;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

@Retention(RetentionPolicy.RUNTIME)
@Target({ElementType.TYPE, ElementType.METHOD})
public @interface Sample6 {
    String value() default "default";

    int[] numbers() default {1, 2, 3};

    Class<?> type() default Object.class;

    Nested nested() default @Nested;

    @interface Nested {
        String name() default "";
    }
}

@Sample6(value = "x", numbers = {4, 5})
class Sample6User {
    @Sample6
    public void annotated() {
    }
}
