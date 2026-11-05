//! Type environments shared by Enter, MemberEnter, Attr, DeferredAttr and
//! Lower: a map from type symbols to their attribution environments.
//!
//! The map is behind a `RefCell` so the service can be shared as
//! `Rc<TypeEnvs>` and used through `&self` (Java: `TypeEnvs` is a mutable
//! HashMap service).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::comp::attr_context::AttrContext;
use crate::comp::env::Env;
use crate::ids::SymbolId;

/// Java: class TypeEnvs
pub struct TypeEnvs {
    map: RefCell<HashMap<SymbolId, Rc<Env<AttrContext>>>>,
}

impl TypeEnvs {
    /// Java: protected TypeEnvs(Context context)
    pub fn new() -> TypeEnvs {
        TypeEnvs {
            map: RefCell::new(HashMap::new()),
        }
    }

    /// Java: Env<AttrContext> get(TypeSymbol sym)
    pub fn get(&self, sym: SymbolId) -> Option<Rc<Env<AttrContext>>> {
        self.map.borrow().get(&sym).cloned()
    }

    /// Java: Env<AttrContext> put(TypeSymbol sym, Env<AttrContext> env)
    pub fn put(&self, sym: SymbolId, env: Rc<Env<AttrContext>>) -> Option<Rc<Env<AttrContext>>> {
        self.map.borrow_mut().insert(sym, env)
    }

    /// Java: Env<AttrContext> remove(TypeSymbol sym)
    pub fn remove(&self, sym: SymbolId) -> Option<Rc<Env<AttrContext>>> {
        self.map.borrow_mut().remove(&sym)
    }

    /// Java: Collection<Env<AttrContext>> values()
    pub fn values(&self) -> Vec<Rc<Env<AttrContext>>> {
        self.map.borrow().values().cloned().collect()
    }

    /// Java: void clear()
    pub fn clear(&self) {
        self.map.borrow_mut().clear();
    }
}

impl Default for TypeEnvs {
    fn default() -> Self {
        TypeEnvs::new()
    }
}
