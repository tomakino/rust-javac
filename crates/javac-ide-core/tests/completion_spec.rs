//! Completion engine specification tests (docs/IDE_COMPLETION.md).
//!
//! These assert the documented rules; they are expected to fail until the
//! engine implements the spec.

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
        .join("completion-spec");
    AnalysisSession::new(AnalysisConfig {
        class_path: Vec::new(),
        output_dir: output,
        source_level: "17".to_string(),
        source_path: Vec::new(),
    })
}

/// Complete at the position of the marker "|" (removed from the text).
fn labels_with_cursor(session: &mut dyn AnalysisHost, name: &str, source: &str) -> Vec<String> {
    let cursor = source.find('|').expect("cursor marker");
    let text = source.replacen('|', "", 1);
    let before = &text[..cursor];
    let line = before.matches('\n').count() as u32;
    let column: u32 = before.rsplit('\n').next().unwrap().chars().map(|c| c.len_utf16() as u32).sum();
    let path = PathBuf::from(format!("target/tmp/completion-spec/{name}.java"));
    let _ = std::fs::create_dir_all("target/tmp/completion-spec");
    std::fs::write(&path, &text).ok();
    session.analyze(&path, Some(&text));
    session
        .completion(&path, Position::new(line, column))
        .into_iter()
        .map(|item| item.label)
        .collect()
}

fn has(labels: &[String], name: &str) -> bool{
    labels.iter().any(|label| label == name)
}

// R1: identifier receiver
#[test]
fn r1_identifier_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "R1", "class R1 { void f() { String s = \"\"; s.| } }");
    assert!(has(&labels, "trim"), "labels: {:?}", labels);
}

// R2: class name receiver only static
#[test]
fn r2_class_receiver_static_only() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "R2", "class R2 { void f() { Math.| } }");
    assert!(has(&labels, "PI"), "labels: {:?}", labels);
    assert!(!has(&labels, "toString"), "instance methods must not appear: {:?}", labels);
}

// R3: this
#[test]
fn r3_this_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "R3", "class R3 { int x; void f() { this.| } }");
    assert!(has(&labels, "x"), "labels: {:?}", labels);
}

// R4 / I5: super only direct superclass
#[test]
fn r4_super_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(
        &mut s,
        "R4",
        "class G4 { int g; } class P4 extends G4 { int p; } class R4 extends P4 { void f() { super.| } }",
    );
    assert!(has(&labels, "p"), "direct superclass field: {:?}", labels);
    assert!(!has(&labels, "g"), "grandparent field must not appear: {:?}", labels);
}

// R5-R9: literals
#[test]
fn r5_string_literal() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "R5", "class R5 { void f() { \"\".| } }");
    assert!(has(&labels, "trim"), "labels: {:?}", labels);
}

#[test]
fn r6_char_literal() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "R6", "class R6 { void f() { 'a'.| } }");
    assert!(has(&labels, "charValue"), "labels: {:?}", labels);
}

#[test]
fn r7_int_literal() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "R7", "class R7 { void f() { 1.| } }");
    assert!(has(&labels, "intValue"), "labels: {:?}", labels);
}

#[test]
fn r8_double_literal() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "R8", "class R8 { void f() { 1.5.| } }");
    assert!(has(&labels, "doubleValue"), "labels: {:?}", labels);
}

#[test]
fn r9_boolean_literal() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "R9", "class R9 { void f() { true.| } }");
    assert!(has(&labels, "booleanValue"), "labels: {:?}", labels);
}

// R10: null has no members
#[test]
fn r10_null_receiver_empty() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "R10", "class R10 { void f() { null.| } }");
    assert!(labels.is_empty(), "null receiver must be empty: {:?}", labels);
}

// R11: constructor / call / chain
#[test]
fn r11_new_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "R11a", "class R11a { void f() { new String().| } }");
    assert!(has(&labels, "trim"), "labels: {:?}", labels);
}

#[test]
fn r11_call_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(
        &mut s,
        "R11b",
        "class R11b { String foo() { return \"\"; } void f() { foo().| } }",
    );
    assert!(has(&labels, "trim"), "labels: {:?}", labels);
}

#[test]
fn r11_chain_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "R11c", "class R11c { void f() { \"\".trim().| } }");
    assert!(has(&labels, "length"), "labels: {:?}", labels);
}

// R12: arrays
#[test]
fn r12_array_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "R12", "class R12 { void f() { int[] a = null; a.| } }");
    assert!(has(&labels, "length"), "labels: {:?}", labels);
    assert!(!has(&labels, "trim"), "array must not expose String members: {:?}", labels);
}

// R13: primitive receiver is empty
#[test]
fn r13_primitive_receiver_empty() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "R13", "class R13 { void f() { int x = 0; x.| } }");
    assert!(labels.is_empty(), "primitive receiver must be empty: {:?}", labels);
}

// R-A: unresolved receiver must not fall back to global
#[test]
fn ra_unresolved_receiver_empty() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "RA", "class RA { void f() { \"\".nope| } }");
    assert!(labels.is_empty(), "unresolved receiver must be empty: {:?}", labels);
}

// V1/V3: package-private across packages
#[test]
fn v1_package_private_other_package() {
    let mut s = session();
    let labels = labels_with_cursor(
        &mut s,
        "V1",
        "package other; class V1 { void f() { String s = \"\"; s.| } }",
    );
    assert!(!has(&labels, "checkBoundsBeginEnd"), "package-private leaked: {:?}", labels);
    assert!(!has(&labels, "COMPACT_STRINGS"), "package-private field leaked: {:?}", labels);
}

// S1: class receiver has no instance members
#[test]
fn s1_static_only_for_class_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "S1", "class S1 { int x; void f() { S1.| } }");
    assert!(!has(&labels, "x"), "instance field on class receiver: {:?}", labels);
}

// S3: constructors excluded
#[test]
fn s3_no_constructors() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "S3", "class S3 { S3() {} void f() { this.| } }");
    assert!(!has(&labels, "<init>"), "constructor leaked: {:?}", labels);
}

// I1: interface default method
#[test]
fn i1_default_method() {
    let mut s = session();
    let labels = labels_with_cursor(
        &mut s,
        "I1",
        "interface I1i { default int hello() { return 1; } } class I1 implements I1i { void f() { this.| } }",
    );
    assert!(has(&labels, "hello"), "default method missing: {:?}", labels);
}

