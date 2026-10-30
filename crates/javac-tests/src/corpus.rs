//! langtools jtreg 语料抽取与 javac oracle 基线。
//!
//! 用途锚点：docs/PLAN.md §7（差分测试体系/P9 全量差分准备）。本文件是测试基础设施
//! （适配工具），不是 javac 源码的移植文件，因此不写移植来源锚点、不写版权头。
//!
//! 语料来源：reference/jdk17u 的 test/langtools/tools/javac（通过 git sparse-checkout
//! 扩展检出的只读参考目录，不进入项目仓库）。抽取器解析 jtreg 头标签，把每个 .java 文件
//! 分类为 compile-only / compile+run / 需要 @library 或 APT / 需要 @modules / 不可用，
//! 并记录排除原因；随后用 javac oracle 为 compile-only 用例记录基线到 target/ 下
//! （不入库），支持刷新与增量。

use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::javac::{Javac, RawOutput, SourceFile};
use crate::normalize::normalize;

/// 只读取语料文件的头部这么多字节用于 jtreg 标签解析。
const MAX_HEADER_BYTES: u64 = 16 * 1024;

/// FNV-1a 64 的初值与乘数（用于源文件内容指纹，纯 std 实现）。
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// jtreg 头里的一个标签，例如 `@test`、`@compile/fail/ref=Foo.out`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// 去掉前导 '@' 的标签名，保留变体后缀（如 "compile/fail/ref=Foo.out"）。
    pub name: String,
    /// 标签同一行后面的参数（原始文本，未再拆分）。
    pub value: String,
}

/// 从一个 Java 源文件解析出的 jtreg 标签集合。
#[derive(Debug, Clone, Default)]
pub struct TestHeader {
    pub tags: Vec<Tag>,
}

impl TestHeader {
    /// 是否存在指定名字的标签（精确匹配）。
    pub fn has(&self, name: &str) -> bool {
        self.tags.iter().any(|tag| tag.name == name)
    }

    /// 第一个精确匹配的标签。
    pub fn get(&self, name: &str) -> Option<&Tag> {
        self.tags.iter().find(|tag| tag.name == name)
    }

    /// 所有精确匹配标签的参数。
    pub fn values(&self, name: &str) -> Vec<&str> {
        self.tags
            .iter()
            .filter(|tag| tag.name == name)
            .map(|tag| tag.value.as_str())
            .collect()
    }

    /// 第一个名字以指定前缀开头的标签（用于 @compile 的多种变体）。
    pub fn first_with_prefix(&self, prefix: &str) -> Option<&Tag> {
        self.tags.iter().find(|tag| tag.name.starts_with(prefix))
    }
}

/// 解析一个 Java 源文件开头第一个含 `@test` 标签的注释块。
pub fn parse_test_header(source: &str) -> TestHeader {
    let mut tags: Vec<Tag> = Vec::new();
    let mut in_block = false;
    let mut block_lines: Vec<String> = Vec::new();

    for line in source.lines().take(200) {
        let trimmed = line.trim_start();
        if !in_block {
            if let Some(rest) = trimmed.strip_prefix("/*") {
                // 单行注释块：/* @test ... */
                if let Some(end) = rest.find("*/") {
                    let content = rest[..end].trim();
                    if is_test_tag_line(content)
                        && let Some(tag) = parse_tag_line(content)
                    {
                        tags.push(tag);
                        return TestHeader { tags };
                    }
                    continue;
                }
                in_block = true;
                block_lines.clear();
                let first = rest.trim();
                if !first.is_empty() {
                    block_lines.push(first.to_string());
                }
            }
            continue;
        }

        // 块内：先找结束标记 "*/"，再去掉行首的 '*' 与空白。
        if let Some(position) = trimmed.find("*/") {
            let head = trimmed[..position].trim_end();
            let head = head.strip_prefix('*').unwrap_or(head).trim();
            if !head.is_empty() {
                block_lines.push(head.to_string());
            }
            in_block = false;
            if block_lines.iter().any(|line| is_test_tag_line(line)) {
                for line in &block_lines {
                    if let Some(tag) = parse_tag_line(line) {
                        tags.push(tag);
                    }
                }
                return TestHeader { tags };
            }
            continue;
        }
        let body = trimmed.strip_prefix('*').unwrap_or(trimmed).trim();
        if !body.is_empty() {
            block_lines.push(body.to_string());
        }
    }

    TestHeader { tags }
}

fn is_test_tag_line(line: &str) -> bool {
    line.trim_start().starts_with("@test")
}

fn parse_tag_line(line: &str) -> Option<Tag> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix('@')?;
    let mut parts = rest.splitn(2, char::is_whitespace);
    let name = parts.next()?.to_string();
    if name.is_empty() {
        return None;
    }
    let value = parts.next().unwrap_or("").trim().to_string();
    Some(Tag { name, value })
}

