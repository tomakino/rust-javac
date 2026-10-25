//! javac 17 oracle 调用。
//!
//! 用途锚点：docs/PLAN.md §7（差分测试体系）。本文件是测试基础设施（适配工具），不是
//! javac 源码的移植文件，因此不写移植来源锚点、不写版权头。
//!
//! 职责：定位本机 javac 17；把源码写入全新的临时目录，用 -d 指定输出目录编译；捕获原始
//! stdout/stderr 字节、退出码和生成的 class 文件清单；记录用于归一化的路径上下文。
//! 默认固定 oracle 环境（可用 options 覆盖）：UTF-8 源码编码、UTF-8 诊断输出、英文诊断
//! （避免本机语言环境导致差分不稳定）。

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::normalize::decode_output;

/// oracle 的 javac 可执行文件。
#[derive(Debug, Clone)]
pub struct Javac {
    executable: PathBuf,
}

impl Javac {
    /// 用显式路径构造（例如 JAVA_HOME/bin/javac）。
    pub fn at(executable: impl Into<PathBuf>) -> Javac {
        Javac {
            executable: executable.into(),
        }
    }

    /// 按 JAVA_HOME -> JAVAC -> PATH -> 本机基线路径的顺序定位 javac。
    pub fn find() -> Option<Javac> {
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            let candidate = Path::new(&home).join("bin").join(javac_executable_name());
            if candidate.is_file() {
                return Some(Javac::at(candidate));
            }
        }
        if let Some(explicit) = std::env::var_os("JAVAC") {
            let candidate = PathBuf::from(explicit);
            if candidate.is_file() {
                return Some(Javac::at(candidate));
            }
        }
        if let Some(candidate) = find_on_path() {
            return Some(Javac::at(candidate));
        }
        let fallback = PathBuf::from(r"C:\Program Files\Eclipse Adoptium\jdk-17.0.19.10-hotspot")
            .join("bin")
            .join(javac_executable_name());
        if fallback.is_file() {
            return Some(Javac::at(fallback));
        }
        None
    }

    /// 定位 javac 并要求版本是 17.x（差分基线 oracle 的版本）。
    ///
    /// 找不到 javac 或版本不是 17 时返回 None（调用方跳过 oracle 测试并说明）。
    pub fn find_jdk17() -> Option<Javac> {
        let javac = Javac::find()?;
        match javac.version() {
            Ok(version) if version.contains("javac 17.") => Some(javac),
            Ok(version) => {
                eprintln!(
                    "javac oracle: {} is not JDK 17 ({}); oracle tests will be skipped",
                    javac.executable.display(),
                    version
                );
                None
            }
            Err(error) => {
                eprintln!(
                    "javac oracle: cannot run {}: {}; oracle tests will be skipped",
                    javac.executable.display(),
                    error
                );
                None
            }
        }
    }

    /// oracle 可执行文件路径。
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    /// javac -version 的输出（合并 stdout/stderr，去掉首尾空白）。
    pub fn version(&self) -> Result<String, Error> {
        let output = Command::new(&self.executable)
            .arg("-version")
            .output()
            .map_err(Error::Io)?;
        let mut text = decode_output(&output.stdout);
        text.push_str(&decode_output(&output.stderr));
        Ok(text.trim().to_string())
    }

    /// 在全新的临时目录中编译给定源文件。
    ///
    /// options 原样透传给 javac（-classpath、--release、-XDshould-stop.at 等）；-d 默认由
    /// harness 指定为临时输出目录，若 options 里已有 -d 则透传用户的 -d（此时 class 清单从
    /// harness 目录收集，可能为空：调用方自行负责）。
    pub fn compile(&self, sources: &[SourceFile], options: &[&str]) -> Result<RawOutput, Error> {
        let temp = TempDir::new("rust-javac-oracle")?;
        let output_dir = temp.path().join("out");
        fs::create_dir_all(&output_dir).map_err(Error::Io)?;

        let mut source_paths = Vec::with_capacity(sources.len());
        for source in sources {
            let path = temp.path().join(&source.name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(Error::Io)?;
            }
            fs::write(&path, source.contents.as_bytes()).map_err(Error::Io)?;
            source_paths.push(path);
        }

        let effective_options = effective_options(options);
        let has_user_output_dir = options.contains(&"-d");
        let mut command = Command::new(&self.executable);
        if !has_user_output_dir {
            command.arg("-d").arg(&output_dir);
        }
        command.args(&effective_options);
        for path in &source_paths {
            command.arg(path);
        }

        let output = command.output().map_err(Error::Io)?;
        let exit_code = output.status.code().unwrap_or(-1);
        let class_files = collect_class_files(&output_dir)?;

        let mut raw = RawOutput::new(exit_code, output.stdout, output.stderr, class_files);
        raw.output_dir = Some(output_dir.clone());
        raw.source_paths = source_paths.clone();
        raw.executable = Some(self.executable.clone());
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            raw.push_path(PathBuf::from(home), "<jdk>");
        }
        for path in &source_paths {
            let relative = path.strip_prefix(temp.path()).unwrap_or(path);
            let relative_text = relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            raw.push_path(path.clone(), format!("<src>/{}", relative_text));
        }
        raw.push_path(output_dir.clone(), "<out>");
        raw.push_path(temp.path().to_path_buf(), "<tmp>");
        raw.push_path(self.executable.clone(), "<javac>");
        raw.sort_paths();
        raw.temp = Some(temp);
        Ok(raw)
    }
}

