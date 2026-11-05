//! Symbols of the analyzed file and positional lookup.

use std::path::Path;
use std::path::PathBuf;

use crate::code::scope::LookupKind;
use crate::code::symbol::{symbol_packge, SymbolKind};
use crate::ids::SymbolId;
use crate::util::name::Name;
use super::positions::{line_offset, word_at};
use super::{canonical, AnalysisSession};

/// One symbol of the analyzed file (classes and their direct members).
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: &'static str,
    pub flat_name: String,
    /// Declaration offset in the source text (NOPOS when unknown).
    pub pos: i32,
}


pub(crate) struct TracedClass {
    sym: SymbolId,
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
        let mut col = column.saturating_sub(1).min(chars.len() - 1);
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
        let mut out = Vec::new();
        for traced in &self.traced {
            let class = &self.compiler.ctx.sym(traced.sym);
            let class_pos = class.pos.get();
            out.push(SymbolInfo {
                name: class.name.to_java_string().to_string_lossy(),
                kind: "class",
                flat_name: traced.flat_name.to_string(),
                pos: class_pos,
            });
            let scope = class.members();
            if let Some(scope) = scope {
                let members = self.compiler.ctx.scope_get_symbols(
                    scope,
                    LookupKind::NON_RECURSIVE,
                    &|_| true,
                );
                for member in members {
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
                    out.push(SymbolInfo {
                        name: sym.name.to_java_string().to_string_lossy(),
                        kind,
                        flat_name: traced.flat_name.to_string(),
                        pos: member_pos,
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
