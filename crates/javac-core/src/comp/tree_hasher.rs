// Origin: com/sun/tools/javac/comp/TreeHasher.java @ 982d53c (jdk-17.0.19-ga)

//! A tree visitor that computes a hash code (TreeHasher.java).
//!
//! PORTING_RULES: TreeHasher extends TreeScanner; the port implements TreeVisitor
//! with the TreeScanner default navigation copied for the node kinds TreeHasher
//! does not override (registered adapter for Java inheritance), and overrides
//! scan/visitLiteral/visitIdent/visitSelect/visitVarDef exactly as upstream.

use std::collections::HashMap;

use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId};
use crate::tree::jc_tree::{TreeKind, TreeVisitor};
use crate::tree::tree_info;
use crate::tree::tree_scanner::TreeScanner;
use crate::util::list::List;

/// Java: public class TreeHasher extends TreeScanner (TreeHasher.java).
///
/// A tree visitor that computes a hash code.
pub struct TreeHasher {
    /// Java: private final Map<Symbol, Integer> symbolHashes.
    symbol_hashes: HashMap<SymbolId, i32>,
    /// Java: private int result = 17.
    result: i32,
}

impl TreeHasher {
    /// Java: public TreeHasher(Map<Symbol, Integer> symbolHashes).
    pub fn new(symbol_hashes: HashMap<SymbolId, i32>) -> TreeHasher {
        TreeHasher { symbol_hashes, result: 17 }
    }

    /// Java: public static int hash(JCTree tree, Collection<? extends Symbol> symbols)
    /// (TreeHasher.java).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - tree: the tree to hash;
    ///   - symbols: the symbols to index for stable identifiers.
    ///
    /// # Returns
    ///   - the hash code (0 for a null tree).
    pub fn hash(ctx: &mut CompilerContext, tree: Option<TreeId>, symbols: &[SymbolId]) -> i32 {
        let Some(tree) = tree else {
            return 0;
        };
        let mut symbol_hashes = HashMap::new();
        for symbol in symbols {
            let size = symbol_hashes.len() as i32;
            symbol_hashes.insert(*symbol, size);
        }
        let mut hasher = TreeHasher::new(symbol_hashes);
        crate::tree::jc_tree::accept(ctx, tree, &mut hasher);
        hasher.result
    }

    /// Java: private void hash(Object object).
    fn hash_object(&mut self, object: i64) {
        self.result = 31i32.wrapping_mul(self.result).wrapping_add(object as i32);
    }

    /// Java: public void scan(JCTree tree).
    pub fn scan(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) {
        let Some(tree) = tree else {
            return;
        };
        let tree = tree_info::skip_parens(ctx, tree);
        if let Some(ty) = ctx.tree(tree).ty {
            if let Some(value) = ctx.typ(ty).const_value().cloned() {
                self.hash_value(&value);
                return;
            }
        }
        self.hash_object(i64::from(ctx.tree(tree).tag as i32));
        crate::tree::jc_tree::accept(ctx, tree, self);
    }

    /// Java: public void scan(List<? extends JCTree> trees).
    pub fn scan_list(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) {
        for tree in trees.iter() {
            self.scan(ctx, Some(tree));
        }
    }

    /// Java: public void scan(List<? extends JCTree> trees) (TreeScanner.java);
    /// nullable-list helper).
    pub fn scan_list_opt(&mut self, ctx: &mut CompilerContext, trees: Option<List<TreeId>>) {
        if let Some(trees) = trees {
            self.scan_list(ctx, &trees);
        }
    }
    /// Java: hash of a constant value (Objects.hashCode of the wrapper).
    fn hash_value(&mut self, value: &crate::util::constants::ConstantValue) {
        use crate::util::constants::ConstantValue;
        let hash = match value {
            ConstantValue::Integer(v) => i64::from(*v),
            ConstantValue::Byte(v) => i64::from(*v),
            ConstantValue::Short(v) => i64::from(*v),
            ConstantValue::Long(v) => *v,
            ConstantValue::Float(v) => i64::from(v.to_bits()),
            ConstantValue::Double(v) => v.to_bits() as i64,
            ConstantValue::Character(v) => i64::from(*v),
            ConstantValue::Boolean(v) => {
                if *v { 1231 } else { 1237 }
            }
            ConstantValue::String(v) => {
                let mut hash: i32 = 0;
                for unit in v.to_string_lossy().encode_utf16() {
                    hash = hash.wrapping_mul(31).wrapping_add(i32::from(unit));
                }
                i64::from(hash)
            }
        };
        self.hash_object(hash);
    }
}

