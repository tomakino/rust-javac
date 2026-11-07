//! Selection-range chain tests (plan chapter 7, matrix 28.2).

use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, Range, SelectionRange, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::Workspace;

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

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("selection-range-spec")
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
        source_path: vec![root.to_path_buf()],
    }
}

fn workspace_with(root: &Path, source: &str) -> (Workspace, PathBuf) {
    let path = root.join("A.java");
    std::fs::write(&path, source).unwrap();
    let mut workspace = Workspace::new(config(root));
    workspace.wait_for_index();
    (workspace, path)
}

fn position_at(text: &str, byte: usize) -> Position {
    let prefix = &text[..byte];
    let line = prefix.matches('\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    let column = text[line_start..byte]
        .chars()
        .map(|ch| ch.len_utf16() as u32)
        .sum();
    Position::new(line, column)
}

fn position_of(text: &str, needle: &str) -> Position {
    position_at(text, text.find(needle).expect("needle"))
}

fn range_of(text: &str, needle: &str) -> (Position, Position) {
    let byte = text.find(needle).expect("needle");
    (position_at(text, byte), position_at(text, byte + needle.len()))
}

fn chain(range: &SelectionRange) -> Vec<Range> {
    let mut ranges = Vec::new();
    let mut current = Some(range);
    while let Some(node) = current {
        ranges.push(node.range);
        current = node.parent.as_deref();
    }
    ranges
}

fn ranges(workspace: &mut Workspace, path: &Path, text: &str, position: Position) -> Vec<Range> {
    let result = workspace.selection_ranges(path, &[position]);
    assert_eq!(result.len(), 1, "one chain per position");
    chain(&result[0])
}

fn assert_start(chain: &[Range], position: Position) {
    assert!(
        chain.iter().any(|range| range.start == position),
        "missing start {position:?} in {chain:?}"
    );
}

fn assert_contains(chain: &[Range], needle: &(Position, Position)) {
    assert!(
        chain
            .iter()
            .any(|range| range.start == needle.0 && range.end == needle.1),
        "missing {needle:?} in {chain:?}"
    );
}

fn assert_nested(chain: &[Range]) {
    for pair in chain.windows(2) {
        let outer = &pair[1];
        let inner = &pair[0];
        assert!(
            (outer.start.line, outer.start.column) <= (inner.start.line, inner.start.column)
                && (outer.end.line, outer.end.column) >= (inner.end.line, inner.end.column),
            "outer {outer:?} must contain inner {inner:?}"
        );
    }
}

#[test]
fn identifier_inside() {
    let source = "class A { void f() { int value = 1; } }\n";
    let (mut workspace, path) = workspace_with(&fixture("identifier"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "value"));
    assert_nested(&chain);
    assert_eq!(chain.first().unwrap().start, range_of(source, "value").0, "{chain:?}");
    assert_contains(&chain, &range_of(source, "value"));
}

#[test]
fn identifier_at_end() {
    let source = "class A { void f() { int value = 1; } }\n";
    let (mut workspace, path) = workspace_with(&fixture("identifier-end"), source);
    let start = range_of(source, "value").0;
    let end = range_of(source, "value").1;
    let chain = ranges(&mut workspace, &path, source, Position::new(end.line, end.column));
    assert_nested(&chain);
    assert!(chain
        .iter()
        .any(|range| range.start == start && range.end == end), "{chain:?}");
}

#[test]
fn method_call_argument() {
    let source = "class A { void f() { call(a, b); } }\n";
    let (mut workspace, path) = workspace_with(&fixture("call-argument"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "a,"));
    assert_nested(&chain);
    assert_start(&chain, range_of(source, "a,").0);
    assert_contains(&chain, &range_of(source, "call(a, b)"));
}

#[test]
fn chained_calls() {
    let source = "class A { void f() { a.b().c(); } }\n";
    let (mut workspace, path) = workspace_with(&fixture("chained-calls"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "c()"));
    assert_nested(&chain);
    assert_start(&chain, range_of(source, "c()").0);
    assert_start(&chain, range_of(source, "a.b().c()").0);
}

#[test]
fn binary_expression() {
    let source = "class A { void f() { int x = a + b * c; } }\n";
    let (mut workspace, path) = workspace_with(&fixture("binary"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "b * c"));
    assert_nested(&chain);
    assert_contains(&chain, &range_of(source, "b * c"));
    assert_contains(&chain, &range_of(source, "a + b * c"));
}

#[test]
fn unary_expression() {
    let source = "class A { void f() { boolean x = !flag; } }\n";
    let (mut workspace, path) = workspace_with(&fixture("unary"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "flag"));
    assert_nested(&chain);
    assert_contains(&chain, &range_of(source, "flag"));
    assert_contains(&chain, &range_of(source, "!flag"));
}

#[test]
fn cast_expression() {
    let source = "class A { void f() { Object o = (Foo) value; } }\n";
    let (mut workspace, path) = workspace_with(&fixture("cast"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "value"));
    assert_nested(&chain);
    assert_contains(&chain, &range_of(source, "value"));
    assert_contains(&chain, &range_of(source, "(Foo) value"));
}

#[test]
fn new_expression_argument() {
    let source = "class A { void f() { new Foo(a); } }\n";
    let (mut workspace, path) = workspace_with(&fixture("new-arg"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "a)"));
    assert_nested(&chain);
    assert_start(&chain, range_of(source, "a)").0);
    assert_contains(&chain, &range_of(source, "new Foo(a)"));
}

#[test]
fn array_initializer() {
    let source = "class A { void f() { int[] a = new int[]{1, 2}; } }\n";
    let (mut workspace, path) = workspace_with(&fixture("array-init"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "1,"));
    assert_nested(&chain);
    assert_start(&chain, range_of(source, "1,").0);
    assert_contains(&chain, &range_of(source, "new int[]{1, 2}"));
}

#[test]
fn generic_type_arguments() {
    let source = "class A { Map<String, List<Integer>> values; }\n";
    let (mut workspace, path) = workspace_with(&fixture("generics"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "String"));
    assert_nested(&chain);
    assert_start(&chain, range_of(source, "Map<").0);
    assert_start(&chain, range_of(source, "String").0);
}

#[test]
fn lambda_body() {
    let source = "class A { void f() { list.forEach(x -> g(x)); } }\n";
    let (mut workspace, path) = workspace_with(&fixture("lambda"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "g(x)"));
    assert_nested(&chain);
    assert_contains(&chain, &range_of(source, "g(x)"));
    eprintln!("[T] {chain:?}");
    assert_contains(&chain, &range_of(source, "x -> g(x)"));
}

#[test]
fn anonymous_class_body() {
    let source = "class A { void f() { Runnable r = new Runnable() { public void run() {} }; } }\n";
    let (mut workspace, path) = workspace_with(&fixture("anonymous"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "run()"));
    assert_nested(&chain);
    assert!(chain.iter().any(|range| range.start.line == 0 && range.end.line == 0), "{chain:?}");
    assert_start(&chain, range_of(source, "run()").0);
}

#[test]
fn try_catch_parameter() {
    let source = "class A { void f() { try { g(); } catch (Exception e) { h(e); } } }\n";
    let (mut workspace, path) = workspace_with(&fixture("try-catch"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "e)"));
    assert_nested(&chain);
    assert_start(&chain, range_of(source, "e)").0);
    assert_start(&chain, range_of(source, "Exception e").0);
}

#[test]
fn switch_case_label() {
    let source = "class A { void f() { switch (x) { case 1: g(); break; } } }\n";
    let (mut workspace, path) = workspace_with(&fixture("switch-case"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "1:"));
    assert_nested(&chain);
    assert_start(&chain, range_of(source, "case 1").0);
}

#[test]
fn comment_falls_back_to_word_and_line() {
    let source = "class A {\n    // value note\n    void f() {}\n}\n";
    let (mut workspace, path) = workspace_with(&fixture("comment"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "value"));
    assert_eq!(chain.len(), 2, "{chain:?}");
    assert_nested(&chain);
    assert_contains(&chain, &range_of(source, "value"));
    assert!(chain[1].start.column <= chain[0].start.column, "{chain:?}");
}

#[test]
fn string_falls_back_to_word_and_line() {
    let source = "class A {\n    String s = \"value\";\n}\n";
    let (mut workspace, path) = workspace_with(&fixture("string"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "value"));
    assert_eq!(chain.len(), 2, "{chain:?}");
    assert_nested(&chain);
    assert_contains(&chain, &range_of(source, "value"));
}

#[test]
fn text_block_falls_back() {
    let source = "class A {\n    String s = \"\"\"\n        value\n        \"\"\";\n}\n";
    let (mut workspace, path) = workspace_with(&fixture("text-block"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "value"));
    assert!(chain.len() >= 2, "{chain:?}");
    assert_nested(&chain);
    assert_contains(&chain, &range_of(source, "value"));
}

#[test]
fn blank_line_returns_a_range() {
    let source = "class A {\n\n    void f() {}\n}\n";
    let (mut workspace, path) = workspace_with(&fixture("blank"), source);
    let chain = ranges(&mut workspace, &path, source, Position::new(1, 0));
    assert!(!chain.is_empty(), "{chain:?}");
    assert_nested(&chain);
}

#[test]
fn chinese_identifier_uses_utf16_columns() {
    let source = "class A { void f() { int 数值 = 1; } }\n";
    let (mut workspace, path) = workspace_with(&fixture("chinese"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "数值"));
    assert_nested(&chain);
    let needle = range_of(source, "数值");
    assert_contains(&chain, &needle);
    assert_eq!(needle.1.column - needle.0.column, 2, "UTF-16 length");
}

#[test]
fn nested_blocks() {
    let source = "class A { void f() { { int x = 1; } } }\n";
    let (mut workspace, path) = workspace_with(&fixture("nested-blocks"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "x = 1"));
    assert_nested(&chain);
    assert_start(&chain, range_of(source, "x = 1").0);
    assert_contains(&chain, &range_of(source, "{ int x = 1; }"));
}

#[test]
fn method_parameter() {
    let source = "class A { void f(int value) { } }\n";
    let (mut workspace, path) = workspace_with(&fixture("parameter"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "value"));
    assert_nested(&chain);
    assert_start(&chain, range_of(source, "value").0);
    assert_start(&chain, range_of(source, "int value").0);
}

#[test]
fn class_name_to_unit() {
    let source = "class A { void f() {} }\n";
    let (mut workspace, path) = workspace_with(&fixture("class-name"), source);
    let chain = ranges(&mut workspace, &path, source, position_of(source, "A"));
    assert_nested(&chain);
    assert_contains(&chain, &range_of(source, "A"));
    assert!(chain.len() >= 2, "{chain:?}");
}

#[test]
fn parameter_name_after_repeated_type_name() {
    // `String String`: a text fallback would latch onto the type name (the
    // first `String`); the AST name position is the second one.
    let source = "class A { void f(String String) { } }\n";
    let (mut workspace, path) = workspace_with(&fixture("parameter-shadow"), source);
    let name_byte = source.rfind("String").expect("parameter name");
    let chain = ranges(&mut workspace, &path, source, position_at(source, name_byte));
    assert_nested(&chain);
    assert_eq!(
        chain.first().expect("innermost").start,
        position_at(source, name_byte),
        "{chain:?}"
    );
}