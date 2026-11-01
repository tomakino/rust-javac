//! Diagnostic example parity harness.
//!
//! Runs every top-level entry of the official javac diagnostic example corpus
//! (reference/jdk17u/test/langtools/tools/javac/diags/examples) through
//! rust-javac and compares stdout+stderr with the frozen expected text under
//! diag-corpus/expected/<name>.txt.
//!
//! The expected texts were produced by the official Example driver with
//! -Duser.language=en; every absolute example path was replaced by <src>/.
//! This harness mirrors that setup:
//!
//! * options come from the "// options:" header lines (split on spaces, the
//!   jdk.version placeholder is replaced with 17; the last parsed file wins,
//!   like Example.parse);
//! * the command line is [-J-Duser.language=en -J-Duser.country=US]
//!   -d <temp> [--class-path <temp>] <options> [-sourcepath <dir>] <sources>;
//! * examples that need annotation processing, module paths, -verbose, custom
//!   file managers or plugins are reported as SKIP (unsupported execution
//!   modes, see the task statement);
//! * the combined output is normalized (\r\n -> \n, example directory ->
//!   <src>) and compared line by line with trailing spaces ignored;
//! * a compiler run that does not finish within DIAG_EXAMPLES_TIMEOUT_SECS
//!   (default 20) is killed and reported as a timeout failure.
//!
//! The test is informational: it prints total/pass/fail/skip plus the failing
//! example names so that cargo test stays green while the corpus drives fixes.
//! No diff files are written.
//!
//! Optional environment variables:
//! * DIAG_EXAMPLES_FILTER      - only run examples whose name contains it;
//! * DIAG_EXAMPLES_VERBOSE     - print the first differing line per failure;
//! * DIAG_EXAMPLES_TIMEOUT_SECS - per-compilation timeout (default 20).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

fn rust_cli() -> &'static str {
    env!("CARGO_BIN_EXE_rust-javac")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("canonicalize repo root")
}

fn examples_dir() -> PathBuf {
    // Extracted from internal/reference/jdk17u so the diagnostic suite no
    // longer depends on the (large, git-ignored) JDK source tree.
    repo_root()
        .join("internal")
        .join("test")
        .join("diag-corpus")
        .join("examples")
}

fn expected_dir() -> PathBuf {
    repo_root()
        .join("internal")
        .join("test")
        .join("diag-corpus")
        .join("expected")
}

fn timeout() -> Duration {
    let secs = std::env::var("DIAG_EXAMPLES_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(20);
    Duration::from_secs(secs)
}

fn is_java(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "java")
}

/// Java: Example.findFiles - collect all .java files below a directory.
fn find_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if dir.is_dir() {
        let mut children: Vec<PathBuf> = std::fs::read_dir(dir)
            .expect("read example dir")
            .flatten()
            .map(|entry| entry.path())
            .collect();
        children.sort();
        for child in children {
            find_files(&child, out);
        }
    } else if dir.is_file() && is_java(dir) {
        out.push(dir.to_path_buf());
    }
}

/// Files of one example, classified like the official Example driver:
/// processors/sourcepath/modulesourcepath/patchmodule/modulepath/classpath
/// subdirectories are not part of the main compilation.
#[derive(Default)]
struct ExampleFiles {
    src: Vec<PathBuf>,
    proc: Vec<PathBuf>,
    src_path: Vec<PathBuf>,
    src_path_dir: Option<PathBuf>,
    module_source_path: Vec<PathBuf>,
    module_source_path_dir: Option<PathBuf>,
    patch_module: Vec<PathBuf>,
    patch_module_dir: Option<PathBuf>,
    module_path: Vec<PathBuf>,
    class_path: Vec<PathBuf>,
    additional: Vec<PathBuf>,
}

fn collect_dir(dir: &Path, files: &mut ExampleFiles) {
    let mut children: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read example dir")
        .flatten()
        .map(|entry| entry.path())
        .collect();
    children.sort();
    for child in children {
        let name = child.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if child.is_dir() {
            match name {
                "processors" => find_files(&child, &mut files.proc),
                "sourcepath" => {
                    files.src_path_dir = Some(child.clone());
                    find_files(&child, &mut files.src_path);
                }
                "modulesourcepath" => {
                    files.module_source_path_dir = Some(child.clone());
                    find_files(&child, &mut files.module_source_path);
                }
                "patchmodule" => {
                    files.patch_module_dir = Some(child.clone());
                    find_files(&child, &mut files.patch_module);
                }
                "additional" => find_files(&child, &mut files.additional),
                "modulepath" => find_files(&child, &mut files.module_path),
                "classpath" => find_files(&child, &mut files.class_path),
                _ => find_files(&child, &mut files.src),
            }
        } else if is_java(&child) {
            files.src.push(child);
        } else if name == "modulesourcepath" {
            // Java: Example.findFiles accepts a plain file named
            // modulesourcepath as the module source path argument (it is then
            // diagnosed as a bad path element by the compiler).
            files.module_source_path_dir = Some(child.clone());
        }
    }
}

