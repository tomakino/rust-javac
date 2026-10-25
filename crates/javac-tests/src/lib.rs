//! rust-javac 差分测试基座（oracle harness）。
//!
//! 用途锚点：docs/PLAN.md §7（验证与差分体系：oracle = 本机 javac 17.0.19）。
//! 本 crate 是测试基础设施（适配工具），不是 javac 源码的移植，因此不写移植来源锚点、
//! 不写版权头。
//!
//! 组成：
//! - [javac]：定位本机 javac 17，在临时目录编译源文件并捕获原始 stdout/stderr、退出码、
//!   生成的 class 清单与归一化路径上下文；
//! - [normalize]：固定归一化规则与差分对比（[normalize]、[compare]、[DiffReport]）。
//!
//! 只依赖 std::process，无第三方依赖。

pub mod corpus;
pub mod diagnostics_data;
pub mod diff;
pub mod javac;
pub mod normalize;

pub use corpus::Error as CorpusError;
pub use corpus::{
    BaselineOptions, BaselineReport, CompileInput, CompileInputStatus, Corpus, CorpusStats,
    TestCase, TestKind, generate_baselines, load_compile_input, parse_test_header,
    split_compile_arguments,
};
pub use diff::{
    BaselineStore, Case, CaseResult, CaseSet, DiffCategory, DiffError, FailureDetail, Oracle,
    Outcome, RunReport, Target, load_corpus_cases, load_diagnostics_cases, run, run_compiler,
};
pub use javac::{Error, Javac, RawOutput, SourceFile, TempDir};
pub use normalize::{
    DiffReport, LineDifference, TextDiff, compare, decode_output, normalize, normalize_text,
};

#[cfg(test)]
mod diagnostics_oracle;

#[cfg(test)]
mod oracle_data;

#[cfg(test)]
mod oracle_tests {
    use super::*;

    /// oracle 测试的公共入口：找不到 javac 17 时打印说明并跳过（返回 None）。
    fn oracle() -> Option<Javac> {
        match Javac::find_jdk17() {
            Some(javac) => Some(javac),
            None => {
                eprintln!("skipping oracle test: no JDK 17 javac found (set JAVA_HOME or JAVAC)");
                None
            }
        }
    }

    const HELLO: &str = "public class Hello {\n    public static void main(String[] args) {\n        System.out.println(\"hello\");\n    }\n}\n";
    const BROKEN: &str = "public class Broken {\n    void m() {\n        int x = ;\n    }\n}\n";

    #[test]
    fn locates_jdk17_oracle() {
        let Some(javac) = oracle() else {
            return;
        };
        let version = javac.version().expect("javac -version");
        assert!(
            version.contains("javac 17."),
            "unexpected oracle version: {}",
            version
        );
    }

    #[test]
    fn hello_world_compiles_cleanly() {
        let Some(javac) = oracle() else {
            return;
        };
        let output = javac
            .compile(&[SourceFile::new("Hello.java", HELLO)], &[])
            .expect("compile Hello.java");
        assert_eq!(output.exit_code, 0);
        let stderr = normalize_text(&decode_output(&output.stderr), &output.paths);
        assert_eq!(stderr, "");
        assert_eq!(output.class_files, vec!["Hello.class".to_string()]);
        let normalized = normalize(&output);
        assert!(normalized.contains("exit_code: 0"), "{}", normalized);
        assert!(
            normalized.contains("--- classes ---\nHello.class\n"),
            "{}",
            normalized
        );
    }

    #[test]
    fn syntax_error_reports_diagnostics() {
        let Some(javac) = oracle() else {
            return;
        };
        let output = javac
            .compile(&[SourceFile::new("Broken.java", BROKEN)], &[])
            .expect("compile Broken.java");
        assert_ne!(output.exit_code, 0);
        let stderr = normalize_text(&decode_output(&output.stderr), &output.paths);
        assert!(stderr.contains("error:"), "{}", stderr);
        assert!(stderr.contains("illegal start of expression"), "{}", stderr);
        assert!(stderr.contains("<src>/Broken.java:3:"), "{}", stderr);
        assert!(
            !stderr.contains('\\'),
            "backslashes were not normalized: {}",
            stderr
        );
        assert!(output.class_files.is_empty());
        let normalized = normalize(&output);
        assert!(normalized.contains("--- stderr ---"), "{}", normalized);
    }

    #[test]
    fn should_stop_at_parse_on_error_stops_after_parsing() {
        // -XDshould-stop.at 覆盖的是"出错时"的停止阶段（默认 INIT），语法错误时停在 PARSE。
        let Some(javac) = oracle() else {
            return;
        };
        let output = javac
            .compile(
                &[SourceFile::new("Broken.java", BROKEN)],
                &["-XDshould-stop.at=PARSE"],
            )
            .expect("compile Broken.java with should-stop.at=PARSE");
        assert_ne!(output.exit_code, 0);
        assert!(output.class_files.is_empty());
        let stderr = normalize_text(&decode_output(&output.stderr), &output.paths);
        assert!(stderr.contains("error:"), "{}", stderr);
    }

    #[test]
    fn should_stop_if_no_error_parse_stops_before_parsing() {
        // 无错误时的停止阶段由 -XDshould-stop.ifNoError 控制（默认 GENERATE）。
        let Some(javac) = oracle() else {
            return;
        };
        let output = javac
            .compile(
                &[SourceFile::new("Hello.java", HELLO)],
                &["-XDshould-stop.ifNoError=PARSE"],
            )
            .expect("compile Hello.java with should-stop.ifNoError=PARSE");
        assert_eq!(
            output.exit_code,
            0,
            "stderr: {}",
            decode_output(&output.stderr)
        );
        assert!(
            output.class_files.is_empty(),
            "class files: {:?}",
            output.class_files
        );
    }

    #[test]
    fn passes_through_release_option() {
        let Some(javac) = oracle() else {
            return;
        };
        let output = javac
            .compile(
                &[SourceFile::new("Hello.java", HELLO)],
                &["--release", "17"],
            )
            .expect("compile with --release 17");
        assert_eq!(
            output.exit_code,
            0,
            "stderr: {}",
            decode_output(&output.stderr)
        );
        assert_eq!(output.class_files, vec!["Hello.class".to_string()]);
    }

    #[test]
    fn multiple_sources_and_nested_packages() {
        let Some(javac) = oracle() else {
            return;
        };
        let sources = [
            SourceFile::new(
                "pkg/Greeter.java",
                "package pkg;\n\npublic class Greeter {\n    public String greet() {\n        return \"hi\";\n    }\n}\n",
            ),
            SourceFile::new(
                "Main.java",
                "public class Main {\n    public static void main(String[] args) {\n        System.out.println(new pkg.Greeter().greet());\n    }\n}\n",
            ),
        ];
        let output = javac.compile(&sources, &[]).expect("compile two sources");
        assert_eq!(
            output.exit_code,
            0,
            "stderr: {}",
            decode_output(&output.stderr)
        );
        assert_eq!(
            output.class_files,
            vec!["Main.class".to_string(), "pkg/Greeter.class".to_string()]
        );
    }

    #[test]
    fn compare_oracle_run_with_itself_is_equal() {
        let Some(javac) = oracle() else {
            return;
        };
        let first = javac
            .compile(&[SourceFile::new("Broken.java", BROKEN)], &[])
            .expect("first compile");
        let second = javac
            .compile(&[SourceFile::new("Broken.java", BROKEN)], &[])
            .expect("second compile");
        let report = compare(&first, &second);
        assert!(
            report.equal,
            "oracle runs of the same input must normalize identically: {}",
            report
        );
    }
}
