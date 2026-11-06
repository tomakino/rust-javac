//! Maven provider tests: model mapping, fingerprint, and class-path parity.

use std::path::{Path, PathBuf};

use javac_build_maven::MavenProvider;
use javac_project_api::{BuildSystemId, BuildSystemProvider, LoadContext, SourceSetKind};

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, text).unwrap();
}

fn fixture(name: &str) -> PathBuf {
    let root = PathBuf::from("target/tmp/maven-provider-spec").join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn registry() -> Vec<Box<dyn BuildSystemProvider>> {
    vec![Box::new(MavenProvider::new())]
}

const AGGREGATOR_POM: &str = r#"<project>
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.example</groupId>
  <artifactId>aggregator</artifactId>
  <version>1.0</version>
  <packaging>pom</packaging>
  <modules>
    <module>child</module>
  </modules>
</project>
"#;

const CHILD_POM: &str = r#"<project>
  <parent>
    <groupId>com.example</groupId>
    <artifactId>aggregator</artifactId>
    <version>1.0</version>
  </parent>
  <artifactId>child</artifactId>
</project>
"#;

#[test]
fn detect_requires_a_pom() {
    let root = fixture("detect");
    let provider = MavenProvider::new();
    assert!(!provider.detect(&root));
    write(&root.join("pom.xml"), AGGREGATOR_POM);
    assert!(provider.detect(&root));
}

#[test]
fn maven_provider_maps_modules_and_source_sets() {
    let root = fixture("modules");
    write(&root.join("pom.xml"), AGGREGATOR_POM);
    write(&root.join("child").join("pom.xml"), CHILD_POM);

    let model = javac_project_api::load_with(&registry(), &root, &LoadContext::default());
    assert_eq!(model.provider, BuildSystemId::MAVEN);
    assert_eq!(model.modules.len(), 2);
    let child = model
        .modules
        .iter()
        .find(|module| module.name == "com.example:child")
        .expect("child module");
    assert_eq!(child.root, root.join("child"));

    let main = child
        .source_sets
        .iter()
        .find(|set| set.kind == SourceSetKind::Main)
        .expect("main source set");
    assert!(main.roots.iter().any(|path| path.ends_with("src/main/java")));
    assert_eq!(
        main.output.as_deref(),
        Some(child.root.join("target/classes").as_path())
    );

    let test = child
        .source_sets
        .iter()
        .find(|set| set.kind == SourceSetKind::Test)
        .expect("test source set");
    assert!(test.roots.iter().any(|path| path.ends_with("src/test/java")));
    assert_eq!(
        test.output.as_deref(),
        Some(child.root.join("target/test-classes").as_path())
    );
    assert!(model.class_path().is_empty());
}

#[test]
fn maven_model_matches_direct_discovery() {
    let root = fixture("equivalence");
    write(&root.join("pom.xml"), AGGREGATOR_POM);
    write(&root.join("child").join("pom.xml"), CHILD_POM);

    let projects = javac_build_maven::maven::discover_all(&root);
    let mut expected_sources = Vec::new();
    let mut expected_classpath = Vec::new();
    for project in &projects {
        for path in project
            .source_roots
            .iter()
            .chain(project.test_source_roots.iter())
        {
            if !expected_sources.contains(path) {
                expected_sources.push(path.clone());
            }
        }
        for entry in &project.classpath {
            if !expected_classpath.contains(entry) {
                expected_classpath.push(entry.clone());
            }
        }
    }
    let model = javac_project_api::load_with(&registry(), &root, &LoadContext::default());
    assert_eq!(model.source_roots(), expected_sources);
    assert_eq!(model.class_path(), expected_classpath);
}

#[test]
fn fingerprint_tracks_pom_changes() {
    let root = fixture("fingerprint");
    write(&root.join("pom.xml"), AGGREGATOR_POM);
    let provider = MavenProvider::new();
    let ctx = LoadContext::default();
    let before = provider.load(&root, &ctx).expect("model");
    let before_hash = before
        .fingerprint
        .inputs
        .iter()
        .find(|(path, _)| path.ends_with("pom.xml"))
        .map(|(_, hash)| *hash)
        .expect("pom input");

    write(
        &root.join("pom.xml"),
        &AGGREGATOR_POM.replace(
            "<artifactId>aggregator</artifactId>",
            "<artifactId>renamed</artifactId>",
        ),
    );
    let after = provider.load(&root, &ctx).expect("model");
    let after_hash = after
        .fingerprint
        .inputs
        .iter()
        .find(|(path, _)| path.ends_with("pom.xml"))
        .map(|(_, hash)| *hash)
        .expect("pom input");
    assert_ne!(before_hash, after_hash);
    assert_ne!(before.fingerprint.model_hash, after.fingerprint.model_hash);
}

#[test]
fn invalid_pom_is_rejected() {
    let root = fixture("fallback");
    std::fs::write(root.join("pom.xml"), "this is not xml").unwrap();
    let result = MavenProvider::new().load(&root, &LoadContext::default());
    assert!(matches!(
        result,
        Err(javac_project_api::LoadError::NotDetected)
    ));
}

#[test]
fn cancelled_load_times_out() {
    let root = fixture("cancelled");
    write(&root.join("pom.xml"), AGGREGATOR_POM);
    let ctx = LoadContext::default();
    ctx.cancel.cancel();
    let result = MavenProvider::new().load(&root, &ctx);
    assert!(matches!(result, Err(javac_project_api::LoadError::Timeout)));
}

#[test]
fn watch_paths_are_the_discovered_poms() {
    let root = fixture("watch");
    write(&root.join("pom.xml"), AGGREGATOR_POM);
    write(&root.join("child").join("pom.xml"), CHILD_POM);
    let paths = MavenProvider::new().watch_paths(&root);
    assert_eq!(paths.len(), 2);
    assert!(paths.iter().all(|path| path.ends_with("pom.xml")));
}
