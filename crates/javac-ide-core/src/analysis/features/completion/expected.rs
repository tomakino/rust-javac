use super::super::super::type_names::type_name;
use super::super::super::FileAnalysis;
use super::common::{method_symbol, ExpectedNew};
use crate::code::type_::TypeKindData;
use crate::code::type_tag::TypeTag;
use crate::tree::jc_tree::TreeKind;
use crate::util::list::List;

#[derive(Clone)]
enum CallSiteKind {
    Method {
        method: crate::ids::SymbolId,
        typeargs: List<crate::ids::TreeId>,
    },
    New {
        class_name: String,
        actuals: Option<List<crate::ids::TypeId>>,
    },
}

struct CallSite {
    pos: usize,
    args: Vec<crate::ids::TreeId>,
    kind: CallSiteKind,
}

fn primitive_expected(tag: TypeTag) -> ExpectedNew {
    let label = match tag {
        TypeTag::BOOLEAN => "boolean",
        TypeTag::BYTE => "byte",
        TypeTag::CHAR => "char",
        TypeTag::SHORT => "short",
        TypeTag::INT => "int",
        TypeTag::LONG => "long",
        TypeTag::FLOAT => "float",
        TypeTag::DOUBLE => "double",
        _ => "?",
    };
    ExpectedNew {
        label: label.to_string(),
        array: false,
        class: false,
        array_labels: Vec::new(),
    }
}

