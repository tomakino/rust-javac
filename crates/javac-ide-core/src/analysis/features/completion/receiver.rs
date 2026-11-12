use std::path::Path;

use super::super::super::positions::prefix_at;
use super::super::super::FileAnalysis;
use super::common::{completion_item, CallContext};
use super::context::InsertionContext;
use crate::tree::jc_tree::TreeKind;



impl FileAnalysis {
    fn literal_class_at(&self, cursor_utf16: usize) -> Option<&'static str> {
        let mut best: Option<(usize, &'static str)> = None;
        for (index, tree) in self.compiler.ctx.trees.iter().enumerate() {
            let TreeKind::Literal {
                value: Some(value), ..
            } = &tree.kind
            else {
                continue;
            };
            let class_name = match value {
                crate::tree::jc_tree::LiteralValue::String(_) => "String",
                crate::tree::jc_tree::LiteralValue::Character(_) => "Character",
                crate::tree::jc_tree::LiteralValue::Boolean(_) => "Boolean",
                crate::tree::jc_tree::LiteralValue::Integer(_)
                | crate::tree::jc_tree::LiteralValue::Byte(_)
                | crate::tree::jc_tree::LiteralValue::Short(_) => "Integer",
                crate::tree::jc_tree::LiteralValue::Long(_) => "Long",
                crate::tree::jc_tree::LiteralValue::Float(_) => "Float",
                crate::tree::jc_tree::LiteralValue::Double(_) => "Double",
            };
            let tree = crate::ids::TreeId::new(index as u32);
            let Some(end) = self.tree_end_utf16_for_completion(tree) else {
                continue;
            };
            if end <= cursor_utf16
                && cursor_utf16 <= end + 1
                && best.as_ref().is_none_or(|(best_end, _)| end > *best_end)
            {
                best = Some((end, class_name));
            }
        }
        best.map(|(_, class_name)| class_name)
    }

    fn tree_end_utf16_for_completion(&self, tree: crate::ids::TreeId) -> Option<usize> {
        for root in self.compiler.ctx.trees.iter() {
            if let TreeKind::CompilationUnit { end_positions, .. } = &root.kind {
                let end = crate::tree::tree_info::get_end_pos(
                    &self.compiler.ctx,
                    Some(tree),
                    end_positions.as_deref(),
                );
                if end >= 0 {
                    return Some(end as usize);
                }
            }
        }
        None
    }


    /// Receiver completion. Returns None when this is not a receiver context,
    /// Some(items) otherwise (including the empty list for unresolved receivers).
    pub(super) fn receiver_completion(
        &mut self,
        file: &Path,
        text: &str,
        position: javac_analysis_api::Position,
        insertion: InsertionContext,
        tokens: &[crate::parser::tokens::Token],
    ) -> Option<Vec<javac_analysis_api::CompletionItem>> {
        self.receiver_prefix_consumed = false;
        let prefix = prefix_at(
            text,
            position.line as usize + 1,
            position.column as usize + 1,
        );
        let cursor_byte = super::super::super::positions::line_offset(
            text,
            position.line as usize + 1,
            position.column as usize + 1,
        );
        let before_utf16 = text[..cursor_byte.saturating_sub(prefix.len())]
            .encode_utf16()
            .count();
        let before: Vec<&crate::parser::tokens::Token> = tokens
            .iter()
            .take_while(|token| token.pos >= 0 && (token.pos as usize) < before_utf16)
            .collect();
        let last = *before.last()?;
        if last.kind == crate::parser::tokens::TokenKind::ERROR {
            return Some(Vec::new());
        }
        if last.kind != crate::parser::tokens::TokenKind::DOT {
            let current_class = self.file_class_symbol(file);
            let call = CallContext {
                current_class,
                current_package: String::new(),
                insertion,
            };
            if let Some(class_name) = self.literal_class_at(before_utf16) {
                return Some(self.literal_receiver(class_name, &call));
            }
            return None;
        }
        if before.len() >= 2
            && before[before.len() - 2].kind == crate::parser::tokens::TokenKind::DOT
        {
            return Some(Vec::new());
        }
        let current_class = self.file_class_symbol(file);
        let current_package = current_class
            .map(|class| self.package_flat(class))
            .unwrap_or_default();
        let call = CallContext {
            current_class,
            current_package: current_package.clone(),
            insertion,
        };
        if before.len() < 2 {
            return Some(self.ast_receiver(text, position, &call));
        }
        if let Some(class_name) = self.literal_class_at(before_utf16) {
            return Some(self.literal_receiver(class_name, &call));
        }
        let previous = before[before.len() - 2];
        if matches!(
            previous.kind,
            crate::parser::tokens::TokenKind::RPAREN
                | crate::parser::tokens::TokenKind::RBRACKET
        ) {
            return Some(self.ast_receiver(text, position, &call));
        }
        // Walk the identifier/dot chain immediately before the final dot.
        let mut names: Vec<String> = Vec::new();
        let mut index = before.len() - 1;
        while index > 0 {
            let token = before[index - 1];
            match token.kind {
                crate::parser::tokens::TokenKind::IDENTIFIER => {
                    names.push(token.name().to_string());
                    index -= 1;
                }
                crate::parser::tokens::TokenKind::THIS => {
                    names.push("this".to_string());
                    index -= 1;
                }
                crate::parser::tokens::TokenKind::SUPER => {
                    names.push("super".to_string());
                    index -= 1;
                }
                crate::parser::tokens::TokenKind::TRUE | crate::parser::tokens::TokenKind::FALSE => {
                    names.push("true".to_string());
                    index -= 1;
                }
                crate::parser::tokens::TokenKind::NULL => {
                    names.push("null".to_string());
                    index -= 1;
                }
                crate::parser::tokens::TokenKind::DOT => index -= 1,
                _ => break,
            }
        }
        if names.is_empty() {
            return Some(self.ast_receiver(text, position, &call));
        }
        names.reverse();
        let receiver = names.last().cloned().unwrap_or_default();
        let chain = names.join(".");
        match receiver.as_str() {
            "this" => {
                return Some(match current_class {
                    Some(class) => self.members_of_type(class, &call, false, false),
                    None => Vec::new(),
                });
            }
            "super" => {
                return Some(match current_class.and_then(|class| self.superclass_of(class)) {
                    Some(class) => self.members_of_type(class, &call, false, true),
                    None => Vec::new(),
                });
            }
            "true" | "false" => return Some(self.literal_receiver("Boolean", &call)),
            "null" => return Some(Vec::new()),
            _ => {}
        }
        if let Some(ty) = self.variable_type(&receiver) {
            return Some(self.members_of_type_id(ty, &call, false));
        }
        if let Some(class) = self.class_symbol_named(&receiver) {
            return Some(self.members_of_type(class, &call, true, false));
        }
        // Fully qualified class typed as a prefix: "java.util.Base64" expands
        // its static members and nested types, like IDEA.
        if !prefix.is_empty() {
            let qualified = format!("{chain}.{prefix}");
            if let Some(mut items) = self.qualified_class_members(&qualified, &call) {
                // The prefix is part of the receiver: keep it and insert
                // ".getDecoder()" after it instead of replacing "Base64".
                for item in &mut items {
                    let text = item
                        .insert_text
                        .clone()
                        .unwrap_or_else(|| item.label.clone());
                    item.insert_text = Some(format!(".{text}"));
                }
                self.receiver_prefix_consumed = true;
                return Some(items);
            }
        }
        if let Some(items) = self.package_members(&chain) {
            return Some(items);
        }
        if let Some(items) = self.qualified_class_members(&chain, &call) {
            return Some(items);
        }
        Some(self.ast_receiver(text, position, &call))
    }

