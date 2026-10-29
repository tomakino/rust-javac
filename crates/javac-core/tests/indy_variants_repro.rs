use javac_core::main::mini_compiler::MiniCompiler;

/// Regression: indy variants (interface, ctor, unbound, generic SAM,
/// serializable/intersection altMetafactory).
#[test]
fn indy_variants_compile() {
    let dir = std::env::temp_dir().join(format!("rust-javac-indyvariants-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("T.java");
    std::fs::write(
        &p,
        "import java.util.function.*;
interface I { void m(); }
interface G<T> { T apply(T t); }
interface A<T> { T get(); }
interface B extends A<String> { String get(); }
class C { C() {} void inst() {} static void stat() {} static String make() { return \"\"; } }
class T {
  static I boundInterface(I i) { return i::m; }
  static Supplier<C> ctorRef() { return C::new; }
  static Function<String,Integer> unbound() { return String::length; }
  static G<String> generic() { return String::trim; }
  static B bridged() { return C::make; }
  static Runnable serializable() { return (Runnable & java.io.Serializable) C::stat; }
}",
    )
    .unwrap();
    let mut c = MiniCompiler::new(&dir);
    let compiled = c.compile_files(std::slice::from_ref(&p)).expect("compile");
    assert_eq!(c.error_count(), 0);
    assert!(!compiled.is_empty());
}
