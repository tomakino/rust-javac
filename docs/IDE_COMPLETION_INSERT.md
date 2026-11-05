# 方法补全的插入策略设计（自动括号 / 智能分号 / 光标落点）

> 状态：已实现（2026-11-06，规格测试 `completion_spec` S1–S14 全绿）
> 依赖：`docs/IDE_COMPLETION.md`（候选引擎规格）
> 客户端：Sublime Text + LSP 2.16（行为已读源码核对）

---

## 1. 目标

方法补全从"只插入名字"升级为"插入一段可直接继续编写的代码"：

1. **方法候选项自动补全圆括号**：`get` → `get()`
2. **参数数量不为 0 时，光标落在 `(` 与 `)` 之间**：`m($0)`
3. **按上下文智能决定是否补 `;`**：
   - 方法返回类型为 `void` 且位于语句位置 → 补分号
   - 位于 `return` 表达式位置 → 补分号（否则语句不完整）
   - 表达式内部（实参、赋值右侧、条件、二元运算）→ 不补
   - 类体（字段/方法声明位置）→ 不补
4. 插入文本必须可被规格测试**逐字符断言**，不依赖编辑器行为。

非目标：不改变候选集合、可见性、排序、去重（这些在 `IDE_COMPLETION.md` 已定义）；不做自动 import；不做参数名占位重排（见 §12 待确认 C）。

---

## 2. 约束与事实

### 2.1 引擎侧

| 事实 | 影响 |
|---|---|
| `JCTree` 只有 `pos`（起点），**没有 end_pos**；`EndPosTable` 仅在 `-Xjcov` 时存在 | 不能靠"包含光标的最深树"判定上下文，必须"词法扫描 + AST 起点锚点" |
| `Block` 树的 `pos` = 左花括号 `{` 的位置 | 可用文本扫描出的最内层 `{` 的 offset 与 Block.pos 精确匹配 |
| 方法返回类型在 `TypeKindData::MethodType { restype, argtypes, .. }` | `restype.tag == TypeTag::VOID` 判 void；`argtypes.len()` 判参数数 |
| 插入发生在 LSP 层，引擎只提供文本 | 插入文本生成必须是无副作用的纯函数 |

### 2.2 客户端（LSP 2.16，已读 `plugin/completion.py`）

`LspSelectCompletionCommand.run` 的实际行为：

1. 有 `textEdit` → 先 `erase` 它的 range，再插入 `newText`
2. 无 `textEdit` → 插入 `insertText`，缺省用 `label`
3. `insertTextFormat == Snippet` → `view.run_command("insert_snippet", {"contents": ...})`
4. 否则 `insert` 纯文本
5. 列表里显示的触发文本来自 `label` / `filterText`，**不使用 insertText**

结论：
- 用 **`textEdit` + `insertTextFormat: 2`** 可以同时做到"替换已输入前缀"和"`$0` 定位光标"
- snippet 语法就是 Sublime 的 `$0` / `${1:...}`，与 LSP 一致

---

## 3. 数据模型变更

