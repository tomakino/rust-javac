//! A visitor that compares two lambda bodies for structural equality
//! (TreeDiffer.java).
//!
//! TreeDiffer extends TreeScanner; the port is a struct that
//! implements TreeVisitor. The two-argument scan method is the upstream
//! context-passing comparison (TreeDiffer.java); node kinds the upstream
//! class does not override keep a no-op implementation, which is behaviourally
//! equivalent because the default TreeScanner navigation never sets result.

// Port note: the upstream comparison keeps nested conditionals; the style lint
// is suppressed to preserve that control-flow shape.
#![allow(clippy::collapsible_if)]

use std::collections::HashMap;

use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId};
use crate::tree::jc_tree::{TreeKind, TreeVisitor};
use crate::tree::tree_info;
use crate::util::list::List;

/// A visitor that compares two lambda bodies for structural equality.
pub struct TreeDiffer {
    equivalent: HashMap<SymbolId, SymbolId>,
    parameter: Option<TreeId>,
    result: bool,
}

#[allow(clippy::collapsible_if)]
impl TreeDiffer {
    /// Creates a differ over two captured symbol lists.
    ///
    /// # Arguments
    /// - `symbols` — the symbols captured by the first lambda
    /// - `other_symbols` — the symbols captured by the second lambda
    pub fn new(symbols: &[SymbolId], other_symbols: &[SymbolId]) -> TreeDiffer {
        TreeDiffer {
            equivalent: TreeDiffer::equiv(symbols, other_symbols),
            parameter: None,
            result: false,
        }
    }

    fn equiv(symbols: &[SymbolId], other_symbols: &[SymbolId]) -> HashMap<SymbolId, SymbolId> {
        let mut result = HashMap::new();
        let mut other = other_symbols.iter();
        for symbol in symbols {
            let Some(next) = other.next() else {
                break;
            };
            result.insert(*symbol, *next);
        }
        result
    }

    /// Compares two trees for structural equality.
    ///
    /// # Arguments
    /// - `ctx` — the compiler context
    /// - `tree` — the first tree
    /// - `other` — the second tree
    ///
    /// # Returns
    /// `true` when the trees are structurally equivalent
    pub fn scan(
        &mut self,
        ctx: &mut CompilerContext,
        tree: Option<TreeId>,
        parameter: Option<TreeId>,
    ) -> bool {
        let (Some(tree), Some(parameter)) = (tree, parameter) else {
            return tree.is_none() && parameter.is_none();
        };
        let tree = tree_info::skip_parens(ctx, tree);
        let parameter = tree_info::skip_parens(ctx, parameter);
        let tree_constant = ctx
            .tree(tree)
            .ty
            .and_then(|ty| ctx.typ(ty).const_value().cloned());
        let parameter_constant = ctx
            .tree(parameter)
            .ty
            .and_then(|ty| ctx.typ(ty).const_value().cloned());
        if tree_constant.is_some() && parameter_constant.is_some() {
            return tree_constant == parameter_constant;
        }
        if ctx.tree(tree).tag != ctx.tree(parameter).tag {
            return false;
        }
        let previous_parameter = self.parameter;
        let previous_result = self.result;
        self.parameter = Some(parameter);
        self.result = false;
        crate::tree::jc_tree::accept(ctx, tree, self);
        let result = self.result;
        self.parameter = previous_parameter;
        self.result = previous_result;
        result
    }

    fn scan_list_pair(
        &mut self,
        ctx: &mut CompilerContext,
        xs: &List<TreeId>,
        ys: &List<TreeId>,
    ) -> bool {
        if xs.length() != ys.length() {
            return false;
        }
        let mut x = xs.iter();
        let mut y = ys.iter();
        loop {
            match (x.next(), y.next()) {
                (None, None) => return true,
                (Some(x), Some(y)) => {
                    if !self.scan(ctx, Some(x), Some(y)) {
                        return false;
                    }
                }
                _ => return false,
            }
        }
    }

    fn scan_opt_list_pair(
        &mut self,
        ctx: &mut CompilerContext,
        xs: Option<&List<TreeId>>,
        ys: Option<&List<TreeId>>,
    ) -> bool {
        match (xs, ys) {
            (None, None) => true,
            (Some(xs), Some(ys)) => self.scan_list_pair(ctx, xs, ys),
            _ => false,
        }
    }

