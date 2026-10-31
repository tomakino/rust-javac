// Origin: com/sun/tools/javac/comp/TransPatterns.java @ 982d53c (jdk-17.0.19-ga)

//! This pass translates pattern-matching constructs, such as instanceof <pattern>.
//!
//! PORTING_RULES: the upstream class extends TreeTranslator; the port keeps a
//! TreeTranslator delegate (base) whose default navigation is reached through a
//! TreeVisitor::visit_tree override, so every node kind that TransPatterns does
//! not specialise is translated by the shared TreeTranslator (registered adapter
//! for Java inheritance). The upstream abstract BindingContext class and its two
//! concrete subclasses map to the BindingContext struct with a kind discriminator.
//!

use std::cell::RefCell;
use std::rc::Rc;

use crate::code::flags;
use crate::code::symbol::{Symbol, SymbolKind};
use crate::code::symtab::Symtab;
use crate::code::types::Types;
use crate::comp::attr::Attr;
use crate::comp::attr_context::AttrContext;
use crate::comp::env::Env;
use crate::comp::operators::Operators;
use crate::comp::resolve::Resolve;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::tree::jc_tree::{Tag, TreeKind, TreeVisitor};
use crate::tree::tree_info;
use crate::tree::tree_maker::{TreeFactory, TreeMaker};
use crate::tree::tree_translator::TreeTranslator;
use crate::util::list::List;
use crate::util::names::Names;
use javac_source::tree::case_tree::CaseKind;

    /// This pass translates pattern-matching constructs, such as instanceof <pattern>.
    ///
    /// The Rust port keeps the upstream visitor shape: a TreeVisitor whose default
    /// navigation descends into every child and whose pattern-specific visitors
    /// replace the pattern trees with desugared statements.
pub struct TransPatterns {
    pub syms: Rc<RefCell<Symtab>>,
    pub attr: Rc<RefCell<Attr>>,
    pub rs: Rc<RefCell<Resolve>>,
    pub types: Rc<Types>,
    pub operators: Rc<Operators>,
    pub names: Rc<Names>,
    pub target: crate::jvm::target::Target,
    pub preview: Rc<crate::code::preview::Preview>,
    pub make: Rc<RefCell<TreeMaker>>,
    pub env: Option<Rc<Env<AttrContext>>>,
    pub binding_context: BindingContext,
    pub pending_match_label: Option<TreeId>,
    pub debug_trans_patterns: bool,
    pub current_class: Option<SymbolId>,
    pub current_method_sym: Option<SymbolId>,
    pub current_value: Option<SymbolId>,
    pub result: Option<TreeId>,
    pub base: TreeTranslator,
}

    /// The binding context of one desugared pattern (upstream BindingContext).
    ///
    /// # Arguments
    /// - `kind` — the Default/Basic/Fence kind discriminator
pub struct BindingContext {
    kind: BindingContextKind,
    hoisted_var_map: Vec<(SymbolId, SymbolId)>,
    parent: Option<Box<BindingContext>>,
    /// Adapter for the visitBlock anonymous fence subclass: the statements list
    /// that a preserved binding declaration is prepended to.
    statements: Option<Rc<RefCell<Vec<TreeId>>>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BindingContextKind {
    Default,
    Basic,
    Fence,
}

impl Default for BindingContext {
    fn default() -> BindingContext {
        BindingContext {
            kind: BindingContextKind::Default,
            hoisted_var_map: Vec::new(),
            parent: None,
            statements: None,
        }
    }
}

impl BindingContext {
    fn new_basic(parent: BindingContext) -> BindingContext {
        BindingContext {
            kind: BindingContextKind::Basic,
            hoisted_var_map: Vec::new(),
            parent: Some(Box::new(parent)),
            statements: None,
        }
    }

    fn new_fence(parent: BindingContext, statements: Rc<RefCell<Vec<TreeId>>>) -> BindingContext {
        BindingContext {
            kind: BindingContextKind::Fence,
            hoisted_var_map: Vec::new(),
            parent: Some(Box::new(parent)),
            statements: Some(statements),
        }
    }

    fn pop(self) -> BindingContext {
        match self.parent {
            Some(parent) => *parent,
            None => self,
        }
    }

    fn binding_declared(
        &mut self,
        ctx: &mut CompilerContext,
        var_symbol: SymbolId,
        current_method_sym: Option<SymbolId>,
    ) -> Option<SymbolId> {
        match self.kind {
            BindingContextKind::Default | BindingContextKind::Fence => None,
            BindingContextKind::Basic => {
                let parent_result = self
                    .parent
                    .as_mut()
                    .and_then(|parent| parent.binding_declared(ctx, var_symbol, current_method_sym));
                if let Some(res) = parent_result {
                    return Some(res);
                }
                let flags_field = ctx.sym(var_symbol).flags_field;
                let name = ctx.sym(var_symbol).name.clone();
                let ty = ctx.sym(var_symbol).ty.expect("binding symbol type");
                let owner = current_method_sym;
                let res = ctx.alloc_symbol(Symbol::new(
                    crate::code::kinds::Kind::VAR,
                    flags_field,
                    name,
                    Some(ty),
                    owner,
                    crate::code::symbol::SymbolKind::VarSymbol {
                        lazy_const_value: None,
                        pos: 0,
                        adr: 0,
                        data_kind: None,
                        data: None,
                    },
                ));
                let raw = ctx.sym(var_symbol).get_raw_attributes();
                ctx.sym_mut(res).set_type_attributes(raw);
                self.hoisted_var_map.push((var_symbol, res));
                Some(res)
            }
        }
    }

    fn get_binding_for(&self, ctx: &CompilerContext, var_symbol: SymbolId) -> Option<SymbolId> {
        let parent_result = self
            .parent
            .as_ref()
            .and_then(|parent| parent.get_binding_for(ctx, var_symbol));
        if let Some(res) = parent_result {
            return Some(res);
        }
        for (binding, var) in &self.hoisted_var_map {
            if binding_is_alias_for(ctx, *binding, var_symbol) {
                return Some(*var);
            }
        }
        None
    }

    fn binding_vars(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        make: &Rc<RefCell<TreeMaker>>,
        diag_pos: i32,
    ) -> List<TreeId> {
        if self.hoisted_var_map.is_empty() {
            return List::nil();
        }
        let entries = self.hoisted_var_map.clone();
        let mut stats: List<TreeId> = List::nil();
        for (binding, var_symbol) in entries {
            let decl = make_hoisted_var_decl(ctx, syms, make, diag_pos, var_symbol);
            let preserved = (ctx.sym(binding).flags_field & flags::MATCH_BINDING_TO_OUTER) != 0;
            let prepended = if preserved {
                if let Some(parent) = self.parent.as_mut() {
                    parent.try_prepend(ctx, binding, decl)
                } else {
                    false
                }
            } else {
                false
            };
            if !prepended {
                stats = stats.append(decl);
            }
        }
        stats
    }

    fn try_prepend(&mut self, _ctx: &mut CompilerContext, _binding: SymbolId, var: TreeId) -> bool {
        if self.kind == BindingContextKind::Fence {
            if let Some(statements) = self.statements.as_ref() {
                statements.borrow_mut().push(var);
            }
            true
        } else {
            false
        }
    }
}

impl TransPatterns {
    #[allow(clippy::too_many_arguments)]
    /// Creates the pass with its context services and the post-pass services
    /// (TransTypes, Enter, names, operators) it shares with Lower.
    ///
    /// # Arguments
    /// - `attr` — the attribution service, used for default methods and scoping
    /// - `diags` — the diagnostic factory
    /// - `log` — the compiler log
    /// - `...` — the remaining registered dependency services
    pub fn new(
        syms: Rc<RefCell<Symtab>>,
        attr: Rc<RefCell<Attr>>,
        rs: Rc<RefCell<Resolve>>,
        types: Rc<Types>,
        operators: Rc<Operators>,
        names: Rc<Names>,
        target: crate::jvm::target::Target,
        preview: Rc<crate::code::preview::Preview>,
        make: Rc<RefCell<TreeMaker>>,
        debug_trans_patterns: bool,
    ) -> TransPatterns {
        TransPatterns {
            syms,
            attr,
            rs,
            types,
            operators,
            names,
            target,
            preview,
            make,
            env: None,
            binding_context: BindingContext::default(),
            pending_match_label: None,
            debug_trans_patterns,
            current_class: None,
            current_method_sym: None,
            current_value: None,
            result: None,
            base: TreeTranslator::new(),
        }
    }

    /// Translates one tree; when a translation-specific environment is active the
    /// visitor replaces the node and the replacement is returned.
    ///
    /// # Arguments
    /// - `ctx` — the compiler context
    /// - `tree` — the tree being translated (None is accepted and returned as None)
    ///
    /// # Returns
    /// The translated tree, or None when the input was None
    pub fn translate(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) -> Option<TreeId> {
        let tree = tree?;
        self.result = None;
        crate::tree::jc_tree::accept(ctx, tree, self);
        self.result.take().or(Some(tree))
    }

    /// Applies `translate` to every tree in the list, preserving order.
    ///
    /// # Arguments
    /// - `ctx` — the compiler context
    /// - `trees` — the trees to translate
    ///
    /// # Returns
    /// The translated trees in the original order
    pub fn translate_list(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &List<TreeId>,
    ) -> List<TreeId> {
        let mut result: List<TreeId> = List::nil();
        for tree in trees.iter() {
            let translated = self.translate(ctx, Some(tree)).expect("translated tree");
            result = result.append(translated);
        }
        result
    }

