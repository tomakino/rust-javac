//! 归一化与差分对比。
//!
//! 用途锚点：docs/PLAN.md §7（差分测试体系与归一化规则）。本文件是测试基础设施（适配工具），
//! 不是 javac 源码的移植文件，因此不写移植来源锚点、不写版权头。
//!
//! 固定归一化规则（顺序固定，单测覆盖）：
//! 1. 原始字节解码：优先 UTF-8；不是合法 UTF-8 时按 Windows-1252 单字节映射（无损保留字节）；
//! 2. 行尾统一：CRLF 与单独的 CR 都换成 LF；
//! 3. 噪声剥离：删除 JVM 提示行（"Picked up JAVA_TOOL_OPTIONS:" 等）；
//! 4. 绝对路径 -> 占位符：调用方登记的路径（源文件/输出目录/临时目录/JDK/类路径）替换为
//!    <src>、<out>、<tmp>、<jdk> 等占位符，长路径优先；
//! 5. 平台路径分隔符：反斜杠换成 '/'；
//! 6. 行尾空白修剪、去掉首尾空行，有内容时以单个 LF 结尾。

use std::fmt;
use std::path::PathBuf;

use crate::javac::RawOutput;

/// 一次文本对比中最多记录多少处差异（避免报告过大）。
const MAX_REPORTED_DIFFERENCES: usize = 20;

/// 原始字节 -> 文本：优先按 UTF-8 严格解码；失败时用 Windows-1252 单字节映射。
///
/// javac oracle 的输出编码由 harness 固定为 UTF-8（-J-Dfile.encoding=UTF-8）；
/// Windows-1252 回退保证即使输出是平台默认编码也不会丢字节（映射后的差异仍然可比）。
pub fn decode_output(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => decode_windows_1252(bytes),
    }
}

/// Windows-1252 单字节映射（0x80..=0x9F 为扩展字符，其余与 Latin-1 相同）。
fn decode_windows_1252(bytes: &[u8]) -> String {
    const HIGH: [char; 32] = [
        '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}',
        '\u{017D}', '\u{008F}', '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}',
        '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}',
        '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
    ];
    bytes
        .iter()
        .map(|&byte| match byte {
            0x00..=0x7F => byte as char,
            0x80..=0x9F => HIGH[(byte - 0x80) as usize],
            _ => byte as char,
        })
        .collect()
}

/// 判断是否为 JVM 环境噪声行（javac 启动时由 JVM 打印）。
fn is_jvm_noise_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("Picked up JAVA_TOOL_OPTIONS:")
        || trimmed.starts_with("Picked up _JAVA_OPTIONS:")
        || trimmed.starts_with("Picked up JDK_JAVA_OPTIONS:")
}

/// 文本归一化。paths 是按实际路径长度降序排列的 (实际路径, 占位符) 列表
/// （由 RawOutput::sort_paths 保证顺序）。
pub fn normalize_text(text: &str, paths: &[(PathBuf, String)]) -> String {
    let unified = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<String> = Vec::new();
    for raw_line in unified.split('\n') {
        if is_jvm_noise_line(raw_line) {
            continue;
        }
        let mut line = raw_line.to_string();
        for (actual, placeholder) in paths {
            let actual_text = actual.to_string_lossy();
            if !actual_text.is_empty() {
                line = line.replace(actual_text.as_ref(), placeholder);
            }
        }
        let line = line.replace('\\', "/");
        lines.push(line.trim_end().to_string());
    }
    while lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        String::new()
    } else {
        let mut result = lines.join("\n");
        result.push('\n');
        result
    }
}

/// 把一次运行归一化为稳定的可比较文本（退出码 + 规范化 stdout/stderr + class 清单）。
pub fn normalize(output: &RawOutput) -> String {
    let mut text = String::new();
    text.push_str("exit_code: ");
    text.push_str(&output.exit_code.to_string());
    text.push('\n');
    text.push_str("--- stdout ---\n");
    text.push_str(&normalize_text(
        &decode_output(&output.stdout),
        &output.paths,
    ));
    text.push_str("--- stderr ---\n");
    text.push_str(&normalize_text(
        &decode_output(&output.stderr),
        &output.paths,
    ));
    text.push_str("--- classes ---\n");
    let mut classes = output.class_files.clone();
    classes.sort();
    for class in &classes {
        text.push_str(class);
        text.push('\n');
    }
    text
}

/// 单行文本差异。
#[derive(Debug, Clone)]
pub struct LineDifference {
    /// 1-based 行号。
    pub line: usize,
    /// 缺失时为 None。
    pub expected: Option<String>,
    /// 缺失时为 None。
    pub actual: Option<String>,
}

/// 一段文本（stdout/stderr）的对比结果。
#[derive(Debug, Clone)]
pub struct TextDiff {
    pub equal: bool,
    pub expected_line_count: usize,
    pub actual_line_count: usize,
    pub differences: Vec<LineDifference>,
}