/// jtreg 用例分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TestKind {
    /// 只有编译动作（@compile 或 @compile/fail，或 @run compile），无其它工具需求。
    CompileOnly,
    /// 需要运行（@run main/othervm/driver 等）。
    CompileAndRun,
    /// 需要 jtreg @library 或注解处理器等外部工装，单次 javac 调用无法复现。
    NeedsLibraryOrApt,
    /// 需要模块系统（@modules）。
    Modules,
    /// 被忽略或其它原因不可用。
    Unavailable,
    /// 不是测试（辅助 .java，无 @test）。
    NotATest,
}

impl TestKind {
    /// 稳定的简短名字（用于索引文件）。
    pub fn as_str(self) -> &'static str {
        match self {
            TestKind::CompileOnly => "compile_only",
            TestKind::CompileAndRun => "compile_and_run",
            TestKind::NeedsLibraryOrApt => "needs_library_or_apt",
            TestKind::Modules => "modules",
            TestKind::Unavailable => "unavailable",
            TestKind::NotATest => "not_a_test",
        }
    }
}

/// 一个被测 .java 文件的索引项。
#[derive(Debug, Clone)]
pub struct TestCase {
    /// 相对语料根的路径，''/'-分隔'。
    pub relative_path: String,
    pub kind: TestKind,
    /// 排除原因（非 CompileOnly 时填写）。
    pub exclude_reason: Option<String>,
    pub summary: Option<String>,
    pub bug_ids: Vec<String>,
    /// @compile 行的原始参数文本。
    pub compile_line: Option<String>,
    /// @compile 行里以 .java 结尾的文件名（不含选项）。
    pub compile_files: Vec<String>,
    /// @compile 行里以 '-' 开头的选项。
    pub compile_options: Vec<String>,
    /// @run 行的原始参数文本。
    pub run_line: Option<String>,
    pub libraries: Vec<String>,
    pub modules: Vec<String>,
    /// 全部标签（供后续扩展）。
    pub tags: Vec<Tag>,
}

impl TestCase {
    /// 该用例是否存在注解处理需求。
    pub fn needs_annotation_processing(&self) -> bool {
        self.compile_options
            .iter()
            .any(|option| option.starts_with("-processor") || option.starts_with("-proc"))
            || self
                .run_line
                .as_deref()
                .is_some_and(|line| line.contains("-processor"))
    }
}

/// 分类一个 jtreg 头。
pub fn classify(header: &TestHeader) -> (TestKind, Option<String>) {
    let Some(test_tag) = header.get("test") else {
        return (TestKind::NotATest, Some("no @test tag".to_string()));
    };
    let _ = test_tag;

    if let Some(ignore) = header.get("ignore") {
        return (
            TestKind::Unavailable,
            Some(format!("ignored: {}", ignore.value)),
        );
    }

    let modules = header.values("modules");
    if !modules.is_empty() {
        return (
            TestKind::Modules,
            Some(format!("requires modules: {}", modules.join(" "))),
        );
    }

    let libraries = header.values("library");
    if !libraries.is_empty() {
        return (
            TestKind::NeedsLibraryOrApt,
            Some(format!("requires jtreg library: {}", libraries.join(" "))),
        );
    }

    let compile_tag = header.first_with_prefix("compile");
    let run_tag = header.first_with_prefix("run");

    if let Some(compile) = compile_tag {
        // @compile/module=<module>/... 把测试编译进 JDK 模块，需要模块系统。
        if let Some(module) = compile.name.strip_prefix("compile/module=") {
            let module = module.split('/').next().unwrap_or(module);
            return (
                TestKind::Modules,
                Some(format!("requires module compilation: {}", module)),
            );
        }
        let (options, _files) = split_compile_arguments(compile);
        if options
            .iter()
            .any(|option| option.starts_with("-processor") || option.starts_with("-processorpath"))
        {
            return (
                TestKind::NeedsLibraryOrApt,
                Some("requires annotation processing".to_string()),
            );
        }
    }

    if let Some(run) = run_tag {
        let action = run.value.split_whitespace().next().unwrap_or("");
        if run.value.contains("-processor") {
            return (
                TestKind::NeedsLibraryOrApt,
                Some("requires annotation processing".to_string()),
            );
        }
        if action == "compile" {
            return (TestKind::CompileOnly, None);
        }
        return (
            TestKind::CompileAndRun,
            Some(format!("requires execution: @run {}", action)),
        );
    }

    if compile_tag.is_some() {
        return (TestKind::CompileOnly, None);
    }

    (
        TestKind::Unavailable,
        Some("no @compile or @run action".to_string()),
    )
}

/// 把 @compile 标签参数拆成 (选项, 文件名)。
pub fn split_compile_arguments(tag: &Tag) -> (Vec<String>, Vec<String>) {
    let mut options = Vec::new();
    let mut files = Vec::new();
    for token in tag.value.split_whitespace() {
        if token.starts_with('-') {
            options.push(token.to_string());
        } else if token.ends_with(".java") {
            files.push(token.to_string());
        }
    }
    (options, files)
}

/// 一个语料的分类统计。
#[derive(Debug, Clone, Default)]
pub struct CorpusStats {
    pub total_files: usize,
    pub by_kind: BTreeMap<TestKind, usize>,
}

