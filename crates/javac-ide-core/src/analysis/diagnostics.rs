//! Conversion of compiler diagnostics to the analysis API.

use crate::util::jc_diagnostic::{DiagnosticType, JCDiagnostic};

/// Convert one compiler diagnostic to the API type.
///
/// `JCDiagnostic::get_position` is a Java `char` (UTF-16) offset, while string
/// slicing and identifier scanning use UTF-8 byte offsets. Convert once at the
/// start, scan in bytes, and convert the end back to UTF-16 for line/column.
pub(crate) fn convert_diagnostic(
    diagnostic: &crate::util::jc_diagnostic::JCDiagnostic,
    text: &str,
) -> javac_analysis_api::Diagnostic {
    use crate::util::jc_diagnostic::DiagnosticType;
    let start_utf16 = diagnostic.get_position().max(0) as usize;
    let start_byte = super::positions::utf16_to_byte_snapped(text, start_utf16);
    let end_byte = diagnostic_end(text, start_byte);
    let start_utf16 = super::positions::byte_to_utf16(text, start_byte);
    let end_utf16 = super::positions::byte_to_utf16(text, end_byte);
    let (line, column) = super::positions::offset_to_line_col(text, start_utf16);
    let (end_line, end_column) = super::positions::offset_to_line_col(text, end_utf16);
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

/// Byte offset just past the identifier starting at `start`.
///
/// `start` must be a char boundary; `text.get` keeps this panic-free even if a
/// caller passes one that is not.
fn diagnostic_end(text: &str, start: usize) -> usize {
    let Some(tail) = text.get(start..) else {
        return start;
    };
    let mut end = start;
    for ch in tail.chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == '$' {
            end += ch.len_utf8();
        } else {
            break;
        }
    }
    if end == start {
        if let Some(ch) = tail.chars().next() {
            end += ch.len_utf8();
        }
    }
    end.min(text.len())
}


