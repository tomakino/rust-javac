
//! Incremental spike: consecutive edits without rebuilding the compiler.

use std::path::PathBuf;
use std::time::Instant;

use javac_analysis_api::AnalysisHost;
use javac_ide_core::analysis::{AnalysisConfig, AnalysisSession};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let class_dir = PathBuf::from(&args[1]);
    let source = PathBuf::from(&args[2]);
    let out_dir = PathBuf::from(&args[3]);
    let mut session = AnalysisSession::new(AnalysisConfig {
        class_path: vec![class_dir],
        output_dir: out_dir,
        source_level: "17".to_string(),
        source_path: Vec::new(),
    });
    let host: &mut dyn AnalysisHost = &mut session;
    let broken = "class Gson { void f() { int x = ; } }\n";
    let clean = "class Gson {}\n";
    let broken2 = "class Gson { void g() { int y = ; } }\n";

    let start = Instant::now();
    let d1 = host.analyze(&source, Some(broken));
    let t1 = start.elapsed();
    let d2 = host.analyze(&source, Some(clean));
    let t2 = start.elapsed() - t1;
    let d3 = host.analyze(&source, Some(broken2));
    let t3 = start.elapsed() - t1 - t2;
    println!("first  {:?} diagnostics={}", t1, d1.len());
    println!("edit1  {:?} diagnostics={}", t2, d2.len());
    println!("edit2  {:?} diagnostics={}", t3, d3.len());
    for d in d2.iter().chain(d3.iter()).take(3) {
        println!("  {:?} {}", d.severity, d.message);
    }
}