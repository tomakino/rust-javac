//! diagnostics-oracle 数据的解析与查询（运行期零 JVM 依赖）。
//!
//! 用途锚点:docs/PLAN.md §7(差分测试体系)。本文件是测试基础设施,不是 javac 源码的
//! 移植文件,因此不写移植来源锚点、不写版权头。
//!
//! 数据文件 data/diagnostics-oracle.txt 由开发期工具
//! crates/javac-tests/src/bin/gen_diagnostics_oracle.rs 生成;格式见数据头与 README。
//! 本模块提供:
//! - [parse] / [DiagnosticsData] 解析入库数据;
//! - [Record::expected_raw] 把一条记录还原成可比较的 [RawOutput](默认或 -XDrawDiagnostics 模式);
//! - 供差分运行器(src/diff.rs)按记录重建用例的期望输出,也供数据完整性测试使用。

use std::path::Path;

use crate::javac::{RawOutput, SourceFile};

/// 一条诊断 oracle 记录(一个用例的两次 javac 运行)。
#[derive(Debug)]
pub struct Record {
    /// 用例名,例如 "syntax/IllegalStartOfExpression" 或 "corpus/IllegalStartOfExpr"。
    pub name: String,
    /// 来源:builtin:<name> 或 corpus:<相对语料根的路径>。
    pub source: String,
    /// 透传给 javac 的选项。
    pub args: Vec<String>,
    /// corpus 示例声明的诊断 key(来自 "// key:" 行;builtin 用例为空)。
    pub corpus_keys: Vec<String>,
    pub exit_code: i32,
    pub draw_exit_code: i32,
    pub draw_diagnostics: usize,
    pub draw_keys: Vec<String>,
    pub classes: Vec<String>,
    pub stdout_default: String,
    pub stderr_default: String,
    pub stdout_draw: String,
    pub stderr_draw: String,
}

/// 诊断输出模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// javac 默认诊断格式(path:line: error: msg + 源码行/^ 定位行)。
    Default,
    /// -XDrawDiagnostics 格式(key/位置/参数)。
    Draw,
}

impl Record {
    /// 把记录还原成可比较的 [RawOutput](供差分运行器与自测使用)。
    pub fn expected_raw(&self, mode: Mode) -> RawOutput {
        let (exit_code, stdout, stderr) = match mode {
            Mode::Default => (self.exit_code, &self.stdout_default, &self.stderr_default),
            Mode::Draw => (self.draw_exit_code, &self.stdout_draw, &self.stderr_draw),
        };
        RawOutput::new(
            exit_code,
            stdout.as_bytes().to_vec(),
            stderr.as_bytes().to_vec(),
            self.classes.clone(),
        )
    }
}

/// 头部字段值(数据文件顶层 "# key: value" 行)。
pub fn header_value<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.lines().find_map(|line| line.strip_prefix(prefix))
}

/// 解析入库的诊断 oracle 数据。
pub fn parse(text: &str) -> Vec<Record> {
    let mut records = Vec::new();
    let mut current: Option<PartialRecord> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("=== CASE: ") {
            assert!(current.is_none(), "nested case marker: {line}");
            let name = rest
                .strip_suffix(" ===")
                .unwrap_or_else(|| panic!("case marker must end with ===: {line}"))
                .to_string();
            current = Some(PartialRecord::new(name));
            continue;
        }
        if line == "=== END ===" {
            let partial = current.take().expect("=== END === without a case");
            records.push(partial.finish());
            continue;
        }
        let Some(partial) = current.as_mut() else {
            assert!(
                line.is_empty() || line.starts_with('#'),
                "unexpected line outside a case: {line}"
            );
            continue;
        };
        if let Some(rest) = line.strip_prefix("--- ") {
            let name = rest
                .strip_suffix(" ---")
                .unwrap_or_else(|| panic!("section marker must end with ---: {line}"));
            partial.begin_section(section_enum(name));
            continue;
        }
        if let Some(rest) = line.strip_prefix("# ") {
            let (key, value) = rest
                .split_once(": ")
                .unwrap_or_else(|| panic!("malformed case header: {line}"));
            partial.set_header(key, value);
            continue;
        }
        if line.is_empty() {
            continue;
        }
        partial.push_line(line);
    }
    assert!(current.is_none(), "unterminated case at end of data");
    records
}

