//! Project-index specification tests (docs/IDE_INDEX_PLAN.md §10).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use javac_analysis_api::WorkspaceService;

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::index::{
    self, IndexLimits, IndexStatus, IndexSymbolKind, ProjectIndex, TypeRef,
};
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
            std::env::set_var("RAVAC_PLATFORM_DIR", root);
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
        .join("index-spec")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn config(source: &Path, output: &Path) -> AnalysisConfig {
    platform_dir();
    AnalysisConfig {
        class_path: Vec::new(),
        output_dir: output.to_path_buf(),
        source_level: "17".to_string(),
        source_path: vec![source.to_path_buf()],
    }
}

fn build_one(config: &AnalysisConfig, batch: usize) -> ProjectIndex {
    let model = WorkspaceModel::from_config(config);
    index::build::build(
        &model,
        config,
        &HashMap::new(),
        batch,
        &IndexLimits::defaults(),
    )
    .index
}

#[test]
fn builds_declarations_occurrences_and_supertypes() {
    let root = fixture("basic");
    std::fs::write(
        root.join("A.java"),
        "class A { B b; void f() { b.value++; } }",
    )
    .unwrap();
    std::fs::write(
        root.join("B.java"),
        "class B { int value; int m() { return 1; } int m(int x) { return x; } }",
    )
    .unwrap();
    std::fs::write(
        root.join("C.java"),
        "class C extends B { int read() { return value; } }",
    )
    .unwrap();
    let output = root.join("out");
    let index = build_one(&config(&root, &output), 2);

    assert!(
        matches!(index.status, IndexStatus::Ready),
        "status: {:?}",
        index.status
    );
    assert_eq!(index.files.len(), 3);
    let a = index.symbols_named("A").into_iter().next().expect("A");
    assert_eq!(a.kind, IndexSymbolKind::Class);
    assert!(a.flat_name.ends_with('A'));
    let c = index.symbols_named("C").into_iter().next().expect("C");
    let b = index.symbols_named("B").into_iter().next().expect("B");
    assert!(
        c.supertypes
            .iter()
            .any(|ty| matches!(ty, TypeRef::Project(id) if index.symbol(*id).is_some_and(|s| s.flat_name == b.flat_name))),
        "C supertypes: {:?}",
        c.supertypes
    );

    let value = index
        .symbols_named("value")
        .into_iter()
        .next()
        .expect("value");
    assert_eq!(value.kind, IndexSymbolKind::Field);
    let value_refs = index.occurrences_of(index.by_key[&value.key]);
    assert!(
        value_refs
            .iter()
            .any(|occ| index.file(occ.file).is_some_and(|f| f.path.ends_with("A.java"))),
        "value refs: {:?}",
        value_refs
    );

    let methods = index.symbols_named("m");
    assert_eq!(methods.len(), 2, "overloads must have distinct keys");
    let a_file = index
        .files
        .iter()
        .position(|f| f.path.ends_with("A.java"))
        .unwrap() as u32;
    let a_index = index.file_index(a_file).expect("A file index");
    assert!(
        a_index
            .occurrences
            .iter()
            .any(|occ| occ.symbol == index.by_key[&value.key])
    );
    assert!(!a_index.occurrences.is_empty());
}

#[test]
fn single_file_batches_preserve_cross_batch_identity() {
    let root = fixture("batches");
    std::fs::write(
        root.join("A.java"),
        "class A { B b; void f() { b.value++; } }",
    )
    .unwrap();
    std::fs::write(
        root.join("B.java"),
        "class B { int value; int m() { return 1; } int m(int x) { return x; } }",
    )
    .unwrap();
    std::fs::write(
        root.join("C.java"),
        "class C extends B { int read() { return value; } }",
    )
    .unwrap();
    let output = root.join("out");
    let index = build_one(&config(&root, &output), 1);
    assert_eq!(index.files.len(), 3);
    let value = index
        .symbols_named("value")
        .into_iter()
        .find(|symbol| symbol.kind == IndexSymbolKind::Field)
        .expect("field value");
    let refs = index.occurrences_of(index.by_key[&value.key]);
    assert!(
        refs.iter()
            .any(|occ| index.file(occ.file).is_some_and(|f| f.path.ends_with("A.java"))),
        "refs: {refs:?}"
    );
    assert_eq!(
        index.symbols_named("m").len(),
        2,
        "overloads must survive batching"
    );
    let c = index.symbols_named("C").into_iter().next().expect("C");
    let b = index.symbols_named("B").into_iter().next().expect("B");
    assert!(c.supertypes.iter().any(|ty| matches!(ty, TypeRef::Project(id) if index.symbol(*id).is_some_and(|s| s.flat_name == b.flat_name))));
}