fn source_has_declarations(path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    text.lines().any(java_statement_line)
}

fn java_statement_line(line: &str) -> bool {
    let trimmed = line.trim_start_matches(' ');
    let bytes = trimmed.as_bytes();
    match bytes.first() {
        None => false,
        Some(b'@') => bytes.get(1).is_some_and(|b| b.is_ascii_alphabetic()),
        Some(byte) => byte.is_ascii_alphabetic(),
    }
}

/// Java: Example.parse - scan the header up to the first Java statement for
/// the last "// options:" and "// run:" lines.
fn parse_header(path: &Path) -> (Option<Vec<String>>, Option<Vec<String>>) {
    let text = std::fs::read_to_string(path).expect("read example source");
    let mut options = None;
    let mut run = None;
    for line in text.split('\n') {
        let line = line.trim_end_matches('\r');
        if let Some(rest) = line.trim_start_matches(' ').strip_prefix("//") {
            let rest = rest.trim_start_matches(' ');
            if let Some(value) = rest.strip_prefix("options:") {
                options = Some(split_words(value));
            } else if let Some(value) = rest.strip_prefix("run:") {
                run = Some(split_words(value));
            }
        }
        if java_statement_line(line) {
            break;
        }
    }
    (options, run)
}

fn split_words(value: &str) -> Vec<String> {
    let placeholder = concat!("$", "{jdk.version}");
    value
        .trim()
        .split(' ')
        .filter(|word| !word.is_empty())
        .map(|word| word.replace(placeholder, "17"))
        .collect()
}

struct Example {
    name: String,
    /// True when the example is a directory (its expected file is
    /// `<name>.dir.txt`, because `<name>.txt` belongs to a same-named file).
    is_dir: bool,
    files: ExampleFiles,
    options: Vec<String>,
    run: Vec<String>,
}

fn collect_examples() -> Vec<Example> {
    let dir = examples_dir();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("read examples dir")
        .flatten()
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    let mut examples = Vec::new();
    for entry in entries {
        let file_name = entry
            .file_name()
            .and_then(|n| n.to_str())
            .expect("example name");
        let name = file_name
            .strip_suffix(".java")
            .unwrap_or(file_name)
            .to_string();
        let mut files = ExampleFiles::default();
        let is_dir = entry.is_dir();
        if is_dir {
            collect_dir(&entry, &mut files);
        } else if is_java(&entry) {
            files.src.push(entry.clone());
        } else {
            continue;
        }
        let mut options = None;
        let mut run = None;
        for source in &files.src {
            let (opt, run_opts) = parse_header(source);
            if opt.is_some() {
                options = opt;
            }
            if run_opts.is_some() {
                run = run_opts;
            }
        }
        examples.push(Example {
            name,
            is_dir,
            files,
            options: options.unwrap_or_default(),
            run: run.unwrap_or_default(),
        });
    }
    examples
}

