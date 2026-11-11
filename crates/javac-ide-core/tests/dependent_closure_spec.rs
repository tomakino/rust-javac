//! Dependency-closure API used by artifact incremental rebuilds.

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
        .join("dependent-closure-spec")
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
fn dependent_closure_contains_referrers() {
    let root = fixture("referrers");
    std::fs::write(root.join("A.java"), "class A { int value; }\n").unwrap();
    std::fs::write(
        root.join("B.java"),
        "class B extends A { int read() { return value; } }\n",
    )
    .unwrap();
    std::fs::write(root.join("C.java"), "class C { int other; }\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    // Editor paths are absolute without `..`; canonicalize the fixture root so
    // the test path matches the index records.
    let base = root.canonicalize().unwrap();

    let closure = workspace.dependent_closure(&base.join("A.java"));
    let names: Vec<String> = closure
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(names.contains(&"A.java".to_string()), "{names:?}");
    assert!(names.contains(&"B.java".to_string()), "{names:?}");
    assert!(!names.contains(&"C.java".to_string()), "{names:?}");
}