    /// Translates a list of variable declarations.
    ///
    /// # Arguments
    /// - `ctx` — the compiler context
    /// - `trees` — the variable declarations
    ///
    /// # Returns
    /// The translated variable declarations
    pub fn translate_var_defs(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }

    /// Translates a list of type parameters.
    ///
    /// # Arguments
    /// - `ctx` — the compiler context
    /// - `trees` — the type parameters
    ///
    /// # Returns
    /// The translated type parameters
    pub fn translate_type_params(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }

    /// Translates a list of switch cases.
    ///
    /// # Arguments
    /// - `ctx` — the compiler context
    /// - `trees` — the case clauses
    ///
    /// # Returns
    /// The translated case clauses
    pub fn translate_cases(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }

    /// Translates a list of catch clauses.
    ///
    /// # Arguments
    /// - `ctx` — the compiler context
    /// - `trees` — the catch clauses
    ///
    /// # Returns
    /// The translated catch clauses
    pub fn translate_catchers(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }

    /// Translates a list of annotations.
    ///
    /// # Arguments
    /// - `ctx` — the compiler context
    /// - `trees` — the annotations
    ///
    /// # Returns
    /// The translated annotations
    pub fn translate_annotations(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
    fn default_visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (expr, pattern) = match &ctx.tree(tree).kind {
            TreeKind::InstanceOf { expr, pattern } => (*expr, *pattern),
            _ => panic!("expected an instanceof"),
        };
        let expr = self.translate(ctx, expr);
        let pattern = self.translate(ctx, pattern);
        if let TreeKind::InstanceOf {
            expr: e,
            pattern: p,
        } = &mut ctx.tree_mut(tree).kind
        {
            *e = expr;
            *p = pattern;
        }
        self.result = Some(tree);
    }








    fn default_visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::Switch {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch"),
        };
        let selector = self.translate(ctx, selector);
        let cases = self.translate_cases(ctx, &cases);
        if let TreeKind::Switch {
            selector: s,
            cases: c,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = selector;
            *c = cases;
        }
        self.result = Some(tree);
    }