// I2: override deduplicated
#[test]
fn i2_override_deduplicated() {
    let mut s = session();
    let labels = labels_with_cursor(
        &mut s,
        "I2",
        "class I2p { public int foo() { return 1; } } class I2 extends I2p { public int foo() { return 2; } void f() { this.| } }",
    );
    let count = labels.iter().filter(|label| *label == "foo").count();
    assert_eq!(count, 1, "duplicate override: {:?}", labels);
}

// I4: Object methods
#[test]
fn i4_object_methods() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "I4", "class I4 { void f() { this.| } }");
    assert!(has(&labels, "toString"), "labels: {:?}", labels);
}

// I3: inherited field
#[test]
fn i3_inherited_field() {
    let mut s = session();
    let labels = labels_with_cursor(
        &mut s,
        "I3",
        "class I3p { protected int inheritedValue; } class I3 extends I3p { void f() { this.| } }",
    );
    assert!(has(&labels, "inheritedValue"), "labels: {:?}", labels);
}

// I5: interface inheritance chain
#[test]
fn i5_interface_inheritance_chain() {
    let mut s = session();
    let labels = labels_with_cursor(
        &mut s,
        "I5",
        "interface I5a { default int fromParent() { return 1; } } interface I5b extends I5a { } class I5 implements I5b { void f() { this.| } }",
    );
    assert!(has(&labels, "fromParent"), "labels: {:?}", labels);
}

// R14: generic method-chain return type
#[test]
fn r14_generic_chain_return_type() {
    let mut s = session();
    let labels = labels_with_cursor(
        &mut s,
        "R14",
        "import java.util.List; class R14 { void f(List<String> list) { list.get(0).| } }",
    );
    assert!(has(&labels, "length"), "generic chain lost String: {:?}", labels);
}

// R15: user method chain
#[test]
fn r15_user_method_chain() {
    let mut s = session();
    let labels = labels_with_cursor(
        &mut s,
        "R15",
        "class R15 { int field; R15 self() { return this; } void f() { self().| } }",
    );
    assert!(has(&labels, "field"), "labels: {:?}", labels);
}

// K1: receiver context has no keywords
#[test]
fn k1_no_keywords_in_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "K1", "class K1 { void f() { String s = \"\"; s.| } }");
    for keyword in ["null", "class", "return", "new"] {
        assert!(!has(&labels, keyword), "keyword {} in receiver completion: {:?}", keyword, labels);
    }
}

// G1: empty prefix does not list all classes
#[test]
fn g1_empty_prefix_no_classes() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "G1", "class G1 { void f() { | | } }");
    assert!(!has(&labels, "String"), "class listed with empty prefix: {:?}", labels);
}

// --- Additional spec coverage (contexts found missing in review) ---

// Receiver with an in-progress prefix (text.tri)
#[test]
fn rc_prefix_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "RC1", "class RC1 { void f() { String text = \"\"; text.tri| } }");
    assert!(has(&labels, "trim"), "labels: {:?}", labels);
    assert!(!has(&labels, "transient"), "global keyword leaked: {:?}", labels);
}

// var-inferred local
#[test]
fn rc_var_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "RC2", "class RC2 { void f() { var text = \"\"; text.| } }");
    assert!(has(&labels, "trim"), "labels: {:?}", labels);
}

// generic receiver
#[test]
fn rc_generic_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "RC3", "import java.util.List; class RC3 { void f() { List<String> l = null; l.| } }");
    assert!(has(&labels, "add"), "labels: {:?}", labels);
}

// enum receiver (static enum constants)
#[test]
fn rc_enum_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "RC4", "class RC4 { enum E { A, B } void f() { E.| } }");
    assert!(has(&labels, "A"), "labels: {:?}", labels);
}

// nested class receiver
#[test]
fn rc_nested_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "RC5", "class RC5 { static class Inner { static int V; } void f() { RC5.Inner.| } }");
    assert!(has(&labels, "V"), "labels: {:?}", labels);
}

// method parameter receiver
#[test]
fn rc_parameter_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "RC6", "class RC6 { void f(String arg) { arg.| } }");
    assert!(has(&labels, "trim"), "labels: {:?}", labels);
}

// for-each variable receiver
#[test]
fn rc_foreach_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "RC7", "import java.util.List; class RC7 { void f(List<String> list) { for (String s : list) { s.| } } }");
    assert!(has(&labels, "trim"), "labels: {:?}", labels);
}

// lambda parameter receiver
#[test]
fn rc_lambda_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "RC8", "import java.util.function.Function; class RC8 { void f() { Function<String, String> fn = s -> s.|; } }");
    assert!(has(&labels, "trim"), "labels: {:?}", labels);
}

// catch parameter receiver
#[test]
fn rc_catch_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "RC9", "class RC9 { void f() { try { } catch (RuntimeException e) { e.| } } }");
    assert!(has(&labels, "getMessage"), "labels: {:?}", labels);
}

// static import: unqualified constant
#[test]
fn rc_static_import() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "RC10", "import static java.lang.Math.*; class RC10 { void f() { PI| } }");
    assert!(has(&labels, "PI"), "static import not visible: {:?}", labels);
}

// this with prefix
#[test]
fn rc_this_prefix() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "RC11", "class RC11 { int value; void f() { this.val| } }");
    assert!(has(&labels, "value"), "labels: {:?}", labels);
}

// field receiver (implicit this field)
#[test]
fn rc_field_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "RC12", "class RC12 { String name; void f() { name.| } }");
    assert!(has(&labels, "trim"), "labels: {:?}", labels);
}

// var + prefix (reported: text.is fell back to global classes)
#[test]
fn rc_var_prefix_receiver() {
    let mut s = session();
    let labels = labels_with_cursor(&mut s, "RC13", "class RC13 { void f() { var text = \"\"; if (text.length() == 0) {} text.is| } }");
    assert!(has(&labels, "isEmpty"), "labels: {:?}", labels);
    assert!(!has(&labels, "IllegalStateException"), "global class leaked: {:?}", labels);
}