#[test]
fn comments_strings_and_shadowed_locals_do_not_pollute_field_references() {
    let root = fixture("occurrences");
    let source = "class D { int value; void f() { String s = \"value\"; // value\n value++; } void g(int value) { value++; } }";
    std::fs::write(root.join("D.java"), source).unwrap();
    let output = root.join("out");
    let index = build_one(&config(&root, &output), 4);
    let fields: Vec<_> = index
        .symbols_named("value")
        .into_iter()
        .filter(|symbol| symbol.kind == IndexSymbolKind::Field)
        .collect();
    assert_eq!(fields.len(), 1);
    let field = fields[0];
    let refs = index.occurrences_of(index.by_key[&field.key]);
    assert_eq!(
        refs.len(),
        2,
        "field declaration + f() reference only: {refs:?}"
    );
    assert!(refs.iter().all(|occ| occ.symbol == index.by_key[&field.key]));
}

#[test]
fn same_name_types_in_different_packages_have_distinct_keys() {
    let root = fixture("same-name");
    std::fs::create_dir_all(root.join("p1")).unwrap();
    std::fs::create_dir_all(root.join("p2")).unwrap();
    std::fs::write(root.join("p1").join("A.java"), "package p1; public class A {}").unwrap();
    std::fs::write(root.join("p2").join("A.java"), "package p2; public class A {}").unwrap();
    let output = root.join("out");
    let index = build_one(&config(&root, &output), 8);
    let flattened: Vec<&str> = index
        .symbols
        .iter()
        .filter(|symbol| symbol.kind == IndexSymbolKind::Class)
        .map(|symbol| symbol.flat_name.as_str())
        .collect();
    assert!(flattened.contains(&"p1.A"), "symbols: {flattened:?}");
    assert!(flattened.contains(&"p2.A"), "symbols: {flattened:?}");
    assert_eq!(
        flattened.iter().filter(|name| name.ends_with(".A")).count(),
        2
    );
}

#[test]
fn interface_inheritance_and_external_supertypes() {
    let root = fixture("supertypes");
    std::fs::write(root.join("I.java"), "interface I { void run(); }").unwrap();
    std::fs::write(root.join("Impl.java"), "class Impl implements I { public void run() {} }")
        .unwrap();
    std::fs::write(
        root.join("Ext.java"),
        "class Ext extends java.util.ArrayList<String> {}",
    )
    .unwrap();
    let output = root.join("out");
    let index = build_one(&config(&root, &output), 8);
    let imp = index.symbols_named("Impl").into_iter().next().expect("Impl");
    assert!(
        imp.supertypes
            .iter()
            .any(|ty| matches!(ty, TypeRef::Project(id) if index.symbol(*id).is_some_and(|s| s.flat_name.ends_with('I')))),
        "Impl supertypes: {:?}",
        imp.supertypes
    );
    let ext = index.symbols_named("Ext").into_iter().next().expect("Ext");
    assert!(
        ext.supertypes
            .iter()
            .any(|ty| matches!(ty, TypeRef::External(name) if name.contains("ArrayList"))),
        "Ext supertypes: {:?}",
        ext.supertypes
    );
}

