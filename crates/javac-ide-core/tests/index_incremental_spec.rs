//! Incremental add/edit/delete coverage (plan §7).

mod common;
use common::platform_path;

use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::index::{self, IndexLimits, ProjectIndex};
use javac_ide_core::workspace::model::{hash_text, WorkspaceModel};
use javac_ide_core::workspace::Workspace;

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("index-incremental-spec")
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

fn build_index(config: &AnalysisConfig) -> ProjectIndex {
    let model = WorkspaceModel::from_config(config);
    index::build::build(
        &model,
        config,
        &std::collections::HashMap::new(),
        128,
        &IndexLimits::defaults(),
    )
    .index
}

#[test]
fn signature_change_rebuilds_dependents_incrementally() {
    let root = fixture("signature");
    let a_path = root.join("A.java");
    let b_path = root.join("B.java");
    std::fs::write(&a_path, "class A { int m(int x) { return x; } }").unwrap();
    std::fs::write(&b_path, "class B { A a; int read() { return a.m(1); } }").unwrap();
    let output = root.join("out");
    let mut workspace = Workspace::new(config(&root, &output));
    workspace.wait_for_index();
    assert!(workspace
        .workspace_symbols("m")
        .iter()
        .any(|symbol| symbol.name == "m"));
    let before = workspace.index_stats();

    // Add an overload: the declaration set changes while B's call still binds
    // to the original method, so B must be re-extracted through the closure.
    std::fs::write(
        &a_path,
        "class A { int m(int x) { return x; } int m(int x, int y) { return x + y; } }",
    )
    .unwrap();
    workspace.wait_for_index();
    let symbols = workspace.workspace_symbols("m");
    assert!(
        symbols.iter().filter(|symbol| symbol.name == "m").count() >= 2,
        "overload missing: {symbols:?}"
    );
    let references = workspace.references(&a_path, Position::new(0, 14));
    assert_eq!(
        references.len(),
        2,
        "expected declaration + B's call: {references:?}"
    );

    let after = workspace.index_stats();
    assert_eq!(
        after.incremental_updates,
        before.incremental_updates + 1,
        "declaration change must use the incremental path: {after:?}"
    );
    assert_eq!(
        after.full_rebuilds, before.full_rebuilds,
        "unexpected full rebuild: {after:?}"
    );

    // B's occurrence still points at the original (int) method identity.
    let snapshot = workspace.index_snapshot();
    let parameter_sets: Vec<Vec<String>> = snapshot
        .symbols
        .iter()
        .filter(|record| record.flat_name == "A.m")
        .map(|record| record.parameter_names.clone())
        .collect();
    assert!(
        parameter_sets.iter().any(|names| names == &["x".to_string()]),
        "incremental update lost the one-parameter names: {parameter_sets:?}"
    );
    assert!(
        parameter_sets
            .iter()
            .any(|names| names == &["x".to_string(), "y".to_string()]),
        "incremental update lost the overload names: {parameter_sets:?}"
    );
    let b = snapshot.file_id_for_path(&b_path).expect("B file");
    let methods: Vec<String> = snapshot
        .by_file
        .get(&b)
        .map(|file_index| {
            file_index
                .occurrences
                .iter()
                .filter_map(|occurrence| snapshot.symbol(occurrence.symbol))
                .filter(|record| record.kind == index::IndexSymbolKind::Method)
                .map(|record| {
                    format!(
                        "{} {}",
                        record.flat_name,
                        snapshot.signature(record.key.signature)
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    assert!(
        methods.iter().any(|name| name.contains("(int)")),
        "B lost its method reference: {methods:?}"
    );
}
#[test]
fn added_file_is_indexed_incrementally() {
    let root = fixture("added");
    std::fs::write(root.join("A.java"), "class A { int value; }").unwrap();
    let output = root.join("out");
    let mut workspace = Workspace::new(config(&root, &output));
    workspace.wait_for_index();
    assert!(workspace
        .workspace_symbols("A")
        .iter()
        .any(|symbol| symbol.name == "A"));
    let before = workspace.index_stats();

    std::fs::write(root.join("C.java"), "class C { int added; }").unwrap();
    workspace.wait_for_index();
    assert!(workspace
        .workspace_symbols("C")
        .iter()
        .any(|symbol| symbol.name == "C"));
    let after = workspace.index_stats();
    assert_eq!(
        after.incremental_updates,
        before.incremental_updates + 1,
        "added file must use the incremental path: {after:?}"
    );
    assert_eq!(
        after.full_rebuilds, before.full_rebuilds,
        "unexpected full rebuild: {after:?}"
    );
    assert!(
        after.files > before.files,
        "file count did not grow: {after:?}"
    );
    assert!(workspace
        .workspace_symbols("added")
        .iter()
        .any(|symbol| symbol.name == "added"));
}

#[test]
fn deleted_file_clears_dependent_occurrences_without_rebuild() {
    let root = fixture("deleted");
    let a_path = root.join("A.java");
    let b_path = root.join("B.java");
    std::fs::write(&a_path, "class A { int value; }").unwrap();
    std::fs::write(&b_path, "class B { A a; int read() { return a.value; } }").unwrap();
    let output = root.join("out");
    let mut workspace = Workspace::new(config(&root, &output));
    workspace.wait_for_index();
    assert!(workspace
        .workspace_symbols("value")
        .iter()
        .any(|symbol| symbol.name == "value"));
    let before = workspace.index_stats();

    std::fs::remove_file(&a_path).unwrap();
    workspace.wait_for_index();
    assert!(
        !workspace
            .workspace_symbols("value")
            .iter()
            .any(|symbol| symbol.name == "value"),
        "deleted declaration still indexed"
    );
    let after = workspace.index_stats();
    assert_eq!(
        after.incremental_updates,
        before.incremental_updates + 1,
        "delete must use the incremental path: {after:?}"
    );
    assert_eq!(
        after.full_rebuilds, before.full_rebuilds,
        "unexpected full rebuild: {after:?}"
    );

    let snapshot = workspace.index_snapshot();
    let b = snapshot.file_id_for_path(&b_path).expect("B file");
    let stale: Vec<String> = snapshot
        .by_file
        .get(&b)
        .map(|file_index| {
            file_index
                .occurrences
                .iter()
                .filter_map(|occurrence| snapshot.symbol(occurrence.symbol))
                .filter(|record| record.flat_name == "A.value")
                .map(|record| record.flat_name.clone())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        stale.is_empty(),
        "dependent still references the deleted symbol: {stale:?}"
    );
}

#[test]
fn references_outside_the_rebuild_set_force_a_full_rebuild() {
    let root = fixture("outside");
    let a_path = root.join("A.java");
    std::fs::write(&a_path, "class A { int value; }").unwrap();
    std::fs::write(
        root.join("B.java"),
        "class B { A a; int read() { return a.value; } }",
    )
    .unwrap();
    let config = config(&root, &root.join("out"));
    let built = build_index(&config);
    let a = built.file_id_for_path(&a_path).expect("A file");

    // Remove `value` while B (not in the batch) still references it.
    let text = "class A { int renamed; }";
    let extraction = index::extract::extract_file(&config, &a_path, a, text);
    assert!(!extraction.failed);
    let tasks = vec![index::update::UpdateTask::Update {
        file: a,
        text_hash: hash_text(text),
        extraction,
    }];
    assert!(
        index::update::apply_batch(&built, &tasks).is_none(),
        "a reference outside the rebuild set must abort the batch"
    );
}

#[test]
fn uncompilable_dependent_falls_back_to_full_rebuild() {
    let root = fixture("fallback");
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

    // Remove the field but leave B untouched: B no longer compiles, so the
    // closure rebuild must fall back to a full build.
    std::fs::write(root.join("A.java"), "class A { int renamed; }").unwrap();
    workspace.wait_for_index();
    let _ = workspace.workspace_symbols("A");
    let after = workspace.index_stats();
    assert!(
        after.full_rebuilds > before.full_rebuilds,
        "expected a full rebuild fallback: {after:?}"
    );
    assert!(
        after.incremental_updates == before.incremental_updates,
        "incremental path must not claim success: {after:?}"
    );
}
