//! Inlay hints: inferred var types and call argument names.
//!
//! Only information the compiler already resolved is used. No JDK source
//! archive or debug metadata is read. Every hint is validated against the
//! current source text so stale trees cannot place a phantom at a wrong token.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use javac_analysis_api::{InlayHint, InlayHintKind, Position};

use crate::analysis::positions::{offset_to_line_col, utf16_to_byte};
use crate::analysis::type_names::type_name;
use crate::analysis::jdk_sources;
use crate::code::symbol::SymbolKind;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId};
use crate::tree::jc_tree::TreeKind;
use crate::tree::tree_scanner::{TreeScannerVisitor, tree_scanner_visitor_delegate};
use crate::workspace::index::{query, ProjectIndex};

use super::super::{canonical, FileAnalysis};

impl FileAnalysis {
    /// Inlay hints for the current file.
    pub(crate) fn inlay_hints_impl(
        &mut self,
        file: &Path,
        index: Option<&ProjectIndex>,
        show_same_name: bool,
    ) -> Vec<InlayHint> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        if text.is_empty() {
            return Vec::new();
        }
        // Only the latest compilation unit for this file may contribute
        // hints. ctx.trees accumulates every analysis round; using old trees
        // makes parameter names flap between rounds.
        let Some(root) = self.latest_root_for(&key) else {
            return Vec::new();
        };
        let mut collector = HintCollector::default();
        collector.scan(&mut self.compiler.ctx, Some(root));
        let mut hints = self.var_hints(&key, &text, &collector.vars);
        hints.extend(self.parameter_hints(&text, &collector.calls, index, show_same_name));
        hints.sort_by_key(|hint| (hint.position.line, hint.position.column));
        hints.dedup_by(|left, right| {
            left.position == right.position && left.kind == right.kind && left.label == right.label
        });
        hints
    }

    fn var_hints(&mut self, key: &Path, text: &str, vars: &[TreeId]) -> Vec<InlayHint> {
        let traced: HashSet<SymbolId> = self.traced.iter().map(|traced| traced.sym).collect();
        let mut seen: HashSet<String> = HashSet::new();
        let mut hints: HashMap<(u32, u32), (InlayHint, String)> = HashMap::new();
        for tree_id in vars {
            let tree = self.compiler.ctx.tree(*tree_id);
            let TreeKind::VariableDecl {
                name,
                vartype,
                sym: Some(sym),
                decl_pos,
                declared_using_var,
                ..
            } = &tree.kind
            else {
                continue;
            };
            let uses_var = *declared_using_var
                || vartype.is_some_and(|vartype| {
                    ast_is_var_type(&self.compiler.ctx, vartype)
                        || crate::tree::tree_info::get_start_pos(&self.compiler.ctx, vartype)
                            == *decl_pos
                });
            if *decl_pos < 0 || !uses_var || !self.owner_in_traced(*sym, &traced) {
                continue;
            }
            let name = name.to_java_string().to_string_lossy();
            if name.is_empty() {
                continue;
            }
            // The current round owns this name even if its type cannot be
            // rendered (for example a capture type). Do not resurrect an old
            // label for it from the cache.
            seen.insert(name.clone());
            let ty = vartype
                .and_then(|vartype| self.compiler.ctx.tree(vartype).ty)
                .or_else(|| self.compiler.ctx.sym(*sym).ty);
            let Some(ty) = ty else { continue };
            let rendered = type_name(&self.compiler.ctx, ty);
            if rendered.is_empty()
                || rendered == "var"
                || rendered == "?"
                || rendered.starts_with('<')
            {
                continue;
            }
            let end = *decl_pos as usize + name.encode_utf16().count();
            let (line, column) = offset_to_line_col(text, end);
            hints.insert(
                (line, column),
                (
                    InlayHint {
                        position: Position::new(line, column),
                        label: format!(": {rendered}"),
                        kind: InlayHintKind::Type,
                        padding_left: false,
                        padding_right: false,
                    },
                    name,
                ),
            );
        }
        let mut current: Vec<(InlayHint, String)> = hints.into_values().collect();
        current.sort_by_key(|(hint, _)| (hint.position.line, hint.position.column));
        if !current.is_empty() {
            // Current analysis wins; cached entries fill in declarations that
            // this round temporarily failed to resolve.
            let mut merged: HashMap<(u32, u32), InlayHint> = current
                .iter()
                .map(|(hint, _)| {
                    ((hint.position.line, hint.position.column), hint.clone())
                })
                .collect();
            let skip_names = if self.compiler.log.nerrors.get() > 0 {
                HashSet::new()
            } else {
                seen.clone()
            };
            for hint in self.cached_inlay_hints(key, vars, text, &skip_names) {
                merged
                    .entry((hint.position.line, hint.position.column))
                    .or_insert(hint);
            }
            let mut cache: HashMap<String, (InlayHint, String)> = self
                .inlay_hint_cache
                .get(key)
                .map(|entries| {
                    entries
                        .iter()
                        .map(|(hint, name)| (name.clone(), (hint.clone(), name.clone())))
                        .collect()
                })
                .unwrap_or_default();
            for (hint, name) in &current {
                cache.insert(name.clone(), (hint.clone(), name.clone()));
            }
            self.inlay_hint_cache
                .insert(key.to_path_buf(), cache.into_values().collect());
            let mut merged: Vec<InlayHint> = merged.into_values().collect();
            merged.sort_by_key(|hint| (hint.position.line, hint.position.column));
            return merged;
        }
        // The buffer may be temporarily unparsable. Reuse the last valid
        // hints only for AST `var` declarations that still exist.
        if vars.is_empty() {
            self.inlay_hint_cache.remove(key);
            return Vec::new();
        }
        let skip_names = if self.compiler.log.nerrors.get() > 0 {
            HashSet::new()
        } else {
            seen.clone()
        };
        self.cached_inlay_hints(key, vars, text, &skip_names)
    }

    fn parameter_hints(
        &self,
        text: &str,
        calls: &[TreeId],
        index: Option<&ProjectIndex>,
        show_same_name: bool,
    ) -> Vec<InlayHint> {
        let tokens = super::completion::token_stream::tokenize(&self.compiler, text);
        let mut hints: HashMap<(u32, u32), InlayHint> = HashMap::new();
        for call in calls {
            let tree = self.compiler.ctx.tree(*call);
            let (callee, args) = match &tree.kind {
                TreeKind::MethodInvocation {
                    meth: Some(meth),
                    args,
                    ..
                } => (method_symbol(&self.compiler.ctx, *meth), args),
                TreeKind::NewClass {
                    constructor: Some(constructor),
                    args,
                    ..
                } => (Some(*constructor), args),
                _ => continue,
            };
            let Some(callee) = callee else { continue };
            let args = args.clone();
            if args.is_empty() {
                continue;
            }
            let method_name = self
                .compiler
                .ctx
                .sym(callee)
                .name
                .to_java_string()
                .to_string_lossy();
            let owner_flat_name = self.compiler.ctx.sym(callee).owner.and_then(|owner| {
                match &self.compiler.ctx.sym(owner).kind_data {
                    SymbolKind::ClassSymbol { flatname, .. } => Some(flatname.to_string()),
                    _ => None,
                }
            });
            let parameter_types = self.parameter_type_names(callee);
            // The resolved symbol can belong to a previous analysis round
            // (empty/synthetic params); fall back to the local declaration,
            // then the project index.
            let mut params: Vec<String> = match &self.compiler.ctx.sym(callee).kind_data {
                SymbolKind::MethodSymbol { params, .. }
                    if !params.is_empty() && params.size() == args.size() =>
                {
                    params
                        .iter()
                        .map(|param| {
                            self.compiler
                                .ctx
                                .sym(param)
                                .name
                                .to_java_string()
                                .to_string_lossy()
                        })
                        .collect()
                }
                _ => self.local_parameter_names(&method_name, args.size() as usize),
            };
            let needs_fallback = params.len() != args.size() as usize
                || params.iter().any(|name| {
                    name.is_empty() || name.starts_with('<') || is_synthetic_parameter(name)
                });
            if needs_fallback
                && let Some(index) = index
                && let Some(names) = query::parameter_names(
                    index,
                    &method_name,
                    args.size() as usize,
                    owner_flat_name.as_deref(),
                )
            {
                params = names;
            }
            let needs_jdk_fallback = params.len() != args.size() as usize
                || params.iter().any(|name| {
                    name.is_empty() || name.starts_with('<') || is_synthetic_parameter(name)
                });
            if needs_jdk_fallback
                && let Some(owner_flat_name) = owner_flat_name.as_deref()
                && is_jdk_owner(owner_flat_name)
                && let Some(names) = jdk_sources::jdk_method_parameter_names(
                    &self.compiler.parser_factory,
                    owner_flat_name,
                    &method_name,
                    args.size() as usize,
                    &parameter_types,
                )
            {
                params = names;
            }
            if params.len() != args.size() as usize {
                continue;
            }
            for (name, argument) in params.iter().zip(args.iter()) {
                if name.is_empty() || name.starts_with('<') || is_synthetic_parameter(name) {
                    continue;
                }
                if !show_same_name && argument_is_same_name(&self.compiler.ctx, text, argument, name)
                {
                    continue;
                }
                let Some(pos) = argument_start(&self.compiler.ctx, argument, &tokens) else {
                    continue;
                };
                let (line, column) = offset_to_line_col(text, pos);
                hints.entry((line, column)).or_insert(InlayHint {
                    position: Position::new(line, column),
                    label: format!("{name}:"),
                    kind: InlayHintKind::Parameter,
                    padding_left: false,
                    padding_right: true,
                });
            }
        }
        let mut hints: Vec<InlayHint> = hints.into_values().collect();
        hints.sort_by_key(|hint| (hint.position.line, hint.position.column));
        hints
    }

    /// Declared parameter type names of a resolved method/constructor. Used as
    /// the descriptor component of the JDK-source cache key and overload
    /// selection; call-site substitutions do not matter for source matching.
    fn parameter_type_names(&self, callee: SymbolId) -> Vec<String> {
        let Some(ty) = self.compiler.ctx.sym(callee).ty else {
            return Vec::new();
        };
        let crate::code::type_::TypeKindData::MethodType { argtypes, .. } =
            &self.compiler.ctx.typ(ty).kind
        else {
            return Vec::new();
        };
        argtypes
            .iter()
            .map(|argtype| type_name(&self.compiler.ctx, argtype))
            .collect()
    }

    /// Parameter names of the local declaration of `name` with `count` params.
    fn local_parameter_names(&self, name: &str, count: usize) -> Vec<String> {
        for tree in self.compiler.ctx.trees.iter() {
            if let TreeKind::MethodDecl {
                name: method_name,
                params,
                ..
            } = &tree.kind
            {
                if method_name.to_java_string().to_string_lossy() == name
                    && params.size() as usize == count
                {
                    return params
                        .iter()
                        .filter_map(|param| match &self.compiler.ctx.tree(param).kind {
                            TreeKind::VariableDecl { name, .. } => {
                                Some(name.to_java_string().to_string_lossy())
                            }
                            _ => None,
                        })
                        .collect();
                }
            }
        }
        Vec::new()
    }

    fn cached_inlay_hints(
        &self,
        key: &Path,
        vars: &[TreeId],
        text: &str,
        skip_names: &HashSet<String>,
    ) -> Vec<InlayHint> {
        let Some(cached) = self.inlay_hint_cache.get(key) else {
            return Vec::new();
        };
        let mut hints: HashMap<(u32, u32), InlayHint> = HashMap::new();
        for var in vars {
            let TreeKind::VariableDecl {
                name,
                decl_pos,
                vartype,
                declared_using_var,
                ..
            } = &self.compiler.ctx.tree(*var).kind
            else {
                continue;
            };
            if !*declared_using_var
                && !vartype.is_some_and(|vartype| {
                    ast_is_var_type(&self.compiler.ctx, vartype)
                        || crate::tree::tree_info::get_start_pos(&self.compiler.ctx, vartype)
                            == *decl_pos
                })
            {
                continue;
            }
            if *decl_pos < 0 {
                continue;
            }
            let name = name.to_java_string().to_string_lossy();
            if skip_names.contains(&name) {
                continue;
            }
            let Some((hint, _)) = cached.iter().find(|(_, cached_name)| *cached_name == name) else {
                continue;
            };
            let end = *decl_pos as usize + name.encode_utf16().count();
            let (line, column) = offset_to_line_col(text, end);
            hints.entry((line, column)).or_insert(InlayHint {
                position: Position::new(line, column),
                label: hint.label.clone(),
                kind: InlayHintKind::Type,
                padding_left: false,
                padding_right: false,
            });
        }
        let mut hints: Vec<InlayHint> = hints.into_values().collect();
        hints.sort_by_key(|hint| (hint.position.line, hint.position.column));
        hints
    }

    fn owner_in_traced(&self, symbol: SymbolId, traced: &HashSet<SymbolId>) -> bool {
        let mut owner = self.compiler.ctx.sym(symbol).owner;
        while let Some(current) = owner {
            let node = self.compiler.ctx.sym(current);
            if matches!(node.kind_data, SymbolKind::ClassSymbol { .. })
                && traced.contains(&current)
            {
                return true;
            }
            owner = node.owner;
        }
        false
    }
}

