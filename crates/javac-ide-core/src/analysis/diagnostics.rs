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
    let before = &text[..offset];
    let line = before.matches('\n').count() as u32;
    let column = before
        .rfind('\n')
        .map(|index| (offset - index - 1) as u32)
        .unwrap_or(offset as u32);
    let severity = match diagnostic.get_type() {
        DiagnosticType::ERROR => javac_analysis_api::Severity::Error,
        DiagnosticType::WARNING => javac_analysis_api::Severity::Warning,
        _ => javac_analysis_api::Severity::Information,
    };
    javac_analysis_api::Diagnostic {
        range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(line, column)),
        severity,
        message: diagnostic.get_message(None).to_string_lossy(),
        code: None,
    }
}


