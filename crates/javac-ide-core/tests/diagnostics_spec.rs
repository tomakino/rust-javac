//! Diagnostic range and quick-fix tests through `FileAnalysis`.

use std::path::PathBuf;

use javac_analysis_api::Position;

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::analysis::FileAnalysis;

fn analysis(extra_source: &str) -> (FileAnalysis, PathBuf, String) {
    if std::env::var_os("RAVAC_PLATFORM_DIR").is_none() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("debug");
        unsafe {
            std::env::set_var("RAVAC_PLATFORM_DIR", root);
        }
    }
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("diagnostics-spec");
    let text = format!(
        "package p;\nimport java.util.List;\npublic class D {{\n    void f() {{ {extra_source} }}\n}}\n"
    );
    let path = PathBuf::from("target/tmp/diagnostics-spec/D.java");
    std::fs::create_dir_all("target/tmp/diagnostics-spec").unwrap();
    std::fs::write(&path, &text).unwrap();
    let analysis = FileAnalysis::new(AnalysisConfig {
        class_path: Vec::new(),
        output_dir: output,
        source_level: "17".to_string(),
        source_path: Vec::new(),
    });
    (analysis, path, text)
}

#[test]
fn compiler_diagnostic_has_a_non_empty_range() {
    let (mut analysis, path, text) = analysis(r#"int x = "s";"#);
    let diagnostics = analysis.diagnostics(&path, Some(&text));
    let error = diagnostics
        .iter()
        .find(|diagnostic| matches!(diagnostic.severity, javac_analysis_api::Severity::Error))
        .expect("compiler error");
    assert!(
        (error.range.end.line, error.range.end.column)
            > (error.range.start.line, error.range.start.column),
        "range: {:?}",
        error.range
    );
}

#[test]
fn unused_import_has_a_removal_quick_fix() {
    let (mut analysis, path, text) = analysis("");
    let diagnostics = analysis.diagnostics(&path, Some(&text));
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_deref() == Some("unused.import")),
        "diagnostics: {diagnostics:?}"
    );
    let actions = analysis.code_actions(&path, Position::new(1, 0));
    let remove = actions
        .iter()
        .find(|action| action.title.contains("Remove unused import"))
        .expect("remove action");
    assert_eq!(remove.edits.len(), 1);
    assert_eq!(remove.edits[0].new_text, "");
    assert_eq!(remove.edits[0].range.start, Position::new(1, 0));
    assert_eq!(remove.edits[0].range.end, Position::new(2, 0));
}
