//! Adversarial cases: out-of-range positions, CRLF, BOM, and odd declarations.

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
        .join("extreme-spec")
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

#[test]
fn position_past_the_end_of_the_line_does_not_panic() {
    let root = fixture("line-end");
    let path = root.join("A.java");
    std::fs::write(&path, "class A {}\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let _ = workspace.hover(&path, Position::new(0, 40));
    let _ = workspace.definition(&path, Position::new(0, 40));
    let _ = workspace.completion(&path, Position::new(0, 40));
}

#[test]
fn empty_file_does_not_panic() {
    let root = fixture("empty");
    let path = root.join("A.java");
    std::fs::write(&path, "").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let _ = workspace.hover(&path, Position::new(0, 0));
    let _ = workspace.definition(&path, Position::new(0, 0));
    let _ = workspace.diagnostics(&path, None);
}

#[test]
fn empty_line_hover_does_not_panic() {
    let root = fixture("empty-line");
    let path = root.join("A.java");
    std::fs::write(&path, "class A {\n\n    int value;\n}\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let _ = workspace.hover(&path, Position::new(1, 0));
    let _ = workspace.hover(&path, Position::new(1, 40));
}

#[test]
fn position_beyond_the_last_line_does_not_panic() {
    let root = fixture("beyond");
    let path = root.join("A.java");
    std::fs::write(&path, "class A { int value; }\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let _ = workspace.hover(&path, Position::new(99, 3));
    let _ = workspace.definition(&path, Position::new(99, 3));
    let _ = workspace.references(&path, Position::new(99, 3));
}

#[test]
fn crlf_chinese_diagnostics_use_utf16_columns() {
    let root = fixture("crlf");
    let path = root.join("Zhong.java");
    let source = "class Zhong {\r\n    // 中文注释，\r\n    void f() { missing++; }\r\n}\r\n";
    std::fs::write(&path, source).unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let diagnostics = workspace.diagnostics(&path, None);
    let error = diagnostics
        .iter()
        .find(|diagnostic| matches!(diagnostic.severity, Severity::Error))
        .expect("error");
    eprintln!("crlf range: {:?}", error.range);
    assert_eq!(error.range.start.line, 2, "range: {:?}", error.range);
    assert_eq!(error.range.start.column, 15, "range: {:?}", error.range);
}

#[test]
fn bom_file_definition_lands_on_the_declared_name() {
    let root = fixture("bom");
    let path = root.join("A.java");
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(b"class A {\n    B b;\n}\nclass B {}\n");
    std::fs::write(&path, bytes).unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let definition = workspace.definition(&path, Position::new(1, 4));
    let definition = definition.expect("definition of B");
    eprintln!("bom definition: {definition:?}");
    assert_eq!(definition.range.start.line, 3);
    assert_eq!(definition.range.start.column, 6, "{definition:?}");
}

#[test]
fn annotation_with_the_same_name_does_not_steal_the_declaration_position() {
    let root = fixture("annotation");
    let path = root.join("A.java");
    std::fs::write(&path, "@B\nclass B {}\nclass A { B b; }\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let definition = workspace.definition(&path, Position::new(2, 10));
    let definition = definition.expect("definition of B");
    eprintln!("annotation definition: {definition:?}");
    assert_eq!(definition.range.start.line, 1, "{definition:?}");
    assert_eq!(definition.range.start.column, 6, "{definition:?}");
}

#[test]
fn enum_constant_definition_lands_on_the_constant() {
    let root = fixture("enum");
    let path = root.join("A.java");
    std::fs::write(&path, "enum Color { RED, GREEN }\nclass A { Color c = Color.RED; }\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let definition = workspace.definition(&path, Position::new(1, 27));
    let definition = definition.expect("definition of RED");
    eprintln!("enum definition: {definition:?}");
    assert_eq!(definition.range.start.line, 0, "{definition:?}");
    assert_eq!(definition.range.start.column, 13, "{definition:?}");
}

#[test]
fn nested_jdk_class_definition_uses_the_outer_source_file() {
    let root = fixture("nested");
    let path = root.join("A.java");
    std::fs::write(
        &path,
        "import java.util.Map;\nclass A { Map.Entry<String, String> e; }\n",
    )
    .unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let definition = workspace.definition(&path, Position::new(1, 16));
    let definition = definition.expect("definition of Entry");
    eprintln!("nested definition: {definition:?}");
    assert!(definition.file.ends_with("Map.java"), "{definition:?}");
}

#[test]
fn definition_after_an_overlay_edit_matches_the_overlay_text() {
    let root = fixture("overlay");
    let path = root.join("A.java");
    let original = "class A {\n    int value;\n    void f() { value = 1; }\n}\n";
    std::fs::write(&path, original).unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    // Insert a header comment without waiting for the incremental index update.
    let edited = format!("// header comment\n{original}");
    workspace.set_overlay(&path, &edited);
    let definition = workspace.definition(&path, Position::new(3, 17));
    let definition = definition.expect("definition of value");
    eprintln!("overlay definition: {definition:?}");
    assert_eq!(definition.range.start.line, 2, "{definition:?}");
}