    fn default_visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch expression"),
        };
        let selector = self.translate(ctx, selector);
        let cases = self.translate_cases(ctx, &cases);
        if let TreeKind::SwitchExpression {
            selector: s,
            cases: c,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = selector;
            *c = cases;
        }
        self.result = Some(tree);
    }


    fn default_visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Binary { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a binary expression"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::Binary { lhs: l, rhs: r, .. } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }


    fn default_visit_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, truepart, falsepart) = match &ctx.tree(tree).kind {
            TreeKind::Conditional {
                cond,
                truepart,
                falsepart,
                ..
            } => (*cond, *truepart, *falsepart),
            _ => panic!("expected a conditional"),
        };
        let cond = self.translate(ctx, cond);
        let truepart = self.translate(ctx, truepart);
        let falsepart = self.translate(ctx, falsepart);
        if let TreeKind::Conditional {
            cond: c,
            truepart: t,
            falsepart: f,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = cond;
            *t = truepart;
            *f = falsepart;
        }
        self.result = Some(tree);
    }


    fn default_visit_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, thenpart, elsepart) = match &ctx.tree(tree).kind {
            TreeKind::If {
                cond,
                thenpart,
                elsepart,
            } => (*cond, *thenpart, *elsepart),
            _ => panic!("expected an if statement"),
        };
        let cond = self.translate(ctx, cond);
        let thenpart = self.translate(ctx, thenpart);
        let elsepart = self.translate(ctx, elsepart);
        if let TreeKind::If {
            cond: c,
            thenpart: t,
            elsepart: e,
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = cond;
            *t = thenpart;
            *e = elsepart;
        }
        self.result = Some(tree);
    }


    fn default_visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (init, cond, step, body) = match &ctx.tree(tree).kind {
            TreeKind::ForLoop {
                init,
                cond,
                step,
                body,
            } => (init.clone(), *cond, step.clone(), *body),
            _ => panic!("expected a for loop"),
        };
        let init = self.translate_list(ctx, &init);
        let cond = self.translate(ctx, cond);
        let step = self.translate_list(ctx, &step);
        let body = self.translate(ctx, body);
        if let TreeKind::ForLoop {
            init: i,
            cond: c,
            step: s,
            body: b,
        } = &mut ctx.tree_mut(tree).kind
        {
            *i = init;
            *c = cond;
            *s = step;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn default_visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, body) = match &ctx.tree(tree).kind {
            TreeKind::WhileLoop { cond, body } => (*cond, *body),
            _ => panic!("expected a while loop"),
        };
        let cond = self.translate(ctx, cond);
        let body = self.translate(ctx, body);
        if let TreeKind::WhileLoop { cond: c, body: b } = &mut ctx.tree_mut(tree).kind {
            *c = cond;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn default_visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, cond) = match &ctx.tree(tree).kind {
            TreeKind::DoWhileLoop { body, cond } => (*body, *cond),
            _ => panic!("expected a do loop"),
        };
        let body = self.translate(ctx, body);
        let cond = self.translate(ctx, cond);
        if let TreeKind::DoWhileLoop { body: b, cond: c } = &mut ctx.tree_mut(tree).kind {
            *b = body;
            *c = cond;
        }
        self.result = Some(tree);
    }


    fn default_visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, restype, typarams, recvparam, params, thrown, body) = match &ctx.tree(tree).kind
        {
            TreeKind::MethodDecl {
                mods,
                restype,
                typarams,
                recvparam,
                params,
                thrown,
                body,
                ..
            } => (
                *mods,
                *restype,
                typarams.clone(),
                *recvparam,
                params.clone(),
                thrown.clone(),
                *body,
            ),
            _ => panic!("expected a method declaration"),
        };
        let mods = self.translate(ctx, mods);
        let restype = self.translate(ctx, restype);
        let typarams = self.translate_type_params(ctx, &typarams);
        let recvparam = self.translate(ctx, recvparam);
        let params = self.translate_var_defs(ctx, &params);
        let thrown = self.translate_list(ctx, &thrown);
        let body = self.translate(ctx, body);
        if let TreeKind::MethodDecl {
            mods: m,
            restype: r,
            typarams: tp,
            recvparam: rp,
            params: ps,
            thrown: th,
            body: b,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *r = restype;
            *tp = typarams;
            *rp = recvparam;
            *ps = params;
            *th = thrown;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn default_visit_ident(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }




    fn default_visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (params, body) = match &ctx.tree(tree).kind {
            TreeKind::Lambda { params, body, .. } => (params.clone(), *body),
            _ => panic!("expected a lambda"),
        };
        let params = self.translate_list(ctx, &params);
        let body = self.translate(ctx, body);
        if let TreeKind::Lambda {
            params: p, body: b, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *p = params;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn default_visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, typarams, extending, implementing, defs) = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl {
                mods,
                typarams,
                extending,
                implementing,
                defs,
                ..
            } => (
                *mods,
                typarams.clone(),
                *extending,
                implementing.clone(),
                defs.clone(),
            ),
            _ => panic!("expected a class declaration"),
        };
        let mods = self.translate(ctx, mods);
        let typarams = self.translate_type_params(ctx, &typarams);
        let extending = self.translate(ctx, extending);
        let implementing = self.translate_list(ctx, &implementing);
        let defs = self.translate_list(ctx, &defs);
        if let TreeKind::ClassDecl {
            mods: m,
            typarams: t,
            extending: e,
            implementing: i,
            defs: d,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *t = typarams;
            *e = extending;
            *i = implementing;
            *d = defs;
        }
        self.result = Some(tree);
    }



    /// Adapter: take the current binding context (std::mem::take on the Java
    /// assignment `bindingContext = new BasicBindingContext()` pattern).
    fn take_binding_context(&mut self) -> BindingContext {
        std::mem::take(&mut self.binding_context)
    }

    fn decorate_statement(&mut self, ctx: &mut CompilerContext, stat: TreeId) -> TreeId {
        let pos = ctx.tree(stat).pos;
        let make = Rc::clone(&self.make);
        let stats = self.binding_context.binding_vars(ctx, &self.syms.borrow_mut(), &make, pos);
        if stats.is_empty() {
            stat
        } else {
            let mut make_ref = make.borrow_mut();
            make_ref.at(pos);
            make_ref.block(ctx, 0, stats.append(stat))
        }
    }

    fn decorate_expression(&mut self, ctx: &mut CompilerContext, expr: TreeId) -> TreeId {
        let hoisted: Vec<SymbolId> = self
            .binding_context
            .hoisted_var_map
            .iter()
            .map(|(_, var)| *var)
            .collect();
        let pos = ctx.tree(expr).pos;
        let expr_ty = ctx.tree(expr).ty;
        let mut result = expr;
        for vsym in hoisted {
            let make = Rc::clone(&self.make);
            let decl = make_hoisted_var_decl(ctx, &self.syms.borrow_mut(), &make, pos, vsym);
            result = {
                let mut make_ref = make.borrow_mut();
                make_ref.at(pos);
                make_ref.let_expr(ctx, List::of(decl), result)
            };
            if let Some(ty) = expr_ty {
                ctx.tree_mut(result).ty = Some(ty);
            }
        }
        result
    }
    fn handle_switch(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        selector: TreeId,
        cases: List<TreeId>,
        has_total_pattern: bool,
        pattern_switch: bool,
    ) {
        if !pattern_switch {
            if ctx.tree(tree).has_tag(Tag::SWITCH) {
                self.default_visit_switch(ctx, tree);
            } else {
                self.default_visit_switch_expression(ctx, tree);
            }
            return;
        }
        {
            let env = self.env.clone().expect("env");
            let toplevel = env.toplevel.expect("env toplevel");
            let sourcefile = match &ctx.tree(toplevel).kind {
                TreeKind::CompilationUnit { sourcefile, .. } => sourcefile.clone(),
                _ => None,
            };
            assert!(self.preview.is_enabled(), "AssertionError: preview is not enabled");
            if let Some(sourcefile) = sourcefile {
                assert!(
                    self.preview.uses_preview(&sourcefile),
                    "AssertionError: source does not use preview"
                );
            }
        }
        let seltype = {
            let selector_ty = ctx.tree(selector).ty.expect("switch selector type");
            if ctx.typ(selector_ty).has_tag(crate::code::type_tag::TypeTag::BOT) {
                self.syms.borrow().object_type
            } else {
                selector_ty
            }
        };
        let mut merged: Vec<TreeId> = Vec::new();
        let mut current = cases.clone();
        while current.non_empty() {
            let head = *current.head();
            current = current.tail();
            if case_stats(ctx, head).is_empty() && current.non_empty() {
                let next = *current.head();
                let head_labels = case_labels(ctx, head);
                let next_labels = case_labels(ctx, next);
                set_case_labels(ctx, next, head_labels.append_list(&next_labels));
            } else {
                merged.push(head);
            }
        }
        let cases = List::from_iterable(merged);
        let (
            object_type,
            int_type,
            bot_type,
            method_class,
            class_type,
            switch_bootstraps_type,
            method_handle_lookup_type,
            method_type_type,
            string_type,
            no_symbol,
        ) = {
            let syms = self.syms.borrow();
            (
                syms.object_type,
                syms.int_type,
                syms.bot_type,
                syms.method_class,
                syms.class_type,
                syms.switch_bootstraps_type,
                syms.method_handle_lookup_type,
                syms.method_type_type,
                syms.string_type,
                syms.no_symbol,
            )
        };
        let mut has_null_case = false;
        for case in cases.iter() {
            for label in case_labels(ctx, case).iter() {
                if is_null_pattern(ctx, label) {
                    has_null_case = true;
                }
            }
        }
        let last_case = cases.last().expect("switch without cases");
        if has_total_pattern && !has_null_case {
            let mut has_default = false;
            for case in cases.iter() {
                for label in case_labels(ctx, case).iter() {
                    if ctx.tree(label).has_tag(Tag::DEFAULTCASELABEL) {
                        has_default = true;
                    }
                }
            }
            if !has_default {
                let labels = case_labels(ctx, last_case);
                let null_label = self.make_lit(ctx, bot_type, None);
                set_case_labels(ctx, last_case, labels.prepend(null_label));
                has_null_case = true;
            }
        }
        let selector = self.translate(ctx, Some(selector)).expect("translated selector");
        let mut statements: Vec<TreeId> = Vec::new();
        let pos = ctx.tree(tree).pos;
        let synthetic = self.target.synthetic_name_char();
        let temp_name = self.names.from_string(&javac_strings::JavaString::from(
            format!("selector{}{}temp", pos, synthetic).as_str(),
        ));
        let temp = new_var_symbol(ctx, flags::SYNTHETIC as i64, temp_name, seltype, self.current_method_sym);
        let mut temp_init = selector;
        if !has_null_case {
            let attr_rc = Rc::clone(&self.attr);
            let mut attr = attr_rc.borrow_mut();
            let syms = self.syms.borrow_mut();
            temp_init = attr.make_null_check(ctx, &syms, selector);
        }
        let temp_def = {
            let mut make = self.make.borrow_mut();
            make.at(pos);
            let syms = self.syms.borrow_mut();
            make.var_def_symbol(ctx, &syms, temp, Some(temp_init))
        };
        statements.push(temp_def);
        let index_name = self.names.from_string(&javac_strings::JavaString::from(
            format!("{}{}index", pos, synthetic).as_str(),
        ));
        let index = new_var_symbol(ctx, flags::SYNTHETIC as i64, index_name, int_type, self.current_method_sym);
        let zero = self.make_lit(
            ctx,
            int_type,
            Some(crate::tree::jc_tree::LiteralValue::Integer(0)),
        );
        let index_def = {
            let mut make = self.make.borrow_mut();
            make.at(pos);
            let syms = self.syms.borrow_mut();
            make.var_def_symbol(ctx, &syms, index, Some(zero))
        };
        statements.push(index_def);
        let wildcard = ctx.alloc_type(crate::code::type_::Type::new(
            crate::code::type_tag::TypeTag::WILDCARD,
            None,
            crate::code::type_::TypeKindData::WildcardType {
                type_: Some(object_type),
                kind: crate::code::bound_kind::BoundKind::UNBOUND,
                bound: None,
                is_printing_bound: std::cell::Cell::new(false),
            },
        ));
        let class_tsym = ctx.typ(class_type).tsym.expect("class type symbol");
        let class_param = ctx.alloc_type(crate::code::type_::Type::new_class_type(
            ctx.typ(class_type).get_enclosing_type(),
            Some(List::of(wildcard)),
            class_tsym,
        ));
        let class_array = {
            let syms = self.syms.borrow_mut();
            self.types.make_array_type(ctx, &syms, class_param)
        };
        let static_arg_types = List::from_iterable(vec![
            method_handle_lookup_type,
            string_type,
            method_type_type,
            class_array,
        ]);
        let mut static_arg_values: Vec<crate::jvm::pool_constant::LoadableConstantRef> = Vec::new();
        for case in cases.iter() {
            for label in case_labels(ctx, case).iter() {
                if let Some(value) = self.to_loadable_constant(ctx, label, seltype) {
                    static_arg_values.push(value);
                }
            }
        }
        let enum_selector = ctx
            .typ(seltype)
            .tsym
            .map(|tsym| (ctx.sym(tsym).flags_field & crate::code::flags::ENUM as i64) != 0)
            .unwrap_or(false);
        let bootstrap_name = if enum_selector {
            self.names.enumSwitch.clone()
        } else {
            self.names.typeSwitch.clone()
        };
        let env = self.env.clone().expect("env");
        let bsm = {
            let attr_rc = Rc::clone(&self.attr);
            let mut attr = attr_rc.borrow_mut();
            let chk = Rc::clone(&attr.chk);
            let infer = Rc::clone(&attr.infer);
            let chk_borrow = chk;
            let syms = self.syms.borrow_mut();
            let diag_pos: Option<Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>> =
                Some(Rc::new(crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos)));
            let rs = self.rs.borrow_mut();
            rs.resolve_internal_method(
                ctx,
                &syms,
                &self.types,
                &mut attr,
                &chk_borrow,
                &infer,
                diag_pos,
                &env,
                switch_bootstraps_type,
                bootstrap_name.clone(),
                static_arg_types,
                None,
            )
        };
        let first_arg = if enum_selector { seltype } else { object_type };
        let indy_type = ctx.alloc_type(crate::code::type_::Type::new_method_type(
            List::from_iterable(vec![first_arg, int_type]),
            int_type,
            List::nil(),
            method_class,
        ));
        let bsm_handle = crate::code::symbol::as_method_handle(ctx, bsm, false);
        let dyn_sym = crate::code::symbol::new_dynamic_method_symbol(
            ctx,
            bootstrap_name.clone(),
            no_symbol,
            bsm_handle,
            indy_type,
            static_arg_values,
        );
        let qualifier = {
            let mut make = self.make.borrow_mut();
            make.at(pos);
            let syms = self.syms.borrow_mut();
            let owner = ctx.sym(bsm).owner.expect("bootstrap owner");
            let base = make.qual_ident(ctx, &syms, owner);
            make.select(ctx, base, bootstrap_name.clone())
        };
        if let TreeKind::FieldAccess { sym: slot, .. } = &mut ctx.tree_mut(qualifier).kind {
            *slot = Some(dyn_sym);
        }
        ctx.tree_mut(qualifier).ty = Some(int_type);
        let temp_ident = {
            let mut make = self.make.borrow_mut();
            make.at(pos);
            make.ident_symbol(ctx, temp)
        };
        let index_ident = {
            let mut make = self.make.borrow_mut();
            make.at(pos);
            make.ident_symbol(ctx, index)
        };
        let selector_expr = {
            let mut make = self.make.borrow_mut();
            make.at(pos);
            make.apply(ctx, List::nil(), qualifier, List::from_iterable(vec![temp_ident, index_ident]))
        };
        ctx.tree_mut(selector_expr).ty = Some(int_type);
        let mut i: i32 = 0;
        let mut previous_completes_normally = false;
        let mut has_default = false;
        for case in cases.iter() {
            let case_pos = ctx.tree(case).pos;
            let labels = case_labels(ctx, case);
            let mut cleared = labels.clone();
            let has_joined_null =
                labels.length() > 1 && labels.iter().any(|label| is_null_pattern(ctx, label));
            if has_joined_null {
                let filtered: Vec<TreeId> = labels
                    .iter()
                    .filter(|label| !is_null_pattern(ctx, *label))
                    .collect();
                cleared = List::from_iterable(filtered);
            }
            let single_pattern = cleared.length() == 1
                && is_pattern_label(ctx, *cleared.head())
                && !previous_completes_normally;
            if single_pattern {
                let pattern = *cleared.head();
                self.binding_context = BindingContext::new_basic(self.take_binding_context());
                let prev_current_value = self.current_value;
                self.current_value = Some(temp);
                let test = self.translate(ctx, Some(pattern)).expect("translated pattern");
                let stats = {
                    let old_stats = case_stats(ctx, case);
                    self.translate_list(ctx, &old_stats)
                };
                let continue_switch = {
                    let mut make = self.make.borrow_mut();
                    make.at(case_pos);
                    make.continue_(ctx, None)
                };
                if let TreeKind::Continue { target: slot, .. } = &mut ctx.tree_mut(continue_switch).kind {
                    *slot = Some(tree);
                }
                let next_index = i + 1;
                let index_literal = self.make_lit(
                    ctx,
                    int_type,
                    Some(crate::tree::jc_tree::LiteralValue::Integer(next_index)),
                );
                let index_assign = {
                    let mut make = self.make.borrow_mut();
                    make.at(pos);
                    make.assign(ctx, index_ident, index_literal)
                };
                ctx.tree_mut(index_assign).ty = Some(int_type);
                let exec_index = {
                    let mut make = self.make.borrow_mut();
                    make.at(pos);
                    make.exec(ctx, index_assign)
                };
                let block = {
                    let mut make = self.make.borrow_mut();
                    make.at(pos);
                    make.block(ctx, 0, List::from_iterable(vec![exec_index, continue_switch]))
                };
                let not_test = self.make_unary(ctx, Tag::NOT, test);
                let if_tree = {
                    let mut make = self.make.borrow_mut();
                    make.at(pos);
                    make.if_(ctx, not_test, block, None)
                };
                let binding_stats = {
                    let make = Rc::clone(&self.make);
                    let syms = self.syms.borrow_mut();
                    self.binding_context.binding_vars(ctx, &syms, &make, case_pos)
                };
                let new_stats = binding_stats.append_list(&stats.prepend(if_tree));
                set_case_stats(ctx, case, new_stats);
                self.current_value = prev_current_value;
                let popped = self.take_binding_context().pop();
                self.binding_context = popped;
            } else {
                let stats = {
                    let old_stats = case_stats(ctx, case);
                    self.translate_list(ctx, &old_stats)
                };
                set_case_stats(ctx, case, stats);
            }
            let mut translated_labels: Vec<TreeId> = Vec::new();
            for label in case_labels(ctx, case).iter() {
                if ctx.tree(label).has_tag(Tag::DEFAULTCASELABEL) {
                    translated_labels.push(label);
                    has_default = true;
                } else if has_total_pattern && !has_default && case == last_case && is_pattern_label(ctx, label) {
                    let default_label = {
                        let mut make = self.make.borrow_mut();
                        make.at(pos);
                        make.default_case_label(ctx)
                    };
                    translated_labels.push(default_label);
                } else {
                    let value = if is_null_pattern(ctx, label) {
                        -1
                    } else {
                        let value = i;
                        i += 1;
                        value
                    };
                    let literal = self.make_lit(
                        ctx,
                        int_type,
                        Some(crate::tree::jc_tree::LiteralValue::Integer(value)),
                    );
                    translated_labels.push(literal);
                }
            }
            set_case_labels(ctx, case, List::from_iterable(translated_labels));
            if matches!(case_kind(ctx, case), CaseKind::STATEMENT) {
                previous_completes_normally = case_completes_normally(ctx, case);
            } else {
                previous_completes_normally = false;
                let last_stat = case_stats(ctx, case).last().expect("case without statements");
                let last_pos = ctx.tree(last_stat).pos;
                let brk = {
                    let mut make = self.make.borrow_mut();
                    make.at(last_pos);
                    make.break_(ctx, None)
                };
                if let TreeKind::Break { target: slot, .. } = &mut ctx.tree_mut(brk).kind {
                    *slot = Some(tree);
                }
                let stats = case_stats(ctx, case).append(brk);
                set_case_stats(ctx, case, stats);
            }
        }
        if ctx.tree(tree).has_tag(Tag::SWITCH) {
            if let TreeKind::Switch { selector: selector_slot, cases: cases_slot, .. } =
                &mut ctx.tree_mut(tree).kind
            {
                *selector_slot = Some(selector_expr);
                *cases_slot = cases;
            }
            statements.push(tree);
            let block = {
                let mut make = self.make.borrow_mut();
                make.at(pos);
                make.block(ctx, 0, List::from_iterable(statements))
            };
            self.result = Some(block);
        } else {
            let tree_ty = ctx.tree(tree).ty;
            if let TreeKind::SwitchExpression { selector: selector_slot, cases: cases_slot, .. } =
                &mut ctx.tree_mut(tree).kind
            {
                *selector_slot = Some(selector_expr);
                *cases_slot = cases;
            }
            let result = {
                let mut make = self.make.borrow_mut();
                make.at(pos);
                make.let_expr(ctx, List::from_iterable(statements), tree)
            };
            if let TreeKind::LetExpr { needs_cond: slot, .. } = &mut ctx.tree_mut(result).kind {
                *slot = true;
            }
            ctx.tree_mut(result).ty = tree_ty;
            self.result = Some(result);
        }
    }


    fn make_lit(&mut self, ctx: &mut CompilerContext, ty: TypeId, value: Option<crate::tree::jc_tree::LiteralValue>) -> TreeId {
        let tag = ctx.typ(ty).get_tag();
        let tree = {
            let mut make = self.make.borrow_mut();
            make.literal(ctx, tag, value.clone())
        };
        let const_ty = match value {
            Some(value) => ctx.type_const_type(ty, literal_value_constant(&value)),
            None => ty,
        };
        ctx.tree_mut(tree).ty = Some(const_ty);
        tree
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_loadable_constant(
        &mut self,
        ctx: &mut CompilerContext,
        label: TreeId,
        selector: TypeId,
    ) -> Option<crate::jvm::pool_constant::LoadableConstantRef> {
        use crate::jvm::pool_constant::{BasicConstant, LoadableConstantRef};
        if is_pattern_label(ctx, label) {
            let principal_type = self.principal_type(ctx, label);
            let syms = self.syms.borrow();
            let selector_type = if self.types.is_subtype(ctx, &syms, selector, principal_type) {
                selector
            } else {
                principal_type
            };
            return Some(LoadableConstantRef::Class(selector_type));
        }
        if is_expression_label(ctx, label) && !tree_info::is_null(ctx, label) {
            let label_ty = ctx.tree(label).ty.expect("case label type");
            let label_tsym = ctx.typ(label_ty).tsym.expect("case label symbol");
            if (ctx.sym(label_tsym).flags_field & flags::ENUM as i64) != 0 {
                let name = match &ctx.tree(label).kind {
                    TreeKind::Ident { name, .. } => name.clone(),
                    _ => panic!("enum case label must be an identifier"),
                };
                return Some(LoadableConstantRef::Basic(BasicConstant::string(
                    name.to_java_string(),
                )));
            }
            let value = ctx.typ(label_ty).const_value().expect("case constant value");
            let basic = match ctx.typ(label_ty).get_tag() {
                crate::code::type_tag::TypeTag::BYTE
                | crate::code::type_tag::TypeTag::CHAR
                | crate::code::type_tag::TypeTag::SHORT
                | crate::code::type_tag::TypeTag::INT => {
                    let int_value = match value {
                        crate::util::constants::ConstantValue::Integer(value) => *value,
                        crate::util::constants::ConstantValue::Byte(value) => i32::from(*value),
                        crate::util::constants::ConstantValue::Short(value) => i32::from(*value),
                        crate::util::constants::ConstantValue::Character(value) => i32::from(*value),
                        _ => panic!("unexpected case constant value"),
                    };
                    BasicConstant::int(int_value)
                }
                crate::code::type_tag::TypeTag::CLASS => {
                    let string_value = match value {
                        crate::util::constants::ConstantValue::String(value) => value,
                        _ => panic!("unexpected case constant value"),
                    };
                    BasicConstant::string(string_value.clone())
                }
                _ => panic!("unexpected case constant tag"),
            };
            return Some(LoadableConstantRef::Basic(basic));
        }
        None
    }
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - env: the class environment;
    ///   - cdef: the class declaration;
    ///   - make: the tree maker.
    /// # Returns
    ///   - the translated class declaration.
    /// # Panics
    ///   - translation of a list element produced null.
    pub fn translate_top_level_class(
        &mut self,
        ctx: &mut CompilerContext,
        env: &Rc<Env<AttrContext>>,
        cdef: TreeId,
        make: Rc<RefCell<TreeMaker>>,
    ) -> TreeId {
        let prev_make = std::mem::replace(&mut self.make, make);
        self.env = Some(Rc::clone(env));
        let result = self.translate(ctx, Some(cdef));
        self.make = prev_make;
        self.env = None;
        result.expect("translated class")
    }

    fn principal_type(&self, ctx: &mut CompilerContext, pattern: TreeId) -> TypeId {
        let primary = tree_info::primary_pattern_type(ctx, pattern);
        let ty = primary.ty.expect("primary pattern type");
        let syms_borrow = self.syms.borrow_mut();
        let erased = self.types.erasure(ctx, &syms_borrow, ty);
        self.types.boxed_type_or_type(ctx, &syms_borrow, erased)
    }

    fn make_type_test(
        &mut self,
        ctx: &mut CompilerContext,
        lhs: TreeId,
        type_tree: TreeId,
    ) -> TreeId {
        let tree = {
            let mut make = self.make.borrow_mut();
            make.type_test(ctx, lhs, type_tree)
        };
        let syms = self.syms.borrow();
        ctx.tree_mut(tree).ty = Some(syms.boolean_type);
        tree
    }

    fn make_binary(
        &mut self,
        ctx: &mut CompilerContext,
        optag: Tag,
        lhs: TreeId,
        rhs: TreeId,
    ) -> TreeId {
        let tree = {
            let mut make = self.make.borrow_mut();
            make.binary(ctx, optag, lhs, rhs)
        };
        let lhs_ty = ctx.tree(lhs).ty.expect("lhs type");
        let rhs_ty = ctx.tree(rhs).ty.expect("rhs type");
        let operator = {
            let syms = self.syms.borrow_mut();
            self.operators.resolve_binary(
                ctx,
                &syms,
                &self.types,
                ctx.tree(tree).pos,
                optag,
                lhs_ty,
                rhs_ty,
            )
        };
        if let TreeKind::Binary { operator: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(operator);
        }
        let operator_ty = ctx.sym(operator).ty.expect("operator type");
        let ret = self.types.get_return_type_public(ctx, operator_ty);
        ctx.tree_mut(tree).ty = Some(ret);
        tree
    }

    fn make_unary(
        &mut self,
        ctx: &mut CompilerContext,
        optag: Tag,
        arg: TreeId,
    ) -> TreeId {
        let tree = {
            let mut make = self.make.borrow_mut();
            make.unary(ctx, optag, arg)
        };
        let arg_ty = ctx.tree(arg).ty.expect("arg type");
        let operator = {
            let syms = self.syms.borrow_mut();
            self.operators.resolve_unary(
                ctx,
                &syms,
                &self.types,
                ctx.tree(tree).pos,
                optag,
                arg_ty,
            )
        };
        if let TreeKind::Unary { operator: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(operator);
        }
        let operator_ty = ctx.sym(operator).ty.expect("operator type");
        let ret = self.types.get_return_type_public(ctx, operator_ty);
        ctx.tree_mut(tree).ty = Some(ret);
        tree
    }

    fn convert(
        &mut self,
        ctx: &mut CompilerContext,
        expr: TreeId,
        target: TypeId,
    ) -> TreeId {
        let pos = ctx.tree(expr).pos;
        let type_tree = {
            let syms = self.syms.borrow_mut();
            let mut make = self.make.borrow_mut();
            make.at(pos);
            make.type_from_type(ctx, &syms, target)
        };
        let result = {
            let mut make = self.make.borrow_mut();
            make.at(pos);
            make.type_cast(ctx, type_tree, expr)
        };
        ctx.tree_mut(result).ty = Some(target);
        result
    }
}

