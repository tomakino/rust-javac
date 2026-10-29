use javac_core::main::mini_compiler::MiniCompiler;
#[test]
fn enum_declaration() {
    let dir = std::env::temp_dir().join(format!("rust-javac-enumdecl-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("T.java");
    std::fs::write(&p, "enum MyEnum { A, B, C } class T {}").unwrap();
    let mut c = MiniCompiler::new(&dir);
    let _ = c.compile_files(std::slice::from_ref(&p));
}
