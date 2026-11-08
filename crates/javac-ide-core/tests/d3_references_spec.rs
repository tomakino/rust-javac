//! D3: references / rename / prepareRename semantics over a multi-file project.
//!
//! Mirrors `scripts/lsp/probe_d3_references.py` through the library API. Every
//! fixture project lives in a unique directory so the persistent index cache of
//! a previous run cannot shadow the current build.

use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, Range, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::Workspace;

const MODEL: &str = r#"package com.example.base;

public class Model {
    public int value;

    public void compute() {
    }
}
"#;
const TAG: &str = "package com.example.base;\n\npublic @interface Tag {\n}\n";
const A_FOO: &str = "package com.example.a;\n\npublic class Foo {\n    public int a;\n}\n";
const B_FOO: &str = "package com.example.b;\n\npublic class Foo {\n    public int b;\n}\n";
const CHILD: &str = r#"package com.example.app;

import com.example.base.Model;

public class Child extends Model {
    @Override
    public void compute() {
    }
}
"#;
const USER: &str = r#"package com.example.app;

import com.example.base.Model;

class User {
    void use(Model model) {
        model.value = 1;
        int x = model.value;
        model.compute();
    }
}
"#;
const USER2: &str = r#"package com.example.app;

import com.example.base.Model;

class User2 {
    void use(Model model) {
        model.value++;
    }
}
"#;
const APP: &str = r#"package com.example.app;

import com.example.base.Model;
import com.example.base.Tag;
import java.util.List;

@Tag
public class App {
    int field;
    Model model;

    void fields() {
        this.field = 1;
        int x = this.field;
        // field in a comment
        String s = "field";
        Model m = new Model();
        m.value = 2;
        int y = m.value;
    }

    void methods() {
        compute();
        this.compute();
        Runnable r = model::compute;
        Model ref = new Child();
        ref.compute();
    }

    void compute() {
    }

    void locals() {
        int x = 1;
        int y = x;
    }

    void locals2() {
        int x = 2;
        int y = x;
    }

    void params(int p) {
        int y = p;
    }

    void catchParam() {
        try {
            compute();
        } catch (Exception e) {
            String s = e.getMessage();
        }
    }

    void forVar() {
        for (String s : List.of("a")) {
            String t = s;
        }
    }

    void lambdaParam() {
        List.of("a").forEach(s -> {
            String u = s;
        });
    }

    void shadow() {
        int field = 0;
        int y = field;
    }

    void qualify(Model other) {
        other.value = 1;
    }

    void generic() {
        List<Model> models = null;
    }

    void samePackageNames() {
        com.example.a.Foo first = null;
        com.example.b.Foo second = null;
    }
}
"#;

const BASE: &str = "src/main/java/com/example/base/";
const APP_DIR: &str = "src/main/java/com/example/app/";
const MAIN: &str = "src/main/java/com/example/app/App.java";

fn fixture_files() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "pom.xml",
            "<project><modelVersion>4.0.0</modelVersion><groupId>com.example</groupId><artifactId>d3-spec</artifactId><version>1.0</version></project>\n",
        ),
        ("src/main/java/com/example/base/Model.java", MODEL),
        ("src/main/java/com/example/base/Tag.java", TAG),
        ("src/main/java/com/example/a/Foo.java", A_FOO),
        ("src/main/java/com/example/b/Foo.java", B_FOO),
        ("src/main/java/com/example/app/Child.java", CHILD),
        ("src/main/java/com/example/app/User.java", USER),
        ("src/main/java/com/example/app/User2.java", USER2),
        (MAIN, APP),
    ]
}

fn text_of(relative: &str) -> &'static str {
    fixture_files()
        .into_iter()
        .find(|(name, _)| *name == relative)
        .map(|(_, text)| text)
        .unwrap_or_default()
}

