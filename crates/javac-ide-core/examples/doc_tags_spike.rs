
//! Doc render spike: @param/@return tags become Markdown.

use std::path::PathBuf;

use javac_analysis_api::{AnalysisHost, Position};
use javac_ide_core::analysis::{AnalysisConfig, AnalysisSession};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let class_dir = PathBuf::from(&args[1]);
    let out_dir = PathBuf::from(&args[2]);
    let mut session = AnalysisSession::new(AnalysisConfig {
        class_path: vec![class_dir],
        output_dir: out_dir,
        source_level: "17".to_string(),
        source_path: Vec::new(),
    });
    let host: &mut dyn AnalysisHost = &mut session;
    let path = PathBuf::from("target/tmp/DocTags.java");
    let text = "/** Adds two numbers. */\nclass DocTags {\n  /**\n   * Adds two numbers.\n   *\n   * @param a first operand\n   * @param b second operand\n   * @return the sum\n   * @throws IllegalArgumentException on overflow\n   */\n  int add(int a, int b) { return a + b; }\n}\n";
    let _ = host.analyze(&path, Some(text));
    println!("add hover: {:?}", host.hover(&path, Position::new(10, 7)));
}