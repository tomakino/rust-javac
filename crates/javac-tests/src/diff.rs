//! 差分运行器核心:目标编译器 vs 参考 oracle。
//!
//! 用途锚点:docs/PLAN.md §7(差分测试体系)与 P9 全量差分准备。本文件是测试基础设施
//! (适配工具),不是 javac 源码的移植文件,因此不写移植来源锚点、不写版权头。
//!
//! 组成:
//! - [Case]:一个用例(源文件 + javac 兼容参数);
//! - [Target]:目标编译器命令(可执行路径 + 固定参数前缀,例如未来的 rust-javac);
//! - [Oracle]:参考 oracle 来源:
//!   * [Oracle::Diagnostics]:入库的 data/diagnostics-oracle.txt(记录数据,运行期零 JVM);
//!   * [Oracle::Baselines]:target/javac-corpus/baselines 下由参考 javac 生成的基线(不入库);
//!   * [Oracle::Javac]:实时运行参考 javac 17(开发期与自测;需要 JDK 17)。
//! - [run]:逐用例运行目标(临时目录),捕获退出码/规范化 stdout 与 stderr/class 清单,
//!   复用 [crate::normalize::compare] 比较;
//! - [RunReport]:通过/失败/跳过统计与限量差异详情;[RunReport::exit_code] 映射 0/1/2。
//!
//! 固定 oracle 环境:目标与实时 javac oracle 都通过 JAVA_TOOL_OPTIONS 注入
//! -Dfile.encoding=UTF-8 -Duser.language=en -Duser.country=US,并追加 -encoding UTF-8;
//! JVM 目标打印的 "Picked up JAVA_TOOL_OPTIONS:" 噪声行会被归一化剥离,因此记录数据
//! (由 javac-tests harness 生成)与本次运行可比;非 JVM 目标会忽略 JAVA_TOOL_OPTIONS。

use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::corpus::{CompileInputStatus, Corpus, Error as CorpusError, load_compile_input};
use crate::diagnostics_data::{BuiltinCases, DiagnosticsData, Mode};
use crate::javac::{Javac, RawOutput, SourceFile, TempDir, collect_class_files};
use crate::normalize::{DiffReport, TextDiff, compare};

/// 退出码:全部用例通过。
pub const EXIT_PASS: i32 = 0;
/// 退出码:存在差分失败的用例。
pub const EXIT_FAIL: i32 = 1;
/// 退出码:环境缺失(目标不可运行/oracle 数据缺失/需要 JDK 17 但找不到/配置错误)。
pub const EXIT_ENVIRONMENT: i32 = 2;

/// 差分运行器错误(均映射为退出码 2)。
#[derive(Debug)]
pub enum DiffError {
    Io(io::Error),
    Corpus(CorpusError),
    /// 目标编译器无法启动(例如可执行文件不存在)。
    TargetUnavailable {
        program: PathBuf,
        error: io::Error,
    },
    /// 需要实时 javac oracle 或 JDK 17 目标环境,但本机没有 JDK 17。
    JdkUnavailable,
    /// oracle 数据/基线目录不存在。
    OracleMissing(PathBuf),
    /// 参数组合或数据格式错误。
    InvalidConfiguration(String),
}

impl fmt::Display for DiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffError::Io(error) => write!(f, "I/O error: {error}"),
            DiffError::Corpus(error) => write!(f, "corpus error: {error}"),
            DiffError::TargetUnavailable { program, error } => write!(
                f,
                "target compiler is not runnable ({}): {error}",
                program.display()
            ),
            DiffError::JdkUnavailable => {
                write!(f, "no JDK 17 javac found (set JAVA_HOME or JAVAC)")
            }
            DiffError::OracleMissing(path) => {
                write!(f, "oracle data not found: {}", path.display())
            }
            DiffError::InvalidConfiguration(message) => {
                write!(f, "invalid configuration: {message}")
            }
        }
    }
}

impl std::error::Error for DiffError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DiffError::Io(error) => Some(error),
            DiffError::Corpus(error) => Some(error),
            DiffError::TargetUnavailable { error, .. } => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for DiffError {
    fn from(error: io::Error) -> DiffError {
        DiffError::Io(error)
    }
}

impl From<CorpusError> for DiffError {
    fn from(error: CorpusError) -> DiffError {
        DiffError::Corpus(error)
    }
}

impl From<crate::javac::Error> for DiffError {
    fn from(error: crate::javac::Error) -> DiffError {
        match error {
            crate::javac::Error::Io(error) => DiffError::Io(error),
            other => DiffError::InvalidConfiguration(format!("javac harness error: {other}")),
        }
    }
}

impl DiffError {
    /// 环境缺失类错误统一映射为退出码 2(CLI 打印说明并跳过)。
    pub fn exit_code(&self) -> i32 {
        EXIT_ENVIRONMENT
    }
}

