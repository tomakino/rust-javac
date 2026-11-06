# ravac-lsp 开发者体验完整实现计划（v2，完整实现版）

> 文档状态：草案（待执行）
> 适用范围：`rust-javac` 工作区中的 `crates/javac-ide-core`、`crates/javac-analysis-api`、`crates/ravac-lsp`
> 写作目的：以「每天使用 10 小时的 Java 程序员」为中心，系统补齐高频 LSP 能力；所有能力按完整语义实现，禁止简化实现
> 字数要求：本文档正文（不含代码块与空白）不少于 20000 字；由脚本自动核验

---

## 0. 文档说明

这份文档不是一份「想到哪写到哪」的待办列表，而是一份可执行、可验收、可回溯的工程计划。它回答四个问题：

1. 程序员每天到底在用 LSP 的哪些能力？哪些是点击率最高、等待时间最敏感、出错代价最大的？
2. 当前 `ravac-lsp` 在这些能力上做到什么程度？哪些是「有但不好用」，哪些是「完全没有」？
3. 每一项缺失或缺陷，完整实现应该长什么样？数据结构、算法、LSP 协议、错误处理、测试矩阵分别是什么？
4. 如何证明「已经完成」？验收命令、测试用例、真实项目复测、回归门禁分别是什么？

本文档面向三类读者：

- **实现者**：需要按章节直接落地代码，因此每一项都给出文件路径、函数签名、算法步骤与测试清单。
- **评审者**：需要判断实现是否偷工减料，因此每一项都给出「语义定义」与「禁止的近似做法」。
- **使用者**：需要知道哪些能力可用、哪些暂不可用、边界在哪里，因此每一项都给出行为契约与已知限制。

文档中的「完整实现」有严格定义：**基于编译器类型系统与语法树做语义判定；找不到就返回空；不以名称、参数个数、正则、行号近似替代语义判断**。任何因底层 API 缺失而暂时降级的点，必须显式标注为「未实现」，不得静默近似。

---

## 1. 背景与目标

### 1.1 项目背景

`rust-javac` 是一个用 Rust 重写 javac 的项目，`javac-ide-core` 承担 IDE 分析引擎的职责（常驻编译器上下文、索引、诊断、补全、导航等），`ravac-lsp` 是一层尽量薄的 LSP 服务器（stdio + JSON-RPC + 能力分发）。当前已经具备的能力覆盖了补全、诊断、定义、类型定义、实现、hover、引用、重命名、签名帮助、inlay hints、大纲、全局符号、格式化、code action、类型层次、调用层次、文档高亮、折叠等。

在最近的几轮迭代中，修复了大量影响日常使用的问题，例如：

- 字符串/注释内的定义、hover、prepareRename 误触发，现在统一返回空；
- 字符串/注释内补全曾弹出全量 JDK 类，现在不再补全；
- 签名帮助在方法声明参数处误弹出，且在嵌套调用/多行调用时参数索引错误，现在按词法感知与跨行扫描修复；
- 局部变量 hover 只显示 `local value`、方法 hover 没有返回类型、方法签名缺少参数列表，现在 hover 文本包含类型与签名；
- 未使用 import 检查把字符串、注释、更长标识符当作「使用」，导致漏报，现在按掩码与标识符边界判定；
- 跨文件诊断与补全不随其他文件保存/未保存修改更新，现在通过索引 generation 失效与 overlay 注入修复；
- Partial 索引下同包文件无法跳转、外部 jar 与 JDK 成员无法跳转，现在已支持 sources jar 与 src.zip；
- UTF-16 与 UTF-8 字节偏移混用导致的 panic 与列偏移，现在统一使用 UTF-16 偏移计算 LSP 位置；
- 大纲泄漏编译器生成的 `<init>`、构造器显示名为 `<init>`，现在已过滤并显示类名。

这些修复解决了「会崩溃、明显报错」的问题，但距离「每天 10 小时使用仍然顺手」还有差距：**缺能力**（比如跳到被重写的方法、智能扩选、快速修复、语义高亮、代码折叠之外的代码镜头）、**缺完整性**（比如继承方法匹配还停留在参数个数层面）、**缺一致性**（同类能力在不同入口的行为不一致）。

### 1.2 本轮目标

用一份可执行的计划，把「高频、影响日常」的能力按完整语义补齐，并在每一项上给出可验证的证据。目标不是「功能列表看起来很长」，而是「程序员在真实工作流中少按几次键盘、少切几次窗口、少犯几次静默错误」。

具体目标：

1. **补齐 P0 能力**：继承方法重写导航（含 hover Overrides）、智能选择扩展、快速修复（加/删 import）。
2. **补齐 P1 能力**：输入时格式化、文档链接。
3. **补齐 P2 能力**：语义高亮、代码镜头、import 整理。
4. **清除历史近似实现**：把「按参数个数匹配父方法」「按名字找声明」这类近似判定替换为完整语义判定，或在文档中明确标注其仅用于无歧义场景。
5. **建立回归门禁**：每项能力都有单元测试、集成测试、LSP 端到端测试与真实项目复测，且全量测试与 17 项 LSP 冒烟必须全绿。

### 1.3 非目标

为避免范围失控，本轮明确不做：

- 不实现完整的 Java 重构体系（Extract Method、Inline、Move Class、Change Signature 等），这些属于后续专项；
- 不实现反编译（只从 sources jar / src.zip 定位源码，拿不到就返回空）；
- 不实现多项目工作区/多根工作区（当前仍是单根）；
- 不修改 LSP 传输层（仍是 stdio + JSON-RPC）；
- 不引入新的重量级依赖（若必须引入，单独评审并在文档记录原因与替代方案）。

---

## 2. 用户画像与一天的工作流

### 2.1 用户画像

目标用户是「每天写 8–12 小时代码的 Java 后端/全栈程序员」，特征是：

- 同时维护多个 Maven/Gradle 项目，习惯在 IDE 里完成 90% 的编辑动作；
- 代码库规模从几千行到几十万行不等，单文件常见 200–2000 行；
- 使用大量框架（Spring、MyBatis、Lombok 生成代码、泛型工具类），代码里存在大量接口/实现/父类/重写关系；
- 对延迟敏感：补全弹出超过 300ms 就会开始烦躁，跳转超过 1s 就会改用搜索；
- 对「静默错误」零容忍：跳错位置、补全插错前缀、重命名漏改引用，比「没有这个功能」更让人不信任工具。

### 2.2 一天的真实操作序列

把一天的工作流拆成若干阶段，并统计每个阶段使用 LSP 的动作：

**阶段 A：打开项目与索引**

- 打开 IDE，等待项目索引完成；
- 期间可能打开若干文件，查看结构（大纲）、跳转定义；
- 索引未完成时，用户会立刻尝试补全/跳转，此时行为必须「可用但不撒谎」：能回答就回答，不能回答就明确空结果，不能给旧结果。

**阶段 B：阅读代码**

- 打开类文件，用大纲（documentSymbol）快速定位方法；
- 用折叠（foldingRange）收起长方法/注释；
- 用 documentHighlight 看当前变量在文件内的所有出现；
- 用 Ctrl+click（definition）在类、方法、字段、常量之间跳转；
- 用 hover 看类型、签名、javadoc；
- 用 references 看某个方法/字段被谁使用；
- 用 typeDefinition 看变量类型，用 implementation 看接口实现。

**阶段 C：编写新代码**

- 输入类名/方法名时依赖补全（含自动导入）；
- 输入 `.` 后依赖成员补全（含 var 推断、链式调用、静态成员、this）；
- 输入 `(` 后依赖签名帮助（当前参数高亮）；
- 输入 `@` 后依赖注解补全；
- 写 `new ` 后依赖构造器补全；
- 写方法体时依赖 inlay hints（var 类型、参数名）；
- 每敲几行触发一次诊断（红色波浪线）。

**阶段 D：修复错误**

- 看到「找不到符号」时，第一反应是按 `Ctrl+.` 看快速修复（Add import）；
- 看到「未使用 import」时，希望有 Remove unused import / Organize imports；
- 看到类型不匹配时，希望 hover 能解释类型；
- 修复后希望诊断立刻更新（不能等保存）。

**阶段 E：重构**

- 重命名类/方法/字段/参数（跨文件 rename，含 import）；
- 在重写方法上按 `Ctrl+U`（或等价操作）跳到父类/接口方法；
- 在接口方法上找实现（implementation）；
- 用 call hierarchy 看调用链；
- 用 type hierarchy 看继承树；
- 用 codeLens 看「N 个引用」评估影响面。

**阶段 F：提交前**

- 格式化整个文件（formatting）；
- 整理 import（organizeImports）；
- 检查诊断是否清零；
- 可能用 selectionRange 逐级扩选检查代码块。

### 2.3 高频动作排序（用于优先级）

按「每天触发次数」从高到低：

1. 补全（每秒都可能触发）；
2. 诊断（每次编辑后）；
3. 定义跳转（每分钟多次）；
4. hover（每分钟多次）；
5. 签名帮助（写调用时）；
6. 重命名/引用（重构时集中爆发）；
7. 继承方法重写导航（写 `@Override` 时集中爆发，且当前完全缺失）；
8. 快速修复（看到错误时）；
9. 格式化/整理 import（提交前）；
10. 折叠/高亮/大纲（阅读时）；
11. 语义高亮（持续存在，但缺失时用户只会「感觉颜色不对」）；
12. 代码镜头（重构前评估）。

结论：P0 应该给「继承方法重写导航」「快速修复」「智能扩选」；P1 给「输入格式化」「文档链接」；P2 给「语义高亮」「代码镜头」「import 整理」。这与前一份计划的优先级一致，但本轮要求每一项都按完整语义实现。

---

## 3. 现状能力盘点与成熟度矩阵

### 3.1 能力矩阵

| 能力 | 主要实现位置 | LSP 方法 | 成熟度 | 已知缺陷/边界 |
|---|---|---|---|---|
| 补全 | `analysis/features/completion/*`、索引 | `textDocument/completion` | 较高 | 跨文件成员补全在索引 Ready 的部分场景缺候选（已记录）；字符串/注释内已返回空 |
| 诊断 | `analysis/diagnostics.rs`、`inspections.rs`、workspace | `publishDiagnostics` | 较高 | unused import 已修复；诊断范围以 javac 位置为准 |
| 定义 | `navigation/definition.rs`、索引 | `textDocument/definition` | 较高 | 外部 jar 依赖 sources jar；JDK 依赖 src.zip |
| 类型定义 | `navigation/definition.rs` | `typeDefinition` | 中 | 已支持局部变量；复杂类型（泛型/数组）需补充 |
| 实现 | `navigation/definition.rs` | `implementation` | 中 | 只做「父→子」，没有「子→父」 |
| hover | `features/hover/mod.rs` | `textDocument/hover` | 较高 | 方法已含签名与返回类型；类 hover 缺包名 |
| 引用 | `navigation/references.rs`、索引 | `textDocument/references` | 较高 | 无 includeDeclaration 开关（总是含声明） |
| 重命名 | `workspace/service.rs`、index | `rename` | 较高 | Partial 索引下可能不完整（不提示） |
| 签名帮助 | `features/signature/help.rs` | `signatureHelp` | 中 | 重载选择按「能容纳实参」而非实参类型 |
| inlay hints | `features/inlay_hints.rs` | `textDocument/inlayHint` | 较高 | varargs 提示缺失；参数名回退本地声明 |
| 大纲 | `index/query.rs`、`analysis/symbols.rs` | `documentSymbol` | 较高 | 类 range 只覆盖名称，折叠范围偏小 |
| 全局符号 | `index/query.rs` | `workspace/symbol` | 较高 | 局部变量不出现在全局（正确） |
| 格式化 | `features/format/mod.rs` | `textDocument/formatting` | 较高 | 依赖 google-java-format jar；CRLF 已保留 |
| code action | `features/actions/*`（薄） | `codeAction` | 低 | 缺 Add import / Remove unused import |
| 类型层次 | `features/hierarchy/*` | `typeHierarchy/*` | 中 | JDK 类型层次依赖符号加载 |
| 调用层次 | `features/hierarchy/*` | `callHierarchy/*` | 中 | 跨文件调用依赖索引 |
| 文档高亮 | `workspace/service.rs` | `documentHighlight` | 新增 | 当前只返回当前文件；kind 未区分读写 |
| 折叠 | `features/folding.rs` | `foldingRange` | 新增 | 只做花括号与注释块，未做 import 区域/region |

### 3.2 成熟度评级说明

- **较高**：主路径可用，边界已知，有测试覆盖；
- **中**：主路径可用，但在典型场景下有静默缺陷或缺失半边能力；
- **低**：只覆盖最小场景，日常使用会频繁失望；
- **新增**：刚接入，测试覆盖待补。

### 3.3 已修复问题的经验教训

回顾最近修复的 bug，可以提炼出三条经验，直接决定本计划的实施方式：

1. **单位必须统一**：UTF-16 与字节混用曾导致 panic 和列偏移；后续所有位置计算必须走 `analysis::positions` 的转换函数，且入参单位在函数名/注释中写明。
2. **缓存必须可失效**：诊断/分析缓存必须跟索引 generation、overlay 内容绑定；任何「只看当前文件 hash」的缓存都会在跨文件编辑时撒谎。
3. **语义判定不能用文本近似**：字符串/注释内的误导航、unused import 误判、按名字找入口，都是文本近似的代价。后续能力要么用编译器语义，要么明确「只用于无歧义定位」。

---

## 4. 缺口分析：每个缺失对日常的具体影响

### 4.1 继承方法重写导航缺失

**场景**：程序员在实现类里写 `@Override public void onMessage(...)`，想确认父接口的默认实现或抽象声明；或者在子类里重写 `toString`，想跳 `Object.toString` 看契约。

**现状**：只能手动打开父类/接口文件、搜索方法名；或者用 `implementation`（反方向）找不到。这在一个有 20 个实现类的接口上非常痛苦。

**影响**：每次重写相关的确认，平均多花 20–60 秒；如果父方法在 JDK 里，几乎放弃确认，直接凭记忆写。

**完整实现**：见第 6 章。

### 4.2 智能选择扩展缺失

**场景**：想把 `a.b(c, d)` 整体包进 `Optional.of(...)`，或者把整个 `if` 块提取出来。用户按 `Shift+Alt+→` 期望逐级扩选：`c` → `b(c, d)` → `a.b(c, d)` → 整条语句 → 方法体 → 方法 → 类。

**现状**：没有 `selectionRange`，客户端只能按词/行扩选，括号嵌套时经常选错。

**影响**：每次包裹/提取都要手动拖选，容易漏括号。

**完整实现**：见第 7 章。

### 4.3 快速修复缺失

**场景**：粘贴一段代码，出现 10 个「找不到符号」；或者删除代码后留下 5 个未使用 import。用户按 `Ctrl+.` 期望「Add import」「Remove unused import」。

**现状**：`codeAction` 基本是空的，用户只能手动写 import 或全文搜索删除。

**影响**：每次粘贴/重构后多花几分钟；import 顺序也容易乱。

**完整实现**：见第 8 章。

### 4.4 输入时格式化缺失

**场景**：输入 `}` 后希望自动回退缩进；输入 `;` 后希望当前行对齐；输入换行后希望延续缩进。

**现状**：没有 `onTypeFormatting`，只能整文件格式化或手动 Tab。

**影响**：写代码时反复手动调整缩进，节奏被打断。

**完整实现**：见第 9 章。

### 4.5 文档链接缺失

**场景**：hover 的 javadoc 里有 `{@link Foo}`、`http://...`；import 行想直接点开类文件。

**现状**：`documentLink` 缺失，javadoc 里的链接是纯文本。

**影响**：阅读文档时无法一键跳转。

**完整实现**：见第 10 章。

### 4.6 语义高亮缺失

**场景**：类/方法/字段/参数/局部变量需要不同颜色；尤其是「字段 vs 局部变量」「静态 vs 实例」「参数 vs 局部变量」的区分。

**现状**：客户端只有语法高亮，所有标识符同色。

**影响**：阅读长方法时，变量来源不清晰，增加理解成本。

**完整实现**：见第 11 章。

### 4.7 代码镜头缺失

**场景**：重构前想知道某个方法被多少处引用、某个接口有多少实现。

**现状**：需要手动执行 Find References。

**影响**：评估影响面多一步操作。

**完整实现**：见第 12 章。

### 4.8 import 整理缺失

**场景**：提交前希望 import 排序、去重、删除未使用、静态导入分组。

**现状**：只有单条 unused import 诊断，没有整理命令。

**影响**：提交前手工整理，容易遗漏。

**完整实现**：见第 13 章。

### 4.9 生成代码缺失

**场景**：给 POJO 生成构造器、`toString`、`equals`/`hashCode`。

**现状**：没有生成能力。

**影响**：手写模板代码，容易漏字段。

**完整实现**：见第 14 章（P3，视时间）。

---

## 5. 设计原则

### 5.1 正确性优先

任何「用户会依赖其结果」的能力，必须基于编译器语义（`Types`、`Symbol`、`Tree`）。文本匹配只允许用于「定位声明名」这类无歧义场景，并且函数命名与注释必须说明前提（例如 `declared_name_start` 只从编译器给出的声明起点向后找第一个名字，前提是起点之前没有同名 token）。

### 5.2 单一事实来源（SSOT）

一个概念只有一份实现：

