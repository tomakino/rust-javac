// Origin: com/sun/tools/javac/util/Dependencies.java @ 982d53c (jdk-17.0.19-ga)

//! Track dependencies in the javac symbol completion process.
//!
//! PORTING_RULES adapters (registered):
//! - Java abstract class Dependencies -> Rust trait; the only implementation
//!   needed by ClassFinder is DummyDependencies (the upstream default when no
//!   GraphDependencies was pre-registered).
//! - push(ClassSymbol, CompletionCause) -> push(ctx, SymbolId, CompletionCause)
//!   because symbols are arena values (ADR 0001).
//!
//! NOT PORTED IN THIS STEP (registered, no substitutes):
//! - GraphDependencies (Dependencies.java:100-430): depends on GraphUtils,
//!   JavaCompiler, Options and javac debug options; it is only installed by
//!   the -XDcompletionDeps option and is not needed by ClassFinder.
use crate::compiler_context::CompilerContext;
use crate::ids::SymbolId;

/// Java: public enum Dependencies.CompletionCause implements GraphUtils.DependencyKind
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionCause {
    CLASS_READER,
    HEADER_PHASE,
    HIERARCHY_PHASE,
    IMPORTS_PHASE,
    MEMBER_ENTER,
    RECORD_PHASE,
    MEMBERS_PHASE,
    PERMITS_PHASE,
    OTHER,
}

/// Java: public abstract class Dependencies
pub trait Dependencies {
    /// Java: public abstract void push(ClassSymbol s, CompletionCause phase)
    fn push(&mut self, ctx: &mut CompilerContext, sym: SymbolId, phase: CompletionCause);

    /// Java: public abstract void pop()
    fn pop(&mut self);
}

/// Java: public static class DummyDependencies extends Dependencies
pub struct DummyDependencies;

impl Dependencies for DummyDependencies {
    fn push(&mut self, _ctx: &mut CompilerContext, _sym: SymbolId, _phase: CompletionCause) {}

    fn pop(&mut self) {}
}

// NOT PORTED: GraphDependencies (Dependencies.java:100-430) — needs GraphUtils,
// JavaCompiler and Options; registered for the diagnostics/debug batch.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler_context::CompilerContext;

    #[test]
    fn dummy_dependencies_are_no_ops() {
        let mut ctx = CompilerContext::new();
        let mut deps = DummyDependencies;
        deps.push(&mut ctx, SymbolId::new(0), CompletionCause::CLASS_READER);
        deps.pop();
    }
}