/// 目标编译器命令:可执行路径 + 固定参数前缀。
///
/// 执行时参数顺序为:固定参数、用例参数、-encoding UTF-8(若用例未给出)、
/// -d <输出目录>(若用例未给出)、源文件绝对路径。
#[derive(Debug, Clone)]
pub struct Target {
    pub program: PathBuf,
    pub args: Vec<String>,
}

impl Target {
    /// 只有可执行文件,无固定参数。
    pub fn new(program: impl Into<PathBuf>) -> Target {
        Target {
            program: program.into(),
            args: Vec::new(),
        }
    }

    /// 可执行文件 + 固定参数前缀(例如解释器 + 脚本路径)。
    pub fn with_args(program: impl Into<PathBuf>, args: Vec<String>) -> Target {
        Target {
            program: program.into(),
            args,
        }
    }
}

/// 一个差分用例。
#[derive(Debug, Clone)]
pub struct Case {
    /// 报告与 oracle 查找使用的用例名(语料相对路径或诊断数据用例名)。
    pub name: String,
    /// 源文件(临时目录内相对路径 + 内容)。
    pub sources: Vec<SourceFile>,
    /// 透传给编译器的参数。
    pub args: Vec<String>,
    /// 语料输入指纹;baselines oracle 用于检测过期基线,其它 oracle 为 None。
    pub source_fnv: Option<u64>,
}

impl Case {
    /// 内联构造用例(oracle 为实时 javac 或诊断数据时使用)。
    pub fn new(name: impl Into<String>, sources: Vec<SourceFile>, args: Vec<String>) -> Case {
        Case {
            name: name.into(),
            sources,
            args,
            source_fnv: None,
        }
    }
}

/// 用例集合:可运行的用例 + 因数据限制被跳过的条目。
#[derive(Debug, Default)]
pub struct CaseSet {
    pub cases: Vec<Case>,
    pub skipped: Vec<(String, String)>,
}

impl CaseSet {
    fn skip(&mut self, name: &str, reason: impl Into<String>) {
        self.skipped.push((name.to_string(), reason.into()));
    }
}

/// 从语料加载 compile-only 用例(供 oracle=baselines / javac 使用)。
///
/// 与基线生成共用 [load_compile_input] 的文件解析与选项过滤规则;不支持加载的用例
/// (源文件不是 UTF-8/找不到 @compile 文件等)记录到 [CaseSet::skipped]。
pub fn load_corpus_cases(
    corpus: &Corpus,
    filter: Option<&str>,
    limit: Option<usize>,
) -> Result<CaseSet, DiffError> {
    let max = limit.unwrap_or(usize::MAX);
    let mut set = CaseSet::default();
    for case in corpus.compile_only_cases() {
        if set.cases.len() >= max {
            break;
        }
        if let Some(filter) = filter
            && !case.relative_path.contains(filter)
        {
            continue;
        }
        match load_compile_input(corpus, case)? {
            CompileInputStatus::Loaded(input) => set.cases.push(Case {
                name: case.relative_path.clone(),
                sources: input.sources,
                args: input.options,
                source_fnv: Some(input.source_fnv),
            }),
            CompileInputStatus::Unsupported(reason) => set.skip(&case.relative_path, reason),
        }
    }
    Ok(set)
}

/// 从诊断 oracle 数据加载用例(供 oracle=diagnostics 使用)。
///
/// source 为 corpus: 的记录从语料根重建源文件;source 为 builtin: 的记录从入库的
/// data/diagnostics-builtin-cases.txt([BuiltinCases])重建源文件(含多文件用例)。
/// builtin 用例文件缺失时,这些记录降级为跳过。-XDrawDiagnostics 模式在用例参数末尾
/// 追加该选项。
pub fn load_diagnostics_cases(
    data: &DiagnosticsData,
    builtin_cases: Option<&BuiltinCases>,
    corpus_root: &Path,
    mode: Mode,
    filter: Option<&str>,
    limit: Option<usize>,
) -> CaseSet {
    let max = limit.unwrap_or(usize::MAX);
    let mut set = CaseSet::default();
    for record in &data.records {
        if set.cases.len() >= max {
            break;
        }
        if let Some(filter) = filter
            && !record.name.contains(filter)
        {
            continue;
        }
        if let Some(name) = record.source.strip_prefix("builtin:") {
            let Some(case) = builtin_cases.and_then(|cases| cases.find(name)) else {
                set.skip(&record.name, "builtin case source is not recorded");
                continue;
            };
            let mut args = case.args.clone();
            if mode == Mode::Draw {
                args.push("-XDrawDiagnostics".to_string());
            }
            set.cases.push(Case {
                name: record.name.clone(),
                sources: case.sources.clone(),
                args,
                source_fnv: None,
            });
            continue;
        }
        let Some(relative) = record.source.strip_prefix("corpus:") else {
            set.skip(
                &record.name,
                format!("unknown case source: {}", record.source),
            );
            continue;
        };
        let path = corpus_root.join(relative);
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(_) => {
                set.skip(
                    &record.name,
                    format!("source file missing: {}", path.display()),
                );
                continue;
            }
        };
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| record.name.clone());
        let mut args = record.args.clone();
        if mode == Mode::Draw {
            args.push("-XDrawDiagnostics".to_string());
        }
        set.cases.push(Case {
            name: record.name.clone(),
            sources: vec![SourceFile::new(file_name, contents)],
            args,
            source_fnv: None,
        });
    }
    set
}