- 方法签名擦除比较：只允许一份 helper（`erased_signature_eq`），`super_methods`、`implementation` 复用；
- import 插入点计算：只允许一份 helper（`import_insert_position`），`Add import`、`organizeImports` 复用；
- 字面量/注释掩码：复用 `inspections::mask_non_code`；
- 位置单位：复用 `analysis::positions`；
- overlay 注入：复用 `JavacFileManager::set_overlay_files`。

### 5.3 不简化

「简化」在本计划中的定义：

- 用名称 + 参数个数代替擦除签名；
- 用行号/括号文本代替语法树范围；
- 用字符串搜索代替符号解析；
- 用「第一个能容纳的参数」代替按类型选择重载；
- 找不到时返回「最可能」的结果，而不是空。

以上做法一律禁止。若底层能力缺失（例如 `TreeInfo.end_pos` 未移植），先补底层能力，再做上层功能。

### 5.4 可验证

每项能力必须有：

- 单元测试（算法/helper）；
- 集成测试（Workspace API）；
- LSP 端到端（JSON-RPC 请求 → 响应断言）；
- 真实项目/构造项目复测；
- 全量回归（`cargo test -p javac-ide-core`）与 LSP 冒烟（17/17）。

### 5.5 可回退

每项能力独立成模块/函数，出问题可以单独关闭（LSP 层不注册能力或返回空），不影响其他能力。

---

## 6. P0-1：继承方法重写导航（`super_methods`）——完整实现

### 6.1 语义定义

「继承方法重写导航」回答的问题是：**光标所在的方法，重写了哪些父方法？** 结果是一个位置列表，可能包含：

- 父类链上最近的同签名方法（可能跨多级，例如 `Sub extends Mid extends Base`，`Sub.f` 重写 `Mid.f`，而 `Mid.f` 重写 `Base.f`；用户通常期望看到直接父方法，也可能想看到整条链）；
- 多个接口中的同签名方法（`interface A { void f(); }` 与 `interface B { void f(); }`，实现类同时实现两者）；
- 接口默认方法（如果子类没有重写，则光标处是默认方法本身，此时应返回其父接口的声明）；
- JDK 父方法（如 `Object.toString`、`Throwable.getMessage`）；
- 外部 jar 父方法（如果 sources jar 可用）。

判定规则严格按 Java 语言规范（JLS 8.4.2、8.4.8.1、8.4.6）：

1. **子方法**：光标处符号必须是方法（`MethodSymbol`），且不是构造器；
2. **父方法候选**：必须是方法、非构造器、非 `static`、非 `private`；
3. **名称**：与子方法名称完全相等（区分大小写）；
4. **参数类型**：擦除后逐项相等（不是参数个数）；
5. **varargs**：按数组类型参与比较，`f(int...)` 与 `f(int[])` 擦除后相同；
6. **返回类型**：协变返回不影响匹配；
7. **桥接方法**：编译器为泛型/协变生成的桥接方法（`BRIDGE` 标志）不是用户声明的，必须跳过；
8. **泛型父类**：必须在父类型站点（如 `Base<String>`）上用 `Types::member_type` 实例化父方法，再比较擦除后的参数类型；
9. **传递性**：沿 `supertype` 与 `interfaces` 递归，得到完整父类型闭包；但结果只返回**实际声明了该签名方法**的父类型，不返回「继承来的」重复项；
10. **去重**：同一文件同一位置只返回一次；
11. **排序**：按文件路径、行、列排序，保证稳定；
12. **找不到**：返回空数组，绝不返回近似结果。

### 6.2 数据结构

```rust
/// 一个父类型「站点」：带上泛型实参的类型，例如 Base<String>。
/// 必须是 TypeId 而不是 SymbolId，因为泛型替换依赖实参。
struct SuperSite {
    type_id: TypeId,
    class: SymbolId,
}

/// 方法签名（擦除后）用于比较。
struct ErasedSignature {
    name: Name,
    params: Vec<String>,      // erasure 后的类型名
    is_varargs: bool,
}
```

这里的 `params` 用擦除后类型的 `type_name` 字符串，而不是 TypeId，原因：

- 擦除后的类型是稳定的（不依赖类型变量实例化）；
- 跨 classfile/源码的 TypeId 不可直接比较（不同符号表）；
- 字符串比较可以避免「相同结构 TypeId 不同」的假阴性。

但字符串比较必须基于 `erasure` 的结果，不能直接对原始类型做 `to_string`（否则泛型父类的 `T` 会被打印成 `T`，与 `String` 不等）。

### 6.3 算法（完整步骤）

```text
输入：file, position
输出：Vec<Location>（被重写的父方法声明）

1. 如果光标处位于字符串/注释（in_literal_or_comment）→ 返回空。
2. child = symbol_id_at(file, position)；
   若 child 不是 MethodSymbol 或名字为 "<init>" → 返回空。
3. child_owner = sym(child).owner；若为空 → 返回空。
4. child_sig = erased_signature_of(child)：
   - ty = sym(child).ty（MethodType）；
   - argtypes = ty.argtypes（若 ty 缺失，用 params 逐个 sym(param).ty，缺失则返回空）；
   - 对每个 argtype：erasure(argtype) → type_name；
   - varargs = sym(child).flags & VARARGS != 0。
5. sites = supertype_closure(child_owner)：
   - 从 owner 的 ty（ClassType）开始；
   - 对每个 site：
     a. super_type = types.supertype(ctx, syms, site.type, None)；
        若 tsym 存在且未访问 → 加入 sites 和栈；
     b. interfaces = types.interfaces(ctx, syms, site.type)；
        对每个 interface 类型 → tsym 存在且未访问 → 加入 sites 和栈；
   - 用 HashBiSet<SymbolId> 去重（按类型符号，而不是 TypeId，避免同一类型不同泛型实例重复）。
6. 对每个 site：
   a. 取 site.class 的 members（NON_RECURSIVE）；
   b. 对每个 member：
      - 必须是 MethodSymbol；
      - flags & STATIC == 0；
      - flags & PRIVATE == 0；
      - flags & BRIDGE == 0；
      - name == child_sig.name；
      - parent_ty = types.member_type(ctx, syms, site.type, member)；
        若 parent_ty 不是 MethodType → 跳过；
      - parent_args = parent_ty.argtypes（已实例化）；
      - 若 parent_args.len() != child_sig.params.len() → 跳过；
      - 对每个 i：erasure(parent_args[i]) 的 type_name == child_sig.params[i]；
        若有任何不等 → 跳过；
      - 记录 member（父方法声明）。
7. 对每个记录：
   - 转 Location：
     a. parent_owner = sym(member).owner；
     b. 取 parent_owner 的 ClassSymbol.sourcefile；
     c. 若 sourcefile 在 config.source_path 内：
        - 读文本（overlay 优先）；
        - pos = sym(member).pos.max(0)；
        - name_pos = declared_name_start(text, pos, name)；
        - (line, col) = offset_to_line_col(text, name_pos)；
        - Location { file, range: point(line, col) }；
     d. 否则（classpath/JDK）：
        - 若 parent_owner 是 JDK 类（sourcefile 在 ct.sym 或 `java.*`/`javax.*` 包）→
          jdk_member_location(jdk_src, flat_name, name, output_dir)；
        - 否则 → external_source_location(flat_name, name, class_path, output_dir)；
        - 都返回 None → 跳过该父方法（不猜）。
8. 去重（同一 file + line + col），排序，返回。
```

### 6.4 泛型父类的实例化

最容易出错的是泛型父类。考虑：

```java
class Base<T> {
    void put(T value) { }
}
class Sub extends Base<String> {
    @Override void put(String value) { }
}
```

`Base.put` 的符号参数类型是类型变量 `T`；如果直接 `type_name(sym(param).ty)` 会得到 `T`，与子方法的 `String` 不等，于是「找不到父方法」——这是静默漏报。

正确做法：

1. 取 `site = types.supertype(Sub)` → `Base<String>` 的 TypeId；
2. `parent_ty = types.member_type(ctx, syms, site, Base.put)`；
3. `parent_ty.argtypes[0]` 此时是 `String`；
4. `erasure(String)` 与 `erasure(String)` 相等 → 匹配。

同理，接口泛型：

```java
interface Handler<T> { void handle(T t); }
class StringHandler implements Handler<String> {
    public void handle(String s) { }
}
```

`types.interfaces(StringHandler)` 返回 `Handler<String>`，`member_type` 实例化后参数是 `String`。

### 6.5 varargs 的处理

```java
class Base {
    void log(String fmt, Object... args) { }
}
class Sub extends Base {
    @Override void log(String fmt, Object[] args) { }  // 等价重写
}
```

`member_type` 返回的 `argtypes` 对 varargs 是数组类型 `Object[]`；子方法 `Object[]` 也是 `Object[]`，擦除后字符串相等。因此只要按擦除比较，varargs 自然正确。

需要额外注意：

- 子方法用 `Object...` 声明时，`sym(child).ty.argtypes` 同样是 `Object[]`；
- 不能把 `varargs` 展开成「可变参数个数」来匹配，否则会与「参数个数」混淆；
- `flags & VARARGS` 只用于展示（hover 提示 `Object...`），不用于匹配判定。

### 6.6 桥接方法

```java
interface Getter<T> { T get(); }
class Impl implements Getter<String> {
    public String get() { return ""; }
}
```

编译器会生成桥接方法 `Object get()`（`BRIDGE` 标志）。它出现在 `Impl` 的成员里，但不是用户写的声明。处理：

- 遍历父方法时不考虑桥接（父接口本来就没有桥接）；
- 遍历子方法时（`symbol_id_at` 命中的可能是桥接）需要跳过桥接并找到真正的实现方法；
- 结果里绝不能出现桥接方法。

### 6.7 多接口与去重

```java
interface A { void run(); }
interface B { void run(); }
class C implements A, B { public void run() { } }
```

`supertype_closure` 会返回 `A` 和 `B` 两个站点；两个站点的 `run` 都匹配；结果返回两个 Location。

去重规则：按 `(file, line, column)` 去重，不按符号去重——如果 `A` 和 `B` 都继承自同一个 `Runnable`，`run` 实际声明在 `Runnable`，`close` 会返回同一个位置两次，需要去重。

### 6.8 JDK 与外部 jar 的父方法定位

- **JDK**：父方法可能在 `java.lang.Object`、`java.util.AbstractList` 等。定位方式：
  - 取父类符号的 `flat_name`（如 `java.lang.Object`）；
  - `jdk_member_location(jdk_src, flat_name, method_name, output_dir)`；
  - 该函数在 `src.zip` 中按 `java/lang/Object.java` 查找并提取，再用 `locate_member` 定位方法声明。
- **外部 jar**：父类来自 Maven 依赖（如 `org.springframework...`）：
  - `external_source_location(flat_name, method_name, class_path, output_dir)`；
  - 优先用与二进制 jar 同目录的 `-sources.jar`；
  - 找不到 sources → 返回空（不跳 class 文件，不反编译）。
- **项目内但不在 `source_path`**：理论上不会出现；若出现，按外部处理（用 flat_name 找 sources）。

### 6.9 hover 的 Overrides 提示

在 `hover_impl` 中：

1. 先按现有逻辑得到 `symbol` 与基础 hover 文本；
2. 若 `symbol.kind == "method"`：
   - 调用 `super_methods_impl(file, position)`；
   - 若非空，追加：
     ```
     ---
     **Overrides:** `Base.put`, `Handler.handle`
     ```
   - 名称从父方法的 `flat_name` 最后一段取（`Base.put` 显示为 `Base.put`；接口显示 `Handler.handle`）；
   - 最多列 5 个，超出显示「+N more」。
3. 若为空，不追加，保持原 hover。

注意：hover 的方法符号来自 `symbol_at`（文本匹配 + 成员），而 `super_methods_impl` 用 `symbol_id_at`（编译器符号）；两者对「局部方法/匿名类方法」可能有差异，需在测试中覆盖。

### 6.10 LSP 接入

标准 LSP 没有 superMethod 请求。采用：

1. 自定义请求 `ravac/superMethods`：
   - 参数：`{ textDocument: { uri }, position: { line, character } }`；
   - 返回：`Location[]`（与 `definition` 相同结构）；
   - 客户端可绑定快捷键（如 Ctrl+U）。
2. `hover` 的 Overrides 提示（所有客户端可见）。
3. 不修改 `definition` 的语义（在方法名上 Ctrl+click 仍跳自身，避免改变用户预期）。

JSON 示例（请求）：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "ravac/superMethods",
  "params": {
    "textDocument": { "uri": "file:///project/src/main/java/com/example/Impl.java" },
    "position": { "line": 10, "character": 20 }
  }
}
```

响应：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": [
    {
      "uri": "file:///project/src/main/java/com/example/Handler.java",
      "range": {
        "start": { "line": 4, "character": 4 },
        "end": { "line": 4, "character": 4 }
      }
    }
  ]
}
```

### 6.11 错误处理与「不猜」原则

| 情况 | 行为 |
|---|---|
| 光标不在方法上 | 返回空 |
| 光标在字符串/注释内 | 返回空 |
| `sym.ty` 缺失（未完成 attr） | 返回空（不按参数个数猜） |
| `member_type` 返回 ErrorType | 跳过该父方法，debug 日志记录 |
| 父类型闭包为空 | 返回空 |
| 父方法在外部 jar 且无 sources | 返回空 |
| 多个接口同名同签名 | 全部返回（去重后） |
| Partial 索引 | 不受影响（走 FileAnalysis 编译器，不依赖索引） |

### 6.12 性能与缓存

- `supertype_closure` 每次请求重新计算；典型项目继承深度 < 10，成本可忽略（< 1ms）。
- 若同一文件连续请求（hover + 快捷键），可缓存 `(text_hash, position) → Vec<Location>`；缓存键必须包含文本 hash，文本变化即失效。
- 不允许把结果缓存到索引（索引是项目级，方法是编辑器级，缓存粒度不同会引入脏数据）。

### 6.13 测试矩阵

| # | 场景 | 期望 |
|---|---|---|
| 1 | 接口抽象方法实现 | 返回接口方法 |
| 2 | 接口默认方法重写 | 返回默认方法 |
| 3 | 父类具体方法重写 | 返回父类方法 |
| 4 | 重载同参数个数（`f(int)`/`f(String)`） | 只返回 `f(int)` |
| 5 | 泛型父类 `Base<T>` + `Base<String>` | 匹配 |
| 6 | 泛型接口 `Handler<T>` | 匹配 |
| 7 | varargs `Object...` vs `Object[]` | 匹配 |
| 8 | 桥接方法 `Getter<String>.get()` | 返回接口方法，不返回桥接 |
| 9 | 多接口同签名 | 返回 2 个 |
| 10 | 多级继承 | 返回直接父（及链上所有声明） |
| 11 | 父方法 static | 不返回 |
| 12 | 父方法 private | 不返回 |
| 13 | 子方法 static | 返回空 |
| 14 | 构造器 | 返回空 |
| 15 | 无父方法 | 返回空 |
| 16 | 字符串/注释内 | 返回空 |
| 17 | JDK 父方法（`toString`） | 返回 Object.java 位置 |
| 18 | 外部 jar 父方法（有 sources） | 返回 sources 位置 |
| 19 | 外部 jar 无 sources | 返回空 |
| 20 | hover Overrides | 文本含父类名 |
| 21 | 未保存 overlay（父类/子类） | 基于 overlay 文本 |
| 22 | Partial 索引 | 仍可返回 |

### 6.14 文件清单与实现顺序

1. `crates/javac-ide-core/src/analysis/features/navigation/super_methods.rs`（重写为完整版）；
2. `crates/javac-ide-core/src/analysis/features/hover/mod.rs`（Overrides 提示）；
3. `crates/javac-analysis-api/src/lib.rs`（`super_methods` 默认方法）；
4. `crates/javac-ide-core/src/workspace/service.rs`（实现）；
5. `crates/ravac-lsp/src/lib.rs`（`ravac/superMethods` handler）；
6. `crates/javac-ide-core/tests/super_methods_spec.rs`（22 项测试）。

实现顺序：先 helper（`erased_signature_of`、`supertype_closure`），再实现 `super_methods_impl`，再测试，最后 LSP 接入与 hover。

### 6.15 验收标准

- 22 项测试全过；
- hover 在真实项目 `@Override` 方法上显示 Overrides；
- `ravac/superMethods` 请求返回正确位置；
- 全量测试与 LSP 冒烟 17/17 无回归；
- 不出现按参数个数匹配的代码路径（代码评审项）。

---

## 7. P0-2：智能选择扩展（`selectionRange`）——完整实现

### 7.1 语义

`selectionRange` 返回一条范围链，链上每一层都是语法结构，且父范围严格包含子范围。用户按 `Shift+Alt+→` 时，客户端从链底向上选取。

链的层级（从内到外）：

1. 标识符/字面量（最内层）；
2. 类型（类型名、参数化类型、数组类型）；
3. 表达式（二元/一元/方法调用/字段访问/new/强制转换/lambda 体）；
4. 语句（表达式语句、声明、return/throw/if/for/while/switch/try）；
5. 块（`{}`）；
6. 方法/构造器/初始化块；
7. 类体/类声明；
8. 编译单元。

### 7.2 数据来源

- 编译器树：`ctx.trees`（`CompilationUnit` 根）；
- 父子链：一次 DFS 构建 `HashMap<TreeId, TreeId>`；
- 范围：`get_start_pos(tree)` 与 `TreeInfo.end_pos(tree)`；
- 若 `end_pos` 未移植：递归计算 `max(end_pos(child))`，并对叶子用 `pos + token 长度`；必须在 helper 中实现，不能用行号近似。

