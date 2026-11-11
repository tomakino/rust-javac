//! Index build/query performance and memory bounds.

mod common;
use common::platform_path;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use javac_analysis_api::WorkspaceService;

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::index::{self, FileId, IndexLimits, IndexStatus, ProjectIndex};
use javac_ide_core::workspace::model::WorkspaceModel;
use javac_ide_core::workspace::Workspace;

const FILE_COUNT: usize = 120;

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("index-perf-spec")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn config(source: &Path, output: &Path) -> AnalysisConfig {
    platform_path();
    AnalysisConfig {
        class_path: Vec::new(),
        output_dir: output.to_path_buf(),
        source_level: "17".to_string(),
        source_path: vec![source.to_path_buf()],
    }
}

fn generate_project(root: &Path) {
    for index in 0..FILE_COUNT {
        let mut text = "package perf;\n\n".to_string();
        text.push_str(&format!("public class C{index} {{\n"));
        text.push_str(&format!("    public int value{index};\n"));
        if index > 0 {
            text.push_str(&format!("    public C{} previous;\n", index - 1));
            text.push_str(&format!(
                "    public int read() {{ return previous.value{}; }}\n",
                index - 1
            ));
        }
        text.push_str("}\n");
        std::fs::write(root.join(format!("C{index}.java")), text).unwrap();
    }
}

fn build_index(config: &AnalysisConfig) -> ProjectIndex {
    let model = WorkspaceModel::from_config(config);
    index::build::build(
        &model,
        config,
        &HashMap::new(),
        128,
        &IndexLimits::defaults(),
    )
    .index
}

#[test]
fn index_build_and_query_stay_within_budget() {
    let root = fixture("build");
    generate_project(&root);
    let output = root.join("out");
    let config = config(&root, &output);

    let started = Instant::now();
    let built = build_index(&config);
    let build_time = started.elapsed();
    assert!(
        matches!(built.status, IndexStatus::Ready),
        "status: {:?}",
        built.status
    );
    assert_eq!(built.files.len(), FILE_COUNT);
    assert!(
        build_time < Duration::from_secs(120),
        "build took {build_time:?}"
    );

    let text_of =
        |id: FileId| built.file(id).and_then(|record| std::fs::read_to_string(&record.path).ok());
    let started = Instant::now();
    for _ in 0..50 {
        let symbols = index::query::workspace_symbols(&built, "C1", &text_of);
        assert!(!symbols.is_empty());
    }
    let query_time = started.elapsed();
    assert!(
        query_time < Duration::from_secs(10),
        "50 queries took {query_time:?}"
    );
}

#[test]
fn index_memory_stays_bounded() {
    let root = fixture("memory");
    generate_project(&root);
    let output = root.join("out");
    let config = config(&root, &output);
    let built = build_index(&config);
    let bytes = built.approximate_memory_bytes();
    assert!(
        bytes < 64 * 1024 * 1024,
        "index uses {bytes} bytes for {FILE_COUNT} files"
    );
}

#[test]
fn incremental_body_edit_does_not_rebuild() {
    let root = fixture("incremental");
    std::fs::write(
        root.join("A.java"),
        "class A { int value; int m() { return value; } }",
    )
    .unwrap();
    std::fs::write(
        root.join("B.java"),
        "class B { A a; int read() { return a.value; } }",
    )
    .unwrap();
    let output = root.join("out");
    let mut workspace = Workspace::new(config(&root, &output));
    workspace.wait_for_index();
    assert!(workspace
        .workspace_symbols("A")
        .iter()
        .any(|symbol| symbol.name == "A"));
    let before = workspace.index_stats();
    assert_eq!(before.incremental_updates, 0, "stats: {before:?}");

    // Body edit only: the declaration set is unchanged.
    std::fs::write(
        root.join("A.java"),
        "class A { int value; int m() { return value + 1; } }",
    )
    .unwrap();
    workspace.wait_for_index();
    assert!(workspace
        .workspace_symbols("value")
        .iter()
        .any(|symbol| symbol.name == "value"));
    let after = workspace.index_stats();
    assert_eq!(
        after.incremental_updates, 1,
        "incremental update not used: {after:?}"
    );
    assert_eq!(
        after.full_rebuilds, before.full_rebuilds,
        "full rebuild was triggered: {after:?}"
    );
    assert!(after.generation > before.generation);
}

#[test]
fn declaration_rename_rebuilds_dependents_incrementally() {
    let root = fixture("rename");
    std::fs::write(root.join("A.java"), "class A { int value; }").unwrap();
    std::fs::write(
        root.join("B.java"),
        "class B { A a; int read() { return a.value; } }",
    )
    .unwrap();
    let output = root.join("out");
    let mut workspace = Workspace::new(config(&root, &output));
    workspace.wait_for_index();
    assert!(workspace
        .workspace_symbols("value")
        .iter()
        .any(|symbol| symbol.name == "value"));
    let before = workspace.index_stats();

    // Declaration change plus the dependent's updated reference.
    std::fs::write(root.join("A.java"), "class A { int renamed; }").unwrap();
    std::fs::write(
        root.join("B.java"),
        "class B { A a; int read() { return a.renamed; } }",
    )
    .unwrap();
    workspace.wait_for_index();
    assert!(workspace
        .workspace_symbols("renamed")
        .iter()
        .any(|symbol| symbol.name == "renamed"));
    let after = workspace.index_stats();
    assert_eq!(
        after.incremental_updates,
        before.incremental_updates + 1,
        "expected an incremental batch: {after:?}"
    );
    assert_eq!(
        after.full_rebuilds, before.full_rebuilds,
        "unexpected full rebuild: {after:?}"
    );
}
#[test]
fn dependents_table_tracks_references() {
    let root = fixture("dependents");
    std::fs::write(root.join("A.java"), "class A { int value; }").unwrap();
    std::fs::write(
        root.join("B.java"),
        "class B { A a; int read() { return a.value; } }",
    )
    .unwrap();
    let output = root.join("out");
    let built = build_index(&config(&root, &output));
    let a = built.file_id_for_path(&root.join("A.java")).expect("A file");
    let b = built.file_id_for_path(&root.join("B.java")).expect("B file");
    assert!(
        built.dependents_of(a).contains(&b),
        "dependents: {:?}",
        built.dependents
    );
    let closure = built.dependent_closure(a);
    assert!(closure.contains(&a) && closure.contains(&b), "{closure:?}");
    assert!(
        !built.dependents_of(b).contains(&a),
        "dependency direction is wrong"
    );
}
