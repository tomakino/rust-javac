#![allow(clippy::result_large_err)]
// Origin: com/sun/tools/javac/comp/InferenceContext.java @ 982d53c (jdk-17.0.19-ga)
//         com/sun/tools/javac/comp/Infer.java @ 982d53c (context parts only)
//
// §19 registration:
// Ported in this file: the InferenceContext structure (undetvars/inferencevars/
// freeTypeListeners), its constructor forms (new/empty), free(Type)/
// free(List<Type>)/freeVarsIn/asInstType/asInstTypes/addFreeTypeListener/
// notifyChange/inferenceVars/undetVars (the methods Check/Attr depend on), the
// FreeTypeListener interface and the InferenceException value type header.
//
// NOT PORTED IN THIS FILE (registered: Infer batch):
// - the remaining InferenceContext methods (restvars/instvars/boundedVars/
//   asUndetVar(s)/save/rollback/dupTo/min/solve/solveAny/incorporate/...): they
//   need the UndetVar model and the GraphSolver/InferenceStep machinery;
// - the Infer service class (Infer.java:82-1945: instantiateMethods/
//   resolveMethod/checkBounds/...): only the empty context entry point is
//   provided here (InferenceContext::empty), the rest of Infer is the Infer
//   batch;
// - the InferenceException diagnostic payload (Infer batch).
use std::rc::Rc;

use crate::code::symtab::Symtab;
use crate::code::types::Types;
use crate::compiler_context::CompilerContext;
use crate::ids::TypeId;
use crate::util::list::List;

/// Java: public static interface Infer.FreeTypeListener
pub trait FreeTypeListener {
    /// Java: void typesInferred(InferenceContext inferenceContext)
    fn types_inferred(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        inference_context: &mut InferenceContext,
    ) -> Result<(), InferenceException>;
}

/// Java: public static class Infer.InferenceException (header only; the
/// diagnostic payload is registered for the Infer batch).
#[derive(Clone, Debug)]
pub struct InferenceException {
    /// Java: final JCDiagnostic diagnostic (registered: the full
    /// Infer.InferenceException carries the diagnostic and the enclosing
    /// context; the Infer batch fills this value).
    pub diagnostic: Option<crate::util::jc_diagnostic::JCDiagnostic>,
}

impl InferenceException {
    pub fn new(diagnostic: Option<crate::util::jc_diagnostic::JCDiagnostic>) -> InferenceException {
        InferenceException { diagnostic }
    }
}

/// Java: public class InferenceContext (the ported subset; see the file header).
pub struct InferenceContext {
    /// Java: List<Type> undetvars
    pub undetvars: List<TypeId>,
    /// Java: List<Type> inferencevars
    pub inferencevars: List<TypeId>,
    /// Java: Map<FreeTypeListener, List<Type>> freeTypeListeners
    pub free_type_listeners: Vec<(Box<dyn FreeTypeListener>, List<TypeId>)>,
}

impl InferenceContext {
    /// Java: public InferenceContext(Infer infer, List<Type> inferencevars,
    /// List<Type> undetvars) — the port does not store the Infer service in the
    /// context (explicit context adapter, ADR 0001).
    pub fn new(inferencevars: List<TypeId>, undetvars: List<TypeId>) -> InferenceContext {
        InferenceContext {
            undetvars,
            inferencevars,
            free_type_listeners: Vec::new(),
        }
    }

    /// Java: Infer.emptyContext — an empty inference context (empty
    /// inferencevars/undetvars, no listeners).
    pub fn empty() -> InferenceContext {
        InferenceContext::new(List::nil(), List::nil())
    }

    /// Java: List<Type> inferenceVars()
    pub fn inference_vars(&self) -> List<TypeId> {
        self.inferencevars.clone()
    }

    /// Java: public List<Type> undetVars()
    pub fn undet_vars(&self) -> List<TypeId> {
        self.undetvars.clone()
    }

    /// Java: final boolean free(Type t) — t.containsAny(inferencevars)
    pub fn free(&self, ctx: &mut CompilerContext, syms: &Symtab, types: &Types, t: TypeId) -> bool {
        types.contains_any(ctx, syms, t, &self.inferencevars)
    }

