//! Unified end-position access and the single text-fill pass.
//!
//! The Java compiler keeps tree ends in an `EndPosTable`; the parser records
//! ends for the declarations it visits and `TreeInfo::end_pos` is
//! authoritative only for blocks/switch/try. This module provides:
//!
//! 1. [`fill_end_positions`]: one post-order walk of a compilation unit that
//!    records `parent = max(own structural end, children ends)` in a
//!    [`MapEndPosTable`]. Class/method/block/variable ends come from one
//!    masked-text structural scan; leaves use `pos + token length`. This is
//!    the port's single text-fill implementation (the missing EndPosTable
//!    filling that javac performs when `-Xjcov` is on).
//! 2. [`end_position`]: table -> `TreeInfo` -> structural fallback, for nodes
//!    looked up before (or outside) a fill.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::tree::end_pos_table::{EndPosTable, MapEndPosTable};
use crate::tree::jc_tree::Tag;
use crate::tree::tree_info::{end_pos, get_end_pos};
use crate::tree::tree_scanner::{ScanEvent, TreeScanner};

thread_local! {
    /// Filled tables per text hash (post-order walk, one per file text).
    static TABLES: RefCell<HashMap<u64, Rc<MapEndPosTable>>> = RefCell::new(HashMap::new());
    /// Fallback single-node results (used before a fill).
    static CACHE: RefCell<HashMap<(u64, u32), i32>> = RefCell::new(HashMap::new());
}

/// Fill the end-position table of `unit` by a post-order walk.
///
/// - children are visited before their parent (leave events);
/// - the parent's end is `max(own structural end, max(child ends))`;
/// - leaves use `pos + token length` (or their structural scan).
pub(crate) fn fill_end_positions(
    ctx: &mut CompilerContext,
    text: &str,
    unit: TreeId,
) -> Rc<MapEndPosTable> {
    let table = Rc::new(MapEndPosTable::new());
    let mut scanner = TreeScanner::default();
    scanner.scan(ctx, Some(unit));
    let events = scanner.take_events();
    let mut stack: Vec<TreeId> = Vec::new();
    for event in events {
        match event {
            ScanEvent::Enter(tree) => stack.push(tree),
            ScanEvent::Leave(tree) => {
                stack.pop();
                // Expression-level end: token/structural closure/child max,
                // without the enclosing statement's trailing semicolon/brace.
                let token = token_end(ctx, text, tree);
                let closure = closing_end(ctx, text, tree);
                let expression = table
                    .get_expression_end(tree)
                    .max(token)
                    .max(closure);
                table.store_expression_end(tree, expression);
                if let Some(parent) = stack.last().copied() {
                    let parent_expression = table.get_expression_end(parent).max(expression);
                    table.store_expression_end(parent, parent_expression);
                }
                // Declaration/statement end: structural semicolon/brace or own
                // token, max with child declaration ends.
                let own = own_end(ctx, text, tree);
                let end = table.get_end_pos(tree).max(own);
                table.store_end(tree, end);
                if let Some(parent) = stack.last().copied() {
                    let parent_end = table.get_end_pos(parent).max(end);
                    table.store_end(parent, parent_end);
                }
            }
        }
    }
    TABLES.with(|tables| {
        tables.borrow_mut().insert(hash_text(text), Rc::clone(&table));
    });
    table
}

/// End (UTF-16 offset) of `tree`, preferring (in order):
/// 1. the end-position table (parser or filled),
/// 2. `TreeInfo::get_end_pos` / `end_pos`,
/// 3. one structural text scan (the only fallback, cached).
pub(crate) fn end_position(
    ctx: &CompilerContext,
    text: &str,
    tree: TreeId,
    table: Option<&dyn EndPosTable>,
) -> i32 {
    if let Some(table) = table {
        let end = table.get_end_pos(tree);
        if end > 0 {
            return end;
        }
    }
    let hash = hash_text(text);
    if let Some(filled) = TABLES.with(|tables| tables.borrow().get(&hash).cloned()) {
        let end = filled.get_end_pos(tree);
        if end > 0 {
            return end;
        }
    }
    let known = get_end_pos(ctx, Some(tree), table);
    if known > 0 {
        return known;
    }
    let authoritative = end_pos(ctx, tree);
    if authoritative > 0 {
        return authoritative;
    }
    let key = (hash, tree.index() as u32);
    if let Some(cached) = CACHE.with(|cache| cache.borrow().get(&key).copied()) {
        return cached;
    }
    let end = scan_end(ctx, text, tree);
    CACHE.with(|cache| {
        cache.borrow_mut().insert(key, end);
    });
    end
}

