//! Super-method navigation: erased signatures, generics, varargs, bridges,
//! multiple interfaces, JDK and classpath sources.

mod common;
use common::platform_dir;

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
        .join("super-methods-spec")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn config(root: &Path) -> AnalysisConfig {
    config_with_class_path(root, Vec::new())
}

fn config_with_class_path(root: &Path, class_path: Vec<PathBuf>) -> AnalysisConfig {
    platform_dir();
    AnalysisConfig {
        class_path,
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: vec![root.to_path_buf()],
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

fn method_position(text: &str, marker: &str) -> Position {
    // `marker` is the text before the method name, e.g. "void ".
    let byte = text.find(marker).expect("method marker") + marker.len();
    let prefix = &text[..byte];
    let line = prefix.matches('\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    let column = text[line_start..byte]
        .chars()
        .map(|ch| ch.len_utf16() as u32)
        .sum();
    Position::new(line, column)
}

fn jdk_src() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("RAVAC_JDK_SRC") {
        return Some(PathBuf::from(path));
    }
    std::env::var_os("JAVA_HOME")
        .map(|home| PathBuf::from(home).join("lib").join("src.zip"))
        .filter(|path| path.exists())
}

fn java_tool(name: &str) -> Option<PathBuf> {
    let home = std::env::var_os("JAVA_HOME")?;
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let path = PathBuf::from(home).join("bin").join(file);
    path.exists().then_some(path)
}

/// Build `lib.jar` (and optionally `lib-sources.jar`) for external-parent tests.
fn build_external_lib(dir: &Path, with_sources: bool) -> Option<PathBuf> {
    let src = dir.join("libsrc").join("ext");
    std::fs::create_dir_all(&src).ok()?;
    std::fs::write(
        src.join("Base.java"),
        "package ext;\npublic class Base { public void f() {} }\n",
    )
    .ok()?;
    let classes = dir.join("libclasses");
    std::fs::create_dir_all(&classes).ok()?;
    let javac = java_tool("javac")?;
    let jar = java_tool("jar")?;
    let status = std::process::Command::new(javac)
        .arg("-d")
        .arg(&classes)
        .arg(src.join("Base.java"))
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    let lib = dir.join("lib.jar");
    let status = std::process::Command::new(&jar)
        .arg("cf")
        .arg(&lib)
        .arg("-C")
        .arg(&classes)
        .arg(".")
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    if with_sources {
        let sources = dir.join("lib-sources.jar");
        let status = std::process::Command::new(&jar)
            .arg("cf")
            .arg(&sources)
            .arg("-C")
            .arg(dir.join("libsrc"))
            .arg(".")
            .status()
            .ok()?;
        if !status.success() {
            return None;
        }
    }
    Some(lib)
}

fn workspace_with(root: &Path, files: &[(&str, &str)]) -> Workspace {
    for (name, text) in files {
        write(root, name, text);
    }
    let mut workspace = Workspace::new(config(root));
    workspace.wait_for_index();
    workspace
}

#[test]
fn interface_abstract_method() {
    let root = fixture("interface-abstract");
    let base = "interface I { void f(int a); }\n";
    let sub = "class A implements I { public void f(int a) {} }\n";
    let mut workspace = workspace_with(&root, &[("I.java", base), ("A.java", sub)]);
    let supers = workspace.super_methods(&root.join("A.java"), method_position(sub, "void "));
    assert_eq!(supers.len(), 1, "{supers:?}");
    assert!(supers[0].file.ends_with("I.java"), "{supers:?}");
}

#[test]
fn interface_default_method() {
    let root = fixture("interface-default");
    let base = "interface I { default void f() {} }\n";
    let sub = "class A implements I { public void f() {} }\n";
    let mut workspace = workspace_with(&root, &[("I.java", base), ("A.java", sub)]);
    let supers = workspace.super_methods(&root.join("A.java"), method_position(sub, "void "));
    assert_eq!(supers.len(), 1, "{supers:?}");
    assert!(supers[0].file.ends_with("I.java"), "{supers:?}");
}

#[test]
fn superclass_concrete_method() {
    let root = fixture("superclass");
    let base = "class Base { void f(String s) {} }\n";
    let sub = "class Sub extends Base { void f(String s) {} }\n";
    let mut workspace = workspace_with(&root, &[("Base.java", base), ("Sub.java", sub)]);
    let supers = workspace.super_methods(&root.join("Sub.java"), method_position(sub, "void "));
    assert_eq!(supers.len(), 1, "{supers:?}");
    assert!(supers[0].file.ends_with("Base.java"), "{supers:?}");
}

#[test]
fn overload_with_same_arity_matches_only_the_right_type() {
    let root = fixture("overload-arity");
    let base = "class Base { void f(int x) {} void f(String s) {} }\n";
    let sub = "class Sub extends Base { void f(int x) {} }\n";
    let mut workspace = workspace_with(&root, &[("Base.java", base), ("Sub.java", sub)]);
    let supers = workspace.super_methods(&root.join("Sub.java"), method_position(sub, "void "));
    assert_eq!(supers.len(), 1, "{supers:?}");
    let base_text = base;
    let int_line = base_text.find("f(int").unwrap();
    let int_column = base_text[..int_line]
        .rsplit('\n')
        .next()
        .map(|line| line.chars().map(|ch| ch.len_utf16() as u32).sum::<u32>())
        .unwrap_or(0);
    assert_eq!(supers[0].range.start.line, 0, "{supers:?}");
    assert_eq!(supers[0].range.start.column, int_column, "{supers:?}");
}

#[test]
fn generic_superclass_is_instantiated() {
    let root = fixture("generic-superclass");
    let base = "class Base<T> { void put(T t) {} }\n";
    let sub = "class Sub extends Base<String> { void put(String s) {} }\n";
    let mut workspace = workspace_with(&root, &[("Base.java", base), ("Sub.java", sub)]);
    let supers = workspace.super_methods(&root.join("Sub.java"), method_position(sub, "void "));
    assert_eq!(supers.len(), 1, "{supers:?}");
    assert!(supers[0].file.ends_with("Base.java"), "{supers:?}");
}

#[test]
fn generic_interface_is_instantiated() {
    let root = fixture("generic-interface");
    let base = "interface Handler<T> { void handle(T t); }\n";
    let sub = "class H implements Handler<String> { public void handle(String s) {} }\n";
    let mut workspace = workspace_with(&root, &[("Handler.java", base), ("H.java", sub)]);
    let supers = workspace.super_methods(&root.join("H.java"), method_position(sub, "void "));
    assert_eq!(supers.len(), 1, "{supers:?}");
    assert!(supers[0].file.ends_with("Handler.java"), "{supers:?}");
}

#[test]
fn raw_supertype_matches_object_parameter() {
    let root = fixture("raw-supertype");
    let base = "class Base<T> { void put(T t) {} }\n";
    let sub = "class Sub extends Base { void put(Object o) {} }\n";
    let mut workspace = workspace_with(&root, &[("Base.java", base), ("Sub.java", sub)]);
    let supers = workspace.super_methods(&root.join("Sub.java"), method_position(sub, "void "));
    assert_eq!(supers.len(), 1, "{supers:?}");
}

#[test]
fn varargs_array_equivalence() {
    let root = fixture("varargs");
    let base = "class Base { void log(String s, Object... args) {} }\n";
    let sub = "class Sub extends Base { void log(String s, Object[] args) {} }\n";
    let mut workspace = workspace_with(&root, &[("Base.java", base), ("Sub.java", sub)]);
    let supers = workspace.super_methods(&root.join("Sub.java"), method_position(sub, "void "));
    assert_eq!(supers.len(), 1, "{supers:?}");
}

#[test]
fn varargs_on_both_sides() {
    let root = fixture("varargs-both");
    let base = "class Base { void log(Object... args) {} }\n";
    let sub = "class Sub extends Base { void log(Object... args) {} }\n";
    let mut workspace = workspace_with(&root, &[("Base.java", base), ("Sub.java", sub)]);
    let supers = workspace.super_methods(&root.join("Sub.java"), method_position(sub, "void "));
    assert_eq!(supers.len(), 1, "{supers:?}");
}

#[test]
fn bridge_method_is_not_returned() {
    let root = fixture("bridge");
    let base = "interface Getter<T> { T get(); }\n";
    let sub = "class Impl implements Getter<String> { public String get() { return \"\"; } }\n";
    let mut workspace = workspace_with(&root, &[("Getter.java", base), ("Impl.java", sub)]);
    let supers = workspace.super_methods(&root.join("Impl.java"), method_position(sub, "String "));
    assert_eq!(supers.len(), 1, "{supers:?}");
    assert!(supers[0].file.ends_with("Getter.java"), "{supers:?}");
}

#[test]
fn multiple_interfaces_return_all() {
    let root = fixture("multi-interface");
    let a = "interface A { void run(); }\n";
    let b = "interface B { void run(); }\n";
    let c = "class C implements A, B { public void run() {} }\n";
    let mut workspace = workspace_with(&root, &[("A.java", a), ("B.java", b), ("C.java", c)]);
    let supers = workspace.super_methods(&root.join("C.java"), method_position(c, "void "));
    assert_eq!(supers.len(), 2, "{supers:?}");
    let files: Vec<String> = supers
        .iter()
        .map(|location| location.file.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(files.contains(&"A.java".to_string()), "{files:?}");
    assert!(files.contains(&"B.java".to_string()), "{files:?}");
}

#[test]
fn transitive_interface_returns_root_method() {
    let root = fixture("transitive-interface");
    let base = "interface Root { void run(); }\n";
    let mid = "interface Mid extends Root {}\n";
    let sub = "class Impl implements Mid { public void run() {} }\n";
    let mut workspace = workspace_with(&root, &[("Root.java", base), ("Mid.java", mid), ("Impl.java", sub)]);
    let supers = workspace.super_methods(&root.join("Impl.java"), method_position(sub, "void "));
    assert_eq!(supers.len(), 1, "{supers:?}");
    assert!(
        supers[0].file.file_name().unwrap().to_string_lossy() == "Root.java",
        "{supers:?}"
    );
}

#[test]
fn multi_level_inheritance_returns_direct_parent() {
    let root = fixture("multi-level");
    let base = "class Base { void f() {} }\n";
    let mid = "class Mid extends Base { void f() {} }\n";
    let sub = "class Sub extends Mid { void f() {} }\n";
    let mut workspace = workspace_with(
        &root,
        &[("Base.java", base), ("Mid.java", mid), ("Sub.java", sub)],
    );
    let supers = workspace.super_methods(&root.join("Sub.java"), method_position(sub, "void "));
    assert!(
        supers.iter().any(|location| location.file.ends_with("Mid.java")),
        "{supers:?}"
    );
}

#[test]
fn static_parent_method_is_not_an_override() {
    let root = fixture("static-parent");
    let base = "class Base { static void f() {} }\n";
    let sub = "class Sub extends Base { void f() {} }\n";
    let mut workspace = workspace_with(&root, &[("Base.java", base), ("Sub.java", sub)]);
    let supers = workspace.super_methods(&root.join("Sub.java"), method_position(sub, "void "));
    assert!(supers.is_empty(), "{supers:?}");
}

#[test]
fn private_parent_method_is_not_an_override() {
    let root = fixture("private-parent");
    let base = "class Base { private void f() {} }\n";
    let sub = "class Sub extends Base { void f() {} }\n";
    let mut workspace = workspace_with(&root, &[("Base.java", base), ("Sub.java", sub)]);
    let supers = workspace.super_methods(&root.join("Sub.java"), method_position(sub, "void "));
    assert!(supers.is_empty(), "{supers:?}");
}

#[test]
fn static_child_method_has_no_super_method() {
    let root = fixture("static-child");
    let base = "class Base { void f() {} }\n";
    let sub = "class Sub extends Base { static void f() {} }\n";
    let mut workspace = workspace_with(&root, &[("Base.java", base), ("Sub.java", sub)]);
    let supers = workspace.super_methods(&root.join("Sub.java"), method_position(sub, "static void "));
    assert!(supers.is_empty(), "{supers:?}");
}

#[test]
fn constructor_has_no_super_method() {
    let root = fixture("constructor");
    let text = "class A { A() {} }\n";
    let mut workspace = workspace_with(&root, &[("A.java", text)]);
    let supers = workspace.super_methods(&root.join("A.java"), method_position(text, "A { "));
    assert!(supers.is_empty(), "{supers:?}");
}

#[test]
fn method_without_parent_returns_empty() {
    let root = fixture("no-parent");
    let text = "class A { void f() {} }\n";
    let mut workspace = workspace_with(&root, &[("A.java", text)]);
    let supers = workspace.super_methods(&root.join("A.java"), method_position(text, "void "));
    assert!(supers.is_empty(), "{supers:?}");
}

#[test]
fn position_inside_string_or_comment_returns_empty() {
    let root = fixture("literal");
    let text = "class A {\n    String s = \"f\";\n    // f\n    void f() {}\n}\n";
    let mut workspace = workspace_with(&root, &[("A.java", text)]);
    let string_position = position(text, "\"f\"");
    assert!(workspace
        .super_methods(&root.join("A.java"), Position::new(string_position.line, string_position.column + 1))
        .is_empty());
    let comment_position = position(text, "// f");
    assert!(workspace
        .super_methods(&root.join("A.java"), Position::new(comment_position.line, comment_position.column + 3))
        .is_empty());
}

#[test]
fn jdk_parent_method_uses_src_zip() {
    let Some(src) = jdk_src() else {
        return; // no JDK sources in this environment
    };
    if !src.exists() {
        return;
    }
    let root = fixture("jdk-parent");
    let text = "class A { public String toString() { return \"\"; } }\n";
    let mut workspace = workspace_with(&root, &[("A.java", text)]);
    let supers = workspace.super_methods(&root.join("A.java"), method_position(text, "String "));
    assert!(!supers.is_empty(), "{supers:?}");
    assert!(
        supers[0].file.to_string_lossy().contains("Object.java"),
        "{supers:?}"
    );
    // The exact line depends on the JDK build; verify the line holds the
    // declaration instead of hard-coding a version-specific number.
    let source = std::fs::read_to_string(&supers[0].file).expect("Object.java");
    let line = source
        .lines()
        .nth(supers[0].range.start.line as usize)
        .unwrap_or_default();
    assert!(line.contains("toString"), "line: {line:?}");
}

#[test]
fn external_jar_parent_uses_sources_jar() {
    let root = fixture("external-sources");
    let Some(lib) = build_external_lib(&root, true) else {
        return; // javac/jar unavailable
    };
    let sub = "class Sub extends ext.Base { public void f() {} }\n";
    write(&root, "Sub.java", sub);
    let mut workspace = Workspace::new(config_with_class_path(&root, vec![lib]));
    workspace.wait_for_index();
    let supers = workspace.super_methods(&root.join("Sub.java"), method_position(sub, "void "));
    assert_eq!(supers.len(), 1, "{supers:?}");
    assert!(
        supers[0].file.to_string_lossy().contains("Base.java"),
        "{supers:?}"
    );
}

#[test]
fn external_jar_without_sources_returns_empty() {
    let root = fixture("external-no-sources");
    let Some(lib) = build_external_lib(&root, false) else {
        return;
    };
    let sub = "class Sub extends ext.Base { public void f() {} }\n";
    write(&root, "Sub.java", sub);
    let mut workspace = Workspace::new(config_with_class_path(&root, vec![lib]));
    workspace.wait_for_index();
    let supers = workspace.super_methods(&root.join("Sub.java"), method_position(sub, "void "));
    assert!(supers.is_empty(), "{supers:?}");
}

#[test]
fn hover_shows_overrides() {
    let root = fixture("hover-overrides");
    let base = "interface I { void f(); }\n";
    let sub = "class A implements I { public void f() {} }\n";
    let mut workspace = workspace_with(&root, &[("I.java", base), ("A.java", sub)]);
    let hover = workspace
        .hover(&root.join("A.java"), method_position(sub, "void "))
        .expect("hover");
    assert!(hover.contains("Overrides"), "{hover}");
    assert!(hover.contains("I#f()"), "{hover}");
    assert!(hover.contains("I#f()](file://"), "{hover}");
}

#[test]
fn overlay_content_is_used() {
    let root = fixture("overlay");
    write(&root, "Base.java", "class Base {}\n");
    write(&root, "Sub.java", "class Sub extends Base {}\n");
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    workspace.set_overlay(&root.join("Base.java"), "class Base { void f() {} }\n");
    workspace.set_overlay(
        &root.join("Sub.java"),
        "class Sub extends Base { void f() {} }\n",
    );
    let supers = workspace.super_methods(
        &root.join("Sub.java"),
        method_position("class Sub extends Base { void f() {} }\n", "void "),
    );
    assert_eq!(supers.len(), 1, "{supers:?}");
}

#[test]
fn partial_index_does_not_block_super_methods() {
    let root = fixture("partial");
    write(&root, "Base.java", "class Base { void f() {} }\n");
    write(&root, "Sub.java", "class Sub extends Base { void f() {} }\n");
    write(&root, "Broken.java", "import ja;\nclass Broken {}\n");
    let mut workspace = Workspace::new(config(&root));
    workspace.wait_for_index();
    let sub = "class Sub extends Base { void f() {} }\n";
    let supers = workspace.super_methods(&root.join("Sub.java"), method_position(sub, "void "));
    assert_eq!(supers.len(), 1, "{supers:?}");
}
