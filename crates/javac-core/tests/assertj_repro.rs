use javac_core::main::mini_compiler::MiniCompiler;
use javac_core::main::option::Option;

/// AssertJ uses assertThat(Iterable<? extends E>) and several bounded
/// assertThat(T) overloads. A diamond LinkedHashSet argument must select the
/// Iterable overload; the bounded identity candidates must be inapplicable.
#[test]
fn assertj_assertthat_diamond() {
    let dir = std::env::temp_dir().join(format!("rust-javac-assertj-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("T.java");
    std::fs::write(
        &path,
        r#"import static org.assertj.core.api.Assertions.assertThat;
import java.util.*;
class T { void f() { assertThat(new LinkedHashSet<>(Collections.singletonList("x"))).isEqualTo(new LinkedHashSet<>(Collections.singletonList("x"))); } }"#,
    )
    .unwrap();
    let cp = "C:\\Users\\Lin\\RustroverProjects\\javac-testdata\\okhttp-deps\\assertj-core-3.11.0.jar";
    let mut compiler = MiniCompiler::new_with_options(&dir, |options| {
        options.put_option(Option::CLASS_PATH, cp);
    });
    let _ = compiler.compile_files(std::slice::from_ref(&path));
    assert_eq!(compiler.error_count(), 0, "assertThat diamond must compile");
}
