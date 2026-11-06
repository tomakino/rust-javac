//! Maven project discovery and compile-classpath resolution.
//!
//! Mirrors the part of m2e that matters for analysis: read pom.xml, resolve
//! the compile/runtime classpath from the local repository (~/.m2/repository),
//! and derive source/output roots. No network access: only artifacts already
//! present in the local repository are used.
//!
//! Resolution is deliberately offline and conservative:
//! - parent POM chain is followed through relativePath or the local repository
//! - parent properties and dependencyManagement are inherited
//! - BOM imports are expanded from the local repository
//! - transitive dependencies are resolved breadth-first (nearest wins)
//! - exclusions, optional and scopes are applied
//! Unsupported POM features are skipped rather than guessed.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use super::xml;

const MAX_DEPTH: usize = 32;

/// One dependency entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MavenDependency {
    pub group_id: String,
    pub artifact_id: String,
    pub version: Option<String>,
    pub scope: String,
    pub optional: bool,
    pub dependency_type: String,
    pub classifier: Option<String>,
    pub exclusions: Vec<(String, String)>,
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

#[derive(Debug, Clone)]
struct ParentRef {
    group_id: String,
    artifact_id: String,
    version: String,
    relative_path: Option<String>,
}

#[derive(Debug, Clone)]
struct RawPom {
    group_id: Option<String>,
    artifact_id: Option<String>,
    version: Option<String>,
    packaging: String,
    parent: Option<ParentRef>,
    properties: HashMap<String, String>,
    dependencies: Vec<MavenDependency>,
    management: Vec<MavenDependency>,
    modules: Vec<PathBuf>,
    source_dir: Option<String>,
    test_source_dir: Option<String>,
}

