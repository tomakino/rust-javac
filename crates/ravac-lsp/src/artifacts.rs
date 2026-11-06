//! Build-artifact maintenance.
//!
//! The analysis compiler reads classes from the class path (never project
//! sources: a `module-info.java` on the source path hides JDK modules). This
//! module keeps that class path populated by compiling the project once,
//! then recompiling on save.
//!
//! # External compiler
//!
//! Compiling runs the `ravac` executable (the distribution's compiler
//! binary), not javac-core in-process (its module handling panicked on
//! `module-info.java` projects). Lookup order:
//!
//! 1. `RAVAC_BIN` (explicit override; a full path to the compiler);
//! 2. the directory of the running `ravac-lsp` executable (`ravac.exe` on
//!    Windows, `ravac` elsewhere).
//!
//! `PATH` is deliberately not searched: a compiler from another project must
//! not be used by accident. When no binary is found the build fails visibly
//! (`artifactStats.status = "failed"` with an error) and the analysis falls
//! back to the source path (slow but correct).
//!
//! Distribution requirement: ship `ravac` next to `ravac-lsp`, or set
//! `RAVAC_BIN`.
//!
//! Private output: `<root>/target/ravac-classes` for projects with a `target`
//! directory, `<root>/.ravac/classes` otherwise, overridable with
//! `RAVAC_ARTIFACT_DIR`. Maven's `target/classes` is never touched.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Environment override for the private artifact directory.
pub const ARTIFACT_DIR_ENV: &str = "RAVAC_ARTIFACT_DIR";

/// Environment override for the compiler executable.
pub const RAVAC_BIN_ENV: &str = "RAVAC_BIN";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactStatus {
    Building,
    Ready,
    Skipped,
    Failed,
}

impl ArtifactStatus {
    pub fn name(self) -> &'static str {
        match self {
            Self::Building => "building",
            Self::Ready => "ready",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArtifactStats {
    pub status: ArtifactStatus,
    pub dir: PathBuf,
    pub files: usize,
    pub classes: usize,
    pub error: Option<String>,
}

impl ArtifactStats {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "status": self.status.name(),
            "dir": self.dir.to_string_lossy(),
            "files": self.files,
            "classes": self.classes,
            "error": self.error,
        })
    }
}

/// Inputs for artifact maintenance.
#[derive(Clone)]
pub struct ArtifactInput {
    pub root: PathBuf,
    /// All project sources (main + test + module-info), absolute paths.
    pub sources: Vec<PathBuf>,
    /// Dependency jars and the project output (if present).
    pub class_path: Vec<PathBuf>,
    pub source_level: String,
    /// The project already has build output: do not compile.
    pub has_project_output: bool,
}

struct ArtifactState {
    status: ArtifactStatus,
    files: usize,
    classes: usize,
    error: Option<String>,
    /// Set when a full build finished and the analysis should switch to the
    /// artifact class path.
    pending_switch: bool,
}

/// Owns the private artifact directory and its build state.
pub struct ArtifactBuilder {
    dir: PathBuf,
    class_path: Vec<PathBuf>,
    source_level: String,
    state: Arc<Mutex<ArtifactState>>,
}