fn binding_aliases(ctx: &CompilerContext, sym: SymbolId) -> List<SymbolId> {
    match &ctx.sym(sym).kind_data {
        SymbolKind::BindingSymbol { aliases, .. } if !aliases.is_empty() => aliases.clone(),
        _ => List::of(sym),
    }
}

fn binding_is_alias_for(ctx: &CompilerContext, binding: SymbolId, other: SymbolId) -> bool {
    let mine = binding_aliases(ctx, binding);
    let theirs = binding_aliases(ctx, other);
    for candidate in theirs.iter() {
        if !mine.iter().any(|entry| entry == candidate) {
            return false;
        }
    }
    true
}

fn make_hoisted_var_decl(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    make: &Rc<RefCell<TreeMaker>>,
    pos: i32,
    var_symbol: SymbolId,
) -> TreeId {
    let mut make_ref = make.borrow_mut();
    make_ref.at(pos);
    make_ref.var_def_symbol(ctx, syms, var_symbol, None)
}

impl TreeVisitor for TransPatterns {    fn visit_top_level(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let defs = match &ctx.tree(tree).kind {
            TreeKind::CompilationUnit { defs, .. } => defs.clone(),
            _ => panic!("expected a compilation unit"),
        };
        let defs = self.translate_list(ctx, &defs);
        if let TreeKind::CompilationUnit { defs: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = defs;
        }
        self.result = Some(tree);
    }

