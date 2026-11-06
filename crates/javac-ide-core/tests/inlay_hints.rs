//! Inlay hint specification tests.

use std::path::PathBuf;

use javac_analysis_api::{WorkspaceService, InlayHintKind};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::Workspace;

fn session() -> Workspace {
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
        .join("inlay-hints");
    Workspace::new(AnalysisConfig {
        class_path: Vec::new(),
        output_dir: output,
        source_level: "17".to_string(),
        source_path: Vec::new(),
    })
}

fn hints_for(
    session: &mut Workspace,
    name: &str,
    text: &str,
) -> (PathBuf, Vec<javac_analysis_api::InlayHint>) {
    let path = PathBuf::from(format!("target/tmp/inlay-hints/{name}.java"));
    let _ = std::fs::create_dir_all("target/tmp/inlay-hints");
    std::fs::write(&path, text).ok();
    WorkspaceService::diagnostics(session, &path, Some(text));
    let hints = WorkspaceService::inlay_hints(session, &path);
    (path, hints)
}

#[test]
fn empty_blank_and_comment_files_have_no_hints() {
    let mut session = session();
    for (name, text) in [
        ("Empty", ""),
        ("Blank", "   \n\n"),
        ("Comment", "// no declarations here\n"),
    ] {
        let (_, hints) = hints_for(&mut session, name, text);
        assert!(hints.is_empty(), "{name}: {hints:?}");
    }
}

#[test]
fn unicode_identifier_and_argument_columns_are_utf16() {
    let mut session = session();
    let text = "class Unicode { void f() { var 数量 = \"你好\"; new StringBuffer(\"值\"); } }\n";
    let (_, hints) = hints_for(&mut session, "Unicode", text);
    let name_end = text.find("var 数量").expect("var") + "var 数量".len();
    let name_column = text[..name_end].encode_utf16().count() as u32;
    assert!(
        hints
            .iter()
            .any(|hint| hint.label == ": String" && hint.position.column == name_column),
        "unicode var: {hints:?}"
    );
    let argument = text.find("\"值\"").expect("arg");
    let argument_column = text[..argument].encode_utf16().count() as u32;
    assert!(
        hints
            .iter()
            .any(|hint| hint.label == "str:" && hint.position.column == argument_column),
        "unicode argument: {hints:?}"
    );
}

#[test]
fn shadowed_variables_get_their_own_hints() {
    let mut session = session();
    let text = "class Shadow { void f() { var x = 1; { var x = \"s\"; } } }\n";
    let (_, hints) = hints_for(&mut session, "Shadow", text);
    assert_eq!(
        hints.iter().filter(|hint| hint.label == ": int").count(),
        1,
        "{hints:?}"
    );
    assert_eq!(
        hints.iter().filter(|hint| hint.label == ": String").count(),
        1,
        "{hints:?}"
    );
    assert_ne!(hints[0].position.column, hints[1].position.column);
}

#[test]
fn multiline_call_argument_keeps_its_line_and_column() {
    let mut session = session();
    let text = "class Multi { void f() { new StringBuffer(\n            16\n        ); } }\n";
    let (_, hints) = hints_for(&mut session, "Multi", text);
    let byte = text.find("16").expect("arg");
    let line = text[..byte].matches('\n').count() as u32;
    let column = text[..byte].rsplit('\n').next().unwrap().encode_utf16().count() as u32;
    assert!(
        hints
            .iter()
            .any(|hint| hint.label == "capacity:" && hint.position.line == line && hint.position.column == column),
        "multiline argument: {hints:?}"
    );
}

#[test]
fn expanded_varargs_are_skipped_but_array_argument_is_hinted() {
    let mut session = session();
    let text = "class V { void m(String... values) {} void f() { m(\"a\", \"b\"); String[] arr = {}; m(arr); } }\n";
    let (_, hints) = hints_for(&mut session, "Varargs", text);
    assert_eq!(
        hints.iter().filter(|hint| hint.label == "values:").count(),
        1,
        "{hints:?}"
    );
}

