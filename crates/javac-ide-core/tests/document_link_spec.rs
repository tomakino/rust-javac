//! M4 document links (10 matrix items, section 28.4).

use std::path::{Path, PathBuf};

use javac_analysis_api::WorkspaceService;

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

fn javac() -> Option<PathBuf> {
    let home = std::env::var_os("JAVA_HOME")?;
    let file = if cfg!(windows) { "javac.exe" } else { "javac" };
    let path = PathBuf::from(home).join("bin").join(file);
    path.exists().then_some(path)
}

fn jar_tool() -> Option<PathBuf> {
    let home = std::env::var_os("JAVA_HOME")?;
    let file = if cfg!(windows) { "jar.exe" } else { "jar" };
    let path = PathBuf::from(home).join("bin").join(file);
    path.exists().then_some(path)
}

fn jdk_src() -> Option<PathBuf> {
    let home = std::env::var_os("JAVA_HOME")?;
    let path = PathBuf::from(home).join("lib").join("src.zip");
    path.exists().then_some(path)
}

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("document-link-spec")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

/// Project source (p/Foo.java) + external jar with a sources jar (a/b/C).
fn fixture_with_sources(name: &str) -> Option<(PathBuf, PathBuf, PathBuf)> {
    let javac = javac()?;
    let jar = jar_tool()?;
    let root = fixture(name);
    let src = root.join("src");
    std::fs::create_dir_all(src.join("p")).unwrap();
    std::fs::write(src.join("p/Foo.java"), "package p; public class Foo {}\n").unwrap();
    let external = root.join("external-src");
    std::fs::create_dir_all(external.join("a/b")).unwrap();
    std::fs::write(
        external.join("a/b/C.java"),
        "package a.b; public class C { public static int value; }\n",
    )
    .unwrap();
    let classes = root.join("classes");
    std::fs::create_dir_all(&classes).unwrap();
    let status = std::process::Command::new(&javac)
        .arg("-d")
        .arg(&classes)
        .arg(external.join("a/b/C.java"))
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    let jar_path = root.join("lib.jar");
    let status = std::process::Command::new(&jar)
        .arg("cf")
        .arg(&jar_path)
        .arg("-C")
        .arg(&classes)
        .arg(".")
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    let sources_jar = root.join("lib-sources.jar");
    let status = std::process::Command::new(&jar)
        .arg("cf")
        .arg(&sources_jar)
        .arg("-C")
        .arg(&external)
        .arg(".")
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    Some((root, jar_path, sources_jar))
}

fn config(root: &Path, class_path: Vec<PathBuf>) -> AnalysisConfig {
    platform_dir();
    AnalysisConfig {
        class_path,
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: vec![root.join("src")],
    }
}

fn links(
    workspace: &mut Workspace,
    root: &Path,
    text: &str,
) -> Vec<javac_analysis_api::DocumentLink> {
    let path = root.join("Test.java");
    std::fs::write(&path, text).unwrap();
    workspace.document_links(&path)
}

#[test]
fn item01_project_class_link() {
    let Some((root, jar, _sources)) = fixture_with_sources("item01") else {
        return;
    };
    let mut workspace = Workspace::new(config(&root, vec![jar]));
    let links = links(&mut workspace, &root, "import p.Foo;\n\nclass Test {}\n");
    assert_eq!(links.len(), 1, "{links:?}");
    assert!(links[0].target.ends_with("p/Foo.java"), "{links:?}");
}

#[test]
fn item02_jdk_class_link() {
    let Some(jdk) = jdk_src() else { return };
    let Some((root, jar, _sources)) = fixture_with_sources("item02") else {
        return;
    };
    let mut workspace = Workspace::new(config(&root, vec![jar]));
    let links = links(&mut workspace, &root, "import java.util.List;\n\nclass Test {}\n");
    assert_eq!(links.len(), 1, "{links:?}");
    assert!(links[0].target.ends_with("List.java"), "{links:?}");
    drop(jdk);
}

