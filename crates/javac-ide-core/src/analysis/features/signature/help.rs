//! Signature help.

use std::path::{Path, PathBuf};

use super::super::super::{canonical, FileAnalysis};

impl FileAnalysis {
    /// Signature help for the call at a position.
    pub(crate) fn signature_help_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::SignatureHelp> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())?;
        let cursor = crate::analysis::positions::line_offset(
            &text,
            position.line as usize + 1,
            position.column as usize + 1,
        );
        let open = call_open_offset(&text, cursor)?;
        // Editing a declaration's parameter list is not a call.
        if is_declaration_paren(&text, open) {
            return None;
        }
        let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
        let name: String = text[..open]
            .chars()
            .rev()
            .take_while(|c| is_ident(*c))
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if name.is_empty() {
            return None;
        }
        let active_parameter = parameter_index(&text[open + 1..cursor]);
        let mut signatures = Vec::new();
        for symbol in self.symbols() {
            if symbol.kind != "method" || symbol.name != name {
                continue;
            }
            let label = symbol.detail;
            let parameters = signature_parameters(&label);
            signatures.push(javac_analysis_api::SignatureInformation { label, parameters });
        }
        if signatures.is_empty() {
            return None;
        }
        // Prefer the first overload that can hold the current argument.
        let active_signature = signatures
            .iter()
            .position(|signature| signature.parameters.len() as u32 > active_parameter)
            .unwrap_or(0) as u32;
        Some(javac_analysis_api::SignatureHelp {
            signatures,
            active_signature,
            active_parameter,
        })
    }
}

/// Whether the parenthesized list at `open` belongs to a declaration.
///
/// A declaration is preceded by a type/modifier (`void f(`); a call is
/// preceded by an operator, `.`, `new`, `return`, or starts the statement.
fn is_declaration_paren(text: &str, open: usize) -> bool {
    let is_ident = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '$';
    let before = text[..open].trim_end();
    let name_start = before
        .rfind(|ch: char| !is_ident(ch))
        .map(|index| index + 1)
        .unwrap_or(0);
    let name = &before[name_start..];
    if name.is_empty() {
        return false;
    }
    let prefix = before[..name_start].trim_end();
    if prefix.is_empty() {
        // Statement-leading call, e.g. `f(1);`.
        return false;
    }
    let last = prefix.chars().next_back().expect("non-empty prefix");
    if !is_ident(last) {
        return false;
    }
    let token_start = prefix
        .rfind(|ch: char| !is_ident(ch))
        .map(|index| index + 1)
        .unwrap_or(0);
    let token = &prefix[token_start..];
    !matches!(
        token,
        "new" | "return"
            | "throw"
            | "if"
            | "while"
            | "for"
            | "switch"
            | "catch"
            | "synchronized"
            | "assert"
            | "case"
            | "else"
            | "yield"
            | "do"
            | "try"
            | "finally"
            | "instanceof"
            | "super"
            | "this"
    )
}

/// The byte offset of the `(` that opens the call containing `cursor`.
///
/// Scans backwards to support calls that span multiple lines, and stops at a
/// statement boundary when the parentheses are balanced.
fn call_open_offset(text: &str, cursor: usize) -> Option<usize> {
    let cursor = cursor.min(text.len());
    let mut depth = 0i32;
    for (index, ch) in text[..cursor].char_indices().rev() {
        match ch {
            ')' => depth += 1,
            '(' => {
                if depth == 0 {
                    return Some(index);
                }
                depth -= 1;
            }
            ';' | '{' | '}' if depth == 0 => return None,
            _ => {}
        }
    }
    None
}

/// Zero-based index of the argument at the cursor.
///
/// Commas inside nested calls, strings, characters, and comments are ignored.
fn parameter_index(text: &str) -> u32 {
    let mut depth = 0i32;
    let mut count = 0u32;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => skip_string(&mut chars, '"'),
            '\'' => skip_string(&mut chars, '\''),
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut previous = '\0';
                for c in chars.by_ref() {
                    if previous == '*' && c == '/' {
                        break;
                    }
                    previous = c;
                }
            }
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => count += 1,
            _ => {}
        }
    }
    count
}

fn skip_string(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, quote: char) {
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            chars.next();
        } else if ch == quote {
            break;
        }
    }
}

/// Split the parameter list out of a method signature label.
fn signature_parameters(label: &str) -> Vec<String> {
    let Some(open) = label.find('(') else {
        return Vec::new();
    };
    let Some(close) = label[open..].find(')') else {
        return Vec::new();
    };
    let inner = &label[open + 1..open + close];
    if inner.trim().is_empty() {
        return Vec::new();
    }
    inner.split(", ").map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_calls_do_not_count_inner_commas() {
        assert_eq!(parameter_index("g(1, 2), "), 1);
        assert_eq!(parameter_index("\"a,b\", "), 1);
        assert_eq!(parameter_index("(1, 2), "), 1);
        assert_eq!(parameter_index("1, \"x,y\", "), 2);
    }

    #[test]
    fn multi_line_calls_find_the_open_paren() {
        let text = "class A {\n    void g() { f(\n        1,\n    }\n}\n";
        let cursor = text.find("1,").unwrap();
        let open = call_open_offset(text, cursor).expect("open");
        assert_eq!(&text[open..open + 1], "(");
    }
}