#[test]
fn workspace_model_captures_source_and_classpath_fingerprint() {
    let root = fixture("workspace-model");
    std::fs::write(root.join("A.java"), "class A {}").unwrap();
    let output = root.join("out");
    let config = config(&root, &output);
    let model = WorkspaceModel::from_config(&config);
    assert_eq!(model.source_roots(), vec![root.clone()]);
    assert_eq!(model.source_level, "17");
    let mut other = config.clone();
    other.class_path.push(root.join("different.jar"));
    let other_model = WorkspaceModel::from_config(&other);
    assert_ne!(model.classpath_hash, other_model.classpath_hash);
    assert_eq!(model.root, root);
}

#[test]
fn workspace_symbols_read_the_project_index() {
    let root = fixture("workspace-symbols");
    std::fs::write(
        root.join("A.java"),
        "class A { B b; void call() { b.value++; } }",
    )
    .unwrap();
    std::fs::write(root.join("B.java"), "class B { int value; void run() {} }").unwrap();
    let output = root.join("out");
    let mut workspace = Workspace::new(config(&root, &output));
    workspace.wait_for_index();
    let symbols = workspace.workspace_symbols("B");
    assert!(
        symbols.iter().any(|symbol| symbol.name == "B"),
        "symbols: {symbols:?}"
    );
    assert!(
        symbols.iter().all(|symbol| symbol.location.file.exists()),
        "locations: {symbols:?}"
    );
    let value = workspace.workspace_symbols("value");
    assert!(
        value.iter().any(|symbol| symbol.name == "value"),
        "symbols: {value:?}"
    );
    let all = workspace.workspace_symbols("");
    assert!(all.len() >= 4, "all symbols: {all:?}");
}

#[test]
fn failed_compilation_is_partial_not_ready() {
    let root = fixture("partial");
    std::fs::write(root.join("Bad.java"), "class Bad { void broken( { }").unwrap();
    let output = root.join("out");
    let index = build_one(&config(&root, &output), 8);
    assert!(!index.is_ready(), "status: {:?}", index.status);
    assert!(matches!(index.status, IndexStatus::Partial { .. }));
}

#[test]
fn added_removed_and_modified_files_update_the_index() {
    let root = fixture("incremental");
    std::fs::write(root.join("A.java"), "class A { B b; }").unwrap();
    std::fs::write(root.join("B.java"), "class B { int oldValue; }").unwrap();
    let output = root.join("out");
    let mut workspace = Workspace::new(config(&root, &output));
    workspace.wait_for_index();
    assert!(workspace
        .workspace_symbols("B")
        .iter()
        .any(|symbol| symbol.name == "B"));

    // Modified file: the workspace must detect the disk hash change.
    std::fs::write(root.join("B.java"), "class B { int newValue; }").unwrap();
    workspace.wait_for_index();
    let symbols = workspace.workspace_symbols("newValue");
    assert!(
        symbols.iter().any(|symbol| symbol.name == "newValue"),
        "stale index: {symbols:?}"
    );

    // Added file.
    std::fs::write(root.join("C.java"), "class C { int addedField; }").unwrap();
    workspace.wait_for_index();
    assert!(workspace
        .workspace_symbols("addedField")
        .iter()
        .any(|symbol| symbol.name == "addedField"));

    // Removed file.
    std::fs::remove_file(root.join("C.java")).unwrap();
    workspace.wait_for_index();
    assert!(!workspace
        .workspace_symbols("addedField")
        .iter()
        .any(|symbol| symbol.name == "addedField"));
}

#[test]
fn overlay_text_is_indexed() {
    let root = fixture("overlay");
    std::fs::write(root.join("A.java"), "class A { int oldValue; }").unwrap();
    let output = root.join("out");
    let mut workspace = Workspace::new(config(&root, &output));
    workspace.wait_for_index();
    let path = root.join("A.java");
    let unsaved = "class A { int overlayValue; }";
    workspace.diagnostics(&path, Some(unsaved));
    workspace.wait_for_index();
    let symbols = workspace.workspace_symbols("overlayValue");
    assert!(
        symbols.iter().any(|symbol| symbol.name == "overlayValue"),
        "overlay not indexed: {symbols:?}"
    );
}

