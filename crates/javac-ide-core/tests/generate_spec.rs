//! M6 generate code action: constructor / toString / equals+hashCode (14.4).

mod common;
use common::platform_dir;

use std::path::{Path, PathBuf};

use javac_analysis_api::Position;

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::analysis::FileAnalysis;

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("generate-spec")
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

fn generate_actions(text: &str, name: &str) -> Vec<javac_analysis_api::CodeAction> {
    let root = fixture(name);
    let path = root.join("Main.java");
    std::fs::write(&path, text).unwrap();
    let mut analysis = FileAnalysis::new(config(&root));
    analysis.diagnostics(&path, None);
    analysis
        .code_actions(&path, Position::new(1, 0))
        .into_iter()
        .filter(|action| action.title.starts_with("Generate"))
        .collect()
}

fn find<'a>(
    actions: &'a [javac_analysis_api::CodeAction],
    title: &str,
) -> Option<&'a javac_analysis_api::CodeAction> {
    actions.iter().find(|action| action.title == title)
}

#[test]
fn item01_no_fields_no_generate() {
    let actions = generate_actions("class Foo {\n}\n", "item01");
    assert!(actions.is_empty(), "{actions:?}");
}

#[test]
fn item02_single_field_constructor_char_by_char() {
    let actions = generate_actions("class Foo {\n  int a;\n}\n", "item02");
    let constructor = find(&actions, "Generate constructor").expect("constructor action");
    assert_eq!(constructor.edits.len(), 1, "{constructor:?}");
    assert_eq!(
        constructor.edits[0].new_text,
        "    public Foo(int a) {\n        this.a = a;\n    }\n"
    );
    assert_eq!(constructor.edits[0].range.start.line, 2);
    assert_eq!(constructor.edits[0].range.start.column, 0);
}

#[test]
fn item03_multiple_fields_char_by_char() {
    let actions = generate_actions("class Foo {\n  int a;\n  String b;\n}\n", "item03");
    let constructor = find(&actions, "Generate constructor").expect("constructor action");
    assert_eq!(
        constructor.edits[0].new_text,
        "    public Foo(int a, String b) {\n        this.a = a;\n        this.b = b;\n    }\n"
    );
    let to_string = find(&actions, "Generate toString()").expect("toString action");
    assert_eq!(
        to_string.edits[0].new_text,
        "    @Override\n    public String toString() {\n        return \"Foo{\" +\n            \"a=\" + a +\n            \", b=\" + b +\n        '}';\n    }\n"
    );
    let equals = find(&actions, "Generate equals() and hashCode()").expect("equals action");
    assert_eq!(equals.edits.len(), 2, "{equals:?}");
    assert!(equals.edits[0].new_text.contains("import java.util.Objects;"), "{equals:?}");
    assert_eq!(
        equals.edits[1].new_text,
        "    @Override\n    public boolean equals(Object o) {\n        if (this == o) return true;\n        if (!(o instanceof Foo)) return false;\n        Foo other = (Foo) o;\n        return Objects.equals(a, other.a) && Objects.equals(b, other.b);\n    }\n    @Override\n    public int hashCode() {\n        return Objects.hash(a, b);\n    }\n"
    );
}

#[test]
fn item04_existing_methods_conflict() {
    let text = "class Foo {\n  int a;\n  Foo(int a) {}\n  @Override public String toString() { return \"\"; }\n  @Override public boolean equals(Object o) { return false; }\n  @Override public int hashCode() { return 0; }\n}\n";
    let actions = generate_actions(text, "item04");
    assert!(find(&actions, "Generate constructor").is_none(), "{actions:?}");
    assert!(find(&actions, "Generate toString()").is_none(), "{actions:?}");
    assert!(find(&actions, "Generate equals() and hashCode()").is_none(), "{actions:?}");
}

#[test]
fn item05_superclass_without_no_arg_constructor() {
    let text = "class Base {\n  Base(int x) {}\n}\nclass Foo extends Base {\n  int a;\n}\n";
    let actions = generate_actions_at(text, "item05", Position::new(4, 2));
    assert!(find(&actions, "Generate constructor").is_none(), "{actions:?}");
    assert!(find(&actions, "Generate toString()").is_some(), "{actions:?}");
}

#[test]
fn item06_crlf_keeps_crlf() {
    let actions = generate_actions("class Foo {\r\n  int a;\r\n}\r\n", "item06");
    let constructor = find(&actions, "Generate constructor").expect("constructor action");
    assert!(
        constructor.edits[0].new_text.contains("\r\n"),
        "{constructor:?}"
    );
    assert!(
        !constructor.edits[0].new_text.contains("\n")
            || constructor.edits[0].new_text.matches("\r\n").count()
                == constructor.edits[0].new_text.matches('\n').count(),
        "{constructor:?}"
    );
}

fn generate_actions_at(
    text: &str,
    name: &str,
    position: Position,
) -> Vec<javac_analysis_api::CodeAction> {
    let root = fixture(name);
    let path = root.join("Main.java");
    std::fs::write(&path, text).unwrap();
    let mut analysis = FileAnalysis::new(config(&root));
    analysis.diagnostics(&path, None);
    analysis
        .code_actions(&path, position)
        .into_iter()
        .filter(|action| action.title.starts_with("Generate"))
        .collect()
}

#[test]
fn item07_field_declaration_order() {
    let actions = generate_actions("class Foo {\n  int z;\n  int a;\n}\n", "item07");
    let constructor = find(&actions, "Generate constructor").expect("constructor action");
    assert_eq!(
        constructor.edits[0].new_text,
        "    public Foo(int z, int a) {\n        this.z = z;\n        this.a = a;\n    }\n"
    );
}

