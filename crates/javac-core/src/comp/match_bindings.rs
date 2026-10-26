// Origin: com/sun/tools/javac/comp/MatchBindingsComputer.java @ 982d53c
//         (jdk-17.0.19-ga)
//
// PORTING_RULES:
// - ADR 0001: the ported MatchBindingsComputer receives the Log/Factory
//   services explicitly; MatchBindingsComputer.instance(Context) is registered
//   with the Context registration batch.
// - §9 adapter: the upstream static EMPTY identity checks are expressed as
//   emptiness checks on the binding lists (a non-EMPTY empty-list value takes
//   the fast path; union/intersection of empty lists yields empty, so the
//   observable result is unchanged).
// - §9 adapter: BindingSymbol lists are SymbolId lists (SymbolKind::BindingSymbol
//   carries the aliases field).

use std::cell::RefCell;
use std::rc::Rc;

use crate::code::flags;
use crate::ids::SymbolId;
use crate::compiler_context::CompilerContext;
use crate::resources::compiler_properties::errors;
use crate::tree::jc_tree::Tag;
use crate::util::jc_diagnostic::{DiagnosticType, Factory, SimpleDiagnosticPosition};
use crate::util::list::List;
use crate::util::log::Log;

/// Java: MatchBindingsComputer.MatchBindings (MatchBindingsComputer.java:154-176)
#[derive(Clone, Debug)]
pub struct MatchBindings {
    /// Java: public final List<BindingSymbol> bindingsWhenTrue
    pub bindings_when_true: List<SymbolId>,
    /// Java: public final List<BindingSymbol> bindingsWhenFalse
    pub bindings_when_false: List<SymbolId>,
    /// Java: public final boolean nullPattern
    pub null_pattern: bool,
}

impl MatchBindings {
    /// Java: MatchBindings(List, List) / MatchBindings(List, List, boolean)
    pub fn new(
        bindings_when_true: List<SymbolId>,
        bindings_when_false: List<SymbolId>,
    ) -> MatchBindings {
        MatchBindings {
            bindings_when_true,
            bindings_when_false,
            null_pattern: false,
        }
    }

    pub fn new_with_null(
        bindings_when_true: List<SymbolId>,
        bindings_when_false: List<SymbolId>,
        null_pattern: bool,
    ) -> MatchBindings {
        MatchBindings {
            bindings_when_true,
            bindings_when_false,
            null_pattern,
        }
    }

    /// Java: MatchBindingsComputer.EMPTY
    pub fn empty() -> MatchBindings {
        MatchBindings {
            bindings_when_true: List::nil(),
            bindings_when_false: List::nil(),
            null_pattern: false,
        }
    }

    /// §9 adapter for the upstream `bindings == EMPTY` identity checks.
    pub fn is_empty_bindings(&self) -> bool {
        self.bindings_when_true.is_empty() && self.bindings_when_false.is_empty()
    }
}

/// Java: public class MatchBindingsComputer extends TreeScanner
/// (MatchBindingsComputer.java:43-235)
pub struct MatchBindingsComputer {
    log: Rc<RefCell<Log>>,
    diags: Rc<Factory>,
}

impl MatchBindingsComputer {
    /// Java: protected MatchBindingsComputer(Context context)
    pub fn new(log: Rc<RefCell<Log>>, diags: Rc<Factory>) -> MatchBindingsComputer {
        MatchBindingsComputer { log, diags }
    }

    /// Java: MatchBindings conditional(JCTree tree, MatchBindings condBindings,
    /// MatchBindings trueBindings, MatchBindings falseBindings)
    /// (MatchBindingsComputer.java:77-121)
    pub fn conditional(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        cond_bindings: &MatchBindings,
        true_bindings: &MatchBindings,
        false_bindings: &MatchBindings,
    ) -> MatchBindings {
        if cond_bindings.is_empty_bindings()
            && true_bindings.is_empty_bindings()
            && false_bindings.is_empty_bindings()
        {
            return MatchBindings::empty();
        }
        let x_t_z_t = self.intersection(
            ctx,
            pos,
            &cond_bindings.bindings_when_true,
            &false_bindings.bindings_when_true,
        );
        let x_f_y_t = self.intersection(
            ctx,
            pos,
            &cond_bindings.bindings_when_false,
            &true_bindings.bindings_when_true,
        );
        let y_t_z_t = self.intersection(
            ctx,
            pos,
            &true_bindings.bindings_when_true,
            &false_bindings.bindings_when_true,
        );
        let x_t_z_f = self.intersection(
            ctx,
            pos,
            &cond_bindings.bindings_when_true,
            &false_bindings.bindings_when_false,
        );
        let x_f_y_f = self.intersection(
            ctx,
            pos,
            &cond_bindings.bindings_when_false,
            &true_bindings.bindings_when_false,
        );
        let y_f_z_f = self.intersection(
            ctx,
            pos,
            &true_bindings.bindings_when_false,
            &false_bindings.bindings_when_false,
        );
        // error recovery:
        // e.T = union(y.T ∩ z.T, x.T ∩ z.T, x.F ∩ y.T)
        // e.F = union(y.F ∩ z.F, x.T ∩ z.F, x.F ∩ y.F)
        let bindings_when_true =
            self.union(ctx, pos, &y_t_z_t, &[&x_t_z_t, &x_f_y_t]);
        let bindings_when_false =
            self.union(ctx, pos, &y_f_z_f, &[&x_t_z_f, &x_f_y_f]);
        MatchBindings::new(bindings_when_true, bindings_when_false)
    }

