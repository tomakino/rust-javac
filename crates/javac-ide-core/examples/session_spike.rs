//! Session spike: repeated analysis of one file with a transient
//! [`FileAnalysis`].

use std::path::PathBuf;
use std::time::Instant;

use javac_analysis_api::Severity;

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::analysis::FileAnalysis;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: session_spike <class-dir> <source> <out-dir>");
        std::process::exit(2);
    }
    let class_dir = PathBuf::from(&args[1]);
    let source = PathBuf::from(&args[2]);
    let out_dir = PathBuf::from(&args[3]);
    let mut analysis = FileAnalysis::new(AnalysisConfig {
        class_path: vec![class_dir],
        output_dir: out_dir,
        source_level: "17".to_string(),
        source_path: Vec::new(),
    });
    for attempt in 1..=3 {
        let t = Instant::now();
        let diagnostics = analysis.diagnostics(&source, None);
        let errors = diagnostics
            .iter()
            .filter(|diagnostic| matches!(diagnostic.severity, Severity::Error))
            .count();
        let warnings = diagnostics
            .iter()
            .filter(|diagnostic| matches!(diagnostic.severity, Severity::Warning))
            .count();
        println!(
            "analyze[{attempt}]: {:.1}ms errors={} warnings={}",
            t.elapsed().as_secs_f64() * 1000.0,
            errors,
            warnings
        );
    }
    let symbols = analysis.symbols();
    println!("symbols: {}", symbols.len());
    for symbol in symbols.iter().take(8) {
        println!("  {} {} ({})", symbol.kind, symbol.name, symbol.flat_name);
    }

    let probe = analysis.symbol_at(&source, 141, 20);
    match probe {
        Some(symbol) => println!(
            "symbol_at(141,20): {} {} ({})",
            symbol.kind, symbol.name, symbol.flat_name
        ),
        None => println!("symbol_at(141,20): none"),
    }
}
