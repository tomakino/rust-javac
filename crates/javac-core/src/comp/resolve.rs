// Origin: com/sun/tools/javac/comp/Resolve.java @ 982d53c (jdk-17.0.19-ga)

//! Method resolution: this file currently ports only the
//! Resolve.MethodResolutionPhase enum, which is a field of AttrContext (the
//! Enter/Attr dependency).
//!
//! NOT PORTED IN THIS STEP (registered; dependencies: Kinds resolution-error
//! kinds, MethodResolutionContext/Candidate/ResolveError/Type/TypeRelation):
//! - the whole Resolve class; mergeResults is therefore unimplemented below.

use crate::ids::SymbolId;

/// Java: enum Resolve.MethodResolutionPhase
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[derive(Default)]
pub enum MethodResolutionPhase {
    #[default]
    BASIC,
    BOX,
    VARARITY,
}

impl MethodResolutionPhase {
    /// Java: public boolean isBoxingRequired()
    pub fn is_boxing_required(self) -> bool {
        matches!(self, MethodResolutionPhase::BOX | MethodResolutionPhase::VARARITY)
    }

    /// Java: public boolean isVarargsRequired()
    pub fn is_varargs_required(self) -> bool {
        matches!(self, MethodResolutionPhase::VARARITY)
    }

    /// Java: public Symbol mergeResults(Symbol prev, Symbol sym)
    pub fn merge_results(self, _prev: SymbolId, _sym: SymbolId) -> SymbolId {
        // TODO(Resolve batch): the VARARITY override inspects Kinds resolution
        // errors (WRONG_MTH/WRONG_MTHS/ABSENT_MTH), which are not ported yet.
        unimplemented!("TODO(Resolve batch): MethodResolutionPhase.mergeResults")
    }
}

