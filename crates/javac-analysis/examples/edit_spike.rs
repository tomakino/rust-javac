
//! Edit spike: analyze, edit the file, analyze again.

use std::path::PathBuf;
use std::time::Instant;

use javac_analysis::{AnalysisConfig, AnalysisSession};

fn main() {
    let dir = PathBuf::from("target/tmp/ide-edit");
    std::fs::create_dir_all(&dir).expect("create dir");
    let source = dir.join("A.java");
    std::fs::write(&source, "class A { int x; }\n").expect("write v1");

    let mut session = AnalysisSession::new(AnalysisConfig {
        class_path: Vec::new(),
        output_dir: dir.join("out"),
        source_level: "17".to_string(),
    });

    let t = Instant::now();
    let r = session.analyze(&source);
    println!("analyze[v1]: {:.1}ms errors={} warnings={} ok={}", t.elapsed().as_secs_f64() * 1000.0, r.errors, r.warnings, r.ok);

    let t = Instant::now();
    let r = session.analyze(&source);
    println!("analyze[unchanged]: {:.1}ms errors={} ok={}", t.elapsed().as_secs_f64() * 1000.0, r.errors, r.ok);

    std::fs::write(&source, "class A { int x; void f() { x = 1; } }\n").expect("write v2");
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let t = Instant::now();
    let r = session.analyze(&source);
    println!("analyze[v2]: {:.1}ms errors={} warnings={} ok={}", t.elapsed().as_secs_f64() * 1000.0, r.errors, r.warnings, r.ok);

    let t = Instant::now();
    let r = session.analyze(&source);
    println!("analyze[v2-cached]: {:.1}ms errors={} ok={}", t.elapsed().as_secs_f64() * 1000.0, r.errors, r.ok);
}