impl CorpusStats {
    pub fn count(&self, kind: TestKind) -> usize {
        self.by_kind.get(&kind).copied().unwrap_or(0)
    }
}

/// 抽取出的语料索引。
#[derive(Debug, Clone)]
pub struct Corpus {
    pub root: PathBuf,
    pub cases: Vec<TestCase>,
}

impl Corpus {
    /// 递归扫描语料根目录下的全部 .java 文件并分类。
    pub fn scan(root: &Path) -> Result<Corpus, Error> {
        if !root.is_dir() {
            return Err(Error::MissingCorpus(root.to_path_buf()));
        }
        let mut paths = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(directory) = stack.pop() {
            for entry in fs::read_dir(&directory).map_err(Error::Io)? {
                let entry = entry.map_err(Error::Io)?;
                let path = entry.path();
                let file_type = entry.file_type().map_err(Error::Io)?;
                if file_type.is_dir() {
                    stack.push(path);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension == "java")
                {
                    paths.push(path);
                }
            }
        }
        paths.sort();

        let mut cases = Vec::with_capacity(paths.len());
        for path in paths {
            let relative_path = relative_name(root, &path)?;
            let source = read_header_text(&path)?;
            let header = parse_test_header(&source);
            let (kind, exclude_reason) = classify(&header);
            cases.push(build_case(relative_path, header, kind, exclude_reason));
        }

        Ok(Corpus {
            root: root.to_path_buf(),
            cases,
        })
    }

    /// 分类统计。
    pub fn stats(&self) -> CorpusStats {
        let mut stats = CorpusStats {
            total_files: self.cases.len(),
            by_kind: BTreeMap::new(),
        };
        for case in &self.cases {
            *stats.by_kind.entry(case.kind).or_insert(0) += 1;
        }
        stats
    }

    /// 所有 compile-only 用例（基线生成的目标集合）。
    pub fn compile_only_cases(&self) -> Vec<&TestCase> {
        self.cases
            .iter()
            .filter(|case| case.kind == TestKind::CompileOnly)
            .collect()
    }

    /// 生成稳定的文本索引（供人工查看与外部工具）。
    pub fn index_text(&self) -> String {
        let stats = self.stats();
        let mut text = String::new();
        text.push_str("# rust-javac jtreg corpus index v1\n");
        let _ = writeln!(text, "root {}", self.root.display());
        let _ = writeln!(text, "total_files {}", stats.total_files);
        for (kind, count) in &stats.by_kind {
            let _ = writeln!(text, "count {} {}", kind.as_str(), count);
        }
        for case in &self.cases {
            let _ = writeln!(text, "case {} {}", case.kind.as_str(), case.relative_path);
            if let Some(reason) = &case.exclude_reason {
                let _ = writeln!(text, "  reason {}", reason);
            }
            if let Some(summary) = &case.summary {
                let _ = writeln!(text, "  summary {}", summary);
            }
            if !case.bug_ids.is_empty() {
                let _ = writeln!(text, "  bug {}", case.bug_ids.join(" "));
            }
            if let Some(compile) = &case.compile_line {
                let _ = writeln!(text, "  compile {}", compile);
            }
            if let Some(run) = &case.run_line {
                let _ = writeln!(text, "  run {}", run);
            }
            if !case.libraries.is_empty() {
                let _ = writeln!(text, "  library {}", case.libraries.join(" "));
            }
            if !case.modules.is_empty() {
                let _ = writeln!(text, "  modules {}", case.modules.join(" "));
            }
        }
        text
    }

    /// 把文本索引写到文件（通常位于 target/ 下，不入库）。
    pub fn write_index(&self, path: &Path) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(Error::Io)?;
        }
        fs::write(path, self.index_text()).map_err(Error::Io)
    }
}

fn build_case(
    relative_path: String,
    header: TestHeader,
    kind: TestKind,
    exclude_reason: Option<String>,
) -> TestCase {
    let summary = header.get("summary").map(|tag| tag.value.clone());
    let bug_ids: Vec<String> = header
        .values("bug")
        .iter()
        .flat_map(|value| value.split_whitespace().map(str::to_string))
        .collect();
    let compile_tag = header.first_with_prefix("compile");
    let run_tag = header.first_with_prefix("run");
    let (compile_options, compile_files) = match compile_tag {
        Some(tag) => split_compile_arguments(tag),
        None => (Vec::new(), Vec::new()),
    };
    let libraries: Vec<String> = header
        .values("library")
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    let modules: Vec<String> = header
        .values("modules")
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    TestCase {
        relative_path,
        kind,
        exclude_reason,
        summary,
        bug_ids,
        compile_line: compile_tag.map(|tag| tag.value.clone()),
        compile_files,
        compile_options,
        run_line: run_tag.map(|tag| tag.value.clone()),
        libraries,
        modules,
        tags: header.tags,
    }
}