/// True when the owner is a JDK class whose declaration can come from src.zip.
fn is_jdk_owner(flat_name: &str) -> bool {
    ["java.", "javax.", "jdk.", "sun.", "com.sun."]
        .iter()
        .any(|prefix| flat_name.starts_with(prefix))
}

/// True when the declared type node is the reserved `var` identifier.
fn ast_is_var_type(ctx: &CompilerContext, tree: TreeId) -> bool {
    match &ctx.tree(tree).kind {
        TreeKind::Ident { name, .. } | TreeKind::FieldAccess { name, .. } => {
            name.to_java_string().to_string_lossy() == "var"
        }
        TreeKind::AnnotatedType {
            underlying_type: Some(underlying),
            ..
        } => ast_is_var_type(ctx, *underlying),
        _ => false,
    }
}

/// Leftmost AST position of an argument expression, snapped to the first
/// scanner token (comments are not tokens).
fn argument_start(
    ctx: &CompilerContext,
    argument: TreeId,
    tokens: &[crate::parser::tokens::Token],
) -> Option<usize> {
    let mut tree = argument;
    loop {
        match &ctx.tree(tree).kind {
            TreeKind::Parens { expr: Some(expr) } => tree = *expr,
            TreeKind::FieldAccess { selected: Some(selected), .. } => tree = *selected,
            TreeKind::MethodInvocation { meth: Some(meth), .. } => tree = *meth,
            _ => break,
        }
    }
    let position = ctx.tree(tree).pos;
    if position < 0 {
        return None;
    }
    let token = tokens.iter().find(|token| token.pos >= position)?;
    // A comment before the argument makes the token's recorded start point at
    // the comment; do not place a hint inside trivia.
    if token.comments.is_some() {
        return None;
    }
    Some(token.pos as usize)
}

