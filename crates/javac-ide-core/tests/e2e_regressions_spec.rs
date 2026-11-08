//! Regressions found by the Gson end-to-end audit.

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
    platform_dir();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("e2e-regressions")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn config(root: &Path) -> AnalysisConfig {
    AnalysisConfig {
        class_path: Vec::new(),
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: vec![root.to_path_buf()],
    }
}

fn write(root: &Path, relative: &str, text: &str) -> PathBuf {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn import_definition_works_from_any_column() {
    let root = fixture("import-definition");
    let helper = write(
        &root,
        "p/Helper.java",
        "package p;\n\npublic class Helper {\n}\n",
    );
    let app = write(
        &root,
        "q/App.java",
        "package q;\n\nimport p.Helper;\n\nclass App {\n    Helper helper;\n}\n",
    );
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let import_line = 2;
    // `import p.Helper;`: `p` starts at column 7, `Helper` at 9.
    for column in [7, 9] {
        let location = workspace
            .definition(&app, Position::new(import_line, column))
            .unwrap_or_else(|| panic!("no definition from import column {column}"));
        assert_eq!(
            std::fs::canonicalize(&location.file).unwrap(),
            std::fs::canonicalize(&helper).unwrap(),
            "column {column}"
        );
    }
}

#[test]
fn index_stats_report_files_and_symbols() {
    let root = fixture("index-stats");
    write(&root, "A.java", "class A {\n    void f() {\n    }\n}\n");
    write(&root, "B.java", "class B {\n    A a;\n}\n");
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let stats = workspace.index_stats();
    assert!(stats.files >= 2, "{stats:?}");
    assert!(stats.symbols >= 2, "{stats:?}");
    assert_eq!(workspace.index_files(), stats.files);
    assert_eq!(workspace.index_symbols(), stats.symbols);
}

#[test]
fn module_info_exports_do_not_produce_false_errors() {
    let root = fixture("module-info");
    let module_info = write(
        &root,
        "module-info.java",
        "module m {\n    exports p;\n}\n",
    );
    write(&root, "p/A.java", "package p;\n\npublic class A {\n}\n");
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let diagnostics = workspace.diagnostics(&module_info, None);
    assert!(
        diagnostics.is_empty(),
        "clean module-info reported errors: {diagnostics:?}"
    );
}

#[test]
fn broken_module_info_still_reports_syntax_errors() {
    let root = fixture("module-info-broken");
    let module_info = write(&root, "module-info.java", "module m {\n    exports p\n}\n");
    write(&root, "p/A.java", "package p;\n\npublic class A {\n}\n");
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let diagnostics = workspace.diagnostics(&module_info, None);
    assert!(
        !diagnostics.is_empty(),
        "syntax errors in module-info were suppressed"
    );
}
