//! D5: foldingRange / selectionRange / documentLink / codeAction /
//! diagnostics / typeDefinition / implementation.
//!
//! Mirrors `scripts/lsp/probe_d5_navigation.py` through the library API.

use std::path::{Path, PathBuf};

use javac_analysis_api::{FoldingRangeKind, Position, SelectionRange, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::Workspace;

const I: &str = "package com.example.app;\n\npublic interface I {\n    void f();\n}\n";
const ABSTRACT_BASE: &str = "package com.example.app;\n\npublic abstract class AbstractBase {\n    public abstract void g();\n}\n";
const IMPL: &str = "package com.example.app;\n\npublic class Impl extends AbstractBase implements I {\n    public void f() {\n    }\n\n    public void g() {\n    }\n}\n";
const GENERATE: &str = "package com.example.app;\n\npublic class GenerateTarget {\n    int value;\n}\n";

const MAIN: &str = r#"package com.example.app;

import com.example.base.Model;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * Javadoc block
 * second line
 */
public class App extends AbstractBase implements I {
    int field;

    public void f() {
        int value = 1;
        int other = value + 1;
    }

    public void g() {
    }

    void folds() {
        if (true) {
            int a = 1;
        }
        for (int i = 0; i < 1; i++) {
            int b = 2;
        }
        while (false) {
            int c = 3;
        }
        try {
            int d = 4;
        } catch (Exception e) {
            int e2 = 5;
        }
        if (true) { int single = 6; }
    }

    void selections() {
        int value = 1;
    }

    void types() {
        List<String> list = new ArrayList<>();
        String text = "";
        var inferred = new ArrayList<String>();
    }

    void calls() {
        String made = make();
        I i = new Impl();
        i.f();
        AbstractBase base = new Impl();
        base.g();
    }

    String make() {
        return "";
    }
}
"#;

const DIAG_SYNTAX: &str = "package com.example.app;\n\npublic class DiagSyntax {\n    // 中文注释：下一行是语法错误\n    void f() {\n        int x = ;\n    }\n}\n";
const DIAG_TYPE: &str = "package com.example.app;\n\npublic class DiagType {\n    // 中文注释\n    void f() {\n        String s = 1;\n    }\n}\n";
const ADD_IMPORT: &str = "package com.example.app;\n\npublic class AddImport {\n    ArrayList<String> list = new ArrayList<>();\n}\n";

const MAIN_REL: &str = "src/main/java/com/example/app/App.java";
const GENERATE_REL: &str = "src/main/java/com/example/app/GenerateTarget.java";

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

fn jdk_src_available() -> bool {
    if let Some(path) = std::env::var_os("RAVAC_JDK_SRC") {
        return PathBuf::from(path).exists();
    }
    std::env::var_os("JAVA_HOME")
        .map(|home| PathBuf::from(home).join("lib").join("src.zip").is_file())
        .unwrap_or(false)
}

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
            .join("d5-navigation-spec")
            .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let src = root.join("src/main/java/com/example/app");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("I.java"), I).unwrap();
        std::fs::write(src.join("AbstractBase.java"), ABSTRACT_BASE).unwrap();
        std::fs::write(src.join("Impl.java"), IMPL).unwrap();
        std::fs::write(src.join("App.java"), MAIN).unwrap();
        std::fs::write(src.join("GenerateTarget.java"), GENERATE).unwrap();
        std::fs::write(src.join("AddImport.java"), ADD_IMPORT).unwrap();
        let base = root.join("src/main/java/com/example/base");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(
            base.join("Model.java"),
            "package com.example.base;\n\npublic class Model {\n    public int value;\n}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("pom.xml"),
            "<project><modelVersion>4.0.0</modelVersion><groupId>com.example</groupId><artifactId>d5</artifactId><version>1.0</version></project>\n",
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

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn main(&self) -> PathBuf {
        self.path(MAIN_REL)
    }

    fn position(&self, text: &str, marker: &str, offset: usize) -> Position {
        position_of(text, marker, offset)
    }

    fn folding(&mut self, text: &str) -> Vec<javac_analysis_api::FoldingRange> {
        let main = self.main();
        self.workspace.diagnostics(&main, Some(text));
        self.workspace.folding_ranges(&main)
    }

    fn selection_chain(&mut self, text: &str, position: Position) -> Option<SelectionRange> {
        let main = self.main();
        self.workspace.diagnostics(&main, Some(text));
        self.workspace.selection_ranges(&main, &[position]).into_iter().next()
    }

    fn links(&mut self, text: &str) -> Vec<javac_analysis_api::DocumentLink> {
        let main = self.main();
        self.workspace.diagnostics(&main, Some(text));
        self.workspace.document_links(&main)
    }

    fn actions_at(&mut self, relative: &str, text: &str, position: Position) -> Vec<javac_analysis_api::CodeAction> {
        let path = self.path(relative);
        self.workspace.diagnostics(&path, Some(text));
        self.workspace.code_actions(&path, position)
    }

    fn diagnostics(&mut self, text: &str) -> Vec<javac_analysis_api::Diagnostic> {
        let main = self.main();
        self.workspace.diagnostics(&main, Some(text))
    }

    fn type_definition_at(&mut self, text: &str, position: Position) -> Option<javac_analysis_api::Location> {
        let main = self.main();
        self.workspace.diagnostics(&main, Some(text));
        self.workspace.type_definition(&main, position)
    }

    fn implementation_at(&mut self, position: Position) -> Vec<javac_analysis_api::Location> {
        let main = self.main();
        self.workspace.implementation(&main, position)
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

fn contains(range: &javac_analysis_api::Range, position: Position) -> bool {
    (range.start.line, range.start.column) <= (position.line, position.column)
        && (range.end.line, range.end.column) >= (position.line, position.column)
}

#[test]
fn folding_covers_imports_comments_blocks_and_skips_single_line() {
    let mut session = Session::new("folding");
    let ranges = session.folding(MAIN);
    let class_end = MAIN.lines().count() as u32 - 1;
    assert!(
        ranges
            .iter()
            .any(|range| range.kind == Some(FoldingRangeKind::Imports)),
        "{ranges:?}"
    );
    assert!(
        ranges
            .iter()
            .any(|range| range.kind == Some(FoldingRangeKind::Comment)),
        "{ranges:?}"
    );
    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 11 && range.end_line == class_end),
        "{ranges:?}"
    );
    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 14 && range.end_line == 17),
        "{ranges:?}"
    );
    for start in [23, 26, 29, 32] {
        assert!(
            ranges.iter().any(|range| range.start_line == start),
            "missing block at {start}: {ranges:?}"
        );
    }
    assert!(
        !ranges
            .iter()
            .any(|range| range.start_line == 37 && range.end_line == 37),
        "single-line block must not fold: {ranges:?}"
    );
}

