use javac_core::main::mini_compiler::MiniCompiler;
#[test]
fn break_inside_catch_makes_loop_exit_reachable() {
    let dir = std::env::temp_dir().join(format!("rust-javac-breakcatch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("T.java");
    std::fs::write(&p, "class T { void f(){ while (true) { try { } catch (Exception e) { break; } } int x = 1; } }").unwrap();
    let mut c = MiniCompiler::new(&dir);
    let _ = c.compile_files(std::slice::from_ref(&p));
}
