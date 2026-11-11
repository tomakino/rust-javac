//! Extractor declaration name starts come from the AST/name table, not text.

mod common;
use common::platform_dir;

use std::path::{Path, PathBuf};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::index::extract::extract_file;

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("extract-name-start")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn class_method_and_parameter_name_starts() {
    platform_dir();
    let root = fixture("main");
    let text = "package p;\nclass A {\n  void f(String String) {}\n}\n";
    let path = root.join("A.java");
    std::fs::write(&path, text).unwrap();
    let config = AnalysisConfig {
        class_path: Vec::new(),
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: vec![root.clone()],
    };
    let extracted = extract_file(&config, &path, 0, text);
    assert!(!extracted.failed, "compile failed");
    let by_name = |name: &str| {
        extracted
            .declarations
            .iter()
            .find(|declaration| declaration.name == name)
            .unwrap_or_else(|| panic!("missing declaration {name}"))
    };
    assert_eq!(
        by_name("A").name_start as usize,
        text.find("A {").expect("class name"),
        "class name start must be the AST name position"
    );
    assert_eq!(
        by_name("f").name_start as usize,
        text.find("f(String").expect("method name"),
        "method name start must be the AST name position"
    );
    // The declaration range is derived from `name_start`.
    assert_eq!(by_name("A").range.start, by_name("A").name_start);
    // The parameter is a Declaration occurrence (its name is the second
    // `String`); a text fallback would pick the type name.
    let parameter_start = text.rfind("String").expect("parameter name") as u32;
    assert!(
        extracted.occurrences.iter().any(|occurrence| {
            occurrence.role
                == javac_ide_core::workspace::index::model::OccurrenceRole::Declaration
                && occurrence.range.start == parameter_start
        }),
        "parameter declaration occurrence must start at the parameter name"
    );
}
