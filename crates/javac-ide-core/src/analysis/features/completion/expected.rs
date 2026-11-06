use super::super::super::positions::utf16_to_byte;
use super::super::super::type_names::type_name;
use super::super::super::FileAnalysis;
use super::common::{argument_index, is_ident_char, last_keyword_before, last_open_paren_before, method_symbol, outer_constructor_name, previous_non_ws_byte, strip_type_arguments, ExpectedNew, OuterNewCall};
use crate::code::type_::TypeKindData;
use crate::code::type_tag::TypeTag;
use crate::tree::jc_tree::TreeKind;
use crate::util::list::List;

impl FileAnalysis {
    /// Expected type of a nested `new` expression. Argument positions are
    /// resolved from the enclosing call; casts, initializers/assignments and
    /// returns are resolved from the surrounding source when the call form is
    /// not present.
    pub(super) fn new_expected_type(
        &mut self,
        text: &str,
        cursor_byte: usize,
        prefix_len: usize,
    ) -> Option<ExpectedNew> {
        let before = text.get(..cursor_byte.saturating_sub(prefix_len))?;
        let new_pos = before.rfind("new")?;
        if new_pos > 0
            && before[..new_pos]
                .chars()
                .next_back()
                .is_some_and(is_ident_char)
        {
            return None;
        }
        if let Some(expected) = self.cast_expected_type(text, new_pos) {
            return Some(expected);
        }
        if let Some(expected) = self.call_argument_expected(text, cursor_byte, prefix_len) {
            return Some(expected);
        }
        if let Some(expected) = self.assignment_expected_type(text, new_pos) {
            return Some(expected);
        }
        self.return_expected_type(text, new_pos)
    }

    pub(super) fn expected_from_ty(&self, ty: crate::ids::TypeId) -> ExpectedNew {
        let array = self.compiler.ctx.typ(ty).tag == TypeTag::ARRAY;
        let label = type_name(&self.compiler.ctx, ty);
        ExpectedNew {
            label: label.clone(),
            array,
            class: self.compiler.ctx.typ(ty).tag == TypeTag::CLASS,
            array_labels: if array { vec![label] } else { Vec::new() },
        }
    }

    pub(super) fn expected_from_text(&mut self, text: &str) -> Option<ExpectedNew> {
        let text = strip_type_arguments(text.trim());
        let text = text.trim();
        if text.is_empty() || text == "var" {
            return None;
        }
        let array = text.ends_with("[]");
        let base = if array {
            text.trim_end_matches("[]").trim()
        } else {
            text
        };
        let base = base.split_whitespace().last().unwrap_or(base).trim();
        if base.is_empty() || base.contains('(') || base.contains(')') {
            return None;
        }
        if array {
            let label = format!("{base}[]");
            return Some(ExpectedNew {
                label: label.clone(),
                array: true,
                class: false,
                array_labels: vec![label],
            });
        }
        if let Some(class) = self.class_by_simple_name(base.rsplit('.').next()?) {
            if let Some(ty) = self.compiler.ctx.sym(class).ty {
                return Some(self.expected_from_ty(ty));
            }
        }
        matches!(
            base,
            "byte" | "short" | "char" | "int" | "long" | "float" | "double" | "boolean"
        )
        .then(|| ExpectedNew {
            label: base.to_string(),
            array: false,
            class: false,
            array_labels: Vec::new(),
        })
    }

    pub(super) fn cast_expected_type(&mut self, text: &str, new_pos: usize) -> Option<ExpectedNew> {
        let close = previous_non_ws_byte(text, new_pos)?;
        if text.as_bytes().get(close) != Some(&b')') {
            return None;
        }
        let mut depth = 0i32;
        let mut open = None;
        for (index, ch) in text[..=close].char_indices().rev() {
            match ch {
                ')' => depth += 1,
                '(' => {
                    depth -= 1;
                    if depth == 0 {
                        open = Some(index);
                        break;
                    }
                }
                _ => {}
            }
        }
        let open = open?;
        let inner = text[open + 1..close].trim();
        if inner.is_empty() || inner.contains('=') || inner.contains(',') || inner.contains(';') {
            return None;
        }
        self.expected_from_text(inner)
    }

