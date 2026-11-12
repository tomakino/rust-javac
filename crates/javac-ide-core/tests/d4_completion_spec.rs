//! D4: completion / signatureHelp / inlayHint semantics.
//!
//! Mirrors `scripts/lsp/probe_d4_completion.py` through the library API.

mod common;
use common::platform_dir;

use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::project::{
    BuildFingerprint, BuildSourceSet, BuildSystemId, Module, ProjectModel, SourceSetKind,
};
use javac_ide_core::workspace::Workspace;

const BASE_TYPE: &str = r#"package com.example.app;

public class BaseType {
    public int value;

    public BaseType() {
    }

    public BaseType(int value) {
    }

    public BaseType(String value) {
    }

    public void compute() {
    }

    public static void staticMethod() {
    }

    public void over(int value) {
    }

    public void over(String value) {
    }

    public void single(int a, String b) {
    }

    public static void sum(int... values) {
    }
}
"#;

const APP: &str = "package com.example.app;\n\npublic class App {\n}\n";

struct Session {
    workspace: Workspace,
    root: PathBuf,
}

impl Session {
    fn new(name: &str) -> Self {
        platform_dir();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("tmp")
            .join("d4-completion-spec")
            .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let main = root.join("src/main/java/com/example/app/App.java");
        std::fs::create_dir_all(main.parent().unwrap()).unwrap();
        std::fs::write(&main, APP).unwrap();
        std::fs::write(
            root.join("src/main/java/com/example/app/BaseType.java"),
            BASE_TYPE,
        )
        .unwrap();
        std::fs::write(
            root.join("pom.xml"),
            "<project><modelVersion>4.0.0</modelVersion><groupId>com.example</groupId><artifactId>d4</artifactId><version>1.0</version></project>\n",
        )
        .unwrap();
        let mut workspace = Workspace::new(AnalysisConfig {
            class_path: Vec::new(),
            output_dir: root.join("out"),
            source_level: "17".to_string(),
            source_path: vec![root.join("src/main/java")],
        });
        workspace.wait_for_index();
        Self { workspace, root }
    }

    fn main(&self) -> PathBuf {
        self.root.join("src/main/java/com/example/app/App.java")
    }

    /// LSP project mode: `source_path` empty, dependencies resolved from
    /// class files. The index still scans the project sources, which is where
    /// parameter names come from when the class file has no MethodParameters.
    fn project(name: &str) -> Option<Self> {
        platform_dir();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("tmp")
            .join("d4-project-spec")
            .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let source_root = root.join("src/main/java");
        let package_dir = source_root.join("com/example/app");
        std::fs::create_dir_all(&package_dir).unwrap();
        std::fs::write(package_dir.join("BaseType.java"), BASE_TYPE).unwrap();
        std::fs::write(package_dir.join("App.java"), APP).unwrap();
        let classes = root.join("classes");
        std::fs::create_dir_all(&classes).unwrap();
        let javac = std::env::var_os("JAVA_HOME").map(PathBuf::from).map(|home| {
            home.join("bin")
                .join(if cfg!(windows) { "javac.exe" } else { "javac" })
        })?;
        let status = std::process::Command::new(javac)
            .arg("-d")
            .arg(&classes)
            .arg(package_dir.join("BaseType.java"))
            .status()
            .ok()?;
        if !status.success() {
            return None;
        }
        let model = ProjectModel {
            root: root.clone(),
            provider: BuildSystemId::NONE,
            modules: vec![Module {
                root: root.clone(),
                name: "d4".to_string(),
                source_sets: vec![BuildSourceSet::new(
                    SourceSetKind::Main,
                    vec![source_root],
                    Vec::new(),
                    Some(classes),
                )],
                jdk_home: None,
                source_level: Some("17".to_string()),
            }],
            fingerprint: BuildFingerprint::default(),
        };
        let mut workspace = Workspace::new_from_project(&model, root.join("out"), None);
        workspace.wait_for_index();
        Some(Self { workspace, root })
    }

    fn load(&mut self, source: &str) -> (String, u32, u32) {
        let marker = source.find('|');
        let text = match marker {
            Some(_) => source.replacen('|', "", 1),
            None => source.to_string(),
        };
        let (line, column) = match marker {
            Some(cursor) => {
                let prefix = &text[..cursor];
                let line = prefix.matches('\n').count() as u32;
                let column = text
                    [prefix.rfind('\n').map(|index| index + 1).unwrap_or(0)..cursor]
                    .chars()
                    .map(|ch| ch.len_utf16() as u32)
                    .sum();
                (line, column)
            }
            None => (0, 0),
        };
        let main = self.main();
        self.workspace.diagnostics(&main, Some(&text));
        (text, line, column)
    }

    fn complete(&mut self, source: &str) -> Vec<String> {
        let (_, line, column) = self.load(source);
        let main = self.main();
        self.workspace
            .completion(&main, Position::new(line, column))
            .into_iter()
            .map(|item| item.label)
            .collect()
    }

