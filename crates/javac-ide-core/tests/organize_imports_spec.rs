//! M5 organize imports (10 matrix items, section 13.4).

mod common;
use common::platform_dir;

use std::path::{Path, PathBuf};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::analysis::FileAnalysis;

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("organize-imports-spec")
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
        source_path: Vec::new(),
    }
}

fn new_text(text: &str, name: &str) -> String {
    let root = fixture(name);
    let path = root.join("Main.java");
    std::fs::write(&path, text).unwrap();
    let mut analysis = FileAnalysis::new(config(&root));
    let edits = analysis.organize_imports(&path);
    match edits.as_slice() {
        [] => String::new(),
        [edit] => edit.new_text.clone(),
        other => panic!("expected one edit, got {other:?}"),
    }
}

fn apply(text: &str, replacement: &str) -> String {
    let root = fixture("apply");
    let path = root.join("Main.java");
    std::fs::write(&path, text).unwrap();
    let mut analysis = FileAnalysis::new(config(&root));
    let edits = analysis.organize_imports(&path);
    let Some(edit) = edits.first() else {
        return text.to_string();
    };
    let lines: Vec<&str> = text.split('\n').collect();
    let start = edit.range.start.line as usize;
    let mut output: Vec<String> = lines[..start].iter().map(|line| line.to_string()).collect();
    let end_line = edit.range.end.line as usize;
    let mut tail: String = lines
        .get(end_line)
        .map(|line| line.chars().skip(edit.range.end.column as usize).collect())
        .unwrap_or_default();
    output.push(replacement.to_string());
    let mut result = output.join("\n");
    let _ = tail;
    let _ = &mut tail;
    let rest = lines.get(end_line + 1..).unwrap_or(&[]);
    for line in rest {
        result.push('\n');
        result.push_str(line);
    }
    result
}

#[test]
fn item01_sorts_imports() {
    let text = "package p;\nimport java.util.Map;\nimport java.util.List;\n\nclass Main {\n  List<String> a;\n  Map<String, String> b;\n}\n";
    let organized = new_text(text, "item01");
    assert!(organized.contains("import java.util.List;"), "{organized:?}");
    assert!(
        organized.find("java.util.List").unwrap() < organized.find("java.util.Map").unwrap(),
        "{organized:?}"
    );
}

#[test]
fn item02_dedupes_imports() {
    let text = "package p;\nimport java.util.List;\nimport java.util.List;\n\nclass Main {\n  List<String> a;\n}\n";
    let organized = new_text(text, "item02");
    assert_eq!(organized.matches("import java.util.List;").count(), 1, "{organized:?}");
}

#[test]
fn item03_removes_unused_import() {
    let text = "package p;\nimport java.util.List;\nimport java.util.Map;\n\nclass Main {}\n";
    let organized = new_text(text, "item03");
    assert!(!organized.contains("java.util."), "{organized:?}");
}

#[test]
fn item04_static_imports_last() {
    let text = "package p;\nimport static java.util.Arrays.asList;\nimport java.util.List;\n\nclass Main {\n  List<String> a = asList(\"x\");\n}\n";
    let organized = new_text(text, "item04");
    let list = organized.find("java.util.List").unwrap();
    let statics = organized.find("static java.util.Arrays.asList").unwrap();
    assert!(list < statics, "{organized:?}");
}

#[test]
fn item05_keeps_wildcard() {
    let text = "package p;\nimport java.util.*;\n\nclass Main {}\n";
    let organized = new_text(text, "item05");
    // No changes means no edit: the wildcard stays untouched.
    assert!(organized.is_empty() || organized.contains("import java.util.*;"), "{organized:?}");
}

#[test]
fn item06_keeps_trailing_comments() {
    let text = "package p;\nimport java.util.List; // lists\n\nclass Main {\n  List<String> a;\n}\n";
    let organized = new_text(text, "item06");
    assert!(organized.is_empty() || organized.contains("// lists"), "{organized:?}");
}

#[test]
fn item07_keeps_crlf() {
    let text = "package p;\r\nimport java.util.Map;\r\nimport java.util.List;\r\n\r\nclass Main {\r\n  List<String> a;\r\n  Map<String, String> b;\r\n}\r\n";
    let organized = new_text(text, "item07");
    assert!(organized.contains("\r\n"), "{organized:?}");
    assert!(!organized.contains("\n\n") || organized.contains("\r\n\r\n"), "{organized:?}");
}

#[test]
fn item08_no_imports_no_edit() {
    let text = "package p;\n\nclass Main {}\n";
    assert_eq!(new_text(text, "item08"), "");
}

#[test]
fn item09_package_only_no_edit() {
    let text = "package p;\n\nclass Main {}\n";
    assert_eq!(new_text(text, "item09"), "");
}

#[test]
fn item10_keeps_comment_between_imports() {
    let text = "package p;\nimport java.util.List;\n// keep me\nimport java.util.Map;\n\nclass Main {\n  List<String> a;\n  Map<String, String> b;\n}\n";
    let organized = new_text(text, "item10");
    assert!(organized.contains("keep me"), "{organized:?}");
}