/// 一个待编译的源文件。name 是临时目录内的相对路径（如 "Hello.java" 或 "pkg/Hello.java"）。
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub name: String,
    pub contents: String,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, contents: impl Into<String>) -> SourceFile {
        SourceFile {
            name: name.into(),
            contents: contents.into(),
        }
    }
}

/// 一次运行（oracle 或未来 rust-javac）的原始输出。
#[derive(Debug)]
pub struct RawOutput {
    /// 进程退出码；进程被信号终止时按 -1 记录。
    pub exit_code: i32,
    /// 原始 stdout 字节。
    pub stdout: Vec<u8>,
    /// 原始 stderr 字节。
    pub stderr: Vec<u8>,
    /// 生成的 class 文件清单（相对输出目录、'/' 分隔；normalize/compare 会排序后比较）。
    pub class_files: Vec<String>,
    /// 归一化上下文：(实际路径, 占位符)，由 sort_paths 保证按实际路径长度降序。
    pub paths: Vec<(PathBuf, String)>,
    /// 编译输出目录（-d）。
    pub output_dir: Option<PathBuf>,
    /// 源文件绝对路径。
    pub source_paths: Vec<PathBuf>,
    /// oracle 可执行文件（归一化为 <javac>）。
    pub executable: Option<PathBuf>,
    /// 临时工作目录；Drop 时递归删除。未来 rust-javac 侧不需要，可为 None。
    pub temp: Option<TempDir>,
}

impl RawOutput {
    /// 构造原始输出（不携带路径上下文与临时目录；未来 rust-javac 侧可直接使用）。
    pub fn new(
        exit_code: i32,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        class_files: Vec<String>,
    ) -> RawOutput {
        RawOutput {
            exit_code,
            stdout,
            stderr,
            class_files,
            paths: Vec::new(),
            output_dir: None,
            source_paths: Vec::new(),
            executable: None,
            temp: None,
        }
    }

    /// 登记一个"实际路径 -> 占位符"的替换规则。
    pub fn push_path(&mut self, actual: impl Into<PathBuf>, placeholder: impl Into<String>) {
        self.paths.push((actual.into(), placeholder.into()));
    }

    /// 链式版本：登记一个替换规则。
    pub fn with_path(
        mut self,
        actual: impl Into<PathBuf>,
        placeholder: impl Into<String>,
    ) -> RawOutput {
        self.push_path(actual, placeholder);
        self
    }

    /// 按实际路径长度降序排序并去重，保证长路径先被替换（例如 <out> 先于 <tmp>）。
    pub fn sort_paths(&mut self) {
        self.paths.sort_by(|left, right| {
            right
                .0
                .to_string_lossy()
                .len()
                .cmp(&left.0.to_string_lossy().len())
        });
        let mut seen = std::collections::HashSet::new();
        self.paths.retain(|(path, _)| seen.insert(path.clone()));
    }

    /// 临时目录路径（没有临时目录时返回 None）。
    pub fn temp_path(&self) -> Option<&Path> {
        self.temp.as_ref().map(TempDir::path)
    }
}

