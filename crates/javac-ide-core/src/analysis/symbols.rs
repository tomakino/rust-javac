//! Symbols of the analyzed file and positional lookup.

use std::path::Path;
use std::path::PathBuf;

use crate::code::scope::LookupKind;
use crate::code::symbol::{symbol_packge, SymbolKind};
use crate::ids::{SymbolId, TreeId};
use crate::util::name::Name;
use super::positions::{line_offset, word_at};
use super::type_names::type_name;
use super::{canonical, AnalysisSession};

/// One symbol of the analyzed file (classes and their direct members).
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: &'static str,
    pub flat_name: String,
    /// Declaration offset in the source text (NOPOS when unknown).
    pub pos: i32,
    /// Hover text (kind, type/signature, qualified name).
    pub detail: String,
    /// Javadoc text of the declaration, when present.
    pub doc: Option<String>,
}


pub(crate) struct TracedClass {
    pub(crate) sym: SymbolId,
    flat_name: Name,
    package: Option<SymbolId>,
}

impl AnalysisSession {
    pub(crate) fn trace_classes_for(&mut self, source: &Path) {
        self.traced.clear();
        let source_name = canonical(source);
        let classes = self.compiler.syms.get_all_classes();
        for sym in classes {
            let matching = match &self.compiler.ctx.sym(sym).kind_data {
                SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
                    .as_ref()
                    .map(|file| canonical(Path::new(&file.get_name().to_string_lossy())))
                    .map(|name| name == source_name)
                    .unwrap_or(false),
                _ => false,
            };
            if matching {
                let flat_name = self.compiler.ctx.sym(sym).flat_name();
                let package = symbol_packge(&self.compiler.ctx, sym);
                self.traced.push(TracedClass {
                    sym,
                    flat_name,
                    package,
                });
            }
        }
    }

    /// The symbol at a source position (1-based line and column).
    ///
    /// The word under the cursor is read from the file text and matched
    /// against the analyzed symbols; this is the base for go-to-definition and
    /// hover.
    pub fn symbol_at(&mut self, source: &Path, line: usize, column: usize) -> Option<SymbolInfo> {
        let key = PathBuf::from(canonical(source));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(source).ok())?;
        let line_text = text.lines().nth(line.saturating_sub(1))?;
        let chars: Vec<char> = line_text.chars().collect();
        if chars.is_empty() {
            return None;
        }
        let mut col = super::positions::char_index_for_utf16(&chars, column.saturating_sub(1));
        let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
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
        let word: String = chars[start..=end].iter().collect();
        if word.is_empty() {
            return None;
        }
        let cursor = line_offset(&text, line, start + 1) as i64;
        self.symbols()
            .into_iter()
            .filter(|symbol| symbol.name == word && symbol.pos >= 0)
            .min_by_key(|symbol| (symbol.pos as i64 - cursor).abs())
            .or_else(|| self.symbols().into_iter().find(|symbol| symbol.name == word))
    }

    /// Symbols of the analyzed file: its classes and their direct members.
    /// This is the base for completion and go-to-definition.
    pub fn symbols(&mut self) -> Vec<SymbolInfo> {
        let docs = self.doc_comments_for_traced();
        let mut decl_trees: std::collections::HashMap<SymbolId, TreeId> =
            std::collections::HashMap::new();
        for (index, tree) in self.compiler.ctx.trees.iter().enumerate() {
            let symbol = match &tree.kind {
                crate::tree::jc_tree::TreeKind::MethodDecl { sym: Some(sym), .. } => *sym,
                crate::tree::jc_tree::TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
                _ => continue,
            };
            decl_trees
                .entry(symbol)
                .or_insert_with(|| TreeId::new(index as u32));
        }
        let mut out = Vec::new();
        for traced in &self.traced {
            let class = &self.compiler.ctx.sym(traced.sym);
            let class_pos = class.pos.get();
            out.push(SymbolInfo {
                name: class.name.to_java_string().to_string_lossy(),
                kind: "class",
                flat_name: traced.flat_name.to_string(),
                pos: class_pos,
                detail: format!("class {}", traced.flat_name),
                doc: docs.get(&traced.sym).cloned(),
            });
            let scope = class.members();
            if let Some(scope) = scope {
                let members = self.compiler.ctx.scope_get_symbols(
                    scope,
                    LookupKind::NON_RECURSIVE,
                    &|_| true,
                );
                for member in members {
                    // Compiler-generated members (default constructor) are not
                    // part of the source outline.
                    if (self.compiler.ctx.sym(member).flags_field
                        & crate::code::flags::GENERATEDCONSTR)
                        != 0
                    {
                        continue;
                    }
                    let sym = &self.compiler.ctx.sym(member);
                    let kind = match sym.kind_data {
                        SymbolKind::MethodSymbol { .. } => "method",
                        SymbolKind::VarSymbol { .. } => "field",
                        SymbolKind::ClassSymbol { .. } => "nested",
                        _ => "member",
                    };
                    let member_pos = match &sym.kind_data {
                        SymbolKind::VarSymbol { pos, .. } => *pos,
                        _ => sym.pos.get(),
                    };
                    let type_text = decl_trees
                        .get(&member)
                        .and_then(|tree| self.member_type_text(*tree))
                        .filter(|text| text != "?");
                    let name = sym.name.to_java_string().to_string_lossy();
                    let detail = match (kind, type_text) {
                        ("method", Some(text)) => {
                            format!("{}.{} {}", traced.flat_name, name, text)
                        }
                        ("method", None) => format!("{}.{}", traced.flat_name, name),
                        (_, Some(text)) => format!("{} {}", text, name),
                        (_, None) => format!("{}.{}", traced.flat_name, name),
                    };
                    out.push(SymbolInfo {
                        name: sym.name.to_java_string().to_string_lossy(),
                        kind,
                        flat_name: traced.flat_name.to_string(),
                        pos: member_pos,
                        detail,
                        doc: docs.get(&member).cloned(),
                    });
                }
            }
        }
        out
    }

    pub(crate) fn remove_traced_classes(&mut self) {
        for traced in std::mem::take(&mut self.traced) {
            self.compiler
                .chk
                .remove_compiled(&self.compiler.ctx, traced.sym);
            if let Some(package) = traced.package {
                if let Some(scope) = self.compiler.ctx.sym(package).members() {
                    self.compiler.ctx.scope_remove(scope, traced.sym);
                }
                self.compiler
                    .syms
                    .remove_class(traced.sym, &traced.flat_name);
            }
        }
    }
}


