//! 诊断 oracle 数据生成器(开发期工具;生成的测试数据在运行期零 JVM 依赖)。
//!
//! 用途锚点:docs/PLAN.md §7(验证与差分体系:oracle = 本机 javac 17.0.19)。
//! 本程序是测试基础设施(适配工具),不是 javac 源码的移植,不写移植来源锚点、不写版权头。
//!
//! 采集内容:对每个用例分别运行两次 javac(默认诊断格式、-XDrawDiagnostics),
//! 记录退出码、规范化后的 stdout/stderr、生成的 class 清单,并提取 draw 模式的
//! 诊断 key 列表;-XDrawDiagnostics 输出同时保留诊断 key、位置(line:col)与参数,
//! 默认格式保留 path:line: error: msg 与定位行/^ 行,供 P1/P4 逐字对照。
//!
//! 用例来源:
//! - builtin:入库的失败用例(crates/javac-tests/data/diagnostics-builtin-cases.txt),
//!   精确控制选项组合(-Xlint、-Werror、-Xmaxerrs、-XDshould-stop.at、--release、多文件等);
//! - corpus:reference/jdk17u 语料中的 test/langtools/tools/javac/diags/examples
//!   (上游按诊断 key 组织的示例集合,见该目录 README.examples.txt);生成器解析示例
//!   文件里的 "// options:" 与 "// key:" 元信息。
//!
//! 用法:
//!     cargo run -p javac-tests --bin gen_diagnostics_oracle -- [OUTPUT] [--corpus-root DIR] [--builtin-cases PATH]
//!
//! 缺省 OUTPUT = crates/javac-tests/data/diagnostics-oracle.txt,
//! 缺省 CORPUS-ROOT = reference/jdk17u/test/langtools/tools/javac/diags/examples,
//! 缺省 BUILTIN-CASES = crates/javac-tests/data/diagnostics-builtin-cases.txt。
//! 找不到 JDK 17 javac 时以退出码 2 失败(不静默生成空数据)。

use std::collections::BTreeSet;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use javac_tests::diagnostics_data::BuiltinCases;
use javac_tests::{Javac, SourceFile, decode_output, normalize_text};

/// 差分基线(与 docs/PLAN.md、其他 oracle 数据一致)。
const BASELINE: &str = "982d53c (jdk-17.0.19-ga)";
const DEFAULT_OUTPUT: &str = "crates/javac-tests/data/diagnostics-oracle.txt";
const DEFAULT_CORPUS_ROOT: &str = "reference/jdk17u/test/langtools/tools/javac/diags/examples";
const DEFAULT_BUILTIN_CASES: &str = "crates/javac-tests/data/diagnostics-builtin-cases.txt";

const USAGE: &str = "usage: cargo run -p javac-tests --bin gen_diagnostics_oracle -- [OUTPUT] [--corpus-root DIR] [--builtin-cases PATH]";

const CORPUS_CASES: &[&str] = &[
    "IllegalStartOfExpr.java",
    "IllegalStartOfStmt.java",
    "IllegalStartOfType.java",
    "MissingSemicolon.java",
    "IdentifierExpected.java",
    "InvalidMethodDecl.java",
    "PrematureEOF.java",
    "UnclosedStringLiteral.java",
    "UnclosedComment.java",
    "IllegalEscapeChar.java",
    "EmptyCharLiteral.java",
    "IntNumberTooLarge.java",
    "NotAStatement.java",
    "ElseWithoutIf.java",
    "CatchWithoutTry.java",
    "IllegalDot.java",
    "CantResolveLocation.java",
    "IncompatibleTypes1.java",
    "OperatorCantBeApplied.java",
    "NonStaticCantBeRef.java",
    "AbstractCantBeInstantiated.java",
    "MissingReturnStatement.java",
    "UnreachableStatement.java",
    "DuplicateClass.java",
    "CyclicInheritance.java",
    "InheritFromFinal.java",
    "CantDeref.java",
    "ArrayRequired.java",
    "CantAssignToFinal.java",
    "UnreportedException.java",
    "PossibleFallThrough.java",
    "FinallyCannotComplete.java",
    "DivZero.java",
    "EmptyIf.java",
    "RedundantCast.java",
    "RawClassUse.java",
    "UncheckedCast.java",
    "MissingSVUID.java",
    "WarningAndWerror.java",
    "Note.java",
    "PreviewFeatureDisabled.java",
    "RecordsCanNotDeclareInstanceFields.java",
    "CantInheritFromSealed.java",
    "SwitchExpressionNoResultExpressions.java",
    "InstanceofPatternNoSubtype.java",
];

