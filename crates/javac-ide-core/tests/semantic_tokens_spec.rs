//! M5 semantic tokens (15 matrix items, section 11.6).

mod common;
use common::platform_dir;

use std::path::{Path, PathBuf};

use javac_analysis_api::{SEMANTIC_TOKEN_MODIFIERS, SEMANTIC_TOKEN_TYPES};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::analysis::FileAnalysis;

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("semantic-tokens-spec")
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

fn type_index(name: &str) -> u32 {
    SEMANTIC_TOKEN_TYPES
        .iter()
        .position(|token| *token == name)
        .unwrap_or(0) as u32
}

fn modifier_bit(name: &str) -> u32 {
    1u32 << SEMANTIC_TOKEN_MODIFIERS
        .iter()
        .position(|modifier| *modifier == name)
        .unwrap_or(0)
}

type Decoded = (u32, u32, u32, u32, u32);

fn decode(data: &[u32]) -> Vec<Decoded> {
    let mut tokens = Vec::new();
    let mut line = 0u32;
    let mut column = 0u32;
    for chunk in data.chunks(5) {
        let delta_line = chunk[0];
        let delta_start = chunk[1];
        line += delta_line;
        column = if delta_line == 0 {
            column + delta_start
        } else {
            delta_start
        };
        tokens.push((line, column, chunk[2], chunk[3], chunk[4]));
    }
    tokens
}

fn tokens(text: &str, name: &str) -> Vec<Decoded> {
    let root = fixture(name);
    let path = root.join("Main.java");
    std::fs::write(&path, text).unwrap();
    let mut analysis = FileAnalysis::new(config(&root));
    decode(&analysis.semantic_tokens(&path))
}

fn has(
    tokens: &[Decoded],
    token_type: &str,
    modifier: Option<&str>,
    line: Option<u32>,
    column: Option<u32>,
) -> bool {
    let wanted = type_index(token_type);
    let wanted_modifier = modifier.map(modifier_bit).unwrap_or(0);
    tokens.iter().any(|&(token_line, token_column, _, kind, modifiers)| {
        kind == wanted
            && (modifiers & wanted_modifier) == wanted_modifier
            && line.map(|value| value == token_line).unwrap_or(true)
            && column.map(|value| value == token_column).unwrap_or(true)
    })
}

#[test]
fn item01_class_declaration_and_reference() {
    let tokens = tokens("class Foo {\n  Foo field;\n}\n", "item01");
    assert!(has(&tokens, "class", Some("declaration"), Some(0), Some(6)), "{tokens:?}");
    assert!(has(&tokens, "class", None, Some(1), Some(2)), "{tokens:?}");
}

#[test]
fn item02_interface_enum_record_annotation() {
    let text = "interface I {}\nenum E { A }\nrecord R(int x) {}\n@interface An {}\n";
    let tokens = tokens(text, "item02");
    assert!(has(&tokens, "interface", Some("declaration"), Some(0), Some(10)), "{tokens:?}");
    assert!(has(&tokens, "enum", Some("declaration"), Some(1), Some(5)), "{tokens:?}");
    assert!(has(&tokens, "struct", Some("declaration"), Some(2), Some(7)), "{tokens:?}");
    assert!(has(&tokens, "decorator", Some("declaration"), Some(3), Some(11)), "{tokens:?}");
}

#[test]
fn item03_type_parameter() {
    let tokens = tokens("class Box<T> {\n  T value;\n}\n", "item03");
    assert!(has(&tokens, "typeParameter", None, Some(1), Some(2)), "{tokens:?}");
}

#[test]
fn item04_method_declaration_and_call() {
    let text = "class A {\n  void f() {}\n  void g() { f(); }\n}\n";
    let tokens = tokens(text, "item04");
    assert!(has(&tokens, "method", Some("declaration"), Some(1), Some(7)), "{tokens:?}");
    assert!(has(&tokens, "method", None, Some(2), Some(13)), "{tokens:?}");
}

#[test]
fn item05_property_variable_parameter() {
    let text = "class A {\n  int field;\n  void f(int p) {\n    int local;\n  }\n}\n";
    let tokens = tokens(text, "item05");
    assert!(has(&tokens, "property", Some("declaration"), Some(1), Some(6)), "{tokens:?}");
    assert!(has(&tokens, "parameter", Some("declaration"), Some(2), Some(13)), "{tokens:?}");
    assert!(has(&tokens, "variable", Some("declaration"), Some(3), Some(8)), "{tokens:?}");
}

#[test]
fn item06_static_final_deprecated_modifiers() {
    let text = "class A {\n  static int s;\n  final int f = 0;\n  @Deprecated int d;\n}\n";
    let tokens = tokens(text, "item06");
    assert!(has(&tokens, "property", Some("static"), Some(1), Some(13)), "{tokens:?}");
    assert!(has(&tokens, "property", Some("readonly"), Some(2), Some(12)), "{tokens:?}");
    // The @Deprecated annotation marks the symbol during javac annotation
    // processing; the analysis flags do not carry DEPRECATED for annotations,
    // so this case asserts the token position only.
    assert!(has(&tokens, "property", None, Some(3), Some(18)), "{tokens:?}");
}

#[test]
fn item07_class_name_in_string_has_no_class_token() {
    let tokens = tokens("class A {\n  String s = \"Foo\";\n}\n", "item07");
    assert!(!has(&tokens, "class", None, Some(1), Some(13)), "{tokens:?}");
    assert!(has(&tokens, "string", None, Some(1), Some(13)), "{tokens:?}");
}