impl TreeVisitor for TreeHasher {
    /// Java: public void visitTopLevel(JCCompilationUnit tree).
    fn visit_top_level(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let defs = match &ctx.tree(tree).kind {
            TreeKind::CompilationUnit { defs, .. } => defs.clone(),
            _ => panic!("expected a compilation unit"),
        };
        self.scan_list(ctx, &defs);
    }


    /// Java: public void visitPackageDef(JCPackageDecl tree).
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


    /// Java: public void visitModuleDef(JCModuleDecl tree).
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


    /// Java: public void visitExports(JCExports tree).
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


    /// Java: public void visitOpens(JCOpens tree).
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


    /// Java: public void visitProvides(JCProvides tree).
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


    /// Java: public void visitRequires(JCRequires tree).
    fn visit_requires(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let module_name = match &ctx.tree(tree).kind {
            TreeKind::Requires { module_name, .. } => *module_name,
            _ => panic!("expected a requires directive"),
        };
        self.scan(ctx, module_name);
    }


    /// Java: public void visitUses(JCUses tree).
    fn visit_uses(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Uses { qualid } => *qualid,
            _ => panic!("expected a uses directive"),
        };
        self.scan(ctx, qualid);
    }


    /// Java: public void visitImport(JCImport tree).
    fn visit_import(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Import { qualid, .. } => *qualid,
            _ => panic!("expected an import"),
        };
        self.scan(ctx, qualid);
    }


    /// Java: public void visitClassDef(JCClassDecl tree).
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


    /// Java: public void visitMethodDef(JCMethodDecl tree).
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


    /// Java: public void visitVarDef(JCVariableDecl tree).
    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let TreeKind::VariableDecl { sym: Some(sym), .. } = &ctx.tree(tree).kind {
            let size = self.symbol_hashes.len() as i32;
            self.symbol_hashes.entry(*sym).or_insert(size);
        }
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


    /// Java: public void visitSkip(JCSkip tree).
    fn visit_skip(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}


    /// Java: public void visitBlock(JCBlock tree).
    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let stats = match &ctx.tree(tree).kind {
            TreeKind::Block { stats, .. } => stats.clone(),
            _ => panic!("expected a block"),
        };
        self.scan_list(ctx, &stats);
    }


    /// Java: public void visitDoLoop(JCDoWhileLoop tree).
    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, cond) = match &ctx.tree(tree).kind {
            TreeKind::DoWhileLoop { body, cond } => (*body, *cond),
            _ => panic!("expected a do loop"),
        };
        self.scan(ctx, body);
        self.scan(ctx, cond);
    }


    /// Java: public void visitWhileLoop(JCWhileLoop tree).
    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, body) = match &ctx.tree(tree).kind {
            TreeKind::WhileLoop { cond, body } => (*cond, *body),
            _ => panic!("expected a while loop"),
        };
        self.scan(ctx, cond);
        self.scan(ctx, body);
    }


    /// Java: public void visitForLoop(JCForLoop tree).
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


    /// Java: public void visitForeachLoop(JCEnhancedForLoop tree).
    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (var, expr, body) = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop { var, expr, body } => (*var, *expr, *body),
            _ => panic!("expected an enhanced for loop"),
        };
        self.scan(ctx, var);
        self.scan(ctx, expr);
        self.scan(ctx, body);
    }


    /// Java: public void visitLabelled(JCLabeledStatement tree).
    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let body = match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement { body, .. } => *body,
            _ => panic!("expected a labelled statement"),
        };
        self.scan(ctx, body);
    }


    /// Java: public void visitSwitch(JCSwitch tree).
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


    /// Java: public void visitCase(JCCase tree).
    fn visit_case(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (labels, stats) = match &ctx.tree(tree).kind {
            TreeKind::Case { labels, stats, .. } => (labels.clone(), stats.clone()),
            _ => panic!("expected a case"),
        };
        self.scan_list(ctx, &labels);
        self.scan_list(ctx, &stats);
    }


    /// Java: public void visitSwitchExpression(JCSwitchExpression tree).
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


    /// Java: public void visitSynchronized(JCSynchronized tree).
    fn visit_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lock, body) = match &ctx.tree(tree).kind {
            TreeKind::Synchronized { lock, body } => (*lock, *body),
            _ => panic!("expected a synchronized statement"),
        };
        self.scan(ctx, lock);
        self.scan(ctx, body);
    }


    /// Java: public void visitTry(JCTry tree).
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


    /// Java: public void visitCatch(JCCatch tree).
    fn visit_catch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (param, body) = match &ctx.tree(tree).kind {
            TreeKind::Catch { param, body } => (*param, *body),
            _ => panic!("expected a catch clause"),
        };
        self.scan(ctx, param);
        self.scan(ctx, body);
    }


    /// Java: public void visitConditional(JCConditional tree).
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


    /// Java: public void visitIf(JCIf tree).
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


    /// Java: public void visitExec(JCExpressionStatement tree).
    fn visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::ExpressionStatement { expr } => *expr,
            _ => panic!("expected an expression statement"),
        };
        self.scan(ctx, expr);
    }


    /// Java: public void visitBreak(JCBreak tree).
    fn visit_break(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}


    /// Java: public void visitYield(JCYield tree).
    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, .. } => *value,
            _ => panic!("expected a yield"),
        };
        self.scan(ctx, value);
    }


    /// Java: public void visitContinue(JCContinue tree).
    fn visit_continue(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}


    /// Java: public void visitReturn(JCReturn tree).
    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Return { expr } => *expr,
            _ => panic!("expected a return"),
        };
        self.scan(ctx, expr);
    }


    /// Java: public void visitThrow(JCThrow tree).
    fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Throw { expr } => *expr,
            _ => panic!("expected a throw"),
        };
        self.scan(ctx, expr);
    }


    /// Java: public void visitAssert(JCAssert tree).
    fn visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, detail) = match &ctx.tree(tree).kind {
            TreeKind::Assert { cond, detail } => (*cond, *detail),
            _ => panic!("expected an assert"),
        };
        self.scan(ctx, cond);
        self.scan(ctx, detail);
    }


    /// Java: public void visitApply(JCMethodInvocation tree).
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


    /// Java: public void visitNewClass(JCNewClass tree).
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


    /// Java: public void visitNewArray(JCNewArray tree).
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


    /// Java: public void visitLambda(JCLambda tree).
    fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, params) = match &ctx.tree(tree).kind {
            TreeKind::Lambda { body, params, .. } => (*body, params.clone()),
            _ => panic!("expected a lambda"),
        };
        self.scan(ctx, body);
        self.scan_list(ctx, &params);
    }


    /// Java: public void visitParens(JCParens tree).
    fn visit_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Parens { expr } => *expr,
            _ => panic!("expected parens"),
        };
        self.scan(ctx, expr);
    }


    /// Java: public void visitAssign(JCAssign tree).
    fn visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Assign { lhs, rhs } => (*lhs, *rhs),
            _ => panic!("expected an assignment"),
        };
        self.scan(ctx, lhs);
        self.scan(ctx, rhs);
    }


    /// Java: public void visitAssignop(JCAssignOp tree).
    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a compound assignment"),
        };
        self.scan(ctx, lhs);
        self.scan(ctx, rhs);
    }


    /// Java: public void visitUnary(JCUnary tree).
    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, .. } => *arg,
            _ => panic!("expected a unary expression"),
        };
        self.scan(ctx, arg);
    }


    /// Java: public void visitBinary(JCBinary tree).
    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Binary { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a binary expression"),
        };
        self.scan(ctx, lhs);
        self.scan(ctx, rhs);
    }


    /// Java: public void visitTypeCast(JCTypeCast tree).
    fn visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, expr) = match &ctx.tree(tree).kind {
            TreeKind::TypeCast { clazz, expr } => (*clazz, *expr),
            _ => panic!("expected a type cast"),
        };
        self.scan(ctx, clazz);
        self.scan(ctx, expr);
    }


    /// Java: public void visitTypeTest(JCInstanceOf tree).
    fn visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (expr, pattern) = match &ctx.tree(tree).kind {
            TreeKind::InstanceOf { expr, pattern } => (*expr, *pattern),
            _ => panic!("expected an instanceof"),
        };
        self.scan(ctx, expr);
        self.scan(ctx, pattern);
    }


    /// Java: public void visitBindingPattern(JCBindingPattern tree).
    fn visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let var = match &ctx.tree(tree).kind {
            TreeKind::BindingPattern { var } => *var,
            _ => panic!("expected a binding pattern"),
        };
        self.scan(ctx, var);
    }


    /// Java: public void visitDefaultCaseLabel(JCDefaultCaseLabel tree).
    fn visit_default_case_label(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}


    /// Java: public void visitParenthesizedPattern(JCParenthesizedPattern that).
    fn visit_parenthesized_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let pattern = match &ctx.tree(tree).kind {
            TreeKind::ParenthesizedPattern { pattern } => *pattern,
            _ => panic!("expected a parenthesized pattern"),
        };
        self.scan(ctx, pattern);
    }


    /// Java: public void visitGuardPattern(JCGuardPattern that).
    fn visit_guard_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (patt, expr) = match &ctx.tree(tree).kind {
            TreeKind::GuardPattern { patt, expr } => (*patt, *expr),
            _ => panic!("expected a guard pattern"),
        };
        self.scan(ctx, patt);
        self.scan(ctx, expr);
    }


    /// Java: public void visitIndexed(JCArrayAccess tree).
    fn visit_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (indexed, index) = match &ctx.tree(tree).kind {
            TreeKind::ArrayAccess { indexed, index } => (*indexed, *index),
            _ => panic!("expected an array access"),
        };
        self.scan(ctx, indexed);
        self.scan(ctx, index);
    }


    /// Java: public void visitSelect(JCFieldAccess tree).
    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let TreeKind::FieldAccess { sym, .. } = &ctx.tree(tree).kind {
            let hash = sym.map(|sym| (sym.index() as i64) % 1_000_003).unwrap_or(0);
            self.hash_object(hash);
        }
        let selected = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { selected, .. } => *selected,
            _ => panic!("expected a field access"),
        };
        self.scan(ctx, selected);
    }


    /// Java: public void visitReference(JCMemberReference tree).
    fn visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (expr, typeargs) = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { expr, typeargs, .. } => (*expr, typeargs.clone()),
            _ => panic!("expected a member reference"),
        };
        self.scan(ctx, expr);
        self.scan_list_opt(ctx, typeargs);
    }


    /// Java: public void visitIdent(JCIdent tree).
    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let TreeKind::Ident { sym, .. } = &ctx.tree(tree).kind {
            if let Some(sym) = sym {
                if let Some(hash) = self.symbol_hashes.get(sym).copied() {
                    self.hash_object(i64::from(hash));
                } else {
                    self.hash_object((sym.index() as i64) % 1_000_003);
                }
            } else {
                self.hash_object(0);
            }
        }
    }

    /// Java: public void visitLiteral(JCLiteral tree).
    fn visit_literal(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let TreeKind::Literal { value, .. } = &ctx.tree(tree).kind {
            let hash = match value {
                Some(crate::tree::jc_tree::LiteralValue::Integer(v)) => i64::from(*v),
                Some(crate::tree::jc_tree::LiteralValue::Byte(v)) => i64::from(*v),
                Some(crate::tree::jc_tree::LiteralValue::Short(v)) => i64::from(*v),
                Some(crate::tree::jc_tree::LiteralValue::Long(v)) => *v,
                Some(crate::tree::jc_tree::LiteralValue::Float(v)) => i64::from(v.to_bits()),
                Some(crate::tree::jc_tree::LiteralValue::Double(v)) => v.to_bits() as i64,
                Some(crate::tree::jc_tree::LiteralValue::Character(v)) => i64::from(*v),
                Some(crate::tree::jc_tree::LiteralValue::Boolean(v)) => {
                    if *v { 1231 } else { 1237 }
                }
                Some(crate::tree::jc_tree::LiteralValue::String(v)) => {
                    let mut hash: i32 = 0;
                    for unit in v.to_string_lossy().encode_utf16() {
                        hash = hash.wrapping_mul(31).wrapping_add(i32::from(unit));
                    }
                    i64::from(hash)
                }
                None => 0,
            };
            self.hash_object(hash);
        }
    }

    /// Java: public void visitTypeIdent(JCPrimitiveTypeTree tree).
    fn visit_type_ident(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}


    /// Java: public void visitTypeArray(JCArrayTypeTree tree).
    fn visit_type_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let elemtype = match &ctx.tree(tree).kind {
            TreeKind::ArrayTypeTree { elemtype } => *elemtype,
            _ => panic!("expected an array type"),
        };
        self.scan(ctx, elemtype);
    }


    /// Java: public void visitTypeApply(JCTypeApply tree).
    fn visit_type_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, arguments) = match &ctx.tree(tree).kind {
            TreeKind::TypeApply { clazz, arguments } => (*clazz, arguments.clone()),
            _ => panic!("expected a parameterized type"),
        };
        self.scan(ctx, clazz);
        self.scan_list(ctx, &arguments);
    }


    /// Java: public void visitTypeUnion(JCTypeUnion tree).
    fn visit_type_union(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let alternatives = match &ctx.tree(tree).kind {
            TreeKind::TypeUnion { alternatives } => alternatives.clone(),
            _ => panic!("expected a union type"),
        };
        self.scan_list(ctx, &alternatives);
    }


    /// Java: public void visitTypeIntersection(JCTypeIntersection tree).
    fn visit_type_intersection(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let bounds = match &ctx.tree(tree).kind {
            TreeKind::TypeIntersection { bounds } => bounds.clone(),
            _ => panic!("expected an intersection type"),
        };
        self.scan_list(ctx, &bounds);
    }


    /// Java: public void visitTypeParameter(JCTypeParameter tree).
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


    /// Java: public void visitWildcard(JCWildcard tree).
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


    /// Java: public void visitTypeBoundKind(TypeBoundKind that).
    fn visit_type_bound_kind(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}


    /// Java: public void visitModifiers(JCModifiers tree).
    fn visit_modifiers(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let annotations = match &ctx.tree(tree).kind {
            TreeKind::Modifiers { annotations, .. } => annotations.clone(),
            _ => panic!("expected modifiers"),
        };
        self.scan_list(ctx, &annotations);
    }


    /// Java: public void visitAnnotation(JCAnnotation tree).
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


    /// Java: public void visitAnnotatedType(JCAnnotatedType tree).
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


    /// Java: public void visitErroneous(JCErroneous tree).
    fn visit_erroneous(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}


    /// Java: public void visitLetExpr(LetExpr tree).
    fn visit_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (defs, expr) = match &ctx.tree(tree).kind {
            TreeKind::LetExpr { defs, expr, .. } => (defs.clone(), *expr),
            _ => panic!("expected a let expression"),
        };
        self.scan_list(ctx, &defs);
        self.scan(ctx, expr);
    }


    /// Java: public void visitTree(JCTree tree).
    fn visit_tree(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {
        panic!("AssertionError: visitTree");
    }
}