impl AnalysisSession {
    /// Javadoc text of the declarations of the traced classes, keyed by symbol.
    fn doc_comments_for_traced(&mut self) -> std::collections::HashMap<SymbolId, String> {
        let source_names: Vec<String> = self
            .traced
            .iter()
            .filter_map(|traced| match &self.compiler.ctx.sym(traced.sym).kind_data {
                SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
                    .as_ref()
                    .map(|file| canonical(Path::new(&file.get_name().to_string_lossy()))),
                _ => None,
            })
            .collect();
        let table = self.compiler.ctx.trees.iter().find_map(|tree| {
            if let crate::tree::jc_tree::TreeKind::CompilationUnit {
                sourcefile,
                doc_comments,
                ..
            } = &tree.kind
            {
                let file = sourcefile.as_ref()?.get_name().to_string_lossy();
                if source_names.iter().any(|name| name == &canonical(Path::new(&file))) {
                    return Some(doc_comments.clone());
                }
            }
            None
        });
        let Some(Some(table)) = table else {
            return std::collections::HashMap::new();
        };
        let mut docs: std::collections::HashMap<SymbolId, String> = std::collections::HashMap::new();
        let mut tree_ids: Vec<(TreeId, SymbolId)> = Vec::new();
        for (index, tree) in self.compiler.ctx.trees.iter().enumerate() {
            let symbol = match &tree.kind {
                crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
                crate::tree::jc_tree::TreeKind::MethodDecl { sym: Some(sym), .. } => *sym,
                crate::tree::jc_tree::TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
                _ => continue,
            };
            tree_ids.push((TreeId::new(index as u32), symbol));
        }
        for (tree_id, symbol) in tree_ids {
            let rendered = {
                let mut table_ref = table.borrow_mut();
                table_ref
                    .get_comment_tree(&mut self.compiler.ctx, tree_id)
                    .and_then(|doc| super::doc_render::format_doc_comment(&doc))
                    .or_else(|| {
                        table_ref
                            .get_comment_text(tree_id)
                            .map(|text| text.to_string_lossy())
                    })
            };
            if let Some(rendered) = rendered {
                docs.insert(symbol, rendered);
            }
        }
        docs
    }
}


impl AnalysisSession {
    /// Type text of a member declaration tree (field type or method signature).
    fn member_type_text(&self, tree: TreeId) -> Option<String> {
        let ctx = &self.compiler.ctx;
        match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::VariableDecl { vartype, .. } => {
                vartype.and_then(|v| ctx.tree(v).ty).map(|ty| type_name(ctx, ty))
            }
            crate::tree::jc_tree::TreeKind::MethodDecl {
                params,
                restype,
                ..
            } => {
                let args = params
                    .iter()
                    .filter_map(|param| match &ctx.tree(param).kind {
                        crate::tree::jc_tree::TreeKind::VariableDecl { vartype, .. } => vartype
                            .and_then(|v| ctx.tree(v).ty)
                            .map(|ty| type_name(ctx, ty)),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let ret = restype
                    .and_then(|r| ctx.tree(r).ty)
                    .map(|ty| type_name(ctx, ty))
                    .unwrap_or_else(|| "void".to_string());
                Some(format!("({args}){ret}"))
            }
            _ => None,
        }
    }
}
