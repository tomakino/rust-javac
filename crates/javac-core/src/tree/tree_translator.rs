//! A subclass of Tree.Visitor, this class defines a general tree translator
//! pattern. Translation proceeds recursively in left-to-right order down a
//! tree, constructing translated nodes by overwriting existing ones. There is
//! one visitor method in this class for every possible kind of tree node. To
//! obtain a specific translator, it suffices to override those visitor methods
//! which do some interesting work. The translator class itself takes care of
//! all navigational aspects.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! ADR 0001 (arena + IDs):
//! - the visitor result field maps to Option<TreeId>; translate(JCTree)
//!   returns Option<TreeId> (Java null -> None);
//! - the in-place write-back (tree.child = translate(tree.child)) first takes a
//!   snapshot of the child IDs (they are Copy), translates, then writes the
//!   slots back through CompilerContext::tree_mut (this is how the upstream
//!   mutation is reproduced without aliasing the arena);
//! - translate(List) rebuilds the list (Java mutates the list cells); a
//!   translate of a list element must not produce null because List<TreeId>
//!   cannot represent null elements - this case panics with an explicit
//!   message (registered section 9: the adapter does not silently drop it);
//! - the subclassing of the upstream class maps to implementing TreeVisitor
//!   with the TreeTranslator state; downstream translators reuse TreeTranslator
//!   (Override is expressed by wrapping/delegating, registered).

use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::tree::jc_tree::{TreeKind, TreeVisitor};
use crate::util::list::List;

/// Java: public class TreeTranslator extends JCTree.Visitor
#[derive(Default)]
pub struct TreeTranslator {
    /// Java: protected JCTree result
    result: Option<TreeId>,
}

impl TreeTranslator {
    pub fn new() -> TreeTranslator {
        TreeTranslator { result: None }
    }

    /// Java: public <T extends JCTree> T translate(T tree)
    pub fn translate(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) -> Option<TreeId> {
        match tree {
            None => None,
            Some(tree) => {
                crate::tree::jc_tree::accept(ctx, tree, self);
                self.result.take()
            }
        }
    }

    /// Java: public <T extends JCTree> List<T> translate(List<T> trees)
    pub fn translate_list(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &List<TreeId>,
    ) -> List<TreeId> {
        let mut result = List::nil();
        for tree in trees.iter() {
            let translated = self.translate(ctx, Some(tree));
            match translated {
                Some(translated) => result = result.append(translated),
                None => panic!(
                    "translation of a list element produced null (not representable in List<TreeId>)"
                ),
            }
        }
        result
    }

    /// Java: public List<JCVariableDecl> translateVarDefs(List<JCVariableDecl> trees)
    pub fn translate_var_defs(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &List<TreeId>,
    ) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }

    /// Java: public List<JCTypeParameter> translateTypeParams(List<JCTypeParameter> trees)
    pub fn translate_type_params(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &List<TreeId>,
    ) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }

    /// Java: public List<JCCase> translateCases(List<JCCase> trees)
    pub fn translate_cases(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &List<TreeId>,
    ) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }

    /// Java: public List<JCCatch> translateCatchers(List<JCCatch> trees)
    pub fn translate_catchers(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &List<TreeId>,
    ) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }

    /// Java: public List<JCAnnotation> translateAnnotations(List<JCAnnotation> trees)
    pub fn translate_annotations(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &List<TreeId>,
    ) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
}

