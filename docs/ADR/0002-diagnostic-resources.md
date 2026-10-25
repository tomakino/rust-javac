# ADR 0002:诊断资源类型化访问器生成

- 状态:已接受(Lead 审核通过;修订 A-G 见文末"审核修订记录")
- 基线:openjdk/jdk17u @ 982d53c (jdk-17.0.19-ga)
- 相关:ADR 0001(arena + ID);docs/PLAN.md;docs/PORTING_RULES.md

## 背景

javac 的诊断消息在构建期从资源文件生成"类型化访问器"类,编译器源码用这些访问器
构造诊断(如 `log.error(Errors.PluginNotFound(name))`)。当前 rust-javac 只有原始消息表
(`resources/generated_compiler.rs` 的 `COMPILER_MESSAGES`),诊断类(JCDiagnostic/Log)
尚未移植(P4 前置)。

### 1. 上游资源与生成器

- 输入:`src/jdk.compiler/share/classes/com/sun/tools/javac/resources/compiler.properties`
  (3829 行,936 个 `compiler.<kind>.<code>` 键;kind 分布:err 471 / warn 129 / note 32 /
  misc 304)。
- 每条消息的上一行(紧邻)是类型注释 `# 0: symbol, 1: type`,描述 `{0}`、`{1}` 占位符的
  类型;没有类型注释的消息无参数(391 条无参数,545 条有参数)。参数个数分布:
  1:286 / 2:152 / 3:55 / 4:33 / 5:8 / 6:6 / 7:4 / 8:1。
- 构建规则:`make/common/modules/GensrcCommonLangtools.gmk` 的 `SetupParseProperties`
  对 `compiler.properties`、`launcher.properties` 调用
  `propertiesparser.PropertiesParser`;后者用 `parser.Message*` 解析、`gen.ClassGenerator`
  按 `resources/templates.properties` 模板生成 `CompilerProperties.java` /
  `LauncherProperties.java`。
- 生成类:`com/sun/tools/javac/resources/CompilerProperties.java`,包含 4 个嵌套类
  `Errors` / `Warnings` / `Notes` / `Fragments`,每个键对应一个静态工厂方法(有参数)或
  静态字段(无参数),返回 `JCDiagnostic.Error` / `Warning` / `Note` / `Fragment`。
- 工厂命名(`ClassGenerator.factoryName`):key 按 `[.-]` 切分,跳过前两段(文件前缀与
  kind),其余每段首字母大写后拼接;`compiler.err.plugin.not.found` → `PluginNotFound`;
  16 个键含 `-`,同样作为分隔符(如 `compiler.warn.missing-explicit-ctor` → `MissingExplicitCtor`)。
- 参数类型(`parser.MessageInfo`):类型注释按 `", "` 分段,每段 `N: <type>`;类型可以是
  `MessageType.SimpleType`(30 种,见下表)、`CompoundType`(`list of X` / `set of X` /
  `collection of X`)、`UnionType` 别名(`message segment` = diagnostic|fragment、
  `file name` = file|path|file object)或显式 `or` 联合(如 `symbol or type`);自定义类型用
  `'...'` 书写(compiler.properties 中未使用)。union 会被 `ClassGenerator.normalizeTypes`
  展开为多个重载。
- 诊断键对象:`JCDiagnostic.DiagnosticInfo{DiagnosticType type, String prefix, String code,
  Object[] args}`,子类 `Error` / `Warning` / `Note` / `Fragment`;
  `key() = prefix + "." + type.key + "." + code`。
- `SetupCompileProperties` 另外把所有 properties 生成为 `ListResourceBundle` 类
  (compiler.java / javac.java / launcher.java 等,非类型化),不属于本 ADR 范围。

### 2. 当前 rust-javac 现状

- `crates/javac-core/src/resources/generated_compiler.rs`:只有 936 条 `(key, value)` 消息表,
  没有类型注释、没有访问器。
- `JCDiagnostic` / `Log` / 诊断类型未移植;但 `javac-core` 已有可复用的值类型:
  `util::name::Name`(Display/Debug/Clone)与 `util::java_string::JavaString`(Display/Debug/Clone)。

## 决策

### 1. 生成器扩展(xtask gen-resources)

`cargo xtask gen-resources` 从"只生成消息表"扩展为三阶段:

1. **解析**:用 Java `Properties` 语义读取 `compiler.properties`(复用现有 `parse_properties`);
   按行扫描,把每个键与其紧邻上一行的类型注释配对;类型注释按 `MessageInfo` 规则解析为
   类型表达式(Simple/Compound/Union/Custom/Unknown)。