fn relative_name(root: &Path, path: &Path) -> Result<String, Error> {
    let relative = path.strip_prefix(root).map_err(|_| {
        Error::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not below {}", path.display(), root.display()),
        ))
    })?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn read_header_text(path: &Path) -> Result<String, Error> {
    let file = fs::File::open(path).map_err(Error::Io)?;
    let mut buffer = Vec::new();
    file.take(MAX_HEADER_BYTES)
        .read_to_end(&mut buffer)
        .map_err(Error::Io)?;
    Ok(String::from_utf8_lossy(&buffer).into_owned())
}

/// 基线生成选项。
#[derive(Debug, Clone)]
pub struct BaselineOptions {
    /// 输出根目录（通常 target/javac-corpus，不入库）。
    pub output_root: PathBuf,
    /// true 时无视已有基线全部重生成；false 时只生成缺失或源文件变化的基线。
    pub refresh: bool,
    /// 只处理前 N 个 compile-only 用例（抽样/调试）。
    pub limit: Option<usize>,
}

impl BaselineOptions {
    pub fn new(output_root: impl Into<PathBuf>) -> BaselineOptions {
        BaselineOptions {
            output_root: output_root.into(),
            refresh: false,
            limit: None,
        }
    }
}

/// 单个用例的基线生成结果。
#[derive(Debug, Clone)]
pub enum BaselineStatus {
    Generated(PathBuf),
    UpToDate,
    Unsupported(String),
    Failed(String),
}

/// 一个用例的基线记录。
#[derive(Debug, Clone)]
pub struct BaselineCase {
    pub relative_path: String,
    pub status: BaselineStatus,
}

/// 基线生成汇总。
#[derive(Debug, Clone, Default)]
pub struct BaselineReport {
    pub generated: usize,
    pub up_to_date: usize,
    pub unsupported: usize,
    pub failed: usize,
    pub cases: Vec<BaselineCase>,
}

/// 为全部 compile-only 用例生成 javac oracle 基线。
pub fn generate_baselines(
    javac: &Javac,
    corpus: &Corpus,
    options: &BaselineOptions,
) -> Result<BaselineReport, Error> {
    let mut report = BaselineReport::default();
    let mut processed = 0usize;
    for case in &corpus.cases {
        if case.kind != TestKind::CompileOnly {
            continue;
        }
        if options.limit.is_some_and(|limit| processed >= limit) {
            break;
        }
        processed += 1;
        let status = generate_one(javac, corpus, case, options)?;
        match &status {
            BaselineStatus::Generated(_) => report.generated += 1,
            BaselineStatus::UpToDate => report.up_to_date += 1,
            BaselineStatus::Unsupported(_) => report.unsupported += 1,
            BaselineStatus::Failed(_) => report.failed += 1,
        }
        report.cases.push(BaselineCase {
            relative_path: case.relative_path.clone(),
            status,
        });
    }
    Ok(report)
}

/// 一个语料用例的编译输入（基线生成与差分运行器共用同一套文件解析与选项过滤规则）。
#[derive(Debug, Clone)]
pub struct CompileInput {
    /// 参与编译的源文件（名字为测试目录内的相对路径）。
    pub sources: Vec<SourceFile>,
    /// 透传给 javac 的选项（已过滤掉会与输出目录冲突的 -d）。
    pub options: Vec<String>,
    /// 源文件与选项的 FNV-1a 64 指纹（与基线文件里的 source_fnv 对应）。
    pub source_fnv: u64,
}

/// 编译输入的加载结果。
#[derive(Debug, Clone)]
pub enum CompileInputStatus {
    Loaded(CompileInput),
    Unsupported(String),
}

/// 加载 compile-only 用例的编译输入。
///
/// 与基线生成（[generate_baselines]）使用同一套解析规则：@compile 行引用的源文件、
/// 以测试文件自身补齐、排除 -d 选项；非 UTF-8 源码与找不到的文件记录为 Unsupported。
pub fn load_compile_input(corpus: &Corpus, case: &TestCase) -> Result<CompileInputStatus, Error> {
    let test_path = corpus.root.join(&case.relative_path);
    let test_dir = test_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| corpus.root.clone());

    // @compile 行给出的源文件；没有给出时编译测试文件自身。
    let mut file_names: Vec<String> = case.compile_files.clone();
    let own_name = test_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !file_names.iter().any(|name| name == &own_name) {
        file_names.push(own_name);
    }
    let mut seen = std::collections::HashSet::new();
    file_names.retain(|name| seen.insert(name.clone()));

    let mut sources = Vec::with_capacity(file_names.len());
    let mut source_fnv = FNV_OFFSET;
    for name in &file_names {
        let Some(path) = resolve_compile_file(&test_dir, &corpus.root, name) else {
            return Ok(CompileInputStatus::Unsupported(format!(
                "compile file not found: {}",
                name
            )));
        };
        if name.contains("..") {
            return Ok(CompileInputStatus::Unsupported(format!(
                "compile file outside the test directory: {}",
                name
            )));
        }
        let bytes = fs::read(&path).map_err(Error::Io)?;
        // 语料里少数用例的源文件不是合法 UTF-8（例如故意测试平台默认编码）。
        // harness 的 SourceFile 以字符串传递源码，因此这些用例无法用固定 UTF-8 环境复现，
        // 记录为 Unsupported 而不是让整批基线生成失败。
        let Some(contents) = decode_source_bytes(&bytes) else {
            return Ok(CompileInputStatus::Unsupported(format!(
                "source file is not valid UTF-8: {}",
                name
            )));
        };
        source_fnv = fnv_update(source_fnv, name.as_bytes());
        source_fnv = fnv_update(source_fnv, contents.as_bytes());
        sources.push(SourceFile::new(name.clone(), contents));
    }

    // 过滤掉会与 harness 输出目录冲突的 -d 选项；其余选项原样透传。
    let mut effective_options = Vec::new();
    let mut skip_next = false;
    for option in &case.compile_options {
        if skip_next {
            skip_next = false;
            continue;
        }
        if option == "-d" {
            skip_next = true;
            continue;
        }
        effective_options.push(option.clone());
    }
    let options_text = effective_options.join(" ");
    source_fnv = fnv_update(source_fnv, options_text.as_bytes());
    Ok(CompileInputStatus::Loaded(CompileInput {
        sources,
        options: effective_options,
        source_fnv,
    }))
}

