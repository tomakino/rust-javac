use javac_core::main::mini_compiler::MiniCompiler;
#[test]
fn inner_new_boxes_user_args() {
    let dir = std::env::temp_dir().join(format!("rust-javac-innernew-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("T.java");
    std::fs::write(&p, "class T { class Inner { Inner() {} } void f() { new Inner(); } }").unwrap();
    let mut c = MiniCompiler::new(&dir);
    let _ = c.compile_files(std::slice::from_ref(&p));
    assert_eq!(c.error_count(), 0);
}
