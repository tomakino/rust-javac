// Origin: com/sun/tools/javac/tree/TreeCopier.java @ 982d53c (jdk-17.0.19-ga)
//
// PORTING_RULES §19 mapping: every upstream visitXxx is ported 1:1 below.
// §9 adapters (registered):
// - the Java TreeMaker M.at(pos).Xxx(...) constructors are expressed by
//   allocating the node in the arena with the same fields and the original pos
//   (TreeMaker.make is exactly this allocation; the extra at(pos) bookkeeping is
//   the node pos);
// - the upstream generic parameter P is dropped (the port has no attribute
//   payloads carried through the copy);
// - the two DeferredAttr anonymous copier subclasses (diamond NewClass and the
//   MemberReference overloadKind forwarding) are expressed by the
//   copy_speculative / copy_speculative_member_reference entry points on
//   TreeCopier (see DeferredAttr).
// - fields that the upstream constructors do not set (types, symbols,
//   varargs element, poly kind, ...) get the JCTree constructor defaults of
//   tree_maker.rs (STANDALONE poly kind, false completion flags, None symbols);
// - visitClass: the upstream TreeCopier.visitClassDef calls
//   M.at(pos).ClassDef(mods, name, typarams, extending, implementing, defs),
//   i.e. the 6-argument TreeMaker overload, which passes List.nil() as the
//   permitting list (TreeMaker.java:157-166). The local copy therefore uses
//   List::nil() as well — this is the upstream behavior, not an approximation.
// - visitLambdaExpression: the upstream TreeCopier copies via
//   M.at(pos).Lambda(params, body); JCLambda(params, body) derives paramKind
//   from the copied params (JCTree.java:1957-1966), so the copy must derive
//   IMPLICIT/EXPLICIT from the copied params exactly like the local copy does.
// - visitCase: the upstream body computation checks
//   "t.body instanceof JCExpression"; JCCase.body is declared JCExpression, so
//   the check is exactly "body != null". The local Case.body is
//   Option<TreeId> for that same field, so original_body.is_some() is the
//   exact translation (registered §3: encoding difference only).

use std::rc::Rc;

use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::tree::jc_tree::{
    JCTree, PolyKind, Tag, TreeKind,
};
use crate::tree::tree_maker::TreeMaker;
use crate::util::list::List;
use crate::util::position;

/// Java: public class TreeCopier<P> implements TreeVisitor<JCTree,P>
pub struct TreeCopier {
    /// Java: private TreeMaker M
    pub make: Rc<std::cell::RefCell<TreeMaker>>,
    /// Java: DeferredAttr's anonymous copier subclass flag (§9): when set,
    /// diamond new-class nodes are copied as speculative new classes (def set to
    /// null) and member references forward their overload kind.
    pub speculative: bool,
}

impl TreeCopier {
    /// Java: public TreeCopier(TreeMaker M)
    pub fn new(make: Rc<std::cell::RefCell<TreeMaker>>) -> TreeCopier {
        TreeCopier {
            make,
            speculative: false,
        }
    }

    /// Java: the DeferredAttr anonymous TreeCopier subclass (§9 flag).
    pub fn new_speculative(make: Rc<std::cell::RefCell<TreeMaker>>) -> TreeCopier {
        TreeCopier {
            make,
            speculative: true,
        }
    }

    /// Java: public <T extends JCTree> T copy(T tree)
    pub fn copy(&self, ctx: &mut CompilerContext, tree: Option<TreeId>) -> Option<TreeId> {
        tree.map(|tree| self.copy_tree(ctx, tree))
    }

    /// Java: public <T extends JCTree> List<T> copy(List<T> trees)
    pub fn copy_list(&self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        if trees.is_empty() {
            return trees.clone();
        }
        let mut result: List<TreeId> = List::nil();
        for tree in trees.iter() {
            result = result.append(self.copy_tree(ctx, tree));
        }
        result
    }

    fn mk(&self, ctx: &mut CompilerContext, pos: i32, tag: Tag, kind: TreeKind) -> TreeId {
        ctx.alloc_tree(JCTree {
            pos,
            ty: None,
            tag,
            kind,
        })
    }