#[derive(Clone, Copy)]
enum Section {
    StdoutDefault,
    StderrDefault,
    StdoutDraw,
    StderrDraw,
}

struct PartialRecord {
    record: Record,
    current: String,
    current_section: Option<Section>,
    seen_first_section: bool,
}

impl PartialRecord {
    fn new(name: String) -> PartialRecord {
        PartialRecord {
            record: Record {
                name,
                source: String::new(),
                args: Vec::new(),
                corpus_keys: Vec::new(),
                exit_code: 0,
                draw_exit_code: 0,
                draw_diagnostics: 0,
                draw_keys: Vec::new(),
                classes: Vec::new(),
                stdout_default: String::new(),
                stderr_default: String::new(),
                stdout_draw: String::new(),
                stderr_draw: String::new(),
            },
            current: String::new(),
            current_section: None,
            seen_first_section: false,
        }
    }

    fn set_header(&mut self, key: &str, value: &str) {
        assert!(
            !self.seen_first_section,
            "header line {key:?} appears after a section in case {}",
            self.record.name
        );
        match key {
            "source" => self.record.source = value.to_string(),
            "args" => self.record.args = split_words(value),
            "corpus-keys" => self.record.corpus_keys = split_commas(value),
            "exit-code" => self.record.exit_code = parse_number("exit-code", value),
            "draw-exit-code" => self.record.draw_exit_code = parse_number("draw-exit-code", value),
            "draw-diagnostics" => {
                self.record.draw_diagnostics = parse_number("draw-diagnostics", value) as usize
            }
            "draw-keys" => self.record.draw_keys = split_commas(value),
            "classes" => self.record.classes = split_words(value),
            other => panic!("unknown case header {other:?} in case {}", self.record.name),
        }
    }

    fn begin_section(&mut self, section: Section) {
        self.finish_section();
        self.seen_first_section = true;
        self.current_section = Some(section);
    }

    fn push_line(&mut self, line: &str) {
        assert!(
            self.current_section.is_some(),
            "section content before a section marker"
        );
        if !self.current.is_empty() {
            self.current.push('\n');
        }
        self.current.push_str(line);
    }

    fn finish_section(&mut self) {
        let Some(section) = self.current_section.take() else {
            return;
        };
        let mut text = std::mem::take(&mut self.current);
        if !text.is_empty() {
            text.push('\n');
        }
        match section {
            Section::StdoutDefault => self.record.stdout_default = text,
            Section::StderrDefault => self.record.stderr_default = text,
            Section::StdoutDraw => self.record.stdout_draw = text,
            Section::StderrDraw => self.record.stderr_draw = text,
        }
    }

    fn finish(mut self) -> Record {
        self.finish_section();
        assert!(
            self.seen_first_section,
            "case {} has no content section",
            self.record.name
        );
        assert!(
            !self.record.source.is_empty(),
            "case {} has no source",
            self.record.name
        );
        self.record
    }
}

fn split_words(value: &str) -> Vec<String> {
    if value == "-" || value.is_empty() {
        Vec::new()
    } else {
        value.split(' ').map(str::to_string).collect()
    }
}

fn split_commas(value: &str) -> Vec<String> {
    if value == "-" || value.is_empty() {
        Vec::new()
    } else {
        value.split(',').map(str::to_string).collect()
    }
}

fn parse_number(field: &str, value: &str) -> i32 {
    value
        .parse()
        .unwrap_or_else(|_| panic!("case header {field} is not a number: {value:?}"))
}

fn section_enum(name: &str) -> Section {
    match name {
        "stdout-default" => Section::StdoutDefault,
        "stderr-default" => Section::StderrDefault,
        "stdout-draw" => Section::StdoutDraw,
        "stderr-draw" => Section::StderrDraw,
        other => panic!("unknown diagnostics oracle section: {other}"),
    }
}

/// 按诊断行提取 -XDrawDiagnostics 的 key(与生成器的提取规则一致)。
pub fn extract_draw_keys(stderr: &str) -> std::collections::BTreeSet<String> {
    let mut keys = std::collections::BTreeSet::new();
    for line in stderr.lines() {
        let text = if let Some(rest) = line.strip_prefix("- compiler.") {
            format!("compiler.{rest}")
        } else if let Some((_, rest)) = line.split_once(": compiler.") {
            format!("compiler.{rest}")
        } else if line.starts_with("compiler.") {
            line.to_string()
        } else {
            continue;
        };
        keys.insert(
            text.split(':')
                .next()
                .unwrap_or(text.as_str())
                .trim()
                .to_string(),
        );
    }
    keys
}

