//! 差分运行器命令行(P9 基础设施)。
//!
//! 用途锚点:docs/PLAN.md §7(差分测试体系)。本程序是测试基础设施(适配工具),不是
//! javac 源码的移植,不写移植来源锚点、不写版权头。
//!
//! 用目标编译器命令对语料/诊断 oracle 用例重跑编译,并与参考 oracle
//! (入库诊断数据 / 已记录 corpus 基线 / 实时 javac 17)比较退出码、规范化 stdout/stderr
//! 与 class 清单。退出码:0 全过,1 有失败,2 环境缺失(见 crates/javac-tests/README.md)。

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use javac_tests::corpus::Corpus;
use javac_tests::diagnostics_data::{BuiltinCases, DiagnosticsData, Mode};
use javac_tests::diff::{
    DiffError, Oracle, RunReport, Target, load_corpus_cases, load_diagnostics_cases, run,
};

const DEFAULT_DIAGNOSTICS: &str = "crates/javac-tests/data/diagnostics-oracle.txt";
const DEFAULT_BUILTIN_CASES: &str = "crates/javac-tests/data/diagnostics-builtin-cases.txt";
const DEFAULT_BASELINES: &str = "target/javac-corpus/baselines";
const DEFAULT_CORPUS_ROOT: &str = "internal/reference/jdk17u/test/langtools/tools/javac";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceKind {
    Diagnostics,
    Corpus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OracleKind {
    Diagnostics,
    Baselines,
    Javac,
}

#[derive(Debug)]
struct Options {
    target: Option<PathBuf>,
    target_args: Vec<String>,
    source: Option<SourceKind>,
    oracle: OracleKind,
    diagnostics: PathBuf,
    builtin_cases: PathBuf,
    baselines: PathBuf,
    corpus_root: PathBuf,
    mode: Mode,
    filter: Option<String>,
    limit: Option<usize>,
    max_details: usize,
    list: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            target: None,
            target_args: Vec::new(),
            source: None,
            oracle: OracleKind::Diagnostics,
            diagnostics: PathBuf::from(DEFAULT_DIAGNOSTICS),
            builtin_cases: PathBuf::from(DEFAULT_BUILTIN_CASES),
            baselines: PathBuf::from(DEFAULT_BASELINES),
            corpus_root: PathBuf::from(DEFAULT_CORPUS_ROOT),
            mode: Mode::Default,
            filter: None,
            limit: None,
            max_details: 10,
            list: false,
        }
    }
}

const USAGE: &str = "usage: diff_runner --target PROGRAM [OPTIONS]

Runs a target compiler over corpus/diagnostics cases and compares the outcome with a
recorded oracle (diagnostics data, corpus baselines, or a live javac 17).

options:
  --target PROGRAM       target compiler executable (required), e.g. javac or rust-javac
  --target-arg VALUE     extra fixed argument for the target command (repeatable)
  --source KIND          case source: diagnostics (default) or corpus
  --oracle KIND          oracle: diagnostics (default), baselines, or javac
  --diagnostics PATH     diagnostics oracle data (default: crates/javac-tests/data/diagnostics-oracle.txt)
  --builtin-cases PATH   builtin case sources (default: crates/javac-tests/data/diagnostics-builtin-cases.txt)
  --baselines DIR        corpus baseline directory (default: target/javac-corpus/baselines)
  --corpus-root DIR      langtools corpus root (default: reference/jdk17u/test/langtools/tools/javac)
  --mode default|draw    diagnostics mode (default: default; --source diagnostics only)
  --filter SUBSTR        only run cases whose name contains SUBSTR
  --limit N              run at most N cases
  --max-details N        print at most N failure/skip details (default 10)
  --list                 list the selected cases without running the target
  -h, --help             show this help

exit codes: 0 all compared cases passed, 1 differential failures, 2 environment missing";

fn main() -> ExitCode {
    match main_logic() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("diff-runner: {error}");
            if matches!(error, DiffError::JdkUnavailable) {
                eprintln!("skipped: no JDK 17 javac found (set JAVA_HOME or JAVAC)");
            }
            ExitCode::from(error.exit_code() as u8)
        }
    }
}

