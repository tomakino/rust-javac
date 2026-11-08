//! Code lens outline rules: one group per symbol range, no constructors,
//! imports or parameters, overloads stay distinct.

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
        .join("code-lens-outline")
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

fn workspace_with(name: &str, text: &str) -> (Workspace, PathBuf) {
    let root = fixture(name);
    let path = root.join("A.java");
    std::fs::write(&path, text).unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    (workspace, path)
}

fn ranges(lenses: &[javac_analysis_api::CodeLens]) -> Vec<(String, u32, u32)> {
    let mut values: Vec<(String, u32, u32)> = lenses
        .iter()
        .map(|lens| (lens.title.clone(), lens.range.start.line, lens.range.start.column))
        .collect();
    values.sort();
    values
}

#[test]
fn outline_symbols_get_one_lens_each() {
    let (mut workspace, path) = workspace_with(
        "outline",
        "class A {\n    int field;\n\n    void f() {\n    }\n}\n",
    );
    let lenses = workspace.code_lenses(&path);
    let values = ranges(&lenses);
    let starts: Vec<(u32, u32)> = values.iter().map(|(_, line, column)| (*line, *column)).collect();
    assert!(starts.contains(&(0, 6)), "class lens: {values:?}");
    assert!(starts.contains(&(1, 8)), "field lens: {values:?}");
    assert!(starts.contains(&(3, 9)), "method lens: {values:?}");
    assert_eq!(lenses.len(), 3, "{values:?}");
}

#[test]
fn explicit_constructor_gets_no_lens() {
    let (mut workspace, path) = workspace_with(
        "explicit-ctor",
        "class A {\n    A() {\n    }\n\n    void f() {\n    }\n}\n",
    );
    let lenses = workspace.code_lenses(&path);
    let values = ranges(&lenses);
    assert!(
        values.iter().all(|(_, line, _)| *line == 0 || *line == 4),
        "constructor produced a lens: {values:?}"
    );
    assert_eq!(lenses.len(), 2, "{values:?}");
}

#[test]
fn synthetic_constructor_gets_no_lens() {
    let (mut workspace, path) = workspace_with("synthetic-ctor", "class A {\n    void f() {\n    }\n}\n");
    let lenses = workspace.code_lenses(&path);
    let values = ranges(&lenses);
    assert_eq!(lenses.len(), 2, "{values:?}");
    assert!(values.iter().all(|(_, line, _)| *line <= 1), "{values:?}");
}

#[test]
fn nested_classes_and_overloads_keep_distinct_lenses() {
    let (mut workspace, path) = workspace_with(
        "nested",
        "class Outer {\n    class Inner {\n    }\n\n    void f(int a) {\n    }\n\n    void f(String a) {\n    }\n}\n",
    );
    let lenses = workspace.code_lenses(&path);
    let values = ranges(&lenses);
    let starts: Vec<(u32, u32)> = values.iter().map(|(_, line, column)| (*line, *column)).collect();
    assert!(starts.contains(&(0, 6)), "Outer: {values:?}");
    assert!(starts.contains(&(1, 10)), "Inner: {values:?}");
    assert!(starts.contains(&(4, 9)), "overload 1: {values:?}");
    assert!(starts.contains(&(7, 9)), "overload 2: {values:?}");
}

#[test]
fn imports_and_parameters_have_no_lenses() {
    let (mut workspace, path) = workspace_with(
        "imports",
        "import java.util.List;\n\nclass A {\n    void f(List<String> items, int count) {\n    }\n}\n",
    );
    let lenses = workspace.code_lenses(&path);
    let values = ranges(&lenses);
    assert_eq!(lenses.len(), 2, "{values:?}");
    assert!(
        values.iter().all(|(_, line, _)| *line != 0),
        "import produced a lens: {values:?}"
    );
}

#[test]
fn incremental_record_duplicates_do_not_add_lenses() {
    let (mut workspace, path) = workspace_with(
        "incremental",
        "class A {\n    void f() {\n    }\n}\n",
    );
    // An edit forces the incremental path, which can leave an extra id at the
    // same range; the lens list must stay one group per range.
    let edited = "class A {\n    void f() {\n        int x = 1;\n    }\n}\n";
    workspace.set_overlay(&path, edited);
    workspace.wait_for_index();
    let lenses = workspace.code_lenses(&path);
    let mut seen = std::collections::HashSet::new();
    for lens in &lenses {
        assert!(
            seen.insert((lens.range.start.line, lens.range.start.column)),
            "duplicate lens range: {:?}",
            ranges(&lenses)
        );
    }
    let _ = Position::new(0, 0);
}