// ---------------------------------------------------------------------------
// S1-S14: insert text rules (docs/IDE_COMPLETION_INSERT.md section 5.2)
// ---------------------------------------------------------------------------

use javac_analysis_api::{CompletionItem, InsertTextFormat};

fn items_with_cursor(
    session: &mut dyn AnalysisHost,
    name: &str,
    source: &str,
) -> Vec<CompletionItem> {
    let cursor = source.find('|').expect("cursor marker");
    let text = source.replacen('|', "", 1);
    let before = &text[..cursor];
    let line = before.matches('\n').count() as u32;
    let column: u32 = before
        .rsplit('\n')
        .next()
        .unwrap()
        .chars()
        .map(|c| c.len_utf16() as u32)
        .sum();
    let path = PathBuf::from(format!("target/tmp/completion-spec/{name}.java"));
    let _ = std::fs::create_dir_all("target/tmp/completion-spec");
    std::fs::write(&path, &text).ok();
    session.analyze(&path, Some(&text));
    session.completion(&path, Position::new(line, column))
}

fn insert_of(items: &[CompletionItem], label: &str) -> (String, InsertTextFormat, Option<u32>) {
    let item = items
        .iter()
        .find(|item| item.label == label)
        .unwrap_or_else(|| panic!("missing item {label}"));
    (
        item.insert_text.clone().unwrap_or_default(),
        item.insert_text_format,
        item.replace_prefix_len,
    )
}

#[test]
fn s1_expression_string_zero_arguments() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "S1",
        "class S1 { String xxx() { return \"\"; } void f() { print(xx|); } void print(String s) {} }",
    );
    assert_eq!(insert_of(&items, "xxx").0, "xxx()");
}

#[test]
fn s2_expression_void_zero_arguments() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "S2",
        "class S2 { void xxx() {} void f() { print(xx|); } void print(String s) {} }",
    );
    assert_eq!(insert_of(&items, "xxx").0, "xxx();");
}

#[test]
fn s3_expression_string_with_arguments() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "S3",
        "class S3 { String xxx(int a) { return \"\"; } void f() { print(xx|); } void print(String s) {} }",
    );
    assert_eq!(insert_of(&items, "xxx").0, "xxx($0)");
}

#[test]
fn s4_expression_void_with_arguments() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "S4",
        "class S4 { void xxx(int a) {} void f() { print(xx|); } void print(String s) {} }",
    );
    assert_eq!(insert_of(&items, "xxx").0, "xxx();");
}

#[test]
fn s5_statement_string_zero_arguments() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "S5",
        "class S5 { String xxx() { return \"\"; } void f() { xx| } }",
    );
    assert_eq!(insert_of(&items, "xxx").0, "xxx()");
}

#[test]
fn s6_statement_void_zero_arguments() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S6", "class S6 { void xxx() {} void f() { xx| } }");
    assert_eq!(insert_of(&items, "xxx").0, "xxx();");
}

#[test]
fn s7_statement_string_with_arguments() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "S7",
        "class S7 { String xxx(int a) { return \"\"; } void f() { xx| } }",
    );
    assert_eq!(insert_of(&items, "xxx").0, "xxx($0)");
}

#[test]
fn s8_statement_void_with_arguments() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "S8",
        "class S8 { void xxx(int a) {} void f() { xx| } }",
    );
    assert_eq!(insert_of(&items, "xxx").0, "xxx($0);");
}

#[test]
fn s9_return_expression() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "S9",
        "class S9 { String get() { return \"\"; } String f() { return ge|; } }",
    );
    assert_eq!(insert_of(&items, "get").0, "get();");
}

#[test]
fn s10_class_body_initializer() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "S10",
        "class S10 { String get() { return \"\"; } String s = ge|; }",
    );
    assert_eq!(insert_of(&items, "get").0, "get()");
}

#[test]
fn s11_class_body_declaration_position() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "S11",
        "class S11 { String get() { return \"\"; } | }",
    );
    assert_eq!(insert_of(&items, "get").0, "get()");
}

#[test]
fn s12_prefix_replacement() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "S12",
        "class S12 { String get() { return \"\"; } void f() { ge| } }",
    );
    assert_eq!(insert_of(&items, "get").2, Some(2));
}

#[test]
fn s13_snippet_dollar_escape() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "S13",
        "class S13 { void a$b(int x) {} void f() { a$| } }",
    );
    assert_eq!(insert_of(&items, "a$b").0, "a\\$b($0);");
}

#[test]
fn s14_varargs_void() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "S14",
        "class S14 { void m(String... a) {} void f() { m| } }",
    );
    assert_eq!(insert_of(&items, "m").0, "m($0);");
}

#[test]
fn s15_anonymous_classes_hidden() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S15", "class S15 { void f() { Sy| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|label| label == "System"), "missing System: {labels:?}");
    assert!(
        !labels.iter().any(|label| label.contains("$1") || label.contains("$2")),
        "anonymous classes leaked: {labels:?}"
    );
}

#[test]
fn s16_nested_classes_hidden() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S16", "class S16 { void f() { Sy| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|label| label == "System"), "missing System: {labels:?}");
    assert!(
        !labels.iter().any(|label| label.contains('$')),
        "nested JDK classes leaked: {labels:?}"
    );
}

#[test]
fn s17_current_file_nested_class_visible() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "S17",
        "class S17 { static class Inner {} void f() { Inn| } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|label| label == "Inner"), "missing Inner: {labels:?}");
}

#[test]
fn s18_unexported_packages_hidden() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S18", "class S18 { void f() { Sy| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|label| label == "System"), "missing System: {labels:?}");
    assert!(
        !labels.iter().any(|label| label == "SystemModulesPlugin"),
        "unexported package class leaked: {labels:?}"
    );
}

#[test]
fn s19_overloads_show_signatures() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S19", "class S19 { void f() { System.out.p| } }");
    let details: Vec<String> = items
        .iter()
        .filter(|item| item.label == "print")
        .filter_map(|item| item.label_detail.clone())
        .collect();
    assert!(details.len() >= 2, "expected print overloads: {details:?}");
    assert!(
        details.iter().all(|detail| detail.starts_with('(')),
        "signatures missing: {details:?}"
    );
}

#[test]
fn s20_receiver_call_insert_context() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S20", "class S20 { void f() { System.out.p| } }");
    let (text, _, _) = insert_of(&items, "print");
    assert_eq!(text, "print($0);");
}

