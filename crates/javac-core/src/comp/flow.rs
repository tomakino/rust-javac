// Origin: com/sun/tools/javac/comp/Flow.java @ 982d53c (jdk-17.0.19-ga)
//
// PORTING_RULES:
// - ADR 0001: the ported Flow takes the services explicitly (Flow.instance/
// Flow(Context) are registered with the Context registration); the
// visitor subclass hierarchy maps to the FlowVisitor trait (the default
// visit methods are the ported TreeScanner navigational bodies, so a
// specialized analyzer overrides only the visit methods it cares about,
// exactly like the upstream TreeScanner subclasses);
// - the upstream TreeScanner.scan(tree) -> tree.accept(this) becomes
// FlowVisitor::scan (accept dispatch), and scan_list/scan_list_opt are the
// ported list helpers;
// - the Java ListBuffer<PendingExit> maps to the ported List<PendingExit> in
// the analyzer state (same append/iterate order);
// - the FlowAnalyzer.ThrownPendingExit / AssignAnalyzer.AssignPendingExit
// subclasses map to PendingExit variants;
// - adapter adapters: JCTree.type checks read the arena type slot; the diagnostic
// position surface is the i32 preferred position; the Log error/warning
// helpers (log.error(pos, fragment/key), log.warning(LintCategory,...)) are
// implemented in this file on top of Factory::create + Log::report
// (registered with the Log).

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;

use crate::code::flags;
use crate::code::kinds::Kind;
use crate::code::lint::Lint;
use crate::code::lint::LintCategory;
use crate::code::source::Feature;
use crate::code::symtab::Symtab;
use crate::code::type_tag::TypeTag;
use crate::code::types::Types;
use crate::comp::attr_context::AttrContext;
use crate::comp::check::Check;
use crate::comp::env::Env;
use crate::comp::resolve::Resolve;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::resources::compiler_properties::errors;
use crate::resources::compiler_properties::warnings;
use crate::resources::diagnostic_contract::ErrorInfo;
use crate::tree::jc_tree::{accept, Tag, TreeKind, TreeVisitor};
use javac_source::tree::case_tree::CaseKind;
use javac_tools::lang::model::element::element_kind::ElementKind;
use crate::tree::tree_info;
use crate::tree::tree_maker::{TreeFactory, TreeMaker};
use crate::util::bits::Bits;
use crate::util::jc_diagnostic::{
    DiagnosticArg, DiagnosticType, Factory, SimpleDiagnosticPosition,
};
use crate::util::list::List;
use crate::util::log::{DiagnosticHandler, Log};
use crate::util::names::Names;

/// ADR 0001 adapter: the ported FlowVisitor trait carries the navigational
/// default methods; this macro forwards every TreeVisitor callback to the
/// FlowVisitor dispatch so the analyzers only override what they need
/// (exactly like the upstream TreeScanner subclassing).
macro_rules! flow_tree_visitor_delegate {
    ($ty:ty $(, $lt:lifetime)*) => {
        impl<$($lt),*> TreeVisitor for $ty {
            fn visit_top_level(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_top_level(self, ctx, tree);
            }
            fn visit_package_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_package_def(self, ctx, tree);
            }
            fn visit_module_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_module_def(self, ctx, tree);
            }
            fn visit_exports(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_exports(self, ctx, tree);
            }
            fn visit_opens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_opens(self, ctx, tree);
            }
            fn visit_provides(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_provides(self, ctx, tree);
            }
            fn visit_requires(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_requires(self, ctx, tree);
            }
            fn visit_uses(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_uses(self, ctx, tree);
            }
            fn visit_import(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_import(self, ctx, tree);
            }
            fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_class_def(self, ctx, tree);
            }
            fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_method_def(self, ctx, tree);
            }
            fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_var_def(self, ctx, tree);
            }
            fn visit_skip(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_skip(self, ctx, tree);
            }
            fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_block(self, ctx, tree);
            }
            fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_do_loop(self, ctx, tree);
            }
            fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_while_loop(self, ctx, tree);
            }
            fn visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_for_loop(self, ctx, tree);
            }
            fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_foreach_loop(self, ctx, tree);
            }
            fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_labelled(self, ctx, tree);
            }
            fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_switch(self, ctx, tree);
            }
            fn visit_case(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_case(self, ctx, tree);
            }
            fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_switch_expression(self, ctx, tree);
            }
            fn visit_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_synchronized(self, ctx, tree);
            }
            fn visit_try(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_try(self, ctx, tree);
            }
            fn visit_catch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_catch(self, ctx, tree);
            }
            fn visit_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_conditional(self, ctx, tree);
            }
            fn visit_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_if(self, ctx, tree);
            }
            fn visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_exec(self, ctx, tree);
            }
            fn visit_break(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_break(self, ctx, tree);
            }
            fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_yield(self, ctx, tree);
            }
            fn visit_continue(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_continue(self, ctx, tree);
            }
            fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_return(self, ctx, tree);
            }
            fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_throw(self, ctx, tree);
            }
            fn visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_assert(self, ctx, tree);
            }
            fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_apply(self, ctx, tree);
            }
            fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_new_class(self, ctx, tree);
            }
            fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_new_array(self, ctx, tree);
            }
            fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_lambda(self, ctx, tree);
            }
            fn visit_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_parens(self, ctx, tree);
            }
            fn visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_assign(self, ctx, tree);
            }
            fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_assignop(self, ctx, tree);
            }
            fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_unary(self, ctx, tree);
            }
            fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_binary(self, ctx, tree);
            }
            fn visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_type_cast(self, ctx, tree);
            }
            fn visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_type_test(self, ctx, tree);
            }
            fn visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_binding_pattern(self, ctx, tree);
            }
            fn visit_default_case_label(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_default_case_label(self, ctx, tree);
            }
            fn visit_parenthesized_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_parenthesized_pattern(self, ctx, tree);
            }
            fn visit_guard_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_guard_pattern(self, ctx, tree);
            }
            fn visit_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_indexed(self, ctx, tree);
            }
            fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_select(self, ctx, tree);
            }
            fn visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_reference(self, ctx, tree);
            }
            fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_ident(self, ctx, tree);
            }
            fn visit_literal(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_literal(self, ctx, tree);
            }
            fn visit_type_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_type_ident(self, ctx, tree);
            }
            fn visit_type_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_type_array(self, ctx, tree);
            }
            fn visit_type_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_type_apply(self, ctx, tree);
            }
            fn visit_type_union(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_type_union(self, ctx, tree);
            }
            fn visit_type_intersection(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_type_intersection(self, ctx, tree);
            }
            fn visit_type_parameter(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_type_parameter(self, ctx, tree);
            }
            fn visit_wildcard(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_wildcard(self, ctx, tree);
            }
            fn visit_type_bound_kind(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_type_bound_kind(self, ctx, tree);
            }
            fn visit_modifiers(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_modifiers(self, ctx, tree);
            }
            fn visit_annotation(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_annotation(self, ctx, tree);
            }
            fn visit_annotated_type(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_annotated_type(self, ctx, tree);
            }
            fn visit_erroneous(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_erroneous(self, ctx, tree);
            }
            fn visit_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_let_expr(self, ctx, tree);
            }
            fn visit_tree(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
                <Self as FlowVisitor>::visit_tree(self, ctx, tree);
            }
        }
    };
}

/// Java: enum Flow.Liveness (Flow.java:3085-3138)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Liveness {
    ALIVE,
    DEAD,
    RECOVERY,
}

impl Liveness {
    pub fn or(self, other: Liveness) -> Liveness {
        match self {
            Liveness::ALIVE => Liveness::ALIVE,
            Liveness::DEAD => other,
            Liveness::RECOVERY => {
                if other == Liveness::ALIVE {
                    Liveness::ALIVE
                } else {
                    Liveness::RECOVERY
                }
            }
        }
    }

    pub fn and(self, other: Liveness) -> Liveness {
        match self {
            Liveness::ALIVE => other,
            Liveness::DEAD => Liveness::DEAD,
            Liveness::RECOVERY => {
                if other == Liveness::DEAD {
                    Liveness::DEAD
                } else {
                    Liveness::RECOVERY
                }
            }
        }
    }

    pub fn from_bool(value: bool) -> Liveness {
        if value { Liveness::ALIVE } else { Liveness::DEAD }
    }

    pub fn or_bool(self, value: bool) -> Liveness {
        self.or(Liveness::from_bool(value))
    }

    pub fn and_bool(self, value: bool) -> Liveness {
        self.and(Liveness::from_bool(value))
    }
}

/// Java: enum Flow.FlowKind (Flow.java:339-360)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
#[allow(non_camel_case_types)]
pub enum FlowKind {
    NORMAL,
    SPECULATIVE_LOOP,
}

impl FlowKind {
    pub fn err_key(self) -> &'static str {
        match self {
            FlowKind::NORMAL => "var.might.already.be.assigned",
            FlowKind::SPECULATIVE_LOOP => "var.might.be.assigned.in.loop",
        }
    }

    pub fn is_final(self) -> bool {
        matches!(self, FlowKind::SPECULATIVE_LOOP)
    }
}

/// Java: Flow.BaseAnalyzer.JumpKind (Flow.java:365-393)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JumpKind {
    BREAK,
    CONTINUE,
    YIELD,
}

impl JumpKind {
    pub fn tree_tag(self) -> Tag {
        match self {
            JumpKind::BREAK => Tag::BREAK,
            JumpKind::CONTINUE => Tag::CONTINUE,
            JumpKind::YIELD => Tag::YIELD,
        }
    }
}

/// Copy helper for Bits (the ported Bits has no Clone; Porting rules adapter).
pub fn clone_bits(bits: &Bits) -> Bits {
    // Java: new Bits(Bits someBits) keeps the UNKNOWN (reset) state when the
    // source has no bits array (Bits.java constructor/getState).
    if bits.is_reset() {
        let mut copy = Bits::new();
        copy.reset();
        return copy;
    }
    let mut copy = Bits::new();
    let mut next = bits.next_bit(0);
    while next >= 0 {
        copy.incl(next);
        next = bits.next_bit(next + 1);
    }
    copy
}

/// Java: Flow.BaseAnalyzer.PendingExit (Flow.java:400-412) and its
/// FlowAnalyzer.ThrownPendingExit / AssignAnalyzer.AssignPendingExit
/// subclasses (Flow.java:977-991, 1745-1768).
#[derive(Clone)]
pub enum PendingExit {
    Plain { tree: TreeId },
    Thrown { tree: TreeId, thrown: TypeId },
    Assign {
        tree: TreeId,
        exit_inits: Rc<RefCell<Bits>>,
        exit_uninits: Rc<RefCell<Bits>>,
    },
}

impl PendingExit {
    pub fn new(tree: TreeId) -> PendingExit {
        PendingExit::Plain { tree }
    }

    /// Java: AssignPendingExit(JCTree tree, Bits inits, Bits uninits)
    pub fn assign(tree: TreeId, inits: &Bits, uninits: &Bits) -> PendingExit {
        PendingExit::Assign {
            tree,
            exit_inits: Rc::new(RefCell::new(clone_bits(inits))),
            exit_uninits: Rc::new(RefCell::new(clone_bits(uninits))),
        }
    }

    pub fn tree(&self) -> TreeId {
        match self {
            PendingExit::Plain { tree }
            | PendingExit::Thrown { tree, .. }
            | PendingExit::Assign { tree, .. } => *tree,
        }
    }

    pub fn resolve_jump(&self) {}
}

/// Java: static abstract class Flow.BaseAnalyzer extends TreeScanner
/// (Flow.java:361-467) — shared jump handling.
pub trait BaseAnalyzer: FlowVisitor {
    fn pending_exits_mut(&mut self) -> &mut List<PendingExit>;
    fn mark_dead(&mut self, ctx: &mut CompilerContext);

    fn resolve_pending_exit(&mut self, _exit: &PendingExit) {}

    /// Java: void recordExit(PendingExit pe)
    fn record_exit(&mut self, ctx: &mut CompilerContext, pe: PendingExit) {
        *self.pending_exits_mut() = self.pending_exits_mut().append(pe);
        self.mark_dead(ctx);
    }

    /// Java: private Liveness resolveJump(JCTree tree,
    /// ListBuffer<PendingExit> oldPendingExits, JumpKind jk) (Flow.java:414-437)
    fn resolve_jump(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        old_pending_exits: List<PendingExit>,
        jk: JumpKind,
    ) -> Liveness {
        let mut resolved = false;
        let mut exits = std::mem::replace(self.pending_exits_mut(), old_pending_exits);
        while exits.non_empty() {
            let exit = exits.head().clone();
            let target = Self::jump_target(ctx, &exit.tree(), jk);
            if ctx.tree(exit.tree()).has_tag(jk.tree_tag()) && target == Some(tree) {
                self.resolve_pending_exit(&exit);
                resolved = true;
            } else {
                *self.pending_exits_mut() = self.pending_exits_mut().append(exit);
            }
            exits = exits.tail();
        }
        Liveness::from_bool(resolved)
    }

    /// Java: JumpKind.getTarget(JCTree) (Flow.java:370-392)
    #[allow(clippy::collapsible_match)]
    fn jump_target(ctx: &CompilerContext, tree: &TreeId, jk: JumpKind) -> Option<TreeId> {
        match jk {
            JumpKind::BREAK => match &ctx.tree(*tree).kind {
                TreeKind::Break { target, .. } => *target,
                _ => None,
            },
            JumpKind::CONTINUE => match &ctx.tree(*tree).kind {
                TreeKind::Continue { target, .. } => *target,
                _ => None,
            },
            JumpKind::YIELD => match &ctx.tree(*tree).kind {
                TreeKind::Yield { target, .. } => *target,
                _ => None,
            },
        }
    }

    fn resolve_continues(&mut self, ctx: &mut CompilerContext, tree: TreeId) -> Liveness {
        self.resolve_jump(ctx, tree, List::nil(), JumpKind::CONTINUE)
    }

    fn resolve_breaks(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        old: List<PendingExit>,
    ) -> Liveness {
        self.resolve_jump(ctx, tree, old, JumpKind::BREAK)
    }

    fn resolve_yields(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        old: List<PendingExit>,
    ) -> Liveness {
        self.resolve_jump(ctx, tree, old, JumpKind::YIELD)
    }
}

/// Java: public class Flow (comp/Flow.java:217-350)
pub struct Flow {
    pub names: Rc<Names>,
    pub log: Rc<RefCell<Log>>,
    pub syms: Rc<RefCell<Symtab>>,
    pub types: Rc<Types>,
    pub chk: Rc<RefCell<Check>>,
    pub rs: Rc<RefCell<Resolve>>,
    pub diags: Rc<Factory>,
    pub lint: Rc<Lint>,
    pub attr_env: Rc<RefCell<Option<Rc<Env<AttrContext>>>>>,
    pub make: Rc<RefCell<Option<Rc<RefCell<TreeMaker>>>>>,
    pub allow_effectively_final_in_inner_classes: bool,
    /// Java: the Attr service (Flow.java's outer Attr) used by the resource
    /// close() lookup in FlowAnalyzer (registered adapter: the Rust Flow keeps
    /// the Attr cell instead of the Java inner-class outer pointer).
    pub attr: Option<Rc<RefCell<crate::comp::attr::Attr>>>,
}

impl Flow {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        _ctx: &mut CompilerContext,
        names: Rc<Names>,
        log: Rc<RefCell<Log>>,
        syms: Rc<RefCell<Symtab>>,
        types: Rc<Types>,
        chk: Rc<RefCell<Check>>,
        rs: Rc<RefCell<Resolve>>,
        diags: Rc<Factory>,
        lint: Rc<Lint>,
        source: crate::code::source::Source,
        attr: Option<Rc<RefCell<crate::comp::attr::Attr>>>, 
    ) -> Flow {
        Flow {
            names,
            log,
            syms,
            types,
            chk,
            rs,
            diags,
            lint,
            attr_env: Rc::new(RefCell::new(None)),
            make: Rc::new(RefCell::new(None)),
            allow_effectively_final_in_inner_classes:
                Feature::EFFECTIVELY_FINAL_IN_INNER_CLASSES.allowed_in_source(source),
            attr,
        }
    }

    /// Java: log.error(DiagnosticPosition, Error) — via Factory::create +
    /// Log::report (Log; ADR 0002 P4 argument placeholders).
    pub fn error(&self, pos: i32, info: ErrorInfo) {
        let args: Vec<DiagnosticArg> = info
            .info()
            .args
            .iter()
            .map(|arg| Rc::new(javac_strings::JavaString::from(arg.to_text().as_str())) as DiagnosticArg)
            .collect();
        let source = self.log.borrow().current_source();
        let diagnostic = self.diags.create(
            DiagnosticType::ERROR,
            source,
            Some(Rc::new(SimpleDiagnosticPosition::new(pos))),
            info.info().code,
            args,
        );
        self.log.borrow_mut().report(diagnostic);
    }

    /// Java: log.error(pos, diags.errorKey(code, args))
    pub fn error_key(&self, pos: i32, code: &'static str, args: Vec<DiagnosticArg>) {
        let source = self.log.borrow().current_source();
        let diagnostic = self.diags.create(
            DiagnosticType::ERROR,
            source,
            Some(Rc::new(SimpleDiagnosticPosition::new(pos))),
            code,
            args,
        );
        self.log.borrow_mut().report(diagnostic);
    }

    /// Java: log.warning(LintCategory, DiagnosticPosition, Warning)
    pub fn warning(
        &self,
        _lint_category: LintCategory,
        pos: i32,
        info: crate::resources::diagnostic_contract::WarningInfo,
    ) {
        let args: Vec<DiagnosticArg> = info
            .info()
            .args
            .iter()
            .map(|arg| Rc::new(javac_strings::JavaString::from(arg.to_text().as_str())) as DiagnosticArg)
            .collect();
        let source = self.log.borrow().current_source();
        let diagnostic = self.diags.create(
            DiagnosticType::WARNING,
            source,
            Some(Rc::new(SimpleDiagnosticPosition::new(pos))),
            info.info().code,
            args,
        );
        self.log.borrow_mut().report(diagnostic);
    }

    /// Java: Lint.augment(tree.sym) — Lint.augment is registered with the
    /// Attribute/Symtab (unimplemented at that entry).
    pub fn augmented_lint(&self, ctx: &CompilerContext, sym: SymbolId) -> Rc<Lint> {
        Rc::new(self.lint.augment(ctx, &self.syms.borrow(), ctx.sym(sym)))
    }
}

/// Java: JCVariableDecl.init
fn var_decl_init(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::VariableDecl { init, .. } => *init,
        _ => None,
    }
}

/// Java: JCBlock.stats
fn block_stats(ctx: &CompilerContext, tree: TreeId) -> List<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::Block { stats, .. } => stats.clone(),
        _ => List::nil(),
    }
}

/// Java: class Flow.FlowAnalyzer extends BaseAnalyzer (Flow.java:959-1544)
/// Java: TreeInfo/Scope accessors shared by the flow analyzers.
fn class_symbol(ctx: &CompilerContext, tree: TreeId) -> Option<SymbolId> {
    match &ctx.tree(tree).kind {
        TreeKind::ClassDecl { sym, .. } => *sym,
        _ => None,
    }
}

fn class_defs(ctx: &CompilerContext, tree: TreeId) -> List<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::ClassDecl { defs, .. } => defs.clone(),
        _ => List::nil(),
    }
}

fn method_body(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::MethodDecl { body, .. } => *body,
        _ => None,
    }
}

fn method_symbol(ctx: &CompilerContext, tree: TreeId) -> Option<SymbolId> {
    match &ctx.tree(tree).kind {
        TreeKind::MethodDecl { sym, .. } => *sym,
        _ => None,
    }
}

fn method_params(ctx: &CompilerContext, tree: TreeId) -> List<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::MethodDecl { params, .. } => params.clone(),
        _ => List::nil(),
    }
}

fn return_type_of(ctx: &CompilerContext, method_type: TypeId) -> TypeId {
    match &ctx.typ(method_type).kind {
        crate::code::type_::TypeKindData::MethodType { restype, .. } => {
            restype.expect("method return type")
        }
        _ => panic!("ClassCastException: MethodType expected"),
    }
}

/// Java: MethodType.getThrownTypes()
#[allow(clippy::collapsible_match)]
fn thrown_types_of(ctx: &CompilerContext, method_type: TypeId) -> List<TypeId> {
    match &ctx.typ(method_type).kind {
        crate::code::type_::TypeKindData::MethodType { thrown, .. } => thrown.clone(),
        crate::code::type_::TypeKindData::ForAll { qtype, .. } => match qtype {
            Some(qtype) => thrown_types_of(ctx, *qtype),
            None => List::nil(),
        },
        _ => List::nil(),
    }
}

fn case_labels(ctx: &CompilerContext, case: TreeId) -> List<TreeId> {
    match &ctx.tree(case).kind {
        TreeKind::Case { labels, .. } => labels.clone(),
        _ => List::nil(),
    }
}

fn case_stats(ctx: &CompilerContext, case: TreeId) -> List<TreeId> {
    match &ctx.tree(case).kind {
        TreeKind::Case { stats, .. } => stats.clone(),
        _ => List::nil(),
    }
}

fn case_body(ctx: &CompilerContext, case: TreeId) -> Option<TreeId> {
    match &ctx.tree(case).kind {
        TreeKind::Case { body, .. } => *body,
        _ => None,
    }
}

fn case_is_rule(ctx: &CompilerContext, case: TreeId) -> bool {
    match &ctx.tree(case).kind {
        TreeKind::Case { case_kind, .. } => matches!(case_kind, CaseKind::RULE),
        _ => false,
    }
}

fn is_pattern_tree(ctx: &CompilerContext, tree: TreeId) -> bool {
    matches!(
        ctx.tree(tree).tag,
        Tag::BINDINGPATTERN | Tag::PARENTHESIZEDPATTERN | Tag::GUARDPATTERN | Tag::DEFAULTCASELABEL
    )
}

