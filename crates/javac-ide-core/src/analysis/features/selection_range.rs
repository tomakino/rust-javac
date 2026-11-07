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
    byte_to_utf16, in_literal_or_comment, line_offset, member_name_start,
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
    pub name_ranges: Vec<(i32, i32)>,
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
            let table = self.end_positions_for(file);
            let name_positions = self.name_positions_for(file);
            let mut collector = ChainCollector::new(text.clone(), table, name_positions);
            collector.scan(&mut self.compiler.ctx, Some(root));
            self.selection_cache.insert(
                key.clone(),
                SelectionCache {
                    text_hash,
                    parents: collector.parents,
                    ranges: collector.ranges,
                    name_ranges: collector.name_ranges,
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
                    &cache.name_ranges,
                )
            })
            .collect()
    }
}

fn selection_range_for(
    text: &str,
    position: Position,
    ranges: &HashMap<TreeId, (i32, i32)>,
    name_ranges: &[(i32, i32)],
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
        .chain(name_ranges.iter().copied())
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

struct ChainCollector {
    text: String,
    /// A copy of `text` with strings/comments blanked (same char count).
    mask: Vec<char>,
    parents: HashMap<TreeId, TreeId>,
    children: HashMap<TreeId, Vec<TreeId>>,
    ranges: HashMap<TreeId, (i32, i32)>,
    name_ranges: Vec<(i32, i32)>,
    stack: Vec<TreeId>,
    seen: HashSet<TreeId>,
    /// Filled end-position table (declaration + expression ends).
    table: Option<std::rc::Rc<crate::tree::end_pos_table::MapEndPosTable>>,
    /// Parser-recorded name token positions (AST name layer).
    name_positions: Option<std::rc::Rc<dyn crate::tree::end_pos_table::EndPosTable>>,
}

impl ChainCollector {
    fn new(
        text: String,
        table: Option<std::rc::Rc<crate::tree::end_pos_table::MapEndPosTable>>,
        name_positions: Option<std::rc::Rc<dyn crate::tree::end_pos_table::EndPosTable>>,
    ) -> Self {
        let masked = crate::analysis::inspections::mask_non_code(&text);
        Self {
            mask: masked.chars().collect(),
            text,
            parents: HashMap::new(),
            children: HashMap::new(),
            ranges: HashMap::new(),
            stack: Vec::new(),
            seen: HashSet::new(),
            table,
            name_positions,
            name_ranges: Vec::new(),
        }
    }

    fn end_of(&self, _ctx: &CompilerContext, tree: TreeId) -> i32 {
        // End comes from the filled end-position table only.
        let Some(table) = &self.table else {
            return -1;
        };
        table.get_expression_end(tree).max(table.get_end_pos(tree))
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
        let (start, declared_name, is_member) = {
            let node = ctx.tree(tree);
            let mut start = node.pos;
            let semantic = get_start_pos(ctx, tree);
            if semantic >= 0 && (start < 0 || semantic < start) {
                start = semantic;
            }
            let declared_name = match &node.kind {
                TreeKind::ClassDecl { name, .. }
                | TreeKind::MethodDecl { name, .. }
                | TreeKind::VariableDecl { name, .. }
                | TreeKind::FieldAccess { name, .. } => {
                    Some(name.to_java_string().to_string_lossy())
                }
                _ => None,
            };
            let is_member = matches!(node.kind, TreeKind::FieldAccess { .. });
            (start, declared_name, is_member)
        };
        // Name layer: parser-recorded name token position (AST) first; the
        // single text helper is only the fallback when the table has no entry
        // (parameters/enum constants are not parser-recorded yet).
        if let Some(name) = declared_name {
            let name: &str = &name;
            if !name.is_empty() && name != "<init>" && start >= 0 {
                let recorded = self.name_positions.as_ref().and_then(|table| {
                    let pos = table.get_end_pos(tree);
                    (pos > 0).then_some(pos as u32)
                });
                let name_start = match recorded {
                    Some(pos) => pos,
                    None => match &ctx.tree(tree).kind {
                        // VariableDecl carries the parser-recorded name position
                        // (fields/locals/parameters).
                        TreeKind::VariableDecl { decl_pos, .. } if *decl_pos > 0 => {
                            *decl_pos as u32
                        }
                        // Non-member declarations without a parser name entry
                        // (type parameters, constructors): the tree position is
                        // the name or the declaration start.
                        _ => {
                            if is_member {
                                member_name_start(&self.text, start as u32, name)
                            } else {
                                ctx.tree(tree).pos.max(0) as u32
                            }
                        }
                    },
                };
                let width = name.encode_utf16().count() as u32;
                self.name_ranges
                    .push((name_start as i32, (name_start + width) as i32));
            }
        }
        self.ranges.insert(tree, (start, start));
        self.stack.push(tree);
        accept(ctx, tree, self);
        self.stack.pop();
        let end = self.end_of(ctx, tree);
        if let Some(range) = self.ranges.get_mut(&tree) {
            range.1 = end;
        }
    }
}
tree_scanner_visitor_delegate!(ChainCollector);
