//! M4 on-type formatting (10 matrix items, section 28.4).

mod common;
use common::platform_dir;

use std::path::{Path, PathBuf};

use std::hash::{Hash, Hasher};

use javac_analysis_api::Position;

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::analysis::FileAnalysis;

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("on-type-formatting-spec")
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

fn edit_text(text: &str, line: u32, character: char) -> String {
    // A per-case directory: the tests run in parallel.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    line.hash(&mut hasher);
    character.hash(&mut hasher);
    let root = fixture(&format!("{:016x}", hasher.finish()));
    let path = root.join("Main.java");
    std::fs::write(&path, text).unwrap();
    let mut analysis = FileAnalysis::new(config(&root));
    let edits = analysis.on_type_formatting(&path, Position::new(line, 0), character);
    match edits.as_slice() {
        [] => String::new(),
        [edit] => edit.new_text.clone(),
        other => panic!("expected one edit, got {other:?}"),
    }
}

#[test]
fn item01_closing_brace_dedents() {
    let text = "class A {\n    void f() {\n            }\n}\n";
    assert_eq!(edit_text(text, 2, '}'), "    ");
}

#[test]
fn item02_semicolon_aligns_with_previous_line() {
    let text = "class A {\n    void f() {\n        int x = 1;\n            int y = 2;\n    }\n}\n";
    assert_eq!(edit_text(text, 3, ';'), "        ");
}

#[test]
fn item03_newline_continues_previous_indent() {
    let text = "class A {\n    void f() {\n        int x = 1;\n\n    }\n}\n";
    assert_eq!(edit_text(text, 3, '\n'), "        ");
}

#[test]
fn item04_closing_brace_in_string_does_not_trigger() {
    let text = "class A {\n    String s = \"}\";\n}\n";
    assert_eq!(edit_text(text, 1, '}'), "");
}

#[test]
fn item05_closing_brace_in_comment_does_not_trigger() {
    let text = "class A {\n    // }\n    int value;\n}\n";
    assert_eq!(edit_text(text, 1, '}'), "");
}

#[test]
fn item06_switch_case_indent() {
    let text = "class A {\n    void f() {\n        switch (x) {\n                case 1:\n                        f();\n        }\n    }\n}\n";
    assert_eq!(edit_text(text, 4, ';'), "                ");
}

#[test]
fn item07_lambda_block_closing_brace() {
    let text = "class A {\n    Runnable r = () -> {\n            int x;\n            };\n}\n";
    assert_eq!(edit_text(text, 3, '}'), "    ");
}

#[test]
fn item08_nested_array_initializer() {
    let text = "class A {\n    int[][] a = {\n            {1, 2},\n            };\n}\n";
    assert_eq!(edit_text(text, 3, '}'), "    ");
}

#[test]
fn item09_empty_line_keeps_indent() {
    let text = "class A {\n    void f() {\n        int x = 1;\n\n    }\n}\n";
    assert_eq!(edit_text(text, 3, '\n'), "        ");
}

#[test]
fn item10_crlf_keeps_indent_without_cr() {
    let text = "class A {\r\n    void f() {\r\n            }\r\n}\r\n";
    assert_eq!(edit_text(text, 2, '}'), "    ");
}

#[test]
fn item11_switch_case_body_is_indented() {
    // Plan 9.2: after `case X:` the body indents 4 past the case label.
    let text = "class A {\n    void f() {\n        switch (x) {\n            case 1:\n            break;\n        }\n    }\n}\n";
    assert_eq!(edit_text(text, 4, '\n'), "                ");
}

#[test]
fn item12_switch_default_body_is_indented() {
    let text = "class A {\n    void f() {\n        switch (x) {\n            default:\n            break;\n        }\n    }\n}\n";
    assert_eq!(edit_text(text, 4, '\n'), "                ");
}