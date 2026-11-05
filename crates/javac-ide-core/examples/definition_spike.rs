
//! Definition spike: symbol lookup / definition on an overlay buffer.

use std::path::PathBuf;

use javac_analysis_api::{AnalysisHost, Position};
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
    let text = "class Gson { int x; void f() { } }\n";
    let _ = host.analyze(&source, Some(text));
    let position = Position::new(0, 7);
    println!("symbol_at: {:?}", host.symbol_at(&source, position));
    println!("definition: {:?}", host.definition(&source, position));
    println!("hover: {:?}", host.hover(&source, position));
}
