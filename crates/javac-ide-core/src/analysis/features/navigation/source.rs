//! Shared source/tree helpers for navigation queries.

use std::path::{Path, PathBuf};

use javac_analysis_api::Range;

use super::super::super::positions::{
    byte_to_utf16, line_offset, offset_to_line_col, word_at,
};
use crate::analysis::features::import_syntax::{
    ImportSyntax, StaticImportSyntax,
};
use super::super::super::{canonical, FileAnalysis};
use super::references::ReferenceCollector;
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
        // Declarations win over reference attribution: enum constants are
        // generated as constructor calls whose identifier is attributed to the
        // enum type, while the useful hover symbol is the constant itself.
        // Some declarations (enum constants) have no published tree position,
        // so their name is located from the declaration tree in the text.
        for index in 0..self.compiler.ctx.trees.len() {
            if let Some(symbol) = declaration_name_at(
                &self.compiler.ctx,
                &text,
                crate::ids::TreeId::new(index as u32),
                start,
            ) {
                return Some((symbol, start));
            }
        }
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
        // A method reference's target is not visited as an identifier: the
        // `MemberReference` tree carries it (with `A::new` stored as `<init>`).
        if let Some((symbol, position)) =
            member_reference_symbol_at(&self.compiler.ctx, &text, &word, start)
        {
            return Some((symbol, position));
        }
        // A static import's last segment is resolved into the import scope
        // rather than onto the qualifier tree (its `FieldAccess.sym` stays
        // unset), so attribute it from the selected origin type's members.
        if let Some((symbol, position)) =
            static_import_member_at(&mut self.compiler.ctx, &text, &word, start, line)
        {
            return Some((symbol, position));
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

    
    /// Decode the package/import declarations from the AST.
    pub(crate) fn import_syntax_impl(&self, file: &Path) -> ImportSyntax {
        let mut syntax = ImportSyntax::default();
        let Some(root) = self.latest_root_for(file) else {
            return syntax;
        };
        let crate::tree::jc_tree::TreeKind::CompilationUnit {
            defs,
            end_positions,
            ..
        } = &self.compiler.ctx.tree(root).kind
        else {
            return syntax;
        };
        let end_positions = end_positions.as_deref();
        let mut end = 0usize;
        for definition in defs.iter() {
            let node = self.compiler.ctx.tree(definition);
            let node_end = crate::tree::tree_info::get_end_pos(
                &self.compiler.ctx,
                Some(definition),
                end_positions,
            );
            match &node.kind {
                crate::tree::jc_tree::TreeKind::PackageDecl { pid: Some(pid), .. } => {
                    if node_end >= 0 {
                        end = end.max(node_end as usize);
                    }
                    syntax.package = import_qualifier_parts(
                        &self.compiler.ctx,
                        *pid,
                        end_positions,
                    )
                    .into_iter()
                    .map(|(name, ..)| name)
                    .collect::<Vec<_>>()
                    .join(".");
                }
                crate::tree::jc_tree::TreeKind::Import {
                    qualid: Some(qualid),
                    static_import,
                    ..
                } => {
                    if node_end >= 0 {
                        end = end.max(node_end as usize);
                    }
                    let parts = import_qualifier_parts(
                        &self.compiler.ctx,
                        *qualid,
                        end_positions,
                    );
                    if parts.is_empty() {
                        continue;
                    }
                    let names: Vec<&str> =
                        parts.iter().map(|(name, ..)| name.as_str()).collect();
                    let wildcard = names.last() == Some(&"*");
                    let names = if wildcard {
                        &names[..names.len() - 1]
                    } else {
                        &names[..]
                    };
                    if *static_import {
                        let (type_names, member) = if wildcard {
                            (names, None)
                        } else if names.len() > 1 {
                            (&names[..names.len() - 1], Some(names[names.len() - 1].to_string()))
                        } else {
                            continue;
                        };
                        syntax.static_imports.push(StaticImportSyntax {
                            type_path: type_names.join("."),
                            member,
                            wildcard,
                        });
                    } else if wildcard {
                        syntax.wildcard.push(names.join("."));
                    } else {
                        syntax.imports.push(names.join("."));
                    }
                }
                _ => {}
            }
        }
        syntax.end = end;
        syntax
    }

    /// AST spans of type positions that make the cursor a type context.
    pub(crate) fn context_type_ranges_impl(&self, file: &Path) -> Vec<(u32, u32)> {
        let Some(root) = self.latest_root_for(file) else {
            return Vec::new();
        };
        let crate::tree::jc_tree::TreeKind::CompilationUnit { end_positions, .. } =
            &self.compiler.ctx.tree(root).kind
        else {
            return Vec::new();
        };
        let end_positions = end_positions.as_deref();
        let mut ranges = Vec::new();
        let mut push = |tree: crate::ids::TreeId| {
            let start = self.compiler.ctx.tree(tree).pos;
            let end = crate::tree::tree_info::get_end_pos(
                &self.compiler.ctx,
                Some(tree),
                end_positions,
            );
            if start >= 0 && end >= start {
                ranges.push((start as u32, end as u32));
            }
        };
        for tree in self.compiler.ctx.trees.iter() {
            match &tree.kind {
                crate::tree::jc_tree::TreeKind::ClassDecl {
                    extending,
                    implementing,
                    permitting,
                    ..
                } => {
                    if let Some(extending) = extending {
                        push(*extending);
                    }
                    for interface in implementing.iter().chain(permitting.iter()) {
                        push(interface);
                    }
                }
                crate::tree::jc_tree::TreeKind::MethodDecl { restype, .. } => {
                    if let Some(restype) = restype {
                        push(*restype);
                    }
                }
                crate::tree::jc_tree::TreeKind::NewClass { clazz, .. }
                | crate::tree::jc_tree::TreeKind::TypeCast { clazz, .. } => {
                    if let Some(clazz) = clazz {
                        push(*clazz);
                    }
                }
                crate::tree::jc_tree::TreeKind::InstanceOf { pattern, .. } => {
                    if let Some(pattern) = pattern {
                        push(*pattern);
                    }
                }
                crate::tree::jc_tree::TreeKind::VariableDecl { vartype, .. } => {
                    if let Some(vartype) = vartype {
                        push(*vartype);
                    }
                }
                _ => {}
            }
        }
        ranges
    }

    /// AST spans of annotation nodes in the current compilation unit.
    pub(crate) fn annotation_ranges_impl(&self, file: &Path) -> Vec<(u32, u32)> {
        let Some(root) = self.latest_root_for(file) else {
            return Vec::new();
        };
        let crate::tree::jc_tree::TreeKind::CompilationUnit { end_positions, .. } =
            &self.compiler.ctx.tree(root).kind
        else {
            return Vec::new();
        };
        let end_positions = end_positions.as_deref();
        self.compiler
            .ctx
            .trees
            .iter()
            .enumerate()
            .filter_map(|(index, tree)| {
                if !matches!(tree.kind, crate::tree::jc_tree::TreeKind::Annotation { .. }) {
                    return None;
                }
                let tree = crate::ids::TreeId::new(index as u32);
                let start = self.compiler.ctx.tree(tree).pos;
                let end = crate::tree::tree_info::get_end_pos(
                    &self.compiler.ctx,
                    Some(tree),
                    end_positions,
                );
                (start >= 0 && end >= start).then_some((start as u32, end as u32))
            })
            .collect()
    }

    /// AST spans of all import declarations in the current compilation unit.
    pub(crate) fn import_ranges_impl(&self, file: &Path) -> Vec<(u32, u32)> {
        let Some(root) = self.latest_root_for(file) else {
            return Vec::new();
        };
        let crate::tree::jc_tree::TreeKind::CompilationUnit {
            defs,
            end_positions,
            ..
        } = &self.compiler.ctx.tree(root).kind
        else {
            return Vec::new();
        };
        let end_positions = end_positions.as_deref();
        defs.iter()
            .filter(|definition| {
                matches!(
                    self.compiler.ctx.tree(*definition).kind,
                    crate::tree::jc_tree::TreeKind::Import { .. }
                )
            })
            .filter_map(|definition| {
                let start = self.compiler.ctx.tree(definition).pos;
                let end = crate::tree::tree_info::get_end_pos(
                    &self.compiler.ctx,
                    Some(definition),
                    end_positions,
                );
                (start >= 0 && end >= start).then_some((start as u32, end as u32))
            })
            .collect()
    }

    /// Document links for import declarations. `Import.qualid` supplies the
    /// class/package AST; `get_end_pos` supplies each name's exact range.
    pub(crate) fn import_links_impl(&self, file: &Path) -> Vec<(Range, String)> {
        let Some(text) = self.source_text(file) else {
            return Vec::new();
        };
        let Some(root) = self.latest_root_for(file) else {
            return Vec::new();
        };
        let crate::tree::jc_tree::TreeKind::CompilationUnit {
            defs,
            end_positions,
            ..
        } = &self.compiler.ctx.tree(root).kind
        else {
            return Vec::new();
        };
        let end_positions = end_positions.as_deref();
        let mut links = Vec::new();
        for definition in defs.iter() {
            let crate::tree::jc_tree::TreeKind::Import {
                qualid: Some(qualid),
                static_import,
                ..
            } = &self.compiler.ctx.tree(definition).kind
            else {
                continue;
            };
            let mut parts = import_qualifier_parts(&self.compiler.ctx, *qualid, end_positions);
            if parts.is_empty() {
                continue;
            }
            let wildcard = parts.last().is_some_and(|(name, ..)| name == "*");
            if wildcard {
                parts.pop();
            }
            if *static_import && !wildcard && parts.len() > 1 {
                parts.pop();
            }
            let Some((_, start, end)) = parts.last() else {
                continue;
            };
            let class = parts
                .iter()
                .map(|(name, ..)| name.as_str())
                .collect::<Vec<_>>()
                .join(".");
            if class.is_empty() {
                continue;
            }
            let (start_line, start_column) = offset_to_line_col(&text, *start);
            let (end_line, end_column) = offset_to_line_col(&text, *end);
            links.push((
                Range::new(
                    javac_analysis_api::Position::new(start_line, start_column),
                    javac_analysis_api::Position::new(end_line, end_column),
                ),
                class,
            ));
        }
        links
    }

    /// Map a cursor anywhere inside an `Import` AST node to the imported
    /// simple name. The import span and the final qualifier name position both
    /// come from AST positions/end positions; no line prefix is inspected.
    pub(crate) fn import_target_position_impl(
        &self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::Position> {
        let text = self.source_text(file)?;
        let root = self.latest_root_for(file)?;
        let crate::tree::jc_tree::TreeKind::CompilationUnit {
            defs,
            end_positions,
            ..
        } = &self.compiler.ctx.tree(root).kind
        else {
            return None;
        };
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        let cursor = byte_to_utf16(&text, line_offset(&text, line, column));
        let end_positions = end_positions.as_deref();
        for definition in defs.iter() {
            let crate::tree::jc_tree::TreeKind::Import { qualid, .. } =
                &self.compiler.ctx.tree(definition).kind
            else {
                continue;
            };
            let start = self.compiler.ctx.tree(definition).pos;
            let end = crate::tree::tree_info::get_end_pos(
                &self.compiler.ctx,
                Some(definition),
                end_positions,
            );
            if start < 0 || end < 0 || cursor < start as usize || cursor > end as usize {
                continue;
            }
            let Some(qualid) = qualid else {
                continue;
            };
            let (name, name_end) = final_qualifier_name(
                &self.compiler.ctx,
                *qualid,
                end_positions,
            )?;
            if name == "*" || name.starts_with('<') || name_end < name.len() {
                continue;
            }
            let simple_start = name_end - name.len();
            let (target_line, target_column) =
                offset_to_line_col(&text, simple_start);
            if target_line == position.line && target_column == position.column {
                return None;
            }
            return Some(javac_analysis_api::Position::new(
                target_line,
                target_column,
            ));
        }
        None
    }
}

/// Flatten an import `qualid` into `(segment, start, end)` AST ranges.
fn import_qualifier_parts(
    ctx: &crate::compiler_context::CompilerContext,
    qualid: crate::ids::TreeId,
    end_positions: Option<&dyn crate::tree::end_pos_table::EndPosTable>,
) -> Vec<(String, usize, usize)> {
    let end = crate::tree::tree_info::get_end_pos(ctx, Some(qualid), end_positions);
    if end < 0 {
        return Vec::new();
    }
    match &ctx.tree(qualid).kind {
        crate::tree::jc_tree::TreeKind::Ident { name, .. } => {
            let name = name.to_java_string().to_string_lossy().to_string();
            let length = name.encode_utf16().count();
            (end as usize >= length)
                .then(|| vec![(name, end as usize - length, end as usize)])
                .unwrap_or_default()
        }
        crate::tree::jc_tree::TreeKind::FieldAccess {
            selected: Some(selected),
            name,
            ..
        } => {
            let mut parts = import_qualifier_parts(ctx, *selected, end_positions);
            let name = name.to_java_string().to_string_lossy().to_string();
            let length = name.encode_utf16().count();
            if end as usize >= length {
                parts.push((name, end as usize - length, end as usize));
            }
            parts
        }
        _ => Vec::new(),
    }
}

/// Last identifier of an import's qualid AST and its UTF-16 end position.
fn final_qualifier_name(
    ctx: &crate::compiler_context::CompilerContext,
    qualid: crate::ids::TreeId,
    end_positions: Option<&dyn crate::tree::end_pos_table::EndPosTable>,
) -> Option<(String, usize)> {
    let name = match &ctx.tree(qualid).kind {
        crate::tree::jc_tree::TreeKind::Ident { name, .. }
        | crate::tree::jc_tree::TreeKind::FieldAccess { name, .. } => {
            name.to_java_string().to_string_lossy().to_string()
        }
        _ => return None,
    };
    let end = crate::tree::tree_info::get_end_pos(ctx, Some(qualid), end_positions);
    (end >= 0).then_some((name, end as usize))
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
    (crate::analysis::positions::name_start_of_symbol(ctx, symbol)? as usize == start)
        .then_some(symbol)
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
        // The imported member may be inherited from a supertype.
        if let Some(member) = crate::analysis::positions::inherited_member(ctx, origin, word) {
            return Some((member, start));
        }
    }
    None
}