#[test]
fn s21_method_detail_is_return_type() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S21", "class S21 { void f() { System.out.p| } }");
    let print = items
        .iter()
        .find(|item| item.label == "print")
        .expect("print candidate");
    assert_eq!(print.detail.as_deref(), Some("void"));
    let signature = print.label_detail.clone().unwrap_or_default();
    assert!(!signature.contains("arg"), "synthetic parameter names leaked: {signature}");
}

#[test]
fn s22_positional_names_without_lvt() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S22", "class S22 { void f() { System.arr| } }");
    let arraycopy = items
        .iter()
        .find(|item| item.label == "arraycopy")
        .expect("arraycopy candidate");
    let signature = arraycopy.label_detail.clone().unwrap_or_default();
    assert!(signature.contains("p0"), "positional names missing: {signature}");
    assert!(signature.contains("p4"), "positional names missing: {signature}");
    assert!(!signature.contains("arg"), "synthetic arg names leaked: {signature}");
}

// --- Sorting (docs/IDE_COMPLETION_SORTING.md sections 3 and 11) ---

fn positions(items: &[CompletionItem]) -> Vec<String> {
    items.iter().map(|item| item.label.clone()).collect()
}

fn index_of(labels: &[String], name: &str) -> usize {
    labels.iter().position(|label| label == name).unwrap_or(usize::MAX)
}

#[test]
fn s23_fields_before_methods() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S23", "class S23 { void f() { System.| } }");
    let labels = positions(&items);
    assert!(index_of(&labels, "out") < index_of(&labels, "arraycopy"), "labels: {labels:?}");
    assert!(index_of(&labels, "in") < index_of(&labels, "arraycopy"), "labels: {labels:?}");
    assert!(index_of(&labels, "err") < index_of(&labels, "arraycopy"), "labels: {labels:?}");
}

#[test]
fn s24_methods_in_name_order() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S24", "class S24 { void f() { System.| } }");
    let labels = positions(&items);
    assert!(index_of(&labels, "arraycopy") < index_of(&labels, "clearProperty"), "labels: {labels:?}");
    assert!(index_of(&labels, "clearProperty") < index_of(&labels, "currentTimeMillis"), "labels: {labels:?}");
}

#[test]
fn s25_classes_after_methods_keywords_last() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S25", "class S25 { void f() { Sy| } }");
    let labels = positions(&items);
    let system = index_of(&labels, "System");
    assert!(system < labels.len(), "missing System: {labels:?}");
    if let Some(synchronized) = labels.iter().position(|label| label == "synchronized") {
        assert!(system < synchronized, "labels: {labels:?}");
    }
}

#[test]
fn s26_overloads_adjacent() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S26", "class S26 { void f() { System.out.pr| } }");
    let labels = positions(&items);
    let print_positions: Vec<usize> = labels
        .iter()
        .enumerate()
        .filter(|(_, label)| label.as_str() == "print")
        .map(|(index, _)| index)
        .collect();
    assert!(print_positions.len() >= 2, "labels: {labels:?}");
    assert!(
        print_positions.windows(2).all(|window| window[1] == window[0] + 1),
        "overloads not adjacent: {print_positions:?}"
    );
}

#[test]
fn s27_exact_match_first() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S27", "class S27 { void f() { System| } }");
    assert_eq!(items.first().map(|item| item.label.as_str()), Some("System"));
}

#[test]
fn s28_camel_hump_match() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S28", "class S28 { void f() { System.gSM| } }");
    let labels = positions(&items);
    assert!(labels.iter().any(|label| label == "getSecurityManager"), "labels: {labels:?}");
}

#[test]
fn s29_start_match_group() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S29", "class S29 { void f() { Sy| } }");
    let labels = positions(&items);
    assert!(labels.iter().any(|label| label == "System"), "labels: {labels:?}");
}

#[test]
fn s30_middle_match_present() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S30", "class S30 { void f() { ystem| } }");
    let labels = positions(&items);
    assert!(labels.iter().any(|label| label == "System"), "labels: {labels:?}");
}

#[test]
fn s32_sort_text_tracks_order() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S32", "class S32 { void f() { System.| } }");
    let sort_texts: Vec<String> = items
        .iter()
        .map(|item| item.sort_text.clone().unwrap_or_default())
        .collect();
    assert!(sort_texts.iter().all(|text| !text.is_empty()), "missing sortText");
    let mut sorted = sort_texts.clone();
    sorted.sort();
    assert_eq!(sort_texts, sorted, "sortText order mismatch");
}

#[test]
fn s31_member_completion_includes_nested_classes() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S31", "class S31 { void f() { System.lo| } }");
    let labels = positions(&items);
    assert!(labels.iter().any(|label| label == "Logger"), "labels: {labels:?}");
    assert!(labels.iter().any(|label| label == "LoggerFinder"), "labels: {labels:?}");
    let logger = index_of(&labels, "Logger");
    let getlogger = index_of(&labels, "getLogger");
    assert!(logger < getlogger, "start matches must precede middle matches: {labels:?}");
}

#[test]
fn s33_prefix_lo_matches_idea_candidates() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S33", "class S33 { void f() { System.lo| } }");
    let labels = positions(&items);
    assert!(labels.iter().any(|label| label == "load"), "labels: {labels:?}");
    assert!(labels.iter().any(|label| label == "Logger"), "labels: {labels:?}");
    assert!(labels.iter().any(|label| label == "getLogger"), "labels: {labels:?}");
    assert!(!labels.iter().any(|label| label == "clearProperty"), "cross-word match leaked: {labels:?}");
    assert!(!labels.iter().any(|label| label == "lineSeparator"), "cross-word match leaked: {labels:?}");
    assert!(!labels.iter().any(|label| label == "runFinalization"), "cross-word match leaked: {labels:?}");
}

#[test]
fn s34_c_matches_idea_quality_order() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "S34", "class S34 { void f() { System.c| } }");
    let labels = positions(&items);
    let console = index_of(&labels, "console");
    let identity = index_of(&labels, "identityHashCode");
    let arraycopy = index_of(&labels, "arraycopy");
    assert!(console < labels.len(), "missing console: {labels:?}");
    assert!(identity < labels.len(), "missing identityHashCode: {labels:?}");
    assert!(arraycopy < labels.len(), "missing arraycopy: {labels:?}");
    assert!(console < identity, "start match first: {labels:?}");
    assert!(identity < arraycopy, "camel before middle match: {labels:?}");
}

