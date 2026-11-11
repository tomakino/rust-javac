//! Same-package type references must resolve through the index.

mod common;
use common::platform_path;

use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::index::IndexStatus;
use javac_ide_core::workspace::Workspace;

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("definition-same-package")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn config(root: &Path) -> AnalysisConfig {
    platform_path();
    AnalysisConfig {
        class_path: Vec::new(),
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: vec![root.to_path_buf()],
    }
}

#[test]
fn definition_resolves_a_same_package_type() {
    let root = fixture("simple");
    let a = root.join("A.java");
    let b = root.join("B.java");
    std::fs::write(&a, "class A { B b; }\n").unwrap();
    std::fs::write(&b, "class B { int value; }\n").unwrap();

    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let definition = workspace.definition(&a, Position::new(0, 10));
    let definition = definition.expect("definition of B");
    assert!(
        definition.file.ends_with("B.java"),
        "unexpected definition: {definition:?}"
    );
}

#[test]
fn definition_resolves_a_same_package_type_with_a_jdk_import() {
    let root = fixture("packaged");
    std::fs::create_dir_all(root.join("com/example")).unwrap();
    let a = root.join("com/example/App.java");
    let b = root.join("com/example/Service.java");
    std::fs::write(
        &a,
        "package com.example;\nimport java.util.List;\npublic class App {\n    private final Service service = new Service();\n}\n",
    )
    .unwrap();
    std::fs::write(&b, "package com.example;\npublic class Service {}\n").unwrap();

    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let definition = workspace.definition(&a, Position::new(3, 20));
    let definition = definition.expect("definition of Service");
    assert!(
        definition.file.ends_with("Service.java"),
        "unexpected definition: {definition:?}"
    );
}
#[test]
fn definition_works_on_a_partial_index() {
    let root = fixture("partial");
    std::fs::create_dir_all(root.join("com/example")).unwrap();
    let a = root.join("com/example/App.java");
    let b = root.join("com/example/Service.java");
    std::fs::write(
        &a,
        "package com.example;\nimport ja;\npublic class App {\n    private final Service service = new Service();\n}\n",
    )
    .unwrap();
    std::fs::write(&b, "package com.example;\npublic class Service {}\n").unwrap();

    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    assert!(
        matches!(
            workspace.index_snapshot().status,
            IndexStatus::Partial { .. }
        ),
        "expected a partial index: {:?}",
        workspace.index_snapshot().status
    );
    let definition = workspace.definition(&a, Position::new(3, 20));
    let definition = definition.expect("definition of Service on a partial index");
    assert!(
        definition.file.ends_with("Service.java"),
        "unexpected definition: {definition:?}"
    );
}
#[test]
fn definition_without_an_index_lands_on_the_declared_name() {
    platform_path();
    let root = fixture("fallback-position");
    let path = root.join("A.java");
    std::fs::write(&path, "class A {\n    B b;\n}\nclass B {}\n").unwrap();
    let mut workspace = Workspace::new(AnalysisConfig {
        class_path: Vec::new(),
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: Vec::new(),
    });
    let definition = workspace
        .definition(&path, Position::new(1, 4))
        .expect("definition of B");
    assert!(definition.file.ends_with("A.java"));
    assert_eq!(definition.range.start.line, 3);
    assert_eq!(
        definition.range.start.column, 6,
        "cursor should land on the type name: {definition:?}"
    );
}