fn generate_one(
    javac: &Javac,
    corpus: &Corpus,
    case: &TestCase,
    options: &BaselineOptions,
) -> Result<BaselineStatus, Error> {
    let input = match load_compile_input(corpus, case)? {
        CompileInputStatus::Loaded(input) => input,
        CompileInputStatus::Unsupported(reason) => {
            return Ok(BaselineStatus::Unsupported(reason));
        }
    };

    let baseline_path = options
        .output_root
        .join("baselines")
        .join(case.relative_path.trim_end_matches(".java"))
        .join("baseline.txt");

    if !options.refresh
        && let Some(existing_fnv) = read_baseline_source_fnv(&baseline_path)?
        && existing_fnv == input.source_fnv
    {
        return Ok(BaselineStatus::UpToDate);
    }

    let option_refs: Vec<&str> = input.options.iter().map(String::as_str).collect();
    let output = javac
        .compile(&input.sources, &option_refs)
        .map_err(Error::Javac)?;

    if let Some(parent) = baseline_path.parent() {
        fs::create_dir_all(parent).map_err(Error::Io)?;
    }
    let options_text = input.options.join(" ");
    let baseline_text = format_baseline(
        &case.relative_path,
        &options_text,
        &output,
        input.source_fnv,
    );
    fs::write(&baseline_path, baseline_text).map_err(Error::Io)?;
    Ok(BaselineStatus::Generated(baseline_path))
}

/// 在测试目录（找不到时再查语料根）解析 @compile 引用的文件。
fn resolve_compile_file(test_dir: &Path, corpus_root: &Path, name: &str) -> Option<PathBuf> {
    let direct = test_dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    let from_root = corpus_root.join(name);
    if from_root.is_file() {
        return Some(from_root);
    }
    None
}

/// 源文件字节按严格 UTF-8 解码；失败返回 None（该用例记为 Unsupported）。
fn decode_source_bytes(bytes: &[u8]) -> Option<String> {
    String::from_utf8(bytes.to_vec()).ok()
}

fn fnv_update(hash: u64, bytes: &[u8]) -> u64 {
    let mut hash = hash;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn format_baseline(
    relative_path: &str,
    options_text: &str,
    output: &RawOutput,
    source_fnv: u64,
) -> String {
    let mut text = String::new();
    text.push_str("# rust-javac javac oracle baseline\n");
    let _ = writeln!(text, "case {}", relative_path);
    let _ = writeln!(text, "source_fnv {}", source_fnv);
    let _ = writeln!(text, "options {}", options_text);
    text.push_str("--- normalized ---\n");
    text.push_str(&normalize(output));
    text
}

/// 读取已有基线里的 source_fnv（第一行匹配）。
fn read_baseline_source_fnv(path: &Path) -> Result<Option<u64>, Error> {
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(path).map_err(Error::Io)?;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("source_fnv ") {
            return Ok(value.trim().parse::<u64>().ok());
        }
    }
    Ok(None)
}

/// 语料/基线工具的错误。
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Javac(crate::javac::Error),
    MissingCorpus(PathBuf),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(error) => write!(f, "I/O error: {}", error),
            Error::Javac(error) => write!(f, "javac oracle error: {}", error),
            Error::MissingCorpus(path) => {
                write!(f, "corpus directory not found: {}", path.display())
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(error) => Some(error),
            Error::Javac(error) => Some(error),
            Error::MissingCorpus(_) => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Error {
        Error::Io(error)
    }
}

impl From<crate::javac::Error> for Error {
    fn from(error: crate::javac::Error) -> Error {
        Error::Javac(error)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::javac::{Javac, TempDir};

    // 记录自 reference/jdk17u（JDK 17.0.19）test/langtools/tools/javac 的语料快照。
    const LANGTOOLS_TOTAL_FILES: usize = 5173;
    const LANGTOOLS_COMPILE_ONLY: usize = 1955;
    const LANGTOOLS_COMPILE_AND_RUN: usize = 394;
    const LANGTOOLS_NEEDS_LIBRARY_OR_APT: usize = 12;
    const LANGTOOLS_MODULES: usize = 1053;
    const LANGTOOLS_UNAVAILABLE: usize = 175;
    const LANGTOOLS_NOT_A_TEST: usize = 1584;

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
        root.is_dir().then_some(root)
    }

