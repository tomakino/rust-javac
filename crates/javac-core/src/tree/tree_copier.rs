// Origin: com/sun/tools/javac/tree/TreeCopier.java @ 982d53c (jdk-17.0.19-ga)
// PORTING_RULES §19 gap registration (method-level audit of the ported upstream class):
// Dependency/phase: tree batch (tree nodes/scanners for the corresponding upstream feature).
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: visitAssignment (TreeCopier.java:109)
// - NOT PORTED: visitCompoundAssignment (TreeCopier.java:117)
// - NOT PORTED: visitConditionalExpression (TreeCopier.java:187)
// - NOT PORTED: visitDoWhileLoop (TreeCopier.java:202)
// - NOT PORTED: visitExpressionStatement (TreeCopier.java:217)
// - NOT PORTED: visitEnhancedForLoop (TreeCopier.java:224)
// - NOT PORTED: visitIdentifier (TreeCopier.java:243)
// - NOT PORTED: visitArrayAccess (TreeCopier.java:265)
// - NOT PORTED: visitLabeledStatement (TreeCopier.java:273)
// - NOT PORTED: visitMethodInvocation (TreeCopier.java:300)
// - NOT PORTED: visitLambdaExpression (TreeCopier.java:336)
// - NOT PORTED: visitMemberSelect (TreeCopier.java:358)
// - NOT PORTED: visitMemberReference (TreeCopier.java:365)
// - NOT PORTED: visitEmptyStatement (TreeCopier.java:373)
// - NOT PORTED: visitCompilationUnit (TreeCopier.java:410)
// - NOT PORTED: visitParameterizedType (TreeCopier.java:435)
// - NOT PORTED: visitUnionType (TreeCopier.java:443)
// - NOT PORTED: visitIntersectionType (TreeCopier.java:450)
// - NOT PORTED: visitArrayType (TreeCopier.java:457)
// - NOT PORTED: visitPrimitiveType (TreeCopier.java:472)
// - NOT PORTED: visitInstanceOf (TreeCopier.java:486)
// - NOT PORTED: visitGuardedPattern (TreeCopier.java:501)
// - NOT PORTED: visitVariable (TreeCopier.java:529)
// - NOT PORTED: visitOther (TreeCopier.java:606)
// End of §19 gap registration.

//! Creates a copy of a tree, using a given TreeMaker. Names, literal values,
//! etc are shared with the original.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: ADR 0001 (arena + IDs):
//! - the upstream copy(T) / copy(T, P) / copy(List<T>) return the copied node /
//!   list; T is TreeId and the nullable trees are Option<TreeId>;
//! - TreeMaker is not ported yet (S3 batch), so the node construction that the
//!   upstream code delegates to M.at(pos).Xxx(...) is expressed by directly
//!   allocating the JCTree node in the arena with the same position and fields.
//!   This is registered in PORTING_RULES section 9; when TreeMaker lands the
//!   construction sites switch to it (the copy semantics stay identical);
//! - a copied node does not keep the Attr-filled type/symbol fields where the
//!   upstream TreeMaker constructors leave them null (ty/sym/... are None), and
//!   it keeps the structural fields exactly as the upstream visitor copies them
//!   (annotations on a new array are dropped by the upstream copier, etc.);
//! - the JCTree fields that the upstream Java class can leave null but the port
//!   cannot represent (JCVariableDecl.name for a receiver var def) keep the
//!   original value; registered section 9;
//! - the Java TreeVisitor<JCTree,P> interface (com.sun.source) is not ported;
//!   the TreeVisitor trait (jc_tree.rs) provides the dispatch and the Copier
//!   stores the result in its result field like TreeTranslator does.

use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::tree::jc_tree::{JCTree, ParameterKind, PolyKind, Tag, TreeKind, TreeVisitor};
use crate::util::list::List;
use crate::util::position::NOPOS;

/// Java: public class TreeCopier<P> implements TreeVisitor<JCTree,P>
#[derive(Default)]
pub struct TreeCopier {
    /// Java: the value returned by the last visit method.
    result: Option<TreeId>,
}

impl TreeCopier {
    pub fn new() -> TreeCopier {
        TreeCopier { result: None }
    }

    /// Java: public <T extends JCTree> T copy(T tree)
    pub fn copy(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) -> Option<TreeId> {
        match tree {
            None => None,
            Some(tree) => {
                self.result = None;
                crate::tree::jc_tree::accept(ctx, tree, self);
                self.result.take()
            }
        }
    }

    /// Java: public <T extends JCTree> List<T> copy(List<T> trees)
    pub fn copy_list(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        let mut result = List::nil();
        for tree in trees.iter() {
            let copied = self.copy(ctx, Some(tree));
            match copied {
                Some(copied) => result = result.append(copied),
                None => panic!("copy of a list element produced null"),
            }
        }
        result
    }

    fn copy_list_opt(
        &mut self,
        ctx: &mut CompilerContext,
        trees: Option<List<TreeId>>,
    ) -> Option<List<TreeId>> {
        trees.map(|trees| self.copy_list(ctx, &trees))
    }