    fn visit_package_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, pid) = match &ctx.tree(tree).kind {
            TreeKind::PackageDecl {
                annotations, pid, ..
            } => (annotations.clone(), *pid),
            _ => panic!("expected a package declaration"),
        };
        let annotations = self.translate_list(ctx, &annotations);
        let pid = self.translate(ctx, pid);
        if let TreeKind::PackageDecl {
            annotations: a,
            pid: p,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *p = pid;
        }
        self.result = Some(tree);
    }

    fn visit_import(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Import { qualid, .. } => *qualid,
            _ => panic!("expected an import"),
        };
        let qualid = self.translate(ctx, qualid);
        if let TreeKind::Import { qualid: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = qualid;
        }
        self.result = Some(tree);
    }

    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let class_sym = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("class declaration without a symbol"),
        };
        let prev_class = self.current_class;
        let prev_method_sym = self.current_method_sym;
        self.current_class = Some(class_sym);
        self.current_method_sym = None;
        self.default_visit_class_def(ctx, tree);
        self.current_class = prev_class;
        self.current_method_sym = prev_method_sym;
    }
    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let method_sym = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("method declaration without a symbol"),
        };
        let prev_method_sym = self.current_method_sym;
        self.current_method_sym = Some(method_sym);
        self.default_visit_method_def(ctx, tree);
        self.current_method_sym = prev_method_sym;
    }
    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, vartype, init, flags_value) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl { mods, vartype, init, .. } => {
                let flags_value = match mods {
                    Some(mods) => match &ctx.tree(*mods).kind {
                        TreeKind::Modifiers { flags, .. } => *flags,
                        _ => 0,
                    },
                    None => 0,
                };
                (*mods, *vartype, *init, flags_value)
            }
            _ => panic!("ClassCastException: JCVariableDecl expected"),
        };
        let prev_method_sym = self.current_method_sym;
        let translated_mods = self.translate(ctx, mods);
        let translated_vartype = self.translate(ctx, vartype);
        if self.current_method_sym.is_none() {
            let current_class = self.current_class;
            let name = self.names.empty.clone();
            let method = ctx.alloc_symbol(Symbol::new(
                crate::code::kinds::Kind::MTH,
                (flags_value & flags::STATIC as i64) | flags::BLOCK as i64,
                name,
                None,
                current_class,
                SymbolKind::MethodSymbol {
                    extra_params: List::nil(),
                    captured_locals: List::nil(),
                    params: List::nil(),
                    default_value: None,
                    original_enclosing_method: None,
                },
            ));
            self.current_method_sym = Some(method);
        }
        let translated_init = self.translate(ctx, init);
        if let TreeKind::VariableDecl { mods: m, vartype: vt, init: i, .. } =
            &mut ctx.tree_mut(tree).kind
        {
            *m = translated_mods;
            *vt = translated_vartype;
            *i = translated_init;
        }
        self.result = Some(tree);
        self.current_method_sym = prev_method_sym;
    }
    fn visit_skip(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let stats = match &ctx.tree(tree).kind {
            TreeKind::Block { stats, .. } => stats.clone(),
            _ => panic!("ClassCastException: JCBlock expected"),
        };
        let block_flags = match &ctx.tree(tree).kind {
            TreeKind::Block { flags, .. } => *flags,
            _ => 0,
        };
        let statements: Rc<RefCell<Vec<TreeId>>> = Rc::new(RefCell::new(Vec::new()));
        let prev_context = self.take_binding_context();
        self.binding_context = BindingContext::new_fence(prev_context, Rc::clone(&statements));
        let old_method_sym = self.current_method_sym;
        if self.current_method_sym.is_none() {
            let current_class = self.current_class;
            let name = self.names.empty.clone();
            let method = ctx.alloc_symbol(Symbol::new(
                crate::code::kinds::Kind::MTH,
                block_flags | flags::BLOCK as i64,
                name,
                None,
                current_class,
                SymbolKind::MethodSymbol {
                    extra_params: List::nil(),
                    captured_locals: List::nil(),
                    params: List::nil(),
                    default_value: None,
                    original_enclosing_method: None,
                },
            ));
            self.current_method_sym = Some(method);
        }
        for stat in stats.iter() {
            let translated = self.translate(ctx, Some(stat)).expect("translated block statement");
            statements.borrow_mut().push(translated);
        }
        let new_stats = List::from_iterable(statements.borrow().clone());
        if let TreeKind::Block { stats: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = new_stats;
        }
        self.result = Some(tree);
        self.current_method_sym = old_method_sym;
        let popped = self.take_binding_context().pop();
        self.binding_context = popped;
    }
    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.binding_context = BindingContext::new_basic(self.take_binding_context());
        self.default_visit_do_loop(ctx, tree);
        let translated = self.result.take().expect("translated tree");
        let result = self.decorate_statement(ctx, translated);
        self.result = Some(result);
        let popped = self.take_binding_context().pop();
        self.binding_context = popped;
    }
    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.binding_context = BindingContext::new_basic(self.take_binding_context());
        self.default_visit_while_loop(ctx, tree);
        let translated = self.result.take().expect("translated tree");
        let result = self.decorate_statement(ctx, translated);
        self.result = Some(result);
        let popped = self.take_binding_context().pop();
        self.binding_context = popped;
    }
    fn visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.binding_context = BindingContext::new_basic(self.take_binding_context());
        self.default_visit_for_loop(ctx, tree);
        let translated = self.result.take().expect("translated tree");
        let result = self.decorate_statement(ctx, translated);
        self.result = Some(result);
        let popped = self.take_binding_context().pop();
        self.binding_context = popped;
    }
    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (var, expr, body) = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop { var, expr, body } => (*var, *expr, *body),
            _ => panic!("expected an enhanced for loop"),
        };
        let var = self.translate(ctx, var);
        let expr = self.translate(ctx, expr);
        let body = self.translate(ctx, body);
        if let TreeKind::EnhancedForLoop {
            var: v,
            expr: e,
            body: b,
        } = &mut ctx.tree_mut(tree).kind
        {
            *v = var;
            *e = expr;
            *b = body;
        }
        self.result = Some(tree);
    }

    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let body = match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement { body, .. } => *body,
            _ => panic!("expected a labelled statement"),
        };
        let body = self.translate(ctx, body);
        if let TreeKind::LabeledStatement { body: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = body;
        }
        self.result = Some(tree);
    }

    fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases, has_total_pattern, pattern_switch) = match &ctx.tree(tree).kind {
            TreeKind::Switch { selector: Some(selector), cases, has_total_pattern, pattern_switch, .. } => (
                *selector, cases.clone(), *has_total_pattern, *pattern_switch,
            ),
            _ => panic!("ClassCastException: JCSwitch expected"),
        };
        self.handle_switch(ctx, tree, selector, cases, has_total_pattern, pattern_switch);
    }

    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases, has_total_pattern, pattern_switch) = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression { selector: Some(selector), cases, has_total_pattern, pattern_switch, .. } => (
                *selector, cases.clone(), *has_total_pattern, *pattern_switch,
            ),
            _ => panic!("ClassCastException: JCSwitchExpression expected"),
        };
        self.handle_switch(ctx, tree, selector, cases, has_total_pattern, pattern_switch);
    }


    fn visit_case(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (labels, stats) = match &ctx.tree(tree).kind {
            TreeKind::Case { labels, stats, .. } => (labels.clone(), stats.clone()),
            _ => panic!("expected a case"),
        };
        let labels = self.translate_list(ctx, &labels);
        let stats = self.translate_list(ctx, &stats);
        if let TreeKind::Case {
            labels: l,
            stats: s,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *l = labels;
            *s = stats;
        }
        self.result = Some(tree);
    }



    fn visit_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lock, body) = match &ctx.tree(tree).kind {
            TreeKind::Synchronized { lock, body } => (*lock, *body),
            _ => panic!("expected a synchronized statement"),
        };
        let lock = self.translate(ctx, lock);
        let body = self.translate(ctx, body);
        if let TreeKind::Synchronized { lock: l, body: b } = &mut ctx.tree_mut(tree).kind {
            *l = lock;
            *b = body;
        }
        self.result = Some(tree);
    }

    fn visit_try(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (resources, body, catchers, finalizer) = match &ctx.tree(tree).kind {
            TreeKind::Try {
                resources,
                body,
                catchers,
                finalizer,
                ..
            } => (resources.clone(), *body, catchers.clone(), *finalizer),
            _ => panic!("expected a try statement"),
        };
        let resources = self.translate_list(ctx, &resources);
        let body = self.translate(ctx, body);
        let catchers = self.translate_catchers(ctx, &catchers);
        let finalizer = self.translate(ctx, finalizer);
        if let TreeKind::Try {
            resources: r,
            body: b,
            catchers: c,
            finalizer: f,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *r = resources;
            *b = body;
            *c = catchers;
            *f = finalizer;
        }
        self.result = Some(tree);
    }

    fn visit_catch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (param, body) = match &ctx.tree(tree).kind {
            TreeKind::Catch { param, body } => (*param, *body),
            _ => panic!("expected a catch clause"),
        };
        let param = self.translate(ctx, param);
        let body = self.translate(ctx, body);
        if let TreeKind::Catch { param: p, body: b } = &mut ctx.tree_mut(tree).kind {
            *p = param;
            *b = body;
        }
        self.result = Some(tree);
    }

    fn visit_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.binding_context = BindingContext::new_basic(self.take_binding_context());
        self.default_visit_conditional(ctx, tree);
        let translated = self.result.take().expect("translated tree");
        let result = self.decorate_expression(ctx, translated);
        self.result = Some(result);
        let popped = self.take_binding_context().pop();
        self.binding_context = popped;
    }
    fn visit_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.binding_context = BindingContext::new_basic(self.take_binding_context());
        self.default_visit_if(ctx, tree);
        let translated = self.result.take().expect("translated tree");
        let result = self.decorate_statement(ctx, translated);
        self.result = Some(result);
        let popped = self.take_binding_context().pop();
        self.binding_context = popped;
    }
    fn visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::ExpressionStatement { expr } => *expr,
            _ => panic!("expected an expression statement"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::ExpressionStatement { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }

    fn visit_break(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, .. } => *value,
            _ => panic!("expected a yield"),
        };
        let value = self.translate(ctx, value);
        if let TreeKind::Yield { value: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = value;
        }
        self.result = Some(tree);
    }

    fn visit_continue(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Return { expr } => *expr,
            _ => panic!("expected a return"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Return { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }

    fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Throw { expr } => *expr,
            _ => panic!("expected a throw"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Throw { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }

    fn visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, detail) = match &ctx.tree(tree).kind {
            TreeKind::Assert { cond, detail } => (*cond, *detail),
            _ => panic!("expected an assert"),
        };
        let cond = self.translate(ctx, cond);
        let detail = self.translate(ctx, detail);
        if let TreeKind::Assert { cond: c, detail: d } = &mut ctx.tree_mut(tree).kind {
            *c = cond;
            *d = detail;
        }
        self.result = Some(tree);
    }

    fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (meth, args) = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation { meth, args, .. } => (*meth, args.clone()),
            _ => panic!("expected a method invocation"),
        };
        let meth = self.translate(ctx, meth);
        let args = self.translate_list(ctx, &args);
        if let TreeKind::MethodInvocation {
            meth: m, args: a, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = meth;
            *a = args;
        }
        self.result = Some(tree);
    }

    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (encl, clazz, args, def) = match &ctx.tree(tree).kind {
            TreeKind::NewClass {
                encl,
                clazz,
                args,
                def,
                ..
            } => (*encl, *clazz, args.clone(), *def),
            _ => panic!("expected a new class"),
        };
        let encl = self.translate(ctx, encl);
        let clazz = self.translate(ctx, clazz);
        let args = self.translate_list(ctx, &args);
        let def = self.translate(ctx, def);
        if let TreeKind::NewClass {
            encl: e,
            clazz: c,
            args: a,
            def: d,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *e = encl;
            *c = clazz;
            *a = args;
            *d = def;
        }
        self.result = Some(tree);
    }

    fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, orig_dim_annotations_list, elemtype, dims, elems) =
            match &ctx.tree(tree).kind {
                TreeKind::NewArray {
                    annotations,
                    dim_annotations,
                    elemtype,
                    dims,
                    elems,
                } => (
                    annotations.clone(),
                    dim_annotations.clone(),
                    *elemtype,
                    dims.clone(),
                    elems.clone(),
                ),
                _ => panic!("expected a new array"),
            };
        let annotations = self.translate_annotations(ctx, &annotations);
        let mut dim_annotations = List::nil();
        for orig_dim_annotations in orig_dim_annotations_list.iter() {
            dim_annotations =
                dim_annotations.append(self.translate_annotations(ctx, &orig_dim_annotations));
        }
        let elemtype = self.translate(ctx, elemtype);
        let dims = self.translate_list(ctx, &dims);
        let elems = elems.map(|elems| self.translate_list(ctx, &elems));
        if let TreeKind::NewArray {
            annotations: a,
            dim_annotations: da,
            elemtype: et,
            dims: d,
            elems: e,
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *da = dim_annotations;
            *et = elemtype;
            *d = dims;
            *e = elems;
        }
        self.result = Some(tree);
    }

    fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let prev_context = self.take_binding_context();
        self.binding_context =
            BindingContext::new_fence(prev_context, Rc::new(RefCell::new(Vec::new())));
        self.default_visit_lambda(ctx, tree);
        let popped = self.take_binding_context().pop();
        self.binding_context = popped;
    }
    fn visit_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Parens { expr } => *expr,
            _ => panic!("expected parens"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Parens { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }

    fn visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Assign { lhs, rhs } => (*lhs, *rhs),
            _ => panic!("expected an assignment"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::Assign { lhs: l, rhs: r } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }

    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a compound assignment"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::AssignOp { lhs: l, rhs: r, .. } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }

    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, .. } => *arg,
            _ => panic!("expected a unary expression"),
        };
        let arg = self.translate(ctx, arg);
        if let TreeKind::Unary { arg: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = arg;
        }
        self.result = Some(tree);
    }

    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.binding_context = BindingContext::new_basic(self.take_binding_context());
        self.default_visit_binary(ctx, tree);
        let translated = self.result.take().expect("translated tree");
        let result = self.decorate_expression(ctx, translated);
        self.result = Some(result);
        let popped = self.take_binding_context().pop();
        self.binding_context = popped;
    }
    fn visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, expr) = match &ctx.tree(tree).kind {
            TreeKind::TypeCast { clazz, expr } => (*clazz, *expr),
            _ => panic!("expected a type cast"),
        };
        let clazz = self.translate(ctx, clazz);
        let expr = self.translate(ctx, expr);
        if let TreeKind::TypeCast { clazz: c, expr: e } = &mut ctx.tree_mut(tree).kind {
            *c = clazz;
            *e = expr;
        }
        self.result = Some(tree);
    }

    fn visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (expr, pattern) = match &ctx.tree(tree).kind {
            TreeKind::InstanceOf { expr: Some(expr), pattern: Some(pattern) } => (*expr, *pattern),
            _ => panic!("JCInstanceOf without expr/pattern"),
        };
        let is_pattern = matches!(
            ctx.tree(pattern).tag,
            Tag::BINDINGPATTERN | Tag::PARENTHESIZEDPATTERN
        );
        if !is_pattern {
            self.default_visit_type_test(ctx, tree);
            return;
        }
        let expr_ty = ctx.tree(expr).ty.expect("instanceof expr type");
        let temp_type = if ctx.typ(expr_ty).has_tag(crate::code::type_tag::TypeTag::BOT) {
            let syms = self.syms.borrow();
            syms.object_type
        } else {
            expr_ty
        };
        let prev_current_value = self.current_value;
        self.binding_context = BindingContext::new_basic(self.take_binding_context());
        let translated_expr = self.translate(ctx, Some(expr)).expect("translated instanceof expr");
        let expr_sym = tree_info::symbol(ctx, translated_expr);
        let is_var_owner_val_mth = expr_sym
            .map(|sym| {
                ctx.sym(sym).kind == crate::code::kinds::Kind::VAR
                    && ctx
                        .sym(sym)
                        .owner
                        .map(|owner| {
                            // Java: exprSym.owner.kind.matches(KindSelector.VAL_MTH)
                            ctx.sym(owner)
                                .kind
                                .matches(crate::code::kinds::KindSelector::VAL_MTH)
                        })
                        .unwrap_or(false)
            })
            .unwrap_or(false);
        self.current_value = if is_var_owner_val_mth {
            expr_sym
        } else {
            let pos = ctx.tree(tree).pos;
            let synthetic = self.target.synthetic_name_char();
            let name = format!(
                "patt{}{}temp",
                pos,
                char::from_u32(u32::from(synthetic)).unwrap_or('$')
            );
            let name = self.names.from_string(&javac_strings::JavaString::from(name.as_str()));
            Some(ctx.alloc_symbol(Symbol::new(
                crate::code::kinds::Kind::VAR,
                (flags::FINAL | flags::SYNTHETIC) as i64,
                name,
                Some(temp_type),
                self.current_method_sym,
                SymbolKind::VarSymbol {
                    lazy_const_value: None,
                    pos: 0,
                    adr: 0,
                    data_kind: None,
                    data: None,
                },
            )))
        };
        let principal = self.principal_type(ctx, pattern);
        let current_value = self.current_value.expect("current value");
        let type_tree = {
            let mut make = self.make.borrow_mut();
            make.at(ctx.tree(tree).pos);
            let syms = self.syms.borrow_mut();
            make.type_from_type(ctx, &syms, principal)
        };
        let current_ident = {
            let mut make = self.make.borrow_mut();
            make.at(ctx.tree(tree).pos);
            make.ident_symbol(ctx, current_value)
        };
        let test = self.make_type_test(ctx, current_ident, type_tree);
        let translated_pattern = self.translate(ctx, Some(pattern)).expect("translated pattern");
        let mut result_expression = self.make_binary(ctx, Tag::AND, test, translated_pattern);
        if Some(current_value) != expr_sym {
            let var_def = {
                let mut make = self.make.borrow_mut();
                make.at(ctx.tree(tree).pos);
                let syms = self.syms.borrow_mut();
                make.var_def_symbol(ctx, &syms, current_value, Some(translated_expr))
            };
            result_expression = {
                let mut make = self.make.borrow_mut();
                make.at(ctx.tree(tree).pos);
                make.let_expr(ctx, List::of(var_def), result_expression)
            };
            if let TreeKind::LetExpr { needs_cond: slot, .. } = &mut ctx.tree_mut(result_expression).kind {
                *slot = true;
            }
            let syms = self.syms.borrow();
            ctx.tree_mut(result_expression).ty = Some(syms.boolean_type);
        }
        let result_expression = self.decorate_expression(ctx, result_expression);
        self.result = Some(result_expression);
        self.current_value = prev_current_value;
        let popped = self.take_binding_context().pop();
        self.binding_context = popped;
    }
    fn visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let var = match &ctx.tree(tree).kind {
            TreeKind::BindingPattern { var: Some(var) } => *var,
            _ => panic!("ClassCastException: JCBindingPattern expected"),
        };
        let binding = match &ctx.tree(var).kind {
            TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("binding pattern variable symbol"),
        };
        let cast_target_type = self.principal_type(ctx, tree);
        let binding_var = self
            .binding_context
            .binding_declared(ctx, binding, self.current_method_sym);
        let result = if let Some(binding_var) = binding_var {
            let pos = ctx.tree(tree).pos;
            let current_value = self.current_value.expect("current value");
            let lhs = {
                let mut make = self.make.borrow_mut();
                make.at(pos);
                make.ident_symbol(ctx, binding_var)
            };
            let current = {
                let mut make = self.make.borrow_mut();
                make.at(pos);
                make.ident_symbol(ctx, current_value)
            };
            let converted = self.convert(ctx, current, cast_target_type);
            let fake_init = {
                let mut make = self.make.borrow_mut();
                make.at(pos);
                make.assign(ctx, lhs, converted)
            };
            let erased = {
                let syms = self.syms.borrow();
                let ty = ctx.sym(binding_var).ty.expect("binding variable type");
                self.types.erasure(ctx, &syms, ty)
            };
            ctx.tree_mut(fake_init).ty = Some(erased);
            let exec = {
                let mut make = self.make.borrow_mut();
                make.exec(ctx, fake_init)
            };
            let literal_true = {
                let mut make = self.make.borrow_mut();
                make.literal_object(ctx, crate::tree::jc_tree::LiteralValue::Boolean(true))
            };
            let nested = {
                let mut make = self.make.borrow_mut();
                make.at(pos);
                make.let_expr(ctx, List::of(exec), literal_true)
            };
            if let TreeKind::LetExpr { needs_cond: slot, .. } = &mut ctx.tree_mut(nested).kind {
                *slot = true;
            }
            let syms = self.syms.borrow();
            ctx.tree_mut(nested).ty = Some(syms.boolean_type);
            nested
        } else {
            let mut make = self.make.borrow_mut();
            make.literal_object(ctx, crate::tree::jc_tree::LiteralValue::Boolean(true))
        };
        self.result = Some(result);
    }
    fn visit_default_case_label(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    fn visit_parenthesized_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let pattern = match &ctx.tree(tree).kind {
            TreeKind::ParenthesizedPattern { pattern: Some(pattern) } => *pattern,
            _ => panic!("ClassCastException: JCParenthesizedPattern expected"),
        };
        self.result = self.translate(ctx, Some(pattern));
    }
    fn visit_guard_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (patt, expr) = match &ctx.tree(tree).kind {
            TreeKind::GuardPattern { patt: Some(patt), expr: Some(expr) } => (*patt, *expr),
            _ => panic!("ClassCastException: JCGuardPattern expected"),
        };
        let pattern = self.translate(ctx, Some(patt)).expect("translated guard pattern");
        let guard = self.translate(ctx, Some(expr)).expect("translated guard");
        let result = self.make_binary(ctx, Tag::AND, pattern, guard);
        self.result = Some(result);
    }
    fn visit_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (indexed, index) = match &ctx.tree(tree).kind {
            TreeKind::ArrayAccess { indexed, index } => (*indexed, *index),
            _ => panic!("expected an array access"),
        };
        let indexed = self.translate(ctx, indexed);
        let index = self.translate(ctx, index);
        if let TreeKind::ArrayAccess {
            indexed: i,
            index: n,
        } = &mut ctx.tree_mut(tree).kind
        {
            *i = indexed;
            *n = index;
        }
        self.result = Some(tree);
    }

    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let selected = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { selected, .. } => *selected,
            _ => panic!("expected a field access"),
        };
        let selected = self.translate(ctx, selected);
        if let TreeKind::FieldAccess { selected: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = selected;
        }
        self.result = Some(tree);
    }

    fn visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { expr, .. } => *expr,
            _ => panic!("expected a member reference"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::MemberReference { expr: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }

    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sym = match &ctx.tree(tree).kind {
            TreeKind::Ident { sym: Some(sym), .. } => *sym,
            _ => {
                self.default_visit_ident(ctx, tree);
                return;
            }
        };
        let mut binding_var = None;
        if (ctx.sym(sym).flags_field & flags::MATCH_BINDING) != 0 {
            binding_var = self.binding_context.get_binding_for(ctx, sym);
        }
        if let Some(binding_var) = binding_var {
            let result = {
                let mut make = self.make.borrow_mut();
                make.at(ctx.tree(tree).pos);
                make.ident_symbol(ctx, binding_var)
            };
            self.result = Some(result);
        } else {
            self.default_visit_ident(ctx, tree);
        }
    }
    fn visit_literal(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    fn visit_type_ident(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    fn visit_type_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let elemtype = match &ctx.tree(tree).kind {
            TreeKind::ArrayTypeTree { elemtype } => *elemtype,
            _ => panic!("expected an array type"),
        };
        let elemtype = self.translate(ctx, elemtype);
        if let TreeKind::ArrayTypeTree { elemtype: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = elemtype;
        }
        self.result = Some(tree);
    }

    fn visit_type_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, arguments) = match &ctx.tree(tree).kind {
            TreeKind::TypeApply { clazz, arguments } => (*clazz, arguments.clone()),
            _ => panic!("expected a parameterized type"),
        };
        let clazz = self.translate(ctx, clazz);
        let arguments = self.translate_list(ctx, &arguments);
        if let TreeKind::TypeApply {
            clazz: c,
            arguments: a,
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = clazz;
            *a = arguments;
        }
        self.result = Some(tree);
    }

    fn visit_type_union(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let alternatives = match &ctx.tree(tree).kind {
            TreeKind::TypeUnion { alternatives } => alternatives.clone(),
            _ => panic!("expected a union type"),
        };
        let alternatives = self.translate_list(ctx, &alternatives);
        if let TreeKind::TypeUnion { alternatives: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = alternatives;
        }
        self.result = Some(tree);
    }

    fn visit_type_intersection(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let bounds = match &ctx.tree(tree).kind {
            TreeKind::TypeIntersection { bounds } => bounds.clone(),
            _ => panic!("expected an intersection type"),
        };
        let bounds = self.translate_list(ctx, &bounds);
        if let TreeKind::TypeIntersection { bounds: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = bounds;
        }
        self.result = Some(tree);
    }

    fn visit_type_parameter(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, bounds) = match &ctx.tree(tree).kind {
            TreeKind::TypeParameter {
                annotations,
                bounds,
                ..
            } => (annotations.clone(), bounds.clone()),
            _ => panic!("expected a type parameter"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        let bounds = self.translate_list(ctx, &bounds);
        if let TreeKind::TypeParameter {
            annotations: a,
            bounds: b,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *b = bounds;
        }
        self.result = Some(tree);
    }

    fn visit_wildcard(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (kind, inner) = match &ctx.tree(tree).kind {
            TreeKind::Wildcard { kind, inner } => (*kind, *inner),
            _ => panic!("expected a wildcard"),
        };
        let kind = self.translate(ctx, kind);
        let inner = self.translate(ctx, inner);
        if let TreeKind::Wildcard { kind: k, inner: i } = &mut ctx.tree_mut(tree).kind {
            *k = kind;
            *i = inner;
        }
        self.result = Some(tree);
    }

    fn visit_type_bound_kind(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    fn visit_modifiers(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let annotations = match &ctx.tree(tree).kind {
            TreeKind::Modifiers { annotations, .. } => annotations.clone(),
            _ => panic!("expected modifiers"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        if let TreeKind::Modifiers {
            annotations: slot, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *slot = annotations;
        }
        self.result = Some(tree);
    }

    fn visit_annotation(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotation_type, args) = match &ctx.tree(tree).kind {
            TreeKind::Annotation {
                annotation_type,
                args,
                ..
            } => (*annotation_type, args.clone()),
            _ => panic!("expected an annotation"),
        };
        let annotation_type = self.translate(ctx, annotation_type);
        let args = self.translate_list(ctx, &args);
        if let TreeKind::Annotation {
            annotation_type: a,
            args: ar,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotation_type;
            *ar = args;
        }
        self.result = Some(tree);
    }

    fn visit_annotated_type(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, underlying_type) = match &ctx.tree(tree).kind {
            TreeKind::AnnotatedType {
                annotations,
                underlying_type,
            } => (annotations.clone(), *underlying_type),
            _ => panic!("expected an annotated type"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        let underlying_type = self.translate(ctx, underlying_type);
        if let TreeKind::AnnotatedType {
            annotations: a,
            underlying_type: u,
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *u = underlying_type;
        }
        self.result = Some(tree);
    }

    fn visit_erroneous(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    fn visit_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (defs, expr) = match &ctx.tree(tree).kind {
            TreeKind::LetExpr { defs, expr, .. } => (defs.clone(), *expr),
            _ => panic!("expected a let expression"),
        };
        let defs = self.translate_list(ctx, &defs);
        let expr = self.translate(ctx, expr);
        if let TreeKind::LetExpr {
            defs: d, expr: e, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *d = defs;
            *e = expr;
        }
        self.result = Some(tree);
    }

    fn visit_module_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, qual_id, directives) = match &ctx.tree(tree).kind {
            TreeKind::ModuleDecl {
                mods,
                qual_id,
                directives,
                ..
            } => (*mods, *qual_id, directives.clone()),
            _ => panic!("expected a module declaration"),
        };
        let mods = self.translate(ctx, mods);
        let qual_id = self.translate(ctx, qual_id);
        let directives = directives.map(|directives| self.translate_list(ctx, &directives));
        if let TreeKind::ModuleDecl {
            mods: m,
            qual_id: q,
            directives: d,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *q = qual_id;
            *d = directives;
        }
        self.result = Some(tree);
    }

    fn visit_exports(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (qualid, module_names) = match &ctx.tree(tree).kind {
            TreeKind::Exports {
                qualid,
                module_names,
                ..
            } => (*qualid, module_names.clone()),
            _ => panic!("expected an exports directive"),
        };
        let qualid = self.translate(ctx, qualid);
        let module_names = module_names.map(|module_names| self.translate_list(ctx, &module_names));
        if let TreeKind::Exports {
            qualid: q,
            module_names: m,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *q = qualid;
            *m = module_names;
        }
        self.result = Some(tree);
    }

    fn visit_opens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (qualid, module_names) = match &ctx.tree(tree).kind {
            TreeKind::Opens {
                qualid,
                module_names,
                ..
            } => (*qualid, module_names.clone()),
            _ => panic!("expected an opens directive"),
        };
        let qualid = self.translate(ctx, qualid);
        let module_names = module_names.map(|module_names| self.translate_list(ctx, &module_names));
        if let TreeKind::Opens {
            qualid: q,
            module_names: m,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *q = qualid;
            *m = module_names;
        }
        self.result = Some(tree);
    }

    fn visit_provides(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (service_name, impl_names) = match &ctx.tree(tree).kind {
            TreeKind::Provides {
                service_name,
                impl_names,
                ..
            } => (*service_name, impl_names.clone()),
            _ => panic!("expected a provides directive"),
        };
        let service_name = self.translate(ctx, service_name);
        let impl_names = self.translate_list(ctx, &impl_names);
        if let TreeKind::Provides {
            service_name: s,
            impl_names: i,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = service_name;
            *i = impl_names;
        }
        self.result = Some(tree);
    }

    fn visit_requires(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let module_name = match &ctx.tree(tree).kind {
            TreeKind::Requires { module_name, .. } => *module_name,
            _ => panic!("expected a requires directive"),
        };
        let module_name = self.translate(ctx, module_name);
        if let TreeKind::Requires {
            module_name: slot, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *slot = module_name;
        }
        self.result = Some(tree);
    }

    fn visit_uses(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Uses { qualid } => *qualid,
            _ => panic!("expected a uses directive"),
        };
        let qualid = self.translate(ctx, qualid);
        if let TreeKind::Uses { qualid: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = qualid;
        }
        self.result = Some(tree);
    }

    fn visit_tree(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        panic!("AssertionError: visitTree {:?}", tree);
    }
}

