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
/// surrogate pair (or lies past the end of the text).
pub(crate) fn utf16_to_byte(text: &str, offset: usize) -> Option<usize> {
    let mut consumed = 0usize;
    for (byte, current) in text.char_indices() {
        if consumed == offset {
            return Some(byte);
        }
        if consumed > offset {
            return None;
        }
        consumed += current.len_utf16();
    }
    (consumed == offset).then_some(text.len())
}

/// Byte offset for a UTF-16 offset, snapping down to the nearest char boundary
/// when the offset splits a surrogate pair.
pub(crate) fn utf16_to_byte_snapped(text: &str, offset: usize) -> usize {
    if let Some(byte) = utf16_to_byte(text, offset) {
        return byte;
    }
    let mut consumed = 0usize;
    for (byte, current) in text.char_indices() {
        if consumed + current.len_utf16() > offset {
            return byte;
        }
        consumed += current.len_utf16();
    }
    text.len()
}

/// UTF-16 offset of a byte offset, snapping down to a char boundary.
pub(crate) fn byte_to_utf16(text: &str, byte: usize) -> usize {
    let mut byte = byte.min(text.len());
    while byte > 0 && !text.is_char_boundary(byte) {
        byte -= 1;
    }
    text[..byte].chars().map(char::len_utf16).sum()
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


/// Position of the declared name for a symbol.
///
/// Compiler symbol positions point at the declaration tree start (`class`,
/// modifiers, return type), while the editor should land on the name.
pub(crate) fn declared_name_start(text: &str, start: u32, name: &str) -> u32 {
    if name.is_empty() {
        return start;
    }
    let start_byte = utf16_to_byte_snapped(text, start as usize);
    let Some(tail) = text.get(start_byte..) else {
        return start;
    };
    let mut search = 0usize;
    while let Some(position) = tail[search..].find(name) {
        let absolute = start_byte + search + position;
        if !text[..absolute].trim_end().ends_with('.') {
            return byte_to_utf16(text, absolute) as u32;
        }
        search += position + name.len();
    }
    start
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
    // Columns are UTF-16 code units, not UTF-8 bytes.
    let width = name.encode_utf16().count() as u32;
    javac_analysis_api::Range::new(
        javac_analysis_api::Position::new(line, column),
        javac_analysis_api::Position::new(line, column + width),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_to_byte_snaps_inside_a_surrogate_pair() {
        let text = "a😀b"; // UTF-16: a=0, emoji=1..3, b=3; bytes: a=0, emoji=1..5, b=5
        assert_eq!(utf16_to_byte(text, 0), Some(0));
        assert_eq!(utf16_to_byte(text, 1), Some(1));
        assert_eq!(utf16_to_byte(text, 2), None, "inside the surrogate pair");
        assert_eq!(utf16_to_byte_snapped(text, 2), 1);
        assert_eq!(utf16_to_byte(text, 3), Some(5));
    }

    #[test]
    fn byte_to_utf16_snaps_down_to_char_boundaries() {
        let text = "中文，😀"; // UTF-16 length 5, UTF-8 length 13
        assert_eq!(byte_to_utf16(text, 0), 0);
        assert_eq!(byte_to_utf16(text, 3), 1);
        assert_eq!(byte_to_utf16(text, 4), 1, "inside 文");
        assert_eq!(byte_to_utf16(text, 6), 2);
        assert_eq!(byte_to_utf16(text, 9), 3);
        assert_eq!(byte_to_utf16(text, 10), 3, "inside the emoji");
        assert_eq!(byte_to_utf16(text, text.len()), 5);
    }

    #[test]
    fn symbol_range_counts_utf16_units() {
        let text = "int 数值;";
        let range = symbol_range(text, 4, "数值");
        assert_eq!(range.start.column, 4);
        assert_eq!(range.end.column, 6, "name width must be UTF-16 code units");

        let text = "int a😀b;";
        let range = symbol_range(text, 4, "a😀b");
        assert_eq!(range.start.column, 4);
        assert_eq!(range.end.column, 8, "emoji is two UTF-16 code units");
    }
}