    /// Java: M.at(pos) - allocate the copied node with the original position.
    fn alloc(&mut self, ctx: &mut CompilerContext, pos: i32, tag: Tag, kind: TreeKind) -> TreeId {
        ctx.alloc_tree(JCTree {
            pos,
            ty: None,
            tag,
            kind,
        })
    }
}

impl TreeVisitor for TreeCopier {
    fn visit_annotated_type(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, underlying_type) = match &ctx.tree(tree).kind {
            TreeKind::AnnotatedType {
                annotations,
                underlying_type,
            } => (annotations.clone(), *underlying_type),
            _ => panic!("expected an annotated type"),
        };
        let annotations = self.copy_list(ctx, &annotations);
        let underlying_type = self.copy(ctx, underlying_type);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::ANNOTATED_TYPE,
            TreeKind::AnnotatedType {
                annotations,
                underlying_type,
            },
        ));
    }

    fn visit_annotation(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotation_type, args, attribute, tag) = match &ctx.tree(tree).kind {
            TreeKind::Annotation {
                annotation_type,
                args,
                attribute,
            } => (
                *annotation_type,
                args.clone(),
                *attribute,
                ctx.tree(tree).tag,
            ),
            _ => panic!("expected an annotation"),
        };
        let annotation_type = self.copy(ctx, annotation_type);
        let args = self.copy_list(ctx, &args);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            tag,
            TreeKind::Annotation {
                annotation_type,
                args,
                attribute,
            },
        ));
    }

    fn visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, detail) = match &ctx.tree(tree).kind {
            TreeKind::Assert { cond, detail } => (*cond, *detail),
            _ => panic!("expected an assert"),
        };
        let cond = self.copy(ctx, cond);
        let detail = self.copy(ctx, detail);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(ctx, pos, Tag::ASSERT, TreeKind::Assert { cond, detail }));
    }

    fn visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Assign { lhs, rhs } => (*lhs, *rhs),
            _ => panic!("expected an assignment"),
        };
        let lhs = self.copy(ctx, lhs);
        let rhs = self.copy(ctx, rhs);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(ctx, pos, Tag::ASSIGN, TreeKind::Assign { lhs, rhs }));
    }

    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs, tag) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { lhs, rhs, .. } => (*lhs, *rhs, ctx.tree(tree).tag),
            _ => panic!("expected a compound assignment"),
        };
        let lhs = self.copy(ctx, lhs);
        let rhs = self.copy(ctx, rhs);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            tag,
            TreeKind::AssignOp {
                lhs,
                rhs,
                operator: None,
            },
        ));
    }

    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs, tag) = match &ctx.tree(tree).kind {
            TreeKind::Binary { lhs, rhs, .. } => (*lhs, *rhs, ctx.tree(tree).tag),
            _ => panic!("expected a binary expression"),
        };
        let lhs = self.copy(ctx, lhs);
        let rhs = self.copy(ctx, rhs);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            tag,
            TreeKind::Binary {
                lhs,
                rhs,
                operator: None,
            },
        ));
    }

    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (flags, stats) = match &ctx.tree(tree).kind {
            TreeKind::Block { flags, stats, .. } => (*flags, stats.clone()),
            _ => panic!("expected a block"),
        };
        let stats = self.copy_list(ctx, &stats);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::BLOCK,
            TreeKind::Block {
                flags,
                stats,
                endpos: NOPOS,
            },
        ));
    }

    fn visit_break(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let label = match &ctx.tree(tree).kind {
            TreeKind::Break { label, .. } => label.clone(),
            _ => panic!("expected a break"),
        };
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::BREAK,
            TreeKind::Break {
                label,
                target: None,
            },
        ));
    }

    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, .. } => *value,
            _ => panic!("expected a yield"),
        };
        let value = self.copy(ctx, value);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::YIELD,
            TreeKind::Yield {
                value,
                target: None,
            },
        ));
    }

    fn visit_case(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (case_kind, labels, stats) = match &ctx.tree(tree).kind {
            TreeKind::Case {
                case_kind,
                labels,
                stats,
                ..
            } => (*case_kind, labels.clone(), stats.clone()),
            _ => panic!("expected a case"),
        };
        // Java: body = t.body instanceof JCExpression && t.stats.head.hasTag(YIELD)
        //               ? ((JCYield) t.stats.head).value : t.stats.head
        // (the upstream copier reads the original stats head/body, not the
        // copies).
        let orig_body = match &ctx.tree(tree).kind {
            TreeKind::Case { body, .. } => *body,
            _ => None,
        };
        let body =
            if case_kind == javac_source::tree::case_tree::CaseKind::RULE && stats.non_empty() {
                let head = *stats.head();
                let head_node = ctx.tree(head);
                if orig_body.is_some() && head_node.has_tag(Tag::YIELD) {
                    match &head_node.kind {
                        TreeKind::Yield { value, .. } => *value,
                        _ => Some(head),
                    }
                } else {
                    Some(head)
                }
            } else {
                None
            };
        let labels = self.copy_list(ctx, &labels);
        let stats = self.copy_list(ctx, &stats);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::CASE,
            TreeKind::Case {
                case_kind,
                labels,
                stats,
                body,
                completes_normally: true,
            },
        ));
    }

    fn visit_catch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (param, body) = match &ctx.tree(tree).kind {
            TreeKind::Catch { param, body } => (*param, *body),
            _ => panic!("expected a catch clause"),
        };
        let param = self.copy(ctx, param);
        let body = self.copy(ctx, body);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(ctx, pos, Tag::CATCH, TreeKind::Catch { param, body }));
    }

    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, name, typarams, extending, implementing, defs) = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl {
                mods,
                name,
                typarams,
                extending,
                implementing,
                defs,
                ..
            } => (
                *mods,
                name.clone(),
                typarams.clone(),
                *extending,
                implementing.clone(),
                defs.clone(),
            ),
            _ => panic!("expected a class declaration"),
        };
        let mods = self.copy(ctx, mods);
        let typarams = self.copy_list(ctx, &typarams);
        let extending = self.copy(ctx, extending);
        let implementing = self.copy_list(ctx, &implementing);
        let defs = self.copy_list(ctx, &defs);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::CLASSDEF,
            TreeKind::ClassDecl {
                mods,
                name,
                typarams,
                extending,
                implementing,
                permitting: List::nil(),
                defs,
                sym: None,
            },
        ));
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
        let cond = self.copy(ctx, cond);
        let truepart = self.copy(ctx, truepart);
        let falsepart = self.copy(ctx, falsepart);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::CONDEXPR,
            TreeKind::Conditional {
                cond,
                truepart,
                falsepart,
                poly_kind: PolyKind::STANDALONE,
            },
        ));
    }

    fn visit_continue(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let label = match &ctx.tree(tree).kind {
            TreeKind::Continue { label, .. } => label.clone(),
            _ => panic!("expected a continue"),
        };
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::CONTINUE,
            TreeKind::Continue {
                label,
                target: None,
            },
        ));
    }

    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, cond) = match &ctx.tree(tree).kind {
            TreeKind::DoWhileLoop { body, cond } => (*body, *cond),
            _ => panic!("expected a do loop"),
        };
        let body = self.copy(ctx, body);
        let cond = self.copy(ctx, cond);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(ctx, pos, Tag::DOLOOP, TreeKind::DoWhileLoop { body, cond }));
    }

    fn visit_erroneous(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let errs = match &ctx.tree(tree).kind {
            TreeKind::Erroneous { errs } => errs.clone(),
            _ => panic!("expected an erroneous tree"),
        };
        let errs = self.copy_list(ctx, &errs);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(ctx, pos, Tag::ERRONEOUS, TreeKind::Erroneous { errs }));
    }

    fn visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::ExpressionStatement { expr } => *expr,
            _ => panic!("expected an expression statement"),
        };
        let expr = self.copy(ctx, expr);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(ctx, pos, Tag::EXEC, TreeKind::ExpressionStatement { expr }));
    }

    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (var, expr, body) = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop { var, expr, body } => (*var, *expr, *body),
            _ => panic!("expected an enhanced for loop"),
        };
        let var = self.copy(ctx, var);
        let expr = self.copy(ctx, expr);
        let body = self.copy(ctx, body);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::FOREACHLOOP,
            TreeKind::EnhancedForLoop { var, expr, body },
        ));
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
        let init = self.copy_list(ctx, &init);
        let cond = self.copy(ctx, cond);
        let step = self.copy_list(ctx, &step);
        let body = self.copy(ctx, body);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::FORLOOP,
            TreeKind::ForLoop {
                init,
                cond,
                step,
                body,
            },
        ));
    }

    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let name = match &ctx.tree(tree).kind {
            TreeKind::Ident { name, .. } => name.clone(),
            _ => panic!("expected an identifier"),
        };
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(ctx, pos, Tag::IDENT, TreeKind::Ident { name, sym: None }));
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
        let cond = self.copy(ctx, cond);
        let thenpart = self.copy(ctx, thenpart);
        let elsepart = self.copy(ctx, elsepart);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::IF,
            TreeKind::If {
                cond,
                thenpart,
                elsepart,
            },
        ));
    }

    fn visit_import(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (qualid, static_import) = match &ctx.tree(tree).kind {
            TreeKind::Import {
                qualid,
                static_import,
                ..
            } => (*qualid, *static_import),
            _ => panic!("expected an import"),
        };
        let qualid = self.copy(ctx, qualid);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::IMPORT,
            TreeKind::Import {
                static_import,
                qualid,
                import_scope: None,
            },
        ));
    }

    fn visit_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (indexed, index) = match &ctx.tree(tree).kind {
            TreeKind::ArrayAccess { indexed, index } => (*indexed, *index),
            _ => panic!("expected an array access"),
        };
        let indexed = self.copy(ctx, indexed);
        let index = self.copy(ctx, index);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::INDEXED,
            TreeKind::ArrayAccess { indexed, index },
        ));
    }

    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (label, body) = match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement { label, body } => (label.clone(), *body),
            _ => panic!("expected a labelled statement"),
        };
        let body = self.copy(ctx, body);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::LABELLED,
            TreeKind::LabeledStatement { label, body },
        ));
    }

    fn visit_literal(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (typetag, value) = match &ctx.tree(tree).kind {
            TreeKind::Literal { typetag, value } => (*typetag, value.clone()),
            _ => panic!("expected a literal"),
        };
        let pos = ctx.tree(tree).pos;
        self.result =
            Some(self.alloc(ctx, pos, Tag::LITERAL, TreeKind::Literal { typetag, value }));
    }

    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, name, restype, typarams, recvparam, params, thrown, body, default_value) =
            match &ctx.tree(tree).kind {
                TreeKind::MethodDecl {
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
                } => (
                    *mods,
                    name.clone(),
                    *restype,
                    typarams.clone(),
                    *recvparam,
                    params.clone(),
                    thrown.clone(),
                    *body,
                    *default_value,
                ),
                _ => panic!("expected a method declaration"),
            };
        let mods = self.copy(ctx, mods);
        let restype = self.copy(ctx, restype);
        let typarams = self.copy_list(ctx, &typarams);
        let recvparam = self.copy(ctx, recvparam);
        let params = self.copy_list(ctx, &params);
        let thrown = self.copy_list(ctx, &thrown);
        let body = self.copy(ctx, body);
        let default_value = self.copy(ctx, default_value);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::METHODDEF,
            TreeKind::MethodDecl {
                mods,
                name,
                restype,
                typarams,
                recvparam,
                params,
                thrown,
                body,
                default_value,
                sym: None,
                completes_normally: true,
            },
        ));
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
        let typeargs = self.copy_list(ctx, &typeargs);
        let meth = self.copy(ctx, meth);
        let args = self.copy_list(ctx, &args);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::APPLY,
            TreeKind::MethodInvocation {
                typeargs,
                meth,
                args,
                varargs_element: None,
                poly_kind: PolyKind::STANDALONE,
            },
        ));
    }

    fn visit_modifiers(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (flags, annotations) = match &ctx.tree(tree).kind {
            TreeKind::Modifiers { flags, annotations } => (*flags, annotations.clone()),
            _ => panic!("expected modifiers"),
        };
        let annotations = self.copy_list(ctx, &annotations);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::MODIFIERS,
            TreeKind::Modifiers { flags, annotations },
        ));
    }

    fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (elemtype, dims, elems) = match &ctx.tree(tree).kind {
            TreeKind::NewArray {
                elemtype,
                dims,
                elems,
                ..
            } => (*elemtype, dims.clone(), elems.clone()),
            _ => panic!("expected a new array"),
        };
        let elemtype = self.copy(ctx, elemtype);
        let dims = self.copy_list(ctx, &dims);
        let elems = elems.map(|elems| self.copy_list(ctx, &elems));
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::NEWARRAY,
            TreeKind::NewArray {
                elemtype,
                dims,
                annotations: List::nil(),
                dim_annotations: List::nil(),
                elems,
            },
        ));
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
        let encl = self.copy(ctx, encl);
        let typeargs = self.copy_list(ctx, &typeargs);
        let clazz = self.copy(ctx, clazz);
        let args = self.copy_list(ctx, &args);
        let def = self.copy(ctx, def);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::NEWCLASS,
            TreeKind::NewClass {
                encl,
                typeargs,
                clazz,
                args,
                def,
                constructor: None,
                varargs_element: None,
                constructor_type: None,
                poly_kind: PolyKind::STANDALONE,
            },
        ));
    }

    fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (params, body) = match &ctx.tree(tree).kind {
            TreeKind::Lambda { params, body, .. } => (params.clone(), *body),
            _ => panic!("expected a lambda"),
        };
        let params = self.copy_list(ctx, &params);
        let body = self.copy(ctx, body);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::LAMBDA,
            TreeKind::Lambda {
                params,
                body,
                can_complete_normally: true,
                param_kind: ParameterKind::IMPLICIT,
                poly_kind: PolyKind::STANDALONE,
                target: None,
            },
        ));
    }

    fn visit_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Parens { expr } => *expr,
            _ => panic!("expected parens"),
        };
        let expr = self.copy(ctx, expr);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(ctx, pos, Tag::PARENS, TreeKind::Parens { expr }));
    }

    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Return { expr } => *expr,
            _ => panic!("expected a return"),
        };
        let expr = self.copy(ctx, expr);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(ctx, pos, Tag::RETURN, TreeKind::Return { expr }));
    }

    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selected, name) = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { selected, name, .. } => (*selected, name.clone()),
            _ => panic!("expected a field access"),
        };
        let selected = self.copy(ctx, selected);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::SELECT,
            TreeKind::FieldAccess {
                selected,
                name,
                sym: None,
            },
        ));
    }

    fn visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mode, name, expr, typeargs) = match &ctx.tree(tree).kind {
            TreeKind::MemberReference {
                mode,
                name,
                expr,
                typeargs,
                ..
            } => (*mode, name.clone(), *expr, typeargs.clone()),
            _ => panic!("expected a member reference"),
        };
        let expr = self.copy(ctx, expr);
        let typeargs = self.copy_list_opt(ctx, typeargs);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::REFERENCE,
            TreeKind::MemberReference {
                mode,
                kind: None,
                name,
                expr,
                typeargs,
                sym: None,
                varargs_element: None,
                ref_poly_kind: PolyKind::STANDALONE,
                owner_accessible: false,
                referent_type: None,
                poly_kind: PolyKind::STANDALONE,
                target: None,
            },
        ));
    }

    fn visit_skip(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(ctx, pos, Tag::SKIP, TreeKind::Skip {}));
    }

    fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::Switch {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch"),
        };
        let selector = self.copy(ctx, selector);
        let cases = self.copy_list(ctx, &cases);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::SWITCH,
            TreeKind::Switch {
                selector,
                cases,
                endpos: NOPOS,
                has_total_pattern: false,
                pattern_switch: false,
            },
        ));
    }

    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch expression"),
        };
        let selector = self.copy(ctx, selector);
        let cases = self.copy_list(ctx, &cases);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::SWITCH_EXPRESSION,
            TreeKind::SwitchExpression {
                selector,
                cases,
                endpos: NOPOS,
                has_total_pattern: false,
                pattern_switch: false,
                poly_kind: PolyKind::STANDALONE,
            },
        ));
    }

    fn visit_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lock, body) = match &ctx.tree(tree).kind {
            TreeKind::Synchronized { lock, body } => (*lock, *body),
            _ => panic!("expected a synchronized statement"),
        };
        let lock = self.copy(ctx, lock);
        let body = self.copy(ctx, body);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::SYNCHRONIZED,
            TreeKind::Synchronized { lock, body },
        ));
    }

    fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Throw { expr } => *expr,
            _ => panic!("expected a throw"),
        };
        let expr = self.copy(ctx, expr);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(ctx, pos, Tag::THROW, TreeKind::Throw { expr }));
    }

    fn visit_top_level(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let defs = match &ctx.tree(tree).kind {
            TreeKind::CompilationUnit { defs, .. } => defs.clone(),
            _ => panic!("expected a compilation unit"),
        };
        let defs = self.copy_list(ctx, &defs);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::TOPLEVEL,
            TreeKind::CompilationUnit {
                defs,
                sourcefile: None,
                modle: None,
                locn: None,
                packge: None,
                toplevel_scope: None,
                named_import_scope: None,
                star_import_scope: None,
                line_map: None,
                doc_comments: None,
                end_positions: None,
            },
        ));
    }

    fn visit_package_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, pid) = match &ctx.tree(tree).kind {
            TreeKind::PackageDecl {
                annotations, pid, ..
            } => (annotations.clone(), *pid),
            _ => panic!("expected a package declaration"),
        };
        let annotations = self.copy_list(ctx, &annotations);
        let pid = self.copy(ctx, pid);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::PACKAGEDEF,
            TreeKind::PackageDecl {
                annotations,
                pid,
                packge: None,
            },
        ));
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
        let resources = self.copy_list(ctx, &resources);
        let body = self.copy(ctx, body);
        let catchers = self.copy_list(ctx, &catchers);
        let finalizer = self.copy(ctx, finalizer);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::TRY,
            TreeKind::Try {
                body,
                catchers,
                finalizer,
                resources,
                finally_can_complete_normally: true,
            },
        ));
    }

    fn visit_type_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, arguments) = match &ctx.tree(tree).kind {
            TreeKind::TypeApply { clazz, arguments } => (*clazz, arguments.clone()),
            _ => panic!("expected a parameterized type"),
        };
        let clazz = self.copy(ctx, clazz);
        let arguments = self.copy_list(ctx, &arguments);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::TYPEAPPLY,
            TreeKind::TypeApply { clazz, arguments },
        ));
    }

    fn visit_type_union(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let alternatives = match &ctx.tree(tree).kind {
            TreeKind::TypeUnion { alternatives } => alternatives.clone(),
            _ => panic!("expected a union type"),
        };
        let alternatives = self.copy_list(ctx, &alternatives);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::TYPEUNION,
            TreeKind::TypeUnion { alternatives },
        ));
    }

    fn visit_type_intersection(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let bounds = match &ctx.tree(tree).kind {
            TreeKind::TypeIntersection { bounds } => bounds.clone(),
            _ => panic!("expected an intersection type"),
        };
        let bounds = self.copy_list(ctx, &bounds);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::TYPEINTERSECTION,
            TreeKind::TypeIntersection { bounds },
        ));
    }

    fn visit_type_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let elemtype = match &ctx.tree(tree).kind {
            TreeKind::ArrayTypeTree { elemtype } => *elemtype,
            _ => panic!("expected an array type"),
        };
        let elemtype = self.copy(ctx, elemtype);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::TYPEARRAY,
            TreeKind::ArrayTypeTree { elemtype },
        ));
    }

    fn visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, expr) = match &ctx.tree(tree).kind {
            TreeKind::TypeCast { clazz, expr } => (*clazz, *expr),
            _ => panic!("expected a type cast"),
        };
        let clazz = self.copy(ctx, clazz);
        let expr = self.copy(ctx, expr);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(ctx, pos, Tag::TYPECAST, TreeKind::TypeCast { clazz, expr }));
    }

    fn visit_type_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let typetag = match &ctx.tree(tree).kind {
            TreeKind::PrimitiveTypeTree { typetag } => *typetag,
            _ => panic!("expected a primitive type"),
        };
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::TYPEIDENT,
            TreeKind::PrimitiveTypeTree { typetag },
        ));
    }

    fn visit_type_parameter(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (name, bounds, annotations) = match &ctx.tree(tree).kind {
            TreeKind::TypeParameter {
                name,
                bounds,
                annotations,
            } => (name.clone(), bounds.clone(), annotations.clone()),
            _ => panic!("expected a type parameter"),
        };
        let bounds = self.copy_list(ctx, &bounds);
        let annotations = self.copy_list(ctx, &annotations);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::TYPEPARAMETER,
            TreeKind::TypeParameter {
                name,
                bounds,
                annotations,
            },
        ));
    }

    fn visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (expr, pattern) = match &ctx.tree(tree).kind {
            TreeKind::InstanceOf { expr, pattern } => (*expr, *pattern),
            _ => panic!("expected an instanceof"),
        };
        let expr = self.copy(ctx, expr);
        let pattern = self.copy(ctx, pattern);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::TYPETEST,
            TreeKind::InstanceOf { expr, pattern },
        ));
    }

    fn visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let var = match &ctx.tree(tree).kind {
            TreeKind::BindingPattern { var } => *var,
            _ => panic!("expected a binding pattern"),
        };
        let var = self.copy(ctx, var);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::BINDINGPATTERN,
            TreeKind::BindingPattern { var },
        ));
    }

    fn visit_guard_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (patt, expr) = match &ctx.tree(tree).kind {
            TreeKind::GuardPattern { patt, expr } => (*patt, *expr),
            _ => panic!("expected a guard pattern"),
        };
        let patt = self.copy(ctx, patt);
        let expr = self.copy(ctx, expr);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::GUARDPATTERN,
            TreeKind::GuardPattern { patt, expr },
        ));
    }

    fn visit_parenthesized_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let pattern = match &ctx.tree(tree).kind {
            TreeKind::ParenthesizedPattern { pattern } => *pattern,
            _ => panic!("expected a parenthesized pattern"),
        };
        let pattern = self.copy(ctx, pattern);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::PARENTHESIZEDPATTERN,
            TreeKind::ParenthesizedPattern { pattern },
        ));
    }

    fn visit_default_case_label(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::DEFAULTCASELABEL,
            TreeKind::DefaultCaseLabel {},
        ));
    }

    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (arg, tag) = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, .. } => (*arg, ctx.tree(tree).tag),
            _ => panic!("expected a unary expression"),
        };
        let arg = self.copy(ctx, arg);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            tag,
            TreeKind::Unary {
                arg,
                operator: None,
            },
        ));
    }

    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, name, nameexpr, vartype, init) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl {
                mods,
                name,
                nameexpr,
                vartype,
                init,
                ..
            } => (*mods, name.clone(), *nameexpr, *vartype, *init),
            _ => panic!("expected a variable declaration"),
        };
        let mods = self.copy(ctx, mods);
        let vartype = self.copy(ctx, vartype);
        let pos = ctx.tree(tree).pos;
        if nameexpr.is_none() {
            let init = self.copy(ctx, init);
            self.result = Some(self.alloc(
                ctx,
                pos,
                Tag::VARDEF,
                TreeKind::VariableDecl {
                    mods,
                    name,
                    nameexpr: None,
                    vartype,
                    init,
                    sym: None,
                    start_pos: NOPOS,
                },
            ));
        } else {
            let nameexpr = self.copy(ctx, nameexpr);
            // Java: ReceiverVarDef(mods, nameexpr, vartype) - the upstream
            // receiver variable has a null name; the port keeps name (the field
            // cannot be null, registered in PORTING_RULES section 9).
            self.result = Some(self.alloc(
                ctx,
                pos,
                Tag::VARDEF,
                TreeKind::VariableDecl {
                    mods,
                    name,
                    nameexpr,
                    vartype,
                    init: None,
                    sym: None,
                    start_pos: NOPOS,
                },
            ));
        }
    }

    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, body) = match &ctx.tree(tree).kind {
            TreeKind::WhileLoop { cond, body } => (*cond, *body),
            _ => panic!("expected a while loop"),
        };
        let cond = self.copy(ctx, cond);
        let body = self.copy(ctx, body);
        let pos = ctx.tree(tree).pos;
        self.result =
            Some(self.alloc(ctx, pos, Tag::WHILELOOP, TreeKind::WhileLoop { cond, body }));
    }

    fn visit_wildcard(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (kind, inner) = match &ctx.tree(tree).kind {
            TreeKind::Wildcard { kind, inner } => (*kind, *inner),
            _ => panic!("expected a wildcard"),
        };
        let kind = kind.map(|kind_tree| {
            let (kind_pos, bound_kind) = match &ctx.tree(kind_tree).kind {
                TreeKind::TypeBoundKind { kind } => (ctx.tree(kind_tree).pos, *kind),
                _ => panic!("expected a type bound kind"),
            };
            self.alloc(
                ctx,
                kind_pos,
                Tag::TYPEBOUNDKIND,
                TreeKind::TypeBoundKind { kind: bound_kind },
            )
        });
        let inner = self.copy(ctx, inner);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(ctx, pos, Tag::WILDCARD, TreeKind::Wildcard { kind, inner }));
    }

    fn visit_module_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, kind, qual_id, directives) = match &ctx.tree(tree).kind {
            TreeKind::ModuleDecl {
                mods,
                kind,
                qual_id,
                directives,
                ..
            } => (*mods, *kind, *qual_id, directives.clone()),
            _ => panic!("expected a module declaration"),
        };
        let mods = self.copy(ctx, mods);
        let qual_id = self.copy(ctx, qual_id);
        let directives = self.copy_list_opt(ctx, directives);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::MODULEDEF,
            TreeKind::ModuleDecl {
                mods,
                ty: None,
                kind,
                qual_id,
                directives,
                sym: None,
            },
        ));
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
        let qualid = self.copy(ctx, qualid);
        let module_names = self.copy_list_opt(ctx, module_names);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::EXPORTS,
            TreeKind::Exports {
                qualid,
                module_names,
                directive: None,
            },
        ));
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
        let qualid = self.copy(ctx, qualid);
        let module_names = self.copy_list_opt(ctx, module_names);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::OPENS,
            TreeKind::Opens {
                qualid,
                module_names,
                directive: None,
            },
        ));
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
        let service_name = self.copy(ctx, service_name);
        let impl_names = self.copy_list(ctx, &impl_names);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::PROVIDES,
            TreeKind::Provides {
                service_name,
                impl_names,
            },
        ));
    }

    fn visit_requires(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (is_transitive, is_static_phase, module_name) = match &ctx.tree(tree).kind {
            TreeKind::Requires {
                is_transitive,
                is_static_phase,
                module_name,
                ..
            } => (*is_transitive, *is_static_phase, *module_name),
            _ => panic!("expected a requires directive"),
        };
        let module_name = self.copy(ctx, module_name);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::REQUIRES,
            TreeKind::Requires {
                is_transitive,
                is_static_phase,
                module_name,
                directive: None,
            },
        ));
    }

    fn visit_uses(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Uses { qualid } => *qualid,
            _ => panic!("expected a uses directive"),
        };
        let qualid = self.copy(ctx, qualid);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(ctx, pos, Tag::USES, TreeKind::Uses { qualid }));
    }

    fn visit_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (defs, expr, needs_cond) = match &ctx.tree(tree).kind {
            TreeKind::LetExpr {
                defs,
                expr,
                needs_cond,
            } => (defs.clone(), *expr, *needs_cond),
            _ => panic!("expected a let expression"),
        };
        let defs = self.copy_list(ctx, &defs);
        let expr = self.copy(ctx, expr);
        let pos = ctx.tree(tree).pos;
        self.result = Some(self.alloc(
            ctx,
            pos,
            Tag::LETEXPR,
            TreeKind::LetExpr {
                defs,
                expr,
                needs_cond,
            },
        ));
    }

    fn visit_tree(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        panic!("AssertionError: unknown tree tag: {:?}", tree);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::jc_tree::JCTree;
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

    #[test]
    fn copy_deep_copies_and_shares_names() {
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
            7,
        );
        let mut copier = TreeCopier::new();
        let copied = copier
            .copy(&mut ctx, Some(binary))
            .expect("copy produced a node");
        assert_ne!(copied, binary);
        let copied_node = ctx.tree(copied);
        assert_eq!(copied_node.pos, 7);
        assert_eq!(copied_node.tag, Tag::PLUS);
        let (lhs, rhs) = match &copied_node.kind {
            TreeKind::Binary { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a binary expression"),
        };
        assert_ne!(lhs, Some(a));
        assert_ne!(rhs, Some(b));
        // The name is shared with the original (Java: Names are shared).
        match (&ctx.tree(lhs.expect("copied lhs")).kind, &ctx.tree(a).kind) {
            (
                TreeKind::Ident {
                    name: copied_name, ..
                },
                TreeKind::Ident {
                    name: original_name,
                    ..
                },
            ) => {
                assert_eq!(
                    copied_name.to_java_string().to_string_lossy(),
                    original_name.to_java_string().to_string_lossy()
                );
            }
            _ => panic!("expected identifiers"),
        }
        // Mutating the copy leaves the original untouched.
        let new_a = ident(&mut ctx, &table, "c");
        if let TreeKind::Binary { lhs: slot, .. } = &mut ctx.tree_mut(copied).kind {
            *slot = Some(new_a);
        }
        match &ctx.tree(binary).kind {
            TreeKind::Binary { lhs, .. } => assert_eq!(*lhs, Some(a)),
            _ => panic!("expected a binary expression"),
        }
    }

    #[test]
    fn copy_class_with_annotations_and_method() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let annotation_type = ident(&mut ctx, &table, "A");
        let annotation = tree(
            &mut ctx,
            Tag::ANNOTATION,
            TreeKind::Annotation {
                annotation_type: Some(annotation_type),
                args: empty(),
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
        let method = tree(
            &mut ctx,
            Tag::METHODDEF,
            TreeKind::MethodDecl {
                mods: None,
                name: make_name(&table, "m"),
                restype: None,
                typarams: empty(),
                recvparam: None,
                params: empty(),
                thrown: empty(),
                body: None,
                default_value: None,
                sym: None,
                completes_normally: true,
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
                defs: List::of(method),
                sym: None,
            },
            3,
        );
        let mut copier = TreeCopier::new();
        let copied = copier
            .copy(&mut ctx, Some(class))
            .expect("copy produced a node");
        assert_ne!(copied, class);
        let (copied_mods, copied_defs) = match &ctx.tree(copied).kind {
            TreeKind::ClassDecl { mods, defs, .. } => (*mods, defs.clone()),
            _ => panic!("expected a class declaration"),
        };
        let copied_mods = copied_mods.expect("copied mods");
        assert_ne!(copied_mods, mods);
        let copied_annotations = match &ctx.tree(copied_mods).kind {
            TreeKind::Modifiers { annotations, .. } => annotations.clone(),
            _ => panic!("expected modifiers"),
        };
        assert_eq!(copied_annotations.length(), 1);
        assert_ne!(copied_annotations.get(0), annotation);
        assert_eq!(copied_defs.length(), 1);
        assert_ne!(copied_defs.get(0), method);
        // Copying a list preserves the order and produces fresh nodes.
        let list = List::of2(annotation, method);
        let copied_list = copier.copy_list(&mut ctx, &list);
        assert_eq!(copied_list.length(), 2);
        assert_ne!(copied_list.get(0), copied_list.get(1));
    }

    #[test]
    fn copy_lambda_and_case() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let body = ident(&mut ctx, &table, "x");
        let param = tree(
            &mut ctx,
            Tag::VARDEF,
            TreeKind::VariableDecl {
                mods: None,
                name: make_name(&table, "p"),
                nameexpr: None,
                vartype: None,
                init: None,
                sym: None,
                start_pos: crate::util::position::NOPOS,
            },
            0,
        );
        let lambda = tree(
            &mut ctx,
            Tag::LAMBDA,
            TreeKind::Lambda {
                params: List::of(param),
                body: Some(body),
                can_complete_normally: true,
                param_kind: crate::tree::jc_tree::ParameterKind::EXPLICIT,
                poly_kind: PolyKind::POLY,
                target: None,
            },
            0,
        );
        let mut copier = TreeCopier::new();
        let copied_lambda = copier
            .copy(&mut ctx, Some(lambda))
            .expect("copy produced a node");
        assert_ne!(copied_lambda, lambda);
        match &ctx.tree(copied_lambda).kind {
            TreeKind::Lambda {
                params,
                body: copied_body,
                param_kind,
                poly_kind,
                ..
            } => {
                assert_eq!(*param_kind, crate::tree::jc_tree::ParameterKind::IMPLICIT);
                assert_eq!(*poly_kind, PolyKind::STANDALONE);
                assert_eq!(params.length(), 1);
                assert_ne!(params.get(0), param);
                assert_ne!(*copied_body, Some(body));
            }
            _ => panic!("expected a lambda"),
        }
        // Case with rule kind keeps the body computed from the statement.
        let exec = tree(
            &mut ctx,
            Tag::EXEC,
            TreeKind::ExpressionStatement { expr: Some(body) },
            0,
        );
        let case = tree(
            &mut ctx,
            Tag::CASE,
            TreeKind::Case {
                case_kind: javac_source::tree::case_tree::CaseKind::RULE,
                labels: empty(),
                stats: List::of(exec),
                body: None,
                completes_normally: true,
            },
            0,
        );
        let copied_case = copier
            .copy(&mut ctx, Some(case))
            .expect("copy produced a node");
        assert_ne!(copied_case, case);
        match &ctx.tree(copied_case).kind {
            TreeKind::Case {
                case_kind,
                stats,
                body,
                ..
            } => {
                assert_eq!(*case_kind, javac_source::tree::case_tree::CaseKind::RULE);
                assert_eq!(stats.length(), 1);
                assert_ne!(stats.get(0), exec);
                assert!(body.is_some());
            }
            _ => panic!("expected a case"),
        }
    }

    #[test]
    fn copy_compilation_unit_drops_metadata() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let package_name = ident(&mut ctx, &table, "p");
        let import = tree(
            &mut ctx,
            Tag::IMPORT,
            TreeKind::Import {
                static_import: false,
                qualid: Some(package_name),
                import_scope: None,
            },
            0,
        );
        let unit = tree(
            &mut ctx,
            Tag::TOPLEVEL,
            TreeKind::CompilationUnit {
                defs: List::of(import),
                sourcefile: None,
                modle: None,
                locn: None,
                packge: None,
                toplevel_scope: None,
                named_import_scope: None,
                star_import_scope: None,
                line_map: None,
                doc_comments: None,
                end_positions: None,
            },
            0,
        );
        let mut copier = TreeCopier::new();
        let copied = copier
            .copy(&mut ctx, Some(unit))
            .expect("copy produced a node");
        assert_ne!(copied, unit);
        match &ctx.tree(copied).kind {
            TreeKind::CompilationUnit {
                defs, sourcefile, ..
            } => {
                assert_eq!(defs.length(), 1);
                assert_ne!(defs.get(0), import);
                assert!(sourcefile.is_none());
            }
            _ => panic!("expected a compilation unit"),
        }
    }
}
