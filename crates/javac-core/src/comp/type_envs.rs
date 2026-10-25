// Origin: com/sun/tools/javac/comp/TypeEnvs.java @ 982d53c (jdk-17.0.19-ga)

//! This class contains the type environments used by Enter, MemberEnter, Attr,
//! DeferredAttr, and Lower.
//!
//! PORTING_RULES adapters (registered):
//! - TypeSymbol/Env object references -> SymbolId / Rc<Env<AttrContext>>;
//! - Context.Key instance() -> explicit construction (no Context singletons).

use std::collections::HashMap;
use std::rc::Rc;

use crate::comp::attr_context::AttrContext;
use crate::comp::env::Env;
use crate::ids::SymbolId;

/// Java: class TypeEnvs
pub struct TypeEnvs {
    map: HashMap<SymbolId, Rc<Env<AttrContext>>>,
}

impl TypeEnvs {
    /// Java: protected TypeEnvs(Context context)
    pub fn new() -> TypeEnvs {
        TypeEnvs {
            map: HashMap::new(),
        }
    }

    /// Java: Env<AttrContext> get(TypeSymbol sym)
    pub fn get(&self, sym: SymbolId) -> Option<Rc<Env<AttrContext>>> {
        self.map.get(&sym).cloned()
    }

    /// Java: Env<AttrContext> put(TypeSymbol sym, Env<AttrContext> env)
    pub fn put(&mut self, sym: SymbolId, env: Rc<Env<AttrContext>>) -> Option<Rc<Env<AttrContext>>> {
        self.map.insert(sym, env)
    }

    /// Java: Env<AttrContext> remove(TypeSymbol sym)
    pub fn remove(&mut self, sym: SymbolId) -> Option<Rc<Env<AttrContext>>> {
        self.map.remove(&sym)
    }

    /// Java: Collection<Env<AttrContext>> values()
    pub fn values(&self) -> Vec<Rc<Env<AttrContext>>> {
        self.map.values().cloned().collect()
    }

    /// Java: void clear()
    pub fn clear(&mut self) {
        self.map.clear();
    }
}

impl Default for TypeEnvs {
    fn default() -> Self {
        TypeEnvs::new()
    }
}