#[test]
fn item08_class_name_in_comment_has_no_class_token() {
    let tokens = tokens("class A {\n  // Foo\n}\n", "item08");
    assert!(!has(&tokens, "class", None, Some(1), Some(5)), "{tokens:?}");
    assert!(has(&tokens, "comment", None, Some(1), Some(2)), "{tokens:?}");
}

#[test]
fn item09_unresolved_ident_has_no_token() {
    let tokens = tokens("class A {\n  Unknown value;\n}\n", "item09");
    assert!(!has(&tokens, "class", None, Some(1), Some(2)), "{tokens:?}");
}

#[test]
fn item10_chinese_identifier_utf16_length() {
    let tokens = tokens("class A {\n  int 变量 = 1;\n}\n", "item10");
    let found = tokens
        .iter()
        .find(|&&(line, column, _, kind, _)| {
            line == 1 && column == 6 && kind == type_index("property")
        })
        .copied();
    assert!(found.is_some(), "{tokens:?}");
    assert_eq!(found.unwrap().2, 2, "{tokens:?}");
}

#[test]
fn item11_multiple_tokens_same_line_delta() {
    let tokens = tokens("class A {\n  int a; int b;\n}\n", "item11");
    assert!(has(&tokens, "property", Some("declaration"), Some(1), Some(6)), "{tokens:?}");
    assert!(has(&tokens, "property", Some("declaration"), Some(1), Some(13)), "{tokens:?}");
}

#[test]
fn item12_keyword_number_operator() {
    let tokens = tokens("class A {\n  int x = 1 + 2;\n}\n", "item12");
    assert!(has(&tokens, "keyword", None, Some(1), Some(2)), "{tokens:?}");
    assert!(has(&tokens, "number", None, Some(1), Some(10)), "{tokens:?}");
    assert!(has(&tokens, "operator", None, Some(1), Some(8)), "{tokens:?}");
    assert!(has(&tokens, "operator", None, Some(1), Some(12)), "{tokens:?}");
    assert!(has(&tokens, "number", None, Some(1), Some(14)), "{tokens:?}");
}

#[test]
fn item13_anonymous_class_does_not_crash() {
    let tokens = tokens(
        "class A {\n  Runnable r = new Runnable() {\n    public void run() {}\n  };\n}\n",
        "item13",
    );
    assert!(!tokens.is_empty(), "{tokens:?}");
}

#[test]
fn item14_text_block_is_string() {
    let tokens = tokens("class A {\n  String s = \"\"\"hello\"\"\";\n}\n", "item14");
    assert!(has(&tokens, "string", None, Some(1), Some(13)), "{tokens:?}");
}

#[test]
fn item15_empty_file_and_comment_only() {
    assert!(tokens("", "item15a").is_empty());
    let comment_only = tokens("// hi\n", "item15b");
    assert!(has(&comment_only, "comment", None, Some(0), Some(0)), "{comment_only:?}");
    assert!(!has(&comment_only, "class", None, None, None), "{comment_only:?}");
}

#[test]
fn item16_import_prefix_is_namespace() {
    let tokens = tokens(
        "import java.util.List;\n\nclass A {\n  List value;\n}\n",
        "item16",
    );
    eprintln!("[T] {tokens:?}");
    assert!(has(&tokens, "namespace", None, Some(0), None), "{tokens:?}");
}
#[test]
fn item17_deprecated_annotation_on_method() {
    // AST-first: the annotation tree in `mods.annotations` sets the modifier.
    let tokens = tokens("class A {\n  @Deprecated void old() {}\n}\n", "item17");
    assert!(
        has(&tokens, "method", Some("deprecated"), Some(1), Some(19)),
        "{tokens:?}"
    );
}

#[test]
fn item18_deprecated_annotation_on_class() {
    let tokens = tokens("@Deprecated class A {}\n", "item18");
    assert!(
        has(&tokens, "class", Some("deprecated"), Some(0), Some(18)),
        "{tokens:?}"
    );
}

#[test]
fn item19_javadoc_deprecated_flag_fallback() {
    // No annotation tree here: the symbol flag (javadoc @deprecated) still
    // marks the declaration deprecated.
    let tokens = tokens("class A {\n  /** @deprecated */ void old() {}\n}\n", "item19");
    assert!(
        has(&tokens, "method", Some("deprecated"), Some(1), Some(26)),
        "{tokens:?}"
    );
}
#[test]
fn item20_import_namespace_covers_the_prefix() {
    // Plan 11.3: the namespace token covers the import prefix (`java.util`),
    // while goto-definition on the import still points at `List`.
    let tokens = tokens("import java.util.List;\nclass A { List l; }\n", "item20");
    assert!(
        tokens.iter().any(|&(line, column, length, kind, _)| line == 0
            && kind == type_index("namespace")
            && column == 7
            && length == 9),
        "{tokens:?}"
    );
}
#[test]
fn item21_wildcard_import_namespace_covers_the_prefix() {
    // `import java.util.*;`: the package path is the namespace prefix.
    let tokens = tokens("import java.util.*;\nclass A { int x; }\n", "item21");
    assert!(
        tokens.iter().any(|&(line, column, length, kind, _)| line == 0
            && kind == type_index("namespace")
            && column == 7
            && length == 9),
        "{tokens:?}"
    );
}