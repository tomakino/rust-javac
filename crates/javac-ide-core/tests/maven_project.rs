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


#[test]
fn resolves_parent_management_and_transitive_dependencies() {
    let base = std::path::Path::new("target/tmp/maven-transitive-test/parent");
    let project_root = base.join("app");
    let repository = base.join("repo");
    write(
        &project_root.join("pom.xml"),
        r#"<project>
  <parent>
    <groupId>com.example</groupId>
    <artifactId>parent</artifactId>
    <version>1.0</version>
  </parent>
  <artifactId>app</artifactId>
  <dependencies>
    <dependency>
      <groupId>com.example</groupId>
      <artifactId>lib</artifactId>
    </dependency>
  </dependencies>
</project>"#,
    );
    write(
        &repository.join("com/example/parent/1.0/parent-1.0.pom"),
        r#"<project>
  <groupId>com.example</groupId>
  <artifactId>parent</artifactId>
  <version>1.0</version>
  <packaging>pom</packaging>
  <properties>
    <lib.version>2.0</lib.version>
  </properties>
  <dependencyManagement>
    <dependencies>
      <dependency>
        <groupId>com.example</groupId>
        <artifactId>lib</artifactId>
        <version>${lib.version}</version>
      </dependency>
    </dependencies>
  </dependencyManagement>
</project>"#,
    );
    write(
        &repository.join("com/example/lib/2.0/lib-2.0.pom"),
        r#"<project>
  <groupId>com.example</groupId>
  <artifactId>lib</artifactId>
  <version>2.0</version>
  <dependencies>
    <dependency>
      <groupId>com.example</groupId>
      <artifactId>transitive</artifactId>
      <version>3.0</version>
    </dependency>
  </dependencies>
</project>"#,
    );
    write(&repository.join("com/example/lib/2.0/lib-2.0.jar"), "");
    write(&repository.join("com/example/transitive/3.0/transitive-3.0.pom"), "<project><groupId>com.example</groupId><artifactId>transitive</artifactId><version>3.0</version></project>");
    write(&repository.join("com/example/transitive/3.0/transitive-3.0.jar"), "");

    let project = maven::read_project_with_repository(&project_root, &repository).expect("project");
    assert_eq!(project.dependencies.len(), 1);
    assert_eq!(project.dependencies[0].version.as_deref(), Some("2.0"));
    let names: Vec<String> = project
        .classpath
        .iter()
        .filter_map(|path| path.file_name().map(|name| name.to_string_lossy().to_string()))
        .collect();
    assert!(names.contains(&"lib-2.0.jar".to_string()));
    assert!(names.contains(&"transitive-3.0.jar".to_string()));
}

#[test]
fn resolves_imported_bom_management() {
    let base = std::path::Path::new("target/tmp/maven-transitive-test/bom");
    let project_root = base.join("app");
    let repository = base.join("repo");
    write(
        &project_root.join("pom.xml"),
        r#"<project>
  <groupId>com.example</groupId>
  <artifactId>app</artifactId>
  <version>1.0</version>
  <dependencyManagement>
    <dependencies>
      <dependency>
        <groupId>com.example</groupId>
        <artifactId>bom</artifactId>
        <version>1.0</version>
        <type>pom</type>
        <scope>import</scope>
      </dependency>
    </dependencies>
  </dependencyManagement>
  <dependencies>
    <dependency>
      <groupId>com.example</groupId>
      <artifactId>lib</artifactId>
    </dependency>
  </dependencies>
</project>"#,
    );
    write(
        &repository.join("com/example/bom/1.0/bom-1.0.pom"),
        r#"<project>
  <groupId>com.example</groupId>
  <artifactId>bom</artifactId>
  <version>1.0</version>
  <packaging>pom</packaging>
  <dependencyManagement>
    <dependencies>
      <dependency>
        <groupId>com.example</groupId>
        <artifactId>lib</artifactId>
        <version>3.0</version>
      </dependency>
    </dependencies>
  </dependencyManagement>
</project>"#,
    );
    write(
        &repository.join("com/example/lib/3.0/lib-3.0.pom"),
        "<project><groupId>com.example</groupId><artifactId>lib</artifactId><version>3.0</version></project>",
    );
    write(&repository.join("com/example/lib/3.0/lib-3.0.jar"), "");

    let project = maven::read_project_with_repository(&project_root, &repository).expect("project");
    assert_eq!(project.dependencies[0].version.as_deref(), Some("3.0"));
    assert_eq!(project.classpath.len(), 1);
    assert!(project.classpath[0].ends_with("lib-3.0.jar"));
}