fn main_logic() -> Result<u8, DiffError> {
    let options = parse_options(env::args().skip(1))?;
    let Some(target_program) = options.target.clone() else {
        return Err(DiffError::InvalidConfiguration(
            "--target PROGRAM is required".to_string(),
        ));
    };
    let target = Target::with_args(target_program, options.target_args.clone());

    // 缺省用例来源随 oracle 走:baselines 对应 corpus,其余对应入库诊断数据。
    let source = options.source.unwrap_or(match options.oracle {
        OracleKind::Baselines => SourceKind::Corpus,
        _ => SourceKind::Diagnostics,
    });
    if source == SourceKind::Corpus && options.oracle == OracleKind::Diagnostics {
        return Err(DiffError::InvalidConfiguration(
            "--source corpus cannot use --oracle diagnostics (use --oracle baselines or javac)"
                .to_string(),
        ));
    }
    if source == SourceKind::Diagnostics && options.oracle == OracleKind::Baselines {
        return Err(DiffError::InvalidConfiguration(
            "--source diagnostics cannot use --oracle baselines (use --source corpus)".to_string(),
        ));
    }

    let builtin_cases = match BuiltinCases::load(&options.builtin_cases) {
        Ok(cases) => Some(cases),
        Err(error) => {
            eprintln!(
                "diff-runner: warning: cannot load builtin cases {}: {error}; builtin cases will be skipped",
                options.builtin_cases.display()
            );
            None
        }
    };

    let mut case_set;
    let oracle;
    let mut load_skips = Vec::new();
    match source {
        SourceKind::Diagnostics => {
            let data = DiagnosticsData::load(&options.diagnostics)
                .map_err(|_| DiffError::OracleMissing(options.diagnostics.clone()))?;
            case_set = load_diagnostics_cases(
                &data,
                builtin_cases.as_ref(),
                &options.corpus_root,
                options.mode,
                options.filter.as_deref(),
                options.limit,
            );
            oracle = match options.oracle {
                OracleKind::Diagnostics => Oracle::Diagnostics {
                    data,
                    mode: options.mode,
                },
                OracleKind::Javac => Oracle::from_javac(javac_tests::Javac::find_jdk17())?,
                OracleKind::Baselines => {
                    return Err(DiffError::InvalidConfiguration(
                        "--source diagnostics cannot use --oracle baselines".to_string(),
                    ))
                }
            };
        }
        SourceKind::Corpus => {
            let corpus = Corpus::scan(&options.corpus_root)?;
            case_set = load_corpus_cases(&corpus, options.filter.as_deref(), options.limit)?;
            oracle = match options.oracle {
                OracleKind::Baselines => Oracle::from_baselines(&options.baselines)?,
                OracleKind::Javac => Oracle::from_javac(javac_tests::Javac::find_jdk17())?,
                OracleKind::Diagnostics => {
                    return Err(DiffError::InvalidConfiguration(
                        "--source corpus cannot use --oracle diagnostics".to_string(),
                    ))
                }
            };
        }
    }
    load_skips.append(&mut case_set.skipped);
    if case_set.cases.is_empty() {
        return Err(DiffError::InvalidConfiguration(format!(
            "no cases selected (source: {source:?}, filter: {:?}); nothing to compare",
            options.filter
        )));
    }

    if options.list {
        println!(
            "source: {:?} ({} cases, {} skipped)",
            source,
            case_set.cases.len(),
            load_skips.len()
        );
        for case in &case_set.cases {
            println!("CASE {}", case.name);
        }
        for (name, reason) in &load_skips {
            println!("SKIP {name}: {reason}");
        }
        return Ok(0);
    }

    let report = run(&target, &case_set.cases, &oracle)?;
    let summary = report.format_summary();
    let mut merged = RunReport::default();
    merged.add_skips(load_skips);
    let exit_code = report.exit_code();
    println!(
        "source: {:?} | oracle: {:?} | mode: {:?}",
        source, options.oracle, options.mode
    );
    println!("{summary}");
    let mut details = report.format_details(options.max_details);
    for result in &merged.results {
        if let javac_tests::diff::Outcome::Skipped(reason) = &result.outcome {
            details.push_str(&format!("SKIP {}: {}\n", result.name, reason));
        }
    }
    if details == "no failures or skips\n" || details.is_empty() {
        println!("no failures");
    } else {
        print!("{details}");
    }
    if exit_code == 0 {
        println!("result: PASS");
    } else {
        println!("result: FAIL");
    }
    Ok(exit_code as u8)
}

fn parse_options(mut arguments: impl Iterator<Item = String>) -> Result<Options, DiffError> {
    let mut options = Options::default();
    while let Some(argument) = arguments.next() {
        let mut value = || {
            arguments
                .next()
                .ok_or_else(|| DiffError::InvalidConfiguration(format!("{argument} needs a value")))
        };
        match argument.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            "--target" => options.target = Some(PathBuf::from(value()?)),
            "--target-arg" => options.target_args.push(value()?),
            "--source" => {
                options.source = Some(match value()?.as_str() {
                    "diagnostics" => SourceKind::Diagnostics,
                    "corpus" => SourceKind::Corpus,
                    other => {
                        return Err(DiffError::InvalidConfiguration(format!(
                            "unknown --source {other} (expected diagnostics or corpus)"
                        )));
                    }
                });
            }
            "--oracle" => {
                options.oracle = match value()?.as_str() {
                    "diagnostics" => OracleKind::Diagnostics,
                    "baselines" => OracleKind::Baselines,
                    "javac" => OracleKind::Javac,
                    other => {
                        return Err(DiffError::InvalidConfiguration(format!(
                            "unknown --oracle {other} (expected diagnostics, baselines or javac)"
                        )));
                    }
                };
            }
            "--diagnostics" => options.diagnostics = PathBuf::from(value()?),
            "--builtin-cases" => options.builtin_cases = PathBuf::from(value()?),
            "--baselines" => options.baselines = PathBuf::from(value()?),
            "--corpus-root" => options.corpus_root = PathBuf::from(value()?),
            "--mode" => {
                options.mode = match value()?.as_str() {
                    "default" => Mode::Default,
                    "draw" => Mode::Draw,
                    other => {
                        return Err(DiffError::InvalidConfiguration(format!(
                            "unknown --mode {other} (expected default or draw)"
                        )));
                    }
                };
            }
            "--filter" => options.filter = Some(value()?),
            "--limit" => options.limit = Some(parse_usize("--limit", &value()?)?),
            "--max-details" => options.max_details = parse_usize("--max-details", &value()?)?,
            "--list" => options.list = true,
            other => {
                return Err(DiffError::InvalidConfiguration(format!(
                    "unknown argument {other}\n{USAGE}"
                )));
            }
        }
    }
    Ok(options)
}

fn parse_usize(field: &str, value: &str) -> Result<usize, DiffError> {
    value.parse::<usize>().map_err(|_| {
        DiffError::InvalidConfiguration(format!("{field} expects a number, got {value:?}"))
    })
}