2. **生成**:写出 `crates/javac-core/src/resources/compiler_properties.rs`:
   - 4 个模块 `errors` / `warnings` / `notes` / `fragments`(对应 `Errors` / `Warnings` /
     `Notes` / `Fragments`);
   - 每个键一个 `pub fn`(无参数也是零参函数),返回 `ErrorInfo` / `WarningInfo` /
     `NoteInfo` / `FragmentInfo`;
   - 命名:Rust `snake_case`(由 Java 工厂名机械转换),文档注释保留 `Java: Errors.Xxx`、
     参数类型与消息模板;
   - 元数据:`COMPILER_MESSAGES: &[(&str, &str)]`(保留现有消息表能力)、
     `COMPILER_KEYS: &[(&str, &str, &str, usize)]`(key、kind、Rust 名、参数个数);
   - 确定性:BTreeMap 排序、固定 LF 换行、无时间戳,同一输入两次生成字节相同。
3. **校验**:递归扫描 `reference/jdk17u` 的 javac 源码,提取
   `(Errors|Warnings|Notes|Fragments).<Name>` 调用点(现有 906 行引用、735 个不同名字),
   校验:名字在生成表中存在、调用参数个数等于生成签名参数个数、无参字段引用对应零参。
   任何不一致都让 `gen-resources` 失败(防止生成器与上游用法漂移)。

### 2. Rust 侧诊断契约(resources/diagnostic_contract.rs)

P4 移植 `JCDiagnostic`/`Log` 之前,先落地最小可编译契约(本 ADR 的一部分);P4 时把契约
重定向/替换为真实 `util::jc_diagnostic` 类型,替换点集中在该文件与 xtask 的类型映射表。

- `DiagnosticType { Error, Warning, Note, Fragment }`
- `DiagnosticInfo { prefix: &'static str, kind: DiagnosticType, code: &'static str,
  args: Vec<DiagnosticArg> }`,`key()` 按 Java 规则拼接
- `ErrorInfo` / `WarningInfo` / `NoteInfo` / `FragmentInfo`(newtype 包装 `DiagnosticInfo`)
- `DiagnosticArg`:P4 前为占位值(保存 Display 文本);`IntoDiagnosticArg` trait 把参数装箱
- 复用真实类型 `Name` / `JavaString`;其余 Java 诊断参数类型 → opaque newtype
- `list/set/collection of X` → `Vec<X>`;`unused` → `()`;`Vec<T>` 的参数也实现装箱

Java 类型 → Rust 契约类型映射表:

| Java 类型注释 | Rust 类型 | 说明 |
|---|---|---|
| `symbol` | `SymbolArg` | P4 后指向 SymbolId/Symbol |
| `type` | `TypeArg` | P4 后指向 TypeId/Type |
| `name` | `Name` | 复用 `util::name::Name` |
| `string` | `JavaString` | 复用 `javac-strings` |
| `symbol kind` | `SymbolKindArg` | |
| `kind name` | `KindNameArg` | |
| `number` | `i32` | Java `int` |
| `boolean` | `bool` | |
| `unused` | `()` | Java `Void` |
| `object` / `fragment` / `diagnostic` | `DiagnosticArg` | |
| `source` / `target` / `token` / `modifier` / `flag` / `url` / `option name` / `profile` / `annotation` / `source version` / `tree tag` | 对应 `SourceArg` / `TargetArg` / `TokenArg` / `ModifierArg` / `FlagArg` / `UrlArg` / `OptionArg` / `ProfileArg` / `AnnotationArg` / `SourceVersionArg` / `TreeTagArg` | opaque 占位 |
| `file` | `FileArg` | opaque 占位;P4 后指向 java.io.File |
| `path` | `PathArg` | opaque 占位;P4 后指向 java.nio.file.Path |
| `file object` | `FileObjectArg` | opaque 占位;P4 后指向 JavaFileObject |
| `list of X` / `set of X` / `collection of X` | `Vec<X>` | |
| `message segment` / `file name` / `X or Y` / 自定义 `'...'` / `<unknown>` | `DiagnosticArg` | union 退化为统一参数 |

### 3. 与 javac 生成类的差异(登记 PORTING_RULES §9)

| 方面 | javac 生成类 | rust-javac |
|---|---|---|
| 无参消息 | `public static final Error XXX = ...` 字段 | 零参函数(参数个数仍为 0) |
| union 参数 | 展开为多个重载 | 单函数,union 参数类型为 `DiagnosticArg`(方法名/参数个数与每个重载一致) |
| 返回类型 | 4 个 `JCDiagnostic` 子类 | 4 个 Info newtype(P4 后映射到 JCDiagnostic) |
| 参数类型 | Java 类型(import 真实类) | P4 前 opaque 占位;P4 后替换 |
| 类名 | `CompilerProperties.Errors` 等嵌套类 | `compiler_properties::errors` 等模块 |

### 4. 一致性验证

- 生成器内:调用点扫描(906 行 / 735 个不同名字)校验名字与参数个数,任一不一致即失败。
- xtask 单测:类型注释解析(普通/compound/union/别名/带注释)、工厂名转换(含 `-` 键与
  数字段)、参数类型映射、生成确定性、以及真实 `compiler.properties` 的数量断言
  (936 键;err 471 / warn 129 / note 32 / misc 304)。
