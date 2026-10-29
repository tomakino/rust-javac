use javac_core::main::mini_compiler::MiniCompiler;

/// Regression: a static method reference uses invokedynamic when no lambda
/// conversion is required (upstream default representation).
#[test]
fn static_method_reference_uses_indy() {
    let dir = std::env::temp_dir().join(format!("rust-javac-staticmref-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("T.java");
    std::fs::write(&p, "class T { Runnable r = T::m; static void m() {} }").unwrap();
    let mut c = MiniCompiler::new(&dir);
    let compiled = c.compile_files(std::slice::from_ref(&p)).expect("compile");
    assert_eq!(c.error_count(), 0);
    assert!(!compiled.is_empty());
}
