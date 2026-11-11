//! X3: member-name positions in selection ranges.
//!
//! Complements `scripts/lsp/probe_x3_member_names.py`. The parser records the
//! name token of `FieldAccess`/`VariableDecl` in `CompilationUnit.name_positions`
//! and `selection_range` consults that AST table first; these tests pin the
//! observable result for `a.b`, `a.b.c`, `this.x` and `super.x`.

mod common;
use common::platform_dir;

use std::path::PathBuf;

use javac_analysis_api::{Position, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::Workspace;

const TEXT: &str = r#"package com.example;

class Base {
    int x;
    int y;
}

class B {
    int b;
    C c;
}

class C {
}

class A extends Base {
    B abc;

    void f() {
        int first = abc.b + abc.c.b;
        int second = this.x;
        int third = super.y;
    }
}
"#;

fn workspace_with(name: &str) -> (Workspace, PathBuf) {
    platform_dir();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("x3-member-name-spec")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("A.java");
    std::fs::write(&path, TEXT).unwrap();
    let workspace = Workspace::new(AnalysisConfig {
        class_path: Vec::new(),
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: Vec::new(),
    });
    (workspace, path)
}

fn assert_member(workspace: &mut Workspace, path: &PathBuf, line: u32, char: u32, selected: &str) {
    let ranges = workspace.selection_ranges(path, &[Position::new(line, char)]);
    let range = ranges
        .first()
        .map(|selection| selection.range)
        .unwrap_or_else(|| panic!("no selection range at {line}:{char}"));
    assert_eq!(
        (range.start.line, range.start.column),
        (line, char),
        "selection start for {selected}"
    );
    assert_eq!(
        range.end.column - range.start.column,
        selected.encode_utf16().count() as u32,
        "selection width for {selected} at {line}:{char}"
    );
}

#[test]
fn member_names_select_the_name_token_not_a_prefix_occurrence() {
    let (mut workspace, path) = workspace_with("members");
    // `int first = abc.b + abc.c.b;` is line 19 (0-based).
    assert_member(&mut workspace, &path, 19, 24, "b");
    assert_member(&mut workspace, &path, 19, 34, "b");
    // `int second = this.x;`
    assert_member(&mut workspace, &path, 20, 26, "x");
    // `int third = super.y;`
    assert_member(&mut workspace, &path, 21, 26, "y");
}
