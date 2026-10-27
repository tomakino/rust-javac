// Origin: com/sun/tools/javac/comp/Env.java @ 982d53c (jdk-17.0.19-ga)

//! A class for environments, instances of which are passed as arguments to tree
//! visitors. Environments refer to important ancestors of the subtree that is
//! currently visited, such as the enclosing method, the enclosing class, or the
//! enclosing toplevel node. They also contain a generic component to carry
//! further information specific to individual passes.
//!
//! PORTING_RULES adapters (registered):
//! - JCTree references -> arena TreeId (Option<TreeId> for the upstream null);
//! - the upstream Env<A> node is mutated by dupto; the port keeps envs in Rc
//!   chains and returns a new node from dupto (the resulting Env has identical
//!   fields; no observable alias difference for the ported callers);
//! - the generic info field is A upstream (an object reference); the port stores
//!   Rc<RefCell<A>> so duplicated environments can share or replace it exactly
//!   like Java object references;
//! - Iterable<Env<A>> -> EnvIterator (the upstream hasNext uses next.outer).

use std::cell::RefCell;
use std::rc::Rc;

use crate::ids::TreeId;

/// Java: public class Env<A> implements Iterable<Env<A>>
pub struct Env<A> {
    /// Java: public Env<A> next;
    pub next: Option<Rc<Env<A>>>,
    /// Java: public Env<A> outer;
    pub outer: Option<Rc<Env<A>>>,
    /// Java: public JCTree tree;
    pub tree: Option<TreeId>,
    /// Java: public JCTree.JCCompilationUnit toplevel;
    pub toplevel: Option<TreeId>,
    /// Java: public JCTree.JCClassDecl enclClass;
    pub encl_class: Option<TreeId>,
    /// Java: public JCTree.JCMethodDecl enclMethod;
    pub encl_method: Option<TreeId>,
    /// Java: public A info;
    pub info: Rc<RefCell<A>>,
    /// Java: public boolean baseClause = false;
    pub base_clause: bool,
}

impl<A> Env<A> {
    /// Java: public Env(JCTree tree, A info)
    pub fn new(tree: Option<TreeId>, info: A) -> Rc<Env<A>> {
        Rc::new(Env {
            next: None,
            outer: None,
            tree,
            toplevel: None,
            encl_class: None,
            encl_method: None,
            info: Rc::new(RefCell::new(info)),
            base_clause: false,
        })
    }

    /// Java: public Env<A> dup(JCTree tree, A info) — the upstream dupto
    /// installs next = this; the port copies this environment node (shared info)
    /// as the next link (registered adapter: the &self form cannot clone the
    /// enclosing Rc).
    pub fn dup(&self, tree: Option<TreeId>, info: A) -> Rc<Env<A>> {
        let next = Rc::new(Env {
            next: self.next.clone(),
            outer: self.outer.clone(),
            tree: self.tree,
            toplevel: self.toplevel,
            encl_class: self.encl_class,
            encl_method: self.encl_method,
            info: Rc::clone(&self.info),
            base_clause: self.base_clause,
        });
        Rc::new(Env {
            next: Some(next),
            outer: self.outer.clone(),
            tree,
            toplevel: self.toplevel,
            encl_class: self.encl_class,
            encl_method: self.encl_method,
            info: Rc::new(RefCell::new(info)),
            base_clause: self.base_clause,
        })
    }

    /// Adapter for Attr.copyEnv: the upstream copyEnv assigns the copied
    /// outer environment after dup (Env.java fields are mutable); the port
    /// builds the Env with the copied outer directly.
    pub fn dup_with_outer(
        &self,
        tree: Option<TreeId>,
        info: A,
        outer: Option<Rc<Env<A>>>,
    ) -> Rc<Env<A>> {
        let next = Rc::new(Env {
            next: self.next.clone(),
            outer: self.outer.clone(),
            tree: self.tree,
            toplevel: self.toplevel,
            encl_class: self.encl_class,
            encl_method: self.encl_method,
            info: Rc::clone(&self.info),
            base_clause: self.base_clause,
        });
        Rc::new(Env {
            next: Some(next),
            outer,
            tree,
            toplevel: self.toplevel,
            encl_class: self.encl_class,
            encl_method: self.encl_method,
            info: Rc::new(RefCell::new(info)),
            base_clause: self.base_clause,
        })
    }