impl TextDiff {
    fn new(expected: &str, actual: &str) -> TextDiff {
        let expected_lines: Vec<&str> = if expected.is_empty() {
            Vec::new()
        } else {
            expected.lines().collect()
        };
        let actual_lines: Vec<&str> = if actual.is_empty() {
            Vec::new()
        } else {
            actual.lines().collect()
        };
        let mut differences = Vec::new();
        let longest = expected_lines.len().max(actual_lines.len());
        for index in 0..longest {
            let expected_line = expected_lines.get(index).copied();
            let actual_line = actual_lines.get(index).copied();
            if expected_line != actual_line {
                differences.push(LineDifference {
                    line: index + 1,
                    expected: expected_line.map(str::to_string),
                    actual: actual_line.map(str::to_string),
                });
                if differences.len() >= MAX_REPORTED_DIFFERENCES {
                    break;
                }
            }
        }
        TextDiff {
            equal: differences.is_empty(),
            expected_line_count: expected_lines.len(),
            actual_line_count: actual_lines.len(),
            differences,
        }
    }
}

/// 差分报告：退出码、规范化 stdout、规范化 stderr、生成的 class 清单。
#[derive(Debug, Clone)]
pub struct DiffReport {
    pub equal: bool,
    pub exit_code_matches: bool,
    pub expected_exit_code: i32,
    pub actual_exit_code: i32,
    pub stdout: TextDiff,
    pub stderr: TextDiff,
    pub class_files_matches: bool,
    pub expected_class_files: Vec<String>,
    pub actual_class_files: Vec<String>,
}

/// 比较两次运行（reference 通常是 javac oracle，actual 是 rust-javac）。
pub fn compare(reference: &RawOutput, actual: &RawOutput) -> DiffReport {
    let expected_stdout = normalize_text(&decode_output(&reference.stdout), &reference.paths);
    let actual_stdout = normalize_text(&decode_output(&actual.stdout), &actual.paths);
    let expected_stderr = normalize_text(&decode_output(&reference.stderr), &reference.paths);
    let actual_stderr = normalize_text(&decode_output(&actual.stderr), &actual.paths);

    let mut expected_class_files = reference.class_files.clone();
    expected_class_files.sort();
    let mut actual_class_files = actual.class_files.clone();
    actual_class_files.sort();

    let exit_code_matches = reference.exit_code == actual.exit_code;
    let stdout = TextDiff::new(&expected_stdout, &actual_stdout);
    let stderr = TextDiff::new(&expected_stderr, &actual_stderr);
    let class_files_matches = expected_class_files == actual_class_files;

    DiffReport {
        equal: exit_code_matches && stdout.equal && stderr.equal && class_files_matches,
        exit_code_matches,
        expected_exit_code: reference.exit_code,
        actual_exit_code: actual.exit_code,
        stdout,
        stderr,
        class_files_matches,
        expected_class_files,
        actual_class_files,
    }
}

impl fmt::Display for DiffReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.equal {
            writeln!(f, "differential report: PASS")?;
            return Ok(());
        }
        writeln!(f, "differential report: FAIL")?;
        if self.exit_code_matches {
            writeln!(f, "exit code: {} (match)", self.expected_exit_code)?;
        } else {
            writeln!(
                f,
                "exit code: expected {}, actual {}",
                self.expected_exit_code, self.actual_exit_code
            )?;
        }
        fmt_text_diff(f, "stdout", &self.stdout)?;
        fmt_text_diff(f, "stderr", &self.stderr)?;
        if self.class_files_matches {
            writeln!(
                f,
                "class files: match ({})",
                self.expected_class_files.len()
            )?;
        } else {
            writeln!(
                f,
                "class files: expected {:?}, actual {:?}",
                self.expected_class_files, self.actual_class_files
            )?;
        }
        Ok(())
    }
}

