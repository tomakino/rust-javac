//! Selection ranges: word -> expression -> statement -> block -> member -> unit.
//!
//! The range chain is built from the compiler tree: one DFS records every
//! node's parent plus its UTF-16 range, then the chain is derived by
//! containment (inner to outer). Strings/comments get the plan-approved
//! fallback (word and line only), never a syntactic approximation.
//!
//! Node ends: `TreeInfo.end_pos` is only authoritative for blocks/switches, so
//! the collector derives the end from the children and then closes the
//! remaining structural brackets with a token scan over a masked copy of the
//! text (strings/comments blanked). This mirrors javac's end-position table
//! without inventing syntax: only closing `)`, `]`, `}` and `>` are consumed.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, Range, SelectionRange};

use super::super::positions::{
    byte_to_utf16, declared_name_start, in_literal_or_comment, line_offset, member_name_start,
    offset_to_line_col, utf16_to_byte,
};
use super::super::{canonical, FileAnalysis};
use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::tree::jc_tree::{accept, TreeKind};
use crate::tree::end_pos_table::EndPosTable;
use crate::tree::tree_info::{end_pos, get_start_pos};
use crate::tree::tree_scanner::{TreeScannerVisitor, tree_scanner_visitor_delegate};

/// Cached parent links and UTF-16 ranges for one file text.
pub(crate) struct SelectionCache {
    pub text_hash: u64,
    pub parents: HashMap<TreeId, TreeId>,
    pub ranges: HashMap<TreeId, (i32, i32)>,
    pub extra_ranges: Vec<(i32, i32)>,
}

impl FileAnalysis {
    /// Selection range chains for each position (inner chain first).
    pub(crate) fn selection_ranges_impl(
        &mut self,
        file: &Path,
        positions: &[Position],
    ) -> Vec<SelectionRange> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        if text.is_empty() {
            return Vec::new();
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        let text_hash = hasher.finish();
        let needs_build = self
            .selection_cache
            .get(&key)
            .map_or(true, |cache| cache.text_hash != text_hash);
        if needs_build {
            let Some(root) = self.latest_root_for(file) else {
                return Vec::new();
            };
            let mut collector = ChainCollector::new(text.clone());
            collector.scan(&mut self.compiler.ctx, Some(root));
            self.selection_cache.insert(
                key.clone(),
                SelectionCache {
                    text_hash,
                    parents: collector.parents,
                    ranges: collector.ranges,
                    extra_ranges: collector.extra_ranges,
                },
            );
        }
        let Some(cache) = self.selection_cache.get(&key) else {
            return Vec::new();
        };
        positions
            .iter()
            .map(|position| {
                selection_range_for(
                    &text,
                    *position,
                    &cache.ranges,
                    &cache.extra_ranges,
                )
            })
            .collect()
    }
}

fn selection_range_for(
    text: &str,
    position: Position,
    ranges: &HashMap<TreeId, (i32, i32)>,
    extra_ranges: &[(i32, i32)],
) -> SelectionRange {
    let byte = line_offset(
        text,
        position.line as usize + 1,
        position.column as usize + 1,
    );
    if in_literal_or_comment(text, byte) {
        return fallback_ranges(text, byte);
    }
    let cursor = byte_to_utf16(text, byte) as i32;
    let mut candidates: Vec<(i32, i32)> = ranges
        .values()
        .copied()
        .chain(extra_ranges.iter().copied())
        .filter(|(start, end)| *start >= 0 && *end > *start && *start <= cursor && cursor <= *end)
        .collect();
    if candidates.is_empty() {
        return fallback_ranges(text, byte);
    }
    // Prefer the candidate that starts at the cursor (the token the cursor is
    // on); among equal starts, the smallest span. This keeps `value` ahead of
    // the preceding `int` whose end touches the same offset.
    candidates.sort_by_key(|(start, end)| (std::cmp::Reverse(*start), end - start));
    candidates.dedup();
    let mut chain: Vec<(i32, i32)> = Vec::new();
    for candidate in candidates {
        match chain.last() {
            None => chain.push(candidate),
            Some((last_start, last_end)) => {
                if candidate.0 <= *last_start && candidate.1 >= *last_end {
                    chain.push(candidate);
                }
            }
        }
    }
    if chain.is_empty() {
        return fallback_ranges(text, byte);
    }
    let mut result: Option<SelectionRange> = None;
    for (start, end) in chain.iter().rev() {
        let (start_line, start_column) = offset_to_line_col(text, *start as usize);
        let (end_line, end_column) = offset_to_line_col(text, *end as usize);
        let range = Range {
            start: Position::new(start_line, start_column),
            end: Position::new(end_line, end_column),
        };
        result = Some(SelectionRange {
            range,
            parent: result.map(Box::new),
        });
    }
    result.expect("non-empty chain")
}

