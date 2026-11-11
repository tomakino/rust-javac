//! Silent-wrong-result checks: values that are returned without an error but
//! would mislead an editor.

mod common;
use common::platform_path;

use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::Workspace;

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("silent-bugs-spec")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn config(root: &Path) -> AnalysisConfig {
    platform_path();
    AnalysisConfig {
        class_path: Vec::new(),
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: vec![root.to_path_buf()],
    }
}

fn workspace_with(name: &str, source: &str) -> (Workspace, PathBuf) {
    let root = fixture(name);
    let path = root.join("A.java");
    std::fs::write(&path, source).unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    (workspace, path)
}

#[test]
fn document_symbols_nest_members_under_their_class() {
    let source = "class A {\n    int f;\n    void m() {}\n    class B {\n        int g;\n    }\n}\n";
    let (mut workspace, path) = workspace_with("symbols", source);
    let symbols = workspace.document_symbols(&path);
    let a = symbols.iter().find(|symbol| symbol.name == "A").expect("A");
    let a_names: Vec<&str> = a.children.iter().map(|child| child.name.as_str()).collect();
    assert!(
        a_names.contains(&"f") && a_names.contains(&"m") && a_names.contains(&"B"),
        "A children: {a_names:?}"
    );
    let b = a
        .children
        .iter()
        .find(|symbol| symbol.name == "B")
        .expect("B");
    let b_names: Vec<&str> = b.children.iter().map(|child| child.name.as_str()).collect();
    assert_eq!(b_names, vec!["g"], "B children: {b_names:?}");
}

#[test]
fn signature_help_tracks_the_active_parameter() {
    let source = "class A {\n    void f(int a, String b) {}\n    void g() { f(1, \"x\"); }\n}\n";
    let (mut workspace, path) = workspace_with("signature", source);
    let line = 2;
    let first = source.lines().nth(line).unwrap().find('1').unwrap() as u32;
    let help = workspace
        .signature_help(&path, Position::new(line as u32, first + 1))
        .expect("signature help");
    assert_eq!(help.active_parameter, 0, "{help:?}");

    let second = source.lines().nth(line).unwrap().find("\"x\"").unwrap() as u32;
    let help = workspace
        .signature_help(&path, Position::new(line as u32, second + 1))
        .expect("signature help");
    assert_eq!(help.active_parameter, 1, "{help:?}");
}

#[test]
fn inlay_type_hint_follows_the_name() {
    let source = "class A {\n    void f() { var value = 1; }\n}\n";
    let (mut workspace, path) = workspace_with("inlay", source);
    let hints = workspace.inlay_hints(&path);
    let hint = hints
        .iter()
        .find(|hint| hint.label.contains("int"))
        .expect("type hint");
    let line = source.lines().nth(1).unwrap();
    let name_end = line.find("value").unwrap() as u32 + "value".len() as u32;
    assert_eq!(hint.position.line, 1, "{hint:?}");
    assert_eq!(hint.position.column, name_end, "{hint:?}");
}

#[test]
fn references_include_declaration_once_and_are_sorted() {
    let source = "class A {\n    int value;\n    void f() { value = 1; value++; }\n}\n";
    let (mut workspace, path) = workspace_with("references", source);
    let declaration = source.lines().nth(1).unwrap().find("value").unwrap() as u32;
    let references = workspace.references(&path, Position::new(1, declaration + 1));
    assert_eq!(references.len(), 3, "references: {references:?}");
    let mut sorted = references.clone();
    sorted.sort_by_key(|location| (location.range.start.line, location.range.start.column));
    assert_eq!(references, sorted, "references must be sorted");
    let mut unique = references.clone();
    unique.dedup();
    assert_eq!(unique.len(), references.len(), "references must be unique");
}

