//! Opt-in check: compile OkHttp 3.14.9 main modules + official test sources.
//!
//! Requires OKHTTP_ROOT and OKHTTP_DEPS. Test sources stay outside this repo.

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
fn okhttp_main_and_tests_compile() {
    let Some(root) = std::env::var_os("OKHTTP_ROOT") else {
        return;
    };
    let Some(deps) = std::env::var_os("OKHTTP_DEPS") else {
        return;
    };
    let root = PathBuf::from(root);
    let mut sources = Vec::new();
    for module in [
        "okhttp",
        "okhttp-tests",
        "mockwebserver",
        "okhttp-testing-support",
        "okhttp-tls",
        "okhttp-urlconnection",
        "okhttp-logging-interceptor",
        "okhttp-dnsoverhttps",
        "okhttp-sse",
    ] {
        collect_java(&root.join(module).join("src/main/java"), &mut sources);
    }
    collect_java(&root.join("okhttp-tests/src/test/java"), &mut sources);
    collect_java(&root.join("okhttp-generated"), &mut sources);
    if let Ok(include) = std::env::var("OKHTTP_INCLUDE") {
        let needles: Vec<String> = include
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| s.replace('\\', "/"))
            .collect();
        sources.retain(|path| {
            let p = path.to_string_lossy().replace('\\', "/");
            needles.iter().any(|needle| p.contains(needle))
        });
    }
    sources.sort();

    let out = std::env::temp_dir().join(format!("rust-javac-okhttp-tests-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("okhttp test output directory");

    let mut jars = Vec::new();
    if let Ok(entries) = std::fs::read_dir(PathBuf::from(&deps)) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "jar") {
                jars.push(path.to_string_lossy().to_string());
            }
        }
    }
    jars.sort();
    // Accept either a directory of dependency jars or an exploded class
    // directory (the file manager's CLASS_PATH jar support is still partial).
    let deps_path = PathBuf::from(deps);
    let classpath = if jars.is_empty() {
        deps_path.to_string_lossy().to_string()
    } else {
        jars.join(";")
    };

    let mut compiler = MiniCompiler::new_with_options(&out, |options| {
        options.put_option(Option::CLASS_PATH, classpath);
    });
    let compiled = compiler
        .compile_files(&sources)
        .expect("okhttp main+test compilation");
    println!(
        "okhttp main+test files={} errors={} classes={}",
        sources.len(),
        compiler.error_count(),
        compiled.len()
    );
    assert_eq!(
        compiler.error_count(),
        0,
        "okhttp main and official tests must compile without errors"
    );
}