/// Java: JCMultiCatch/alternatives — the catch sub-clauses of a catch clause
/// (TreeInfo.isMultiCatch: the catch parameter type is a type union).
fn catch_sub_clauses(ctx: &CompilerContext, catcher: TreeId, param: Option<TreeId>) -> List<TreeId> {
    match param {
        Some(param) => match var_decl_vartype(ctx, param) {
            Some(vartype) => match &ctx.tree(vartype).kind {
                TreeKind::TypeUnion { alternatives, .. } => alternatives.clone(),
                _ => List::of(vartype),
            },
            None => List::nil(),
        },
        None => {
            let _ = catcher;
            List::nil()
        }
    }
}

fn var_decl_vartype(ctx: &CompilerContext, param: TreeId) -> Option<TreeId> {
    match &ctx.tree(param).kind {
        TreeKind::VariableDecl { vartype, .. } => *vartype,
        _ => None,
    }
}

/// Java: FlowAnalyzer.isExceptionOrThrowable(Type) (Flow.java:1630-1633)
fn is_exception_or_throwable(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    exc: TypeId,
) -> bool {
    let tsym = ctx.typ(exc).tsym;
    tsym == ctx.typ(syms.throwable_type).tsym || tsym == ctx.typ(syms.exception_type).tsym
}

/// Diagnostic TypeArg placeholder (ADR 0002 P4).
fn type_place(
    ctx: &CompilerContext,
    ty: TypeId,
) -> crate::resources::diagnostic_contract::TypeArg {
    let text = ctx
        .typ(ty)
        .tsym
        .map(|tsym| crate::comp::resolve::symbol_display_name(ctx, tsym))
        .unwrap_or_else(|| format!("{:?}", ctx.typ(ty).get_tag()));
    crate::resources::diagnostic_contract::TypeArg::placeholder(text)
}


/// FlowVisitor: the ported Tree visitor for the flow analyzers.
/// The default visit methods are the ported TreeScanner navigational bodies
/// (subclasses override only the visits they care about, like upstream).
pub trait FlowVisitor: TreeVisitor + Sized {
    /// Java: TreeScanner.scan(JCTree tree)
    fn scan(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) {
        if let Some(tree) = tree {
            accept(ctx, tree, self);
        }
    }

    /// Java: TreeScanner.scan(List<? extends JCTree> trees)
    fn scan_list(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) {
        for tree in trees.iter() {
            self.scan(ctx, Some(tree));
        }
    }

    /// Java: the nullable-list scan helper
    fn scan_list_opt(&mut self, ctx: &mut CompilerContext, trees: Option<List<TreeId>>) {
        if let Some(trees) = trees {
            self.scan_list(ctx, &trees);
        }
    }


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

/// Java: class Flow.AliveAnalyzer extends BaseAnalyzer (Flow.java:471-958)
pub struct AliveAnalyzer<'a> {
    flow: &'a Flow,
    pending_exits: List<PendingExit>,
    pub(crate) alive: Liveness,
    lint: Rc<Lint>,
}

impl<'a> AliveAnalyzer<'a> {
    /// Java: implicit AliveAnalyzer() (the upstream inner class captures Flow)
    pub fn new(flow: &'a Flow) -> AliveAnalyzer<'a> {
        AliveAnalyzer {
            flow,
            pending_exits: List::nil(),
            alive: Liveness::ALIVE,
            lint: Rc::clone(&flow.lint),
        }
    }

    /// Java: void scanDef(JCTree tree) (Flow.java:486-491)
    fn scan_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.scan_stat(ctx, tree);
        if ctx.tree(tree).has_tag(Tag::BLOCK) && self.alive == Liveness::DEAD {
            self.flow
                .error(ctx.tree(tree).pos, errors::initializer_must_be_able_to_complete_normally());
        }
    }

    /// Java: void scanStat(JCTree tree) (Flow.java:494-501)
    fn scan_stat(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if self.alive == Liveness::DEAD {
            self.flow
                .error(ctx.tree(tree).pos, errors::unreachable_stmt());
            if !ctx.tree(tree).has_tag(Tag::SKIP) {
                self.alive = Liveness::RECOVERY;
            }
        }
        self.scan(ctx, Some(tree));
    }

    /// Java: void scanStats(List<? extends JCStatement> trees) (Flow.java:504-508)
    fn scan_stats(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) {
        for tree in trees.iter() {
            self.scan_stat(ctx, tree);
        }
    }

    /// Java: private void clearPendingExits(boolean inMethod) (Flow.java:585-592)
    fn clear_pending_exits(&mut self, ctx: &mut CompilerContext, in_method: bool) {
        let exits = std::mem::take(&mut self.pending_exits);
        let mut current = exits;
        while current.non_empty() {
            let exit = current.head().clone();
            let tree = exit.tree();
            let is_return = in_method && ctx.tree(tree).has_tag(Tag::RETURN);
            let has_error = self.flow.log.borrow().has_error_on(Some(Rc::new(
                SimpleDiagnosticPosition::new(ctx.tree(tree).pos),
            )));
            assert!(is_return || has_error, "AssertionError: clearPendingExits (Flow.java:590)");
            current = current.tail();
        }
    }

    /// Java: private void handleConstantCaseLabel(Set<Symbol> constants,
    /// JCCaseLabel pat) (Flow.java:746-760)
    fn handle_constant_case_label(
        &mut self,
        ctx: &mut CompilerContext,
        constants: &mut Option<HashSet<SymbolId>>,
        pat: TreeId,
    ) {
        let Some(constants) = constants else {
            return;
        };
        if is_pattern_tree(ctx, pat) {
            let pattern_type = tree_info::primary_pattern_type(ctx, pat);
            if pattern_type.unconditional()
                && let Some(ty) = pattern_type.ty
                    && let Some(tsym) = ctx.typ(ty).tsym {
                        constants.insert(tsym);
                    }
        } else if ctx.tree(pat).has_tag(Tag::IDENT) {
            let sym = tree_info::symbol(ctx, pat);
            if let Some(sym) = sym
                && (ctx.sym(sym).flags_field & flags::ENUM as i64) != 0 {
                    constants.insert(sym);
                }
        }
    }

    /// Java: private void transitiveCovers(Set<Symbol> covered)
    /// (Flow.java:752-786): close the covered set transitively (enum constants
    /// cover their enum type, sealed permitted subclasses cover the sealed
    /// supertype).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - covered: the covered symbol set (updated in place).
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: Flow.java:752-786.
    fn transitive_covers(&mut self, ctx: &mut CompilerContext, covered: &mut HashSet<SymbolId>) {
        let mut todo: Vec<SymbolId> = covered.iter().copied().collect();
        while let Some(sym) = todo.pop() {
            match ctx.sym(sym).kind {
                Kind::VAR => {
                    let Some(owner) = ctx.sym(sym).owner else {
                        continue;
                    };
                    // Java: sym.owner.members().getSymbols(s -> s.isEnum() &&
                    // s.kind == VAR); a null member scope is unreachable for an
                    // entered class symbol and is treated as the empty scope.
                    let constants = match ctx.sym(owner).members() {
                        Some(members) => {
                            let scope = ctx.scope(members).clone();
                            scope.get_symbols(
                                members,
                                crate::code::scope::LookupKind::NON_RECURSIVE,
                                &|_| true,
                                ctx,
                            )
                        }
                        None => Vec::new(),
                    };
                    let mut has_all = true;
                    for constant in constants {
                        if ctx.sym(constant).is_enum()
                            && ctx.sym(constant).kind == Kind::VAR
                            && !covered.contains(&constant)
                        {
                            has_all = false;
                            break;
                        }
                    }
                    if has_all && covered.insert(owner) {
                        todo.push(owner);
                    }
                }
                Kind::TYP => {
                    let sym_type = ctx.sym(sym).ty.expect("class symbol type");
                    let supers = {
                        let syms = self.flow.syms.borrow();
                        self.flow.types.direct_supertypes(ctx, &syms, sym_type)
                    };
                    let mut current = supers;
                    while current.non_empty() {
                        let sup = *current.head();
                        current = current.tail();
                        let Some(sup_tsym) = ctx.typ(sup).tsym else {
                            continue;
                        };
                        if ctx.sym(sup_tsym).kind == Kind::TYP
                            && ctx.sym(sup_tsym).is_abstract()
                            && ctx.sym(sup_tsym).is_sealed()
                        {
                            let permitted = crate::code::symbol::Symbol::permitted_subs(ctx, sup_tsym);
                            let mut has_all = true;
                            for sub in permitted.iter() {
                                if !covered.contains(&sub) {
                                    has_all = false;
                                    break;
                                }
                            }
                            if has_all && covered.insert(sup_tsym) {
                                todo.push(sup_tsym);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Java: private boolean isExhaustive(Type seltype, Set<Symbol> covered)
    /// (Flow.java:788-803).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - seltype: the switch selector type;
    ///   - covered: the covered symbol set (updated by transitiveCovers).
    ///
    /// # Returns
    ///   - true iff the selector type is exhaustively covered.
    ///
    /// # Panics
    ///   - a type-variable selector without an upper bound (unreachable).
    ///
    /// Upstream: Flow.java:788-803.
    fn is_exhaustive(
        &mut self,
        ctx: &mut CompilerContext,
        seltype: TypeId,
        covered: &mut HashSet<SymbolId>,
    ) -> bool {
        self.transitive_covers(ctx, covered);
        match ctx.typ(seltype).get_tag() {
            TypeTag::CLASS => {
                if self.flow.types.is_compound_public(ctx, seltype) {
                    if ctx.typ(seltype).is_intersection() {
                        let components = ctx.typ(seltype).get_components();
                        let mut current = components;
                        while current.non_empty() {
                            let component = *current.head();
                            current = current.tail();
                            if self.is_exhaustive(ctx, component, covered) {
                                return true;
                            }
                        }
                    }
                    false
                } else {
                    // Java: covered.contains(seltype.tsym).
                    ctx.typ(seltype)
                        .tsym
                        .map(|tsym| covered.contains(&tsym))
                        .unwrap_or(false)
                }
            }
            TypeTag::TYPEVAR => {
                let upper = ctx.typ(seltype)
                    .get_upper_bound()
                    .expect("type variable upper bound");
                self.is_exhaustive(ctx, upper, covered)
            }
            _ => false,
        }
    }

    /// Java: public void analyzeTree(Env, TreeMaker) (Flow.java:939-957)
    #[allow(clippy::collapsible_match)]
    pub fn analyze_tree(
        &mut self,
        ctx: &mut CompilerContext,
        env: &Rc<Env<AttrContext>>,
        tree: TreeId,
    ) {
        *self.flow.attr_env.borrow_mut() = Some(Rc::clone(env));
        self.pending_exits = List::nil();
        self.alive = Liveness::ALIVE;
        self.scan(ctx, Some(tree));
        self.pending_exits = List::nil();
    }
}

impl<'a> BaseAnalyzer for AliveAnalyzer<'a> {
    fn pending_exits_mut(&mut self) -> &mut List<PendingExit> {
        &mut self.pending_exits
    }

    fn mark_dead(&mut self, _ctx: &mut CompilerContext) {
        self.alive = Liveness::DEAD;
    }
}

flow_tree_visitor_delegate!(AliveAnalyzer<'a>, 'a);

impl<'a> FlowVisitor for AliveAnalyzer<'a> {
    fn visit_package_def(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let Some(class_sym) = class_symbol(ctx, tree) else {
            return;
        };
        let alive_prev = self.alive;
        let pending_exits_prev = self.pending_exits.clone();
        let lint_prev = Rc::clone(&self.lint);
        self.pending_exits = List::nil();
        self.lint = self.flow.augmented_lint(ctx, class_sym);
        let defs = class_defs(ctx, tree);
        // process all the static initializers
        for def in defs.iter() {
            let is_static = (tree_info::flags(ctx, def) & flags::STATIC as i64) != 0;
            if !ctx.tree(def).has_tag(Tag::METHODDEF) && is_static {
                self.scan_def(ctx, def);
                self.clear_pending_exits(ctx, false);
            }
        }
        // process all the instance initializers
        for def in defs.iter() {
            let is_static = (tree_info::flags(ctx, def) & flags::STATIC as i64) != 0;
            if !ctx.tree(def).has_tag(Tag::METHODDEF) && !is_static {
                self.scan_def(ctx, def);
                self.clear_pending_exits(ctx, false);
            }
        }
        // process all the methods
        for def in defs.iter() {
            if ctx.tree(def).has_tag(Tag::METHODDEF) {
                self.scan(ctx, Some(def));
            }
        }
        self.pending_exits = pending_exits_prev;
        self.alive = alive_prev;
        self.lint = lint_prev;
    }

    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let Some(body) = method_body(ctx, tree) else {
            return;
        };
        let method_sym = method_symbol(ctx, tree);
        let lint_prev = Rc::clone(&self.lint);
        if let Some(method_sym) = method_sym {
            self.lint = self.flow.augmented_lint(ctx, method_sym);
        }
        assert!(
            self.pending_exits.is_empty(),
            "AssertionError: pendingExits.isEmpty() (Flow.java:570)"
        );
        self.alive = Liveness::ALIVE;
        self.scan_stat(ctx, body);
        if let TreeKind::MethodDecl {
            completes_normally, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *completes_normally = self.alive != Liveness::DEAD;
        }
        if self.alive == Liveness::ALIVE
            && let Some(method_sym) = method_sym {
                let method_type = ctx.sym(method_sym).ty.expect("method type");
                let return_type = return_type_of(ctx, method_type);
                if !ctx.typ(return_type).has_tag(TypeTag::VOID) {
                    self.flow
                        .error(tree_info::diag_end_pos(ctx, body), errors::missing_ret_stmt());
                }
            }
        self.clear_pending_exits(ctx, true);
        self.lint = lint_prev;
    }

    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (init, sym) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl { init, sym, .. } => (*init, *sym),
            _ => panic!("expected a variable declaration"),
        };
        if let Some(init) = init {
            let lint_prev = Rc::clone(&self.lint);
            if let Some(sym) = sym {
                self.lint = self.flow.augmented_lint(ctx, sym);
            }
            self.scan(ctx, Some(init));
            self.lint = lint_prev;
        }
    }

    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let stats = match &ctx.tree(tree).kind {
            TreeKind::Block { stats, .. } => stats.clone(),
            _ => panic!("expected a block"),
        };
        self.scan_stats(ctx, &stats);
    }

    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, cond) = match &ctx.tree(tree).kind {
            TreeKind::DoWhileLoop { body, cond } => (*body, *cond),
            _ => panic!("expected a do loop"),
        };
        let prev_pending_exits = std::mem::take(&mut self.pending_exits);
        self.scan_stat(ctx, body.expect("do loop body"));
        self.alive = self.alive.or(self.resolve_continues(ctx, tree));
        self.scan(ctx, cond);
        let cond_is_true = cond
            .and_then(|cond| ctx.tree(cond).ty)
            .map(|ty| ctx.typ(ty).is_true())
            .unwrap_or(false);
        self.alive = self.alive.and_bool(!cond_is_true);
        let breaks = self.resolve_breaks(ctx, tree, prev_pending_exits);
        self.alive = self.alive.or(breaks);
    }

    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, body) = match &ctx.tree(tree).kind {
            TreeKind::WhileLoop { cond, body } => (*cond, *body),
            _ => panic!("expected a while loop"),
        };
        let prev_pending_exits = std::mem::take(&mut self.pending_exits);
        self.scan(ctx, cond);
        let cond_type = cond.and_then(|cond| ctx.tree(cond).ty);
        let cond_is_false = cond_type.map(|ty| ctx.typ(ty).is_false()).unwrap_or(false);
        self.alive = Liveness::from_bool(!cond_is_false);
        self.scan_stat(ctx, body.expect("while body"));
        self.alive = self.alive.or(self.resolve_continues(ctx, tree));
        let breaks = self.resolve_breaks(ctx, tree, prev_pending_exits);
        let cond_is_true = cond_type.map(|ty| ctx.typ(ty).is_true()).unwrap_or(false);
        self.alive = breaks.or_bool(!cond_is_true);
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
        let prev_pending_exits = std::mem::take(&mut self.pending_exits);
        self.scan_stats(ctx, &init);
        let cond_type = cond.and_then(|cond| ctx.tree(cond).ty);
        if let Some(cond) = cond {
            self.scan(ctx, Some(cond));
            let cond_is_false = cond_type.map(|ty| ctx.typ(ty).is_false()).unwrap_or(false);
            self.alive = Liveness::from_bool(!cond_is_false);
        } else {
            self.alive = Liveness::ALIVE;
        }
        self.scan_stat(ctx, body.expect("for body"));
        self.alive = self.alive.or(self.resolve_continues(ctx, tree));
        for stat in step.iter() {
            self.scan(ctx, Some(stat));
        }
        let breaks = self.resolve_breaks(ctx, tree, prev_pending_exits);
        let cond_is_true = cond_type.map(|ty| ctx.typ(ty).is_true()).unwrap_or(false);
        self.alive = breaks.or_bool(cond.is_some() && !cond_is_true);
    }

    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (var, expr, body) = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop { var, expr, body } => (*var, *expr, *body),
            _ => panic!("expected an enhanced for loop"),
        };
        if let Some(var) = var {
            <Self as FlowVisitor>::visit_var_def(self, ctx, var);
        }
        let prev_pending_exits = std::mem::take(&mut self.pending_exits);
        self.scan(ctx, expr);
        self.scan_stat(ctx, body.expect("foreach body"));
        self.alive = self.alive.or(self.resolve_continues(ctx, tree));
        self.resolve_breaks(ctx, tree, prev_pending_exits);
        self.alive = Liveness::ALIVE;
    }

    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let body = match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement { body, .. } => *body,
            _ => panic!("expected a labelled statement"),
        };
        let prev_pending_exits = std::mem::take(&mut self.pending_exits);
        self.scan_stat(ctx, body.expect("labelled body"));
        let breaks = self.resolve_breaks(ctx, tree, prev_pending_exits);
        self.alive = self.alive.or(breaks);
    }

    fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases, pattern_switch, has_total_pattern) = match &ctx.tree(tree).kind {
            TreeKind::Switch {
                selector,
                cases,
                pattern_switch,
                has_total_pattern,
                ..
            } => (*selector, cases.clone(), *pattern_switch, *has_total_pattern),
            _ => panic!("expected a switch"),
        };
        let prev_pending_exits = std::mem::take(&mut self.pending_exits);
        self.scan(ctx, selector);
        let mut found_null = false;
        for case in cases.iter() {
            for label in case_labels(ctx, case).iter() {
                if tree_info::is_null(ctx, label) {
                    found_null = true;
                }
            }
        }
        let exhaustive_switch = pattern_switch || found_null;
        let mut constants: Option<HashSet<SymbolId>> =
            if exhaustive_switch { Some(HashSet::new()) } else { None };
        let mut remaining = cases.clone();
        while remaining.non_empty() {
            self.alive = Liveness::ALIVE;
            let case = *remaining.head();
            for label in case_labels(ctx, case).iter() {
                self.scan(ctx, Some(label));
                self.handle_constant_case_label(ctx, &mut constants, label);
            }
            let stats = case_stats(ctx, case);
            self.scan_stats(ctx, &stats);
            let next = remaining.tail();
            if self.alive != Liveness::DEAD && case_is_rule(ctx, case) {
                scan_synthetic_break(self.flow, self, ctx, tree);
                self.alive = Liveness::DEAD;
            }
            if self.alive == Liveness::ALIVE
                && self.lint.is_enabled(LintCategory::FALLTHROUGH)
                && !stats.is_empty()
                && next.non_empty()
            {
                let next_case = *next.head();
                self.flow.warning(
                    LintCategory::FALLTHROUGH,
                    ctx.tree(next_case).pos,
                    warnings::possible_fall_through_into_case(),
                );
            }
            remaining = next;
        }
        if !has_total_pattern && exhaustive_switch {
            let selector_type = selector.and_then(|selector| ctx.tree(selector).ty);
            let is_error_enum_switch = match (selector, selector_type) {
                (Some(selector), Some(selector_type)) => {
                    tree_info::is_error_enum_switch(ctx, selector, &cases)
                        || ctx.typ(selector_type).has_tag(TypeTag::ERROR)
                }
                _ => false,
            };
            let exhaustive = if is_error_enum_switch {
                true
            } else if let (Some(selector_type), Some(constants)) =
                (selector_type, constants.as_mut())
            {
                self.is_exhaustive(ctx, selector_type, constants)
            } else {
                false
            };
            if !exhaustive {
                self.flow
                    .error(ctx.tree(tree).pos, errors::not_exhaustive_statement());
            }
        }
        if !has_total_pattern {
            self.alive = Liveness::ALIVE;
        }
        let breaks = self.resolve_breaks(ctx, tree, prev_pending_exits);
        self.alive = self.alive.or(breaks);
    }

    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases, has_total_pattern) = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression {
                selector,
                cases,
                has_total_pattern,
                ..
            } => (*selector, cases.clone(), *has_total_pattern),
            _ => panic!("expected a switch expression"),
        };
        let prev_pending_exits = std::mem::take(&mut self.pending_exits);
        self.scan(ctx, selector);
        let mut constants: HashSet<SymbolId> = HashSet::new();
        let prev_alive = self.alive;
        let mut remaining = cases.clone();
        while remaining.non_empty() {
            self.alive = Liveness::ALIVE;
            let case = *remaining.head();
            for label in case_labels(ctx, case).iter() {
                self.scan(ctx, Some(label));
                let mut some_constants = Some(constants.clone());
                self.handle_constant_case_label(ctx, &mut some_constants, label);
                if let Some(updated) = some_constants {
                    constants = updated;
                }
            }
            let stats = case_stats(ctx, case);
            self.scan_stats(ctx, &stats);
            let next = remaining.tail();
            if self.alive == Liveness::ALIVE {
                if case_is_rule(ctx, case) {
                    let body = case_body(ctx, case);
                    self.flow.error(
                        body.map(|body| tree_info::diag_end_pos(ctx, body))
                            .unwrap_or(ctx.tree(case).pos),
                        errors::rule_completes_normally(),
                    );
                } else if next.is_empty() {
                    self.flow.error(
                        tree_info::diag_end_pos(ctx, tree),
                        errors::switch_expression_completes_normally(),
                    );
                }
            }
            remaining = next;
        }
        if !has_total_pattern {
            let selector_type = selector.and_then(|selector| ctx.tree(selector).ty);
            let is_error_enum_switch = match (selector, selector_type) {
                (Some(selector), Some(selector_type)) => {
                    tree_info::is_error_enum_switch(ctx, selector, &cases)
                        || ctx.typ(selector_type).has_tag(TypeTag::ERROR)
                }
                _ => false,
            };
            let exhaustive = if is_error_enum_switch {
                true
            } else if let Some(selector_type) = selector_type {
                self.is_exhaustive(ctx, selector_type, &mut constants)
            } else {
                false
            };
            if !exhaustive {
                self.flow
                    .error(ctx.tree(tree).pos, errors::not_exhaustive());
            }
        }
        self.alive = prev_alive;
        let yields = self.resolve_yields(ctx, tree, prev_pending_exits);
        self.alive = self.alive.or(yields);
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
        let prev_pending_exits = std::mem::take(&mut self.pending_exits);
        for resource in resources.iter() {
            if matches!(ctx.tree(resource).kind, TreeKind::VariableDecl { .. }) {
                <Self as FlowVisitor>::visit_var_def(self, ctx, resource);
            } else {
                self.scan(ctx, Some(resource));
            }
        }
        self.scan_stat(ctx, body.expect("try body"));
        let mut alive_end = self.alive;
        for catcher in catchers.iter() {
            self.alive = Liveness::ALIVE;
            let (param, catch_body) = match &ctx.tree(catcher).kind {
                TreeKind::Catch { param, body, .. } => (*param, *body),
                _ => panic!("expected a catch clause"),
            };
            if let Some(param) = param {
                self.scan(ctx, Some(param));
            }
            self.scan_stat(ctx, catch_body.expect("catch body"));
            alive_end = alive_end.or(self.alive);
        }
        if let Some(finalizer) = finalizer {
            let exits = std::mem::replace(&mut self.pending_exits, prev_pending_exits);
            self.alive = Liveness::ALIVE;
            self.scan_stat(ctx, finalizer);
            if let TreeKind::Try {
                finally_can_complete_normally,
                ..
            } = &mut ctx.tree_mut(tree).kind
            {
                *finally_can_complete_normally = self.alive != Liveness::DEAD;
            }
            if self.alive == Liveness::DEAD {
                if self.lint.is_enabled(LintCategory::FINALLY) {
                    self.flow.warning(
                        LintCategory::FINALLY,
                        tree_info::diag_end_pos(ctx, finalizer),
                        warnings::finally_cannot_complete(),
                    );
                }
            } else {
                self.pending_exits = exits;
                self.alive = alive_end;
            }
        } else {
            self.alive = alive_end;
            self.pending_exits = prev_pending_exits;
        }
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
        self.scan_stat(ctx, thenpart.expect("if then"));
        if let Some(elsepart) = elsepart {
            let alive_after_then = self.alive;
            self.alive = Liveness::ALIVE;
            self.scan_stat(ctx, elsepart);
            self.alive = self.alive.or(alive_after_then);
        } else {
            self.alive = Liveness::ALIVE;
        }
    }

    fn visit_break(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.record_exit(ctx, PendingExit::new(tree));
    }

    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, .. } => *value,
            _ => panic!("expected a yield"),
        };
        self.scan(ctx, value);
        self.record_exit(ctx, PendingExit::new(tree));
    }

    fn visit_continue(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.record_exit(ctx, PendingExit::new(tree));
    }

    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Return { expr } => *expr,
            _ => panic!("expected a return"),
        };
        self.scan(ctx, expr);
        self.record_exit(ctx, PendingExit::new(tree));
    }

    fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Throw { expr } => *expr,
            _ => panic!("expected a throw"),
        };
        self.scan(ctx, expr);
        self.mark_dead(ctx);
    }

    fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (meth, args) = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation { meth, args, .. } => (*meth, args.clone()),
            _ => panic!("expected a method invocation"),
        };
        self.scan(ctx, meth);
        self.scan_list(ctx, &args);
    }

    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (encl, args, def) = match &ctx.tree(tree).kind {
            TreeKind::NewClass { encl, args, def, .. } => (*encl, args.clone(), *def),
            _ => panic!("expected a new class"),
        };
        self.scan(ctx, encl);
        self.scan_list(ctx, &args);
        self.scan(ctx, def);
    }

    fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let Some(ty) = ctx.tree(tree).ty
            && ctx.typ(ty).has_tag(TypeTag::ERROR) {
                return;
            }
        let (body, params) = match &ctx.tree(tree).kind {
            TreeKind::Lambda { body, params, .. } => (*body, params.clone()),
            _ => panic!("expected a lambda"),
        };
        let prev_pending = std::mem::take(&mut self.pending_exits);
        let prev_alive = self.alive;
        self.alive = Liveness::ALIVE;
        for param in params.iter() {
            self.scan(ctx, Some(param));
        }
        if let Some(body) = body {
            self.scan_stat(ctx, body);
        }
        if let TreeKind::Lambda {
            can_complete_normally,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *can_complete_normally = self.alive != Liveness::DEAD;
        }
        self.pending_exits = prev_pending;
        self.alive = prev_alive;
    }

    fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (dims, elems) = match &ctx.tree(tree).kind {
            TreeKind::NewArray { dims, elems, .. } => (dims.clone(), elems.clone()),
            _ => panic!("expected a new array"),
        };
        self.scan_list(ctx, &dims);
        if let Some(elems) = elems {
            self.scan_list(ctx, &elems);
        }
    }

    fn visit_module_def(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}
}