#[test]
fn prepare_rename_covers_the_name_only() {
    let source = "class A {\n    int value;\n}\n";
    let (mut workspace, path) = workspace_with("rename-range", source);
    let line = source.lines().nth(1).unwrap();
    let start = line.find("value").unwrap() as u32;
    let range = workspace
        .prepare_rename(&path, Position::new(1, start + 1))
        .expect("prepare rename");
    assert_eq!(range.start.line, 1, "{range:?}");
    assert_eq!(range.start.column, start, "{range:?}");
    assert_eq!(range.end.column, start + 5, "{range:?}");
}

#[test]
fn signature_help_picks_a_matching_overload() {
    let source = "class A {\n    void f(int a, int b) {}\n    void f(int a) {}\n    void g() { f(1); }\n}\n";
    let (mut workspace, path) = workspace_with("overload", source);
    let line = source.lines().nth(3).unwrap();
    let column = line.find('1').unwrap() as u32;
    let help = workspace
        .signature_help(&path, Position::new(3, column + 1))
        .expect("signature help");
    assert_eq!(help.signatures.len(), 2, "{help:?}");
    assert_eq!(
        help.signatures[help.active_signature as usize]
            .parameters
            .len(),
        1,
        "active signature must hold one argument: {help:?}"
    );
}

#[test]
fn signature_help_ignores_commas_in_nested_calls() {
    let source = "class A {\n    void g(int x, int y) {}\n    void h() { g(g(1, 2), 3); }\n}\n";
    let (mut workspace, path) = workspace_with("nested-call", source);
    let line = source.lines().nth(2).unwrap();
    let column = line.rfind('3').unwrap() as u32;
    let help = workspace
        .signature_help(&path, Position::new(2, column + 1))
        .expect("signature help");
    assert_eq!(help.active_parameter, 1, "{help:?}");
}

#[test]
fn signature_help_works_across_lines() {
    let source = "class A {\n    void f(int a, int b) {}\n    void g() {\n        f(\n            1,\n            2\n        );\n    }\n}\n";
    let (mut workspace, path) = workspace_with("multi-line", source);
    let line = 5;
    let column = source.lines().nth(line).unwrap().find('2').unwrap() as u32;
    let help = workspace
        .signature_help(&path, Position::new(line as u32, column + 1))
        .expect("signature help");
    assert_eq!(help.active_parameter, 1, "{help:?}");
    assert_eq!(help.active_signature, 0, "{help:?}");
}

#[test]
fn explicit_constructor_definition_lands_on_the_constructor_name() {
    let source = "class A {\n    A() {}\n}\n";
    let (mut workspace, path) = workspace_with("ctor", source);
    let definition = workspace
        .definition(&path, Position::new(1, 5))
        .expect("constructor definition");
    assert_eq!(definition.range.start.line, 1, "{definition:?}");
    assert_eq!(definition.range.start.column, 4, "{definition:?}");
}

#[test]
fn constructor_symbols_use_the_class_name() {
    let source = "class A {\n    A() {}\n}\n";
    let (mut workspace, path) = workspace_with("ctor-symbols", source);
    let symbols = workspace.document_symbols(&path);
    let a = symbols.iter().find(|symbol| symbol.name == "A").expect("A");
    let names: Vec<&str> = a.children.iter().map(|child| child.name.as_str()).collect();
    assert!(names.contains(&"A"), "constructor missing: {names:?}");
    assert!(!names.contains(&"<init>"), "raw <init> leaked: {names:?}");
}

#[test]
fn inlay_parameter_hint_sits_before_the_argument() {
    let source = "class A {\n    void m(int x) {}\n    void g() { m(1); }\n}\n";
    let (mut workspace, path) = workspace_with("param-hint", source);
    let hints = workspace.inlay_hints(&path);
    let hint = hints
        .iter()
        .find(|hint| hint.label.starts_with("x"))
        .expect("parameter hint");
    let line = source.lines().nth(2).unwrap();
    let argument = line.find('1').unwrap() as u32;
    assert_eq!(hint.position.line, 2, "{hint:?}");
    assert_eq!(hint.position.column, argument, "{hint:?}");
}