#[test]
fn t43_generic_return_type_detail() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "T43", "class T43 { void f() { System.| } }");
    let getenv = items
        .iter()
        .find(|item| item.label == "getenv" && item.label_detail.as_deref() == Some("()"))
        .expect("getenv()");
    assert_eq!(
        getenv.detail.as_deref(),
        Some("Map<String, String>"),
        "getenv() detail"
    );
}

#[test]
fn t44_generic_parameter_wildcard_and_varargs() {
    let mut s = session();
    let source = "class T44 { void f(java.util.List<? extends Number> values, String... args) {} void g() { T44 t = new T44(); t.| } }";
    let items = items_with_cursor(&mut s, "T44", source);
    let item = items.iter().find(|item| item.label == "f").expect("f");
    assert_eq!(
        item.label_detail.as_deref(),
        Some("(List<? extends Number> values, String... args)"),
        "f signature"
    );
}

#[test]
fn t45_generic_type_variable_detail() {
    let mut s = session();
    let source = "class T45 { java.util.List<String> names; void g() { T45 t = new T45(); t.| } }";
    let items = items_with_cursor(&mut s, "T45", source);
    let item = items.iter().find(|item| item.label == "names").expect("names");
    assert!(
        item.detail.as_deref() == Some("List<String>"),
        "names detail: {:?}",
        item.detail
    );
}

#[test]
fn t46_double_dot_returns_nothing() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "T46", "class T46 { void f() { System..| } }");
    assert!(items.is_empty(), "System.. must not complete: {:?}", items.iter().map(|i| &i.label).collect::<Vec<_>>());
}

#[test]
fn t47_package_members() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "T47", "class T47 { void f() { java.util.| } }");
    let labels = items.iter().map(|i| i.label.clone()).collect::<Vec<_>>();
    assert!(labels.iter().any(|l| l == "Arrays"), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l.starts_with("ArrayList")), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l == "stream"), "labels: {labels:?}");
}

#[test]
fn t48_package_prefix_does_not_borrow_receiver() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T48",
        "class T48 { void f() { app.service.build().name(\"demo\").size(3).finish(); java.util| } }",
    );
    let labels = items.iter().map(|i| i.label.clone()).collect::<Vec<_>>();
    assert!(!labels.iter().any(|l| l == "finish"), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l == "util"), "labels: {labels:?}");
}
#[test]
fn t49_package_class_shows_type_parameters_but_inserts_plain_name() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "T49", "class T49 { void f() { java.util.| } }");
    let item = items
        .iter()
        .find(|item| item.label == "ArrayList<E>")
        .expect("ArrayList<E>");
    assert_eq!(item.insert_text.as_deref(), Some("ArrayList"));
    let deque = items
        .iter()
        .find(|item| item.label == "ArrayDeque<E>")
        .expect("ArrayDeque<E>");
    assert_eq!(deque.insert_text.as_deref(), Some("ArrayDeque"));
}

#[test]
fn t50_qualified_class_receiver() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T50",
        "class T50 { void f() { java.util.Base64.get| } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|l| l == "getDecoder"), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l == "getEncoder"), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l == "getInteger"), "labels: {labels:?}");
}

#[test]
fn t51_qualified_class_prefix_expands_members() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T51",
        "class T51 { void f() { java.util.Base64| } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|l| l == "Decoder"), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l == "Encoder"), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l == "getDecoder"), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l == "getInteger"), "labels: {labels:?}");
}

#[test]
fn t52_new_constructor_completion() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(&mut s, "T52", "class T52 { void f() { new Zip| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|l| l.starts_with("ZipFile(")), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l.starts_with("ZipEntry(")), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l.starts_with("ZipFileSystemProvider")), "labels: {labels:?}");
    let zip_file = items
        .iter()
        .find(|item| item.label.starts_with("ZipFile("))
        .expect("ZipFile");
    assert_eq!(zip_file.detail.as_deref(), Some("java.util.zip"));
    assert!(zip_file.insert_text.as_deref().unwrap_or("").starts_with("ZipFile("));
}

#[test]
fn t52b_nested_new_uses_array_expected_type_mode() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T52b",
        "class T52b { void f() { new String(new by|); } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels.first().map(String::as_str), Some("byte[]"), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l == "Byte"), "labels: {labels:?}");
    assert!(
        !labels.iter().any(|l| l.starts_with("Byte(")),
        "constructors must not be expanded when an array is expected: {labels:?}"
    );
    let byte = labels.iter().position(|l| l == "Byte").expect("Byte");
    for middle in ["DataBufferByte", "IIOByteBuffer"] {
        if let Some(index) = labels.iter().position(|l| l == middle) {
            assert!(byte < index, "{middle} must rank after start matches: {labels:?}");
        }
    }
}

#[test]
fn t52d_new_prefix_does_not_match_middle_substrings() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(&mut s, "T52d", "class T52d { void f() { new byte| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|l| l.starts_with("Byte(")), "labels: {labels:?}");
    let byte = labels
        .iter()
        .position(|l| l.starts_with("Byte("))
        .expect("Byte constructor");
    for middle in ["DataBufferByte", "IIOByteBuffer", "InterruptedByTimeoutException"] {
        if let Some(index) = labels.iter().position(|l| l.starts_with(middle)) {
            assert!(byte < index, "{middle} must rank after start matches: {labels:?}");
        }
    }
}

#[test]
fn t52c_new_argument_of_method_uses_expected_array() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T52c",
        "class T52c { void take(byte[] data) {} void f() { take(new by|); } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels.first().map(String::as_str), Some("byte[]"), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l == "Byte"), "labels: {labels:?}");
    assert!(
        !labels.iter().any(|l| l.starts_with("Byte(")),
        "constructors must not be expanded: {labels:?}"
    );
}

#[test]
fn t52e_new_initializer_expected_array() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T52e",
        "class T52e { void f() { byte[] data = new by|; } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels.first().map(String::as_str), Some("byte[]"), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l.starts_with("Byte(")), "labels: {labels:?}");
}

#[test]
fn t52f_new_return_expected_array() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T52f",
        "class T52f { byte[] f() { return new by|; } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels.first().map(String::as_str), Some("byte[]"), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l.starts_with("Byte(")), "labels: {labels:?}");
}

#[test]
fn t52g_new_cast_expected_array() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T52g",
        "class T52g { void f() { Object o = (byte[]) new by|; } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels.first().map(String::as_str), Some("byte[]"), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l.starts_with("Byte(")), "labels: {labels:?}");
}

#[test]
fn t52h_assignment_expected_class_is_prioritized() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T52h",
        "class T52h { void f() { String s = new Str|; } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels.first().map(String::as_str), Some("String"), "labels: {labels:?}");
}

