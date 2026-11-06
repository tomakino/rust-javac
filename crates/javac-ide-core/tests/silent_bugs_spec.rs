//! Silent-wrong-result checks: values that are returned without an error but
//! would mislead an editor.

use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, WorkspaceService};

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
        .join("silent-bugs-spec")
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

fn workspace_with(name: &str, source: &str) -> (Workspace, PathBuf) {
    let root = fixture(name);
    let path = root.join("A.java");
    std::fs::write(&path, source).unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    (workspace, path)
}

#[test]
fn document_symbols_nest_members_under_their_class() {
    let source = "class A {\n    int f;\n    void m() {}\n    class B {\n        int g;\n    }\n}\n";
    let (mut workspace, path) = workspace_with("symbols", source);
    let symbols = workspace.document_symbols(&path);
    let a = symbols.iter().find(|symbol| symbol.name == "A").expect("A");
    let a_names: Vec<&str> = a.children.iter().map(|child| child.name.as_str()).collect();
    assert!(
        a_names.contains(&"f") && a_names.contains(&"m") && a_names.contains(&"B"),
        "A children: {a_names:?}"
    );
    let b = a
        .children
        .iter()
        .find(|symbol| symbol.name == "B")
        .expect("B");
    let b_names: Vec<&str> = b.children.iter().map(|child| child.name.as_str()).collect();
    assert_eq!(b_names, vec!["g"], "B children: {b_names:?}");
}

#[test]
fn signature_help_tracks_the_active_parameter() {
    let source = "class A {\n    void f(int a, String b) {}\n    void g() { f(1, \"x\"); }\n}\n";
    let (mut workspace, path) = workspace_with("signature", source);
    let line = 2;
    let first = source.lines().nth(line).unwrap().find('1').unwrap() as u32;
    let help = workspace
        .signature_help(&path, Position::new(line as u32, first + 1))
        .expect("signature help");
    assert_eq!(help.active_parameter, 0, "{help:?}");

    let second = source.lines().nth(line).unwrap().find("\"x\"").unwrap() as u32;
    let help = workspace
        .signature_help(&path, Position::new(line as u32, second + 1))
        .expect("signature help");
    assert_eq!(help.active_parameter, 1, "{help:?}");
}

#[test]
fn inlay_type_hint_follows_the_name() {
    let source = "class A {\n    void f() { int value = 1; }\n}\n";
    let (mut workspace, path) = workspace_with("inlay", source);
    let hints = workspace.inlay_hints(&path);
    let hint = hints
        .iter()
        .find(|hint| hint.label.contains("int"))
        .expect("type hint");
    let line = source.lines().nth(1).unwrap();
    let name_end = line.find("value").unwrap() as u32 + "value".len() as u32;
    assert_eq!(hint.position.line, 1, "{hint:?}");
    assert_eq!(hint.position.column, name_end, "{hint:?}");
}

#[test]
fn references_include_declaration_once_and_are_sorted() {
    let source = "class A {\n    int value;\n    void f() { value = 1; value++; }\n}\n";
    let (mut workspace, path) = workspace_with("references", source);
    let declaration = source.lines().nth(1).unwrap().find("value").unwrap() as u32;
    let references = workspace.references(&path, Position::new(1, declaration + 1));
    assert_eq!(references.len(), 3, "references: {references:?}");
    let mut sorted = references.clone();
    sorted.sort_by_key(|location| (location.range.start.line, location.range.start.column));
    assert_eq!(references, sorted, "references must be sorted");
    let mut unique = references.clone();
    unique.dedup();
    assert_eq!(unique.len(), references.len(), "references must be unique");
}

#[test]
fn prepare_rename_covers_the_name_only() {
    let source = "class A {\n    int value;\n}\n";
    let (mut workspace, path) = workspace_with("rename-range", source);
    let line = source.lines().nth(1).unwrap();
    let start = line.find("value").unwrap() as u32;
    let range = workspace
        .prepare_rename(&path, Position::new(1, start + 1))
        .expect("prepare rename");
    assert_eq!(range.start.line, 1, "{range:?}");
    assert_eq!(range.start.column, start, "{range:?}");
    assert_eq!(range.end.column, start + 5, "{range:?}");
}