impl FileAnalysis {
    /// Expected type of a nested `new` expression, resolved from enclosing AST
    /// nodes: call arguments, casts, assignments/initializers and returns.
    pub(super) fn new_expected_type(
        &mut self,
        text: &str,
        cursor_byte: usize,
        prefix_len: usize,
    ) -> Option<ExpectedNew> {
        let cursor = text[..cursor_byte.saturating_sub(prefix_len)]
            .encode_utf16()
            .count();
        let current_new = self.innermost_new_pos(cursor);
        if let Some(site) = self.enclosing_call(cursor, current_new) {
            let argument = self.call_argument_index(&site.args, cursor);
            match &site.kind {
                CallSiteKind::Method { method, typeargs } => {
                    if let Some(expected) = self.method_argument_expected(
                        *method,
                        typeargs.clone(),
                        &site.args,
                        argument,
                    ) {
                        return Some(expected);
                    }
                }
                CallSiteKind::New {
                    class_name,
                    actuals,
                } => {
                    let argument_types: Vec<Option<crate::ids::TypeId>> = site
                        .args
                        .iter()
                        .map(|arg| self.compiler.ctx.tree(*arg).ty)
                        .collect();
                    if let Some(expected) = self.constructor_expected(
                        class_name,
                        argument,
                        site.args.len(),
                        actuals.as_ref(),
                        &argument_types,
                    ) {
                        return Some(expected);
                    }
                }
            }
        }
        if let Some(expected) = self.cast_expected_type(cursor) {
            return Some(expected);
        }
        if let Some(expected) = self.assignment_expected_type(cursor) {
            return Some(expected);
        }
        if let Some(expected) = self.initializer_expected_type(cursor) {
            return Some(expected);
        }
        self.return_expected_type(cursor)
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

    fn expected_from_type_tree(&self, tree: crate::ids::TreeId) -> Option<ExpectedNew> {
        if let Some(ty) = self.compiler.ctx.tree(tree).ty {
            return Some(self.expected_from_ty(ty));
        }
        match &self.compiler.ctx.tree(tree).kind {
            TreeKind::PrimitiveTypeTree { typetag } => Some(primitive_expected(*typetag)),
            TreeKind::ArrayTypeTree { elemtype: Some(elemtype) } => {
                let elem = self.expected_from_type_tree(*elemtype)?;
                let label = format!("{}[]", elem.label);
                Some(ExpectedNew {
                    label: label.clone(),
                    array: true,
                    class: false,
                    array_labels: vec![label],
                })
            }
            TreeKind::AnnotatedType {
                underlying_type: Some(underlying_type),
                ..
            } => self.expected_from_type_tree(*underlying_type),
            _ => None,
        }
    }

    fn tree_end_utf16(&self, tree: crate::ids::TreeId) -> Option<usize> {
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

    fn node_range(&self, tree: crate::ids::TreeId) -> Option<(usize, usize)> {
        let start = self.compiler.ctx.tree(tree).pos;
        if start < 0 {
            return None;
        }
        let end = self.tree_end_utf16(tree)?;
        (end >= start as usize).then_some((start as usize, end))
    }

    fn innermost_new_pos(&self, cursor: usize) -> Option<usize> {
        self.compiler
            .ctx
            .trees
            .iter()
            .enumerate()
            .filter_map(|(index, tree)| {
                if !matches!(tree.kind, TreeKind::NewClass { .. }) || tree.pos < 0 {
                    return None;
                }
                let tree = crate::ids::TreeId::new(index as u32);
                let (start, end) = self.node_range(tree)?;
                (start <= cursor && cursor <= end).then_some(start)
            })
            .max()
    }

    fn enclosing_call(
        &self,
        cursor: usize,
        before: Option<usize>,
    ) -> Option<CallSite> {
        let mut best: Option<CallSite> = None;
        for (index, tree) in self.compiler.ctx.trees.iter().enumerate() {
            let tree_id = crate::ids::TreeId::new(index as u32);
            let (kind, args) = match &tree.kind {
                TreeKind::MethodInvocation {
                    meth, typeargs, args, ..
                } => {
                    let Some(meth) = meth else { continue };
                    let Some(method) = method_symbol(&self.compiler.ctx, *meth) else {
                        continue;
                    };
                    (
                        CallSiteKind::Method {
                            method,
                            typeargs: typeargs.clone(),
                        },
                        args.clone(),
                    )
                }
                TreeKind::NewClass { clazz, args, .. } => {
                    let Some(clazz) = clazz else { continue };
                    let Some(class_name) = self.simple_type_name(*clazz) else {
                        continue;
                    };
                    let actuals = tree.ty.and_then(|ty| {
                        match &self.compiler.ctx.typ(ty).kind {
                            TypeKindData::ClassType {
                                typarams_field: Some(params),
                                ..
                            } => Some(params.clone()),
                            _ => None,
                        }
                    });
                    (
                        CallSiteKind::New {
                            class_name,
                            actuals,
                        },
                        args.clone(),
                    )
                }
                _ => continue,
            };
            let Some((start, end)) = self.node_range(tree_id) else {
                continue;
            };
            if start > cursor || end < cursor {
                continue;
            }
            if before.is_some_and(|limit| start >= limit) {
                continue;
            }
            let site = CallSite {
                pos: start,
                args: args.iter().collect(),
                kind,
            };
            if best.as_ref().is_none_or(|current| site.pos > current.pos) {
                best = Some(site);
            }
        }
        best
    }

    fn simple_type_name(&self, tree: crate::ids::TreeId) -> Option<String> {
        let ty = self.compiler.ctx.tree(tree).ty?;
        let sym = self.compiler.ctx.typ(ty).tsym?;
        Some(self.compiler.ctx.sym(sym).name.to_java_string().to_string_lossy().to_string())
    }

    fn call_argument_index(&self, args: &[crate::ids::TreeId], cursor: usize) -> usize {
        args.iter()
            .filter(|arg| {
                self.compiler.ctx.tree(**arg).pos >= 0
                    && self
                        .tree_end_utf16(**arg)
                        .is_some_and(|end| end <= cursor)
            })
            .count()
    }

    fn cast_expected_type(&mut self, cursor: usize) -> Option<ExpectedNew> {
        let mut best: Option<(usize, ExpectedNew)> = None;
        for (index, tree) in self.compiler.ctx.trees.iter().enumerate() {
            let TreeKind::TypeCast {
                clazz: Some(clazz), ..
            } = &tree.kind
            else {
                continue;
            };
            let tree_id = crate::ids::TreeId::new(index as u32);
            let Some((start, end)) = self.node_range(tree_id) else {
                continue;
            };
            if !(start <= cursor && cursor <= end) {
                continue;
            }
            let Some(clazz_end) = self.tree_end_utf16(*clazz) else {
                continue;
            };
            if clazz_end > cursor {
                continue;
            }
            if let Some(expected) = self.expected_from_type_tree(*clazz) {
                if best.as_ref().is_none_or(|(pos, _)| start > *pos) {
                    best = Some((start, expected));
                }
            }
        }
        best.map(|(_, expected)| expected)
    }

    fn assignment_expected_type(&mut self, cursor: usize) -> Option<ExpectedNew> {
        self.compiler
            .ctx
            .trees
            .iter()
            .enumerate()
            .filter_map(|(index, tree)| {
                let TreeKind::Assign { lhs: Some(lhs), .. } = &tree.kind else {
                    return None;
                };
                let tree = crate::ids::TreeId::new(index as u32);
                let (start, end) = self.node_range(tree)?;
                if !(start <= cursor && cursor <= end) {
                    return None;
                }
                let ty = self.compiler.ctx.tree(*lhs).ty?;
                Some((start, self.expected_from_ty(ty)))
            })
            .max_by_key(|(pos, _)| *pos)
            .map(|(_, expected)| expected)
    }

    fn initializer_expected_type(&mut self, cursor: usize) -> Option<ExpectedNew> {
        self.compiler
            .ctx
            .trees
            .iter()
            .find_map(|tree| {
                let TreeKind::VariableDecl {
                    vartype: Some(vartype),
                    init: Some(init),
                    ..
                } = &tree.kind
                else {
                    return None;
                };
                let (start, end) = self.node_range(*init)?;
                (start <= cursor && cursor <= end)
                    .then(|| self.expected_from_type_tree(*vartype))
                    .flatten()
            })
    }

    fn return_expected_type(&mut self, cursor: usize) -> Option<ExpectedNew> {
        let in_return = self
            .compiler
            .ctx
            .trees
            .iter()
            .enumerate()
            .any(|(index, tree)| {
                matches!(tree.kind, TreeKind::Return { .. })
                    && self
                        .node_range(crate::ids::TreeId::new(index as u32))
                        .is_some_and(|(start, end)| start <= cursor && cursor <= end)
            });
        if !in_return {
            return None;
        }
        let method = self
            .compiler
            .ctx
            .trees
            .iter()
            .enumerate()
            .filter_map(|(index, tree)| {
                let TreeKind::MethodDecl { sym: Some(sym), .. } = &tree.kind else {
                    return None;
                };
                (tree.pos >= 0 && tree.pos as usize <= cursor).then_some((tree.pos, *sym, index))
            })
            .max_by_key(|(pos, ..)| *pos)?;
        let ty = self.compiler.ctx.sym(method.1).ty?;
        let restype = match &self.compiler.ctx.typ(ty).kind {
            TypeKindData::MethodType { restype, .. }
            | TypeKindData::PartiallyInferredMethodType { restype, .. } => *restype,
            _ => None,
        }?;
        Some(self.expected_from_ty(restype))
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
        method: crate::ids::SymbolId,
        typeargs: List<crate::ids::TreeId>,
        args: &[crate::ids::TreeId],
        argument: usize,
    ) -> Option<ExpectedNew> {
        let argument_types: Vec<Option<crate::ids::TypeId>> = args
            .iter()
            .map(|arg| self.compiler.ctx.tree(*arg).ty)
            .collect();
        let arguments = args.len();
        if argument >= arguments {
            return None;
        }
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