fn new_var_symbol(
    ctx: &mut CompilerContext,
    flags_field: i64,
    name: crate::util::name::Name,
    ty: TypeId,
    owner: Option<SymbolId>,
) -> SymbolId {
    ctx.alloc_symbol(Symbol::new(
        crate::code::kinds::Kind::VAR,
        flags_field,
        name,
        Some(ty),
        owner,
        SymbolKind::VarSymbol {
            lazy_const_value: None,
            pos: 0,
            adr: 0,
            data_kind: None,
            data: None,
        },
    ))
}

fn case_stats(ctx: &CompilerContext, case: TreeId) -> List<TreeId> {
    match &ctx.tree(case).kind {
        TreeKind::Case { stats, .. } => stats.clone(),
        _ => panic!("ClassCastException: JCCase expected"),
    }
}

fn set_case_stats(ctx: &mut CompilerContext, case: TreeId, stats: List<TreeId>) {
    if let TreeKind::Case { stats: slot, .. } = &mut ctx.tree_mut(case).kind {
        *slot = stats;
    }
}

fn case_labels(ctx: &CompilerContext, case: TreeId) -> List<TreeId> {
    match &ctx.tree(case).kind {
        TreeKind::Case { labels, .. } => labels.clone(),
        _ => panic!("ClassCastException: JCCase expected"),
    }
}