/// 已记录基线目录(target/javac-corpus/baselines)。
#[derive(Debug, Clone)]
pub struct BaselineStore {
    pub root: PathBuf,
}

/// 一条基线记录:期望输出 + 源输入指纹。
#[derive(Debug)]
pub struct BaselineRecord {
    pub source_fnv: Option<u64>,
    pub expected: RawOutput,
}

impl BaselineStore {
    /// 打开基线目录。
    pub fn open(root: &Path) -> Result<BaselineStore, DiffError> {
        if !root.is_dir() {
            return Err(DiffError::OracleMissing(root.to_path_buf()));
        }
        Ok(BaselineStore {
            root: root.to_path_buf(),
        })
    }

    /// 读取一个用例的基线;没有记录时返回 None。
    pub fn read(&self, case_name: &str) -> Result<Option<BaselineRecord>, DiffError> {
        let path = self
            .root
            .join(case_name.trim_end_matches(".java"))
            .join("baseline.txt");
        if !path.is_file() {
            return Ok(None);
        }
        let text = fs::read_to_string(&path)?;
        parse_baseline(&text).map(Some).map_err(|message| {
            DiffError::InvalidConfiguration(format!(
                "malformed baseline {}: {message}",
                path.display()
            ))
        })
    }
}

/// 解析 baseline.txt:头部 source_fnv + normalize() 输出的四段内容。
fn parse_baseline(text: &str) -> Result<BaselineRecord, String> {
    let mut source_fnv = None;
    let mut normalized: Option<Vec<&str>> = None;
    let mut lines = text.lines();
    for line in lines.by_ref() {
        if let Some(value) = line.strip_prefix("source_fnv ") {
            source_fnv = value.trim().parse::<u64>().ok();
        }
        if line == "--- normalized ---" {
            normalized = Some(lines.by_ref().collect());
            break;
        }
    }
    let normalized = normalized.ok_or_else(|| "missing --- normalized --- section".to_string())?;
    let mut exit_code = None;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut classes = Vec::new();
    #[derive(Clone, Copy, PartialEq)]
    enum Section {
        None,
        Stdout,
        Stderr,
        Classes,
    }
    let mut section = Section::None;
    for line in normalized {
        if let Some(value) = line.strip_prefix("exit_code: ") {
            exit_code = value.trim().parse::<i32>().ok();
            continue;
        }
        if line == "--- stdout ---" {
            section = Section::Stdout;
            continue;
        }
        if line == "--- stderr ---" {
            section = Section::Stderr;
            continue;
        }
        if line == "--- classes ---" {
            section = Section::Classes;
            continue;
        }
        match section {
            Section::Stdout => stdout.push(line),
            Section::Stderr => stderr.push(line),
            Section::Classes => classes.push(line),
            Section::None => {
                return Err(format!("line before exit_code section: {line}"));
            }
        }
    }
    let exit_code = exit_code.ok_or_else(|| "missing exit_code line".to_string())?;
    Ok(BaselineRecord {
        source_fnv,
        expected: RawOutput::new(
            exit_code,
            join_lines(&stdout).into_bytes(),
            join_lines(&stderr).into_bytes(),
            classes.iter().map(|line| (*line).to_string()).collect(),
        ),
    })
}

fn join_lines(lines: &[&str]) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        let mut text = lines.join("\n");
        text.push('\n');
        text
    }
}

/// 参考 oracle 来源。
#[derive(Debug)]
pub enum Oracle {
    /// 实时运行参考 javac 17。
    Javac(Javac),
    /// 入库的诊断 oracle 数据(按用例名查找记录)。
    Diagnostics { data: DiagnosticsData, mode: Mode },
    /// 已记录的 corpus 基线(按语料相对路径查找 baseline.txt)。
    Baselines(BaselineStore),
}

struct Expected {
    raw: RawOutput,
    source_fnv: Option<u64>,
}

impl Oracle {
    /// 构造实时 javac oracle;缺 JDK 17 时返回 [DiffError::JdkUnavailable](CLI 退出码 2)。
    pub fn from_javac(find: Option<Javac>) -> Result<Oracle, DiffError> {
        find.map(Oracle::Javac).ok_or(DiffError::JdkUnavailable)
    }

    /// 加载入库的诊断 oracle 数据。
    pub fn from_diagnostics(path: &Path, mode: Mode) -> Result<Oracle, DiffError> {
        if !path.is_file() {
            return Err(DiffError::OracleMissing(path.to_path_buf()));
        }
        let data = DiagnosticsData::load(path).map_err(DiffError::Io)?;
        Ok(Oracle::Diagnostics { data, mode })
    }