/// Java: class Flow.LambdaAliveAnalyzer extends AliveAnalyzer
/// (Flow.java:1545-1576)
pub struct LambdaAliveAnalyzer<'a> {
    inner: AliveAnalyzer<'a>,
    in_lambda: bool,
}

impl<'a> LambdaAliveAnalyzer<'a> {
    /// Java: AliveAnalyzer.analyzeTree(Env, JCTree, TreeMaker)
    pub fn analyze_tree(
        &mut self,
        ctx: &mut CompilerContext,
        env: &Rc<Env<AttrContext>>,
        tree: TreeId,
    ) {
        self.inner.analyze_tree(ctx, env, tree);
    }

    pub fn new(flow: &'a Flow) -> LambdaAliveAnalyzer<'a> {
        LambdaAliveAnalyzer {
            inner: AliveAnalyzer::new(flow),
            in_lambda: false,
        }
    }
}

flow_tree_visitor_delegate!(LambdaAliveAnalyzer<'a>, 'a);

impl<'a> FlowVisitor for LambdaAliveAnalyzer<'a> {
    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.inner.record_exit(ctx, PendingExit::new(tree));
    }

    fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if self.in_lambda || lambda_body_is_expression(ctx, tree) {
            return;
        }
        self.in_lambda = true;
        <AliveAnalyzer as FlowVisitor>::visit_lambda(&mut self.inner, ctx, tree);
        self.in_lambda = false;
    }

    fn visit_class_def(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}
}

impl<'a> BaseAnalyzer for LambdaAliveAnalyzer<'a> {
    fn pending_exits_mut(&mut self) -> &mut List<PendingExit> {
        self.inner.pending_exits_mut()
    }

    fn mark_dead(&mut self, ctx: &mut CompilerContext) {
        self.inner.mark_dead(ctx)
    }
}

/// Java: class Flow.SnippetAliveAnalyzer extends AliveAnalyzer
/// (Flow.java:1577-1590)
pub struct SnippetAliveAnalyzer<'a> {
    inner: AliveAnalyzer<'a>,
}

impl<'a> SnippetAliveAnalyzer<'a> {
    /// Java: AliveAnalyzer.analyzeTree(Env, JCTree, TreeMaker)
    pub fn analyze_tree(
        &mut self,
        ctx: &mut CompilerContext,
        env: &Rc<Env<AttrContext>>,
        tree: TreeId,
    ) {
        self.inner.analyze_tree(ctx, env, tree);
    }

    pub fn new(flow: &'a Flow) -> SnippetAliveAnalyzer<'a> {
        SnippetAliveAnalyzer {
            inner: AliveAnalyzer::new(flow),
        }
    }

    /// Java: public boolean isAlive() (Flow.java:1586-1588)
    pub fn is_alive(&self) -> bool {
        self.inner.alive != Liveness::DEAD
    }
}

flow_tree_visitor_delegate!(SnippetAliveAnalyzer<'a>, 'a);

impl<'a> FlowVisitor for SnippetAliveAnalyzer<'a> {
    fn visit_class_def(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_lambda(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}
}

impl<'a> BaseAnalyzer for SnippetAliveAnalyzer<'a> {
    fn pending_exits_mut(&mut self) -> &mut List<PendingExit> {
        self.inner.pending_exits_mut()
    }

    fn mark_dead(&mut self, ctx: &mut CompilerContext) {
        self.inner.mark_dead(ctx)
    }
}

/// Java: class Flow.SnippetBreakAnalyzer extends AliveAnalyzer
/// (Flow.java:1591-1642)
pub struct SnippetBreakAnalyzer<'a> {
    inner: AliveAnalyzer<'a>,
    seen_trees: HashSet<TreeId>,
    breaks_out: bool,
}

impl<'a> SnippetBreakAnalyzer<'a> {
    /// Java: AliveAnalyzer.analyzeTree(Env, JCTree, TreeMaker)
    pub fn analyze_tree(
        &mut self,
        ctx: &mut CompilerContext,
        env: &Rc<Env<AttrContext>>,
        tree: TreeId,
    ) {
        self.inner.analyze_tree(ctx, env, tree);
    }

    pub fn new(flow: &'a Flow) -> SnippetBreakAnalyzer<'a> {
        SnippetBreakAnalyzer {
            inner: AliveAnalyzer::new(flow),
            seen_trees: HashSet::new(),
            breaks_out: false,
        }
    }

    /// Java: public boolean breaksOut() (Flow.java:1638-1640)
    pub fn breaks_out(&self) -> bool {
        self.breaks_out
    }
}

flow_tree_visitor_delegate!(SnippetBreakAnalyzer<'a>, 'a);

impl<'a> FlowVisitor for SnippetBreakAnalyzer<'a> {
    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.seen_trees.insert(tree);
        <AliveAnalyzer as FlowVisitor>::visit_labelled(&mut self.inner, ctx, tree);
    }

    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.seen_trees.insert(tree);
        <AliveAnalyzer as FlowVisitor>::visit_while_loop(&mut self.inner, ctx, tree);
    }

    fn visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.seen_trees.insert(tree);
        <AliveAnalyzer as FlowVisitor>::visit_for_loop(&mut self.inner, ctx, tree);
    }

    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.seen_trees.insert(tree);
        <AliveAnalyzer as FlowVisitor>::visit_foreach_loop(&mut self.inner, ctx, tree);
    }

    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.seen_trees.insert(tree);
        <AliveAnalyzer as FlowVisitor>::visit_do_loop(&mut self.inner, ctx, tree);
    }

    fn visit_break(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let target = match &ctx.tree(tree).kind {
            TreeKind::Break { target, .. } => *target,
            _ => None,
        };
        self.breaks_out |= self.inner.alive == Liveness::ALIVE
            && !target
                .map(|target| self.seen_trees.contains(&target))
                .unwrap_or(false);
        <AliveAnalyzer as FlowVisitor>::visit_break(&mut self.inner, ctx, tree);
    }
}

impl<'a> BaseAnalyzer for SnippetBreakAnalyzer<'a> {
    fn pending_exits_mut(&mut self) -> &mut List<PendingExit> {
        self.inner.pending_exits_mut()
    }

    fn mark_dead(&mut self, ctx: &mut CompilerContext) {
        self.inner.mark_dead(ctx)
    }
}

/// Java: JCLambda.bodyKind (JCExpression vs JCBlock)
#[allow(clippy::collapsible_match)]
fn lambda_body_is_expression(ctx: &CompilerContext, tree: TreeId) -> bool {
    match &ctx.tree(tree).kind {
        TreeKind::Lambda { body, .. } => match body {
            Some(body) => !ctx.tree(*body).has_tag(Tag::BLOCK),
            None => false,
        },
        _ => false,
    }
}

/// Java: Flow.BaseAnalyzer.scanSyntheticBreak(TreeMaker make, JCTree swtch)
/// (Flow.java:456-467)
fn scan_synthetic_break<V: FlowVisitor>(
    flow: &Flow,
    v: &mut V,
    ctx: &mut CompilerContext,
    swtch: TreeId,
) {
    let make = flow.make.borrow().as_ref().map(Rc::clone);
    let Some(make) = make else {
        panic!("AssertionError: Flow.make is null");
    };
    if ctx.tree(swtch).has_tag(Tag::SWITCH_EXPRESSION) {
        let brk = {
            let mut maker = make.borrow_mut();
            maker.at(crate::util::position::NOPOS);
            maker.yield_(ctx, None)
        };
        if let TreeKind::Yield { target, .. } = &mut ctx.tree_mut(brk).kind {
            *target = Some(swtch);
        }
        v.scan(ctx, Some(brk));
    } else {
        let brk = {
            let mut maker = make.borrow_mut();
            maker.at(crate::util::position::NOPOS);
            maker.break_(ctx, None)
        };
        if let TreeKind::Break { target, .. } = &mut ctx.tree_mut(brk).kind {
            *target = Some(swtch);
        }
        v.scan(ctx, Some(brk));
    }
}

/// Java: class Flow.FlowAnalyzer extends BaseAnalyzer (Flow.java:959-1544)
pub struct FlowAnalyzer<'a> {
    flow: &'a Flow,
    pending_exits: List<PendingExit>,
    precise_rethrow_types: HashMap<SymbolId, List<TypeId>>,
    caught: List<TypeId>,
    thrown: List<TypeId>,
    class_def: Option<TreeId>,
    /// Java: FlowAnalyzer.returnType (Flow.java:964) — read by the upstream
    /// visitReturn path only through checkExpr; kept for structure parity.
    #[allow(dead_code)]
    return_type: Option<TypeId>,
    lint: Rc<Lint>,
    /// Java: LambdaFlowAnalyzer.inferredThrownTypes / inLambda
    /// (Flow.java:1686-1722) — Porting rules adapter: same struct, mode flag.
    inferred_thrown_types: List<TypeId>,
    lambda_mode: bool,
    in_lambda: bool,
}