fn short(relative: &str) -> String {
    relative
        .split("src/main/java/")
        .nth(1)
        .unwrap_or(relative)
        .to_string()
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

struct Project {
    workspace: Workspace,
    root: PathBuf,
}

impl Project {
    fn new(name: &str) -> Self {
        platform_dir();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("tmp")
            .join("d3-references-spec")
            .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (relative, text) in fixture_files() {
            let path = root.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, text).unwrap();
        }
        let workspace = Workspace::new(AnalysisConfig {
            class_path: Vec::new(),
            output_dir: root.join("out"),
            source_level: "17".to_string(),
            source_path: vec![root.join("src/main/java")],
        });
        let mut project = Self { workspace, root };
        project.workspace.wait_for_index();
        project
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn relative_of(&self, path: &Path) -> String {
        let text = path.to_string_lossy().replace('\\', "/");
        text.split("src/main/java/")
            .nth(1)
            .map(str::to_string)
            .unwrap_or_else(|| path.file_name().unwrap().to_string_lossy().to_string())
    }

    fn position(&self, relative: &str, marker: &str, offset: usize) -> Position {
        position_of(text_of(relative), marker, offset)
    }

    fn references(&mut self, relative: &str, marker: &str, offset: usize) -> Vec<(String, u32, u32)> {
        let path = self.path(relative);
        let position = self.position(relative, marker, offset);
        let mut points: Vec<(String, u32, u32)> = self
            .workspace
            .references(&path, position)
            .iter()
            .map(|location| {
                (
                    self.relative_of(&location.file),
                    location.range.start.line,
                    location.range.start.column,
                )
            })
            .collect();
        points.sort();
        points
    }

    fn rename(
        &mut self,
        relative: &str,
        marker: &str,
        offset: usize,
        new_name: &str,
    ) -> Vec<(String, u32, u32, String)> {
        let path = self.path(relative);
        let position = self.position(relative, marker, offset);
        let mut edits: Vec<(String, u32, u32, String)> = self
            .workspace
            .rename_workspace(&path, position, new_name)
            .iter()
            .map(|edit| {
                (
                    self.relative_of(&edit.file),
                    edit.edit.range.start.line,
                    edit.edit.range.start.column,
                    edit.edit.new_text.clone(),
                )
            })
            .collect();
        edits.sort();
        edits
    }

    fn prepare(
        &mut self,
        relative: &str,
        marker: &str,
        offset: usize,
    ) -> Option<Range> {
        let path = self.path(relative);
        let position = self.position(relative, marker, offset);
        self.workspace.prepare_rename(&path, position)
    }
}

fn position_of(text: &str, marker: &str, offset: usize) -> Position {
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

fn want(relative: &str, marker: &str, offset: usize) -> (String, u32, u32) {
    let position = position_of(text_of(relative), marker, offset);
    (
        short(relative),
        position.line,
        position.column,
    )
}

#[test]
fn field_references_include_this_and_exclude_shadowed_locals() {
    let mut project = Project::new("field");
    let got = project.references(
        MAIN,
        "this.field = 1;",
        5,
    );
    let mut want_points = vec![
        want(MAIN, "    int field;", 8),
        want(MAIN, "this.field = 1;", 5),
        want(MAIN, "int x = this.field;", 13),
    ];
    want_points.sort();
    assert_eq!(got, want_points);

    let shadow = project.references(MAIN, "int field = 0;", 4);
    let mut want_shadow = vec![
        want(MAIN, "int field = 0;", 4),
        want(MAIN, "int y = field;", 8),
    ];
    want_shadow.sort();
    assert_eq!(shadow, want_shadow, "shadowed local must not see the field");
}

#[test]
fn method_references_cover_calls_and_method_references() {
    let mut project = Project::new("method");
    let model = BASE.to_string() + "Model.java";
    let user = APP_DIR.to_string() + "User.java";
    let got = project.references(&model, "public void compute() {", 12);
    let mut want_points = vec![
        want(&model, "public void compute() {", 12),
        want(&user, "model.compute();", 6),
        want(MAIN, "model::compute", 7),
        want(MAIN, "ref.compute();", 4),
    ];
    want_points.sort();
    assert_eq!(got, want_points);
}

#[test]
fn class_references_cover_declarations_imports_new_and_generics() {
    let mut project = Project::new("class");
    let model = BASE.to_string() + "Model.java";
    let user = APP_DIR.to_string() + "User.java";
    let user2 = APP_DIR.to_string() + "User2.java";
    let child = APP_DIR.to_string() + "Child.java";
    let got = project.references(&model, "public class Model {", 13);
    let mut want_points = vec![
        want(&model, "public class Model {", 13),
        want(&user, "import com.example.base.Model;", 24),
        want(&user, "void use(Model model)", 9),
        want(&user2, "import com.example.base.Model;", 24),
        want(&user2, "void use(Model model)", 9),
        want(MAIN, "import com.example.base.Model;", 24),
        want(MAIN, "    Model model;", 4),
        want(MAIN, "Model m = new Model();", 0),
        want(MAIN, "new Model()", 4),
        want(MAIN, "Model ref = new Child();", 0),
        want(MAIN, "List<Model> models", 5),
        want(MAIN, "void qualify(Model other)", 13),
        want(&child, "import com.example.base.Model;", 24),
        want(&child, "extends Model", 8),
    ];
    want_points.sort();
    assert_eq!(got, want_points);
}

#[test]
fn local_and_parameter_references_stay_in_their_scope() {
    let mut project = Project::new("locals");
    let local = project.references(MAIN, "int x = 1;", 4);
    let mut want_local = vec![
        want(MAIN, "int x = 1;", 4),
        want(MAIN, "int y = x;", 8),
    ];
    want_local.sort();
    assert_eq!(local, want_local);

    let parameter = project.references(MAIN, "void params(int p)", 16);
    let mut want_parameter = vec![
        want(MAIN, "void params(int p)", 16),
        want(MAIN, "int y = p;", 8),
    ];
    want_parameter.sort();
    assert_eq!(parameter, want_parameter);

    let catch = project.references(MAIN, "catch (Exception e)", 17);
    let mut want_catch = vec![
        want(MAIN, "catch (Exception e)", 17),
        want(MAIN, "String s = e.getMessage();", 11),
    ];
    want_catch.sort();
    assert_eq!(catch, want_catch);

    let for_var = project.references(MAIN, "for (String s : List.of(\"a\"))", 12);
    let mut want_for = vec![
        want(MAIN, "for (String s : List.of(\"a\"))", 12),
        want(MAIN, "String t = s;", 11),
    ];
    want_for.sort();
    assert_eq!(for_var, want_for);

    let lambda = project.references(MAIN, "forEach(s ->", 8);
    let mut want_lambda = vec![
        want(MAIN, "forEach(s ->", 8),
        want(MAIN, "String u = s;", 11),
    ];
    want_lambda.sort();
    assert_eq!(lambda, want_lambda);
}

#[test]
fn cross_file_field_references() {
    let mut project = Project::new("cross-field");
    let model = BASE.to_string() + "Model.java";
    let user = APP_DIR.to_string() + "User.java";
    let user2 = APP_DIR.to_string() + "User2.java";
    let got = project.references(&model, "public int value;", 11);
    let mut want_points = vec![
        want(&model, "public int value;", 11),
        want(&user, "model.value = 1;", 6),
        want(&user, "int x = model.value;", 14),
        want(&user2, "model.value++;", 6),
        want(MAIN, "m.value = 2;", 2),
        want(MAIN, "int y = m.value;", 10),
        want(MAIN, "other.value = 1;", 6),
    ];
    want_points.sort();
    assert_eq!(got, want_points);
}

#[test]
fn same_name_classes_in_different_packages_do_not_mix() {
    let mut project = Project::new("same-name");
    let a = "src/main/java/com/example/a/Foo.java";
    let b = "src/main/java/com/example/b/Foo.java";
    let got_a = project.references(a, "public class Foo {", 13);
    let mut want_a = vec![
        want(a, "public class Foo {", 13),
        want(MAIN, "com.example.a.Foo first", 14),
    ];
    want_a.sort();
    assert_eq!(got_a, want_a);
    let got_b = project.references(b, "public class Foo {", 13);
    let mut want_b = vec![
        want(b, "public class Foo {", 13),
        want(MAIN, "com.example.b.Foo second", 14),
    ];
    want_b.sort();
    assert_eq!(got_b, want_b);
}

#[test]
fn rename_field_updates_uses_but_not_literals_or_locals() {
    let mut project = Project::new("rename-field");
    let edits = project.rename(MAIN, "this.field = 1;", 5, "renamed");
    let mut want_edits = vec![
        (short(MAIN), 8, 8, "renamed".to_string()),
        (short(MAIN), 12, 13, "renamed".to_string()),
        (short(MAIN), 13, 21, "renamed".to_string()),
    ];
    want_edits.sort();
    assert_eq!(edits, want_edits);
    let text = text_of(MAIN);
    assert!(text.contains("int field = 0;"));
    assert!(text.contains("// field in a comment"));
    assert!(text.contains("\"field\""));
}

#[test]
fn rename_method_includes_calls_references_and_override() {
    let mut project = Project::new("rename-method");
    let model = BASE.to_string() + "Model.java";
    let edits = project.rename(&model, "public void compute() {", 12, "run");
    let mut want_edits = vec![
        (short(&model), 5, 16, "run".to_string()),
        (short(&(APP_DIR.to_string() + "Child.java")), 6, 16, "run".to_string()),
        (short(&(APP_DIR.to_string() + "User.java")), 8, 14, "run".to_string()),
        (short(MAIN), 24, 28, "run".to_string()),
        (short(MAIN), 26, 12, "run".to_string()),
    ];
    want_edits.sort();
    assert_eq!(edits, want_edits);
}

#[test]
fn rename_class_updates_all_references() {
    let mut project = Project::new("rename-class");
    let model = BASE.to_string() + "Model.java";
    let edits = project.rename(&model, "public class Model {", 13, "RenamedModel");
    assert_eq!(edits.len(), 14, "edits: {edits:?}");
    assert!(edits.iter().all(|edit| edit.3 == "RenamedModel"));
    // The declaration itself must be renamed.
    assert!(
        edits
            .iter()
            .any(|edit| edit.0 == short(&model) && edit.1 == 2 && edit.2 == 13)
    );
}

#[test]
fn rename_local_only_touches_its_scope() {
    let mut project = Project::new("rename-local");
    let edits = project.rename(MAIN, "int x = 1;", 4, "renamedX");
    let mut want_edits = vec![
        (short(MAIN), 33, 12, "renamedX".to_string()),
        (short(MAIN), 34, 16, "renamedX".to_string()),
    ];
    want_edits.sort();
    assert_eq!(edits, want_edits);
}

#[test]
fn prepare_rename_returns_the_token_range() {
    let mut project = Project::new("prepare");
    let field = project.prepare(MAIN, "this.field = 1;", 5).expect("field");
    assert_eq!((field.start.line, field.start.column), (12, 13));
    assert_eq!((field.end.line, field.end.column), (12, 18));

    let local = project.prepare(MAIN, "int x = 1;", 4).expect("local");
    assert_eq!((local.start.line, local.start.column), (33, 12));
    assert_eq!((local.end.line, local.end.column), (33, 13));

    let model = BASE.to_string() + "Model.java";
    let class = project.prepare(&model, "public class Model {", 13).expect("class");
    assert_eq!((class.start.line, class.start.column), (2, 13));
    assert_eq!((class.end.line, class.end.column), (2, 18));

    assert!(project.prepare(MAIN, "int field;", 0).is_none());
}

#[test]
fn rename_does_not_reject_a_conflicting_name() {
    // Documented behaviour: the current server returns edits even when the new
    // name already exists (no conflict detection). The probe records this.
    let mut project = Project::new("rename-conflict");
    let edits = project.rename(MAIN, "this.field = 1;", 5, "model");
    assert!(edits.iter().all(|edit| edit.3 == "model"));
}