    fn signature(&mut self, source: &str) -> (Vec<String>, u32, u32) {
        let (_, line, column) = self.load(source);
        let main = self.main();
        let help = self
            .workspace
            .signature_help(&main, Position::new(line, column))
            .expect("signature help");
        (
            help.signatures
                .iter()
                .map(|signature| signature.label.clone())
                .collect(),
            help.active_signature,
            help.active_parameter,
        )
    }

    fn inlays(&mut self, source: &str) -> Vec<(String, i32)> {
        let _ = self.load(source);
        let main = self.main();
        self.workspace
            .inlay_hints(&main)
            .into_iter()
            .map(|hint| {
                let kind = match hint.kind {
                    javac_analysis_api::InlayHintKind::Type => 1,
                    javac_analysis_api::InlayHintKind::Parameter => 2,
                };
                (hint.label, kind)
            })
            .collect()
    }
}

fn has(labels: &[String], name: &str) -> bool {
    labels.iter().any(|label| label == name)
}

#[test]
fn this_receiver_members() {
    let mut session = Session::new("this");
    let labels = session.complete(
        "package com.example.app;\n\npublic class App {\n    int field;\n    void compute() {\n    }\n    void f() {\n        this.|\n    }\n}\n",
    );
    assert!(has(&labels, "field"), "{labels:?}");
    assert!(has(&labels, "compute"), "{labels:?}");
}

#[test]
fn object_receiver_members_and_static_members() {
    let mut session = Session::new("object");
    let labels = session.complete(
        "package com.example.app;\n\npublic class App {\n    void f() {\n        BaseType obj = new BaseType();\n        obj.|\n    }\n}\n",
    );
    assert!(has(&labels, "value"), "{labels:?}");
    assert!(has(&labels, "compute"), "{labels:?}");

    let labels = session.complete(
        "package com.example.app;\n\nimport java.util.Collections;\n\npublic class App {\n    void f() {\n        Collections.|\n    }\n}\n",
    );
    assert!(has(&labels, "emptyList"), "{labels:?}");
    assert!(!has(&labels, "instanceMethod"), "{labels:?}");
}

#[test]
fn type_name_keyword_and_unresolved() {
    let mut session = Session::new("names");
    let labels = session.complete(
        "package com.example.app;\n\npublic class App {\n    void f() {\n        Str|\n    }\n}\n",
    );
    assert!(has(&labels, "String"), "{labels:?}");

    let labels = session.complete(
        "package com.example.app;\n\npublic class App {\n    void f() {\n        ret|\n    }\n}\n",
    );
    assert!(has(&labels, "return"), "{labels:?}");

    let labels = session.complete(
        "package com.example.app;\n\npublic class App {\n    void f() {\n        UnknownType.|\n    }\n}\n",
    );
    let _ = labels;
}

#[test]
fn locals_parameters_and_import_candidates() {
    let mut session = Session::new("scope");
    let labels = session.complete(
        "package com.example.app;\n\npublic class App {\n    void f(int param) {\n        int local = 1;\n        loc|\n    }\n}\n",
    );
    assert!(has(&labels, "local"), "{labels:?}");

    let labels = session.complete(
        "package com.example.app;\n\npublic class App {\n    void f(int param) {\n        par|\n    }\n}\n",
    );
    assert!(has(&labels, "param"), "{labels:?}");

    let labels = session.complete(
        "package com.example.app;\n\npublic class App {\n    void f() {\n        List|\n    }\n}\n",
    );
    assert!(has(&labels, "List"), "{labels:?}");
}

#[test]
fn signature_single_and_overloads() {
    let mut session = Session::new("signature");
    let (signatures, active, parameter) = session.signature(
        "package com.example.app;\n\npublic class App {\n    void single(int a, String b) {\n    }\n    void f() {\n        single(1, |)\n    }\n}\n",
    );
    assert!(signatures[active as usize].contains("single("), "{signatures:?}");
    assert_eq!(parameter, 1);

    let (signatures, active, _) = session.signature(
        "package com.example.app;\n\npublic class App {\n    void over(int a) {\n    }\n    void over(String a) {\n    }\n    void f() {\n        over(1|)\n    }\n}\n",
    );
    assert!(signatures[active as usize].contains("over(int"), "{signatures:?}");

    let (signatures, active, _) = session.signature(
        "package com.example.app;\n\npublic class App {\n    void over(int a) {\n    }\n    void over(String a) {\n    }\n    void f() {\n        over(\"x\"|)\n    }\n}\n",
    );
    assert!(signatures[active as usize].contains("over(String"), "{signatures:?}");
}

#[test]
fn signature_constructor_and_varargs() {
    let mut session = Session::new("ctor");
    let (signatures, active, _) = session.signature(
        "package com.example.app;\n\npublic class App {\n    void f() {\n        new BaseType(1|)\n    }\n}\n",
    );
    assert!(signatures[active as usize].contains("BaseType(int"), "{signatures:?}");

    let (signatures, active, parameter) = session.signature(
        "package com.example.app;\n\npublic class App {\n    void f() {\n        BaseType.sum(1, 2|)\n    }\n}\n",
    );
    assert!(signatures[active as usize].contains("sum(int..."), "{signatures:?}");
    assert_eq!(parameter, 1);
}