impl ArtifactBuilder {
    pub fn new(input: ArtifactInput) -> Self {
        let dir = artifact_dir(&input.root);
        let status = if input.has_project_output {
            ArtifactStatus::Skipped
        } else {
            ArtifactStatus::Building
        };
        let state = Arc::new(Mutex::new(ArtifactState {
            status,
            files: input.sources.len(),
            classes: 0,
            error: None,
            pending_switch: false,
        }));
        Self {
            dir,
            class_path: input.class_path,
            source_level: input.source_level,
            state,
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn stats(&self) -> ArtifactStats {
        let state = self.state.lock().unwrap();
        ArtifactStats {
            status: state.status,
            dir: self.dir.clone(),
            files: state.files,
            classes: state.classes,
            error: state.error.clone(),
        }
    }

    /// Start the full build on a worker thread; `initialize` returns meanwhile.
    pub fn start_full_build(&self, sources: Vec<PathBuf>) {
        {
            let state = self.state.lock().unwrap();
            if state.status == ArtifactStatus::Skipped {
                return;
            }
        }
        let dir = self.dir.clone();
        let class_path = self.class_path.clone();
        let source_level = self.source_level.clone();
        let state = Arc::clone(&self.state);
        std::thread::spawn(move || {
            if let Err(error) = prepare_dir(&dir) {
                fail(&state, error);
                return;
            }
            match compile(&dir, &class_path, &source_level, &sources) {
                Ok(classes) => {
                    let mut guard = state.lock().unwrap();
                    guard.status = ArtifactStatus::Ready;
                    guard.classes = classes;
                    guard.error = None;
                    guard.pending_switch = true;
                }
                Err(error) => fail(&state, error),
            }
        });
    }

    /// Full build on the calling thread (before the index compiler starts,
    /// which avoids sharing the platform image with another compiler thread).
    pub fn build_blocking(&self, sources: Vec<PathBuf>) -> bool {
        {
            let state = self.state.lock().unwrap();
            if state.status == ArtifactStatus::Skipped {
                return false;
            }
        }
        if let Err(error) = prepare_dir(&self.dir) {
            fail(&self.state, error);
            return false;
        }
        match compile(&self.dir, &self.class_path, &self.source_level, &sources) {
            Ok(classes) => {
                let mut guard = self.state.lock().unwrap();
                guard.status = ArtifactStatus::Ready;
                guard.classes = classes;
                guard.error = None;
                guard.pending_switch = true;
                true
            }
            Err(error) => {
                fail(&self.state, error);
                false
            }
        }
    }

    /// Compile `files` (the saved file plus its dependent closure) in place.
    pub fn build_files(&self, files: &[PathBuf]) -> bool {
        // The private directory itself must be on the class path: referenced
        // project classes live there and are not part of this closure.
        let class_path = self.analysis_class_path();
        // A `module-info.class` on the class path makes javac treat those
        // classes as a module and hide their packages from an unnamed-module
        // compilation, so hide it for the incremental round.
        let hidden = hide_module_info(&self.dir);
        let result = compile(&self.dir, &class_path, &self.source_level, files);
        restore_module_info(&self.dir, hidden);
        match result {
            Ok(_) => {
                let mut guard = self.state.lock().unwrap();
                if guard.status == ArtifactStatus::Failed {
                    guard.status = ArtifactStatus::Ready;
                    guard.error = None;
                }
                true
            }
            Err(error) => {
                fail(&self.state, error);
                false
            }
        }
    }

    /// The analysis class path once the full build completed (take once).
    pub fn take_completion(&self) -> Option<Vec<PathBuf>> {
        let mut guard = self.state.lock().unwrap();
        if !guard.pending_switch {
            return None;
        }
        guard.pending_switch = false;
        Some(self.analysis_class_path())
    }

    /// Class path used by the analysis: private artifacts first, then the
    /// project output and dependencies.
    pub fn analysis_class_path(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        paths.push(self.dir.clone());
        for entry in &self.class_path {
            if !paths.contains(entry) {
                paths.push(entry.clone());
            }
        }
        paths
    }
}

fn fail(state: &Arc<Mutex<ArtifactState>>, error: String) {
    let mut guard = state.lock().unwrap();
    guard.status = ArtifactStatus::Failed;
    guard.error = Some(error);
    guard.pending_switch = false;
}

/// The private artifact directory for a workspace root.
pub fn artifact_dir(root: &Path) -> PathBuf {
    if let Some(dir) = std::env::var_os(ARTIFACT_DIR_ENV) {
        return PathBuf::from(dir);
    }
    let target = root.join("target");
    if target.is_dir() {
        target.join("ravac-classes")
    } else {
        root.join(".ravac").join("classes")
    }
}

fn prepare_dir(dir: &Path) -> Result<(), String> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)
            .map_err(|error| format!("cannot clear {}: {error}", dir.display()))?;
    }
    std::fs::create_dir_all(dir)
        .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
    Ok(())
}

fn hide_module_info(dir: &Path) -> Option<PathBuf> {
    let source = dir.join("module-info.class");
    if !source.exists() {
        return None;
    }
    let backup = dir.parent()?.join("module-info.class.ravac-hidden");
    let _ = std::fs::remove_file(&backup);
    std::fs::rename(&source, &backup).ok()?;
    Some(backup)
}

fn restore_module_info(dir: &Path, hidden: Option<PathBuf>) {
    if let Some(backup) = hidden {
        let _ = std::fs::rename(&backup, dir.join("module-info.class"));
    }
}

/// Locate the compiler: `RAVAC_BIN` first, then the directory of the
/// running `ravac-lsp` binary. `PATH` is not searched (see module docs).
fn ravac_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(RAVAC_BIN_ENV) {
        return Some(PathBuf::from(path));
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    for name in ["ravac.exe", "ravac"] {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn compile(
    dir: &Path,
    class_path: &[PathBuf],
    source_level: &str,
    files: &[PathBuf],
) -> Result<usize, String> {
    if files.is_empty() {
        return Ok(0);
    }
    let _ = source_level;
    let ravac =
        ravac_binary().ok_or_else(|| "ravac binary not found next to ravac-lsp".to_string())?;
    let separator = if cfg!(windows) { ";" } else { ":" };
    let class_path = class_path
        .iter()
        .map(|entry| entry.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(separator);
    let argfile = dir.join("_ravac-sources.txt");
    let list = files
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&argfile, list)
        .map_err(|error| format!("cannot write {}: {error}", argfile.display()))?;
    let mut command = std::process::Command::new(&ravac);
    command.arg("-d").arg(dir);
    if !class_path.is_empty() {
        command.arg("-cp").arg(class_path);
    }
    command.arg(format!("@{}", argfile.display()));
    let output = command
        .output()
        .map_err(|error| format!("cannot run {}: {error}", ravac.display()))?;
    let _ = std::fs::remove_file(&argfile);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let summary = stderr
            .lines()
            .filter(|line| line.contains("错误") || line.contains("error"))
            .take(3)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(if summary.is_empty() {
            format!("compilation failed: {}", output.status)
        } else {
            summary
        });
    }
    Ok(count_classes(dir))
}

fn count_classes(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut count = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            count += count_classes(&path);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "class")
        {
            count += 1;
        }
    }
    count
}
