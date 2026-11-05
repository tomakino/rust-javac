//! Extra inspections aligned with IntelliJ IDEA (beyond javac errors).

use javac_analysis_api::{Diagnostic, Position, Range, Severity};

/// Report imports that are never referenced in the file.
pub(crate) fn unused_imports(text: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (line_index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("import ") || trimmed.starts_with("import static ") {
            continue;
        }
        let Some(end) = trimmed.find(';') else {
            continue;
        };
        let path = trimmed[7..end].trim();
        if path.ends_with(".*") || path.is_empty() {
            continue;
        }
        let Some(simple) = path.rsplit('.').next() else {
            continue;
        };
        if simple.is_empty() {
            continue;
        }
        // The import line itself is one occurrence; more means it is used.
        let occurrences = text.matches(simple).count();
        if occurrences <= 1 {
            let start = Position::new(line_index as u32, 0);
            let end_position = Position::new(
                line_index as u32,
                line.chars().map(|c| c.len_utf16() as u32).sum(),
            );
            diagnostics.push(Diagnostic {
                range: Range::new(start, end_position),
                severity: Severity::Warning,
                message: format!("Unused import: {path}"),
                code: Some("unused.import".to_string()),
            });
        }
    }
    diagnostics
}
