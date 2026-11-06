
//! Javadoc spike: hover returns the declaration documentation.

use std::path::PathBuf;

use javac_analysis_api::{WorkspaceService, Position};
use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::Workspace;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let class_dir = PathBuf::from(&args[1]);
    let out_dir = PathBuf::from(&args[2]);
    let mut session = Workspace::new(AnalysisConfig {
        class_path: vec![class_dir],
        output_dir: out_dir,
        source_level: "17".to_string(),
        source_path: Vec::new(),
    });
    let host: &mut dyn WorkspaceService = &mut session;
    let path = PathBuf::from("target/tmp/JavadocFoo.java");
    let text = "/** The JavadocFoo class. */\nclass JavadocFoo {\n  /** The bar field. */\n  int bar;\n  /** Writes a value. */\n  void write() {}\n}\n";
    let diagnostics = host.diagnostics(&path, Some(text));
    println!("analyze diagnostics: {}", diagnostics.len());
    for diagnostic in &diagnostics {
        println!("  {:?} {}", diagnostic.severity, diagnostic.message);
    }
    println!("symbols: {}", host.symbols(&path).len());
    println!("symbol_at(1,7): {:?}", host.symbol_at(&path, Position::new(1, 7)));
    for (label, line, column) in [("class", 1, 7), ("field", 3, 7), ("method", 5, 8)] {
        println!("{label}: {:?}", host.hover(&path, Position::new(line, column)));
    }
}