    /// 打开已记录的 corpus 基线目录。
    pub fn from_baselines(root: &Path) -> Result<Oracle, DiffError> {
        Ok(Oracle::Baselines(BaselineStore::open(root)?))
    }

    fn expected(&self, case: &Case) -> Result<Option<Expected>, DiffError> {
        match self {
            Oracle::Javac(javac) => {
                let raw = run_compiler(javac.executable(), &[], case)?;
                Ok(Some(Expected {
                    raw,
                    source_fnv: None,
                }))
            }
            Oracle::Diagnostics { data, mode } => {
                Ok(data.find(&case.name).map(|record| Expected {
                    raw: record.expected_raw(*mode),
                    source_fnv: None,
                }))
            }
            Oracle::Baselines(store) => Ok(store.read(&case.name)?.map(|record| Expected {
                raw: record.expected,
                source_fnv: record.source_fnv,
            })),
        }
    }
}

/// 差异类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffCategory {
    ExitCode,
    Stdout,
    Stderr,
    Classes,
}

impl DiffCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            DiffCategory::ExitCode => "exit-code",
            DiffCategory::Stdout => "stdout",
            DiffCategory::Stderr => "stderr",
            DiffCategory::Classes => "classes",
        }
    }
}

/// 一个失败用例的差异详情。
#[derive(Debug)]
pub struct FailureDetail {
    pub categories: Vec<DiffCategory>,
    pub diff: DiffReport,
}

/// 单个用例的结果。
#[derive(Debug)]
pub enum Outcome {
    Passed,
    Failed(FailureDetail),
    Skipped(String),
}

/// 单个用例的结果记录。
#[derive(Debug)]
pub struct CaseResult {
    pub name: String,
    pub outcome: Outcome,
}

/// 一次差分运行的汇总。
#[derive(Debug, Default)]
pub struct RunReport {
    pub results: Vec<CaseResult>,
}

impl RunReport {
    pub fn push_passed(&mut self, name: &str) {
        self.results.push(CaseResult {
            name: name.to_string(),
            outcome: Outcome::Passed,
        });
    }

    pub fn push_failed(&mut self, name: &str, detail: FailureDetail) {
        self.results.push(CaseResult {
            name: name.to_string(),
            outcome: Outcome::Failed(detail),
        });
    }

    pub fn push_skipped(&mut self, name: &str, reason: impl Into<String>) {
        self.results.push(CaseResult {
            name: name.to_string(),
            outcome: Outcome::Skipped(reason.into()),
        });
    }

    /// 合并加载阶段因数据限制跳过的用例。
    pub fn add_skips(&mut self, skips: impl IntoIterator<Item = (String, String)>) {
        for (name, reason) in skips {
            self.push_skipped(&name, reason);
        }
    }

    pub fn passed(&self) -> usize {
        self.results
            .iter()
            .filter(|result| matches!(result.outcome, Outcome::Passed))
            .count()
    }

    pub fn failed(&self) -> usize {
        self.results
            .iter()
            .filter(|result| matches!(result.outcome, Outcome::Failed(_)))
            .count()
    }

    pub fn skipped(&self) -> usize {
        self.results
            .iter()
            .filter(|result| matches!(result.outcome, Outcome::Skipped(_)))
            .count()
    }

    /// 实际运行(通过 + 失败)的用例数。
    pub fn run(&self) -> usize {
        self.passed() + self.failed()
    }

    /// 退出码:0 全部通过;1 有失败;环境缺失由 [DiffError::exit_code] 给出为 2。
    pub fn exit_code(&self) -> i32 {
        if self.failed() > 0 {
            EXIT_FAIL
        } else {
            EXIT_PASS
        }
    }

    /// 一行统计。
    pub fn format_summary(&self) -> String {
        format!(
            "cases: {} (run {}, passed {}, failed {}, skipped {})",
            self.results.len(),
            self.run(),
            self.passed(),
            self.failed(),
            self.skipped()
        )
    }

    /// 限量的失败/跳过详情(含用例名、差异类别、期望/实际行)。
    pub fn format_details(&self, max_details: usize) -> String {
        let mut text = String::new();
        let mut shown = 0usize;
        for result in &self.results {
            if shown >= max_details {
                break;
            }
            match &result.outcome {
                Outcome::Passed => continue,
                Outcome::Skipped(reason) => {
                    shown += 1;
                    let _ = writeln!(text, "SKIP {}: {}", result.name, reason);
                }
                Outcome::Failed(detail) => {
                    shown += 1;
                    let categories: Vec<&str> = detail
                        .categories
                        .iter()
                        .map(|category| category.as_str())
                        .collect();
                    let _ = writeln!(text, "FAIL {} [{}]", result.name, categories.join(", "));
                    if !detail.diff.exit_code_matches {
                        let _ = writeln!(
                            text,
                            "  exit code: expected {}, actual {}",
                            detail.diff.expected_exit_code, detail.diff.actual_exit_code
                        );
                    }
                    write_text_diff(&mut text, "stdout", &detail.diff.stdout);
                    write_text_diff(&mut text, "stderr", &detail.diff.stderr);
                    if !detail.diff.class_files_matches {
                        let _ = writeln!(
                            text,
                            "  class files: expected {:?}, actual {:?}",
                            detail.diff.expected_class_files, detail.diff.actual_class_files
                        );
                    }
                }
            }
        }
        if shown == 0 {
            text.push_str("no failures or skips\n");
        }
        text
    }
}

