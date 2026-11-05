
//! Multi-file debug spike: A broken -> B clean -> A fixed.

use std::path::PathBuf;

use javac_analysis_api::AnalysisHost;
use javac_ide_core::analysis::{AnalysisConfig, AnalysisSession};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let class_dir = PathBuf::from(&args[1]);
    let out_dir = PathBuf::from(&args[2]);
    let mut session = AnalysisSession::new(AnalysisConfig {
        class_path: vec![class_dir],
        output_dir: out_dir,
        source_level: "17".to_string(),
    });
    let host: &mut dyn AnalysisHost = &mut session;
    let a = PathBuf::from("target/tmp/editor-sim/A.java");
    let b = PathBuf::from("target/tmp/editor-sim/B.java");
    let da = host.analyze(&a, Some("class A { void f() { int x = ; } }\n"));
    println!("A broken: {}", da.len());
    let db = host.analyze(&b, Some("class B {}\n"));
    println!("B clean: {}", db.len());
    let da2 = host.analyze(&a, Some("class A {}\n"));
    println!("A fixed: {}", da2.len());
    for diagnostic in &da2 {
        println!("  {:?} {}", diagnostic.severity, diagnostic.message);
    }
}