struct Captured {
    name: String,
    source: String,
    args: Vec<String>,
    corpus_keys: Vec<String>,
    exit_code: i32,
    draw_exit_code: i32,
    draw_diagnostics: usize,
    draw_keys: Vec<String>,
    classes: Vec<String>,
    stdout_default: String,
    stderr_default: String,
    stdout_draw: String,
    stderr_draw: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("gen-diagnostics-oracle: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut output = PathBuf::from(DEFAULT_OUTPUT);
    let mut corpus_root = PathBuf::from(DEFAULT_CORPUS_ROOT);
    let mut builtin_cases_path = PathBuf::from(DEFAULT_BUILTIN_CASES);
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--corpus-root" => {
                corpus_root = PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| "--corpus-root needs a directory".to_string())?,
                );
            }
            "--builtin-cases" => {
                builtin_cases_path = PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| "--builtin-cases needs a file".to_string())?,
                );
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => output = PathBuf::from(other),
        }
    }

    let javac = Javac::find_jdk17().ok_or_else(|| {
        "no JDK 17 javac found (set JAVA_HOME or JAVAC); refusing to write partial data".to_string()
    })?;
    let version = javac.version().map_err(|error| error.to_string())?;
    let jdk_version = version
        .trim()
        .strip_prefix("javac ")
        .unwrap_or(version.trim())
        .to_string();

    let builtin_cases = BuiltinCases::load(&builtin_cases_path).map_err(|error| {
        format!(
            "cannot load builtin cases {}: {error}",
            builtin_cases_path.display()
        )
    })?;

    let mut cases: Vec<Captured> = Vec::new();
    for builtin in &builtin_cases.cases {
        cases.push(capture(
            &javac,
            &builtin.name,
            format!("builtin:{}", builtin.name),
            &builtin.sources,
            &builtin.args,
            Vec::new(),
        )?);
    }

    let mut corpus_found = 0usize;
    let mut corpus_missing: Vec<String> = Vec::new();
    for file_name in CORPUS_CASES {
        let path = corpus_root.join(file_name);
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(_) => {
                corpus_missing.push((*file_name).to_string());
                continue;
            }
        };
        corpus_found += 1;
        let relative = format!("diags/examples/{file_name}");
        let stem = Path::new(file_name)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| (*file_name).to_string());
        let sources = [SourceFile::new(*file_name, contents.clone())];
        let args = corpus_options(&contents);
        cases.push(capture(
            &javac,
            &format!("corpus/{stem}"),
            format!("corpus:{relative}"),
            &sources,
            &args,
            corpus_keys(&contents),
        )?);
    }

    let mut text = String::new();
    let _ = writeln!(
        text,
        "# diagnostics oracle data (development-time generated; tests never start a JVM)"
    );
    let _ = writeln!(
        text,
        "# generated-by: crates/javac-tests/src/bin/gen_diagnostics_oracle.rs"
    );
    let _ = writeln!(text, "# jdk-version: {jdk_version}");
    let _ = writeln!(text, "# baseline: {BASELINE}");
    let _ = writeln!(
        text,
        "# oracle-env: -encoding UTF-8 -J-Dfile.encoding=UTF-8 -J-Duser.language=en -J-Duser.country=US"
    );
    let _ = writeln!(
        text,
        "# normalization: source/output/temp/JDK paths -> <src>/<out>/<tmp>/<jdk>; CRLF -> LF; '\\' -> '/'"
    );
    let _ = writeln!(
        text,
        "# modes: stderr-default = javac with the given args; stderr-draw = the same args plus -XDrawDiagnostics"
    );
    let _ = writeln!(
        text,
        "# draw-source-names: -XDrawDiagnostics lines use simple source file names; default-format lines keep the normalized <src>/ path"
    );
    let _ = writeln!(text, "# builtin-cases: {}", builtin_cases.cases.len());
    let _ = writeln!(
        text,
        "# builtin-cases-file: {}",
        builtin_cases_path.display()
    );
    let _ = writeln!(text, "# corpus-root: {}", corpus_root.display());
    let _ = writeln!(text, "# corpus-cases: {corpus_found}");
    if !corpus_missing.is_empty() {
        let _ = writeln!(text, "# corpus-missing: {}", corpus_missing.join(","));
    }
    let _ = writeln!(
        text,
        "# command: cargo run -p javac-tests --bin gen_diagnostics_oracle -- {}",
        output.display()
    );
    let _ = writeln!(text, "# total-cases: {}", cases.len());

    let all_keys: BTreeSet<&str> = cases
        .iter()
        .flat_map(|case| case.draw_keys.iter().map(String::as_str))
        .collect();
    let total_diagnostics: usize = cases.iter().map(|case| case.draw_diagnostics).sum();
    let _ = writeln!(text, "# total-draw-diagnostics: {total_diagnostics}");
    let _ = writeln!(text, "# unique-draw-keys: {}", all_keys.len());
    for case in &cases {
        render(case, &mut text);
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(&output, text).map_err(|error| format!("write {}: {error}", output.display()))?;

    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut notes = 0usize;
    for key in &all_keys {
        if key.starts_with("compiler.err.") {
            errors += 1;
        } else if key.starts_with("compiler.warn.") {
            warnings += 1;
        } else if key.starts_with("compiler.note.") {
            notes += 1;
        }
    }
    println!("wrote {}", output.display());
    println!(
        "cases: {} (builtin {}, corpus {}/{}",
        cases.len(),
        builtin_cases.cases.len(),
        corpus_found,
        CORPUS_CASES.len()
    );
    if !corpus_missing.is_empty() {
        println!(
            "corpus files missing (skipped): {}",
            corpus_missing.join(",")
        );
    }
    println!("draw diagnostics: {total_diagnostics}");
    println!(
        "unique draw keys: {} (err {}, warn {}, note {}, other {})",
        all_keys.len(),
        errors,
        warnings,
        notes,
        all_keys.len() - errors - warnings - notes
    );
    Ok(())
}

