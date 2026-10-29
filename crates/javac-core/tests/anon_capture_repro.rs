use javac_core::main::mini_compiler::MiniCompiler;

/// Regression: an anonymous class capturing a local must store the captured
/// variable in a val$x field and read it from the anonymous method.
#[test]
fn anonymous_class_captures_local() {
    let dir = std::env::temp_dir().join(format!("rust-javac-anon-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("T.java");
    std::fs::write(
        &p,
        "class T { interface Base { void evaluate(); } \
         static Base make(String x) { \
           return new Base() { public void evaluate() { System.out.println(x); } }; \
         } }",
    )
    .unwrap();
    let mut c = MiniCompiler::new(&dir);
    let compiled = c.compile_files(std::slice::from_ref(&p)).expect("compile");
    assert_eq!(c.error_count(), 0);
    assert!(!compiled.is_empty());
}

/// Regression: an anonymous class inside a lambda capturing the lambda
/// parameter. WebSocketWriterTest's TestRule is exactly this shape.
#[test]
fn anonymous_class_captures_lambda_parameter() {
    let dir = std::env::temp_dir().join(format!("rust-javac-anon2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("T.java");
    std::fs::write(
        &p,
        "class T { interface Base { void evaluate(); } interface F { Base make(Base base); } \
         static F f = base -> new Base() { public void evaluate() { base.evaluate(); } }; }",
    )
    .unwrap();
    let mut c = MiniCompiler::new(&dir);
    let compiled = c.compile_files(std::slice::from_ref(&p)).expect("compile");
    assert_eq!(c.error_count(), 0);
    assert!(!compiled.is_empty());
}
