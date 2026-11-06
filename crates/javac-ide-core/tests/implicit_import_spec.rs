//! Regression: implicit source-path compilation plus an external import must
//! not panic in `ImportsPhase::do_import`.

use std::path::{Path, PathBuf};

use javac_analysis_api::Severity;
use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::analysis::FileAnalysis;
use javac_ide_core::workspace::Workspace;

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

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("implicit-import-spec")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("com/example")).unwrap();
    root
}

fn config(root: &Path) -> AnalysisConfig {
    platform_dir();
    AnalysisConfig {
        class_path: Vec::new(),
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: vec![root.to_path_buf()],
    }
}

fn write_app(root: &Path, imports: &str) -> PathBuf {
    let app = root.join("com/example/App.java");
    std::fs::write(
        &app,
        format!(
            "package com.example;\n{imports}\npublic class App {{\n    private final Service service = new Service();\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("com/example/Service.java"),
        "package com.example;\npublic class Service {}\n",
    )
    .unwrap();
    app
}

#[test]
fn implicit_compile_with_jdk_import_does_not_panic() {
    let root = fixture("jdk");
    let app = write_app(&root, "import java.util.List;");
    let mut analysis = FileAnalysis::new(config(&root));
    let diagnostics = analysis.diagnostics(&app, None);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("ClassCastException")),
        "diagnostics: {diagnostics:?}"
    );
}

#[test]
fn implicit_compile_without_external_import_does_not_panic() {
    let root = fixture("no-import");
    let app = write_app(&root, "");
    let mut analysis = FileAnalysis::new(config(&root));
    let diagnostics = analysis.diagnostics(&app, None);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("ClassCastException")),
        "diagnostics: {diagnostics:?}"
    );
}

#[test]
fn single_identifier_import_reports_an_error() {
    let root = fixture("single-ident");
    let app = write_app(&root, "import ja;");
    let mut analysis = FileAnalysis::new(config(&root));
    let diagnostics = analysis.diagnostics(&app, None);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| matches!(diagnostic.severity, Severity::Error)),
        "expected an unresolved-import error: {diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("ClassCastException")),
        "diagnostics: {diagnostics:?}"
    );
}
#[test]
fn single_identifier_import_without_implicit_compile_matches() {
    let root = fixture("single-ident-no-implicit");
    let app = root.join("com/example/App.java");
    std::fs::write(&app, "package com.example;\nimport ja;\npublic class App { int value; }\n").unwrap();
    let mut analysis = FileAnalysis::new(config(&root));
    let _ = analysis.diagnostics(&app, None);
}
#[test]
fn background_index_build_survives_a_single_name_import() {
    let root = fixture("index");
    write_app(&root, "import ja;");
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let stats = workspace.index_stats();
    assert!(
        stats.files >= 1,
        "index build did not finish (process may have panicked): {stats:?}"
    );
    assert!(!workspace.is_building(), "{stats:?}");
}