#[test]
fn item03_external_sources_jar_link() {
    let Some((root, jar, _sources)) = fixture_with_sources("item03") else {
        return;
    };
    let mut workspace = Workspace::new(config(&root, vec![jar]));
    let links = links(&mut workspace, &root, "import a.b.C;\n\nclass Test {}\n");
    assert_eq!(links.len(), 1, "{links:?}");
    assert!(links[0].target.ends_with("a/b/C.java"), "{links:?}");
}

#[test]
fn item04_external_without_sources_no_link() {
    let Some((root, jar, _sources)) = fixture_with_sources("item04") else {
        return;
    };
    std::fs::remove_file(root.join("lib-sources.jar")).ok();
    let mut workspace = Workspace::new(config(&root, vec![jar]));
    let links = links(&mut workspace, &root, "import a.b.C;\n\nclass Test {}\n");
    assert!(links.is_empty(), "{links:?}");
}

#[test]
fn item05_invalid_import_no_link() {
    let Some((root, jar, _sources)) = fixture_with_sources("item05") else {
        return;
    };
    let mut workspace = Workspace::new(config(&root, vec![jar]));
    let links = links(&mut workspace, &root, "import a.b.Missing;\n\nclass Test {}\n");
    assert!(links.is_empty(), "{links:?}");
}

#[test]
fn item06_static_import_resolves_class() {
    let Some(jdk) = jdk_src() else { return };
    let Some((root, jar, _sources)) = fixture_with_sources("item06") else {
        return;
    };
    let mut workspace = Workspace::new(config(&root, vec![jar]));
    let links = links(
        &mut workspace,
        &root,
        "import static java.util.Arrays.asList;\n\nclass Test {}\n",
    );
    assert_eq!(links.len(), 1, "{links:?}");
    assert!(links[0].target.ends_with("Arrays.java"), "{links:?}");
    drop(jdk);
}

#[test]
fn item07_url_link() {
    let Some((root, jar, _sources)) = fixture_with_sources("item07") else {
        return;
    };
    let mut workspace = Workspace::new(config(&root, vec![jar]));
    let links = links(
        &mut workspace,
        &root,
        "// https://example.com/docs\nclass Test {}\n",
    );
    assert_eq!(links.len(), 1, "{links:?}");
    assert_eq!(links[0].target, "https://example.com/docs");
}

#[test]
fn item08_multiple_urls() {
    let Some((root, jar, _sources)) = fixture_with_sources("item08") else {
        return;
    };
    let mut workspace = Workspace::new(config(&root, vec![jar]));
    let links = links(
        &mut workspace,
        &root,
        "// https://a.example/x\n// https://b.example/y\nclass Test {}\n",
    );
    assert_eq!(links.len(), 2, "{links:?}");
    assert!(links.iter().any(|link| link.target == "https://a.example/x"), "{links:?}");
    assert!(links.iter().any(|link| link.target == "https://b.example/y"), "{links:?}");
}

#[test]
fn item09_url_in_comment_and_string() {
    let Some((root, jar, _sources)) = fixture_with_sources("item09") else {
        return;
    };
    let mut workspace = Workspace::new(config(&root, vec![jar]));
    let links = links(
        &mut workspace,
        &root,
        "/* https://comment.example */\nclass Test {\n  String s = \"https://string.example\";\n}\n",
    );
    assert_eq!(links.len(), 2, "{links:?}");
    assert!(links.iter().any(|link| link.target == "https://comment.example"), "{links:?}");
    assert!(links.iter().any(|link| link.target == "https://string.example"), "{links:?}");
}

#[test]
fn item10_crlf_import_and_url() {
    let Some((root, jar, _sources)) = fixture_with_sources("item10") else {
        return;
    };
    let mut workspace = Workspace::new(config(&root, vec![jar]));
    let links = links(
        &mut workspace,
        &root,
        "import p.Foo;\r\n// https://crlf.example\r\nclass Test {}\r\n",
    );
    assert_eq!(links.len(), 2, "{links:?}");
    assert!(links.iter().any(|link| link.target.ends_with("p/Foo.java")), "{links:?}");
    assert!(links.iter().any(|link| link.target == "https://crlf.example"), "{links:?}");
}
