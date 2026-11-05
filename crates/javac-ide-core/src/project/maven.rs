//! Maven project discovery and compile-classpath resolution.
//!
//! Mirrors the part of m2e that matters for analysis: read \`pom.xml\`,
//! resolve the compile/runtime classpath from the local repository
//! (\`~/.m2/repository\`), and derive source/output roots. No network access:
//! only artifacts already present in the local repository are used.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::xml;

/// One \`<dependency>\` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MavenDependency {
    pub group_id: String,
    pub artifact_id: String,
    pub version: Option<String>,
    pub scope: String,
}

/// A parsed Maven module.
#[derive(Debug, Clone)]
pub struct MavenProject {
    pub root: PathBuf,
    pub group_id: String,
    pub artifact_id: String,
    pub version: String,
    pub source_roots: Vec<PathBuf>,
    pub test_source_roots: Vec<PathBuf>,
    pub output_dir: PathBuf,
    pub test_output_dir: PathBuf,
    pub dependencies: Vec<MavenDependency>,
    /// Resolved jars on the compile/runtime classpath (ordered).
    pub classpath: Vec<PathBuf>,
    /// Nested modules (their poms are also imported).
    pub modules: Vec<PathBuf>,
}

/// Discover every Maven module under the given root directory.
pub fn discover_all(root: &Path) -> Vec<MavenProject> {
    let mut projects = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    let mut seen = std::collections::HashSet::new();
    while let Some(directory) = pending.pop() {
        let pom = directory.join("pom.xml");
        if !pom.is_file() || !seen.insert(directory.clone()) {
            continue;
        }
        let Some(project) = read_project(&directory) else {
            continue;
        };
        pending.extend(project.modules.iter().cloned());
        projects.push(project);
    }
    projects.sort_by(|left, right| left.root.cmp(&right.root));
    projects
}

/// Parse a single module (does not recurse into modules).
pub fn read_project(root: &Path) -> Option<MavenProject> {
    read_project_with_repository(root, &default_repository())
}

/// Default local repository (~/.m2/repository).
fn default_repository() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(".m2")
        .join("repository")
}

/// Parse a module using an explicit local repository (tests, custom setups).
pub fn read_project_with_repository(root: &Path, repository: &Path) -> Option<MavenProject> {
    let text = std::fs::read_to_string(root.join("pom.xml")).ok()?;
    let document = xml::parse(&text)?;
    let mut properties: HashMap<String, String> = HashMap::new();
    let mut parent_group = None;
    let mut parent_version = None;
    if let Some(parent) = document.child("parent") {
        parent_group = parent.child_text("groupId").map(str::to_string);
        parent_version = parent.child_text("version").map(str::to_string);
    }
    if let Some(properties_element) = document.child("properties") {
        for child in &properties_element.children {
            properties.insert(child.name.clone(), child.text.trim().to_string());
        }
    }
    let group_id = document
        .child_text("groupId")
        .or(parent_group.as_deref())
        .unwrap_or("")
        .to_string();
    let artifact_id = document.child_text("artifactId").unwrap_or("").to_string();
    let version = document
        .child_text("version")
        .or(parent_version.as_deref())
        .unwrap_or("")
        .to_string();
    properties.insert("project.groupId".to_string(), group_id.clone());
    properties.insert("project.artifactId".to_string(), artifact_id.clone());
    properties.insert("project.version".to_string(), version.clone());
    properties.insert("pom.version".to_string(), version.clone());

    let mut dependencies = Vec::new();
    if let Some(dependencies_element) = document.child("dependencies") {
        for dependency in dependencies_element.children_named("dependency") {
            let scope = dependency.child_text("scope").unwrap_or("compile").to_string();
            dependencies.push(MavenDependency {
                group_id: substitute(dependency.child_text("groupId").unwrap_or(""), &properties),
                artifact_id: substitute(
                    dependency.child_text("artifactId").unwrap_or(""),
                    &properties,
                ),
                version: dependency
                    .child_text("version")
                    .map(|raw| substitute(raw, &properties)),
                scope,
            });
        }
    }

    let mut modules = Vec::new();
    if let Some(modules_element) = document.child("modules") {
        for module in modules_element.children_named("module") {
            let path = root.join(module.text.trim());
            modules.push(path);
        }
    }

    let classpath = resolve_classpath(&dependencies, repository);
    Some(MavenProject {
        root: root.to_path_buf(),
        group_id,
        artifact_id,
        version,
        source_roots: vec![root.join("src").join("main").join("java")],
        test_source_roots: vec![root.join("src").join("test").join("java")],
        output_dir: root.join("target").join("classes"),
        test_output_dir: root.join("target").join("test-classes"),
        dependencies,
        classpath,
        modules,
    })
}

/// Replace \`\${...}\` placeholders from the properties map.
fn substitute(raw: &str, properties: &HashMap<String, String>) -> String {
    let mut current = raw.to_string();
    for _ in 0..8 {
        let Some(start) = current.find("${") else { break };
        let Some(end) = current[start..].find('}') else { break };
        let key = &current[start + 2..start + end];
        let Some(value) = properties.get(key) else { break };
        current.replace_range(start..start + end + 1, value);
    }
    current
}

/// Map compile/runtime dependencies to jars in the local repository.
fn resolve_classpath(dependencies: &[MavenDependency], repository: &Path) -> Vec<PathBuf> {
    let mut classpath = Vec::new();
    for dependency in dependencies {
        if dependency.scope == "test" || dependency.scope == "system" {
            continue;
        }
        let Some(version) = dependency.version.as_deref() else {
            continue;
        };
        let artifact_root = repository
            .join(dependency.group_id.replace('.', "/"))
            .join(&dependency.artifact_id)
            .join(version);
        let jar = artifact_root.join(format!(
            "{}-{}.jar",
            dependency.artifact_id, version
        ));
        if jar.is_file() && !classpath.contains(&jar) {
            classpath.push(jar);
        }
    }
    classpath
}