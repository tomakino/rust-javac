//! Methods used to support text blocks lint.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! java.util.Set<WhitespaceChecks> -> std HashSet;
//! String.split("\\R") is expressed with the ported String.lines helper (the
//! text block content has normalized line terminators); String.isBlank /
//! stripLeading are the ported java_string_methods.

use std::collections::HashSet;

use javac_strings::JavaString;

use super::java_string_methods;

/// Java: TextBlockSupport.WhitespaceChecks (upstream Java enum names kept).
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum WhitespaceChecks {
    INCONSISTENT,
    TRAILING,
}

/// Java: static Set<WhitespaceChecks> checkWhitespace(String string).
pub(crate) fn check_whitespace(string: &JavaString) -> HashSet<WhitespaceChecks> {
    let mut checks = HashSet::new();
    if string.is_empty() {
        return checks;
    }
    // Maximum common indentation.
    let mut outdent = 0i32;
    // No need to check indentation if opting out (last line is empty.)
    let units = string.as_units();
    let last_char = units[units.len() - 1];
    let opt_out = last_char == '\n' as u16 || last_char == '\r' as u16;
    let lines = java_string_methods::lines(string);
    let length = lines.len();
    let last_line = if length == 0 { JavaString::new() } else { lines[length - 1].clone() };
    if !opt_out {
        // Prime with the last line indentation (may be blank.)
        outdent = java_string_methods::index_of_non_whitespace(&last_line);
        for line in &lines {
            // Blank lines have no influence (last line accounted for.)
            if !java_string_methods::is_blank(line) {
                outdent = outdent.min(java_string_methods::index_of_non_whitespace(line));
                if outdent == 0 {
                    break;
                }
            }
        }
    }
    // Last line is representative.
    let start = last_line.substring(0, outdent as usize);
    for line in &lines {
        // Fail if a line does not have the same indentation.
        if !java_string_methods::is_blank(line) && !line.starts_with(&start) {
            // Mix of different white space
            checks.insert(WhitespaceChecks::INCONSISTENT);
        }
        // Line has content even after indent is removed.
        if (outdent as usize) < line.len() {
            // Is the last character a white space.
            let last = line.as_units()[line.len() - 1];
            if java_string_methods::is_java_whitespace(last) {
                // Has trailing white space.
                checks.insert(WhitespaceChecks::TRAILING);
            }
        }
    }
    checks
}
