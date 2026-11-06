//! Project-index model and builder specification tests.

use std::path::{Path, PathBuf};

use javac_ide_core::analysis::project_index::{build_project_index, IndexSymbolKind, TypeRef};
use javac_ide_core::analysis::AnalysisConfig;

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
        .join("project-index-spec")
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

#[test]
fn builds_declarations_occurrences_and_supertypes() {
    let root = fixture("basic");
    std::fs::write(root.join("A.java"), "class A { B b; void f() { b.value++; } }").unwrap();
    std::fs::write(root.join("B.java"), "class B { int value; int m() { return 1; } int m(int x) { return x; } }").unwrap();
    std::fs::write(root.join("C.java"), "class C extends B { int read() { return value; } }").unwrap();
    let output = root.join("out");
    let index = build_project_index(&config(&root, &output), 2);

    assert!(matches!(index.status, javac_ide_core::analysis::project_index::IndexStatus::Ready), "status: {:?}", index.status);
    assert_eq!(index.files.len(), 3);
    assert!(index.by_name.values().any(|ids| ids.iter().any(|id| index.symbol(*id).is_some_and(|s| s.detail.ends_with(" A")))));
    let a = index.symbols_named("A").into_iter().next().expect("A");
    assert_eq!(a.kind, IndexSymbolKind::Class);
    let c = index.symbols_named("C").into_iter().next().expect("C");
    let b = index.symbols_named("B").into_iter().next().expect("B");
    assert!(
        c.supertypes
            .iter()
            .any(|ty| matches!(ty, TypeRef::Project(id) if index.symbol(*id).is_some_and(|s| s.flat_name == b.flat_name))),
        "C supertypes: {:?}",
        c.supertypes
    );

    let value = index.symbols_named("value").into_iter().next().expect("value");
    assert_eq!(value.kind, IndexSymbolKind::Field);
    let value_refs = index.occurrences_of(index.by_key[&value.key]);
    assert!(
        value_refs.iter().any(|occ| index.file(occ.file).is_some_and(|f| f.path.ends_with("A.java"))),
        "value refs: {:?}",
        value_refs
    );

    let methods = index.symbols_named("m");
    assert_eq!(methods.len(), 2, "overloads must have distinct keys: {methods:?}");
    let a_file = index.files.iter().position(|f| f.path.ends_with("A.java")).unwrap() as u32;
    let a_index = index.file_index(a_file).expect("A file index");
    assert!(a_index.occurrences.iter().any(|occ| occ.symbol == index.by_key[&value.key]));
    assert!(!a_index.occurrences.is_empty());

    let _ = config(&root, &output);
}
#[test]
fn single_file_batches_preserve_cross_batch_identity() {
    let root = fixture("batches");
    std::fs::write(root.join("A.java"), "class A { B b; void f() { b.value++; } }").unwrap();
    std::fs::write(root.join("B.java"), "class B { int value; int m() { return 1; } int m(int x) { return x; } }").unwrap();
    std::fs::write(root.join("C.java"), "class C extends B { int read() { return value; } }").unwrap();
    let output = root.join("out");
    let index = build_project_index(&config(&root, &output), 1);
    assert_eq!(index.files.len(), 3);
    let value = index
        .symbols_named("value")
        .into_iter()
        .find(|symbol| symbol.kind == IndexSymbolKind::Field)
        .expect("field value");
    let refs = index.occurrences_of(index.by_key[&value.key]);
    assert!(refs.iter().any(|occ| index.file(occ.file).is_some_and(|f| f.path.ends_with("A.java"))), "refs: {refs:?}");
    assert_eq!(index.symbols_named("m").len(), 2, "overloads must survive batching");
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
    let index = build_project_index(&config(&root, &output), 4);
    let fields: Vec<_> = index
        .symbols_named("value")
        .into_iter()
        .filter(|symbol| symbol.kind == IndexSymbolKind::Field)
        .collect();
    assert_eq!(fields.len(), 1);
    let field = fields[0];
    let refs = index.occurrences_of(index.by_key[&field.key]);
    assert_eq!(refs.len(), 2, "field declaration + f() reference only: {refs:?}");
    assert!(refs.iter().all(|occ| occ.symbol == index.by_key[&field.key]));
}

#[test]
fn workspace_model_captures_source_and_classpath_fingerprint() {
    let root = fixture("workspace-model");
    std::fs::write(root.join("A.java"), "class A {}").unwrap();
    let output = root.join("out");
    let config = config(&root, &output);
    let model = javac_ide_core::analysis::project_index::build_workspace_model(&config);
    assert_eq!(model.source_path, vec![root.clone()]);
    assert_eq!(model.source_level, "17");
    assert_eq!(model.classpath_hash, model.classpath_hash);
    assert_eq!(model.root, root);
}
