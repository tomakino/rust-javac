//! F: index scale performance at the library boundary.
//!
//! Complements `scripts/lsp/probe_f_performance.py`. The library path isolates
//! the index build itself (no LSP transport / artifact builder): generate N
//! classes in batches of 128, wait for the index and check cross-file
//! resolution + a mixed-request round.

mod common;
use common::platform_dir;

use std::path::PathBuf;
use std::time::Instant;

use javac_analysis_api::{Position, WorkspaceService};

use javac_ide_core::project::{self, LoadContext};
use javac_ide_core::workspace::Workspace;

fn gen_project(name: &str, count: usize, chained: bool) -> (PathBuf, Vec<PathBuf>) {
    platform_dir();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("f-performance-spec")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut files = Vec::new();
    for index in 0..count {
        let package = format!("p{}", index / 100);
        let class_name = format!("C{index}");
        let mut body = vec![
            format!("package com.example.{package};"),
            String::new(),
        ];
        if chained && index > 0 {
            let previous_package = format!("p{}", (index - 1) / 100);
            if previous_package != package {
                body.push(format!(
                    "import com.example.{previous_package}.C{};",
                    index - 1
                ));
                body.push(String::new());
            }
        }
        body.push(format!("public class {class_name} {{"));
        body.push(format!("    int value{index};"));
        body.push(format!("    String name{index};"));
        body.push(String::new());
        body.push(format!("    int compute{index}(int a) {{"));
        body.push(format!("        return a + value{index};"));
        body.push("    }".to_string());
        body.push(String::new());
        body.push(format!("    int use{index}() {{"));
        if chained && index > 0 {
            body.push(format!("        C{} other = new C{}();", index - 1, index - 1));
            body.push(format!(
                "        return other.compute{}(value{index});",
                index - 1
            ));
        } else {
            body.push(format!("        return value{index};"));
        }
        body.push("    }".to_string());
        body.push("}".to_string());
        body.push(String::new());
        let rel = format!("src/main/java/com/example/{package}/{class_name}.java");
        let path = root.join(&rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, body.join("\n")).unwrap();
        files.push(path);
    }
    (root, files)
}

fn load(root: &PathBuf) -> Workspace {
    let providers: Vec<Box<dyn javac_project_api::BuildSystemProvider>> =
        vec![Box::new(project::DirScanProvider)];
    let model = project::load_with(&providers, root, &LoadContext::default());
    let output = root.join("out");
    let mut workspace = Workspace::new_from_project(&model, output, None);
    workspace.wait_for_index();
    workspace
}

#[test]
fn cold_index_of_82_files_is_ready_and_resolves_cross_file() {
    let (root, files) = gen_project("cold-82", 82, true);
    let started = Instant::now();
    let mut workspace = load(&root);
    let elapsed = started.elapsed();
    let stats = workspace.index_stats();
    println!("82 files: index ready in {elapsed:?} ({stats:?})");
    assert!(stats.generation > 0, "{stats:?}");
    assert!(stats.files >= 80, "{stats:?}");
    // The chained project resolves C1's C0 reference to C0.java.
    let target = &files[1];
    let location = workspace.definition(target, Position::new(11, 8));
    assert!(
        location
            .as_ref()
            .is_some_and(|location| location.file.ends_with("C0.java")),
        "cross-file definition: {location:?}"
    );
    // Elapsed is recorded, not asserted tightly (debug builds are slow); the
    // LSP probe records the real number. Guard against pathological blowups.
    assert!(elapsed.as_secs() < 60, "index too slow: {elapsed:?}");
}

#[test]
fn poll_index_publishes_a_finished_build() {
    let (root, _files) = gen_project("poll", 10, true);
    let providers: Vec<Box<dyn javac_project_api::BuildSystemProvider>> =
        vec![Box::new(project::DirScanProvider)];
    let model = project::load_with(&providers, &root, &LoadContext::default());
    let mut workspace = Workspace::new_from_project(&model, root.join("out"), None);
    let started = Instant::now();
    let mut stats = workspace.index_stats();
    while stats.generation == 0 && started.elapsed() < std::time::Duration::from_secs(10) {
        std::thread::sleep(std::time::Duration::from_millis(5));
        workspace.poll_index();
        stats = workspace.index_stats();
    }
    assert!(stats.generation > 0, "poll_index did not publish: {stats:?}");
}

#[test]
fn cold_index_of_1000_files_is_ready() {
    let (root, files) = gen_project("cold-1000", 1000, false);
    let started = Instant::now();
    let mut workspace = load(&root);
    let elapsed = started.elapsed();
    let stats = workspace.index_stats();
    println!("1000 files: index ready in {elapsed:?} ({stats:?})");
    assert!(stats.generation > 0, "{stats:?}");
    assert!(stats.files >= 990, "{stats:?}");
    // One class per file resolves within its own file.
    let target = &files[500];
    let location = workspace.definition(target, Position::new(2, 13));
    assert!(location.is_some(), "definition in generated class: {location:?}");
    assert!(elapsed.as_secs() < 120, "index too slow: {elapsed:?}");
}

#[test]
fn mixed_request_rounds_are_stable() {
    let (root, files) = gen_project("rounds", 20, true);
    let mut workspace = load(&root);
    let first = &files[0];
    let mut rounds = Vec::new();
    for _ in 0..20 {
        let started = Instant::now();
        let _ = workspace.hover(first, Position::new(3, 8));
        let _ = workspace.definition(first, Position::new(14, 22));
        let _ = workspace.symbols(first);
        let _ = workspace.semantic_tokens(first);
        let _ = workspace.inlay_hints(first);
        rounds.push(started.elapsed());
    }
    let average: std::time::Duration =
        rounds.iter().sum::<std::time::Duration>() / rounds.len() as u32;
    println!("20 mixed rounds: avg {average:?} rounds={rounds:?}");
    // No blow-up between the first and the last round.
    assert!(rounds.last().unwrap() < &(rounds[0] * 20 + std::time::Duration::from_millis(500)), "{rounds:?}");
}