fn skip_reason(example: &Example) -> Option<&'static str> {
    let options = &example.options;
    let run = &example.run;
    let files = &example.files;
    if !files.proc.is_empty()
        || options.iter().any(|option| {
            matches!(
                option.as_str(),
                "-processor" | "-proc:only" | "-processorpath" | "--processor-module-path"
            )
        })
    {
        return Some("annotation-processing");
    }
    if options.iter().any(|option| option == "-verbose") {
        return Some("verbose");
    }
    if options.iter().any(|option| option == "-doe") {
        // -doe (dump-on-error) makes the reference javac print the JVM stack
        // trace of Log.writeDiagnostic; the port has no JVM, so that output is
        // not reproducible (only UnsupportedEncoding uses -doe).
        return Some("jvm-stacktrace");
    }
    if options.iter().any(|option| option == "-profile") {
        // -profile is the deprecated JDK 8 profile mechanism (valid only for
        // --release 8); the deprecated mechanism stays out of scope instead of
        // carrying legacy debt.
        return Some("deprecated-profile");
    }
    if let Some(index) = options.iter().position(|option| option == "-source")
        && options
            .get(index + 1)
            .is_some_and(|value| value == "7")
    {
        // -source 7 selects the legacy inference diagnostic path; deprecated
        // mechanisms stay out of scope.
        return Some("deprecated-source");
    }
    // Automatic-module examples require a jar on the module path (the
    // official Example driver packages the compiled classes with
    // JarOutputStream, Example.java:233-269); they stay out of scope here.
    if !files.module_path.is_empty()
        && !files
            .module_path
            .iter()
            .any(|path| path.file_name().is_some_and(|name| name == "module-info.java"))
    {
        return Some("modules-automatic");
    }
    if run.iter().any(|option| option == "-filemanager") {
        return Some("custom-file-manager");
    }
    if options
        .iter()
        .any(|option| option == "-Xplugin" || option.starts_with("-Xplugin:"))
    {
        return Some("plugin");
    }
    None
}

fn normalize(text: &str, dir: &Path) -> String {
    let windows = dir.to_string_lossy().to_string();
    let unix = windows.replace('\\', "/");
    let mut text = text.replace("\r\n", "\n");
    text = text.replace(&format!("{windows}\\"), "<src>/");
    text = text.replace(&format!("{unix}/"), "<src>/");
    text = text.replace(&windows, "<src>");
    text = text.replace(&unix, "<src>");
    normalize_temp_markers(&text)
}

/// Replaces the per-run temporary directory markers (`expected-gen-<n>` in the
/// frozen expected text, `diag-examples-...` in our runs) with `<tmp>` so that
/// environment-specific paths compare equal. The message format is unchanged;
/// only the random directory name is normalized.
fn normalize_temp_markers(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let mut best: Option<(usize, &str)> = None;
        for marker in ["expected-gen-", "diag-examples-"] {
            if let Some(pos) = rest.find(marker)
                && best.map_or(true, |(best_pos, _)| pos < best_pos)
            {
                best = Some((pos, marker));
            }
        }
        let Some((pos, marker)) = best else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..pos]);
        out.push_str("<tmp>");
        rest = &rest[pos + marker.len()..];
        let end = rest
            .find(|c: char| c == '\\' || c == '/')
            .unwrap_or(rest.len());
        rest = &rest[end..];
    }
    out
}

fn normalized_lines(text: &str, dir: &Path) -> Vec<String> {
    normalize(text, dir)
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect()
}

struct RunResult {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    success: bool,
    timed_out: bool,
}

fn run_compiler(args: &[String], cwd: &Path) -> RunResult {
    let mut child = Command::new(rust_cli())
        .args(["-J-Duser.language=en", "-J-Duser.country=US"])
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rust-javac");
    let mut stdout = child.stdout.take().expect("stdout");
    let mut stderr = child.stderr.take().expect("stderr");
    let stdout_reader = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = stdout.read_to_end(&mut buffer);
        buffer
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = stderr.read_to_end(&mut buffer);
        buffer
    });
    let deadline = Instant::now() + timeout();
    let mut timed_out = false;
    let mut success = false;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                success = status.success();
                break;
            }
            Ok(None) => {}
            Err(_) => break,
        }
        if Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            let _ = child.wait();
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();
    RunResult {
        stdout,
        stderr,
        success,
        timed_out,
    }
}

fn first_diff(actual: &[String], expected: &[String]) -> String {
    for (index, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        if a != e {
            return format!(
                "line {}: rust={:?} expected={:?}",
                index + 1,
                truncate(a),
                truncate(e)
            );
        }
    }
    format!(
        "line count: rust={} expected={}",
        actual.len(),
        expected.len()
    )
}

fn truncate(line: &str) -> String {
    const LIMIT: usize = 240;
    if line.len() <= LIMIT {
        return line.to_string();
    }
    let mut end = LIMIT;
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &line[..end])
}

