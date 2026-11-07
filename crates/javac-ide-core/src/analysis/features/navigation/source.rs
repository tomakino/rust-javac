//! Shared source/tree helpers for navigation queries.

use std::path::{Path, PathBuf};

use super::super::super::positions::{line_offset, offset_to_line_col, word_at};
use super::super::super::{canonical, FileAnalysis};
use super::references::ReferenceCollector;
use crate::code::scope::LookupKind;
use crate::code::symbol::SymbolKind;
use crate::ids::{SymbolId, TreeId};
use crate::tree::jc_tree::TreeKind;
use crate::tree::tree_scanner::TreeScannerVisitor;

impl FileAnalysis {
    pub(super) fn source_text(&self, file: &Path) -> Option<String> {
        let key = PathBuf::from(canonical(file));
        self.texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
    }

    /// Symbol attributed at a source position, plus the identifier's UTF-16 start.
    pub(crate) fn symbol_id_at(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<(SymbolId, usize)> {
        let text = self.source_text(file)?;
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        let word = word_at(&text, line, column)?;
        if word.is_empty() {
            return None;
        }
        // `line_offset`/`word_start_offset` work in bytes; compiler tree
        // positions are Java char (UTF-16) offsets.
        let cursor = line_offset(&text, line, column);
        let start_byte = word_start_offset(&text, cursor);
        let start = crate::analysis::positions::byte_to_utf16(&text, start_byte);
        // A reference under the cursor (root may be absent for extracted/JDK
        // sources, in which case the declaration fallback below still applies).
        if let Some(root) = self.latest_root_for(file) {
            let mut collector = ReferenceCollector::new(
                text.clone(),
                file.to_path_buf(),
                None,
                Some(start as i32),
            );
            collector.scan(&mut self.compiler.ctx, Some(root));
            if let Some(symbol) = collector.query {
                return Some((symbol, start));
            }
        }
        // A static import's last segment is resolved into the import scope
        // rather than onto the qualifier tree (its `FieldAccess.sym` stays
        // unset), so attribute it from the selected origin type's members.
        if let Some((symbol, position)) =
            static_import_member_at(&mut self.compiler.ctx, &text, &word, start, line)
        {
            return Some((symbol, position));
        }
        // Declarations have exact symbol positions even when their tree has no
        // reliable token position (for example a field access node).
        for (index, tree) in self.compiler.ctx.trees.iter().enumerate() {
            let symbol = match &tree.kind {
                crate::tree::jc_tree::TreeKind::VariableDecl {
                    sym: Some(sym),
                    decl_pos,
                    ..
                } if *decl_pos == start as i32 => Some(*sym),
                crate::tree::jc_tree::TreeKind::MethodDecl { sym: Some(sym), .. }
                | crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. } => {
                    (self.compiler.ctx.sym(*sym).pos.get() == start as i32).then_some(*sym)
                }
                _ => None,
            };
            if let Some(symbol) = symbol {
                return Some((symbol, start));
            }
            // Extracted JDK sources have no reference root and some parser
            // decl positions are not published: locate the declaration name by
            // the tree start plus the symbol name in the text (last resort, the
            // declaration tree itself is the AST evidence).
            if let Some(symbol) = declaration_name_at(
                &self.compiler.ctx,
                &text,
                crate::ids::TreeId::new(index as u32),
                start,
            ) {
                return Some((symbol, start));
            }
        }
        None
    }

    pub(crate) fn latest_root_for(&self, file: &Path) -> Option<crate::ids::TreeId> {
        let key = canonical(file);
        let mut root = None;
        for (index, tree) in self.compiler.ctx.trees.iter().enumerate() {
            if let crate::tree::jc_tree::TreeKind::CompilationUnit { sourcefile, .. } = &tree.kind {
                let matches = sourcefile.as_ref().is_some_and(|source| {
                    canonical(Path::new(&source.get_name().to_string_lossy())) == key
                });
                if matches {
                    root = Some(crate::ids::TreeId::new(index as u32));
                }
            }
        }
        root
    }
}

pub(super) fn word_start_offset(text: &str, cursor: usize) -> usize {
    let mut start = cursor.min(text.len());
    let is_ident = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '$';
    for (index, ch) in text[..start].char_indices().rev() {
        if is_ident(ch) {
            start = index;
        } else {
            break;
        }
    }
    start
}

/// A declaration tree whose name starts at `start`: the tree position plus the
/// symbol name in the source text (used when no parser `decl_pos` is available).
fn declaration_name_at(
    ctx: &crate::compiler_context::CompilerContext,
    text: &str,
    tree_id: crate::ids::TreeId,
    start: usize,
) -> Option<crate::ids::SymbolId> {
    let (symbol, base) = match &ctx.tree(tree_id).kind {
        crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(symbol), .. }
        | crate::tree::jc_tree::TreeKind::MethodDecl { sym: Some(symbol), .. }
        | crate::tree::jc_tree::TreeKind::VariableDecl { sym: Some(symbol), .. } => {
            (*symbol, ctx.tree(tree_id).pos.max(0) as usize)
        }
        _ => return None,
    };
    let name = ctx.sym(symbol).name.to_java_string().to_string_lossy().to_string();
    if name.is_empty() || name.starts_with('<') {
        return None;
    }
    let offset = text.get(base..)?.find(&name)?;
    (base + offset == start).then_some(symbol)
}

/// The member symbol named `word` of an `import static ...` clause on `line`.
///
/// The compiler resolves a static import into the compilation unit's import
/// scope and leaves the qualifier `FieldAccess.sym` unset, so the reference
/// collector cannot attribute the imported name. The qualifier still names the
/// origin type, whose direct members are the import candidates.
fn static_import_member_at(
    ctx: &mut crate::compiler_context::CompilerContext,
    text: &str,
    word: &str,
    start: usize,
    line: usize,
) -> Option<(SymbolId, usize)> {
    for index in 0..ctx.trees.len() {
        let tree = TreeId::new(index as u32);
        let TreeKind::Import {
            qualid: Some(qualid),
            static_import: true,
            ..
        } = &ctx.tree(tree).kind
        else {
            continue;
        };
        let import_pos = ctx.tree(tree).pos;
        if import_pos < 0 {
            continue;
        }
        let (import_line, _) = offset_to_line_col(text, import_pos as usize);
        if import_line as usize + 1 != line {
            continue;
        }
        let TreeKind::FieldAccess {
            selected: Some(selected),
            name,
            ..
        } = &ctx.tree(*qualid).kind
        else {
            continue;
        };
        if name.to_java_string().to_string_lossy() != word {
            continue;
        }
        let Some(origin) = crate::tree::tree_info::symbol(ctx, *selected) else {
            continue;
        };
        if !matches!(ctx.sym(origin).kind_data, SymbolKind::ClassSymbol { .. }) {
            continue;
        }
        let Some(scope) = ctx.sym(origin).members() else {
            continue;
        };
        let members = ctx.scope_get_symbols(scope, LookupKind::NON_RECURSIVE, &|_| true);
        for member in members {
            let matches = match &ctx.sym(member).kind_data {
                SymbolKind::VarSymbol { .. } | SymbolKind::MethodSymbol { .. } => {
                    ctx.sym(member).name.to_java_string().to_string_lossy() == word
                }
                _ => false,
            };
            if matches {
                return Some((member, start));
            }
        }
    }
    None
}
