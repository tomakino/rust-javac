//! D2: definition/hover for types, methods, constructors, method references
//! and annotations, over a small multi-package project.
//!
//! Mirrors `scripts/lsp/probe_d2_types.py` (the real LSP transcript) through
//! the library API; every fixture project lives in a unique directory so the
//! on-disk index cache cannot shadow the current build.

use std::path::{Path, PathBuf};

use javac_analysis_api::{Location, Position, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::Workspace;

const BASE_TYPE: &str = r#"package com.example.base;

public class BaseType {
    public static int inherited = 1;

    public BaseType() {
    }

    public BaseType(int value) {
    }

    public BaseType(String value) {
    }

    public static void staticMethod() {
    }

    public static void inheritedMethod() {
    }

    public void instanceMethod() {
    }

    public void over(int value) {
    }

    public void over(String value) {
    }
}
"#;

const BASE_IFACE: &str = "package com.example.base;\n\npublic interface BaseIface {\n    void run();\n}\n";
const COLOR: &str = "package com.example.base;\n\npublic enum Color {\n    RED,\n    GREEN\n}\n";
const POINT: &str = "package com.example.base;\n\npublic record Point(int x, int y) {\n}\n";
const MARKER: &str = "package com.example.base;\n\npublic @interface Marker {\n}\n";
const ANNO: &str = "package com.example.base;\n\npublic @interface Anno {\n    int value();\n}\n";
const OUTER: &str =
    "package com.example.base;\n\npublic class Outer {\n    public static class Inner {\n    }\n}\n";
const SUB: &str = "package com.example.base;\n\npublic class Sub extends BaseType {\n    public Sub() {\n        super();\n    }\n}\n";

const MAIN: &str = r#"package com.example.app;

import com.example.base.Anno;
import com.example.base.BaseIface;
import com.example.base.BaseType;
import com.example.base.Color;
import com.example.base.Marker;
import com.example.base.Outer;
import com.example.base.Point;
import java.util.function.Supplier;
import static com.example.base.Sub.inherited;
import static com.example.base.Sub.inheritedMethod;
import static java.util.Collections.*;

public class D2Probe extends BaseType {
    public D2Probe() {
        super();
    }

    void types() {
        BaseType baseType = null;
        BaseIface baseIface = null;
        Color color = null;
        Point point = null;
        Outer.Inner inner = null;
        java.util.List<String> list = null;
    }

    void anonymous() {
        Runnable runnable = new Runnable() {
            public void run() {
            }
        };
    }

    void localClass() {
        class Local {
        }
        Local local = new Local();
    }

    void overloads() {
        over(1);
        over("x");
    }

    void overrides() {
        this.instanceMethod();
    }

    @Override
    public void instanceMethod() {
    }

    void constructors() {
        new BaseType(1);
        new BaseType("x");
    }

    void references() {
        Runnable staticRef = BaseType::staticMethod;
        Supplier<BaseType> constructorRef = BaseType::new;
        BaseType base = new BaseType();
        Runnable boundRef = base::instanceMethod;
    }

    void annotationUse() {
        @Marker
        int markerField = 0;
        @Anno(value = 1)
        int annoField = 0;
    }

    void generics() {
        Box<String> box = null;
        String value = box.get();
    }

    void wildcardImport() {
        emptyList();
    }

    void inheritedImport() {
        int value = inherited;
        inheritedMethod();
    }
}

class Box<T> {
    T value;

    T get() {
        return value;
    }
}
"#;

const MAIN_REL: &str = "src/main/java/com/example/app/D2Probe.java";

fn fixture_files() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "pom.xml",
            "<project><modelVersion>4.0.0</modelVersion><groupId>com.example</groupId><artifactId>d2-spec</artifactId><version>1.0</version></project>\n",
        ),
        ("src/main/java/com/example/base/BaseType.java", BASE_TYPE),
        ("src/main/java/com/example/base/BaseIface.java", BASE_IFACE),
        ("src/main/java/com/example/base/Color.java", COLOR),
        ("src/main/java/com/example/base/Point.java", POINT),
        ("src/main/java/com/example/base/Marker.java", MARKER),
        ("src/main/java/com/example/base/Anno.java", ANNO),
        ("src/main/java/com/example/base/Outer.java", OUTER),
        ("src/main/java/com/example/base/Sub.java", SUB),
        (MAIN_REL, MAIN),
    ]
}

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

fn text_of(relative: &str) -> &'static str {
    fixture_files()
        .into_iter()
        .find(|(name, _)| *name == relative)
        .map(|(_, text)| text)
        .unwrap_or_default()
}