/// 统计 -XDrawDiagnostics 输出的诊断行数。
pub fn count_draw_diagnostics(stderr: &str) -> usize {
    stderr
        .lines()
        .filter(|line| {
            line.starts_with("- compiler.")
                || line.split_once(": compiler.").is_some()
                || line.starts_with("compiler.")
        })
        .count()
}

/// 入库的诊断 oracle 数据集合。
#[derive(Debug)]
pub struct DiagnosticsData {
    pub records: Vec<Record>,
}

impl DiagnosticsData {
    /// 解析数据文本。
    pub fn parse(text: &str) -> DiagnosticsData {
        DiagnosticsData {
            records: parse(text),
        }
    }

    /// 从文件加载数据。
    pub fn load(path: &Path) -> Result<DiagnosticsData, std::io::Error> {
        let text = std::fs::read_to_string(path)?;
        Ok(DiagnosticsData::parse(&text))
    }

    /// 按用例名查找记录。
    pub fn find(&self, name: &str) -> Option<&Record> {
        self.records.iter().find(|record| record.name == name)
    }
}

/// 一个 builtin 诊断用例(源码入库于 data/diagnostics-builtin-cases.txt)。
#[derive(Debug, Clone)]
pub struct BuiltinCase {
    pub name: String,
    pub args: Vec<String>,
    pub sources: Vec<SourceFile>,
}

/// 入库的 builtin 诊断用例集合。
#[derive(Debug, Default)]
pub struct BuiltinCases {
    pub cases: Vec<BuiltinCase>,
}

impl BuiltinCases {
    /// 解析用例文件。格式(所有源文件内容以单个 LF 结尾):
    ///
    /// ```text
    /// === CASE: <name> ===
    /// # args: - | <javac options>
    /// --- source: <file name> ---
    /// <source lines>
    /// === END ===
    /// ```
    ///
    /// 每个用例可包含多个 source 段;args 段必须在 source 段之前。
    pub fn parse(text: &str) -> BuiltinCases {
        let mut cases = Vec::new();
        let mut current: Option<PartialBuiltinCase> = None;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("=== CASE: ") {
                assert!(current.is_none(), "nested builtin case marker: {line}");
                let name = rest
                    .strip_suffix(" ===")
                    .unwrap_or_else(|| panic!("builtin case marker must end with ===: {line}"))
                    .to_string();
                current = Some(PartialBuiltinCase::new(name));
                continue;
            }
            let Some(case) = current.as_mut() else {
                assert!(
                    line.is_empty() || line.starts_with('#'),
                    "unexpected line outside a builtin case: {line}"
                );
                continue;
            };
            if let Some(rest) = line.strip_prefix("--- source: ") {
                let file_name = rest
                    .strip_suffix(" ---")
                    .unwrap_or_else(|| panic!("builtin source marker must end with ---: {line}"))
                    .to_string();
                case.begin_source(file_name);
                continue;
            }
            if line == "=== END ===" {
                cases.push(current.take().expect("case").finish());
                continue;
            }
            if case.source_name.is_some() {
                // Source content: keep every line verbatim, including lines that look like
                // headers of other sections.
                case.push_source_line(line);
                continue;
            }
            if let Some(rest) = line.strip_prefix("# args: ") {
                case.args = split_words(rest);
                continue;
            }
            if line.is_empty() {
                continue;
            }
            panic!("unexpected line in builtin case {}: {line}", case.name);
        }
        assert!(
            current.is_none(),
            "unterminated builtin case at end of data"
        );
        BuiltinCases { cases }
    }

    /// 从文件加载。
    pub fn load(path: &Path) -> Result<BuiltinCases, std::io::Error> {
        let text = std::fs::read_to_string(path)?;
        Ok(BuiltinCases::parse(&text))
    }

    /// 按用例名查找。
    pub fn find(&self, name: &str) -> Option<&BuiltinCase> {
        self.cases.iter().find(|case| case.name == name)
    }
}

struct PartialBuiltinCase {
    name: String,
    args: Vec<String>,
    sources: Vec<SourceFile>,
    source_name: Option<String>,
    source_lines: Vec<String>,
}