/// True when the argument's source text is exactly the parameter name.
fn argument_is_same_name(
    ctx: &CompilerContext,
    _text: &str,
    argument: TreeId,
    name: &str,
) -> bool {
    let TreeKind::Ident {
        name: argument_name,
        ..
    } = &ctx.tree(argument).kind
    else {
        return false;
    };
    if argument_name.to_java_string().to_string_lossy() != name {
        return false;
    }
    true
}

#[derive(Default)]
struct HintCollector {
    vars: Vec<TreeId>,
    calls: Vec<TreeId>,
}

impl TreeScannerVisitor for HintCollector {
    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.vars.push(tree);
        let (mods, vartype, nameexpr, init) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl {
                mods,
                vartype,
                nameexpr,
                init,
                ..
            } => (*mods, *vartype, *nameexpr, *init),
            _ => return,
        };
        self.scan(ctx, mods);
        self.scan(ctx, vartype);
        self.scan(ctx, nameexpr);
        self.scan(ctx, init);
    }

    fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.calls.push(tree);
        let (typeargs, meth, args) = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation {
                typeargs,
                meth,
                args,
                ..
            } => (typeargs.clone(), *meth, args.clone()),
            _ => return,
        };
        self.scan_list(ctx, &typeargs);
        self.scan(ctx, meth);
        self.scan_list(ctx, &args);
    }

    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.calls.push(tree);
        let (encl, typeargs, clazz, args, def) = match &ctx.tree(tree).kind {
            TreeKind::NewClass {
                encl,
                typeargs,
                clazz,
                args,
                def,
                ..
            } => (*encl, typeargs.clone(), *clazz, args.clone(), *def),
            _ => return,
        };
        self.scan(ctx, encl);
        self.scan_list(ctx, &typeargs);
        self.scan(ctx, clazz);
        self.scan_list(ctx, &args);
        self.scan(ctx, def);
    }
}

tree_scanner_visitor_delegate!(HintCollector);

fn method_symbol(
    ctx: &crate::compiler_context::CompilerContext,
    meth: TreeId,
) -> Option<SymbolId> {
    match &ctx.tree(meth).kind {
        TreeKind::Ident { sym, .. } | TreeKind::FieldAccess { sym, .. } => *sym,
        _ => None,
    }
}

fn is_synthetic_parameter(name: &str) -> bool {
    name.strip_prefix("arg")
        .map(|digits| !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_digit()))
        .unwrap_or(false)
}