fn set_case_labels(ctx: &mut CompilerContext, case: TreeId, labels: List<TreeId>) {
    if let TreeKind::Case { labels: slot, .. } = &mut ctx.tree_mut(case).kind {
        *slot = labels;
    }
}

fn case_kind(ctx: &CompilerContext, case: TreeId) -> CaseKind {
    match &ctx.tree(case).kind {
        TreeKind::Case { case_kind, .. } => *case_kind,
        _ => panic!("ClassCastException: JCCase expected"),
    }
}

fn case_completes_normally(ctx: &CompilerContext, case: TreeId) -> bool {
    match &ctx.tree(case).kind {
        TreeKind::Case { completes_normally, .. } => *completes_normally,
        _ => panic!("ClassCastException: JCCase expected"),
    }
}

fn is_pattern_label(ctx: &CompilerContext, label: TreeId) -> bool {
    matches!(
        ctx.tree(label).tag,
        Tag::BINDINGPATTERN | Tag::PARENTHESIZEDPATTERN | Tag::GUARDPATTERN
    )
}

fn is_expression_label(ctx: &CompilerContext, label: TreeId) -> bool {
    !is_pattern_label(ctx, label) && !ctx.tree(label).has_tag(Tag::DEFAULTCASELABEL)
}

fn is_null_pattern(ctx: &CompilerContext, label: TreeId) -> bool {
    is_expression_label(ctx, label) && tree_info::is_null(ctx, label)
}

fn literal_value_constant(value: &crate::tree::jc_tree::LiteralValue) -> crate::util::constants::ConstantValue {
    use crate::tree::jc_tree::LiteralValue;
    use crate::util::constants::ConstantValue;
    match value {
        LiteralValue::Integer(value) => ConstantValue::Integer(*value),
        LiteralValue::Byte(value) => ConstantValue::Byte(*value),
        LiteralValue::Short(value) => ConstantValue::Short(*value),
        LiteralValue::Long(value) => ConstantValue::Long(*value),
        LiteralValue::Float(value) => ConstantValue::Float(*value),
        LiteralValue::Double(value) => ConstantValue::Double(*value),
        LiteralValue::Character(value) => ConstantValue::Character(*value),
        LiteralValue::Boolean(value) => ConstantValue::Boolean(*value),
        LiteralValue::String(value) => ConstantValue::String(value.clone()),
    }
}
