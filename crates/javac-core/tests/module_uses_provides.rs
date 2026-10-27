//! End-to-end regression test for module directives: a module-info with
//! `uses` and `provides` compiles through the ported phases (zero JVM at test
//! time; the expected result is the javac 17 output for this source set).

use javac_core::main::mini_compiler::MiniCompiler;
use javac_tools::tools::standard_java_file_manager::StandardJavaFileManager;
use javac_tools::tools::standard_location::StandardLocation;
use std::path::{Path, PathBuf};

fn write_source(root: &Path, relative: &str, source: &str) -> PathBuf {
    let path = root.join("src").join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("source directory");
    }
    std::fs::write(&path, source).expect("source file");
    path
}

#[test]
fn module_info_uses_provides_compiles() {
    let root = std::env::temp_dir().join(format!(
        "rust-javac-module-uses-provides-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let out = root.join("out");
    std::fs::create_dir_all(&out).expect("output directory");
    let module_info = write_source(
        &root,
        "module-info.java",
        "module m.test {\n    uses p.Service;\n    provides p.Service with p.ServiceImpl;\n}\n",
    );
    let service = write_source(
        &root,
        "p/Service.java",
        "package p;\n\npublic interface Service {\n    void run();\n}\n",
    );
    let implementation = write_source(
        &root,
        "p/ServiceImpl.java",
        "package p;\n\npublic class ServiceImpl implements Service {\n    public ServiceImpl() {}\n\n    public void run() {}\n}\n",
    );
    let mut compiler = MiniCompiler::new(&out);
    let source_path = StandardLocation::location_for(&javac_strings::JavaString::from("SOURCE_PATH"));
    compiler
        .file_manager
        .set_location(&*source_path, Some(&[root.join("src")]))
        .expect("set source path");
    let compiled = compiler
        .compile_files(&[module_info, service, implementation])
        .expect("module-info uses/provides compilation");
    let names: Vec<&str> = compiled.iter().map(|class| class.name.as_str()).collect();
    assert!(names.contains(&"module-info"), "generated: {names:?}");
    assert!(names.contains(&"p.Service"), "generated: {names:?}");
    assert!(names.contains(&"p.ServiceImpl"), "generated: {names:?}");
    assert!(compiler.error_count() == 0, "diagnostics: {}", compiler.error_count());
    let _ = std::fs::remove_dir_all(&root);
}
