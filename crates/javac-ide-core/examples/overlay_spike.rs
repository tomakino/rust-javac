
//! Overlay spike: analyze an in-memory (unsaved) text.

use std::path::PathBuf;

use javac_analysis_api::WorkspaceService;
use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::Workspace;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let class_dir = PathBuf::from(&args[1]);
    let source = PathBuf::from(&args[2]);
    let out_dir = PathBuf::from(&args[3]);
    let mut session = Workspace::new(AnalysisConfig {
        class_path: vec![class_dir],
        output_dir: out_dir,
        source_level: "17".to_string(),
        source_path: Vec::new(),
    });
    let host: &mut dyn WorkspaceService = &mut session;
    let broken = "class Gson { void f() { int x = ; } }\n";
    let diagnostics = host.diagnostics(&source, Some(broken));
    println!("overlay diagnostics: {}", diagnostics.len());
    for diagnostic in diagnostics.iter().take(3) {
        println!("  {:?} {} @{:?}", diagnostic.severity, diagnostic.message, diagnostic.range);
    }
    let clean = "class Gson {}\n";
    let diagnostics = host.diagnostics(&source, Some(clean));
    println!("clean overlay diagnostics: {}", diagnostics.len());
}