//! Compiler-wide state and a borrowing view over it.
//!
//! `Session` owns the services that live for one compilation: the arenas,
//! symbols, types, and names. `Cx` is a short-lived view that bundles them so a
//! phase function can take one context argument instead of four.
//!
//! The view exposes separate accessors (ctx, ctx_mut, syms, types, names)
//! instead of one wide mutable handle: Rust does not allow a mutable and an
//! immutable borrow of the same field at once, and the compiler frequently
//! needs the context mutably while reading the other services.

use std::rc::Rc;

use crate::code::source::Source;
use crate::code::symbol::Symbol;
use crate::code::symtab::Symtab;
use crate::code::type_::Type;
use crate::code::types::Types;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::jvm::target::Target;
use crate::tree::jc_tree::JCTree;
use crate::util::names::Names;

/// Compiler state shared by all phases.
pub struct Session {
    ctx: CompilerContext,
    syms: Symtab,
    types: Types,
    names: Rc<Names>,
}

impl Session {
    /// Creates a session with fresh arenas and the standard services.
    pub fn new(source: Source, target: Target) -> Session {
        let names = Rc::new(Names::new(false));
        let mut ctx = CompilerContext::new();
        let syms = Symtab::new(&mut ctx, Rc::clone(&names), target, source);
        let types = Types::with_names(Rc::clone(&names));
        Session {
            ctx,
            syms,
            types,
            names,
        }
    }

    /// Borrows the session as a phase context.
    pub fn cx(&mut self) -> Cx<'_> {
        Cx {
            ctx: &mut self.ctx,
            syms: &self.syms,
            types: &self.types,
            names: &self.names,
        }
    }

    /// Read-only access to the arenas.
    pub fn ctx(&self) -> &CompilerContext {
        &self.ctx
    }

    /// The symbol table.
    pub fn syms(&self) -> &Symtab {
        &self.syms
    }

    /// Type services.
    pub fn types(&self) -> &Types {
        &self.types
    }

    /// The name table.
    pub fn names(&self) -> &Names {
        &self.names
    }
}

/// A borrowing view of a `Session`, used by phase functions.
pub struct Cx<'a> {
    ctx: &'a mut CompilerContext,
    syms: &'a Symtab,
    types: &'a Types,
    names: &'a Rc<Names>,
}

impl<'a> Cx<'a> {
    /// Read-only arena access.
    pub fn ctx(&self) -> &CompilerContext {
        self.ctx
    }

    /// Mutable arena access.
    pub fn ctx_mut(&mut self) -> &mut CompilerContext {
        self.ctx
    }

    /// The symbol table.
    pub fn syms(&self) -> &Symtab {
        self.syms
    }

    /// Type services.
    pub fn types(&self) -> &Types {
        self.types
    }

    /// The name table.
    pub fn names(&self) -> &Names {
        self.names
    }

    /// Read-only access to a tree node.
    pub fn tree(&self, id: TreeId) -> &JCTree {
        self.ctx.tree(id)
    }

    /// Mutable access to a tree node.
    pub fn tree_mut(&mut self, id: TreeId) -> &mut JCTree {
        self.ctx.tree_mut(id)
    }

    /// Read-only access to a symbol.
    pub fn sym(&self, id: SymbolId) -> &Symbol {
        self.ctx.sym(id)
    }

    /// Read-only access to a type.
    pub fn typ(&self, id: TypeId) -> &Type {
        self.ctx.typ(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_exposes_services_through_cx() {
        let mut session = Session::new(Source::DEFAULT, Target::DEFAULT);
        let ctx_ptr = session.ctx() as *const CompilerContext;
        let syms_ptr = session.syms() as *const Symtab;
        let cx = session.cx();
        assert_eq!(cx.ctx() as *const CompilerContext, ctx_ptr);
        assert_eq!(cx.syms() as *const Symtab, syms_ptr);
        let _ = &cx.names().std.java_base;
        let _ = cx.types();
    }
}