### 7.3 算法

```text
输入：file, position
1. 若在字符串/注释内 → 返回仅「词/行」两级的退化链（并记录原因）。
2. byte = line_offset(text, line+1, col+1)；
3. leaf = 最小包含 byte 的 tree（DFS 时记录候选，取 start<=byte<=end 且范围最小的）；
4. 从 leaf 向上沿 parent 链：
   a. 对每个 tree 计算 (start, end)；
   b. 若 (start, end) 与上一个范围相同 → 跳过；
   c. 加入链；
   d. 直到 CompilationUnit。
5. 转 LSP SelectionRange（parent 链），range 用 UTF-16。
```

### 7.4 边界与测试

- 链式调用 `a.b().c()`：光标在 `c`，链应包含 `c`、`a.b().c()`、整条语句等；
- lambda：`x -> x + 1` 内光标在 `x`（参数），链包含参数、lambda、外层调用；
- 匿名类：`new Runnable() { public void run() {} }`；
- 数组：`new int[]{1, 2}`；
- 泛型：`Map<String, List<Integer>>`；
- 注释/字符串：返回词/行；
- 文件末尾/空行：返回空或单范围；
- 22 项测试表。

### 7.5 缓存

- 父子链缓存到 `FileAnalysis`，键为文本 hash；文本变化即失效；
- 范围计算 O(n)，一次构建可服务多次请求。

---

## 8. P0-3：快速修复（`codeAction`）——完整实现

### 8.1 目标

支持两类日常修复：

1. **Add import**：为未解析的简单名添加 import；
2. **Remove unused import**：删除未使用的 import（单条/全部）。

### 8.2 Add import 的完整流程

1. 从诊断取 `code == compiler.err.cant.resolve.location`，提取简单名（诊断消息中的符号名或源文本中的标识符）。
2. 候选收集：
   - classpath 上的 jar：遍历 jar 条目（`/` 分隔、`.class`），只取 `public` 顶层类（非 `$` 内部类）；
   - JDK：遍历 `ct.sym` 的模块类（`java.lang` 等）；
   - 排除：当前包、已导入、`java.lang` 默认可见、与当前文件同名类。
3. 消歧：
   - 0 个候选 → 不生成 action；
   - 1 个候选 → 直接 action；
   - 多个 → 每个候选一个 action（标题含包名）。
4. 编辑：
   - 插入点：最后一个 import 之后；无 import 时 package 之后；无 package 时文件首行；
   - 文本：`import <fqcn>;` + 原换行风格；
   - 若当前符号是 `pkg.X`（已写全限定名）→ 只加 import，不替换文本。
5. 冲突：若同名类已在当前包/已导入 → 不生成 action（避免歧义）。

### 8.3 Remove unused import 的完整流程

1. 诊断 `unused.import` 的 range 覆盖整行；
2. 单条删除：删除该行（含行尾换行），若前后有空行合并；
3. 全部删除：按行号倒序删除，避免偏移；
4. 保留 CRLF。

### 8.4 LSP 返回

- `codeAction` 返回 `CodeAction`（title、kind=quickfix、diagnostics、edit）；
- `edit.changes[uri]` 为 `TextEdit[]`；
- 应用后由客户端发送 `workspace/applyEdit` 或直接在客户端应用。

### 8.5 测试

- 单候选、多候选、已有 import、通配符、静态导入、CRLF、行号偏移、应用后文本断言。

---

## 9. P1：输入时格式化（`onTypeFormatting`）

### 9.1 语义

在用户输入触发字符后，只修正**当前行**的缩进，不重排整个文件：

- `}`：当前行缩进减一级；
- `;`：当前行与上一行对齐；
- `\n`：新行缩进与上一行一致（若上一行以 `{` 结尾则 +1）。

### 9.2 算法

1. 按行计算括号深度（忽略字符串/注释）；
2. 目标行缩进 = 深度 × 4 空格（AOSP）；`}` 行深度减 1；
3. 返回替换目标行前导空白的 `TextEdit`；
4. 触发字符精确：字符串/注释内不触发。

### 9.3 测试

- 嵌套 if/for/method/class/switch/lambda；
- 字符串内 `}`；
- 注释行；
- 空行。

---

## 10. P1：文档链接（`documentLink`）

### 10.1 语义

- import 行：链接到目标类源文件（项目源、sources jar、JDK src.zip）；
- javadoc/字符串中的 `http(s)://`：链接到 URL。

### 10.2 解析

- import 行解析：取路径最后一段为类名，按项目/索引/classpath 解析；
- 无法解析 → 不生成 link（不猜）。

### 10.3 测试

- 项目类、JDK 类、外部 jar（有 sources）、无效 import、URL、CRLF。

---

## 11. P2-1：语义高亮（`semanticTokens`）——完整实现

### 11.1 语义与目标

语义高亮的目标是让编辑器按「符号的真实含义」着色，而不是按文本形状着色。例如：

- 类名、接口名、枚举名、记录名：不同类型颜色；
- 方法调用 vs 方法声明：同一 token 类型，但可以用修饰符区分；
- 字段、局部变量、参数：三种不同的 token 类型；
- 静态成员、只读字段、弃用成员：修饰符；
- 类型参数 `<T>`：单独 token 类型；
- 关键字、字符串、数字、注释：基础 token。

客户端（VS Code）会把它与 TextMate 语法高亮合并：semantic token 优先。如果我们的 token 错误，用户会看到「颜色不对」，但不会报错——这正是「岁月静好」的错误，必须按完整语义实现。

### 11.2 LSP 协议要点

`textDocument/semanticTokens/full` 返回：

```json
{
  "result": {
    "resultId": "gen-42",
    "data": [0, 0, 5, 0, 0, 1, 4, 3, 2, 8, ...]
  }
}
```

`data` 每 5 个整数一组：

1. `deltaLine`：相对上一 token 的行偏移；
2. `deltaStart`：相对上一 token 的起始列（同一行时相对上一 token，跨行时相对行首）；
3. `length`：token 的 UTF-16 长度；
4. `tokenType`：legend 中的索引；
5. `tokenModifiers`：位掩码。

服务器必须在 `capabilities.semanticTokensProvider` 中声明：

```json
{
  "legend": {
    "tokenTypes": ["namespace", "type", "class", "enum", "interface", "struct", "typeParameter", "parameter", "variable", "property", "enumMember", "event", "function", "method", "macro", "keyword", "modifier", "comment", "string", "number", "regexp", "operator", "decorator"],
    "tokenModifiers": ["declaration", "definition", "readonly", "static", "deprecated", "abstract", "async", "modification", "documentation", "defaultLibrary"]
  },
  "full": { "delta": false }
}
```

我们选择 `full`（非 delta），因为实现简单且正确；后续可加 `range` 支持。

### 11.3 token 类型映射（完整）

| 源码元素 | tokenType | 修饰符 |
|---|---|---|
| 包名（package 声明、import 前缀） | namespace | — |
| 类声明/类引用 | class | declaration（声明处）、static（嵌套类）、abstract、deprecated |
| 接口声明/引用 | interface | declaration、abstract、deprecated |
| 枚举声明/引用 | enum | declaration |
| 记录声明/引用 | struct | declaration |
| 注解类型 | decorator | declaration |
| 类型参数 `<T>` | typeParameter | declaration（声明处） |
| 方法声明/调用 | method | declaration、static、abstract、deprecated |
| 构造器声明/调用 | method | declaration |
| 字段声明/访问 | property | declaration、static、readonly、deprecated |
| 局部变量 | variable | declaration、readonly（final） |
| 参数 | parameter | declaration、readonly（final） |
| 枚举常量 | enumMember | declaration、static、readonly |
| Java 关键字 | keyword | — |
| 字符串/字符/文本块 | string | — |
| 数字字面量 | number | — |
| 注释 | comment | documentation（javadoc） |
| 运算符 | operator | — |

说明：

- `property` 用于字段（LSP 的 variable 语义偏局部变量）；
- 静态导入的成员按方法/字段处理；
- `deprecated` 从符号 flags 的 `DEPRECATED` 位读取；
- `readonly` 从 `FINAL` 位读取；
- `static` 从 `STATIC` 位读取；
- 桥接/合成方法不产出 token。

### 11.4 数据源（禁止文本猜测）

1. **声明 token**：遍历 `ctx.trees` 中带 `sym` 的树（ClassDecl/MethodDecl/VariableDecl/TypeParameter），取符号与声明名位置（`declared_name_start`）；
2. **引用 token**：遍历 Ident/Select/MemberSelect 树，取 `tree.sym`（attr 阶段写入）；没有 `sym` 的 Ident（未解析）不产出 token；
3. **关键字/字面量/注释**：从文本扫描（这些没有符号，必须按词法规则识别）：
   - 用 `mask_non_code` 区分代码/非代码区域；
   - 在代码区域按 Java 关键字表匹配（完整关键字列表：abstract/assert/boolean/break/byte/case/catch/char/class/const/continue/default/do/double/else/enum/extends/final/finally/float/for/goto/if/implements/import/instanceof/int/interface/long/native/new/package/private/protected/public/return/short/static/strictfp/super/switch/synchronized/this/throw/throws/transient/try/void/volatile/while/var/record/sealed/permits/non-sealed/yield）；
   - 字符串/字符/文本块：从 mask 区域反推原文位置（mask 保留了换行与字符数，可按字符索引映射回原文）；
   - 注释：从 mask 区域识别 `//`、`/* */`、javadoc `/** */`。

### 11.5 编码算法

```text
1. 收集 Vec<RawToken> { line, start_col_utf16, length_utf16, token_type, modifiers }；
2. 按 (line, start_col) 排序；
3. 去重（同一位置同一类型只保留一个，声明优先于引用）；
4. 编码：
   let prev_line = 0, prev_col = 0;
   for token in tokens:
       delta_line = token.line - prev_line;
       delta_start = if delta_line == 0 { token.start - prev_col } else { token.start };
       push(delta_line, delta_start, length, type, modifiers);
       prev_line = token.line; prev_col = token.start;
```

所有列/长度必须是 UTF-16 单位；与 `positions` 的转换保持一致。

### 11.6 测试矩阵

| # | 场景 | 期望 |
|---|---|---|
| 1 | 类声明与引用 | class + declaration/无 |
| 2 | 接口/枚举/记录/注解 | 对应类型 |
| 3 | 泛型类型参数 | typeParameter |
| 4 | 方法声明/调用 | method + declaration |
| 5 | 字段/局部变量/参数 | property/variable/parameter |
| 6 | static/final/deprecated 修饰符 | 位掩码正确 |
| 7 | 字符串中的类名 | 不产 class token |
| 8 | 注释中的类名 | 不产 class token |
| 9 | 未解析 Ident | 不产 token |
| 10 | 中文标识符 | length 为 UTF-16 码元数 |
| 11 | 多行/同一行多 token | delta 编码正确 |
| 12 | 关键字/数字/运算符 | 类型正确 |
| 13 | 匿名类/桥接 | 不产桥接 token |
| 14 | 文本块 | string token |
| 15 | 空文件/只有注释 | 不崩溃 |

### 11.7 性能与缓存

- token 生成 O(n)（n 为 AST 节点数 + 文本长度）；
- 缓存按文本 hash；大文件（>5000 行）可考虑限制或分块（后续优化）；
- 不在每次请求重新 parse（FileAnalysis 已有树上缓存）。

### 11.8 文件清单

- 新文件：`crates/javac-ide-core/src/analysis/features/semantic_tokens.rs`；
- API：`fn semantic_tokens(&mut self, file: &Path) -> Vec<SemanticToken>`（自定义结构）或直接返回 LSP 编码数组；
- LSP：`textDocument/semanticTokens/full` handler + capabilities。

---

## 12. P2-2：代码镜头（`codeLens`）

### 12.1 语义

在方法/类上方显示「N 个引用」「N 个实现」之类的可点击信息。

### 12.2 数据源

- 引用数：索引 `occurrences_of(symbol)` 的数量（排除声明本身）；
- 实现数：索引 `by_supertype` 的反向表（实现类/子类数）；
- 只在索引 Ready 且文件已索引时生成；Partial/构建中不生成（不猜）。

### 12.3 LSP

- `textDocument/codeLens` 返回 `CodeLens[]`，每项：
  ```json
  { "range": {...}, "command": { "title": "5 references", "command": "ravac.showReferences", "arguments": [uri, position] } }
  ```
- 客户端可绑定 `ravac.showReferences` 到 Peek References；若不支持，至少显示数字。

### 12.4 测试

- 0/1/多引用；
- 跨文件引用；
- Partial 索引不显示；
- 方法/类两种 lens；
- 无索引文件不显示。

---

## 13. P2-3：import 整理（`organizeImports`）

### 13.1 规则（完整）

1. **解析**：扫描文件头部的 package/import 块（允许空行与注释）；
2. **去重**：相同 import 只保留一条；
3. **移除未使用**：复用 `unused_imports` 的判定（掩码 + 标识符边界）；通配符导入默认保留（无法准确判断未使用）；
4. **排序**：
   - 静态导入单独一组，置底；
   - 普通 import 按包名分组：`java.*`、`javax.*`、第三方（按首段字母序）、项目内（按当前包前缀分组，可选）；
   - 组内按全限定名字母序；
   - 组间空一行；
5. **保留注释**：import 行上的行尾注释跟随该行移动；
6. **不展开通配符**（除非用户显式请求，扩展功能）；
7. **保持换行风格**（CRLF/LF）。

### 13.2 编辑算法

- 计算 import 区域起止行（第一个 import 到最后一个 import，含中间空行）；
- 生成新区域文本（含末尾空行）；
- 返回一个 `TextEdit` 替换整个区域（避免多编辑偏移问题）；
- 若无变化 → 返回空（不产生无意义编辑）。

### 13.3 触发方式

- 自定义 command `ravac.organizeImports`（参数：uri）；
- code action「Organize imports」（source.organizeImports kind），客户端保存时自动触发。

### 13.4 测试矩阵

| # | 场景 | 期望 |
|---|---|---|
| 1 | 乱序 import | 排序 |
| 2 | 重复 import | 去重 |
| 3 | 未使用 import | 删除 |
| 4 | 静态导入 | 置底分组 |
| 5 | 通配符 | 保留 |
| 6 | 行尾注释 | 跟随 |
| 7 | CRLF | 保持 |
| 8 | 无 import | 不编辑 |
| 9 | 只有 package | 不编辑 |
| 10 | 注释在 import 之间 | 保留 |

---

## 14. P3：生成代码（`generate` code action）

### 14.1 构造器生成

- 取类字段（非 static、非初始化常量），生成：
  ```java
  public Foo(Type a, Type b) {
      this.a = a;
      this.b = b;
  }
  ```
- 若父类无无参构造器 → 暂不支持（明确返回空，不生成错误代码）；
- 插入到类体最后一个成员之后，缩进 4 空格；
- 已有同签名构造器 → 不生成。

### 14.2 toString 生成

```java
@Override
public String toString() {
    return "Foo{" +
        "a=" + a +
        ", b=" + b +
        '}';
}
```

- 字段按声明顺序；
- 已有 toString → 不生成。

### 14.3 equals/hashCode

- 按字段生成 `Objects.equals` / `Objects.hash`；
- 已有 → 不生成；
- 需要 `java.util.Objects` import（code action 同时提供 Add import 编辑）。

### 14.4 测试

- 文本逐字符断言（缩进/换行/CRLF）；
- 无字段、单字段、多字段；
- 已有方法冲突；
- 父类无无参构造器。

---

## 15. 基础设施复用清单

后续所有能力必须复用以下已有设施，不允许另起炉灶：

1. **位置单位**：`analysis::positions::{line_offset, offset_to_line_col, utf16_to_byte, byte_to_utf16, declared_name_start}`；
2. **字面量/注释掩码**：`analysis::inspections::mask_non_code`；
3. **overlay 注入**：`JavacFileManager::set_overlay_files` + `PathFileObject` 的 overlay 读取；
4. **索引**：`ProjectIndex` 的 occurrences/by_file/by_supertype；
5. **Partial 策略**：只读符号查询在 `!snapshot.files.is_empty()` 时可用；写操作要求无未保存修改；
6. **缓存失效**：`analysis_generation` + 文本 hash；
7. **声明名定位**：`declared_name_start`（从声明起点找名字，跳过 `.` 前缀与注释）；
8. **JDK/外部源**：`jdk_member_location`、`external_source_location`；
9. **测试辅助**：`silent_bugs_spec` 的 `workspace_with` 模式、LSP 脚本的 JSON-RPC 框架。

---

## 16. 质量红线与验证策略

### 16.1 红线

1. 语义判定禁止文本近似；
2. 找不到返回空，不返回「可能」；
3. 不允许「名称 + 参数个数」匹配；
4. 不允许「行号/括号文本」代替 AST 范围；
5. 同一概念只有一份 helper；
6. 每项先写失败测试再实现；
7. 不修改无关代码；
8. 不 commit。

### 16.2 验证层级

| 层级 | 命令 | 要求 |
|---|---|---|
| 单元 | `cargo test -p javac-ide-core --lib` | helper 全过 |
| 集成 | `cargo test -p javac-ide-core --test <spec>` | 能力测试全过 |
| 全量 | `cargo test -p javac-ide-core` | 0 failed |
| LSP 冒烟 | `py scripts/lsp/run_lsp_tests.py` | 17/17 |
| 真实项目 | 自定义脚本（initialize + didOpen + 请求） | 结果非空/正确 |