/// Plan-approved fallback for strings/comments/blank positions: word and line.
fn fallback_ranges(text: &str, byte: usize) -> SelectionRange {
    let is_ident = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '$';
    let mut start = byte;
    while start > 0 {
        let Some((index, ch)) = text[..start].char_indices().next_back() else {
            break;
        };
        if !is_ident(ch) {
            break;
        }
        start = index;
    }
    let mut end = byte;
    while end < text.len() {
        let Some(ch) = text[end..].chars().next() else {
            break;
        };
        if !is_ident(ch) {
            break;
        }
        end += ch.len_utf8();
    }
    let line_start = text[..byte].rfind('\n').map(|index| index + 1).unwrap_or(0);
    let line_end = text[byte..]
        .find('\n')
        .map(|index| byte + index)
        .unwrap_or(text.len());
    let (word_start_line, word_start_column) =
        offset_to_line_col(text, byte_to_utf16(text, start) as usize);
    let (word_end_line, word_end_column) =
        offset_to_line_col(text, byte_to_utf16(text, end) as usize);
    let (line_start_line, line_start_column) =
        offset_to_line_col(text, byte_to_utf16(text, line_start) as usize);
    let (line_end_line, line_end_column) =
        offset_to_line_col(text, byte_to_utf16(text, line_end) as usize);
    let word = Range {
        start: Position::new(word_start_line, word_start_column),
        end: Position::new(word_end_line, word_end_column),
    };
    let line = Range {
        start: Position::new(line_start_line, line_start_column),
        end: Position::new(line_end_line, line_end_column),
    };
    SelectionRange {
        range: word,
        parent: Some(Box::new(SelectionRange {
            range: line,
            parent: None,
        })),
    }
}

/// UTF-16 offset of a char index.
fn utf16_of_char_index(text: &str, index: usize) -> i32 {
    let mut offset = 0i32;
    for (seen, ch) in text.chars().enumerate() {
        if seen == index {
            break;
        }
        offset += ch.len_utf16() as i32;
    }
    offset
}

/// Char index of a UTF-16 offset.
fn char_index_of_utf16(text: &str, offset: i32) -> Option<usize> {
    let byte = utf16_to_byte(text, offset.max(0) as usize)?;
    Some(text[..byte].chars().count())
}

struct ChainCollector {
    text: String,
    /// A copy of `text` with strings/comments blanked (same char count).
    mask: Vec<char>,
    parents: HashMap<TreeId, TreeId>,
    children: HashMap<TreeId, Vec<TreeId>>,
    ranges: HashMap<TreeId, (i32, i32)>,
    extra_ranges: Vec<(i32, i32)>,
    stack: Vec<TreeId>,
    seen: HashSet<TreeId>,
}

impl ChainCollector {
    fn new(text: String) -> Self {
        let masked = crate::analysis::inspections::mask_non_code(&text);
        Self {
            mask: masked.chars().collect(),
            text,
            parents: HashMap::new(),
            children: HashMap::new(),
            ranges: HashMap::new(),
            extra_ranges: Vec::new(),
            stack: Vec::new(),
            seen: HashSet::new(),
        }
    }

    fn end_of(&self, ctx: &CompilerContext, tree: TreeId) -> i32 {
        let start = self.ranges.get(&tree).map(|range| range.0).unwrap_or(-1);
        let mut end = self
            .children
            .get(&tree)
            .and_then(|children| {
                children
                    .iter()
                    .filter_map(|child| self.ranges.get(child).map(|range| range.1))
                    .max()
            })
            .unwrap_or(-1);
        let known = crate::tree::tree_info::get_end_pos(ctx, Some(tree), None);
        if known > start {
            end = end.max(known);
        }
        if end <= start {
            end = self.token_end(start);
        }
        end
    }

