use javac_core::main::mini_compiler::MiniCompiler;

/// Regression: an inner class reads an outer field inside an array dimension.
#[test]
fn inner_class_outer_field_in_array_dimension() {
    let dir = std::env::temp_dir().join(format!("rust-javac-innerarr-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("T.java");
    std::fs::write(
        &p,
        "class T { int size = 1; class Inner { int f() { return size; } int[] g() { return new int[size]; } } }",
    )
    .unwrap();
    let mut c = MiniCompiler::new(&dir);
    let compiled = c.compile_files(std::slice::from_ref(&p)).expect("compile");
    assert_eq!(c.error_count(), 0);
    assert!(!compiled.is_empty());
}
