//! Opt-in real-project compatibility test.
use std::path::{Path, PathBuf};
use javac_tools::tools::standard_java_file_manager::StandardJavaFileManager;
fn collect_java_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else { return; };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() { collect_java_files(&path, out); }
        else if path.extension().and_then(|e| e.to_str()) == Some("java")
            && path.file_name().and_then(|n| n.to_str()) != Some("module-info.java") { out.push(path); }
    }
}
#[test]
fn gson_real_project_compiles() {
    let Some(src) = std::env::var_os("GSON_SRC") else { return; };
    let src = PathBuf::from(src);
    let mut files = Vec::new();
    collect_java_files(&src, &mut files);
    let generated = PathBuf::from("C:/Users/Lin/RustroverProjects/javac-testdata/gson-crosscheck/generated");
    if generated.is_dir() { collect_java_files(&generated, &mut files); }
    let out = std::env::temp_dir().join(format!("rust-javac-gson-out-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("clock").as_nanos()));
    std::fs::create_dir_all(&out).expect("out dir");
    let mut compiler = javac_core::main::mini_compiler::MiniCompiler::new_with_options(&out, |_o| {});
    compiler.log.borrow_mut().install_diagnostic_handler(javac_core::util::log::DiagnosticHandler::Deferred { prev: None, deferred: std::collections::VecDeque::new(), filter: None });
    let result = compiler.compile_files(&files);
    let diagnostics = compiler.log.borrow_mut().take_deferred_diagnostics();
    compiler.log.borrow_mut().pop_diagnostic_handler();
    let codes: Vec<String> = diagnostics.iter().map(|d| d.get_code_string().to_string()).collect();
    eprintln!("gson files={} result_ok={} diagnostics={} classes={}", files.len(), result.is_ok(), diagnostics.len(), compiler.last_compiled_classes.len());
    eprintln!("diagnostics: {codes:?}");
    let details: Vec<String> = diagnostics
        .iter()
        .map(|d| format!("{}: {}", d.get_code_string(), d.get_message(None).to_string_lossy()))
        .collect();
    eprintln!("diagnostic messages: {details:?}");
    assert!(result.is_ok(), "gson compile failed: {codes:?}");
}
