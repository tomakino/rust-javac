//! Symbol-level references and rename tests.

use std::path::PathBuf;

use javac_analysis_api::{AnalysisHost, Position};

use javac_ide_core::analysis::{AnalysisConfig, AnalysisSession};

fn session() -> AnalysisSession {
    if std::env::var_os("RAVAC_PLATFORM_DIR").is_none() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("debug");
        unsafe {
            std::env::set_var("RAVAC_PLATFORM_DIR", root);
        }
    }
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("navigation-spec");
    AnalysisSession::new(AnalysisConfig {
        class_path: Vec::new(),
        output_dir: output,
        source_level: "17".to_string(),
        source_path: Vec::new(),
    })
}

const SOURCE: &str = r#"public class Refs {
    private int value;

    void f(int value) {
        this.value = value;
        int local = value;
        // value in a comment
        String s = "value";
        local = this.value;
    }
}
"#;

fn analyzed() -> (AnalysisSession, PathBuf) {
    let mut session = session();
    let path = PathBuf::from("target/tmp/navigation-spec/Refs.java");
    std::fs::create_dir_all("target/tmp/navigation-spec").unwrap();
    std::fs::write(&path, SOURCE).unwrap();
    AnalysisHost::analyze(&mut session, &path, Some(SOURCE));
    (session, path)
}

#[test]
fn references_only_match_the_field_symbol() {
    let (mut session, path) = analyzed();
    let references = AnalysisHost::references(&mut session, &path, Position::new(1, 16));
    let points: Vec<(u32, u32)> = references
        .iter()
        .map(|location| (location.range.start.line, location.range.start.column))
        .collect();
    assert_eq!(points, vec![(1, 16), (4, 13), (8, 21)], "references: {references:?}");
}

#[test]
fn prepare_rename_reports_the_field_range() {
    let (mut session, path) = analyzed();
    let range = AnalysisHost::prepare_rename(&mut session, &path, Position::new(1, 16)).expect("rename range");
    assert_eq!(range.start, Position::new(1, 16));
    assert_eq!(range.end, Position::new(1, 21));
}

#[test]
fn rename_does_not_touch_comments_strings_or_shadowed_names() {
    let (mut session, path) = analyzed();
    let edits = AnalysisHost::rename(&mut session, &path, Position::new(1, 16), "renamed");
    assert_eq!(edits.len(), 3, "edits: {edits:?}");
    assert!(edits.iter().all(|edit| edit.new_text == "renamed"));
    let lines: Vec<u32> = edits.iter().map(|edit| edit.range.start.line).collect();
    assert_eq!(lines, vec![1, 4, 8], "edits: {edits:?}");
}

#[test]
fn references_cross_files_with_project_source_path() {
    let base = PathBuf::from("target/tmp/navigation-cross-spec");
    std::fs::create_dir_all(&base).unwrap();
    let a_path = base.join("A.java");
    let b_path = base.join("B.java");
    let a_text = "public class A {\n    void f(B b) {\n        b.value = 1;\n        b.value++;\n    }\n}\n";
    let b_text = "public class B {\n    int value;\n}\n";
    std::fs::write(&a_path, a_text).unwrap();
    std::fs::write(&b_path, b_text).unwrap();

    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("navigation-cross-spec-out");
    if std::env::var_os("RAVAC_PLATFORM_DIR").is_none() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("debug");
        unsafe {
            std::env::set_var("RAVAC_PLATFORM_DIR", root);
        }
    }
    let mut session = AnalysisSession::new(AnalysisConfig {
        class_path: Vec::new(),
        output_dir: output,
        source_level: "17".to_string(),
        source_path: vec![base.clone()],
    });
    AnalysisHost::analyze(&mut session, &a_path, Some(a_text));
    let references = AnalysisHost::references(&mut session, &a_path, Position::new(2, 10));
    assert_eq!(references.len(), 3, "references: {references:?}");
    let files: Vec<String> = references
        .iter()
        .map(|location| location.file.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(files.iter().any(|file| file == "A.java"));
    assert!(files.iter().any(|file| file == "B.java"));

    let edits = AnalysisHost::rename_workspace(&mut session, &a_path, Position::new(2, 10), "renamed");
    assert_eq!(edits.len(), 3, "edits: {edits:?}");
    let a_edits = edits.iter().filter(|edit| edit.file.ends_with("A.java")).count();
    let b_edits = edits.iter().filter(|edit| edit.file.ends_with("B.java")).count();
    assert_eq!((a_edits, b_edits), (2, 1), "edits: {edits:?}");
}