    pub(super) fn assignment_expected_type(&mut self, text: &str, new_pos: usize) -> Option<ExpectedNew> {
        let start = text[..new_pos]
            .rfind([';', '{', '}'])
            .map(|index| index + 1)
            .unwrap_or(0);
        let segment = &text[start..new_pos];
        let equals = segment.rfind('=')?;
        let before_eq = segment[..equals].chars().next_back();
        let after_eq = segment[equals + 1..].chars().next();
        if before_eq.is_some_and(|ch| matches!(ch, '=' | '!' | '<' | '>' | '+' | '-' | '*' | '/' | '%' | '&' | '|' | '^'))
            || after_eq == Some('=')
        {
            return None;
        }
        let lhs = segment[..equals].trim();
        if lhs.is_empty() || lhs.contains(';') || lhs.contains('(') || lhs.contains(')') {
            return None;
        }
        let name_start = lhs
            .rfind(char::is_whitespace)
            .map(|index| index + 1)
            .unwrap_or(0);
        let name = lhs[name_start..].trim();
        if name.is_empty() || name.contains('.') || name.starts_with('@') {
            return None;
        }
        let type_text = strip_type_arguments(lhs[..name_start].trim());
        let type_text = type_text.split_whitespace().last().unwrap_or("").trim();
        if type_text.is_empty() {
            return self.find_local_var_type(name, new_pos);
        }
        if type_text == "var" {
            return self.find_local_var_type(name, new_pos);
        }
        self.expected_from_text(type_text)
    }

    pub(super) fn find_local_var_type(&self, name: &str, before: usize) -> Option<ExpectedNew> {
        let mut best: Option<(i32, ExpectedNew)> = None;
        for tree in self.compiler.ctx.trees.iter() {
            let TreeKind::VariableDecl {
                name: decl_name,
                sym: Some(sym),
                decl_pos,
                ..
            } = &tree.kind
            else {
                continue;
            };
            if decl_name.to_java_string().to_string_lossy() != name
                || *decl_pos < 0
                || *decl_pos as usize >= before
            {
                continue;
            }
            let Some(ty) = self.compiler.ctx.sym(*sym).ty else {
                continue;
            };
            let expected = self.expected_from_ty(ty);
            if best.as_ref().is_none_or(|(pos, _)| *decl_pos > *pos) {
                best = Some((*decl_pos, expected));
            }
        }
        best.map(|(_, expected)| expected)
    }

    pub(super) fn return_expected_type(&mut self, text: &str, new_pos: usize) -> Option<ExpectedNew> {
        let ret = last_keyword_before(text, new_pos, "return")?;
        if text[ret + "return".len()..new_pos].contains(';') {
            return None;
        }
        let mut best: Option<(i32, crate::ids::SymbolId)> = None;
        for tree in self.compiler.ctx.trees.iter() {
            let TreeKind::MethodDecl { sym: Some(sym), .. } = &tree.kind else {
                continue;
            };
            if tree.pos >= 0
                && tree.pos as usize <= new_pos
                && best.as_ref().is_none_or(|(pos, _)| tree.pos > *pos)
            {
                best = Some((tree.pos, *sym));
            }
        }
        let (_, method) = best?;
        let ty = self.compiler.ctx.sym(method).ty?;
        let restype = match &self.compiler.ctx.typ(ty).kind {
            TypeKindData::MethodType { restype, .. }
            | TypeKindData::PartiallyInferredMethodType { restype, .. } => *restype,
            _ => None,
        }?;
        Some(self.expected_from_ty(restype))
    }

    /// Expected type of a nested `new` expression at an enclosing call
    /// argument. The enclosing constructor is found lexically, so it still
    /// works while the inner expression is incomplete.
    pub(super) fn call_argument_expected(
        &mut self,
        text: &str,
        cursor_byte: usize,
        prefix_len: usize,
    ) -> Option<ExpectedNew> {
        let before = text.get(..cursor_byte.saturating_sub(prefix_len))?;
        let new_pos = before.rfind("new")?;
        if new_pos > 0
            && before[..new_pos]
                .chars()
                .next_back()
                .is_some_and(is_ident_char)
        {
            return None;
        }
        let open = last_open_paren_before(text, new_pos)?;
        let argument = argument_index(text, open, new_pos);
        let outer_new = self.outer_new_call(text, open);
        let actuals = outer_new.as_ref().and_then(|call| call.typeargs.as_ref());
        let argument_types: &[Option<crate::ids::TypeId>] = outer_new
            .as_ref()
            .map_or(&[], |call| call.argument_types.as_slice());
        let arguments = outer_new.as_ref().map_or(0, |call| call.arguments);
        if let Some(class_name) = outer_constructor_name(text, open) {
            return self.constructor_expected(
                &class_name,
                argument,
                arguments,
                actuals,
                argument_types,
            );
        }
        self.method_argument_expected(text, open, argument)
    }

