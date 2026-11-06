//! Completion: spec-driven receiver resolution and member collection.

use std::path::{Path, PathBuf};

use super::super::positions::{char_index_for_utf16, prefix_at};
use super::super::type_names::type_name;
use super::super::{canonical, AnalysisSession};
use super::map_kind;

use crate::analysis::jdk_docs::JdkDocCache;

use crate::code::type_::TypeKindData;

mod context;
mod imports;
mod insert;
mod lexer;
mod rank;
mod search;

use context::InsertionContext;
use imports::ImportContext;

impl AnalysisSession {
    /// Completion request entry point.
    pub(crate) fn completion_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Vec<javac_analysis_api::CompletionItem> {
        let text = self
            .texts
            .get(&PathBuf::from(canonical(file)))
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let prefix = prefix_at(
            &text,
            position.line as usize + 1,
            position.column as usize + 1,
        );
        let cursor_byte = super::super::positions::line_offset(
            &text,
            position.line as usize + 1,
            position.column as usize + 1,
        );
        let cursor_utf16 = text[..cursor_byte].encode_utf16().count();
        let anchors = self.insertion_anchors(&text);
        // Classify at the start of the prefix: the last token before the word
        // being completed, not the partially typed word itself. For receiver
        // completion (a.xxx) skip the whole receiver expression so the context
        // is the position before "a", not the dot.
        let prefix_utf16 = prefix.encode_utf16().count() as u32;
        let mut before = cursor_byte.saturating_sub(prefix.len());
        while before > 0 {
            let Some(previous) = text[..before].chars().next_back() else {
                break;
            };
            if previous.is_whitespace() {
                before -= previous.len_utf8();
            } else {
                break;
            }
        }
        let is_receiver = before > 0 && text[..before].ends_with('.');
        let scan_byte = if is_receiver {
            expression_start_byte(&text, before)
        } else {
            cursor_byte.saturating_sub(prefix.len())
        };
        let scan_utf16 = text[..scan_byte].encode_utf16().count();
        let insertion = context::classify(&text, scan_utf16, &anchors);
        let import_context = ImportContext::new(
            &text,
            self.traced_class_names(),
            insertion != context::InsertionContext::ImportPackage,
        );
        // Receiver context: always return its result, including the empty one.
        if let Some(mut items) = self.receiver_completion(file, &text, position, insertion) {
            let filter_prefix = if self.receiver_prefix_consumed {
                ""
            } else {
                prefix.as_str()
            };
            items = rank::rank_items(items, filter_prefix, rank::RankContext::Receiver);
            items.dedup_by(|a, b| {
                a.label == b.label && a.kind == b.kind && a.label_detail == b.label_detail
            });
            items.truncate(100);
            let replace_len = if self.receiver_prefix_consumed {
                0
            } else {
                prefix_utf16
            };
            self.apply_prefix_replace(&mut items, replace_len);
            self.apply_sort_text(&mut items);
            return items;
        }
        // New-expression: constructor completion ("new Zip" -> ZipFile(...)).
        {
            let before_prefix = &text[..cursor_byte - prefix.len()];
            let trimmed_before = before_prefix.trim_end();
            let new_context = trimmed_before
                .strip_suffix("new")
                .map(|head| {
                    head.chars()
                        .last()
                        .map(|c| {
                            !(c.is_alphanumeric() || c == '_' || c == '$' || c == '.')
                        })
                        .unwrap_or(true)
                })
                .unwrap_or(false);
            if new_context && !prefix.is_empty() {
                let current_class = self.file_class_symbol(file);
                let current_package = current_class
                    .map(|class| self.package_flat(class))
                    .unwrap_or_default();
                let call = CallContext {
                    current_class,
                    current_package,
                    insertion,
                };
                let mut items = self.new_completion(&prefix, &call, &import_context);
                if !items.is_empty() {
                    items = rank::rank_items(items, &prefix, rank::RankContext::New);
                    items.truncate(100);
                    self.apply_prefix_replace(&mut items, prefix_utf16);
                    self.apply_sort_text(&mut items);
                    return items;
                }
            }
        }
        let mut items: Vec<javac_analysis_api::CompletionItem> = Vec::new();
        for symbol in self.symbols() {
            if symbol.kind == "method" {
                // Methods are emitted from the AST below so their return type
                // and parameter count are available for the insert text.
                continue;
            }
            let mut item = completion_item(
                symbol.name,
                map_kind(symbol.kind),
                Some(symbol.flat_name),
                Some(symbol.detail),
            );
            item.documentation = symbol.doc;
            items.push(item);
        }
        let target = canonical(file);
        let file_class_flat = self
            .file_class_symbol(file)
            .map(|class| self.compiler.ctx.sym(class).flat_name().to_string())
            .unwrap_or_default();
        let file_methods: Vec<(String, crate::ids::SymbolId)> = self
            .compiler
            .ctx
            .trees
            .iter()
            .filter_map(|tree| {
                let crate::tree::jc_tree::TreeKind::MethodDecl {
                    name,
                    sym: Some(sym),
                    ..
                } = &tree.kind
                else {
                    return None;
                };
                // The tree arena accumulates every analyzed file, so only keep
                // methods declared in the file being completed.
                let owner = self.compiler.ctx.sym(*sym).owner;
                let matches = owner
                    .map(|owner| match &self.compiler.ctx.sym(owner).kind_data {
                        crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
                            .as_ref()
                            .map(|source| {
                                canonical(Path::new(&source.get_name().to_string_lossy()))
                                    == target
                            })
                            .unwrap_or(false),
                        _ => false,
                    })
                    .unwrap_or(false);
                if matches {
                    Some((name.to_java_string().to_string_lossy(), *sym))
                } else {
                    None
                }
            })
            .collect();
        for (label, symbol) in file_methods {
            if label.starts_with('<') || search::match_label(&prefix, &label).is_none() {
                continue;
            }
            let (is_void, argument_count) = self.method_shape(symbol);
            let plan = insert::method_plan(&label, is_void, argument_count, insertion);
            let mut item = completion_item(
                label.clone(),
                javac_analysis_api::SymbolKind::Method,
                None,
                None,
            );
            item.insert_text = Some(plan.text);
            item.insert_text_format = plan.format;
            item.label_detail = self.method_signature(symbol);
            item.detail = self.method_return_type(symbol);
            item.documentation =
                self.member_documentation(&file_class_flat, &label, Some(argument_count));
            items.push(item);
        }
        self.ensure_index();
        let indexed: Vec<(String, &'static str, bool)> = self
            .index
            .iter()
            .map(|(key, entry)| (key.clone(), entry.kind, entry.is_public))
            .collect();
        for (key, kind, is_public) in indexed {
            if kind != "class" && kind != "nested" {
                continue;
            }
            let Some(name) = key.rsplit('.').next() else { continue };
            let mut item = completion_item(
                name.to_string(),
                map_kind(kind),
                Some(key.clone()),
                None,
            );
            item.documentation = self.class_documentation(&key);
            if kind == "class"
                && let Some(edit) = import_context.edit_for(&key, is_public)
            {
                item.additional_text_edits.push(edit);
            }
            items.push(item);
        }
        for keyword in KEYWORDS {
            if keyword.starts_with(&prefix) {
                items.push(completion_item(
                    keyword.to_string(),
                    javac_analysis_api::SymbolKind::Keyword,
                    Some("keyword".to_string()),
                    None,
                ));
            }
        }
        // Local variables and parameters of this file.
        let in_file = |pos: i32| -> bool {
            if pos < 0 {
                return false;
            }
            let pos = pos as usize;
            match text.get(pos..) {
                Some(rest) => rest.starts_with(&prefix) || prefix.is_empty(),
                None => false,
            }
        };
        for tree in self.compiler.ctx.trees.iter() {
            if let crate::tree::jc_tree::TreeKind::VariableDecl {
                name,
                sym: _,
                decl_pos,
                vartype,
                ..
            } = &tree.kind
                && in_file(*decl_pos)
            {
                let label = name.to_java_string().to_string_lossy();
                if search::match_label(&prefix, &label).is_none() {
                    continue;
                }
                let ty = vartype
                    .and_then(|v| self.compiler.ctx.tree(v).ty)
                    .map(|ty| type_name(&self.compiler.ctx, ty))
                    .unwrap_or_else(|| "?".to_string());
                items.push(completion_item(
                    label,
                    javac_analysis_api::SymbolKind::LocalVariable,
                    Some(ty),
                    None,
                ));
            }
        }
        // Static imports: unqualified members.
        {
            let call = CallContext {
                current_class: self.file_class_symbol(file),
                current_package: String::new(),
                insertion,
            };
            let static_paths: Vec<String> = text
                .lines()
                .filter_map(|line| {
                    let trimmed = line.trim();
                    let rest = trimmed.strip_prefix("import static ")?;
                    Some(rest.split(';').next().unwrap_or(rest).trim().to_string())
                })
                .collect();
            for path in static_paths {
                let type_path = path.strip_suffix(".*").unwrap_or_else(|| {
                    path.rsplit_once('.').map(|(ty, _)| ty).unwrap_or(&path)
                });
                let simple = type_path.rsplit('.').next().unwrap_or(type_path);
                let Some(class) = self.class_by_simple_name(simple) else {
                    continue;
                };
                let members = self.members_of_type(class, &call, true, false);
                if path.ends_with(".*") {
                    for mut item in members {
                        item.detail = Some(path.clone());
                        items.push(item);
                    }
                } else if let Some((_, member)) = path.rsplit_once('.') {
                    for item in members {
                        if item.label == member {
                            items.push(item);
                        }
                    }
                }
            }
        }
        if !prefix.is_empty() {
            let target = canonical(file);
            for sym in self.compiler.syms.get_all_classes() {
                let symbol = self.compiler.ctx.sym(sym);
                let flat = symbol.flat_name().to_string();
                // Anonymous classes (Outer$1) are compiler artifacts, not
                // addressable source entities.
                if is_anonymous_class(&flat) {
                    continue;
                }
                // Nested classes of other files are not shown as top-level
                // candidates (Outer$Inner); keep the current file's own ones.
                if flat.contains('$') {
                    let source_is_current = match &symbol.kind_data {
                        crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
                            .as_ref()
                            .map(|source| {
                                canonical(Path::new(&source.get_name().to_string_lossy())) == target
                            })
                            .unwrap_or(false),
                        _ => false,
                    };
                    if !source_is_current {
                        continue;
                    }
                }
                // Classes in JDK-internal packages are not part of the
                // supported API surface (IDEA hides them from completion).
                if is_internal_package(&flat) {
                    continue;
                }
                let label = symbol.name.to_java_string().to_string_lossy();
                if search::match_label(&prefix, &label).is_none() {
                    continue;
                }
                let is_public =
                    (self.compiler.ctx.sym(sym).flags_field
                        & crate::code::flags::PUBLIC as i64)
                        != 0;
                let mut item = completion_item(
                    label,
                    javac_analysis_api::SymbolKind::Class,
                    Some(flat.clone()),
                    None,
                );
                item.documentation = self.class_documentation(&flat);
                if let Some(edit) = import_context.edit_for(&flat, is_public) {
                    item.additional_text_edits.push(edit);
                }
                items.push(item);
            }
        }
        let rank_context = match insertion {
            context::InsertionContext::ClassBody => rank::RankContext::ClassBody,
            context::InsertionContext::ImportPackage => rank::RankContext::Import,
            _ => rank::RankContext::Statement,
        };
        items = rank::rank_items(items, &prefix, rank_context);
        items.dedup_by(|a, b| {
            a.label == b.label && a.kind == b.kind && a.label_detail == b.label_detail
        });
        items.truncate(100);
        self.apply_prefix_replace(&mut items, prefix_utf16);
        self.apply_sort_text(&mut items);
        items
    }

    /// Receiver completion. Returns None when this is not a receiver context,
    /// Some(items) otherwise (including the empty list for unresolved receivers).
    fn receiver_completion(
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

    fn literal_receiver(
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
    fn package_members(
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

    /// Constructor completion after "new": one item per public/protected
    /// constructor, labelled "Class(param types)" with the package as detail.
    fn new_completion(
        &mut self,
        prefix: &str,
        call: &CallContext,
        import_context: &ImportContext,
    ) -> Vec<javac_analysis_api::CompletionItem> {
        let packages = self
            .compiler
            .syms
            .all_visible_packages(&mut self.compiler.ctx);
        let mut items = Vec::new();
        for (package_name, package) in packages {
            let package_flat = package_name.to_java_string().to_string_lossy();
            // Package completion enters the classes of the package.
            let _ = self
                .compiler
                .ctx
                .complete_symbol(&self.compiler.syms, package);
            let Some(scope) = self.compiler.ctx.sym(package).members() else {
                continue;
            };
            let classes = self.compiler.ctx.scope_get_symbols(
                scope,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            );
            for class in classes {
                let (simple, is_class) = {
                    let symbol = self.compiler.ctx.sym(class);
                    (
                        symbol.name.to_java_string().to_string_lossy(),
                        matches!(
                            symbol.kind_data,
                            crate::code::symbol::SymbolKind::ClassSymbol { .. }
                        ),
                    )
                };
                if !is_class {
                    continue;
                }
                // Nested class files are also entered under their flat binary
                // name by the package scan; they are not "new X" candidates.
                // Reading them here would rewrite their owner before the outer
                // class is read (same poisoning as package completion).
                let flat = self.compiler.ctx.sym(class).flat_name().to_string();
                if flat.contains('$') {
                    continue;
                }
                // Every match quality expands constructors ("new Zip",
                // "new zie", "new Filein" -> FileInputStream(...)).
                if search::match_label(prefix, &simple).is_none() {
                    continue;
                }
                if is_internal_package(&format!("{package_flat}.{simple}")) {
                    continue;
                }
                // Reading the class file fills flags and the member scope.
                let _ = self
                    .compiler
                    .ctx
                    .complete_symbol(&self.compiler.syms, class);
                let is_visible_class = {
                    let symbol = self.compiler.ctx.sym(class);
                    self.is_visible(symbol, call)
                };
                if !is_visible_class {
                    continue;
                }
                let is_public_class = (self.compiler.ctx.sym(class).flags_field
                    & crate::code::flags::PUBLIC as i64)
                    != 0;
                let class_flat = format!("{package_flat}.{simple}");
                let Some(class_scope) = self.compiler.ctx.sym(class).members() else {
                    continue;
                };
                let members = self.compiler.ctx.scope_get_symbols(
                    class_scope,
                    crate::code::scope::LookupKind::NON_RECURSIVE,
                    &|_| true,
                );
                for member in members {
                    let (is_ctor, is_public) = {
                        let ctor = self.compiler.ctx.sym(member);
                        (
                            ctor.name.to_java_string().to_string_lossy() == "<init>"
                                && (ctor.flags_field
                                    & (crate::code::flags::GENERATEDCONSTR
                                        | crate::code::flags::SYNTHETIC as i64))
                                    == 0,
                            (ctor.flags_field
                                & (crate::code::flags::PUBLIC
                                    | crate::code::flags::PROTECTED) as i64)
                                != 0,
                        )
                    };
                    if !is_ctor || !is_public {
                        continue;
                    }
                    let signature = self.method_signature(member).unwrap_or_default();
                    let mut item = completion_item(
                        format!("{simple}{signature}"),
                        javac_analysis_api::SymbolKind::Class,
                        Some(package_flat.clone()),
                        None,
                    );
                    // Constructor arguments are placeholders for the editor, not
                    // parameter names: never insert p0/p1 into the source.
                    let snippet = insert::escape_snippet(&simple);
                    item.insert_text = Some(format!("{snippet}($0)"));
                    item.insert_text_format = javac_analysis_api::InsertTextFormat::Snippet;
                    item.documentation = self.class_documentation(&class_flat);
                    if let Some(edit) = import_context.edit_for(&class_flat, is_public_class) {
                        item.additional_text_edits.push(edit);
                    }
                    items.push(item);
                }
            }
        }
        items
    }

    /// Members of a fully qualified class ("java.util.Base64") - static only.
    fn qualified_class_members(
        &mut self,
        chain: &str,
        call: &CallContext,
    ) -> Option<Vec<javac_analysis_api::CompletionItem>> {
        let (package_name, class_name) = chain.rsplit_once('.')?;
        let package = self
            .compiler
            .syms
            .lookup_package_str(&mut self.compiler.ctx, package_name)?;
        let scope = self.compiler.ctx.sym(package).members()?;
        let members = self.compiler.ctx.scope_get_symbols(
            scope,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        let mut class = None;
        for member in members {
            let symbol = self.compiler.ctx.sym(member);
            if symbol.name.to_java_string().to_string_lossy() != class_name {
                continue;
            }
            if matches!(
                symbol.kind_data,
                crate::code::symbol::SymbolKind::ClassSymbol { .. }
            ) {
                class = Some(member);
                break;
            }
        }
        let class = class?;
        Some(self.members_of_type(class, call, true, false))
    }

    /// Type parameter list of a class declaration, e.g. "<E, F>" ("" when raw).
    fn class_type_parameters(&self, symbol: crate::ids::SymbolId) -> String {
        let ctx = &self.compiler.ctx;
        let Some(ty) = ctx.sym(symbol).ty else {
            return String::new();
        };
        let params = match &ctx.typ(ty).kind {
            // ClassReader stores the declaration's type parameters here
            // (read_signature_attr), not in allparams_field.
            TypeKindData::ClassType { typarams_field, .. } => {
                typarams_field.clone().unwrap_or_default()
            }
            _ => return String::new(),
        };
        if params.is_empty() {
            return String::new();
        }
        let names = params
            .iter()
            .filter_map(|param| {
                ctx.typ(param)
                    .tsym
                    .map(|tsym| ctx.sym(tsym).name.to_java_string().to_string_lossy())
            })
            .collect::<Vec<_>>();
        format!("<{}>", names.join(", "))
    }

    fn ast_receiver(
        &mut self,
        text: &str,
        position: javac_analysis_api::Position,
        call: &CallContext,
    ) -> Vec<javac_analysis_api::CompletionItem> {
        let target_line = position.line as usize + 1;
        let target_column = position.column as usize + 1;
        let cursor_offset =
            super::super::positions::line_offset(text, target_line, target_column);
        let mut best: Option<(i32, crate::ids::TypeId)> = None;
        for tree in self.compiler.ctx.trees.iter() {
            let crate::tree::jc_tree::TreeKind::FieldAccess { selected, .. } = &tree.kind else {
                continue;
            };
            if tree.pos < 0 || tree.pos as usize > cursor_offset {
                continue;
            }
            let Some(selected) = selected else { continue };
            let Some(ty) = self.compiler.ctx.tree(*selected).ty else { continue };
            if best.map(|(pos, _)| tree.pos > pos).unwrap_or(true) {
                best = Some((tree.pos, ty));
            }
        }
        match best {
            Some((_, ty)) => self.members_of_type_id(ty, call, false),
            None => Vec::new(),
        }
    }

    /// Members of a type id.
    fn members_of_type_id(
        &mut self,
        ty: crate::ids::TypeId,
        call: &CallContext,
        static_only: bool,
    ) -> Vec<javac_analysis_api::CompletionItem> {
        match &self.compiler.ctx.typ(ty).kind {
            TypeKindData::ClassType { .. } => match self.compiler.ctx.typ(ty).tsym {
                Some(class) => self.members_of_type(class, call, static_only, false),
                None => Vec::new(),
            },
            TypeKindData::ArrayType { elemtype, .. } => {
                let _ = elemtype;
                let mut items = self.object_members(call);
                items.push(completion_item(
                    "length".to_string(),
                    javac_analysis_api::SymbolKind::Field,
                    Some("int".to_string()),
                    None,
                ));
                items
            }
            TypeKindData::TypeVar { bound, .. } => match bound {
                Some(bound) => self.members_of_type_id(*bound, call, static_only),
                None => self.object_members(call),
            },
            _ => Vec::new(),
        }
    }

    /// Members of a class, inherited and visibility-filtered.
    fn members_of_type(
        &mut self,
        class: crate::ids::SymbolId,
        call: &CallContext,
        static_only: bool,
        direct_only: bool,
    ) -> Vec<javac_analysis_api::CompletionItem> {
        let mut items = Vec::new();
        let mut visited = Vec::new();
        self.collect_members(class, call, static_only, direct_only, false, &mut visited, &mut items);
        items
    }

    fn collect_members(
        &mut self,
        class: crate::ids::SymbolId,
        call: &CallContext,
        static_only: bool,
        direct_only: bool,
        from_interface: bool,
        visited: &mut Vec<crate::ids::SymbolId>,
        items: &mut Vec<javac_analysis_api::CompletionItem>,
    ) {
        if visited.contains(&class) {
            return;
        }
        visited.push(class);
        {
            let completer = self.compiler.ctx.sym(class).completer.clone();
            if let Some(completer) = completer {
                let syms = std::rc::Rc::clone(&self.compiler.syms);
                let mut completer = completer.borrow_mut();
                let _ = completer.complete(&mut self.compiler.ctx, &syms, class);
            }
        }
        let class_flat = self.compiler.ctx.sym(class).flat_name().to_string();
        if let Some(scope) = self.compiler.ctx.sym(class).members() {
            let members = self.compiler.ctx.scope_get_symbols(
                scope,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            );
            for member in members {
                let symbol = self.compiler.ctx.sym(member);
                if (symbol.flags_field & crate::code::flags::GENERATEDCONSTR) != 0 {
                    continue;
                }
                if (symbol.flags_field & crate::code::flags::SYNTHETIC as i64) != 0 {
                    continue;
                }
                let name = symbol.name.to_java_string().to_string_lossy();
                if name.starts_with('<') {
                    continue;
                }
                let mut is_static = symbol.flags_field & crate::code::flags::STATIC as i64 != 0;
                // A static nested type's STATIC bit may only appear in the
                // InnerClasses attribute; nested types without one carry an
                // enclosing type instead (IDEA counts those as static).
                if !is_static
                    && symbol.flat_name().to_string().contains('$')
                    && let Some(ty) = symbol.ty
                    && matches!(
                        self.compiler.ctx.typ(ty).kind,
                        TypeKindData::ClassType { .. }
                    )
                    && self.compiler.ctx.typ(ty).get_enclosing_type().is_none()
                {
                    is_static = true;
                }
                if static_only && !is_static {
                    continue;
                }
                // Interface static methods are not inherited (JLS 8.4.8):
                // ArrayList. must not offer List.of.
                if from_interface && is_static {
                    continue;
                }
                if !self.is_visible(symbol, call) {
                    continue;
                }
                let kind = match symbol.kind_data {
                    crate::code::symbol::SymbolKind::MethodSymbol { .. } => "method",
                    crate::code::symbol::SymbolKind::VarSymbol { .. } => "field",
                    // IDEA's reference completion offers nested classes after
                    // a qualifier (System.lo offers Logger/LoggerFinder).
                    crate::code::symbol::SymbolKind::ClassSymbol { .. } => "nested",
                    _ => "member",
                };
                let mut item = completion_item(
                    name.clone(),
                    map_kind(kind),
                    Some(class_flat.clone()),
                    None,
                );
                if kind == "method" {
                    let (is_void, argument_count) = self.method_shape(member);
                    let plan = insert::method_plan(&name, is_void, argument_count, call.insertion);
                    item.insert_text = Some(plan.text);
                    item.insert_text_format = plan.format;
                    item.label_detail = self.method_signature(member);
                    item.detail = self.method_return_type(member);
                    item.documentation =
                        self.member_documentation(&class_flat, &name, Some(argument_count));
                } else if kind == "field" {
                    // Fields show their declared type (PrintStream), not the
                    // declaring class.
                    if let Some(ty) = self.compiler.ctx.sym(member).ty {
                        item.detail = Some(type_name(&self.compiler.ctx, ty));
                    }
                    item.documentation = self.member_documentation(&class_flat, &name, None);
                } else {
                    // Nested types carry their own type comment.
                    item.documentation = self.member_documentation(&class_flat, &name, None);
                }
                items.push(item);
            }
        }
        if direct_only {
            return;
        }
        if let Some(ty) = self.compiler.ctx.sym(class).ty {
            let (supertype, interfaces) = match &self.compiler.ctx.typ(ty).kind {
                TypeKindData::ClassType {
                    supertype_field,
                    interfaces_field,
                    ..
                }
                | TypeKindData::ErasedClassType {
                    supertype_field,
                    interfaces_field,
                    ..
                } => (*supertype_field, interfaces_field.clone()),
                _ => (None, None),
            };
            if let Some(super_ty) = supertype {
                if let Some(super_class) = self.compiler.ctx.typ(super_ty).tsym {
                    self.collect_members(super_class, call, static_only, false, false, visited, items);
                }
            }
            if let Some(interfaces) = interfaces {
                for interface in interfaces.iter() {
                    if let Some(interface_class) = self.compiler.ctx.typ(interface).tsym {
                        self.collect_members(
                            interface_class,
                            call,
                            static_only,
                            false,
                            true,
                            visited,
                            items,
                        );
                    }
                }
            }
        }
    }

    fn object_members(&mut self, call: &CallContext) -> Vec<javac_analysis_api::CompletionItem> {
        match self.class_by_simple_name("Object") {
            Some(class) => self.members_of_type(class, call, false, false),
            None => Vec::new(),
        }
    }

    fn is_visible(&self, symbol: &crate::code::symbol::Symbol, call: &CallContext) -> bool {
        let flags = symbol.flags_field;
        if flags & crate::code::flags::PRIVATE as i64 != 0 {
            return symbol.owner.map(|owner| Some(owner) == call.current_class).unwrap_or(false);
        }
        let same_package = symbol
            .owner
            .map(|owner| {
                let owner_symbol = self.compiler.ctx.sym(owner);
                let package = match &owner_symbol.kind_data {
                    crate::code::symbol::SymbolKind::PackageSymbol { .. } => owner_symbol
                        .get_qualified_name()
                        .to_java_string()
                        .to_string_lossy(),
                    _ => self.package_flat(owner),
                };
                package == call.current_package
            })
            .unwrap_or(false);
        if flags & crate::code::flags::PROTECTED as i64 != 0 {
            return same_package;
        }
        if flags & crate::code::flags::PUBLIC as i64 != 0 {
            return true;
        }
        same_package
    }

    fn superclass_of(&mut self, class: crate::ids::SymbolId) -> Option<crate::ids::SymbolId> {
        let ty = self.compiler.ctx.sym(class).ty?;
        let super_ty = match &self.compiler.ctx.typ(ty).kind {
            TypeKindData::ClassType { supertype_field, .. }
            | TypeKindData::ErasedClassType { supertype_field, .. } => *supertype_field,
            _ => None,
        }?;
        self.compiler.ctx.typ(super_ty).tsym
    }

    fn package_flat(&self, class: crate::ids::SymbolId) -> String {
        crate::code::symbol::symbol_packge(&self.compiler.ctx, class)
            .map(|packge| self.compiler.ctx.sym(packge).flat_name().to_string())
            .unwrap_or_default()
    }

    fn variable_type(&mut self, name: &str) -> Option<crate::ids::TypeId> {
        for tree in self.compiler.ctx.trees.iter() {
            if let crate::tree::jc_tree::TreeKind::VariableDecl {
                name: decl_name,
                sym: Some(sym),
                vartype,
                ..
            } = &tree.kind
                && decl_name.to_java_string().to_string_lossy() == name
            {
                // var declarations carry the inferred type on the type tree.
                if let Some(vartype) = vartype
                    && let Some(ty) = self.compiler.ctx.tree(*vartype).ty
                {
                    return Some(ty);
                }
                return self.compiler.ctx.sym(*sym).ty;
            }
        }
        None
    }

    fn class_by_simple_name(&mut self, name: &str) -> Option<crate::ids::SymbolId> {
        for sym in self.compiler.syms.get_all_classes() {
            let simple = self
                .compiler
                .ctx
                .sym(sym)
                .name
                .to_java_string()
                .to_string_lossy();
            if simple == name {
                return Some(sym);
            }
        }
        None
    }

    fn file_class_symbol(&mut self, file: &Path) -> Option<crate::ids::SymbolId> {
        let target = canonical(file);
        let mut last = None;
        for tree in self.compiler.ctx.trees.iter() {
            if let crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. } = &tree.kind {
                let matches = match &self.compiler.ctx.sym(*sym).kind_data {
                    crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
                        .as_ref()
                        .map(|source| canonical(Path::new(&source.get_name().to_string_lossy())) == target)
                        .unwrap_or(false),
                    _ => false,
                };
                if matches {
                    last = Some(*sym);
                }
            }
        }
        last
    }

    /// Javadoc of a class: project source first, then the JDK sources.
    fn class_documentation(&mut self, flat: &str) -> Option<String> {
        if let Some(doc) = self.traced_class_doc(flat) {
            return Some(doc);
        }
        self.jdk_docs
            .get_or_insert_with(JdkDocCache::new)
            .class_doc(flat)
    }

    /// Javadoc of a member: project source first, then the JDK sources.
    fn member_documentation(
        &mut self,
        class_flat: &str,
        name: &str,
        arity: Option<usize>,
    ) -> Option<String> {
        if let Some(doc) = self.traced_member_doc(class_flat, name) {
            return Some(doc);
        }
        self.jdk_docs
            .get_or_insert_with(JdkDocCache::new)
            .member_doc(class_flat, name, arity)
    }

    fn method_shape(&mut self, symbol: crate::ids::SymbolId) -> (bool, usize) {
        // The parameter list comes from the symbol itself: JDK methods loaded
        // from class files carry params even when the method type is not
        // attributed yet.
        let argument_count = match &self.compiler.ctx.sym(symbol).kind_data {
            crate::code::symbol::SymbolKind::MethodSymbol { params, .. } => params.size() as usize,
            _ => 0,
        };
        let is_void = self
            .compiler
            .ctx
            .sym(symbol)
            .ty
            .and_then(|ty| match &self.compiler.ctx.typ(ty).kind {
                TypeKindData::MethodType { restype, .. }
                | TypeKindData::PartiallyInferredMethodType { restype, .. } => *restype,
                _ => None,
            })
            .map(|restype| {
                self.compiler.ctx.typ(restype).tag == crate::code::type_tag::TypeTag::VOID
            })
            .unwrap_or(false);
        (is_void, argument_count)
    }

    fn insertion_anchors(&mut self, text: &str) -> context::Anchors {
        let scan = lexer::scan(text, usize::MAX);
        let mut anchors = context::Anchors::default();
        for tree in self.compiler.ctx.trees.iter() {
            if tree.pos < 0 {
                continue;
            }
            let pos = tree.pos as usize;
            match &tree.kind {
                crate::tree::jc_tree::TreeKind::Block { .. } => anchors.block_starts.push(pos),
                crate::tree::jc_tree::TreeKind::Switch { .. } => {
                    if let Some(brace) = scan.braces.iter().find(|brace| {
                        brace.kind == lexer::BraceKind::Brace && brace.offset >= pos
                    }) {
                        anchors.switch_starts.push(brace.offset);
                    }
                }
                crate::tree::jc_tree::TreeKind::NewArray { .. } => {
                    if let Some(brace) = scan.braces.iter().find(|brace| {
                        brace.kind == lexer::BraceKind::Brace && brace.offset >= pos
                    }) {
                        anchors.array_starts.push(brace.offset);
                    }
                }
                _ => {}
            }
        }
        anchors
    }

    /// Return type of a method (shown in the detail column).
    fn method_return_type(&mut self, symbol: crate::ids::SymbolId) -> Option<String> {
        let ty = self.compiler.ctx.sym(symbol).ty?;
        let restype = match &self.compiler.ctx.typ(ty).kind {
            TypeKindData::MethodType { restype, .. }
            | TypeKindData::PartiallyInferredMethodType { restype, .. } => *restype,
            _ => None,
        }?;
        Some(type_name(&self.compiler.ctx, restype))
    }

    /// Human-readable parameter list for a method, e.g. "(int, String)".
    fn method_signature(&mut self, symbol: crate::ids::SymbolId) -> Option<String> {
        let params = match &self.compiler.ctx.sym(symbol).kind_data {
            crate::code::symbol::SymbolKind::MethodSymbol { params, .. } => params.clone(),
            _ => return None,
        };
        let varargs = (self.compiler.ctx.sym(symbol).flags_field
            & crate::code::flags::VARARGS)
            != 0;
        let last = params.iter().count().saturating_sub(1);
        let mut parts = Vec::new();
        for (index, param) in params.iter().enumerate() {
            let param = self.compiler.ctx.sym(param);
            let mut ty = param
                .ty
                .map(|ty| type_name(&self.compiler.ctx, ty))
                .unwrap_or_else(|| "?".to_string());
            if varargs && index == last {
                if let Some(stripped) = ty.strip_suffix("[]") {
                    ty = format!("{stripped}...");
                }
            }
            let name = param.name.to_java_string().to_string_lossy();
            // Synthetic names (arg0, arg1, ...) come from class files without
            // parameter metadata (native/abstract methods have no Code and thus
            // no LocalVariableTable); use positional p0, p1 instead.
            let synthetic = name
                .strip_prefix("arg")
                .map(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
                .unwrap_or(false);
            let name = if synthetic || name.is_empty() {
                format!("p{index}")
            } else {
                name.to_string()
            };
            parts.push(format!("{ty} {name}"));
        }
        Some(format!("({})", parts.join(", ")))
    }

    fn apply_prefix_replace(
        &self,
        items: &mut [javac_analysis_api::CompletionItem],
        prefix_len: u32,
    ) {
        if prefix_len == 0 {
            return;
        }
        for item in items {
            if item.insert_text.is_some() {
                item.replace_prefix_len = Some(prefix_len);
            }
        }
    }

    /// LSP clients re-sort by sortText or label; emit the server order so the
    /// sort_items result survives the client.
    fn apply_sort_text(&self, items: &mut [javac_analysis_api::CompletionItem]) {
        for (index, item) in items.iter_mut().enumerate() {
            item.sort_text = Some(format!("{index:04}"));
        }
    }
}
/// Compilation-point context for visibility decisions.
struct CallContext {
    current_class: Option<crate::ids::SymbolId>,
    current_package: String,
    insertion: InsertionContext,
}

const KEYWORDS: &[&str] = &[
    "public", "private", "protected", "static", "final", "abstract", "native",
    "synchronized", "transient", "volatile", "strictfp", "default", "class",
    "interface", "enum", "record", "extends", "implements", "import", "package",
    "void", "int", "long", "short", "byte", "char", "boolean", "float", "double",
    "if", "else", "for", "while", "do", "switch", "case", "break", "continue",
    "return", "new", "try", "catch", "finally", "throw", "throws", "this", "super",
    "null", "true", "false", "instanceof", "var", "yield", "assert",
];
/// Build a completion item with the minimal fields set.
fn completion_item(
    label: String,
    kind: javac_analysis_api::SymbolKind,
    detail: Option<String>,
    label_detail: Option<String>,
) -> javac_analysis_api::CompletionItem {
    javac_analysis_api::CompletionItem {
        label,
        kind,
        detail,
        documentation: None,
        label_detail,
        sort_text: None,
        insert_text: None,
        insert_text_format: javac_analysis_api::InsertTextFormat::PlainText,
        replace_prefix_len: None,
        filter_text: None,
        additional_text_edits: Vec::new(),
    }
}

/// Byte offset where the receiver expression starts (before the dot).
fn expression_start_byte(text: &str, before: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = before;
    let mut depth = 0i32;
    while index > 0 {
        let byte = bytes[index - 1];
        match byte {
            b')' | b']' => {
                depth += 1;
                index -= 1;
            }
            b'(' | b'[' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                index -= 1;
            }
            b'.' => {
                index -= 1;
            }
            b' ' | b'\t' => {
                if depth == 0 {
                    break;
                }
                index -= 1;
            }
            byte if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' => {
                index -= 1;
            }
            _ => break,
        }
    }
    index
}

/// Anonymous classes (Outer$1) are compiler artifacts, not source entities.
fn is_anonymous_class(flat: &str) -> bool {
    let simple = flat.rsplit(['.', '$']).next().unwrap_or(flat);
    !simple.is_empty() && simple.chars().all(|c| c.is_ascii_digit())
}

/// JDK-internal packages are hidden from completion (IDEA hides them too).
fn is_internal_package(flat: &str) -> bool {
    let package = flat.rsplit_once('.').map(|(package, _)| package).unwrap_or(flat);
    package.starts_with("sun.")
        || package.starts_with("com.sun.")
        || package.contains(".internal")
        || package.starts_with("jdk.internal.")
        || package.starts_with("jdk.nio.zipfs")
}
