//! Conversion of compiler diagnostics to the analysis API.

use crate::util::jc_diagnostic::{DiagnosticType, JCDiagnostic};

/// Convert one compiler diagnostic to the API type.
pub(crate) fn convert_diagnostic(
    diagnostic: &crate::util::jc_diagnostic::JCDiagnostic,
    text: &str,
) -> javac_analysis_api::Diagnostic {
    use crate::util::jc_diagnostic::DiagnosticType;
    let offset = diagnostic.get_position().max(0) as usize;
    let offset = offset.min(text.len());
    let end = diagnostic_end(text, offset);
    let (line, column) = super::positions::offset_to_line_col(text, offset);
    let (end_line, end_column) = super::positions::offset_to_line_col(text, end);
    let severity = match diagnostic.get_type() {
        DiagnosticType::ERROR => javac_analysis_api::Severity::Error,
        DiagnosticType::WARNING => javac_analysis_api::Severity::Warning,
        _ => javac_analysis_api::Severity::Information,
    };
    javac_analysis_api::Diagnostic {
        range: javac_analysis_api::Range::new(
            javac_analysis_api::Position::new(line, column),
            javac_analysis_api::Position::new(end_line, end_column),
        ),
        severity,
        message: diagnostic.get_message(None).to_string_lossy(),
        code: Some(diagnostic.get_code_string()),
    }
}

fn diagnostic_end(text: &str, offset: usize) -> usize {
    let mut end = offset;
    for ch in text[offset..].chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == '$' {
            end += ch.len_utf8();
        } else {
            break;
        }
    }
    if end == offset {
        if let Some(ch) = text[offset..].chars().next() {
            end += ch.len_utf8();
        }
    }
    end.min(text.len())
}


