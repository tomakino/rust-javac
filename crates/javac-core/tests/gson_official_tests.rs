//! Opt-in check: compile gson main + official test sources with javac-core.
//!
//! Requires GSON_SRC, GSON_GENERATED, GSON_TEST_SRC and JUNIT_JAR. Test sources
//! are never copied into this repository.

use std::path::{Path, PathBuf};

use javac_core::main::mini_compiler::MiniCompiler;
use javac_core::main::option::Option;

fn collect_java(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_java(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "java")
            && path.file_name().is_some_and(|name| name != "module-info.java")
        {
            out.push(path);
        }
    }
}

#[test]
fn gson_main_and_tests_compile() {
    let Some(src) = std::env::var_os("GSON_SRC") else {
        return;
    };
    let Some(generated) = std::env::var_os("GSON_GENERATED") else {
        return;
    };
    let Some(test_src) = std::env::var_os("GSON_TEST_SRC") else {
        return;
    };
    let Some(junit) = std::env::var_os("JUNIT_JAR") else {
        return;
    };
    let mut sources = Vec::new();
    collect_java(&PathBuf::from(src), &mut sources);
    collect_java(&PathBuf::from(test_src), &mut sources);
    sources.push(PathBuf::from(generated));
    sources.sort();

    let out = std::env::temp_dir().join(format!("rust-javac-gson-tests-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("gson test output directory");

    let junit_path = PathBuf::from(junit);
    let mut compiler = MiniCompiler::new_with_options(&out, |options| {
        options.put_option(Option::CLASS_PATH, junit_path.to_string_lossy());
    });
    let compiled = compiler.compile_files(&sources).expect("gson main+test compilation");
    println!(
        "gson main+test files={} errors={} classes={}",
        sources.len(),
        compiler.error_count(),
        compiled.len()
    );
    assert_eq!(
        compiler.error_count(),
        0,
        "gson main and official tests must compile without errors"
    );
}
