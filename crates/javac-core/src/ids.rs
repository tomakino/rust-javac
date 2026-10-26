//! The ADR 0001 arena + ID model: index IDs for the six core compile-time objects.
//!
//! The corresponding Java objects (Symbol/Type/JCTree/Scope/Attribute/Directive) are stored in Rust
//! by the arenas held by CompilerContext and referenced only through IDs (ADR 0001); Java
//! null references use Option<XxxId>. An ID is valid only for the arenas of one compilation and must not cross
//! compilation instances. Attribute/Directive are immutable objects with identity semantics (Java does not override
//! equals by value), so they also use arena+ID (ADR 0001 addendum).
//!
//! PORTING_RULES: non-1:1 adapter file (Java object references -> u32 arena index), see ADR 0001.

/// Generate one arena-index ID type (the four domain types share the same representation).
macro_rules! arena_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(u32);

        impl $name {
            /// Build an ID from an arena index (the caller guarantees the index comes from the matching arena).
            pub const fn new(index: u32) -> $name {
                $name(index)
            }

            /// Return the arena index.
            pub const fn index(self) -> u32 {
                self.0
            }
        }
    };
}

arena_id! {
    /// Symbol arena index (Java: a reference to a Symbol or one of its subclasses).
    SymbolId
}

arena_id! {
    /// Type arena index (Java: a reference to a Type or subclass; a Type is immutable after construction, ADR 0001 rule 5).
    TypeId
}

arena_id! {
    /// Tree arena index (Java: a reference to a JCTree or subclass; Attr writes type, Lower may replace contents).
    TreeId
}

arena_id! {
    /// Scope arena index (Java: a Scope reference; prev/next use Option<ScopeId>).
    ScopeId
}

arena_id! {
    /// Attribute arena index (Java: an Attribute reference; immutable but identity-bearing).
    AttributeId
}

arena_id! {
    /// Directive arena index (Java: a Directive reference; immutable but identity-bearing).
    DirectiveId
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeSet, HashSet};

    #[test]
    fn ids_roundtrip_through_index() {
        const SYMBOL: SymbolId = SymbolId::new(7);
        assert_eq!(SYMBOL.index(), 7);
        assert_eq!(SymbolId::new(0).index(), 0);
        assert_eq!(TypeId::new(u32::MAX).index(), u32::MAX);
        assert_eq!(TreeId::new(42).index(), 42);
        assert_eq!(ScopeId::new(3).index(), 3);
        assert_eq!(AttributeId::new(5).index(), 5);
        assert_eq!(DirectiveId::new(6).index(), 6);
    }

    #[test]
    fn ids_are_hashable_and_ordered() {
        let mut set = HashSet::new();
        set.insert(SymbolId::new(1));
        set.insert(SymbolId::new(2));
        set.insert(SymbolId::new(1));
        assert_eq!(set.len(), 2);
        let ordered: BTreeSet<TypeId> = [TypeId::new(3), TypeId::new(1), TypeId::new(2)]
            .into_iter()
            .collect();
        let indexes: Vec<u32> = ordered.iter().map(|id| id.index()).collect();
        assert_eq!(indexes, vec![1, 2, 3]);
    }

    #[test]
    fn ids_are_copy_and_debug_named() {
        let id = TreeId::new(9);
        let copy = id;
        assert_eq!(id, copy);
        assert_eq!(format!("{id:?}"), "TreeId(9)");
        assert_eq!(format!("{:?}", ScopeId::new(0)), "ScopeId(0)");
        assert_eq!(format!("{:?}", TypeId::new(u32::MAX)), "TypeId(4294967295)");
        assert_eq!(format!("{:?}", AttributeId::new(1)), "AttributeId(1)");
        assert_eq!(format!("{:?}", DirectiveId::new(2)), "DirectiveId(2)");
    }
}
