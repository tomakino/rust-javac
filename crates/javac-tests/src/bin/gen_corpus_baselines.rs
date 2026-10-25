//! corpus javac oracle 基线生成器(P9 差分基础设施)。
//!
//! 用途锚点:docs/PLAN.md §7(差分测试体系)。本程序是测试基础设施(适配工具),不是
//! javac 源码的移植,不写移植来源锚点、不写版权头。
//!
//! 用参考 javac 17 为 langtools 语料的 compile-only 用例生成基线(退出码/规范化
//! stdout+stderr/class 清单 + 源输入指纹),输出到 target/ 下(不入库);差分运行器
//! (diff_runner --source corpus --oracle baselines)消费这些基线。
//!
//! 用法:
//!     cargo run -p javac-tests --bin gen_corpus_baselines -- [--corpus-root DIR] [--output DIR]
//!         [--filter SUBSTR] [--limit N] [--refresh]
//!
//! 缺省 CORPUS-ROOT = reference/jdk17u/test/langtools/tools/javac,
//! 缺省 OUTPUT = target/javac-corpus(基线写在 <OUTPUT>/baselines/ 下)。
//! 找不到 JDK 17 javac 或语料目录时退出码 2,不写空基线。
//! 退出码:0 完成(允许 unsupported/up-to-date),1 有生成失败用例,2 环境缺失。

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use javac_tests::Javac;
use javac_tests::corpus::{BaselineOptions, BaselineStatus, Corpus, generate_baselines};

const DEFAULT_CORPUS_ROOT: &str = "reference/jdk17u/test/langtools/tools/javac";
const DEFAULT_OUTPUT: &str = "target/javac-corpus";
const MAX_DETAILS: usize = 10;

const USAGE: &str = "usage: gen_corpus_baselines [--corpus-root DIR] [--output DIR] [--filter SUBSTR] [--limit N] [--refresh]

Generates reference-javac baselines for the langtools compile-only corpus cases
(written below <output>/baselines/, not committed).

options:
  --corpus-root DIR   langtools corpus root (default: reference/jdk17u/test/langtools/tools/javac)
  --output DIR        baseline output root (default: target/javac-corpus)
  --filter SUBSTR     only cases whose relative path contains SUBSTR (batching)
  --limit N           process at most N compile-only cases after filtering
  --refresh           regenerate baselines even when the source fingerprint is unchanged
  -h, --help          show this help

exit codes: 0 done, 1 some cases failed, 2 environment missing (no JDK 17/corpus)";

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("gen-corpus-baselines: {error}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<u8, String> {
    let mut corpus_root = PathBuf::from(DEFAULT_CORPUS_ROOT);
    let mut output = PathBuf::from(DEFAULT_OUTPUT);
    let mut filter: Option<String> = None;
    let mut limit: Option<usize> = None;
    let mut refresh = false;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        let mut value = || {
            arguments
                .next()
                .ok_or_else(|| format!("{argument} needs a value"))
        };
        match argument.as_str() {
            "--corpus-root" => corpus_root = PathBuf::from(value()?),
            "--output" => output = PathBuf::from(value()?),
            "--filter" => filter = Some(value()?),
            "--limit" => {
                limit = Some(
                    value()?
                        .parse::<usize>()
                        .map_err(|_| "--limit expects a number".to_string())?,
                );
            }
            "--refresh" => refresh = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(0);
            }
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }

    let javac = Javac::find_jdk17().ok_or_else(|| {
        "no JDK 17 javac found (set JAVA_HOME or JAVAC); refusing to write partial baselines"
            .to_string()
    })?;
    let mut corpus = Corpus::scan(&corpus_root)
        .map_err(|error| format!("cannot scan corpus {}: {error}", corpus_root.display()))?;
    if let Some(filter) = &filter {
        corpus
            .cases
            .retain(|case| case.relative_path.contains(filter));
    }
    let mut options = BaselineOptions::new(&output);
    options.refresh = refresh;
    options.limit = limit;
    let report = generate_baselines(&javac, &corpus, &options)
        .map_err(|error| format!("baseline generation failed: {error}"))?;

    println!("corpus root: {}", corpus.root.display());
    println!("output: {}", output.display());
    println!(
        "cases: generated {}, up-to-date {}, unsupported {}, failed {}",
        report.generated, report.up_to_date, report.unsupported, report.failed
    );
    let mut shown = 0usize;
    for case in &report.cases {
        if shown >= MAX_DETAILS {
            break;
        }
        match &case.status {
            BaselineStatus::Unsupported(reason) => {
                println!("UNSUPPORTED {}: {reason}", case.relative_path);
                shown += 1;
            }
            BaselineStatus::Failed(reason) => {
                println!("FAILED {}: {reason}", case.relative_path);
                shown += 1;
            }
            BaselineStatus::Generated(_) | BaselineStatus::UpToDate => {}
        }
    }
    if report.failed > 0 {
        println!("result: FAIL");
        Ok(1)
    } else {
        println!("result: PASS");
        Ok(0)
    }
}