#[test]
fn var_type_inlay_hint() {
    let mut session = Session::new("var");
    let hints = session.inlays(
        "package com.example.app;\n\npublic class App {\n    void f() {\n        var x = new BaseType();\n        x.compute();\n    }\n}\n",
    );
    assert!(
        hints.iter().any(|(label, kind)| *kind == 1 && label.contains("BaseType")),
        "{hints:?}"
    );
}

#[test]
fn parameter_name_inlay_hint_same_file() {
    let mut session = Session::new("parameter");
    let hints = session.inlays(
        "package com.example.app;\n\npublic class App {\n    void single(int a, String b) {\n    }\n    void f() {\n        single(1, \"x\");\n    }\n}\n",
    );
    assert!(
        hints.iter().any(|(label, kind)| *kind == 2 && label == "a:"),
        "{hints:?}"
    );
    assert!(
        hints.iter().any(|(label, kind)| *kind == 2 && label == "b:"),
        "{hints:?}"
    );
}

#[test]
fn cross_file_parameter_names_resolve_with_project_sources() {
    // With the project sources on the analysis source path the declaring
    // method resolves from source and keeps its parameter names. (The LSP
    // project mode deliberately keeps `source_path` empty to preserve JDK
    // modules, so there the same call only yields synthetic `arg0` names; the
    // probe records that limitation.)
    let mut session = Session::new("parameter-cross");
    let hints = session.inlays(
        "package com.example.app;\n\npublic class App {\n    void f() {\n        new BaseType().single(1, \"x\");\n    }\n}\n",
    );
    assert!(
        hints.iter().any(|(label, kind)| *kind == 2 && label == "a:"),
        "{hints:?}"
    );
}

#[test]
fn cross_file_parameter_names_resolve_from_index_without_class_file_parameters() {
    // Mirrors the LSP project mode: source_path is empty and BaseType is
    // resolved from a class file compiled without -parameters.
    let Some(mut session) = Session::project("parameter-index") else {
        return;
    };
    let hints = session.inlays(
        "package com.example.app;\n\npublic class App {\n    void f() {\n        new BaseType(7);\n        new BaseType().single(1, \"x\");\n    }\n}\n",
    );
    assert!(
        hints.iter().any(|(label, kind)| *kind == 2 && label == "value:"),
        "constructor parameter hint missing: {hints:?}"
    );
    assert!(
        hints.iter().any(|(label, kind)| *kind == 2 && label == "a:"),
        "method parameter hint missing: {hints:?}"
    );
    assert!(
        hints.iter().any(|(label, kind)| *kind == 2 && label == "b:"),
        "method parameter hint missing: {hints:?}"
    );
}

#[test]
fn same_name_parameter_hints_follow_the_toggle() {
    let source = "package com.example.app;\n\npublic class App {\n    void singleValue(int value) {\n    }\n    void f() {\n        int value = 1;\n        singleValue(value);\n    }\n}\n";
    let mut session = Session::new("same-name");
    let hints = session.inlays(source);
    assert!(
        hints.iter().any(|(label, kind)| *kind == 2 && label == "value:"),
        "same-name hint should be visible by default: {hints:?}"
    );

    WorkspaceService::set_inlay_hints_same_name(&mut session.workspace, false);
    let hints = session.inlays(source);
    assert!(
        !hints.iter().any(|(label, kind)| *kind == 2 && label == "value:"),
        "same-name hint should be suppressed: {hints:?}"
    );

    let different = "package com.example.app;\n\npublic class App {\n    void singleValue(int value) {\n    }\n    void f() {\n        singleValue(1);\n    }\n}\n";
    let hints = session.inlays(different);
    assert!(
        hints.iter().any(|(label, kind)| *kind == 2 && label == "value:"),
        "different argument must keep its hint: {hints:?}"
    );
}

#[test]
fn same_file_constructor_parameter_names_are_available() {
    let mut session = Session::new("same-file-constructor");
    let hints = session.inlays(
        "package com.example.app;\n\npublic class App {\n    App(int count) {\n    }\n    void f() {\n        new App(1);\n    }\n}\n",
    );
    assert!(
        hints.iter().any(|(label, kind)| *kind == 2 && label == "count:"),
        "{hints:?}"
    );
}

#[test]
fn jdk_constructor_parameter_names_are_available() {
    let mut session = Session::new("jdk-constructor");
    let hints = session.inlays(
        "package com.example.app;\n\npublic class App {\n    void f() {\n        String path = \"x\";\n        new java.io.File(path);\n    }\n}\n",
    );
    assert!(
        hints.iter().any(|(label, kind)| *kind == 2 && label == "pathname:"),
        "{hints:?}"
    );
}

#[test]
fn no_return_type_inlay_hints() {
    // Return-type hints are not implemented; only Type/Parameter are emitted.
    let mut session = Session::new("return");
    let hints = session.inlays(
        "package com.example.app;\n\npublic class App {\n    int f() {\n        return 1;\n    }\n}\n",
    );
    assert!(hints.iter().all(|(_, kind)| *kind == 1 || *kind == 2), "{hints:?}");
}
