//! M3 code actions: Add import / Remove unused import (12 matrix items).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use javac_analysis_api::{Position, TextEdit};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::analysis::FileAnalysis;

fn platform_dir() {
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
}

fn javac() -> Option<PathBuf> {
    let home = std::env::var_os("JAVA_HOME")?;
    let file = if cfg!(windows) { "javac.exe" } else { "javac" };
    let path = PathBuf::from(home).join("bin").join(file);
    path.exists().then_some(path)
}

fn jar_tool() -> Option<PathBuf> {
    let home = std::env::var_os("JAVA_HOME")?;
    let file = if cfg!(windows) { "jar.exe" } else { "jar" };
    let path = PathBuf::from(home).join("bin").join(file);
    path.exists().then_some(path)
}

fn test_root() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("code-actions-spec");
    let _ = std::fs::create_dir_all(&root);
    root
}

fn fixture(name: &str) -> PathBuf {
    let root = test_root().join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

/// One shared class path jar: `a.Foo`, `b.Foo`, `c.Bar`, `p.Foo`.
///
/// Sharing it keeps the import-candidate cache (keyed by class path) warm
/// across the twelve cases.
fn shared_classpath() -> Option<PathBuf> {
    static SHARED: OnceLock<Option<PathBuf>> = OnceLock::new();
    SHARED.get_or_init(build_shared).clone()
}

fn build_shared() -> Option<PathBuf> {
    let javac = javac()?;
    let jar = jar_tool()?;
    let root = test_root().join("shared");
    let jar_path = root.join("lib.jar");
    let _ = std::fs::remove_dir_all(&root);
    let src = root.join("src");
    for package in ["a", "b", "c"] {
        std::fs::create_dir_all(src.join(package)).unwrap();
    }
    std::fs::write(src.join("a/Foo.java"), "package a; public class Foo {}\n").unwrap();
    std::fs::write(src.join("b/Foo.java"), "package b; public class Foo {}\n").unwrap();
    std::fs::write(
        src.join("c/Bar.java"),
        "package c; public class Bar { public static int value; }\n",
    )
    .unwrap();
    let classes = root.join("classes");
    std::fs::create_dir_all(&classes).unwrap();
    let status = std::process::Command::new(&javac)
        .arg("-d")
        .arg(&classes)
        .arg(src.join("a/Foo.java"))
        .arg(src.join("b/Foo.java"))
        .arg(src.join("c/Bar.java"))
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    let status = std::process::Command::new(&jar)
        .arg("cf")
        .arg(&jar_path)
        .arg("-C")
        .arg(&classes)
        .arg(".")
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    Some(jar_path)
}

fn config(root: &Path, class_path: PathBuf) -> AnalysisConfig {
    platform_dir();
    AnalysisConfig {
        class_path: vec![class_path],
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: Vec::new(),
    }
}

fn source(root: &Path, name: &str, text: &str) -> PathBuf {
    let path = root.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

fn actions_at(
    analysis: &mut FileAnalysis,
    path: &Path,
    pos: Position,
) -> Vec<javac_analysis_api::CodeAction> {
    analysis.diagnostics(path, None);
    analysis
        .code_actions(path, pos)
        .into_iter()
        .filter(|action| !action.title.starts_with("Generate"))
        .collect()
}

fn add_actions(actions: &[javac_analysis_api::CodeAction]) -> Vec<&javac_analysis_api::CodeAction> {
    actions
        .iter()
        .filter(|action| !action.title.to_lowercase().contains("unused"))
        .collect()
}

fn insert_line(action: &javac_analysis_api::CodeAction) -> u32 {
    action
        .edits
        .first()
        .map(|edit| edit.range.start.line)
        .unwrap_or(u32::MAX)
}

/// Apply `edits` (all ranges relative to `text`) and return the new text.
fn apply(text: &str, edits: &[TextEdit]) -> String {
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    let mut edits: Vec<&TextEdit> = edits.iter().collect();
    edits.sort_by_key(|edit| (edit.range.start.line, edit.range.start.column));
    for edit in edits.iter().rev() {
        let start = edit.range.start;
        let end = edit.range.end;
        let start_line = start.line as usize;
        let end_line = end.line as usize;
        if start_line >= lines.len() {
            continue;
        }
        if start_line == end_line {
            let line = &lines[start_line];
            let mut chars: Vec<char> = line.chars().collect();
            let start_index = start.column as usize;
            let end_index = end.column as usize;
            let replacement: Vec<char> = edit.new_text.chars().collect();
            chars.splice(start_index..end_index.min(chars.len()), replacement);
            lines[start_line] = chars.into_iter().collect();
        } else {
            let mut chars: Vec<char> = lines[start_line].chars().collect();
            chars.truncate(start.column as usize);
            let mut tail: Vec<char> = lines
                .get(end_line)
                .map(|line| line.chars().collect())
                .unwrap_or_default();
            tail.drain(..(end.column as usize).min(tail.len()));
            let mut merged: String = chars.into_iter().collect();
            merged.push_str(&edit.new_text);
            merged.push_str(&tail.into_iter().collect::<String>());
            lines.splice(start_line..=end_line.min(lines.len() - 1), [merged]);
        }
    }
    lines.join("\n")
}

#[test]
fn item01_unimported_list_single_candidate() {
    let Some(jar) = shared_classpath() else { return };
    let root = fixture("item01");
    let path = source(
        &root,
        "Main.java",
        "package p;\nimport c.Bar;\n\nclass Main {\n  List<String> values;\n}\n",
    );
    let mut analysis = FileAnalysis::new(config(&root, jar));
    let actions = actions_at(&mut analysis, &path, Position::new(4, 10));
    let add = add_actions(&actions);
    // `List` also matches java.awt.List, so the matrix case is "contains the
    // java.util.List candidate" (each candidate is its own action).
    let list = add
        .iter()
        .find(|action| action.title.contains("java.util.List"))
        .expect("java.util.List action");
    assert!(
        list.edits[0].new_text.contains("import java.util.List;"),
        "{actions:?}"
    );
}

#[test]
fn item02_already_imported_zero() {
    let Some(jar) = shared_classpath() else { return };
    let root = fixture("item02");
    let path = source(
        &root,
        "Main.java",
        "package p;\nimport java.util.List;\n\nclass Main {\n  List<String> values;\n}\n",
    );
    let mut analysis = FileAnalysis::new(config(&root, jar));
    let actions = actions_at(&mut analysis, &path, Position::new(4, 10));
    assert!(actions.is_empty(), "{actions:?}");
}

#[test]
fn item03_two_same_named_candidates() {
    let Some(jar) = shared_classpath() else { return };
    let root = fixture("item03");
    let path = source(
        &root,
        "Main.java",
        "package p;\n\nclass Main {\n  Foo field;\n}\n",
    );
    let mut analysis = FileAnalysis::new(config(&root, jar));
    let actions = actions_at(&mut analysis, &path, Position::new(3, 7));
    let add = add_actions(&actions);
    assert_eq!(add.len(), 2, "{actions:?}");
    let titles: Vec<&str> = add.iter().map(|action| action.title.as_str()).collect();
    assert!(titles.iter().any(|title| title.contains("a.Foo")), "{titles:?}");
    assert!(titles.iter().any(|title| title.contains("b.Foo")), "{titles:?}");
}

#[test]
fn item04_wildcard_covers_class_zero() {
    let Some(jar) = shared_classpath() else { return };
    let root = fixture("item04");
    let path = source(
        &root,
        "Main.java",
        "package p;\nimport java.util.*;\n\nclass Main {\n  List<String> values;\n}\n",
    );
    let mut analysis = FileAnalysis::new(config(&root, jar));
    let actions = actions_at(&mut analysis, &path, Position::new(4, 10));
    assert!(actions.is_empty(), "{actions:?}");
}

#[test]
fn item05_class_in_current_package_zero() {
    let Some(_) = shared_classpath() else { return };
    let root = fixture("item05");
    let src = root.join("src/p");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("Foo.java"), "package p; public class Foo {}\n").unwrap();
    let classes = root.join("classes");
    std::fs::create_dir_all(&classes).unwrap();
    let status = std::process::Command::new(javac().unwrap())
        .arg("-d")
        .arg(&classes)
        .arg(src.join("Foo.java"))
        .status()
        .ok();
    assert!(status.map(|status| status.success()).unwrap_or(false));
    let path = source(
        &root,
        "Main.java",
        "package p;\n\nclass Main {\n  Foo field;\n}\n",
    );
    let mut analysis = FileAnalysis::new(config(&root, classes));
    let actions = actions_at(&mut analysis, &path, Position::new(3, 7));
    assert!(actions.is_empty(), "{actions:?}");
}

#[test]
fn item06_invalid_symbol_zero() {
    let Some(jar) = shared_classpath() else { return };
    let root = fixture("item06");
    let path = source(
        &root,
        "Main.java",
        "package p;\n\nclass Main {\n  NotAType value;\n}\n",
    );
    let mut analysis = FileAnalysis::new(config(&root, jar));
    let actions = actions_at(&mut analysis, &path, Position::new(3, 10));
    assert!(actions.is_empty(), "{actions:?}");
}

#[test]
fn item07_no_package_inserts_first_line() {
    let Some(jar) = shared_classpath() else { return };
    let root = fixture("item07");
    let path = source(&root, "Main.java", "class Main {\n  List<String> values;\n}\n");
    let mut analysis = FileAnalysis::new(config(&root, jar));
    let actions = actions_at(&mut analysis, &path, Position::new(1, 10));
    let add = add_actions(&actions);
    let list = add
        .iter()
        .find(|action| action.title.contains("java.util.List"))
        .expect("java.util.List action");
    assert_eq!(insert_line(list), 0, "{actions:?}");
}

#[test]
fn item08_package_without_import_inserts_after_package() {
    let Some(jar) = shared_classpath() else { return };
    let root = fixture("item08");
    let path = source(
        &root,
        "Main.java",
        "package p;\n\nclass Main {\n  List<String> values;\n}\n",
    );
    let mut analysis = FileAnalysis::new(config(&root, jar));
    let actions = actions_at(&mut analysis, &path, Position::new(3, 10));
    let add = add_actions(&actions);
    let list = add
        .iter()
        .find(|action| action.title.contains("java.util.List"))
        .expect("java.util.List action");
    assert_eq!(insert_line(list), 1, "{actions:?}");
}

#[test]
fn item09_multiple_imports_inserts_after_last() {
    let Some(jar) = shared_classpath() else { return };
    let root = fixture("item09");
    let path = source(
        &root,
        "Main.java",
        "package p;\nimport java.util.Collection;\nimport c.Bar;\n\nclass Main {\n  List<String> values;\n}\n",
    );
    let mut analysis = FileAnalysis::new(config(&root, jar));
    let actions = actions_at(&mut analysis, &path, Position::new(5, 10));
    let add = add_actions(&actions);
    let list = add
        .iter()
        .find(|action| action.title.contains("java.util.List"))
        .expect("java.util.List action");
    assert_eq!(insert_line(list), 3, "{actions:?}");
}

#[test]
fn item10_crlf_keeps_crlf() {
    let Some(jar) = shared_classpath() else { return };
    let root = fixture("item10");
    let path = source(
        &root,
        "Main.java",
        "package p;\r\n\r\nclass Main {\r\n  List<String> values;\r\n}\r\n",
    );
    let mut analysis = FileAnalysis::new(config(&root, jar));
    let actions = actions_at(&mut analysis, &path, Position::new(3, 10));
    let add = add_actions(&actions);
    let list = add
        .iter()
        .find(|action| action.title.contains("java.util.List"))
        .expect("java.util.List action");
    assert!(list.edits[0].new_text.contains("\r\n"), "{actions:?}");
}

#[test]
fn item11_remove_single_unused_import_line() {
    let Some(jar) = shared_classpath() else { return };
    let root = fixture("item11");
    let text = "package p;\nimport java.util.List;\nimport c.Bar;\n\nclass Main {\n  int value = Bar.value;\n}\n";
    let path = source(&root, "Main.java", text);
    let mut analysis = FileAnalysis::new(config(&root, jar));
    analysis.diagnostics(&path, None);
    let actions = analysis.code_actions(&path, Position::new(1, 5));
    let remove: Vec<&javac_analysis_api::CodeAction> = actions
        .iter()
        .filter(|action| action.title.to_lowercase().contains("unused"))
        .collect();
    assert_eq!(remove.len(), 1, "{actions:?}");
    assert_eq!(remove[0].edits.len(), 1, "{actions:?}");
    let edit = &remove[0].edits[0];
    assert_eq!(edit.range.start.line, 1, "{edit:?}");
    assert_eq!(edit.range.end.line, 2, "{edit:?}");
    assert_eq!(edit.new_text, "", "{edit:?}");
    let updated = apply(text, &remove[0].edits);
    assert!(!updated.contains("import java.util.List;"), "{updated:?}");
    assert!(!updated.contains("\n\n\n"), "{updated:?}");
}

#[test]
fn item12_remove_all_unused_imports_descending() {
    let Some(jar) = shared_classpath() else { return };
    let root = fixture("item12");
    let text = "package p;\nimport java.util.List;\nimport java.util.Map;\nimport c.Bar;\n\nclass Main {\n  int value = Bar.value;\n}\n";
    let path = source(&root, "Main.java", text);
    let mut analysis = FileAnalysis::new(config(&root, jar));
    analysis.diagnostics(&path, None);
    let actions = analysis.code_actions(&path, Position::new(5, 4));
    let all: Vec<&javac_analysis_api::CodeAction> = actions
        .iter()
        .filter(|action| {
            let title = action.title.to_lowercase();
            title.contains("all") && title.contains("unused")
        })
        .collect();
    assert_eq!(all.len(), 1, "{actions:?}");
    let edits = &all[0].edits;
    assert_eq!(edits.len(), 2, "{edits:?}");
    assert!(
        edits[0].range.start.line > edits[1].range.start.line,
        "{edits:?}"
    );
    let updated = apply(text, edits);
    assert!(!updated.contains("import java.util.List;"), "{updated:?}");
    assert!(!updated.contains("import java.util.Map;"), "{updated:?}");
    assert!(updated.contains("import c.Bar;"), "{updated:?}");
}

#[test]
fn item13_non_public_platform_class_is_not_a_candidate() {
    let root = fixture("item13");
    let path = source(&root, "Main.java", "class Main { ImmutableCollections x; }\n");
    let mut analysis = FileAnalysis::new(config(&root, PathBuf::new()));
    let actions = actions_at(&mut analysis, &path, Position::new(0, 29));
    // java.util.ImmutableCollections is package-private in the JDK; the
    // jimage ACC_PUBLIC read must keep it out of the candidates.
    assert!(
        !actions
            .iter()
            .any(|action| action.title.contains("ImmutableCollections")),
        "{actions:?}"
    );
}