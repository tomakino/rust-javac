//! Diagnostics for one file must observe edits to the other files it depends on.

use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, Severity, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
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
        .join("cross-file-diagnostics-spec")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
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

fn has_add4_error(diagnostics: &[javac_analysis_api::Diagnostic]) -> bool {
    diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("add4")
            && matches!(diagnostic.severity, Severity::Error)
    })
}

#[test]
fn diagnostics_follow_a_saved_change_in_another_file() {
    let root = fixture("saved");
    let app = root.join("App.java");
    let service = root.join("Service.java");
    std::fs::write(&app, "class App { void f(Service s) { s.add4(1, 2); } }\n").unwrap();
    std::fs::write(&service, "class Service { }\n").unwrap();

    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let before = workspace.diagnostics(&app, None);
    assert!(has_add4_error(&before), "before: {before:?}");

    // Save the new method; App.java does not change.
    std::fs::write(
        &service,
        "class Service { int add4(int a, int b) { return a + b; } }\n",
    )
    .unwrap();
    let after = workspace.diagnostics(&app, None);
    assert!(!has_add4_error(&after), "after: {after:?}");
}

#[test]
fn diagnostics_see_an_unsaved_edit_in_another_file() {
    let root = fixture("overlay");
    let app = root.join("App.java");
    let service = root.join("Service.java");
    std::fs::write(&app, "class App { void f(Service s) { s.add4(1, 2); } }\n").unwrap();
    std::fs::write(&service, "class Service { }\n").unwrap();

    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    assert!(has_add4_error(&workspace.diagnostics(&app, None)));

    // The user opens Service.java and adds the method without saving.
    workspace.set_overlay(
        &service,
        "class Service { int add4(int a, int b) { return a + b; } }\n",
    );
    let diagnostics = workspace.diagnostics(&app, None);
    assert!(!has_add4_error(&diagnostics), "diagnostics: {diagnostics:?}");
}