#[test]
fn type_definition_points_at_the_declared_type() {
    let source = "class A {\n    void f() { Service s = new Service(); }\n}\nclass Service {}\n";
    let (mut workspace, path) = workspace_with("type-def", source);
    let line = source.lines().nth(1).unwrap();
    let column = line.find(" s ").unwrap() as u32 + 1;
    let definition = workspace
        .type_definition(&path, Position::new(1, column + 1))
        .expect("type definition");
    assert_eq!(definition.range.start.line, 3, "{definition:?}");
}

#[test]
fn implementation_points_at_the_implementing_class() {
    let source = "interface I {}\nclass A implements I {}\n";
    let (mut workspace, path) = workspace_with("implementation", source);
    let line = source.lines().nth(1).unwrap();
    let definition = workspace
        .implementation(&path, Position::new(1, line.find('I').unwrap() as u32 + 1))
        .into_iter()
        .next()
        .expect("implementation");
    assert!(definition.file.ends_with("A.java"));
    assert_eq!(definition.range.start.line, 1, "{definition:?}");
}

#[test]
fn hover_works_for_local_variables() {
    let source = "class A {\n    void f() { int value = 1; }\n}\n";
    let (mut workspace, path) = workspace_with("hover-local", source);
    let line = source.lines().nth(1).unwrap();
    let column = line.find("value").unwrap() as u32;
    let hover = workspace
        .hover(&path, Position::new(1, column + 1))
        .expect("hover for a local variable");
    assert!(
        hover.contains("int") && hover.contains("value"),
        "hover: {hover}"
    );
}

#[test]
fn document_symbols_class_range_covers_members() {
    let source = "class A {\n    int f;\n    void m() {}\n}\n";
    let (mut workspace, path) = workspace_with("symbol-range", source);
    let symbols = workspace.document_symbols(&path);
    let a = symbols.iter().find(|symbol| symbol.name == "A").expect("A");
    assert_eq!(a.range.start.line, 0, "class range: {a:?}");
}

