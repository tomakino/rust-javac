//! Partial map recording which compiler phases have been executed for each
//! compilation unit (ATTR/FLOW phases).
//!
//! ADR 0001: the upstream Context service is constructed explicitly; the map
//! keys are the environment reference identities (upstream HashMap identity
//! semantics), so the port keys by Rc pointer while keeping the Rc alive.
//!
//! Upstream: CompileStates.java:25-92.

use std::collections::HashMap;
use std::rc::Rc;

use crate::comp::attr_context::AttrContext;
use crate::comp::env::Env;

/// Java: enum CompileStates.CompileState (CompileStates.java:52-70).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompileState {
    /// INIT(0)
    INIT,
    /// PARSE(1)
    PARSE,
    /// ENTER(2)
    ENTER,
    /// PROCESS(3)
    PROCESS,
    /// ATTR(4)
    ATTR,
    /// FLOW(5)
    FLOW,
    /// TRANSTYPES(6)
    TRANSTYPES,
    /// TRANSPATTERNS(7)
    TRANSPATTERNS,
    /// UNLAMBDA(8)
    UNLAMBDA,
    /// LOWER(9)
    LOWER,
    /// GENERATE(10)
    GENERATE,
}

impl CompileState {
    /// Java: CompileState(int value) (CompileStates.java:56-58).
    pub fn value(self) -> i32 {
        match self {
            CompileState::INIT => 0,
            CompileState::PARSE => 1,
            CompileState::ENTER => 2,
            CompileState::PROCESS => 3,
            CompileState::ATTR => 4,
            CompileState::FLOW => 5,
            CompileState::TRANSTYPES => 6,
            CompileState::TRANSPATTERNS => 7,
            CompileState::UNLAMBDA => 8,
            CompileState::LOWER => 9,
            CompileState::GENERATE => 10,
        }
    }

    /// Java: public boolean isAfter(CompileState other) (CompileStates.java:59-61).
    pub fn is_after(self, other: CompileState) -> bool {
        self.value() > other.value()
    }

    /// Java: public static CompileState max(CompileState a, CompileState b)
    /// (CompileStates.java:62-64).
    pub fn max(a: CompileState, b: CompileState) -> CompileState {
        if a.value() > b.value() { a } else { b }
    }
}

/// Java: public class CompileStates extends HashMap<Env<AttrContext>, CompileState>
/// (CompileStates.java:33-92).
#[derive(Default)]
pub struct CompileStates {
    /// Java: the HashMap entries; the Rc keeps every key environment alive so the
    /// pointer identity cannot be reused while it is a map key.
    entries: HashMap<usize, (Rc<Env<AttrContext>>, CompileState)>,
}

impl CompileStates {
    /// Java: CompileStates(Context context) (CompileStates.java:80-83).
    ///
    /// # Arguments
    ///   - none.
    ///
    /// # Returns
    ///   - the empty map.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: CompileStates.java:80-83.
    pub fn new() -> CompileStates {
        CompileStates::default()
    }

    /// Java: HashMap.get(env).
    ///
    /// # Arguments
    ///   - env: the attribution environment key.
    ///
    /// # Returns
    ///   - the recorded compile state.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: CompileStates.java:85-88 (isDone lookup).
    pub fn get(&self, env: &Rc<Env<AttrContext>>) -> Option<CompileState> {
        let key = Rc::as_ptr(env) as usize;
        self.entries.get(&key).map(|(_, state)| *state)
    }

    /// Java: HashMap.put(env, state).
    ///
    /// # Arguments
    ///   - env: the attribution environment key;
    ///   - state: the compile state.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: CompileStates.java:33-38 (HashMap put).
    pub fn put(&mut self, env: &Rc<Env<AttrContext>>, state: CompileState) {
        let key = Rc::as_ptr(env) as usize;
        self.entries.insert(key, (Rc::clone(env), state));
    }

    /// Java: boolean isDone(Env<AttrContext> env, CompileState cs)
    /// (CompileStates.java:85-88).
    ///
    /// # Arguments
    ///   - env: the attribution environment key;
    ///   - cs: the state to check.
    ///
    /// # Returns
    ///   - true when the recorded state is not before cs.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: CompileStates.java:85-88.
    pub fn is_done(&self, env: &Rc<Env<AttrContext>>, cs: CompileState) -> bool {
        match self.get(env) {
            Some(ecs) => !cs.is_after(ecs),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CompileState, CompileStates};
    use crate::comp::attr_context::AttrContext;
    use crate::comp::env::Env;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn env() -> Rc<Env<AttrContext>> {
        Rc::new(Env {
            next: None,
            outer: None,
            tree: None,
            toplevel: None,
            encl_class: None,
            encl_method: None,
            info: Rc::new(RefCell::new(AttrContext::new())),
            base_clause: false,
        })
    }

    #[test]
    fn compile_states_record_and_query_match_the_upstream_map() {
        let mut states = CompileStates::new();
        let env = env();
        assert!(!states.is_done(&env, CompileState::ATTR));
        states.put(&env, CompileState::ATTR);
        assert_eq!(states.get(&env), Some(CompileState::ATTR));
        assert!(states.is_done(&env, CompileState::ATTR));
        assert!(states.is_done(&env, CompileState::PARSE));
        assert!(!states.is_done(&env, CompileState::FLOW));
        states.put(&env, CompileState::FLOW);
        assert_eq!(states.get(&env), Some(CompileState::FLOW));
    }

    #[test]
    fn compile_state_ordering_matches_the_upstream_enumeration() {
        assert!(!CompileState::INIT.is_after(CompileState::PARSE));
        assert!(CompileState::GENERATE.is_after(CompileState::LOWER));
        assert!(!CompileState::ATTR.is_after(CompileState::ATTR));
        assert_eq!(CompileState::max(CompileState::ENTER, CompileState::FLOW), CompileState::FLOW);
        assert_eq!(CompileState::max(CompileState::GENERATE, CompileState::LOWER), CompileState::GENERATE);
        assert_eq!(CompileState::GENERATE.value(), 10);
    }
}