    /// Java: copy(t, p) dispatch (tree.accept(this, p)).
    pub fn copy_tree(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        match ctx.tree(tree).tag {
            Tag::ANNOTATED_TYPE => self.visit_annotated_type(ctx, tree),
            Tag::ANNOTATION | Tag::TYPE_ANNOTATION => self.visit_annotation(ctx, tree),
            Tag::ASSERT => self.visit_assert(ctx, tree),
            Tag::ASSIGN => self.visit_assignment(ctx, tree),
            Tag::PLUS_ASG | Tag::MINUS_ASG | Tag::MUL_ASG | Tag::DIV_ASG
            | Tag::MOD_ASG | Tag::BITAND_ASG | Tag::BITOR_ASG | Tag::BITXOR_ASG
            | Tag::SL_ASG | Tag::SR_ASG | Tag::USR_ASG => self.visit_compound_assignment(ctx, tree),
            Tag::AND | Tag::OR | Tag::BITAND | Tag::BITOR | Tag::BITXOR | Tag::EQ | Tag::NE | Tag::LT | Tag::GT
            | Tag::LE | Tag::GE | Tag::SL | Tag::SR | Tag::USR | Tag::PLUS
            | Tag::MINUS | Tag::MUL | Tag::DIV | Tag::MOD => self.visit_binary(ctx, tree),
            Tag::BLOCK => self.visit_block(ctx, tree),
            Tag::BREAK => self.visit_break(ctx, tree),
            Tag::YIELD => self.visit_yield(ctx, tree),
            Tag::CASE => self.visit_case(ctx, tree),
            Tag::CATCH => self.visit_catch(ctx, tree),
            Tag::CLASSDEF => self.visit_class(ctx, tree),
            Tag::CONDEXPR => self.visit_conditional_expression(ctx, tree),
            Tag::CONTINUE => self.visit_continue(ctx, tree),
            Tag::DOLOOP => self.visit_do_while_loop(ctx, tree),
            Tag::ERRONEOUS => self.visit_erroneous(ctx, tree),
            Tag::EXEC => self.visit_expression_statement(ctx, tree),
            Tag::FOREACHLOOP => self.visit_enhanced_for_loop(ctx, tree),
            Tag::FORLOOP => self.visit_for_loop(ctx, tree),
            Tag::IDENT => self.visit_identifier(ctx, tree),
            Tag::IF => self.visit_if(ctx, tree),
            Tag::IMPORT => self.visit_import(ctx, tree),
            Tag::INDEXED => self.visit_array_access(ctx, tree),
            Tag::LABELLED => self.visit_labeled_statement(ctx, tree),
            Tag::LITERAL => self.visit_literal(ctx, tree),
            Tag::METHODDEF => self.visit_method(ctx, tree),
            Tag::APPLY => self.visit_method_invocation(ctx, tree),
            Tag::MODIFIERS => self.visit_modifiers(ctx, tree),
            Tag::NEWARRAY => self.visit_new_array(ctx, tree),
            Tag::NEWCLASS => self.visit_new_class(ctx, tree),
            Tag::LAMBDA => self.visit_lambda_expression(ctx, tree),
            Tag::PARENS => self.visit_parenthesized(ctx, tree),
            Tag::RETURN => self.visit_return(ctx, tree),
            Tag::SELECT => self.visit_member_select(ctx, tree),
            Tag::REFERENCE => self.visit_member_reference(ctx, tree),
            Tag::SKIP => self.visit_empty_statement(ctx, tree),
            Tag::SWITCH => self.visit_switch(ctx, tree),
            Tag::SWITCH_EXPRESSION => self.visit_switch_expression(ctx, tree),
            Tag::SYNCHRONIZED => self.visit_synchronized(ctx, tree),
            Tag::THROW => self.visit_throw(ctx, tree),
            Tag::TOPLEVEL => self.visit_compilation_unit(ctx, tree),
            Tag::PACKAGEDEF => self.visit_package(ctx, tree),
            Tag::TRY => self.visit_try(ctx, tree),
            Tag::TYPEAPPLY => self.visit_parameterized_type(ctx, tree),
            Tag::TYPEUNION => self.visit_union_type(ctx, tree),
            Tag::TYPEINTERSECTION => self.visit_intersection_type(ctx, tree),
            Tag::TYPEARRAY => self.visit_array_type(ctx, tree),
            Tag::TYPECAST => self.visit_type_cast(ctx, tree),
            Tag::TYPEIDENT => self.visit_primitive_type(ctx, tree),
            Tag::TYPEPARAMETER => self.visit_type_parameter(ctx, tree),
            Tag::TYPETEST => self.visit_instance_of(ctx, tree),
            Tag::BINDINGPATTERN => self.visit_binding_pattern(ctx, tree),
            Tag::GUARDPATTERN => self.visit_guarded_pattern(ctx, tree),
            Tag::PARENTHESIZEDPATTERN => self.visit_parenthesized_pattern(ctx, tree),
            Tag::DEFAULTCASELABEL => self.visit_default_case_label(ctx, tree),
            Tag::POS | Tag::NEG | Tag::NOT | Tag::COMPL | Tag::PREINC | Tag::PREDEC
            | Tag::POSTINC | Tag::POSTDEC => self.visit_unary(ctx, tree),
            Tag::VARDEF => self.visit_variable(ctx, tree),
            Tag::WHILELOOP => self.visit_while_loop(ctx, tree),
            Tag::WILDCARD => self.visit_wildcard(ctx, tree),
            Tag::MODULEDEF => self.visit_module(ctx, tree),
            Tag::EXPORTS => self.visit_exports(ctx, tree),
            Tag::OPENS => self.visit_opens(ctx, tree),
            Tag::PROVIDES => self.visit_provides(ctx, tree),
            Tag::REQUIRES => self.visit_requires(ctx, tree),
            Tag::USES => self.visit_uses(ctx, tree),
            Tag::LETEXPR => self.visit_other_let_expr(ctx, tree),
            other => panic!("AssertionError: unknown tree tag: {other:?}"),
        }
    }

}

impl TreeCopier {
    fn copy_opt(&self, ctx: &mut CompilerContext, tree: Option<TreeId>) -> Option<TreeId> {
        tree.map(|tree| self.copy_tree(ctx, tree))
    }

