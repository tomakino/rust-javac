//! ADR 0001 的 arena + ID 模型:六类核心编译期对象的下标 ID。
//!
//! 对应的 Java 对象(Symbol/Type/JCTree/Scope/Attribute/Directive)在 Rust 中由
//! CompilerContext 持有的 arena 存储,外部一律通过 ID 引用(ADR 0001);Java 的
//! null 引用用 Option<XxxId> 表示。ID 只对同一次编译的 arena 有效,不得跨编译
//! 实例使用。Attribute/Directive 是"不可变但有身份语义"的对象(Java 未按值重写
//! equals),因此也走 arena+ID(ADR 0001 补记)。
//!
//! PORTING_RULES:非 1:1 适配文件(Java 对象引用 -> u32 arena index),见 ADR 0001。

/// 生成一个 arena 下标 ID 类型(四个领域类型共享同一表示)。
macro_rules! arena_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(u32);

        impl $name {
            /// 由 arena 下标构造 ID(调用方保证下标来自对应的 arena)。
            pub const fn new(index: u32) -> $name {
                $name(index)
            }

            /// 返回 arena 下标。
            pub const fn index(self) -> u32 {
                self.0
            }
        }
    };
}

arena_id! {
    /// Symbol arena 下标(Java: Symbol 或其子类对象引用)。
    SymbolId
}

arena_id! {
    /// Type arena 下标(Java: Type 或其子类对象引用;Type 构造后不可变,ADR 0001 规则 5)。
    TypeId
}

arena_id! {
    /// Tree arena 下标(Java: JCTree 或其子类对象引用;属性阶段写 type,Lower 可替换内容)。
    TreeId
}

arena_id! {
    /// Scope arena 下标(Java: Scope 对象引用;prev/next 用 Option<ScopeId>)。
    ScopeId
}

arena_id! {
    /// Attribute arena 下标(Java: Attribute 对象引用;不可变但有身份语义)。
    AttributeId
}

arena_id! {
    /// Directive arena 下标(Java: Directive 对象引用;不可变但有身份语义)。
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