    pub(super) fn class_type_formals(&mut self, class: crate::ids::SymbolId) -> Option<List<crate::ids::TypeId>> {
        let _ = self
            .compiler
            .ctx
            .complete_symbol(&self.compiler.syms, class);
        let mut formals = Vec::new();
        for tree in self.compiler.ctx.trees.iter() {
            let TreeKind::ClassDecl {
                sym: Some(declared),
                typarams,
                ..
            } = &tree.kind
            else {
                continue;
            };
            if *declared != class {
                continue;
            }
            for typaram in typarams.iter() {
                if let Some(ty) = self.compiler.ctx.tree(typaram).ty {
                    formals.push(ty);
                }
            }
            break;
        }
        if formals.is_empty()
            && let Some(scope) = self.compiler.ctx.sym(class).members()
        {
            let members = self.compiler.ctx.scope_get_symbols(
                scope,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            );
            for member in members {
                let symbol = self.compiler.ctx.sym(member);
                if matches!(
                    symbol.kind_data,
                    crate::code::symbol::SymbolKind::TypeVariableSymbol
                ) && let Some(ty) = symbol.ty
                {
                    formals.push(ty);
                }
            }
        }
        (!formals.is_empty()).then(|| List::from_iterable(formals))
    }

    pub(super) fn outer_new_call(&self, text: &str, open: usize) -> Option<OuterNewCall> {
        for tree in self.compiler.ctx.trees.iter() {
            let TreeKind::NewClass { args, .. } = &tree.kind else {
                continue;
            };
            let Some(start) = utf16_to_byte(text, tree.pos.max(0) as usize) else {
                continue;
            };
            let Some(call_open) = text[start..].find('(').map(|offset| start + offset) else {
                continue;
            };
            if call_open != open {
                continue;
            }
            let typeargs = tree
                .ty
                .and_then(|ty| match &self.compiler.ctx.typ(ty).kind {
                    TypeKindData::ClassType {
                        typarams_field: Some(params),
                        ..
                    } => Some(params.clone()),
                    _ => None,
                });
            return Some(OuterNewCall {
                typeargs,
                argument_types: args
                    .iter()
                    .map(|arg| self.compiler.ctx.tree(arg).ty)
                    .collect(),
                arguments: args.size() as usize,
            });
        }
        None
    }