#[test]
fn comment_before_argument_is_skipped() {
    let mut session = session();
    let text = "class C { void m(int value) {} void f() { m(/* c */ 1); } }\n";
    let (_, hints) = hints_for(&mut session, "CommentArgument", text);
    assert!(
        hints.iter().all(|hint| hint.label != "value:"),
        "comment argument: {hints:?}"
    );
}

#[test]
fn method_return_and_array_initializer_types() {
    let mut session = session();
    let text = "class T { String s() { return \"\"; } void f() { var a = s(); var b = new int[]{1}; } }\n";
    let (_, hints) = hints_for(&mut session, "Returned", text);
    let labels: Vec<&str> = hints.iter().map(|hint| hint.label.as_str()).collect();
    assert!(labels.contains(&": String"), "{labels:?}");
    assert!(labels.contains(&": int[]"), "{labels:?}");
}

#[test]
fn hints_remain_available_for_an_earlier_analyzed_file() {
    let mut session = session();
    let (one_path, one) = hints_for(&mut session, "EarlierOne", "class EarlierOne { void f() { var a = 1; } }\n");
    assert!(one.iter().any(|hint| hint.label == ": int"), "{one:?}");
    let (_, two) = hints_for(&mut session, "LaterTwo", "class LaterTwo { void f() { var b = \"s\"; } }\n");
    assert!(two.iter().any(|hint| hint.label == ": String"), "{two:?}");
    let again = WorkspaceService::inlay_hints(&mut session, &one_path);
    assert!(
        again.iter().any(|hint| hint.label == ": int"),
        "earlier file lost hints after analyzing another file: {again:?}"
    );
}

#[test]
fn var_in_static_and_instance_initializers() {
    let mut session = session();
    let text = "class Init { static { var s = \"a\"; } { var n = 1; } }\n";
    let (_, hints) = hints_for(&mut session, "Init", text);
    assert!(hints.iter().any(|hint| hint.label == ": String"), "{hints:?}");
    assert!(hints.iter().any(|hint| hint.label == ": int"), "{hints:?}");
}

#[test]
fn multiple_resources_and_multi_parameter_lambda() {
    let mut session = session();
    let text = "class Multi2 { void f(java.util.function.BiFunction<String,Integer,String> b) { try (var a = new java.io.StringReader(\"a\"); var c = new java.io.StringReader(\"b\")) { } b = (var x, var y) -> x + y; } }\n";
    let (_, hints) = hints_for(&mut session, "Multi2", text);
    let labels: Vec<&str> = hints.iter().map(|hint| hint.label.as_str()).collect();
    assert!(labels.contains(&": StringReader"), "{labels:?}");
    assert!(labels.contains(&": String"), "{labels:?}");
    assert!(labels.contains(&": Integer"), "{labels:?}");
}

#[test]
fn primitive_and_array_types() {
    let mut session = session();
    let text = "class P { void f() { var b = true; var c = 'x'; var s = (short)1; var by = (byte)1; var fl = 1.0f; var arr = new int[]{1}; } }\n";
    let (_, hints) = hints_for(&mut session, "Primitive", text);
    let labels: Vec<&str> = hints.iter().map(|hint| hint.label.as_str()).collect();
    for expected in [": boolean", ": char", ": short", ": byte", ": float", ": int[]"] {
        assert!(labels.contains(&expected), "missing {expected}: {labels:?}");
    }
}

#[test]
fn comment_and_newline_between_var_and_name() {
    let mut session = session();
    let text = "class ML { void f() { var /* c */ x = 1; var\n        y = \"s\"; } }\n";
    let (_, hints) = hints_for(&mut session, "ML", text);
    assert!(hints.iter().any(|hint| hint.label == ": int"), "{hints:?}");
    assert!(hints.iter().any(|hint| hint.label == ": String"), "{hints:?}");
}

#[test]
fn crlf_file_positions() {
    let mut session = session();
    let text = "class Crlf {\r\n    void f() {\r\n        var x = 1;\r\n        new StringBuffer(16);\r\n    }\r\n}\r\n";
    let (_, hints) = hints_for(&mut session, "Crlf", text);
    assert!(hints.iter().any(|hint| hint.label == ": int" && hint.position.line == 2), "{hints:?}");
    assert!(hints.iter().any(|hint| hint.label == "capacity:" && hint.position.line == 3), "{hints:?}");
}