#[test]
fn disk_roundtrip_and_hash_mismatch_fallback() {
    let root = fixture("dump");
    std::fs::write(root.join("A.java"), "/** A doc. */\nclass A { int value; }").unwrap();
    std::fs::write(root.join("B.java"), "class B { A a; }").unwrap();
    let output = root.join("out");
    // Isolate the cache directory so the test never touches the real one.
    let cache = root.join("cache");
    std::fs::create_dir_all(&cache).unwrap();
    unsafe {
        std::env::set_var("LOCALAPPDATA", &cache);
        std::env::set_var("XDG_CACHE_HOME", &cache);
    }
    let config = config(&root, &output);
    let index = build_one(&config, 8);
    let path = index::dump::cache_path(index::dump::workspace_hash(
        &WorkspaceModel::from_config(&config),
    ))
    .expect("cache path");
    index::dump::save(&index, &path).unwrap();
    let loaded = index::dump::load(&WorkspaceModel::from_config(&config)).expect("load");
    assert_eq!(loaded.symbols.len(), index.symbols.len());
    assert_eq!(loaded.files.len(), index.files.len());
    let loaded_a = loaded.symbols_named("A").into_iter().next().expect("A");
    assert!(
        loaded_a.javadoc.as_deref().unwrap_or("").contains("A doc"),
        "javadoc lost on roundtrip: {:?}",
        loaded_a.javadoc
    );

    // Change a file without rebuilding: the recorded hash no longer matches.
    std::fs::write(root.join("A.java"), "class A { int other; }").unwrap();
    assert!(
        index::dump::load(&WorkspaceModel::from_config(&config)).is_none(),
        "a stale file hash must discard the cache"
    );
}

#[test]
fn file_threshold_reports_partial() {
    let root = fixture("threshold");
    std::fs::write(root.join("A.java"), "class A {}").unwrap();
    std::fs::write(root.join("B.java"), "class B {}").unwrap();
    let output = root.join("out");
    let config = config(&root, &output);
    let model = WorkspaceModel::from_config(&config);
    let limits = IndexLimits::new(1, 1024 * 1024);
    let result = index::build::build(&model, &config, &HashMap::new(), 8, &limits);
    assert!(matches!(result.index.status, IndexStatus::Partial { .. }));
    assert_eq!(result.skipped_files.len(), 1);
    assert_eq!(result.index.files.len(), 1);
}

#[test]
fn symbol_records_carry_rendered_javadoc() {
    let root = fixture("javadoc");
    std::fs::write(
        root.join("A.java"),
        "/** A doc. */\nclass A {\n    /** value doc. */\n    int value;\n    /** read doc. */\n    int read() { return value; }\n}\n",
    )
    .unwrap();
    let output = root.join("out");
    let index = build_one(&config(&root, &output), 8);
    let class = index.symbols_named("A").into_iter().next().expect("A");
    assert!(
        class.javadoc.as_deref().unwrap_or("").contains("A doc"),
        "javadoc: {:?}",
        class.javadoc
    );
    let value = index
        .symbols_named("value")
        .into_iter()
        .next()
        .expect("value");
    assert!(
        value.javadoc.as_deref().unwrap_or("").contains("value doc"),
        "javadoc: {:?}",
        value.javadoc
    );
}

#[test]
fn hover_reads_javadoc_from_the_index() {
    use javac_analysis_api::Position;
    let root = fixture("hover-doc");
    let path = root.join("A.java");
    std::fs::write(
        &path,
        "/** A doc. */\nclass A {\n    /** value doc. */\n    int value;\n}\n",
    )
    .unwrap();
    let output = root.join("out");
    let mut workspace = Workspace::new(config(&root, &output));
    workspace.wait_for_index();
    let hover = workspace.hover(&path, Position::new(3, 8)).expect("hover");
    assert!(hover.contains("value doc"), "hover: {hover}");
    assert!(hover.contains("int value"), "hover: {hover}");
}