//! LSP position conversions (UTF-16 columns).

use std::path::PathBuf;

/// UTF-16 offset of a 1-based line/column (LSP coordinates).
pub(crate) fn line_offset(text: &str, line: usize, column: usize) -> usize {
    let mut current_line = 1;
    let mut current_column = 1;
    for (byte_index, current) in text.char_indices() {
        if current_line == line && current_column >= column {
            return byte_index;
        }
        if current == '\n' {
            current_line += 1;
            current_column = 1;
        } else {
            current_column += current.len_utf16();
        }
    }
    text.len()
}


/// Byte offset for a UTF-16 offset, or None when the offset splits a
/// surrogate pair.
pub(crate) fn utf16_to_byte(text: &str, offset: usize) -> Option<usize> {
    let mut consumed = 0usize;
    for (byte, current) in text.char_indices() {
        if consumed >= offset {
            return Some(byte);
        }
        consumed += current.len_utf16();
    }
    (consumed == offset).then_some(text.len())
}

/// Zero-based line/UTF-16 column of a UTF-16 offset.
pub(crate) fn offset_to_line_col(text: &str, offset: usize) -> (u32, u32) {
    let mut line = 0u32;
    let mut column = 0u32;
    let mut consumed = 0usize;
    for current in text.chars() {
        if consumed >= offset {
            break;
        }
        if current == '\n' {
            line += 1;
            column = 0;
        } else {
            column += current.len_utf16() as u32;
        }
        consumed += current.len_utf16();
    }
    (line, column)
}


/// The identifier under a 1-based line/column.
pub(crate) fn word_at(text: &str, line: usize, column: usize) -> Option<String> {
    let line_text = text.lines().nth(line.saturating_sub(1))?;
    let chars: Vec<char> = line_text.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let mut col = char_index_for_utf16(&chars, column.saturating_sub(1))
        .min(chars.len().saturating_sub(1));
    if !is_ident(chars[col]) && col > 0 && is_ident(chars[col - 1]) {
        col -= 1;
    }
    if !is_ident(chars[col]) {
        return None;
    }
    let mut start = col;
    while start > 0 && is_ident(chars[start - 1]) {
        start -= 1;
    }
    let mut end = col;
    while end + 1 < chars.len() && is_ident(chars[end + 1]) {
        end += 1;
    }
    Some(chars[start..=end].iter().collect())
}

/// Build a Location from an indexed declaration.
pub(crate) fn indexed_location(file: PathBuf, pos: i32) -> Option<javac_analysis_api::Location> {
    let text = std::fs::read_to_string(&file).ok()?;
    let (line, column) = offset_to_line_col(&text, pos.max(0) as usize);
    Some(javac_analysis_api::Location {
        file,
        range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(line, column)),
    })
}


/// The identifier prefix before a 1-based line/column (completion input).
pub(crate) fn prefix_at(text: &str, line: usize, column: usize) -> String {
    let line_text = text.lines().nth(line.saturating_sub(1)).unwrap_or("");
    let chars: Vec<char> = line_text.chars().collect();
    let mut end = char_index_for_utf16(&chars, column.saturating_sub(1)).min(chars.len());
    let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let mut start = end;
    while start > 0 && is_ident(chars[start - 1]) {
        start -= 1;
    }
    end = end.max(start);
    chars[start..end].iter().collect()
}


/// A range for an identifier declared at the given offset.
pub(crate) fn symbol_range(
    text: &str,
    pos: i32,
    name: &str,
) -> javac_analysis_api::Range {
    let (line, column) = offset_to_line_col(text, pos.max(0) as usize);
    javac_analysis_api::Range::new(
        javac_analysis_api::Position::new(line, column),
        javac_analysis_api::Position::new(line, column + name.len() as u32),
    )
}


/// All word-boundary occurrences of an identifier in a text.
pub(crate) fn occurrences(
    text: &str,
    word: &str,
    file: &std::path::Path,
) -> Vec<javac_analysis_api::Location> {
    let mut result = Vec::new();
    if word.is_empty() {
        return result;
    }
    let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    for (line_index, line_text) in text.lines().enumerate() {
        let chars: Vec<char> = line_text.chars().collect();
        let mut start = 0;
        while start + word.chars().count() <= chars.len() {
            let slice: String = chars[start..].iter().collect();
            let Some(offset) = slice.find(word) else { break };
            let begin = start + offset;
            let end = begin + word.chars().count();
            let before_ok = begin == 0 || !is_ident(chars[begin - 1]);
            let after_ok = end >= chars.len() || !is_ident(chars[end]);
            if before_ok && after_ok {
                result.push(javac_analysis_api::Location {
                    file: file.to_path_buf(),
                    range: javac_analysis_api::Range::new(
                        javac_analysis_api::Position::new(
                            line_index as u32,
                            utf16_column(&chars, begin),
                        ),
                        javac_analysis_api::Position::new(
                            line_index as u32,
                            utf16_column(&chars, end),
                        ),
                    ),
                });
            }
            start = begin + 1;
        }
    }
    result
}


/// Convert a UTF-16 column to a char index in the given line.
pub(crate) fn char_index_for_utf16(chars: &[char], utf16_column: usize) -> usize {
    let mut units = 0usize;
    for (index, current) in chars.iter().enumerate() {
        if units >= utf16_column {
            return index;
        }
        units += current.len_utf16();
    }
    // Past the last character: the cursor is at the end of the line.
    chars.len()
}

/// Convert a char index to a UTF-16 column.
fn utf16_column(chars: &[char], char_index: usize) -> u32 {
    chars
        .iter()
        .take(char_index)
        .map(|current| current.len_utf16() as u32)
        .sum()
}
