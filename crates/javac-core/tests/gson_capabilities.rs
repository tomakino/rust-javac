//! Opt-in gson capability check on generated class files.
//!
//! Requires GSON_SRC, GSON_GENERATED and a JDK 17 javac/java on PATH (or
//! JAVA_HOME). The repository never stores gson sources; only the small
//! GsonCapabilities.java fixture lives here.

use std::path::{Path, PathBuf};
use std::process::Command;

use javac_core::main::mini_compiler::MiniCompiler;

fn collect_java(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_java(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "java")
            && path.file_name().is_some_and(|name| name != "module-info.java")
        {
            out.push(path);
        }
    }
}

fn tool(name: &str) -> PathBuf {
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        let candidate = Path::new(&home).join("bin").join(format!("{name}.exe"));
        if candidate.exists() {
            return candidate;
        }
        let candidate = Path::new(&home).join("bin").join(name);
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from(name)
}

#[test]
fn gson_capabilities_run_on_jdk17() {
    let Some(src) = std::env::var_os("GSON_SRC") else {
        return;
    };
    let Some(generated) = std::env::var_os("GSON_GENERATED") else {
        return;
    };
    let mut sources = Vec::new();
    collect_java(&PathBuf::from(src), &mut sources);
    sources.push(PathBuf::from(generated));
    sources.sort();

    let out = std::env::temp_dir().join(format!("rust-javac-gson-cap-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("gson output directory");

    let mut compiler = MiniCompiler::new(&out);
    let compiled = compiler.compile_files(&sources).expect("gson compilation");
    println!(
        "gson files={} errors={} classes={}",
        sources.len(),
        compiler.error_count(),
        compiled.len()
    );
    assert_eq!(compiler.error_count(), 0, "gson must compile without errors");

    let fixture = out.join("GsonCapabilities.java");
    std::fs::write(&fixture, include_str!("fixtures/GsonCapabilities.java"))
        .expect("write capability fixture");

    let javac = Command::new(tool("javac"))
        .arg("-cp")
        .arg(&out)
        .arg("-d")
        .arg(&out)
        .arg(&fixture)
        .output()
        .expect("run javac");
    assert!(
        javac.status.success(),
        "javac failed: {}{}",
        String::from_utf8_lossy(&javac.stdout),
        String::from_utf8_lossy(&javac.stderr)
    );

    let run = Command::new(tool("java"))
        .arg("-Xverify:all")
        .arg("-cp")
        .arg(&out)
        .arg("GsonCapabilities")
        .output()
        .expect("run java");
    let stdout = String::from_utf8_lossy(&run.stdout);
    let stderr = String::from_utf8_lossy(&run.stderr);
    println!("{stdout}");
    eprintln!("{stderr}");
    assert!(
        run.status.success(),
        "GsonCapabilities failed: {stdout}{stderr}"
    );
    assert!(
        stdout.contains("PASSED=56 FAILED=0"),
        "unexpected capability result: {stdout}{stderr}"
    );
}
