//! UTF-16 unit consistency for completion prefixes, symbol ranges, and index
//! occurrence ranges on non-ASCII identifiers.

use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::index::{self, IndexLimits};
use javac_ide_core::workspace::model::WorkspaceModel;
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
        .join("utf16-units-spec")
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
fn completion_prefix_after_a_non_ascii_declaration() {
    let root = fixture("completion");
    let source = "class A {\n    void f() {\n        int 数值 = 1;\n        数|\n    }\n}\n";
    let cursor = source.find('|').expect("cursor marker");
    let text = source.replacen('|', "", 1);
    let before = &text[..cursor];
    let line = before.matches('\n').count() as u32;
    let column = before
        .rsplit('\n')
        .next()
        .unwrap()
        .chars()
        .map(|ch| ch.len_utf16() as u32)
        .sum();
    let path = root.join("A.java");
    std::fs::write(&path, &text).unwrap();

    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let labels: Vec<String> = workspace
        .completion(&path, Position::new(line, column))
        .into_iter()
        .map(|item| item.label)
        .collect();
    assert!(
        labels.iter().any(|label| label == "数值"),
        "local variable missing from completion: {labels:?}"
    );
}

#[test]
fn index_occurrences_store_utf16_ranges() {
    let root = fixture("index");
    let source = "class A { int 数值; int read() { return 数值; } }\n";
    let path = root.join("A.java");
    std::fs::write(&path, source).unwrap();
    let config = config(&root);
    let model = WorkspaceModel::from_config(&config);
    let built = index::build::build(
        &model,
        &config,
        &std::collections::HashMap::new(),
        8,
        &IndexLimits::defaults(),
    )
    .index;

    let field = built
        .symbols_named("数值")
        .into_iter()
        .next()
        .expect("field 数值");
    assert_eq!(
        field.range.end - field.range.start,
        2,
        "occurrence width must be UTF-16 code units: {:?}",
        field.range
    );

    let file = built.file_id_for_path(&path).expect("indexed file");
    let declaration_column = "class A { int ".encode_utf16().count() as u32;
    assert_eq!(declaration_column, 14);
    assert!(
        index::query::symbol_at(&built, file, source, Position::new(0, declaration_column))
            .is_some(),
        "declaration lookup must use UTF-16 offsets"
    );

    let reference_prefix = "class A { int 数值; int read() { return ";
    let reference_column = reference_prefix.encode_utf16().count() as u32;
    let text_of = |_file: index::FileId| Some(source.to_string());
    let references = index::query::references(
        &built,
        file,
        source,
        Position::new(0, reference_column),
        &text_of,
    );
    assert_eq!(references.len(), 2, "references: {references:?}");
    assert!(
        references
            .iter()
            .any(|location| location.range.start.column == declaration_column),
        "declaration column wrong: {references:?}"
    );
    assert!(
        references
            .iter()
            .any(|location| location.range.start.column == reference_column),
        "reference column wrong: {references:?}"
    );
}

#[test]
fn navigation_handles_non_ascii_identifiers() {
    let root = fixture("navigation");
    let source = "class A { int 数值; int read() { return 数值; } }";
    let path = root.join("A.java");
    std::fs::write(&path, source).unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();

    let declaration_column = "class A { int ".encode_utf16().count() as u32;
    let reference_prefix = "class A { int 数值; int read() { return ";
    let reference_column = reference_prefix.encode_utf16().count() as u32;

    let references = workspace.references(&path, Position::new(0, declaration_column));
    assert_eq!(references.len(), 2, "references: {references:?}");
    assert!(
        references
            .iter()
            .any(|location| location.range.start.column == declaration_column)
    );
    assert!(
        references
            .iter()
            .any(|location| location.range.start.column == reference_column)
    );

    let definition = workspace
        .definition(&path, Position::new(0, reference_column))
        .expect("definition");
    assert_eq!(definition.range.start.column, declaration_column);
}
#[test]
fn call_hierarchy_handles_non_ascii_names() {
    let root = fixture("hierarchy");
    let path = root.join("Calls.java");
    std::fs::write(
        &path,
        "class Calls {\n    void 调用者() { 被调用(); }\n    void 被调用() { }\n}\n",
    )
    .unwrap();
    let mut workspace = Workspace::new(config(&root));
    let item = workspace
        .prepare_call_hierarchy(&path, Position::new(2, 10))
        .expect("call hierarchy item");
    let calls = workspace.incoming_calls(&item);
    assert_eq!(calls.len(), 1, "incoming calls: {calls:?}");
    assert_eq!(calls[0].item.name, "调用者");
}
