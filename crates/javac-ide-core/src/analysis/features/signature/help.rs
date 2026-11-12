//! Signature help.
//!
//! The call is located through the attributed trees (a `MethodInvocation` or
//! `NewClass` whose callee name ends at the `(`), so overloads are chosen by
//! the resolved argument types, constructors are supported, and methods
//! declared in other project/JDK files resolve through their symbols.

use std::path::{Path, PathBuf};

use javac_analysis_api::{ParameterInformation, SignatureHelp, SignatureInformation};

use super::super::super::{canonical, FileAnalysis};

use crate::code::scope::LookupKind;
use crate::code::symbol::SymbolKind;
use crate::ids::{SymbolId, TreeId};
use crate::tree::jc_tree::TreeKind;
use crate::tree::tree_info;

/// A resolved call site: the candidate signatures and the argument trees.
struct CallAt {
    candidates: Vec<(SymbolId, bool)>,
    arguments: Vec<TreeId>,
}

enum CallKind {
    Method(SymbolId, Vec<TreeId>),
    NewClass(SymbolId, Option<SymbolId>, Vec<TreeId>),
}

impl FileAnalysis {
    /// Signature help for the call at a position.
    pub(crate) fn signature_help_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<SignatureHelp> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())?;
        let cursor = crate::analysis::positions::line_offset(
            &text,
            position.line as usize + 1,
            position.column as usize + 1,
        );
        let tokens = super::super::completion::token_stream::tokenize(&self.compiler, &text);
        let cursor_utf16 = crate::analysis::positions::byte_to_utf16(&text, cursor) as i32;
        let open_index = open_paren_token(&tokens, cursor_utf16)?;
        let open = tokens[open_index].pos as usize;
        let name = ident_before_token(&tokens, open_index)?;
        // Editing a declaration's parameter list is not a call.
        if is_declaration_paren(
            &self.compiler.ctx,
            self.latest_root_for(file),
            &tokens,
            open_index,
            &name,
        ) {
            return None;
        }
        let active_parameter = parameter_index_tokens(&tokens, open_index, cursor_utf16);
        if let Some(call) = self.call_at(open, &name) {
            let signatures: Vec<SignatureInformation> = call
                .candidates
                .iter()
                .filter_map(|(symbol, constructor)| {
                    self.signature_information(*symbol, *constructor)
                })
                .collect();
            if !signatures.is_empty() {
                let argument_types: Vec<Option<String>> = call
                    .arguments
                    .iter()
                    .map(|argument| self.argument_type_name(*argument))
                    .collect();
                let active_signature =
                    choose_signature(&signatures, &argument_types, active_parameter);
                return Some(SignatureHelp {
                    signatures,
                    active_signature,
                    active_parameter,
                });
            }
        }
        // Fallback: methods declared in this file (kept for incomplete code
        // where attribution did not produce a call tree yet).
        self.declared_method_signatures(&name, active_parameter)
    }

    fn declared_method_signatures(&mut self, name: &str, active_parameter: u32) -> Option<SignatureHelp> {
        let ast_parameters = self.declared_method_parameters(name);
        let mut signatures = Vec::new();
        for (index, symbol) in self.symbols().into_iter().enumerate() {
            if symbol.kind != "method" || symbol.name != name {
                continue;
            }
            let label = symbol.detail;
            let parameters = ast_parameters
                .get(index)
                .map(|(_, parameters)| parameters.clone())
                .unwrap_or_default();
            signatures.push(SignatureInformation {
                label,
                parameters,
                documentation: symbol.doc,
            });
        }
        if signatures.is_empty() {
            return None;
        }
        let active_signature = signatures
            .iter()
            .position(|signature| signature.parameters.len() as u32 > active_parameter)
            .unwrap_or(0) as u32;
        Some(SignatureHelp {
            signatures,
            active_signature,
            active_parameter,
        })
    }

    /// Parameter lists of methods declared in this file, keyed by method name.
    fn declared_method_parameters(&self, name: &str) -> Vec<(String, Vec<ParameterInformation>)> {
        let ctx = &self.compiler.ctx;
        let mut result = Vec::new();
        for tree in ctx.trees.iter() {
            let TreeKind::MethodDecl { name: method_name, params, .. } = &tree.kind else {
                continue;
            };
            let method_name = method_name.to_java_string().to_string_lossy().to_string();
            if method_name != name {
                continue;
            }
            let count = params.size() as usize;
            let mut parameters = Vec::new();
            for (index, param) in params.iter().enumerate() {
                let TreeKind::VariableDecl { name, vartype, sym, .. } = &ctx.tree(param).kind else {
                    continue;
                };
                let type_id = vartype
                    .and_then(|vartype| ctx.tree(vartype).ty)
                    .or_else(|| sym.map(|sym| ctx.sym(sym).ty).flatten());
                let varargs = index + 1 == count
                    && sym.is_some_and(|sym| ctx.sym(sym).flags_field & crate::code::flags::VARARGS as i64 != 0);
                parameters.push(ParameterInformation {
                    type_name: type_id
                        .map(|ty| {
                            if varargs {
                                crate::analysis::signatures::varargs_element_type(ctx, ty)
                            } else {
                                crate::analysis::type_names::type_name(ctx, ty)
                            }
                        })
                        .unwrap_or_else(|| "?".to_string()),
                    name: Some(name.to_java_string().to_string_lossy().to_string()),
                    varargs,
                });
            }
            result.push((method_name, parameters));
        }
        result
    }

    fn call_at(&mut self, open: usize, name: &str) -> Option<CallAt> {
        let mut best: Option<(i32, CallKind)> = None;
        {
            let ctx = &self.compiler.ctx;
            for index in 0..ctx.trees.len() {
                let tree = TreeId::new(index as u32);
                match &ctx.tree(tree).kind {
                    TreeKind::MethodInvocation {
                        meth: Some(meth),
                        args,
                        ..
                    } => {
                        let Some(symbol) = tree_info::symbol(ctx, *meth) else {
                            continue;
                        };
                        let symbol_name = symbol_name(ctx, symbol);
                        if symbol_name != name || symbol_name == "<init>" {
                            continue;
                        }
                        let start = tree_info::get_start_pos(ctx, *meth);
                        if start < 0 || start as usize > open {
                            continue;
                        }
                        let arguments: Vec<TreeId> = args.iter().collect();
                        if !arguments_match(ctx, &arguments, open) {
                            continue;
                        }
                        if best
                            .as_ref()
                            .map(|(best_start, _)| start >= *best_start)
                            .unwrap_or(true)
                        {
                            best = Some((start, CallKind::Method(symbol, arguments)));
                        }
                    }
                    TreeKind::NewClass {
                        clazz: Some(clazz),
                        constructor,
                        args,
                        ..
                    } => {
                        let Some(class) = tree_info::symbol(ctx, *clazz) else {
                            continue;
                        };
                        if symbol_name(ctx, class) != name {
                            continue;
                        }
                        let start = tree_info::get_start_pos(ctx, *clazz);
                        if start < 0 || start as usize > open {
                            continue;
                        }
                        let arguments: Vec<TreeId> = args.iter().collect();
                        if !arguments_match(ctx, &arguments, open) {
                            continue;
                        }
                        if best
                            .as_ref()
                            .map(|(best_start, _)| start >= *best_start)
                            .unwrap_or(true)
                        {
                            best = Some((
                                start,
                                CallKind::NewClass(class, *constructor, arguments),
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }
        let (_, kind) = best?;
        match kind {
            CallKind::Method(symbol, arguments) => Some(CallAt {
                candidates: method_candidates(&mut self.compiler.ctx, symbol),
                arguments,
            }),
            CallKind::NewClass(class, constructor, arguments) => {
                let mut candidates = Vec::new();
                let ctx = &mut self.compiler.ctx;
                if let Some(scope) = ctx.sym(class).members() {
                    let members =
                        ctx.scope_get_symbols(scope, LookupKind::NON_RECURSIVE, &|_| true);
                    for member in members {
                        if matches!(
                            ctx.sym(member).kind_data,
                            SymbolKind::MethodSymbol { .. }
                        ) && symbol_name(ctx, member) == "<init>"
                        {
                            candidates.push((member, true));
                        }
                    }
                }
                if candidates.is_empty()
                    && let Some(constructor) = constructor
                {
                    candidates.push((constructor, true));
                }
                if candidates.is_empty() {
                    return None;
                }
                Some(CallAt {
                    candidates,
                    arguments,
                })
            }
        }
    }

    /// A `SignatureInformation` built from the resolved symbol (parameter names
    /// come from source/class-file symbols, not from the hover detail string).
    fn signature_information(
        &mut self,
        symbol: SymbolId,
        _constructor: bool,
    ) -> Option<SignatureInformation> {
        let parts = crate::analysis::signatures::signature_parts(
            &self.compiler.ctx,
            symbol,
            crate::analysis::signatures::ParameterNames::Verbatim,
        )?;
        let documentation = self.documentation_for(symbol);
        Some(SignatureInformation {
            label: parts.label(),
            parameters: parts.parameter_info,
            documentation,
        })
    }

    fn argument_type_name(&self, tree: TreeId) -> Option<String> {
        let ty = self.compiler.ctx.tree(tree).ty?;
        let name = crate::analysis::type_names::type_name(&self.compiler.ctx, ty);
        (!name.is_empty() && name != "<none>").then_some(name)
    }
}

fn symbol_name(ctx: &crate::compiler_context::CompilerContext, symbol: SymbolId) -> String {
    ctx.sym(symbol)
        .name
        .to_java_string()
        .to_string_lossy()
        .to_string()
}

/// Overloads of `method`: the resolved declaration plus same-named methods of
/// its owner type.
fn method_candidates(
    ctx: &mut crate::compiler_context::CompilerContext,
    method: SymbolId,
) -> Vec<(SymbolId, bool)> {
    let mut candidates = vec![(method, false)];
    if let Some(owner) = ctx.sym(method).owner {
        let name = symbol_name(ctx, method);
        collect_owner_methods(ctx, owner, &name, &mut candidates);
    }
    candidates.sort_unstable();
    candidates.dedup();
    candidates
}

fn collect_owner_methods(
    ctx: &mut crate::compiler_context::CompilerContext,
    owner: SymbolId,
    name: &str,
    candidates: &mut Vec<(SymbolId, bool)>,
) {
    let Some(scope) = ctx.sym(owner).members() else {
        return;
    };
    let name = name.to_string();
    let members = ctx.scope_get_symbols(scope, LookupKind::NON_RECURSIVE, &|_| true);
    for member in members {
        if matches!(ctx.sym(member).kind_data, SymbolKind::MethodSymbol { .. })
            && symbol_name(ctx, member) == name
        {
            candidates.push((member, false));
        }
    }
}

/// The parenthesized argument list at `open` belongs to this call only when its
/// first argument starts after `open`.
fn arguments_match(
    ctx: &crate::compiler_context::CompilerContext,
    args: &[TreeId],
    open: usize,
) -> bool {
    let Some(first) = args.first() else {
        return true;
    };
    tree_info::get_start_pos(ctx, *first) > open as i32
}

/// Choose the overload matching the resolved argument types. `None` scores
/// (unknown argument type) fall back to the arity-based choice.
fn choose_signature(
    signatures: &[SignatureInformation],
    argument_types: &[Option<String>],
    active_parameter: u32,
) -> u32 {
    if signatures.len() == 1 {
        return 0;
    }
    let mut best = 0usize;
    let mut best_score: Option<i32> = None;
    for (index, signature) in signatures.iter().enumerate() {
        let Some(score) = signature_score(signature, argument_types) else {
            continue;
        };
        if best_score.map(|current| score > current).unwrap_or(true) {
            best_score = Some(score);
            best = index;
        }
    }
    if best_score.is_some() {
        return best as u32;
    }
    signatures
        .iter()
        .position(|signature| signature.parameters.len() as u32 > active_parameter)
        .unwrap_or(0) as u32
}

fn signature_score(
    signature: &SignatureInformation,
    argument_types: &[Option<String>],
) -> Option<i32> {
    let parameter_count = signature.parameters.len();
    let varargs = signature.parameters.last().is_some_and(|parameter| parameter.varargs);
    if !varargs && parameter_count != argument_types.len() {
        return None;
    }
    if varargs && argument_types.len() + 1 < parameter_count {
        return None;
    }
    let mut score = 0i32;
    for (index, argument) in argument_types.iter().enumerate() {
        let argument = argument.as_deref()?;
        let parameter = if index < parameter_count {
            &signature.parameters[index]
        } else {
            signature.parameters.last()?
        };
        let same = simple_type_name(argument) == simple_type_name(&parameter.type_name)
            || parameter.type_name == "Object";
        score += if same { 2 } else { -1 };
    }
    Some(score)
}

/// Head type without package qualifiers or type arguments.
fn simple_type_name(text: &str) -> &str {
    let head = text.split('<').next().unwrap_or(text);
    let head = head.split('[').next().unwrap_or(head);
    head.rsplit('.').next().unwrap_or(head)
}

fn open_paren_token(tokens: &[crate::parser::tokens::Token], cursor: i32) -> Option<usize> {
    let mut stack = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.pos >= cursor {
            break;
        }
        match token.kind {
            crate::parser::tokens::TokenKind::LPAREN => stack.push(index),
            crate::parser::tokens::TokenKind::RPAREN => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.last().copied()
}

fn ident_before_token(
    tokens: &[crate::parser::tokens::Token],
    open: usize,
) -> Option<String> {
    let token = tokens.get(open.checked_sub(1)?)?;
    if token.kind != crate::parser::tokens::TokenKind::IDENTIFIER {
        return None;
    }
    let name = token.name().to_java_string().to_string_lossy().to_string();
    (!name.is_empty()).then_some(name)
}

/// Whether the `(` token belongs to a method declaration's parameter list.
/// Only methods in the current compilation unit are considered.
fn is_declaration_paren(
    ctx: &crate::compiler_context::CompilerContext,
    root: Option<TreeId>,
    tokens: &[crate::parser::tokens::Token],
    open_token: usize,
    name: &str,
) -> bool {
    let Some(root) = root else {
        return false;
    };
    let Some(open) = tokens.get(open_token).map(|token| token.pos) else {
        return false;
    };
    let TreeKind::CompilationUnit { defs, .. } = &ctx.tree(root).kind else {
        return false;
    };
    defs.iter()
        .any(|def| class_has_declaration_paren(ctx, def, open, name))
}

fn class_has_declaration_paren(
    ctx: &crate::compiler_context::CompilerContext,
    tree: TreeId,
    open: i32,
    name: &str,
) -> bool {
    match &ctx.tree(tree).kind {
        TreeKind::ClassDecl { defs, .. } => defs.iter().any(|def| match &ctx.tree(def).kind {
            TreeKind::ClassDecl { .. } => class_has_declaration_paren(ctx, def, open, name),
            TreeKind::MethodDecl {
                name: method_name,
                body,
                ..
            } => {
                method_name.to_java_string().to_string_lossy() == name
                    && match body {
                        Some(body) => open < tree_info::get_start_pos(ctx, *body),
                        None => true,
                    }
            }
            _ => false,
        }),
        _ => false,
    }
}

/// Zero-based argument index from the tokens between the open paren and the
/// cursor; nested parentheses do not contribute commas.
fn parameter_index_tokens(
    tokens: &[crate::parser::tokens::Token],
    open: usize,
    cursor: i32,
) -> u32 {
    let mut depth = 0i32;
    let mut count = 0u32;
    for token in tokens.iter().skip(open + 1) {
        if token.pos >= cursor {
            break;
        }
        match token.kind {
            crate::parser::tokens::TokenKind::LPAREN => depth += 1,
            crate::parser::tokens::TokenKind::RPAREN => depth -= 1,
            crate::parser::tokens::TokenKind::COMMA if depth == 0 => count += 1,
            _ => {}
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overload_score_prefers_the_argument_type() {
        let int_signature = SignatureInformation {
            label: "void over(int a)".to_string(),
            parameters: vec![ParameterInformation {
                type_name: "int".to_string(),
                name: Some("a".to_string()),
                varargs: false,
            }],
            documentation: None,
        };
        let string_signature = SignatureInformation {
            label: "void over(String a)".to_string(),
            parameters: vec![ParameterInformation {
                type_name: "String".to_string(),
                name: Some("a".to_string()),
                varargs: false,
            }],
            documentation: None,
        };
        let signatures = vec![string_signature, int_signature];
        assert_eq!(
            choose_signature(&signatures, &[Some("int".to_string())], 0),
            1
        );
        assert_eq!(
            choose_signature(&signatures, &[Some("String".to_string())], 0),
            0
        );
    }

    #[test]
    fn varargs_signature_accepts_extra_arguments() {
        let signature = SignatureInformation {
            label: "void f(String... names)".to_string(),
            parameters: vec![ParameterInformation {
                type_name: "String".to_string(),
                name: Some("names".to_string()),
                varargs: true,
            }],
            documentation: None,
        };
        assert_eq!(
            signature_score(
                &signature,
                &[Some("String".to_string()), Some("String".to_string())],
            ),
            Some(4)
        );
    }
}
