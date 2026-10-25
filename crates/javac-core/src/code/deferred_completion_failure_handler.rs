// Origin: com/sun/tools/javac/code/DeferredCompletionFailureHandler.java @ 982d53c (jdk-17.0.19-ga)

//! When a CompletionFailure is thrown when user code is running, it should not be
//! thrown out to the client code, but rather skipped, and then rethrown later if
//! javac itself will complete the Symbol.
//!
//! PORTING_RULES adapters (registered):
//! - Java unchecked CompletionFailure -> Result<(), CompletionFailure>; the
//!   handlers therefore operate on Rust error values instead of throwing.
//! - Symbol fields changed by FlipSymbolDescription are arena values reached
//!   through CompilerContext (ADR 0001); flip() takes &mut CompilerContext.
//! - Symbol.completer is not ported yet (TODO(P3)); flip() intentionally does
//!   not swap it (no such field exists locally) and records the deviation here.
//! - Handler instances -> Rust trait objects; the three upstream handlers are
//!   UserCodeHandler/SpeculativeCodeHandler/JavacCodeHandler below.

use std::rc::Rc;

use crate::code::kinds::Kind;
use crate::code::symbol::{Completer, CompletionFailure, SymbolKind};
use crate::ids::SymbolId;
use crate::compiler_context::CompilerContext;
use crate::ids::TypeId;

/// Java: DeferredCompletionFailureHandler.Handler
pub trait Handler {
    /// Java: void install()
    fn install(&mut self, ctx: &mut CompilerContext);

    /// Java: void handleAPICompletionFailure(CompletionFailure cf)
    fn handle_api_completion_failure(
        &mut self,
        cf: CompletionFailure,
    ) -> Result<(), CompletionFailure>;

    /// Java: void classSymbolCompleteFailed(ClassSymbol sym, Completer origCompleter)
    fn class_symbol_complete_failed(
        &mut self,
        ctx: &mut CompilerContext,
        sym: SymbolId,
        orig_completer: Box<dyn Completer>,
    );

    /// Java: void classSymbolRemoved(ClassSymbol sym)
    fn class_symbol_removed(&mut self, ctx: &mut CompilerContext, sym: SymbolId);

    /// Java: void uninstall()
    fn uninstall(&mut self, ctx: &mut CompilerContext);
}

/// Java: static class FlipSymbolDescription
pub struct FlipSymbolDescription {
    pub sym: SymbolId,
    pub ty: Option<TypeId>,
    pub kind: Kind,
    pub members: Option<crate::ids::ScopeId>,
    /// Java: Completer completer (see the module note: not swapped locally).
    pub completer: Option<Box<dyn Completer>>,
}

impl FlipSymbolDescription {
    pub fn new(
        ctx: &CompilerContext,
        sym: SymbolId,
        completer: Box<dyn Completer>,
    ) -> FlipSymbolDescription {
        FlipSymbolDescription {
            sym,
            ty: ctx.sym(sym).ty,
            kind: ctx.sym(sym).kind,
            members: None,
            completer: Some(completer),
        }
    }

    /// Java: public void flip()
    pub fn flip(&mut self, ctx: &mut CompilerContext) {
        let prev_ty = ctx.sym(self.sym).ty;
        ctx.sym_mut(self.sym).ty = self.ty;
        self.ty = prev_ty;
        let prev_kind = ctx.sym(self.sym).kind;
        ctx.sym_mut(self.sym).kind = self.kind;
        self.kind = prev_kind;
        let prev_members = match &ctx.sym(self.sym).kind_data {
            SymbolKind::ClassSymbol { members_field, .. } => *members_field,
            _ => None,
        };
        if let SymbolKind::ClassSymbol { members_field, .. } = &mut ctx.sym_mut(self.sym).kind_data {
            *members_field = self.members;
        }
        self.members = prev_members;
        // Java: swaps sym.completer/this.completer. Symbol.completer is not
        // ported (TODO(P3)); no local field exists, so this swap is registered
        // as an adapter deviation instead of inventing a field.
    }
}

/// Java: private class DeferredCompleter implements Completer
pub struct DeferredCompleter {
    pub orig_completer: Box<dyn Completer>,
}

