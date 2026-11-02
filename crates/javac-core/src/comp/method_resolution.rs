//! Method resolution phases and the resolution step order.

use crate::code::kinds::Kind;
use crate::compiler_context::CompilerContext;
use crate::ids::SymbolId;

/// Java: enum Resolve.MethodResolutionPhase
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MethodResolutionPhase {
    #[default]
    BASIC,
    BOX,
    VARARITY,
}

impl MethodResolutionPhase {
    /// Java: public boolean isBoxingRequired()
    pub fn is_boxing_required(self) -> bool {
        matches!(
            self,
            MethodResolutionPhase::BOX | MethodResolutionPhase::VARARITY
        )
    }

    /// Java: public boolean isVarargsRequired()
    pub fn is_varargs_required(self) -> bool {
        matches!(self, MethodResolutionPhase::VARARITY)
    }

    /// Java: enum ordinal (BASIC < BOX < VARARITY).
    pub fn ordinal(self) -> usize {
        match self {
            MethodResolutionPhase::BASIC => 0,
            MethodResolutionPhase::BOX => 1,
            MethodResolutionPhase::VARARITY => 2,
        }
    }

    /// Java: MethodResolutionPhase.values()
    pub fn values() -> Vec<MethodResolutionPhase> {
        vec![
            MethodResolutionPhase::BASIC,
            MethodResolutionPhase::BOX,
            MethodResolutionPhase::VARARITY,
        ]
    }

    /// Java: boolean shouldStop(Symbol sym, MethodResolutionPhase phase) is on
    /// LookupHelper; phase comparison uses the ordinal.
    pub fn is_after(self, other: MethodResolutionPhase) -> bool {
        self.ordinal() > other.ordinal()
    }

    /// Java: public Symbol mergeResults(Symbol prev, Symbol sym)
    /// (Resolve.java:4867-4925; VARARITY overrides the base behavior)
    pub fn merge_results(
        self,
        ctx: &crate::compiler_context::CompilerContext,
        prev: SymbolId,
        sym: SymbolId,
    ) -> SymbolId {
        if self != MethodResolutionPhase::VARARITY {
            return sym;
        }
        let prev_kind = ctx.sym(prev).kind;
        let sym_kind = ctx.sym(sym).kind;
        // Java: Assert.check(bestSoFar.kind.isResolutionError() &&
        // bestSoFar.kind != AMBIGUOUS) (Resolve.java:4872-4874)
        assert!(
            prev_kind.is_resolution_error() && prev_kind != Kind::AMBIGUOUS,
            "AssertionError: mergeResults invariant"
        );
        if !sym_kind.is_resolution_error() {
            // varargs resolution successful
            return sym;
        }
        match prev_kind {
            Kind::WRONG_MTH | Kind::WRONG_MTHS => match sym_kind {
                Kind::WRONG_MTH => {
                    if prev_kind == Kind::WRONG_MTHS {
                        prev
                    } else {
                        sym
                    }
                }
                Kind::ABSENT_MTH => prev,
                _ => sym,
            },
            _ => prev,
        }
    }

}

/// Java: final List<MethodResolutionPhase> methodResolutionSteps
/// (Resolve.java:4927)
pub fn method_resolution_steps() -> crate::util::list::List<MethodResolutionPhase> {
    crate::util::list::List::of3(
        MethodResolutionPhase::BASIC,
        MethodResolutionPhase::BOX,
        MethodResolutionPhase::VARARITY,
    )
}