#[test]
fn supplementary_unicode_columns() {
    let mut session = session();
    let text = "class Emoji { void f() { String s = \"😀\"; new StringBuffer(\"😀\"); } }\n";
    let (_, hints) = hints_for(&mut session, "Emoji", text);
    let argument = text.rfind("\"😀\"").expect("arg");
    let column = text[..argument].encode_utf16().count() as u32;
    assert!(
        hints.iter().any(|hint| hint.label == "str:" && hint.position.column == column),
        "emoji column: {hints:?}"
    );
}

#[test]
fn repeated_analysis_does_not_duplicate_hints() {
    let mut session = session();
    let text = "class Again { void f() { var x = 1; } }\n";
    let path = PathBuf::from("target/tmp/inlay-hints/Again.java");
    let _ = std::fs::create_dir_all("target/tmp/inlay-hints");
    std::fs::write(&path, text).ok();
    WorkspaceService::diagnostics(&mut session, &path, Some(text));
    let first = WorkspaceService::inlay_hints(&mut session, &path);
    WorkspaceService::diagnostics(&mut session, &path, Some(text));
    let second = WorkspaceService::inlay_hints(&mut session, &path);
    assert_eq!(first.len(), second.len(), "{first:?} != {second:?}");
    let mut keys: Vec<(u32, u32, String)> = second
        .iter()
        .map(|hint| (hint.position.line, hint.position.column, hint.label.clone()))
        .collect();
    let before = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(before, keys.len(), "duplicate hints: {second:?}");
}

#[test]
fn hints_before_analysis_and_for_unknown_file_are_empty() {
    let mut session = session();
    let path = PathBuf::from("target/tmp/inlay-hints/NeverAnalyzed.java");
    assert!(WorkspaceService::inlay_hints(&mut session, &path).is_empty());
}

#[test]
fn generic_method_parameter_name() {
    let mut session = session();
    let text = "class G { <T> T id(T value) { return value; } void f() { id(\"x\"); } }\n";
    let (_, hints) = hints_for(&mut session, "Generic", text);
    assert!(hints.iter().any(|hint| hint.label == "value:"), "{hints:?}");
}

#[test]
fn static_method_and_no_arg_constructor() {
    let mut session = session();
    let text = "class S { static void go(int count) {} void f() { go(1); new StringBuilder(); } }\n";
    let (_, hints) = hints_for(&mut session, "Static", text);
    assert!(hints.iter().any(|hint| hint.label == "count:"), "{hints:?}");
    assert!(hints.iter().all(|hint| hint.kind != InlayHintKind::Parameter || hint.label == "count:"), "{hints:?}");
}

#[test]
fn var_in_local_class() {
    let mut session = session();
    let text = "class L { void f() { class Local { void g() { var x = \"s\"; } } } }\n";
    let (_, hints) = hints_for(&mut session, "LocalClass", text);
    assert!(hints.iter().any(|hint| hint.label == ": String"), "{hints:?}");
}

#[test]
fn long_generic_type_is_rendered() {
    let mut session = session();
    let text = "class LongT { void f() { var map = new java.util.HashMap<String, java.util.List<Integer>>(); } }\n";
    let (_, hints) = hints_for(&mut session, "LongType", text);
    let label = hints.iter().find(|hint| hint.kind == InlayHintKind::Type).map(|hint| hint.label.as_str());
    let label = label.expect("type hint");
    assert!(label.contains("HashMap"), "{label}");
    assert!(label.contains("List"), "{label}");
}

#[test]
fn method_reference_does_not_produce_argument_hints() {
    let mut session = session();
    let text = "class Ref { void f() { Runnable r = System.out::println; } }\n";
    let (_, hints) = hints_for(&mut session, "MethodRef", text);
    assert!(hints.iter().all(|hint| hint.kind != InlayHintKind::Parameter), "{hints:?}");
}