    /// Java: public Env<A> dup(JCTree tree) — reuses this environment info.
    ///
    /// # Arguments
    ///   - this: the environment to duplicate;
    ///   - tree: the duplicated environment tree.
    ///
    /// # Returns
    ///   - the duplicate environment (its next points at this environment).
    ///
    /// # Panics
    ///   - none.
    pub fn dup_shared(this: &Rc<Env<A>>, tree: Option<TreeId>) -> Rc<Env<A>> {
        Rc::new(Env {
            next: Some(Rc::clone(this)),
            outer: this.outer.clone(),
            tree,
            toplevel: this.toplevel,
            encl_class: this.encl_class,
            encl_method: this.encl_method,
            info: Rc::clone(&this.info),
            base_clause: this.base_clause,
        })
    }

    /// Java: public Env<A> dupto(Env<A> that)
    ///
    /// PORTING_RULES adapter: returns a new Env with that.tree/that.info and
    /// this.outer/toplevel/enclClass/enclMethod, next = this (the upstream
    /// mutates `that` in place and returns it; the fields are identical).
    pub fn dupto(this: &Rc<Env<A>>, that: Env<A>) -> Rc<Env<A>> {
        Rc::new(Env {
            next: Some(Rc::clone(this)),
            outer: this.outer.clone(),
            tree: that.tree,
            toplevel: this.toplevel,
            encl_class: this.encl_class,
            encl_method: this.encl_method,
            info: that.info,
            base_clause: that.base_clause,
        })
    }
}

impl<A> Env<A> {
    /// Java: public Env<A> enclosing(JCTree.Tag tag)
    pub fn enclosing(
        this: &Rc<Env<A>>,
        ctx: &crate::compiler_context::CompilerContext,
        tag: crate::tree::jc_tree::Tag,
    ) -> Option<Rc<Env<A>>> {
        let mut env: Option<Rc<Env<A>>> = Some(Rc::clone(this));
        while let Some(current) = env {
            if let Some(tree) = current.tree
                && ctx.tree(tree).has_tag(tag) {
                    return Some(current);
                }
            env = current.next.clone();
        }
        None
    }
}

impl<A: std::fmt::Display> std::fmt::Display for Env<A> {
    /// Java: public String toString()
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Env[{}", self.info.borrow())?;
        if let Some(outer) = &self.outer {
            write!(f, ",outer={outer}")?;
        }
        write!(f, "]")
    }
}

/// Java: Env.iterator() (the upstream Iterator<Env<A>> uses next.outer).
pub struct EnvIterator<A> {
    next: Option<Rc<Env<A>>>,
}

impl<A> EnvIterator<A> {
    /// Java: hasNext() { return next.outer != null; }
    pub fn has_next(&self) -> bool {
        match &self.next {
            Some(next) => next.outer.is_some(),
            None => false,
        }
    }
}

impl<A> Iterator for EnvIterator<A> {
    type Item = Rc<Env<A>>;

    fn next(&mut self) -> Option<Rc<Env<A>>> {
        if !self.has_next() {
            return None;
        }
        let current = self.next.take().expect("EnvIterator current");
        self.next = current.outer.clone();
        Some(current)
    }
}

impl<A> Env<A> {
    /// Java: public Iterator<Env<A>> iterator()
    pub fn iterator(this: &Rc<Env<A>>) -> EnvIterator<A> {
        EnvIterator {
            next: Some(Rc::clone(this)),
        }
    }
}