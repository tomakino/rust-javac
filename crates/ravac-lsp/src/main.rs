//! ravac-lsp binary: constructs the resident analysis session and serves LSP.

use std::io::{self, BufReader};
use std::path::PathBuf;

use javac_analysis_api::WorkspaceService;
use javac_ide_core::analysis::AnalysisConfig;
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
    let mut inlay_hints_same_name = true;
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
            "--inlay-hints-same-name" if index + 1 < args.len() => {
                inlay_hints_same_name = parse_bool_flag(&args[index + 1]);
                index += 2;
            }
            arg if arg.starts_with("--inlay-hints-same-name=") => {
                inlay_hints_same_name =
                    parse_bool_flag(arg.trim_start_matches("--inlay-hints-same-name="));
                index += 1;
            }
            _ => index += 1,
        }
    }
    let mut workspace: Box<dyn WorkspaceService> = match project_dir {
        Some(project_dir) => {
            // The real workspace is created by `initialize` after the artifact
            // build. Keep the pre-initialize host empty: starting the index
            // compiler here raced with the artifact compiler on the shared
            // platform image (spurious module errors / panics).
            let _ = project_dir;
            Box::new(Workspace::new(AnalysisConfig {
                class_path: Vec::new(),
                output_dir: output_dir.clone(),
                source_level: source_level
                    .clone()
                    .unwrap_or_else(|| "17".to_string()),
                source_path: Vec::new(),
            }))
        }
        None => Box::new(Workspace::new(AnalysisConfig {
            class_path,
            output_dir: output_dir.clone(),
            source_level: source_level.unwrap_or_else(|| "17".to_string()),
            source_path: Vec::new(),
        })),
    };
    workspace.set_inlay_hints_same_name(inlay_hints_same_name);
    let stdin = BufReader::new(io::stdin());
    let stdout = io::stdout();
    ravac_lsp::run(&mut workspace, stdin, stdout);
    let _ = output_dir;
}

fn parse_bool_flag(value: &str) -> bool {
    match value.trim().to_ascii_lowercase().as_str() {
        "false" | "0" | "no" | "off" => false,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_bool_flag;

    #[test]
    fn inlay_hints_same_name_flag_parses_false_values() {
        assert!(!parse_bool_flag("false"));
        assert!(!parse_bool_flag("0"));
        assert!(!parse_bool_flag("off"));
    }

    #[test]
    fn inlay_hints_same_name_flag_defaults_to_true() {
        assert!(parse_bool_flag("true"));
        assert!(parse_bool_flag("1"));
        assert!(parse_bool_flag("anything-else"));
    }
}
