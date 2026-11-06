
//! Code action spike: import quick fix from the engine.

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
    let path = PathBuf::from("target/tmp/Imp.java");
    let text = "class Imp { List l; }\n";
    let diagnostics = host.diagnostics(&path, Some(text));
    println!("diagnostics: {}", diagnostics.len());
    let actions = host.code_actions(&path, Position::new(0, text.find("List").unwrap_or(12) as u32));
    println!("actions: {:?}", actions);
}