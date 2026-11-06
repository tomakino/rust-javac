//! Build-time snapshot behavior (plan §7: snapshot / build thread).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use javac_analysis_api::{Position, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
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
        .join("index-snapshot-spec")
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
fn queries_serve_the_previous_snapshot_during_a_build() {
    let root = fixture("previous");
    std::fs::write(root.join("A.java"), "class A { int oldValue; }").unwrap();
    std::fs::write(root.join("B.java"), "class B { A a; }").unwrap();
    let output = root.join("out");
    let mut workspace = Workspace::new(config(&root, &output));
    workspace.wait_for_index();
    assert!(!workspace.is_building());
    assert!(workspace
        .workspace_symbols("oldValue")
        .iter()
        .any(|symbol| symbol.name == "oldValue"));

    // Make the rebuild observable, then start it.
    workspace.set_build_delay_for_tests(Duration::from_millis(500));
    std::fs::write(root.join("A.java"), "class A { int newValue; }").unwrap();
    workspace.rebuild();
    assert!(workspace.is_building(), "background build did not start");

    let started = Instant::now();
    let fresh = workspace.workspace_symbols("newValue");
    let elapsed = started.elapsed();
    assert!(
        workspace.is_building(),
        "query blocked until the build finished"
    );
    assert!(
        elapsed < Duration::from_millis(500),
        "query waited for the build: {elapsed:?}"
    );
    assert!(
        fresh.is_empty(),
        "mid-build query saw the new snapshot: {fresh:?}"
    );
    let old = workspace.workspace_symbols("oldValue");
    assert!(
        old.iter().any(|symbol| symbol.name == "oldValue"),
        "previous snapshot lost: {old:?}"
    );

    workspace.wait_for_index();
    assert!(!workspace.is_building());
    assert!(workspace
        .workspace_symbols("newValue")
        .iter()
        .any(|symbol| symbol.name == "newValue"));
}

#[test]
fn queries_fall_back_to_file_analysis_without_a_snapshot() {
    let root = fixture("no-snapshot");
    let path = root.join("A.java");
    std::fs::write(&path, "class A { int value; void run() {} }").unwrap();
    let output = root.join("out");
    let mut workspace = Workspace::new(config(&root, &output));
    workspace.wait_for_index();

    // Drop the snapshot and keep the rebuild in flight.
    workspace.set_build_delay_for_tests(Duration::from_millis(500));
    workspace.clear_index_for_tests();
    assert!(workspace.index_snapshot().files.is_empty());

    let location = workspace
        .definition(&path, Position::new(0, 14))
        .expect("FileAnalysis fallback");
    assert!(
        location.file.ends_with("A.java"),
        "unexpected definition: {location:?}"
    );
    assert!(workspace.is_building(), "no build in flight");
    assert!(
        workspace.workspace_symbols("A").is_empty(),
        "index answered without a snapshot"
    );

    workspace.wait_for_index();
    assert!(!workspace.is_building());
    assert!(workspace
        .workspace_symbols("A")
        .iter()
        .any(|symbol| symbol.name == "A"));
}