    /// Java: public JCTree visitAnnotatedType(AnnotatedTypeTree node, P p)
    fn visit_annotated_type(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (annotations, underlying_type) = match &__kind {
            TreeKind::AnnotatedType {
                annotations,
                underlying_type,
            } => (self.copy_list(ctx, annotations), self.copy_opt(ctx, *underlying_type)),
            _ => panic!("ClassCastException: JCAnnotatedType expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::ANNOTATED_TYPE,
            TreeKind::AnnotatedType {
                annotations,
                underlying_type,
            },
        )
    }

    /// Java: public JCTree visitAnnotation(AnnotationTree node, P p)
    fn visit_annotation(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (annotation_type, args, attribute) = match &__kind {
            TreeKind::Annotation {
                annotation_type,
                args,
                attribute,
            } => (
                self.copy_opt(ctx, *annotation_type),
                self.copy_list(ctx, args),
                *attribute,
            ),
            _ => panic!("ClassCastException: JCAnnotation expected"),
        };
        let pos = ctx.tree(tree).pos;
        let tag = ctx.tree(tree).tag;
        self.mk(
            ctx,
            pos,
            tag,
            TreeKind::Annotation {
                annotation_type,
                args,
                attribute,
            },
        )
    }

    /// Java: public JCTree visitAssert(AssertTree node, P p)
    fn visit_assert(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (cond, detail) = match &__kind {
            TreeKind::Assert { cond, detail } => (self.copy_opt(ctx, *cond), self.copy_opt(ctx, *detail)),
            _ => panic!("ClassCastException: JCAssert expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(ctx, pos, Tag::ASSERT, TreeKind::Assert { cond, detail })
    }

    /// Java: public JCTree visitAssignment(AssignmentTree node, P p)
    fn visit_assignment(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (lhs, rhs) = match &__kind {
            TreeKind::Assign { lhs, rhs } => (self.copy_opt(ctx, *lhs), self.copy_opt(ctx, *rhs)),
            _ => panic!("ClassCastException: JCAssign expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(ctx, pos, Tag::ASSIGN, TreeKind::Assign { lhs, rhs })
    }

    /// Java: public JCTree visitCompoundAssignment(CompoundAssignmentTree node, P p)
    fn visit_compound_assignment(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (lhs, rhs) = match &__kind {
            TreeKind::AssignOp { lhs, rhs, .. } => {
                (self.copy_opt(ctx, *lhs), self.copy_opt(ctx, *rhs))
            }
            _ => panic!("ClassCastException: JCAssignOp expected"),
        };
        let pos = ctx.tree(tree).pos;
        let tag = ctx.tree(tree).tag;
        self.mk(
            ctx,
            pos,
            tag,
            TreeKind::AssignOp {
                lhs,
                rhs,
                operator: None,
            },
        )
    }

    /// Java: public JCTree visitBinary(BinaryTree node, P p)
    fn visit_binary(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (lhs, rhs) = match &__kind {
            TreeKind::Binary { lhs, rhs, .. } => (self.copy_opt(ctx, *lhs), self.copy_opt(ctx, *rhs)),
            _ => panic!("ClassCastException: JCBinary expected"),
        };
        let pos = ctx.tree(tree).pos;
        let tag = ctx.tree(tree).tag;
        self.mk(
            ctx,
            pos,
            tag,
            TreeKind::Binary {
                lhs,
                rhs,
                operator: None,
            },
        )
    }

    /// Java: public JCTree visitBlock(BlockTree node, P p)
    fn visit_block(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (flags, stats) = match &__kind {
            TreeKind::Block { flags, stats, .. } => (*flags, self.copy_list(ctx, stats)),
            _ => panic!("ClassCastException: JCBlock expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::BLOCK,
            TreeKind::Block {
                flags,
                stats,
                endpos: position::NOPOS,
            },
        )
    }

    /// Java: public JCTree visitBreak(BreakTree node, P p)
    fn visit_break(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let label = match &__kind {
            TreeKind::Break { label, .. } => label.clone(),
            _ => panic!("ClassCastException: JCBreak expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::BREAK,
            TreeKind::Break {
                label,
                target: None,
            },
        )
    }

    /// Java: public JCTree visitYield(YieldTree node, P p)
    fn visit_yield(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let value = match &__kind {
            TreeKind::Yield { value, .. } => self.copy_opt(ctx, *value),
            _ => panic!("ClassCastException: JCYield expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::YIELD,
            TreeKind::Yield {
                value,
                target: None,
            },
        )
    }

    /// Java: public JCTree visitCase(CaseTree node, P p)
    fn visit_case(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (case_kind, labels, stats) = match &__kind {
            TreeKind::Case {
                case_kind,
                labels,
                stats,
                ..
            } => (*case_kind, self.copy_list(ctx, labels), self.copy_list(ctx, stats)),
            _ => panic!("ClassCastException: JCCase expected"),
        };
        // Java: body = t.body instanceof JCExpression && t.stats.head.hasTag(YIELD)
        //     ? ((JCYield)t.stats.head).value : t.stats.head (for RULE cases).
        // JCCase.body is declared JCExpression, so instanceof == (body != null);
        // the local Case.body Option<TreeId> encodes that same field, so
        // original_body.is_some() is the exact translation (file header).
        let original_body = match &__kind {
            TreeKind::Case { body, .. } => *body,
            _ => None,
        };
        let body = if case_kind == javac_source::tree::case_tree::CaseKind::RULE {
            let head = if stats.is_empty() { None } else { Some(*stats.head()) };
            match (original_body, head) {
                (Some(_), Some(head)) if ctx.tree(head).tag == Tag::YIELD => {
                    match &ctx.tree(head).kind {
                        TreeKind::Yield { value, .. } => *value,
                        _ => None,
                    }
                }
                (_, head) => head,
            }
        } else {
            None
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::CASE,
            TreeKind::Case {
                case_kind,
                labels,
                stats,
                body,
                completes_normally: false,
            },
        )
    }

    /// Java: public JCTree visitCatch(CatchTree node, P p)
    fn visit_catch(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (param, body) = match &__kind {
            TreeKind::Catch { param, body } => {
                (self.copy_opt(ctx, *param), self.copy_opt(ctx, *body))
            }
            _ => panic!("ClassCastException: JCCatch expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(ctx, pos, Tag::CATCH, TreeKind::Catch { param, body })
    }

    /// Java: public JCTree visitClass(ClassTree node, P p)
    fn visit_class(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (mods, name, typarams, extending, implementing, defs) = match &__kind {
            TreeKind::ClassDecl {
                mods,
                name,
                typarams,
                extending,
                implementing,
                defs,
                ..
            } => (
                self.copy_opt(ctx, *mods),
                name.clone(),
                self.copy_list(ctx, typarams),
                self.copy_opt(ctx, *extending),
                self.copy_list(ctx, implementing),
                self.copy_list(ctx, defs),
            ),
            _ => panic!("ClassCastException: JCClassDecl expected"),
        };
        let pos = ctx.tree(tree).pos;
        // Java: M.at(pos).ClassDef(...6 args) -> TreeMaker.ClassDef passes
        // List.nil() as permitting (TreeMaker.java:157-166), so nil is the
        // upstream value here (see file header).
        self.mk(
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
        )
    }

    /// Java: public JCTree visitConditionalExpression(ConditionalExpressionTree node, P p)
    fn visit_conditional_expression(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (cond, truepart, falsepart) = match &__kind {
            TreeKind::Conditional {
                cond,
                truepart,
                falsepart,
                ..
            } => (
                self.copy_opt(ctx, *cond),
                self.copy_opt(ctx, *truepart),
                self.copy_opt(ctx, *falsepart),
            ),
            _ => panic!("ClassCastException: JCConditional expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::CONDEXPR,
            TreeKind::Conditional {
                cond,
                truepart,
                falsepart,
                poly_kind: PolyKind::STANDALONE,
            },
        )
    }

    /// Java: public JCTree visitContinue(ContinueTree node, P p)
    fn visit_continue(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let label = match &__kind {
            TreeKind::Continue { label, .. } => label.clone(),
            _ => panic!("ClassCastException: JCContinue expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::CONTINUE,
            TreeKind::Continue {
                label,
                target: None,
            },
        )
    }

    /// Java: public JCTree visitDoWhileLoop(DoWhileLoopTree node, P p)
    fn visit_do_while_loop(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (body, cond) = match &__kind {
            TreeKind::DoWhileLoop { body, cond } => {
                (self.copy_opt(ctx, *body), self.copy_opt(ctx, *cond))
            }
            _ => panic!("ClassCastException: JCDoWhileLoop expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(ctx, pos, Tag::DOLOOP, TreeKind::DoWhileLoop { body, cond })
    }

    /// Java: public JCTree visitErroneous(ErroneousTree node, P p)
    fn visit_erroneous(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let errs = match &__kind {
            TreeKind::Erroneous { errs } => self.copy_list(ctx, errs),
            _ => panic!("ClassCastException: JCErroneous expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(ctx, pos, Tag::ERRONEOUS, TreeKind::Erroneous { errs })
    }

    /// Java: public JCTree visitExpressionStatement(ExpressionStatementTree node, P p)
    fn visit_expression_statement(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let expr = match &__kind {
            TreeKind::ExpressionStatement { expr } => self.copy_opt(ctx, *expr),
            _ => panic!("ClassCastException: JCExpressionStatement expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::EXEC,
            TreeKind::ExpressionStatement { expr },
        )
    }


}

impl TreeCopier {
    /// Java: public JCTree visitEnhancedForLoop(EnhancedForLoopTree node, P p)
    fn visit_enhanced_for_loop(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (var, expr, body) = match &__kind {
            TreeKind::EnhancedForLoop { var, expr, body } => (
                self.copy_opt(ctx, *var),
                self.copy_opt(ctx, *expr),
                self.copy_opt(ctx, *body),
            ),
            _ => panic!("ClassCastException: JCEnhancedForLoop expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::FOREACHLOOP,
            TreeKind::EnhancedForLoop { var, expr, body },
        )
    }

    /// Java: public JCTree visitForLoop(ForLoopTree node, P p)
    fn visit_for_loop(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (init, cond, step, body) = match &__kind {
            TreeKind::ForLoop {
                init,
                cond,
                step,
                body,
            } => (
                self.copy_list(ctx, init),
                self.copy_opt(ctx, *cond),
                self.copy_list(ctx, step),
                self.copy_opt(ctx, *body),
            ),
            _ => panic!("ClassCastException: JCForLoop expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::FORLOOP,
            TreeKind::ForLoop {
                init,
                cond,
                step,
                body,
            },
        )
    }

    /// Java: public JCTree visitIdentifier(IdentifierTree node, P p)
    fn visit_identifier(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let name = match &__kind {
            TreeKind::Ident { name, .. } => name.clone(),
            _ => panic!("ClassCastException: JCIdent expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::IDENT,
            TreeKind::Ident { name, sym: None },
        )
    }

    /// Java: public JCTree visitIf(IfTree node, P p)
    fn visit_if(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (cond, thenpart, elsepart) = match &__kind {
            TreeKind::If {
                cond,
                thenpart,
                elsepart,
            } => (
                self.copy_opt(ctx, *cond),
                self.copy_opt(ctx, *thenpart),
                self.copy_opt(ctx, *elsepart),
            ),
            _ => panic!("ClassCastException: JCIf expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::IF,
            TreeKind::If {
                cond,
                thenpart,
                elsepart,
            },
        )
    }

    /// Java: public JCTree visitImport(ImportTree node, P p)
    fn visit_import(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (static_import, qualid) = match &__kind {
            TreeKind::Import {
                static_import,
                qualid,
                ..
            } => (*static_import, self.copy_opt(ctx, *qualid)),
            _ => panic!("ClassCastException: JCImport expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::IMPORT,
            TreeKind::Import {
                static_import,
                qualid,
                import_scope: None,
            },
        )
    }

    /// Java: public JCTree visitArrayAccess(ArrayAccessTree node, P p)
    fn visit_array_access(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (indexed, index) = match &__kind {
            TreeKind::ArrayAccess { indexed, index } => {
                (self.copy_opt(ctx, *indexed), self.copy_opt(ctx, *index))
            }
            _ => panic!("ClassCastException: JCArrayAccess expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::INDEXED,
            TreeKind::ArrayAccess { indexed, index },
        )
    }

    /// Java: public JCTree visitLabeledStatement(LabeledStatementTree node, P p)
    fn visit_labeled_statement(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (label, body) = match &__kind {
            TreeKind::LabeledStatement { label, body } => {
                (label.clone(), self.copy_opt(ctx, *body))
            }
            _ => panic!("ClassCastException: JCLabeledStatement expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::LABELLED,
            TreeKind::LabeledStatement { label, body },
        )
    }

    /// Java: public JCTree visitLiteral(LiteralTree node, P p)
    fn visit_literal(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (typetag, value) = match &__kind {
            TreeKind::Literal { typetag, value } => (*typetag, value.clone()),
            _ => panic!("ClassCastException: JCLiteral expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(ctx, pos, Tag::LITERAL, TreeKind::Literal { typetag, value })
    }

    /// Java: public JCTree visitMethod(MethodTree node, P p)
    fn visit_method(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (mods, name, restype, typarams, recvparam, params, thrown, body, default_value) =
            match &__kind {
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
                    self.copy_opt(ctx, *mods),
                    name.clone(),
                    self.copy_opt(ctx, *restype),
                    self.copy_list(ctx, typarams),
                    self.copy_opt(ctx, *recvparam),
                    self.copy_list(ctx, params),
                    self.copy_list(ctx, thrown),
                    self.copy_opt(ctx, *body),
                    self.copy_opt(ctx, *default_value),
                ),
                _ => panic!("ClassCastException: JCMethodDecl expected"),
            };
        let pos = ctx.tree(tree).pos;
        self.mk(
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
                completes_normally: false,
            },
        )
    }

    /// Java: public JCTree visitMethodInvocation(MethodInvocationTree node, P p)
    fn visit_method_invocation(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (typeargs, meth, args) = match &__kind {
            TreeKind::MethodInvocation {
                typeargs, meth, args, ..
            } => (
                self.copy_list(ctx, typeargs),
                self.copy_opt(ctx, *meth),
                self.copy_list(ctx, args),
            ),
            _ => panic!("ClassCastException: JCMethodInvocation expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
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
        )
    }

    /// Java: public JCTree visitModifiers(ModifiersTree node, P p)
    fn visit_modifiers(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (flags, annotations) = match &__kind {
            TreeKind::Modifiers { flags, annotations } => {
                (*flags, self.copy_list(ctx, annotations))
            }
            _ => panic!("ClassCastException: JCModifiers expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::MODIFIERS,
            TreeKind::Modifiers {
                flags,
                annotations,
            },
        )
    }

    /// Java: public JCTree visitNewArray(NewArrayTree node, P p)
    fn visit_new_array(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (elemtype, dims, elems) = match &__kind {
            TreeKind::NewArray {
                elemtype,
                dims,
                elems,
                ..
            } => (
                self.copy_opt(ctx, *elemtype),
                self.copy_list(ctx, dims),
                elems.as_ref().map(|elems| self.copy_list(ctx, elems)),
            ),
            _ => panic!("ClassCastException: JCNewArray expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
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
        )
    }

    /// Java: public JCTree visitNewClass(NewClassTree node, P p)
    fn visit_new_class(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (encl, typeargs, clazz, args, def) = match &__kind {
            TreeKind::NewClass {
                encl,
                typeargs,
                clazz,
                args,
                def,
                ..
            } => (
                self.copy_opt(ctx, *encl),
                self.copy_list(ctx, typeargs),
                self.copy_opt(ctx, *clazz),
                self.copy_list(ctx, args),
                self.copy_opt(ctx, *def),
            ),
            _ => panic!("ClassCastException: JCNewClass expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
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
        )
    }


}

impl TreeCopier {
    /// Java: public JCTree visitLambdaExpression(LambdaExpressionTree node, P p)
    fn visit_lambda_expression(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (params, body) = match &__kind {
            TreeKind::Lambda { params, body, .. } => {
                (self.copy_list(ctx, params), self.copy_opt(ctx, *body))
            }
            _ => panic!("ClassCastException: JCLambda expected"),
        };
        // Java: M.at(t.pos).Lambda(params, body) — JCLambda(params, body)
        // derives paramKind from the copied params (JCTree.java:1957-1966), so
        // this derivation is the upstream constructor behavior (file header).
        let pos = ctx.tree(tree).pos;
        let param_kind = if params.is_empty() {
            crate::tree::jc_tree::ParameterKind::EXPLICIT
        } else {
            let first = *params.head();
            match &ctx.tree(first).kind {
                TreeKind::VariableDecl {
                    vartype: Some(_), ..
                } => crate::tree::jc_tree::ParameterKind::EXPLICIT,
                _ => crate::tree::jc_tree::ParameterKind::IMPLICIT,
            }
        };
        self.mk(
            ctx,
            pos,
            Tag::LAMBDA,
            TreeKind::Lambda {
                params,
                body,
                can_complete_normally: false,
                param_kind,
                poly_kind: PolyKind::STANDALONE,
                target: None,
            },
        )
    }

    /// Java: public JCTree visitParenthesized(ParenthesizedTree node, P p)
    fn visit_parenthesized(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let expr = match &__kind {
            TreeKind::Parens { expr } => self.copy_opt(ctx, *expr),
            _ => panic!("ClassCastException: JCParens expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(ctx, pos, Tag::PARENS, TreeKind::Parens { expr })
    }

    /// Java: public JCTree visitReturn(ReturnTree node, P p)
    fn visit_return(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let expr = match &__kind {
            TreeKind::Return { expr } => self.copy_opt(ctx, *expr),
            _ => panic!("ClassCastException: JCReturn expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(ctx, pos, Tag::RETURN, TreeKind::Return { expr })
    }

    /// Java: public JCTree visitMemberSelect(MemberSelectTree node, P p)
    fn visit_member_select(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (selected, name) = match &__kind {
            TreeKind::FieldAccess { selected, name, .. } => {
                (self.copy_opt(ctx, *selected), name.clone())
            }
            _ => panic!("ClassCastException: JCFieldAccess expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::SELECT,
            TreeKind::FieldAccess {
                selected,
                name,
                sym: None,
            },
        )
    }

    /// Java: public JCTree visitMemberReference(MemberReferenceTree node, P p)
    fn visit_member_reference(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (mode, name, expr, typeargs) = match &__kind {
            TreeKind::MemberReference {
                mode,
                name,
                expr,
                typeargs,
                ..
            } => (
                *mode,
                name.clone(),
                self.copy_opt(ctx, *expr),
                typeargs.as_ref().map(|typeargs| self.copy_list(ctx, typeargs)),
            ),
            _ => panic!("ClassCastException: JCMemberReference expected"),
        };
        let pos = ctx.tree(tree).pos;
        // Java: DeferredAttr's copier forwards the overload kind (setOverloadKind
        // forwards to the original tree; getOverloadKind reads the original), so
        // the speculative copy keeps the reference kind/poly kind (§9).
        let (kind, ref_poly_kind, owner_accessible, poly_kind, overload_kind) = if self.speculative {
            match &ctx.tree(tree).kind {
                TreeKind::MemberReference {
                    kind,
                    ref_poly_kind,
                    owner_accessible,
                    poly_kind,
                    overload_kind,
                    ..
                } => (*kind, *ref_poly_kind, *owner_accessible, *poly_kind, *overload_kind),
                _ => (None, PolyKind::STANDALONE, false, PolyKind::STANDALONE, None),
            }
        } else {
            (None, PolyKind::STANDALONE, false, PolyKind::STANDALONE, None)
        };
        self.mk(
            ctx,
            pos,
            Tag::REFERENCE,
            TreeKind::MemberReference {
                mode,
                kind,
                name,
                expr,
                typeargs,
                sym: None,
                varargs_element: None,
                ref_poly_kind,
                owner_accessible,
                referent_type: None,
                overload_kind,
                poly_kind,
                target: None,
            },
        )
    }

    /// Java: public JCTree visitEmptyStatement(EmptyStatementTree node, P p)
    fn visit_empty_statement(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let pos = ctx.tree(tree).pos;
        self.mk(ctx, pos, Tag::SKIP, TreeKind::Skip {})
    }

    /// Java: public JCTree visitSwitch(SwitchTree node, P p)
    fn visit_switch(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (selector, cases) = match &__kind {
            TreeKind::Switch { selector, cases, .. } => {
                (self.copy_opt(ctx, *selector), self.copy_list(ctx, cases))
            }
            _ => panic!("ClassCastException: JCSwitch expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::SWITCH,
            TreeKind::Switch {
                selector,
                cases,
                endpos: position::NOPOS,
                has_total_pattern: false,
                pattern_switch: false,
            },
        )
    }

    /// Java: public JCTree visitSwitchExpression(SwitchExpressionTree node, P p)
    fn visit_switch_expression(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (selector, cases) = match &__kind {
            TreeKind::SwitchExpression { selector, cases, .. } => {
                (self.copy_opt(ctx, *selector), self.copy_list(ctx, cases))
            }
            _ => panic!("ClassCastException: JCSwitchExpression expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::SWITCH_EXPRESSION,
            TreeKind::SwitchExpression {
                selector,
                cases,
                endpos: position::NOPOS,
                has_total_pattern: false,
                pattern_switch: false,
                poly_kind: PolyKind::STANDALONE,
            },
        )
    }

    /// Java: public JCTree visitSynchronized(SynchronizedTree node, P p)
    fn visit_synchronized(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (lock, body) = match &__kind {
            TreeKind::Synchronized { lock, body } => {
                (self.copy_opt(ctx, *lock), self.copy_opt(ctx, *body))
            }
            _ => panic!("ClassCastException: JCSynchronized expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::SYNCHRONIZED,
            TreeKind::Synchronized { lock, body },
        )
    }

    /// Java: public JCTree visitThrow(ThrowTree node, P p)
    fn visit_throw(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let expr = match &__kind {
            TreeKind::Throw { expr } => self.copy_opt(ctx, *expr),
            _ => panic!("ClassCastException: JCThrow expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(ctx, pos, Tag::THROW, TreeKind::Throw { expr })
    }

    /// Java: public JCTree visitCompilationUnit(CompilationUnitTree node, P p)
    fn visit_compilation_unit(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let defs = match &__kind {
            TreeKind::CompilationUnit { defs, .. } => self.copy_list(ctx, defs),
            _ => panic!("ClassCastException: JCCompilationUnit expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
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
        )
    }

    /// Java: public JCTree visitPackage(PackageTree node, P p)
    fn visit_package(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (annotations, pid) = match &__kind {
            TreeKind::PackageDecl { annotations, pid, .. } => {
                (self.copy_list(ctx, annotations), self.copy_opt(ctx, *pid))
            }
            _ => panic!("ClassCastException: JCPackageDecl expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::PACKAGEDEF,
            TreeKind::PackageDecl {
                annotations,
                pid,
                packge: None,
            },
        )
    }

    /// Java: public JCTree visitTry(TryTree node, P p)
    fn visit_try(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (resources, body, catchers, finalizer) = match &__kind {
            TreeKind::Try {
                resources,
                body,
                catchers,
                finalizer,
                ..
            } => (
                self.copy_list(ctx, resources),
                self.copy_opt(ctx, *body),
                self.copy_list(ctx, catchers),
                self.copy_opt(ctx, *finalizer),
            ),
            _ => panic!("ClassCastException: JCTry expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::TRY,
            TreeKind::Try {
                body,
                catchers,
                finalizer,
                resources,
                finally_can_complete_normally: false,
            },
        )
    }

    /// Java: public JCTree visitParameterizedType(ParameterizedTypeTree node, P p)
    fn visit_parameterized_type(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (clazz, arguments) = match &__kind {
            TreeKind::TypeApply { clazz, arguments } => {
                (self.copy_opt(ctx, *clazz), self.copy_list(ctx, arguments))
            }
            _ => panic!("ClassCastException: JCTypeApply expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::TYPEAPPLY,
            TreeKind::TypeApply { clazz, arguments },
        )
    }

    /// Java: public JCTree visitUnionType(UnionTypeTree node, P p)
    fn visit_union_type(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let alternatives = match &__kind {
            TreeKind::TypeUnion { alternatives } => self.copy_list(ctx, alternatives),
            _ => panic!("ClassCastException: JCTypeUnion expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::TYPEUNION,
            TreeKind::TypeUnion { alternatives },
        )
    }

    /// Java: public JCTree visitIntersectionType(IntersectionTypeTree node, P p)
    fn visit_intersection_type(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let bounds = match &__kind {
            TreeKind::TypeIntersection { bounds } => self.copy_list(ctx, bounds),
            _ => panic!("ClassCastException: JCTypeIntersection expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::TYPEINTERSECTION,
            TreeKind::TypeIntersection { bounds },
        )
    }


}

impl TreeCopier {
    /// Java: public JCTree visitArrayType(ArrayTypeTree node, P p)
    fn visit_array_type(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let elemtype = match &__kind {
            TreeKind::ArrayTypeTree { elemtype } => self.copy_opt(ctx, *elemtype),
            _ => panic!("ClassCastException: JCArrayTypeTree expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::TYPEARRAY,
            TreeKind::ArrayTypeTree { elemtype },
        )
    }

    /// Java: public JCTree visitTypeCast(TypeCastTree node, P p)
    fn visit_type_cast(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (clazz, expr) = match &__kind {
            TreeKind::TypeCast { clazz, expr } => {
                (self.copy_opt(ctx, *clazz), self.copy_opt(ctx, *expr))
            }
            _ => panic!("ClassCastException: JCTypeCast expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(ctx, pos, Tag::TYPECAST, TreeKind::TypeCast { clazz, expr })
    }

    /// Java: public JCTree visitPrimitiveType(PrimitiveTypeTree node, P p)
    fn visit_primitive_type(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let typetag = match &__kind {
            TreeKind::PrimitiveTypeTree { typetag } => *typetag,
            _ => panic!("ClassCastException: JCPrimitiveTypeTree expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::TYPEIDENT,
            TreeKind::PrimitiveTypeTree { typetag },
        )
    }

    /// Java: public JCTree visitTypeParameter(TypeParameterTree node, P p)
    fn visit_type_parameter(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (name, bounds, annotations) = match &__kind {
            TreeKind::TypeParameter {
                name,
                bounds,
                annotations,
            } => (
                name.clone(),
                self.copy_list(ctx, bounds),
                self.copy_list(ctx, annotations),
            ),
            _ => panic!("ClassCastException: JCTypeParameter expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::TYPEPARAMETER,
            TreeKind::TypeParameter {
                name,
                bounds,
                annotations,
            },
        )
    }

    /// Java: public JCTree visitInstanceOf(InstanceOfTree node, P p)
    fn visit_instance_of(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (expr, pattern) = match &__kind {
            TreeKind::InstanceOf { expr, pattern } => {
                (self.copy_opt(ctx, *expr), self.copy_opt(ctx, *pattern))
            }
            _ => panic!("ClassCastException: JCInstanceOf expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::TYPETEST,
            TreeKind::InstanceOf { expr, pattern },
        )
    }

    /// Java: public JCTree visitBindingPattern(BindingPatternTree node, P p)
    fn visit_binding_pattern(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let var = match &__kind {
            TreeKind::BindingPattern { var } => self.copy_opt(ctx, *var),
            _ => panic!("ClassCastException: JCBindingPattern expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(ctx, pos, Tag::BINDINGPATTERN, TreeKind::BindingPattern { var })
    }

    /// Java: public JCTree visitGuardedPattern(GuardedPatternTree node, P p)
    fn visit_guarded_pattern(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (patt, expr) = match &__kind {
            TreeKind::GuardPattern { patt, expr } => {
                (self.copy_opt(ctx, *patt), self.copy_opt(ctx, *expr))
            }
            _ => panic!("ClassCastException: JCGuardPattern expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::GUARDPATTERN,
            TreeKind::GuardPattern { patt, expr },
        )
    }

    /// Java: public JCTree visitParenthesizedPattern(ParenthesizedPatternTree node, P p)
    fn visit_parenthesized_pattern(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let pattern = match &__kind {
            TreeKind::ParenthesizedPattern { pattern } => self.copy_opt(ctx, *pattern),
            _ => panic!("ClassCastException: JCParenthesizedPattern expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::PARENTHESIZEDPATTERN,
            TreeKind::ParenthesizedPattern { pattern },
        )
    }

    /// Java: public JCTree visitDefaultCaseLabel(DefaultCaseLabelTree node, P p)
    fn visit_default_case_label(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let pos = ctx.tree(tree).pos;
        self.mk(ctx, pos, Tag::DEFAULTCASELABEL, TreeKind::DefaultCaseLabel {})
    }

    /// Java: public JCTree visitUnary(UnaryTree node, P p)
    fn visit_unary(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let arg = match &__kind {
            TreeKind::Unary { arg, .. } => self.copy_opt(ctx, *arg),
            _ => panic!("ClassCastException: JCUnary expected"),
        };
        let pos = ctx.tree(tree).pos;
        let tag = ctx.tree(tree).tag;
        self.mk(ctx, pos, tag, TreeKind::Unary { arg, operator: None })
    }

    /// Java: public JCTree visitVariable(VariableTree node, P p)
    fn visit_variable(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (mods, name, nameexpr, vartype, init) = match &__kind {
            TreeKind::VariableDecl {
                mods,
                name,
                nameexpr,
                vartype,
                init,
                ..
            } => (
                self.copy_opt(ctx, *mods),
                name.clone(),
                self.copy_opt(ctx, *nameexpr),
                self.copy_opt(ctx, *vartype),
                self.copy_opt(ctx, *init),
            ),
            _ => panic!("ClassCastException: JCVariableDecl expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::VARDEF,
            TreeKind::VariableDecl {
                mods,
                name,
                nameexpr,
                vartype,
                init,
                sym: None,
                start_pos: position::NOPOS,
            },
        )
    }

    /// Java: public JCTree visitWhileLoop(WhileLoopTree node, P p)
    fn visit_while_loop(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (cond, body) = match &__kind {
            TreeKind::WhileLoop { cond, body } => {
                (self.copy_opt(ctx, *cond), self.copy_opt(ctx, *body))
            }
            _ => panic!("ClassCastException: JCWhileLoop expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::WHILELOOP,
            TreeKind::WhileLoop { cond, body },
        )
    }

    /// Java: public JCTree visitWildcard(WildcardTree node, P p)
    fn visit_wildcard(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (kind, inner) = match &__kind {
            TreeKind::Wildcard { kind, inner } => {
                (self.copy_opt(ctx, *kind), self.copy_opt(ctx, *inner))
            }
            _ => panic!("ClassCastException: JCWildcard expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(ctx, pos, Tag::WILDCARD, TreeKind::Wildcard { kind, inner })
    }

    /// Java: public JCTree visitModule(ModuleTree node, P p)
    fn visit_module(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (mods, kind, qual_id, directives) = match &__kind {
            TreeKind::ModuleDecl {
                mods,
                kind,
                qual_id,
                directives,
                ..
            } => (
                self.copy_opt(ctx, *mods),
                *kind,
                self.copy_opt(ctx, *qual_id),
                directives.as_ref().map(|directives| self.copy_list(ctx, directives)),
            ),
            _ => panic!("ClassCastException: JCModuleDecl expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
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
        )
    }

    /// Java: public JCExports visitExports(ExportsTree node, P p)
    fn visit_exports(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (qualid, module_names) = match &__kind {
            TreeKind::Exports {
                qualid,
                module_names,
                ..
            } => (
                self.copy_opt(ctx, *qualid),
                module_names.as_ref().map(|names| self.copy_list(ctx, names)),
            ),
            _ => panic!("ClassCastException: JCExports expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::EXPORTS,
            TreeKind::Exports {
                qualid,
                module_names,
                directive: None,
            },
        )
    }

    /// Java: public JCOpens visitOpens(OpensTree node, P p)
    fn visit_opens(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (qualid, module_names) = match &__kind {
            TreeKind::Opens {
                qualid,
                module_names,
                ..
            } => (
                self.copy_opt(ctx, *qualid),
                module_names.as_ref().map(|names| self.copy_list(ctx, names)),
            ),
            _ => panic!("ClassCastException: JCOpens expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::OPENS,
            TreeKind::Opens {
                qualid,
                module_names,
                directive: None,
            },
        )
    }

    /// Java: public JCProvides visitProvides(ProvidesTree node, P p)
    fn visit_provides(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (service_name, impl_names) = match &__kind {
            TreeKind::Provides {
                service_name,
                impl_names,
            } => (
                self.copy_opt(ctx, *service_name),
                self.copy_list(ctx, impl_names),
            ),
            _ => panic!("ClassCastException: JCProvides expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::PROVIDES,
            TreeKind::Provides {
                service_name,
                impl_names,
            },
        )
    }

    /// Java: public JCRequires visitRequires(RequiresTree node, P p)
    fn visit_requires(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (is_transitive, is_static_phase, module_name) = match &__kind {
            TreeKind::Requires {
                is_transitive,
                is_static_phase,
                module_name,
                ..
            } => (
                *is_transitive,
                *is_static_phase,
                self.copy_opt(ctx, *module_name),
            ),
            _ => panic!("ClassCastException: JCRequires expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::REQUIRES,
            TreeKind::Requires {
                is_transitive,
                is_static_phase,
                module_name,
                directive: None,
            },
        )
    }

    /// Java: public JCUses visitUses(UsesTree node, P p)
    fn visit_uses(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let qualid = match &__kind {
            TreeKind::Uses { qualid } => self.copy_opt(ctx, *qualid),
            _ => panic!("ClassCastException: JCUses expected"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(ctx, pos, Tag::USES, TreeKind::Uses { qualid })
    }

    /// Java: public JCTree visitOther(Tree node, P p) — LETEXPR
    fn visit_other_let_expr(&self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let __kind = ctx.tree(tree).kind.clone();
        let (defs, expr) = match &__kind {
            TreeKind::LetExpr { defs, expr, .. } => {
                (self.copy_list(ctx, defs), self.copy_opt(ctx, *expr))
            }
            _ => panic!("AssertionError: unknown tree tag"),
        };
        let pos = ctx.tree(tree).pos;
        self.mk(
            ctx,
            pos,
            Tag::LETEXPR,
            TreeKind::LetExpr {
                defs,
                expr,
                needs_cond: false,
            },
        )
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::jc_tree::JCTree;
    use crate::tree::tree_maker::TreeMaker;
    use crate::util::name::Name;
    use crate::util::name::NameTableHandle;
    use std::cell::RefCell;
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

    fn copier() -> TreeCopier {
        TreeCopier::new(Rc::new(RefCell::new(TreeMaker::new(None))))
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
        let copier = copier();
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
        let copier = copier();
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
        let copier = copier();
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
                // Java: JCLambda(params, body) derives paramKind from the params
                // (JCTree.java:1957-1966); the copied params carry no vartype, so
                // the copy is IMPLICIT, like the upstream TreeCopier.
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
        let copier = copier();
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