impl<'a> FlowAnalyzer<'a> {
    /// Java: public FlowAnalyzer() (Flow.java:966-980)
    pub fn new(flow: &'a Flow) -> FlowAnalyzer<'a> {
        FlowAnalyzer {
            flow,
            pending_exits: List::nil(),
            precise_rethrow_types: HashMap::new(),
            caught: List::nil(),
            thrown: List::nil(),
            class_def: None,
            return_type: None,
            lint: Rc::clone(&flow.lint),
            inferred_thrown_types: List::nil(),
            lambda_mode: false,
            in_lambda: false,
        }
    }

    /// Java: LambdaFlowAnalyzer() (Flow.java:1686-1722)
    pub fn new_lambda(flow: &'a Flow) -> FlowAnalyzer<'a> {
        let mut analyzer = FlowAnalyzer::new(flow);
        analyzer.lambda_mode = true;
        analyzer
    }

    /// Java: void errorUncaught() (Flow.java:1000-1018)
    fn error_uncaught(&mut self, ctx: &mut CompilerContext) {
        let pending = std::mem::take(&mut self.pending_exits);
        let mut current = pending;
        while current.non_empty() {
            let exit = current.head().clone();
            match &exit {
                PendingExit::Thrown { tree, thrown } => {
                    let class_def_pos = self.class_def.map(|class_def| ctx.tree(class_def).pos);
                    if self.class_def.is_some() && class_def_pos == Some(ctx.tree(*tree).pos) {
                        self.flow.error(
                            ctx.tree(*tree).pos,
                            errors::unreported_exception_default_constructor(type_place(
                                ctx, *thrown,
                            )),
                        );
                    } else if ctx.tree(*tree).has_tag(Tag::VARDEF) {
                        let sym = var_sym(ctx, *tree);
                        let is_resource = sym
                            .map(|sym| {
                                matches!(
                                    &ctx.sym(sym).kind_data,
                                    crate::code::symbol::SymbolKind::VarSymbol {
                                        data_kind: Some(ElementKind::RESOURCE_VARIABLE),
                                        ..
                                    }
                                )
                            })
                            .unwrap_or(false);
                        if is_resource {
                            let name = ctx.sym(sym.expect("resource symbol")).name.clone();
                            self.flow.error(
                                ctx.tree(*tree).pos,
                                errors::unreported_exception_implicit_close(
                                    type_place(ctx, *thrown),
                                    name,
                                ),
                            );
                        } else {
                            self.flow.error(
                                ctx.tree(*tree).pos,
                                errors::unreported_exception_need_to_catch_or_throw(type_place(
                                    ctx, *thrown,
                                )),
                            );
                        }
                    } else {
                        self.flow.error(
                            ctx.tree(*tree).pos,
                            errors::unreported_exception_need_to_catch_or_throw(type_place(
                                ctx, *thrown,
                            )),
                        );
                    }
                }
                _ => {
                    assert!(
                        self.flow.log.borrow().has_error_on(Some(Rc::new(
                            SimpleDiagnosticPosition::new(ctx.tree(exit.tree()).pos),
                        ))),
                        "AssertionError: errorUncaught (Flow.java:1014)"
                    );
                }
            }
            current = current.tail();
        }
    }

    /// Java: void markThrown(JCTree tree, Type exc) (Flow.java:1032-1041)
    fn mark_thrown(&mut self, ctx: &mut CompilerContext, tree: TreeId, exc: TypeId) {
        let syms = self.flow.syms.borrow();
        let pos = tree_info::diagnostic_position_for(
            ctx,
            tree_info::symbol(ctx, tree).unwrap_or(syms.no_symbol),
            tree,
        );
        if !self
            .flow
            .chk
            .borrow()
            .is_unchecked_pos(ctx, &syms, pos, exc)
        {
            if !self
                .flow
                .chk
                .borrow()
                .is_handled(ctx, &syms, exc, &self.caught)
            {
                self.pending_exits = self
                    .pending_exits
                    .clone()
                    .append(PendingExit::Thrown { tree, thrown: exc });
            }
            self.thrown = self
                .flow
                .chk
                .borrow()
                .incl(ctx, &syms, exc, &self.thrown);
        }
    }

    /// Java: void checkCaughtType(DiagnosticPosition pos, Type exc,
    /// List<Type> thrownInTry, List<Type> caughtInTry) (Flow.java:1600-1625)
    fn check_caught_type(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        exc: TypeId,
        thrown_in_try: &List<TypeId>,
        caught_in_try: &List<TypeId>,
    ) {
        let syms = self.flow.syms.borrow();
        let chk = self.flow.chk.borrow();
        if chk.subset(ctx, &syms, exc, caught_in_try) {
            self.flow
                .error(pos, errors::except_already_caught(type_place(ctx, exc)));
        } else if !chk.is_unchecked_pos(ctx, &syms, pos, exc)
            && !is_exception_or_throwable(ctx, &syms, exc)
            && !chk.intersects(ctx, &syms, exc, thrown_in_try)
        {
            self.flow
                .error(pos, errors::except_never_thrown_in_try(type_place(ctx, exc)));
        } else {
            let catchable = chk.intersect(ctx, &syms, &List::of(exc), thrown_in_try);
            if chk.diff(ctx, &syms, &catchable, caught_in_try).is_empty()
                && !is_exception_or_throwable(ctx, &syms, exc)
            {
                if catchable.length() == 1 {
                    self.flow.warning(
                        LintCategory::TRY,
                        pos,
                        warnings::unreachable_catch(vec![type_place(ctx, exc)]),
                    );
                } else {
                    self.flow.warning(
                        LintCategory::TRY,
                        pos,
                        warnings::unreachable_catch1(vec![type_place(ctx, exc)]),
                    );
                }
            }
        }
    }

    /// Java: public void analyzeTree(Env, TreeMaker) (Flow.java:1520-1543)
    pub fn analyze_tree(
        &mut self,
        ctx: &mut CompilerContext,
        env: &Rc<Env<AttrContext>>,
        tree: TreeId,
    ) {
        let caught_prev = self.caught.clone();
        let thrown_prev = self.thrown.clone();
        self.caught = List::nil();
        self.thrown = List::nil();
        *self.flow.attr_env.borrow_mut() = Some(Rc::clone(env));
        self.scan(ctx, Some(tree));
        self.error_uncaught(ctx);
        self.caught = caught_prev;
        self.thrown = thrown_prev;
        self.pending_exits = List::nil();
    }
}

impl<'a> BaseAnalyzer for FlowAnalyzer<'a> {
    fn pending_exits_mut(&mut self) -> &mut List<PendingExit> {
        &mut self.pending_exits
    }

    /// Java: FlowAnalyzer.markDead() (Flow.java:997-999): do nothing.
    fn mark_dead(&mut self, _ctx: &mut CompilerContext) {}
}

flow_tree_visitor_delegate!(FlowAnalyzer<'a>, 'a);

impl<'a> FlowVisitor for FlowAnalyzer<'a> {
    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if self.lambda_mode {
            // Java: LambdaFlowAnalyzer.visitClassDef (Flow.java:1715-1717)
            return;
        }
        let Some(class_sym) = class_symbol(ctx, tree) else {
            return;
        };
        let anonymous_class = ctx.sym(class_sym).name.is_empty();
        let class_def_prev = self.class_def;
        let thrown_prev = self.thrown.clone();
        let caught_prev = self.caught.clone();
        let pending_prev = std::mem::take(&mut self.pending_exits);
        let lint_prev = Rc::clone(&self.lint);
        if !anonymous_class {
            self.caught = List::nil();
        }
        self.class_def = Some(tree);
        self.thrown = List::nil();
        self.lint = self.flow.augmented_lint(ctx, class_sym);
        let defs = class_defs(ctx, tree);
        // process all the static initializers
        for def in defs.iter() {
            let is_static = (tree_info::flags(ctx, def) & flags::STATIC as i64) != 0;
            if !ctx.tree(def).has_tag(Tag::METHODDEF) && is_static {
                self.scan(ctx, Some(def));
                self.error_uncaught(ctx);
            }
        }
        // add intersection of all thrown clauses of initial constructors to
        // the set of caught exceptions, unless this is an anonymous class.
        if !anonymous_class {
            let mut first_constructor = true;
            for def in defs.iter() {
                if tree_info::is_initial_constructor(ctx, def) {
                    let Some(method_sym) = method_symbol(ctx, def) else {
                        continue;
                    };
                    let mthrown = thrown_types_of(ctx, ctx.sym(method_sym).ty.expect("method type"));
                    if first_constructor {
                        self.caught = mthrown;
                        first_constructor = false;
                    } else {
                        let syms = self.flow.syms.borrow();
                        self.caught =
                            self.flow
                                .chk
                                .borrow()
                                .intersect(ctx, &syms, &mthrown, &self.caught);
                    }
                }
            }
        }
        // process all the instance initializers
        for def in defs.iter() {
            let is_static = (tree_info::flags(ctx, def) & flags::STATIC as i64) != 0;
            if !ctx.tree(def).has_tag(Tag::METHODDEF) && !is_static {
                self.scan(ctx, Some(def));
                self.error_uncaught(ctx);
            }
        }
        // process all the methods
        for def in defs.iter() {
            if ctx.tree(def).has_tag(Tag::METHODDEF) {
                self.scan(ctx, Some(def));
            }
        }
        self.pending_exits = pending_prev;
        self.class_def = class_def_prev;
        self.caught = caught_prev;
        self.thrown = thrown_prev;
        self.lint = lint_prev;
    }

    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let Some(body) = method_body(ctx, tree) else {
            return;
        };
        let Some(method_sym) = method_symbol(ctx, tree) else {
            return;
        };
        let lint_prev = Rc::clone(&self.lint);
        self.lint = self.flow.augmented_lint(ctx, method_sym);
        let caught_prev = self.caught.clone();
        let thrown_prev = self.thrown.clone();
        let method_type = ctx.sym(method_sym).ty.expect("method type");
        let mthrown = thrown_types_of(ctx, method_type);
        assert!(
            self.pending_exits.is_empty(),
            "AssertionError: pendingExits.isEmpty() (Flow.java:1140)"
        );
        for param in method_params(ctx, tree).iter() {
            self.scan(ctx, Some(param));
        }
        if tree_info::is_initial_constructor(ctx, tree) {
            let syms = self.flow.syms.borrow();
            self.caught = self
                .flow
                .chk
                .borrow()
                .union(ctx, &syms, &self.caught, &mthrown);
        } else if (ctx.sym(method_sym).flags_field
            & (flags::BLOCK as i64 | flags::STATIC as i64))
            != flags::BLOCK as i64
        {
            self.caught = mthrown.clone();
        }
        self.scan(ctx, Some(body));
        let exits = std::mem::take(&mut self.pending_exits);
        let mut current = exits;
        while current.non_empty() {
            let exit = current.head().clone();
            match &exit {
                PendingExit::Thrown { .. } => {
                    self.pending_exits = self.pending_exits.clone().append(exit);
                }
                _ => {
                    let tree_id = exit.tree();
                    assert!(
                        ctx.tree(tree_id).has_tag(Tag::RETURN)
                            || self.flow.log.borrow().has_error_on(Some(Rc::new(
                                SimpleDiagnosticPosition::new(ctx.tree(tree_id).pos),
                            ))),
                        "AssertionError: visitMethodDef exit (Flow.java:1185)"
                    );
                }
            }
            current = current.tail();
        }
        self.caught = caught_prev;
        self.thrown = thrown_prev;
        self.lint = lint_prev;
    }

    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if var_decl_init(ctx, tree).is_some() {
            let init = var_decl_init(ctx, tree);
            self.scan(ctx, init);
        }
    }

    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let stats = block_stats(ctx, tree);
        self.scan_list(ctx, &stats);
    }

    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, cond) = match &ctx.tree(tree).kind {
            TreeKind::DoWhileLoop { body, cond } => (*body, *cond),
            _ => panic!("expected a do loop"),
        };
        let prev_pending = std::mem::take(&mut self.pending_exits);
        self.scan(ctx, body);
        self.resolve_continues(ctx, tree);
        self.scan(ctx, cond);
        self.resolve_breaks(ctx, tree, prev_pending);
    }

    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, body) = match &ctx.tree(tree).kind {
            TreeKind::WhileLoop { cond, body } => (*cond, *body),
            _ => panic!("expected a while loop"),
        };
        let prev_pending = std::mem::take(&mut self.pending_exits);
        self.scan(ctx, cond);
        self.scan(ctx, body);
        self.resolve_continues(ctx, tree);
        self.resolve_breaks(ctx, tree, prev_pending);
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
        let prev_pending = std::mem::take(&mut self.pending_exits);
        self.scan_list(ctx, &init);
        self.scan(ctx, cond);
        self.scan(ctx, body);
        self.resolve_continues(ctx, tree);
        self.scan_list(ctx, &step);
        self.resolve_breaks(ctx, tree, prev_pending);
    }

    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (var, expr, body) = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop { var, expr, body } => (*var, *expr, *body),
            _ => panic!("expected an enhanced for loop"),
        };
        if let Some(var) = var {
            <Self as FlowVisitor>::visit_var_def(self, ctx, var);
        }
        let prev_pending = std::mem::take(&mut self.pending_exits);
        self.scan(ctx, expr);
        self.scan(ctx, body);
        self.resolve_continues(ctx, tree);
        self.resolve_breaks(ctx, tree, prev_pending);
    }

    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let body = match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement { body, .. } => *body,
            _ => panic!("expected a labelled statement"),
        };
        let prev_pending = std::mem::take(&mut self.pending_exits);
        self.scan(ctx, body);
        self.resolve_breaks(ctx, tree, prev_pending);
    }

    fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::Switch {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch"),
        };
        let prev_pending = std::mem::take(&mut self.pending_exits);
        self.scan(ctx, selector);
        for case in cases.iter() {
            for label in case_labels(ctx, case).iter() {
                self.scan(ctx, Some(label));
            }
            let stats = case_stats(ctx, case);
            self.scan_list(ctx, &stats);
            if case_is_rule(ctx, case) {
                scan_synthetic_break(self.flow, self, ctx, tree);
            }
        }
        self.resolve_breaks(ctx, tree, prev_pending);
    }

    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch expression"),
        };
        let prev_pending = std::mem::take(&mut self.pending_exits);
        self.scan(ctx, selector);
        for case in cases.iter() {
            for label in case_labels(ctx, case).iter() {
                self.scan(ctx, Some(label));
            }
            let stats = case_stats(ctx, case);
            self.scan_list(ctx, &stats);
        }
        self.resolve_yields(ctx, tree, prev_pending);
    }

    fn visit_break(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.record_exit(ctx, PendingExit::new(tree));
    }

    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, .. } => *value,
            _ => panic!("expected a yield"),
        };
        self.scan(ctx, value);
        self.record_exit(ctx, PendingExit::new(tree));
    }

    fn visit_continue(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.record_exit(ctx, PendingExit::new(tree));
    }

    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Return { expr } => *expr,
            _ => panic!("expected a return"),
        };
        self.scan(ctx, expr);
        self.record_exit(ctx, PendingExit::new(tree));
    }

    fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Throw { expr } => *expr,
            _ => panic!("expected a throw"),
        };
        self.scan(ctx, expr);
        if let Some(expr) = expr {
            let sym = tree_info::symbol(ctx, expr);
            let precise = sym.filter(|sym| {
                ctx.sym(*sym).kind == Kind::VAR
                    && (ctx.sym(*sym).flags_field
                        & (flags::FINAL as i64 | flags::EFFECTIVELY_FINAL))
                        != 0
                    && self.precise_rethrow_types.contains_key(sym)
            });
            match precise {
                Some(sym) => {
                    let types = self
                        .precise_rethrow_types
                        .get(&sym)
                        .expect("precise rethrow type")
                        .clone();
                    for ty in types.iter() {
                        self.mark_thrown(ctx, tree, ty);
                    }
                }
                None => {
                    let thrown_type = ctx.tree(expr).ty.expect("thrown expression type");
                    self.mark_thrown(ctx, tree, thrown_type);
                }
            }
        }
        self.mark_dead(ctx);
    }

    fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (meth, args) = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation { meth, args, .. } => (*meth, args.clone()),
            _ => panic!("expected a method invocation"),
        };
        self.scan(ctx, meth);
        self.scan_list(ctx, &args);
        let thrown = meth.and_then(|meth| ctx.tree(meth).ty);
        if let Some(meth_type) = thrown {
            for ty in thrown_types_of(ctx, meth_type).iter() {
                self.mark_thrown(ctx, tree, ty);
            }
        }
    }

    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (encl, args, def, constructor, constructor_type) = match &ctx.tree(tree).kind {
            TreeKind::NewClass {
                encl,
                args,
                def,
                constructor,
                constructor_type,
                ..
            } => (*encl, args.clone(), *def, *constructor, *constructor_type),
            _ => panic!("expected a new class"),
        };
        self.scan(ctx, encl);
        self.scan_list(ctx, &args);
        if let Some(constructor_type) = constructor_type {
            for ty in thrown_types_of(ctx, constructor_type).iter() {
                self.mark_thrown(ctx, tree, ty);
            }
        }
        // Java: Flow.java:1443-1473. If the new class expression defines an
        // anonymous class, analysis of the anonymous constructor may encounter
        // thrown types which are unsubstituted type variables; the constructor's
        // formal thrown types are therefore added to the caught set for the
        // duration of the class definition analysis.
        let caught_prev = self.caught.clone();
        if let Some(def) = def {
            if let Some(constructor) = constructor
                && let Some(constructor_signature) = ctx.sym(constructor).ty
            {
                let mut caught = self.caught.clone();
                {
                    let syms = self.flow.syms.borrow();
                    let chk = self.flow.chk.borrow();
                    for ty in thrown_types_of(ctx, constructor_signature).iter() {
                        caught = chk.incl(ctx, &syms, ty, &caught);
                    }
                }
                self.caught = caught;
            }
            self.scan(ctx, Some(def));
        }
        self.caught = caught_prev;
    }

    fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let Some(ty) = ctx.tree(tree).ty
            && ctx.typ(ty).has_tag(TypeTag::ERROR) {
                return;
            }
        if self.lambda_mode {
            // Java: LambdaFlowAnalyzer.visitLambda (Flow.java:1688-1713)
            if self.in_lambda {
                return;
            }
            let prev_caught = self.caught.clone();
            let prev_thrown = self.thrown.clone();
            let prev_pending = std::mem::take(&mut self.pending_exits);
            self.in_lambda = true;
            self.caught = List::of(self.flow.syms.borrow().throwable_type);
            self.thrown = List::nil();
            let body = match &ctx.tree(tree).kind {
                TreeKind::Lambda { body, .. } => *body,
                _ => panic!("expected a lambda"),
            };
            self.scan(ctx, body);
            self.inferred_thrown_types = self.thrown.clone();
            self.pending_exits = prev_pending;
            self.caught = prev_caught;
            self.thrown = prev_thrown;
            self.in_lambda = false;
            return;
        }
        // Java: Flow.java:1476-1509 (FlowAnalyzer.visitLambda).
        let prev_caught = self.caught.clone();
        let prev_thrown = self.thrown.clone();
        let prev_pending = std::mem::take(&mut self.pending_exits);
        let target = match &ctx.tree(tree).kind {
            TreeKind::Lambda { target, .. } => *target,
            _ => panic!("expected a lambda"),
        };
        let descriptor = {
            let mut syms = self.flow.syms.borrow_mut();
            let chk = self.flow.chk.borrow();
            self.flow
                .types
                .find_descriptor_type(
                    ctx,
                    &mut syms,
                    &self.flow.diags,
                    &chk,
                    target.expect("lambda target type"),
                )
                .expect("functional interface descriptor")
        };
        self.caught = ctx.typ(descriptor).get_thrown_types(ctx);
        self.thrown = List::nil();
        let body = match &ctx.tree(tree).kind {
            TreeKind::Lambda { body, .. } => *body,
            _ => panic!("expected a lambda"),
        };
        self.scan(ctx, body);
        let mut exits = std::mem::take(&mut self.pending_exits);
        while exits.non_empty() {
            let exit = exits.head().clone();
            exits = exits.tail();
            match &exit {
                PendingExit::Thrown { .. } => {
                    self.pending_exits = self.pending_exits.clone().append(exit);
                }
                _ => {
                    let exit_tree = match &exit {
                        PendingExit::Plain { tree } => *tree,
                        PendingExit::Assign { tree, .. } => *tree,
                        PendingExit::Thrown { tree, .. } => *tree,
                    };
                    let has_return = ctx.tree(exit_tree).has_tag(Tag::RETURN);
                    let has_error = self.flow.log.borrow().has_error_on(Some(Rc::new(
                        crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(ctx.tree(exit_tree).pos),
                    ) as Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>));
                    if !(has_return || has_error) {
                        panic!("AssertionError: unexpected lambda pending exit");
                    }
                }
            }
        }
        self.error_uncaught(ctx);
        self.pending_exits = prev_pending;
        self.caught = prev_caught;
        self.thrown = prev_thrown;
    }

    fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (dims, elems) = match &ctx.tree(tree).kind {
            TreeKind::NewArray { dims, elems, .. } => (dims.clone(), elems.clone()),
            _ => panic!("expected a new array"),
        };
        self.scan_list(ctx, &dims);
        if let Some(elems) = elems {
            self.scan_list(ctx, &elems);
        }
    }

    fn visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, detail) = match &ctx.tree(tree).kind {
            TreeKind::Assert { cond, detail } => (*cond, *detail),
            _ => panic!("expected an assert"),
        };
        self.scan(ctx, cond);
        self.scan(ctx, detail);
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

    fn visit_module_def(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

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
        let prev_caught = self.caught.clone();
        let mut prev_thrown = self.thrown.clone();
        self.thrown = List::nil();
        for catcher in catchers.iter() {
            let param = match &ctx.tree(catcher).kind {
                TreeKind::Catch { param, .. } => *param,
                _ => panic!("expected a catch clause"),
            };
            for clause in catch_sub_clauses(ctx, catcher, param).iter() {
                let clause_type = ctx.tree(clause).ty.expect("catch type");
                let syms = self.flow.syms.borrow();
                self.caught = self
                    .flow
                    .chk
                    .borrow()
                    .incl(ctx, &syms, clause_type, &self.caught);
            }
        }
        let prev_pending = std::mem::take(&mut self.pending_exits);
        for resource in resources.iter() {
            if matches!(ctx.tree(resource).kind, TreeKind::VariableDecl { .. }) {
                <Self as FlowVisitor>::visit_var_def(self, ctx, resource);
            } else {
                self.scan(ctx, Some(resource));
            }
        }
        if !resources.is_empty() {
            // Java: Flow.java:1288-1308: the close() methods of the resources
            // throw their declared exceptions at the end of the try block.
            for resource in resources.iter() {
                let Some(resource_type) = ctx.tree(resource).ty else {
                    continue;
                };
                let closeable_supertypes = if self
                    .flow
                    .types
                    .is_compound_public(ctx, resource_type)
                {
                    let mut supers = {
                        let syms = self.flow.syms.borrow();
                        self.flow.types.interfaces(ctx, &syms, resource_type)
                    };
                    supers = supers.prepend({
                        let syms = self.flow.syms.borrow();
                        self.flow.types.supertype(ctx, &syms, resource_type)
                    });
                    supers
                } else {
                    List::of(resource_type)
                };
                let mut current = closeable_supertypes;
                while current.non_empty() {
                    let sup = *current.head();
                    current = current.tail();
                    let auto_closeable_tsym = {
                        let syms = self.flow.syms.borrow();
                        ctx.typ(syms.auto_closeable_type)
                            .tsym
                            .expect("AutoCloseable symbol")
                    };
                    let as_super = {
                        let syms = self.flow.syms.borrow();
                        self.flow
                            .types
                            .as_super(ctx, &syms, sup, auto_closeable_tsym)
                    };
                    if as_super.is_none() {
                        continue;
                    }
                    let site = self.flow.types.skip_type_vars_public(ctx, sup);
                    let close_method = {
                        let attr_rc = self
                            .flow
                            .attr
                            .as_ref()
                            .expect("AssertionError: the Attr service is not installed on Flow");
                        let mut attr = attr_rc.borrow_mut();
                        let chk_rc = Rc::clone(&self.flow.chk);
                        let chk = chk_rc.borrow();
                        let infer = Rc::clone(&attr.infer);
                        let env = {
                            let env = self.flow.attr_env.borrow().clone();
                            env.expect("AssertionError: attrEnv")
                        };
                        let mut rs = self.flow.rs.borrow_mut();
                        let mut syms = self.flow.syms.borrow_mut();
                        let location = ctx.typ(site).tsym.expect("close site symbol");
                        rs.resolve_qualified_method(
                            ctx,
                            &mut syms,
                            &self.flow.types,
                            &mut attr,
                            &chk,
                            &infer,
                            Some(Rc::new(SimpleDiagnosticPosition::new(ctx.tree(resource).pos))),
                            &env,
                            location,
                            site,
                            self.flow.names.close.clone(),
                            List::nil(),
                            Some(List::nil()),
                        )
                    };
                    let member_type = {
                        let syms = self.flow.syms.borrow();
                        self.flow
                            .types
                            .member_type(ctx, &syms, resource_type, close_method)
                    };
                    if matches!(
                        ctx.sym(close_method).kind,
                        crate::code::kinds::Kind::MTH
                    ) {
                        // Java: mt.getThrownTypes()
                        let mut thrown =
                            crate::code::types::Types::get_thrown_types_public(
                                &self.flow.types,
                                ctx,
                                member_type,
                            );
                        while thrown.non_empty() {
                            let exc = *thrown.head();
                            thrown = thrown.tail();
                            self.mark_thrown(ctx, resource, exc);
                        }
                    }
                }
            }
        }
        self.scan(ctx, body);
        let syms = self.flow.syms.borrow();
        let mut thrown_in_try = self
            .flow
            .chk
            .borrow()
            .union(ctx, &syms, &self.thrown, &List::of(syms.runtime_exception_type));
        thrown_in_try = self
            .flow
            .chk
            .borrow()
            .incl(ctx, &syms, syms.error_type, &thrown_in_try);
        self.thrown = prev_thrown.clone();
        self.caught = prev_caught.clone();
        let mut caught_in_try: List<TypeId> = List::nil();
        for catcher in catchers.iter() {
            let param = match &ctx.tree(catcher).kind {
                TreeKind::Catch { param, .. } => *param,
                _ => panic!("expected a catch clause"),
            };
            let sub_clauses = catch_sub_clauses(ctx, catcher, param);
            let mut ctypes: List<TypeId> = List::nil();
            let rethrown_types = self
                .flow
                .chk
                .borrow()
                .diff(ctx, &syms, &thrown_in_try, &caught_in_try);
            for clause in sub_clauses.iter() {
                let exc = ctx.tree(clause).ty.expect("catch type");
                if exc != syms.unknown_type {
                    ctypes = ctypes.append(exc);
                    if self
                        .flow
                        .types
                        .is_same_type(ctx, &syms, exc, syms.object_type)
                    {
                        continue;
                    }
                    let pos = if sub_clauses.length() > 1 {
                        ctx.tree(clause).pos
                    } else {
                        ctx.tree(catcher).pos
                    };
                    self.check_caught_type(ctx, pos, exc, &thrown_in_try, &caught_in_try);
                    caught_in_try =
                        self.flow
                            .chk
                            .borrow()
                            .incl(ctx, &syms, exc, &caught_in_try);
                }
            }
            if let Some(param) = param {
                self.scan(ctx, Some(param));
                if let Some(param_sym) = var_sym(ctx, param) {
                    let precise =
                        self.flow
                            .chk
                            .borrow()
                            .intersect(ctx, &syms, &ctypes, &rethrown_types);
                    self.precise_rethrow_types.insert(param_sym, precise);
                }
            }
            let catch_body = match &ctx.tree(catcher).kind {
                TreeKind::Catch { body, .. } => *body,
                _ => panic!("expected a catch clause"),
            };
            self.scan(ctx, catch_body);
            if let Some(param) = param
                && let Some(param_sym) = var_sym(ctx, param) {
                    self.precise_rethrow_types.remove(&param_sym);
                }
        }
        if let Some(finalizer) = finalizer {
            let saved_thrown = self.thrown.clone();
            let exits = std::mem::replace(&mut self.pending_exits, prev_pending);
            self.thrown = List::nil();
            self.scan(ctx, Some(finalizer));
            let finally_can_complete_normally = match &ctx.tree(tree).kind {
                TreeKind::Try {
                    finally_can_complete_normally,
                    ..
                } => *finally_can_complete_normally,
                _ => false,
            };
            if !finally_can_complete_normally {
                let combined =
                    self.flow
                        .chk
                        .borrow()
                        .union(ctx, &syms, &self.thrown, &saved_thrown);
                self.thrown = combined;
            } else {
                let uncaught_in_try =
                    self.flow
                        .chk
                        .borrow()
                        .diff(ctx, &syms, &thrown_in_try, &caught_in_try);
                let with_uncaught =
                    self.flow
                        .chk
                        .borrow()
                        .union(ctx, &syms, &self.thrown, &uncaught_in_try);
                self.thrown =
                    self.flow
                        .chk
                        .borrow()
                        .union(ctx, &syms, &with_uncaught, &saved_thrown);
                let mut current = exits;
                while current.non_empty() {
                    self.pending_exits = self.pending_exits.clone().append(current.head().clone());
                    current = current.tail();
                }
            }
        } else {
            let uncaught_in_try = self
                .flow
                .chk
                .borrow()
                .diff(ctx, &syms, &thrown_in_try, &caught_in_try);
            self.thrown =
                self.flow
                    .chk
                    .borrow()
                    .union(ctx, &syms, &self.thrown, &uncaught_in_try);
            let exits = std::mem::replace(&mut self.pending_exits, prev_pending);
            let mut current = exits;
            while current.non_empty() {
                self.pending_exits = self.pending_exits.clone().append(current.head().clone());
                current = current.tail();
            }
        }
        let _ = &mut prev_thrown;
    }


}

