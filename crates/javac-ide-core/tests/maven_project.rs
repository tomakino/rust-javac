//! Maven project model tests.

use javac_ide_core::project::maven;

fn write(path: &std::path::Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, text).unwrap();
}

#[test]
fn parses_pom_and_resolves_local_repository_jars() {
    let root = std::path::Path::new("target/tmp/maven-project-test/module-a");
    let repository = std::path::Path::new("target/tmp/maven-project-test/repo");
    write(
        &root.join("pom.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.example</groupId>
  <artifactId>module-a</artifactId>
  <version>1.2.3</version>
  <properties>
    <lib.version>2.0</lib.version>
  </properties>
  <dependencies>
    <dependency>
      <groupId>com.example</groupId>
      <artifactId>lib</artifactId>
      <version>${lib.version}</version>
    </dependency>
    <dependency>
      <groupId>com.example</groupId>
      <artifactId>test-lib</artifactId>
      <version>1.0</version>
      <scope>test</scope>
    </dependency>
  </dependencies>
</project>
"#,
    );
    write(&repository.join("com/example/lib/2.0/lib-2.0.jar"), "");
    write(&repository.join("com/example/test-lib/1.0/test-lib-1.0.jar"), "");
    let project = maven::read_project_with_repository(root, repository).expect("project");
    assert_eq!(project.group_id, "com.example");
    assert_eq!(project.artifact_id, "module-a");
    assert_eq!(project.version, "1.2.3");
    assert_eq!(project.dependencies.len(), 2);
    assert_eq!(project.classpath.len(), 1, "test scope must be excluded");
    assert!(project.classpath[0].ends_with("lib-2.0.jar"));
    assert_eq!(
        project.output_dir,
        root.join("target").join("classes")
    );
}

#[test]
fn discovers_nested_modules() {
    let root = std::path::Path::new("target/tmp/maven-project-test/aggregator");
    let repository = std::path::Path::new("target/tmp/maven-project-test/repo");
    write(
        &root.join("pom.xml"),
        r#"<project>
  <groupId>com.example</groupId>
  <artifactId>aggregator</artifactId>
  <version>1.0</version>
  <packaging>pom</packaging>
  <modules>
    <module>child</module>
  </modules>
</project>
"#,
    );
    write(
        &root.join("child").join("pom.xml"),
        r#"<project>
  <parent>
    <groupId>com.example</groupId>
    <artifactId>aggregator</artifactId>
    <version>1.0</version>
  </parent>
  <artifactId>child</artifactId>
</project>
"#,
    );
    let projects = maven::discover_all(root);
    assert_eq!(projects.len(), 2);
    let child = projects
        .iter()
        .find(|project| project.artifact_id == "child")
        .expect("child module");
    assert_eq!(child.group_id, "com.example");
    assert_eq!(child.version, "1.0");
    let _ = repository;
}