#[test]
fn references_include_the_import_statement() {
    let root = fixture("import-refs");
    let a_dir = root.join("a");
    let b_dir = root.join("b");
    std::fs::create_dir_all(&a_dir).unwrap();
    std::fs::create_dir_all(&b_dir).unwrap();
    let a = a_dir.join("A.java");
    let b = b_dir.join("B.java");
    std::fs::write(&a, "package a;\nimport b.B;\nclass A { B b; }\n").unwrap();
    std::fs::write(&b, "package b;\npublic class B {}\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let references = workspace.references(&b, Position::new(1, 13));
    eprintln!("import references: {references:?}");
    assert!(
        references
            .iter()
            .any(|location| location.file.ends_with("A.java") && location.range.start.line == 1),
        "import occurrence missing: {references:?}"
    );
}

#[test]
fn definition_on_an_import_statement() {
    let root = fixture("import-def");
    let a_dir = root.join("a");
    let b_dir = root.join("b");
    std::fs::create_dir_all(&a_dir).unwrap();
    std::fs::create_dir_all(&b_dir).unwrap();
    let a = a_dir.join("A.java");
    let b = b_dir.join("B.java");
    std::fs::write(&a, "package a;\nimport b.B;\nclass A { B b; }\n").unwrap();
    std::fs::write(&b, "package b;\npublic class B {}\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let definition = workspace
        .definition(&a, Position::new(1, 9))
        .expect("definition of imported B");
    assert!(definition.file.ends_with("B.java"), "{definition:?}");
    assert_eq!(definition.range.start.line, 1, "{definition:?}");
}

#[test]
fn completion_replaces_only_the_typed_prefix() {
    let source = "class B { int add4(int a) { return a; } }\nclass A { void f() { B b = new B(); b.ad } }\n";
    let (mut workspace, path) = workspace_with("completion-prefix", source);
    let line = source.lines().nth(1).unwrap();
    let column = line.find("b.ad").unwrap() as u32 + "b.ad".len() as u32;
    let items = workspace.completion(&path, Position::new(1, column));
    let item = items
        .iter()
        .find(|item| item.label == "add4")
        .expect("add4 completion");
    assert_eq!(
        item.replace_prefix_len,
        Some(2),
        "typed prefix must be replaced: {items:?}"
    );
}

#[test]
fn method_hover_shows_the_return_type() {
    let source = "class B { int compute() { return 1; } }\nclass A { void f() { B b = new B(); b.compute(); } }\n";
    let (mut workspace, path) = workspace_with("method-hover", source);
    let line = source.lines().nth(1).unwrap();
    let column = line.find("compute").unwrap() as u32;
    let hover = workspace
        .hover(&path, Position::new(1, column + 1))
        .expect("hover for a method");
    assert!(hover.contains("compute"), "{hover}");
    assert!(hover.contains("compute()"), "signature missing: {hover}");
    assert!(hover.contains("int"), "return type missing: {hover}");
}

#[test]
fn workspace_symbols_exclude_local_variables() {
    let source = "class A {\n    void f() { int local = 1; local++; }\n}\n";
    let (mut workspace, _path) = workspace_with("ws-locals", source);
    let symbols = workspace.workspace_symbols("local");
    assert!(
        symbols.iter().all(|symbol| symbol.name != "local"),
        "local variables must not appear in workspace symbols: {symbols:?}"
    );
}

#[test]
fn document_symbols_map_interface_and_enum_kinds() {
    let source = "interface I {}\nenum E { A }\n";
    let (mut workspace, path) = workspace_with("kinds", source);
    let symbols = workspace.document_symbols(&path);
    let i = symbols.iter().find(|symbol| symbol.name == "I").expect("I");
    let e = symbols.iter().find(|symbol| symbol.name == "E").expect("E");
    assert_eq!(i.kind, javac_analysis_api::SymbolKind::Interface, "{i:?}");
    assert_eq!(e.kind, javac_analysis_api::SymbolKind::Enum, "{e:?}");
}

#[test]
fn unused_import_ignores_strings_and_comments() {
    let root = fixture("unused-import");
    let a_dir = root.join("a");
    let b_dir = root.join("b");
    std::fs::create_dir_all(&a_dir).unwrap();
    std::fs::create_dir_all(&b_dir).unwrap();
    let a = a_dir.join("A.java");
    std::fs::write(&a, "package a;\nimport b.B;\nclass A { String s = \"B\"; // B\n}\n").unwrap();
    std::fs::write(b_dir.join("B.java"), "package b;\npublic class B {}\n").unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let diagnostics = workspace.diagnostics(&a, None);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            let message = diagnostic.message.to_lowercase();
            message.contains("unused") || message.contains("未使用")
        }),
        "unused import must be reported: {diagnostics:?}"
    );
}

#[test]
fn method_completion_inserts_parentheses() {
    let source = "class B { int add4(int a) { return a; } }\nclass A { void f() { B b = new B(); b.ad } }\n";
    let (mut workspace, path) = workspace_with("completion-parens", source);
    let line = source.lines().nth(1).unwrap();
    let column = line.find("b.ad").unwrap() as u32 + "b.ad".len() as u32;
    let items = workspace.completion(&path, Position::new(1, column));
    let item = items
        .iter()
        .find(|item| item.label == "add4")
        .expect("add4 completion");
    let insert = item.insert_text.clone().unwrap_or_else(|| item.label.clone());
    assert!(
        insert.starts_with("add4("),
        "method completion should insert call parentheses: {item:?}"
    );
}

#[test]
fn no_navigation_inside_string_literals() {
    let source = "class A {\n    int value;\n    String s = \"value\";\n}\n";
    let (mut workspace, path) = workspace_with("string-nav", source);
    let line = source.lines().nth(2).unwrap();
    let column = line.find("value").unwrap() as u32;
    let definition = workspace.definition(&path, Position::new(2, column + 1));
    let hover = workspace.hover(&path, Position::new(2, column + 1));
    assert!(
        definition.is_none(),
        "definition inside a string literal: {definition:?}"
    );
    assert!(hover.is_none(), "hover inside a string literal: {hover:?}");
}

