use std::collections::{BTreeMap, HashSet};

use super::super::super::FileAnalysis;
use super::imports::ImportContext;
use super::common::{completion_item, is_internal_package, CallContext};
use super::{insert, search};
use crate::code::type_::TypeKindData;

impl FileAnalysis {
    /// Constructor completion after "new": one item per public/protected
    /// constructor, labelled "Class(param types)" with the package as detail.
    pub(super) fn new_completion(
        &mut self,
        prefix: &str,
        call: &CallContext,
        import_context: &ImportContext,
        expand_constructors: bool,
        insert_parentheses: bool,
    ) -> Vec<javac_analysis_api::CompletionItem> {
        // IDEA searches classes through the short-name index, not by scanning
        // every package. The catalog is built once per analysis round and then
        // reused by every completion request until the file is re-analyzed.
        let by_name = match self.class_short_names.take() {
            Some(catalog) => catalog,
            None => self.build_class_short_names(),
        };
        let mut items = Vec::new();
        for (simple, classes) in &by_name {
            let simple = simple.clone();
            for class in classes.iter().copied() {
                let package_flat = self.package_flat(class);
                // Nested class files are also entered under their flat binary
                // name; they are not "new X" candidates.
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
                if !expand_constructors {
                    let mut item = completion_item(
                        simple.clone(),
                        javac_analysis_api::SymbolKind::Class,
                        Some(package_flat.clone()),
                        None,
                    );
                    if insert_parentheses {
                        let snippet = insert::escape_snippet(&simple);
                        item.insert_text = Some(format!("{snippet}($0)"));
                        item.insert_text_format =
                            javac_analysis_api::InsertTextFormat::Snippet;
                    } else {
                        item.insert_text = Some(simple.clone());
                    }
                    item.documentation = self.class_documentation(&class_flat);
                    if let Some(edit) = import_context.edit_for(&class_flat, is_public_class) {
                        item.additional_text_edits.push(edit);
                    }
                    items.push(item);
                    continue;
                }
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
        self.class_short_names = Some(by_name);
        items
    }

    pub(crate) fn build_class_short_names(
        &mut self,
    ) -> BTreeMap<String, Vec<crate::ids::SymbolId>> {
        // Make sure platform packages have entered their classes before the
        // catalog is built. This is the load boundary; the catalog itself is
        // reused across completion requests.
        let packages = self
            .compiler
            .syms
            .all_visible_packages(&mut self.compiler.ctx);
        for (_, package) in &packages {
            let _ = self
                .compiler
                .ctx
                .complete_symbol(&self.compiler.syms, *package);
        }
        let mut by_name: BTreeMap<String, Vec<crate::ids::SymbolId>> = BTreeMap::new();
        let mut seen = HashSet::new();
        for class in self.compiler.syms.get_all_classes() {
            let symbol = self.compiler.ctx.sym(class);
            if !matches!(
                symbol.kind_data,
                crate::code::symbol::SymbolKind::ClassSymbol { .. }
            ) {
                continue;
            }
            // The symbol table can contain the same flat class through more
            // than one module/view; completion must not repeat it.
            let flat = symbol.flat_name().to_string();
            if !seen.insert(flat) {
                continue;
            }
            let simple = symbol.name.to_java_string().to_string_lossy();
            if !simple.is_empty() {
                by_name.entry(simple).or_default().push(class);
            }
        }
        by_name
    }

    /// Members of a fully qualified class ("java.util.Base64") - static only.
    pub(super) fn qualified_class_members(
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
    pub(super) fn class_type_parameters(&self, symbol: crate::ids::SymbolId) -> String {
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

}