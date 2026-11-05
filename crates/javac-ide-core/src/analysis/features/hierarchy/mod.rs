//! Type and call hierarchies.

use std::path::{Path, PathBuf};

use super::super::positions::offset_to_line_col;
use super::super::{canonical, AnalysisSession};

impl AnalysisSession {
    /// Prepare the call hierarchy for a method at a position.
    pub(crate) fn prepare_call_hierarchy_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::CallHierarchyItem> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())?;
        let word = super::super::positions::word_at(
            &text,
            position.line as usize + 1,
            position.column as usize + 1,
        )?;
        self.method_item(&word)
    }

    /// Incoming calls of a method.
    pub(crate) fn incoming_calls_impl(
        &mut self,
        item: &javac_analysis_api::CallHierarchyItem,
    ) -> Vec<javac_analysis_api::CallHierarchyCall> {
        let Some(text) = self
            .texts
            .get(&PathBuf::from(canonical(&item.file)))
            .cloned()
            .or_else(|| std::fs::read_to_string(&item.file).ok())
        else {
            return Vec::new();
        };
        let file_name = canonical(&item.file);
        // Method bodies of this file, with their callable ranges.
        let mut methods: Vec<(String, i32, i32)> = Vec::new();
        for tree in self.compiler.ctx.trees.iter() {
            if let crate::tree::jc_tree::TreeKind::MethodDecl {
                name,
                sym: Some(sym),
                body: Some(body),
                ..
            } = &tree.kind
            {
                let owner = self.compiler.ctx.sym(*sym).owner;
                let owner_file = owner.and_then(|owner| match &self.compiler.ctx.sym(owner).kind_data {
                    crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
                        .as_ref()
                        .map(|file| canonical(Path::new(&file.get_name().to_string_lossy()))),
                    _ => None,
                });
                if owner_file.as_deref() != Some(file_name.as_str()) {
                    continue;
                }
                methods.push((
                    name.to_java_string().to_string_lossy(),
                    self.compiler.ctx.tree(*body).pos,
                    0,
                ));
            }
        }
        methods.sort_by_key(|(_, start, _)| *start);
        let method_count = methods.len();
        for index in 0..method_count {
            let end = methods
                .get(index + 1)
                .map(|(_, start, _)| *start)
                .unwrap_or(text.len() as i32);
            methods[index].2 = end;
        }
        let mut calls = Vec::new();
        for tree in self.compiler.ctx.trees.iter() {
            let (call_name, pos) = match &tree.kind {
                crate::tree::jc_tree::TreeKind::MethodInvocation {
                    meth: Some(meth), ..
                } => {
                    let pos = self.compiler.ctx.tree(*meth).pos;
                    match &self.compiler.ctx.tree(*meth).kind {
                        crate::tree::jc_tree::TreeKind::Ident { name, .. } => {
                            (name.to_java_string().to_string_lossy(), pos)
                        }
                        crate::tree::jc_tree::TreeKind::FieldAccess { name, .. } => {
                            (name.to_java_string().to_string_lossy(), pos)
                        }
                        _ => continue,
                    }
                }
                _ => continue,
            };
            if call_name != item.name {
                continue;
            }
            if pos < 0 || pos as usize >= text.len() {
                continue;
            }
            if !text[pos as usize..].starts_with(&call_name) {
                continue;
            }
            let from = methods
                .iter()
                .find(|(_, start, end)| pos >= *start && pos < *end)
                .map(|(name, _, _)| name.clone());
            let Some(from_name) = from else {
                continue;
            };
            let Some(from_item) = self.method_item(&from_name) else {
                continue;
            };
            let (line, column) = offset_to_line_col(&text, pos.max(0) as usize);
            calls.push(javac_analysis_api::CallHierarchyCall {
                item: from_item,
                from_ranges: vec![javac_analysis_api::Range::point(
                    javac_analysis_api::Position::new(line, column),
                )],
            });
        }
        calls
    }

    /// Outgoing calls of a method.
    pub(crate) fn outgoing_calls_impl(
        &mut self,
        item: &javac_analysis_api::CallHierarchyItem,
    ) -> Vec<javac_analysis_api::CallHierarchyCall> {
        let Some(text) = self
            .texts
            .get(&PathBuf::from(canonical(&item.file)))
            .cloned()
            .or_else(|| std::fs::read_to_string(&item.file).ok())
        else {
            return Vec::new();
        };
        let methods = self.method_ranges(&item.file);
        let Some((_, range_start, range_end)) =
            methods.iter().find(|(name, _, _)| name == &item.name).cloned()
        else {
            return Vec::new();
        };
        let mut calls = Vec::new();
        for tree in self.compiler.ctx.trees.iter() {
            let (name, pos) = match &tree.kind {
                crate::tree::jc_tree::TreeKind::MethodInvocation {
                    meth: Some(meth), ..
                } => {
                    let pos = self.compiler.ctx.tree(*meth).pos;
                    match &self.compiler.ctx.tree(*meth).kind {
                        crate::tree::jc_tree::TreeKind::Ident { name, .. } => {
                            (name.to_java_string().to_string_lossy(), pos)
                        }
                        crate::tree::jc_tree::TreeKind::FieldAccess { name, .. } => {
                            (name.to_java_string().to_string_lossy(), pos)
                        }
                        _ => continue,
                    }
                }
                _ => continue,
            };
            if pos < range_start || pos >= range_end {
                continue;
            }
            if pos < 0 || pos as usize >= text.len() || !text[pos as usize..].starts_with(&name) {
                continue;
            }
            let Some(to_item) = self.method_item(&name) else {
                continue;
            };
            let (line, column) = offset_to_line_col(&text, pos.max(0) as usize);
            calls.push(javac_analysis_api::CallHierarchyCall {
                item: to_item,
                from_ranges: vec![javac_analysis_api::Range::point(
                    javac_analysis_api::Position::new(line, column),
                )],
            });
        }
        calls
    }

    /// Prepare the type hierarchy for the type at a position.
    pub(crate) fn prepare_type_hierarchy_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::TypeHierarchyItem> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())?;
        let word = super::super::positions::word_at(
            &text,
            position.line as usize + 1,
            position.column as usize + 1,
        )?;
        let class = self.class_symbol_named(&word)?;
        self.type_hierarchy_item(class)
    }

    /// Supertypes of a type.
    pub(crate) fn supertypes_impl(
        &mut self,
        item: &javac_analysis_api::TypeHierarchyItem,
    ) -> Vec<javac_analysis_api::TypeHierarchyItem> {
        let Some(class) = self.class_symbol_named(&item.name) else {
            return Vec::new();
        };
        let class_flat = self.compiler.ctx.sym(class).flat_name().to_string();
        let mut supertypes = Vec::new();
        for tree in self.compiler.ctx.trees.iter() {
            if let crate::tree::jc_tree::TreeKind::ClassDecl {
                sym: Some(sym),
                extending,
                implementing,
                ..
            } = &tree.kind
                && self.compiler.ctx.sym(*sym).flat_name().to_string() == class_flat
            {
                let trees: Vec<crate::ids::TreeId> =
                    extending.iter().copied().chain(implementing.iter()).collect();
                for supertree in trees {
                    if let Some(ty) = self.compiler.ctx.tree(supertree).ty {
                        if let Some(tsym) = self.compiler.ctx.typ(ty).tsym {
                            let simple = self
                                .compiler
                                .ctx
                                .sym(tsym)
                                .name
                                .to_java_string()
                                .to_string_lossy();
                            let resolved = self.class_symbol_named(&simple).unwrap_or(tsym);
                            if let Some(item) = self.type_hierarchy_item(resolved) {
                                supertypes.push(item);
                            }
                        }
                    }
                }
                break;
            }
        }
        supertypes
    }

    /// Subtypes of a type.
    pub(crate) fn subtypes_impl(
        &mut self,
        item: &javac_analysis_api::TypeHierarchyItem,
    ) -> Vec<javac_analysis_api::TypeHierarchyItem> {
        let Some(target) = self.class_symbol_named(&item.name) else {
            return Vec::new();
        };
        let target_flat = self.compiler.ctx.sym(target).flat_name().to_string();
        let mut subtypes = Vec::new();
        for tree in self.compiler.ctx.trees.iter() {
            let Ok((sym, extending, implementing)) = (|| match &tree.kind {
                crate::tree::jc_tree::TreeKind::ClassDecl {
                    sym: Some(sym),
                    extending,
                    implementing,
                    ..
                } => Ok((*sym, *extending, implementing.clone())),
                _ => Err(()),
            })() else {
                continue;
            };
            if sym == target {
                continue;
            }
            let supertrees: Vec<crate::ids::TreeId> =
                extending.iter().copied().chain(implementing.iter()).collect();
            let mut matches = false;
            for supertree in supertrees {
                if let Some(ty) = self.compiler.ctx.tree(supertree).ty
                    && self
                        .compiler
                        .ctx
                        .typ(ty)
                        .tsym
                        .map(|tsym| self.compiler.ctx.sym(tsym).flat_name().to_string() == target_flat)
                        .unwrap_or(false)
                {
                    matches = true;
                    break;
                }
            }
            if matches {
                if let Some(item) = self.type_hierarchy_item(sym) {
                    subtypes.push(item);
                }
            }
        }
        subtypes
    }
}