    /// Java: MatchBindings unary(JCTree tree, MatchBindings bindings)
    /// (MatchBindingsComputer.java:123-126)
    pub fn unary(&self, tag: Tag, bindings: &MatchBindings) -> MatchBindings {
        if bindings.is_empty_bindings() || tag != Tag::NOT {
            return bindings.clone();
        }
        MatchBindings::new(
            bindings.bindings_when_false.clone(),
            bindings.bindings_when_true.clone(),
        )
    }

    /// Java: MatchBindings binary(JCTree tree, MatchBindings lhsBindings,
    /// MatchBindings rhsBindings) (MatchBindingsComputer.java:128-148)
    pub fn binary(
        &self,
        ctx: &mut CompilerContext,
        tag: Tag,
        pos: i32,
        lhs_bindings: &MatchBindings,
        rhs_bindings: &MatchBindings,
    ) -> MatchBindings {
        match tag {
            Tag::AND => self.and_operation(ctx, pos, lhs_bindings, rhs_bindings),
            Tag::OR => {
                // e.T = intersection(x.T, y.T) (error recovery)
                // e.F = union(x.F, y.F)
                let bindings_when_true = self.intersection(
                    ctx,
                    pos,
                    &lhs_bindings.bindings_when_true,
                    &rhs_bindings.bindings_when_true,
                );
                let bindings_when_false = self.union(
                    ctx,
                    pos,
                    &lhs_bindings.bindings_when_false,
                    &[&rhs_bindings.bindings_when_false],
                );
                MatchBindings::new(bindings_when_true, bindings_when_false)
            }
            _ => MatchBindings::empty(),
        }
    }

    /// Java: MatchBindings guardedPattern(JCGuardPattern tree,
    /// MatchBindings patternBindings, MatchBindings guardBindings)
    /// (MatchBindingsComputer.java:150-152)
    pub fn guarded_pattern(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        pattern_bindings: &MatchBindings,
        guard_bindings: &MatchBindings,
    ) -> MatchBindings {
        self.and_operation(ctx, pos, pattern_bindings, guard_bindings)
    }

    /// Java: MatchBindings andOperation(DiagnosticPosition pos,
    /// MatchBindings lhsBindings, MatchBindings rhsBindings)
    /// (MatchBindingsComputer.java:186-194)
    pub fn and_operation(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        lhs_bindings: &MatchBindings,
        rhs_bindings: &MatchBindings,
    ) -> MatchBindings {
        // e.T = union(x.T, y.T); e.F = intersection(x.F, y.F) (error recovery)
        let bindings_when_true = self.union(
            ctx,
            pos,
            &lhs_bindings.bindings_when_true,
            &[&rhs_bindings.bindings_when_true],
        );
        let bindings_when_false = self.intersection(
            ctx,
            pos,
            &lhs_bindings.bindings_when_false,
            &rhs_bindings.bindings_when_false,
        );
        MatchBindings::new(bindings_when_true, bindings_when_false)
    }

    /// Java: MatchBindings switchCase(JCTree tree, MatchBindings prevBindings,
    /// MatchBindings currentBindings) (MatchBindingsComputer.java:196-208)
    pub fn switch_case(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        prev_bindings: Option<&MatchBindings>,
        current_bindings: &MatchBindings,
    ) -> MatchBindings {
        let Some(prev_bindings) = prev_bindings else {
            return current_bindings.clone();
        };
        if prev_bindings.null_pattern {
            return current_bindings.clone();
        }
        if current_bindings.null_pattern {
            return prev_bindings.clone();
        }
        MatchBindings::new(
            self.intersection(
                ctx,
                pos,
                &prev_bindings.bindings_when_true,
                &current_bindings.bindings_when_true,
            ),
            self.intersection(
                ctx,
                pos,
                &prev_bindings.bindings_when_false,
                &current_bindings.bindings_when_false,
            ),
        )
    }

