use std::path::{Path, PathBuf};

use super::super::super::canonical;
use crate::workspace::index::{IndexSymbolKind, ProjectIndex};

use super::super::super::positions::prefix_at;
use super::super::super::type_names::type_name;
use super::super::super::FileAnalysis;
use super::super::map_kind;
use super::common::{completion_item, index_kind_name, is_anonymous_class, is_internal_package, CallContext, KEYWORDS};
use super::imports::ImportContext;
use super::{context, insert, rank, search, token_stream};

impl FileAnalysis {
    /// Completion request entry point.
    pub(crate) fn completion_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
        index: Option<&ProjectIndex>,
    ) -> Vec<javac_analysis_api::CompletionItem> {
        let text = self
            .texts
            .get(&PathBuf::from(canonical(file)))
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        // Strings and comments are not code: do not offer code completions.
        if super::super::super::positions::in_literal_or_comment(
            &text,
            super::super::super::positions::line_offset(
                &text,
                position.line as usize + 1,
                position.column as usize + 1,
            ),
        ) {
            return Vec::new();
        }
        let prefix = prefix_at(
            &text,
            position.line as usize + 1,
            position.column as usize + 1,
        );
        let cursor_byte = super::super::super::positions::line_offset(
            &text,
            position.line as usize + 1,
            position.column as usize + 1,
        );
        let cursor_utf16 = text[..cursor_byte].encode_utf16().count();
        let tokens = token_stream::tokenize(&self.compiler, &text);
        let anchors = self.insertion_anchors(&tokens);
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
        let prefix_start_utf16 = text[..before].encode_utf16().count();
        let is_receiver =
            token_stream::last_before(&tokens, prefix_start_utf16)
                == Some(crate::parser::tokens::TokenKind::DOT);
        let scan_byte = if is_receiver {
            let cursor_utf16 = text[..before].encode_utf16().count();
            token_stream::receiver_start_utf16(&tokens, cursor_utf16)
                .and_then(|start| super::super::super::positions::utf16_to_byte(&text, start))
                .unwrap_or(cursor_byte)
        } else {
            cursor_byte.saturating_sub(prefix.len())
        };
        let scan_utf16 = text[..scan_byte].encode_utf16().count();
        let insertion = context::classify(
            &tokens,
            scan_utf16,
            &anchors,
            &self.import_ranges_impl(file),
            &self.annotation_ranges_impl(file),
            &self.context_type_ranges_impl(file),
        );
        let import_syntax = self.import_syntax_impl(file);
        let import_context = ImportContext::from_syntax(
            &text,
            &import_syntax,
            self.traced_class_names(),
            insertion != context::InsertionContext::ImportPackage,
        );
        // Import declaration completion: package/type segments before the
        // cursor, decoded from the token stream. The AST import ranges decide
        // that this is an import line; no line text is parsed.
        if insertion == context::InsertionContext::ImportPackage
            && let Some((mut parts, static_import)) =
                token_stream::import_qualifier(&tokens, cursor_utf16)
        {
            if !prefix.is_empty() && parts.last().is_some_and(|part| part == &prefix) {
                parts.pop();
            }
            let base = parts.join(".");
            let mut seen = std::collections::HashSet::new();
            let mut items = Vec::new();
            if static_import {
                let class = self.compiler.syms.get_all_classes().into_iter().find(|symbol| {
                    self.compiler.ctx.sym(*symbol).flat_name().to_string() == base
                });
                if let Some(class) = class {
                    let call = CallContext {
                        current_class: self.file_class_symbol(file),
                        current_package: String::new(),
                        insertion,
                    };
                    for mut item in self.members_of_type(class, &call, true, false) {
                        if search::match_label(&prefix, &item.label).is_none() {
                            continue;
                        }
                        item.detail = Some(base.clone());
                        items.push(item);
                    }
                } else {
                    for symbol in self.compiler.syms.get_all_classes() {
                        let flat = self.compiler.ctx.sym(symbol).flat_name().to_string();
                        let rest = if base.is_empty() {
                            Some(flat.as_str())
                        } else {
                            flat.strip_prefix(&format!("{base}."))
                        };
                        let Some(rest) = rest else { continue };
                        let segment = rest.split('.').next().unwrap_or(rest);
                        if segment.is_empty() || !seen.insert(segment.to_string()) {
                            continue;
                        }
                        if search::match_label(&prefix, segment).is_some() {
                            items.push(completion_item(
                                segment.to_string(),
                                javac_analysis_api::SymbolKind::Class,
                                Some(flat.clone()),
                                None,
                            ));
                        }
                    }
                }
            } else {
                for symbol in self.compiler.syms.get_all_classes() {
                    let flat = self.compiler.ctx.sym(symbol).flat_name().to_string();
                    let rest = if base.is_empty() {
                        Some(flat.as_str())
                    } else {
                        flat.strip_prefix(&format!("{base}."))
                    };
                    let Some(rest) = rest else { continue };
                    let segment = rest.split('.').next().unwrap_or(rest);
                    if segment.is_empty() {
                        continue;
                    }
                    let package_entry = rest.contains('.');
                    let key = format!("{base}.{segment}");
                    if !seen.insert(key) {
                        continue;
                    }
                    if search::match_label(&prefix, segment).is_some() {
                        items.push(completion_item(
                            segment.to_string(),
                            javac_analysis_api::SymbolKind::Class,
                            Some(if package_entry {
                                format!("{base}.{segment}")
                            } else {
                                flat.clone()
                            }),
                            None,
                        ));
                    }
                }
            }
            items = rank::rank_items(items, &prefix, rank::RankContext::Import);
            items.truncate(100);
            self.apply_prefix_replace(&mut items, prefix_utf16);
            return items;
        }
        if let Some(mut items) = self.receiver_completion(file, &text, position, insertion, &tokens) {
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
            let prefix_start = text[..cursor_byte - prefix.len()]
                .encode_utf16()
                .count();
            let new_context = token_stream::after_new(&tokens, prefix_start);
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
                let expected_type =
                    self.new_expected_type(&text, cursor_byte, prefix.len());
                        let array_expected = expected_type
                    .as_ref()
                    .filter(|expected| expected.array)
                    .map(|expected| expected.array_labels.clone())
                    .unwrap_or_default();
                let expand_constructors = expected_type.as_ref().is_none_or(|expected| {
                    !expected.array
                }) && self.completion_show_constructors;
                let mut items = self.new_completion(
                    &prefix,
                    &call,
                    &import_context,
                    expand_constructors,
                    array_expected.is_empty(),
                );
                if !items.is_empty() {
                    items = rank::rank_items(items, &prefix, rank::RankContext::New);
                    items.truncate(100);
                }
                for array in array_expected
                    .iter()
                    .filter(|array| search::match_label(&prefix, array).is_some())
                {
                    let mut item = completion_item(
                        array.clone(),
                        javac_analysis_api::SymbolKind::Class,
                        None,
                        None,
                    );
                    item.insert_text = Some(array.clone());
                    items.insert(0, item);
                }
                for primitive in [
                    "byte[]", "short[]", "char[]", "int[]", "long[]", "float[]",
                    "double[]", "boolean[]",
                ] {
                    if array_expected.iter().any(|existing| existing == primitive)
                        || search::match_label(&prefix, primitive).is_none()
                    {
                        continue;
                    }
                    let mut item = completion_item(
                        primitive.to_string(),
                        javac_analysis_api::SymbolKind::Class,
                        None,
                        None,
                    );
                    item.insert_text = Some(primitive.to_string());
                    items.insert(0, item);
                }
                if let Some(expected) = expected_type.as_ref().filter(|expected| {
                    expected.class && search::match_label(&prefix, &expected.label).is_some()
                }) {
                    let mut item = completion_item(
                        expected.label.clone(),
                        javac_analysis_api::SymbolKind::Class,
                        None,
                        None,
                    );
                    item.insert_text = Some(expected.label.clone());
                    items.insert(0, item);
                }
                if !items.is_empty() {
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
            item.signature = self.structured_signature(symbol);
            item.detail = self.method_return_type(symbol);
            item.documentation =
                self.member_documentation(&file_class_flat, &label, Some(argument_count));
            items.push(item);
        }
        let indexed: Vec<(String, String, IndexSymbolKind, bool)> = index
            .filter(|index| index.is_ready())
            .map(|index| {
                index
                    .symbols
                    .iter()
                    .filter(|record| record.kind.is_type())
                    .map(|record| {
                        (
                            record.flat_name.clone(),
                            crate::workspace::index::model::simple_name(&record.flat_name),
                            record.kind,
                            (record.modifiers & crate::code::flags::PUBLIC as u32) != 0,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        for (key, name, kind, is_public) in indexed {
            let mut item = completion_item(
                name,
                map_kind(index_kind_name(kind)),
                Some(key.clone()),
                None,
            );
            item.documentation = self.class_documentation(&key);
            if kind.is_type()
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
            // Declaration positions are Java char (UTF-16) offsets.
            let byte = crate::analysis::positions::utf16_to_byte_snapped(&text, pos as usize);
            match text.get(byte..) {
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
            for import in &import_syntax.static_imports {
                let path = import.type_path.clone();
                let simple = path.rsplit('.').next().unwrap_or(&path);
                let Some(class) = self.class_by_simple_name(simple) else {
                    continue;
                };
                let members = self.members_of_type(class, &call, true, false);
                if import.wildcard {
                    for mut item in members {
                        item.detail = Some(path.clone());
                        items.push(item);
                    }
                } else if let Some(member) = &import.member {
                    for item in members {
                        if item.label == *member {
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

}