fn capture(
    javac: &Javac,
    name: &str,
    source: String,
    sources: &[SourceFile],
    args: &[String],
    corpus_keys: Vec<String>,
) -> Result<Captured, String> {
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let default = javac
        .compile(sources, &arg_refs)
        .map_err(|error| format!("{name}: {error}"))?;
    let mut draw_args = arg_refs.clone();
    draw_args.push("-XDrawDiagnostics");
    let draw = javac
        .compile(sources, &draw_args)
        .map_err(|error| format!("{name} (-XDrawDiagnostics): {error}"))?;
    if default.class_files != draw.class_files {
        return Err(format!(
            "{name}: class output differs between the two runs ({:?} vs {:?})",
            default.class_files, draw.class_files
        ));
    }

    let stdout_default = normalize_text(&decode_output(&default.stdout), &default.paths);
    let stderr_default = normalize_text(&decode_output(&default.stderr), &default.paths);
    let stdout_draw = normalize_text(&decode_output(&draw.stdout), &draw.paths);
    let stderr_draw = normalize_text(&decode_output(&draw.stderr), &draw.paths);
    let (draw_diagnostics, draw_keys) = collect_draw_keys(&stderr_draw);

    Ok(Captured {
        name: name.to_string(),
        source,
        args: args.to_vec(),
        corpus_keys,
        exit_code: default.exit_code,
        draw_exit_code: draw.exit_code,
        draw_diagnostics,
        draw_keys,
        classes: default.class_files.clone(),
        stdout_default,
        stderr_default,
        stdout_draw,
        stderr_draw,
    })
}