### 3.1 `javac-analysis-api`

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: SymbolKind,
    pub detail: Option<String>,
    pub documentation: Option<String>,

    /// 实际插入的文本（snippet 或纯文本）；None 表示用 label。
    pub insert_text: Option<String>,
    /// 插入文本格式。
    pub insert_text_format: InsertTextFormat,
    /// 是否用 textEdit 替换光标前已输入的 prefix。
    pub replace_prefix: bool,
    /// 列表过滤文本（默认 label）。
    pub filter_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InsertTextFormat {
    #[default]
    PlainText,
    Snippet,
}
```

### 3.2 LSP 映射（`crates/ravac-lsp/src/lib.rs`）

| API 字段 | LSP 字段 | 说明 |
|---|---|---|
| `insert_text` | `textEdit.newText` 或 `insertText` | `replace_prefix` 时用 textEdit |
| `insert_text_format` | `insertTextFormat`（1/2） | Snippet=2 |
| `replace_prefix` | `textEdit.range` | start = position − prefix 长度（UTF-16），end = position |
| `filter_text` | `filterText` | 缺省 `label` |

`textEdit.range` 只在同一行内计算（prefix 不可能跨行；换行时 `line_offset` 会截断）。

---

## 4. 上下文分类器

### 4.1 输出枚举

```rust
pub(crate) enum InsertionContext {
    /// 块内语句位置：光标前是 `;` `{` `}` 或行首。
    StatementStart,
    /// `return` 关键字之后（同一语句、光标前无其他 token）。
    ReturnExpression,
    /// 表达式内部：实参、赋值右侧、条件、二元运算、三元、lambda 体表达式等。
    Expression,
    /// 类/接口/枚举/记录/匿名类的成员区（字段或方法声明位置）。
    ClassBody,
    /// 类型位置：extends/implements/new/泛型实参/变量类型/强制转换。
    TypeContext,
    /// import/package 声明。
    ImportPackage,
    /// 注解参数。
    Annotation,
    /// 无法判定（语法错误、光标在注释/字符串内等）。
    Unknown,
}
```

### 4.2 判定算法（纯函数）

输入：`text`、光标 byte offset、AST 锚点集合：

```rust
pub(crate) struct ContextAnchors {
    /// 所有 JCBlock 的 pos（左花括号 offset）。
    pub block_starts: Vec<usize>,
    /// 类体左花括号 offset → 是否类体（由 ClassDecl/EnumDecl/RecordDecl/
    /// InterfaceDecl/AnnotationType/匿名类体推导）。
    pub class_body_starts: Vec<usize>,
    /// switch 体左花括号 offset。
    pub switch_starts: Vec<usize>,
    /// 数组初始化左花括号 offset（new int[]{...}）。
    pub array_starts: Vec<usize>,
}
```

步骤：

1. **词法扫描**（`lexer.rs`）从文件头扫到光标，跳过：
   - 行注释 `//...`
   - 块注释 `/* ... */`（可嵌套? 否，Java 不嵌套）
   - 字符串 `"..."`、字符 `'...'`、文本块 `"""..."""`（含转义 `\\`）
   产出：
   - `brace_stack: Vec<Brace>`（`{`/`(`/`[` 的 offset 与类型）
   - `last_token: Option<(kind, offset, is_statement_boundary)>`
2. **光标在字符串/注释内** → `Unknown`（补全层通常已不返回候选）。
3. **最内层 `{`**（`brace_stack` 中最后一个 `{`）：
   - offset ∈ `block_starts` → 进入块内判定（步骤 4）
   - offset ∈ `class_body_starts` → `ClassBody`
   - offset ∈ `switch_starts` → `StatementStart`（case 标号之后）
   - offset ∈ `array_starts` → `Expression`
   - 不存在 `{` → `TypeContext`（编译单元顶层）
4. **块内细分**（看 `last_token`）：
   - `;`、`{`、`}` 或行首无 token → `StatementStart`
   - `return`（且 return 与光标之间只有空白/prefix）→ `ReturnExpression`
   - `(`、`,`、`=`、`&&`、`||`、`+`、`-`、`*`、`/`、`%`、`?`、`:`、`->`、`[`、`instanceof`、`new` → `Expression`
   - `identifier`（如 `System.out.print` 的 `print`）→ `Expression`（方法调用实参或链式）
   - 其他 → `Expression`（保守，不补分号）
5. **类型位置覆盖**：若 `last_token` 是 `extends`/`implements`/`new`/`<`/`,` 且处于类型参数列表、或光标前是类型声明上下文 → `TypeContext`（优先于步骤 4）。
   实现上由 `lexer` 记录"最近关键字"辅助判断，宁可漏判（降级为 Expression）也不能误判。

### 4.3 为什么不用"光标最近的 AST 树"

`JCTree` 无 end_pos，某树的 `pos <= cursor` 不代表光标在其范围内（可能已经离开）。用**文本括号栈**得到确定性的"最内层花括号"，再与 AST 起点匹配，是唯一无歧义的组合方式。

---

## 5. 插入文本生成规则（按用户给定的光标语义）

记号：
- `|`：插入完成后的光标位置（与规格测试的 `|` 标记一致）
- `name`：方法名（snippet 转义见 §6.2）
- "参数数" = `argtypes.len()`（varargs 方法按 ≥1 计）

### 5.1 用户给定的八个基准例（规范来源）

表达式上下文（实参内）：

```
print(xx|);   + String + 0 参  → print(xxx()|);
print(xx|);   + void   + 0 参  → print(xxx();|);
print(xx|);   + String + ≥1 参 → print(xxx(|));
print(xx|);   + void   + ≥1 参 → print(xxx();|);
```

语句位置：

```
xxx|   + String + 0 参  → xxx()|
xxx|   + void   + 0 参  → xxx();|
xxx|   + String + ≥1 参 → xxx(|)
xxx|   + void   + ≥1 参 → xxx(|);
```

### 5.2 归纳后的规则矩阵

