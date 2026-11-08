//! C: LSP 3.17 protocol shape at the library boundary.
//!
//! Complements `scripts/lsp/probe_c_protocol.py` (the real LSP transcript):
//! empty results, document-symbol ranges and the constructor/keyword behaviour
//! that made `class` jump to `Object`.

use std::path::PathBuf;

use javac_analysis_api::{Position, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::Workspace;

const TEXT: &str = r#"package com.example.app;

import java.util.List;

public class App {
    Helper helper;

    List<String> names;

    void f() {
        helper.missingField = 1;
        new Second();
    }
}

class Second {
    int value;
}
"#;

const EMPTY: &str = "package com.example.app;\n\npublic class Empty {\n}\n";

fn platform_dir() {
    if std::env::var_os("RAVAC_PLATFORM_DIR").is_none() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("debug");
        unsafe {
            std::env::set_var("RAVAC_PLATFORM_DIR", &root);
        }
    }
}

fn workspace_with(name: &str) -> (Workspace, PathBuf, PathBuf) {
    platform_dir();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("c-protocol-spec")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("App.java");
    std::fs::write(&path, TEXT).unwrap();
    let empty = root.join("Empty.java");
    std::fs::write(&empty, EMPTY).unwrap();
    let workspace = Workspace::new(AnalysisConfig {
        class_path: Vec::new(),
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: Vec::new(),
    });
    (workspace, path, empty)
}

fn at(line: u32, character: u32) -> Position {
    Position::new(line, character)
}

#[test]
fn definition_on_the_class_keyword_is_empty() {
    let (mut workspace, _path, empty) = workspace_with("class-keyword");
    // `public class Empty {`: (2, 7) is the `class` keyword.
    let location = workspace.definition(&empty, at(2, 7));
    assert!(location.is_none(), "{location:?}");
}

#[test]
fn definition_on_package_keyword_and_blank_line_is_empty() {
    let (mut workspace, path, _empty) = workspace_with("keywords");
    assert!(workspace.definition(&path, at(0, 0)).is_none());
    assert!(workspace.definition(&path, at(1, 0)).is_none());
    assert!(workspace.hover(&path, at(1, 0)).is_none());
}

#[test]
fn document_symbols_have_non_empty_names_and_ranges() {
    let (mut workspace, path, _empty) = workspace_with("symbols");
    let symbols = workspace.symbols(&path);
    assert!(symbols.len() >= 2, "{symbols:?}");
    // The LSP layer turns these into document symbols; the probe checks the
    // JSON shape (selectionRange within range, legal kind numbers).
    for symbol in &symbols {
        assert!(!symbol.name.is_empty(), "{symbol:?}");
    }
}

#[test]
fn hover_markdown_fences_are_balanced() {
    let (mut workspace, path, _empty) = workspace_with("hover");
    let value = workspace.hover(&path, at(4, 13)).expect("hover on App");
    assert_eq!(value.matches("```").count() % 2, 0, "{value:?}");
    assert!(
        !value.chars().any(|ch| ch.is_control() && ch != '\n' && ch != '\t'),
        "{value:?}"
    );
}

#[test]
fn definition_on_an_unresolved_member_is_empty() {
    let (mut workspace, path, _empty) = workspace_with("unresolved");
    // `helper.missingField`: the field does not exist on the unresolved type;
    // answering with the use site would be a wrong location.
    let location = workspace.definition(&path, at(10, 16));
    assert!(location.is_none(), "{location:?}");
}

#[test]
fn definition_on_a_constructor_call_still_finds_the_class() {
    let (mut workspace, path, _empty) = workspace_with("constructor");
    // `new Second()`: the cursor is on the type name, so the constructor
    // resolves to the class declaration in this file.
    let location = workspace.definition(&path, at(11, 13)).expect("Second");
    assert_eq!(
        location.file.file_name().and_then(|name| name.to_str()),
        Some("App.java")
    );
    assert_eq!(location.range.start.line, 15, "{location:?}");
}
