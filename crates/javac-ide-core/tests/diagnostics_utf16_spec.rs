//! Diagnostic conversion must use UTF-16 (Java char) offsets, never byte
//! offsets, on text that contains multi-byte characters.

use std::path::{Path, PathBuf};

use javac_analysis_api::Severity;

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::analysis::FileAnalysis;

fn platform_dir() -> PathBuf {
    if std::env::var_os("RAVAC_PLATFORM_DIR").is_none() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("debug");
        unsafe {
            std::env::set_var("RAVAC_PLATFORM_DIR", &root);
        }
    }
    PathBuf::from(std::env::var_os("RAVAC_PLATFORM_DIR").expect("platform dir"))
}

fn session(name: &str, source: &str) -> (FileAnalysis, PathBuf) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("diagnostics-utf16-spec")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("Zhong.java");
    std::fs::write(&path, source).unwrap();
    let analysis = FileAnalysis::new(AnalysisConfig {
        class_path: Vec::new(),
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: Vec::new(),
    });
    (analysis, path)
}

#[test]
fn error_after_chinese_and_emoji_uses_utf16_columns() {
    platform_dir();
    let source = "class Zhong {\n    // 中文注释，包含中文逗号，\n    String s = \"中文，😀\";\n    void f() { missing++; }\n}\n";
    let (mut analysis, path) = session("chinese", source);
    let diagnostics = analysis.diagnostics(&path, None);
    let error = diagnostics
        .iter()
        .find(|diagnostic| matches!(diagnostic.severity, Severity::Error))
        .expect("compiler error");
    eprintln!(
        "range={:?} end={:?} message={}",
        error.range.start, error.range.end, error.message
    );
    assert_eq!(error.range.start.line, 3, "range: {:?}", error.range);
    // 4 spaces + "void f() { " is 15 UTF-16 columns.
    assert_eq!(error.range.start.column, 15, "range: {:?}", error.range);
}

#[test]
fn error_inside_a_line_after_emoji_uses_utf16_columns() {
    platform_dir();
    let source = "class Zhong {\n    void f() { String s = \"😀\"; missing++; }\n}\n";
    let (mut analysis, path) = session("emoji", source);
    let diagnostics = analysis.diagnostics(&path, None);
    let error = diagnostics
        .iter()
        .find(|diagnostic| matches!(diagnostic.severity, Severity::Error))
        .expect("compiler error");
    eprintln!(
        "range={:?} end={:?} message={}",
        error.range.start, error.range.end, error.message
    );
    // 4 spaces + "void f() { String s = \"😀\"; " counts UTF-16 units.
    let line = "    void f() { String s = \"😀\"; missing++; }";
    let prefix_utf16: u32 = line
        .split("missing")
        .next()
        .unwrap()
        .chars()
        .map(|ch| ch.len_utf16() as u32)
        .sum();
    assert_eq!(error.range.start.line, 1, "range: {:?}", error.range);
    assert_eq!(error.range.start.column, prefix_utf16, "range: {:?}", error.range);
}