    fn javac17() -> Option<Javac> {
        match Javac::find_jdk17() {
            Some(javac) => Some(javac),
            None => {
                eprintln!("skipping: no JDK 17 javac found (set JAVA_HOME or JAVAC)");
                None
            }
        }
    }

    /// 构造标准 jtreg 头（license 块 + 标签块）。
    fn make_header(tags: &str) -> String {
        let mut text =
            String::from("/*\n * Copyright (c) 2024, Oracle and/or its affiliates.\n */\n\n/*\n");
        for line in tags.lines() {
            text.push_str(" * ");
            text.push_str(line);
            text.push('\n');
        }
        text.push_str(" */\n");
        text
    }

    fn write_case(root: &Path, name: &str, tags: &str) {
        fs::write(root.join(name), make_header(tags)).expect("write synthetic case");
    }

    #[test]
    fn parses_jtreg_header_tags() {
        let source = make_header(
            "@test\n@bug 1234 5678\n@summary Some summary\n@library /tools/lib\n@modules java.base/jdk.internal.misc\n@compile/fail/ref=Foo.out -XDrawDiagnostics Foo.java\n@run main Foo",
        );
        let header = parse_test_header(&source);
        assert!(header.has("test"));
        assert_eq!(header.values("bug"), vec!["1234 5678"]);
        assert_eq!(
            header.get("summary").map(|tag| tag.value.as_str()),
            Some("Some summary")
        );
        assert_eq!(header.values("library"), vec!["/tools/lib"]);
        assert_eq!(
            header.values("modules"),
            vec!["java.base/jdk.internal.misc"]
        );
        let compile = header.first_with_prefix("compile").expect("compile tag");
        assert_eq!(compile.name, "compile/fail/ref=Foo.out");
        let (options, files) = split_compile_arguments(compile);
        assert_eq!(options, vec!["-XDrawDiagnostics".to_string()]);
        assert_eq!(files, vec!["Foo.java".to_string()]);
        assert_eq!(
            header.first_with_prefix("run").expect("run tag").value,
            "main Foo"
        );
    }

    #[test]
    fn parses_single_line_test_block() {
        let header = parse_test_header("/* @test */\nclass A {}\n");
        assert!(header.has("test"));
    }

    #[test]
    fn reports_no_tags_for_auxiliary_files() {
        let header = parse_test_header("class Helper {}\n");
        assert!(!header.has("test"));
        assert_eq!(classify(&header).0, TestKind::NotATest);
    }

    #[test]
    fn classifies_each_kind() {
        let classify_tags = |tags: &str| classify(&parse_test_header(&make_header(tags)));

        assert_eq!(
            classify_tags("@test\n@compile A.java").0,
            TestKind::CompileOnly
        );
        assert_eq!(
            classify_tags("@test\n@compile/fail/ref=A.out -XDrawDiagnostics A.java").0,
            TestKind::CompileOnly
        );
        assert_eq!(
            classify_tags("@test\n@run compile A.java").0,
            TestKind::CompileOnly
        );

        assert_eq!(
            classify_tags("@test\n@run main A").0,
            TestKind::CompileAndRun
        );
        assert_eq!(
            classify_tags("@test\n@library /tools/lib\n@compile A.java").0,
            TestKind::NeedsLibraryOrApt
        );
        assert_eq!(
            classify_tags("@test\n@modules java.base\n@compile A.java").0,
            TestKind::Modules
        );
        assert_eq!(
            classify_tags("@test\n@ignore 123\n@compile A.java").0,
            TestKind::Unavailable
        );
        assert_eq!(classify_tags("@bug 1\nclass A {}").0, TestKind::NotATest);

        let (kind, reason) = classify_tags("@test");
        assert_eq!(kind, TestKind::Unavailable);
        assert!(reason.is_some());
    }