#[test]
fn line_comment_between_var_and_name() {
    let mut session = session();
    let text = "class LineComment { void f() { var // c\n        x = 1; } }\n";
    let (_, hints) = hints_for(&mut session, "LineComment", text);
    assert!(hints.iter().any(|hint| hint.label == ": int"), "{hints:?}");
}

#[test]
fn annotation_before_var() {
    let mut session = session();
    let text = "class Annotated { void f() { @Deprecated var x = \"s\"; } }\n";
    let (_, hints) = hints_for(&mut session, "Annotated", text);
    assert!(hints.iter().any(|hint| hint.label == ": String"), "{hints:?}");
}

#[test]
fn file_path_with_spaces() {
    let mut session = session();
    let text = "class Spaced { void f() { var x = 1; } }\n";
    let (_, hints) = hints_for(&mut session, "Space Path", text);
    assert!(hints.iter().any(|hint| hint.label == ": int"), "{hints:?}");
}

#[test]
fn call_arguments_get_parameter_names() {
    let mut session = session();
    let path = PathBuf::from("target/tmp/inlay-hints/Parameters.java");
    let _ = std::fs::create_dir_all("target/tmp/inlay-hints");
    let text = "class Parameters { void f() { new StringBuffer(16); String s = new String(\"x\"); } }\n";
    std::fs::write(&path, text).ok();
    WorkspaceService::diagnostics(&mut session, &path, Some(text));

    let hints = WorkspaceService::inlay_hints(&mut session, &path);
    let after = |needle: &str| (text.find(needle).expect(needle)) as u32;
    let capacity = hints
        .iter()
        .find(|hint| hint.position.column == after("16") && hint.position.line == 0)
        .expect("capacity hint");
    assert_eq!(capacity.label, "capacity:");
    assert_eq!(capacity.kind, InlayHintKind::Parameter);
    let original = hints
        .iter()
        .find(|hint| hint.position.column == after("\"x\"") && hint.position.line == 0)
        .expect("original hint");
    assert_eq!(original.label, "original:");
    assert!(hints.iter().all(|hint| !hint.label.starts_with('p')));

    // Move the call several lines down and break analysis elsewhere; the
    // cached parameter hint is rebound by its argument snippet.
    let shifted = "class Parameters {\n\n    void f() {\n        int broken = ;\n        new StringBuffer();\n        new StringBuffer(16);\n    }\n}\n";
    std::fs::write(&path, shifted).ok();
    WorkspaceService::diagnostics(&mut session, &path, Some(shifted));
    let hints = WorkspaceService::inlay_hints(&mut session, &path);
    assert!(
        hints
            .iter()
            .any(|hint| hint.label == "capacity:" && hint.position.line == 5),
        "parameter hint did not follow its argument: {hints:?}"
    );
}

#[test]
fn last_hints_survive_a_temporary_parse_failure() {
    let mut session = session();
    let path = PathBuf::from("target/tmp/inlay-hints/Stable.java");
    let _ = std::fs::create_dir_all("target/tmp/inlay-hints");
    let valid = "class Stable { void f() { var s = \"x\"; } }\n";
    std::fs::write(&path, valid).ok();
    WorkspaceService::diagnostics(&mut session, &path, Some(valid));
    assert_eq!(WorkspaceService::inlay_hints(&mut session, &path).len(), 1);

    // A line inserted above moves the declaration; the cached hint must be
    // rebound by the var token, not by its old line/column.
    let broken = "class Stable {\n    void f() {\n        var s = ;\n    }\n}\n";
    std::fs::write(&path, broken).ok();
    WorkspaceService::diagnostics(&mut session, &path, Some(broken));
    let hints = WorkspaceService::inlay_hints(&mut session, &path);
    assert!(
        hints
            .iter()
            .any(|hint| hint.label == ": String" && hint.position.line == 2),
        "hint disappeared or did not move with the var token: {hints:?}"
    );

    let without_var = "class Stable { void f() { int s = 1; } }\n";
    std::fs::write(&path, without_var).ok();
    WorkspaceService::diagnostics(&mut session, &path, Some(without_var));
    assert!(
        WorkspaceService::inlay_hints(&mut session, &path).is_empty(),
        "stale hints survived after var removal"
    );
}