#[derive(Debug, Clone)]
struct ManagedDep {
    group_id: String,
    artifact_id: String,
    version: Option<String>,
    scope: Option<String>,
    dependency_type: String,
    classifier: Option<String>,
    exclusions: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct LoadedPom {
    group_id: String,
    artifact_id: String,
    version: String,
    packaging: String,
    properties: HashMap<String, String>,
    /// Raw dependency entries, inherited from parents and not yet interpolated.
    dependencies: Vec<MavenDependency>,
    /// Resolved dependencyManagement entries (own, parent and imported BOMs).
    management: Vec<ManagedDep>,
    modules: Vec<PathBuf>,
    source_dir: PathBuf,
    test_source_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct PendingDependency {
    dependency: MavenDependency,
    exclusions: Vec<(String, String)>,
}

/// Discover every Maven module under the given root directory.
pub fn discover_all(root: &Path) -> Vec<MavenProject> {
    let mut projects = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    let mut seen = HashSet::new();
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
    let loaded = load_pom(&root.join("pom.xml"), repository, 0)?;
    let management = managed_map(&loaded.management);
    let dependencies = effective_dependencies(&loaded, None);
    let classpath = resolve_transitive(&dependencies, &management, repository);
    Some(MavenProject {
        root: root.to_path_buf(),
        group_id: loaded.group_id,
        artifact_id: loaded.artifact_id,
        version: loaded.version,
        source_roots: vec![loaded.source_dir],
        test_source_roots: vec![loaded.test_source_dir],
        output_dir: root.join("target").join("classes"),
        test_output_dir: root.join("target").join("test-classes"),
        dependencies,
        classpath,
        modules: loaded.modules,
    })
}

fn read_raw_pom(path: &Path) -> Option<RawPom> {
    let text = std::fs::read_to_string(path).ok()?;
    let root = path.parent().unwrap_or(Path::new(""));
    let document = xml::parse(&text)?;
    let parent = document.child("parent").map(|parent| ParentRef {
        group_id: parent.child_text("groupId").unwrap_or("").to_string(),
        artifact_id: parent.child_text("artifactId").unwrap_or("").to_string(),
        version: parent.child_text("version").unwrap_or("").to_string(),
        relative_path: parent.child("relativePath").map(|element| element.text.trim().to_string()),
    });
    let mut properties = HashMap::new();
    if let Some(element) = document.child("properties") {
        for child in &element.children {
            properties.insert(child.name.clone(), child.text.trim().to_string());
        }
    }
    let dependencies = document
        .child("dependencies")
        .map(parse_dependencies)
        .unwrap_or_default();
    let management = document
        .child("dependencyManagement")
        .and_then(|element| element.child("dependencies"))
        .map(parse_dependencies)
        .unwrap_or_default();
    let modules = document
        .child("modules")
        .map(|element| {
            element
                .children_named("module")
                .map(|module| root.join(module.text.trim()))
                .collect()
        })
        .unwrap_or_default();
    let build = document.child("build");
    let source_dir = build
        .and_then(|element| element.child_text("sourceDirectory"))
        .map(str::to_string);
    let test_source_dir = build
        .and_then(|element| element.child_text("testSourceDirectory"))
        .map(str::to_string);
    Some(RawPom {
        group_id: document.child_text("groupId").map(str::to_string),
        artifact_id: document.child_text("artifactId").map(str::to_string),
        version: document.child_text("version").map(str::to_string),
        packaging: document.child_text("packaging").unwrap_or("jar").to_string(),
        parent,
        properties,
        dependencies,
        management,
        modules,
        source_dir,
        test_source_dir,
    })
}

fn parse_dependencies(element: &xml::Element) -> Vec<MavenDependency> {
    element
        .children_named("dependency")
        .map(parse_dependency)
        .collect()
}

fn parse_dependency(element: &xml::Element) -> MavenDependency {
    let exclusions = element
        .child("exclusions")
        .map(|exclusions| {
            exclusions
                .children_named("exclusion")
                .filter_map(|exclusion| {
                    let group = exclusion.child_text("groupId")?.trim();
                    let artifact = exclusion.child_text("artifactId")?.trim();
                    Some((group.to_string(), artifact.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();
    MavenDependency {
        group_id: element.child_text("groupId").unwrap_or("").to_string(),
        artifact_id: element.child_text("artifactId").unwrap_or("").to_string(),
        version: element.child_text("version").map(str::to_string),
        scope: element.child_text("scope").unwrap_or("compile").to_string(),
        optional: element
            .child_text("optional")
            .map(|value| value.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        dependency_type: element.child_text("type").unwrap_or("jar").to_string(),
        classifier: element
            .child_text("classifier")
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        exclusions,
    }
}

fn load_pom(path: &Path, repository: &Path, depth: usize) -> Option<LoadedPom> {
    if depth > MAX_DEPTH {
        return None;
    }
    let raw = read_raw_pom(path)?;
    let root = path.parent().unwrap_or(Path::new(""));
    let parent = raw.parent.as_ref().and_then(|parent| {
        let parent_path = find_parent_path(root, parent, repository, &raw.properties)?;
        load_pom(&parent_path, repository, depth + 1)
    });

    let mut properties = parent
        .as_ref()
        .map(|parent| parent.properties.clone())
        .unwrap_or_default();
    properties.extend(raw.properties.clone());

    let group_raw = raw
        .group_id
        .clone()
        .or_else(|| parent.as_ref().map(|parent| parent.group_id.clone()))
        .unwrap_or_default();
    let version_raw = raw
        .version
        .clone()
        .or_else(|| parent.as_ref().map(|parent| parent.version.clone()))
        .unwrap_or_default();
    let group_id = substitute(&group_raw, &properties);
    let artifact_id = substitute(raw.artifact_id.as_deref().unwrap_or(""), &properties);
    let version = substitute(&version_raw, &properties);

    properties.insert("project.groupId".to_string(), group_id.clone());
    properties.insert("project.artifactId".to_string(), artifact_id.clone());
    properties.insert("project.version".to_string(), version.clone());
    properties.insert("project.packaging".to_string(), raw.packaging.clone());
    properties.insert("pom.groupId".to_string(), group_id.clone());
    properties.insert("pom.artifactId".to_string(), artifact_id.clone());
    properties.insert("pom.version".to_string(), version.clone());
    properties.insert("project.basedir".to_string(), root.to_string_lossy().to_string());
    if let Some(parent) = &parent {
        properties.insert("project.parent.groupId".to_string(), parent.group_id.clone());
        properties.insert("project.parent.artifactId".to_string(), parent.artifact_id.clone());
        properties.insert("project.parent.version".to_string(), parent.version.clone());
    }
    let snapshot = properties.clone();
    for value in properties.values_mut() {
        *value = substitute(value, &snapshot);
    }

    let mut dependencies = parent
        .as_ref()
        .map(|parent| parent.dependencies.clone())
        .unwrap_or_default();
    dependencies.extend(raw.dependencies.clone());

    let mut imported = Vec::new();
    let mut own = Vec::new();
    for dependency in &raw.management {
        let managed = managed_dependency(dependency, &properties);
        if managed.dependency_type == "pom" && managed.scope.as_deref() == Some("import") {
            let Some(version) = managed.version.as_deref() else {
                continue;
            };
            let bom_path = artifact_pom_path(
                repository,
                &managed.group_id,
                &managed.artifact_id,
                version,
            );
            if let Some(bom) = load_pom(&bom_path, repository, depth + 1) {
                imported.extend(bom.management);
            }
        } else {
            own.push(managed);
        }
    }
    let mut management = imported;
    if let Some(parent) = &parent {
        management.extend(parent.management.clone());
    }
    management.extend(own);

    let source_dir = raw
        .source_dir
        .as_deref()
        .map(|value| substitute(value, &properties))
        .filter(|value| !value.is_empty())
        .map(|value| root.join(value))
        .unwrap_or_else(|| root.join("src").join("main").join("java"));
    let test_source_dir = raw
        .test_source_dir
        .as_deref()
        .map(|value| substitute(value, &properties))
        .filter(|value| !value.is_empty())
        .map(|value| root.join(value))
        .unwrap_or_else(|| root.join("src").join("test").join("java"));

    Some(LoadedPom {
        group_id,
        artifact_id,
        version,
        packaging: raw.packaging,
        properties,
        dependencies,
        management,
        modules: raw.modules,
        source_dir,
        test_source_dir,
    })
}

fn find_parent_path(
    root: &Path,
    parent: &ParentRef,
    repository: &Path,
    properties: &HashMap<String, String>,
) -> Option<PathBuf> {
    let relative = match parent.relative_path.as_deref() {
        Some("") => None,
        Some(value) => Some(value),
        None => Some("../pom.xml"),
    };
    if let Some(relative) = relative {
        let candidate = root.join(relative);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let group_id = substitute(&parent.group_id, properties);
    let artifact_id = substitute(&parent.artifact_id, properties);
    let version = substitute(&parent.version, properties);
    if group_id.is_empty() || artifact_id.is_empty() || version.is_empty() {
        return None;
    }
    let candidate = artifact_pom_path(repository, &group_id, &artifact_id, &version);
    candidate.is_file().then_some(candidate)
}

fn managed_dependency(dependency: &MavenDependency, properties: &HashMap<String, String>) -> ManagedDep {
    ManagedDep {
        group_id: substitute(&dependency.group_id, properties),
        artifact_id: substitute(&dependency.artifact_id, properties),
        version: dependency
            .version
            .as_deref()
            .map(|value| substitute(value, properties))
            .filter(|value| !value.is_empty()),
        scope: Some(substitute(&dependency.scope, properties)).filter(|value| !value.is_empty()),
        dependency_type: substitute(&dependency.dependency_type, properties),
        classifier: dependency
            .classifier
            .as_deref()
            .map(|value| substitute(value, properties))
            .filter(|value| !value.is_empty()),
        exclusions: dependency
            .exclusions
            .iter()
            .map(|(group, artifact)| {
                (
                    substitute(group, properties),
                    substitute(artifact, properties),
                )
            })
            .collect(),
    }
}

fn managed_map(management: &[ManagedDep]) -> HashMap<(String, String), ManagedDep> {
    let mut map = HashMap::new();
    for entry in management {
        map.insert(
            (entry.group_id.clone(), entry.artifact_id.clone()),
            entry.clone(),
        );
    }
    map
}

fn effective_dependencies(
    pom: &LoadedPom,
    root_management: Option<&HashMap<(String, String), ManagedDep>>,
) -> Vec<MavenDependency> {
    let own_management = managed_map(&pom.management);
    let mut result = Vec::new();
    for dependency in &pom.dependencies {
        let group_id = substitute(&dependency.group_id, &pom.properties);
        let artifact_id = substitute(&dependency.artifact_id, &pom.properties);
        let key = (group_id.clone(), artifact_id.clone());
        let mut version = dependency
            .version
            .as_deref()
            .map(|value| substitute(value, &pom.properties))
            .filter(|value| !value.is_empty());
        if version.is_none() {
            version = own_management
                .get(&key)
                .and_then(|managed| managed.version.clone());
        }
        if let Some(root_management) = root_management {
            if let Some(managed) = root_management.get(&key) {
                if managed.version.is_some() {
                    version = managed.version.clone();
                }
            }
        }
        let mut exclusions: Vec<(String, String)> = dependency
            .exclusions
            .iter()
            .map(|(group, artifact)| {
                (
                    substitute(group, &pom.properties),
                    substitute(artifact, &pom.properties),
                )
            })
            .collect();
        if let Some(managed) = own_management.get(&key) {
            exclusions.extend(managed.exclusions.clone());
        }
        result.push(MavenDependency {
            group_id,
            artifact_id,
            version,
            scope: substitute(&dependency.scope, &pom.properties),
            optional: dependency.optional,
            dependency_type: substitute(&dependency.dependency_type, &pom.properties),
            classifier: dependency
                .classifier
                .as_deref()
                .map(|value| substitute(value, &pom.properties))
                .filter(|value| !value.is_empty()),
            exclusions,
        });
    }
    result
}

fn resolve_transitive(
    direct: &[MavenDependency],
    root_management: &HashMap<(String, String), ManagedDep>,
    repository: &Path,
) -> Vec<PathBuf> {
    let mut queue = VecDeque::new();
    for dependency in direct {
        if root_scope(&dependency.scope) {
            queue.push_back(PendingDependency {
                dependency: dependency.clone(),
                exclusions: Vec::new(),
            });
        }
    }
    let mut seen = HashSet::new();
    let mut jars = Vec::new();
    let mut cache: HashMap<PathBuf, Option<LoadedPom>> = HashMap::new();
    while let Some(pending) = queue.pop_front() {
        let dependency = &pending.dependency;
        if is_excluded(&pending.exclusions, dependency) {
            continue;
        }
        let key = format!(
            "{}:{}:{}:{}",
            dependency.group_id,
            dependency.artifact_id,
            dependency.dependency_type,
            dependency.classifier.as_deref().unwrap_or("")
        );
        if !seen.insert(key) {
            continue;
        }
        let Some(version) = dependency.version.clone() else {
            continue;
        };
        if dependency.dependency_type != "pom" {
            let jar = artifact_jar_path(repository, dependency, &version);
            if jar.is_file() && !jars.contains(&jar) {
                jars.push(jar);
            }
        }
        if !transitive_parent_scope(&dependency.scope) {
            continue;
        }
        let pom_path = artifact_pom_path(
            repository,
            &dependency.group_id,
            &dependency.artifact_id,
            &version,
        );
        if !pom_path.is_file() {
            continue;
        }
        let loaded = load_pom_cached(&pom_path, repository, &mut cache);
        let Some(loaded) = loaded else {
            continue;
        };
        let children = effective_dependencies(&loaded, Some(root_management));
        for child in children {
            if child.optional {
                continue;
            }
            if !transitive_child_scope(&dependency.scope, &child.scope) {
                continue;
            }
            let mut exclusions = pending.exclusions.clone();
            exclusions.extend(dependency.exclusions.clone());
            queue.push_back(PendingDependency {
                dependency: child,
                exclusions,
            });
        }
    }
    jars
}

fn load_pom_cached(
    path: &Path,
    repository: &Path,
    cache: &mut HashMap<PathBuf, Option<LoadedPom>>,
) -> Option<LoadedPom> {
    if let Some(cached) = cache.get(path) {
        return cached.clone();
    }
    let loaded = load_pom(path, repository, 0);
    cache.insert(path.to_path_buf(), loaded.clone());
    loaded
}

fn root_scope(scope: &str) -> bool {
    matches!(scope, "compile" | "runtime" | "provided")
}

fn transitive_parent_scope(scope: &str) -> bool {
    matches!(scope, "compile" | "runtime")
}

fn transitive_child_scope(parent: &str, child: &str) -> bool {
    matches!(parent, "compile" | "runtime") && matches!(child, "compile" | "runtime")
}

fn is_excluded(exclusions: &[(String, String)], dependency: &MavenDependency) -> bool {
    exclusions.iter().any(|(group, artifact)| {
        (group == "*" || group == &dependency.group_id)
            && (artifact == "*" || artifact == &dependency.artifact_id)
    })
}

fn artifact_pom_path(
    repository: &Path,
    group_id: &str,
    artifact_id: &str,
    version: &str,
) -> PathBuf {
    repository
        .join(group_id.replace('.', "/"))
        .join(artifact_id)
        .join(version)
        .join(format!("{artifact_id}-{version}.pom"))
}

fn artifact_jar_path(repository: &Path, dependency: &MavenDependency, version: &str) -> PathBuf {
    let file = match &dependency.classifier {
        Some(classifier) if !classifier.is_empty() => {
            format!("{}-{}-{}.jar", dependency.artifact_id, version, classifier)
        }
        _ => format!("{}-{}.jar", dependency.artifact_id, version),
    };
    repository
        .join(dependency.group_id.replace('.', "/"))
        .join(&dependency.artifact_id)
        .join(version)
        .join(file)
}

/// Replace ${...} placeholders from the properties map.
fn substitute(raw: &str, properties: &HashMap<String, String>) -> String {
    let mut current = raw.to_string();
    for _ in 0..8 {
        let Some(start) = current.find("${") else {
            break;
        };
        let Some(end) = current[start..].find('}') else {
            break;
        };
        let key = &current[start + 2..start + end];
        let Some(value) = properties.get(key) else {
            break;
        };
        current.replace_range(start..start + end + 1, value);
    }
    current
}
