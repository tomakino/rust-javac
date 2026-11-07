//! Outline names must never be empty (VS Code rejects `name must not be falsy`).

use std::path::{Path, PathBuf};

use javac_analysis_api::WorkspaceService;

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
        .join("document-symbol-names")
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

const SOURCE: &str = "class A {\n  void f() {\n    Runnable r = new Runnable() {\n      public void run() {}\n    };\n    class Local { int x; }\n  }\n}\n";

#[test]
fn file_analysis_outline_names_are_not_empty() {
    let root = fixture("file-analysis");
    let path = root.join("A.java");
    std::fs::write(&path, SOURCE).unwrap();
    let mut workspace = Workspace::new(config(&root));
    let symbols = workspace.symbols(&path);
    for symbol in &symbols {
        assert!(!symbol.name.is_empty(), "empty name: {symbol:?}");
    }
}

#[test]
fn index_outline_names_are_not_empty() {
    let root = fixture("index");
    let path = root.join("A.java");
    std::fs::write(&path, SOURCE).unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let symbols = workspace.document_symbols(&path);
    for symbol in &symbols {
        assert!(!symbol.name.is_empty(), "empty name: {symbol:?}");
    }
}
