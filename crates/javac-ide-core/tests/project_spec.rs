//! Project model and core-provider tests.
//!
//! Concrete build systems (Maven) live in their own crates; this file only
//! covers the core API and the directory-scan fallback.

use std::path::{Path, PathBuf};

use javac_ide_core::project::{
    self, BuildFingerprint, BuildSourceSet, BuildSystemId, LoadContext, Module, ProjectModel,
    SourceSetKind,
};
use javac_ide_core::workspace::model::WorkspaceModel;

fn fixture(name: &str) -> PathBuf {
    let root = PathBuf::from("target/tmp/project-spec").join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn builtin_registry_contains_the_directory_scan_fallback() {
    let providers = project::providers();
    assert_eq!(providers.len(), 1);
    let fallback = &providers[0];
    assert_eq!(fallback.id(), BuildSystemId::NONE);
    assert!(
        fallback.detect(Path::new("target/tmp")),
        "fallback must always detect"
    );
}

#[test]
fn fallback_detects_conventional_source_roots() {
    let root = fixture("dir-scan");
    std::fs::create_dir_all(root.join("src/main/java")).unwrap();
    std::fs::create_dir_all(root.join("src/test/java")).unwrap();
    let model = project::load(&root, &LoadContext::default());
    assert_eq!(model.provider, BuildSystemId::NONE);
    assert_eq!(model.modules.len(), 1);
    let kinds: Vec<SourceSetKind> = model.modules[0]
        .source_sets
        .iter()
        .map(|set| set.kind)
        .collect();
    assert_eq!(kinds, vec![SourceSetKind::Main, SourceSetKind::Test]);
    assert!(model.modules[0].source_sets[0].roots[0].ends_with("src/main/java"));
    assert!(model.modules[0].source_sets[1].roots[0].ends_with("src/test/java"));
    assert!(model.class_path().is_empty());
}

#[test]
fn fallback_ignores_absent_source_roots() {
    let root = fixture("empty");
    let model = project::load(&root, &LoadContext::default());
    assert_eq!(model.provider, BuildSystemId::NONE);
    assert!(model.modules.is_empty());
    assert!(model.source_roots().is_empty());
}

#[test]
fn workspace_model_declares_main_and_test_sets() {
    let root = fixture("workspace-model");
    std::fs::create_dir_all(root.join("src/main/java")).unwrap();
    std::fs::create_dir_all(root.join("src/test/java")).unwrap();
    let model = project::load(&root, &LoadContext::default());
    let workspace = WorkspaceModel::from_project(&model);
    assert_eq!(workspace.root, root);
    let kinds: Vec<SourceSetKind> = workspace.sources.iter().map(|set| set.kind).collect();
    assert_eq!(kinds, vec![SourceSetKind::Main, SourceSetKind::Test]);
    assert_eq!(
        workspace.source_roots().len(),
        2,
        "roots: {:?}",
        workspace.source_roots()
    );
}

#[test]
fn workspace_model_merges_modules_and_deduplicates_roots() {
    let root = PathBuf::from("target/tmp/project-spec/merge");
    let shared = root.join("shared");
    let model = ProjectModel {
        root: root.clone(),
        provider: BuildSystemId::NONE,
        modules: vec![
            Module {
                root: root.join("a"),
                name: "a".to_string(),
                source_sets: vec![
                    BuildSourceSet::new(
                        SourceSetKind::Main,
                        vec![shared.clone()],
                        vec![PathBuf::from("lib/a.jar")],
                        None,
                    ),
                    BuildSourceSet::new(
                        SourceSetKind::Test,
                        vec![root.join("a/src/test/java")],
                        vec![PathBuf::from("lib/a.jar")],
                        None,
                    ),
                ],
                jdk_home: None,
                source_level: None,
            },
            Module {
                root: root.join("b"),
                name: "b".to_string(),
                source_sets: vec![BuildSourceSet::new(
                    SourceSetKind::Main,
                    vec![shared.clone()],
                    vec![PathBuf::from("lib/b.jar")],
                    None,
                )],
                jdk_home: None,
                source_level: Some("21".to_string()),
            },
        ],
        fingerprint: BuildFingerprint::default(),
    };
    let workspace = WorkspaceModel::from_project(&model);
    assert_eq!(workspace.root, root);
    assert_eq!(workspace.source_level, "21");
    assert_eq!(workspace.sources.len(), 2, "main and test only");

    let main = workspace
        .sources
        .iter()
        .find(|set| set.kind == SourceSetKind::Main)
        .expect("main");
    assert_eq!(main.roots, vec![shared], "shared root duplicated");
    assert_eq!(
        main.classpath,
        vec![PathBuf::from("lib/a.jar"), PathBuf::from("lib/b.jar")]
    );
    let test = workspace
        .sources
        .iter()
        .find(|set| set.kind == SourceSetKind::Test)
        .expect("test");
    assert_eq!(test.roots, vec![root.join("a/src/test/java")]);
    assert_eq!(
        workspace.class_path(),
        vec![PathBuf::from("lib/a.jar"), PathBuf::from("lib/b.jar")]
    );
}

#[test]
fn old_source_set_kind_path_still_works() {
    // The index re-exports the type so existing call sites keep compiling.
    let kind: javac_ide_core::workspace::index::model::SourceSetKind = SourceSetKind::Main;
    assert_eq!(kind.tag(), 1);
    assert_eq!(SourceSetKind::from_tag(2), Some(SourceSetKind::Test));
    assert_eq!(SourceSetKind::from_tag(9), None);
}
