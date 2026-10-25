// Origin: com/sun/tools/javac/tree/TreeScanner.java @ 982d53c (jdk-17.0.19-ga)

//! A subclass of Tree.Visitor, this class defines a general tree scanner
//! pattern. Translation proceeds recursively in left-to-right order down a
//! tree. There is one visitor method in this class for every possible kind of
//! tree node. To obtain a specific scanner, it suffices to override those
//! visitor methods which do some interesting work. The scanner class itself
//! takes care of all navigational aspects.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: ADR 0001 (arena + IDs):
//! - the upstream subclassing (TreeScanner extends Visitor, subclasses override
//!   visitXxx) maps to a TreeScanner struct plus the TreeVisitor trait; a
//!   specialized scanner implements TreeVisitor and overrides the visit methods
//!   it cares about (ADR 0001 registered adaptation);
//! - scan(JCTree) / scan(List) take the CompilerContext explicitly and the
//!   nullable JCTree parameter is Option<TreeId>;
//! - the node child fields are read through the arena; the scan order is copied
//!   exactly from the upstream methods (order is observable by user scanners).

use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::tree::jc_tree::{TreeKind, TreeVisitor};
use crate::util::list::List;

/// Java: public class TreeScanner extends Visitor
#[derive(Default)]
pub struct TreeScanner;

impl TreeScanner {
    /// Java: public void scan(JCTree tree) { if (tree != null) tree.accept(this); }
    pub fn scan(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) {
        if let Some(tree) = tree {
            crate::tree::jc_tree::accept(ctx, tree, self);
        }
    }

    /// Java: public void scan(List<? extends JCTree> trees)
    pub fn scan_list(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) {
        for tree in trees.iter() {
            self.scan(ctx, Some(tree));
        }
    }
}

impl TreeVisitor for TreeScanner {
    fn visit_top_level(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let defs = match &ctx.tree(tree).kind {
            TreeKind::CompilationUnit { defs, .. } => defs.clone(),
            _ => panic!("expected a compilation unit"),
        };
        self.scan_list(ctx, &defs);
    }

