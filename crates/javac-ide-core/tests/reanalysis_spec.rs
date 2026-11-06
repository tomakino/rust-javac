//! Re-analysis after edits must match javac: no spurious override errors.

use std::path::{Path, PathBuf};

use javac_analysis_api::WorkspaceService;

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::project::model::{
    BuildFingerprint, BuildSourceSet, Module, ProjectModel, SourceSetKind,
};
use javac_ide_core::project::provider::BuildSystemId;
use javac_ide_core::workspace::Workspace;
use javac_ide_core::workspace::analysis::FileAnalysis;

const TYPE_ADAPTER: &str = r#"
package com.google.gson;

public abstract class TypeAdapter<T> {
    public abstract T read() throws Exception;
    public abstract void write(T value) throws Exception;
}
"#;

const BASE: &str = r#"
package com.google.gson.internal.bind;

import com.google.gson.TypeAdapter;

public abstract class SerializationDelegatingTypeAdapter<T> extends TypeAdapter<T> {
    public abstract TypeAdapter<T> getSerializationDelegate();
}
"#;

const PROBE: &str = r#"
package com.google.gson;

import com.google.gson.internal.bind.SerializationDelegatingTypeAdapter;

class Probe {
    static class FutureTypeAdapter<T> extends SerializationDelegatingTypeAdapter<T> {
        private TypeAdapter<T> delegate = null;

        @Override public TypeAdapter<T> getSerializationDelegate() {
            return delegate;
        }

        @Override public T read() throws Exception {
            return delegate.read();
        }

        @Override public void write(T value) throws Exception {
            delegate.write(value);
        }
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

fn javac() -> Option<PathBuf> {
    let home = std::env::var_os("JAVA_HOME")?;
    let file = if cfg!(windows) { "javac.exe" } else { "javac" };
    let path = PathBuf::from(home).join("bin").join(file);
    path.exists().then_some(path)
}

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("reanalysis-spec")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src/main/java/com/google/gson/internal/bind")).unwrap();
    root
}

fn setup(name: &str) -> Option<(PathBuf, PathBuf, PathBuf)> {
    let javac = javac()?;
    let root = fixture(name);
    let src = root.join("src/main/java");
    std::fs::write(src.join("com/google/gson/TypeAdapter.java"), TYPE_ADAPTER).unwrap();
    std::fs::write(
        src.join("com/google/gson/internal/bind/SerializationDelegatingTypeAdapter.java"),
        BASE,
    )
    .unwrap();
    let probe = src.join("com/google/gson/Probe.java");
    std::fs::write(&probe, PROBE).unwrap();
    let classes = root.join("classes");
    std::fs::create_dir_all(&classes).unwrap();
    let status = std::process::Command::new(javac)
        .arg("-d")
        .arg(&classes)
        .arg(src.join("com/google/gson/TypeAdapter.java"))
        .arg(src.join("com/google/gson/internal/bind/SerializationDelegatingTypeAdapter.java"))
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    Some((root, probe, classes))
}

fn config(root: &Path, classes: PathBuf) -> AnalysisConfig {
    platform_dir();
    AnalysisConfig {
        class_path: vec![classes],
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: Vec::new(),
    }
}

fn project(root: &Path, classes: PathBuf) -> ProjectModel {
    ProjectModel {
        root: root.to_path_buf(),
        provider: BuildSystemId::NONE,
        modules: vec![Module {
            root: root.to_path_buf(),
            name: "probe".to_string(),
            source_sets: vec![BuildSourceSet::new(
                SourceSetKind::Main,
                vec![root.join("src/main/java")],
                Vec::new(),
                Some(classes),
            )],
            jdk_home: None,
            source_level: Some("17".to_string()),
        }],
        fingerprint: BuildFingerprint::default(),
    }
}

#[test]
fn classpath_reanalysis_matches_javac() {
    let Some((root, probe, classes)) = setup("direct") else {
        return; // no javac in this environment
    };
    let mut analysis = FileAnalysis::new(config(&root, classes));
    let first = analysis.diagnostics(&probe, None);
    assert!(first.is_empty(), "first round: {first:?}");

    // Editing the file must not introduce spurious override diagnostics.
    let edited = PROBE.replace(
        "private TypeAdapter<T> delegate = null;",
        "private TypeAdapter<T> delegate = null; int extra;",
    );
    let after = analysis.diagnostics(&probe, Some(&edited));
    assert!(after.is_empty(), "after edit: {after:?}");

    // A real error must still be reported (and only that error).
    let broken = PROBE.replace(
        "private TypeAdapter<T> delegate = null;",
        "private TypeAdapter<T> delegate = null; int bad = \"x\";",
    );
    let errors = analysis.diagnostics(&probe, Some(&broken));
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        !errors[0].message.contains("getSerializationDelegate"),
        "{errors:?}"
    );
}

#[test]
fn workspace_reanalysis_matches_javac() {
    let Some((root, probe, classes)) = setup("workspace") else {
        return;
    };
    platform_dir();
    let text = PROBE.to_string();
    let mut workspace = Workspace::new_from_project(&project(&root, classes), root.join("out"), None);
    let _ = workspace.symbols(&probe);
    let first = workspace.diagnostics(&probe, Some(&text));
    assert!(first.is_empty(), "first round: {first:?}");

    let broken = format!("{text}\nclass ProbeBad {{ int bad = \"x\"; }}\n");
    let errors = workspace.diagnostics(&probe, Some(&broken));
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        !errors[0].message.contains("getSerializationDelegate"),
        "{errors:?}"
    );
}