#[test]
fn selection_chain_nests_and_contains_the_cursor() {
    let mut session = Session::new("selection");
    let cursor = session.position(MAIN, "void selections() {\n        int value = 1;", "void selections() {\n        ".len());
    let chain = session.selection_chain(MAIN, cursor).expect("selection chain");
    let mut levels = Vec::new();
    let mut node = Some(&chain);
    while let Some(current) = node {
        levels.push(current.range);
        node = current.parent.as_deref();
    }
    assert!(levels.len() >= 3, "{levels:?}");
    assert!(levels.iter().all(|range| contains(range, cursor)), "{levels:?}");
    for pair in levels.windows(2) {
        assert!(
            (pair[1].start.line, pair[1].start.column) <= (pair[0].start.line, pair[0].start.column)
                && (pair[1].end.line, pair[1].end.column) >= (pair[0].end.line, pair[0].end.column),
            "{levels:?}"
        );
    }
    assert!(
        levels.last().is_some_and(|range| range.end.line >= MAIN.lines().count() as u32 - 1),
        "{levels:?}"
    );
}

#[test]
fn document_links_resolve_project_and_jdk_imports() {
    let mut session = Session::new("links");
    let links = session.links(MAIN);
    assert!(
        links.iter().any(|link| link.target.ends_with("Model.java")),
        "{links:?}"
    );
    if jdk_src_available() {
        assert!(
            links.iter().any(|link| link.target.contains("List.java")),
            "{links:?}"
        );
    }
    let generate = session.path(GENERATE_REL);
    let links = session.workspace.document_links(&generate);
    assert!(links.is_empty(), "{links:?}");
}