    pub(super) fn constructor_expected(
        &mut self,
        class_name: &str,
        argument: usize,
        arguments: usize,
        explicit_actuals: Option<&List<crate::ids::TypeId>>,
        argument_types: &[Option<crate::ids::TypeId>],
    ) -> Option<ExpectedNew> {
        let class = self.class_by_simple_name(class_name)?;
        let _ = self
            .compiler
            .ctx
            .complete_symbol(&self.compiler.syms, class);
        let formals = self.class_type_formals(class);
        let scope = self.compiler.ctx.sym(class).members()?;
        let members = self.compiler.ctx.scope_get_symbols(
            scope,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        let mut fallback: Option<ExpectedNew> = None;
        let mut array_labels: Vec<String> = Vec::new();
        for member in members {
            let constructor = self.compiler.ctx.sym(member);
            if constructor.name.to_java_string().to_string_lossy() != "<init>" {
                continue;
            }
            let params = match &constructor.kind_data {
                crate::code::symbol::SymbolKind::MethodSymbol { params, .. } => params.clone(),
                _ => continue,
            };
            let varargs = (constructor.flags_field & crate::code::flags::VARARGS) != 0;
            if argument >= arguments {
                continue;
            }
            let inferred_actuals = if explicit_actuals.is_none() {
                formals.as_ref().and_then(|formals| {
                    self.infer_typeargs_from_previous(
                        formals,
                        &params,
                        varargs,
                        argument,
                        argument_types,
                    )
                })
            } else {
                None
            };
            let substitutions = explicit_actuals.or(inferred_actuals.as_ref());
            let param = params.iter().nth(argument).or_else(|| {
                if varargs && argument + 1 >= params.size() as usize {
                    params.iter().last()
                } else {
                    None
                }
            });
            let Some(param) = param else { continue };
            let Some(mut ty) = self.compiler.ctx.sym(param).ty else {
                continue;
            };
            if let (Some(formals), Some(actuals)) = (&formals, substitutions)
                && formals.size() == actuals.size()
            {
                let types = &self.compiler.types;
                let syms = &self.compiler.syms;
                let ctx = &mut self.compiler.ctx;
                ty = types.subst_type(ctx, syms, ty, formals, actuals, None);
            }
            let expected_ty = if varargs && argument + 1 >= params.size() as usize {
                match &self.compiler.ctx.typ(ty).kind {
                    TypeKindData::ArrayType { elemtype, .. } => *elemtype,
                    _ => Some(ty),
                }
            } else {
                Some(ty)
            };
            let Some(expected_ty) = expected_ty else {
                continue;
            };
            let expected = self.expected_from_ty(expected_ty);
            if expected.array {
                for label in expected.array_labels {
                    if !array_labels.contains(&label) {
                        array_labels.push(label);
                    }
                }
                continue;
            }
            if fallback.is_none() {
                fallback = Some(expected);
            }
        }
        if !array_labels.is_empty() {
            return Some(ExpectedNew {
                label: array_labels[0].clone(),
                array: true,
                class: false,
                array_labels,
            });
        }
        fallback
    }

    pub(super) fn method_argument_expected(
        &mut self,
        text: &str,
        open: usize,
        argument: usize,
    ) -> Option<ExpectedNew> {
        // Find the outer call and copy everything needed before mutating the
        // compiler context for generic substitution.
        let mut outer = None;
        for tree in self.compiler.ctx.trees.iter() {
            let TreeKind::MethodInvocation {
                meth: Some(meth),
                typeargs,
                args,
                ..
            } = &tree.kind
            else {
                continue;
            };
            let Some(start) = utf16_to_byte(text, tree.pos.max(0) as usize) else {
                continue;
            };
            let Some(call_open) = text[start..].find('(').map(|offset| start + offset) else {
                continue;
            };
            if call_open != open {
                continue;
            }
            let Some(method) = method_symbol(&self.compiler.ctx, *meth) else {
                continue;
            };
            let argument_types: Vec<Option<crate::ids::TypeId>> = args
                .iter()
                .map(|arg| self.compiler.ctx.tree(arg).ty)
                .collect();
            outer = Some((
                method,
                typeargs.clone(),
                args.size() as usize,
                argument_types,
            ));
            break;
        }
        let (method, typeargs, arguments, argument_types) = outer?;
        if argument >= arguments {
            return None;
        }
        let selected_name = self.compiler.ctx.sym(method).name.clone();
        let mut candidates = Vec::new();
        if let Some(owner) = self.compiler.ctx.sym(method).owner
            && let Some(scope) = self.compiler.ctx.sym(owner).members()
        {
            let members = self.compiler.ctx.scope_get_symbols(
                scope,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            );
            for member in members {
                let symbol = self.compiler.ctx.sym(member);
                if symbol.name == selected_name
                    && matches!(
                        symbol.kind_data,
                        crate::code::symbol::SymbolKind::MethodSymbol { .. }
                    )
                {
                    candidates.push(member);
                }
            }
        }
        if candidates.is_empty() {
            candidates.push(method);
        }
        let explicit_typeargs: Option<List<crate::ids::TypeId>> = {
            let actuals: Vec<crate::ids::TypeId> = typeargs
                .iter()
                .filter_map(|arg| self.compiler.ctx.tree(arg).ty)
                .collect();
            (actuals.len() == typeargs.size() as usize && !actuals.is_empty())
                .then(|| List::from_iterable(actuals))
        };
        let mut selected_fallback: Option<ExpectedNew> = None;
        let mut fallback: Option<ExpectedNew> = None;
        let mut array_labels: Vec<String> = Vec::new();
        for candidate in candidates {
            let params = match &self.compiler.ctx.sym(candidate).kind_data {
                crate::code::symbol::SymbolKind::MethodSymbol { params, .. } => params.clone(),
                _ => continue,
            };
            let varargs =
                (self.compiler.ctx.sym(candidate).flags_field & crate::code::flags::VARARGS) != 0;
            let method_tvars = self.compiler.ctx.sym(candidate).ty.and_then(|ty| {
                if let TypeKindData::ForAll { tvars, .. } = &self.compiler.ctx.typ(ty).kind {
                    Some(tvars.clone())
                } else {
                    None
                }
            });
            let inferred_typeargs = if explicit_typeargs.is_none() {
                method_tvars.as_ref().and_then(|tvars| {
                    self.infer_typeargs_from_previous(
                        tvars,
                        &params,
                        varargs,
                        argument,
                        &argument_types,
                    )
                })
            } else {
                None
            };
            let substitutions = explicit_typeargs.as_ref().or(inferred_typeargs.as_ref());
            let Some(param) = params.iter().nth(argument).or_else(|| {
                if varargs && argument + 1 >= params.size() as usize {
                    params.iter().last()
                } else {
                    None
                }
            }) else {
                continue;
            };
            let Some(mut ty) = self.compiler.ctx.sym(param).ty else {
                continue;
            };
            if let (Some(formals), Some(actuals)) = (&method_tvars, substitutions)
                && formals.size() == actuals.size()
            {
                let types = &self.compiler.types;
                let syms = &self.compiler.syms;
                let ctx = &mut self.compiler.ctx;
                ty = types.subst_type(ctx, syms, ty, formals, actuals, None);
            }
            let expected_ty = if varargs && argument + 1 >= params.size() as usize {
                match &self.compiler.ctx.typ(ty).kind {
                    TypeKindData::ArrayType { elemtype, .. } => *elemtype,
                    _ => Some(ty),
                }
            } else {
                Some(ty)
            };
            let Some(expected_ty) = expected_ty else {
                continue;
            };
            let expected = self.expected_from_ty(expected_ty);
            if expected.array {
                for label in expected.array_labels {
                    if !array_labels.contains(&label) {
                        array_labels.push(label);
                    }
                }
                continue;
            }
            if candidate == method && selected_fallback.is_none() {
                selected_fallback = Some(expected);
            } else if fallback.is_none() {
                fallback = Some(expected);
            }
        }
        if !array_labels.is_empty() {
            return Some(ExpectedNew {
                label: array_labels[0].clone(),
                array: true,
                class: false,
                array_labels,
            });
        }
        selected_fallback.or(fallback)
    }

    pub(super) fn infer_typeargs_from_previous(
        &self,
        formals: &List<crate::ids::TypeId>,
        params: &List<crate::ids::SymbolId>,
        varargs: bool,
        argument: usize,
        argument_types: &[Option<crate::ids::TypeId>],
    ) -> Option<List<crate::ids::TypeId>> {
        if formals.size() == 0 {
            return None;
        }
        let mut inferred: Vec<Option<crate::ids::TypeId>> = vec![None; formals.size() as usize];
        for (index, actual) in argument_types.iter().take(argument).enumerate() {
            let Some(actual) = actual else { continue };
            if params.size() == 0 {
                break;
            }
            let param_index = index.min(params.size() as usize - 1);
            let Some(param) = params.iter().nth(param_index) else {
                break;
            };
            let Some(mut param_ty) = self.compiler.ctx.sym(param).ty else {
                continue;
            };
            if varargs && param_index + 1 == params.size() as usize {
                if let TypeKindData::ArrayType {
                    elemtype: Some(elem),
                    ..
                } = &self.compiler.ctx.typ(param_ty).kind
                {
                    param_ty = *elem;
                }
            }
            for (formal_index, formal) in formals.iter().enumerate() {
                if param_ty == formal {
                    inferred[formal_index] = Some(*actual);
                } else if let TypeKindData::ArrayType {
                    elemtype: Some(elem),
                    ..
                } = &self.compiler.ctx.typ(param_ty).kind
                    && *elem == formal
                    && let TypeKindData::ArrayType {
                        elemtype: Some(actual_elem),
                        ..
                    } = &self.compiler.ctx.typ(*actual).kind
                {
                    inferred[formal_index] = Some(*actual_elem);
                }
            }
        }
        let mut result = Vec::with_capacity(formals.size() as usize);
        for (index, formal) in formals.iter().enumerate() {
            result.push(inferred[index].unwrap_or(formal));
        }
        Some(List::from_iterable(result))
    }

}