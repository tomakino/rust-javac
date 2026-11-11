//! E: exception degradation at the library boundary.
//!
//! Complements `scripts/lsp/probe_e_degradation.py` (the real LSP transcript):
//! syntax errors, unresolved symbols, empty/comment-only files, BOM/CRLF
//! positions and non-UTF-8 bytes must degrade without panicking.

mod common;
use common::platform_dir;

use std::path::PathBuf;

use javac_analysis_api::{Position, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::Workspace;

fn workspace_with_bytes(name: &str, bytes: &[u8]) -> (Workspace, PathBuf) {
    platform_dir();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("e-degradation-spec")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("A.java");
    std::fs::write(&path, bytes).unwrap();
    let workspace = Workspace::new(AnalysisConfig {
        class_path: Vec::new(),
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: Vec::new(),
    });
    (workspace, path)
}

fn workspace_with(name: &str, text: &str) -> (Workspace, PathBuf) {
    workspace_with_bytes(name, text.as_bytes())
}

fn at(line: u32, character: u32) -> Position {
    Position::new(line, character)
}

#[test]
fn syntax_errors_keep_partial_symbols_and_do_not_panic() {
    let (mut workspace, path) = workspace_with(
        "syntax",
        "class A {\n    int x void f() {\n}\n",
    );
    let symbols = workspace.symbols(&path);
    assert!(
        symbols.iter().any(|symbol| symbol.name == "A"),
        "{symbols:?}"
    );
    // Hover/definition on broken members answer without panicking (empty is ok).
    let _ = workspace.hover(&path, at(1, 4));
    let _ = workspace.definition(&path, at(1, 4));
}

#[test]
fn unresolved_symbols_have_no_definition() {
    let (mut workspace, path) = workspace_with(
        "unresolved",
        "class A {\n    UnknownType x;\n\n    void f() {\n        unknownMethod();\n    }\n}\n",
    );
    assert!(workspace.definition(&path, at(1, 4)).is_none());
    assert!(workspace.definition(&path, at(4, 8)).is_none());
}

#[test]
fn empty_comment_and_package_only_files_have_no_symbols() {
    for (name, text) in [
        ("empty", ""),
        ("comment", "// just a comment\n/* block */\n"),
        ("package", "package com.example;\n"),
    ] {
        let (mut workspace, path) = workspace_with(name, text);
        assert!(workspace.symbols(&path).is_empty(), "{name}");
    }
}

#[test]
fn half_parsed_file_does_not_panic() {
    let (mut workspace, path) = workspace_with("half", "class A {\n    void f() {\n");
    let _ = workspace.symbols(&path);
    let _ = workspace.hover(&path, at(1, 4));
}

#[test]
fn bom_keeps_the_same_positions_as_plain_text() {
    let body = "class A {\n    int value;\n\n    int get() {\n        return value;\n    }\n}\n";
    let (mut plain, plain_path) = workspace_with("bom-plain", body);
    let (mut bom, bom_path) = workspace_with("bom", &format!("\u{feff}{body}"));

    let plain_location = plain.definition(&plain_path, at(4, 15)).expect("plain");
    let bom_location = bom.definition(&bom_path, at(4, 15)).expect("bom");
    assert_eq!(plain_location.range.start, bom_location.range.start);
    assert_eq!(bom_location.range.start.line, 1);
    assert_eq!(bom_location.range.start.column, 8);
}

#[test]
fn crlf_and_mixed_endings_keep_positions() {
    let crlf = "class A {\r\n    int value;\r\n\r\n    int get() {\r\n        return value;\r\n    }\r\n}\r\n";
    let (mut workspace, path) = workspace_with("crlf", crlf);
    let location = workspace.definition(&path, at(4, 15)).expect("crlf");
    assert_eq!((location.range.start.line, location.range.start.column), (1, 8));

    let mixed = "class A {\n    int a;\r\n    int b;\n\n    void f() {\r\n        int c = a + b;\n    }\r\n}\n";
    let (mut workspace, path) = workspace_with("mixed", mixed);
    let location = workspace.definition(&path, at(5, 16)).expect("mixed");
    assert_eq!((location.range.start.line, location.range.start.column), (1, 8));
}

#[test]
fn non_utf8_bytes_degrade_to_no_symbols() {
    // GBK bytes for `// 中文` are not valid UTF-8; the file must not panic.
    let (mut workspace, path) = workspace_with_bytes(
        "gbk",
        b"class A {\n    // \xd6\xd0\xce\xc4\n    int value;\n}\n",
    );
    let _ = workspace.symbols(&path);
    let _ = workspace.hover(&path, at(2, 8));
    let _ = workspace.definition(&path, at(2, 8));
}