fn write_text_diff(text: &mut String, name: &str, diff: &TextDiff) {
    if diff.equal {
        return;
    }
    let _ = writeln!(
        text,
        "  {}: differs (expected {} lines, actual {} lines)",
        name, diff.expected_line_count, diff.actual_line_count
    );
    for difference in diff.differences.iter().take(3) {
        match (&difference.expected, &difference.actual) {
            (Some(expected), Some(actual)) => {
                let _ = writeln!(text, "    line {}:", difference.line);
                let _ = writeln!(text, "      expected: {expected}");
                let _ = writeln!(text, "      actual:   {actual}");
            }
            (Some(expected), None) => {
                let _ = writeln!(
                    text,
                    "    line {}: missing in actual: {expected}",
                    difference.line
                );
            }
            (None, Some(actual)) => {
                let _ = writeln!(
                    text,
                    "    line {}: unexpected in actual: {actual}",
                    difference.line
                );
            }
            (None, None) => {}
        }
    }
}

fn diff_categories(report: &DiffReport) -> Vec<DiffCategory> {
    let mut categories = Vec::new();
    if !report.exit_code_matches {
        categories.push(DiffCategory::ExitCode);
    }
    if !report.stdout.equal {
        categories.push(DiffCategory::Stdout);
    }
    if !report.stderr.equal {
        categories.push(DiffCategory::Stderr);
    }
    if !report.class_files_matches {
        categories.push(DiffCategory::Classes);
    }
    categories
}

/// 运行一次差分:逐用例取 oracle 期望、运行目标、比较并汇总。
///
/// 找不到 oracle 记录的用例记为跳过(不影响退出码);基线指纹与语料输入不一致时记为
/// 跳过(stale baseline),不会误报为差分失败。
pub fn run(target: &Target, cases: &[Case], oracle: &Oracle) -> Result<RunReport, DiffError> {
    let mut report = RunReport::default();
    for case in cases {
        let Some(expected) = oracle.expected(case)? else {
            report.push_skipped(&case.name, "no recorded oracle output for this case");
            continue;
        };
        if let (Some(case_fnv), Some(oracle_fnv)) = (case.source_fnv, expected.source_fnv)
            && case_fnv != oracle_fnv
        {
            report.push_skipped(
                &case.name,
                format!("stale oracle baseline (source fingerprint {case_fnv} != {oracle_fnv})"),
            );
            continue;
        }
        let actual = run_compiler(&target.program, &target.args, case)?;
        let diff = compare(&expected.raw, &actual);
        if diff.equal {
            report.push_passed(&case.name);
        } else {
            report.push_failed(
                &case.name,
                FailureDetail {
                    categories: diff_categories(&diff),
                    diff,
                },
            );
        }
    }
    Ok(report)
}

