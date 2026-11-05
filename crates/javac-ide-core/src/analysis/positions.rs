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
    let mut col = column.saturating_sub(1).min(chars.len() - 1);
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
