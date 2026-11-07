//! Field/parameter same-name resolution (Writer.java `lock` case).

use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::Workspace;

fn platform_dir() {
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
}

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("writer-conflict")
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

const SOURCE: &str = "class W {\n  protected Object lock;\n  W(String lock) {\n    this.lock = lock;\n  }\n}\n";

fn parameter_position() -> Position {
    let line = SOURCE.lines().nth(2).unwrap();
    Position::new(2, line.find("lock").unwrap() as u32)
}

#[test]
fn hover_on_parameter_shows_the_parameter() {
    let root = fixture("hover");
    let path = root.join("W.java");
    std::fs::write(&path, SOURCE).unwrap();
    let mut workspace = Workspace::new(config(&root));
    let hover = workspace.hover(&path, parameter_position()).expect("hover");
    // The field is `protected Object lock`; the parameter is plain `Object
    // lock`. A field-first match would leak the `protected` modifier.
    assert!(
        !hover.contains("protected"),
        "hover must describe the parameter, got {hover:?}"
    );
    assert!(hover.contains("String lock"), "got {hover:?}");
}

#[test]
fn definition_on_parameter_does_not_jump_to_the_field() {
    let root = fixture("definition");
    let path = root.join("W.java");
    std::fs::write(&path, SOURCE).unwrap();
    let mut workspace = Workspace::new(config(&root));
    let location = workspace
        .definition(&path, parameter_position())
        .expect("definition");
    assert_eq!(
        location.range.start.line, 2,
        "parameter definition must stay on the parameter declaration: {location:?}"
    );
}

const METHOD_SOURCE: &str = "class W {\n  String lock;\n  void f(String lock) {\n    String x = lock;\n  }\n}\n";

fn method_parameter_position() -> Position {
    let line = METHOD_SOURCE.lines().nth(2).unwrap();
    Position::new(2, line.find("lock").unwrap() as u32)
}

#[test]
fn definition_on_method_parameter_stays_on_the_parameter() {
    let root = fixture("method-definition");
    let path = root.join("W.java");
    std::fs::write(&path, METHOD_SOURCE).unwrap();
    let mut workspace = Workspace::new(config(&root));
    let location = workspace
        .definition(&path, method_parameter_position())
        .expect("definition of the parameter");
    assert_eq!(
        location.range.start.line, 2,
        "parameter definition must stay on the parameter declaration: {location:?}"
    );
}