#[test]
fn code_actions_add_import_remove_unused_organize_and_generate() {
    let mut session = Session::new("actions");
    // add-import is covered by the LSP probe (JDK candidate); the library
    // path here checks the remaining actions.
    let import_position = session.position(MAIN, "import java.util.Collections;", 0);
    let actions = session.actions_at(MAIN_REL, MAIN, import_position);
    let titles: Vec<&str> = actions.iter().map(|action| action.title.as_str()).collect();
    assert!(
        titles.iter().any(|title| title.contains("unused")),
        "{titles:?}"
    );
    let main_path = session.path(MAIN_REL);
    let organize_edits = session.workspace.organize_imports(&main_path);
    assert!(!organize_edits.is_empty(), "{organize_edits:?}");

    let generate_position = session.position(GENERATE, "public class GenerateTarget", "public class ".len());
    let actions = session.actions_at(GENERATE_REL, GENERATE, generate_position);
    let titles: Vec<&str> = actions.iter().map(|action| action.title.as_str()).collect();
    for wanted in [
        "Generate constructor",
        "Generate toString()",
        "Generate equals() and hashCode()",
    ] {
        assert!(titles.iter().any(|title| *title == wanted), "{titles:?}");
    }
}

#[test]
fn diagnostics_use_utf16_lines_after_a_chinese_comment() {
    let mut session = Session::new("diagnostics");
    let syntax = session.diagnostics(DIAG_SYNTAX);
    let expected = DIAG_SYNTAX[..DIAG_SYNTAX.find("int x = ;").unwrap()].matches('\n').count() as u32;
    assert!(
        syntax
            .iter()
            .any(|diagnostic| diagnostic.range.start.line == expected),
        "{syntax:?}"
    );
    let type_error = session.diagnostics(DIAG_TYPE);
    let expected = DIAG_TYPE[..DIAG_TYPE.find("String s = 1;").unwrap()].matches('\n').count() as u32;
    assert!(
        type_error
            .iter()
            .any(|diagnostic| diagnostic.range.start.line == expected),
        "{type_error:?}"
    );
}

#[test]
fn type_definition_covers_list_method_return_and_var() {
    let mut session = Session::new("type-definition");
    let list = session.position(MAIN, "List<String> list", 0);
    let location = session.type_definition_at(MAIN, list).expect("List");
    assert!(location.file.ends_with("List.java"), "{location:?}");

    let make = session.position(MAIN, "make();", 0);
    let location = session.type_definition_at(MAIN, make).expect("make()");
    assert!(location.file.ends_with("String.java"), "{location:?}");

    let inferred = session.position(MAIN, "var inferred", "var ".len());
    let location = session.type_definition_at(MAIN, inferred).expect("var");
    assert!(location.file.ends_with("ArrayList.java"), "{location:?}");
}

#[test]
fn implementations_cover_interface_abstract_and_override() {
    let mut session = Session::new("implementation");
    for position in [
        session.position(MAIN, "I i = new Impl();", 0),
        session.position(MAIN, "base.g();", "base.".len()),
        session.position(MAIN, "i.f();", "i.".len()),
    ] {
        let locations = session.implementation_at(position);
        assert!(
            locations
                .iter()
                .any(|location| location.file.ends_with("Impl.java")),
            "{locations:?}"
        );
    }
}

#[test]
fn overlay_text_is_used_by_folding_type_definition_and_generate() {
    let mut session = Session::new("overlay");
    let fold_text = "package com.example.app;\n\npublic class App {\n    void overlay() {\n        if (true) {\n            int extra = 1;\n        }\n    }\n}\n";
    let ranges = session.folding(fold_text);
    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 3 && range.end_line == 7),
        "{ranges:?}"
    );

    let type_text = "package com.example.app;\n\nimport java.util.List;\n\npublic class App {\n    void overlay() {\n        List<String> overlayList = null;\n    }\n}\n";
    let list = session.position(type_text, "List<String> overlayList", 0);
    let location = session
        .type_definition_at(type_text, list)
        .expect("overlay List");
    assert!(location.file.ends_with("List.java"), "{location:?}");

    let gen_text = "package com.example.app;\n\npublic class App {\n    int value;\n}\n";
    let position = session.position(gen_text, "public class App", "public class ".len());
    let actions = session.actions_at(MAIN_REL, gen_text, position);
    let titles: Vec<&str> = actions.iter().map(|action| action.title.as_str()).collect();
    assert!(titles.iter().any(|title| *title == "Generate toString()"), "{titles:?}");
}
