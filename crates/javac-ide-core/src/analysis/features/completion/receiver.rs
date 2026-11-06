use std::path::Path;

use super::super::super::positions::{char_index_for_utf16, prefix_at};
use super::super::super::FileAnalysis;
use super::common::{completion_item, CallContext};
use super::context::InsertionContext;

impl FileAnalysis {
    /// Receiver completion. Returns None when this is not a receiver context,
    /// Some(items) otherwise (including the empty list for unresolved receivers).
    pub(super) fn receiver_completion(
        &mut self,
        file: &Path,
        text: &str,
        position: javac_analysis_api::Position,
        insertion: InsertionContext,
    ) -> Option<Vec<javac_analysis_api::CompletionItem>> {
        self.receiver_prefix_consumed = false;
        let line_text = text.lines().nth(position.line as usize)?;
        let chars: Vec<char> = line_text.chars().collect();
        let cursor = char_index_for_utf16(&chars, position.column as usize);
        let prefix = prefix_at(
            text,
            position.line as usize + 1,
            position.column as usize + 1,
        );
        let mut before: String = chars[..cursor].iter().collect();
        if !prefix.is_empty() {
            let keep: String = before
                .chars()
                .rev()
                .skip(prefix.chars().count())
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            before = keep;
        }
        let trimmed = before.trim_end();
        if !trimmed.ends_with('.') {
            return None;
        }
        // "System.." is not valid Java; do not fall back to the AST and invent
        // members for a broken expression.
        if trimmed.ends_with("..") {
            return Some(Vec::new());
        }
        let trimmed_chars: Vec<char> = trimmed.chars().collect();
        let dot = trimmed_chars.len() - 1;
        let current_class = self.file_class_symbol(file);
        let current_package = current_class
            .map(|class| self.package_flat(class))
            .unwrap_or_default();
        let call = CallContext {
            current_class,
            current_package: current_package.clone(),
            insertion,
        };
        // Literals.
        if dot > 0 {
            let prev = trimmed_chars[dot - 1];
            if prev == '"' || prev == '\'' {
                let quote = prev;
                let mut open = dot - 1;
                while open > 0 {
                    open -= 1;
                    if trimmed_chars[open] == quote
                        && (open == 0 || trimmed_chars[open - 1] != '\\')
                    {
                        break;
                    }
                }
                let literal: String = trimmed_chars[open..dot].iter().collect();
                return Some(self.literal_receiver(&literal, &call));
            }
            if prev.is_ascii_digit() {
                let mut start = dot;
                while start > 0
                    && (trimmed_chars[start - 1].is_ascii_digit()
                        || trimmed_chars[start - 1] == '.'
                        || trimmed_chars[start - 1] == 'L'
                        || trimmed_chars[start - 1] == 'f'
                        || trimmed_chars[start - 1] == 'd')
                {
                    start -= 1;
                }
                // Only a standalone numeric literal (1., 1.0, 1L., ...) is a
                // receiver; "Base64." ends with a digit but is an identifier.
                let standalone = start == 0
                    || !(trimmed_chars[start - 1].is_alphanumeric()
                        || trimmed_chars[start - 1] == '_'
                        || trimmed_chars[start - 1] == '$');
                if standalone {
                    let literal: String = trimmed_chars[start..dot].iter().collect();
                    return Some(self.literal_receiver(&literal, &call));
                }
            }
        }
        // Identifier receivers.
        let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
        let mut start = dot;
        while start > 0 && is_ident(trimmed_chars[start - 1]) {
            start -= 1;
        }
        let receiver: String = trimmed_chars[start..dot].iter().collect();
        if !receiver.is_empty() {
            match receiver.as_str() {
                "this" => {
                    return Some(match current_class {
                        Some(class) => self.members_of_type(class, &call, false, false),
                        None => Vec::new(),
                    });
                }
                "super" => {
                    return Some(match current_class.and_then(|c| self.superclass_of(c)) {
                        Some(class) => self.members_of_type(class, &call, false, true),
                        None => Vec::new(),
                    });
                }
                "true" | "false" => {
                    return Some(self.literal_receiver("true", &call));
                }
                "null" => return Some(Vec::new()),
                _ => {}
            }
            if let Some(ty) = self.variable_type(&receiver) {
                return Some(self.members_of_type_id(ty, &call, false));
            }
            if let Some(class) = self.class_symbol_named(&receiver) {
                return Some(self.members_of_type(class, &call, true, false));
            }
        }
        // Multi-segment package receivers: "java.util." (chain) and "java.util"
        // typed as a prefix (receiver "java").
        let mut chain_start = dot;
        while chain_start > 0 {
            let candidate = trimmed_chars[chain_start - 1];
            if candidate.is_alphanumeric() || candidate == '_' || candidate == '$' || candidate == '.'
            {
                chain_start -= 1;
            } else {
                break;
            }
        }
        let chain: String = trimmed_chars[chain_start..dot].iter().collect();
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
        // Fully qualified class receiver: "java.util.Base64.get".
        if let Some(items) = self.qualified_class_members(&chain, &call) {
            return Some(items);
        }
        // AST fallback for new/call/chain/parens receivers.
        return Some(self.ast_receiver(text, position, &call));
    }

    pub(super) fn literal_receiver(
        &mut self,
        literal: &str,
        call: &CallContext,
    ) -> Vec<javac_analysis_api::CompletionItem> {
        let class_name = if literal.starts_with('"') {
            "String"
        } else if literal.starts_with('\'') {
            "Character"
        } else if literal == "true" || literal == "false" {
            "Boolean"
        } else if literal.ends_with('L') {
            "Long"
        } else if literal.ends_with('f') {
            "Float"
        } else if literal.ends_with('d') || literal.contains('.') {
            "Double"
        } else {
            "Integer"
        };
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