fn fmt_text_diff(f: &mut fmt::Formatter<'_>, name: &str, diff: &TextDiff) -> fmt::Result {
    if diff.equal {
        writeln!(f, "{}: match ({} lines)", name, diff.expected_line_count)?;
        return Ok(());
    }
    writeln!(
        f,
        "{}: differs (expected {} lines, actual {} lines)",
        name, diff.expected_line_count, diff.actual_line_count
    )?;
    for difference in &diff.differences {
        match (&difference.expected, &difference.actual) {
            (Some(expected), Some(actual)) => {
                writeln!(f, "  line {}:", difference.line)?;
                writeln!(f, "    expected: {}", expected)?;
                writeln!(f, "    actual:   {}", actual)?;
            }
            (Some(expected), None) => {
                writeln!(
                    f,
                    "  line {}: missing in actual: {}",
                    difference.line, expected
                )?;
            }
            (None, Some(actual)) => {
                writeln!(
                    f,
                    "  line {}: unexpected in actual: {}",
                    difference.line, actual
                )?;
            }
            (None, None) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output_with(
        exit_code: i32,
        stdout: &[u8],
        stderr: &[u8],
        class_files: &[&str],
    ) -> RawOutput {
        RawOutput::new(
            exit_code,
            stdout.to_vec(),
            stderr.to_vec(),
            class_files.iter().map(|name| name.to_string()).collect(),
        )
    }

    #[test]
    fn decodes_utf8() {
        assert_eq!(decode_output("hello 中文".as_bytes()), "hello 中文");
        assert_eq!(decode_output(b""), "");
    }

    #[test]
    fn falls_back_to_windows_1252() {
        // 0x80 是 Windows-1252 的欧元符号；0xE9 与 Latin-1 相同（é）。
        assert_eq!(decode_output(&[0x80, 0xE9]), "\u{20AC}\u{00E9}");
        assert_eq!(decode_output(&[0x41, 0x42]), "AB");
    }

    #[test]
    fn unifies_line_endings() {
        assert_eq!(normalize_text("a\r\nb\rc\nd", &[]), "a\nb\nc\nd\n");
        assert_eq!(normalize_text("single", &[]), "single\n");
        assert_eq!(normalize_text("", &[]), "");
    }

    #[test]
    fn strips_jvm_noise_lines() {
        let text = "Picked up JAVA_TOOL_OPTIONS: -Xmx1g\nerror: bad\nPicked up _JAVA_OPTIONS: -Xss\ntail\n";
        assert_eq!(normalize_text(text, &[]), "error: bad\ntail\n");
    }

    #[test]
    fn replaces_registered_paths_longest_first() {
        let temp = PathBuf::from(r"C:\tmp\work");
        let output_dir = PathBuf::from(r"C:\tmp\work\out");
        let source = PathBuf::from(r"C:\tmp\work\Hello.java");
        let mut stderr = Vec::new();
        stderr.extend_from_slice(r"C:\tmp\work\Hello.java:3: error: bad".as_bytes());
        stderr.push(b'\n');
        let mut output = output_with(1, b"", &stderr, &[]);
        output.push_path(output_dir, "<out>");
        output.push_path(source, "<src>/Hello.java");
        output.push_path(temp, "<tmp>");
        output.sort_paths();
        let normalized = normalize_text(&decode_output(&output.stderr), &output.paths);
        assert_eq!(normalized, "<src>/Hello.java:3: error: bad\n");
    }

    #[test]
    fn trims_trailing_whitespace_and_blank_lines() {
        assert_eq!(normalize_text("x  \ny\n\n", &[]), "x\ny\n");
        assert_eq!(normalize_text("\n\nx\n\n", &[]), "x\n");
    }

    #[test]
    fn path_separators_become_slashes() {
        assert_eq!(
            normalize_text(r"load C:\classes\a.jar", &[]),
            "load C:/classes/a.jar\n"
        );
    }

    #[test]
    fn normalizes_a_whole_output() {
        let output = output_with(
            0,
            b"out\n",
            b"Picked up JAVA_TOOL_OPTIONS: -Dx\nwarn\n",
            &["pkg/A.class", "B.class"],
        );
        let normalized = normalize(&output);
        assert_eq!(
            normalized,
            "exit_code: 0\n--- stdout ---\nout\n--- stderr ---\nwarn\n--- classes ---\nB.class\npkg/A.class\n"
        );
    }

    #[test]
    fn compare_equal_runs() {
        let reference = output_with(0, b"same\n", b"", &["A.class"]);
        let actual = output_with(0, b"same\n", b"", &["A.class"]);
        let report = compare(&reference, &actual);
        assert!(report.equal);
        assert!(report.exit_code_matches);
        assert!(report.stdout.equal);
        assert!(report.stderr.equal);
        assert!(report.class_files_matches);
        assert!(report.to_string().contains("PASS"));
    }

    #[test]
    fn compare_reports_all_difference_kinds() {
        let reference = output_with(0, b"line1\nline2\n", b"", &["A.class"]);
        let actual = output_with(
            1,
            b"line1\nline2 changed\n",
            b"boom\n",
            &["A.class", "B.class"],
        );
        let report = compare(&reference, &actual);
        assert!(!report.equal);
        assert!(!report.exit_code_matches);
        assert!(!report.stdout.equal);
        assert_eq!(report.stdout.differences[0].line, 2);
        assert_eq!(
            report.stdout.differences[0].expected.as_deref(),
            Some("line2")
        );
        assert_eq!(
            report.stdout.differences[0].actual.as_deref(),
            Some("line2 changed")
        );
        assert!(!report.stderr.equal);
        assert!(!report.class_files_matches);
        let text = report.to_string();
        assert!(text.contains("FAIL"), "{}", text);
        assert!(text.contains("line2 changed"), "{}", text);
        assert!(text.contains("expected 0, actual 1"), "{}", text);
    }

    #[test]
    fn compare_handles_line_count_differences() {
        let reference = output_with(0, b"a\nb\nc\n", b"", &[]);
        let actual = output_with(0, b"a\n", b"", &[]);
        let report = compare(&reference, &actual);
        assert!(!report.equal);
        let differences = &report.stdout.differences;
        assert_eq!(differences.len(), 2);
        assert_eq!(differences[0].line, 2);
        assert!(differences[0].actual.is_none());
    }
}