/// Java: JCVariableDecl.sym
fn var_sym(ctx: &CompilerContext, var_decl: TreeId) -> Option<SymbolId> {
    match &ctx.tree(var_decl).kind {
        TreeKind::VariableDecl { sym, .. } => *sym,
        _ => None,
    }
}

/// Java: VarSymbol.adr
fn var_symbol_adr(ctx: &CompilerContext, sym: SymbolId) -> i32 {
    match &ctx.sym(sym).kind_data {
        crate::code::symbol::SymbolKind::VarSymbol { adr, .. } => *adr,
        _ => 0,
    }
}

/// Java: Symbols display for diagnostic argument placeholders (ADR 0002 P4)
fn symbol_display(ctx: &CompilerContext, sym: SymbolId) -> String {
    crate::comp::resolve::symbol_display_name(ctx, sym)
}

fn symbol_place(
    ctx: &CompilerContext,
    sym: SymbolId,
) -> crate::resources::diagnostic_contract::SymbolArg {
    crate::resources::diagnostic_contract::SymbolArg::placeholder(symbol_display(ctx, sym))
}

/// Java: ClassSymbol.isEnclosedBy(ClassSymbol clazz) (Symbol.java:575-587)
fn symbol_is_enclosed_by(ctx: &CompilerContext, sym: SymbolId, clazz: SymbolId) -> bool {
    let mut current = Some(sym);
    while let Some(sym) = current {
        if sym == clazz {
            return true;
        }
        let owner = ctx.sym(sym).owner;
        current = match owner {
            Some(owner) if ctx.sym(owner).kind == Kind::TYP => Some(owner),
            _ => None,
        };
    }
    false
}

/// Java: public class Flow.AssignAnalyzer extends BaseAnalyzer
/// (Flow.java:1724-2899) — definite assignment/unassignment analysis.
pub struct AssignAnalyzer<'a> {
    flow: &'a Flow,
    pending_exits: List<PendingExit>,
    pub inits: Bits,
    pub uninits: Bits,
    pub uninits_try: Bits,
    pub inits_when_true: Bits,
    pub inits_when_false: Bits,
    pub uninits_when_true: Bits,
    pub uninits_when_false: Bits,
    vardecls: Vec<Option<TreeId>>,
    class_def: Option<TreeId>,
    first_adr: i32,
    next_adr: i32,
    return_adr: i32,
    unrefd_resources: Option<crate::ids::ScopeId>,
    flow_kind: FlowKind,
    start_pos: i32,
    is_initial_constructor: bool,
    lint: Rc<Lint>,
    /// Java: LambdaAssignAnalyzer.enclosedSymbols (Flow.java:1645) — Some in
    /// the lambda thrown-types pass (Porting rules adapter: same struct, mode flag).
    enclosed_scope: Option<crate::ids::ScopeId>,
    /// Java: LambdaAssignAnalyzer.inLambda (Flow.java:1646)
    lambda_mode: bool,
    in_lambda: bool,
}

