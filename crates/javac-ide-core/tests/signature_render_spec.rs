//! Method signature rendering (hover, completion, signature help, outline).

use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, WorkspaceService};

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

fn session() -> Workspace {
    platform_dir();
    let output = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("signature-render");
    Workspace::new(AnalysisConfig {
        class_path: Vec::new(),
        output_dir: output,
        source_level: "17".to_string(),
        source_path: Vec::new(),
    })
}

fn overlay(session: &mut Workspace, name: &str, source: &str) -> PathBuf {
    let path = PathBuf::from(format!("target/tmp/signature-render/{name}.java"));
    std::fs::create_dir_all("target/tmp/signature-render").unwrap();
    std::fs::write(&path, source).unwrap();
    WorkspaceService::diagnostics(session, &path, Some(source));
    path
}

fn position_of(text: &str, marker: &str, offset: usize) -> Position {
    let start = text
        .find(marker)
        .unwrap_or_else(|| panic!("marker not found: {marker}"))
        + offset;
    let prefix = &text[..start];
    let line = prefix.matches('\n').count() as u32;
    let column: u32 = prefix
        .rsplit('\n')
        .next()
        .unwrap()
        .encode_utf16()
        .count() as u32;
    Position::new(line, column)
}

fn cursor_removed(source: &str) -> (String, Position) {
    let cursor = source.find('|').expect("cursor marker");
    let text = source.replacen('|', "", 1);
    let prefix = &text[..cursor];
    let line = prefix.matches('\n').count() as u32;
    let column: u32 = prefix
        .rsplit('\n')
        .next()
        .unwrap()
        .encode_utf16()
        .count() as u32;
    (text, Position::new(line, column))
}

#[test]
fn hover_and_outline_render_standard_signatures() {
    let source = r#"class Sig {
    Sig() {
    }

    Sig(int value) {
    }

    String newJsonReader(java.io.Reader reader) {
        return "";
    }

    <T> T identity(T value) {
        return value;
    }

    void print(String... args) {
    }

    void noArgs() {
    }

    void use() {
        identity(1);
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "Sig", source);

    let cases = [
        ("newJsonReader", "String newJsonReader(Reader reader)"),
        ("identity", "<T> T identity(T value)"),
        ("print", "void print(String... args)"),
        ("noArgs", "void noArgs()"),
        ("Sig(int value)", "Sig(int value)"),
    ];
    for (marker, expected) in cases {
        let hover = workspace
            .hover(&path, position_of(source, marker, 0))
            .unwrap_or_else(|| panic!("no hover at {marker}"));
        assert!(
            hover.contains(expected),
            "hover at {marker}: expected {expected:?}, got {hover:?}"
        );
    }
}

#[test]
fn completion_detail_and_signature_help_regression() {
    let mut workspace = session();
    let completion_source = "class SigC { <T> T identity(T value) { return value; } void print(String... args) { } void f() { this.ident| } }";
    let (text, position) = cursor_removed(completion_source);
    let path = overlay(&mut workspace, "SigC", &text);
    let items = workspace.completion(&path, position);
    let identity = items
        .iter()
        .find(|item| item.label == "identity")
        .unwrap_or_else(|| panic!("identity candidate: {:?}", items.iter().map(|item| &item.label).collect::<Vec<_>>()));
    assert_eq!(identity.detail.as_deref(), Some("T"));
    assert_eq!(identity.label_detail.as_deref(), Some("(T value)"));
    let second_source = "class SigP { void print(String... args) { } void f() { this.pri| } }";
    let (text, position) = cursor_removed(second_source);
    let path = overlay(&mut workspace, "SigP", &text);
    let items = workspace.completion(&path, position);
    let print = items
        .iter()
        .find(|item| item.label == "print")
        .unwrap_or_else(|| panic!("print candidate: {:?}", items.iter().map(|item| &item.label).collect::<Vec<_>>()));
    assert_eq!(print.detail.as_deref(), Some("void"));
    assert_eq!(print.label_detail.as_deref(), Some("(String... args)"));

    let help_source = "class SigS {\n    void single(int a, String b) {\n    }\n\n    void f() {\n        single(1, )\n    }\n}\n";
    let path = overlay(&mut workspace, "SigS", help_source);
    let position = position_of(help_source, "single(1, ", "single(1, ".len());
    let help = workspace
        .signature_help(&path, position)
        .expect("signature help");
    let labels: Vec<String> = help
        .signatures
        .iter()
        .map(|signature| signature.label.clone())
        .collect();
    assert!(
        labels.iter().any(|label| label == "void single(int a, String b)"),
        "signature labels: {labels:?}"
    );
}