/// A fresh project + indexed workspace. The unique directory keeps the
/// persistent index cache of a previous run from being loaded.
fn project(name: &str) -> (Workspace, PathBuf, PathBuf) {
    platform_dir();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("d2-types-spec")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let source = root.join("src/main/java");
    for (relative, text) in fixture_files() {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, text).unwrap();
    }
    let main = root.join(MAIN_REL);
    let mut workspace = Workspace::new(AnalysisConfig {
        class_path: Vec::new(),
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: vec![source],
    });
    workspace.wait_for_index();
    (workspace, root, main)
}

fn position(text: &str, marker: &str, offset: usize) -> Position {
    let start = text.find(marker).expect("marker") + offset;
    let prefix = &text[..start];
    let line = prefix.matches('\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    let column = text[line_start..start]
        .chars()
        .map(|ch| ch.len_utf16() as u32)
        .sum();
    Position::new(line, column)
}

fn definition(workspace: &mut Workspace, main: &Path, marker: &str, offset: usize) -> Location {
    workspace
        .definition(main, position(MAIN, marker, offset))
        .unwrap_or_else(|| panic!("no definition at {marker:?}+{offset}"))
}

fn hover(workspace: &mut Workspace, main: &Path, marker: &str, offset: usize) -> String {
    workspace
        .hover(main, position(MAIN, marker, offset))
        .unwrap_or_else(|| panic!("no hover at {marker:?}+{offset}"))
}

fn assert_definition_at(
    location: &Location,
    relative: &str,
    marker: &str,
    offset: usize,
) {
    let expected_text = text_of(relative);
    let expected = position(expected_text, marker, offset);
    assert!(
        location.file.ends_with(Path::new(relative).file_name().unwrap()),
        "expected {relative}, got {location:?}"
    );
    assert_eq!(
        (location.range.start.line, location.range.start.column),
        (expected.line, expected.column),
        "{location:?}"
    );
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
fn type_references_cross_package() {
    let (mut workspace, _root, main) = project("types");
    assert_definition_at(
        &definition(&mut workspace, &main, "BaseType baseType", 0),
        "src/main/java/com/example/base/BaseType.java",
        "public class BaseType {",
        "public class ".len(),
    );
    assert_definition_at(
        &definition(&mut workspace, &main, "BaseIface baseIface", 0),
        "src/main/java/com/example/base/BaseIface.java",
        "public interface BaseIface {",
        "public interface ".len(),
    );
    assert_definition_at(
        &definition(&mut workspace, &main, "Color color", 0),
        "src/main/java/com/example/base/Color.java",
        "public enum Color {",
        "public enum ".len(),
    );
    assert_definition_at(
        &definition(&mut workspace, &main, "Point point", 0),
        "src/main/java/com/example/base/Point.java",
        "public record Point(int x, int y) {",
        "public record ".len(),
    );
}

#[test]
fn nested_class_reference() {
    let (mut workspace, _root, main) = project("nested");
    assert_definition_at(
        &definition(&mut workspace, &main, "Outer.Inner inner", "Outer.".len()),
        "src/main/java/com/example/base/Outer.java",
        "public static class Inner {",
        "public static class ".len(),
    );
}

#[test]
fn anonymous_class_type() {
    if !jdk_src_available() {
        return;
    }
    let (mut workspace, _root, main) = project("anonymous");
    let location = definition(&mut workspace, &main, "new Runnable()", "new ".len());
    assert!(
        location.file.ends_with("Runnable.java"),
        "expected Runnable.java, got {location:?}"
    );
    assert!(hover(&mut workspace, &main, "new Runnable()", "new ".len()).contains("Runnable"));
}

#[test]
fn local_class_reference() {
    let (mut workspace, _root, main) = project("local");
    assert_definition_at(
        &definition(&mut workspace, &main, "Local local = new Local();", 0),
        MAIN_REL,
        "class Local {",
        "class ".len(),
    );
}

#[test]
fn method_overloads_use_the_argument_types() {
    let (mut workspace, _root, main) = project("overloads");
    assert_definition_at(
        &definition(&mut workspace, &main, "over(1);", 0),
        "src/main/java/com/example/base/BaseType.java",
        "public void over(int value) {",
        "public void ".len(),
    );
    assert_definition_at(
        &definition(&mut workspace, &main, "over(\"x\");", 0),
        "src/main/java/com/example/base/BaseType.java",
        "public void over(String value) {",
        "public void ".len(),
    );
}

#[test]
fn override_declaration_and_call_stay_on_the_override() {
    let (mut workspace, _root, main) = project("override");
    let declaration = definition(
        &mut workspace,
        &main,
        "public void instanceMethod() {",
        "public void ".len(),
    );
    assert_definition_at(
        &declaration,
        MAIN_REL,
        "public void instanceMethod() {",
        "public void ".len(),
    );
    let call = definition(&mut workspace, &main, "this.instanceMethod();", "this.".len());
    assert_definition_at(
        &call,
        MAIN_REL,
        "public void instanceMethod() {",
        "public void ".len(),
    );
    let hover = hover(
        &mut workspace,
        &main,
        "public void instanceMethod() {",
        "public void ".len(),
    );
    assert!(hover.contains("Overrides"), "{hover}");
    assert!(hover.contains("com.example.base.BaseType#instanceMethod()"), "{hover}");
    assert!(hover.contains("com.example.base.BaseType#instanceMethod()](file://"), "{hover}");
}

#[test]
fn constructor_overloads() {
    let (mut workspace, _root, main) = project("constructors");
    assert_definition_at(
        &definition(&mut workspace, &main, "new BaseType(1);", "new ".len()),
        "src/main/java/com/example/base/BaseType.java",
        "public BaseType(int value) {",
        "public ".len(),
    );
    assert_definition_at(
        &definition(&mut workspace, &main, "new BaseType(\"x\");", "new ".len()),
        "src/main/java/com/example/base/BaseType.java",
        "public BaseType(String value) {",
        "public ".len(),
    );
    let hover = hover(&mut workspace, &main, "new BaseType(1);", "new ".len());
    assert!(hover.contains("BaseType(int value)"), "{hover}");
}

#[test]
fn method_references_resolve_their_targets() {
    let (mut workspace, _root, main) = project("references");
    assert_definition_at(
        &definition(&mut workspace, &main, "BaseType::staticMethod", "BaseType::".len()),
        "src/main/java/com/example/base/BaseType.java",
        "public static void staticMethod() {",
        "public static void ".len(),
    );
    assert_definition_at(
        &definition(&mut workspace, &main, "BaseType::new", "BaseType::".len()),
        "src/main/java/com/example/base/BaseType.java",
        "public BaseType() {",
        "public ".len(),
    );
    assert_definition_at(
        &definition(&mut workspace, &main, "base::instanceMethod", "base::".len()),
        "src/main/java/com/example/base/BaseType.java",
        "public void instanceMethod() {",
        "public void ".len(),
    );
    let hover = hover(&mut workspace, &main, "BaseType::new", "BaseType::".len());
    assert!(hover.contains("BaseType()"), "{hover}");
}

#[test]
fn annotation_type_and_member() {
    let (mut workspace, _root, main) = project("annotations");
    assert_definition_at(
        &definition(&mut workspace, &main, "@Marker", 1),
        "src/main/java/com/example/base/Marker.java",
        "public @interface Marker {",
        "public @interface ".len(),
    );
    assert_definition_at(
        &definition(&mut workspace, &main, "value = 1", 0),
        "src/main/java/com/example/base/Anno.java",
        "int value();",
        "int ".len(),
    );
    assert!(hover(&mut workspace, &main, "@Marker", 1).contains("Marker"));
    assert!(hover(&mut workspace, &main, "value = 1", 0).contains("value"));
}

#[test]
fn generic_method_signature_type_parameter() {
    let (mut workspace, _root, main) = project("generic");
    assert_definition_at(
        &definition(&mut workspace, &main, "T get() {", 0),
        MAIN_REL,
        "class Box<T>",
        "class Box<".len(),
    );
    assert!(hover(&mut workspace, &main, "T get() {", 0).contains("T"));
}

#[test]
fn qualified_jdk_type() {
    if !jdk_src_available() {
        return;
    }
    let (mut workspace, _root, main) = project("qualified");
    let location = definition(&mut workspace, &main, "java.util.List<String>", "java.util.".len());
    assert!(
        location.file.ends_with("List.java"),
        "expected List.java, got {location:?}"
    );
    assert!(hover(&mut workspace, &main, "java.util.List<String>", "java.util.".len())
        .contains("List"));
}

#[test]
fn static_import_wildcard_member() {
    if !jdk_src_available() {
        return;
    }
    let (mut workspace, _root, main) = project("wildcard");
    let location = definition(&mut workspace, &main, "emptyList();", 0);
    assert!(
        location.file.ends_with("Collections.java"),
        "expected Collections.java, got {location:?}"
    );
    assert!(hover(&mut workspace, &main, "emptyList();", 0).contains("emptyList"));
}

#[test]
fn inherited_static_import_member() {
    let (mut workspace, _root, main) = project("inherited-import");
    assert_definition_at(
        &definition(&mut workspace, &main, "Sub.inherited;", "Sub.".len()),
        "src/main/java/com/example/base/BaseType.java",
        "public static int inherited = 1;",
        "public static int ".len(),
    );
    assert_definition_at(
        &definition(&mut workspace, &main, "int value = inherited;", "int value = ".len()),
        "src/main/java/com/example/base/BaseType.java",
        "public static int inherited = 1;",
        "public static int ".len(),
    );
    assert!(hover(&mut workspace, &main, "Sub.inherited;", "Sub.".len()).contains("inherited"));
}
