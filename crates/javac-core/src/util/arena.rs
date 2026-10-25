//! 泛型 arena 容器:ADR 0001 的 ID/arena 基础设施。
//!
//! Arena<T> 拥有元素,元素只能通过 arena 下标访问;arena 只增不减(ADR 0001 规则 8),
//! 编译结束随 CompilerContext 整体释放。本模块只提供容器,不定义编译器实体;
//! CompilerContext(后续任务)将按 ADR 0001 持有 Symbol/Type/Tree/Scope 各 Arena。
//!
//! PORTING_RULES:非 1:1 适配文件(Java 可变对象图 -> arena + ID),见 ADR 0001。

/// 只增不减的 arena:push 返回新元素的下标,外部用下标构造对应 ID。
///
/// 不引入 Rc/RefCell:所有权归 arena,访问通过 ID(ADR 0001 规则 4)。
#[derive(Debug, Clone)]
pub struct Arena<T> {
    values: Vec<T>,
}

impl<T> Arena<T> {
    /// 创建空 arena。
    pub const fn new() -> Arena<T> {
        Arena { values: Vec::new() }
    }

    /// 追加元素,返回其 arena 下标(调用方用对应 ID 类型的 new(index) 包装)。
    pub fn push(&mut self, value: T) -> u32 {
        let index = self.values.len();
        assert!(
            index <= u32::MAX as usize,
            "arena size exceeds the u32 index space"
        );
        self.values.push(value);
        index as u32
    }

    /// 按下标读取元素;越界是内部错误,debug 下直接断言(ADR 0001 规则 8)。
    pub fn get(&self, index: u32) -> &T {
        debug_assert!(
            (index as usize) < self.values.len(),
            "arena index out of range: {index} (len {})",
            self.values.len()
        );
        &self.values[index as usize]
    }

    /// 按下标可变读取元素;越界是内部错误,debug 下直接断言(ADR 0001 规则 8)。
    pub fn get_mut(&mut self, index: u32) -> &mut T {
        debug_assert!(
            (index as usize) < self.values.len(),
            "arena index out of range: {index} (len {})",
            self.values.len()
        );
        &mut self.values[index as usize]
    }

    /// 元素个数。
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// 按下标顺序迭代全部元素。
    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        self.values.iter()
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Arena<T> {
        Arena::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct Item {
        value: u32,
    }

    #[test]
    fn push_get_and_metadata() {
        let mut arena = Arena::new();
        assert!(arena.is_empty());
        assert_eq!(arena.len(), 0);
        let first = arena.push(Item { value: 10 });
        let second = arena.push(Item { value: 20 });
        assert_eq!((first, second), (0, 1));
        assert_eq!(arena.len(), 2);
        assert!(!arena.is_empty());
        assert_eq!(arena.get(first), &Item { value: 10 });
        assert_eq!(arena.get(second), &Item { value: 20 });
    }

    #[test]
    fn get_mut_updates_in_place() {
        let mut arena = Arena::new();
        let index = arena.push(Item { value: 1 });
        arena.get_mut(index).value = 99;
        assert_eq!(arena.get(index).value, 99);
    }

    #[test]
    fn iter_returns_elements_in_index_order() {
        let mut arena = Arena::new();
        for value in [3u32, 1, 2] {
            arena.push(Item { value });
        }
        let values: Vec<u32> = arena.iter().map(|item| item.value).collect();
        assert_eq!(values, vec![3, 1, 2]);
    }

    #[test]
    fn default_matches_new() {
        let arena: Arena<Item> = Arena::default();
        assert!(arena.is_empty());
        assert_eq!(arena.len(), 0);
    }

    #[test]
    #[should_panic(expected = "arena index out of range")]
    fn get_out_of_range_panics() {
        let arena: Arena<Item> = Arena::new();
        let _ = arena.get(0);
    }

    #[test]
    #[should_panic(expected = "arena index out of range")]
    fn get_mut_out_of_range_panics() {
        let mut arena: Arena<Item> = Arena::new();
        let _ = arena.get_mut(0);
    }
}