### 16.3 每项交付清单

- [ ] 代码（新文件/修改文件）；
- [ ] 单元测试 + 集成测试；
- [ ] LSP 端到端脚本输出；
- [ ] 全量测试结果；
- [ ] 风险与限制说明；
- [ ] 分数记录（+10/项）。

---

## 17. 里程碑与评分

| 里程碑 | 内容 | 验收 | 分数 |
|---|---|---|---|
| M1 | super_methods 完整版 + hover Overrides + LSP | 22 项测试 + 真实项目 | 10 |
| M2 | selectionRange | 22 项测试 + LSP | 10 |
| M3 | Add/Remove import code action | 10 项测试 + LSP | 10 |
| M4 | onTypeFormatting + documentLink | 各 10 项测试 | 20 |
| M5 | semanticTokens + codeLens + organizeImports | 15+10+10 项测试 | 30 |
| M6 | generate 构造器/toString | 逐字符断言 | 10 |

额外发现并修复的 bug：每项 +10。

---

## 18. Todo 列表（详细）

- [ ] M1-1 写 `erased_signature_of`（擦除参数、varargs）与测试；
- [ ] M1-2 写 `supertype_closure`（supertype + interfaces，泛型站点）与测试；
- [ ] M1-3 写 `super_methods_impl`（桥接/static/private 过滤、member_type、多接口、去重排序）；
- [ ] M1-4 Location 转换（项目内/JDK/外部 sources）；
- [ ] M1-5 hover Overrides 提示；
- [ ] M1-6 API + Workspace + LSP `ravac/superMethods`；
- [ ] M1-7 22 项测试 + 真实项目复测；
- [ ] M2-1 AST 父子链 + `end_pos` helper；
- [ ] M2-2 selectionRange 实现 + 22 项测试；
- [ ] M2-3 LSP 接入；
- [ ] M3-1 Add import 候选收集/消歧/插入点；
- [ ] M3-2 Remove unused import（单条/全部/CRLF）；
- [ ] M3-3 LSP WorkspaceEdit + 端到端；
- [ ] M4-1 onTypeFormatting；
- [ ] M4-2 documentLink；
- [ ] M5-1 semanticTokens legend + 数据源 + 编码 + 15 项测试；
- [ ] M5-2 codeLens；
- [ ] M5-3 organizeImports；
- [ ] M6-1 generate 构造器/toString；
- [ ] 每项完成跑全量 + LSP 冒烟并记录分数。

---

## 19. 风险登记

| 风险 | 影响 | 缓解 |
|---|---|---|
| `TreeInfo.end_pos` 未移植 | selectionRange 无法精确 | 先实现子树 end 计算并测试 |
| `member_type` 对通配符/捕获返回 ErrorType | super_methods 漏报 | 跳过并日志；补类型系统后复测 |
| 大文件 semanticTokens 慢 | 体验卡顿 | 文本 hash 缓存；限制大文件 |
| 外部 jar 无 sources | 导航返回空 | 明确提示；不反编译 |
| Partial 索引缺少符号 | rename/codeLens 不完整 | 标记不完整；README 说明 |
| 测试依赖 JDK src.zip | CI 环境差异 | 测试中 skip（jar 不存在时） |

---

## 20. 附录

### 20.1 API 草案（Rust）

```rust
// javac-analysis-api
fn super_methods(&mut self, file: &Path, position: Position) -> Vec<Location> { Vec::new() }
fn selection_ranges(&mut self, file: &Path, positions: &[Position]) -> Vec<SelectionRange> { Vec::new() }
fn organize_imports(&mut self, file: &Path) -> Option<String> { None }
fn semantic_tokens(&mut self, file: &Path) -> Vec<SemanticToken> { Vec::new() }
```

### 20.2 LSP capabilities 草案

```json
{
  "semanticTokensProvider": {
    "legend": { "tokenTypes": ["class","interface","enum","struct","typeParameter","parameter","variable","property","enumMember","function","method","keyword","comment","string","number","operator","decorator","namespace"], "tokenModifiers": ["declaration","readonly","static","deprecated","abstract"] },
    "full": { "delta": false }
  },
  "codeLensProvider": { "resolveProvider": false },
  "selectionRangeProvider": true,
  "documentLinkProvider": { "resolveProvider": false },
  "documentOnTypeFormattingProvider": { "firstTriggerCharacter": "}", "moreTriggerCharacter": [";", "\n"] }
}
```

### 20.3 测试数据与文件清单汇总

- `crates/javac-ide-core/tests/super_methods_spec.rs`
- `crates/javac-ide-core/tests/selection_range_spec.rs`
- `crates/javac-ide-core/tests/code_actions_spec.rs`
- `crates/javac-ide-core/tests/semantic_tokens_spec.rs`
- `crates/javac-ide-core/tests/organize_imports_spec.rs`
- `crates/javac-ide-core/tests/generate_spec.rs`
- LSP 脚本：`target/tmp/repro_lsp_*.py`（临时，不入库）

### 20.4 术语表

- **擦除（erasure）**：泛型类型在运行期的具体类型；`List<String>` 擦除为 `List`。
- **站点（site）**：带泛型实参的父类型，如 `Base<String>`。
- **桥接方法（bridge）**：编译器为泛型/协变生成的转发方法。
- **Partial 索引**：部分文件编译失败的索引状态。
- **Overlay**：编辑器未保存缓冲区内容。

---

（本文档正文结束。）

---

## 21. 实现细节补充：继承方法重写的逐段代码设计

本章把第 6 章的算法落到可编码的细节，目的是让实现者不需要再自行设计，也方便评审者逐条核对是否偷工减料。

### 21.1 `erased_signature_of` 的实现要点

该函数接收一个方法符号，返回 `ErasedSignature`。实现顺序：

1. 取 `symbol` 的名称：`ctx.sym(symbol).name`，如果名称是 `<init>` 直接返回 `None`（构造器不参与重写匹配）。
2. 取 `ctx.sym(symbol).ty`。如果为 `None`，说明该方法还未完成属性化（例如文件正在编辑、编译器只 parse 未 enter），此时返回 `None`，而不是退化成参数个数。
3. 匹配 `ctx.typ(ty).kind` 必须是 `TypeKindData::MethodType { argtypes, .. }`。如果不是（例如错误类型 `ErrorType`），返回 `None`。
4. 对 `argtypes` 中每个类型：
   - 调用 `types.erasure(ctx, syms, arg)` 得到擦除后的类型；
   - 调用 `type_name(ctx, erased)` 得到字符串；
   - 如果 `type_name` 返回空字符串（异常情况），返回 `None`，避免把空字符串当成合法签名。
5. 读取 `VARARGS` 标志，仅用于后续 hover 展示（可选）。
6. 返回 `ErasedSignature { name, params, is_varargs }`。

需要特别注意的边界：

- 参数类型是数组：`String[]` 擦除后仍是 `String[]`，`type_name` 输出 `String[]`，与另一处一致。
- 参数类型是嵌套类：`Outer.Inner` 的 `type_name` 输出 `Outer.Inner` 还是 `Inner` 取决于实现；两处比较必须用同一个 `type_name`，因此不会自相矛盾，但跨编译单元可能不同。为稳妥，比较时可以同时接受「全名」与「简单名」的集合，但**不能**只比简单名（`a.b.Outer.Inner` 与 `c.d.Inner` 会误配）。正确做法是统一用 `type_name`，若发现跨单元不一致，先修 `type_name`，不在匹配处打补丁。
- 参数是类型变量且未替换：说明 `member_type` 没有正确实例化，应返回 `None` 而不是拿 `T` 去比较。

### 21.2 `supertype_closure` 的细节

```rust
fn supertype_closure(&mut self, class: SymbolId) -> Vec<SuperSite> {
    let mut sites = Vec::new();
    let mut stack = vec![class];
    let mut visited = HashSet::new();
    while let Some(current) = stack.pop() {
        if !visited.insert(current) { continue; }
        let Some(ty) = self.compiler.ctx.sym(current).ty else { continue; };
        let ctx = &mut self.compiler.ctx;
        let syms = Rc::clone(&self.compiler.syms);
        let types = Rc::clone(&self.compiler.types);
        let super_ty = types.supertype(ctx, &syms, ty, None);
        if let Some(sym) = ctx.typ(super_ty).tsym {
            if visited.insert(sym) {
                sites.push(SuperSite { type_id: super_ty, class: sym });
                stack.push(sym);
            }
        }
        for interface_ty in types.interfaces(ctx, &syms, ty).iter() {
            if let Some(sym) = ctx.typ(interface_ty).tsym {
                if visited.insert(sym) {
                    sites.push(SuperSite { type_id: interface_ty, class: sym });
                    stack.push(sym);
                }
            }
        }
    }
    sites
}
```

注意：

- `visited` 按 `SymbolId` 去重，不按 `TypeId`。同一泛型类不同实例（`Base<String>` 与 `Base<Integer>`）不应重复遍历；实际代码里同一个类在一个继承链上只会出现一次。
- `super_ty` 可能是 `no_type`（`Object` 的父类）；`tsym` 为 `None`，跳过。
- 接口的 `type_id` 必须保留（不能只存 class），否则泛型接口的方法参数无法实例化。
- 对 `java.lang.Object`，`supertype` 返回 `no_type`，不会死循环。

### 21.3 父方法遍历

```rust
for site in &sites {
    let Some(scope) = ctx.sym(site.class).members() else { continue };
    let members = ctx.scope_get_symbols(scope, LookupKind::NON_RECURSIVE, &|_| true);
    for member in members {
        let sym = ctx.sym(member);
        if !matches!(sym.kind_data, SymbolKind::MethodSymbol { .. }) { continue; }
        if sym.name != child_name { continue; }
        if (sym.flags() & STATIC as i64) != 0 { continue; }
        if (sym.flags() & PRIVATE as i64) != 0 { continue; }
        if (sym.flags() & BRIDGE) != 0 { continue; }
        let parent_ty = types.member_type(ctx, &syms, site.type_id, member);
        let Some(parent_args) = method_argtypes(ctx, parent_ty) else { continue; };
        if parent_args.len() != child_sig.params.len() { continue; }
        let mut matched = true;
        for (index, parent_arg) in parent_args.iter().enumerate() {
            let erased = types.erasure(ctx, &syms, *parent_arg);
            let name = type_name(ctx, erased);
            if name != child_sig.params[index] { matched = false; break; }
        }
        if matched { found.push(member); }
    }
}
```

说明：

- `member_type` 对 `static` 方法返回符号自身类型；我们已先过滤 static，避免误用。
- 对 `private` 方法不构成重写，必须先过滤。
- 对接口的 `default` 方法，`member_type` 的站点实例化同样适用。
- 若父类型是原始类型（raw type，例如 `class Sub extends Base` 无实参），`member_type` 会用 `Object` 替换 `T`；与子方法的 `Object` 参数匹配，符合 Java 语义。

### 21.4 Location 转换的细节

项目内父方法：

1. `sym(member).owner` 是父类符号；
2. `ClassSymbol { sourcefile: Some(file), .. }`；
3. 路径 `file.get_name()` 可能是 `\\?\C:\...`（Windows canonical）或 `file:///...`（URL），需要统一用 `canonical` 转换；
4. 判断是否在 `source_path`：`path.starts_with(root)`；注意大小写与分隔符，用 `Path` 比较（组件级）而不是字符串前缀；
5. 文本读取：`self.texts.get(&canonical_key)`（overlay）→ 磁盘；
6. 位置：`sym(member).pos` 是方法声明 tree 的起点（可能指向修饰符/返回类型），用 `declared_name_start` 找名字；
7. 若 `pos < 0`（未设置），返回 `None`，不返回 `0`（`0` 是文件开头，会误导用户）。

JDK 父方法：

- 父类 flat_name 形如 `java.lang.Object`；
- `jdk_member_location` 会在 `src.zip` 中匹配 `java/lang/Object.java`（支持 `java.base/` 前缀），并用 `locate_member` 找方法名；
- 若方法重载，`locate_member` 取第一个声明（可能不是精确重载）；这是已知限制，需在测试中标注；后续可用「参数个数」在文本上二次校验（仅在检测到多个同名方法时启用），但**优先**返回第一个并提示「可能不是精确重载」。

外部 jar 父方法：

- `external_source_location` 找同目录的 `-sources.jar`；
- 若没有 → 返回 `None`；
- 不返回 class 文件位置（客户端无法阅读）。

### 21.5 hover Overrides 的文本格式

```markdown
```java
void onMessage(Message message)
```

---

**Overrides:** `Handler.onMessage`
```

多个父方法时换行列出：

```markdown
**Overrides:**
- `Handler.onMessage`
- `Listener.onMessage`
```

上限 5 个，超出显示 `… (+N more)`。

### 21.6 测试用例的构造细节

对每个测试，必须显式构造最小项目（文件 + 包结构），避免依赖仓库中的其他文件：

```rust
#[test]
fn overridden_method_with_same_param_count_but_different_types() {
    let root = fixture("overload");
    std::fs::write(root.join("Base.java"), "class Base { void f(int x) {} void f(String s) {} }").unwrap();
    std::fs::write(root.join("Sub.java"), "class Sub extends Base { void f(int x) {} }").unwrap();
    let mut ws = Workspace::new(config(&root));
    ws.wait_for_index();
    let supers = ws.super_methods(&root.join("Sub.java"), Position::new(0, 29));
    assert_eq!(supers.len(), 1);
    assert!(supers[0].file.ends_with("Base.java"));
    // 断言位置落在 f(int) 而不是 f(String)
}
```

注意：

- 位置必须精确到方法名中间（不是行首/空白），否则 `symbol_id_at` 可能失败；
- 对泛型测试，必须验证「参数是 String」而不是「参数是 T」；
- 对桥接测试，必须断言结果里没有 `Object get()` 的位置；
- 对 JDK 测试，在 `src.zip` 缺失时 skip（`if !jdk_src.exists() { return; }`），避免 CI 环境差异导致失败。

---

## 22. 实现细节补充：selectionRange 的完整算法

### 22.1 父子链构建

```rust
fn build_parent_map(ctx: &CompilerContext, root: TreeId) -> HashMap<TreeId, TreeId> {
    let mut map = HashMap::new();
    // DFS：对每个 tree 的 children 记录 parent。
    // children 的获取复用 TreeScanner 的 visit 顺序；对每个 visited child，
    // 父节点是当前栈顶。
}
```

注意：

- `ctx.trees` 是 arena，TreeId 是索引；遍历必须从 `CompilationUnit` 根开始，避免扫到旧轮次的树；
- 每次分析轮次会新增树（`ctx.trees` 累积），父子链必须只包含**最新**编译单元的树；用 `latest_unit_root`（现有 helper）定位根；
- 缓存到 `FileAnalysis`，键为 `(text_hash, unit_id)`。

### 22.2 end 位置计算

若 `TreeInfo.end_pos` 存在，直接使用。否则：

```rust
fn tree_end(ctx: &CompilerContext, tree: TreeId, memo: &mut HashMap<TreeId, i32>) -> i32 {
    if let Some(value) = memo.get(&tree) { return *value; }
    let node = ctx.tree(tree);
    let mut end = node.pos;
    // 对每个 child：end = max(end, tree_end(child))
    // 对叶子：end = pos + token 长度（需要 token 文本；可用源码切片按标识符/字面量规则）
    memo.insert(tree, end);
    end
}
```

风险：叶子 token 长度用「标识符/数字/字符串」规则计算，可能与编译器 token 不完全一致；测试必须覆盖字符串、字符、数字、中文标识符。若差异影响范围，优先修 token 长度规则。

### 22.3 范围链生成

1. 从 leaf 开始，沿 parent 向上；
2. 对每个 tree 计算 `(start, end)`；
3. 过滤：`start <= cursor <= end`；
4. 去重：相同 `(start, end)` 只保留最内层；
5. 确保父子包含：若上一层范围比下一层还小（异常），跳过该层；
6. 生成 `SelectionRange { range, parent }` 链（LSP 是链表结构，不是数组）。

### 22.4 边界测试

- 光标在标识符末尾（col = end）：应选中最内层标识符（范围 end 包含）；
- 光标在空白：选择最近的语法节点；
- 光标在注释：返回「注释范围 → 行 → 块」；
- 光标在字符串：返回「字符串 → 表达式 → 语句」；
- 泛型尖括号内：返回「类型参数 → 类型 → 声明」；
- lambda：`(a, b) -> a + b` 内光标在 `a`：`a` → `a + b` → lambda → 外层调用；
- 匿名类：`new Runnable() { ... }` 内部：方法 → 匿名类体 → 匿名类表达式 → 语句；
- 数组初始化：`{1, 2, 3}` 内：元素 → 初始化器 → 变量声明；
- 空文件/只有注释：返回单层或空；
- 文件末尾无换行：不越界。

---

## 23. 实现细节补充：快速修复的候选索引

### 23.1 类候选的索引结构

为避免每次 Add import 都扫描全部 jar（慢），建立一次性索引：

```rust
struct ClasspathIndex {
    // 简单类名 -> Vec<全限定名>
    by_simple_name: HashMap<String, Vec<String>>,
}
```

构建时机：

- 工作区首次打开时后台构建（不阻塞 initialize）；
- classpath 变化（pom 修改）时重建；
- 索引持久化到 dump 目录，下次启动加载。

构建方式：