    fn token_end(&self, start: i32) -> i32 {
        if start < 0 {
            return start;
        }
        let Some(byte) = utf16_to_byte(&self.text, start as usize) else {
            return start;
        };
        let rest = &self.text[byte..];
        let Some(first) = rest.chars().next() else {
            return start;
        };
        let is_ident = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '$';
        let mut consumed = first.len_utf8();
        if is_ident(first) {
            for (index, ch) in rest.char_indices().skip(1) {
                if is_ident(ch) {
                    consumed = index + ch.len_utf8();
                } else {
                    break;
                }
            }
        } else if first == '"' || first == '\''
        {
            let quote = first;
            let mut chars = rest.char_indices().skip(1);
            while let Some((index, ch)) = chars.next() {
                if ch == '\\' {
                    let _ = chars.next();
                    continue;
                }
                if ch == quote {
                    consumed = index + ch.len_utf8();
                    break;
                }
            }
        }
        start + rest[..consumed]
            .chars()
            .map(|ch| ch.len_utf16() as i32)
            .sum::<i32>()
    }

    fn close_brackets(&self, start: i32, end: i32) -> i32 {
        let Some(start_char) = char_index_of_utf16(&self.text, start) else {
            return end;
        };
        let Some(end_char) = char_index_of_utf16(&self.text, end) else {
            return end;
        };
        if end_char <= start_char || end_char > self.mask.len() {
            return end;
        }
        let mut depth = (0i32, 0i32, 0i32);
        for ch in &self.mask[start_char..end_char] {
            match ch {
                '(' => depth.0 += 1,
                ')' => depth.0 -= 1,
                '[' => depth.1 += 1,
                ']' => depth.1 -= 1,
                '{' => depth.2 += 1,
                '}' => depth.2 -= 1,
                _ => {}
            }
        }
        if depth.0 <= 0 && depth.1 <= 0 && depth.2 <= 0 {
            return end;
        }
        let mut pending = depth;
        for (index, ch) in self.mask.iter().enumerate().skip(end_char) {
            match ch {
                '(' => pending.0 += 1,
                ')' => {
                    pending.0 -= 1;
                    if pending.0 < 0 {
                        pending.0 = 0;
                    }
                }
                '[' => pending.1 += 1,
                ']' => {
                    pending.1 -= 1;
                    if pending.1 < 0 {
                        pending.1 = 0;
                    }
                }
                '{' => pending.2 += 1,
                '}' => {
                    pending.2 -= 1;
                    if pending.2 < 0 {
                        pending.2 = 0;
                    }
                }
                _ => {}
            }
            if pending.0 <= 0 && pending.1 <= 0 && pending.2 <= 0 {
                return utf16_of_char_index(&self.text, index + 1);
            }
        }
        end
    }

    /// Consume one closing token right after `end` (skipping whitespace).
    fn close_trailing(&self, end: i32, closer: char) -> i32 {
        let Some(mut index) = char_index_of_utf16(&self.text, end) else {
            return end;
        };
        while index < self.mask.len() && self.mask[index].is_whitespace() {
            index += 1;
        }
        if self.mask.get(index) == Some(&closer) {
            return utf16_of_char_index(&self.text, index + 1);
        }
        end
    }

    /// Consume an opener that starts right after `end` (empty argument lists,
    /// empty method parameter lists) and its matching closer.
    fn close_after(&self, end: i32, opener: char, closer: char) -> i32 {
        let Some(mut index) = char_index_of_utf16(&self.text, end) else {
            return end;
        };
        while index < self.mask.len() && self.mask[index].is_whitespace() {
            index += 1;
        }
        if self.mask.get(index) != Some(&opener) {
            return end;
        }
        let mut depth = 0i32;
        while index < self.mask.len() {
            let ch = self.mask[index];
            if ch == opener {
                depth += 1;
            } else if ch == closer {
                depth -= 1;
                if depth == 0 {
                    return utf16_of_char_index(&self.text, index + 1);
                }
            }
            index += 1;
        }
        end
    }

