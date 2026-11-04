//! Robustness checks: malformed inputs must produce diagnostics, not panic.

use std::process::Command;

#[test]
fn malformed_inputs_do_not_panic() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("robustness");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cases = [
        ("BadSyntax.java", "class {"),
        ("UnclosedComment.java", "class A { } /*"),
        ("BadUnicode.java", "class A { String s = \\uZZZZ; }"),
        ("Dup.java", "class A { int x; int x; }"),
        (
            "BadType.java",
            "class A { void m() { Object o = new Missing(); } }",
        ),
    ];
    for (name, src) in cases {
        let path = dir.join(name);
        std::fs::write(&path, src).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_ravac"))
            .arg("-d")
            .arg(dir.join("out"))
            .arg(&path)
            .output()
            .expect("run rust-javac");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("panicked"), "{name} panicked: {stderr}");
        assert!(!output.status.success(), "{name} should fail");
    }
}