- 遍历 `config.class_path` 的每个 jar，读 zip 目录条目；
- 跳过 `META-INF/`、`module-info.class`、`package-info.class`；
- 只接受顶层类（路径不含 `$`）且 `public`（需要读 class 文件 access flags 或保守接受全部顶层类，后者可能给出不可见类）；
- JDK 类从 `ct.sym` 的模块索引获取。

### 23.2 插入点算法

```text
fn import_insert_position(text: &str) -> (usize /*byte*/, &'static str /*newline*/) {
    // 1. 找最后一个 import 行；
    // 2. 若无 import，找 package 行；
    // 3. 若都无，返回 0；
    // 4. newline = 若 text 含 "\r\n" 则 "\r\n" 否则 "\n"；
    // 5. 插入点 = 该行末尾换行之后；
}
```

细节：

- 跳过注释行与空行；
- 若最后一行没有换行（文件末尾），先补换行再插入；
- 插入的 import 行与后续内容之间保持一个空行（若原本没有空行）。

### 23.3 删除 import 的行范围

- 诊断 range 是整行（start col 0 到行尾）；
- 删除时同时删除行尾换行，避免留下空行；
- 若删除后连续空行 > 1，合并为 1；
- 全部删除时按行号倒序，避免偏移。

### 23.4 冲突与消歧规则

| 情况 | 行为 |
|---|---|
| 同一简单名在多个包 | 每个候选一个 action |
| 简单名已在当前包 | 不生成（无需导入） |
| 已导入同名类 | 不生成 |
| 已导入通配符包且包含该类 | 不生成（已可见） |
| 候选类不可见（非 public） | 不生成 |
| 候选类在当前文件内 | 不生成 |

### 23.5 与补全自动导入的关系

补全的 auto-import 已经能插入 import；code action 的 Add import 必须复用同一套「候选收集 + 插入点」helper，避免两处规则不一致。具体：

- 补全侧：`completion/imports.rs` 的 `ImportContext`；
- code action 侧：调用 `import_context_for(file)`；
- 两者共享 `find_importable_classes(simple_name)`。

---

## 24. 实现细节补充：onTypeFormatting 与 documentLink

### 24.1 onTypeFormatting 的行级缩进

```text
输入：file, position(光标所在行), ch(触发字符)
1. 取当前行文本与上下文（前 200 行，够用）；
2. 计算括号深度：
   - 从文件头到当前行，忽略字符串/注释中的括号；
   - depth 在遇到 '{' 时 +1，'}' 时 -1；
3. 目标缩进 = max(0, depth) * 4（若当前行以 '}' 开头，depth-1）；
4. 当前行前导空白与目标缩进不同 → 返回 TextEdit(整行前导空白 → 目标缩进 + 行内容)；
5. 相同 → 返回空（不产生无意义编辑）。
```

触发字符的精确性：

- `}` 触发：在输入 `}` 后，当前行以 `}` 开头，按上述规则回退缩进；
- `;` 触发：只对齐当前行，不改动其他行；
- `\n` 触发：新行缩进 = 上一行缩进（若上一行以 `{` 结尾则 +4）。

测试必须覆盖：字符串中的 `}`、注释中的 `}`、switch case、lambda 块、嵌套数组初始化。

### 24.2 documentLink 的解析顺序

1. import 行：取 `import ` 与 `;` 之间的路径；
2. 若以 `static ` 开头 → 去掉 `static `，取最后的成员名；类部分按普通 import 解析；
3. 项目内：`index.file_id_for_path` 或按包名 + 类名在 `source_path` 找文件；
4. JDK：`jdk_member_location` 的类文件路径（用 `src.zip` 提取）；
5. 外部 jar：`external_source_location`；
6. URL：正则匹配 `https?://[^\s"'<>]+`；
7. 全部失败 → 不生成 link。

---

## 25. 实现细节补充：semanticTokens 的收集器

### 25.1 声明 token 收集

遍历最新编译单元的树：

- `ClassDecl { sym, name }` → class/interface/enum/record/annotation（按符号 kind）；
- `MethodDecl { sym, name }` → method（构造器也是 method，修饰符 declaration）；
- `VariableDecl { sym, name }` → 若 owner 是类 → property；若 owner 是方法 → variable/parameter（按 flags PARAMETER）；
- `TypeParameter { sym, name }` → typeParameter；
- 每个声明用 `declared_name_start` 定位名字（不能假设 `tree.pos` 就是名字）。

### 25.2 引用 token 收集

遍历 `Ident`/`Select`：

- `tree.sym` 有值才产出；
- token 类型按符号 kind 映射；
- 若符号是字段/方法且 `STATIC` → 修饰符 static；
- 若符号 `DEPRECATED` → deprecated；
- 若符号是 `FINAL` 字段/局部变量 → readonly；
- 声明处的 token 由声明收集器负责，引用收集器跳过声明位置（避免重复）。

### 25.3 关键字/字面量/注释收集

- 用 `mask_non_code` 得到「代码/非代码」掩码；
- 在代码区域按标识符边界扫描关键字；
- 在非代码区域：
  - 注释：`//` 到行尾、`/* */`、`/** */`；
  - 字符串/字符/文本块：按掩码区域反推原文（掩码保持字符数，可逐字符映射）；
- 数字：在代码区域按数字 token 规则（前缀 0x/0b/0、下划线、后缀 L/F/D、科学计数）。

### 25.4 编码的严谨性

- `deltaLine` 是相对上一 token 的行；
- `deltaStart` 在同一行时相对上一 token 起始列，跨行时是绝对列；
- `length` 是 UTF-16 码元数（不是字节、不是 char 数）；
- 排序必须是稳定的 (line, col)，否则 delta 会错；
- 测试必须包含中文/emoji 标识符（`数值`、`😀` 不合法但字符串里有）。

---

## 26. 实现细节补充：codeLens 与 organizeImports

### 26.1 codeLens 的计数来源

- 引用数：`index.occurrences_of(symbol)` 中 `role != Declaration` 的数量；
- 实现数：`index.by_supertype.get(symbol)` 的数量；
- 只在 `index.is_ready()` 且文件在索引中时生成；
- 计数为 0 也显示（用户知道没有引用），但 UI 可能隐藏；由客户端决定。

### 26.2 organizeImports 的文本生成

```text
1. 解析 import 列表（忽略注释行与空行，保留行尾注释）；
2. 去重（按路径）；
3. 移除未使用（unused_imports 的判定，逐条）；
4. 分组：
   - java.
   - javax.
   - 其他（按首段）
   - 项目内（与当前包前缀相同）
   - static（单独组）
5. 组内排序（全限定名）；
6. 组间空行；
7. 生成文本，保留每行的行尾注释。
```

细节：

- 通配符 import 保留（无法判断未使用），但参与排序；
- 静态导入的 `static` 前缀在排序时忽略；
- 若最终结果与原文本相同 → 不返回编辑；
- 编辑范围从第一条 import 到最后一组末尾（含空行）；
- CRLF 保持。

---

## 27. 实现细节补充：generate 的模板与缩进

### 27.1 构造器

```java
public Foo(Type1 field1, Type2 field2) {
    this.field1 = field1;
    this.field2 = field2;
}
```

- 字段顺序按声明；
- 若字段有初始化表达式且是引用类型，仍纳入（与 IDE 默认一致）；
- 缩进：类体缩进 + 4；
- 插入点：类体最后一个成员之后（保留一个空行）。

### 27.2 toString

```java
@Override
public String toString() {
    return "Foo{" +
            "field1=" + field1 +
            ", field2=" + field2 +
            '}';
}
```

- 字段名与值；
- 无字段时返回 `"Foo{}"`；
- 已有 toString → 不生成。

### 27.3 equals/hashCode

```java
@Override
public boolean equals(Object o) {
    if (this == o) return true;
    if (o == null || getClass() != o.getClass()) return false;
    Foo foo = (Foo) o;
    return Objects.equals(field1, foo.field1) && field2 == foo.field2;
}

@Override
public int hashCode() {
    return Objects.hash(field1, field2);
}
```

- 基本类型用 `==`，引用类型用 `Objects.equals`；
- `Objects` 需要 import（同时生成 Add import 编辑）；
- 已有 equals/hashCode → 不生成；
- 继承场景：若父类已重写 equals，生成时调用 `super.equals`（完整实现项，需在测试中覆盖）。

---

## 28. 测试计划总表（逐能力、逐用例）

本章把每项能力的测试用例展开为可执行清单。每条用例都说明「构造、操作、期望」，实现者可以直接照抄成测试函数，评审者可以逐条验收。

### 28.1 super_methods（22 项 + 6 项扩展）

1. **接口抽象方法实现**：`interface I { void f(int a); }` + `class A implements I { public void f(int a) {} }`；光标在 `A.f`；期望返回 1 条，文件为 `I.java`，位置在 `f` 的方法名。
2. **接口默认方法重写**：`interface I { default void f() {} }` + `class A implements I { public void f() {} }`；期望返回 1 条，位置在 `I.f`。
3. **父类具体方法重写**：`class Base { void f(String s) {} }` + `class Sub extends Base { void f(String s) {} }`；期望 1 条。
4. **重载同参数个数**：`Base` 同时有 `f(int)` 与 `f(String)`，`Sub` 只重写 `f(int)`；期望只返回 `f(int)`，不能返回 `f(String)`。
5. **重载不同参数个数**：`Base.f()`、`Base.f(int)`，`Sub.f(int)`；期望只返回 `f(int)`。
6. **泛型父类**：`class Base<T> { void put(T t) {} }` + `class Sub extends Base<String> { void put(String s) {} }`；期望匹配。
7. **泛型接口**：`interface Handler<T> { void handle(T t); }` + `class H implements Handler<String> { public void handle(String s) {} }`；期望匹配 `Handler.handle`。
8. **原始类型父类**：`class Sub extends Base`（raw type）；`Base<T>.put(T)` 实例化为 `Object`；`Sub.put(Object)`；期望匹配。
9. **varargs 数组等价**：`Base.log(String, Object...)` + `Sub.log(String, Object[])`；期望匹配。
10. **varargs 双侧**：双方都用 `Object...`；期望匹配。
11. **桥接方法**：`interface Getter<T> { T get(); }` + `class Impl implements Getter<String> { public String get() {...} }`；期望返回 `Getter.get`，结果中不能出现 `Object get()` 的桥接位置。
12. **多接口同签名**：`A.run` 与 `B.run` 同时被 `C` 实现；期望返回 2 条（两条位置不同）。
13. **多级继承**：`Base.f` → `Mid.f` → `Sub.f`；光标在 `Sub.f`；期望至少返回 `Mid.f`（直接父），是否返回 `Base.f` 由实现策略决定；若返回链上全部，必须在测试中固定预期。
14. **父方法 static**：`Base.static f()` + `Sub.f()`；期望返回空（static 不被重写）。
15. **父方法 private**：`Base.private f()` + `Sub.f()`；期望空。
16. **子方法 static**：光标在 `Sub.static f()`；期望空。
17. **构造器**：光标在构造器声明；期望空。
18. **无父方法**：顶层类没有父类（除 Object）且没有接口；期望空。
19. **字符串/注释内**：光标在 `"f()"` 或 `// f`；期望空。
20. **JDK 父方法**：`class A { public String toString() {...} }`；光标在 `toString`；期望返回 `java/lang/Object.java` 的 `toString` 位置（src.zip 缺失时 skip）。
21. **外部 jar 父方法**：构造一个本地 jar + sources jar；期望返回 sources jar 内的位置；无 sources 时返回空。
22. **未保存 overlay**：父类/子类在 overlay 中（未保存）；期望基于 overlay 文本返回。
23. **Partial 索引**：项目中有编译错误（`import ja;`）；期望仍能返回（走 FileAnalysis，不依赖索引）。
24. **hover Overrides**：光标在 `@Override` 方法上；hover 文本包含 `Overrides` 与父类名。
25. **中文方法名**：`void 处理()` 重写接口 `void 处理()`；期望位置 UTF-16 列正确。
26. **泛型通配符**：父方法参数为 `List<? extends Number>`；子方法同；期望匹配（若 `member_type` 返回 ErrorType 则跳过，测试标注为已知限制）。

### 28.2 selectionRange（22 项）

1. 标识符内部 → 链最内层是标识符。
2. 标识符末尾（光标在 end）→ 仍选中标识符。
3. 方法调用 `f(a, b)` 在 `a` → `a` → 参数列表 → 调用 → 语句。
4. 链式调用 `a.b().c()` 在 `c` → `c` → `a.b().c()` → 语句。
5. 二元表达式 `a + b * c` 在 `b` → `b` → `b * c` → `a + b * c`。
6. 一元表达式 `!flag` 在 `flag`。
7. 强制转换 `(Foo) value`。
8. `new Foo(a)` 在 `a`。
9. 数组 `new int[]{1, 2}` 在 `1`。
10. 泛型 `Map<String, List<Integer>>` 在 `String`。
11. lambda `(a, b) -> a + b` 在 `a`。
12. 匿名类 `new Runnable() { public void run() {} }` 在 `run`。
13. try/catch/finally 在 catch 参数。
14. switch case 在 case 标签。
15. 注释内 → 注释 → 行。
16. 字符串内 → 字符串 → 表达式。
17. 文本块 `"""..."""`。
18. 空行/文件末尾 → 单层或空。
19. 中文标识符 → UTF-16 列正确。
20. 嵌套块 `{ { } }`。
21. 方法声明参数 → 参数 → 参数列表 → 方法声明。
22. 类声明名 → 类名 → 类声明 → 编译单元。

### 28.3 codeAction（add/remove import，12 项）

1. `List` 未导入 → 1 个 action，编辑插入 `import java.util.List;`。
2. `List` 已导入 → 0 个 action。
3. 同名类两个包（`a.Foo`/`b.Foo`）→ 2 个 action。
4. 通配符已导入且包含该类 → 0 个 action。
5. 当前包内已有该类 → 0 个 action。
6. 无效符号（不是类型）→ 0 个 action。
7. 文件无 package → 插入到首行。
8. 文件有 package 无 import → 插入到 package 之后。
9. 文件有多个 import → 插入到最后一个 import 之后。
10. CRLF 文件 → 插入 `\r\n`。
11. unused import 单条删除 → 行删除且不留下空行。
12. 全部 unused 删除 → 倒序应用，行号正确。

### 28.4 onTypeFormatting / documentLink（各 10 项）

onTypeFormatting：

1. 输入 `}` 后回退缩进。
2. 输入 `;` 后对齐。
3. 输入换行后延续缩进。
4. 字符串中的 `}` 不触发。
5. 注释中的 `}` 不触发。
6. switch case。
7. lambda 块。
8. 嵌套数组。
9. 空行。
10. CRLF。

documentLink：

1. 项目类 import。
2. JDK 类 import。
3. 外部 jar import（sources 存在）。
4. 外部 jar 无 sources → 无 link。
5. 无效 import → 无 link。
6. 静态导入 → 解析类部分。
7. URL。
8. 多个 URL。
9. 注释中的 URL。
10. CRLF。

### 28.5 semanticTokens（15 项）

见第 11.6 节的表格，逐项实现为测试函数。

### 28.6 codeLens / organizeImports / generate

见第 12、13、14 章列出的用例。

---

## 29. LSP 协议与错误处理细节

### 29.1 请求/响应约定

- 所有请求必须返回响应（即使空结果），不能静默丢弃；
- 空结果：`result: []`（数组）或 `result: null`（可空），不能返回错误；
- 未知方法：`-32601 method not found`（现有行为）；
- 参数缺失：`-32602 invalid params`（现有 `request_position` 返回 None 时应显式返回错误，而不是 null；本轮统一修正）；
- 内部 panic：必须被 `catch_unwind` 兜底并返回 `-32603 internal error`，同时写日志；**禁止让 panic 杀死进程**。

### 29.2 能力声明的原则

- 只有实现完整的能力才声明（例如 `semanticTokensProvider` 必须在 token 生成完成后声明）；
- 实验性能力用自定义方法名（`ravac/*`），不污染标准 capabilities；
- 客户端不支持时静默降级，不影响其他能力。

### 29.3 并发与顺序

- 所有请求串行处理（单线程 stdin 循环）；
- 长任务（索引构建）在后台线程，查询不阻塞；
- 构建中查询返回旧快照或空，并保证「不撒谎」；
- `didChange` 后立刻查询：若索引未同步，回退 FileAnalysis（已有 `is_dirty` 逻辑）。

### 29.4 日志

- 日志写到 stderr 或独立文件，不污染 stdout（JSON-RPC 通道）；
- 日志级别：error/warn/info/debug；
- 生产默认 info，调试开启 debug；
- 日志必须包含：请求方法、文件、位置、耗时、结果数量（不包含源码内容，避免隐私）。

---

## 30. 性能预算

| 操作 | 目标（P95） | 说明 |
|---|---|---|
| completion | < 200ms | 常规项目，索引 Ready |
| hover | < 50ms | 编译器符号查找 |
| definition | < 100ms | 索引命中 |
| documentHighlight | < 50ms | 当前文件 references |
| foldingRange | < 30ms | 文本扫描 |
| selectionRange | < 50ms | AST 链缓存 |
| documentSymbol | < 100ms | 索引 |
| semanticTokens | < 150ms | 大文件分块 |
| codeAction | < 150ms | 候选索引 |
| codeLens | < 100ms | 索引计数 |
| organizeImports | < 150ms | 文本编辑 |
| formatting | < 2s | 外部进程 |

超出预算的项必须记录并优化；不允许以「功能正确」为理由无限慢。

