
//! Overlay spike: analyze an in-memory (unsaved) text.

use std::path::PathBuf;

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
    });
    let host: &mut dyn AnalysisHost = &mut session;
    let broken = "class Gson { void f() { int x = ; } }\n";
    let diagnostics = host.analyze(&source, Some(broken));
    println!("overlay diagnostics: {}", diagnostics.len());
    for diagnostic in diagnostics.iter().take(3) {
        println!("  {:?} {} @{:?}", diagnostic.severity, diagnostic.message, diagnostic.range);
    }
    let clean = "class Gson {}\n";
    let diagnostics = host.analyze(&source, Some(clean));
    println!("clean overlay diagnostics: {}", diagnostics.len());
}