impl DeferredCompleter {
    pub fn new(orig_completer: Box<dyn Completer>) -> DeferredCompleter {
        DeferredCompleter { orig_completer }
    }
}

impl Completer for DeferredCompleter {
    fn complete(&mut self, sym: SymbolId) -> Result<(), CompletionFailure> {
        self.orig_completer.complete(sym)
    }
}

/// Java: DeferredCompletionFailureHandler.UserCodeHandler
pub struct UserCodeHandler {
    pub class2flip: std::collections::HashMap<SymbolId, FlipSymbolDescription>,
}

impl Handler for UserCodeHandler {
    fn install(&mut self, ctx: &mut CompilerContext) {
        for flip in self.class2flip.values_mut() {
            flip.flip(ctx);
        }
    }

    fn handle_api_completion_failure(
        &mut self,
        _cf: CompletionFailure,
    ) -> Result<(), CompletionFailure> {
        // Java: ignore
        Ok(())
    }

    fn class_symbol_complete_failed(
        &mut self,
        ctx: &mut CompilerContext,
        sym: SymbolId,
        orig_completer: Box<dyn Completer>,
    ) {
        let deferred = DeferredCompleter::new(orig_completer);
        self.class2flip
            .insert(sym, FlipSymbolDescription::new(ctx, sym, Box::new(deferred)));
    }

    fn class_symbol_removed(&mut self, _ctx: &mut CompilerContext, sym: SymbolId) {
        self.class2flip.remove(&sym);
    }

    fn uninstall(&mut self, ctx: &mut CompilerContext) {
        for flip in self.class2flip.values_mut() {
            flip.flip(ctx);
        }
    }
}

impl Default for UserCodeHandler {
    fn default() -> UserCodeHandler {
        UserCodeHandler::new()
    }
}

impl UserCodeHandler {
    pub fn new() -> UserCodeHandler {
        UserCodeHandler {
            class2flip: std::collections::HashMap::new(),
        }
    }
}

/// Java: DeferredCompletionFailureHandler.SpeculativeCodeHandler
pub struct SpeculativeCodeHandler {
    pub class2flip: std::collections::HashMap<SymbolId, FlipSymbolDescription>,
}

impl Handler for SpeculativeCodeHandler {
    fn install(&mut self, _ctx: &mut CompilerContext) {}

    fn handle_api_completion_failure(
        &mut self,
        cf: CompletionFailure,
    ) -> Result<(), CompletionFailure> {
        Err(cf)
    }

    fn class_symbol_complete_failed(
        &mut self,
        ctx: &mut CompilerContext,
        sym: SymbolId,
        orig_completer: Box<dyn Completer>,
    ) {
        let deferred = DeferredCompleter::new(orig_completer);
        self.class2flip
            .insert(sym, FlipSymbolDescription::new(ctx, sym, Box::new(deferred)));
    }

    fn class_symbol_removed(&mut self, _ctx: &mut CompilerContext, sym: SymbolId) {
        self.class2flip.remove(&sym);
    }

    fn uninstall(&mut self, ctx: &mut CompilerContext) {
        for flip in self.class2flip.values_mut() {
            flip.flip(ctx);
        }
        self.class2flip.clear();
    }
}

impl Default for SpeculativeCodeHandler {
    fn default() -> SpeculativeCodeHandler {
        SpeculativeCodeHandler::new()
    }
}

impl SpeculativeCodeHandler {
    pub fn new() -> SpeculativeCodeHandler {
        SpeculativeCodeHandler {
            class2flip: std::collections::HashMap::new(),
        }
    }
}

/// Java: DeferredCompletionFailureHandler.JavacCodeHandler
pub struct JavacCodeHandler;

impl Handler for JavacCodeHandler {
    fn install(&mut self, _ctx: &mut CompilerContext) {}

    fn handle_api_completion_failure(
        &mut self,
        cf: CompletionFailure,
    ) -> Result<(), CompletionFailure> {
        Err(cf)
    }

    fn class_symbol_complete_failed(
        &mut self,
        _ctx: &mut CompilerContext,
        _sym: SymbolId,
        _orig_completer: Box<dyn Completer>,
    ) {
    }