/// 自动清理的临时目录（纯 std 实现）。
#[derive(Debug)]
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// 在系统临时目录下创建唯一目录；Drop 时递归删除。
    pub fn new(prefix: &str) -> Result<TempDir, Error> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        for _ in 0..1000 {
            let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
            let mut path = std::env::temp_dir();
            path.push(format!("{}-{}-{}", prefix, std::process::id(), sequence));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(TempDir { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(Error::Io(error)),
            }
        }
        Err(Error::Io(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not create a unique temporary directory",
        )))
    }

    /// 目录路径。
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// harness 错误。
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    /// 找不到 javac oracle。
    JavacNotFound,
    /// 找到的 javac 不是 17.x。
    UnsupportedVersion {
        executable: PathBuf,
        version: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(error) => write!(f, "I/O error: {}", error),
            Error::JavacNotFound => write!(f, "no javac oracle found (set JAVA_HOME or JAVAC)"),
            Error::UnsupportedVersion {
                executable,
                version,
            } => write!(
                f,
                "javac oracle at {} is not JDK 17 ({})",
                executable.display(),
                version
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Error {
        Error::Io(error)
    }
}

fn javac_executable_name() -> &'static str {
    if cfg!(windows) { "javac.exe" } else { "javac" }
}

fn find_on_path() -> Option<PathBuf> {
    let path_value = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path_value) {
        let candidate = directory.join(javac_executable_name());
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// 默认固定 oracle 环境 + 用户参数透传。
fn effective_options(options: &[&str]) -> Vec<String> {
    let mut result: Vec<String> = options.iter().map(|option| (*option).to_string()).collect();
    if !options.contains(&"-encoding") {
        result.push("-encoding".to_string());
        result.push("UTF-8".to_string());
    }
    if !options
        .iter()
        .any(|option| option.starts_with("-J-Dfile.encoding"))
    {
        result.push("-J-Dfile.encoding=UTF-8".to_string());
    }
    if !options
        .iter()
        .any(|option| option.starts_with("-J-Duser.language"))
    {
        result.push("-J-Duser.language=en".to_string());
        result.push("-J-Duser.country=US".to_string());
    }
    result
}

/// 递归收集输出目录下的 class 文件（相对路径、'/' 分隔、排序）。
///
/// 差分运行器（crate::diff）复用同一收集规则，保证 class 清单可比。
pub(crate) fn collect_class_files(root: &Path) -> Result<Vec<String>, Error> {
    fn walk(root: &Path, directory: &Path, files: &mut Vec<String>) -> Result<(), Error> {
        for entry in fs::read_dir(directory).map_err(Error::Io)? {
            let entry = entry.map_err(Error::Io)?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(Error::Io)?;
            if file_type.is_dir() {
                walk(root, &path, files)?;
            } else if path
                .extension()
                .is_some_and(|extension| extension == "class")
            {
                let relative = path.strip_prefix(root).unwrap_or(&path);
                let name = relative
                    .components()
                    .map(|component| component.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                files.push(name);
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    if root.is_dir() {
        walk(root, root, &mut files)?;
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_dir_is_removed_on_drop() {
        let path = {
            let temp = TempDir::new("rust-javac-tests-temp").expect("create temp dir");
            let path = temp.path().to_path_buf();
            assert!(path.is_dir());
            path
        };
        assert!(
            !path.exists(),
            "temp dir should be removed on drop: {}",
            path.display()
        );
    }

    #[test]
    fn source_file_keeps_name_and_contents() {
        let source = SourceFile::new("pkg/Hello.java", "class Hello {}");
        assert_eq!(source.name, "pkg/Hello.java");
        assert_eq!(source.contents, "class Hello {}");
    }

    #[test]
    fn path_context_is_sorted_longest_first() {
        let mut output = RawOutput::new(0, Vec::new(), Vec::new(), Vec::new());
        output.push_path(PathBuf::from("C:/a"), "<a>");
        output.push_path(PathBuf::from("C:/a/b/c"), "<c>");
        output.sort_paths();
        assert_eq!(output.paths[0].0, PathBuf::from("C:/a/b/c"));
        assert_eq!(output.paths[1].0, PathBuf::from("C:/a"));
    }

    #[test]
    fn effective_options_add_fixed_oracle_environment() {
        let options = effective_options(&["--release", "17"]);
        assert!(options.contains(&"-encoding".to_string()));
        assert!(options.contains(&"UTF-8".to_string()));
        assert!(options.contains(&"-J-Dfile.encoding=UTF-8".to_string()));
        assert!(options.contains(&"-J-Duser.language=en".to_string()));
        assert!(options.contains(&"-J-Duser.country=US".to_string()));
        assert!(options.contains(&"--release".to_string()));
    }

    #[test]
    fn effective_options_respect_explicit_encoding_and_language() {
        let options = effective_options(&["-encoding", "GBK", "-J-Duser.language=zh"]);
        assert!(!options.contains(&"UTF-8".to_string()));
        assert!(!options.contains(&"-J-Duser.country=US".to_string()));
        assert!(options.contains(&"GBK".to_string()));
    }
}
