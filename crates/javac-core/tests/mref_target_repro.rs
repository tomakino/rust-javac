use javac_core::main::mini_compiler::MiniCompiler;
#[test]
fn bound_method_reference_argument_has_target() {
    let dir = std::env::temp_dir().join(format!("rust-javac-mreftarget-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("T.java");
    std::fs::write(&p, "import java.util.concurrent.*;
class Call { void execute() {} }
class T { void f(Call call, ExecutorService executor) { Future<?> result = executor.submit(call::execute); } }").unwrap();
    let mut c = MiniCompiler::new(&dir);
    let _ = c.compile_files(std::slice::from_ref(&p));
    assert_eq!(c.error_count(), 0);
}