    #[test]
    fn scans_synthetic_corpus_and_writes_index() {
        let temp = TempDir::new("rust-javac-corpus").expect("create temp corpus");
        write_case(
            temp.path(),
            "Good.java",
            "@test\n@summary good\n@compile Good.java",
        );
        write_case(
            temp.path(),
            "Bad.java",
            "@test\n@compile/fail/ref=Bad.out -XDrawDiagnostics Bad.java",
        );
        write_case(temp.path(), "RunMe.java", "@test\n@run main RunMe");
        write_case(
            temp.path(),
            "LibTest.java",
            "@test\n@library /tools/lib\n@compile LibTest.java",
        );
        write_case(
            temp.path(),
            "ModTest.java",
            "@test\n@modules java.base\n@compile ModTest.java",
        );
        write_case(
            temp.path(),
            "Ignored.java",
            "@test\n@ignore 123\n@compile Ignored.java",
        );
        fs::write(temp.path().join("Helper.java"), "class Helper {}\n")
            .expect("write auxiliary file");

        let corpus = Corpus::scan(temp.path()).expect("scan synthetic corpus");
        assert_eq!(corpus.cases.len(), 7);
        let stats = corpus.stats();
        assert_eq!(stats.total_files, 7);
        assert_eq!(stats.count(TestKind::CompileOnly), 2);
        assert_eq!(stats.count(TestKind::CompileAndRun), 1);
        assert_eq!(stats.count(TestKind::NeedsLibraryOrApt), 1);
        assert_eq!(stats.count(TestKind::Modules), 1);
        assert_eq!(stats.count(TestKind::Unavailable), 1);
        assert_eq!(stats.count(TestKind::NotATest), 1);
        assert_eq!(corpus.compile_only_cases().len(), 2);

        let index = corpus.index_text();
        assert!(index.contains("total_files 7"), "{}", index);
        assert!(index.contains("count compile_only 2"), "{}", index);
        assert!(index.contains("case compile_only Good.java"), "{}", index);
        assert!(
            index.contains("case needs_library_or_apt LibTest.java"),
            "{}",
            index
        );
        assert!(index.contains("reason ignored: 123"), "{}", index);

        let index_path = temp.path().join("index.txt");
        corpus.write_index(&index_path).expect("write index");
        assert!(index_path.is_file());
    }

    #[test]
    fn corpus_snapshot_matches_langtools() {
        let Some(root) = corpus_root() else {
            eprintln!("skipping corpus_snapshot_matches_langtools: langtools corpus not present");
            return;
        };
        let corpus = Corpus::scan(&root).expect("scan langtools corpus");
        let stats = corpus.stats();
        eprintln!(
            "langtools corpus: total={} compile_only={} compile_and_run={} needs_library_or_apt={} modules={} unavailable={} not_a_test={}",
            stats.total_files,
            stats.count(TestKind::CompileOnly),
            stats.count(TestKind::CompileAndRun),
            stats.count(TestKind::NeedsLibraryOrApt),
            stats.count(TestKind::Modules),
            stats.count(TestKind::Unavailable),
            stats.count(TestKind::NotATest)
        );
        if stats.total_files != LANGTOOLS_TOTAL_FILES {
            eprintln!(
                "skipping snapshot assertions: corpus has {} files, recorded snapshot has {}",
                stats.total_files, LANGTOOLS_TOTAL_FILES
            );
            return;
        }
        assert_eq!(stats.count(TestKind::CompileOnly), LANGTOOLS_COMPILE_ONLY);
        assert_eq!(
            stats.count(TestKind::CompileAndRun),
            LANGTOOLS_COMPILE_AND_RUN
        );
        assert_eq!(
            stats.count(TestKind::NeedsLibraryOrApt),
            LANGTOOLS_NEEDS_LIBRARY_OR_APT
        );
        assert_eq!(stats.count(TestKind::Modules), LANGTOOLS_MODULES);
        assert_eq!(stats.count(TestKind::Unavailable), LANGTOOLS_UNAVAILABLE);
        assert_eq!(stats.count(TestKind::NotATest), LANGTOOLS_NOT_A_TEST);
        assert_eq!(corpus.compile_only_cases().len(), LANGTOOLS_COMPILE_ONLY);

        // 索引输出到 target/（不入库）。
        let target = project_root().join("target/javac-corpus");
        corpus
            .write_index(&target.join("index.txt"))
            .expect("write corpus index");
        assert!(target.join("index.txt").is_file());
    }

    #[test]
    fn sample_cases_are_classified_as_recorded() {
        let Some(root) = corpus_root() else {
            eprintln!("skipping sample_cases_are_classified_as_recorded: corpartus not present");
            return;
        };
        let corpus = Corpus::scan(&root).expect("scan langtools corpus");
        if corpus.cases.len() != LANGTOOLS_TOTAL_FILES {
            eprintln!("skipping sample assertions: corpus is not the recorded snapshot");
            return;
        }
        let find = |path: &str| {
            corpus
                .cases
                .iter()
                .find(|case| case.relative_path == path)
                .unwrap_or_else(|| panic!("missing expected case {}", path))
        };

        let abstract_override = find("AbstractOverride.java");
        assert_eq!(abstract_override.kind, TestKind::CompileOnly);
        assert_eq!(
            abstract_override.compile_files,
            vec!["AbstractOverride.java"]
        );
        assert_eq!(abstract_override.bug_ids, vec!["4929736"]);

        assert_eq!(find("AddReferenceThis.java").kind, TestKind::CompileOnly);
        assert_eq!(find("Ambig3.java").kind, TestKind::CompileOnly);
        assert_eq!(
            find("4846262/CheckEBCDICLocaleTest.java").kind,
            TestKind::Modules
        );
        assert_eq!(find("6257443/T6257443.java").kind, TestKind::CompileAndRun);
        assert_eq!(
            find("annotations/typeAnnotations/failures/CantAnnotatePackages.java").kind,
            TestKind::Unavailable
        );
        assert_eq!(
            find("Paths/ClassPath.java").kind,
            TestKind::NeedsLibraryOrApt
        );
        // @compile/module=... 变体属于模块编译，不是普通 compile-only。
        let module_compile = find("T4093617/T4093617.java");
        assert_eq!(module_compile.kind, TestKind::Modules);
        assert!(
            module_compile
                .exclude_reason
                .as_deref()
                .unwrap_or("")
                .contains("requires module compilation: java.base")
        );
        let modules_case = corpus
            .cases
            .iter()
            .find(|case| case.kind == TestKind::Modules)
            .expect("modules case");
        assert!(
            modules_case
                .exclude_reason
                .as_deref()
                .unwrap_or("")
                .contains("requires modules")
        );
    }