| 上下文 | 返回类型 | 参数数 | 插入文本 | 光标 |
|---|---|---|---|---|
| StatementStart | 非 void | 0 | `name()` | 末尾（`)` 后） |
| StatementStart | 非 void | ≥1 | `name()` | `(` 后 |
| StatementStart | void | 0 | `name();` | 末尾（`;` 后） |
| StatementStart | void | ≥1 | `name();` | `(` 后 |
| Expression | 非 void | 0 | `name()` | 末尾 |
| Expression | 非 void | ≥1 | `name()` | `(` 后 |
| Expression | void | 0 | `name();` | 末尾 |
| Expression | void | ≥1 | `name();` | 末尾（**不进括号**） |
| ReturnExpression | 非 void | 0 | `name();` | 末尾 |
| ReturnExpression | 非 void | ≥1 | `name();` | `(` 后 |
| ClassBody | 任意 | 0 | `name()` | 末尾 |
| ClassBody | 任意 | ≥1 | `name()` | `(` 后 |
| TypeContext / ImportPackage / Annotation / Unknown | 任意 | — | `name`（无括号） | 末尾 |

归纳：

- **非 void**：StatementStart / Expression / ClassBody **都不补分号**；唯一例外是 ReturnExpression（`return get()` 缺 `;` 是语法错误，已确认 B）。
- **void**：**一律补分号**。光标只有两种落点：
  - **语句位置 + 参数 ≥1** → `(` 后（`xxx(|);`）
  - 其余情况（含表达式位置、参数 0）→ 末尾（`;` 后）
- **表达式 + void** 的插入结果（`print(xxx();)`）本身是语法错误。这是**有意的取舍**：void 方法本来就不能作为表达式值，出现在实参里属于用户误用；引擎按统一规则插入（补分号、光标在末尾），不额外修正、不特殊分支。用户已确认此取舍（2026-11-06）。

---

## 6. 光标定位与转义

### 6.1 光标落点（`$0`）

| 场景 | 落点 |
|---|---|
| 参数 0 | 无 `$0`（插入后光标在文本末尾） |
| 参数 ≥1 且按 §5.2 需要进括号 | `$0` 放在 `(` 之后、`)` 之前 |
| 参数 ≥1 但不进括号（表达式 + void） | 无 `$0`（光标在 `;` 后） |
| 补分号且光标在末尾 | 分号是最后一个字符，无 `$0` |
| 补分号且光标在括号内 | `$0` 在 `(` 后，分号写在 `$0` 之后的 `)` 后面 |

### 6.2 snippet 转义

方法名允许 `$`（Java 标识符），snippet 中 `$` 是元字符，必须转义：

```rust
fn escape_snippet(name: &str) -> String {
    name.replace('$', "\\$")
}
```

插入文本中的其他字符不参与 snippet 语法，无需转义（`(`、`)`、`;` 都是普通字符）。参数名占位若将来启用，需要转义 `\` 和 `}`。

---

## 7. 前缀替换（textEdit）

### 7.1 为什么必须

Sublime 的 `lsp_select_completion` 在无 `textEdit` 时直接 `insert` 光标处文本。用户输入 `ge` 后选择 `get`，会得到 `geget()`。因此方法候选必须返回 `textEdit`，range 覆盖已输入的 prefix。

### 7.2 range 计算

```
start = position - prefix_utf16_len
end   = position
```

- 只在同一行内计算（prefix 不可能含换行）
- `prefix_utf16_len` 由 `prefix_at` 计算（已按 UTF-16 处理中文/emoji 标识符）

### 7.3 哪些候选带 replace_prefix

| 候选类型 | replace_prefix |
|---|---|
| Method（含构造器？构造器不出现） | true |
| Field | false（Sublime 自己替换词） |
| Class/Interface/Enum/Record | false |
| Keyword | false |
| LocalVariable/Parameter | false |

第一版只对 Method 启用，避免影响现有已验证行为。

---

## 8. 客户端映射（已核对 LSP 2.16）

```
服务端 item:
  label = "get"
  kind  = Method
  textEdit = { range: [start..end], newText: "get($0);" }
  insertTextFormat = 2

Sublime:
  format_completion → trigger = "get"（列表显示）
  LspSelectCompletionCommand.run:
    erase [start..end)（删掉已输入的 "ge"）
    insertTextFormat == Snippet → insert_snippet("get($0);")
    光标落在 $0
