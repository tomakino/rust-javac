//! `super(...)` constructor navigation: project sources (same file and cross
//! file), JDK `src.zip`, and the neighbouring `this(...)` / `super.m()` forms.

use std::path::{Path, PathBuf};

use javac_analysis_api::{Location, Position, WorkspaceService};

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

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

fn fixture(name: &str) -> PathBuf {
    let root = repo_root()
        .join("target")
        .join("tmp")
        .join("super-constructor-spec")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn config(root: &Path, source_path: Vec<PathBuf>) -> AnalysisConfig {
    platform_dir();
    AnalysisConfig {
        class_path: Vec::new(),
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path,
    }
}

fn write(root: &Path, name: &str, text: &str) -> PathBuf {
    let path = root.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, text).unwrap();
    path
}

fn position(text: &str, needle: &str) -> Position {
    let byte = text.find(needle).expect("needle");
    let prefix = &text[..byte];
    let line = prefix.matches('\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    let column = text[line_start..byte]
        .chars()
        .map(|ch| ch.len_utf16() as u32)
        .sum();
    Position::new(line, column)
}

fn line_text(file: &Path, line: u32) -> String {
    std::fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .nth(line as usize)
        .unwrap_or_default()
        .to_string()
}

fn definition_at(
    workspace: &mut Workspace,
    path: &Path,
    text: &str,
    needle: &str,
) -> Location {
    workspace
        .definition(path, position(text, needle))
        .unwrap_or_else(|| panic!("no definition for {needle:?}"))
}

/// The `this(...)` / `super.m()` forms keep working: they must not be
/// hijacked by the constructor-call branch.
#[test]
fn this_call_still_jumps_to_the_sibling_constructor() {
    let root = fixture("this-call");
    let text = "class A {\n    A() { this(1); }\n    A(int x) {}\n}\n";
    let path = write(&root, "A.java", text);
    let mut workspace = Workspace::new(config(&root, Vec::new()));
    let definition = definition_at(&mut workspace, &path, text, "this(1)");
    assert!(definition.file.ends_with("A.java"), "{definition:?}");
    let line = line_text(&definition.file, definition.range.start.line);
    assert!(line.contains("A(int x)"), "{line:?}");
    assert_eq!(
        definition.range.start.column,
        line.find("A(int x)").unwrap() as u32,
        "the cursor must land on the constructor name: {definition:?}"
    );
}

#[test]
fn super_method_call_jumps_to_the_parent_method() {
    let root = fixture("super-method");
    let text = "class A { void f() {} }\nclass B extends A { B() { super.f(); } }\n";
    let path = write(&root, "B.java", text);
    let mut workspace = Workspace::new(config(&root, Vec::new()));
    let start = position(text, "super.f");
    let cursor = Position::new(start.line, start.column + "super.".len() as u32);
    let definition = workspace
        .definition(&path, cursor)
        .expect("definition of f");
    assert!(definition.file.ends_with("B.java"), "{definition:?}");
    let line = line_text(&definition.file, definition.range.start.line);
    assert!(line.contains("void f()"), "{line:?}");
    assert_eq!(
        definition.range.start.column,
        line.find("f()").unwrap() as u32,
        "the cursor must land on the parent method name: {definition:?}"
    );
}

#[test]
fn project_super_call_jumps_to_the_parent_constructor() {
    let root = fixture("project-no-arg");
    let text = "class A { A() {} }\nclass B extends A { B() { super(); } }\n";
    let path = write(&root, "B.java", text);
    let mut workspace = Workspace::new(config(&root, Vec::new()));
    let definition = definition_at(&mut workspace, &path, text, "super();");
    assert!(definition.file.ends_with("B.java"), "{definition:?}");
    let line = line_text(&definition.file, definition.range.start.line);
    assert!(line.contains("class A { A() {} }"), "{line:?}");
    assert_eq!(
        definition.range.start.column,
        line.find("A()").unwrap() as u32,
        "the cursor must land on the A() constructor: {definition:?}"
    );
}

#[test]
fn project_super_call_with_arguments_matches_the_overload() {
    let root = fixture("project-arity");
    let text = "class A { A() {} A(int x) {} }\nclass B extends A { B() { super(1); } }\n";
    let path = write(&root, "B.java", text);
    let mut workspace = Workspace::new(config(&root, Vec::new()));
    let definition = definition_at(&mut workspace, &path, text, "super(1);");
    assert!(definition.file.ends_with("B.java"), "{definition:?}");
    let line = line_text(&definition.file, definition.range.start.line);
    assert!(line.contains("A(int x)"), "{line:?}");
    assert_eq!(
        definition.range.start.column,
        line.find("A(int x)").unwrap() as u32,
        "the cursor must land on A(int): {definition:?}"
    );
}

#[test]
fn cross_file_super_call_jumps_to_the_parent_constructor() {
    let root = fixture("cross-file");
    let a_text = "class A { A() {} A(int x) {} }\n";
    let b_text = "class B extends A { B() { super(1); } }\n";
    write(&root, "A.java", a_text);
    let b_path = write(&root, "B.java", b_text);
    let mut workspace = Workspace::new(config(&root, vec![root.clone()]));
    workspace.wait_for_index();
    let definition = definition_at(&mut workspace, &b_path, b_text, "super(1);");
    assert!(
        definition.file.ends_with("A.java"),
        "expected A.java, got {definition:?}"
    );
    assert_eq!(definition.range.start.line, 0, "{definition:?}");
    assert_eq!(
        definition.range.start.column,
        a_text.find("A(int x)").unwrap() as u32,
        "the cursor must land on A(int): {definition:?}"
    );
}

fn jdk_src_zip() -> Option<PathBuf> {
    if let Some(platform) = std::env::var_os("RAVAC_PLATFORM_DIR") {
        let path = PathBuf::from(platform).join("src.zip");
        if path.is_file() {
            return Some(path);
        }
    }
    if let Some(path) = std::env::var_os("RAVAC_JDK_SRC") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }
    std::env::var_os("JAVA_HOME")
        .map(|home| PathBuf::from(home).join("lib").join("src.zip"))
        .filter(|path| path.is_file())
}