- javac-core 侧单测:生成的访问器可编译、可调用;`COMPILER_KEYS` 数量与抽样键一致。

### 5. 输出与后续

- 删除旧 `generated_compiler.rs`(内容并入新文件),更新 `resources/mod.rs`。
- 后续:`launcher.properties`(15 键)用同一生成器生成 `LauncherProperties`;P4 移植
  `JCDiagnostic`/`Log` 后接入真实诊断类型;消息格式化(`MessageFormat`/`{n}` 占位、
  `''` 转义)与本地化属于 P4。

## 审核修订记录(Lead 审核通过时提出,已落实到实现)

- **A. union 参数**:允许"单函数 + DiagnosticArg"方案。登记为 P4 改进项:接入真实
  JCDiagnostic 后,要么恢复类型区分(泛型/trait 约束或按具体类型重载),要么在每处调用点
  保留显式类型断言;生成器的调用点校验以"参数个数集合"校验(同一消息占位符数固定,
  union 展开不改变个数;实现中该集合通常只有一个元素)。
- **B. file / path / file object 不合并**:分别使用 `FileArg` / `PathArg` /
  `FileObjectArg`(P4 前均为 opaque),保留 Java 类型区分;一条消息同时出现多者时各自独立。
- **C. P4 替换契约保留对象语义与身份**:P4 替换时诊断参数必须是原始对象(arena
  ID/真实类型),不得以预先格式化的文本替代。替换点为
  `crates/javac-core/src/resources/diagnostic_contract.rs`(类型定义)与
  `xtask/src/gen_resources.rs` 的 `SIMPLE_TYPES` 映射表(生成签名);验收方式为 P4 诊断
  格式化与 javac 逐字对齐(用 javac-tests 差分基座对 stdout/stderr 做字节级比较)。
- **D. 调用点扫描器稳健性**:扫描器按字符状态机跳过字符串/字符/注释,跨行调用与任意
  嵌套括号按括号层级计数参数;字段访问(无括号)按零参校验;失败信息包含
  `文件:行`。扫描器对跨行、嵌套括号、字段访问、注释/字符串中的伪引用分别有单测。
- **E. 生成文件头**:固定为
  `// GENERATED by cargo xtask gen-resources from com/sun/tools/javac/resources/compiler.properties @ 982d53c (jdk-17.0.19-ga) — DO NOT EDIT.`
- **F. 兼容与登记**:新生成文件保留 `COMPILER_MESSAGES` 消息表能力;`resources/mod.rs`
  更新声明;生成物变化需登记到 docs/STATUS.md(由 Lead 统一更新,或由本任务在汇报中列出)。
- **G. 生成器额外断言**:参数个数等于类型注释段数;无参键数 391、总数 936 与
  err 471 / warn 129 / note 32 / misc 304 分布均在生成阶段断言。
  注:上游 compiler.properties 实测有 29 个键的值含 `{n}` 但紧邻上一行不是类型注释
  (被空行/普通注释隔开);upstream PropertiesParser 对它们生成零参工厂(dummyInfo),
  本生成器保持上游行为并打印警告计数,调用点校验作为兜底(见修订 D 的校验)。

## 实现结果(实际数据)

- 生成 `crates/javac-core/src/resources/compiler_properties.rs`:8465 行(约 437KB),936 个
  访问器(Errors 471 / Warnings 129 / Notes 32 / Fragments 304;无参 391,有参 545)。
- 调用点校验:扫描 javac 参考源码的 (Errors|Warnings|Notes|Fragments) 引用,得到 932 个
  引用、718 个不同工厂名,名字与参数个数 0 处不一致(生成阶段作为硬校验)。
- 生成器警告:29 个键的值含 {n} 但紧邻上一行不是类型注释(上游 PropertiesParser 同样按
  零参工厂生成);其余键:参数个数 = 类型注释段数。
- xtask 单测 10 个:java.util.Properties 解析、类型注释解析/类型映射、工厂命名(含 '-' 键、
  数字段、Rust 关键字转义)、生成确定性、上游快照统计断言、调用点扫描器(跨行/嵌套括号/
  字段访问/注释与字符串中的伪引用)、真实 javac 调用点校验。
- javac-core 单测:生成的访问器可编译可调用(类型化 ErrorInfo/SymbolArg 等),COMPILER_KEYS
  936 条与抽样键一致。
- 生成文件头带 `GENERATED ... — DO NOT EDIT`;生成器对 8 参数消息生成
  `#![allow(clippy::too_many_arguments)]`(模块级)以保持 clippy 零诊断。

## 结果

- 936 个类型化访问器,名称与参数个数与 javac 生成类一致,并能用上游调用点静态验证;
- P4 前占位契约集中、可编译,替换点明确;
- 代价/风险:union 重载差异已登记;参数类型 P4 前为占位;生成文件从 942 行增长到约
  2000 行(生成物,可接受)。