```

列表显示、过滤仍基于 `label` / `filterText`，不受插入文本影响。

---

## 9. 模块划分

```
crates/javac-ide-core/src/analysis/features/completion/
├── mod.rs        // 编排：候选生成后调用 insert::augment()（现有文件）
├── lexer.rs      // 轻量扫描：跳过注释/字符串/字符，维护括号栈与最后 token
├── context.rs    // InsertionContext + ContextAnchors + classify()
└── insert.rs     // 方法 → 插入文本（R1–R11 规则表）、snippet 转义
```

- `lexer.rs`：纯文本，无 AST、无符号；可独立单测
- `context.rs`：调用 lexer + AST 锚点（Block/ClassDecl/Switch/NewArray 遍历）
- `insert.rs`：依赖 `MethodType { restype, argtypes }`，输出 `InsertPlan { text, format, replace_prefix }`
- `mod.rs`：在 `members_of_type` 生成 `CompletionItem` 的位置调用 `insert::augment(...)`

API 层（`crates/javac-analysis-api`）扩展字段；LSP 层（`crates/ravac-lsp`）映射。

---

## 10. 测试计划

### 10.1 规格测试（新增，引擎级）

```
S1   print(xx|); + String + 0 参   → insertText "xxx()"       光标末尾
S2   print(xx|); + void   + 0 参   → insertText "xxx();"      光标末尾
S3   print(xx|); + String + ≥1 参  → insertText "xxx()"       光标在 "(" 后
S4   print(xx|); + void   + ≥1 参  → insertText "xxx();"      光标末尾
S5   xxx|         + String + 0 参  → insertText "xxx()"       光标末尾
S6   xxx|         + void   + 0 参  → insertText "xxx();"      光标末尾
S7   xxx|         + String + ≥1 参 → insertText "xxx()"       光标在 "(" 后
S8   xxx|         + void   + ≥1 参 → insertText "xxx();"      光标在 "(" 后
S9   return get|  + String         → insertText "get();"      光标末尾
S10  String s = | + String         → insertText "get()"       光标末尾
S11  类体声明位置 + 任意            → insertText "xxx()"       光标末尾
S12  prefix 替换：ge| + get         → textEdit.range 覆盖 "ge"
S13  方法名 a$b + ≥1 参            → insertText "a\\$b()"     光标在 "(" 后
S14  varargs void m(String... a)   → insertText "m();"        光标在 "(" 后（语句位置）
```

### 10.2 上下文分类单测（`context.rs`）

- 块内语句（方法体、静态块、初始化块、lambda 体、构造器体）
- 类体（普通类、接口、枚举、记录、匿名类）
- switch 体、case 标号后
- 数组初始化
- 字段初始化 `=` 后
- return、实参、三元、二元、强制转换
- import/package/注解
- 字符串/注释内（Unknown）
- 语法错误（保守降级）

### 10.3 回归

- 现有 37 项 `completion_spec` 全绿（候选/可见性/排序不变）
- `completion_matrix` 14 项
- `editor_sim` 51 项、LSP 冒烟 11 项

---

## 11. 边界与降级

| 情况 | 行为 |
|---|---|
| 语法错误、AST 缺失、锚点对不上 | 降级：只插 `name`（不括号、不分号） |
| 光标在注释/字符串内 | 不补全（现有行为） |
| 方法重载 | 每个候选按自身 `argtypes` 独立生成 |
| varargs | 视为 ≥1 参数 |
| 静态导入 / 类名.方法 | 规则相同；textEdit range 只覆盖方法名前缀 |
| 构造器 | 不出现（候选规格 S3） |
| 注解元素（`@Foo(value = ...`） | Annotation → R11 |
| 多光标 | 客户端 `_translated_regions` 已处理主光标；range 按主光标计算 |
| 链式调用 receiver | 只影响候选来源，不影响本方法的插入文本 |
| 方法名含 `$` | snippet 转义 |
| 光标前是 `instanceof ` | Expression（不补分号） |

---

## 12. 已确认决策（2026-11-06）

- **A. 非 void**：语句位置 / 表达式 / 类体一律**不补**分号。已确认。
- **B. `return` 后（非 void）**：**补**分号（`return get();`），否则语句不完整。已确认。
- **C. 多参数占位**：**不生成** `${1:arg}`。已确认。
- **D. 类体**：不补分号。已确认（对应"光标是不是类嵌套中"）。
- **E. 表达式 + void**：按用户示例**补分号、光标在分号后**（不进括号）；插入结果本身语法非法，属用户指定行为。
- **F. 开关**：第一版**不提供** `initializationOptions` 开关，行为固定。

---

## 13. 实施顺序（确认后）

1. `lexer.rs` + 单测（跳过注释/字符串、括号栈、最后 token）
2. `context.rs` + 单测（锚点遍历 + classify）
3. `insert.rs` + 单测（§5.2 规则矩阵）
4. API 扩展 + LSP 映射（textEdit/insertTextFormat）
5. `completion_spec.rs` 新增 S1–S14，全绿
6. Sublime 实机验证（含中文标识符、`$` 名、重载、varargs）
7. 文档更新（本设计并入 `IDE_COMPLETION.md` 附录）