impl PartialBuiltinCase {
    fn new(name: String) -> PartialBuiltinCase {
        PartialBuiltinCase {
            name,
            args: Vec::new(),
            sources: Vec::new(),
            source_name: None,
            source_lines: Vec::new(),
        }
    }

    fn begin_source(&mut self, file_name: String) {
        self.finish_source();
        self.source_name = Some(file_name);
    }

    fn push_source_line(&mut self, line: &str) {
        self.source_lines.push(line.to_string());
    }

    fn finish_source(&mut self) {
        let Some(file_name) = self.source_name.take() else {
            return;
        };
        let lines = std::mem::take(&mut self.source_lines);
        let contents = if lines.is_empty() {
            String::new()
        } else {
            let mut text = lines.join("\n");
            text.push('\n');
            text
        };
        self.sources.push(SourceFile::new(file_name, contents));
    }

    fn finish(mut self) -> BuiltinCase {
        self.finish_source();
        assert!(
            !self.sources.is_empty(),
            "builtin case {} has no source",
            self.name
        );
        BuiltinCase {
            name: self.name,
            args: self.args,
            sources: self.sources,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
=== CASE: demo/case ===
# source: builtin:demo/case
# args: -
# exit-code: 1
# draw-exit-code: 1
# draw-diagnostics: 1
# draw-keys: compiler.err.expected
# classes: -
--- stdout-default ---
--- stderr-default ---
<src>/Demo.java:1: error: ';' expected
--- stdout-draw ---
--- stderr-draw ---
Demo.java:1:14: compiler.err.expected: ';'
=== END ===
";

    #[test]
    fn parses_a_record_and_restores_both_modes() {
        let data = DiagnosticsData::parse(SAMPLE);
        assert_eq!(data.records.len(), 1);
        let record = data.find("demo/case").expect("record");
        assert_eq!(record.exit_code, 1);
        assert_eq!(record.draw_keys, vec!["compiler.err.expected"]);
        assert_eq!(record.classes, Vec::<String>::new());

        let default = record.expected_raw(Mode::Default);
        assert_eq!(default.exit_code, 1);
        assert_eq!(
            String::from_utf8(default.stderr).expect("stderr"),
            "<src>/Demo.java:1: error: ';' expected\n"
        );
        let draw = record.expected_raw(Mode::Draw);
        assert_eq!(draw.exit_code, 1);
        assert_eq!(
            String::from_utf8(draw.stderr).expect("stderr"),
            "Demo.java:1:14: compiler.err.expected: ';'\n"
        );
    }

    #[test]
    fn extracts_draw_keys_and_counts() {
        let stderr = "A.java:1:1: compiler.err.expected: ';'\n- compiler.note.unchecked.recompile\n1 error\n";
        let keys = extract_draw_keys(stderr);
        assert_eq!(
            keys.into_iter().collect::<Vec<_>>(),
            vec![
                "compiler.err.expected".to_string(),
                "compiler.note.unchecked.recompile".to_string()
            ]
        );
        assert_eq!(count_draw_diagnostics(stderr), 2);
    }
    #[test]
    fn parses_builtin_cases_with_sources_and_args() {
        let text = "\
# builtin cases comment
=== CASE: demo/one ===
# args: -Xlint:all
--- source: One.java ---
class One {
    int x;
}
--- source: Two.java ---
class Two {
}
=== END ===
=== CASE: demo/two ===
# args: -
--- source: Plain.java ---
class Plain {
}
=== END ===
";
        let cases = BuiltinCases::parse(text);
        assert_eq!(cases.cases.len(), 2);
        let one = cases.find("demo/one").expect("demo/one");
        assert_eq!(one.args, vec!["-Xlint:all".to_string()]);
        assert_eq!(one.sources.len(), 2);
        assert_eq!(one.sources[0].name, "One.java");
        assert_eq!(one.sources[0].contents, "class One {\n    int x;\n}\n");
        assert_eq!(one.sources[1].name, "Two.java");
        assert_eq!(one.sources[1].contents, "class Two {\n}\n");
        let two = cases.find("demo/two").expect("demo/two");
        assert!(two.args.is_empty());
        assert_eq!(two.sources.len(), 1);
        assert_eq!(two.sources[0].contents, "class Plain {\n}\n");
        assert!(cases.find("demo/missing").is_none());
    }
}