#[test]
fn applies_exclusions_optional_and_scopes() {
    let base = std::path::Path::new("target/tmp/maven-transitive-test/scopes");
    let project_root = base.join("app");
    let repository = base.join("repo");
    write(
        &project_root.join("pom.xml"),
        r#"<project>
  <groupId>com.example</groupId>
  <artifactId>app</artifactId>
  <version>1.0</version>
  <dependencies>
    <dependency>
      <groupId>com.example</groupId>
      <artifactId>parent-lib</artifactId>
      <version>1.0</version>
      <exclusions>
        <exclusion>
          <groupId>com.example</groupId>
          <artifactId>excluded-lib</artifactId>
        </exclusion>
      </exclusions>
    </dependency>
  </dependencies>
</project>"#,
    );
    write(
        &repository.join("com/example/parent-lib/1.0/parent-lib-1.0.pom"),
        r#"<project>
  <groupId>com.example</groupId>
  <artifactId>parent-lib</artifactId>
  <version>1.0</version>
  <dependencies>
    <dependency><groupId>com.example</groupId><artifactId>excluded-lib</artifactId><version>1.0</version></dependency>
    <dependency><groupId>com.example</groupId><artifactId>optional-lib</artifactId><version>1.0</version><optional>true</optional></dependency>
    <dependency><groupId>com.example</groupId><artifactId>test-lib</artifactId><version>1.0</version><scope>test</scope></dependency>
    <dependency><groupId>com.example</groupId><artifactId>kept-lib</artifactId><version>1.0</version></dependency>
  </dependencies>
</project>"#,
    );
    for artifact in ["parent-lib", "excluded-lib", "optional-lib", "test-lib", "kept-lib"] {
        write(&repository.join(format!("com/example/{artifact}/1.0/{artifact}-1.0.jar")), "");
    }

    let project = maven::read_project_with_repository(&project_root, &repository).expect("project");
    let names: Vec<String> = project
        .classpath
        .iter()
        .filter_map(|path| path.file_name().map(|name| name.to_string_lossy().to_string()))
        .collect();
    assert!(names.contains(&"parent-lib-1.0.jar".to_string()));
    assert!(names.contains(&"kept-lib-1.0.jar".to_string()));
    assert!(!names.contains(&"excluded-lib-1.0.jar".to_string()));
    assert!(!names.contains(&"optional-lib-1.0.jar".to_string()));
    assert!(!names.contains(&"test-lib-1.0.jar".to_string()));
}

#[test]
fn nearest_dependency_version_wins() {
    let base = std::path::Path::new("target/tmp/maven-transitive-test/nearest");
    let project_root = base.join("app");
    let repository = base.join("repo");
    write(
        &project_root.join("pom.xml"),
        r#"<project>
  <groupId>com.example</groupId>
  <artifactId>app</artifactId>
  <version>1.0</version>
  <dependencies>
    <dependency><groupId>com.example</groupId><artifactId>a</artifactId><version>1.0</version></dependency>
    <dependency><groupId>com.example</groupId><artifactId>b</artifactId><version>1.0</version></dependency>
  </dependencies>
</project>"#,
    );
    write(
        &repository.join("com/example/a/1.0/a-1.0.pom"),
        r#"<project><groupId>com.example</groupId><artifactId>a</artifactId><version>1.0</version><dependencies><dependency><groupId>com.example</groupId><artifactId>c</artifactId><version>1.0</version></dependency></dependencies></project>"#,
    );
    write(
        &repository.join("com/example/b/1.0/b-1.0.pom"),
        r#"<project><groupId>com.example</groupId><artifactId>b</artifactId><version>1.0</version><dependencies><dependency><groupId>com.example</groupId><artifactId>c</artifactId><version>2.0</version></dependency></dependencies></project>"#,
    );
    for artifact in ["a", "b"] {
        write(&repository.join(format!("com/example/{artifact}/1.0/{artifact}-1.0.jar")), "");
    }
    for version in ["1.0", "2.0"] {
        write(&repository.join(format!("com/example/c/{version}/c-{version}.jar")), "");
        write(&repository.join(format!("com/example/c/{version}/c-{version}.pom")), &format!("<project><groupId>com.example</groupId><artifactId>c</artifactId><version>{version}</version></project>"));
    }

    let project = maven::read_project_with_repository(&project_root, &repository).expect("project");
    let names: Vec<String> = project
        .classpath
        .iter()
        .filter_map(|path| path.file_name().map(|name| name.to_string_lossy().to_string()))
        .collect();
    assert!(names.contains(&"c-1.0.jar".to_string()), "classpath: {names:?}");
    assert!(!names.contains(&"c-2.0.jar".to_string()), "classpath: {names:?}");
}
