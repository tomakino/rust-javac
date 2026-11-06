//! ravac-lsp binary: constructs the resident analysis session and serves LSP.

use std::io::{self, BufReader};
use std::path::PathBuf;

use javac_analysis_api::WorkspaceService;
use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::project::{self, LoadContext};
use javac_ide_core::workspace::Workspace;

fn main() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("ravac-lsp panic: {info}");
        let log = std::env::temp_dir().join("ravac-lsp-panic.log");
        let _ = std::fs::write(&log, format!("{info}\n"));
    }));
    let mut class_path: Vec<PathBuf> = Vec::new();
    let output_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("ravac-lsp-out")))
        .unwrap_or_else(|| std::env::temp_dir().join("ravac-lsp"));
    let mut source_level: Option<String> = None;
    let mut project_dir: Option<PathBuf> = None;
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
                source_level = Some(args[index + 1].clone());
                index += 2;
            }
            "--project" if index + 1 < args.len() => {
                project_dir = Some(PathBuf::from(&args[index + 1]));
                index += 2;
            }
            _ => index += 1,
        }
    }
    let mut workspace: Box<dyn WorkspaceService> = match project_dir {
        Some(project_dir) => {
            let model = project::load_with(
                &ravac_lsp::project_registry(),
                &project_dir,
                &LoadContext::default(),
            );
            Box::new(Workspace::new_from_project(
                &model,
                output_dir.clone(),
                source_level,
            ))
        }
        None => Box::new(Workspace::new(AnalysisConfig {
            class_path,
            output_dir: output_dir.clone(),
            source_level: source_level.unwrap_or_else(|| "17".to_string()),
            source_path: Vec::new(),
        })),
    };
    let stdin = BufReader::new(io::stdin());
    let stdout = io::stdout();
    ravac_lsp::run(&mut workspace, stdin, stdout);
    let _ = output_dir;
}