    /// A `>`-closing type (type apply/parameter) or `]`-closing array.
    fn close_type(&self, tree: TreeId, end: i32) -> i32 {
        let _ = tree;
        let Some(mut index) = char_index_of_utf16(&self.text, end) else {
            return end;
        };
        while index < self.mask.len() {
            match self.mask[index] {
                '>' | ']' => {
                    index += 1;
                }
                ' ' | '\t' | '\r' | '\n' => index += 1,
                _ => break,
            }
        }
        utf16_of_char_index(&self.text, index)
    }
}

impl TreeScannerVisitor for ChainCollector {
    fn scan(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) {
        let Some(tree) = tree else {
            return;
        };
        if !self.seen.insert(tree) {
            return;
        }
        if let Some(parent) = self.stack.last() {
            self.parents.insert(tree, *parent);
            self.children.entry(*parent).or_default().push(tree);
        }
        let (start, declared_name, is_type, needs_paren, needs_brace, is_decl, is_member, trailing_closer) = {
            let node = ctx.tree(tree);
            let mut start = node.pos;
            // Declarations start before their name (modifiers/return type);
            // expressions start at their operator. Use the smaller of the
            // raw pos and the semantic start so both cases are exact.
            let semantic = get_start_pos(ctx, tree);
            if semantic >= 0 && (start < 0 || semantic < start) {
                start = semantic;
            }
            // Class/method names are `Name` fields, not trees: record the
            // declared name span so "select class name" has its own layer
            // (the same unambiguous helper the index uses).
            let declared_name = match &node.kind {
                TreeKind::ClassDecl { name, .. }
                | TreeKind::MethodDecl { name, .. }
                | TreeKind::VariableDecl { name, .. }
                | TreeKind::FieldAccess { name, .. } => {
                    Some(name.to_java_string().to_string_lossy())
                }
                _ => None,
            };
            let is_type = matches!(
                node.kind,
                TreeKind::TypeApply { .. }
                    | TreeKind::TypeParameter { .. }
                    | TreeKind::ArrayTypeTree { .. }
            );
            let needs_paren = matches!(
                node.kind,
                TreeKind::MethodInvocation { .. }
                    | TreeKind::NewClass { .. }
                    | TreeKind::MethodDecl { .. }
                    | TreeKind::Annotation { .. }
            );
            let needs_brace = matches!(node.kind, TreeKind::NewArray { .. });
            let trailing_closer = match &node.kind {
                TreeKind::Parens { .. } => Some(')'),
                TreeKind::ArrayAccess { .. } => Some(']'),
                TreeKind::Block { .. } => Some('}'),
                _ => None,
            };
            let is_member = matches!(node.kind, TreeKind::FieldAccess { .. });
            let is_decl = matches!(
                node.kind,
                TreeKind::ClassDecl { .. }
                    | TreeKind::MethodDecl { .. }
                    | TreeKind::VariableDecl { .. }
                    | TreeKind::FieldAccess { .. }
            );
            (
                start,
                declared_name,
                is_type,
                needs_paren,
                needs_brace,
                is_decl,
                is_member,
                trailing_closer,
            )
        };
        let mut name_end = start;
        if let Some(name) = declared_name {
            let name: &str = &name;
            if !name.is_empty() && name != "<init>" && start >= 0 {
                let name_start = if is_member {
                    member_name_start(&self.text, start as u32, name)
                } else {
                    declared_name_start(&self.text, start as u32, name)
                };
                let width = name.encode_utf16().count() as u32;
                self.extra_ranges
                    .push((name_start as i32, (name_start + width) as i32));
                name_end = (name_start + width) as i32;
            }
        }
        self.ranges.insert(tree, (start, start));
        self.stack.push(tree);
        accept(ctx, tree, self);
        self.stack.pop();
        let mut end = self.end_of(ctx, tree);
        if is_decl {
            end = end.max(name_end);
        }
        if is_type {
            end = end.max(self.close_type(tree, end));
        }
        if needs_paren {
            let closed = self.close_after(end, '(', ')');
            end = if closed == end {
                self.close_trailing(end, ')')
            } else {
                closed
            };
        }
        if needs_brace {
            end = end.max(self.close_trailing(end, '}'));
        }
        if let Some(closer) = trailing_closer {
            end = end.max(self.close_trailing(end, closer));
        }
        if let Some(range) = self.ranges.get_mut(&tree) {
            range.1 = end;
        }
    }
}
tree_scanner_visitor_delegate!(ChainCollector);