    /// Java: final boolean free(List<Type> ts)
    pub fn free_list(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        ts: List<TypeId>,
    ) -> bool {
        let mut current = ts;
        while current.non_empty() {
            let t = *current.head();
            if self.free(ctx, syms, types, t) {
                return true;
            }
            current = current.tail();
        }
        false
    }

    /// Java: final List<Type> freeVarsIn(Type t)
    pub fn free_vars_in(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
    ) -> List<TypeId> {
        let mut result: List<TypeId> = List::nil();
        let mut current = self.inferencevars.clone();
        while current.non_empty() {
            let iv = *current.head();
            if types.contains(ctx, syms, t, iv) {
                result = result.append(iv);
            }
            current = current.tail();
        }
        result
    }

    /// Java: final List<Type> freeVarsIn(List<Type> ts)
    pub fn free_vars_in_list(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        ts: List<TypeId>,
    ) -> List<TypeId> {
        let mut result: List<TypeId> = List::nil();
        let mut current = ts;
        while current.non_empty() {
            let t = *current.head();
            let vars = self.free_vars_in(ctx, syms, types, t);
            let mut vars_current = vars;
            while vars_current.non_empty() {
                let var = *vars_current.head();
                if !result.iter().any(|existing| existing == var) {
                    result = result.append(var);
                }
                vars_current = vars_current.tail();
            }
            current = current.tail();
        }
        result
    }

    /// Java: List<Type> instTypes() (needs UndetVar.getInst; the Infer batch).
    pub fn inst_types(
        &self,
        _ctx: &mut CompilerContext,
        _syms: &Symtab,
    ) -> List<TypeId> {
        // TODO(Infer/UndetVar batch): UndetVar.getInst()/qtype dispatch.
        unimplemented!("TODO(Infer/UndetVar batch): InferenceContext.instTypes")
    }

    /// Java: Type asInstType(Type t)
    pub fn as_inst_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
    ) -> TypeId {
        let inst_types = if self.undetvars.is_empty() {
            // Empty context: instTypes() is the empty list and subst of empty
            // lists returns the type unchanged (upstream Types.subst with empty
            // from/to).
            List::nil()
        } else {
            self.inst_types(ctx, syms)
        };
        let result = types.subst(ctx, syms, &List::of(t), &self.inferencevars, &inst_types);
        *result.head()
    }

    /// Java: List<Type> asInstTypes(List<Type> ts)
    pub fn as_inst_types(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        ts: List<TypeId>,
    ) -> List<TypeId> {
        let mut result: List<TypeId> = List::nil();
        let mut current = ts;
        while current.non_empty() {
            let t = *current.head();
            let inst = self.as_inst_type(ctx, syms, types, t);
            result = result.append(inst);
            current = current.tail();
        }
        result
    }

    /// Java: void addFreeTypeListener(List<Type> types, FreeTypeListener ftl)
    pub fn add_free_type_listener(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        type_list: List<TypeId>,
        listener: Box<dyn FreeTypeListener>,
    ) {
        let free_vars = self.free_vars_in_list(ctx, syms, types, type_list);
        self.free_type_listeners.push((listener, free_vars));
    }

    /// Java: void notifyChange() / notifyChange(List<Type> inferredVars)
    pub fn notify_change(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
    ) -> Result<(), InferenceException> {
        // TODO(Infer batch): restvars()/the inferredVars diff needs the UndetVar
        // model; the empty context has no listeners and no vars (no-op).
        if self.free_type_listeners.is_empty() {
            return Ok(());
        }
        unimplemented!("TODO(Infer/UndetVar batch): InferenceContext.notifyChange(restvars)")
    }
}

impl Default for InferenceContext {
    fn default() -> Self {
        InferenceContext::empty()
    }
}

/// Java: Infer.emptyContext (the shared empty inference context; the port
/// shares it through Rc and never mutates it).
pub fn empty_context() -> Rc<InferenceContext> {
    Rc::new(InferenceContext::empty())
}
