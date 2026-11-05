
//! ravac-lsp binary: constructs the resident analysis session and serves LSP.

use std::io::{self, BufReader};
use std::path::PathBuf;

use javac_ide_core::analysis::{AnalysisConfig, AnalysisSession};

fn main() {
    let mut class_path: Vec<PathBuf> = Vec::new();
    let mut output_dir = PathBuf::from("target/ravac-lsp");
    let mut source_level = "17".to_string();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--class-path" if index + 1 < args.len() => {
                class_path = args[index + 1]
                    .split(';')
                    .filter(|entry| !entry.is_empty())
                    .map(PathBuf::from)
                    .collect();
                index += 2;
            }
            "--source" if index + 1 < args.len() => {
                source_level = args[index + 1].clone();
                index += 2;
            }
            _ => index += 1,
        }
    }
    let mut session = AnalysisSession::new(AnalysisConfig {
        class_path,
        output_dir: output_dir.clone(),
        source_level,
    });
    let stdin = BufReader::new(io::stdin());
    let stdout = io::stdout();
    ravac_lsp::run(&mut session, stdin, stdout);
    let _ = output_dir;
}
