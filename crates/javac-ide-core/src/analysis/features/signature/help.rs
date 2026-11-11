//! Signature help.
//!
//! The call is located through the attributed trees (a `MethodInvocation` or
//! `NewClass` whose callee name ends at the `(`), so overloads are chosen by
//! the resolved argument types, constructors are supported, and methods
//! declared in other project/JDK files resolve through their symbols.

use std::path::{Path, PathBuf};

use javac_analysis_api::{SignatureHelp, SignatureInformation};

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
        let open = call_open_offset(&text, cursor)?;
        // Editing a declaration's parameter list is not a call.
        if is_declaration_paren(&text, open) {
            return None;
        }
        let name = ident_before(&text, open);
        if name.is_empty() {
            return None;
        }
        let active_parameter = parameter_index(&text[open + 1..cursor]);
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
        let mut signatures = Vec::new();
        for symbol in self.symbols() {
            if symbol.kind != "method" || symbol.name != name {
                continue;
            }
            let label = symbol.detail;
            let parameters = signature_parameters(&label);
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
            parameters: parts.parameters,
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
    let parameter_types: Vec<&str> = signature
        .parameters
        .iter()
        .map(|label| label.split_once(' ').map(|(ty, _)| ty).unwrap_or(label))
        .collect();
    let varargs = parameter_types
        .last()
        .is_some_and(|ty| ty.ends_with("..."));
    if !varargs && parameter_types.len() != argument_types.len() {
        return None;
    }
    if varargs && argument_types.len() + 1 < parameter_types.len() {
        return None;
    }
    let mut score = 0i32;
    for (index, argument) in argument_types.iter().enumerate() {
        let argument = argument.as_deref()?;
        let parameter = if index < parameter_types.len() {
            parameter_types[index]
        } else {
            *parameter_types.last()?
        };
        let parameter = parameter.trim_end_matches("...");
        let same = argument == parameter
            || argument.ends_with(&format!(".{parameter}"))
            || parameter.ends_with(&format!(".{argument}"))
            || parameter == "Object";
        score += if same { 2 } else { -1 };
    }
    Some(score)
}

fn ident_before(text: &str, open: usize) -> String {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text[..open]
        .chars()
        .rev()
        .take_while(|c| is_ident(*c))
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

/// Whether the parenthesized list at `open` belongs to a declaration.
///
/// A declaration is preceded by a type/modifier (`void f(`); a call is
/// preceded by an operator, `.`, `new`, `return`, or starts the statement.
fn is_declaration_paren(text: &str, open: usize) -> bool {
    let is_ident = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '$';
    let before = text[..open].trim_end();
    let name_start = before
        .rfind(|ch: char| !is_ident(ch))
        .map(|index| index + 1)
        .unwrap_or(0);
    let name = &before[name_start..];
    if name.is_empty() {
        return false;
    }
    let prefix = before[..name_start].trim_end();
    if prefix.is_empty() {
        // Statement-leading call, e.g. `f(1);`.
        return false;
    }
    let last = prefix.chars().next_back().expect("non-empty prefix");
    if !is_ident(last) {
        return false;
    }
    let token_start = prefix
        .rfind(|ch: char| !is_ident(ch))
        .map(|index| index + 1)
        .unwrap_or(0);
    let token = &prefix[token_start..];
    !matches!(
        token,
        "new" | "return"
            | "throw"
            | "if"
            | "while"
            | "for"
            | "switch"
            | "catch"
            | "synchronized"
            | "assert"
            | "case"
            | "else"
            | "yield"
            | "do"
            | "try"
            | "finally"
            | "instanceof"
            | "super"
            | "this"
    )
}

/// The byte offset of the `(` that opens the call containing `cursor`.
///
/// Scans backwards to support calls that span multiple lines, and stops at a
/// statement boundary when the parentheses are balanced.
fn call_open_offset(text: &str, cursor: usize) -> Option<usize> {
    let cursor = cursor.min(text.len());
    let mut depth = 0i32;
    for (index, ch) in text[..cursor].char_indices().rev() {
        match ch {
            ')' => depth += 1,
            '(' => {
                if depth == 0 {
                    return Some(index);
                }
                depth -= 1;
            }
            ';' | '{' | '}' if depth == 0 => return None,
            _ => {}
        }
    }
    None
}

/// Zero-based index of the argument at the cursor.
///
/// Commas inside nested calls, strings, characters, and comments are ignored.
fn parameter_index(text: &str) -> u32 {
    let mut depth = 0i32;
    let mut count = 0u32;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => skip_string(&mut chars, '"'),
            '\'' => skip_string(&mut chars, '\''),
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut previous = '\0';
                for c in chars.by_ref() {
                    if previous == '*' && c == '/' {
                        break;
                    }
                    previous = c;
                }
            }
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => count += 1,
            _ => {}
        }
    }
    count
}

fn skip_string(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, quote: char) {
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            chars.next();
        } else if ch == quote {
            break;
        }
    }
}

/// Split the parameter list out of a method signature label.
fn signature_parameters(label: &str) -> Vec<String> {
    let Some(open) = label.find('(') else {
        return Vec::new();
    };
    let Some(close) = label[open..].find(')') else {
        return Vec::new();
    };
    let inner = &label[open + 1..open + close];
    if inner.trim().is_empty() {
        return Vec::new();
    }
    inner.split(", ").map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_calls_do_not_count_inner_commas() {
        assert_eq!(parameter_index("g(1, 2), "), 1);
        assert_eq!(parameter_index("\"a,b\", "), 1);
        assert_eq!(parameter_index("(1, 2), "), 1);
        assert_eq!(parameter_index("1, \"x,y\", "), 2);
    }

    #[test]
    fn multi_line_calls_find_the_open_paren() {
        let text = "class A {\n    void g() { f(\n        1,\n    }\n}\n";
        let cursor = text.find("1,").unwrap();
        let open = call_open_offset(text, cursor).expect("open");
        assert_eq!(&text[open..open + 1], "(");
    }

    #[test]
    fn overload_score_prefers_the_argument_type() {
        let int_signature = SignatureInformation {
            label: "void over(int a)".to_string(),
            parameters: vec!["int a".to_string()],
            documentation: None,
        };
        let string_signature = SignatureInformation {
            label: "void over(String a)".to_string(),
            parameters: vec!["String a".to_string()],
            documentation: None,
        };
        let signatures = vec![string_signature, int_signature];
        assert_eq!(
            choose_signature(
                &signatures,
                &[Some("int".to_string())],
                0
            ),
            1
        );
        assert_eq!(
            choose_signature(
                &signatures,
                &[Some("String".to_string())],
                0
            ),
            0
        );
    }
}