/// The extracted `java.lang.NullPointerException` source used by the reported
/// scenario; the JDK `src.zip` is the fallback.
fn null_pointer_exception_source(fixture: &Path) -> Option<PathBuf> {
    let existing = repo_root()
        .join("target/debug/ravac-lsp-out/jdk-src/java.base/java/lang/NullPointerException.java");
    if existing.is_file() {
        return Some(existing);
    }
    let zip = jdk_src_zip()?;
    let archive = javac_zip::ZipArchive::open(&zip).ok()?;
    let entry = archive
        .entries()
        .iter()
        .map(|entry| entry.name().to_string())
        .find(|name| name.ends_with("/java/lang/NullPointerException.java"))?;
    let bytes = archive.get_entry(&entry)?;
    let path = fixture.join("NullPointerException.java");
    std::fs::write(&path, &bytes).ok()?;
    Some(path)
}

#[test]
fn jdk_super_call_jumps_to_the_parent_constructor() {
    let root = fixture("jdk-parent");
    let Some(path) = null_pointer_exception_source(&root) else {
        return; // no JDK sources in this environment
    };
    let text = std::fs::read_to_string(&path).unwrap();
    let mut workspace = Workspace::new(config(&root, Vec::new()));
    let definition = definition_at(&mut workspace, &path, &text, "super();");
    assert!(
        definition.file.ends_with("RuntimeException.java"),
        "expected RuntimeException.java, got {definition:?}"
    );
    let line = line_text(&definition.file, definition.range.start.line);
    assert!(
        line.contains("RuntimeException()"),
        "expected the no-arg constructor, got line {line:?}"
    );
    assert_eq!(
        definition.range.start.column,
        line.find("RuntimeException").unwrap() as u32,
        "the cursor must land on the constructor name: {definition:?}"
    );
}