impl TreeVisitor for TreeTranslator {
    fn visit_top_level(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
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

    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
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

    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, nameexpr, vartype, init) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl {
                mods,
                nameexpr,
                vartype,
                init,
                ..
            } => (*mods, *nameexpr, *vartype, *init),
            _ => panic!("expected a variable declaration"),
        };
        let mods = self.translate(ctx, mods);
        let nameexpr = self.translate(ctx, nameexpr);
        let vartype = self.translate(ctx, vartype);
        let init = self.translate(ctx, init);
        if let TreeKind::VariableDecl {
            mods: m,
            nameexpr: ne,
            vartype: vt,
            init: i,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *ne = nameexpr;
            *vt = vartype;
            *i = init;
        }
        self.result = Some(tree);
    }

    fn visit_skip(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let stats = match &ctx.tree(tree).kind {
            TreeKind::Block { stats, .. } => stats.clone(),
            _ => panic!("expected a block"),
        };
        let stats = self.translate_list(ctx, &stats);
        if let TreeKind::Block { stats: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = stats;
        }
        self.result = Some(tree);
    }

    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
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

    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
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

    fn visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
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

    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
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

    fn visit_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
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

    fn visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let var = match &ctx.tree(tree).kind {
            TreeKind::BindingPattern { var } => *var,
            _ => panic!("expected a binding pattern"),
        };
        let var = self.translate(ctx, var);
        if let TreeKind::BindingPattern { var: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = var;
        }
        self.result = Some(tree);
    }

    fn visit_default_case_label(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    fn visit_parenthesized_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let pattern = match &ctx.tree(tree).kind {
            TreeKind::ParenthesizedPattern { pattern } => *pattern,
            _ => panic!("expected a parenthesized pattern"),
        };
        let pattern = self.translate(ctx, pattern);
        if let TreeKind::ParenthesizedPattern { pattern: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = pattern;
        }
        self.result = Some(tree);
    }

    fn visit_guard_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (patt, expr) = match &ctx.tree(tree).kind {
            TreeKind::GuardPattern { patt, expr } => (*patt, *expr),
            _ => panic!("expected a guard pattern"),
        };
        let patt = self.translate(ctx, patt);
        let expr = self.translate(ctx, expr);
        if let TreeKind::GuardPattern { patt: p, expr: e } = &mut ctx.tree_mut(tree).kind {
            *p = patt;
            *e = expr;
        }
        self.result = Some(tree);
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

    fn visit_ident(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::jc_tree::{JCTree, Tag};
    use crate::util::name::Name;
    use crate::util::name::NameTableHandle;
    use std::rc::Rc;

    fn make_name(table: &Rc<NameTableHandle>, text: &str) -> Name {
        let units: Vec<u16> = text.encode_utf16().collect();
        table.from_chars(&units, 0, units.len() as i32)
    }

    fn tree(ctx: &mut CompilerContext, tag: Tag, kind: TreeKind, pos: i32) -> TreeId {
        ctx.alloc_tree(JCTree {
            pos,
            ty: None,
            tag,
            kind,
        })
    }

    fn ident(ctx: &mut CompilerContext, table: &Rc<NameTableHandle>, text: &str) -> TreeId {
        tree(
            ctx,
            Tag::IDENT,
            TreeKind::Ident {
                name: make_name(table, text),
                sym: None,
            },
            0,
        )
    }

    #[test]
    fn translate_rewrites_children_in_place() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let a = ident(&mut ctx, &table, "a");
        let b = ident(&mut ctx, &table, "b");
        let binary = tree(
            &mut ctx,
            Tag::PLUS,
            TreeKind::Binary {
                lhs: Some(a),
                rhs: Some(b),
                operator: None,
            },
            0,
        );
        let mut translator = TreeTranslator::new();
        let result = translator.translate(&mut ctx, Some(binary));
        assert_eq!(result, Some(binary));
        let (lhs, rhs) = match &ctx.tree(binary).kind {
            TreeKind::Binary { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a binary expression"),
        };
        assert_eq!(lhs, Some(a));
        assert_eq!(rhs, Some(b));
    }

    #[test]
    fn translate_none_and_lists() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let a = ident(&mut ctx, &table, "a");
        let b = ident(&mut ctx, &table, "b");
        let mut translator = TreeTranslator::new();
        assert_eq!(translator.translate(&mut ctx, None), None);
        let list = List::of2(a, b);
        let translated = translator.translate_list(&mut ctx, &list);
        assert_eq!(translated.length(), 2);
        assert_eq!(translated.get(0), a);
        assert_eq!(translated.get(1), b);
    }

    #[test]
    fn translate_writes_back_in_operator_operands() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let a = ident(&mut ctx, &table, "a");
        let b = ident(&mut ctx, &table, "b");
        let binary = tree(
            &mut ctx,
            Tag::PLUS,
            TreeKind::Binary {
                lhs: Some(a),
                rhs: Some(b),
                operator: None,
            },
            0,
        );
        let mut translator = TreeTranslator::new();
        let result = translator.translate(&mut ctx, Some(binary));
        assert_eq!(result, Some(binary));
        match &ctx.tree(binary).kind {
            TreeKind::Binary { lhs, rhs, .. } => {
                assert_eq!(*lhs, Some(a));
                assert_eq!(*rhs, Some(b));
            }
            _ => panic!("expected a binary expression"),
        }
    }
}