/// 提取 -XDrawDiagnostics 输出的诊断 key(行首的编译器资源 key,如 compiler.err.cant.resolve.location)。
/// 返回 (诊断条数, 去重排序后的 key 列表)。
fn collect_draw_keys(stderr: &str) -> (usize, Vec<String>) {
    let mut keys = BTreeSet::new();
    let mut count = 0usize;
    for line in stderr.lines() {
        // javac renders diagnostics without a position as "- compiler.key" lines (for example
        // the deprecated/unchecked notes and "warnings found and -Werror specified").
        let key_text = if let Some(rest) = line.strip_prefix("- compiler.") {
            format!("compiler.{rest}")
        } else if let Some((_, rest)) = line.split_once(": compiler.") {
            format!("compiler.{rest}")
        } else if line.starts_with("compiler.") {
            line.to_string()
        } else {
            continue;
        };
        let key = key_text
            .split(':')
            .next()
            .unwrap_or(key_text.as_str())
            .trim();
        keys.insert(key.to_string());
        count += 1;
    }
    (count, keys.into_iter().collect())
}

/// 解析 corpus 示例的 "// options:" 元信息。
fn corpus_options(source: &str) -> Vec<String> {
    let mut options = Vec::new();
    for line in source.lines() {
        if let Some(rest) = line.trim_start().strip_prefix("// options:") {
            options.extend(rest.split_whitespace().map(str::to_string));
        }
    }
    options
}

/// 解析 corpus 示例的 "// key:" 元信息(该示例声明要生成的诊断 key)。
fn corpus_keys(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("// key:"))
        .map(|rest| rest.trim().to_string())
        .filter(|key| !key.is_empty())
        .collect()
}

fn render(case: &Captured, text: &mut String) {
    let _ = writeln!(text, "=== CASE: {} ===", case.name);
    let _ = writeln!(text, "# source: {}", case.source);
    let _ = writeln!(text, "# args: {}", list_or_dash(&case.args, " "));
    if !case.corpus_keys.is_empty() {
        let _ = writeln!(text, "# corpus-keys: {}", case.corpus_keys.join(","));
    }
    let _ = writeln!(text, "# exit-code: {}", case.exit_code);
    let _ = writeln!(text, "# draw-exit-code: {}", case.draw_exit_code);
    let _ = writeln!(text, "# draw-diagnostics: {}", case.draw_diagnostics);
    let _ = writeln!(text, "# draw-keys: {}", list_or_dash(&case.draw_keys, ","));
    let _ = writeln!(text, "# classes: {}", list_or_dash(&case.classes, " "));
    let _ = writeln!(text, "--- stdout-default ---");
    push_lines(text, &case.stdout_default);
    let _ = writeln!(text, "--- stderr-default ---");
    push_lines(text, &case.stderr_default);
    let _ = writeln!(text, "--- stdout-draw ---");
    push_lines(text, &case.stdout_draw);
    let _ = writeln!(text, "--- stderr-draw ---");
    push_lines(text, &case.stderr_draw);
    let _ = writeln!(text, "=== END ===");
}

fn push_lines(text: &mut String, content: &str) {
    for line in content.lines() {
        let _ = writeln!(text, "{line}");
    }
}

fn list_or_dash(items: &[String], separator: &str) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        items.join(separator)
    }
}