#[test]
fn t52i_varargs_element_is_not_array_expected() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(
        &mut s,
        "T52i",
        "class T52i { void take(byte... values) {} void f() { take(new by|); } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|l| l.starts_with("Byte(")), "labels: {labels:?}");
}

#[test]
fn t52j_constructor_generic_substitution() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T52j",
        "class T52j { static class Box<T> { Box(T value) {} } void f() { new Box<byte[]>(new by|); } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels.first().map(String::as_str), Some("byte[]"), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l.starts_with("Byte(")), "labels: {labels:?}");
}

#[test]
fn t52k_method_generic_substitution() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T52k",
        "class T52k { <T> void take(T value) {} void f() { this.<byte[]>take(new by|); } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels.first().map(String::as_str), Some("byte[]"), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l.starts_with("Byte(")), "labels: {labels:?}");
}

#[test]
fn t52l_method_overloads_contribute_expected_types() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T52l",
        "class T52l { void take(String value) {} void take(byte[] value) {} void f() { take(new by|); } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels.first().map(String::as_str), Some("byte[]"), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l.starts_with("Byte(")), "labels: {labels:?}");
}

#[test]
fn t52m_generic_inference_from_previous_argument() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T52m",
        "class T52m { <T> void take(T first, T second) {} void f() { take(new byte[0], new by|); } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels.first().map(String::as_str), Some("byte[]"), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l.starts_with("Byte(")), "labels: {labels:?}");
}

#[test]
fn t52n_constructor_generic_inference_from_previous_argument() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T52n",
        "class T52n { static class Box<T> { Box(T first, T second) {} } void f() { new Box<>(new byte[0], new by|); } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels.first().map(String::as_str), Some("byte[]"), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l.starts_with("Byte(")), "labels: {labels:?}");
}

#[test]
fn t52o_plain_type_mode_without_constructor_expansion() {
    let mut s = session();
    s.set_completion_show_constructors(false);
    let items = items_with_cursor(
        &mut s,
        "T52o",
        "class T52o { void f() { new Str| } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|l| l == "StringBuilder"), "labels: {labels:?}");
    assert!(
        !labels.iter().any(|l| l.starts_with("StringBuilder(")),
        "constructors must be off: {labels:?}"
    );
    let item = items
        .iter()
        .find(|item| item.label == "StringBuilder")
        .expect("StringBuilder");
    assert_eq!(
        item.insert_text.as_deref(),
        Some("StringBuilder($0)"),
        "plain type insert should add parentheses"
    );
}

#[test]
fn t52p_method_argument_expected_class_is_prioritized() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T52p",
        "class T52p { static class Target {} void take(Target value) {} void f() { take(new Tar|); } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels.first().map(String::as_str), Some("Target"), "labels: {labels:?}");
}

#[test]
fn t52q_primitive_array_expected() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T52q",
        "class T52q { void f() { new String(new int|); } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels.first().map(String::as_str), Some("int[]"), "labels: {labels:?}");
}

#[test]
fn t52r_class_candidates_are_unique() {
    let mut s = session();
    s.set_completion_show_constructors(false);
    let items = items_with_cursor(
        &mut s,
        "T52r",
        "class T52r { void f() { new Identity| } }",
    );
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    let mut unique = labels.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(labels.len(), unique.len(), "duplicate classes: {labels:?}");
}

#[test]
fn t52s_primitive_array_without_expected_type() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "T52s", "class T52s { void f() { new int| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels.first().map(String::as_str), Some("int[]"), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l.starts_with("Integer(")), "labels: {labels:?}");
}

#[test]
fn t53_top_level_package_subpackages() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "T53", "class T53 { void f() { java.| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|l| l == "util"), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l == "lang"), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l == "io"), "labels: {labels:?}");
}

#[test]
fn t54_package_members_mix_types_and_subpackages() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "T54", "class T54 { void f() { java.util.| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    // Subpackages come first (VS Code/IDEA package completion order).
    assert_eq!(labels[0], "concurrent", "labels: {labels:?}");
    assert!(labels.iter().any(|l| l == "stream"), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l.starts_with("ArrayList")), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l.starts_with("AbstractList")), "labels: {labels:?}");
}

#[test]
fn t54b_package_member_prefix_filters_types() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "T54b", "class T54b { void f() { java.util.Ha| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|l| l.starts_with("HashMap")), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l.starts_with("ArrayList")), "labels: {labels:?}");
}

#[test]
fn t55_package_prefix_returns_only_subpackage() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "T55", "class T55 { void f() { java.util| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels, vec!["util".to_string()], "labels: {labels:?}");
}

#[test]
fn t56_lang_package_members() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "T56", "class T56 { void f() { java.lang.S| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|l| l == "String"), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l == "System"), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l == "StringBuilder"), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l == "Integer"), "labels: {labels:?}");
}

#[test]
fn t57_qualified_class_prefix_inserts_dot_member() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "T57", "class T57 { void f() { java.util.Base64| } }");
    let item = items
        .iter()
        .find(|item| item.label == "getDecoder")
        .expect("getDecoder");
    assert_eq!(item.insert_text.as_deref(), Some(".getDecoder()"));
    assert_eq!(item.detail.as_deref(), Some("Decoder"));
}