impl<'a> AssignAnalyzer<'a> {
    /// Java: public AssignAnalyzer() (Flow.java:1770-1778)
    pub fn new(flow: &'a Flow) -> AssignAnalyzer<'a> {
        AssignAnalyzer {
            flow,
            pending_exits: List::nil(),
            inits: Bits::new(),
            uninits: Bits::new(),
            uninits_try: Bits::new(),
            inits_when_true: Bits::with_reset(true),
            inits_when_false: Bits::with_reset(true),
            uninits_when_true: Bits::with_reset(true),
            uninits_when_false: Bits::with_reset(true),
            vardecls: Vec::new(),
            class_def: None,
            first_adr: 0,
            next_adr: 0,
            return_adr: 0,
            unrefd_resources: None,
            flow_kind: FlowKind::NORMAL,
            start_pos: 0,
            is_initial_constructor: false,
            lint: Rc::clone(&flow.lint),
            enclosed_scope: None,
            lambda_mode: false,
            in_lambda: false,
        }
    }

    /// Java: LambdaAssignAnalyzer(Env<AttrContext> env) (Flow.java:1648-1650)
    pub fn new_lambda(flow: &'a Flow, enclosed_scope: crate::ids::ScopeId) -> AssignAnalyzer<'a> {
        let mut analyzer = AssignAnalyzer::new(flow);
        analyzer.enclosed_scope = Some(enclosed_scope);
        analyzer.lambda_mode = true;
        analyzer
    }

    /// Java: void referenced(Symbol sym) (Flow.java:2851-2853) — the
    /// unrefdResources scope removal is registered with the Scope.
    fn referenced(&mut self, _ctx: &CompilerContext, _sym: SymbolId) {}

    /// Java: protected boolean trackable(VarSymbol sym) (Flow.java:1793-1799)
    fn trackable(&self, ctx: &CompilerContext, sym: SymbolId) -> bool {
        if let Some(scope) = self.enclosed_scope {
            // Java: LambdaAssignAnalyzer.trackable (Flow.java:1673-1676)
            return ctx.scope_includes(scope, sym)
                && matches!(ctx.sym(sym).owner.map(|owner| ctx.sym(owner).kind), Some(Kind::MTH));
        }
        let sym_pos = match &ctx.sym(sym).kind_data {
            crate::code::symbol::SymbolKind::VarSymbol { pos, .. } => *pos,
            _ => 0,
        };
        sym_pos >= self.start_pos
            && (ctx
                .sym(sym)
                .owner
                .map(|owner| matches!(ctx.sym(owner).kind, Kind::MTH | Kind::VAR))
                .unwrap_or(false)
                || self.is_final_uninitialized_field(ctx, sym))
    }

    /// Java: boolean isFinalUninitializedField(VarSymbol sym)
    /// (Flow.java:1801-1805)
    fn is_final_uninitialized_field(&self, ctx: &CompilerContext, sym: SymbolId) -> bool {
        let owner = ctx.sym(sym).owner;
        let flags_field = ctx.sym(sym).flags_field;
        let class_def_sym = self
            .class_def
            .and_then(|class_def| class_symbol(ctx, class_def));
        matches!(owner.map(|owner| ctx.sym(owner).kind), Some(Kind::TYP))
            && (flags_field
                & (flags::FINAL as i64 | flags::HASINIT as i64 | flags::PARAMETER))
                == flags::FINAL as i64
            && match (class_def_sym, owner) {
                (Some(class_sym), Some(owner)) => symbol_is_enclosed_by(ctx, owner, class_sym),
                _ => false,
            }
    }

    /// Java: boolean isFinalUninitializedStaticField(VarSymbol sym)
    /// (Flow.java:1807-1809)
    fn is_final_uninitialized_static_field(&self, ctx: &CompilerContext, sym: SymbolId) -> bool {
        self.is_final_uninitialized_field(ctx, sym)
            && (ctx.sym(sym).flags_field & flags::STATIC as i64) != 0
    }

    /// Java: void newVar(JCVariableDecl varDecl) (Flow.java:1815-1826)
    fn new_var(&mut self, ctx: &mut CompilerContext, var_decl: TreeId) {
        let Some(sym) = var_sym(ctx, var_decl) else {
            return;
        };
        if (ctx.sym(sym).flags_field & flags::FINAL as i64) == 0 {
            ctx.sym_mut(sym).flags_field |= flags::EFFECTIVELY_FINAL;
        }
        let adr = self.next_adr;
        if self.vardecls.len() <= adr as usize {
            self.vardecls.resize(adr as usize + 1, None);
        }
        if let crate::code::symbol::SymbolKind::VarSymbol { adr: slot, .. } =
            &mut ctx.sym_mut(sym).kind_data
        {
            *slot = adr;
        }
        self.vardecls[adr as usize] = Some(var_decl);
        self.inits.excl(adr);
        self.uninits.incl(adr);
        self.next_adr += 1;
    }

    /// Java: void letInit(DiagnosticPosition pos, VarSymbol sym)
    /// (Flow.java:1831-1866)
    fn let_init(&mut self, ctx: &mut CompilerContext, pos: i32, sym: SymbolId) {
        let adr = var_symbol_adr(ctx, sym);
        if adr >= self.first_adr && self.trackable(ctx, sym) {
            let flags_field = ctx.sym(sym).flags_field;
            if (flags_field & flags::EFFECTIVELY_FINAL) != 0 {
                if !self.uninits.is_member(adr) {
                    ctx.sym_mut(sym).flags_field &= !flags::EFFECTIVELY_FINAL;
                } else {
                    self.uninit(ctx, sym);
                }
            } else if (flags_field & flags::FINAL as i64) != 0 {
                if (flags_field & flags::PARAMETER) != 0 {
                    if (flags_field & flags::UNION) != 0 {
                        self.flow.error(
                            pos,
                            errors::multicatch_parameter_may_not_be_assigned(symbol_place(
                                ctx, sym,
                            )),
                        );
                    } else {
                        self.flow.error(
                            pos,
                            errors::final_parameter_may_not_be_assigned(symbol_place(ctx, sym)),
                        );
                    }
                } else if !self.uninits.is_member(adr) {
                    self.flow.error_key(
                        pos,
                        self.flow_kind.err_key(),
                        vec![Rc::new(javac_strings::JavaString::from(
                            symbol_display(ctx, sym).as_str(),
                        )) as DiagnosticArg],
                    );
                } else {
                    self.uninit(ctx, sym);
                }
            }
            self.inits.incl(adr);
        } else if (ctx.sym(sym).flags_field & flags::FINAL as i64) != 0 {
            self.flow.error(
                pos,
                errors::var_might_already_be_assigned(symbol_place(ctx, sym)),
            );
        }
    }

    /// Java: void uninit(VarSymbol sym) (Flow.java:1868-1878)
    fn uninit(&mut self, ctx: &CompilerContext, sym: SymbolId) {
        let adr = var_symbol_adr(ctx, sym);
        self.uninits.excl(adr);
        self.uninits_try.excl(adr);
    }

    /// Java: letInit(JCTree) (Flow.java:1885-1893)
    fn let_init_tree(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree = crate::tree::tree_info::skip_parens(ctx, tree);
        if (ctx.tree(tree).has_tag(Tag::IDENT) || ctx.tree(tree).has_tag(Tag::SELECT))
            && let Some(sym) = tree_info::symbol(ctx, tree)
                && ctx.sym(sym).kind == Kind::VAR {
                    self.let_init(ctx, ctx.tree(tree).pos, sym);
                }
    }

    /// Java: void checkInit(DiagnosticPosition pos, VarSymbol sym)
    /// (Flow.java:1898-1900)
    fn check_init(&mut self, ctx: &mut CompilerContext, pos: i32, sym: SymbolId) {
        let info = errors::var_might_not_have_been_initialized(symbol_place(ctx, sym));
        self.check_init_with(ctx, pos, sym, info);
    }

    /// Java: void checkInit(pos, sym, Error) (Flow.java:1902-1911)
    fn check_init_with(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        sym: SymbolId,
        info: ErrorInfo,
    ) {
        let adr = var_symbol_adr(ctx, sym);
        let owner_kind = ctx.sym(sym).owner.map(|owner| ctx.sym(owner).kind);
        if (adr >= self.first_adr || owner_kind != Some(Kind::TYP))
            && self.trackable(ctx, sym)
            && !self.inits.is_member(adr)
            && (ctx.sym(sym).flags_field & flags::CLASH) == 0
        {
            self.flow.error(pos, info);
            self.inits.incl(adr);
        }
    }

    /// Java: private void resetBits(Bits... bits) (Flow.java:1916-1920)
    fn reset_bits(&mut self) {
        self.inits.reset();
        self.uninits.reset();
        self.uninits_try.reset();
        self.inits_when_true.reset();
        self.inits_when_false.reset();
        self.uninits_when_true.reset();
        self.uninits_when_false.reset();
    }

    /// Java: void split(boolean setToNull) (Flow.java:1925-1932)
    fn split(&mut self, set_to_null: bool) {
        self.inits_when_false.assign(&mut clone_bits(&self.inits));
        self.uninits_when_false.assign(&mut clone_bits(&self.uninits));
        self.inits_when_true.assign(&mut clone_bits(&self.inits));
        self.uninits_when_true.assign(&mut clone_bits(&self.uninits));
        if set_to_null {
            self.inits.reset();
            self.uninits.reset();
        }
    }

    /// Java: protected void merge() (Flow.java:1936-1939)
    fn merge(&mut self) {
        self.inits
            .assign(&mut clone_bits(self.inits_when_false.and_set(&self.inits_when_true)));
        self.uninits
            .assign(&mut clone_bits(self.uninits_when_false.and_set(&self.uninits_when_true)));
    }

    /// Java: void scanExpr(JCTree tree) (Flow.java:1948-1953)
    fn scan_expr(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) {
        if let Some(tree) = tree {
            self.scan(ctx, Some(tree));
            if self.inits.is_reset() {
                self.merge();
            }
        }
    }

    /// Java: void scanExprs(List<? extends JCExpression> trees)
    /// (Flow.java:1958-1962)
    fn scan_exprs(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) {
        for tree in trees.iter() {
            self.scan_expr(ctx, Some(tree));
        }
    }

    /// Java: void scanCond(JCTree tree) (Flow.java:1967-1993)
    fn scan_cond(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree_type = ctx.tree(tree).ty;
        let is_false = tree_type.map(|ty| ctx.typ(ty).is_false()).unwrap_or(false);
        let is_true = tree_type.map(|ty| ctx.typ(ty).is_true()).unwrap_or(false);
        let unknown = tree_type
            .map(|ty| ctx.typ(ty).has_tag(TypeTag::UNKNOWN))
            .unwrap_or(false);
        if is_false {
            if self.inits.is_reset() {
                self.merge();
            }
            self.inits_when_true.assign(&mut clone_bits(&self.inits));
            self.inits_when_true.incl_range(self.first_adr, self.next_adr);
            self.uninits_when_true.assign(&mut clone_bits(&self.uninits));
            self.uninits_when_true.incl_range(self.first_adr, self.next_adr);
            self.inits_when_false.assign(&mut clone_bits(&self.inits));
            self.uninits_when_false.assign(&mut clone_bits(&self.uninits));
        } else if is_true {
            if self.inits.is_reset() {
                self.merge();
            }
            self.inits_when_false.assign(&mut clone_bits(&self.inits));
            self.inits_when_false.incl_range(self.first_adr, self.next_adr);
            self.uninits_when_false.assign(&mut clone_bits(&self.uninits));
            self.uninits_when_false.incl_range(self.first_adr, self.next_adr);
            self.inits_when_true.assign(&mut clone_bits(&self.inits));
            self.uninits_when_true.assign(&mut clone_bits(&self.uninits));
        } else {
            self.scan(ctx, Some(tree));
            if !self.inits.is_reset() {
                self.split(!unknown);
            }
        }
        if !unknown {
            self.inits.reset();
            self.uninits.reset();
        }
    }

    /// Java: private void handleSwitch(JCTree tree, JCExpression selector,
    /// List<JCCase> cases, boolean hasTotalPattern) (Flow.java:2444-2530)
    fn handle_switch(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        selector: Option<TreeId>,
        cases: &List<TreeId>,
        has_total_pattern: bool,
    ) {
        let prev_pending_exits = std::mem::take(&mut self.pending_exits);
        let next_adr_prev = self.next_adr;
        self.scan_expr(ctx, selector);
        let inits_switch = clone_bits(&self.inits);
        let uninits_switch = clone_bits(&self.uninits);
        let mut remaining = cases.clone();
        while remaining.non_empty() {
            self.inits.assign(&mut clone_bits(&inits_switch));
            self.uninits.assign(&mut clone_bits(&uninits_switch));
            let case = *remaining.head();
            for label in case_labels(ctx, case).iter() {
                self.scan(ctx, Some(label));
                if self.inits.is_reset() {
                    let inits_when_true = clone_bits(&self.inits_when_true);
                    self.inits.assign(&mut clone_bits(&inits_when_true));
                    let uninits_when_true = clone_bits(&self.uninits_when_true);
                    self.uninits.assign(&mut clone_bits(&uninits_when_true));
                }
            }
            let stats = case_stats(ctx, case);
            let next = remaining.tail();
            // handling: case Integer i: case null: joins the two cases
            let join_null_case = stats.is_empty()
                && next.non_empty()
                && {
                    let next_case = *next.head();
                    let next_labels = case_labels(ctx, next_case);
                    next_labels.length() == 1 && {
                        let label = *next_labels.head();
                        !is_pattern_tree(ctx, label) && tree_info::is_null(ctx, label)
                    }
                };
            let (case, next) = if join_null_case {
                let case = *next.head();
                (case, next.clone().tail())
            } else {
                (case, next.clone())
            };
            let stats = case_stats(ctx, case);
            self.scan_list(ctx, &stats);
            let completes_normally = match &ctx.tree(case).kind {
                TreeKind::Case {
                    completes_normally, ..
                } => *completes_normally,
                _ => false,
            };
            if completes_normally && case_is_rule(ctx, case) {
                scan_synthetic_break(self.flow, self, ctx, tree);
            }
            let mut inits_copy = clone_bits(&self.inits);
            let mut uninits_copy = clone_bits(&self.uninits);
            self.add_vars(ctx, &stats, &mut inits_copy, &mut uninits_copy);
            remaining = next;
        }
        if !has_total_pattern {
            if ctx.tree(tree).has_tag(Tag::SWITCH_EXPRESSION) {
                self.mark_dead(ctx);
            } else {
                self.inits.assign(&mut clone_bits(&inits_switch));
                let mut copy = clone_bits(&uninits_switch);
                copy.and_set(&self.uninits);
                self.uninits.assign(&mut clone_bits(&copy));
            }
        }
        if ctx.tree(tree).has_tag(Tag::SWITCH_EXPRESSION) {
            self.resolve_yields(ctx, tree, prev_pending_exits);
        } else {
            self.resolve_breaks(ctx, tree, prev_pending_exits);
        }
        self.next_adr = next_adr_prev;
    }

    /// Java: private void addVars(List<JCStatement> stats, Bits inits,
    /// Bits uninits) (Flow.java:2528-2540)
    fn add_vars(
        &self,
        ctx: &CompilerContext,
        stats: &List<TreeId>,
        inits: &mut Bits,
        uninits: &mut Bits,
    ) {
        for stat in stats.iter() {
            if ctx.tree(stat).has_tag(Tag::VARDEF)
                && let Some(sym) = var_sym(ctx, stat) {
                    let adr = var_symbol_adr(ctx, sym);
                    inits.excl(adr);
                    uninits.incl(adr);
                }
        }
    }

    /// Java: public void analyzeTree(Env, TreeMaker) (Flow.java:2880-2899)
    pub fn analyze_tree(
        &mut self,
        ctx: &mut CompilerContext,
        env: &Rc<Env<AttrContext>>,
        tree: TreeId,
    ) {
        let prev_pending = std::mem::take(&mut self.pending_exits);
        let prev_first_adr = self.first_adr;
        let prev_next_adr = self.next_adr;
        let prev_return_adr = self.return_adr;
        let prev_flow_kind = self.flow_kind;
        let prev_lint = Rc::clone(&self.lint);
        self.first_adr = 0;
        self.next_adr = 0;
        self.return_adr = 0;
        self.flow_kind = FlowKind::NORMAL;
        self.class_def = None;
        self.is_initial_constructor = false;
        self.start_pos = ctx.tree(tree).pos;
        // Java: unrefdResources = WriteableScope.create(env.enclClass.sym)
        // (Flow.java:2870); the resource-reference scope is not wired in the
        // port (registered gap), as before this batch.
        self.unrefd_resources = None;
        *self.flow.attr_env.borrow_mut() = Some(Rc::clone(env));
        self.scan(ctx, Some(tree));
        // Java: finally { resetBits(inits, uninits, uninitsTry, ...);
        // startPos = -1; firstadr = 0; nextadr = 0; ... } (Flow.java:2872-2887).
        self.reset_bits();
        self.pending_exits = prev_pending;
        self.first_adr = prev_first_adr;
        self.next_adr = prev_next_adr;
        self.return_adr = prev_return_adr;
        self.flow_kind = prev_flow_kind;
        self.lint = prev_lint;
    }
}

impl<'a> BaseAnalyzer for AssignAnalyzer<'a> {
    fn pending_exits_mut(&mut self) -> &mut List<PendingExit> {
        &mut self.pending_exits
    }

    /// Java: protected void markDead() (Flow.java:1781-1792)
    fn mark_dead(&mut self, ctx: &mut CompilerContext) {
        if !self.is_initial_constructor {
            self.inits.incl_range(self.return_adr, self.next_adr);
        } else {
            let mut address = self.return_adr;
            while address < self.next_adr {
                let is_static_final_uninit = self
                    .vardecls
                    .get(address as usize)
                    .copied()
                    .flatten()
                    .and_then(|var_decl| var_sym(ctx, var_decl))
                    .map(|sym| self.is_final_uninitialized_static_field(ctx, sym))
                    .unwrap_or(false);
                if !is_static_final_uninit {
                    self.inits.incl(address);
                }
                address += 1;
            }
        }
        self.uninits.incl_range(self.return_adr, self.next_adr);
    }

    /// Java: AssignPendingExit.resolveJump() (Flow.java:1762-1767)
    fn resolve_pending_exit(&mut self, exit: &PendingExit) {
        if let PendingExit::Assign {
            exit_inits,
            exit_uninits,
            ..
        } = exit
        {
            self.inits.and_set(&exit_inits.borrow());
            self.uninits.and_set(&exit_uninits.borrow());
        }
    }
}

flow_tree_visitor_delegate!(AssignAnalyzer<'a>, 'a);

impl<'a> FlowVisitor for AssignAnalyzer<'a> {
    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if self.lambda_mode {
            // Java: LambdaAssignAnalyzer.visitClassDef (Flow.java:1678-1680)
            return;
        }
        let Some(class_sym) = class_symbol(ctx, tree) else {
            return;
        };
        let lint_prev = Rc::clone(&self.lint);
        self.lint = self.flow.augmented_lint(ctx, class_sym);
        let class_def_prev = self.class_def;
        let first_adr_prev = self.first_adr;
        let next_adr_prev = self.next_adr;
        let pending_prev = std::mem::take(&mut self.pending_exits);
        if !ctx.sym(class_sym).name.is_empty() {
            self.first_adr = self.next_adr;
        }
        self.class_def = Some(tree);
        let defs = class_defs(ctx, tree);
        // define all the static fields
        for def in defs.iter() {
            if matches!(ctx.tree(def).kind, TreeKind::VariableDecl { .. }) {
                let is_static = (tree_info::flags(ctx, def) & flags::STATIC as i64) != 0;
                if is_static
                    && let Some(sym) = var_sym(ctx, def)
                        && self.trackable(ctx, sym) {
                            self.new_var(ctx, def);
                        }
            }
        }
        // process all the static initializers
        for def in defs.iter() {
            let is_static = (tree_info::flags(ctx, def) & flags::STATIC as i64) != 0;
            if !ctx.tree(def).has_tag(Tag::METHODDEF) && is_static {
                self.scan(ctx, Some(def));
                self.clear_pending_exits(ctx, false);
            }
        }
        // define all the instance fields
        for def in defs.iter() {
            if matches!(ctx.tree(def).kind, TreeKind::VariableDecl { .. }) {
                let is_static = (tree_info::flags(ctx, def) & flags::STATIC as i64) != 0;
                if !is_static
                    && let Some(sym) = var_sym(ctx, def)
                        && self.trackable(ctx, sym) {
                            self.new_var(ctx, def);
                        }
            }
        }
        // process all the instance initializers
        for def in defs.iter() {
            let is_static = (tree_info::flags(ctx, def) & flags::STATIC as i64) != 0;
            if !ctx.tree(def).has_tag(Tag::METHODDEF) && !is_static {
                self.scan(ctx, Some(def));
                self.clear_pending_exits(ctx, false);
            }
        }
        // process all the methods
        for def in defs.iter() {
            if ctx.tree(def).has_tag(Tag::METHODDEF) {
                self.scan(ctx, Some(def));
            }
        }
        self.pending_exits = pending_prev;
        self.next_adr = next_adr_prev;
        self.first_adr = first_adr_prev;
        self.class_def = class_def_prev;
        self.lint = lint_prev;
    }

    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let Some(body) = method_body(ctx, tree) else {
            return;
        };
        let Some(method_sym) = method_symbol(ctx, tree) else {
            return;
        };
        if (ctx.sym(method_sym).flags_field & flags::SYNTHETIC as i64) != 0 {
            return;
        }
        let lint_prev = Rc::clone(&self.lint);
        self.lint = self.flow.augmented_lint(ctx, method_sym);
        let inits_prev = clone_bits(&self.inits);
        let uninits_prev = clone_bits(&self.uninits);
        let next_adr_prev = self.next_adr;
        let first_adr_prev = self.first_adr;
        let return_adr_prev = self.return_adr;
        assert!(
            self.pending_exits.is_empty(),
            "AssertionError: pendingExits.isEmpty() (Flow.java:2177)"
        );
        let last_initial_constructor = self.is_initial_constructor;
        self.is_initial_constructor = tree_info::is_initial_constructor(ctx, tree);
        if !self.is_initial_constructor {
            self.first_adr = self.next_adr;
        }
        for param in method_params(ctx, tree).iter() {
            self.scan(ctx, Some(param));
            let param_sym = var_sym(ctx, param).expect("parameter symbol");
            assert!(
                (ctx.sym(param_sym).flags_field & flags::PARAMETER) != 0,
                "AssertionError: Method parameter without PARAMETER flag (Flow.java:2200)"
            );
            self.init_param(ctx, param);
        }
        self.scan(ctx, Some(body));
        let is_compact_constructor =
            (ctx.sym(method_sym).flags_field & flags::COMPACT_RECORD_CONSTRUCTOR) != 0;
        if self.is_initial_constructor {
            let is_synthesized = (ctx.sym(method_sym).flags_field & flags::GENERATEDCONSTR) != 0;
            let class_def_sym = self
                .class_def
                .and_then(|class_def| class_symbol(ctx, class_def));
            let completes_normally = match &ctx.tree(tree).kind {
                TreeKind::MethodDecl {
                    completes_normally, ..
                } => *completes_normally,
                _ => false,
            };
            let var_decls: Vec<TreeId> = self
                .vardecls
                .iter()
                .filter_map(|var_decl| *var_decl)
                .collect();
            for var_decl in var_decls {
                let Some(var) = var_sym(ctx, var_decl) else {
                    continue;
                };
                if ctx.sym(var).owner != class_def_sym {
                    continue;
                }
                if is_synthesized && !is_compact_constructor {
                    let info = errors::var_not_initialized_in_default_constructor(symbol_place(
                        ctx, var,
                    ));
                    self.check_init_with(
                        ctx,
                        tree_info::diagnostic_position_for(ctx, var, var_decl),
                        var,
                        info,
                    );
                } else if is_compact_constructor {
                    let is_instance_record_field = class_def_sym
                        .map(|class_def_sym| {
                            (ctx.sym(class_def_sym).flags_field & flags::RECORD) != 0
                        })
                        .unwrap_or(false)
                        && (ctx.sym(var).flags_field
                            & (flags::PRIVATE as i64
                                | flags::FINAL as i64
                                | flags::GENERATED_MEMBER as i64
                                | flags::RECORD))
                            != 0
                        && (ctx.sym(var).flags_field & flags::STATIC as i64) == 0
                        && matches!(ctx.sym(var).owner.map(|owner| ctx.sym(owner).kind), Some(Kind::TYP));
                    if is_instance_record_field {
                        let adr = var_symbol_adr(ctx, var);
                        let not_initialized = !self.inits.is_member(adr);
                        if not_initialized && self.uninits.is_member(adr) && completes_normally {
                            ctx.sym_mut(var).flags_field |= flags::UNINITIALIZED_FIELD;
                        } else {
                            self.check_init(ctx, tree_info::diag_end_pos(ctx, body), var);
                        }
                    } else {
                        self.check_init(
                            ctx,
                            tree_info::diagnostic_position_for(ctx, var, var_decl),
                            var,
                        );
                    }
                } else {
                    self.check_init(ctx, tree_info::diag_end_pos(ctx, body), var);
                }
            }
        }
        self.clear_pending_exits(ctx, true);
        self.inits.assign(&mut clone_bits(&inits_prev));
        self.uninits.assign(&mut clone_bits(&uninits_prev));
        self.next_adr = next_adr_prev;
        self.first_adr = first_adr_prev;
        self.return_adr = return_adr_prev;
        self.is_initial_constructor = last_initial_constructor;
        self.lint = lint_prev;
    }

    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let Some(sym) = var_sym(ctx, tree) else {
            return;
        };
        if let Some(scope) = self.enclosed_scope {
            // Java: LambdaAssignAnalyzer.visitVarDef (Flow.java:1665-1669)
            ctx.scope_enter(scope, sym);
        }
        let lint_prev = Rc::clone(&self.lint);
        self.lint = self.flow.augmented_lint(ctx, sym);
        let track = self.trackable(ctx, sym);
        let owner_kind = ctx.sym(sym).owner.map(|owner| ctx.sym(owner).kind);
        if track && matches!(owner_kind, Some(Kind::MTH) | Some(Kind::VAR)) {
            self.new_var(ctx, tree);
        }
        if let Some(init) = var_decl_init(ctx, tree) {
            self.scan_expr(ctx, Some(init));
            if track {
                self.let_init(ctx, ctx.tree(tree).pos, sym);
            }
        }
        self.lint = lint_prev;
    }

    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let next_adr_prev = self.next_adr;
        let stats = block_stats(ctx, tree);
        self.scan_list(ctx, &stats);
        self.next_adr = next_adr_prev;
    }

    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, cond) = match &ctx.tree(tree).kind {
            TreeKind::DoWhileLoop { body, cond } => (*body, *cond),
            _ => panic!("expected a do loop"),
        };
        let prev_pending = std::mem::take(&mut self.pending_exits);
        let prev_flow_kind = self.flow_kind;
        self.flow_kind = FlowKind::NORMAL;
        let mut inits_skip = Bits::with_reset(true);
        let mut uninits_skip = Bits::with_reset(true);
        let prev_errors = self.flow.log.borrow().nerrors;
        loop {
            let mut entry = clone_bits(&self.uninits);
            entry.exclude_from(self.next_adr);
            self.scan(ctx, body);
            self.resolve_continues(ctx, tree);
            self.scan_cond(ctx, cond.expect("do loop cond"));
            if !self.flow_kind.is_final() {
                inits_skip.assign(&mut clone_bits(&self.inits_when_false));
                uninits_skip.assign(&mut clone_bits(&self.uninits_when_false));
            }
            let mut probe = clone_bits(&entry);
            probe.diff_set(&self.uninits_when_true);
            if self.flow.log.borrow().nerrors != prev_errors
                || self.flow_kind.is_final()
                || probe.next_bit(self.first_adr) == -1
            {
                break;
            }
            let when_true = clone_bits(&self.inits_when_true);
            self.inits.assign(&mut clone_bits(&when_true));
            let mut entry_clone = clone_bits(&entry);
            entry_clone.and_set(&self.uninits_when_true);
            self.uninits.assign(&mut clone_bits(&entry_clone));
            self.flow_kind = FlowKind::SPECULATIVE_LOOP;
        }
        self.flow_kind = prev_flow_kind;
        self.inits.assign(&mut inits_skip);
        self.uninits.assign(&mut uninits_skip);
        self.resolve_breaks(ctx, tree, prev_pending);
    }

    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, body) = match &ctx.tree(tree).kind {
            TreeKind::WhileLoop { cond, body } => (*cond, *body),
            _ => panic!("expected a while loop"),
        };
        let prev_pending = std::mem::take(&mut self.pending_exits);
        let prev_flow_kind = self.flow_kind;
        self.flow_kind = FlowKind::NORMAL;
        let mut inits_skip = Bits::with_reset(true);
        let mut uninits_skip = Bits::with_reset(true);
        let prev_errors = self.flow.log.borrow().nerrors;
        let mut uninits_entry = clone_bits(&self.uninits);
        uninits_entry.exclude_from(self.next_adr);
        loop {
            self.scan_cond(ctx, cond.expect("while cond"));
            if !self.flow_kind.is_final() {
                inits_skip.assign(&mut clone_bits(&self.inits_when_false));
                uninits_skip.assign(&mut clone_bits(&self.uninits_when_false));
            }
            let inits_when_true = clone_bits(&self.inits_when_true);
            self.inits.assign(&mut clone_bits(&inits_when_true));
            let uninits_when_true = clone_bits(&self.uninits_when_true);
            self.uninits.assign(&mut clone_bits(&uninits_when_true));
            self.scan(ctx, body);
            self.resolve_continues(ctx, tree);
            let mut probe = clone_bits(&uninits_entry);
            probe.diff_set(&self.uninits);
            if self.flow.log.borrow().nerrors != prev_errors
                || self.flow_kind.is_final()
                || probe.next_bit(self.first_adr) == -1
            {
                break;
            }
            let mut entry_clone = clone_bits(&uninits_entry);
            entry_clone.and_set(&self.uninits);
            self.uninits.assign(&mut clone_bits(&entry_clone));
            self.flow_kind = FlowKind::SPECULATIVE_LOOP;
        }
        self.flow_kind = prev_flow_kind;
        self.inits.assign(&mut inits_skip);
        self.uninits.assign(&mut uninits_skip);
        self.resolve_breaks(ctx, tree, prev_pending);
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
        let prev_pending = std::mem::take(&mut self.pending_exits);
        let prev_flow_kind = self.flow_kind;
        self.flow_kind = FlowKind::NORMAL;
        let next_adr_prev = self.next_adr;
        self.scan_list(ctx, &init);
        let mut inits_skip = Bits::with_reset(true);
        let mut uninits_skip = Bits::with_reset(true);
        let prev_errors = self.flow.log.borrow().nerrors;
        loop {
            let mut uninits_entry = clone_bits(&self.uninits);
            uninits_entry.exclude_from(self.next_adr);
            if let Some(cond) = cond {
                self.scan_cond(ctx, cond);
                if !self.flow_kind.is_final() {
                    inits_skip.assign(&mut clone_bits(&self.inits_when_false));
                    uninits_skip.assign(&mut clone_bits(&self.uninits_when_false));
                }
                let inits_when_true = clone_bits(&self.inits_when_true);
                self.inits.assign(&mut clone_bits(&inits_when_true));
                let uninits_when_true = clone_bits(&self.uninits_when_true);
                self.uninits.assign(&mut clone_bits(&uninits_when_true));
            } else if !self.flow_kind.is_final() {
                inits_skip.assign(&mut clone_bits(&self.inits));
                inits_skip.incl_range(self.first_adr, self.next_adr);
                uninits_skip.assign(&mut clone_bits(&self.uninits));
                uninits_skip.incl_range(self.first_adr, self.next_adr);
            }
            self.scan(ctx, body);
            self.resolve_continues(ctx, tree);
            self.scan_list(ctx, &step);
            let mut probe = clone_bits(&uninits_entry);
            probe.diff_set(&self.uninits);
            if self.flow.log.borrow().nerrors != prev_errors
                || self.flow_kind.is_final()
                || probe.next_bit(self.first_adr) == -1
            {
                break;
            }
            let mut entry_clone = clone_bits(&uninits_entry);
            entry_clone.and_set(&self.uninits);
            self.uninits.assign(&mut clone_bits(&entry_clone));
            self.flow_kind = FlowKind::SPECULATIVE_LOOP;
        }
        self.flow_kind = prev_flow_kind;
        self.inits.assign(&mut inits_skip);
        self.uninits.assign(&mut uninits_skip);
        self.resolve_breaks(ctx, tree, prev_pending);
        self.next_adr = next_adr_prev;
    }

    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (var, expr, body) = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop { var, expr, body } => (*var, *expr, *body),
            _ => panic!("expected an enhanced for loop"),
        };
        let Some(var) = var else {
            return;
        };
        <Self as FlowVisitor>::visit_var_def(self, ctx, var);
        let prev_pending = std::mem::take(&mut self.pending_exits);
        let prev_flow_kind = self.flow_kind;
        self.flow_kind = FlowKind::NORMAL;
        let next_adr_prev = self.next_adr;
        self.scan(ctx, expr);
        let inits_start = clone_bits(&self.inits);
        let uninits_start = clone_bits(&self.uninits);
        if let Some(sym) = var_sym(ctx, var) {
            self.let_init(ctx, ctx.tree(tree).pos, sym);
        }
        let prev_errors = self.flow.log.borrow().nerrors;
        loop {
            let mut uninits_entry = clone_bits(&self.uninits);
            uninits_entry.exclude_from(self.next_adr);
            self.scan(ctx, body);
            self.resolve_continues(ctx, tree);
            let mut probe = clone_bits(&uninits_entry);
            probe.diff_set(&self.uninits);
            if self.flow.log.borrow().nerrors != prev_errors
                || self.flow_kind.is_final()
                || probe.next_bit(self.first_adr) == -1
            {
                break;
            }
            let mut entry_clone = clone_bits(&uninits_entry);
            entry_clone.and_set(&self.uninits);
            self.uninits.assign(&mut clone_bits(&entry_clone));
            self.flow_kind = FlowKind::SPECULATIVE_LOOP;
        }
        self.flow_kind = prev_flow_kind;
        self.inits.assign(&mut clone_bits(&inits_start));
        let mut uninits_end = clone_bits(&uninits_start);
        uninits_end.and_set(&self.uninits);
        self.uninits.assign(&mut clone_bits(&uninits_end));
        self.resolve_breaks(ctx, tree, prev_pending);
        self.next_adr = next_adr_prev;
    }

    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let body = match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement { body, .. } => *body,
            _ => panic!("expected a labelled statement"),
        };
        let prev_pending = std::mem::take(&mut self.pending_exits);
        self.scan(ctx, body);
        self.resolve_breaks(ctx, tree, prev_pending);
    }

    fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases, has_total_pattern) = match &ctx.tree(tree).kind {
            TreeKind::Switch {
                selector,
                cases,
                has_total_pattern,
                ..
            } => (*selector, cases.clone(), *has_total_pattern),
            _ => panic!("expected a switch"),
        };
        self.handle_switch(ctx, tree, selector, &cases, has_total_pattern);
    }

    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases, has_total_pattern) = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression {
                selector,
                cases,
                has_total_pattern,
                ..
            } => (*selector, cases.clone(), *has_total_pattern),
            _ => panic!("expected a switch expression"),
        };
        self.handle_switch(ctx, tree, selector, &cases, has_total_pattern);
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
        let mut resource_var_decls: List<TreeId> = List::nil();
        let uninits_try_prev = clone_bits(&self.uninits_try);
        let pending_prev = std::mem::take(&mut self.pending_exits);
        let inits_try = clone_bits(&self.inits);
        self.uninits_try.assign(&mut clone_bits(&self.uninits));
        for resource in resources.iter() {
            if matches!(ctx.tree(resource).kind, TreeKind::VariableDecl { .. }) {
                <Self as FlowVisitor>::visit_var_def(self, ctx, resource);
                resource_var_decls = resource_var_decls.append(resource);
            } else {
                self.scan_expr(ctx, Some(resource));
            }
        }
        self.scan(ctx, body);
        self.uninits_try.assign(&mut clone_bits(&self.uninits));
        let inits_end = clone_bits(&self.inits);
        let mut uninits_end = clone_bits(&self.uninits);
        let next_adr_catch = self.next_adr;
        if !resource_var_decls.is_empty() && self.lint.is_enabled(LintCategory::TRY) {
            for res_var in resource_var_decls.iter() {
                let Some(sym) = var_sym(ctx, res_var) else {
                    continue;
                };
                let unreferenced = match self.unrefd_resources {
                    Some(scope) => {
                        let symbols = ctx.symbols_arena();
                        !ctx.scope(scope).includes(scope, sym, symbols)
                    }
                    None => false,
                };
                if unreferenced {
                    self.flow.warning(
                        LintCategory::TRY,
                        ctx.tree(res_var).pos,
                        warnings::try_resource_not_referenced(symbol_place(ctx, sym)),
                    );
                }
            }
        }
        let inits_catch_prev = clone_bits(&inits_try);
        let uninits_catch_prev = clone_bits(&self.uninits_try);
        let mut inits_end_mut = inits_end;
        for catcher in catchers.iter() {
            let param = match &ctx.tree(catcher).kind {
                TreeKind::Catch { param, .. } => *param,
                _ => panic!("expected a catch clause"),
            };
            self.inits.assign(&mut clone_bits(&inits_catch_prev));
            self.uninits.assign(&mut clone_bits(&uninits_catch_prev));
            if let Some(param) = param {
                self.scan(ctx, Some(param));
                self.init_param(ctx, param);
            }
            let catch_body = match &ctx.tree(catcher).kind {
                TreeKind::Catch { body, .. } => *body,
                _ => panic!("expected a catch clause"),
            };
            self.scan(ctx, catch_body);
            inits_end_mut.and_set(&self.inits);
            uninits_end.and_set(&self.uninits);
            self.next_adr = next_adr_catch;
        }
        let inits_end = inits_end_mut;
        if let Some(finalizer) = finalizer {
            self.inits.assign(&mut clone_bits(&inits_try));
            self.uninits.assign(&mut clone_bits(&self.uninits_try));
            let exits = std::mem::replace(&mut self.pending_exits, pending_prev);
            self.scan(ctx, Some(finalizer));
            let finally_can_complete_normally = match &ctx.tree(tree).kind {
                TreeKind::Try {
                    finally_can_complete_normally,
                    ..
                } => *finally_can_complete_normally,
                _ => false,
            };
            if finally_can_complete_normally {
                let mut uninits_combined = clone_bits(&uninits_end);
                uninits_combined.and_set(&self.uninits);
                self.uninits.assign(&mut clone_bits(&uninits_combined));
                let mut current = exits;
                while current.non_empty() {
                    let exit = current.head().clone();
                    if let PendingExit::Assign {
                        exit_inits,
                        exit_uninits,
                        ..
                    } = &exit
                    {
                        let mut inits_copy = clone_bits(&exit_inits.borrow());
                        inits_copy.or_set(&self.inits);
                        *exit_inits.borrow_mut() = inits_copy;
                        let mut uninits_copy = clone_bits(&exit_uninits.borrow());
                        uninits_copy.and_set(&self.uninits);
                        *exit_uninits.borrow_mut() = uninits_copy;
                    }
                    self.pending_exits = self.pending_exits.clone().append(exit);
                    current = current.tail();
                }
                let mut inits_copy = clone_bits(&inits_end);
                inits_copy.or_set(&self.inits);
                self.inits.assign(&mut clone_bits(&inits_copy));
            } else {
                let mut current = exits;
                while current.non_empty() {
                    self.pending_exits = self.pending_exits.clone().append(current.head().clone());
                    current = current.tail();
                }
            }
        } else {
            self.inits.assign(&mut clone_bits(&inits_end));
            let mut uninits_copy = clone_bits(&uninits_end);
            self.uninits.assign(&mut uninits_copy);
            let exits = std::mem::replace(&mut self.pending_exits, pending_prev);
            let mut current = exits;
            while current.non_empty() {
                self.pending_exits = self.pending_exits.clone().append(current.head().clone());
                current = current.tail();
            }
        }
        let mut combined = clone_bits(&uninits_try_prev);
        combined.and_set(&self.uninits);
        self.uninits_try.assign(&mut clone_bits(&combined));
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
        self.scan_cond(ctx, cond.expect("conditional cond"));
        let mut inits_before_else = clone_bits(&self.inits_when_false);
        let mut uninits_before_else = clone_bits(&self.uninits_when_false);
        let inits_when_true = clone_bits(&self.inits_when_true);
        self.inits.assign(&mut clone_bits(&inits_when_true));
        let uninits_when_true = clone_bits(&self.uninits_when_true);
        self.uninits.assign(&mut clone_bits(&uninits_when_true));
        let truepart = truepart.expect("conditional truepart");
        let falsepart = falsepart.expect("conditional falsepart");
        let both_boolean = ctx
            .tree(truepart)
            .ty
            .map(|ty| ctx.typ(ty).has_tag(TypeTag::BOOLEAN))
            .unwrap_or(false)
            && ctx
                .tree(falsepart)
                .ty
                .map(|ty| ctx.typ(ty).has_tag(TypeTag::BOOLEAN))
                .unwrap_or(false);
        if both_boolean {
            self.scan_cond(ctx, truepart);
            let inits_after_then_when_true = clone_bits(&self.inits_when_true);
            let inits_after_then_when_false = clone_bits(&self.inits_when_false);
            let uninits_after_then_when_true = clone_bits(&self.uninits_when_true);
            let uninits_after_then_when_false = clone_bits(&self.uninits_when_false);
            self.inits.assign(&mut inits_before_else);
            self.uninits.assign(&mut uninits_before_else);
            self.scan_cond(ctx, falsepart);
            self.inits_when_true.and_set(&inits_after_then_when_true);
            self.inits_when_false.and_set(&inits_after_then_when_false);
            self.uninits_when_true.and_set(&uninits_after_then_when_true);
            self.uninits_when_false.and_set(&uninits_after_then_when_false);
        } else {
            self.scan_expr(ctx, Some(truepart));
            let inits_after_then = clone_bits(&self.inits);
            let uninits_after_then = clone_bits(&self.uninits);
            self.inits.assign(&mut inits_before_else);
            self.uninits.assign(&mut uninits_before_else);
            self.scan_expr(ctx, Some(falsepart));
            self.inits.and_set(&inits_after_then);
            self.uninits.and_set(&uninits_after_then);
        }
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
        self.scan_cond(ctx, cond.expect("if cond"));
        let mut inits_before_else = clone_bits(&self.inits_when_false);
        let mut uninits_before_else = clone_bits(&self.uninits_when_false);
        let inits_when_true = clone_bits(&self.inits_when_true);
        self.inits.assign(&mut clone_bits(&inits_when_true));
        let uninits_when_true = clone_bits(&self.uninits_when_true);
        self.uninits.assign(&mut clone_bits(&uninits_when_true));
        self.scan(ctx, thenpart);
        if let Some(elsepart) = elsepart {
            let inits_after_then = clone_bits(&self.inits);
            let uninits_after_then = clone_bits(&self.uninits);
            self.inits.assign(&mut inits_before_else);
            self.uninits.assign(&mut uninits_before_else);
            self.scan(ctx, Some(elsepart));
            self.inits.and_set(&inits_after_then);
            self.uninits.and_set(&uninits_after_then);
        } else {
            self.inits.and_set(&inits_before_else);
            self.uninits.and_set(&uninits_before_else);
        }
    }

    fn visit_break(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.record_exit(ctx, PendingExit::assign(tree, &self.inits, &self.uninits));
    }

    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, .. } => *value,
            _ => panic!("expected a yield"),
        };
        self.scan_expr(ctx, value);
        self.record_exit(ctx, PendingExit::assign(tree, &self.inits, &self.uninits));
    }

    fn visit_continue(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.record_exit(ctx, PendingExit::assign(tree, &self.inits, &self.uninits));
    }

    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Return { expr } => *expr,
            _ => panic!("expected a return"),
        };
        self.scan_expr(ctx, expr);
        self.record_exit(ctx, PendingExit::assign(tree, &self.inits, &self.uninits));
    }

    fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Throw { expr } => *expr,
            _ => panic!("expected a throw"),
        };
        self.scan_expr(ctx, expr);
        self.mark_dead(ctx);
    }

    fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (meth, args) = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation { meth, args, .. } => (*meth, args.clone()),
            _ => panic!("expected a method invocation"),
        };
        self.scan_expr(ctx, meth);
        self.scan_exprs(ctx, &args);
    }

    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (encl, args, def) = match &ctx.tree(tree).kind {
            TreeKind::NewClass { encl, args, def, .. } => (*encl, args.clone(), *def),
            _ => panic!("expected a new class"),
        };
        self.scan_expr(ctx, encl);
        self.scan_exprs(ctx, &args);
        self.scan(ctx, def);
    }

    fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if self.lambda_mode {
            // Java: LambdaAssignAnalyzer.visitLambda (Flow.java:1652-1663)
            if self.in_lambda {
                return;
            }
            self.in_lambda = true;
        }
        let (body, params) = match &ctx.tree(tree).kind {
            TreeKind::Lambda { body, params, .. } => (*body, params.clone()),
            _ => panic!("expected a lambda"),
        };
        let mut prev_uninits = clone_bits(&self.uninits);
        let mut prev_inits = clone_bits(&self.inits);
        let return_adr_prev = self.return_adr;
        let next_adr_prev = self.next_adr;
        let prev_pending = std::mem::take(&mut self.pending_exits);
        self.return_adr = self.next_adr;
        for param in params.iter() {
            self.scan(ctx, Some(param));
            if let Some(sym) = var_sym(ctx, param) {
                let adr = var_symbol_adr(ctx, sym);
                self.inits.incl(adr);
                self.uninits.excl(adr);
            }
        }
        if let Some(body) = body {
            if lambda_body_is_expression(ctx, tree) {
                self.scan_expr(ctx, Some(body));
            } else {
                self.scan(ctx, Some(body));
            }
        }
        self.return_adr = return_adr_prev;
        self.uninits.assign(&mut prev_uninits);
        self.inits.assign(&mut prev_inits);
        self.pending_exits = prev_pending;
        self.next_adr = next_adr_prev;
        if self.lambda_mode {
            self.in_lambda = false;
        }
    }

    fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (dims, elems) = match &ctx.tree(tree).kind {
            TreeKind::NewArray { dims, elems, .. } => (dims.clone(), elems.clone()),
            _ => panic!("expected a new array"),
        };
        self.scan_exprs(ctx, &dims);
        if let Some(elems) = elems {
            self.scan_exprs(ctx, &elems);
        }
    }

    fn visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, detail) = match &ctx.tree(tree).kind {
            TreeKind::Assert { cond, detail } => (*cond, *detail),
            _ => panic!("expected an assert"),
        };
        let inits_exit = clone_bits(&self.inits);
        let mut uninits_exit = clone_bits(&self.uninits);
        self.scan_cond(ctx, cond.expect("assert cond"));
        uninits_exit.and_set(&self.uninits_when_true);
        if let Some(detail) = detail {
            let inits_when_false = clone_bits(&self.inits_when_false);
            self.inits.assign(&mut clone_bits(&inits_when_false));
            let uninits_when_false = clone_bits(&self.uninits_when_false);
            self.uninits.assign(&mut clone_bits(&uninits_when_false));
            self.scan_expr(ctx, Some(detail));
        }
        self.inits.assign(&mut clone_bits(&inits_exit));
        self.uninits.assign(&mut uninits_exit);
    }

    fn visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Assign { lhs, rhs } => (
                lhs.expect("assignment lhs"),
                rhs.expect("assignment rhs"),
            ),
            _ => panic!("expected an assignment"),
        };
        if !tree_info::is_ident_or_this_dot_ident(ctx, lhs) {
            self.scan_expr(ctx, Some(lhs));
        }
        self.scan_expr(ctx, Some(rhs));
        self.let_init_tree(ctx, lhs);
    }

    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let selected = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { selected, .. } => *selected,
            _ => panic!("expected a field access"),
        };
        self.scan(ctx, selected);
        if let Some(sym) = tree_info::symbol(ctx, tree)
            && ctx.sym(sym).kind == Kind::VAR {
                if let Some(selected) = selected
                    && tree_info::is_this_qualifier(ctx, selected) {
                        self.check_init(ctx, ctx.tree(tree).pos, sym);
                    }
                self.referenced(ctx, sym);
            }
    }

    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { lhs, rhs, .. } => (
                lhs.expect("compound assignment lhs"),
                rhs.expect("compound assignment rhs"),
            ),
            _ => panic!("expected a compound assignment"),
        };
        self.scan_expr(ctx, Some(lhs));
        self.scan_expr(ctx, Some(rhs));
        self.let_init_tree(ctx, lhs);
    }

    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tag = ctx.tree(tree).tag;
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, .. } => arg.expect("unary arg"),
            _ => panic!("expected a unary expression"),
        };
        match tag {
            Tag::NOT => {
                self.scan_cond(ctx, arg);
                let t = clone_bits(&self.inits_when_false);
                let inits_when_true = clone_bits(&self.inits_when_true);
                self.inits_when_false.assign(&mut clone_bits(&inits_when_true));
                self.inits_when_true.assign(&mut clone_bits(&t));
                let t2 = clone_bits(&self.uninits_when_false);
                let uninits_when_true = clone_bits(&self.uninits_when_true);
                self.uninits_when_false.assign(&mut clone_bits(&uninits_when_true));
                self.uninits_when_true.assign(&mut clone_bits(&t2));
            }
            Tag::PREINC | Tag::POSTINC | Tag::PREDEC | Tag::POSTDEC => {
                self.scan_expr(ctx, Some(arg));
                self.let_init_tree(ctx, arg);
            }
            _ => self.scan_expr(ctx, Some(arg)),
        }
    }

    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tag = ctx.tree(tree).tag;
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Binary { lhs, rhs, .. } => (
                lhs.expect("binary lhs"),
                rhs.expect("binary rhs"),
            ),
            _ => panic!("expected a binary expression"),
        };
        match tag {
            Tag::AND => {
                self.scan_cond(ctx, lhs);
                let inits_when_false_left = clone_bits(&self.inits_when_false);
                let uninits_when_false_left = clone_bits(&self.uninits_when_false);
                let inits_when_true = clone_bits(&self.inits_when_true);
                self.inits.assign(&mut clone_bits(&inits_when_true));
                let uninits_when_true = clone_bits(&self.uninits_when_true);
                self.uninits.assign(&mut clone_bits(&uninits_when_true));
                self.scan_cond(ctx, rhs);
                self.inits_when_false.and_set(&inits_when_false_left);
                self.uninits_when_false.and_set(&uninits_when_false_left);
            }
            Tag::OR => {
                self.scan_cond(ctx, lhs);
                let inits_when_true_left = clone_bits(&self.inits_when_true);
                let uninits_when_true_left = clone_bits(&self.uninits_when_true);
                let inits_when_false = clone_bits(&self.inits_when_false);
                self.inits.assign(&mut clone_bits(&inits_when_false));
                let uninits_when_false = clone_bits(&self.uninits_when_false);
                self.uninits.assign(&mut clone_bits(&uninits_when_false));
                self.scan_cond(ctx, rhs);
                self.inits_when_true.and_set(&inits_when_true_left);
                self.uninits_when_true.and_set(&uninits_when_true_left);
            }
            _ => {
                self.scan_expr(ctx, Some(lhs));
                self.scan_expr(ctx, Some(rhs));
            }
        }
    }

    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let Some(sym) = tree_info::symbol(ctx, tree)
            && ctx.sym(sym).kind == Kind::VAR {
                self.check_init(ctx, ctx.tree(tree).pos, sym);
                self.referenced(ctx, sym);
            }
    }

    fn visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let var = match &ctx.tree(tree).kind {
            TreeKind::BindingPattern { var } => *var,
            _ => panic!("expected a binding pattern"),
        };
        if let Some(var) = var {
            self.scan(ctx, Some(var));
            self.init_param(ctx, var);
        }
    }

    fn visit_annotated_type(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let underlying_type = match &ctx.tree(tree).kind {
            TreeKind::AnnotatedType {
                underlying_type, ..
            } => *underlying_type,
            _ => panic!("expected an annotated type"),
        };
        if let Some(underlying_type) = underlying_type {
            self.scan(ctx, Some(underlying_type));
        }
    }

    fn visit_module_def(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}
}