    fn scan_dim_annotations(
        &mut self,
        ctx: &mut CompilerContext,
        xs: &List<List<TreeId>>,
        ys: &List<List<TreeId>>,
    ) -> bool {
        if xs.length() != ys.length() {
            return false;
        }
        let mut x = xs.iter();
        let mut y = ys.iter();
        loop {
            match (x.next(), y.next()) {
                (None, None) => return true,
                (Some(x), Some(y)) => {
                    if !self.scan_list_pair(ctx, &x, &y) {
                        return false;
                    }
                }
                _ => return false,
            }
        }
    }
}

impl TreeVisitor for TreeDiffer {
    fn visit_tree(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {
        panic!("AssertionError: visitTree");
    }

    fn visit_top_level(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::CompilationUnit {
            defs,
            modle,
            packge,
            ..
        } = &tree_kind
        else {
            panic!("expected a compilation unit");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::CompilationUnit {
            defs: other_defs,
            modle: other_modle,
            packge: other_packge,
            ..
        } = &that_kind
        else {
            panic!("expected a compilation unit");
        };
        self.result = self.scan_list_pair(ctx, defs, other_defs)
            && *modle == *other_modle
            && *packge == *other_packge;
    }

    fn visit_package_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::PackageDecl {
            annotations,
            pid,
            packge,
        } = &tree_kind
        else {
            panic!("expected a package declaration");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::PackageDecl {
            annotations: other_annotations,
            pid: other_pid,
            packge: other_packge,
        } = &that_kind
        else {
            panic!("expected a package declaration");
        };
        self.result = self.scan_list_pair(ctx, annotations, other_annotations)
            && self.scan(ctx, *pid, *other_pid)
            && *packge == *other_packge;
    }

    fn visit_import(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Import {
            static_import,
            qualid,
            ..
        } = &tree_kind
        else {
            panic!("expected an import");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Import {
            static_import: other_static,
            qualid: other_qualid,
            ..
        } = &that_kind
        else {
            panic!("expected an import");
        };
        self.result = *static_import == *other_static && self.scan(ctx, *qualid, *other_qualid);
    }

    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::ClassDecl {
            mods,
            name,
            typarams,
            extending,
            implementing,
            defs,
            ..
        } = &tree_kind
        else {
            panic!("expected a class declaration");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::ClassDecl {
            mods: other_mods,
            name: other_name,
            typarams: other_typarams,
            extending: other_extending,
            implementing: other_implementing,
            defs: other_defs,
            ..
        } = &that_kind
        else {
            panic!("expected a class declaration");
        };
        self.result = self.scan(ctx, *mods, *other_mods)
            && name == other_name
            && self.scan_list_pair(ctx, typarams, other_typarams)
            && self.scan(ctx, *extending, *other_extending)
            && self.scan_list_pair(ctx, implementing, other_implementing)
            && self.scan_list_pair(ctx, defs, other_defs);
    }

    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::MethodDecl {
            mods,
            name,
            restype,
            typarams,
            recvparam,
            params,
            thrown,
            body,
            default_value,
            ..
        } = &tree_kind
        else {
            panic!("expected a method declaration");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::MethodDecl {
            mods: other_mods,
            name: other_name,
            restype: other_restype,
            typarams: other_typarams,
            recvparam: other_recvparam,
            params: other_params,
            thrown: other_thrown,
            body: other_body,
            default_value: other_default,
            ..
        } = &that_kind
        else {
            panic!("expected a method declaration");
        };
        self.result = self.scan(ctx, *mods, *other_mods)
            && name == other_name
            && self.scan(ctx, *restype, *other_restype)
            && self.scan_list_pair(ctx, typarams, other_typarams)
            && self.scan(ctx, *recvparam, *other_recvparam)
            && self.scan_list_pair(ctx, params, other_params)
            && self.scan_list_pair(ctx, thrown, other_thrown)
            && self.scan(ctx, *body, *other_body)
            && self.scan(ctx, *default_value, *other_default);
    }

    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::VariableDecl {
            mods,
            name,
            nameexpr,
            vartype,
            init,
            sym,
            ..
        } = &tree_kind
        else {
            panic!("expected a variable declaration");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::VariableDecl {
            mods: other_mods,
            name: other_name,
            nameexpr: other_nameexpr,
            vartype: other_vartype,
            init: other_init,
            sym: other_sym,
            ..
        } = &that_kind
        else {
            panic!("expected a variable declaration");
        };
        self.result = self.scan(ctx, *mods, *other_mods)
            && *name == other_name.clone()
            && self.scan(ctx, *nameexpr, *other_nameexpr)
            && self.scan(ctx, *vartype, *other_vartype)
            && self.scan(ctx, *init, *other_init);
        if !self.result {
            return;
        }
        if let (Some(sym), Some(other_sym)) = (sym, *other_sym) {
            self.equivalent.insert(*sym, other_sym);
        }
    }

    fn visit_skip(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Block { flags, stats, .. } = &tree_kind else {
            panic!("expected a block");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Block {
            flags: other_flags,
            stats: other_stats,
            ..
        } = &that_kind
        else {
            panic!("expected a block");
        };
        self.result = *flags == *other_flags && self.scan_list_pair(ctx, stats, other_stats);
    }

    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::DoWhileLoop { body, cond } = &tree_kind else {
            panic!("expected a do loop");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::DoWhileLoop {
            body: other_body,
            cond: other_cond,
        } = &that_kind
        else {
            panic!("expected a do loop");
        };
        self.result = self.scan(ctx, *body, *other_body) && self.scan(ctx, *cond, *other_cond);
    }

    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::WhileLoop { cond, body } = &tree_kind else {
            panic!("expected a while loop");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::WhileLoop {
            cond: other_cond,
            body: other_body,
        } = &that_kind
        else {
            panic!("expected a while loop");
        };
        self.result = self.scan(ctx, *cond, *other_cond) && self.scan(ctx, *body, *other_body);
    }

    fn visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::ForLoop {
            init,
            cond,
            step,
            body,
        } = &tree_kind
        else {
            panic!("expected a for loop");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::ForLoop {
            init: other_init,
            cond: other_cond,
            step: other_step,
            body: other_body,
        } = &that_kind
        else {
            panic!("expected a for loop");
        };
        self.result = self.scan_list_pair(ctx, init, other_init)
            && self.scan(ctx, *cond, *other_cond)
            && self.scan_list_pair(ctx, step, other_step)
            && self.scan(ctx, *body, *other_body);
    }

    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::EnhancedForLoop { var, expr, body } = &tree_kind else {
            panic!("expected an enhanced for loop");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::EnhancedForLoop {
            var: other_var,
            expr: other_expr,
            body: other_body,
        } = &that_kind
        else {
            panic!("expected an enhanced for loop");
        };
        self.result = self.scan(ctx, *var, *other_var)
            && self.scan(ctx, *expr, *other_expr)
            && self.scan(ctx, *body, *other_body);
    }

    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::LabeledStatement { label, body } = &tree_kind else {
            panic!("expected a labelled statement");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::LabeledStatement {
            label: other_label,
            body: other_body,
        } = &that_kind
        else {
            panic!("expected a labelled statement");
        };
        self.result = *label == other_label.clone() && self.scan(ctx, *body, *other_body);
    }

    fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Switch {
            selector, cases, ..
        } = &tree_kind
        else {
            panic!("expected a switch");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Switch {
            selector: other_selector,
            cases: other_cases,
            ..
        } = &that_kind
        else {
            panic!("expected a switch");
        };
        self.result = self.scan(ctx, *selector, *other_selector)
            && self.scan_list_pair(ctx, cases, other_cases);
    }

    fn visit_case(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Case { labels, stats, .. } = &tree_kind else {
            panic!("expected a case");
        };
        let labels = labels.clone();
        let stats = stats.clone();
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Case {
            labels: other_labels,
            stats: other_stats,
            ..
        } = &that_kind
        else {
            panic!("expected a case");
        };
        self.result = self.scan_list_pair(ctx, &labels, other_labels)
            && self.scan_list_pair(ctx, &stats, other_stats);
    }

    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::SwitchExpression {
            selector, cases, ..
        } = &tree_kind
        else {
            panic!("expected a switch expression");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::SwitchExpression {
            selector: other_selector,
            cases: other_cases,
            ..
        } = &that_kind
        else {
            panic!("expected a switch expression");
        };
        self.result = self.scan(ctx, *selector, *other_selector)
            && self.scan_list_pair(ctx, cases, other_cases);
    }

    fn visit_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Synchronized { lock, body } = &tree_kind else {
            panic!("expected a synchronized statement");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Synchronized {
            lock: other_lock,
            body: other_body,
        } = &that_kind
        else {
            panic!("expected a synchronized statement");
        };
        self.result = self.scan(ctx, *lock, *other_lock) && self.scan(ctx, *body, *other_body);
    }

    fn visit_try(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Try {
            resources,
            body,
            catchers,
            finalizer,
            ..
        } = &tree_kind
        else {
            panic!("expected a try statement");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Try {
            resources: other_resources,
            body: other_body,
            catchers: other_catchers,
            finalizer: other_finalizer,
            ..
        } = &that_kind
        else {
            panic!("expected a try statement");
        };
        self.result = self.scan_list_pair(ctx, resources, other_resources)
            && self.scan(ctx, *body, *other_body)
            && self.scan_list_pair(ctx, catchers, other_catchers)
            && self.scan(ctx, *finalizer, *other_finalizer);
    }

    fn visit_catch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Catch { param, body } = &tree_kind else {
            panic!("expected a catch clause");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Catch {
            param: other_param,
            body: other_body,
        } = &that_kind
        else {
            panic!("expected a catch clause");
        };
        self.result = self.scan(ctx, *param, *other_param) && self.scan(ctx, *body, *other_body);
    }

    fn visit_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Conditional {
            cond,
            truepart,
            falsepart,
            ..
        } = &tree_kind
        else {
            panic!("expected a conditional");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Conditional {
            cond: other_cond,
            truepart: other_true,
            falsepart: other_false,
            ..
        } = &that_kind
        else {
            panic!("expected a conditional");
        };
        self.result = self.scan(ctx, *cond, *other_cond)
            && self.scan(ctx, *truepart, *other_true)
            && self.scan(ctx, *falsepart, *other_false);
    }

    fn visit_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::If {
            cond,
            thenpart,
            elsepart,
        } = &tree_kind
        else {
            panic!("expected an if statement");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::If {
            cond: other_cond,
            thenpart: other_then,
            elsepart: other_else,
        } = &that_kind
        else {
            panic!("expected an if statement");
        };
        self.result = self.scan(ctx, *cond, *other_cond)
            && self.scan(ctx, *thenpart, *other_then)
            && self.scan(ctx, *elsepart, *other_else);
    }

    fn visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::ExpressionStatement { expr } = &tree_kind else {
            panic!("expected an expression statement");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::ExpressionStatement { expr: other } = &that_kind else {
            panic!("expected an expression statement");
        };
        self.result = self.scan(ctx, *expr, *other);
    }

    fn visit_break(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Break { label, .. } = &tree_kind else {
            panic!("expected a break");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Break { label: other, .. } = &that_kind else {
            panic!("expected a break");
        };
        self.result = label == other;
    }

    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Yield { value, .. } = &tree_kind else {
            panic!("expected a yield");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Yield { value: other, .. } = &that_kind else {
            panic!("expected a yield");
        };
        self.result = self.scan(ctx, *value, *other);
    }

    fn visit_continue(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Continue { label, .. } = &tree_kind else {
            panic!("expected a continue");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Continue { label: other, .. } = &that_kind else {
            panic!("expected a continue");
        };
        self.result = label == other;
    }

    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Return { expr } = &tree_kind else {
            panic!("expected a return");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Return { expr: other } = &that_kind else {
            panic!("expected a return");
        };
        self.result = self.scan(ctx, *expr, *other);
    }

    fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Throw { expr } = &tree_kind else {
            panic!("expected a throw");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Throw { expr: other } = &that_kind else {
            panic!("expected a throw");
        };
        self.result = self.scan(ctx, *expr, *other);
    }

    fn visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Assert { cond, detail } = &tree_kind else {
            panic!("expected an assert");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Assert {
            cond: other_cond,
            detail: other_detail,
        } = &that_kind
        else {
            panic!("expected an assert");
        };
        self.result = self.scan(ctx, *cond, *other_cond) && self.scan(ctx, *detail, *other_detail);
    }

    fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::MethodInvocation {
            typeargs,
            meth,
            args,
            poly_kind,
            ..
        } = &tree_kind
        else {
            panic!("expected a method invocation");
        };
        let typeargs = typeargs.clone();
        let meth = *meth;
        let args = args.clone();
        let poly = *poly_kind;
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::MethodInvocation {
            typeargs: other_typeargs,
            meth: other_meth,
            args: other_args,
            poly_kind: other_poly,
            ..
        } = &that_kind
        else {
            panic!("expected a method invocation");
        };
        self.result = self.scan_list_pair(ctx, &typeargs, other_typeargs)
            && self.scan(ctx, meth, *other_meth)
            && self.scan_list_pair(ctx, &args, other_args)
            && poly == *other_poly;
    }

    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::NewClass {
            encl,
            typeargs,
            clazz,
            args,
            def,
            constructor,
            ..
        } = &tree_kind
        else {
            panic!("expected a new class");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::NewClass {
            encl: other_encl,
            typeargs: other_typeargs,
            clazz: other_clazz,
            args: other_args,
            def: other_def,
            constructor: other_constructor,
            ..
        } = &that_kind
        else {
            panic!("expected a new class");
        };
        self.result = self.scan(ctx, *encl, *other_encl)
            && self.scan_list_pair(ctx, typeargs, other_typeargs)
            && self.scan(ctx, *clazz, *other_clazz)
            && self.scan_list_pair(ctx, args, other_args)
            && self.scan(ctx, *def, *other_def)
            && *constructor == *other_constructor;
    }

    fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::NewArray {
            annotations,
            elemtype,
            dims,
            dim_annotations,
            elems,
        } = &tree_kind
        else {
            panic!("expected a new array");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::NewArray {
            annotations: other_annotations,
            elemtype: other_elemtype,
            dims: other_dims,
            dim_annotations: other_dim_annotations,
            elems: other_elems,
        } = &that_kind
        else {
            panic!("expected a new array");
        };
        self.result = self.scan_list_pair(ctx, annotations, other_annotations)
            && self.scan(ctx, *elemtype, *other_elemtype)
            && self.scan_list_pair(ctx, dims, other_dims)
            && self.scan_dim_annotations(ctx, dim_annotations, other_dim_annotations)
            && self.scan_opt_list_pair(ctx, elems.as_ref(), other_elems.as_ref());
    }

    fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Lambda {
            params,
            body,
            param_kind,
            ..
        } = &tree_kind
        else {
            panic!("expected a lambda");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Lambda {
            params: other_params,
            body: other_body,
            param_kind: other_kind,
            ..
        } = &that_kind
        else {
            panic!("expected a lambda");
        };
        self.result = self.scan_list_pair(ctx, params, other_params)
            && self.scan(ctx, *body, *other_body)
            && *param_kind == *other_kind;
    }

    fn visit_parens(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Assign { lhs, rhs } = &tree_kind else {
            panic!("expected an assignment");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Assign {
            lhs: other_lhs,
            rhs: other_rhs,
        } = &that_kind
        else {
            panic!("expected an assignment");
        };
        self.result = self.scan(ctx, *lhs, *other_lhs) && self.scan(ctx, *rhs, *other_rhs);
    }

    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::AssignOp { lhs, rhs, operator } = &tree_kind else {
            panic!("expected a compound assignment");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::AssignOp {
            lhs: other_lhs,
            rhs: other_rhs,
            operator: other_operator,
        } = &that_kind
        else {
            panic!("expected a compound assignment");
        };
        self.result = self.scan(ctx, *lhs, *other_lhs)
            && self.scan(ctx, *rhs, *other_rhs)
            && *operator == *other_operator;
    }

    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Unary { arg, operator } = &tree_kind else {
            panic!("expected a unary expression");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Unary {
            arg: other_arg,
            operator: other_operator,
        } = &that_kind
        else {
            panic!("expected a unary expression");
        };
        self.result = self.scan(ctx, *arg, *other_arg) && *operator == *other_operator;
    }

    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Binary { lhs, rhs, operator } = &tree_kind else {
            panic!("expected a binary expression");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Binary {
            lhs: other_lhs,
            rhs: other_rhs,
            operator: other_operator,
        } = &that_kind
        else {
            panic!("expected a binary expression");
        };
        self.result = self.scan(ctx, *lhs, *other_lhs)
            && self.scan(ctx, *rhs, *other_rhs)
            && *operator == *other_operator;
    }

    fn visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::TypeCast { clazz, expr } = &tree_kind else {
            panic!("expected a type cast");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::TypeCast {
            clazz: other_clazz,
            expr: other_expr,
        } = &that_kind
        else {
            panic!("expected a type cast");
        };
        self.result = self.scan(ctx, *clazz, *other_clazz) && self.scan(ctx, *expr, *other_expr);
    }

    fn visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::InstanceOf { expr, pattern } = &tree_kind else {
            panic!("expected an instanceof");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::InstanceOf {
            expr: other_expr,
            pattern: other_pattern,
        } = &that_kind
        else {
            panic!("expected an instanceof");
        };
        self.result =
            self.scan(ctx, *expr, *other_expr) && self.scan(ctx, *pattern, *other_pattern);
    }

    fn visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::BindingPattern { var } = &tree_kind else {
            panic!("expected a binding pattern");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::BindingPattern { var: other } = &that_kind else {
            panic!("expected a binding pattern");
        };
        self.result = self.scan(ctx, *var, *other);
    }

    fn visit_default_case_label(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {
        self.result = true;
    }

    fn visit_parenthesized_pattern(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_guard_pattern(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::ArrayAccess { indexed, index } = &tree_kind else {
            panic!("expected an array access");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::ArrayAccess {
            indexed: other_indexed,
            index: other_index,
        } = &that_kind
        else {
            panic!("expected an array access");
        };
        self.result =
            self.scan(ctx, *indexed, *other_indexed) && self.scan(ctx, *index, *other_index);
    }

    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::FieldAccess { selected, sym, .. } = &tree_kind else {
            panic!("expected a field access");
        };
        let selected = *selected;
        let symbol = *sym;
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::FieldAccess {
            selected: other_selected,
            sym: other_sym,
            ..
        } = &that_kind
        else {
            panic!("expected a field access");
        };
        self.result = self.scan(ctx, selected, *other_selected) && symbol == *other_sym;
    }

    fn visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::MemberReference {
            mode,
            kind,
            name,
            expr,
            typeargs,
            ..
        } = &tree_kind
        else {
            panic!("expected a member reference");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::MemberReference {
            mode: other_mode,
            kind: other_kind,
            name: other_name,
            expr: other_expr,
            typeargs: other_typeargs,
            ..
        } = &that_kind
        else {
            panic!("expected a member reference");
        };
        self.result = *mode == *other_mode
            && kind == other_kind
            && *name == other_name.clone()
            && self.scan(ctx, *expr, *other_expr)
            && self.scan_opt_list_pair(ctx, typeargs.as_ref(), other_typeargs.as_ref());
    }

    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Ident { sym, .. } = &tree_kind else {
            panic!("expected an identifier");
        };
        let symbol = *sym;
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Ident { sym: other, .. } = &that_kind else {
            panic!("expected an identifier");
        };
        let other_symbol = *other;
        if let (Some(symbol), Some(other_symbol)) = (symbol, other_symbol) {
            if self.equivalent.get(&symbol).copied() == Some(other_symbol) {
                self.result = true;
                return;
            }
        }
        self.result = symbol == other_symbol;
    }

    fn visit_literal(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Literal { typetag, value } = &tree_kind else {
            panic!("expected a literal");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Literal {
            typetag: other_tag,
            value: other_value,
        } = &that_kind
        else {
            panic!("expected a literal");
        };
        self.result = typetag == other_tag && value == other_value;
    }

    fn visit_type_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::PrimitiveTypeTree { typetag } = &tree_kind else {
            panic!("expected a primitive type");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::PrimitiveTypeTree { typetag: other } = &that_kind else {
            panic!("expected a primitive type");
        };
        self.result = typetag == other;
    }

    fn visit_type_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::ArrayTypeTree { elemtype } = &tree_kind else {
            panic!("expected an array type");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::ArrayTypeTree { elemtype: other } = &that_kind else {
            panic!("expected an array type");
        };
        self.result = self.scan(ctx, *elemtype, *other);
    }

    fn visit_type_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::TypeApply { clazz, arguments } = &tree_kind else {
            panic!("expected a parameterized type");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::TypeApply {
            clazz: other_clazz,
            arguments: other_arguments,
        } = &that_kind
        else {
            panic!("expected a parameterized type");
        };
        self.result = self.scan(ctx, *clazz, *other_clazz)
            && self.scan_list_pair(ctx, arguments, other_arguments);
    }

    fn visit_type_union(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::TypeUnion { alternatives } = &tree_kind else {
            panic!("expected a union type");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::TypeUnion {
            alternatives: other,
        } = &that_kind
        else {
            panic!("expected a union type");
        };
        self.result = self.scan_list_pair(ctx, alternatives, other);
    }

    fn visit_type_intersection(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::TypeIntersection { bounds } = &tree_kind else {
            panic!("expected an intersection type");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::TypeIntersection { bounds: other } = &that_kind else {
            panic!("expected an intersection type");
        };
        self.result = self.scan_list_pair(ctx, bounds, other);
    }

    fn visit_type_parameter(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::TypeParameter {
            name,
            bounds,
            annotations,
        } = &tree_kind
        else {
            panic!("expected a type parameter");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::TypeParameter {
            name: other_name,
            bounds: other_bounds,
            annotations: other_annotations,
        } = &that_kind
        else {
            panic!("expected a type parameter");
        };
        self.result = *name == other_name.clone()
            && self.scan_list_pair(ctx, bounds, other_bounds)
            && self.scan_list_pair(ctx, annotations, other_annotations);
    }

    fn visit_wildcard(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Wildcard { kind, inner } = &tree_kind else {
            panic!("expected a wildcard");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Wildcard {
            kind: other_kind,
            inner: other_inner,
        } = &that_kind
        else {
            panic!("expected a wildcard");
        };
        self.result = self.scan(ctx, *kind, *other_kind) && self.scan(ctx, *inner, *other_inner);
    }

    fn visit_type_bound_kind(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::TypeBoundKind { kind } = &tree_kind else {
            panic!("expected a type bound kind");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::TypeBoundKind { kind: other } = &that_kind else {
            panic!("expected a type bound kind");
        };
        self.result = kind == other;
    }

    fn visit_annotation(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Annotation {
            annotation_type,
            args,
            ..
        } = &tree_kind
        else {
            panic!("expected an annotation");
        };
        let annotation_type = *annotation_type;
        let args = args.clone();
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Annotation {
            annotation_type: other_type,
            args: other_args,
            ..
        } = &that_kind
        else {
            panic!("expected an annotation");
        };
        self.result = self.scan(ctx, annotation_type, *other_type)
            && self.scan_list_pair(ctx, &args, other_args);
    }

    fn visit_modifiers(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Modifiers { flags, annotations } = &tree_kind else {
            panic!("expected modifiers");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Modifiers {
            flags: other_flags,
            annotations: other_annotations,
        } = &that_kind
        else {
            panic!("expected modifiers");
        };
        self.result =
            *flags == *other_flags && self.scan_list_pair(ctx, annotations, other_annotations);
    }

    fn visit_annotated_type(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::AnnotatedType {
            annotations,
            underlying_type,
        } = &tree_kind
        else {
            panic!("expected an annotated type");
        };
        let annotations = annotations.clone();
        let underlying = *underlying_type;
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::AnnotatedType {
            annotations: other_annotations,
            underlying_type: other_underlying,
        } = &that_kind
        else {
            panic!("expected an annotated type");
        };
        self.result = self.scan_list_pair(ctx, &annotations, other_annotations)
            && self.scan(ctx, underlying, *other_underlying);
    }

    fn visit_erroneous(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Erroneous { errs } = &tree_kind else {
            panic!("expected erroneous");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Erroneous { errs: other } = &that_kind else {
            panic!("expected erroneous");
        };
        self.result = self.scan_list_pair(ctx, errs, other);
    }

    fn visit_module_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::ModuleDecl {
            mods,
            qual_id,
            directives,
            ..
        } = &tree_kind
        else {
            panic!("expected a module declaration");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::ModuleDecl {
            mods: other_mods,
            qual_id: other_qual,
            directives: other_directives,
            ..
        } = &that_kind
        else {
            panic!("expected a module declaration");
        };
        self.result = self.scan(ctx, *mods, *other_mods)
            && self.scan(ctx, *qual_id, *other_qual)
            && self.scan_opt_list_pair(ctx, directives.as_ref(), other_directives.as_ref());
    }

    fn visit_uses(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::Uses { qualid } = &tree_kind else {
            panic!("expected a uses directive");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::Uses { qualid: other } = &that_kind else {
            panic!("expected a uses directive");
        };
        self.result = self.scan(ctx, *qualid, *other);
    }

    fn visit_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_kind = ctx.tree(tree).kind.clone();
        let TreeKind::LetExpr { defs, expr, .. } = &tree_kind else {
            panic!("expected a let expression");
        };
        let that = self.parameter.expect("parameter");
        let that_kind = ctx.tree(that).kind.clone();
        let TreeKind::LetExpr {
            defs: other_defs,
            expr: other_expr,
            ..
        } = &that_kind
        else {
            panic!("expected a let expression");
        };
        self.result =
            self.scan_list_pair(ctx, defs, other_defs) && self.scan(ctx, *expr, *other_expr);
    }
}