#[test]
fn t58_new_zip_constructors() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(&mut s, "T58", "class T58 { void f() { new Zip| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|l| l.starts_with("ZipFile(")), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l.starts_with("ZipInputStream(")), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l.starts_with("ZipOutputStream(")), "labels: {labels:?}");
    let zip_file = items
        .iter()
        .find(|item| item.label == "ZipFile(File p0)")
        .or_else(|| items.iter().find(|item| item.label.starts_with("ZipFile(")))
        .expect("ZipFile ctor");
    assert_eq!(zip_file.detail.as_deref(), Some("java.util.zip"));
    assert!(zip_file.insert_text.as_deref().unwrap_or("").starts_with("ZipFile("));
}

#[test]
fn t59_new_camel_expands_constructors() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(&mut s, "T59", "class T59 { void f() { new zie| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|l| l.starts_with("ZipEntry(")), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l.starts_with("ZipException(")), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l.starts_with("ZipError(")), "labels: {labels:?}");
    assert_eq!(labels.len(), 5, "labels: {labels:?}");
}

#[test]
fn t60_new_lowercase_prefix_expands_constructors() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(&mut s, "T60", "class T60 { void f() { new zipf| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|l| l.starts_with("ZipFile(")), "labels: {labels:?}");
    let zip_file = items
        .iter()
        .find(|item| item.label.starts_with("ZipFile("))
        .expect("ZipFile ctor");
    assert_eq!(zip_file.detail.as_deref(), Some("java.util.zip"));
}

#[test]
fn t60b_new_mixed_case_expands_input_streams() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(&mut s, "T60b", "class T60b { void f() { new Filein| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|l| l.starts_with("FileInputStream(")), "labels: {labels:?}");
}

#[test]
fn t61_new_file_constructors() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(&mut s, "T61", "class T61 { void f() { new File| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(labels.iter().any(|l| l == "File(String pathname)"), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l == "File(URI uri)"), "labels: {labels:?}");
    assert!(labels.iter().any(|l| l == "File(File parent, String child)"), "labels: {labels:?}");
}

#[test]
fn t62_new_skips_package_private_classes() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "T62", "class T62 { void f() { new Zip| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert!(!labels.iter().any(|l| l.starts_with("ZipEntryIterator")), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l.starts_with("ZipCoder")), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l.starts_with("ZipUtils")), "labels: {labels:?}");
}

#[test]
fn t63_wildcard_super_and_extends() {
    let mut s = session();
    let source = "class T63 { void f(java.util.List<? extends Number> a, java.util.List<? super Integer> b, java.util.List<String> c) {} void g() { T63 t = new T63(); t.| } }";
    let items = items_with_cursor(&mut s, "T63", source);
    let item = items.iter().find(|item| item.label == "f").expect("f");
    assert_eq!(
        item.label_detail.as_deref(),
        Some("(List<? extends Number> a, List<? super Integer> b, List<String> c)")
    );
}

#[test]
fn t64_field_detail_is_declared_type() {
    let mut s = session();
    let items = items_with_cursor(&mut s, "T64", "class T64 { java.io.PrintStream out; void g() { T64 t = new T64(); t.| } }");
    let item = items.iter().find(|item| item.label == "out").expect("out");
    assert_eq!(item.detail.as_deref(), Some("PrintStream"));
}

// IDEA CamelHump matches "Fileinp" as File + I(nput) from ImageInputStream
// and FileCacheImageInputStream too: the acronym/word boundaries are real
// match positions, not a custom start-only filter.
#[test]
fn t60c_new_fileinp_matches_idea_camel() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(&mut s, "T60c", "class T60c { void f() { new Fileinp| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels.len(), 6, "labels: {labels:?}");
    for expected in [
        "FileInputStream",
        "FileImageInputStream",
        "FileCacheImageInputStream",
    ] {
        assert!(
            labels.iter().any(|label| label.starts_with(&format!("{expected}("))),
            "missing {expected}: {labels:?}"
        );
    }
    assert!(!labels.iter().any(|l| l.starts_with("File(")), "labels: {labels:?}");
    assert!(!labels.iter().any(|l| l.starts_with("FileHandler(")), "labels: {labels:?}");
}

#[test]
fn t60d_new_fileinput_matches_idea_camel() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(&mut s, "T60d", "class T60d { void f() { new FileInput| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    assert_eq!(labels.len(), 6, "labels: {labels:?}");
    for expected in [
        "FileInputStream",
        "FileImageInputStream",
        "FileCacheImageInputStream",
    ] {
        assert!(
            labels.iter().any(|label| label.starts_with(&format!("{expected}("))),
            "missing {expected}: {labels:?}"
        );
    }
}

// Module coverage: java.desktop/jdk.compiler packages must participate in
// "new Fil" completion, not just java.base.
#[test]
fn t66_new_fil_covers_all_jdk_modules() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(&mut s, "T66", "class T66 { void f() { new Fil| } }");
    let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
    for expected in [
        "FileImageInputStream(",
        "FileSystemView(",
        "FileDialog(",
        "FileTypeDetector(",
    ] {
        assert!(
            labels.iter().any(|label| label.starts_with(expected)),
            "missing {expected}: {labels:?}"
        );
    }
}

// Constructor completion must not insert synthetic parameter names; the
// editor places the cursor inside the parentheses instead.
#[test]
fn t75_constructor_insert_is_a_placeholder() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(
        &mut s,
        "T75",
        "class T75 { void f() { new StringBuffer| } }",
    );
    let item = items
        .iter()
        .find(|item| item.label == "StringBuffer(int capacity)")
        .expect("StringBuffer(int capacity)");
    assert_eq!(item.insert_text.as_deref(), Some("StringBuffer($0)"));
    assert_eq!(
        item.insert_text_format,
        javac_analysis_api::InsertTextFormat::Snippet
    );
}

// Auto-import: selecting an unimported type adds the import statement.
#[test]
fn t69_completion_adds_import_for_new_type() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(&mut s, "T69", "class T69 { void f() { new ArrayLis| } }");
    let item = items
        .iter()
        .find(|item| item.label.starts_with("ArrayList("))
        .expect("ArrayList constructor");
    let edit = item
        .additional_text_edits
        .first()
        .expect("auto-import edit");
    assert_eq!(edit.new_text, "import java.util.ArrayList;\n");
    assert_eq!(edit.range.start, Position::new(0, 0));
}

#[test]
fn t70_completion_skips_existing_import() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(
        &mut s,
        "T70",
        "import java.util.ArrayList;\nclass T70 { void f() { new ArrayLis| } }",
    );
    let item = items
        .iter()
        .find(|item| item.label.starts_with("ArrayList("))
        .expect("ArrayList constructor");
    assert!(
        item.additional_text_edits.is_empty(),
        "already imported: {:?}",
        item.additional_text_edits
    );
}

#[test]
fn t71_completion_import_after_import_block() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(
        &mut s,
        "T71",
        "package p;\nimport java.util.List;\nclass T71 { void f() { new ArrayDeque| } }",
    );
    let item = items
        .iter()
        .find(|item| item.label.starts_with("ArrayDeque("))
        .expect("ArrayDeque constructor");
    let edit = item
        .additional_text_edits
        .first()
        .expect("auto-import edit");
    assert_eq!(edit.new_text, "import java.util.ArrayDeque;\n");
    assert_eq!(edit.range.start, Position::new(2, 0));
}

#[test]
fn t72_completion_skips_java_lang_import() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(&mut s, "T72", "class T72 { void f() { new StringBuil| } }");
    let item = items
        .iter()
        .find(|item| item.label.starts_with("StringBuilder("))
        .expect("StringBuilder constructor");
    assert!(
        item.additional_text_edits.is_empty(),
        "java.lang needs no import: {:?}",
        item.additional_text_edits
    );
}

#[test]
fn t73_completion_skips_wildcard_import() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(
        &mut s,
        "T73",
        "import java.util.*;\nclass T73 { void f() { new ArrayLis| } }",
    );
    let item = items
        .iter()
        .find(|item| item.label.starts_with("ArrayList("))
        .expect("ArrayList constructor");
    assert!(
        item.additional_text_edits.is_empty(),
        "wildcard import covers ArrayList: {:?}",
        item.additional_text_edits
    );
}

