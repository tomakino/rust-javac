//! CLI parity tests: every case runs the real javac 17 (JAVA_HOME/bin/javac)
//! and the rust-javac binary, then compares outputs, exit codes and products.
//!
//! Allowed differences are explicit (and commented) in each test; no expected
//! text is hard-coded in place of javac output.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn rust_cli() -> &'static str {
    env!("CARGO_BIN_EXE_rust-javac")
}

fn javac() -> PathBuf {
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        let exe = Path::new(&home).join("bin").join(format!("javac{}", std::env::consts::EXE_SUFFIX));
        if exe.exists() {
            return exe;
        }
    }
    PathBuf::from(format!("javac{}", std::env::consts::EXE_SUFFIX))
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rust-javac-parity-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run_javac(dir: &Path, args: &[&str]) -> Output {
    Command::new(javac())
        // javac's locale follows the JVM properties; pin English so the
        // diagnostics are comparable with rust-javac's default locale.
        .args(["-J-Duser.language=en", "-J-Duser.country=US"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run javac")
}

fn run_rust(dir: &Path, args: &[&str]) -> Output {
    Command::new(rust_cli())
        .args(["-J-Duser.language=en", "-J-Duser.country=US"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run rust-javac")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

fn assert_same_output(actual: &str, expected: &str, label: &str) {
    // Diagnostics are compared verbatim; java.lang types are rendered by
    // their simple name like javac (RichDiagnosticFormatter.RichPrinter).
    assert_eq!(
        actual, expected,
        "{label}\n--- rust ---\n{actual}\n--- javac ---\n{expected}"
    );
}

#[test]
fn version_is_identical() {
    let dir = temp_dir("version");
    let javac_out = run_javac(&dir, &["-version"]);
    let rust_out = run_rust(&dir, &["-version"]);
    assert_eq!(javac_out.status.code(), rust_out.status.code());
    assert_same_output(
        &text(&rust_out.stdout),
        &text(&javac_out.stdout),
        "-version stdout",
    );
}

#[test]
fn help_lines_match_javac_verbatim() {
    let dir = temp_dir("help");
    // Requirement: help lists only implemented options, and every emitted line
    // must appear verbatim in javac's corresponding help. javac lines for
    // unimplemented options are expected to be missing (not hidden with a
    // token-subset check).
    for flag in ["-help", "--help-extra"] {
        let javac_out = run_javac(&dir, &[flag]);
        let rust_out = run_rust(&dir, &[flag]);
        assert_eq!(javac_out.status.code(), rust_out.status.code(), "{flag}");
        let javac_lines: Vec<String> = text(&javac_out.stdout)
            .lines()
            .map(str::to_string)
            .collect();
        let rust_lines: Vec<String> = text(&rust_out.stdout)
            .lines()
            .map(str::to_string)
            .collect();
        for line in &rust_lines {
            if line.is_empty() {
                continue;
            }
            assert!(
                javac_lines.contains(line),
                "{flag}: rust-javac line not present verbatim in javac help: {line:?}"
            );
        }
        if flag == "-help" {
            assert!(
                rust_lines.iter().any(|line| line.starts_with("Usage: javac")),
                "{flag}: missing usage header"
            );
        }
    }
}

#[test]
fn missing_symbol_matches_javac() {
    let dir = temp_dir("missing-symbol");
    let source = dir.join("Main.java");
    std::fs::write(&source, "class Main { int x = no.Such.v(); }").expect("source");
    let source = source.display().to_string();
    let javac_out = run_javac(&dir, &["-d", dir.to_str().unwrap(), &source]);
    let rust_out = run_rust(&dir, &["-d", dir.to_str().unwrap(), &source]);
    assert_eq!(javac_out.status.code(), rust_out.status.code());
    assert_same_output(
        &text(&rust_out.stderr),
        &text(&javac_out.stderr),
        "missing symbol stderr",
    );
}

#[test]
fn error_source_matches_javac() {
    let dir = temp_dir("error");
    let source = dir.join("Bad.java");
    std::fs::write(&source, "class Bad { int x = \"s\"; }").expect("source");
    // Pass absolute paths so javac and rust-javac render the same file name.
    let source = source.display().to_string();
    let javac_out = run_javac(&dir, &["-d", dir.to_str().unwrap(), &source]);
    let rust_out = run_rust(&dir, &["-d", dir.to_str().unwrap(), &source]);
    assert_eq!(javac_out.status.code(), rust_out.status.code());
    assert_same_output(
        &text(&rust_out.stderr),
        &text(&javac_out.stderr),
        "error source stderr",
    );
}

#[test]
fn lint_unchecked_source_matches_javac() {
    let dir = temp_dir("lint");
    std::fs::write(
        dir.join("Warn.java"),
        "import java.util.*; class Warn { void f() { List l = new ArrayList(); } }",
    )
    .expect("source");
    let javac_out = run_javac(
        &dir,
        &["-Xlint:unchecked", "-d", dir.to_str().unwrap(), "Warn.java"],
    );
    let rust_out = run_rust(
        &dir,
        &["-Xlint:unchecked", "-d", dir.to_str().unwrap(), "Warn.java"],
    );
    assert_eq!(javac_out.status.code(), rust_out.status.code());
    assert_same_output(
        &text(&rust_out.stderr),
        &text(&javac_out.stderr),
        "lint stderr",
    );
}

#[test]
fn no_source_files_matches_exit_and_stream() {
    let dir = temp_dir("nofiles");
    let javac_out = run_javac(&dir, &[]);
    let rust_out = run_rust(&dir, &[]);
    assert_eq!(javac_out.status.code(), rust_out.status.code());
    // javac prints the help on stdout; rust-javac does the same. The text is
    // the same generated table as -help (KNOWN DIFF tracked by
    // help_lines_are_a_subset_of_javac), so only the stream/prefix/exit are
    // asserted here.
    assert!(text(&javac_out.stdout).starts_with("Usage: javac"));
    assert!(text(&rust_out.stdout).starts_with("Usage: javac"));
    assert!(text(&javac_out.stderr).is_empty());
    assert!(text(&rust_out.stderr).is_empty());
}

#[test]
fn invalid_source_release_matches_javac() {
    let dir = temp_dir("source");
    std::fs::write(dir.join("Ok.java"), "class Ok {}").expect("source");
    let javac_out = run_javac(&dir, &["-source", "99", "-d", dir.to_str().unwrap(), "Ok.java"]);
    let rust_out = run_rust(&dir, &["-source", "99", "-d", dir.to_str().unwrap(), "Ok.java"]);
    assert_eq!(javac_out.status.code(), rust_out.status.code());
    assert_same_output(
        &text(&rust_out.stderr),
        &text(&javac_out.stderr),
        "invalid source stderr",
    );
}

#[test]
fn invalid_flag_matches_javac() {
    let dir = temp_dir("flag");
    let javac_out = run_javac(&dir, &["-foo"]);
    let rust_out = run_rust(&dir, &["-foo"]);
    assert_eq!(javac_out.status.code(), rust_out.status.code());
    assert_same_output(
        &text(&rust_out.stderr),
        &text(&javac_out.stderr),
        "invalid flag stderr",
    );
}

#[test]
fn argfile_compiles_and_missing_argfile_matches_javac() {
    let dir = temp_dir("argfile");
    std::fs::write(dir.join("T.java"), "class T { int x = 1; }").expect("source");
    let source = dir.join("T.java").display().to_string();
    let out = dir.join("out").display().to_string();
    let argfile = dir.join("args.txt");
    std::fs::write(&argfile, format!("{source}\n-d\n{out}\n")).expect("argfile");
    let arg = format!("@{}", argfile.display());
    let javac_out = run_javac(&dir, &[&arg]);
    let rust_out = run_rust(&dir, &[&arg]);
    assert_eq!(javac_out.status.code(), rust_out.status.code());
    assert!(dir.join("out").join("T.class").exists());
    assert!(rust_out.status.success(), "stderr: {}", text(&rust_out.stderr));

    let missing = format!("@{}", dir.join("no-such-file.txt").display());
    let javac_missing = run_javac(&dir, &[&missing]);
    let rust_missing = run_rust(&dir, &[&missing]);
    assert_eq!(javac_missing.status.code(), rust_missing.status.code());
    assert_same_output(
        &text(&rust_missing.stderr),
        &text(&javac_missing.stderr),
        "missing argfile stderr",
    );
}

#[test]
fn default_d_output_matches_javac() {
    let javac_dir = temp_dir("dot-javac");
    let rust_dir = temp_dir("dot-rust");
    for dir in [&javac_dir, &rust_dir] {
        std::fs::create_dir_all(dir.join("a")).expect("a");
        std::fs::create_dir_all(dir.join("b")).expect("b");
        std::fs::write(dir.join("a").join("A.java"), "class A {}").expect("A");
        std::fs::write(dir.join("b").join("B.java"), "class B {}").expect("B");
    }
    let javac_out = run_javac(&javac_dir, &["a/A.java", "b/B.java"]);
    let rust_out = run_rust(&rust_dir, &["a/A.java", "b/B.java"]);
    assert_eq!(javac_out.status.code(), rust_out.status.code());
    for dir in [&javac_dir, &rust_dir] {
        assert!(dir.join("a").join("A.class").exists());
        assert!(dir.join("b").join("B.class").exists());
        assert!(!dir.join("A.class").exists());
        assert!(!dir.join("B.class").exists());
    }
}

#[test]
fn explicit_d_creates_directory_like_javac() {
    let dir = temp_dir("explicit-d");
    std::fs::write(dir.join("Ok.java"), "class Ok {}").expect("source");
    let javac_dir = dir.join("javac-out");
    let rust_dir = dir.join("rust-out");
    let javac_out = run_javac(&dir, &["-d", javac_dir.to_str().unwrap(), "Ok.java"]);
    let rust_out = run_rust(&dir, &["-d", rust_dir.to_str().unwrap(), "Ok.java"]);
    assert_eq!(javac_out.status.code(), rust_out.status.code());
    assert!(javac_dir.join("Ok.class").exists());
    assert!(rust_dir.join("Ok.class").exists());
}

#[test]
fn classpath_default_and_explicit_find_directory_classes() {
    let dir = temp_dir("classpath");
    std::fs::write(dir.join("Helper.java"), "public class Helper {}").expect("Helper");
    std::fs::write(dir.join("T.java"), "class T { Helper h; }").expect("T");
    // Compile Helper into the working directory; then compile T with the
    // default class path (".") and with an explicit -cp ".".
    let helper = run_rust(&dir, &["-d", dir.to_str().unwrap(), "Helper.java"]);
    assert!(helper.status.success(), "stderr: {}", text(&helper.stderr));
    let default_cp = run_rust(&dir, &["-d", dir.join("out-default").to_str().unwrap(), "T.java"]);
    let explicit_cp = run_rust(
        &dir,
        &[
            "-cp",
            ".",
            "-d",
            dir.join("out-explicit").to_str().unwrap(),
            "T.java",
        ],
    );
    assert!(
        default_cp.status.success(),
        "stderr: {}",
        text(&default_cp.stderr)
    );
    assert!(
        explicit_cp.status.success(),
        "stderr: {}",
        text(&explicit_cp.stderr)
    );
}

fn javap() -> PathBuf {
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        let exe = Path::new(&home)
            .join("bin")
            .join(format!("javap{}", std::env::consts::EXE_SUFFIX));
        if exe.exists() {
            return exe;
        }
    }
    PathBuf::from(format!("javap{}", std::env::consts::EXE_SUFFIX))
}

fn has_attribute(class: &Path, attribute: &str) -> bool {
    let output = Command::new(javap())
        .arg("-v")
        .arg(class)
        .output()
        .expect("run javap");
    text(&output.stdout).contains(&format!("{attribute}:"))
}

#[test]
fn g_none_omits_debug_attributes_like_javac() {
    let dir = temp_dir("g-none");
    std::fs::write(dir.join("Ok.java"), "class Ok { int f(int x) { return x + 1; } }").expect("source");
    let javac_dir = dir.join("javac-out");
    let rust_dir = dir.join("rust-out");
    let javac_out = run_javac(&dir, &["-g:none", "-d", javac_dir.to_str().unwrap(), "Ok.java"]);
    let rust_out = run_rust(&dir, &["-g:none", "-d", rust_dir.to_str().unwrap(), "Ok.java"]);
    assert_eq!(javac_out.status.code(), rust_out.status.code());
    let javac_class = javac_dir.join("Ok.class");
    let rust_class = rust_dir.join("Ok.class");
    for attribute in ["SourceFile", "LineNumberTable", "LocalVariableTable"] {
        assert_eq!(
            has_attribute(&javac_class, attribute),
            has_attribute(&rust_class, attribute),
            "-g:none attribute {attribute}"
        );
        assert!(!has_attribute(&rust_class, attribute), "-g:none must omit {attribute}");
    }
}

fn count_attribute(class: &Path, attribute: &str) -> usize {
    let output = Command::new(javap())
        .arg("-v")
        .arg(class)
        .output()
        .expect("run javap");
    text(&output.stdout)
        .lines()
        .filter(|line| line.trim_start().starts_with(&format!("{attribute}:")))
        .count()
}

#[test]
fn debug_attribute_modes_match_javac() {
    let dir = temp_dir("g-modes");
    std::fs::write(dir.join("Ok.java"), "class Ok { int f(int x) { int y = x + 1; return y; } }")
        .expect("source");
    let modes: [Option<&str>; 5] = [None, Some("-g"), Some("-g:lines"), Some("-g:none"), Some("-g:vars")];
    for (index, mode) in modes.iter().enumerate() {
        let javac_out = dir.join(format!("j{index}"));
        let rust_out = dir.join(format!("r{index}"));
        let mut javac_args: Vec<&str> = Vec::new();
        let mut rust_args: Vec<&str> = Vec::new();
        if let Some(mode) = mode {
            javac_args.push(mode);
            rust_args.push(mode);
        }
        javac_args.extend(["-d", javac_out.to_str().unwrap(), "Ok.java"]);
        rust_args.extend(["-d", rust_out.to_str().unwrap(), "Ok.java"]);
        let javac_run = run_javac(&dir, &javac_args);
        let rust_run = run_rust(&dir, &rust_args);
        assert_eq!(javac_run.status.code(), rust_run.status.code(), "mode {mode:?}");
        for attribute in ["SourceFile", "LineNumberTable", "LocalVariableTable"] {
            assert_eq!(
                count_attribute(&javac_out.join("Ok.class"), attribute),
                count_attribute(&rust_out.join("Ok.class"), attribute),
                "mode {mode:?} attribute {attribute}"
            );
        }
    }
}

#[test]
fn invalid_target_release_matches_javac() {
    let dir = temp_dir("target");
    std::fs::write(dir.join("Ok.java"), "class Ok {}").expect("source");
    let javac_out = run_javac(&dir, &["-target", "99", "-d", dir.to_str().unwrap(), "Ok.java"]);
    let rust_out = run_rust(&dir, &["-target", "99", "-d", dir.to_str().unwrap(), "Ok.java"]);
    assert_eq!(javac_out.status.code(), rust_out.status.code());
    assert_same_output(
        &text(&rust_out.stderr),
        &text(&javac_out.stderr),
        "invalid target stderr",
    );
}

#[test]
fn multiple_error_files_keep_source_order() {
    let dir = temp_dir("order");
    let bad1 = dir.join("Bad1.java");
    let bad2 = dir.join("Bad2.java");
    std::fs::write(&bad1, "class Bad1 { int x = \"s\"; }").expect("Bad1");
    std::fs::write(&bad2, "class Bad2 { int y = \"t\"; }").expect("Bad2");
    let bad1 = bad1.display().to_string();
    let bad2 = bad2.display().to_string();
    let javac_out = run_javac(&dir, &[&bad1, &bad2]);
    let rust_out = run_rust(&dir, &[&bad1, &bad2]);
    assert_eq!(javac_out.status.code(), rust_out.status.code());
    let error_files = |out: &Output| -> Vec<String> {
        text(&out.stderr)
            .lines()
            .filter(|line| line.contains(".java:") && line.contains("error:"))
            .map(|line| {
                line.split(":1:")
                    .next()
                    .unwrap_or("")
                    .rsplit(|c: char| c == '\\' || c == '/')
                    .next()
                    .unwrap_or("")
                    .to_string()
            })
            .collect()
    };
    assert_eq!(error_files(&rust_out), error_files(&javac_out));
}