    fn visit_package_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, pid) = match &ctx.tree(tree).kind {
            TreeKind::PackageDecl {
                annotations, pid, ..
            } => (annotations.clone(), *pid),
            _ => panic!("expected a package declaration"),
        };
        self.scan_list(ctx, &annotations);
        self.scan(ctx, pid);
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
        self.scan(ctx, mods);
        self.scan(ctx, qual_id);
        self.scan_list_opt(ctx, directives);
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
        self.scan(ctx, qualid);
        self.scan_list_opt(ctx, module_names);
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
        self.scan(ctx, qualid);
        self.scan_list_opt(ctx, module_names);
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
        self.scan(ctx, service_name);
        self.scan_list(ctx, &impl_names);
    }

    fn visit_requires(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let module_name = match &ctx.tree(tree).kind {
            TreeKind::Requires { module_name, .. } => *module_name,
            _ => panic!("expected a requires directive"),
        };
        self.scan(ctx, module_name);
    }

    fn visit_uses(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Uses { qualid } => *qualid,
            _ => panic!("expected a uses directive"),
        };
        self.scan(ctx, qualid);
    }

    fn visit_import(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Import { qualid, .. } => *qualid,
            _ => panic!("expected an import"),
        };
        self.scan(ctx, qualid);
    }

    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, typarams, extending, implementing, permitting, defs) = match &ctx.tree(tree).kind
        {
            TreeKind::ClassDecl {
                mods,
                typarams,
                extending,
                implementing,
                permitting,
                defs,
                ..
            } => (
                *mods,
                typarams.clone(),
                *extending,
                implementing.clone(),
                permitting.clone(),
                defs.clone(),
            ),
            _ => panic!("expected a class declaration"),
        };
        self.scan(ctx, mods);
        self.scan_list(ctx, &typarams);
        self.scan(ctx, extending);
        self.scan_list(ctx, &implementing);
        self.scan_list(ctx, &permitting);
        self.scan_list(ctx, &defs);
    }

    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, restype, typarams, recvparam, params, thrown, default_value, body) =
            match &ctx.tree(tree).kind {
                TreeKind::MethodDecl {
                    mods,
                    restype,
                    typarams,
                    recvparam,
                    params,
                    thrown,
                    default_value,
                    body,
                    ..
                } => (
                    *mods,
                    *restype,
                    typarams.clone(),
                    *recvparam,
                    params.clone(),
                    thrown.clone(),
                    *default_value,
                    *body,
                ),
                _ => panic!("expected a method declaration"),
            };
        self.scan(ctx, mods);
        self.scan(ctx, restype);
        self.scan_list(ctx, &typarams);
        self.scan(ctx, recvparam);
        self.scan_list(ctx, &params);
        self.scan_list(ctx, &thrown);
        self.scan(ctx, default_value);
        self.scan(ctx, body);
    }

    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, vartype, nameexpr, init) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl {
                mods,
                vartype,
                nameexpr,
                init,
                ..
            } => (*mods, *vartype, *nameexpr, *init),
            _ => panic!("expected a variable declaration"),
        };
        self.scan(ctx, mods);
        self.scan(ctx, vartype);
        self.scan(ctx, nameexpr);
        self.scan(ctx, init);
    }

    fn visit_skip(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let stats = match &ctx.tree(tree).kind {
            TreeKind::Block { stats, .. } => stats.clone(),
            _ => panic!("expected a block"),
        };
        self.scan_list(ctx, &stats);
    }

    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, cond) = match &ctx.tree(tree).kind {
            TreeKind::DoWhileLoop { body, cond } => (*body, *cond),
            _ => panic!("expected a do loop"),
        };
        self.scan(ctx, body);
        self.scan(ctx, cond);
    }

    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, body) = match &ctx.tree(tree).kind {
            TreeKind::WhileLoop { cond, body } => (*cond, *body),
            _ => panic!("expected a while loop"),
        };
        self.scan(ctx, cond);
        self.scan(ctx, body);
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
        self.scan_list(ctx, &init);
        self.scan(ctx, cond);
        self.scan_list(ctx, &step);
        self.scan(ctx, body);
    }

    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (var, expr, body) = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop { var, expr, body } => (*var, *expr, *body),
            _ => panic!("expected an enhanced for loop"),
        };
        self.scan(ctx, var);
        self.scan(ctx, expr);
        self.scan(ctx, body);
    }

    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let body = match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement { body, .. } => *body,
            _ => panic!("expected a labelled statement"),
        };
        self.scan(ctx, body);
    }

    fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::Switch {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch"),
        };
        self.scan(ctx, selector);
        self.scan_list(ctx, &cases);
    }

    fn visit_case(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (labels, stats) = match &ctx.tree(tree).kind {
            TreeKind::Case { labels, stats, .. } => (labels.clone(), stats.clone()),
            _ => panic!("expected a case"),
        };
        self.scan_list(ctx, &labels);
        self.scan_list(ctx, &stats);
    }

    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch expression"),
        };
        self.scan(ctx, selector);
        self.scan_list(ctx, &cases);
    }

    fn visit_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lock, body) = match &ctx.tree(tree).kind {
            TreeKind::Synchronized { lock, body } => (*lock, *body),
            _ => panic!("expected a synchronized statement"),
        };
        self.scan(ctx, lock);
        self.scan(ctx, body);
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
        self.scan_list(ctx, &resources);
        self.scan(ctx, body);
        self.scan_list(ctx, &catchers);
        self.scan(ctx, finalizer);
    }

    fn visit_catch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (param, body) = match &ctx.tree(tree).kind {
            TreeKind::Catch { param, body } => (*param, *body),
            _ => panic!("expected a catch clause"),
        };
        self.scan(ctx, param);
        self.scan(ctx, body);
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
        self.scan(ctx, cond);
        self.scan(ctx, truepart);
        self.scan(ctx, falsepart);
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
        self.scan(ctx, cond);
        self.scan(ctx, thenpart);
        self.scan(ctx, elsepart);
    }

    fn visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::ExpressionStatement { expr } => *expr,
            _ => panic!("expected an expression statement"),
        };
        self.scan(ctx, expr);
    }

    fn visit_break(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, .. } => *value,
            _ => panic!("expected a yield"),
        };
        self.scan(ctx, value);
    }

    fn visit_continue(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Return { expr } => *expr,
            _ => panic!("expected a return"),
        };
        self.scan(ctx, expr);
    }

    fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Throw { expr } => *expr,
            _ => panic!("expected a throw"),
        };
        self.scan(ctx, expr);
    }

    fn visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, detail) = match &ctx.tree(tree).kind {
            TreeKind::Assert { cond, detail } => (*cond, *detail),
            _ => panic!("expected an assert"),
        };
        self.scan(ctx, cond);
        self.scan(ctx, detail);
    }

    fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (typeargs, meth, args) = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation {
                typeargs,
                meth,
                args,
                ..
            } => (typeargs.clone(), *meth, args.clone()),
            _ => panic!("expected a method invocation"),
        };
        self.scan_list(ctx, &typeargs);
        self.scan(ctx, meth);
        self.scan_list(ctx, &args);
    }

    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (encl, typeargs, clazz, args, def) = match &ctx.tree(tree).kind {
            TreeKind::NewClass {
                encl,
                typeargs,
                clazz,
                args,
                def,
                ..
            } => (*encl, typeargs.clone(), *clazz, args.clone(), *def),
            _ => panic!("expected a new class"),
        };
        self.scan(ctx, encl);
        self.scan_list(ctx, &typeargs);
        self.scan(ctx, clazz);
        self.scan_list(ctx, &args);
        self.scan(ctx, def);
    }

    fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, elemtype, dims, dim_annotations, elems) = match &ctx.tree(tree).kind {
            TreeKind::NewArray {
                annotations,
                elemtype,
                dims,
                dim_annotations,
                elems,
            } => (
                annotations.clone(),
                *elemtype,
                dims.clone(),
                dim_annotations.clone(),
                elems.clone(),
            ),
            _ => panic!("expected a new array"),
        };
        self.scan_list(ctx, &annotations);
        self.scan(ctx, elemtype);
        self.scan_list(ctx, &dims);
        for annos in dim_annotations.iter() {
            self.scan_list(ctx, &annos);
        }
        self.scan_list_opt(ctx, elems);
    }

    fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, params) = match &ctx.tree(tree).kind {
            TreeKind::Lambda { body, params, .. } => (*body, params.clone()),
            _ => panic!("expected a lambda"),
        };
        self.scan(ctx, body);
        self.scan_list(ctx, &params);
    }

    fn visit_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Parens { expr } => *expr,
            _ => panic!("expected parens"),
        };
        self.scan(ctx, expr);
    }

    fn visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Assign { lhs, rhs } => (*lhs, *rhs),
            _ => panic!("expected an assignment"),
        };
        self.scan(ctx, lhs);
        self.scan(ctx, rhs);
    }

    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a compound assignment"),
        };
        self.scan(ctx, lhs);
        self.scan(ctx, rhs);
    }

    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, .. } => *arg,
            _ => panic!("expected a unary expression"),
        };
        self.scan(ctx, arg);
    }

    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Binary { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a binary expression"),
        };
        self.scan(ctx, lhs);
        self.scan(ctx, rhs);
    }

    fn visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, expr) = match &ctx.tree(tree).kind {
            TreeKind::TypeCast { clazz, expr } => (*clazz, *expr),
            _ => panic!("expected a type cast"),
        };
        self.scan(ctx, clazz);
        self.scan(ctx, expr);
    }

    fn visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (expr, pattern) = match &ctx.tree(tree).kind {
            TreeKind::InstanceOf { expr, pattern } => (*expr, *pattern),
            _ => panic!("expected an instanceof"),
        };
        self.scan(ctx, expr);
        self.scan(ctx, pattern);
    }

    fn visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let var = match &ctx.tree(tree).kind {
            TreeKind::BindingPattern { var } => *var,
            _ => panic!("expected a binding pattern"),
        };
        self.scan(ctx, var);
    }

    fn visit_default_case_label(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_parenthesized_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let pattern = match &ctx.tree(tree).kind {
            TreeKind::ParenthesizedPattern { pattern } => *pattern,
            _ => panic!("expected a parenthesized pattern"),
        };
        self.scan(ctx, pattern);
    }

    fn visit_guard_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (patt, expr) = match &ctx.tree(tree).kind {
            TreeKind::GuardPattern { patt, expr } => (*patt, *expr),
            _ => panic!("expected a guard pattern"),
        };
        self.scan(ctx, patt);
        self.scan(ctx, expr);
    }

    fn visit_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (indexed, index) = match &ctx.tree(tree).kind {
            TreeKind::ArrayAccess { indexed, index } => (*indexed, *index),
            _ => panic!("expected an array access"),
        };
        self.scan(ctx, indexed);
        self.scan(ctx, index);
    }

    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let selected = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { selected, .. } => *selected,
            _ => panic!("expected a field access"),
        };
        self.scan(ctx, selected);
    }

    fn visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (expr, typeargs) = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { expr, typeargs, .. } => (*expr, typeargs.clone()),
            _ => panic!("expected a member reference"),
        };
        self.scan(ctx, expr);
        self.scan_list_opt(ctx, typeargs);
    }

    fn visit_ident(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_literal(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_type_ident(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_type_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let elemtype = match &ctx.tree(tree).kind {
            TreeKind::ArrayTypeTree { elemtype } => *elemtype,
            _ => panic!("expected an array type"),
        };
        self.scan(ctx, elemtype);
    }

    fn visit_type_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, arguments) = match &ctx.tree(tree).kind {
            TreeKind::TypeApply { clazz, arguments } => (*clazz, arguments.clone()),
            _ => panic!("expected a parameterized type"),
        };
        self.scan(ctx, clazz);
        self.scan_list(ctx, &arguments);
    }

    fn visit_type_union(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let alternatives = match &ctx.tree(tree).kind {
            TreeKind::TypeUnion { alternatives } => alternatives.clone(),
            _ => panic!("expected a union type"),
        };
        self.scan_list(ctx, &alternatives);
    }

    fn visit_type_intersection(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let bounds = match &ctx.tree(tree).kind {
            TreeKind::TypeIntersection { bounds } => bounds.clone(),
            _ => panic!("expected an intersection type"),
        };
        self.scan_list(ctx, &bounds);
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
        self.scan_list(ctx, &annotations);
        self.scan_list(ctx, &bounds);
    }

    fn visit_wildcard(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (kind, inner) = match &ctx.tree(tree).kind {
            TreeKind::Wildcard { kind, inner } => (*kind, *inner),
            _ => panic!("expected a wildcard"),
        };
        self.scan(ctx, kind);
        if inner.is_some() {
            self.scan(ctx, inner);
        }
    }

    fn visit_type_bound_kind(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_modifiers(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let annotations = match &ctx.tree(tree).kind {
            TreeKind::Modifiers { annotations, .. } => annotations.clone(),
            _ => panic!("expected modifiers"),
        };
        self.scan_list(ctx, &annotations);
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
        self.scan(ctx, annotation_type);
        self.scan_list(ctx, &args);
    }

    fn visit_annotated_type(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, underlying_type) = match &ctx.tree(tree).kind {
            TreeKind::AnnotatedType {
                annotations,
                underlying_type,
            } => (annotations.clone(), *underlying_type),
            _ => panic!("expected an annotated type"),
        };
        self.scan_list(ctx, &annotations);
        self.scan(ctx, underlying_type);
    }

    fn visit_erroneous(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (defs, expr) = match &ctx.tree(tree).kind {
            TreeKind::LetExpr { defs, expr, .. } => (defs.clone(), *expr),
            _ => panic!("expected a let expression"),
        };
        self.scan_list(ctx, &defs);
        self.scan(ctx, expr);
    }

    fn visit_tree(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {
        panic!("AssertionError: visitTree");
    }
}