    pub(super) fn literal_receiver(
        &mut self,
        class_name: &str,
        call: &CallContext,
    ) -> Vec<javac_analysis_api::CompletionItem> {
        match self.class_by_simple_name(class_name) {
            Some(class) => self.members_of_type(class, call, false, false),
            None => Vec::new(),
        }
    }

    /// Classes and subpackages of a package (java.util. -> Arrays, stream, ...).
    pub(super) fn package_members(
        &mut self,
        name: &str,
    ) -> Option<Vec<javac_analysis_api::CompletionItem>> {
        if name.is_empty() {
            return None;
        }
        let package = self
            .compiler
            .syms
            .lookup_package_str(&mut self.compiler.ctx, name)?;
        let mut items = Vec::new();
        for (subpackage, _) in self
            .compiler
            .syms
            .subpackages_of(&self.compiler.ctx, name)
        {
            let mut item = completion_item(
                subpackage.clone(),
                javac_analysis_api::SymbolKind::Package,
                None,
                None,
            );
            item.insert_text = Some(format!("{subpackage}."));
            item.insert_text_format = javac_analysis_api::InsertTextFormat::PlainText;
            items.push(item);
        }
        let Some(scope) = self.compiler.ctx.sym(package).members() else {
            // Packages without direct classes still contribute subpackages.
            return Some(items);
        };
        let members = self.compiler.ctx.scope_get_symbols(
            scope,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        for member in members {
            let symbol = self.compiler.ctx.sym(member);
            let simple = symbol.name.to_java_string().to_string_lossy();
            if simple.starts_with('<') {
                continue;
            }
            match symbol.kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { .. } => {
                    // The package completer also enters nested class files under
                    // their flat binary name (Base64$Decoder). They are not
                    // package members IDEA offers, and completing them here
                    // rewrites their owner/name before their outer class is
                    // read, which poisons a later qualified completion
                    // (java.util.Base64 -> Decoder/Encoder). Skip them.
                    if symbol.flat_name().to_string().contains('$') {
                        continue;
                    }
                    // Class file symbols fill their type parameters during completion.
                    let _ = self
                        .compiler
                        .ctx
                        .complete_symbol(&self.compiler.syms, member);
                    let parameters = self.class_type_parameters(member);
                    let flat = format!("{name}.{simple}");
                    let mut item = completion_item(
                        format!("{simple}{parameters}"),
                        javac_analysis_api::SymbolKind::Class,
                        Some(name.to_string()),
                        None,
                    );
                    // The visible label carries <E>, the inserted text does not.
                    item.insert_text = Some(simple);
                    item.insert_text_format = javac_analysis_api::InsertTextFormat::PlainText;
                    item.documentation = self.class_documentation(&flat);
                    items.push(item);
                }
                crate::code::symbol::SymbolKind::PackageSymbol { .. } => {
                    let mut item = completion_item(
                        simple.clone(),
                        javac_analysis_api::SymbolKind::Package,
                        None,
                        None,
                    );
                    item.insert_text = Some(format!("{simple}."));
                    item.insert_text_format = javac_analysis_api::InsertTextFormat::PlainText;
                    items.push(item);
                }
                _ => {}
            }
        }
        Some(items)
    }

}