/// 在全新临时目录中运行一次编译器命令并捕获输出。
///
/// 参数顺序:固定参数、用例参数、-encoding UTF-8(若未给出)、-d 输出目录(若未给出)、
/// 源文件绝对路径。目标与实时 javac oracle 都走这里,自测时同一命令两次运行的规范化
/// 结果一致(路径占位由 RawOutput::paths 统一替换)。
pub fn run_compiler(
    program: &Path,
    fixed_args: &[String],
    case: &Case,
) -> Result<RawOutput, DiffError> {
    let temp = TempDir::new("rust-javac-diff")?;
    let output_dir = temp.path().join("out");
    fs::create_dir_all(&output_dir)?;

    let mut source_paths = Vec::with_capacity(case.sources.len());
    for source in &case.sources {
        let path = temp.path().join(&source.name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, source.contents.as_bytes())?;
        source_paths.push(path);
    }

    let mut args: Vec<String> = Vec::new();
    args.extend(fixed_args.iter().cloned());
    args.extend(case.args.iter().cloned());
    if !args.iter().any(|arg| arg == "-encoding") {
        args.push("-encoding".to_string());
        args.push("UTF-8".to_string());
    }
    if !args.iter().any(|arg| arg == "-d") {
        args.push("-d".to_string());
        args.push(output_dir.display().to_string());
    }
    for path in &source_paths {
        args.push(path.display().to_string());
    }

    let mut command = Command::new(program);
    command.args(&args);
    // 固定 oracle 环境:JVM 目标的编码与语言设置;非 JVM 目标忽略该变量。
    command.env(
        "JAVA_TOOL_OPTIONS",
        "-Dfile.encoding=UTF-8 -Duser.language=en -Duser.country=US",
    );
    let output = command.output().map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            DiffError::TargetUnavailable {
                program: program.to_path_buf(),
                error,
            }
        } else {
            DiffError::Io(error)
        }
    })?;
    let exit_code = output.status.code().unwrap_or(-1);
    let class_files = collect_class_files(&output_dir)?;

    let mut raw = RawOutput::new(exit_code, output.stdout, output.stderr, class_files);
    raw.push_path(output_dir, "<out>");
    raw.push_path(temp.path(), "<tmp>");
    for path in &source_paths {
        let relative = path.strip_prefix(temp.path()).unwrap_or(path);
        let relative_text = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        raw.push_path(path.clone(), format!("<src>/{relative_text}"));
    }
    raw.push_path(program, "<target>");
    raw.sort_paths();
    raw.temp = Some(temp);
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::corpus::{BaselineOptions, Corpus, generate_baselines};
    use crate::javac::Javac;

    const HELLO: &str = "public class Hello {\n    int field = 1;\n}\n";
    const SYNTAX_ERROR: &str = "class Broken {\n    void m() {\n        int x = ;\n    }\n}\n";
    const TYPE_ERROR: &str = "class TypeErrorCase {\n    int x = missingValue;\n}\n";

    fn sample_cases() -> Vec<Case> {
        vec![
            Case::new(
                "sample/Hello.java",
                vec![SourceFile::new("Hello.java", HELLO)],
                Vec::new(),
            ),
            Case::new(
                "sample/Broken.java",
                vec![SourceFile::new("Broken.java", SYNTAX_ERROR)],
                Vec::new(),
            ),
            Case::new(
                "sample/TypeErrorCase.java",
                vec![SourceFile::new("TypeErrorCase.java", TYPE_ERROR)],
                Vec::new(),
            ),
        ]
    }

    fn project_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates directory")
            .parent()
            .expect("project root")
            .to_path_buf()
    }

    fn corpus_root() -> Option<PathBuf> {
        let root = project_root().join("internal/reference/jdk17u/test/langtools/tools/javac");
        if root.is_dir() { Some(root) } else { None }
    }

    fn builtin_cases() -> BuiltinCases {
        BuiltinCases::parse(include_str!("../data/diagnostics-builtin-cases.txt"))
    }

    fn diagnostics_data() -> DiagnosticsData {
        DiagnosticsData::parse(include_str!("../data/diagnostics-oracle.txt"))
    }

    #[test]
    fn reference_javac_agrees_with_itself() {
        let Some(javac) = Javac::find_jdk17() else {
            eprintln!("skipping reference self-diff: no JDK 17 javac found");
            return;
        };
        let target = Target::new(javac.executable());
        let oracle = Oracle::Javac(javac);
        let report = run(&target, &sample_cases(), &oracle).expect("run differential");
        assert_eq!(report.passed(), 3, "{}", report.format_details(5));
        assert_eq!(report.failed(), 0, "{}", report.format_details(5));
        assert_eq!(report.skipped(), 0);
        assert_eq!(report.exit_code(), EXIT_PASS);
    }

    fn broken_target() -> (TempDir, Target) {
        let temp = TempDir::new("rust-javac-diff-broken").expect("temp dir");
        if cfg!(windows) {
            let script = temp.path().join("broken.bat");
            fs::write(
                &script,
                "@echo off\r\necho broken target output 1>&2\r\nexit /b 1\r\n",
            )
            .expect("write batch script");
            (
                temp,
                Target::with_args(
                    "cmd.exe",
                    vec!["/c".to_string(), script.display().to_string()],
                ),
            )
        } else {
            let script = temp.path().join("broken.sh");
            fs::write(
                &script,
                "#!/bin/sh\necho \"broken target output\" >&2\nexit 1\n",
            )
            .expect("write shell script");
            (
                temp,
                Target::with_args("/bin/sh", vec![script.display().to_string()]),
            )
        }
    }

    #[test]
    fn detects_a_broken_target() {
        let Some(javac) = Javac::find_jdk17() else {
            eprintln!("skipping broken-target diff: no JDK 17 javac found");
            return;
        };
        let (_temp, target) = broken_target();
        let oracle = Oracle::Javac(javac);
        let report = run(&target, &sample_cases(), &oracle).expect("run differential");
        assert!(
            report.failed() >= 1,
            "a broken target must be detected: {}",
            report.format_summary()
        );
        assert_eq!(report.exit_code(), EXIT_FAIL);
        let detail = report
            .results
            .iter()
            .find_map(|result| match &result.outcome {
                Outcome::Failed(detail) => Some(detail),
                _ => None,
            })
            .expect("failure detail");
        assert!(
            detail.categories.contains(&DiffCategory::Stderr)
                || detail.categories.contains(&DiffCategory::ExitCode),
            "unexpected categories: {:?}",
            detail.categories
        );
        let details = report.format_details(5);
        assert!(details.contains("FAIL "), "{details}");
        assert!(details.contains("expected"), "{details}");
    }

    #[test]
    fn missing_jdk_is_an_environment_error() {
        assert_eq!(DiffError::JdkUnavailable.exit_code(), EXIT_ENVIRONMENT);
        assert!(DiffError::JdkUnavailable.to_string().contains("JDK 17"));
        let resolved = Oracle::from_javac(None);
        assert!(matches!(resolved, Err(DiffError::JdkUnavailable)));
        // 本机有 JDK 17 时 from_javac(Javac::find_jdk17()) 应成功;没有时跳过该半部断言。
        if let Some(javac) = Javac::find_jdk17() {
            assert!(Oracle::from_javac(Some(javac)).is_ok());
        } else {
            eprintln!("skipping live oracle construction: no JDK 17 javac found");
        }
    }

    #[test]
    fn filters_and_limits_diagnostics_cases() {
        let Some(root) = corpus_root() else {
            eprintln!("skipping diagnostics case loading: reference corpus not found");
            return;
        };
        let data = diagnostics_data();
        let builtin = builtin_cases();

        // corpus: 来源的记录能从 reference 语料重建源文件。
        let set = load_diagnostics_cases(
            &data,
            Some(&builtin),
            &root,
            Mode::Default,
            Some("corpus/IllegalStartOfExpr"),
            None,
        );
        assert_eq!(set.cases.len(), 1);
        assert_eq!(set.cases[0].name, "corpus/IllegalStartOfExpr");
        assert_eq!(set.cases[0].sources.len(), 1);
        assert_eq!(set.cases[0].sources[0].name, "IllegalStartOfExpr.java");
        let record = data.find("corpus/IllegalStartOfExpr").expect("record");
        assert_eq!(set.cases[0].args, record.args);

        // -XDrawDiagnostics 模式在参数末尾追加该选项。
        let draw = load_diagnostics_cases(
            &data,
            Some(&builtin),
            &root,
            Mode::Draw,
            Some("corpus/IllegalStartOfExpr"),
            None,
        );
        let mut expected_args = record.args.clone();
        expected_args.push("-XDrawDiagnostics".to_string());
        assert_eq!(draw.cases[0].args, expected_args);

        // builtin: 来源的记录从入库用例文件重建源文件。
        let builtin_set = load_diagnostics_cases(
            &data,
            Some(&builtin),
            &root,
            Mode::Default,
            Some("syntax/IllegalStartOfExpression"),
            None,
        );
        assert_eq!(builtin_set.cases.len(), 1);
        let builtin_case = &builtin_set.cases[0];
        assert_eq!(builtin_case.name, "syntax/IllegalStartOfExpression");
        assert_eq!(builtin_case.sources.len(), 1);
        assert_eq!(
            builtin_case.sources[0].name,
            "IllegalStartOfExpression.java"
        );
        let builtin_record = data
            .find("syntax/IllegalStartOfExpression")
            .expect("builtin record");
        assert_eq!(builtin_case.args, builtin_record.args);

        // 多文件 builtin 用例的两个源文件都被重建。
        let multi = load_diagnostics_cases(
            &data,
            Some(&builtin),
            &root,
            Mode::Default,
            Some("multifile/TwoTypeErrors"),
            None,
        );
        assert_eq!(multi.cases.len(), 1);
        assert_eq!(multi.cases[0].sources.len(), 2);
        assert_eq!(multi.cases[0].sources[0].name, "TypeErrorA.java");
        assert_eq!(multi.cases[0].sources[1].name, "TypeErrorB.java");

        // 缺少 builtin 用例文件时,builtin 记录降级为跳过。
        let no_builtin = load_diagnostics_cases(&data, None, &root, Mode::Default, None, None);
        assert_eq!(no_builtin.cases.len(), 45);
        assert_eq!(no_builtin.skipped.len(), 35);

        let limited = load_diagnostics_cases(
            &data,
            Some(&builtin),
            &root,
            Mode::Default,
            Some("corpus/"),
            Some(3),
        );
        assert_eq!(limited.cases.len(), 3);
        let all = load_diagnostics_cases(&data, Some(&builtin), &root, Mode::Default, None, None);
        assert_eq!(all.cases.len(), 80);
        assert!(all.skipped.is_empty());
    }

    #[test]
    fn reference_javac_matches_diagnostics_data() {
        let Some(javac) = Javac::find_jdk17() else {
            eprintln!("skipping diagnostics-oracle self-check: no JDK 17 javac found");
            return;
        };
        let Some(root) = corpus_root() else {
            eprintln!("skipping diagnostics-oracle self-check: reference corpus not found");
            return;
        };
        let data = diagnostics_data();
        let builtin = builtin_cases();
        let set = load_diagnostics_cases(&data, Some(&builtin), &root, Mode::Default, None, None);
        assert_eq!(
            set.cases.len(),
            80,
            "all 80 diagnostics cases must be loadable"
        );
        assert!(set.skipped.is_empty());
        let target = Target::new(javac.executable());
        let oracle = Oracle::Diagnostics {
            data,
            mode: Mode::Default,
        };
        let report = run(&target, &set.cases, &oracle).expect("run differential");
        assert_eq!(report.failed(), 0, "{}", report.format_details(10));
        assert_eq!(report.skipped(), 0);
        assert_eq!(report.passed(), set.cases.len());
    }

    #[test]
    fn corpus_baselines_round_trip_against_reference_javac() {
        let Some(javac) = Javac::find_jdk17() else {
            eprintln!("skipping corpus-baseline round-trip: no JDK 17 javac found");
            return;
        };
        let Some(root) = corpus_root() else {
            eprintln!("skipping corpus-baseline round-trip: reference corpus not found");
            return;
        };
        let corpus = Corpus::scan(&root).expect("scan corpus");
        let temp = TempDir::new("rust-javac-diff-corpus-baselines").expect("temp dir");
        let mut options = BaselineOptions::new(temp.path());
        options.limit = Some(3);
        let generated = generate_baselines(&javac, &corpus, &options).expect("generate baselines");
        if generated.generated + generated.up_to_date == 0 {
            eprintln!("skipping corpus-baseline round-trip: no baseline generated");
            return;
        }
        let set = load_corpus_cases(&corpus, None, Some(3)).expect("load corpus cases");
        if set.cases.is_empty() {
            eprintln!("skipping corpus-baseline round-trip: no loadable case");
            return;
        }
        let oracle = Oracle::from_baselines(&temp.path().join("baselines")).expect("baselines");
        let target = Target::new(javac.executable());
        let report = run(&target, &set.cases, &oracle).expect("run differential");
        assert_eq!(report.failed(), 0, "{}", report.format_details(10));
        assert!(report.passed() >= 1, "{}", report.format_summary());
    }

    #[test]
    fn baseline_store_reads_recorded_output() {
        let temp = TempDir::new("rust-javac-diff-baseline").expect("temp dir");
        let root = temp.path().join("baselines");
        let directory = root.join("compiler/Hello");
        fs::create_dir_all(&directory).expect("baseline directory");
        fs::write(
            directory.join("baseline.txt"),
            "# rust-javac javac oracle baseline\ncase compiler/Hello.java\nsource_fnv 42\noptions \n--- normalized ---\nexit_code: 0\n--- stdout ---\n--- stderr ---\n--- classes ---\nHello.class\n",
        )
        .expect("write baseline");

        let store = BaselineStore::open(&root).expect("open baseline store");
        let record = store
            .read("compiler/Hello.java")
            .expect("read baseline")
            .expect("baseline record");
        assert_eq!(record.source_fnv, Some(42));
        assert_eq!(record.expected.exit_code, 0);
        assert_eq!(record.expected.class_files, vec!["Hello.class".to_string()]);
        let normalized = crate::normalize::normalize(&record.expected);
        assert!(normalized.contains("exit_code: 0"), "{normalized}");
        assert!(
            store
                .read("compiler/Missing.java")
                .expect("missing baseline")
                .is_none()
        );

        // 指纹不匹配(stale baseline)时跳过,不运行目标(因此目标不存在也不会报错)。
        let oracle = Oracle::Baselines(BaselineStore::open(&root).expect("reopen store"));
        let case = Case {
            name: "compiler/Hello.java".to_string(),
            sources: vec![SourceFile::new("Hello.java", "class Hello {}\n")],
            args: Vec::new(),
            source_fnv: Some(7),
        };
        let report = run(
            &Target::new("rust-javac-definitely-not-a-command"),
            &[case],
            &oracle,
        )
        .expect("stale baseline must not run the target");
        assert_eq!(report.skipped(), 1);
        assert_eq!(report.failed(), 0);
        assert_eq!(report.exit_code(), EXIT_PASS);
    }

    #[test]
    fn missing_records_are_skipped_without_running_the_target() {
        let data = DiagnosticsData::parse("");
        let oracle = Oracle::Diagnostics {
            data,
            mode: Mode::Default,
        };
        let case = Case::new(
            "missing/case",
            vec![SourceFile::new("Missing.java", "class Missing {}\n")],
            Vec::new(),
        );
        let report = run(
            &Target::new("rust-javac-definitely-not-a-command"),
            &[case],
            &oracle,
        )
        .expect("missing record must not run the target");
        assert_eq!(report.skipped(), 1);
        assert_eq!(report.failed(), 0);
        assert_eq!(report.exit_code(), EXIT_PASS);
    }

    #[test]
    fn empty_report_passes_and_errors_map_to_environment_exit_code() {
        let report = RunReport::default();
        assert_eq!(report.exit_code(), EXIT_PASS);
        assert_eq!(
            report.format_summary(),
            "cases: 0 (run 0, passed 0, failed 0, skipped 0)"
        );
        let io_error = DiffError::Io(io::Error::other("boom"));
        assert_eq!(io_error.exit_code(), EXIT_ENVIRONMENT);
        assert_eq!(
            DiffError::OracleMissing(PathBuf::from("nope")).exit_code(),
            EXIT_ENVIRONMENT
        );
    }
}
