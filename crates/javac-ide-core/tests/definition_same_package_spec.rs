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
fn import_definition_maps_dots_and_semicolon_to_the_simple_name() {
    let root = fixture("import-click");
    std::fs::create_dir_all(root.join("com/example/base")).unwrap();
    std::fs::create_dir_all(root.join("com/example/app")).unwrap();
    let model = root.join("com/example/base/Model.java");
    std::fs::write(
        &model,
        "package com.example.base;\npublic class Model {}\n",
    )
    .unwrap();
    let app = root.join("com/example/app/App.java");
    let source = "package com.example.app;\nimport com.example.base.Model;\npublic class App { Model model; }\n";
    std::fs::write(&app, source).unwrap();

    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let line_start = source.find("import ").unwrap();
    let dot = line_start + "import com.example.base".len();
    let semicolon = line_start + source[line_start..].find(';').unwrap();
    let column = |offset: usize| {
        source[line_start..offset]
            .chars()
            .map(|ch| ch.len_utf16() as u32)
            .sum::<u32>()
    };
    for column in [column(dot), column(semicolon)] {
        let definition = workspace.definition(&app, Position::new(1, column));
        assert!(
            definition
                .as_ref()
                .is_some_and(|location| location.file.ends_with("Model.java")),
            "column {column}: {definition:?}"
        );
    }
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
