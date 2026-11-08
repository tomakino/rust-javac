//! Cross-file rename must cover every reference, including non-ASCII names
//! and partially indexed projects.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, TextDocumentEdit, WorkspaceService};

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
        .join("rename-spec")
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

fn files(edits: &[TextDocumentEdit]) -> BTreeSet<PathBuf> {
    edits.iter().map(|edit| edit.file.clone()).collect()
}

#[test]
fn rename_cross_file_covers_both_files() {
    let root = fixture("basic");
    let a = root.join("A.java");
    let b = root.join("B.java");
    std::fs::write(&a, "class A { B b; void f() { b.value = 1; } }\n").unwrap();
    std::fs::write(&b, "class B { int value; }\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let edits = workspace.rename_workspace(&b, Position::new(0, 15), "renamed");
    let files = files(&edits);
    assert_eq!(files.len(), 2, "edits: {edits:?}");
    assert!(
        edits.iter().all(|edit| edit.edit.new_text == "renamed"),
        "edits: {edits:?}"
    );
}

#[test]
fn rename_cross_file_works_on_a_partial_index() {
    let root = fixture("partial");
    let a = root.join("A.java");
    let b = root.join("B.java");
    std::fs::write(&a, "class A { B b; void f() { b.value = 1; } }\n").unwrap();
    std::fs::write(&b, "import ja;\nclass B { int value; }\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let edits = workspace.rename_workspace(&b, Position::new(1, 15), "renamed");
    let files = files(&edits);
    assert_eq!(files.len(), 2, "edits: {edits:?}");
}

#[test]
fn rename_cross_file_non_ascii_positions() {
    let root = fixture("non-ascii");
    let a = root.join("A.java");
    let b = root.join("B.java");
    let a_text = "class A { B b; void f() { b.数值 = 1; } }\n";
    let b_text = "class B { int 数值; }\n";
    std::fs::write(&a, a_text).unwrap();
    std::fs::write(&b, b_text).unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let b_column = b_text.lines().next().unwrap().find("数值").unwrap() as u32;
    let edits = workspace.rename_workspace(&b, Position::new(0, b_column + 1), "值");
    let files = files(&edits);
    assert_eq!(files.len(), 2, "edits: {edits:?}");
    // The reference in A.java must start at the UTF-16 column of 数值.
    let a_column = a_text.lines().next().unwrap().find("数值").unwrap() as u32;
    let a_edit = edits
        .iter()
        .find(|edit| edit.file.ends_with("A.java"))
        .expect("A.java edit");
    assert_eq!(a_edit.edit.range.start.line, 0, "{:?}", edits);
    assert_eq!(a_edit.edit.range.start.column, a_column, "edits: {edits:?}");
    assert_eq!(a_edit.edit.range.end.column, a_column + 2, "edits: {edits:?}");
}

#[test]
fn rename_with_an_unsaved_edit_does_not_use_stale_ranges() {
    let root = fixture("unsaved");
    let a = root.join("A.java");
    let b = root.join("B.java");
    std::fs::write(&a, "class A { B b; void f() { b.value = 1; } }\n").unwrap();
    std::fs::write(&b, "class B { int value; }\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    // Insert a line so the indexed ranges no longer match the buffer.
    let edited = format!("// header\nclass A {{ B b; void f() {{ b.value = 1; }} }}\n");
    workspace.set_overlay(&a, &edited);
    let edits = workspace.rename_workspace(&b, Position::new(0, 15), "renamed");
    // Either refused (empty) or the ranges must match the edited buffer,
    // never the stale index.
    for edit in &edits {
        if edit.file.ends_with("A.java") {
            assert_eq!(
                edit.edit.range.start.line, 1,
                "stale range used for an edited buffer: {edits:?}"
            );
        }
    }
}

#[test]
fn rename_method_across_files() {
    let root = fixture("method");
    let a = root.join("A.java");
    let b = root.join("B.java");
    std::fs::write(&a, "class A { void f() { B b = new B(); b.compute(); } }\n").unwrap();
    std::fs::write(&b, "class B { int compute() { return 1; } }\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let edits = workspace.rename_workspace(&b, Position::new(0, 16), "calc");
    let files = files(&edits);
    assert_eq!(files.len(), 2, "edits: {edits:?}");
    assert!(
        edits.iter().all(|edit| edit.edit.new_text == "calc"),
        "edits: {edits:?}"
    );
}

#[test]
fn rename_type_includes_constructor_calls() {
    let root = fixture("type");
    let a = root.join("A.java");
    let b = root.join("B.java");
    std::fs::write(&a, "class A { B b = new B(); }\n").unwrap();
    std::fs::write(&b, "class B { }\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let edits = workspace.rename_workspace(&b, Position::new(0, 6), "C");
    let files = files(&edits);
    assert_eq!(files.len(), 2, "edits: {edits:?}");
    let a_edits: Vec<_> = edits
        .iter()
        .filter(|edit| edit.file.ends_with("A.java"))
        .collect();
    assert!(
        a_edits.len() >= 2,
        "type renaming must also update constructor calls: {edits:?}"
    );
}

#[test]
fn rename_does_not_touch_same_named_members_of_other_classes() {
    let root = fixture("same-name");
    let a = root.join("A.java");
    let b = root.join("B.java");
    std::fs::write(&a, "class A { int value; void f() { value = 2; } }\n").unwrap();
    std::fs::write(&b, "class B { int value; void g() { value = 1; } }\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let edits = workspace.rename_workspace(&b, Position::new(0, 12), "renamed");
    assert!(
        edits.iter().all(|edit| !edit.file.ends_with("A.java")),
        "rename leaked into A.java: {edits:?}"
    );
}

#[test]
fn rename_overload_only_touches_the_selected_signature() {
    let root = fixture("overload");
    let a = root.join("A.java");
    let b = root.join("B.java");
    std::fs::write(
        &a,
        "class A { void f(B b) { b.get(); b.get(1); } }\n",
    )
    .unwrap();
    std::fs::write(
        &b,
        "class B { int get() { return 1; } String get(int x) { return \"\"; } }\n",
    )
    .unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let edits = workspace.rename_workspace(&b, Position::new(0, 16), "obtain");
    let a_edits: Vec<_> = edits
        .iter()
        .filter(|edit| edit.file.ends_with("A.java"))
        .collect();
    assert_eq!(a_edits.len(), 1, "overload rename: {edits:?}");
    assert_eq!(a_edits[0].edit.range.start.column, 26, "{edits:?}");
}

#[test]
fn rename_method_includes_override_declaration() {
    let root = fixture("override");
    let model = root.join("Model.java");
    let child = root.join("Child.java");
    let app = root.join("App.java");
    std::fs::write(&model, "class Model { void compute() { } }\n").unwrap();
    std::fs::write(
        &child,
        "class Child extends Model { @Override void compute() { } }\n",
    )
    .unwrap();
    std::fs::write(
        &app,
        "class App { void f(Child child) { child.compute(); Model model = child; model.compute(); } }\n",
    )
    .unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let edits = workspace.rename_workspace(&model, Position::new(0, 19), "run");
    assert!(
        edits
            .iter()
            .any(|edit| edit.file.ends_with("Child.java") && edit.edit.new_text == "run"),
        "override declaration must be renamed: {edits:?}"
    );
    assert!(
        edits
            .iter()
            .any(|edit| edit.file.ends_with("App.java") && edit.edit.new_text == "run"),
        "call sites must be renamed: {edits:?}"
    );
}