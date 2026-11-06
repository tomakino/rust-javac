//! Completion through a real project index (non-empty source_path).

use std::path::{Path, PathBuf};

use javac_analysis_api::{AnalysisHost, Position};
use javac_ide_core::analysis::{AnalysisConfig, AnalysisSession};

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
        .join("completion-project-spec")
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

fn labels_at(session: &mut AnalysisSession, name: &str, source: &str) -> Vec<String> {
    let cursor = source.find('|').expect("cursor marker");
    let text = source.replacen('|', "", 1);
    let before = &text[..cursor];
    let line = before.matches('\n').count() as u32;
    let column = before
        .rsplit('\n')
        .next()
        .unwrap()
        .chars()
        .map(|ch| ch.len_utf16() as u32)
        .sum();
    let root = session.config().source_path[0].clone();
    let path = root.join(format!("{name}.java"));
    std::fs::write(&path, &text).unwrap();
    AnalysisHost::analyze(session, &path, Some(&text));
    session
        .completion(&path, Position::new(line, column))
        .into_iter()
        .map(|item| item.label)
        .collect()
}

#[test]
fn project_index_supplies_class_candidates() {
    let root = fixture("classes");
    std::fs::write(root.join("ProjectThing.java"), "class ProjectThing { ProjectThing() {} }").unwrap();
    let output = root.join("out");
    let mut session = AnalysisSession::new(config(&root, &output));
    let labels = labels_at(
        &mut session,
        "A",
        "class A { void f() { new Proj| } }",
    );
    assert!(
        labels.iter().any(|label| label.starts_with("ProjectThing")),
        "project class missing from completion: {labels:?}"
    );
}