impl<'a> AssignAnalyzer<'a> {
    /// Java: private void clearPendingExits(boolean inMethod)
    /// (Flow.java:2242-2263)
    fn clear_pending_exits(&mut self, ctx: &mut CompilerContext, in_method: bool) {
        let exits = std::mem::take(&mut self.pending_exits);
        let mut current = exits;
        while current.non_empty() {
            let exit = current.head().clone();
            let tree_id = exit.tree();
            let is_return = in_method && ctx.tree(tree_id).has_tag(Tag::RETURN);
            let has_error = self.flow.log.borrow().has_error_on(Some(Rc::new(
                SimpleDiagnosticPosition::new(ctx.tree(tree_id).pos),
            )));
            assert!(is_return || has_error, "AssertionError: clearPendingExits");
            if in_method && self.is_initial_constructor
                && let PendingExit::Assign { exit_inits, .. } = &exit {
                    let exit_inits = clone_bits(&exit_inits.borrow());
                    self.inits.assign(&mut clone_bits(&exit_inits));
                    let var_decls: Vec<SymbolId> = self
                        .vardecls
                        .iter()
                        .filter_map(|var_decl| *var_decl)
                        .filter_map(|var_decl| var_sym(ctx, var_decl))
                        .collect();
                    for var in var_decls {
                        self.check_init(ctx, ctx.tree(tree_id).pos, var);
                    }
                }
            current = current.tail();
        }
    }

    /// Java: protected void initParam(JCVariableDecl def) (Flow.java:2265-2268)
    fn init_param(&mut self, ctx: &CompilerContext, def: TreeId) {
        let Some(sym) = var_sym(ctx, def) else {
            return;
        };
        let adr = var_symbol_adr(ctx, sym);
        self.inits.incl(adr);
        self.uninits.excl(adr);
    }
}

/// Java: class Flow.CaptureAnalyzer extends BaseAnalyzer (Flow.java:2900-3084)
pub struct CaptureAnalyzer<'a> {
    flow: &'a Flow,
    pending_exits: List<PendingExit>,
    /// Java: JCTree currentTree (local class or lambda)
    current_tree: Option<TreeId>,
    /// Java: the inherited lint field (used by the augment sites in the
    /// upstream base pass; the ported CaptureAnalyzer keeps it for structure).
    #[allow(dead_code)]
    lint: Rc<Lint>,
}

impl<'a> CaptureAnalyzer<'a> {
    pub fn new(flow: &'a Flow) -> CaptureAnalyzer<'a> {
        CaptureAnalyzer {
            flow,
            pending_exits: List::nil(),
            current_tree: None,
            lint: Rc::clone(&flow.lint),
        }
    }

    /// Java: void checkEffectivelyFinal(DiagnosticPosition pos, VarSymbol sym)
    /// (Flow.java:2910-2932)
    fn check_effectively_final(&mut self, ctx: &mut CompilerContext, pos: i32, sym: SymbolId) {
        let Some(current_tree) = self.current_tree else {
            return;
        };
        let sym_pos = match &ctx.sym(sym).kind_data {
            crate::code::symbol::SymbolKind::VarSymbol { pos, .. } => *pos,
            _ => 0,
        };
        if matches!(ctx.sym(sym).owner.map(|owner| ctx.sym(owner).kind), Some(Kind::MTH))
            && sym_pos < tree_info::get_start_pos(ctx, current_tree)
        {
            match ctx.tree(current_tree).tag {
                Tag::CLASSDEF => {
                    if !self.flow.allow_effectively_final_in_inner_classes
                        && (ctx.sym(sym).flags_field & flags::FINAL as i64) == 0 {
                            self.report_inner_cls_needs_final_error(ctx, pos, sym);
                        }
                }
                Tag::GUARDPATTERN | Tag::LAMBDA
                    if (ctx.sym(sym).flags_field
                        & (flags::EFFECTIVELY_FINAL | flags::FINAL as i64))
                        == 0
                    => {
                        self.report_effectively_final_error(ctx, pos, sym);
                    }
                _ => {}
            }
        }
    }

    /// Java: void letInit(JCTree tree) (Flow.java:2934-2957)
    fn let_init(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tree = crate::tree::tree_info::skip_parens(ctx, tree);
        if (ctx.tree(tree).has_tag(Tag::IDENT) || ctx.tree(tree).has_tag(Tag::SELECT))
            && let Some(sym) = tree_info::symbol(ctx, tree) {
                let Some(current_tree) = self.current_tree else {
                    return;
                };
                let sym_pos = match &ctx.sym(sym).kind_data {
                    crate::code::symbol::SymbolKind::VarSymbol { pos, .. } => *pos,
                    _ => 0,
                };
                if ctx.sym(sym).kind == Kind::VAR
                    && matches!(ctx.sym(sym).owner.map(|owner| ctx.sym(owner).kind), Some(Kind::MTH))
                    && sym_pos < tree_info::get_start_pos(ctx, current_tree)
                {
                    match ctx.tree(current_tree).tag {
                        Tag::CLASSDEF => {
                            if !self.flow.allow_effectively_final_in_inner_classes {
                                self.report_inner_cls_needs_final_error(
                                    ctx,
                                    ctx.tree(tree).pos,
                                    sym,
                                );
                            } else {
                                self.report_effectively_final_error(ctx, ctx.tree(tree).pos, sym);
                            }
                        }
                        Tag::GUARDPATTERN | Tag::LAMBDA => {
                            self.report_effectively_final_error(ctx, ctx.tree(tree).pos, sym);
                        }
                        _ => {}
                    }
                }
            }
    }

    /// Java: void reportEffectivelyFinalError(DiagnosticPosition pos, Symbol sym)
    /// (Flow.java:2959-2967)
    fn report_effectively_final_error(&mut self, ctx: &CompilerContext, pos: i32, sym: SymbolId) {
        let fragment = match self
            .current_tree
            .map(|current_tree| ctx.tree(current_tree).tag)
        {
            Some(Tag::LAMBDA) => "lambda",
            Some(Tag::GUARDPATTERN) => "guard",
            Some(Tag::CLASSDEF) => "inner class",
            _ => "lambda",
        };
        self.flow.error(
            pos,
            errors::cant_ref_non_effectively_final_var(
                symbol_place(ctx, sym),
                crate::resources::diagnostic_contract::DiagnosticArg::from_display(fragment),
            ),
        );
    }

    /// Java: void reportInnerClsNeedsFinalError(DiagnosticPosition pos,
    /// Symbol sym) (Flow.java:2969-2972)
    fn report_inner_cls_needs_final_error(
        &mut self,
        ctx: &CompilerContext,
        pos: i32,
        sym: SymbolId,
    ) {
        self.flow.error(
            pos,
            errors::local_var_accessed_from_icls_needs_final(symbol_place(ctx, sym)),
        );
    }

    /// Java: public void analyzeTree(Env, JCTree, TreeMaker)
    /// (Flow.java:3070-3082)
    pub fn analyze_tree(
        &mut self,
        ctx: &mut CompilerContext,
        env: &Rc<Env<AttrContext>>,
        tree: TreeId,
    ) {
        let prev_pending = std::mem::take(&mut self.pending_exits);
        *self.flow.attr_env.borrow_mut() = Some(Rc::clone(env));
        self.scan(ctx, Some(tree));
        self.pending_exits = prev_pending;
    }
}

impl<'a> BaseAnalyzer for CaptureAnalyzer<'a> {
    fn pending_exits_mut(&mut self) -> &mut List<PendingExit> {
        &mut self.pending_exits
    }

    /// Java: CaptureAnalyzer.markDead() (Flow.java:2905-2908): do nothing.
    fn mark_dead(&mut self, _ctx: &mut CompilerContext) {}
}

flow_tree_visitor_delegate!(CaptureAnalyzer<'a>, 'a);

impl<'a> FlowVisitor for CaptureAnalyzer<'a> {
    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let prev_tree = self.current_tree;
        self.current_tree = {
            let directly_local = class_symbol(ctx, tree)
                .map(|sym| crate::code::symbol::symbol_is_directly_or_indirectly_local(ctx, sym))
                .unwrap_or(false);
            if directly_local { Some(tree) } else { None }
        };
        capture_default_visit_class_def(self, ctx, tree);
        self.current_tree = prev_tree;
    }

    fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let prev_tree = self.current_tree;
        self.current_tree = Some(tree);
        capture_default_visit_lambda(self, ctx, tree);
        self.current_tree = prev_tree;
    }

    fn visit_guard_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (patt, expr) = match &ctx.tree(tree).kind {
            TreeKind::GuardPattern { patt, expr } => (*patt, *expr),
            _ => panic!("expected a guard pattern"),
        };
        self.scan(ctx, patt);
        let prev_tree = self.current_tree;
        self.current_tree = Some(tree);
        self.scan(ctx, expr);
        self.current_tree = prev_tree;
    }

    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let Some(sym) = tree_info::symbol(ctx, tree)
            && ctx.sym(sym).kind == Kind::VAR {
                self.check_effectively_final(ctx, ctx.tree(tree).pos, sym);
            }
    }

    fn visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Assign { lhs, rhs } => (
                lhs.expect("assignment lhs"),
                rhs.expect("assignment rhs"),
            ),
            _ => panic!("expected an assignment"),
        };
        if !ctx.tree(lhs).has_tag(Tag::IDENT) {
            self.scan(ctx, Some(lhs));
        }
        self.scan(ctx, Some(rhs));
        self.let_init(ctx, lhs);
    }

    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { lhs, rhs, .. } => (
                lhs.expect("compound assignment lhs"),
                rhs.expect("compound assignment rhs"),
            ),
            _ => panic!("expected a compound assignment"),
        };
        self.scan(ctx, Some(lhs));
        self.scan(ctx, Some(rhs));
        self.let_init(ctx, lhs);
    }

    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, .. } => arg.expect("unary arg"),
            _ => panic!("expected a unary expression"),
        };
        self.scan(ctx, Some(arg));
        match ctx.tree(tree).tag {
            Tag::PREINC | Tag::POSTINC | Tag::PREDEC | Tag::POSTDEC => {
                self.let_init(ctx, arg);
            }
            _ => {}
        }
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
        for resource in resources.iter() {
            if !ctx.tree(resource).has_tag(Tag::VARDEF)
                && let Some(var) = tree_info::symbol(ctx, resource)
                    && (ctx.sym(var).flags_field
                        & (flags::FINAL as i64 | flags::EFFECTIVELY_FINAL))
                        == 0
                    {
                        self.flow.error(
                            ctx.tree(resource).pos,
                            errors::try_with_resources_expr_effectively_final_var(symbol_place(
                                ctx, var,
                            )),
                        );
                    }
        }
        // Java: super.visitTry(tree)
        self.scan_list(ctx, &resources);
        self.scan(ctx, body);
        self.scan_list(ctx, &catchers);
        self.scan(ctx, finalizer);
    }

    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, .. } => *value,
            _ => panic!("expected a yield"),
        };
        self.scan(ctx, value);
    }

    fn visit_module_def(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}
}

/// Java: CaptureAnalyzer.visitClassDef super call (TreeScanner.visitClassDef)
fn capture_default_visit_class_def(
    v: &mut CaptureAnalyzer,
    ctx: &mut CompilerContext,
    tree: TreeId,
) {
    let (mods, typarams, extending, implementing, permitting, defs) = match &ctx.tree(tree).kind {
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
    v.scan(ctx, mods);
    v.scan_list(ctx, &typarams);
    v.scan(ctx, extending);
    v.scan_list(ctx, &implementing);
    v.scan_list(ctx, &permitting);
    v.scan_list(ctx, &defs);
}

/// Java: CaptureAnalyzer.visitLambda super call (TreeScanner.visitLambda)
fn capture_default_visit_lambda(v: &mut CaptureAnalyzer, ctx: &mut CompilerContext, tree: TreeId) {
    let (params, body) = match &ctx.tree(tree).kind {
        TreeKind::Lambda { params, body, .. } => (params.clone(), *body),
        _ => panic!("expected a lambda"),
    };
    v.scan_list(ctx, &params);
    v.scan(ctx, body);
}

// ---------------------------------------------------------------------------
// Flow entry points (Flow.java:219-350)
// ---------------------------------------------------------------------------

impl Flow {
    /// Java: public void analyzeTree(Env<AttrContext> env, TreeMaker make)
    /// (Flow.java:219-224)
    pub fn analyze_tree(
        &self,
        ctx: &mut CompilerContext,
        env: &Rc<Env<AttrContext>>,
        make: &Rc<RefCell<TreeMaker>>,
    ) {
        *self.make.borrow_mut() = Some(Rc::clone(make));
        let tree = env.tree.expect("analyzeTree env.tree");
        AliveAnalyzer::new(self).analyze_tree(ctx, env, tree);
        AssignAnalyzer::new(self).analyze_tree(ctx, env, tree);
        FlowAnalyzer::new(self).analyze_tree(ctx, env, tree);
        CaptureAnalyzer::new(self).analyze_tree(ctx, env, tree);
        *self.make.borrow_mut() = None;
    }

    /// Java: public void analyzeLambda(Env, JCLambda, TreeMaker, boolean)
    /// (Flow.java:226-243)
    pub fn analyze_lambda(
        &self,
        ctx: &mut CompilerContext,
        env: &Rc<Env<AttrContext>>,
        that: TreeId,
        make: &Rc<RefCell<TreeMaker>>,
        speculative: bool,
    ) {
        if !speculative {
            self.log
                .borrow_mut()
                .install_diagnostic_handler(DiagnosticHandler::Discard { prev: None });
        }
        *self.make.borrow_mut() = Some(Rc::clone(make));
        LambdaAliveAnalyzer::new(self).analyze_tree(ctx, env, that);
        if !speculative {
            self.log.borrow_mut().pop_diagnostic_handler();
        }
        *self.make.borrow_mut() = None;
    }

    /// Java: public List<Type> analyzeLambdaThrownTypes(Env, JCLambda,
    /// TreeMaker) (Flow.java:245-261)
    pub fn analyze_lambda_thrown_types(
        &self,
        ctx: &mut CompilerContext,
        env: &Rc<Env<AttrContext>>,
        that: TreeId,
        make: &Rc<RefCell<TreeMaker>>,
    ) -> List<TypeId> {
        self.log
            .borrow_mut()
            .install_diagnostic_handler(DiagnosticHandler::Discard { prev: None });
        *self.make.borrow_mut() = Some(Rc::clone(make));
        let enclosed_class = env
            .encl_class
            .and_then(|encl_class| class_symbol(ctx, encl_class));
        let scope = ctx.scope_create(enclosed_class);
        AssignAnalyzer::new_lambda(self, scope).analyze_tree(ctx, env, that);
        let mut flow_analyzer = FlowAnalyzer::new_lambda(self);
        flow_analyzer.analyze_tree(ctx, env, that);
        self.log.borrow_mut().pop_diagnostic_handler();
        *self.make.borrow_mut() = None;
        flow_analyzer.inferred_thrown_types
    }

    /// Java: public boolean aliveAfter(Env, JCTree, TreeMaker)
    /// (Flow.java:263-278)
    pub fn alive_after(
        &self,
        ctx: &mut CompilerContext,
        env: &Rc<Env<AttrContext>>,
        that: TreeId,
        make: &Rc<RefCell<TreeMaker>>,
    ) -> bool {
        self.log
            .borrow_mut()
            .install_diagnostic_handler(DiagnosticHandler::Discard { prev: None });
        *self.make.borrow_mut() = Some(Rc::clone(make));
        let mut analyzer = SnippetAliveAnalyzer::new(self);
        analyzer.analyze_tree(ctx, env, that);
        self.log.borrow_mut().pop_diagnostic_handler();
        *self.make.borrow_mut() = None;
        analyzer.is_alive()
    }

    /// Java: public boolean breaksOutOf(Env, JCTree loop, JCTree body,
    /// TreeMaker) (Flow.java:280-297)
    pub fn breaks_out_of(
        &self,
        ctx: &mut CompilerContext,
        env: &Rc<Env<AttrContext>>,
        _loop_tree: TreeId,
        body: TreeId,
        make: &Rc<RefCell<TreeMaker>>,
    ) -> bool {
        self.log
            .borrow_mut()
            .install_diagnostic_handler(DiagnosticHandler::Discard { prev: None });
        *self.make.borrow_mut() = Some(Rc::clone(make));
        let mut analyzer = SnippetBreakAnalyzer::new(self);
        analyzer.analyze_tree(ctx, env, body);
        self.log.borrow_mut().pop_diagnostic_handler();
        *self.make.borrow_mut() = None;
        analyzer.breaks_out()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liveness_algebra() {
        use Liveness::*;
        assert_eq!(ALIVE.or(DEAD), ALIVE);
        assert_eq!(DEAD.or(ALIVE), ALIVE);
        assert_eq!(DEAD.or(DEAD), DEAD);
        assert_eq!(RECOVERY.or(ALIVE), ALIVE);
        assert_eq!(RECOVERY.or(RECOVERY), RECOVERY);
        assert_eq!(RECOVERY.or(DEAD), RECOVERY);
        assert_eq!(ALIVE.and(DEAD), DEAD);
        assert_eq!(DEAD.and(ALIVE), DEAD);
        assert_eq!(RECOVERY.and(DEAD), DEAD);
        assert_eq!(RECOVERY.and(ALIVE), RECOVERY);
        assert_eq!(ALIVE.and(RECOVERY), RECOVERY);
        assert_eq!(Liveness::from_bool(true), ALIVE);
        assert_eq!(Liveness::from_bool(false), DEAD);
        assert_eq!(DEAD.or_bool(true), ALIVE);
        assert_eq!(ALIVE.and_bool(false), DEAD);
    }

    #[test]
    fn flow_kind_keys() {
        assert_eq!(FlowKind::NORMAL.err_key(), "var.might.already.be.assigned");
        assert_eq!(
            FlowKind::SPECULATIVE_LOOP.err_key(),
            "var.might.be.assigned.in.loop"
        );
        assert!(!FlowKind::NORMAL.is_final());
        assert!(FlowKind::SPECULATIVE_LOOP.is_final());
    }

    #[test]
    fn jump_kind_tags() {
        assert_eq!(JumpKind::BREAK.tree_tag(), Tag::BREAK);
        assert_eq!(JumpKind::CONTINUE.tree_tag(), Tag::CONTINUE);
        assert_eq!(JumpKind::YIELD.tree_tag(), Tag::YIELD);
    }

    #[test]
    fn clone_bits_is_independent_copy() {
        let mut bits = Bits::new();
        bits.incl(1);
        bits.incl(5);
        let copy = clone_bits(&bits);
        assert!(copy.is_member(1));
        assert!(copy.is_member(5));
        assert!(!copy.is_member(2));
        assert_eq!(copy.next_bit(0), 1);
        assert_eq!(copy.next_bit(2), 5);
        assert_eq!(copy.next_bit(6), -1);
        // mutating the copy must not affect the source
        let mut mutated = copy;
        mutated.excl(1);
        assert!(!mutated.is_member(1));
        assert!(bits.is_member(1));
    }

    #[test]
    fn pending_exit_tree_and_assign_bits() {
        let tree = crate::ids::TreeId::new(7);
        assert_eq!(PendingExit::new(tree).tree(), tree);
        assert!(matches!(PendingExit::new(tree), PendingExit::Plain { .. }));
        let mut inits = Bits::new();
        inits.incl(3);
        let mut uninits = Bits::new();
        uninits.incl(4);
        let exit = PendingExit::assign(tree, &inits, &uninits);
        assert_eq!(exit.tree(), tree);
        match &exit {
            PendingExit::Assign {
                exit_inits,
                exit_uninits,
                ..
            } => {
                assert!(exit_inits.borrow().is_member(3));
                assert!(!exit_inits.borrow().is_member(4));
                assert!(exit_uninits.borrow().is_member(4));
                assert!(!exit_uninits.borrow().is_member(3));
            }
            _ => panic!("expected AssignPendingExit"),
        }
        // the exit bits are copies (mutating the source does not affect them)
        let mut inits2 = inits;
        inits2.incl(9);
        match &exit {
            PendingExit::Assign { exit_inits, .. } => {
                assert!(!exit_inits.borrow().is_member(9))
            }
            _ => panic!("expected AssignPendingExit"),
        }
    }

    #[test]
    fn bits_defaults_of_analyzer_state() {
        // Flow.IsFinal etc. are checked through FlowKind/JumpKind tests above;
        // here we verify the analyzer-state bit helper contract used by the
        // loop fixpoints (diffSet/andSet/excludeFrom/nextBit).
        let mut entry = Bits::new();
        entry.incl_range(0, 5);
        let mut current = Bits::new();
        current.incl(0);
        current.incl(2);
        let mut probe = clone_bits(&entry);
        probe.diff_set(&current);
        assert_eq!(probe.next_bit(0), 1);
        probe.exclude_from(2);
        assert_eq!(probe.next_bit(0), 1);
        probe.excl(1);
        assert_eq!(probe.next_bit(0), -1);
    }
    #[test]
    fn analyze_tree_runs_on_an_empty_class_without_panicking() {
        use crate::code::source::Source;
        use crate::code::symtab::Symtab;
        use crate::code::type_::Type;
        use crate::code::types::Types;
        use crate::comp::attr_context::AttrContext;
        use crate::comp::check::Check;
        use crate::comp::env::Env;
        use crate::comp::resolve::Resolve;
        use crate::jvm::target::Target;
        use crate::tree::jc_tree::JCTree;
        use crate::util::context::Context;
        use crate::util::javac_messages::JavacMessages;
        use crate::util::options::Options;

        let mut ctx = CompilerContext::new();
        let mut jcontext = Context::new();
        let names = Rc::new(Names::new(false));
        let log = Log::instance(&mut jcontext);
        let source = Source::JDK17;
        let syms = Rc::new(RefCell::new(Symtab::new(
            &mut ctx,
            Rc::clone(&names),
            Target::DEFAULT,
            source,
        )));
        let types = Rc::new(Types::with_names(Rc::clone(&names)));
        let messages = JavacMessages::instance(&mut jcontext);
        let diags = Rc::new(Factory::new(messages, "compiler"));
        let lint = Lint::instance(&mut jcontext);
        let deferred = Rc::new(RefCell::new(
            crate::code::deferred_lint_handler::DeferredLintHandler::new(),
        ));
        let check = Rc::new(RefCell::new(Check::new(
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&diags),
            source,
            '$',
            Rc::clone(&types),
            Rc::clone(&lint),
            Rc::clone(&deferred),
            Rc::clone(&syms),
        )));
        crate::comp::check::attach_self(&check);
        let options = Options::instance(&mut jcontext);
        let resolve = Rc::new(RefCell::new(Resolve::new(
            &mut ctx,
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&types),
            Rc::clone(&diags),
            source,
            options,
            None,
        )));
        let flow = Flow::new(
            &mut ctx,
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&syms),
            Rc::clone(&types),
            check,
            resolve,
            Rc::clone(&diags),
            Rc::clone(&lint),
            source,
            None,
        );
        let make = Rc::new(RefCell::new(TreeMaker::new(Some(Rc::clone(&names)))));
        // An empty ClassDecl with a class symbol exercises every analyzer's
        // class/method entry point, including the ported Lint.augment.
        let no_type = ctx.alloc_type(Type::new(
            TypeTag::NONE,
            None,
            crate::code::type_::TypeKindData::JCNoType { kind: crate::code::type_::JCNoTypeKind::Base },
        ));
        let class_sym = crate::code::symtab::new_class_symbol(
            &mut ctx,
            0,
            names.empty.clone(),
            None,
            no_type,
        );
        let class = ctx.alloc_tree(JCTree {
            pos: 1,
            ty: None,
            tag: Tag::CLASSDEF,
            kind: TreeKind::ClassDecl {
                mods: None,
                name: names.empty.clone(),
                typarams: List::nil(),
                extending: None,
                implementing: List::nil(),
                permitting: List::nil(),
                defs: List::nil(),
                sym: Some(class_sym),
            },
        });
        let env: Rc<Env<AttrContext>> = Rc::new(Env {
            next: None,
            outer: None,
            tree: Some(class),
            toplevel: None,
            encl_class: Some(class),
            encl_method: None,
            info: Rc::new(RefCell::new(AttrContext::new())),
            base_clause: false,
        });
        flow.analyze_tree(&mut ctx, &env, &make);
        assert_eq!(log.borrow().nerrors, 0);
    }


    /// Java: Flow.AliveAnalyzer.isExhaustive/transitiveCovers (Flow.java:752-803):
    /// enum constants cover the enum type, permitted subclasses cover the sealed
    /// supertype, and a type variable exhausts through its upper bound.
    #[test]
    fn switch_exhaustiveness_covers_enum_and_sealed_types() {
        use crate::code::flags;
        use crate::code::kinds::Kind as SymKind;
        use crate::code::source::Source;
        use crate::code::symbol::{Symbol, SymbolKind};
        use crate::code::symtab::Symtab;
        use crate::code::type_::Type;
        use crate::code::types::Types;
        use crate::comp::check::Check;
        use crate::comp::resolve::Resolve;
        use crate::jvm::target::Target;
        use crate::util::context::Context;
        use crate::util::javac_messages::JavacMessages;
        use crate::util::options::Options;

        let mut ctx = CompilerContext::new();
        let mut jcontext = Context::new();
        let names = Rc::new(Names::new(false));
        let log = Log::instance(&mut jcontext);
        let source = Source::JDK17;
        let syms = Rc::new(RefCell::new(Symtab::new(
            &mut ctx,
            Rc::clone(&names),
            Target::DEFAULT,
            source,
        )));
        let types = Rc::new(Types::with_names(Rc::clone(&names)));
        let messages = JavacMessages::instance(&mut jcontext);
        let diags = Rc::new(Factory::new(messages, "compiler"));
        let lint = Lint::instance(&mut jcontext);
        let deferred = Rc::new(RefCell::new(
            crate::code::deferred_lint_handler::DeferredLintHandler::new(),
        ));
        let check = Rc::new(RefCell::new(Check::new(
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&diags),
            source,
            '$',
            Rc::clone(&types),
            Rc::clone(&lint),
            Rc::clone(&deferred),
            Rc::clone(&syms),
        )));
        crate::comp::check::attach_self(&check);
        let options = Options::instance(&mut jcontext);
        let resolve = Rc::new(RefCell::new(Resolve::new(
            &mut ctx,
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&types),
            Rc::clone(&diags),
            source,
            options,
            None,
        )));
        let flow = Flow::new(
            &mut ctx,
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&syms),
            Rc::clone(&types),
            check,
            resolve,
            Rc::clone(&diags),
            Rc::clone(&lint),
            source,
            None,
        );
        let mut analyzer = AliveAnalyzer::new(&flow);
        let no_type = ctx.alloc_type(Type::new(
            TypeTag::NONE,
            None,
            crate::code::type_::TypeKindData::JCNoType { kind: crate::code::type_::JCNoTypeKind::Base },
        ));
        let enum_sym = crate::code::symtab::new_class_symbol(
            &mut ctx,
            flags::ENUM as i64,
            names.from_utf(b"E"),
            None,
            no_type,
        );
        let enum_type = ctx.sym(enum_sym).ty.expect("enum type");
        let members = ctx.scope_create(Some(enum_sym));
        let (a, b) = {
            let mut member_scope = ctx.scope(members).clone();
            let mut enter_constant = |name: &[u8]| {
                let constant = ctx.alloc_symbol(Symbol::new(
                    SymKind::VAR,
                    flags::ENUM as i64,
                    names.from_utf(name),
                    None,
                    Some(enum_sym),
                    SymbolKind::VarSymbol { pos: 0, adr: 0, data_kind: None, data: None, lazy_const_value: None },
                ));
                let symbols = ctx.symbols_arena();
                member_scope.enter(members, constant, symbols);
                constant
            };
            let a = enter_constant(b"A");
            let b = enter_constant(b"B");
            *ctx.scope_mut(members) = member_scope;
            (a, b)
        };
        if let SymbolKind::ClassSymbol { members_field, .. } = &mut ctx.sym_mut(enum_sym).kind_data {
            *members_field = Some(members);
        }
        let mut one = HashSet::new();
        one.insert(a);
        assert!(!analyzer.is_exhaustive(&mut ctx, enum_type, &mut one));
        let mut both = HashSet::new();
        both.insert(a);
        both.insert(b);
        assert!(analyzer.is_exhaustive(&mut ctx, enum_type, &mut both));
        let p_sym = crate::code::symtab::new_class_symbol(
            &mut ctx,
            (flags::ABSTRACT as i64) | (flags::SEALED as i64),
            names.from_utf(b"P"),
            None,
            no_type,
        );
        let p_type = ctx.sym(p_sym).ty.expect("P type");
        let s1_sym = crate::code::symtab::new_class_symbol(
            &mut ctx,
            0,
            names.from_utf(b"S1"),
            None,
            no_type,
        );
        let s2_sym = crate::code::symtab::new_class_symbol(
            &mut ctx,
            0,
            names.from_utf(b"S2"),
            None,
            no_type,
        );
        let s1_type = ctx.sym(s1_sym).ty.expect("S1 type");
        let s2_type = ctx.sym(s2_sym).ty.expect("S2 type");
        for sub_type in [s1_type, s2_type] {
            if let crate::code::type_::TypeKindData::ClassType { supertype_field, .. } =
                &mut ctx.typ_mut(sub_type).kind
            {
                *supertype_field = Some(p_type);
            }
        }
        if let SymbolKind::ClassSymbol { permitted, .. } = &mut ctx.sym_mut(p_sym).kind_data {
            *permitted = List::of(s1_sym).append(s2_sym);
        }
        let mut sealed_covered = HashSet::new();
        sealed_covered.insert(s1_sym);
        sealed_covered.insert(s2_sym);
        assert!(analyzer.is_exhaustive(&mut ctx, p_type, &mut sealed_covered));
        let mut partial = HashSet::new();
        partial.insert(s1_sym);
        assert!(!analyzer.is_exhaustive(&mut ctx, p_type, &mut partial));
        let tvar = ctx.alloc_type(Type::new(
            TypeTag::TYPEVAR,
            None,
            crate::code::type_::TypeKindData::TypeVar {
                bound: Some(p_type),
                lower: None,
            },
        ));
        let mut tvar_covered = HashSet::new();
        tvar_covered.insert(s1_sym);
        tvar_covered.insert(s2_sym);
        assert!(analyzer.is_exhaustive(&mut ctx, tvar, &mut tvar_covered));
    }
}