/// Symbol of a method reference under the cursor (`A::method`, `A::new`,
/// `obj::method`).
///
/// The `MemberReference` target is not visited as an identifier, so match its
/// target name against the cursor word; requiring the `::` prefix keeps a
/// plain identifier of the same name from matching.
fn member_reference_symbol_at(
    ctx: &crate::compiler_context::CompilerContext,
    text: &str,
    word: &str,
    start: usize,
) -> Option<(SymbolId, usize)> {
    use crate::tree::jc_tree::ReferenceMode;
    let byte = crate::analysis::positions::utf16_to_byte_snapped(text, start);
    if !text.get(..byte)?.trim_end().ends_with("::") {
        return None;
    }
    let mut found = None;
    let mut ambiguous = false;
    for index in 0..ctx.trees.len() {
        let tree = TreeId::new(index as u32);
        let TreeKind::MemberReference {
            name,
            mode,
            sym: Some(sym),
            ..
        } = &ctx.tree(tree).kind
        else {
            continue;
        };
        let target = if matches!(mode, ReferenceMode::NEW) {
            "new".to_string()
        } else {
            name.to_java_string().to_string_lossy().to_string()
        };
        if target != word {
            continue;
        }
        match found {
            None => found = Some(*sym),
            Some(existing) if existing != *sym => ambiguous = true,
            _ => {}
        }
    }
    if ambiguous { None } else { found.map(|symbol| (symbol, start)) }
}