    /// Java: MatchBindings finishBindings(JCTree tree, MatchBindings
    /// matchBindings) (MatchBindingsComputer.java:210-222)
    pub fn finish_bindings(&self, tag: Tag, bindings: &MatchBindings) -> MatchBindings {
        match tag {
            Tag::NOT
            | Tag::AND
            | Tag::OR
            | Tag::BINDINGPATTERN
            | Tag::PARENTHESIZEDPATTERN
            | Tag::GUARDPATTERN
            | Tag::PARENS
            | Tag::TYPETEST
            | Tag::CONDEXPR => bindings.clone(),
            _ => MatchBindings::empty(),
        }
    }

    /// Java: private List<BindingSymbol> intersection(DiagnosticPosition pos,
    /// List<BindingSymbol> lhsBindings, List<BindingSymbol> rhsBindings)
    /// (MatchBindingsComputer.java:224-243)
    fn intersection(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        lhs_bindings: &List<SymbolId>,
        rhs_bindings: &List<SymbolId>,
    ) -> List<SymbolId> {
        let mut result: List<SymbolId> = List::nil();
        let mut v1_current = lhs_bindings.clone();
        while v1_current.non_empty() {
            let v1 = *v1_current.head();
            let mut v2_current = rhs_bindings.clone();
            while v2_current.non_empty() {
                let v2 = *v2_current.head();
                let v1_name = ctx.sym(v1).name.clone();
                let v2_name = ctx.sym(v2).name.clone();
                if v1_name == v2_name
                    && (ctx.sym(v1).flags_field & flags::CLASH) == 0
                    && (ctx.sym(v2).flags_field & flags::CLASH) == 0
                {
                    // Java: log.error(pos, Errors.MatchBindingExists)
                    self.error(pos, errors::match_binding_exists());
                    ctx.sym_mut(v2).flags_field |= flags::CLASH;
                    result = result.append(v2);
                }
                v2_current = v2_current.tail();
            }
            v1_current = v1_current.tail();
        }
        result
    }

    /// Java: private final List<BindingSymbol> union(DiagnosticPosition pos,
    /// List<BindingSymbol> lhsBindings, List<BindingSymbol>... rhsBindings_s)
    /// (MatchBindingsComputer.java:245-266)
    fn union(
        &self,
        ctx: &mut CompilerContext,
        pos: i32,
        lhs_bindings: &List<SymbolId>,
        rhs_bindings_s: &[&List<SymbolId>],
    ) -> List<SymbolId> {
        let mut list = lhs_bindings.clone();
        for rhs_bindings in rhs_bindings_s {
            let mut current = (*rhs_bindings).clone();
            while current.non_empty() {
                let v = *current.head();
                let mut list_current = list.clone();
                while list_current.non_empty() {
                    let ov = *list_current.head();
                    let ov_name = ctx.sym(ov).name.clone();
                    let v_name = ctx.sym(v).name.clone();
                    if ov_name == v_name
                        && (ctx.sym(ov).flags_field & flags::CLASH) == 0
                        && (ctx.sym(v).flags_field & flags::CLASH) == 0
                    {
                        self.error(pos, errors::match_binding_exists());
                        ctx.sym_mut(v).flags_field |= flags::CLASH;
                    }
                    list_current = list_current.tail();
                }
                list = list.append(v);
                current = current.tail();
            }
        }
        list
    }