#[test]
fn no_rename_inside_comments() {
    let source = "class A {\n    int value;\n    // value\n}\n";
    let (mut workspace, path) = workspace_with("comment-rename", source);
    let line = source.lines().nth(2).unwrap();
    let column = line.find("value").unwrap() as u32;
    let range = workspace.prepare_rename(&path, Position::new(2, column + 1));
    assert!(
        range.is_none(),
        "rename range inside a comment: {range:?}"
    );
}

#[test]
fn signature_help_is_not_shown_at_a_declaration() {
    let source = "class A {\n    void f(int a, String b) {}\n}\n";
    let (mut workspace, path) = workspace_with("signature-decl", source);
    let line = source.lines().nth(1).unwrap();
    let column = line.find("int").unwrap() as u32;
    let help = workspace.signature_help(&path, Position::new(1, column + 1));
    assert!(
        help.is_none(),
        "signature help at a declaration: {help:?}"
    );
}

#[test]
fn no_completion_inside_strings_or_comments() {
    let source = "class A {\n    String s = \"value\";\n    // value\n}\n";
    let (mut workspace, path) = workspace_with("completion-string", source);
    let string_line = source.lines().nth(1).unwrap();
    let string_column = string_line.find("value").unwrap() as u32;
    let items = workspace.completion(&path, Position::new(1, string_column + 1));
    assert!(
        items.is_empty(),
        "completion inside a string literal: {items:?}"
    );
    let comment_line = source.lines().nth(2).unwrap();
    let comment_column = comment_line.find("value").unwrap() as u32;
    let items = workspace.completion(&path, Position::new(2, comment_column + 1));
    assert!(
        items.is_empty(),
        "completion inside a comment: {items:?}"
    );
}

#[test]
fn completion_works_after_var() {
    let source = "class Service { int add4(int a) { return a; } }\nclass A { void f() { var s = new Service(); s.ad } }\n";
    let (mut workspace, path) = workspace_with("completion-var", source);
    let line = source.lines().nth(1).unwrap();
    let column = line.find("s.ad").unwrap() as u32 + "s.ad".len() as u32;
    let items = workspace.completion(&path, Position::new(1, column));
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"add4"), "labels: {labels:?}");
}

#[test]
fn completion_works_for_chained_calls() {
    let source = "class Builder { Builder name(String n) { return this; } int build() { return 1; } }\nclass A { void f() { new Builder().name(\"x\").bu } }\n";
    let (mut workspace, path) = workspace_with("completion-chain", source);
    let line = source.lines().nth(1).unwrap();
    let column = line.rfind(".bu").unwrap() as u32 + 3;
    let items = workspace.completion(&path, Position::new(1, column));
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"build"), "labels: {labels:?}");
}

#[test]
fn completion_works_for_this_members() {
    let source = "class A { int value; void f() { this.va } }\n";
    let (mut workspace, path) = workspace_with("completion-this", source);
    let line = source.lines().next().unwrap();
    let column = line.find("this.va").unwrap() as u32 + "this.va".len() as u32;
    let items = workspace.completion(&path, Position::new(0, column));
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"value"), "labels: {labels:?}");
}

#[test]
fn completion_offers_annotations() {
    let source = "class A {\n    @Over\n    void f() {}\n}\n";
    let (mut workspace, path) = workspace_with("completion-ann", source);
    let items = workspace.completion(&path, Position::new(1, 10));
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"Override"), "labels: {labels:?}");
}

#[test]
fn completion_works_for_static_members() {
    let source = "class Util { static int add(int a) { return a; } }\nclass A { void f() { Util.ad } }\n";
    let (mut workspace, path) = workspace_with("completion-static", source);
    let line = source.lines().nth(1).unwrap();
    let column = line.find("Util.ad").unwrap() as u32 + "Util.ad".len() as u32;
    let items = workspace.completion(&path, Position::new(1, column));
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"add"), "labels: {labels:?}");
}

