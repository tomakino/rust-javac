//! Generate code actions: constructor, toString, equals/hashCode.
//!
//! Field lists, existing methods and the superclass constructor come from the
//! compiler symbol table. The class-body closing brace is located with a
//! bracket scan that skips comments and literals (an unambiguous structure,
//! like the bracket depth used by on-type formatting).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use javac_analysis_api::{CodeAction, Position, Range, TextEdit};

use crate::code::flags;
use crate::code::scope::LookupKind;
use crate::code::symbol::SymbolKind;
use crate::tree::jc_tree::TreeKind;

use super::super::{canonical, FileAnalysis};

struct ClassContext {
    class: crate::ids::SymbolId,
    name: String,
    fields: Vec<(String, String)>,
    has_constructor: bool,
    has_to_string: bool,
    has_equals: bool,
    has_hash_code: bool,
    super_ok: bool,
    insert: InsertPoint,
}

struct InsertPoint {
    line: u32,
    column: u32,
    indent: String,
    brace_indent: String,
    newline: &'static str,
}

fn type_name(ctx: &crate::compiler_context::CompilerContext, ty: crate::ids::TypeId) -> String {
    crate::analysis::type_names::type_name(ctx, ty)
}

fn method_arg_count(ctx: &crate::compiler_context::CompilerContext, method: crate::ids::SymbolId) -> usize {
    let Some(ty) = ctx.sym(method).ty else {
        return 0;
    };
    crate::code::types::get_parameter_types(ctx, ty).length() as usize
}

impl FileAnalysis {
    pub(crate) fn generate_actions(&mut self, file: &Path, position: Position) -> Vec<CodeAction> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let _ = self.analyze(file);
        let Some(context) = self.class_context(file, &text, position) else {
            return Vec::new();
        };
        let mut actions = Vec::new();
        let (indent, newline) = (&context.insert.indent, context.insert.newline);
        let types_known = context
            .fields
            .iter()
            .all(|(_, field_type)| !field_type.is_empty() && field_type != "?");
        if !context.fields.is_empty() && types_known && !context.has_constructor && context.super_ok {
            let params = context
                .fields
                .iter()
                .map(|(name, ty)| format!("{ty} {name}"))
                .collect::<Vec<_>>()
                .join(", ");
            let mut body = format!("{indent}public {}({params}) {{{newline}", context.name);
            for (name, _) in &context.fields {
                body.push_str(&format!("{indent}    this.{name} = {name};{newline}"));
            }
            body.push_str(&format!("{indent}}}{newline}"));
            actions.push(CodeAction {
                title: "Generate constructor".to_string(),
                edits: vec![context.insert.edit(body)],
                diagnostics: Vec::new(),
            });
        }
        if !context.fields.is_empty() && !context.has_to_string {
            let mut body = format!("{indent}@Override{newline}");
            body.push_str(&format!("{indent}public String toString() {{{newline}"));
            body.push_str(&format!("{indent}    return \"{}{{\" +{newline}", context.name));
            for (index, (name, _)) in context.fields.iter().enumerate() {
                let separator = if index == 0 { "" } else { ", " };
                body.push_str(&format!(
                    "{indent}        \"{separator}{name}=\" + {name} +{newline}"
                ));
            }
            body.push_str(&format!("{indent}    '}}';{newline}"));
            body.push_str(&format!("{indent}}}{newline}"));
            actions.push(CodeAction {
                title: "Generate toString()".to_string(),
                edits: vec![context.insert.edit(body)],
                diagnostics: Vec::new(),
            });
        }
        if !context.fields.is_empty() && (!context.has_equals || !context.has_hash_code) {
            let mut edits = Vec::new();
            let imports_objects = text.contains("import java.util.Objects;");
            if !imports_objects {
                let (import_line, import_newline, _) =
                    crate::analysis::features::actions::import_insert_point(&text);
                edits.push(TextEdit {
                    range: Range::point(Position::new(import_line, 0)),
                    new_text: format!("import java.util.Objects;{import_newline}"),
                });
            }
            let mut body = String::new();
            if !context.has_equals {
                body.push_str(&format!("{indent}@Override{newline}"));
                body.push_str(&format!(
                    "{indent}public boolean equals(Object o) {{{newline}"
                ));
                body.push_str(&format!("{indent}    if (this == o) return true;{newline}"));
                body.push_str(&format!(
                    "{indent}    if (!(o instanceof {})) return false;{newline}",
                    context.name
                ));
                body.push_str(&format!(
                    "{indent}    {} other = ({}) o;{newline}",
                    context.name, context.name
                ));
                let comparisons = context
                    .fields
                    .iter()
                    .map(|(name, _)| format!("Objects.equals({name}, other.{name})"))
                    .collect::<Vec<_>>()
                    .join(" && ");
                body.push_str(&format!("{indent}    return {comparisons};{newline}"));
                body.push_str(&format!("{indent}}}{newline}"));
            }
            if !context.has_hash_code {
                body.push_str(&format!("{indent}@Override{newline}"));
                body.push_str(&format!("{indent}public int hashCode() {{{newline}"));
                let names = context
                    .fields
                    .iter()
                    .map(|(name, _)| name.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                body.push_str(&format!("{indent}    return Objects.hash({names});{newline}"));
                body.push_str(&format!("{indent}}}{newline}"));
            }
            edits.push(context.insert.edit(body));
            actions.push(CodeAction {
                title: "Generate equals() and hashCode()".to_string(),
                edits,
                diagnostics: Vec::new(),
            });
        }
        actions
    }