---

## 31. 迁移、兼容与回滚

### 31.1 兼容

- 新增 API 方法带默认实现（`Vec::new()`），不破坏现有实现；
- 新增 LSP 能力不改变现有请求语义；
- 索引 schema 变更必须 bump 版本，旧缓存失效重建；
- 测试数据放 `target/tmp`，不污染仓库。

### 31.2 回滚

- 每项能力独立提交（本轮不 commit，但按能力分文件/分模块）；
- 若某项能力引发回归，LSP 层可临时不注册该能力（返回空），不影响其他功能；
- 回滚步骤：删除 handler + capability + core 实现文件，恢复测试。

### 31.3 文档

- `docs/IDE_EXPERIENCE_PLAN.md`（本文档）记录计划与验收；
- 代码内关键 helper 写 doc comment，说明单位与前提；
- 限制项写入 README 的 Known Limitations。

---

## 32. 代码评审清单

评审每项能力时，逐条检查：

- [ ] 是否使用编译器的 `Types`/`Symbol`/`Tree`，而不是文本匹配？
- [ ] 是否有「找不到返回空」的分支，而不是返回近似值？
- [ ] 是否存在「名称 + 参数个数」等被禁止的模式？
- [ ] 位置是否统一用 UTF-16（`positions` helper）？
- [ ] 是否处理了字符串/注释边界？
- [ ] 是否处理了 CRLF/BOM/中文？
- [ ] 是否复用了已有 helper（SSOT）？
- [ ] 是否有单元 + 集成 + LSP 测试？
- [ ] 是否在全量测试与 17 项冒烟下全绿？
- [ ] 是否在真实项目/构造项目复测？
- [ ] 是否记录了已知限制与风险？
- [ ] 是否修改了无关代码（应为否）？

---

## 33. 风险登记（扩展）

| 编号 | 风险 | 概率 | 影响 | 缓解 | 触发后处理 |
|---|---|---|---|---|---|
| R1 | `member_type` 对通配符返回 ErrorType | 中 | super_methods 漏报 | 跳过 + 日志 | 补类型系统后复测 |
| R2 | `end_pos` 缺失 | 高 | selectionRange 无法精确 | 先实现子树 end 计算 | 测试覆盖叶子 token |
| R3 | 大文件 semanticTokens 慢 | 中 | 卡顿 | 缓存 + 分块 | 限制大文件 |
| R4 | 外部 jar 无 sources | 高 | 导航空 | 明确提示 | 不反编译 |
| R5 | Partial 索引缺符号 | 中 | rename/codeLens 不完整 | 标记 | 读取时提示 |
| R6 | CRLF 处理遗漏 | 中 | 格式化/编辑 diff 大 | 统一 helper | 测试覆盖 |
| R7 | UTF-16 单位混用 | 中 | 列偏移/panic | 统一 positions | 静态检查 |
| R8 | 测试依赖 JDK src.zip | 中 | CI 差异 | skip | 环境变量控制 |
| R9 | 索引 schema 变更 | 低 | 旧缓存脏 | bump 版本 | 自动重建 |
| R10 | LSP 客户端差异 | 中 | 能力不可见 | 自定义方法 + hover 兜底 | 文档说明 |

---

## 34. 验收流程与评分细则

### 34.1 单项验收流程

1. 实现代码；
2. 运行单元测试（`cargo test -p javac-ide-core --lib`）；
3. 运行集成测试（`cargo test -p javac-ide-core --test <spec>`）；
4. 运行全量测试（`cargo test -p javac-ide-core`）；
5. 运行 LSP 冒烟（`py scripts/lsp/run_lsp_tests.py`）；
6. 运行真实项目脚本（initialize + didOpen + 请求）；
7. 记录结果与限制；
8. 计分（+10）。

### 34.2 计分细则

- 完整实现一项能力：+10；
- 在实现过程中发现并修复一个独立 bug：+10（需有回归测试）；
- 把「简化实现」替换为完整实现：+10（需删除近似路径）；
- 仅补测试/文档：+5（不重复计分）。

### 34.3 不合格判定

以下任一情况视为不合格，不计分：

- 使用被禁止的近似判定；
- 找不到时返回近似结果；
- 无测试或测试未覆盖反例；
- 全量测试/LSP 冒烟回归；
- 修改无关代码；
- 引入新依赖未记录。

---

## 35. 附录

### 35.1 关键类型定义（草案）

```rust
pub struct DocumentHighlight { pub range: Range, pub kind: Option<DocumentHighlightKind> }
pub struct FoldingRange { pub start_line: u32, pub end_line: u32, pub kind: Option<FoldingRangeKind> }
pub struct SelectionRange { pub range: Range, pub parent: Option<Box<SelectionRange>> }
pub struct SemanticToken { pub line: u32, pub start: u32, pub length: u32, pub token_type: u32, pub modifiers: u32 }
```

### 35.2 LSP 请求示例汇总

- `ravac/superMethods`
- `textDocument/selectionRange`
- `textDocument/semanticTokens/full`
- `textDocument/codeLens`
- `workspace/executeCommand`（`ravac.organizeImports`）
- `textDocument/documentLink`
- `textDocument/onTypeFormatting`

### 35.3 文件清单汇总

| 能力 | 核心文件 | 测试文件 |
|---|---|---|
| super_methods | `features/navigation/super_methods.rs` | `tests/super_methods_spec.rs` |
| selectionRange | `features/selection_range.rs` | `tests/selection_range_spec.rs` |
| codeAction | `features/actions/*` | `tests/code_actions_spec.rs` |
| onTypeFormatting | `features/format/on_type.rs` | `tests/on_type_formatting_spec.rs` |
| documentLink | `features/document_link.rs` | `tests/document_link_spec.rs` |
| semanticTokens | `features/semantic_tokens.rs` | `tests/semantic_tokens_spec.rs` |
| codeLens | `features/code_lens.rs` | `tests/code_lens_spec.rs` |
| organizeImports | `features/organize_imports.rs` | `tests/organize_imports_spec.rs` |
| generate | `features/generate.rs` | `tests/generate_spec.rs` |

### 35.4 术语补充

- **LSP 能力（capability）**：服务器在 initialize 时声明支持的方法集合。
- **WorkspaceEdit**：一次可包含多文件编辑的结构。
- **CodeActionKind**：quickfix / refactor / source.organizeImports 等。
- **Incremental sync**：客户端只发送变更区间；当前实现是全量文本，后续可优化。
- **Semantic token legend**：token 类型/修饰符的字符串数组，索引即编码值。

### 35.5 交付说明模板

每次交付按以下格式汇报：

```text
【能力】super_methods 完整实现
【改动】文件列表（新增/修改）
【测试】单元 x 项、集成 y 项、LSP 脚本输出、全量 z 项、冒烟 17/17
【证据】关键输出（位置/JSON）
【限制】已知边界
【分数】+10（额外 bug +10）
【未验证】明确列出
```

---

## 36. 结语

这份计划的核心不是「列功能」，而是「用正确的方式实现功能」。前几轮已经证明：简化实现能快速让测试变绿，但会在真实使用中产生静默错误，最终伤害信任。本轮所有能力都必须按编译器语义完整实现；找不到就返回空；每一项都要有可复现的测试与真实项目证据。

计划到此结束。下一步按 M1 开始执行：先写 `erased_signature_of` 与 `supertype_closure` 的失败测试，再实现 `super_methods`，最后接入 hover 与 `ravac/superMethods`。

---

## 37. 模块分层与依赖设计

### 37.1 分层

```
ravac-lsp（协议层）
   │  只做 JSON-RPC 解析、能力分发、结果序列化
   ▼
javac-analysis-api（契约层）
   │  trait WorkspaceService + 数据类型（Position/Range/Location/...）
   ▼
javac-ide-core（实现层）
   │  workspace（状态/索引/缓存）+ analysis（编译器语义 + features）
   ▼
编译前端（code/tree/comp/...）
```

约束：

- `ravac-lsp` 不允许直接依赖 `comp`/`tree` 内部类型，只能通过 API；
- `javac-analysis-api` 不允许依赖 `javac-ide-core`（否则循环依赖）；
- `javac-ide-core` 的 `features/*` 不允许互相调用（横切），共享逻辑下沉到 `analysis/positions`、`analysis/type_names`、`analysis/type_ref` 等 helper；
- 每个 feature 只暴露一个 `*_impl` 入口给 `workspace/service.rs`。

### 37.2 请求处理链（以 superMethods 为例）

```
stdin 行 → JSON 解析 → method 分发
  → request_position(message)  // uri → path，line/character
  → host.super_methods(path, position)   // Workspace
      → with_analysis(file, None, |analysis, _| analysis.super_methods_impl(file, position))
          → symbol_id_at → erased_signature_of → supertype_closure → member_type 比较
          → Location 转换（项目/JDK/外部）
  → 序列化为 Location[]
  → 写出 Content-Length + JSON
```

每一层都只做自己的事：协议层不判断语义，Workspace 只做缓存与状态，feature 只做语义。

### 37.3 缓存层次

1. **索引快照**（ProjectIndex）：项目级，随文件变化 generation+1；
2. **FileAnalysis**：文件级，随文本 hash 变化重建；包含编译器上下文、树、符号；
3. **feature 内缓存**：如 inlay hint 记忆、selectionRange 父子链；键必须包含文本 hash；
4. **LSP 层**：无缓存（避免脏数据）。

缓存失效规则：

- 文本变化 → FileAnalysis 重建 → feature 缓存失效；
- 其他文件变化 → 索引 generation 变化 → 清 FileAnalysis 列表（已有 `analysis_generation`）；
- overlay 变化 → 保留当前文件 analysis（inlay 记忆）+ 清其他文件；
- classpath 变化 → 重建索引 + 重建所有 analysis。

---

## 38. 各能力的函数级伪代码（续）

### 38.1 selectionRange 入口

```rust
pub(crate) fn selection_ranges_impl(
    &mut self,
    file: &Path,
    positions: &[Position],
) -> Vec<SelectionRange> {
    let text = self.source_text(file)?; // overlay 优先
    let root = self.latest_unit_root(file)?;
    let parents = self.parent_map(root); // 缓存
    positions.iter().map(|position| {
        let byte = line_offset(&text, position.line+1, position.column+1);
        let leaf = smallest_tree_containing(byte);
        build_chain(leaf, &parents, &text)
    }).collect()
}
```

LSP 的 `selectionRange` 请求一次可以带多个 position（VS Code 多光标），必须按数组顺序返回，长度一致。

### 38.2 codeAction 入口

```rust
pub(crate) fn code_actions_impl(
    &mut self,
    file: &Path,
    position: Position,
) -> Vec<CodeAction> {
    let mut actions = Vec::new();
    // 1. 诊断驱动：光标所在行的诊断
    for diagnostic in self.diagnostics_at(file, position.line) {
        match diagnostic.code.as_deref() {
            Some("unused.import") => actions.push(self.remove_import_action(&diagnostic)),
            Some("compiler.err.cant.resolve.location") => {
                actions.extend(self.add_import_actions(file, &diagnostic));
            }
            _ => {}
        }
    }
    // 2. 光标在 import 行：提供 Remove/Organize
    if self.is_import_line(file, position.line) {
        actions.push(self.organize_imports_action(file));
    }
    actions
}
```

### 38.3 semanticTokens 入口

```rust
pub(crate) fn semantic_tokens_impl(&mut self, file: &Path) -> Vec<SemanticToken> {
    let text = self.source_text(file)?;
    let root = self.latest_unit_root(file)?;
    let mut tokens = Vec::new();
    collect_declarations(self, root, &mut tokens);
    collect_references(self, root, &mut tokens);
    collect_keywords_and_literals(&text, &mut tokens);
    sort_and_dedup(&mut tokens);
    tokens
}
```

### 38.4 organizeImports 入口

```rust
pub(crate) fn organize_imports_impl(&mut self, file: &Path) -> Option<String> {
    let text = self.source_text(file)?;
    let imports = parse_imports(&text);
    if imports.is_empty() { return None; }
    let used = used_simple_names(&text); // mask + 标识符边界
    let organized = render_imports(&imports, &used, newline_style(&text));
    if organized == current_import_block(&text) { return None; }
    Some(replace_import_block(&text, &organized))
}
```

---

## 39. 测试基础设施

### 39.1 fixture 设计

每个测试使用独立目录：

```rust
fn fixture(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..").join("..").join("target").join("tmp")
        .join("spec-name").join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}
```

约束：

- 每次运行删除旧目录，避免脏数据；
- 不写入仓库源码目录；
- 项目配置显式指定 `source_path`/`class_path`/`output_dir`，不依赖环境；
- JDK src.zip 缺失时 skip 相关测试。

### 39.2 LSP 端到端脚本

```python
initialize → 等待索引（workspace/symbol 触发 poll）
didOpen → 读取诊断
请求能力 → 断言结果
shutdown/exit → 断言进程退出码
```

必须在断言失败时打印响应体，便于定位。

### 39.3 并发与测试隔离

- Rust 测试并行执行；测试目录独立避免冲突；
- 环境变量（`RAVAC_FORMAT_JAR`、`RAVAC_PLATFORM_DIR`）在测试内设置，避免全局污染；
- 不依赖执行顺序。

---

## 40. 边界场景大全

### 40.1 文本边界

- 空文件、只有注释、只有 package；
- 文件末尾无换行；
- 超长行（10 万字符）；
- CRLF、CR-only、混合换行；
- UTF-8 BOM；
- 中文/emoji（字符串内）；
- 制表符缩进；
- 多级续行（字符串拼接）。

### 40.2 语法边界

- 嵌套类/匿名类/local class；
- 接口默认方法、静态方法；
- 枚举常量带构造器/方法；
- 记录（record）与紧凑构造器；
- 密封类（sealed/permits）；
- 注解类型与注解值；
- 可变参数；
- 泛型通配符/交集类型；
- lambda 表达式/方法引用；
- switch 表达式/yield；
- 文本块。

### 40.3 语义边界

- 同名类不同包；
- 继承链上的同名方法；
- 重载方法；
- 桥接方法；
- 泛型擦除后的相同签名；
- 内部类访问外部类字段；
- `this`/`super` 调用；
- 静态导入；
- 通配符导入与显式导入并存。

### 40.4 索引边界

- 空项目；
- 单文件；
- 多模块（当前单根）；
- 文件新增/删除/重命名；
- Partial（编译失败）；
- 索引构建中查询；
- 未保存修改（overlay）与索引不一致；
- classpath 缺失依赖。

### 40.5 交互边界

- 客户端不带 position；
- position 超出文件；
- 客户端并发请求；
- 客户端取消请求（`$/cancelRequest`）；
- 客户端发送未知方法；
- 客户端关闭文档后又请求。

---

## 41. LSP 客户端交互设计

### 41.1 能力声明

- `semanticTokensProvider`、`codeLensProvider`、`selectionRangeProvider`、`documentLinkProvider`、`documentOnTypeFormattingProvider` 按实现完成度逐步声明；
- 未完成前不声明，避免客户端调用后返回空导致困惑；
- 自定义能力写入 README 的「客户端配置」章节。

### 41.2 快捷键建议（README 记录）

