//! Opt-in gson real-project compilation check.
//!
//! Enabled only when GSON_SRC (the gson/src/main/java directory) and
//! GSON_GENERATED (the generated GsonBuildConfig.java) are set; gson sources
//! are never copied into this repository.

use std::path::{Path, PathBuf};

use javac_core::main::mini_compiler::MiniCompiler;

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
fn gson_sources_compile_without_errors() {
    let Some(src) = std::env::var_os("GSON_SRC") else {
        return;
    };
    let Some(generated) = std::env::var_os("GSON_GENERATED") else {
        return;
    };
    let mut sources = Vec::new();
    collect_java(&PathBuf::from(src), &mut sources);
    sources.push(PathBuf::from(generated));
    sources.sort();

    let out = std::env::temp_dir().join(format!("rust-javac-gson-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("gson output directory");

    let mut compiler = MiniCompiler::new(&out);
    let compiled = compiler.compile_files(&sources).expect("gson compilation");
    println!(
        "gson files={} errors={} classes={}",
        sources.len(),
        compiler.error_count(),
        compiled.len()
    );
    assert_eq!(compiler.error_count(), 0, "gson must compile without errors");
}