    fn class_context(
        &mut self,
        file: &Path,
        text: &str,
        position: Position,
    ) -> Option<ClassContext> {
        let target = PathBuf::from(canonical(file));
        let mut classes: Vec<(crate::ids::TreeId, usize, usize)> = Vec::new();
        for index in 0..self.compiler.ctx.trees.len() {
            let unit = crate::ids::TreeId::new(index as u32);
            let (source, defs) = match &self.compiler.ctx.tree(unit).kind {
                TreeKind::CompilationUnit {
                    sourcefile: Some(source),
                    defs,
                    ..
                } => (source.get_name().to_string_lossy().to_string(), defs.clone()),
                _ => continue,
            };
            if PathBuf::from(canonical(Path::new(&source))) != target {
                continue;
            }
            for def in defs.iter() {
                if let TreeKind::ClassDecl { .. } = &self.compiler.ctx.tree(def).kind {
                    // Tree positions are UTF-16 offsets; text scanning is bytes.
                    let pos_utf16 = self.compiler.ctx.tree(def).pos.max(0) as usize;
                    let pos = crate::analysis::positions::utf16_to_byte_snapped(text, pos_utf16);
                    let close = find_char(text, pos, '{')
                        .and_then(|open| matching_brace(text, open))
                        .unwrap_or(pos);
                    classes.push((def, pos, close));
                }
            }
        }
        // The class containing the cursor (structure only).
        let cursor = crate::analysis::positions::line_offset(
            text,
            position.line as usize + 1,
            position.column as usize + 1,
        );
        let class_tree = classes
            .iter()
            .find(|(_, pos, close)| *pos <= cursor && cursor <= *close)
            .map(|(tree, _, _)| *tree)?;
        let class = match &self.compiler.ctx.tree(class_tree).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => return None,
        };
        let class_pos_utf16 = self.compiler.ctx.tree(class_tree).pos.max(0);
        let class_pos = crate::analysis::positions::utf16_to_byte_snapped(text, class_pos_utf16 as usize);
        // Declaration order comes from the AST (VariableDecl children).
        let (defs, field_order) = match &self.compiler.ctx.tree(class_tree).kind {
            TreeKind::ClassDecl { defs, .. } => {
                let order = defs
                    .iter()
                    .filter_map(|def| match &self.compiler.ctx.tree(def).kind {
                        TreeKind::VariableDecl { name, .. } => {
                            Some(name.to_java_string().to_string_lossy().to_string())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                (defs.clone(), order)
            }
            _ => return None,
        };
        let name = self
            .compiler
            .ctx
            .sym(class)
            .name
            .to_java_string()
            .to_string_lossy()
            .to_string();
        let members = self.compiler.ctx.sym(class).members()?;
        let symbols = self.compiler.ctx.scope_get_symbols(
            members,
            LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        let mut field_symbols: Vec<(String, crate::ids::TypeId, String)> = Vec::new();
        let mut has_to_string = false;
        let mut has_equals = false;
        let mut has_hash_code = false;
        let mut seen = HashSet::new();
        for symbol in symbols {
            let symbol_ref = self.compiler.ctx.sym(symbol);
            let is_static = (symbol_ref.flags() & flags::STATIC as i64) != 0;
            match &symbol_ref.kind_data {
                SymbolKind::VarSymbol { .. } => {
                    if is_static {
                        continue;
                    }
                    let field_name = symbol_ref
                        .name
                        .to_java_string()
                        .to_string_lossy()
                        .to_string();
                    if !seen.insert(field_name.clone()) {
                        continue;
                    }
                    let Some(ty) = symbol_ref.ty else {
                        continue;
                    };
                    let display = type_name(&self.compiler.ctx, ty);
                    field_symbols.push((field_name, ty, display));
                }
                SymbolKind::MethodSymbol { .. } => {
                    let method_name = symbol_ref
                        .name
                        .to_java_string()
                        .to_string_lossy()
                        .to_string();
                    if method_name == "toString" {
                        has_to_string = true;
                    } else if method_name == "equals" {
                        has_equals = true;
                    } else if method_name == "hashCode" {
                        has_hash_code = true;
                    }
                }
                _ => {}
            }
        }
        field_symbols.sort_by_key(|(field_name, _, _)| {
            field_order
                .iter()
                .position(|name| name == field_name)
                .unwrap_or(usize::MAX)
        });
        let fields: Vec<(String, String)> = field_symbols
            .iter()
            .map(|(field_name, _, display)| (field_name.clone(), display.clone()))
            .collect();
        // Erased field types, in declaration order.
        let mut field_erased: Vec<String> = Vec::new();
        for (_, ty, _) in &field_symbols {
            let erased = {
                let compiler = &mut self.compiler;
                let types = &compiler.types;
                let ctx = &mut compiler.ctx;
                let syms = &compiler.syms;
                types.erasure(ctx, syms, *ty)
            };
            field_erased.push(type_name(&self.compiler.ctx, erased));
        }
        // Same-signature constructor check (explicit constructors only,
        // erased parameter types).
        let mut has_constructor = false;
        for def in defs.iter() {
            let (method_name, method_sym) = match &self.compiler.ctx.tree(def).kind {
                TreeKind::MethodDecl { name, sym, .. } => (
                    name.to_java_string().to_string_lossy().to_string(),
                    *sym,
                ),
                _ => continue,
            };
            if !(method_name == name || method_name == "<init>")
                || self.compiler.ctx.tree(def).pos <= class_pos_utf16
            {
                continue;
            }
            let Some(method_sym) = method_sym else {
                continue;
            };
            let Some(ty) = self.compiler.ctx.sym(method_sym).ty else {
                continue;
            };
            let params: Vec<crate::ids::TypeId> =
                crate::code::types::get_parameter_types(&self.compiler.ctx, ty)
                    .iter()
                    .collect();
            let mut param_names = Vec::new();
            for param in params {
                let erased = {
                    let compiler = &mut self.compiler;
                    let types = &compiler.types;
                    let ctx = &mut compiler.ctx;
                    let syms = &compiler.syms;
                    types.erasure(ctx, syms, param)
                };
                param_names.push(type_name(&self.compiler.ctx, erased));
            }
            if param_names == field_erased {
                has_constructor = true;
                break;
            }
        }
        let super_ok = self.super_has_no_arg_constructor(class);
        let insert = insert_point(text, class_pos)?;
        Some(ClassContext {
            class,
            name,
            fields,
            has_constructor,
            has_to_string,
            has_equals,
            has_hash_code,
            super_ok,
            insert,
        })
    }
    fn super_has_no_arg_constructor(&mut self, class: crate::ids::SymbolId) -> bool {
        let Some(class_type) = self.compiler.ctx.sym(class).ty else {
            return true;
        };
        let supertype = {
            let compiler = &mut self.compiler;
            let types = &compiler.types;
            let ctx = &mut compiler.ctx;
            let syms = &compiler.syms;
            types.supertype(ctx, syms, class_type, None)
        };
        let Some(parent) = self.compiler.ctx.typ(supertype).tsym else {
            return true;
        };
        let Some(members) = self.compiler.ctx.sym(parent).members() else {
            return true;
        };
        let symbols = self.compiler.ctx.scope_get_symbols(
            members,
            LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        let mut constructors = Vec::new();
        for symbol in symbols {
            let symbol_ref = self.compiler.ctx.sym(symbol);
            if let SymbolKind::MethodSymbol { .. } = symbol_ref.kind_data {
                let name = symbol_ref.name.to_java_string().to_string_lossy();
                if name == "<init>" {
                    constructors.push(symbol);
                }
            }
        }
        constructors.is_empty()
            || constructors
                .iter()
                .any(|constructor| method_arg_count(&self.compiler.ctx, *constructor) == 0)
    }
}

impl InsertPoint {
    fn edit(&self, mut body: String) -> TextEdit {
        body.push_str(&self.brace_indent);
        TextEdit {
            range: Range::new(Position::new(self.line, 0), Position::new(self.line, self.column)),
            new_text: body,
        }
    }
}

fn insert_point(text: &str, class_pos: usize) -> Option<InsertPoint> {
    let open = find_char(text, class_pos, '{')?;
    let close = matching_brace(text, open)?;
    let line_start = text[..close].rfind('\n').map(|index| index + 1).unwrap_or(0);
    let line = text[..line_start].matches('\n').count() as u32;
    let column = text[line_start..close]
        .chars()
        .map(|character| character.len_utf16() as u32)
        .sum();
    let brace_indent: String = text[line_start..close].to_string();
    let indent = format!("{brace_indent}    ");
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    Some(InsertPoint {
        line,
        column,
        indent,
        brace_indent,
        newline,
    })
}

/// Scan for `needle` from `start`, skipping comments and string literals.
/// The class body opener: the first `{` after the declaration (the text
/// between the class keyword and its body has no comments or literals).
fn find_char(text: &str, start: usize, needle: char) -> Option<usize> {
    text[start..]
        .char_indices()
        .find(|(_, character)| *character == needle)
        .map(|(index, _)| start + index)
}

/// Matching `}` for the `{` at `open`, skipping comments and literals.
fn matching_brace(text: &str, open: usize) -> Option<usize> {
    let chars: Vec<(usize, char)> = text[open..].char_indices().collect();
    let mut depth = 0usize;
    let mut index = 0usize;
    while index < chars.len() {
        let (offset, character) = chars[index];
        let position = open + offset;
        if character == '/' && index + 1 < chars.len() {
            let next = chars[index + 1].1;
            if next == '/' {
                index += 2;
                while index < chars.len() && chars[index].1 != '\n' {
                    index += 1;
                }
                continue;
            }
            if next == '*' {
                index += 2;
                while index < chars.len() {
                    if chars[index].1 == '*' && index + 1 < chars.len() && chars[index + 1].1 == '/' {
                        index += 2;
                        break;
                    }
                    index += 1;
                }
                continue;
            }
        }
        if character == '"' || character == '\'' {
            let quote = character;
            index += 1;
            let mut escaped = false;
            while index < chars.len() {
                let value = chars[index].1;
                index += 1;
                if escaped {
                    escaped = false;
                } else if value == '\\' {
                    escaped = true;
                } else if value == quote {
                    break;
                }
            }
            continue;
        }
        if character == '{' {
            depth += 1;
        } else if character == '}' {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(position);
            }
        }
        index += 1;
    }
    None
}
