module feature.matrix {
    exports feature.matrix;
    uses java.lang.Runnable;
    provides java.lang.Runnable with feature.matrix.Hello;
}
