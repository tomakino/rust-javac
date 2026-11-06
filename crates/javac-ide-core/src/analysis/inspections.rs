//! Extra inspections aligned with IntelliJ IDEA (beyond javac errors).

use javac_analysis_api::{Diagnostic, Position, Range, Severity};

/// Report imports that are never referenced in the code.
///
/// Strings, character literals, and comments are masked before matching, and
/// matches must be whole identifiers (`List` does not match `ArrayList`).
pub(crate) fn unused_imports(text: &str) -> Vec<Diagnostic> {
    let code = mask_non_code(text);
    let code_lines: Vec<&str> = code.lines().collect();
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
        let used = code_lines.iter().enumerate().any(|(index, code_line)| {
            index != line_index
                && !code_line.trim_start().starts_with("import ")
                && has_identifier(code_line, simple)
        });
        if !used {
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

/// Replace comments and literals with spaces, keeping newlines.
pub(crate) fn mask_non_code(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch == '/' && index + 1 < chars.len() && chars[index + 1] == '/' {
            while index < chars.len() && chars[index] != '\n' {
                out.push(' ');
                index += 1;
            }
        } else if ch == '/' && index + 1 < chars.len() && chars[index + 1] == '*' {
            out.push(' ');
            out.push(' ');
            index += 2;
            while index < chars.len() {
                if chars[index] == '*' && index + 1 < chars.len() && chars[index + 1] == '/' {
                    out.push(' ');
                    out.push(' ');
                    index += 2;
                    break;
                }
                out.push(if chars[index] == '\n' { '\n' } else { ' ' });
                index += 1;
            }
        } else if ch == '"' && index + 2 < chars.len() && chars[index + 1] == '"' && chars[index + 2] == '"' {
            out.push_str("   ");
            index += 3;
            while index < chars.len() {
                if index + 2 < chars.len()
                    && chars[index] == '"'
                    && chars[index + 1] == '"'
                    && chars[index + 2] == '"'
                {
                    out.push_str("   ");
                    index += 3;
                    break;
                }
                out.push(if chars[index] == '\n' { '\n' } else { ' ' });
                index += 1;
            }
        } else if ch == '"' || ch == '\'' {
            let quote = ch;
            out.push(' ');
            index += 1;
            while index < chars.len() {
                let current = chars[index];
                if current == '\\' && index + 1 < chars.len() {
                    out.push(' ');
                    out.push(' ');
                    index += 2;
                    continue;
                }
                out.push(if current == '\n' { '\n' } else { ' ' });
                index += 1;
                if current == quote {
                    break;
                }
            }
        } else {
            out.push(ch);
            index += 1;
        }
    }
    out
}

fn has_identifier(line: &str, name: &str) -> bool {
    let is_part = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '$';
    let mut search = 0;
    while let Some(position) = line[search..].find(name) {
        let absolute = search + position;
        let before = line[..absolute].chars().next_back();
        let after = line[absolute + name.len()..].chars().next();
        if before.is_none_or(|ch| !is_part(ch)) && after.is_none_or(|ch| !is_part(ch)) {
            return true;
        }
        search = absolute + name.len();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unused_when_only_a_string_mentions_the_name() {
        let text = "import b.B;\nclass A { String s = \"B\"; }\n";
        assert_eq!(unused_imports(text).len(), 1, "{:?}", unused_imports(text));
    }

    #[test]
    fn unused_when_only_a_comment_mentions_the_name() {
        let text = "import b.B;\nclass A { // B\n    int x;\n}\n";
        assert_eq!(unused_imports(text).len(), 1);
    }

    #[test]
    fn used_when_code_references_the_name() {
        let text = "import b.B;\nclass A { B b; }\n";
        assert!(unused_imports(text).is_empty());
    }

    #[test]
    fn a_longer_identifier_is_not_a_use() {
        let text = "import b.List;\nclass A { ArrayList list; }\n";
        assert_eq!(unused_imports(text).len(), 1);
    }
}