#[test]
fn t74_completion_skips_conflicting_import() {
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(
        &mut s,
        "T74",
        "import other.ArrayList;\nclass T74 { void f() { new ArrayLis| } }",
    );
    let item = items
        .iter()
        .find(|item| item.label.starts_with("ArrayList("))
        .expect("ArrayList constructor");
    assert!(
        item.additional_text_edits.is_empty(),
        "conflicting simple name: {:?}",
        item.additional_text_edits
    );
}

// Regression: a package completion must not poison the package's symbol table
// for a later qualified-class completion (Base64 nested Decoder/Encoder).
fn jdk_src_available() -> bool {
    if let Some(path) = std::env::var_os("RAVAC_JDK_SRC") {
        return std::path::Path::new(&path).exists();
    }
    std::env::var_os("JAVA_HOME")
        .map(|home| {
            std::path::Path::new(&home)
                .join("lib")
                .join("src.zip")
                .exists()
        })
        .unwrap_or(false)
}

// Completion right pane: project source Javadoc.
#[test]
fn t67_completion_shows_project_javadoc() {
    let mut s = session();
    let items = items_with_cursor(
        &mut s,
        "T67",
        "/** Greeter docs. */\nclass T67 {\n    /** Adds one. */\n    int plus(int x) { return x + 1; }\n    void f() { this.| }\n}\n",
    );
    let plus = items.iter().find(|item| item.label == "plus").expect("plus");
    assert!(
        plus.documentation
            .as_deref()
            .unwrap_or("")
            .contains("Adds one."),
        "member documentation: {:?}",
        plus.documentation
    );

    let class_items = items_with_cursor(
        &mut s,
        "T67",
        "/** Greeter docs. */\nclass T67 { void f() { T6| } }\n",
    );
    let class_item = class_items
        .iter()
        .find(|item| item.label == "T67")
        .expect("class T67");
    assert!(
        class_item
            .documentation
            .as_deref()
            .unwrap_or("")
            .contains("Greeter docs."),
        "class documentation: {:?}",
        class_item.documentation
    );

    let direct_items = items_with_cursor(
        &mut s,
        "T67",
        "/** Greeter docs. */\nclass T67 {\n    /** Adds one. */\n    int plus(int x) { return x + 1; }\n    void f() { pl| }\n}\n",
    );
    let direct = direct_items
        .iter()
        .find(|item| item.label == "plus")
        .expect("direct plus");
    assert!(
        direct
            .documentation
            .as_deref()
            .unwrap_or("")
            .contains("Adds one."),
        "file method documentation: {:?}",
        direct.documentation
    );
}

// Completion right pane: JDK source Javadoc (src.zip).
#[test]
fn t68_completion_shows_jdk_javadoc() {
    if !jdk_src_available() {
        return;
    }
    let mut s = session();
    s.set_completion_show_constructors(true);
    let items = items_with_cursor(
        &mut s,
        "T68",
        "class T68 { void f() { java.util.Base64.| } }",
    );
    let decoder = items
        .iter()
        .find(|item| item.label == "getDecoder")
        .expect("getDecoder");
    let doc = decoder.documentation.as_deref().unwrap_or("");
    assert!(doc.to_ascii_lowercase().contains("decoder"), "doc: {doc:?}");

    let ctor_items = items_with_cursor(
        &mut s,
        "T68",
        "class T68 { void f() { new ArrayList| } }",
    );
    let ctor = ctor_items
        .iter()
        .find(|item| item.label.starts_with("ArrayList("))
        .expect("ArrayList constructor");
    assert!(
        ctor.documentation
            .as_deref()
            .unwrap_or("")
            .contains("Resizable-array"),
        "class documentation: {:?}",
        ctor.documentation
    );
}

#[test]
fn t65_package_completion_keeps_nested_types() {
    let mut s = session();
    // First request: package member completion enters java.util's classes.
    let first = labels_with_cursor(&mut s, "T65", "class T65 { void f() { java.util.| } }");
    assert!(has(&first, "Base64"), "package members: {first:?}");
    // Second request: a qualified class must still expand its nested types.
    let second = labels_with_cursor(&mut s, "T65", "class T65 { void f() { java.util.Base64| } }");
    assert!(has(&second, "Decoder"), "nested Decoder lost: {second:?}");
    assert!(has(&second, "Encoder"), "nested Encoder lost: {second:?}");
    assert!(has(&second, "getDecoder"), "static member lost: {second:?}");
}