#[test]
fn completion_ranks_locals_before_keywords() {
    let source = "class A {\n    void f() {\n        int value = 1;\n        \n    }\n}\n";
    let (mut workspace, path) = workspace_with("completion-rank", source);
    let items = workspace.completion(&path, Position::new(3, 8));
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    let value = labels.iter().position(|label| *label == "value");
    let keyword = labels.iter().position(|label| *label == "abstract");
    assert!(value.is_some(), "local missing: {labels:?}");
    assert!(
        value < keyword,
        "local must rank before keywords: {labels:?}"
    );
}

#[test]
fn format_preserves_crlf() {
    let jar = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tools")
        .join("google-java-format.jar");
    if !jar.exists() {
        return; // formatting tool not vendored in this checkout
    }
    unsafe { std::env::set_var("RAVAC_FORMAT_JAR", &jar); }
    let root = fixture("format-crlf");
    let path = root.join("A.java");
    let text = "class A{\r\nint x;\r\n}\r\n";
    std::fs::write(&path, text).unwrap();
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let formatted = workspace.format(&path, text).expect("formatted output");
    assert!(
        formatted.contains("\r\n"),
        "CRLF was lost by formatting: {formatted:?}"
    );
}

#[test]
fn parameter_hints_work_in_nested_calls() {
    let source = "class A {\n    void m(int x) {}\n    void g() { m(m(1)); }\n}\n";
    let (mut workspace, path) = workspace_with("param-hint-nested", source);
    let hints = workspace.inlay_hints(&path);
    let count = hints
        .iter()
        .filter(|hint| hint.label.starts_with("x"))
        .count();
    assert!(count >= 2, "expected hints for both calls: {hints:?}");
}

#[test]
fn constructor_parameter_hints() {
    let source = "class S { S(int x) {} }\nclass A { void f() { new S(1); } }\n";
    let (mut workspace, path) = workspace_with("ctor-hint", source);
    let hints = workspace.inlay_hints(&path);
    assert!(
        hints.iter().any(|hint| hint.label.starts_with("x")),
        "constructor parameter hint missing: {hints:?}"
    );
}

#[test]
fn completion_lists_members_after_a_dot() {
    let source = "class A { int value; void f() { this. } }\n";
    let (mut workspace, path) = workspace_with("completion-dot", source);
    let line = source.lines().next().unwrap();
    let column = line.find("this.").unwrap() as u32 + 5;
    let items = workspace.completion(&path, Position::new(0, column));
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"value"), "labels: {labels:?}");
}

#[test]
fn completion_lists_classes_after_new() {
    let source = "class Service {}\nclass A { void f() { new  } }\n";
    let (mut workspace, path) = workspace_with("completion-new", source);
    let line = source.lines().nth(1).unwrap();
    let column = line.find("new ").unwrap() as u32 + 4;
    let items = workspace.completion(&path, Position::new(1, column));
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"Service"), "labels: {labels:?}");
}

#[test]
fn document_highlight_returns_current_file_spans() {
    let source = "class A {\n    int value;\n    void f() { value = 1; value++; }\n}\n";
    let (mut workspace, path) = workspace_with("highlight", source);
    let line = source.lines().nth(1).unwrap();
    let column = line.find("value").unwrap() as u32;
    let highlights = workspace.document_highlight(&path, Position::new(1, column + 1));
    assert_eq!(highlights.len(), 3, "{highlights:?}");
}

#[test]
fn folding_ranges_cover_braces_and_comments() {
    let source = "class A {\n    // one\n    // two\n    void f() {\n        int x = 1;\n    }\n}\n";
    let (mut workspace, path) = workspace_with("folding", source);
    let ranges = workspace.folding_ranges(&path);
    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 0 && range.end_line == 6),
        "{ranges:?}"
    );
    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 1
                && range.end_line == 2
                && range.kind.is_some()),
        "{ranges:?}"
    );
}