impl TreeScanner {
    /// Java: scan(tree.moduleNames) etc. where the upstream list may be null.
    fn scan_list_opt(&mut self, ctx: &mut CompilerContext, trees: Option<List<TreeId>>) {
        if let Some(trees) = trees {
            self.scan_list(ctx, &trees);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::jc_tree::{JCTree, PolyKind, Tag};
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

    fn empty() -> List<TreeId> {
        List::nil()
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

    /// A specialized scanner recording the visited identifier names, like the
    /// upstream subclasses do: it overrides visitIdent and delegates the
    /// navigational methods to TreeScanner.
    #[derive(Default)]
    struct Recorder {
        names: Vec<String>,
    }

    impl Recorder {
        /// Java: tree.accept(this) from a scanner visit method.
        fn scan(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) {
            if let Some(tree) = tree {
                crate::tree::jc_tree::accept(ctx, tree, self);
            }
        }

        fn scan_list(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) {
            for tree in trees.iter() {
                self.scan(ctx, Some(tree));
            }
        }
    }

    impl TreeVisitor for Recorder {
        fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
            match &ctx.tree(tree).kind {
                TreeKind::Ident { name, .. } => {
                    self.names.push(name.to_java_string().to_string_lossy())
                }
                _ => panic!("expected an identifier"),
            }
        }

        fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
            let stats = match &ctx.tree(tree).kind {
                TreeKind::Block { stats, .. } => stats.clone(),
                _ => panic!("expected a block"),
            };
            self.scan_list(ctx, &stats);
        }

        fn visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
            let expr = match &ctx.tree(tree).kind {
                TreeKind::ExpressionStatement { expr } => *expr,
                _ => panic!("expected an expression statement"),
            };
            self.scan(ctx, expr);
        }

        fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
            let (meth, args) = match &ctx.tree(tree).kind {
                TreeKind::MethodInvocation { meth, args, .. } => (*meth, args.clone()),
                _ => panic!("expected a method invocation"),
            };
            self.scan(ctx, meth);
            self.scan_list(ctx, &args);
        }

        fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
            let (mods, defs) = match &ctx.tree(tree).kind {
                TreeKind::ClassDecl { mods, defs, .. } => (*mods, defs.clone()),
                _ => panic!("expected a class declaration"),
            };
            self.scan(ctx, mods);
            self.scan_list(ctx, &defs);
        }

        fn visit_modifiers(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
            let annotations = match &ctx.tree(tree).kind {
                TreeKind::Modifiers { annotations, .. } => annotations.clone(),
                _ => panic!("expected modifiers"),
            };
            self.scan_list(ctx, &annotations);
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
            self.scan(ctx, annotation_type);
            self.scan_list(ctx, &args);
        }

        fn visit_literal(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}
    }

    #[test]
    fn scan_visits_in_left_to_right_order() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let a = ident(&mut ctx, &table, "a");
        let b = ident(&mut ctx, &table, "b");
        let c = ident(&mut ctx, &table, "c");
        let apply = tree(
            &mut ctx,
            Tag::APPLY,
            TreeKind::MethodInvocation {
                typeargs: empty(),
                meth: Some(a),
                args: List::of2(b, c),
                varargs_element: None,
                poly_kind: PolyKind::STANDALONE,
            },
            0,
        );
        let exec = tree(
            &mut ctx,
            Tag::EXEC,
            TreeKind::ExpressionStatement { expr: Some(apply) },
            0,
        );
        let block = tree(
            &mut ctx,
            Tag::BLOCK,
            TreeKind::Block {
                flags: 0,
                stats: List::of(exec),
                endpos: crate::util::position::NOPOS,
            },
            0,
        );
        let mut recorder = Recorder::default();
        recorder.scan(&mut ctx, Some(block));
        assert_eq!(recorder.names, vec!["a", "b", "c"]);
    }

    #[test]
    fn scan_class_and_annotations() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let value = tree(
            &mut ctx,
            Tag::LITERAL,
            TreeKind::Literal {
                typetag: crate::code::type_tag::TypeTag::INT,
                value: Some(crate::tree::jc_tree::LiteralValue::Integer(1)),
            },
            0,
        );
        let annotation_type = ident(&mut ctx, &table, "A");
        let annotation = tree(
            &mut ctx,
            Tag::ANNOTATION,
            TreeKind::Annotation {
                annotation_type: Some(annotation_type),
                args: List::of(value),
                attribute: None,
            },
            0,
        );
        let mods = tree(
            &mut ctx,
            Tag::MODIFIERS,
            TreeKind::Modifiers {
                flags: 0,
                annotations: List::of(annotation),
            },
            0,
        );
        let class = tree(
            &mut ctx,
            Tag::CLASSDEF,
            TreeKind::ClassDecl {
                mods: Some(mods),
                name: make_name(&table, "C"),
                typarams: empty(),
                extending: None,
                implementing: empty(),
                permitting: empty(),
                defs: empty(),
                sym: None,
            },
            0,
        );
        let mut recorder = Recorder::default();
        recorder.scan(&mut ctx, Some(class));
        // The annotation type A is an identifier and is visited in order.
        assert_eq!(recorder.names, vec!["A"]);
    }
}