    fn class_symbol_removed(&mut self, _ctx: &mut CompilerContext, _sym: SymbolId) {}

    fn uninstall(&mut self, _ctx: &mut CompilerContext) {}
}

/// Java: public class DeferredCompletionFailureHandler
pub struct DeferredCompletionFailureHandler {
    /// Java: Handler handler (initially javacCodeHandler)
    pub handler: Box<dyn Handler>,
}

impl DeferredCompletionFailureHandler {
    /// Java: public static DeferredCompletionFailureHandler instance(Context context)
    pub fn instance() -> Rc<DeferredCompletionFailureHandler> {
        Rc::new(DeferredCompletionFailureHandler {
            handler: Box::new(JavacCodeHandler),
        })
    }

    /// Java: public Handler setHandler(Handler h)
    ///
    /// PORTING_RULES adapter: handler install/uninstall mutate symbols through
    /// CompilerContext, so the context is passed explicitly (Rust has no
    /// Context.current()).
    pub fn set_handler(
        &mut self,
        ctx: &mut CompilerContext,
        mut h: Box<dyn Handler>,
    ) -> Box<dyn Handler> {
        self.handler.uninstall(ctx);
        std::mem::swap(&mut self.handler, &mut h);
        self.handler.install(ctx);
        h
    }

    /// Java: public void handleAPICompletionFailure(CompletionFailure cf)
    pub fn handle_api_completion_failure(
        &mut self,
        cf: CompletionFailure,
    ) -> Result<(), CompletionFailure> {
        self.handler.handle_api_completion_failure(cf)
    }

    /// Java: public void classSymbolCompleteFailed(ClassSymbol sym, Completer origCompleter)
    pub fn class_symbol_complete_failed(
        &mut self,
        ctx: &mut CompilerContext,
        sym: SymbolId,
        orig_completer: Box<dyn Completer>,
    ) {
        self.handler
            .class_symbol_complete_failed(ctx, sym, orig_completer);
    }

    /// Java: public void classSymbolRemoved(ClassSymbol sym)
    pub fn class_symbol_removed(&mut self, ctx: &mut CompilerContext, sym: SymbolId) {
        self.handler.class_symbol_removed(ctx, sym);
    }

    /// Java: public boolean isDeferredCompleter(Completer c) { return c instanceof DeferredCompleter; }
    pub fn is_deferred_completer(&self, _c: &dyn Completer) -> bool {
        // PORTING_RULES adapter: Rust has no instanceof; DeferredCompleter is the only
        // local completer wrapper, so callers can use the type check directly.
        unimplemented!("TODO(P3): isDeferredCompleter needs the completer trait downcast")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure() -> CompletionFailure {
        let mut context = crate::util::context::Context::new();
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut context);
        let factory = crate::util::jc_diagnostic::Factory::new(messages, "compiler");
        let diag = factory.fragment(
            crate::resources::compiler_properties::fragments::illegal_start_of_class_file().into(),
        );
        CompletionFailure::new(None, Box::new(move || diag.clone()), None)
    }

    fn dcfh() -> DeferredCompletionFailureHandler {
        DeferredCompletionFailureHandler {
            handler: Box::new(JavacCodeHandler),
        }
    }

    #[test]
    fn javac_code_handler_rethrows_api_completion_failure() {
        let mut handler = dcfh();
        assert!(handler.handle_api_completion_failure(failure()).is_err());
    }

    #[test]
    fn set_handler_returns_previous_and_installs_new() {
        let mut ctx = CompilerContext::new();
        let mut handler = dcfh();
        let mut prev = handler.set_handler(&mut ctx, Box::new(UserCodeHandler::new()));
        // Java: UserCodeHandler.handleAPICompletionFailure ignores the failure.
        assert!(handler.handle_api_completion_failure(failure()).is_ok());
        // The previous handler was the default javacCodeHandler (rethrows).
        assert!(prev.handle_api_completion_failure(failure()).is_err());
    }

    #[test]
    fn speculative_handler_rethrows_api_completion_failure() {
        let mut handler = DeferredCompletionFailureHandler {
            handler: Box::new(SpeculativeCodeHandler::new()),
        };
        assert!(handler.handle_api_completion_failure(failure()).is_err());
    }
}