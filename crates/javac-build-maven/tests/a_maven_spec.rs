//! Audit A: Maven project shapes (module discovery, source roots, local
//! repository resolution).
//!
//! These tests exercise the project layer only; the end-to-end LSP evidence is
//! `scripts/lsp/probe_a_maven.py`.

use std::path::{Path, PathBuf};

use javac_build_maven::{discover_all, read_project, read_project_with_repository};

fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("a-maven-spec")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn write_pom(root: &Path, text: &str) {
    std::fs::write(root.join("pom.xml"), text).unwrap();
}

fn single_pom(artefact: &str, build: &str) -> String {
    format!(
        "<project><modelVersion>4.0.0</modelVersion><groupId>com.audit</groupId>\
<artifactId>{artefact}</artifactId><version>1.0</version>{build}</project>\n"
    )
}

#[test]
fn single_module_standard_layout() {
    let root = fixture("single");
    write_pom(&root, &single_pom("single", ""));
    std::fs::create_dir_all(root.join("src/main/java/com/audit")).unwrap();
    std::fs::write(
        root.join("src/main/java/com/audit/App.java"),
        "package com.audit;\n\npublic class App {\n}\n",
    )
    .unwrap();
    let project = read_project(&root).expect("project");
    assert_eq!(project.source_roots, vec![root.join("src/main/java")]);
    assert_eq!(project.output_dir, root.join("target/classes"));
    assert!(project.classpath.is_empty(), "{:?}", project.classpath);
}

#[test]
fn multi_module_discovery_keeps_each_source_root() {
    let root = fixture("multi");
    write_pom(
        &root,
        "<project><modelVersion>4.0.0</modelVersion><groupId>com.audit</groupId>\
<artifactId>parent</artifactId><version>1.0</version><packaging>pom</packaging>\
<modules><module>m-a</module><module>m-b</module></modules></project>\n",
    );
    for module in ["m-a", "m-b"] {
        let dir = root.join(module);
        std::fs::create_dir_all(dir.join("src/main/java/com/audit")).unwrap();
        write_pom(
            &dir,
            &format!(
                "<project><modelVersion>4.0.0</modelVersion>\
<parent><groupId>com.audit</groupId><artifactId>parent</artifactId><version>1.0</version>\
<relativePath>../pom.xml</relativePath></parent>\
<artifactId>{module}</artifactId></project>\n"
            ),
        );
        std::fs::write(
            dir.join("src/main/java/com/audit/App.java"),
            "package com.audit;\n\npublic class App {\n}\n",
        )
        .unwrap();
    }
    let projects = discover_all(&root);
    assert_eq!(projects.len(), 3, "{projects:?}");
    let roots: Vec<PathBuf> = projects
        .iter()
        .flat_map(|project| project.source_roots.clone())
        .collect();
    assert!(roots.contains(&root.join("m-a/src/main/java")), "{roots:?}");
    assert!(roots.contains(&root.join("m-b/src/main/java")), "{roots:?}");
}

#[test]
fn custom_source_directory_is_used() {
    let root = fixture("custom");
    write_pom(
        &root,
        &single_pom(
            "custom",
            "<build><sourceDirectory>src/main/custom</sourceDirectory></build>",
        ),
    );
    std::fs::create_dir_all(root.join("src/main/custom/com/audit")).unwrap();
    let project = read_project(&root).expect("project");
    assert_eq!(project.source_roots, vec![root.join("src/main/custom")]);
}

#[test]
fn generated_sources_are_not_a_source_root() {
    let root = fixture("generated");
    write_pom(&root, &single_pom("generated", ""));
    std::fs::create_dir_all(root.join("src/main/java/com/audit")).unwrap();
    std::fs::create_dir_all(root.join("target/generated-sources/annotations/com/audit")).unwrap();
    let project = read_project(&root).expect("project");
    assert_eq!(project.source_roots, vec![root.join("src/main/java")]);
    assert!(
        project
            .source_roots
            .iter()
            .all(|root| !root.to_string_lossy().contains("generated-sources")),
        "{:?}",
        project.source_roots
    );
}

#[test]
fn dependency_jars_resolve_from_the_local_repository() {
    let root = fixture("dependency");
    let repository = root.join("repo");
    let artifact = repository.join("com/audit/lib/1.0");
    std::fs::create_dir_all(&artifact).unwrap();
    std::fs::write(artifact.join("lib-1.0.jar"), b"").unwrap();
    std::fs::write(artifact.join("lib-1.0-sources.jar"), b"").unwrap();
    write_pom(
        &root,
        &single_pom(
            "dep",
            "<dependencies><dependency><groupId>com.audit</groupId>\
<artifactId>lib</artifactId><version>1.0</version></dependency></dependencies>",
        ),
    );
    let project =
        read_project_with_repository(&root, &repository).expect("project with repository");
    assert_eq!(
        project.classpath,
        vec![artifact.join("lib-1.0.jar")],
        "{:?}",
        project.classpath
    );
}
