//! Project symbol index.

use std::path::PathBuf;

use crate::code::scope::LookupKind;
use crate::code::symbol::SymbolKind;
use super::AnalysisSession;

/// A declaration location in the project index.
#[derive(Debug, Clone)]
pub(crate) struct IndexEntry {
    pub(crate) file: PathBuf,
    pub(crate) pos: i32,
    pub(crate) kind: &'static str,
}

impl AnalysisSession {
    /// Index every source-backed class (and its direct members) of the
    /// compiler's symbol table.
    pub(crate) fn build_index(&mut self) {
        self.index.clear();
        let classes = self.compiler.syms.get_all_classes();
        for sym in classes {
            let (flat, sourcefile) = {
                let symbol = self.compiler.ctx.sym(sym);
                match &symbol.kind_data {
                    SymbolKind::ClassSymbol { flatname, sourcefile, .. } => (
                        flatname.to_string(),
                        sourcefile
                            .as_ref()
                            .map(|file| PathBuf::from(file.get_name().to_string_lossy())),
                    ),
                    _ => continue,
                }
            };
            let Some(file) = sourcefile else { continue };
            let pos = self.compiler.ctx.sym(sym).pos.get();
            self.index.insert(
                flat.clone(),
                IndexEntry {
                    file: file.clone(),
                    pos,
                    kind: "class",
                },
            );
            if let Some(scope) = self.compiler.ctx.sym(sym).members() {
                let members = self.compiler.ctx.scope_get_symbols(
                    scope,
                    LookupKind::NON_RECURSIVE,
                    &|_| true,
                );
                for member in members {
                    let member_symbol = self.compiler.ctx.sym(member);
                    let name = member_symbol.name.to_java_string().to_string_lossy();
                    let kind = match member_symbol.kind_data {
                        SymbolKind::MethodSymbol { .. } => "method",
                        SymbolKind::VarSymbol { .. } => "field",
                        SymbolKind::ClassSymbol { .. } => "nested",
                        _ => "member",
                    };
                    self.index.insert(
                        format!("{flat}.{name}"),
                        IndexEntry {
                            file: file.clone(),
                            pos: member_symbol.pos.get(),
                            kind,
                        },
                    );
                }
            }
        }
    }

    /// Declaration location of a symbol by its index key.
    pub(crate) fn index_lookup(&self, key: &str) -> Option<(PathBuf, i32)> {
        self.index.get(key).map(|entry| (entry.file.clone(), entry.pos))
    }

    pub(crate) fn ensure_index(&mut self) {
        if !self.index_built {
            self.build_index();
            self.index_built = true;
        }
    }
}