#[test]
fn old_analysis_rounds_do_not_leak_parameter_names() {
    let mut session = session();
    let path = PathBuf::from("target/tmp/inlay-hints/Rounds.java");
    let _ = std::fs::create_dir_all("target/tmp/inlay-hints");
    let first = "class Rounds { int add(int first, int second) { return 0; } void f() { add(1, 2); } }\n";
    std::fs::write(&path, first).ok();
    WorkspaceService::diagnostics(&mut session, &path, Some(first));
    let hints = WorkspaceService::inlay_hints(&mut session, &path);
    assert!(hints.iter().any(|hint| hint.label == "first:"), "{hints:?}");

    let second = first.replace("first", "left").replace("second", "right");
    std::fs::write(&path, &second).ok();
    WorkspaceService::diagnostics(&mut session, &path, Some(&second));
    let hints = WorkspaceService::inlay_hints(&mut session, &path);
    assert!(hints.iter().any(|hint| hint.label == "left:"), "{hints:?}");
    assert!(hints.iter().any(|hint| hint.label == "right:"), "{hints:?}");
    assert!(
        hints.iter().all(|hint| hint.label != "first:" && hint.label != "second:"),
        "stale round leaked: {hints:?}"
    );
}

#[test]
fn enhanced_for_var_gets_type_hint() {
    let mut session = session();
    let path = PathBuf::from("target/tmp/inlay-hints/Enhanced.java");
    let _ = std::fs::create_dir_all("target/tmp/inlay-hints");
    let text = "class Enhanced { void f(java.util.List<String> names) { for (var name : names) { } } }\n";
    std::fs::write(&path, text).ok();
    WorkspaceService::diagnostics(&mut session, &path, Some(text));

    let hints = WorkspaceService::inlay_hints(&mut session, &path);
    let after = (text.find("var name").expect("var name") + "var name".len()) as u32;
    assert!(
        hints.iter().any(|hint| hint.label == ": String" && hint.position.column == after),
        "enhanced-for var has no type hint: {hints:?}"
    );
}

#[test]
fn var_in_try_and_lambda_gets_type_hint() {
    let mut session = session();
    let path = PathBuf::from("target/tmp/inlay-hints/TryLambda.java");
    let _ = std::fs::create_dir_all("target/tmp/inlay-hints");
    let text = "class TryLambda { void f() { try (var reader = new java.io.StringReader(\"x\")) { } java.util.function.Consumer<String> c = (var value) -> { }; } }\n";
    std::fs::write(&path, text).ok();
    WorkspaceService::diagnostics(&mut session, &path, Some(text));

    let hints = WorkspaceService::inlay_hints(&mut session, &path);
    let reader = (text.find("var reader").expect("reader") + "var reader".len()) as u32;
    let value = (text.find("var value").expect("value") + "var value".len()) as u32;
    assert!(
        hints.iter().any(|hint| hint.label == ": StringReader" && hint.position.column == reader),
        "try-with-resources var missing: {hints:?}"
    );
    assert!(
        hints.iter().any(|hint| hint.label == ": String" && hint.position.column == value),
        "lambda var missing: {hints:?}"
    );
}

#[test]
fn collection_type_change_does_not_reuse_old_label() {
    let mut session = session();
    let path = PathBuf::from("target/tmp/inlay-hints/Capture.java");
    let _ = std::fs::create_dir_all("target/tmp/inlay-hints");
    let first = "class Capture { void f(java.util.List<String> names) { for (var name : names) { } } }\n";
    std::fs::write(&path, first).ok();
    WorkspaceService::diagnostics(&mut session, &path, Some(first));
    let hints = WorkspaceService::inlay_hints(&mut session, &path);
    assert!(hints.iter().any(|hint| hint.label == ": String"), "{hints:?}");

    let second = "class Capture { void f(java.util.List<?> names) { for (var name : names) { } } }\n";
    std::fs::write(&path, second).ok();
    WorkspaceService::diagnostics(&mut session, &path, Some(second));
    let hints = WorkspaceService::inlay_hints(&mut session, &path);
    assert!(
        hints.iter().all(|hint| hint.label != ": String"),
        "stale String hint after type change: {hints:?}"
    );
}

