//! D1: definition/hover for variables and shadowing (compiler-symbol based).
//!
//! Complements `scripts/lsp/probe_d1_variables.py` (the real LSP transcript):
//! every scenario here goes through the library API and asserts the cursor
//! lands on the declaration the shadowing rules select.

use std::path::{Path, PathBuf};

use javac_analysis_api::{Location, Position, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::Workspace;

const TEXT: &str = r#"import java.util.List;
import static java.util.Collections.emptyList;

class Base {
    int x;
    int baseOnly;
}

class D1Probe extends Base implements Runnable {
    int x;
    int e;
    int s;
    List<String> list;
    List<String> items;

    public void run() {
    }

    void localShadow() {
        int x = 1;
        int y = x;
    }

    void paramShadow(String x) {
        use(x);
    }

    void catchShadow() {
        try {
            use(null);
        } catch (Exception e) {
            use(e);
        }
    }

    void forEachShadow() {
        for (String s : list) {
            use(s);
        }
    }

    void lambdaShadow() {
        items.forEach(s -> use(s));
    }

    void methodRef() {
        items.forEach(this::use);
    }

    void thisVsBare(int x) {
        int z = x;
        this.x = 1;
    }

    void superField() {
        int v = super.x;
    }

    void use(Object o) {
    }
}

enum E {
    A,
    B
}

class EnumUse {
    E e = E.A;
}

record P(int x) {
    int twice() {
        return x * 2;
    }
}

class Box<T> {
    T value;

    T get() {
        return value;
    }
}
"#;

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

fn workspace_with(name: &str) -> (Workspace, PathBuf) {
    platform_dir();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("d1-variables-spec")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("D1Probe.java");
    std::fs::write(&path, TEXT).unwrap();
    let mut workspace = Workspace::new(AnalysisConfig {
        class_path: Vec::new(),
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: Vec::new(),
    });
    workspace.set_overlay(&path, TEXT);
    (workspace, path)
}

fn position(text: &str, marker: &str, offset: usize) -> Position {
    let start = text.find(marker).expect("marker") + offset;
    let prefix = &text[..start];
    let line = prefix.matches('\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    let column = text[line_start..start].chars().map(|ch| ch.len_utf16() as u32).sum();
    Position::new(line, column)
}

fn definition(workspace: &mut Workspace, path: &Path, marker: &str, offset: usize) -> Location {
    let cursor = position(TEXT, marker, offset);
    workspace
        .definition(path, cursor)
        .unwrap_or_else(|| panic!("no definition at {marker:?}+{offset}"))
}

fn hover(workspace: &mut Workspace, path: &Path, marker: &str, offset: usize) -> String {
    let cursor = position(TEXT, marker, offset);
    workspace
        .hover(path, cursor)
        .unwrap_or_else(|| panic!("no hover at {marker:?}+{offset}"))
}

fn assert_definition_at(
    location: &Location,
    text: &str,
    marker: &str,
    offset: usize,
    what: &str,
) {
    assert!(location.file.ends_with("D1Probe.java"), "{what}: {location:?}");
    let expected = position(text, marker, offset);
    assert_eq!(location.range.start.line, expected.line, "{what}: {location:?}");
    assert_eq!(
        location.range.start.column, expected.column,
        "{what}: {location:?}"
    );
}

#[test]
fn local_variable_shadows_field() {
    let (mut workspace, path) = workspace_with("local");
    let definition = definition(&mut workspace, &path, "int y = x;", 8);
    assert_definition_at(&definition, TEXT, "int x = 1;", 4, "local x");
    assert!(hover(&mut workspace, &path, "int y = x;", 8).contains("int x"));
}

#[test]
fn parameter_shadows_field() {
    let (mut workspace, path) = workspace_with("parameter");
    let definition = definition(&mut workspace, &path, "use(x);", 4);
    assert_definition_at(
        &definition,
        TEXT,
        "paramShadow(String x) {",
        "paramShadow(String ".len(),
        "parameter x",
    );
    assert!(hover(&mut workspace, &path, "use(x);", 4).contains("String x"));
}

#[test]
fn catch_parameter() {
    let (mut workspace, path) = workspace_with("catch");
    let definition = definition(&mut workspace, &path, "use(e);", 4);
    assert_definition_at(
        &definition,
        TEXT,
        "catch (Exception e) {",
        "catch (Exception ".len(),
        "catch e",
    );
    assert!(hover(&mut workspace, &path, "use(e);", 4).contains("Exception e"));
}

#[test]
fn enhanced_for_variable() {
    let (mut workspace, path) = workspace_with("for");
    let definition = definition(&mut workspace, &path, "use(s);", 4);
    assert_definition_at(
        &definition,
        TEXT,
        "for (String s : list) {",
        "for (String ".len(),
        "for s",
    );
    assert!(hover(&mut workspace, &path, "use(s);", 4).contains("String s"));
}

#[test]
fn lambda_parameter_shadows_field() {
    let (mut workspace, path) = workspace_with("lambda");
    let definition = definition(&mut workspace, &path, "use(s))", 4);
    assert_definition_at(&definition, TEXT, "forEach(s ->", "forEach(".len(), "lambda s");
    assert!(hover(&mut workspace, &path, "use(s))", 4).contains("String s"));
}

#[test]
fn method_reference_resolves_the_method() {
    let (mut workspace, path) = workspace_with("method-ref");
    let definition = definition(&mut workspace, &path, "this::use", 6);
    assert_definition_at(&definition, TEXT, "void use(Object o) {", 5, "use");
    assert!(hover(&mut workspace, &path, "this::use", 6).contains("use"));
}

#[test]
fn this_field_differs_from_the_bare_parameter() {
    let (mut workspace, path) = workspace_with("this-field");
    let field = definition(&mut workspace, &path, "this.x = 1;", 5);
    assert_definition_at(&field, TEXT, "int x;\n    int e;", 4, "this.x field");
    let bare = definition(&mut workspace, &path, "int z = x;", 8);
    assert_definition_at(
        &bare,
        TEXT,
        "thisVsBare(int x) {",
        "thisVsBare(int ".len(),
        "bare x",
    );
    assert!(hover(&mut workspace, &path, "this.x = 1;", 5).contains("int x"));
}

#[test]
fn super_field_jumps_to_the_parent() {
    let (mut workspace, path) = workspace_with("super-field");
    let definition = definition(&mut workspace, &path, "int v = super.x;", "int v = super.".len());
    assert_definition_at(&definition, TEXT, "int x;\n    int baseOnly;", 4, "super.x");
    assert!(hover(&mut workspace, &path, "int v = super.x;", "int v = super.".len())
        .contains("int x"));
}

#[test]
fn enum_constant_reference_jumps_to_the_constant() {
    let (mut workspace, path) = workspace_with("enum");
    let definition = definition(&mut workspace, &path, "= E.A;", 4);
    assert_definition_at(&definition, TEXT, "enum E {", 13, "enum constant A");
    assert!(hover(&mut workspace, &path, "= E.A;", 4).contains("A"));
}

#[test]
fn record_component_reference_jumps_to_the_component() {
    let (mut workspace, path) = workspace_with("record");
    let definition = definition(&mut workspace, &path, "return x * 2;", 7);
    assert_definition_at(
        &definition,
        TEXT,
        "record P(int x)",
        "record P(int ".len(),
        "record component x",
    );
    assert!(hover(&mut workspace, &path, "return x * 2;", 7).contains("int x"));
}

#[test]
fn generic_type_parameter_reference() {
    let (mut workspace, path) = workspace_with("generic");
    let definition = definition(&mut workspace, &path, "T value;", 0);
    assert_definition_at(&definition, TEXT, "class Box<T>", "class Box<".len(), "type parameter T");
    assert!(hover(&mut workspace, &path, "T value;", 0).contains("T"));
}

fn jdk_src_available() -> bool {
    if let Some(path) = std::env::var_os("RAVAC_JDK_SRC") {
        return PathBuf::from(path).exists();
    }
    std::env::var_os("JAVA_HOME")
        .map(|home| PathBuf::from(home).join("lib").join("src.zip").is_file())
        .unwrap_or(false)
}

#[test]
fn static_import_member_resolves_to_its_declaration() {
    if !jdk_src_available() {
        return; // no JDK sources in this environment
    }
    let (mut workspace, path) = workspace_with("static-import");
    let cursor = position(TEXT, "Collections.emptyList;", "Collections.".len());
    let definition = workspace
        .definition(&path, cursor)
        .expect("static import definition");
    assert!(
        definition.file.ends_with("Collections.java"),
        "expected Collections.java, got {definition:?}"
    );
    let hover = hover(&mut workspace, &path, "Collections.emptyList;", "Collections.".len());
    assert!(hover.contains("emptyList"), "{hover}");
}