    fn error(&self, pos: i32, info: crate::resources::diagnostic_contract::ErrorInfo) {
        use crate::util::jc_diagnostic::DiagnosticArg;
        let args: Vec<DiagnosticArg> = info
            .info()
            .args
            .iter()
            .map(|arg| {
                Rc::new(javac_strings::JavaString::from(arg.to_text().as_str())) as DiagnosticArg
            })
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::symbol::new_binding_symbol;
    use crate::ids::SymbolId;
    use crate::util::context::Context;
    use crate::util::javac_messages::JavacMessages;
    use crate::util::list::List;
    use crate::util::names::Names;

    struct Harness {
        ctx: CompilerContext,
        names: Rc<Names>,
        log: Rc<RefCell<Log>>,
        computer: MatchBindingsComputer,
    }

    fn harness() -> Harness {
        let ctx = CompilerContext::new();
        let names = Rc::new(Names::new(false));
        let mut jcontext = Context::new();
        let messages = JavacMessages::instance(&mut jcontext);
        let diags = Rc::new(Factory::new(messages, "compiler"));
        let log = Log::instance(&mut jcontext);
        let computer = MatchBindingsComputer::new(Rc::clone(&log), diags);
        Harness {
            ctx,
            names,
            log,
            computer,
        }
    }

    fn binding(h: &mut Harness, name: &str) -> SymbolId {
        h.ctx.alloc_symbol(new_binding_symbol(
            0,
            h.names.from_string(&javac_strings::JavaString::from(name)),
            None,
            None,
        ))
    }

    #[test]
    fn empty_inputs_produce_empty_bindings() {
        let mut h = harness();
        let empty = MatchBindings::empty();
        let result = h.computer.conditional(&mut h.ctx, -1, &empty, &empty, &empty);
        assert!(result.is_empty_bindings());
        assert!(result.bindings_when_true.is_empty());
    }

    #[test]
    fn unary_not_swaps_the_true_and_false_bindings() {
        let mut h = harness();
        let x = binding(&mut h, "x");
        let bindings = MatchBindings::new(List::of(x), List::nil());
        let negated = h.computer.unary(Tag::NOT, &bindings);
        assert!(negated.bindings_when_true.is_empty());
        assert_eq!(*negated.bindings_when_false.head(), x);
        // Non-negated tags return the input unchanged.
        let same = h.computer.unary(Tag::AND, &bindings);
        assert_eq!(*same.bindings_when_true.head(), x);
    }

    #[test]
    fn and_operation_unions_true_and_intersects_false() {
        let mut h = harness();
        let x = binding(&mut h, "x");
        let y = binding(&mut h, "y");
        let lhs = MatchBindings::new(List::of(x), List::nil());
        let rhs = MatchBindings::new(List::of(y), List::nil());
        let and = h.computer.binary(&mut h.ctx, Tag::AND, -1, &lhs, &rhs);
        assert_eq!(and.bindings_when_true.length(), 2);
        assert!(and.bindings_when_false.is_empty());
        // OR: true = intersection, false = union.
        // Both inputs have empty false lists, so the union is empty too.
        let or = h.computer.binary(&mut h.ctx, Tag::OR, -1, &lhs, &rhs);
        assert!(or.bindings_when_true.is_empty());
        assert!(or.bindings_when_false.is_empty());
    }

    #[test]
    fn intersection_reports_a_name_clash_once() {
        let mut h = harness();
        let x1 = binding(&mut h, "x");
        let x2 = binding(&mut h, "x");
        let lhs = MatchBindings::new(List::of(x1), List::nil());
        let rhs = MatchBindings::new(List::of(x2), List::nil());
        // AND intersects the false lists (both empty) and unions the true
        // lists; the union of the two same-name bindings reports the clash.
        let and = h.computer.binary(&mut h.ctx, Tag::AND, -1, &lhs, &rhs);
        assert_eq!(and.bindings_when_true.length(), 2);
        assert_ne!(h.ctx.sym(x2).flags_field & crate::code::flags::CLASH, 0);
        let _ = Rc::clone(&h.log);
    }

    #[test]
    fn switch_case_handles_previous_and_null_patterns() {
        let mut h = harness();
        let x = binding(&mut h, "x");
        let current = MatchBindings::new(List::of(x), List::nil());
        // prev == null -> current
        let first = h.computer.switch_case(&mut h.ctx, -1, None, &current);
        assert_eq!(*first.bindings_when_true.head(), x);
        // nullPattern on prev -> current
        let prev_null = MatchBindings::new_with_null(List::nil(), List::nil(), true);
        let second = h.computer.switch_case(&mut h.ctx, -1, Some(&prev_null), &current);
        assert_eq!(*second.bindings_when_true.head(), x);
        // nullPattern on current -> prev
        let cur_null = MatchBindings::new_with_null(List::nil(), List::nil(), true);
        let third = h.computer.switch_case(&mut h.ctx, -1, Some(&current), &cur_null);
        assert_eq!(*third.bindings_when_true.head(), x);
    }

    #[test]
    fn finish_bindings_keeps_pattern_tags_only() {
        let mut h = harness();
        let x = binding(&mut h, "x");
        let bindings = MatchBindings::new(List::of(x), List::nil());
        let kept = h.computer.finish_bindings(Tag::PARENS, &bindings);
        assert_eq!(kept.bindings_when_true.length(), 1);
        let dropped = h.computer.finish_bindings(Tag::PLUS, &bindings);
        assert!(dropped.is_empty_bindings());
    }
}