    #[test]
    fn generates_sample_baselines_incrementally() {
        let Some(root) = corpus_root() else {
            eprintln!("skipping generates_sample_baselines_incrementally: corpus not present");
            return;
        };
        let Some(javac) = javac17() else {
            return;
        };
        let corpus = Corpus::scan(&root).expect("scan langtools corpus");
        if corpus.cases.len() != LANGTOOLS_TOTAL_FILES {
            eprintln!("skipping baseline assertions: corpus is not the recorded snapshot");
            return;
        }

        let output_root = project_root().join("target/javac-corpus-test");
        let _ = fs::remove_dir_all(&output_root);
        let mut options = BaselineOptions::new(output_root.clone());
        options.limit = Some(20);
        let first = generate_baselines(&javac, &corpus, &options).expect("first baseline run");
        eprintln!(
            "first baseline run: generated={} up_to_date={} unsupported={} failed={}",
            first.generated, first.up_to_date, first.unsupported, first.failed
        );
        assert!(first.generated > 0, "no baseline was generated");
        assert_eq!(
            first.generated + first.up_to_date + first.unsupported + first.failed,
            20
        );
        assert_eq!(
            first.up_to_date, 0,
            "an empty baseline tree cannot be fresh"
        );

        let baseline_path = first
            .cases
            .iter()
            .find_map(|case| match &case.status {
                BaselineStatus::Generated(path) => Some(path.clone()),
                _ => None,
            })
            .expect("at least one generated baseline");
        let text = fs::read_to_string(&baseline_path).expect("read generated baseline");
        assert!(
            text.starts_with("# rust-javac javac oracle baseline\n"),
            "{}",
            text
        );
        assert!(text.contains("source_fnv "), "{}", text);
        assert!(text.contains("--- normalized ---"), "{}", text);
        assert!(text.contains("exit_code: "), "{}", text);
        assert!(
            !text.contains("rust-javac-oracle"),
            "temporary paths were not normalized:\n{}",
            text
        );

        // 增量：第二次运行只做已生成基线的跳过检查。
        let second = generate_baselines(&javac, &corpus, &options).expect("incremental run");
        eprintln!(
            "incremental run: generated={} up_to_date={} unsupported={} failed={}",
            second.generated, second.up_to_date, second.unsupported, second.failed
        );
        assert_eq!(second.up_to_date, first.generated);
        assert!(
            second.generated <= first.failed,
            "incremental run regenerated {} baselines (first failures: {})",
            second.generated,
            first.failed
        );

        // 刷新：无视已有基线全部重生成。
        let mut refresh = BaselineOptions::new(output_root);
        refresh.limit = Some(20);
        refresh.refresh = true;
        let refreshed = generate_baselines(&javac, &corpus, &refresh).expect("refresh run");
        eprintln!(
            "refresh run: generated={} up_to_date={} unsupported={} failed={}",
            refreshed.generated, refreshed.up_to_date, refreshed.unsupported, refreshed.failed
        );
        assert_eq!(refreshed.generated, first.generated);
        assert_eq!(refreshed.unsupported, first.unsupported);
    }

    #[test]
    #[ignore = "full corpus baseline generation takes a long time; run explicitly"]
    fn generates_full_compile_only_baselines() {
        let Some(root) = corpus_root() else {
            eprintln!("skipping generates_full_compile_only_baselines: corpus not present");
            return;
        };
        let Some(javac) = javac17() else {
            return;
        };
        let corpus = Corpus::scan(&root).expect("scan langtools corpus");
        let total = corpus.compile_only_cases().len();
        let output_root = project_root().join("target/javac-corpus");
        let options = BaselineOptions::new(output_root);
        let report = generate_baselines(&javac, &corpus, &options).expect("full baseline run");
        eprintln!(
            "full baseline run: cases={} generated={} up_to_date={} unsupported={} failed={}",
            total, report.generated, report.up_to_date, report.unsupported, report.failed
        );
        for case in &report.cases {
            match &case.status {
                BaselineStatus::Unsupported(reason) => {
                    eprintln!("unsupported: {} ({})", case.relative_path, reason);
                }
                BaselineStatus::Failed(reason) => {
                    eprintln!("failed: {} ({})", case.relative_path, reason);
                }
                _ => {}
            }
        }
        assert_eq!(
            report.generated + report.up_to_date + report.unsupported + report.failed,
            total
        );
        assert!(report.unsupported + report.failed < total / 2);
    }
}
