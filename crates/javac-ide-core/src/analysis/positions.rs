//! LSP position conversions (UTF-16 columns).

use std::path::{Path, PathBuf};

/// Platform-tolerant comparison key for a file path.
///
/// Index records hold `canonicalize`d paths (`\\?\C:\...` on Windows); editor
/// paths are plain absolute paths. `canonicalize` is slow on Windows, so
/// compare the normalized text instead (prefix, separators and case).
pub(crate) fn normalized_path_key(path: &Path) -> String {
    let text = path.to_string_lossy().replace('/', "\\");
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    text.to_lowercase()
}

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
fn name_start_in(text: &str, start: u32, name: &str, reject_qualified: bool) -> u32 {
    if name.is_empty() {
        return start;
    }
    let start_byte = utf16_to_byte_snapped(text, start as usize);
    let Some(tail) = text.get(start_byte..) else {
        return start;
    };
    let mut search = 0usize;
    let is_ident = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '$';
    while let Some(position) = tail[search..].find(name) {
        let absolute = start_byte + search + position;
        let after = absolute + name.len();
        let before_ok = text[..absolute]
            .chars()
            .next_back()
            .map_or(true, |ch| !is_ident(ch));
        let after_ok = text[after..]
            .chars()
            .next()
            .map_or(true, |ch| !is_ident(ch));
        let qualified = reject_qualified && text[..absolute].trim_end().ends_with('.');
        if before_ok && after_ok && !qualified {
            return byte_to_utf16(text, absolute) as u32;
        }
        search += position + name.len();
    }
    start
}

/// Start of a member name (`a.b`): the qualifier dot is expected here.
pub(crate) fn member_name_start(text: &str, start: u32, name: &str) -> u32 {
    name_start_in(text, start, name, false)
}


/// Whether the byte offset is inside a string/character literal or a comment.
pub(crate) fn in_literal_or_comment(text: &str, byte: usize) -> bool {
    let byte = byte.min(text.len());
    let char_index = text[..byte].chars().count();
    let masked = crate::analysis::inspections::mask_non_code(text);
    let original = text.chars().nth(char_index);
    let masked_char = masked.chars().nth(char_index);
    matches!((original, masked_char), (Some(original), Some(masked)) if !original.is_whitespace() && masked == ' ')
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

/// AST name position of a symbol's declaration: parser name table (the
/// `CompilationUnit.name_positions` published by the parser), then
/// `VariableDecl.decl_pos`, then the declaring tree position.
///
/// Returns `None` when the symbol is not declared in the current arena
/// (class-path / JDK symbols, which have no source tree here).
pub(crate) fn name_start_of_symbol(
    ctx: &crate::compiler_context::CompilerContext,
    symbol: crate::ids::SymbolId,
) -> Option<u32> {
    // `target` may be NOPOS for parameters; the declaration tree is matched by
    // `sym`/`decl_pos` below, so a missing position is not fatal.
    let target = ctx.sym(symbol).pos.get();
    let mut name_tables: Vec<std::rc::Rc<dyn crate::tree::end_pos_table::EndPosTable>> = Vec::new();
    for index in 0..ctx.trees.len() {
        let unit = crate::ids::TreeId::new(index as u32);
        if let crate::tree::jc_tree::TreeKind::CompilationUnit {
            name_positions: Some(table),
            ..
        } = &ctx.tree(unit).kind
        {
            name_tables.push(std::rc::Rc::clone(table));
        }
    }
    // The declaring tree is identified by its `sym` (unique per declaration);
    // `VariableDecl` carries the name token in `decl_pos` (`sym.pos` may point
    // at the type instead), class/method symbols point at the declaration.
    let mut candidate = None;
    for index in 0..ctx.trees.len() {
        let tree = crate::ids::TreeId::new(index as u32);
        let owner = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::ClassDecl {
                sym: Some(owner), ..
            }
            | crate::tree::jc_tree::TreeKind::MethodDecl {
                sym: Some(owner), ..
            }
            | crate::tree::jc_tree::TreeKind::VariableDecl {
                sym: Some(owner), ..
            } => *owner,
            _ => continue,
        };
        if owner == symbol {
            candidate = Some(tree);
            break;
        }
    }
    let tree = candidate?;
    let mut name_tables: Vec<std::rc::Rc<dyn crate::tree::end_pos_table::EndPosTable>> =
        Vec::new();
    for index in 0..ctx.trees.len() {
        let unit = crate::ids::TreeId::new(index as u32);
        if let crate::tree::jc_tree::TreeKind::CompilationUnit {
            name_positions: Some(table),
            ..
        } = &ctx.tree(unit).kind
        {
            name_tables.push(std::rc::Rc::clone(table));
        }
    }
    for table in &name_tables {
        let position = table.get_end_pos(tree);
        if position > 0 {
            return Some(position as u32);
        }
    }
    if let crate::tree::jc_tree::TreeKind::VariableDecl { decl_pos, .. } = &ctx.tree(tree).kind
        && *decl_pos > 0
    {
        return Some(*decl_pos as u32);
    }
    Some(ctx.tree(tree).pos.max(0) as u32)
}
#[cfg(test)]
mod name_start_tests {
    use super::name_start_of_symbol;
    use crate::analysis::AnalysisConfig;
    use crate::tree::jc_tree::TreeKind;
    use crate::workspace::analysis::FileAnalysis;

    #[test]
    fn parameter_has_an_ast_name_start() {
        if std::env::var_os("RAVAC_PLATFORM_DIR").is_none() {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("target")
                .join("debug");
            unsafe {
                std::env::set_var("RAVAC_PLATFORM_DIR", &root);
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("tmp")
            .join("name-start-unit");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("A.java");
        let text = "class A { void f(String lock) { String x = lock; } }\n";
        std::fs::write(&path, text).unwrap();
        let config = AnalysisConfig {
            class_path: Vec::new(),
            output_dir: root.join("out"),
            source_level: "17".to_string(),
            source_path: vec![root.clone()],
        };
        let mut analysis = FileAnalysis::new(config);
        analysis.texts.insert(path.clone(), text.to_string());
        analysis.compiler.overlay_text = Some((path.clone(), text.to_string()));
        let _ = analysis.compiler.compile_files(&[path.clone()]);
        let mut checked = 0;
        for index in 0..analysis.compiler.ctx.trees.len() {
            let tree = crate::ids::TreeId::new(index as u32);
            if let TreeKind::VariableDecl {
                sym: Some(symbol),
                decl_pos,
                mods: Some(mods),
                ..
            } = &analysis.compiler.ctx.tree(tree).kind
            {
                let flags_value = match &analysis.compiler.ctx.tree(*mods).kind {
                    TreeKind::Modifiers { flags, .. } => *flags,
                    _ => 0,
                };
                if flags_value & crate::code::flags::PARAMETER != 0 {
                    eprintln!(
                        "[P] sym_pos={} tree_pos={} decl_pos={:?}",
                        analysis.compiler.ctx.sym(*symbol).pos.get(),
                        analysis.compiler.ctx.tree(tree).pos,
                        decl_pos
                    );
                    assert_eq!(
                        name_start_of_symbol(&analysis.compiler.ctx, *symbol),
                        Some(*decl_pos as u32),
                        "parameter must have its AST name start"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 0, "no parameter declarations checked");
    }
}