fn hash_text(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// Expression-level end: `pos + token length` (no trailing semicolon).
fn token_end(ctx: &CompilerContext, text: &str, tree: TreeId) -> i32 {
    let pos = ctx.tree(tree).pos.max(0) as usize;
    let byte = crate::analysis::positions::utf16_to_byte_snapped(text, pos);
    let length = token_len(text, byte);
    crate::analysis::positions::byte_to_utf16(text, byte + length) as i32
}

/// Structural closure for the node's bracket groups (`(`/`[`/`{`),
/// stopping at a depth-0 semicolon or unmatched brace. Returns the end of the
/// last closed group (array initializers have `[...]` then `{...}`).
fn closing_end(ctx: &CompilerContext, text: &str, tree: TreeId) -> i32 {
    // NOPOS/0 nodes must not scan from the file start: they would hit the
    // enclosing class body brace.
    if ctx.tree(tree).pos <= 0 {
        return 0;
    }
    let pos = ctx.tree(tree).pos.max(0) as usize;
    let start_byte = crate::analysis::positions::utf16_to_byte_snapped(text, pos);
    let start_char = text[..start_byte].chars().count();
    let masked = crate::analysis::inspections::mask_non_code(text);
    let mut depth = 0usize;
    let mut last_close = 0i32;
    for (masked_char, (byte, _original)) in masked
        .chars()
        .skip(start_char)
        .zip(text.char_indices().skip(start_char))
    {
        match masked_char {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                if depth == 0 {
                    return last_close;
                }
                depth -= 1;
                if depth == 0 {
                    last_close = crate::analysis::positions::byte_to_utf16(text, byte + 1) as i32;
                }
            }
            ';' if depth == 0 => return last_close,
            _ => {}
        }
    }
    last_close
}

/// A node's own end: structural for containers (class/method/block/variable/
/// import/package), `pos + token length` for leaves, then max with children
/// (the caller bubbles child ends up).
fn own_end(ctx: &CompilerContext, text: &str, tree: TreeId) -> i32 {
    if let Tag::VARDEF = ctx.tree(tree).tag {
        // A parameter declaration (method/lambda) has no trailing `;`;
        // scanning for one would run into the enclosing statement.
        let is_parameter = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::VariableDecl {
                sym: Some(sym), ..
            } => ctx.sym(*sym).flags() & crate::code::flags::PARAMETER as i64 != 0,
            _ => false,
        };
        return if is_parameter {
            token_end(ctx, text, tree)
        } else {
            scan_end(ctx, text, tree)
        };
    }
    match ctx.tree(tree).tag {
        Tag::CLASSDEF | Tag::METHODDEF | Tag::BLOCK | Tag::IMPORT | Tag::PACKAGEDEF => {
            scan_end(ctx, text, tree)
        }
        _ => token_end(ctx, text, tree),
    }
}

/// Byte length of the lexical token starting at `start`.
fn token_len(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    if start >= bytes.len() {
        return 0;
    }
    let first = bytes[start];
    if first == b'"' || first == b'\'' {
        let quote = first;
        let mut index = start + 1;
        let mut escaped = false;
        while index < bytes.len() {
            let value = bytes[index];
            index += 1;
            if escaped {
                escaped = false;
            } else if value == b'\\' {
                escaped = true;
            } else if value == quote {
                break;
            }
        }
        return index - start;
    }
    if first.is_ascii_alphanumeric() || first == b'_' || first == b'$' || first >= 0x80 {
        let mut index = start;
        while index < bytes.len() {
            let value = bytes[index];
            if value.is_ascii_alphanumeric() || value == b'_' || value == b'$' || value >= 0x80 {
                index += 1;
            } else {
                break;
            }
        }
        return index - start;
    }
    1
}

/// The single structural text scan: `;` at depth 0 or the matching `}`.
fn scan_end(ctx: &CompilerContext, text: &str, tree: TreeId) -> i32 {
    let start_utf16 = ctx.tree(tree).pos.max(0) as usize;
    let start_byte = crate::analysis::positions::utf16_to_byte_snapped(text, start_utf16);
    let start_char = text[..start_byte].chars().count();
    let masked = crate::analysis::inspections::mask_non_code(text);
    let mut depth = 0usize;
    for (masked_char, (byte, _original)) in masked
        .chars()
        .skip(start_char)
        .zip(text.char_indices().skip(start_char))
    {
        match masked_char {
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    return crate::analysis::positions::byte_to_utf16(text, byte + 1) as i32;
                }
                depth -= 1;
                if depth == 0 {
                    return crate::analysis::positions::byte_to_utf16(text, byte + 1) as i32;
                }
            }
            ';' if depth == 0 => {
                return crate::analysis::positions::byte_to_utf16(text, byte + 1) as i32;
            }
            _ => {}
        }
    }
    crate::analysis::positions::byte_to_utf16(text, text.len()) as i32
}
