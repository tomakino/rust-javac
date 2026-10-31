//! End-to-end tests for the CLI driver (P1-1).

use std::path::PathBuf;
use std::process::Command;

fn cli() -> &'static str {
    env!("CARGO_BIN_EXE_rust-javac")
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rust-javac-cli-{name}-{}-{}",
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

#[test]
fn version_prints_release_and_exits_zero() {
    let output = Command::new(cli()).arg("-version").output().expect("run cli");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("javac 17"));
}

#[test]
fn help_prints_usage_and_exits_zero() {
    let output = Command::new(cli())
        .args(["-J-Duser.language=en", "-J-Duser.country=US", "-help"])
        .output()
        .expect("run cli");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage: javac"));
}

#[test]
fn error_source_reports_diagnostic_and_exit_one() {
    let dir = temp_dir("error");
    let source = dir.join("T.java");
    std::fs::write(&source, "class T { int x = \"s\"; }").expect("source");
    let output = Command::new(cli())
        .args(["-J-Duser.language=en", "-J-Duser.country=US"])
        .arg("-d")
        .arg(&dir)
        .arg(&source)
        .output()
        .expect("run cli");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error:"), "stderr: {stderr}");
    assert!(stderr.contains("1 error"), "stderr: {stderr}");
}

#[test]
fn argfile_is_expanded_and_compiles() {
    let dir = temp_dir("argfile");
    let source = dir.join("T.java");
    std::fs::write(&source, "class T { int x = 1; }").expect("source");
    let argfile = dir.join("args.txt");
    std::fs::write(
        &argfile,
        format!("{}\n-d\n{}\n", source.display(), dir.display()),
    )
    .expect("argfile");
    let output = Command::new(cli())
        .arg(format!("@{}", argfile.display()))
        .output()
        .expect("run cli");
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(dir.join("T.class").exists());
}

#[test]
fn default_output_is_next_to_each_source() {
    let dir = temp_dir("output");
    let a = dir.join("a");
    let b = dir.join("b");
    std::fs::create_dir_all(&a).expect("a");
    std::fs::create_dir_all(&b).expect("b");
    let source_a = a.join("A.java");
    let source_b = b.join("B.java");
    std::fs::write(&source_a, "class A {}").expect("A");
    std::fs::write(&source_b, "class B {}").expect("B");
    for source in ["a/A.java", "b/B.java"] {
        let output = Command::new(cli())
            .current_dir(&dir)
            .arg(source)
            .output()
            .expect("run cli");
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(a.join("A.class").exists());
    assert!(b.join("B.class").exists());
    assert!(!dir.join("A.class").exists());
    assert!(!dir.join("B.class").exists());

    let out = dir.join("out");
    let output = Command::new(cli())
        .current_dir(&dir)
        .arg("-d")
        .arg(&out)
        .arg("a/A.java")
        .arg("b/B.java")
        .output()
        .expect("run cli");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out.join("A.class").exists());
    assert!(out.join("B.class").exists());
}
