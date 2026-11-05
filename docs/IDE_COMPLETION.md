# 补全引擎设计

对标 IntelliJ IDEA 的 Java 代码补全，LSP（textDocument/completion）承载。

本文档是**规格**：实现必须逐条满足；测试必须按本文档的规则写断言，不得迁就实现。

---

## 1. 目标与非目标

**目标**
- 补全候选与 IDEA 语义一致：可见性、静态/实例、继承、上下文过滤、排序。
- 解析失败时**返回空**而不是回退到全局类列表。
- 每个规则有对应断言，规则与实现可双向追溯。

**非目标**
- 不做模糊/驼峰匹配（前缀匹配，大小写敏感）。Sublime 客户端可自行二次过滤，但服务器不得返回前缀不匹配的项。
- 不实现代码片段（snippet）补全。
- 不做跨文件重载签名合并。

---

## 2. 输入

LSP \`textDocument/completion\`：

| 字段 | 用途 |
|---|---|
| \`textDocument.uri\` | 目标文件（编辑器未保存文本通过 didOpen/didChange 同步） |
| \`position.line/character\` | 0-based，UTF-16 列 |
| \`context.triggerKind\` | 1=Invoked，2=TriggerCharacter |
| \`context.triggerCharacter\` | 被触发的字符（\`.\`、\`:\`） |

服务器状态：当前文件的文本、最近一次 analyze 的 AST（\`ctx.trees\`）、符号表（\`syms\`）、当前文件索引。

---

## 3. 上下文分类（第一步）

在产生任何候选前，先判定补全点属于哪一类。判定互斥，按顺序：

1. **ImportContext**：行首（忽略空白）以 \`import \` 开头，且光标在分号前。
2. **PackageContext**：行首以 \`package \` 开头。
3. **TypeContext**：光标处于类型位置——变量/字段/参数/返回值/throws/extends/implements/new 之后的类型槽。
4. **AnnotationContext**：光标处于注解名位置（前一个非空白 token 是 \`@\`）。
5. **ReceiverContext**：光标前（同一行）存在 \`.\`，且 \`.\` 前是表达式。
6. **StatementContext**：行首/语句开头（\`;\`、\`{\`、\`}\` 之后）。
7. **ExpressionContext**：其余位置（表达式内部）。

**规则 C1**：类型/注解/Import/Package 上下文**不得**给出成员候选。
**规则 C2**：ReceiverContext **不得**给出全局类名候选。

---

## 4. Receiver 语义

### 4.1 语法形态与解析

\`.\` 前的表达式称为 receiver。按形态解析其类型：

| # | 形态 | 示例 | 类型来源 |
|---|---|---|---|
| R1 | 标识符：局部变量/参数/字段 | \`s.\` | 符号 \`sym.ty\` |
| R2 | 标识符：类名 | \`Math.\` | 类符号；**仅静态**成员 |
| R3 | \`this\` | \`this.\` | 当前文件的类 |
| R4 | \`super\` | \`super.\` | 当前类的直接超类 |
| R5 | 字符串字面量 | \`"x".\` | \`java.lang.String\` |
| R6 | char 字面量 | \`'a'.\` | \`java.lang.Character\` |
| R7 | 整数字面量 | \`1.\` / \`1L.\` | \`Integer\` / \`Long\` |
| R8 | 浮点字面量 | \`1.5.\` / \`1.5f.\` | \`Double\` / \`Float\` |
| R9 | boolean 字面量 | \`true.\` | \`java.lang.Boolean\` |
| R10 | \`null\` | \`null.\` | **无成员**（空列表） |
| R11 | 构造/方法调用/链式/括号表达式 | \`new String().\` \`foo().\` \`a.b().\` | AST 中光标处最内层 \`FieldAccess.selected.ty\`（Attr 已填充） |
| R12 | 数组 | \`arr.\` | 仅 \`length\`（int）+ Object 成员 |
| R13 | 原语变量 | \`int x; x.\` | **空列表** |
| R14 | 其他（解析失败） | 无法解析 | **空列表** |

**规则 R-A**：R12–R14 必须返回空列表，**不得**回退到全局候选。
**规则 R-B**：R11 只使用**已 Attr 的 AST**；若 AST 中找不到该位置的 \`FieldAccess\`，按 R14 处理。

### 4.2 泛型

- \`List<String> list; list.\` 的成员按**擦除类型** \`List\` 给出（IDEA 会用类型参数特化方法返回值；本期不做）。
- 标记为 TypeVar 的 receiver（如 \`T t; t.\`）→ 使用 TypeVar 的**上界**（无上界 → Object）。

---

## 5. 可见性规则

定义调用点上下文：

- \`C\` = 当前文件中包含光标的类
- \`P\` = \`C\` 所在包
- \`R\` = receiver 的静态类型（类）
- \`sub\` = C 是 R 的子类（含间接）
- \`same\` = C 与 R 在同一个包
- \`sameClass\` = C == R

| 修饰符 | 出现在补全中的条件 |
|---|---|
| \`public\` | 始终 |
| \`protected\` | \`sub\` 或 \`same\` |
| 包私有（无修饰符） | \`same\` |
| \`private\` | \`sameClass\` |
| 合成成员（\`<init>\`、\`<clinit>\`、bridge、\`$deserialize\` 等） | **从不** |

**规则 V1**：可见性判断使用 **调用点**，而不是成员声明点的包。
**规则 V2**：包私有与 protected 的判断依赖 receiver 的**静态类型 R**，不是运行时类型。
**规则 V3**：\`java.lang\` 的包判断与项目包一致处理；没有包声明时视为默认包。

---

## 6. 静态与实例

| receiver 形态 | 成员集合 |
|---|---|
| 类名（R2） | 仅 \`static\` |
| 实例（R1、R3、R4、R5–R9、R11、R12） | 非静态成员；**同时**包含可访问的 \`static\` 成员（Java 允许通过实例访问静态成员） |

**规则 S1**：类名 receiver 下，实例成员**不得**出现。
**规则 S2**：\`this\` 在静态方法内 → 语法错误，按 R14 空列表处理。
**规则 S3**：构造器（\`<init>\`）不作为成员候选；匿名类合成构造器同样排除。

---

## 7. 继承

**规则 I1**：成员集合 = R 自身 + 所有超类链 + 所有实现的接口（含接口默认方法）。
**规则 I2**：子类覆盖父类方法时，只保留子类声明（按签名去重：名称 + 参数类型）。
**规则 I3**：继承来的成员同样应用第 5 节可见性规则。
**规则 I4**：\`Object\` 的 public 方法对所有类可用（包括 \`toString\`/\`equals\`/\`hashCode\` 等），但 \`Object.\` 自身作为 receiver 时也适用。
**规则 I5**：\`super.\` 只列**直接超类**（不递归）的可见成员，且不含 \`Object\` 之外的间接父类。

---

## 8. 候选集合（非 receiver 上下文）

### 8.1 ExpressionContext / StatementContext

按顺序合并（同一 label+kind 去重）：

1. **局部变量**：光标前已声明的局部变量与参数（作用域包含光标，声明位置 < 光标）。
2. **字段**：当前类及其超类的可见字段（实例字段 + 静态字段）。
3. **方法**：当前类及其超类的可见方法（不含构造器）。
4. **类型**：当前文件声明的类、同包类、import 的类、classpath 中的类（前缀非空时）。
5. **关键字**：按上下文子集给出（见 8.3）。
6. **包/导入**：ImportContext / PackageContext 单独处理（见 8.4）。

**规则 G1**：合并后按前缀过滤，前缀为空时**只**返回局部变量、字段、方法（不返回全部类）。
**规则 G2**：结果去重（label+kind）后最多 100 条。

### 8.2 TypeContext

只返回类型（类/接口/枚举/记录/类型参数）：
1. 当前文件声明类型
2. 同包类型
3. 已 import 类型
4. classpath 类型（前缀非空）
5. 基本类型关键字（\`int\`、\`boolean\` 等）

### 8.3 关键字上下文表

| 上下文 | 关键字集合 |
|---|---|
| ExpressionContext | \`new\`、\`this\`、\`super\`、\`null\`、\`true\`、\`false\`、\`instanceof\` |
| StatementContext | \`if\`、\`else\`、\`for\`、\`while\`、\`do\`、\`switch\`、\`case\`、\`break\`、\`continue\`、\`return\`、\`try\`、\`catch\`、\`finally\`、\`throw\`、\`synchronized\`、\`assert\`、\`yield\` |
| 声明位置（类体开头/成员前） | 修饰符：\`public\`、\`private\`、\`protected\`、\`static\`、\`final\`、\`abstract\`、\`native\`、\`synchronized\`、\`transient\`、\`volatile\`、\`strictfp\`、\`default\`；声明：\`class\`、\`interface\`、\`enum\`、\`record\`、\`void\`、基本类型 |
| AnnotationContext | 仅注解类型 |
| ImportContext | \`static\` + 包名/类名 |
| PackageContext | 包名 |

**规则 K1**：ReceiverContext **不返回**任何关键字（\`this\` 等不含）。
**规则 K2**：\`null\`、\`true\`、\`false\` 只出现在 ExpressionContext。

### 8.4 Import / Package

- **ImportContext**：\`import java.util.\` → 该包的**类**（按前缀）；\`import \` 后无点 → 包名 + 顶层类。
- **PackageContext**：包名候选。
- 未导入类建议（可选，后续）：\`additionalTextEdits\` 自动补 import；本期只出候选，不带编辑。

---

## 9. 排序

排序键（升序）：

1. **前缀完全相等**（0）优先于前缀更长（1）。
2. **大小写敏感匹配**在前；大小写不敏感匹配在后（仅当服务器开启不敏感模式，默认关闭）。
3. 类别：局部变量/参数（0） < 字段（1） < 方法（2） < 类（3） < 关键字（4）。
4. label 字典序。

**规则 O1**：排序在**过滤后**执行；截断在排序后执行。

---

## 10. 去重与截断

**规则 D1**：同 label+kind 的候选只保留**绑定最强的**一个（局部 > 字段 > 方法 > 类）。
**规则 D2**：最多 100 条；截断保留排序后的前 100。

---

## 11. 懒加载与性能

**规则 P1**：类成员在第一次需要时调用 \`Completer::complete\`（JDK/classpath 类懒加载）；失败时按空成员处理，不 panic。
**规则 P2**：补全请求内只允许一次 AST 扫描（FieldAccess 回退），不得在大范围内重复遍历。
**规则 P3**：单次补全在 50ms 内完成（10 万类 classpath 下只在前缀非空时扫描类表，且允许提前截断）。

---

## 12. 测试规格（断言必须覆盖）

每条规则至少一个断言：

| 规则 | 用例 |
|---|---|
| R1 | \`String s = ""; s.\` → 含 trim |
| R2 | \`Math.\` → 含 PI；**不含**实例方法（如无 static 标记的） |
| R3 | \`this.\` → 含当前类字段 |
| R4 | \`super.\` → 含直接父类字段，**不含**祖父类独有成员 |
| R5–R9 | 字面量 → 对应包装类成员 |
| R10 | \`null.\` → **空** |
| R11 | \`new String().\` / \`foo().\` / \`a.b().\` → 成员 |
| R12 | \`int[] a; a.\` → 含 length；不含 String 成员 |
| R13 | \`int x; x.\` → **空** |
| R14 | \`foo(\` 后 \`).\` 解析失败 → **空** |
| R-A | receiver 解析失败时**不得**出现全局类名 |
| R-B | AST 无该位置 FieldAccess → **空** |
| V1–V3 | 包私有成员在**不同包**不出现；同包出现；private 仅同类 |
| S1 | \`类名.\` 不含实例成员 |
| S2 | 静态方法内 \`this.\` → 空 |
| S3 | 不含 \`<init>\` |
| I1 | 接口默认方法出现在实现类实例补全中 |
| I2 | 子类覆盖后只出现一次 |
| I4 | 任意类实例补全含 \`toString\` |
| I5 | \`super.\` 不含祖父类独有成员 |
| G1 | 空前缀不返回全部 classpath 类 |
| K1 | \`s.\` 不含 \`null\`/\`class\` |
| K2 | \`null\` 仅表达式上下文 |
| O1 | 排序：完全匹配在前 |
| D1 | 同 label+kind 去重 |
| D2 | 上限 100 |

---

## 13. 验收

1. 单元/引擎测试：上述断言全部通过（新增 \`completion_spec\` 测试集）。
2. 模拟器：editor_sim 场景覆盖 R1–R14、V1–V3、S1–S2、I1–I5。
3. 手工验收（Sublime）：
   - \`"".\` 只出现 public/可访问成员，**不得**出现 \`checkBoundsBeginEnd\`、\`COMPACT_STRINGS\`
   - \`"".null\` → 补全列表为**空**（或仅报错），**不得**出现 \`null\` 关键字与 \`NullPointerException\`
   - \`Math.\` → 只有静态成员
   - \`int x; x.\` → 空
4. 回归：javac-core 750、javac-ide-core 746、Gson 208/208。

---

## 14. 错误处理

- 所有解析失败路径返回**空列表**，不抛出、不 panic。
- 不可访问成员静默排除，不返回占位项。
- 日志：调试构建下记录失败原因（receiver 形态 + 失败阶段），release 不输出。

---

## 15. 与现有实现的差距（必须修正）

| 现状 | 规格要求 |
|---|---|
| 成员过滤只排除 \`private\` | 按第 5 节可见性表 |
| receiver 为空时回退全局 | 返回空（R-A） |
| 无 \`super\` | R4 + I5 |
| 无接口默认方法 | I1 |
| 无静态/实例区分 | 第 6 节 |
| 无上下文分类 | 第 3 节 |
| 关键字全局列表 | 第 8.3 节上下文子集 |
| 无数组/原语/TypeVar 规则 | R12/R13/4.2 |
