//! Unified end-position access.
//!
//! The Java compiler keeps tree ends in an `EndPosTable`; in this port the
//! parser records ends for the declarations it visits and `TreeInfo::end_pos`
//! is authoritative only for blocks/switch/try. This module is the single
//! fallback entry point: it prefers the parser table, then `TreeInfo`,
//! then one masked-text structural scan (cached per text hash and tree).

use std::cell::RefCell;
use std::collections::HashMap;

use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::tree::end_pos_table::EndPosTable;
use crate::tree::tree_info::{end_pos, get_end_pos};

thread_local! {
    static CACHE: RefCell<HashMap<(u64, u32), i32>> = RefCell::new(HashMap::new());
    /// Fillable per-text end-position tables. Computed ends are recorded here
    /// (the single text-fill implementation) so later lookups hit the table.
    static TABLES: RefCell<
        HashMap<u64, std::rc::Rc<crate::tree::end_pos_table::MapEndPosTable>>,
    > = RefCell::new(HashMap::new());
}

/// End (UTF-16 offset) of `tree`, preferring (in order):
/// 1. the parser's end-position table (`unit.end_positions`),
/// 2. `TreeInfo::get_end_pos` (recursive for many node kinds),
/// 3. `TreeInfo::end_pos` (blocks/switch/try),
/// 4. one masked-text structural scan (the only text fallback, cached).
pub(crate) fn end_position(
    ctx: &CompilerContext,
    text: &str,
    tree: TreeId,
    table: Option<&dyn EndPosTable>,
) -> i32 {
    let hash = hash_text(text);
    let fillable = TABLES.with(|tables| {
        let mut tables = tables.borrow_mut();
        std::rc::Rc::clone(tables.entry(hash).or_insert_with(|| {
            std::rc::Rc::new(crate::tree::end_pos_table::MapEndPosTable::new())
        }))
    });
    let filled = fillable.get_end_pos(tree);
    if filled > 0 {
        return filled;
    }
    let known = get_end_pos(ctx, Some(tree), table);
    if known > 0 {
        fillable.store_end(tree, known);
        return known;
    }
    let authoritative = end_pos(ctx, tree);
    if authoritative > 0 {
        fillable.store_end(tree, authoritative);
        return authoritative;
    }
    let key = (hash, tree.index() as u32);
    if let Some(cached) = CACHE.with(|cache| cache.borrow().get(&key).copied()) {
        fillable.store_end(tree, cached);
        return cached;
    }
    let end = scan_end(ctx, text, tree);
    fillable.store_end(tree, end);
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

/// The single structural text fallback: scan the masked text for the end of
/// the node (`;` at depth 0 or the matching `}`).
fn scan_end(ctx: &CompilerContext, text: &str, tree: TreeId) -> i32 {
    let start_utf16 = ctx.tree(tree).pos.max(0) as usize;
    let start_byte = crate::analysis::positions::utf16_to_byte_snapped(text, start_utf16);
    let start_char = text[..start_byte].chars().count();
    let masked = crate::analysis::inspections::mask_non_code(text);
    let mut depth = 0usize;
    for (masked_char, (byte, original)) in masked
        .chars()
        .skip(start_char)
        .zip(text.char_indices().skip(start_char))
    {
        let _ = original;
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
