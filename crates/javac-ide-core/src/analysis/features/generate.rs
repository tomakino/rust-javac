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
            // Reuse the Add-import exclusion helper: exact imports and
            // wildcard imports (java.util.*) both make Objects available.
            let (exact_imports, wildcard_imports) =
                crate::analysis::features::actions::imports_of(&text);
            let imports_objects = exact_imports.contains("java.util.Objects")
                || wildcard_imports.contains("java.util");
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
        // Class ranges come from the filled end-position table (post-order
        // text fill), not from text scanning. Each entry is (tree, class start
        // byte, class end byte, class end UTF-16).
        let table = self.end_positions_for(file);
        // The class containing the cursor: every ClassDecl in the arena
        // (nested classes included), innermost wins (largest pos).
        let cursor = crate::analysis::positions::line_offset(
            text,
            position.line as usize + 1,
            position.column as usize + 1,
        );
        let mut best: Option<(crate::ids::TreeId, i32, i32)> = None;
        for index in 0..self.compiler.ctx.trees.len() {
            let tree = crate::ids::TreeId::new(index as u32);
            if !matches!(self.compiler.ctx.tree(tree).kind, TreeKind::ClassDecl { .. }) {
                continue;
            }
            let pos = self.compiler.ctx.tree(tree).pos;
            if pos < 0 {
                continue;
            }
            let end = crate::tree::end_position::end_position(
                &self.compiler.ctx,
                text,
                tree,
                table
                    .as_ref()
                    .map(|table| table.as_ref() as &dyn crate::tree::end_pos_table::EndPosTable),
            );
            if end <= 0 {
                continue;
            }
            let pos_byte = crate::analysis::positions::utf16_to_byte_snapped(text, pos as usize);
            let end_byte = crate::analysis::positions::utf16_to_byte_snapped(text, end as usize);
            if cursor < pos_byte || cursor > end_byte {
                continue;
            }
            if best.map_or(true, |(_, best_pos, _)| pos > best_pos) {
                best = Some((tree, pos, end));
            }
        }
        let (class_tree, class_end_utf16) = best.map(|(tree, _, end)| (tree, end))?;
        let class = match &self.compiler.ctx.tree(class_tree).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => return None,
        };
        let class_pos_utf16 = self.compiler.ctx.tree(class_tree).pos.max(0);        // Declaration order comes from the AST (VariableDecl children).
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
                SymbolKind::MethodSymbol { .. } => {}
                _ => {}
            }
        }
        // Existing methods are matched by erased parameter types (not names):
        // a `toString(int)` or `equals(String)` overload does not conflict.
        for def in defs.iter() {
            let (method_name, method_sym) = match &self.compiler.ctx.tree(def).kind {
                TreeKind::MethodDecl { name, sym, .. } => (
                    name.to_java_string().to_string_lossy().to_string(),
                    *sym,
                ),
                _ => continue,
            };
            if !(method_name == "toString" || method_name == "equals" || method_name == "hashCode")
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
            let mut erased = Vec::new();
            for param in params {
                let erasure = {
                    let compiler = &mut self.compiler;
                    let types = &compiler.types;
                    let ctx = &mut compiler.ctx;
                    let syms = &compiler.syms;
                    types.erasure(ctx, syms, param)
                };
                erased.push(type_name(&self.compiler.ctx, erasure));
            }
            let matches = match method_name.as_str() {
                "toString" | "hashCode" => erased.is_empty(),
                "equals" => erased.len() == 1 && erased[0] == "Object",
                _ => false,
            };
            if matches {
                match method_name.as_str() {
                    "toString" => has_to_string = true,
                    "equals" => has_equals = true,
                    _ => has_hash_code = true,
                }
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
        let insert = insert_point(text, class_end_utf16)?;
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

fn insert_point(text: &str, class_end_utf16: i32) -> Option<InsertPoint> {
    if class_end_utf16 <= 0 {
        return None;
    }
    // The class end is just past its closing brace (AST end position).
    let brace_utf16 = (class_end_utf16 - 1) as usize;
    let brace_byte = crate::analysis::positions::utf16_to_byte_snapped(text, brace_utf16);
    if text.as_bytes().get(brace_byte) != Some(&b'}') {
        return None;
    }
    let line_start = text[..brace_byte]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let line = text[..line_start].matches('\n').count() as u32;
    let column = text[line_start..brace_byte]
        .chars()
        .map(|character| character.len_utf16() as u32)
        .sum();
    let brace_indent: String = text[line_start..brace_byte].to_string();
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