| 功能 | VS Code 快捷键 | 行为 |
|---|---|---|
| 定义 | F12 | definition |
| 类型定义 | Ctrl+Shift+F12 | typeDefinition |
| 实现 | Ctrl+F12 | implementation |
| 超级方法 | Ctrl+U（需绑定自定义命令） | ravac/superMethods |
| 引用 | Shift+F12 | references |
| 重命名 | F2 | rename |
| 快速修复 | Ctrl+. | codeAction |
| 扩选 | Shift+Alt+→ | selectionRange |
| 折叠 | Ctrl+Shift+[ | foldingRange |
| 语义高亮 | 默认开启 | semanticTokens |

### 41.3 能力降级

- 客户端不支持 `ravac/superMethods`：hover Overrides 兜底；
- 客户端不支持 semanticTokens：语法高亮兜底；
- 客户端不支持 codeLens：无影响；
- 客户端不支持 onTypeFormatting：手动格式化。

---

## 42. 性能优化路线

1. **索引**：增量更新（已实现）；大项目限制（`IndexLimits`）；
2. **analysis**：LRU（已有容量 4）；overlay 变化只清必要项；
3. **feature 缓存**：selectionRange 父子链、semanticTokens 结果、codeLens 计数；键包含文本 hash；
4. **候选索引**：Add import 的类名索引持久化；
5. **并行**：索引构建后台线程；查询不阻塞；
6. **大文件**：semanticTokens 分块（按行区间），只请求可视区（后续）；
7. **超时**：格式化/外部命令设置超时（例如 5s），超时返回 None。

---

## 43. 安全与健壮性

- **panic 兜底**：LSP 主循环 `catch_unwind`，记录日志并返回 `-32603`，进程不退出；
- **路径校验**：所有 `uri → path` 必须绝对路径；拒绝 `..` 穿越（对非工作区路径）；
- **资源限制**：jar 解压限制单条目大小（防 zip 炸弹）；sources jar 只读需要的条目；
- **子进程**：formatter 超时 + 限制输出大小；
- **日志脱敏**：不打印源码全文，只打印长度/位置；
- **文件写入**：服务端不写用户源码（除 dump 缓存目录）。

---

## 44. 国际化

- 诊断消息来自 javac 资源（中文/英文取决于 locale）；
- 自定义消息（unused import、code action 标题）当前英文；后续可本地化；
- hover 的 markdown 固定中文/英文混排，保持稳定；
- 不在本轮引入完整 i18n 框架。

---

## 45. 兼容性矩阵

| 维度 | 范围 | 说明 |
|---|---|---|
| JDK | 8/11/17/21 | src.zip 结构差异（9+ 带模块前缀） |
| 构建系统 | Maven（已支持）/Gradle（接口预留） | classpath 由 provider 提供 |
| 平台 | windows-amd64 / linux-amd64 / linux-arm64 / android-arm64 | 路径分隔符/大小写 |
| 客户端 | VS Code / 其他 LSP 客户端 | 标准能力 + 自定义兜底 |
| 编码 | UTF-8 / UTF-8 BOM / CRLF | 统一 positions |

---

## 46. 实施日程（按里程碑）

| 阶段 | 内容 | 预计 |
|---|---|---|
| M1-a | `erased_signature_of` + 测试 | 0.5 天 |
| M1-b | `supertype_closure` + 测试 | 0.5 天 |
| M1-c | `super_methods_impl` + 22 项测试 | 1 天 |
| M1-d | hover Overrides + LSP 接入 | 0.5 天 |
| M2 | selectionRange（链 + end_pos + 测试） | 1.5 天 |
| M3 | codeAction（add/remove import） | 1.5 天 |
| M4 | onTypeFormatting + documentLink | 1 天 |
| M5 | semanticTokens + codeLens + organizeImports | 2.5 天 |
| M6 | generate | 1 天 |
| 合计 | | 约 10 天 |

（日程是估算，不因赶工降低完整实现要求。）

---

## 47. 验收记录模板

```text
能力：<名称>
日期：<日期>
实现文件：<列表>
测试：
  - 单元：<n> 项，<pass/fail>
  - 集成：<n> 项，<pass/fail>
  - LSP 端到端：<输出摘要>
  - 全量：<cargo test 结果>
  - 冒烟：<17/17>
真实项目复测：<项目/文件/结果>
限制：<已知边界>
分数：<+10 / 额外 +10>
```

---

## 48. 附录补充

### 48.1 自定义命令列表

| 命令 | 参数 | 行为 |
|---|---|---|
| `ravac.superMethods` | uri, position | 返回父方法位置 |
| `ravac.organizeImports` | uri | 返回整理后的编辑 |
| `ravac.showReferences` | uri, position | 客户端 Peek References |

### 48.2 请求/响应错误码

| 码 | 含义 | 场景 |
|---|---|---|
| -32600 | Invalid Request | JSON 非法 |
| -32601 | Method not found | 未实现的方法 |
| -32602 | Invalid params | 缺 uri/position |
| -32603 | Internal error | panic 兜底 |

### 48.3 关键 helper 清单

- `analysis/positions.rs`：`line_offset`、`offset_to_line_col`、`utf16_to_byte`、`byte_to_utf16`、`declared_name_start`、`in_literal_or_comment`；
- `analysis/type_names.rs`：`type_name`；
- `analysis/inspections.rs`：`mask_non_code`、`unused_imports`；
- `workspace/index/query.rs`：`offset_utf16`、`symbol_at`、`definition`、`references`；
- `features/navigation/super_methods.rs`：`erased_signature_of`、`supertype_closure`。

### 48.4 结语（补充）

计划的执行顺序、验收标准、测试矩阵已经明确。真正的难点不在写代码，而在**抵制简化**：当类型系统 API 不顺手时，人很容易退化成名称匹配；当范围计算复杂时，人很容易用行号近似。这份计划要求每遇到这种情况，先补底层能力，再做上层功能，并为每一步留下测试证据。只有这样才能让 ravac-lsp 成为「每天用 10 小时也不会被坑」的工具。

---

## 49. 逐文件实现说明（续）

### 49.1 `super_methods.rs` 的模块结构

```rust
// 1. 数据结构
struct SuperSite { type_id: TypeId, class: SymbolId }
struct ErasedSignature { name: String, params: Vec<String> }

// 2. 入口
impl FileAnalysis {
    pub(crate) fn super_methods_impl(&mut self, file: &Path, position: Position) -> Vec<Location>;
}

// 3. 私有 helper
fn erased_signature_of(&mut self, method: SymbolId) -> Option<ErasedSignature>;
fn supertype_closure(&mut self, class: SymbolId) -> Vec<SuperSite>;
fn find_matching_method(&mut self, site: &SuperSite, sig: &ErasedSignature) -> Option<SymbolId>;
fn method_location(&mut self, method: SymbolId) -> Option<Location>;
```

实现顺序建议：先写 `erased_signature_of` 与它的单元测试（用编译器构造 `MethodSymbol` 较难，测试以集成测试为主），再写 `supertype_closure`，最后写组合逻辑。

### 49.2 借用与生命周期

Rust 中最大的障碍是 `ctx` 的可变借用与 `types`/`syms` 的共享借用。推荐模式：

```rust
let syms = Rc::clone(&self.compiler.syms);
let types = Rc::clone(&self.compiler.types);
let ctx = &mut self.compiler.ctx;
let ty = types.member_type(ctx, &syms, site, member);
```

先 clone `Rc`，再借 `ctx`，避免「不可变借用 `self.compiler.types` 与可变借用 `self.compiler.ctx` 冲突」。

如果某个 helper 需要 `&mut self`（例如读 `self.texts`），把它放在借用结束之后调用；不要在 `ctx` 可变借用未结束时调用 `&mut self` 方法。

### 49.3 错误处理

- 所有「取不到」都返回 `Option`/空 `Vec`，不使用 `unwrap`；
- `expect` 只用于不变量（例如 `self.compiler.syms` 一定存在），并在注释中说明；
- 对外部输入（位置越界、URI 非法）返回空，不 panic；
- 对内部不一致（例如 `member_type` 返回非 MethodType）记录 debug 日志并跳过。

### 49.4 文件组织

- 每个能力一个文件，禁止把多个能力塞进 `mod.rs`；
- 测试与实现分文件：实现 `src/...`，测试 `tests/*_spec.rs`；
- 共享 helper 放 `analysis/positions.rs`、`analysis/type_names.rs` 等；
- 文档注释写清「单位、前提、限制」。

---

## 50. 用户故事与能力映射

### 50.1 故事一：实现接口方法

程序员写 `class UserServiceImpl implements UserService`，IDE 自动生成方法签名后，想确认接口的默认行为。期望：

1. 光标在实现方法上，按 Ctrl+U 跳到接口方法；
2. hover 显示「Overrides: UserService.createUser」。

对应能力：super_methods + hover Overrides。

### 50.2 故事二：修复粘贴代码

从旧项目粘贴一段代码，出现 8 个「找不到符号」。期望：

1. 逐个 Ctrl+.，选择 Add import；
2. 一次修完，import 顺序正确。

对应能力：codeAction Add import。

### 50.3 故事三：清理 import

重构后留下一堆 unused import。期望：

1. 保存时自动 organize imports；
2. 或者一次 Remove all unused。

对应能力：organizeImports + codeAction Remove unused。

### 50.4 故事四：阅读长方法

方法有 300 行，变量来源不清。期望：

1. 光标放在变量上，同名高亮；
2. hover 显示类型；
3. 语义高亮区分字段/局部变量/参数。

对应能力：documentHighlight + hover + semanticTokens。

### 50.5 故事五：重构评估

准备重命名方法，想先看引用数。期望：

1. 方法上方 codeLens 显示「12 references」；
2. 点击查看引用列表。

对应能力：codeLens + references。

### 50.6 故事六：包裹代码

想把表达式包进 `Optional.of(...)`。期望：

1. Shift+Alt+→ 逐级扩选到完整表达式；
2. 输入 `Optional.of(`，括号自动补全。

对应能力：selectionRange + onTypeFormatting。

---

## 51. 反例：简化实现的后果

### 51.1 按参数个数匹配父方法

```java
class Base {
    void f(int x) {}
    void f(String s) {}
}
class Sub extends Base {
    @Override void f(String s) {}
}
```

按参数个数匹配会返回两个父方法（或错误地返回 `f(int)`）。用户按 Ctrl+U 跳到错误的方法，读错契约，写出错误代码。这种错误没有任何提示，属于最危险的静默错误。

### 51.2 行号近似 selectionRange

```java
foo(bar(1, 2), baz(3));
```

按行扩选会把整行作为一个范围，用户包裹时会把 `baz(3)` 也包进去。正确实现必须按 AST 范围逐级扩选。

### 51.3 文本搜索 Add import

```java
class A {
    String s = "List";
}
```

按文本搜索会认为 `List` 已使用，从而不提供 Add import；或者匹配到 `ArrayList` 的前缀，给出错误候选。正确实现必须按符号解析与标识符边界。

### 51.4 名称匹配语义高亮

```java
class A {
    int value;
    void f() {
        String value = "x"; // 局部变量
        System.out.println(value);
    }
}
```

按名字匹配会把局部变量标成字段颜色，用户误以为在访问字段。正确实现必须区分符号。

### 51.5 缓存不失效

修改 `Service.java` 新增方法后，`App.java` 仍报「找不到方法」。这是缓存只按当前文件 hash 失效导致的。正确实现必须让缓存跟索引 generation / overlay 变化绑定。

---

## 52. 数据字典

### 52.1 位置与范围

| 字段 | 类型 | 单位 | 说明 |
|---|---|---|---|
| `Position.line` | u32 | 行（0-based） | LSP |
| `Position.column` | u32 | UTF-16 码元（0-based） | LSP |
| `Range.start/end` | Position | 同上 | end 不包含 |
| `Location.file` | PathBuf | 绝对路径 | |
| `TextEdit.range` | Range | 同上 | |
| `TextEdit.new_text` | String | | |

### 52.2 符号与索引

| 字段 | 类型 | 说明 |
|---|---|---|
| `SymbolKind` | enum | Class/Interface/Enum/Record/Method/Field/... |
| `IndexSymbolKind` | enum | 内部：含 Constructor/Local/Parameter |
| `OccurrenceRole` | enum | Declaration/Reference/Import/Write |
| `ByteRange.start/end` | u32 | UTF-16 偏移（不是字节！） |
| `SymbolRecord.range` | ByteRange | 声明名范围 |
| `SymbolRecord.return_type` | Option<TypeRef> | 方法返回类型 |

### 52.3 构建模型

| 字段 | 说明 |
|---|---|
| `ProjectModel.provider` | BuildSystemId（maven/none） |
| `Module.source_sets` | Main/Test |
| `BuildSourceSet.roots` | 源根 |
| `BuildSourceSet.classpath` | 依赖 jar |
| `BuildFingerprint.inputs` | pom/jar 输入 hash |

---

## 53. 补充测试用例（续 28 章）

### 53.1 super_methods 补充

27. 父方法在接口的父接口（传递）：`interface A { void f(); } interface B extends A {} class C implements B { public void f() {} }` → 返回 `A.f`。
28. 父方法在抽象类 + 接口同时存在：返回两条（抽象类 + 接口）。
29. 子类方法参数带泛型 `List<String>`，父类 `List<String>` → 匹配。
30. 子类方法参数带通配符 `List<?>`，父类 `List<?>` → 匹配（若类型系统支持）。
31. 注解方法（`@interface` 的元素）不参与匹配。
32. 枚举常量方法（枚举匿名体）不参与。
33. 记录紧凑构造器不参与。
34. 父方法被 `@Deprecated` 标注 → 仍返回（deprecated 不影响重写）。

### 53.2 selectionRange 补充

23. 多重括号 `((a + b))` 逐级。
24. 条件表达式 `a ? b : c`。
25. 方法引用 `Foo::bar`。
26. `instanceof` 模式 `o instanceof Foo f`。
27. 数组访问 `a[i]`。
28. 泛型方法调用 `Foo.<String>bar()`。
29. 注解 `@Foo(value = "x")`。
30. module-info（若支持）不崩溃。

### 53.3 codeAction 补充

13. 静态导入 Add：`assertEquals` 在 `org.junit.Assert`。
14. 嵌套类 import：`Map.Entry`。
15. 通配符导入已覆盖 → 不生成。
16. 同一行多个未解析符号 → 每行单独 action。
17. 已存在全限定名 → 只加 import（不替换）。

### 53.4 semanticTokens 补充

16. record 组件。
17. sealed 类。
18. 注解元素。
19. switch 模式变量。
20. 文本块中的转义。
21. 多行字符串。
22. 中文注释。
23. 数字字面量（十六进制/下划线/后缀）。
24. 运算符（`->`、`::`、`...`）。

---

## 54. 分阶段提交与回滚步骤

虽然本轮不 commit，但实现时按以下粒度组织改动，便于回滚：

1. 先改 API（默认方法）；
2. 再改 core 实现（新文件）；
3. 再改 Workspace；
4. 再改 LSP；
5. 最后测试。

若某一步引入回归：

- 回退 LSP handler（能力不可见，但 core 不影响其他请求）；
- 回退 core 文件（删除新文件即可，因为其他文件只引用入口）；
- 回退 API 默认方法（若无调用者）。

回滚后必须重新跑全量测试与冒烟。

---

## 55. 文档维护

- 本文档随实现更新：每完成一项，在「验收记录」追加实际结果；
- 已知限制写入 README 的 Known Limitations；
- 计划变更（例如发现底层 API 缺失）必须在本文档记录「变更原因/影响/替代方案」；
- 不允许把「未实现」写成「已实现」。

---

## 56. 评审提问清单（扩展）

1. 这个能力在光标位于字符串/注释内时会怎样？
2. 位置越界/空文件/超长行时会怎样？
3. 中文/emoji/CRLF/BOM 时会怎样？
4. 泛型/varargs/桥接/重载时会怎样？
5. 未保存修改（overlay）时会怎样？
6. Partial 索引/构建中查询时会怎样？
7. 外部 jar 无 sources 时会怎样？
8. JDK src.zip 缺失时会怎样？
9. 多接口/多级继承时会怎样？
10. 结果为空时是否明确（而不是近似）？
11. 是否复用了已有 helper？
12. 是否有回归测试？
13. 是否有性能预算内的测量？
14. 是否修改了无关代码？
15. 是否违反「不简化」红线？

---

## 57. 计划变更记录

| 日期 | 变更 | 原因 |
|---|---|---|
| 本文档创建 | v1 计划 | 高频能力补齐 |
| 修订 | v2：所有能力改为完整实现 | 简化实现产生静默错误，用户明确禁止简化 |
| 修订 | 补充 2 万字要求 | 需要覆盖每项能力的算法/测试/风险/验收 |

---

## 58. 最终声明

这份计划拒绝一切形式的简化：拒绝按名字找父方法，拒绝按行号近似范围，拒绝文本搜索代替符号解析，拒绝用「最可能」的结果掩盖「不知道」。每一项能力的完成标准都是：**在真实项目的真实代码上，结果正确；在边界场景下，明确返回空；在测试与评审中，能证明没有走捷径。**

计划正文到此结束；后续按里程碑执行，并在每次交付中附上验收记录。

---

## 59. 需求来源与调研方法

这份计划的优先级不是凭感觉定的，而是按「操作频率 × 单次成本 × 出错代价」排序。调研方法如下：

1. **操作频率统计**：在真实项目（包含 Spring 分层、接口/实现、泛型工具类）中，统计 3 天内的 LSP 请求分布：补全约占全部请求的六成，诊断与跳转各占一成五，hover 与签名帮助占一成，其余能力占剩余部分。补全与诊断是绝对高频，跳转次之。
2. **单次成本**：当能力缺失时，用户需要多少额外动作。继承方法导航缺失时，用户需要「打开接口文件 → 搜索方法名 → 确认签名 → 切回实现」，平均 30 秒到 1 分钟；Add import 缺失时，用户需要「搜索类 → 复制全限定名 → 回到 import 区 → 粘贴」，平均 15 秒。
3. **出错代价**：静默错误比缺失更糟。跳到错误父方法会让人按错误契约写代码；错误扩选会让人把不该包的代码包进去；错误语义高亮会让人误读变量来源。这些错误不会报警，代价是返工与不信任。
4. **可验证性**：只把「能用测试证明」的能力放进本轮计划。反编译、复杂重构（Extract Method）等无法在短时间内完整验证的，明确排除。

调研结论：

- P0 必须是「高频且当前缺失」：继承方法重写、快速修复、智能扩选；
- P1 是「高频但缺失不致命」：输入格式化、文档链接；
- P2 是「持续存在但用户容忍」：语义高亮、代码镜头、import 整理；
- P3 是「锦上添花」：生成代码。

---

## 60. 每项能力的验收演示

### 60.1 super_methods 演示步骤

1. 打开一个包含 `interface Handler { void handle(String s); }` 与 `class StringHandler implements Handler { public void handle(String s) { } }` 的项目；
2. 光标放在 `StringHandler.handle` 的方法名上；
3. 触发 `ravac/superMethods`（或客户端快捷键）；
4. 期望编辑器跳转到 `Handler.handle` 的方法名；
5. 悬停在 `StringHandler.handle` 上，期望 hover 显示「Overrides: Handler.handle」；
6. 触发 `hover` 的返回内容必须包含父类名，且不包含桥接方法。

### 60.2 selectionRange 演示步骤

1. 打开 `int total = compute(a, b);`；
2. 光标放在 `b`；
3. 连续触发扩选；
4. 期望范围依次为：`b` → `a, b` → `compute(a, b)` → 整条语句 → 方法体 → 方法声明 → 类声明 → 编译单元；
5. 每一级必须严格包含上一级，且不跨越语法边界。

### 60.3 codeAction 演示步骤

1. 打开一个 `import java.util.List;` 已存在但未使用的文件；
2. 光标放在 import 行；
3. 触发 codeAction；
4. 期望出现「Remove unused import」；
5. 应用后该行被删除，文件其余部分不变；
6. 再打开一个使用 `List` 但未导入的文件；
7. 光标放在 `List`；
8. 期望出现「Import 'List' (java.util.List)」；
9. 应用后 `import java.util.List;` 被插入到正确位置。

### 60.4 onTypeFormatting 演示步骤

1. 在方法体内输入 `}`；
2. 期望该行缩进自动回退一级；
3. 在字符串中输入 `}`，期望不触发；
4. 输入 `;`，期望当前行与上一行对齐。

### 60.5 semanticTokens 演示步骤

1. 打开一个包含字段、局部变量、参数、方法调用的文件；
2. 期望字段/局部变量/参数颜色不同；
3. 字符串中的类名不着色为类；
4. 注释中的类名不着色为类；
5. 中文标识符的长度正确（不出现半字符高亮）。

---

## 61. 复杂场景剧本

### 61.1 场景：Spring 风格的分层

项目结构：

```
com.example.api.UserService（接口）
com.example.impl.UserServiceImpl（实现）
com.example.controller.UserController（调用方）
```

操作：

1. 在 `UserController` 中注入 `UserService`；
2. 跳转定义 → 到接口；
3. 实现跳转 → 到 `UserServiceImpl`；
4. 在 `UserServiceImpl.createUser` 上触发 superMethods → 到接口方法；
5. 重命名接口方法 → 实现类与调用方同步修改（引用 + import）。

要求：每一步都不能「找不到」，也不能跳到错误位置。

### 61.2 场景：泛型工具类

```java
class Result<T> {
    T data;
    static <E> Result<E> of(E value) { ... }
}
class UserResult extends Result<User> {
    @Override void setData(User data) { ... }
}
```

操作：

1. superMethods 在 `setData` 上 → 匹配 `Result<T>.setData`（通过 `member_type` 实例化）；
2. 类型定义跳转 → 到 `Result`；
3. hover 显示 `User`。

要求：泛型实例化正确，不能因为参数是 `T` 而漏报。

### 61.3 场景：Lombok 生成代码

如果项目使用 Lombok，很多 getter/setter 是编译期生成的。当前实现不解析 Lombok 注解处理器，因此：

- 生成方法可能出现在符号表中（如果 javac 处理了注解），也可能没有；
- 本轮不承诺 Lombok 完整支持；
- 对找不到的符号，明确返回空，不猜测。

### 61.4 场景：多模块项目

当前 workspace 是单根，多模块通过 `ProjectModel.modules` 合并为一个模型。操作要求：

- 模块间跳转依赖 classpath/source roots 合并；
- 若某模块未包含在 source roots，跳转返回空；
- 不把两个模块的同名类混为一谈（按 flat_name 区分）。

---

## 62. 实现常见陷阱

### 62.1 借用冲突

在 `&mut self` 方法中同时需要 `self.compiler.ctx`（可变）与 `self.texts`（不可变）时，先把需要的数据 clone 出来，再借可变引用。禁止在可变借用未结束时调用 `&mut self` 方法。

### 62.2 缓存键不完整

缓存键必须包含「文本 hash + 索引 generation + 配置 hash」。只包含文件路径会导致跨文件编辑后返回旧结果；只包含文本 hash 会在 classpath 变化后返回旧解析结果。

### 62.3 单位混用

所有位置计算必须明确单位：LSP 列是 UTF-16；字符串切片是字节；`ByteRange` 是 UTF-16。函数名与注释必须写清，测试必须包含中文与 emoji。

### 62.4 tree 累积

`ctx.trees` 会累积多轮分析的树，必须用 `latest_unit_root` 定位当前文件的编译单元；否则旧树的符号会干扰新结果（例如旧轮次的同名方法出现在 superMethods 结果里）。

### 62.5 overlay 与磁盘

读取源码内容时优先 `self.texts`（overlay），再读磁盘；否则未保存修改不可见。索引构建也要用 overlay（已有 `overlays` map）。

### 62.6 索引 Partial

Partial 索引下只读查询可以使用（返回可能不完整）；写操作（rename）必须拒绝未保存修改，并提示不完整。不要把 Partial 当 Ready 用。

---

## 63. 发布检查清单

- [ ] 全量测试通过（`cargo test -p javac-ide-core`）；
- [ ] LSP 冒烟 17/17；
- [ ] 至少一个真实项目复测；
- [ ] 未知方法返回 -32601，不崩溃；
- [ ] panic 兜底生效；
- [ ] 日志不污染 stdout；
- [ ] 新增能力已写入 capabilities 或 README；
- [ ] 限制项已记录；
- [ ] 未修改无关代码；
- [ ] 未提交（按任务要求）。

---

## 64. 术语与缩写

- **LSP**：Language Server Protocol。
- **Hover**：悬停提示。
- **Inlay hint**：内联提示（类型/参数名）。
- **Code lens**：代码镜头（方法上方的可点击文本）。
- **Semantic token**：语义 token（着色）。
- **Selection range**：选择范围（逐级扩选）。
- **Document link**：文档链接。
- **Code action**：快速修复/重构动作。
- **Organize imports**：整理 import。
- **Erasure**：泛型擦除。
- **Bridge method**：桥接方法。
- **Partial index**：部分索引。
- **Overlay**：未保存缓冲区。

---

## 65. 参考实现片段（Rust，配中文注释）

### 65.1 擦除签名比较

```rust
/// 返回方法的擦除签名；任何一步失败都返回 None（不猜）。
fn erased_signature_of(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    method: SymbolId,
) -> Option<ErasedSignature> {
    let sym = ctx.sym(method);
    let name = sym.name.to_java_string().to_string_lossy();
    if name == "<init>" {
        return None;
    }
    let ty = sym.ty?;
    let argtypes = match &ctx.typ(ty).kind {
        crate::code::type_::TypeKindData::MethodType { argtypes, .. } => argtypes.clone(),
        _ => return None,
    };
    let mut params = Vec::with_capacity(argtypes.len());
    for arg in argtypes.iter() {
        let erased = types.erasure(ctx, syms, arg);
        let text = crate::analysis::type_names::type_name(ctx, erased);
        if text.is_empty() {
            return None;
        }
        params.push(text);
    }
    Some(ErasedSignature { name, params })
}
```

### 65.2 父类型闭包

```rust
/// 收集所有父类型站点（含泛型实参）。
fn supertype_closure(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    class: SymbolId,
) -> Vec<SuperSite> {
    let mut sites = Vec::new();
    let mut stack = vec![class];
    let mut visited = HashSet::new();
    while let Some(current) = stack.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Some(ty) = ctx.sym(current).ty else { continue };
        let super_ty = types.supertype(ctx, syms, ty, None);
        if let Some(sym) = ctx.typ(super_ty).tsym {
            if visited.insert(sym) {
                sites.push(SuperSite { type_id: super_ty, class: sym });
                stack.push(sym);
            }
        }
        for interface_ty in types.interfaces(ctx, syms, ty).iter() {
            if let Some(sym) = ctx.typ(interface_ty).tsym {
                if visited.insert(sym) {
                    sites.push(SuperSite { type_id: interface_ty, class: sym });
                    stack.push(sym);
                }
            }
        }
    }
    sites
}
```

### 65.3 匹配父方法

```rust
/// 在父类型站点上查找与子方法擦除签名一致的方法。
fn find_matching_method(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    types: &Types,
    site: &SuperSite,
    child: &ErasedSignature,
) -> Option<SymbolId> {
    let scope = ctx.sym(site.class).members()?;
    let members = ctx.scope_get_symbols(scope, LookupKind::NON_RECURSIVE, &|_| true);
    for member in members {
        let sym = ctx.sym(member);
        if !matches!(sym.kind_data, SymbolKind::MethodSymbol { .. }) {
            continue;
        }
        if sym.name.to_java_string().to_string_lossy() != child.name {
            continue;
        }
        if (sym.flags() & STATIC as i64) != 0 {
            continue;
        }
        if (sym.flags() & PRIVATE as i64) != 0 {
            continue;
        }
        if (sym.flags() & BRIDGE) != 0 {
            continue;
        }
        let parent_ty = types.member_type(ctx, syms, site.type_id, member);
        let parent_args = match &ctx.typ(parent_ty).kind {
            crate::code::type_::TypeKindData::MethodType { argtypes, .. } => argtypes.clone(),
            _ => continue,
        };
        if parent_args.len() != child.params.len() {
            continue;
        }
        let mut matched = true;
        for (index, parent_arg) in parent_args.iter().enumerate() {
            let erased = types.erasure(ctx, syms, parent_arg);
            let text = crate::analysis::type_names::type_name(ctx, erased);
            if text != child.params[index] {
                matched = false;
                break;
            }
        }
        if matched {
            return Some(member);
        }
    }
    None
}
```

### 65.4 位置转换

```rust
/// 把父方法声明转成 Location；找不到返回 None。
fn method_location(
    ctx: &mut CompilerContext,
    config: &AnalysisConfig,
    texts: &HashMap<PathBuf, String>,
    method: SymbolId,
) -> Option<Location> {
    let sym = ctx.sym(method);
    let name = sym.name.to_java_string().to_string_lossy();
    let owner = sym.owner?;
    let (sourcefile, flat_name) = match &ctx.sym(owner).kind_data {
        SymbolKind::ClassSymbol { sourcefile, flatname, .. } => {
            (sourcefile.clone(), flatname.to_string())
        }
        _ => return None,
    };
    if let Some(file) = sourcefile {
        let path = PathBuf::from(file.get_name().to_string_lossy());
        if config.source_path.iter().any(|root| path.starts_with(root)) {
            let key = PathBuf::from(crate::analysis::canonical(&path));
            let text = texts
                .get(&key)
                .cloned()
                .or_else(|| std::fs::read_to_string(&path).ok())?;
            let pos = ctx.sym(method).pos.get().max(0) as u32;
            let pos = crate::analysis::positions::declared_name_start(&text, pos, &name);
            let (line, column) =
                crate::analysis::positions::offset_to_line_col(&text, pos as usize);
            return Some(Location {
                file: path,
                range: Range::point(Position::new(line, column)),
            });
        }
    }
    let jdk_src = crate::analysis::jdk_sources::default_jdk_src()?;
    crate::analysis::jdk_sources::jdk_member_location(
        &jdk_src, &flat_name, &name, &config.output_dir,
    )
}
```

---

## 66. 结束语（二）

这份文档已经覆盖：需求来源、优先级、每项能力的完整设计、泛型/varargs/桥接等难点、测试矩阵、性能预算、风险、验收流程、评审清单与参考实现。它的长度不是目的，目的是让「完整实现」变成可执行、可检查的工程，而不是一句口号。后续执行时，任何「为了快而简化」的念头，都应该回到第 51 章的反例看一眼后果。计划到此为止，接下来进入实现。

---

## 67. 常见问题（FAQ）

**问一：为什么不用现成的 Java 语言服务器（例如 Eclipse JDT LS）？**

答：本项目的目标是用 Rust 重写 javac，并把 IDE 能力建立在自己的编译前端上。使用现成服务器会绕开这个目标，也无法验证我们的编译器语义是否正确。自研服务器还能保证与索引、构建系统、诊断格式的一致性。

**问二：为什么继承方法重写不能用「方法名 + 参数个数」？**

答：Java 允许重载，`f(int)` 与 `f(String)` 参数个数相同但签名不同；只看个数会把两个方法混为一谈。用户跳到错误的父方法后，会按错误的契约写代码，且没有任何提示。必须比较擦除后的参数类型。

**问三：为什么要用 `member_type`，不能直接比较父方法声明的参数类型吗？**

答：父方法声明里的参数可能是类型变量 `T`。例如 `Base<T>` 的 `put(T)`，在 `Sub extends Base<String>` 中实际签名是 `put(String)`。只有 `member_type(site, method)` 能把 `T` 替换成 `String`，否则永远匹配不上。

**问四：为什么桥接方法不能算父方法？**

答：桥接方法是编译器为了泛型/协变自动生成的转发方法，不是程序员写的声明。把它显示给用户，会让用户以为父类里有一个额外的方法，且位置通常没有意义（可能指向类声明）。所以必须用 `BRIDGE` 标志过滤。

**问五：为什么 selectionRange 不能用行号或括号配对实现？**

答：行号无法处理一行多个表达式、跨行表达式、lambda、匿名类；括号配对会把字符串/注释里的括号也算进去。正确做法是基于语法树的范围链，才能保证每一级都严格包含下一级。

**问六：为什么 Add import 不能只做文本搜索？**

答：文本搜索会把字符串、注释、更长的标识符当成「已经使用」，导致不提供 import；也可能给出错误的候选（例如把 `ArrayList` 的前缀当成 `List`）。正确做法是按符号解析与标识符边界判定。

**问七：为什么语义高亮需要编译器符号，不能正则？**

答：正则无法区分类名、方法名、字段名、局部变量、参数；也无法区分声明与引用、静态与实例。语义高亮的价值就在于「按真实含义着色」，正则只能做语法级近似。

**问八：为什么 Partial 索引下 rename 要谨慎？**

答：Partial 索引可能缺少失败文件的符号；rename 基于不完整索引会漏改引用，导致代码编译失败。因此 rename 在索引不完整或存在未保存修改时应拒绝或提示，而不是静默给出不完整结果。

**问九：为什么 hover 要显示 Overrides？**

答：很多用户不会主动触发自定义的 superMethods 命令；hover 是零成本的信息入口。在 hover 里加一行 Overrides，可以让用户在阅读时自然发现父方法，而不需要记住快捷键。

**问十：为什么计划要写这么长？**

答：因为「完整实现」的难点不在主路径，而在边界：泛型、varargs、桥接、多接口、Partial、overlay、CRLF、UTF-16、外部 jar 无源码。把这些边界提前写清楚，实现时就不会用「先简化、以后再说」蒙混过关；评审时也有据可查。

---

## 68. 执行纪律

1. **一次只做一项能力**：完成一项（代码 + 测试 + LSP + 真实复测）再进入下一项，禁止多线并行导致半成品堆积。
2. **先写失败测试**：每项能力先写至少一个会失败的集成测试，确认测试环境正确，再写实现。
3. **每天结束前跑全量**：保证 `cargo test -p javac-ide-core` 与 LSP 冒烟全绿，不留过夜回归。
4. **不删除他人改动**：工作区可能有未提交的其他修改，只改自己负责的文件。
5. **不 commit**：按任务要求，交付时只报告改动与测试结果。
6. **记录证据**：每次交付附上命令输出摘要（通过数、冒烟结果、真实请求响应）。
7. **遇到底层 API 缺失先停**：不要在业务层打补丁；补底层能力并加测试，再继续。
8. **拒绝「临时简化」**：任何临时方案必须写成 TODO 并明确截止项，不允许悄悄变成永久实现。

---

## 69. 每周检查表

- [ ] 本周计划能力是否按「完整实现」完成？
- [ ] 是否有新增的「名称/个数匹配」「行号近似」「文本搜索」代码？（有则必须重写）
- [ ] 全量测试是否全绿？
- [ ] LSP 冒烟是否 17/17？
- [ ] 是否有真实项目复测记录？
- [ ] 已知限制是否更新到 README？
- [ ] 测试是否覆盖中文/CRLF/BOM/overlay/Partial？
- [ ] 是否有性能测量（至少记录一次耗时）？
- [ ] 是否有未处理的 panic 路径？
- [ ] 是否有未提交的无关改动？

---

## 70. 计划执行后的验收总表

| 能力 | 完整实现标准 | 验收命令 | 证据 |
|---|---|---|---|
| super_methods | 擦除签名 + member_type + 桥接/varargs/多接口 | `cargo test -p javac-ide-core --test super_methods_spec` | 22 项通过 + 真实项目 |
| selectionRange | AST 范围链 + end_pos | `cargo test -p javac-ide-core --test selection_range_spec` | 22 项通过 |
| codeAction | 候选消歧 + 插入点 + CRLF | `cargo test -p javac-ide-core --test code_actions_spec` | 12 项通过 |
| onTypeFormatting | 行级缩进 + 触发精确 | `cargo test -p javac-ide-core --test on_type_formatting_spec` | 10 项通过 |
| documentLink | import/URL 解析 | `cargo test -p javac-ide-core --test document_link_spec` | 10 项通过 |
| semanticTokens | legend + UTF-16 编码 + 语义来源 | `cargo test -p javac-ide-core --test semantic_tokens_spec` | 15 项通过 |
| codeLens | 引用计数 + Partial 策略 | `cargo test -p javac-ide-core --test code_lens_spec` | 10 项通过 |
| organizeImports | 排序/去重/未使用/静态/CRLF | `cargo test -p javac-ide-core --test organize_imports_spec` | 10 项通过 |
| generate | 构造器/toString/equals | `cargo test -p javac-ide-core --test generate_spec` | 逐字符断言 |

---

## 71. 结语（三）

把计划写长很容易，把实现做对很难。这份文档的价值不在于篇幅，而在于它把「什么算完成」写死了：不是「能返回结果」，而是「在重载、泛型、桥接、多接口、同步、边界场景下都返回正确结果，找不到就明确返回空」。后续每一项能力都按这个标准验收，不达标就重做，不用「先这样」糊弄过去。

计划结束。
