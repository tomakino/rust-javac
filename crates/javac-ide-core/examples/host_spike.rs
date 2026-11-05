
//! Host spike: drive the analysis session through the compiler-agnostic API.

use std::path::PathBuf;

use javac_analysis_api::{AnalysisHost, Position};
use javac_ide_core::analysis::{AnalysisConfig, AnalysisSession};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: host_spike <class-dir> <source> <out-dir>");
        std::process::exit(2);
    }
    let mut session = AnalysisSession::new(AnalysisConfig {
        class_path: vec![PathBuf::from(&args[1])],
        output_dir: PathBuf::from(&args[3]),
        source_level: "17".to_string(),
        source_path: Vec::new(),
    });
    let file = PathBuf::from(&args[2]);
    let host: &mut dyn AnalysisHost = &mut session;

    let diagnostics = host.analyze(&file, None);
    println!("diagnostics: {}", diagnostics.len());

    let symbols = host.symbols(&file);
    println!("symbols: {}", symbols.len());

    let position = Position::new(140, 19);
    match host.symbol_at(&file, position) {
        Some(symbol) => println!("symbol_at(140,19): {:?} {}", symbol.kind, symbol.name),
        None => println!("symbol_at(140,19): none"),
    }
    match host.hover(&file, position) {
        Some(text) => println!("hover: {text}"),
        None => println!("hover: none"),
    }
}