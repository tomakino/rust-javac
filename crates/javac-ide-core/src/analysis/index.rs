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
    /// Fully qualified names of direct supertypes (extends/implements).
    pub(crate) supertypes: Vec<String>,
}

impl AnalysisSession {
    /// Index every source-backed class (and its direct members) of the
    /// compiler's symbol table.
    pub(crate) fn build_index(&mut self) {
        self.index.clear();
        let classes = self.compiler.syms.get_all_classes();
        for sym in classes {
            let sourcefile = match &self.compiler.ctx.sym(sym).kind_data {
                SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
                    .as_ref()
                    .map(|file| PathBuf::from(file.get_name().to_string_lossy())),
                _ => None,
            };
            if let Some(file) = sourcefile {
                self.index_class(sym, &file);
            }
        }
    }
}

impl AnalysisSession {
    /// Add a class and its direct members to the index.
    fn index_class(&mut self, sym: crate::ids::SymbolId, file: &PathBuf) {
        let flat = match &self.compiler.ctx.sym(sym).kind_data {
            SymbolKind::ClassSymbol { flatname, .. } => flatname.to_string(),
            _ => return,
        };
        let pos = self.compiler.ctx.sym(sym).pos.get();
        let mut supertypes = Vec::new();
        for tree in self.compiler.ctx.trees.iter() {
            if let SymbolKind::ClassSymbol { .. } = &self.compiler.ctx.sym(sym).kind_data
                && let crate::tree::jc_tree::TreeKind::ClassDecl {
                    sym: Some(class_sym),
                    extending,
                    implementing,
                    ..
                } = &tree.kind
                && *class_sym == sym
            {
                let supertrees: Vec<crate::ids::TreeId> = extending
                    .iter()
                    .copied()
                    .chain(implementing.iter())
                    .collect();
                for supertree in supertrees {
                    if let Some(ty) = self.compiler.ctx.tree(supertree).ty {
                        if let crate::code::type_::TypeKindData::ClassType { .. } =
                            &self.compiler.ctx.typ(ty).kind
                            && let Some(tsym) = self.compiler.ctx.typ(ty).tsym
                        {
                            supertypes.push(self.compiler.ctx.sym(tsym).flat_name().to_string());
                        }
                    }
                }
                break;
            }
        }
        self.index.insert(
            flat.clone(),
            IndexEntry {
                file: file.clone(),
                pos,
                kind: "class",
                supertypes,
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
                        supertypes: Vec::new(),
                    },
                );
            }
        }
    }

    /// Replace the index entries of one analyzed file.
    pub(crate) fn update_index_for(&mut self, source: &std::path::Path) {
        let file = PathBuf::from(super::canonical(source));
        self.index.retain(|_, entry| entry.file != file);
        let symbols: Vec<crate::ids::SymbolId> =
            self.traced.iter().map(|traced| traced.sym).collect();
        for sym in symbols {
            self.index_class(sym, &file);
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

