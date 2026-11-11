//! M5 code lenses (reference/implementation counts).

mod common;
use common::platform_dir;

use std::path::{Path, PathBuf};

use javac_analysis_api::WorkspaceService;

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::Workspace;

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("code-lens-spec")
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

fn workspace_with(root: &Path, files: &[(&str, &str)]) -> Workspace {
    for (name, text) in files {
        std::fs::write(root.join(name), text).unwrap();
    }
    let mut workspace = Workspace::new(config(root));
    workspace.wait_for_index();
    workspace
}

#[test]
fn item01_zero_references() {
    let root = fixture("item01");
    let path = root.join("A.java");
    std::fs::write(&path, "class A {\n  void f() {}\n}\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let lenses = workspace.code_lenses(&path);
    assert!(lenses.iter().any(|lens| lens.title == "0 references"), "{lenses:?}");
}

#[test]
fn item02_one_reference() {
    let root = fixture("item02");
    let mut workspace = workspace_with(
        &root,
        &[
            ("A.java", "class A {\n  void f() {}\n}\n"),
            ("B.java", "class B {\n  void g(A a) { a.f(); }\n}\n"),
        ],
    );
    let lenses = workspace.code_lenses(&root.join("A.java"));
    assert!(lenses.iter().any(|lens| lens.title == "1 references"), "{lenses:?}");
}

#[test]
fn item03_multiple_references() {
    let root = fixture("item03");
    let mut workspace = workspace_with(
        &root,
        &[
            ("A.java", "class A {\n  void f() {}\n}\n"),
            ("B.java", "class B {\n  void g(A a) { a.f(); a.f(); }\n}\n"),
        ],
    );
    let lenses = workspace.code_lenses(&root.join("A.java"));
    assert!(lenses.iter().any(|lens| lens.title == "2 references"), "{lenses:?}");
}

#[test]
fn item04_cross_file_references() {
    let root = fixture("item04");
    let mut workspace = workspace_with(
        &root,
        &[
            ("A.java", "class A {\n  int value;\n}\n"),
            ("B.java", "class B {\n  int read(A a) { return a.value; }\n}\n"),
        ],
    );
    let lenses = workspace.code_lenses(&root.join("A.java"));
    assert!(lenses.iter().any(|lens| lens.title == "1 references"), "{lenses:?}");
}

#[test]
fn item05_partial_index_no_lenses() {
    let root = fixture("item05");
    let path = root.join("A.java");
    std::fs::write(&path, "class A {\n  void f() {}\n}\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.set_build_delay_for_tests(std::time::Duration::from_millis(1500));
    // Initial build in flight: the status is not Ready.
    let lenses = workspace.code_lenses(&path);
    assert!(lenses.is_empty(), "{lenses:?}");
}

#[test]
fn item06_class_and_method_lenses() {
    let root = fixture("item06");
    let mut workspace = workspace_with(
        &root,
        &[
            ("A.java", "class A {\n  void f() {}\n}\n"),
            ("B.java", "class B extends A {\n  void g() { f(); }\n}\n"),
        ],
    );
    let lenses = workspace.code_lenses(&root.join("A.java"));
    // Class: references and implementations; method: references.
    assert!(lenses.iter().any(|lens| lens.title.ends_with("references")), "{lenses:?}");
    assert!(lenses.iter().any(|lens| lens.title.ends_with("implementations")), "{lenses:?}");
}

#[test]
fn item07_lens_positions_with_chinese_and_emoji() {
    let root = fixture("item07");
    std::fs::write(
        root.join("A.java"),
        "// 中文注释 😀\nclass A {\n  int 变量;\n}\n",
    )
    .unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let lenses = workspace.code_lenses(&root.join("A.java"));
    // The field declaration is on line 2, UTF-16 column 6.
    assert!(
        lenses
            .iter()
            .any(|lens| lens.range.start.line == 2 && lens.range.start.column == 6),
        "{lenses:?}"
    );
}