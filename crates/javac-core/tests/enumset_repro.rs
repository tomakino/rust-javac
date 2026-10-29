use javac_core::main::mini_compiler::MiniCompiler;
#[test]
fn enumset_f_bounded_varargs() {
    let dir = std::env::temp_dir().join(format!("rust-javac-enumset-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("T.java");
    std::fs::write(&p, "import java.time.*; import java.util.*; class T { Set<DayOfWeek> s = EnumSet.of(DayOfWeek.MONDAY, DayOfWeek.TUESDAY, DayOfWeek.WEDNESDAY); }").unwrap();
    let mut c = MiniCompiler::new(&dir);
    let _ = c.compile_files(std::slice::from_ref(&p));
}
