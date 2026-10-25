# ADR 0001:Symbol/Type/Tree/Scope 使用 arena + ID

- 状态:已接受
- 基线:openjdk/jdk17u @ 982d53c (jdk-17.0.19-ga)

## 背景

javac 的实现建立在 JVM 的 GC 可变对象图之上:

- `Symbol` / `Type` / `JCTree` / `Scope` 是互相引用的可变对象,存在环
  (ClassSymbol.members 与成员的 owner;Type.tsym 与 Symbol.type;Tree 父子)。
- 多个组件(Context 服务)持有同一对象引用,并在任意时刻修改补全状态
  (CompletionFailure、Symbol 补全、Lower 重写树、Scope 链)。
- Rust 没有 GC,不能用引用计数的对象图直接表达(环会泄漏;可变别名需要
  RefCell,运行期会 panic)。

## 决策

核心编译期对象一律使用 **arena + 无符号 ID**:

| Java 对象 | Rust |
|---|---|
| `Symbol`(及其子类) | `SymbolArena`(`Vec<Symbol>`)+ `SymbolId(u32)` |
| `Type`(及其子类) | `TypeArena`(`Vec<Type>`)+ `TypeId(u32)` |
| `JCTree`(及其子类) | `TreeArena`(`Vec<JCTree>`)+ `TreeId(u32)` |
| `Scope` | `ScopeArena`(`Vec<Scope>`)+ `ScopeId(u32)` |
| Java `null` 引用 | `Option<XxxId>` |

规则:

1. arena 与全部 `*Id` 类型由 `CompilerContext` 独占持有;每次编译一个 Context,
   编译结束整体释放。ID 不得跨编译实例使用。
2. Java 代码里的对象引用(字段、参数、返回值、集合元素)在 Rust 中一律用 ID 表达;
   平台层对象(文件对象、JavaString、诊断文本、文件名等值类型)允许 Rc/值语义。
3. 访问模式:`ctx.sym(id)` / `ctx.sym_mut(id)` / `ctx.typ(id)` / `ctx.tree(id)`
   等访问器;组件逻辑为带 `ctx: &mut CompilerContext` 参数的函数(或 impl 方法)。
   Java 的 `x.field` -> `ctx.sym(x).field`;`sym.complete()` -> `symbols::complete(ctx, id)`。
4. 借用:禁止用 `Rc<RefCell<...>>` 表达核心对象图(PORTING_RULES §9 白名单除外:
   NameTable、List 的 Cell tail、接口层 Writer)。需要同时访问多个 arena 时用字段
   拆分借用或 `std::mem::take`;不得引入运行期借用 panic 风险。
5. Type 的不可变语义:按 javac 语义,Type 构造完成后不修改(惰性字段除外,按上游
   在属性补全阶段填充);arena 内允许内部可变字段,但必须与上游写入时机一致。
6. Tree 可变:属性阶段写 `type` 字段,Lower 重写节点时替换 ID 指向的节点内容;
   父子关系用 ID 存储,不保留父子双向强引用。
7. 作用域:Scope 用 arena;Scope 的 prev/next(空作用域链)用 `Option<ScopeId>`;
   符号表条目为 `HashMap<Name, SymbolId>`,按上游语义处理遮蔽/重复。
8. 不变量与断言:所有 arena 访问在 debug 下断言 ID 有效;编译期 arena 只增不减
   (与 javac 的 GC 生命周期等价——编译器不依赖对象被回收)。
9. 适配文件:ID 类型与访问器放 `crates/javac-core/src/ids.rs` / CompilerContext 模块
   (非 1:1,登记 PORTING_RULES §9)。

## 结果

- 无 GC、无环泄漏、无运行期借用 panic 风险;与 javac 的对象生命周期语义等价。
- 代价:Java 调用点需要机械改写为 `ctx` 访问形式;这是"表达层映射"而非行为差异。
- 后续若性能不足,可优化 arena 布局/内联缓存,不改变可观察行为。

## 补充(2026-xx):Attribute / Directive arena

Attribute(注解值)与 Directive(模块指令)是不可变但有身份语义的 Java 对象(Java 未重写 equals
的类按引用相等)。按本文档规则扩展第 5/6 种 arena:AttributeArena/AttributeId、
DirectiveArena/DirectiveId。JCTree 的 JCAnnotation(attribute)与 JCExports/JCOpens/JCRequires
(directive)字段用 Option<ID>;访问经 CompilerContext 访问器。Array/Compound 等 Attribute
内部的属性引用同样用 AttributeId。
