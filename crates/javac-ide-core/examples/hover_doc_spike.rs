
//! Hover documentation spike.

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
    let path = PathBuf::from("target/tmp/HoverDoc.java");
    let text = "class A{\n\n\npublic static void main(String[] args) {\n\tm();\n\tm2(\"\");\n}\n\n\n/**\n * 6666666\n * test\n *\n */\n@Deprecated\nstatic void m(){\n\n}\n\n\n/**\n * 6666666\n * test\n *\n */\nstatic void m2(String a){\n\n}\n\n\n}\n";
    host.diagnostics(&path, Some(text));
    let call_col = text.lines().nth(5).unwrap().find("m2").unwrap() as u32;
    let hover = host.hover(&path, Position::new(5, call_col));
    match hover {
        Some(hover) => println!("{}", hover),
        None => println!("NO HOVER"),
    }
}