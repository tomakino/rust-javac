# rust-javac 代码与注释规范

本规范适用于 crates/javac-core 及后续重构阶段（P0–P5）。

## 1. 文件头模板（不超过 10 行）

    //! Attr：标注与类型检查主流程。
    //!
    //! 对应：com/sun/tools/javac/comp/Attr.java @ 982d53c
    //! 偏差：见 docs/PORTING_NOTES.md#attr

规则：

- 第一行是职责（一句话，中文），不是“移植登记”。
- “对应”写官方包路径 + 提交哈希（982d53c）。
- “偏差”指向 docs/PORTING_NOTES.md 的锚点；没有偏差时写“无已知偏差”。
- 已移植方法清单、NOT PORTED 清单、Registration 映射一律迁到 docs/PORTING_MAP.md，不再放在文件头。
- #![allow(...)] 等内层属性保留在文件最顶部，顺序不变。

## 2. 注释的三类

### 2.1 锚点（Anchor）

用于把 Rust 代码定位到官方实现，一行一个：

    // Java: checkFlags(Check.java:2914)

- 只写官方方法名与行号，不写解释。
- 保留官方命名（checkFlags），不翻译成 snake_case。
- 锚点允许出现在函数上方或分支上方，但不得重复。

### 2.2 偏差（Port note）

用于记录本地实现与官方的差异，必须简短：

    // Port note PN-001: 字段声明位置有两个来源（诊断用名字、行号用声明起点），
    // 当前共用 VarDecl.pos；详见 docs/PORTING_NOTES.md#pn-001。

规则：

- 每条偏差在 PORTING_NOTES 有唯一编号（PN-xxx），注释只写编号 + 一句话。
- 偏差条目必须包含：官方行为、本地行为、影响、状态、计划阶段。
- 不允许在源码里写“当时为什么这么改”的长篇历史；历史进 commit message 或 PORTING_NOTES 的“历史”栏。

### 2.3 实现说明（Why）

用于解释 Rust 特有的设计（借用、arena、所有权、性能），不涉及官方语义：

    // Rust: 树节点保存在 arena 中，跨阶段可变状态集中在 Session，
    // 避免 Rc<RefCell> 的借用重入。

- 只解释“为什么这样写”，不重复代码本身。
- 不写“这行代码做了 X”式的复述。

## 3. 禁止事项

- 文件头大段 “Ported in this file / NOT PORTED / Registration” 清单（迁 PORTING_MAP）。
- 历史叙事式注释（“之前用 isSameType 导致…所以改成…”）。
- 重复注释（同一事实在文件头、函数上、行内各写一遍）。
- 用注释“证明”与官方一致（由测试与 // Java: 锚点承担）。

## 4. 命名

- 类型名对应官方类（Attr、Check、TreeInfo），保持可搜索。
- 函数名用 snake_case；官方方法在锚点中保留原名。
- 字段名用 snake_case；与官方字段一一对应时，可在首次出现处加 // Java: fieldName。

## 5. 错误与所有权约定

- 用户可触发的失败返回 PResult<T>（Result<T, Diagnostic>）；expect() 只用于内部不变量。
- panic 只用于“不可能发生”的状态，且必须带明确消息。
- 跨阶段可变状态进入 Session；单阶段可变用 &mut；只读共享用 &T / Rc<T>（无 RefCell）。
- 回调使用 FnMut，不使用 Rc<RefCell<dyn Fn>>。

## 6. 文档

- 每个模块必须有 //! 文档：职责、对应官方类、偏差链接。
- //! 不超过 6 行；详细登记进 PORTING_MAP。
- 公共 API 使用 /// 文档；内部函数按需。

## 7. 门禁（scripts/gate.py）

| 级别 | 内容 | 触发 |
|---|---|---|
| none | 不跑 | 仅 docs/脚本/注释 |
| L0 | cargo test -p javac-core --lib | 任何代码改动 |
| L1 | L0 + repros + gson | parser、诊断、CLI、小逻辑 |
| L2 | L1 + guava + okhttp | 类型、推断、Attr、Lower、Codegen |
| L3 | 全量 | 合并点、每日、大改 |

- 日常只跑 L0/L1；L2 仅在触及类型/生成时；L3 仅在合并前。
- 失败即停，不继续更高级别。

## 8. 提交

- 一个逻辑单元一个 commit；禁止为“改一行”单独提交。
- commit message 写“做了什么 + 验证级别 + 结果”，不写历史叙事。
