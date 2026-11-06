use std::path::Path;

use super::super::super::canonical;

use super::super::super::type_names::type_name;
use super::super::super::FileAnalysis;
use super::super::map_kind;
use super::common::{completion_item, CallContext};
use super::{context, insert, lexer};
use crate::analysis::jdk_docs::JdkDocCache;
use crate::code::type_::TypeKindData;

impl FileAnalysis {
    pub(super) fn ast_receiver(
        &mut self,
        text: &str,
        position: javac_analysis_api::Position,
        call: &CallContext,
    ) -> Vec<javac_analysis_api::CompletionItem> {
        let target_line = position.line as usize + 1;
        let target_column = position.column as usize + 1;
        let cursor_offset =
            super::super::super::positions::line_offset(text, target_line, target_column);
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
    pub(super) fn members_of_type_id(
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
    pub(super) fn members_of_type(
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

    pub(super) fn collect_members(
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

    pub(super) fn object_members(&mut self, call: &CallContext) -> Vec<javac_analysis_api::CompletionItem> {
        match self.class_by_simple_name("Object") {
            Some(class) => self.members_of_type(class, call, false, false),
            None => Vec::new(),
        }
    }

    pub(super) fn is_visible(&self, symbol: &crate::code::symbol::Symbol, call: &CallContext) -> bool {
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

    pub(super) fn superclass_of(&mut self, class: crate::ids::SymbolId) -> Option<crate::ids::SymbolId> {
        let ty = self.compiler.ctx.sym(class).ty?;
        let super_ty = match &self.compiler.ctx.typ(ty).kind {
            TypeKindData::ClassType { supertype_field, .. }
            | TypeKindData::ErasedClassType { supertype_field, .. } => *supertype_field,
            _ => None,
        }?;
        self.compiler.ctx.typ(super_ty).tsym
    }

    pub(super) fn package_flat(&self, class: crate::ids::SymbolId) -> String {
        crate::code::symbol::symbol_packge(&self.compiler.ctx, class)
            .map(|packge| self.compiler.ctx.sym(packge).flat_name().to_string())
            .unwrap_or_default()
    }

    pub(super) fn variable_type(&mut self, name: &str) -> Option<crate::ids::TypeId> {
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

    pub(super) fn class_by_simple_name(&mut self, name: &str) -> Option<crate::ids::SymbolId> {
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

    pub(super) fn file_class_symbol(&mut self, file: &Path) -> Option<crate::ids::SymbolId> {
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
    pub(super) fn class_documentation(&mut self, flat: &str) -> Option<String> {
        if let Some(doc) = self.traced_class_doc(flat) {
            return Some(doc);
        }
        self.jdk_docs
            .get_or_insert_with(JdkDocCache::new)
            .class_doc(flat)
    }

    /// Javadoc of a member: project source first, then the JDK sources.
    pub(super) fn member_documentation(
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

    pub(super) fn method_shape(&mut self, symbol: crate::ids::SymbolId) -> (bool, usize) {
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

    pub(super) fn insertion_anchors(&mut self, text: &str) -> context::Anchors {
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
    pub(super) fn method_return_type(&mut self, symbol: crate::ids::SymbolId) -> Option<String> {
        let ty = self.compiler.ctx.sym(symbol).ty?;
        let restype = match &self.compiler.ctx.typ(ty).kind {
            TypeKindData::MethodType { restype, .. }
            | TypeKindData::PartiallyInferredMethodType { restype, .. } => *restype,
            _ => None,
        }?;
        Some(type_name(&self.compiler.ctx, restype))
    }

    /// Human-readable parameter list for a method, e.g. "(int, String)".
    pub(super) fn method_signature(&mut self, symbol: crate::ids::SymbolId) -> Option<String> {
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

    pub(super) fn apply_prefix_replace(
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
    pub(super) fn apply_sort_text(&self, items: &mut [javac_analysis_api::CompletionItem]) {
        for (index, item) in items.iter_mut().enumerate() {
            item.sort_text = Some(format!("{index:04}"));
        }
    }
}