enum FailureKind {
    Pass,
    Fail,
    Skip(&'static str),
}

struct Outcome {
    name: String,
    kind: FailureKind,
    detail: String,
}

fn run_example(example: &Example, index: usize) -> Outcome {
    let name = example.name.clone();
    if let Some(reason) = skip_reason(example) {
        return Outcome {
            name,
            kind: FailureKind::Skip(reason),
            detail: String::new(),
        };
    }
    let temp = std::env::temp_dir().join(format!(
        "diag-examples-{index}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).expect("temp dir");
    let classes = temp.join("classes");
    std::fs::create_dir_all(&classes).expect("classes dir");

    let mut args: Vec<String> = Vec::new();
    let mut prefix_errors = String::new();

    // Java: Example.run - classpath sources are compiled first, then passed as
    // --class-path to the main compilation.
    if !example.files.class_path.is_empty() {
        let classpath_dir = temp.join("classpath");
        std::fs::create_dir_all(&classpath_dir).expect("classpath dir");
        let mut cp_args = vec!["-d".to_string(), classpath_dir.to_string_lossy().to_string()];
        cp_args.extend(
            example
                .files
                .class_path
                .iter()
                .map(|path| path.to_string_lossy().to_string()),
        );
        let result = run_compiler(&cp_args, &temp);
        if !result.success {
            prefix_errors.push_str(&String::from_utf8_lossy(&result.stderr));
        }
        args.push("--class-path".to_string());
        args.push(classpath_dir.to_string_lossy().to_string());
    }

    args.push("-d".to_string());
    args.push(classes.to_string_lossy().to_string());
    args.extend(example.options.iter().cloned());

    // Java: Example.run - additional files are compiled before the main pass.
    if !example.files.additional.is_empty() {
        let mut extra = vec!["-d".to_string(), classes.to_string_lossy().to_string()];
        extra.extend(
            example
                .files
                .additional
                .iter()
                .map(|path| path.to_string_lossy().to_string()),
        );
        let _ = run_compiler(&extra, &temp);
    }

    // Java: Example.run (Example.java:219-272): compile the modulepath sources
    // into an exploded module directory first, then pass that directory as
    // --module-path to the main compilation.
    if !example.files.module_path.is_empty() {
        let module_root = example
            .files
            .module_path
            .iter()
            .find_map(|path| {
                path.ancestors()
                    .find(|ancestor| {
                        ancestor.file_name().and_then(|name| name.to_str()) == Some("modulepath")
                    })
                    .map(std::path::Path::to_path_buf)
            })
            .expect("modulepath directory");
        let module_dir = temp.join("modulepath");
        std::fs::create_dir_all(&module_dir).expect("module dir");
        let mut module_args = vec![
            "-d".to_string(),
            module_dir.to_string_lossy().to_string(),
            "--module-source-path".to_string(),
            module_root.to_string_lossy().to_string(),
        ];
        module_args.extend(
            example
                .files
                .module_path
                .iter()
                .map(|path| path.to_string_lossy().to_string()),
        );
        // The driver compiles the module sources with its own compiler and
        // discards that output; the diagnostics come from the main pass.
        let _ = run_compiler(&module_args, &temp);
        args.push("--module-path".to_string());
        args.push(module_dir.to_string_lossy().to_string());
    }

    if let Some(source_path) = &example.files.src_path_dir {
        args.push("-sourcepath".to_string());
        args.push(source_path.to_string_lossy().to_string());
    }

    // Java: Example.run (Example.java:325-331): --module-source-path plus the
    // module source files and the non-empty top-level sources.
    if let Some(module_source_path) = &example.files.module_source_path_dir {
        args.push("--module-source-path".to_string());
        args.push(module_source_path.to_string_lossy().to_string());
    }

    // Java: Example.run (Example.java:333-341).
    if let Some(patch_module_dir) = &example.files.patch_module_dir {
        let mut patches: Vec<PathBuf> = std::fs::read_dir(patch_module_dir)
            .expect("patch module dir")
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        patches.sort();
        for patch in patches {
            let name = patch
                .file_name()
                .and_then(|name| name.to_str())
                .expect("patch module name");
            args.push("--patch-module".to_string());
            args.push(format!("{}={}", name, patch.to_string_lossy()));
        }
    }

    let sources: Vec<&PathBuf> = if example.files.module_source_path_dir.is_some() {
        example
            .files
            .module_source_path
            .iter()
            .chain(
                example
                    .files
                    .src
                    .iter()
                    .filter(|path| source_has_declarations(path)),
            )
            .collect()
    } else if example.files.patch_module_dir.is_some() {
        example
            .files
            .patch_module
            .iter()
            .chain(
                example
                    .files
                    .src
                    .iter()
                    .filter(|path| source_has_declarations(path)),
            )
            .collect()
    } else {
        example.files.src.iter().collect()
    };
    args.extend(
        sources
            .into_iter()
            .map(|path| path.to_string_lossy().to_string()),
    );

    let result = run_compiler(&args, &temp);
    let mut combined = String::new();
    combined.push_str(&String::from_utf8_lossy(&result.stdout));
    combined.push_str(&String::from_utf8_lossy(&result.stderr));
    combined.push_str(&prefix_errors);

    let dir_expected = expected_dir().join(format!("{name}.dir.txt"));
    let expected_name = if example.is_dir && dir_expected.exists() {
        format!("{name}.dir.txt")
    } else {
        format!("{name}.txt")
    };
    let expected_path = expected_dir().join(expected_name);
    let expected_text = match std::fs::read_to_string(&expected_path) {
        Ok(text) => text,
        Err(_) => {
            let _ = std::fs::remove_dir_all(&temp);
            return Outcome {
                name,
                kind: FailureKind::Fail,
                detail: format!("missing expected file {}", expected_path.display()),
            };
        }
    };

    let dir = examples_dir();
    let actual_lines = normalized_lines(&combined, &dir);
    let expected_lines = normalized_lines(&expected_text, &dir);
    let passed = !result.timed_out && actual_lines == expected_lines;
    let detail = if result.timed_out {
        format!("timeout after {}s", timeout().as_secs())
    } else if passed {
        String::new()
    } else {
        first_diff(&actual_lines, &expected_lines)
    };
    let _ = std::fs::remove_dir_all(&temp);
    Outcome {
        name,
        kind: if passed {
            FailureKind::Pass
        } else {
            FailureKind::Fail
        },
        detail,
    }
}

#[test]
fn official_diagnostic_examples() {
    let examples = examples_dir();
    let expected = expected_dir();
    if !examples.is_dir() || !expected.is_dir() {
        println!(
            "DIAG_EXAMPLES skipped: fixtures not found (examples={}, expected={}); see README (internal/ setup)",
            examples.display(),
            expected.display()
        );
        return;
    }
    let filter = std::env::var("DIAG_EXAMPLES_FILTER").ok();
    let verbose = std::env::var_os("DIAG_EXAMPLES_VERBOSE").is_some();
    let examples = collect_examples();
    let selected: Vec<&Example> = examples
        .iter()
        .filter(|example| {
            filter
                .as_deref()
                .map_or(true, |value| example.name.contains(value))
        })
        .collect();
    println!(
        "DIAG_EXAMPLES corpus={} selected={}",
        examples.len(),
        selected.len()
    );

    let next = AtomicUsize::new(0);
    let outcomes = Mutex::new(Vec::new());
    let workers = std::thread::available_parallelism()
        .map(|count| count.get().min(8))
        .unwrap_or(4);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(example) = selected.get(index) else {
                    break;
                };
                let outcome = run_example(example, index);
                outcomes.lock().unwrap().push(outcome);
            });
        }
    });

    let mut outcomes = outcomes.into_inner().unwrap();
    outcomes.sort_by(|a, b| a.name.cmp(&b.name));
    let mut fail = 0usize;
    let mut skip = 0usize;
    let mut skip_reasons: Vec<(&'static str, usize)> = Vec::new();
    for outcome in &outcomes {
        match outcome.kind {
            FailureKind::Pass => {}
            FailureKind::Fail => {
                fail += 1;
                if verbose {
                    println!("FAIL {}: {}", outcome.name, outcome.detail);
                }
            }
            FailureKind::Skip(reason) => {
                skip += 1;
                match skip_reasons.iter_mut().find(|(name, _)| *name == reason) {
                    Some((_, count)) => *count += 1,
                    None => skip_reasons.push((reason, 1)),
                }
                if verbose {
                    println!("SKIP {} ({reason})", outcome.name);
                }
            }
        }
    }
    let pass = outcomes.len() - fail - skip;
    println!(
        "DIAG_EXAMPLES total={} pass={pass} fail={fail} skip={skip}",
        outcomes.len()
    );
    if !skip_reasons.is_empty() {
        skip_reasons.sort();
        let summary: Vec<String> = skip_reasons
            .iter()
            .map(|(reason, count)| format!("{reason}={count}"))
            .collect();
        println!("DIAG_EXAMPLES skip reasons: {}", summary.join(", "));
    }
    for outcome in &outcomes {
        if matches!(outcome.kind, FailureKind::Fail) {
            println!("DIAG_EXAMPLES FAIL {}", outcome.name);
            if verbose {
                println!("  {}", outcome.detail);
            }
        }
    }
}
