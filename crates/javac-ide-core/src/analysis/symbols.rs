//! Symbols of the analyzed file and positional lookup.

use std::path::Path;
use std::path::PathBuf;

use crate::code::scope::LookupKind;
use crate::code::symbol::{symbol_packge, SymbolKind};
use crate::ids::{SymbolId, TreeId};
use crate::util::name::Name;
use super::positions::{byte_to_utf16, line_offset, word_at};
use super::type_names::type_name;
use super::{canonical, FileAnalysis};

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

impl FileAnalysis {
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
        // A cursor past the end of the line refers to the last character.
        col = col.min(chars.len().saturating_sub(1));
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
        // AST/compiler first: parameters and locals are not in `symbols()`
        // (class + members), so a same-named field would otherwise win.
        let position = javac_analysis_api::Position::new(
            line.saturating_sub(1) as u32,
            column.saturating_sub(1) as u32,
        );
        if let Some((symbol, _)) = self.symbol_id_at(source, position) {
            // `pos` is the declaration position, never the reference position:
            // go-to-definition uses it, so a class-path symbol without a tree
            // here keeps -1 instead of jumping to the cursor.
            let declaration =
                crate::analysis::positions::name_start_of_symbol(&self.compiler.ctx, symbol)
                    .map(|position| position as i32)
                    .unwrap_or(-1);
            if let Some(info) = self.symbol_info(symbol, declaration) {
                return Some(info);
            }
        }
        // Fallback: declarations matched by name + nearest position.
        // Compiler symbol positions point at the declaration start; move them
        // onto the declared name for editor locations.
        let cursor = byte_to_utf16(&text, line_offset(&text, line, column)) as i64;
        self.symbols()
            .into_iter()
            .filter(|symbol| symbol.name == word && symbol.pos >= 0)
            .min_by_key(|symbol| (symbol.pos as i64 - cursor).abs())
            .or_else(|| self.symbols().into_iter().find(|symbol| symbol.name == word))
    }

    /// `SymbolInfo` for a compiler symbol (hover/definition). Returns `None`
    /// for nameless synthetic symbols (constructors included).
    fn symbol_info(
        &mut self,
        symbol: crate::ids::SymbolId,
        declaration: i32,
    ) -> Option<SymbolInfo> {
        let ctx = &self.compiler.ctx;
        let record = ctx.sym(symbol);
        let name = record.name.to_java_string().to_string_lossy().to_string();
        if name.is_empty() || name.starts_with('<') {
            return None;
        }
        let kind = match &record.kind_data {
            SymbolKind::ClassSymbol { .. } => "class",
            SymbolKind::MethodSymbol { .. } => "method",
            SymbolKind::VarSymbol { .. } => {
                if record.flags_field & crate::code::flags::PARAMETER != 0 {
                    "parameter"
                } else if record.owner.is_some_and(|owner| {
                    matches!(
                        ctx.sym(owner).kind_data,
                        SymbolKind::ClassSymbol { .. }
                    )
                }) {
                    "field"
                } else {
                    "local"
                }
            }
            _ => "member",
        };
        let ty = record
            .ty
            .map(|ty| type_name(ctx, ty))
            .unwrap_or_default();
        let detail = if ty.is_empty() {
            name.clone()
        } else {
            format!("{ty} {name}")
        };
        Some(SymbolInfo {
            name: name.clone(),
            kind,
            flat_name: name,
            pos: declaration,
            detail,
            doc: None,
        })
    }

    /// Source text of the currently analyzed file, when available.
    fn current_source_text(&self) -> Option<String> {
        for traced in &self.traced {
            if let SymbolKind::ClassSymbol {
                sourcefile: Some(file),
                ..
            } = &self.compiler.ctx.sym(traced.sym).kind_data
            {
                let path = PathBuf::from(file.get_name().to_string_lossy());
                let key = PathBuf::from(canonical(&path));
                return self
                    .texts
                    .get(&key)
                    .cloned()
                    .or_else(|| std::fs::read_to_string(&path).ok());
            }
        }
        None
    }

    /// Symbols of the analyzed file: its classes and their direct members.
    /// This is the base for completion and go-to-definition.
    pub fn symbols(&mut self) -> Vec<SymbolInfo> {
        let text = self.current_source_text();
        let docs = self.doc_comments_for_traced();
        let mut decl_trees: std::collections::HashMap<SymbolId, TreeId> =
            std::collections::HashMap::new();
        for (index, tree) in self.compiler.ctx.trees.iter().enumerate() {
            let symbol = match &tree.kind {
                crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
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
            let class_name = class.name.to_java_string().to_string_lossy().to_string();
            // Values used inside the member loop; holding `class` across the
            // `scope_get_symbols` call would alias the mutable context.
            let class_pos = class.pos.get();
            let class_flags = class.flags_field;
            let class_is_enum = (class_flags & 0x4000) != 0;
            if class_name.is_empty() {
                // Anonymous classes have an empty `Name`; they are not outline
                // entries.
                continue;
            }
            if class.owner.is_some_and(|owner| {
                matches!(
                    self.compiler.ctx.sym(owner).kind_data,
                    SymbolKind::ClassSymbol { .. }
                )
            }) {
                // Nested classes are added from their owner's members below.
                continue;
            }
            let class_pos = crate::analysis::positions::name_start_of_symbol(&self.compiler.ctx, traced.sym)
                .map(|position| position as i32)
                .unwrap_or_else(|| class.pos.get());
            out.push(SymbolInfo {
                name: class_name.clone(),
                kind: "class",
                flat_name: traced.flat_name.to_string(),
                pos: class_pos,
                detail: decl_trees
                    .get(&traced.sym)
                    .and_then(|tree| super::signatures::class_signature(&self.compiler.ctx, *tree))
                    .unwrap_or_else(|| format!("class {}", traced.flat_name)),
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
                    let raw_name = sym.name.to_java_string().to_string_lossy();
                    if raw_name.is_empty() || (raw_name.starts_with('<') && raw_name != "<init>") {
                        // Nameless/synthetic members are not outline entries.
                        continue;
                    }
                    let is_constructor = raw_name == "<init>";
                    let member_name = if is_constructor {
                        class_name.to_string()
                    } else {
                        raw_name.to_string()
                    };
                    let kind = if is_constructor {
                        "constructor"
                    } else {
                        match sym.kind_data {
                            SymbolKind::MethodSymbol { .. } => "method",
                            SymbolKind::VarSymbol { .. } => "field",
                            SymbolKind::ClassSymbol { .. } => "nested",
                            _ => "member",
                        }
                    };
                    let member_pos = match &sym.kind_data {
                        SymbolKind::VarSymbol { pos, .. } => *pos,
                        _ => sym.pos.get(),
                    };
                    let member_pos = crate::analysis::positions::name_start_of_symbol(&self.compiler.ctx, member)
                        .map(|position| position as i32)
                        .unwrap_or(member_pos);
                    // Compiler-synthesised members: record/enum methods carry
                    // the generated-member flag; `values`/`valueOf` point at
                    // the enum (no own source position).
                    let generated = matches!(&sym.kind_data, SymbolKind::MethodSymbol { .. })
                        && (sym.flags_field & (0x2000_0000_0000_0000_u64 as i64)) != 0;
                    let enum_synthetic = class_is_enum
                        && matches!(raw_name.as_str(), "values" | "valueOf")
                        && sym.pos.get() == class_pos;
                    let enum_synthetic =
                        class_is_enum && matches!(raw_name.as_str(), "values" | "valueOf");
                    if generated || enum_synthetic {
                        continue;
                    }
                    let detail = decl_trees
                        .get(&member)
                        .and_then(|tree| match kind {
                            "method" => super::signatures::method_signature(
                                &self.compiler.ctx,
                                *tree,
                            ),
                            _ => super::signatures::field_signature(&self.compiler.ctx, *tree),
                        })
                        .unwrap_or_else(|| {
                            format!(
                                "{}.{}",
                                traced.flat_name,
                                sym.name.to_java_string().to_string_lossy()
                            )
                        });
                    out.push(SymbolInfo {
                        name: member_name,
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


impl FileAnalysis {
    /// Simple names declared in the currently analyzed file (import conflicts).
    pub(crate) fn traced_class_names(&self) -> Vec<String> {
        self.traced
            .iter()
            .map(|traced| {
                self.compiler
                    .ctx
                    .sym(traced.sym)
                    .name
                    .to_java_string()
                    .to_string_lossy()
            })
            .collect()
    }

    /// Javadoc of a class declared in the currently analyzed file.
    pub(crate) fn traced_class_doc(&mut self, flat: &str) -> Option<String> {
        let symbol = self
            .traced
            .iter()
            .find(|traced| traced.flat_name.to_string() == flat)
            .map(|traced| traced.sym)?;
        self.doc_comments_for_traced().get(&symbol).cloned()
    }

    /// Javadoc of a member of a class declared in the currently analyzed file.
    pub(crate) fn traced_member_doc(&mut self, class_flat: &str, name: &str) -> Option<String> {
        let class = self
            .traced
            .iter()
            .find(|traced| traced.flat_name.to_string() == class_flat)
            .map(|traced| traced.sym)?;
        let scope = self.compiler.ctx.sym(class).members()?;
        let member = self
            .compiler
            .ctx
            .scope_get_symbols(scope, LookupKind::NON_RECURSIVE, &|_| true)
            .into_iter()
            .find(|member| {
                self.compiler.ctx.sym(*member).name.to_java_string().to_string_lossy() == name
            })?;
        self.doc_comments_for_traced().get(&member).cloned()
    }

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
        // Trees accumulate across analysis rounds; keep the newest compilation
        // unit for this source so the doc table matches the current symbols.
        let mut table = None;
        for tree in self.compiler.ctx.trees.iter() {
            if let crate::tree::jc_tree::TreeKind::CompilationUnit {
                sourcefile,
                doc_comments,
                ..
            } = &tree.kind
                && let Some(file) = sourcefile.as_ref().map(|file| file.get_name().to_string_lossy())
                && source_names
                    .iter()
                    .any(|name| name == &canonical(Path::new(&file)))
            {
                table = Some(doc_comments.clone());
            }
        }
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


impl FileAnalysis {
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