#[test]
fn item08_cursor_class_in_multi_class_file() {
    let text = "class Base {\n  int b;\n}\nclass Foo {\n  int a;\n}\n";
    let actions = generate_actions_at(text, "item08", Position::new(4, 2));
    let constructor = find(&actions, "Generate constructor").expect("constructor action");
    assert_eq!(
        constructor.edits[0].new_text,
        "    public Foo(int a) {\n        this.a = a;\n    }\n"
    );
    assert!(actions.iter().all(|action| !action.title.contains("Base")), "{actions:?}");
}

#[test]
fn item09_different_signature_constructor_is_generated() {
    let text = "class Foo {\n  int a;\n  Foo(int a, int b) {}\n}\n";
    let actions = generate_actions(text, "item09");
    let constructor = find(&actions, "Generate constructor").expect("constructor action");
    assert_eq!(
        constructor.edits[0].new_text,
        "    public Foo(int a) {\n        this.a = a;\n    }\n"
    );
}
#[test]
fn item10_chinese_offsets_do_not_mix_units() {
    // A comment containing braces plus Chinese and an emoji before the class:
    // tree positions are UTF-16 while text scanning is byte based.
    let text = "// 中文 {😀}\nclass Foo {\n  int 变量;\n}\n";
    let actions = generate_actions(text, "item10");
    let constructor = find(&actions, "Generate constructor").expect("constructor action");
    assert_eq!(constructor.edits[0].range.start.line, 3, "{constructor:?}");
    assert_eq!(constructor.edits[0].range.start.column, 0, "{constructor:?}");
    assert_eq!(
        constructor.edits[0].new_text,
        "    public Foo(int 变量) {\n        this.变量 = 变量;\n    }\n"
    );
}
#[test]
fn item11_wildcard_import_covers_objects() {
    let text = "import java.util.*;\n\nclass Foo {\n  int a;\n}\n";
    let actions = generate_actions_at(text, "item11", Position::new(3, 2));
    let equals = find(&actions, "Generate equals() and hashCode()").expect("equals action");
    // java.util.* already makes Objects visible: no import edit.
    assert_eq!(equals.edits.len(), 1, "{equals:?}");
}

#[test]
fn item12_overloads_do_not_conflict() {
    let text = "class Foo {\n  int a;\n  public String toString(int x) { return \"\"; }\n  public boolean equals(String s) { return false; }\n  public int hashCode(int x) { return 0; }\n}\n";
    let actions = generate_actions_at(text, "item12", Position::new(3, 2));
    assert!(find(&actions, "Generate toString()").is_some(), "{actions:?}");
    assert!(find(&actions, "Generate equals() and hashCode()").is_some(), "{actions:?}");
}

#[test]
fn item13_annotation_with_braces_before_class() {
    let text = "@SuppressWarnings({\"unchecked\"})\nclass Foo {\n  int a;\n}\n";
    let actions = generate_actions_at(text, "item13", Position::new(2, 2));
    let constructor = find(&actions, "Generate constructor").expect("constructor action");
    assert_eq!(
        constructor.edits[0].new_text,
        "    public Foo(int a) {\n        this.a = a;\n    }\n"
    );
    assert_eq!(constructor.edits[0].range.start.line, 3, "{constructor:?}");
}
#[test]
fn item14_nested_class_cursor() {
    let text = "class Outer {\n  class Inner {\n    int a;\n  }\n}\n";
    let actions = generate_actions_at(text, "item14", Position::new(2, 4));
    let constructor = find(&actions, "Generate constructor").expect("constructor action");
    // The innermost class containing the cursor (Inner), not the outer one.
    // Inner is indented by 2, so its members indent by 6 and its body by 10.
    assert_eq!(
        constructor.edits[0].new_text,
        "      public Inner(int a) {\n          this.a = a;\n      }\n  "
    );
    assert_eq!(constructor.edits[0].range.start.line, 3, "{constructor:?}");
}
#[test]
fn item15_insert_after_last_member_before_trailing_comment() {
    // Plan 14.1: the generated member goes after the last member and before a
    // trailing comment line (the comment stays below it).
    let text = "class Foo {\n  int a;\n  // trailing\n}\n";
    let actions = generate_actions(text, "item15");
    let constructor = find(&actions, "Generate constructor").expect("constructor action");
    assert_eq!(
        constructor.edits[0].new_text,
        "    public Foo(int a) {\n        this.a = a;\n    }\n"
    );
    // Inserted on the line right after `int a;` (the comment line), empty range.
    assert_eq!(constructor.edits[0].range.start.line, 2, "{constructor:?}");
    assert_eq!(constructor.edits[0].range.start.column, 0, "{constructor:?}");
    assert_eq!(
        constructor.edits[0].range.end,
        constructor.edits[0].range.start,
        "{constructor:?}"
    );
}

#[test]
fn item16_insert_after_last_member_before_blank_line() {
    let text = "class Foo {\n  int a;\n\n}\n";
    let actions = generate_actions(text, "item16");
    let constructor = find(&actions, "Generate constructor").expect("constructor action");
    assert_eq!(
        constructor.edits[0].new_text,
        "    public Foo(int a) {\n        this.a = a;\n    }\n"
    );
    assert_eq!(constructor.edits[0].range.start.line, 2, "{constructor:?}");
    assert_eq!(constructor.edits[0].range.start.column, 0, "{constructor:?}");
    assert_eq!(
        constructor.edits[0].range.end,
        constructor.edits[0].range.start,
        "{constructor:?}"
    );
}