#[test]
fn nested_call_arguments_get_distinct_positions() {
    let mut session = session();
    let path = PathBuf::from("target/tmp/inlay-hints/Nested.java");
    let _ = std::fs::create_dir_all("target/tmp/inlay-hints");
    let text = "class Nested { int add(int left, int right) { return 0; } void f() { System.out.println(add(1, 2)); } }\n";
    std::fs::write(&path, text).ok();
    WorkspaceService::diagnostics(&mut session, &path, Some(text));

    let hints = WorkspaceService::inlay_hints(&mut session, &path);
    let outer = text.rfind("add(1").expect("inner call") as u32;
    let first = text.find("1, 2").expect("first arg") as u32;
    let second = text.find("2)").expect("second arg") as u32;
    assert!(
        hints.iter().any(|hint| hint.label == "x:" && hint.position.column == outer),
        "println parameter must start at the inner call: {hints:?}"
    );
    assert!(
        hints.iter().any(|hint| hint.label == "left:" && hint.position.column == first),
        "left must be at the first argument: {hints:?}"
    );
    assert!(
        hints.iter().any(|hint| hint.label == "right:" && hint.position.column == second),
        "right must be at the second argument: {hints:?}"
    );
}

#[test]
fn whitespace_edits_keep_all_hints() {
    let mut session = session();
    let path = PathBuf::from("target/tmp/inlay-hints/Whitespace.java");
    let _ = std::fs::create_dir_all("target/tmp/inlay-hints");
    let variants = [
        "class Whitespace {\n    void f() {\n        var x = 1L;\n        var a = x;\n        new StringBuffer(16);\n    }\n}\n",
        "class Whitespace { void f() { var x = 1L; var a = x; new StringBuffer(16); } }\n",
        "class Whitespace {\n\n\n    void f() {\n        var  x  =  1L;\n\n        var a = x;\n        new StringBuffer( 16 );\n    }\n}\n",
        "class Whitespace { void f() {\nvar x = 1L; var a = x;\nnew StringBuffer(16);\n} }\n",
    ];
    for (index, text) in variants.iter().enumerate() {
        std::fs::write(&path, text).ok();
        WorkspaceService::diagnostics(&mut session, &path, Some(text));
        let hints = WorkspaceService::inlay_hints(&mut session, &path);
        let labels: Vec<&str> = hints.iter().map(|hint| hint.label.as_str()).collect();
        assert!(
            labels.iter().filter(|label| **label == ": long").count() == 2,
            "variant {index} lost type hints: {labels:?}"
        );
        assert!(
            labels.contains(&"capacity:"),
            "variant {index} lost parameter hint: {labels:?}"
        );
    }
}

#[test]
fn var_hints_land_after_the_variable_name_only() {
    let mut session = session();
    let path = PathBuf::from("target/tmp/inlay-hints/Inferred.java");
    let _ = std::fs::create_dir_all("target/tmp/inlay-hints");
    let text = "class Inferred { void f() { int x = 1; var s = \"x\"; var n = x; } }\n";
    std::fs::write(&path, text).ok();
    WorkspaceService::diagnostics(&mut session, &path, Some(text));

    let hints = WorkspaceService::inlay_hints(&mut session, &path);
    assert_eq!(hints.len(), 2, "hints: {hints:?}");
    assert!(hints.iter().all(|hint| hint.kind == InlayHintKind::Type));

    let after = |needle: &str| (text.find(needle).expect(needle) + needle.len()) as u32;
    let s_position = after("var s");
    let n_position = after("var n");
    let x_position = after("int x");

    let s_hint = hints
        .iter()
        .find(|hint| hint.position.line == 0 && hint.position.column == s_position)
        .expect("hint after s");
    assert_eq!(s_hint.label, ": String");
    let n_hint = hints
        .iter()
        .find(|hint| hint.position.line == 0 && hint.position.column == n_position)
        .expect("hint after n");
    assert_eq!(n_hint.label, ": int");
    assert!(
        hints
            .iter()
            .all(|hint| hint.position.column != x_position),
        "explicit int must not get a hint: {hints:?}"
    );
}
