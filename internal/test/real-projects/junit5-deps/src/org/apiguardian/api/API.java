package org.apiguardian.api;

import java.lang.annotation.Documented;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

@Documented
@Retention(RetentionPolicy.CLASS)
@Target({ElementType.PACKAGE, ElementType.TYPE, ElementType.METHOD,
         ElementType.CONSTRUCTOR, ElementType.FIELD})
public @interface API {
    Status status() default Status.STABLE;
    String since() default "";
    String[] consumers() default {};

    enum Status {
        INTERNAL, DEPRECATED, EXPERIMENTAL, MAINTAINED, STABLE
    }
}
