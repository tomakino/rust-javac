//! Diagnostic corpus harness: compares rust-javac against the real javac 17
//! for every case under tests/diagnostic_corpus/cases.
//!
//! Each case directory contains one or more .java files (and optionally
//! args.txt with one compiler argument per line). The harness copies the case
//! into a temp directory, runs javac and rust-javac with identical absolute
//! source paths and arguments, and compares exit code/stdout/stderr after
//! normalizing the temp path.
//!
//! The test is informational (it prints total/pass/fail and the first diff per
//! failure) so that cargo test stays green while the corpus drives fixes.

use std::path::{Path, PathBuf};
use std::process::Command;

fn javac() -> Option<PathBuf> {
    let home = std::env::var_os("JAVA_HOME")?;
    let exe = Path::new(&home)
        .join("bin")
        .join(format!("javac{}", std::env::consts::EXE_SUFFIX));
    if exe.exists() { Some(exe) } else { None }
}

fn rust_cli() -> &'static str {
    env!("CARGO_BIN_EXE_rust-javac")
}

fn normalize(text: &str, dir: &Path) -> String {
    let dir = dir.to_string_lossy();
    text.replace("\r\n", "\n")
        .replace(&*dir, "")
        .replace('\\', "/")
}

/// Count the unique diagnostic message keys (976-message upstream catalogue:
/// err/warn are triggered directly; misc/note appear inside error contexts).
fn message_counts() -> (usize, usize, usize, usize) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("javac-core")
        .join("src")
        .join("resources")
        .join("compiler_properties.rs");
    let text = std::fs::read_to_string(path).expect("read compiler properties");
    let mut keys = std::collections::HashSet::new();
    for line in text.lines() {
        let Some(start) = line.find("(\"compiler.") else {
            continue;
        };
        let rest = &line[start + 2..];
        let Some(end) = rest.find('"') else {
            continue;
        };
        let key = &rest[..end];
        if key.starts_with("compiler.err.")
            || key.starts_with("compiler.warn.")
            || key.starts_with("compiler.misc.")
            || key.starts_with("compiler.note.")
        {
            keys.insert(key.to_string());
        }
    }
    let count = |prefix: &str| keys.iter().filter(|key| key.starts_with(prefix)).count();
    (
        count("compiler.err."),
        count("compiler.warn."),
        count("compiler.misc."),
        count("compiler.note."),
    )
}

fn first_diff_line(actual: &str, expected: &str) -> String {
    for (index, (a, e)) in actual.lines().zip(expected.lines()).enumerate() {
        if a != e {
            return format!("line {}: rust={a:?} javac={e:?}", index + 1);
        }
    }
    let a = actual.lines().count();
    let e = expected.lines().count();
    format!("line count: rust={a} javac={e}")
}

#[test]
fn diagnostic_corpus() {
    let Some(javac) = javac() else {
        eprintln!("DIAG_CORPUS skipped: JAVA_HOME/bin/javac not found");
        return;
    };
    let cases_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("diagnostic_corpus")
        .join("cases");
    let filter = std::env::var("DIAG_CORPUS_FILTER").ok();
    let mut names: Vec<String> = std::fs::read_dir(&cases_dir)
        .expect("read cases dir")
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    let mut total = 0usize;
    let mut pass = 0usize;
    let mut javac_empty: Vec<String> = Vec::new();
    let mut failures: Vec<(String, String)> = Vec::new();
    for name in names {
        if let Some(filter) = &filter
            && !name.contains(filter)
        {
            continue;
        }
        total += 1;
        let case_dir = cases_dir.join(&name);
        let tmp = std::env::temp_dir().join(format!(
            "diag-corpus-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let mut args: Vec<String> = Vec::new();
        let mut sources: Vec<PathBuf> = Vec::new();
        for entry in std::fs::read_dir(&case_dir).expect("case dir").flatten() {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();
            let copied = tmp.join(&file_name);
            std::fs::copy(&path, &copied).expect("copy case file");
            if file_name.ends_with(".java") {
                sources.push(copied);
            } else if file_name == "args.txt" {
                let text = std::fs::read_to_string(&copied).expect("read args");
                for line in text.lines() {
                    let line = line.trim();
                    if !line.is_empty() {
                        args.push(line.to_string());
                    }
                }
            }
        }
        sources.sort();
        let javac_out = tmp.join("javac-out");
        let rust_out = tmp.join("rust-out");
        let mut javac_cmd = Command::new(&javac);
        javac_cmd
            .args(["-J-Duser.language=en", "-J-Duser.country=US"])
            .args(&args)
            .arg("-d")
            .arg(&javac_out)
            .args(&sources)
            .current_dir(&tmp);
        let mut rust_cmd = Command::new(rust_cli());
        rust_cmd
            .args(["-J-Duser.language=en", "-J-Duser.country=US"])
            .args(&args)
            .arg("-d")
            .arg(&rust_out)
            .args(&sources)
            .current_dir(&tmp);
        let javac_result = javac_cmd.output().expect("run javac");
        let rust_result = rust_cmd.output().expect("run rust-javac");
        let javac_stdout = normalize(&String::from_utf8_lossy(&javac_result.stdout), &tmp);
        let javac_stderr = normalize(&String::from_utf8_lossy(&javac_result.stderr), &tmp);
        let rust_stdout = normalize(&String::from_utf8_lossy(&rust_result.stdout), &tmp);
        let rust_stderr = normalize(&String::from_utf8_lossy(&rust_result.stderr), &tmp);
        if javac_result.status.success() && javac_stderr.trim().is_empty() {
            javac_empty.push(name.clone());
            let _ = std::fs::remove_dir_all(&tmp);
            continue;
        }
        if rust_result.status.code() == javac_result.status.code()
            && rust_stdout == javac_stdout
            && rust_stderr == javac_stderr
        {
            pass += 1;
        } else {
            let diff = if rust_result.status.code() != javac_result.status.code() {
                format!(
                    "exit: rust={:?} javac={:?}",
                    rust_result.status.code(),
                    javac_result.status.code()
                )
            } else if rust_stderr != javac_stderr {
                first_diff_line(&rust_stderr, &javac_stderr)
            } else {
                first_diff_line(&rust_stdout, &javac_stdout)
            };
            failures.push((name.clone(), diff));
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
    println!(
        "DIAG_CORPUS total={total} pass={pass} fail={} javac_empty={}",
        failures.len(),
        javac_empty.len()
    );
    let (err, warn, misc, note) = message_counts();
    println!(
        "DIAG_CORPUS_MESSAGES err={err} warn={warn} misc={misc} note={note} total={}",
        err + warn + misc + note
    );
    for name in &javac_empty {
        println!("EMPTY {name}");
    }
    for (name, diff) in &failures {
        println!("FAIL {name}: {diff}");
    }
}
