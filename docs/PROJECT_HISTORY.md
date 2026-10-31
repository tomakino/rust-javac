# rust-javac 项目历程（完整整理）

> 2026-10-25 → 2026-11-01，282 个提交（`4b08a1e` → `8486e92`）。
> 目标：用 Rust 对 JDK 17（jdk17u@982d53c，17.0.19+10）的 `javac` 做 **1:1 行为等价**重实现。

## 1. 目标与红线

- **验收口径**：`.class` 逐字节一致、`-XDrawDiagnostics` 逐行一致（内容/位置/数量/顺序）、CLI 输出与退出码一致。
- **唯一依据**：官方 JDK 17 源码（`internal/reference/jdk17u`）。不近似、不兜底、不特判；发现端口与官方偏离时按官方源码修正。
- **不重蹈官方废弃/错误路线**：不为废弃机制（`-profile`、`-source 7`）和已知错误的官方行为补实现；CLI 只加非废弃、语义正确的功能。
- **历史债务要还**：未实现点（`unimplemented!`/`unreachable!`/静默占位）不以"以后再说"方式保留，逐批审计清零并登记。
- 用户偏好：全程中文；质量优先于速度；不做 `Start-Sleep` 空转；不用 adb 手势/截图估坐标验证。

## 2. 时间线（按提交历史）

### 2.1 基础设施与并行移植（P0/P0.5，10-25/26）

- 建立多 crate 结构：`javac-core`（流水线+诊断）、`rust-javac`（CLI）、`javac-tools`（javax.tools 风格 FileManager）、`javac-source`（com.sun.source）、`javac-unicode`、`javac-floating`、`javac-jimage`/`jmod`/`zip`/`ctsym`/`classpath`/`charset`/`fs`、`javac-tests`（语料/oracle）、`xtask`（资源生成）。
- 并行"util-porter"批次移植；checkpoint 恢复机制（网络中断后从工作区恢复）。
- 早期关键契约：arena id（`TreeId`/`SymbolId`/`TypeId`，ADR 0001）、服务显式传参、`.properties` 资源 `include_str!` 嵌入。

### 2.2 解析器与 tree oracle（P1）

- `JavacParser` 完整语法：record/sealed/pattern/lambda/method-ref/switch-expr/module/annotation。
- `TreeMaker`、`Pretty`、`TreeInfo`、`TreeCopier`（1849 行、~75 visitor）逐 1:1。
- tree-oracle 分层比对最终 **37/37**；反复以官方 tree 输出修正解析器细节。
- 坑：`>>>` 嵌套泛型由 `Scanner.split` 语义错误导致——`split` 应返回右半 token 并把左半存入 `prev_token`（官方 `Scanner.split`），修复后解析通过。

### 2.3 ClassReader / ClassFinder（P2）

- `ClassReader` 全部 26 类 attribute reader（Signature/Record/PermittedSubclasses/MethodParameters/LVT/Module/ModuleResolution/Runtime*Annotations/AnnotationDefault/TypeAnnotations/TypeAnnotationPosition/InnerClasses/EnclosingMethod/SourceFile/ConstantValue/Code…），并用 `javap` 做 oracle。
- `ClassFinder` 完成 `complete/completeOwners/fillIn/loadClass/includeClassFile/scan` 与 `Symbol.completer`/`DCFH`/`Symtab` 绑定。
- 注解代理（AnnotationProxyMaker）、`BadEnclosingMethod`、`ProxyType` deproxy、版本/预览 classfile 诊断。

### 2.4 Enter / MemberEnter / TypeEnter（P3）

- `Env`/`AttrContext`/`TypeEnvs`/`Enter`/`MemberEnter`/`TypeEnter` 全量；`Scope` 的 `dup/leave/import` 语义。
- `TypeEnter` 阶段（Imports/Hierarchy/Permits/Header/Record/Members）、默认构造器、record/enum 成员、`finishClass`。
- 模块系统（`Modules` 3435 行）、`ModuleFinder`、`Locations`（含 `getLocationForModule(Path)`、`--patch-module`、`--module-source-path`）、JRT/ct.sym/jmod 读取。

### 2.5 Resolve / Types / Infer / DeferredAttr（P3 后段）

- `Resolve` A+B：`findField/Var/Type/Ident`、访问检查、`selectBest`、`mostSpecific`、`mergeAbstracts`、成员引用族、多态签名、verbose 诊断、`code/Printer` 1:1。
- `Types`：关系（adapt/rewriteSupers/capture/glb/closure/asSub/…）、投影、`SignatureGenerator`、126 项审计补全。
- `Infer`：InferenceContext、图求解、`instantiateMethod/instantiateFunctionalInterface`、多态签名。
- `DeferredAttr`：DeferredType/Node/Context/Map、speculative、stuck 策略；`ArgumentAttr`（6 种实参类型、`UniquePos`）；`AttrRecover`；`StructuralStuckChecker`。
- 关键语义：`InferenceException extends InapplicableMethodException`；`ArgumentAttr.speculativeTypes` 以 `ResultInfo.id` 为键。

### 2.6 Flow / Lint / Annotate（Batch 1-3.5）

- `Flow` 完整（AliveAnalyzer/CaptureAnalyzer/AssignAnalyzer、match bindings、effectively-final 片段含嵌套 JCDiagnostic）。
- `Lint` 完整（`augment`/AugmentVisitor、`-Xlint` 解析与默认集）。
- `Annotate` 完整（annotateLater/annotateNow/TypeAnnotate/queue/flush/重复容器、合成注解类型）。

### 2.7 Attr 全量（Batch 5-6）

- 表达式：Ident/Select/Apply/NewArray/Assert/Conditional/Assignop/Unary/Binary/TypeCast/Lambda/Reference/NewClass/匿名类。
- 语句：Block/循环/Try/同步/Switch/SwitchExpr/pattern（type test/binding/guard）/Return/Throw/Yield。
- 类型：attribClass/attribClassBody、visitTypeParameter/checkIntersection、type annotations 校验。
- 依赖族：`checkAllDefined`/`nonCyclic`/cyclic constructors、override/hide/clash、`checkDeprecatedAnnotation`/`checkFunctionalInterface`/`checkLeaksNotAccessible`、`PostAttrAnalyzer`、`Analyzer`。

### 2.8 TypeAnnotations / Analyzer / B7

- `TypeAnnotations` ~2200 行：AnnotationType/targets/positions/separateAnnotationsKinds/resolveFrame/visits/organizeBodies+Signatures。
- 匿名类 diamond 监听（快照/重放）、`NewClass.classDeclRemoved`。

### 2.9 编译器后端

- `TransTypes`（2328 行：cast/coerce/retype/bridges/translateClass）。
- `TransPatterns`（~2780 行）与 `LambdaToMethod`（10241 行；indy/metafactory、去重、静态捕获）。
- `Lower`（8775 行全 visitor：StringConcat、TWR、enum/record、class literal、assert、条件、访问器）。
- `Code`（2974 行）、`Items`（1066）、`StringConcat`（778）、`Gen`（4333）、`ClassWriter`（2261）、`PoolWriter`、`CRTable`、`StackMapTableFrame`（370）。
- `JNIWriter` 1:1。

### 2.10 装配与模块

- `mini_compiler` 装配 JavaCompiler 链：parse → modules → enter → process → attr → flow → desugar(transTypes→TransPatterns→LambdaToMethod→Lower) → gen；`should-stop` 策略（PARSE/ENTER/PROCESS/ATTR/FLOW/TRANSTYPES/TRANSPATTERNS/UNLAMBDA/LOWER/GENERATE）。
- 模块测试（module-info uses/provides）真实端到端通过；`java.home` 发现顺序修正（JAVA_HOME → PATH → 基线 JDK 17）。

### 2.11 债务清理（10-27/28）

- `unimplemented!` 从 ~349 逐批降到 **0**；`unreachable!` 102 → **0**（统一 `util/assert` 的 Assert.error 语义）。
- `let _` 清理 424 → 412（保留有依据的）；panic 分类表（`AUDIT_PANIC_TABLE.md` 237 行）。
- 诊断审计（935 个工厂）：wired 639 / 可接线 67 / 未移植 40 / 范围外 24 / 官方无触发 165。

### 2.12 真实项目攻坚（Gson / okhttp）

- **Gson 2.10.1（80 源）**：从 161 个诊断/0 class → 13 → 10 → **0 错误、207 class 逐字节一致**；随后 Gson JUnit 1258/2（与官方一致）。
- **okhttp 3.14.9（229 源）**：0 错误；okhttp 官方 JUnit 默认 4 类 416 tests、internal 15 类 256 tests 全绿。
- 关键根因：`Infer.instantiateMethod` 自加参数检查、`finishImports` 时序、`ResolvedConstructorType.overloadCheck` 非官方特判、`InferenceException` 捕获、`speculativeTypes` 身份键。

### 2.13 诊断语料库

- 官方 langtools 669 个诊断示例，与固化 expected 归一化比对：547 → 612 → **602 通过 / 0 失败 / 67 跳过**。
- 67 skip 白名单：annotation-processing 47、deprecated-source 12、custom-file-manager 2、modules-automatic 2、jvm-stacktrace 1、plugin 1、verbose 1、deprecated-profile 1。

### 2.14 CLI 对齐

- 官方帮助/扩展帮助/版本/无参/非法选项/文件不存在/退出码；`@argfile`（`#` 注释、空白分隔）。
- `--help-lint`（31 key + all/none，逐字节一致）、`-Xstdout`（含 CRLF 的 `LineSeparatorWriter`）、`-J<flag>`（含 `-J-Duser.language` 影响诊断）。
- 诊断语言：Windows UI 语言检测；`-J-Duser.language` 注入进程默认 locale。
- 移除废弃机制：`-profile`、`-source 7` 对应语料标记 skip，不实现。

### 2.15 性能

- 坑：仓库根 `-cp .` 扫描 `internal/reference` 导致单文件 45 s；跳过 `internal/reference` 后 1.26 s（干净目录 0.70 s）。
- release：fat LTO + codegen-units=1；`target-cpu=native`（本机基准）。
- 优化：`Name` 去 bytes（共享 name table，修正版通过）、`FxNameMap`（预分配）、`Symtab` flat-name 索引、inapplicable 失败通道 Result 化（-17.5%）、`javac-zip` 整文件驻留 + rawzip slice + zlib-rs（-16~21%）。
- 结果（2026-10-31）：Gson release 0.973 s（javac 2.40 s）；okhttp release 24.28 s（javac 8.17 s）。E2E 计划记录 Gson release 中位数 **0.82 s vs javac 2.44 s**。
- 不可行方案登记：PGO（缺 profiler_builtins）、mimalloc（缺完整 MinGW/Windows SDK）、samply（ETW GBK 崩溃，本地补丁不入库）。

### 2.16 字节一致性攻坚（okhttp 69 → 0）

- `InnerClasses` 缺被引用成员类（`SignatureGenerator.classReference`→`enterInner`，合并签名类引用）。
- lambda 合成方法 `LineNumberTable` 行号（lambda 体定位到 lambda 表达式；官方默认关闭 dedup）。
- `EnclosingMethod` 用 lambda 原始外层方法；本地类名索引键用外层类 flatname。
- 常量池 key：`Symbol.poolKey = Pair(newOwner, baseSymbol())`——继承接口方法按调用点克隆、mergeAbstracts 用身份 key、import 作用域克隆补 `baseSymbol`。
- `try-with-resources` 异常路径 `make.pos` 恢复；字符串 switch 合成节点定位到 switch 语句。
- 最终：**okhttp 547/547 字节一致，Gson 207/207 字节一致**。

### 2.17 E2E 计划与门禁

- `docs/e2e-test-plan.md` + `scripts/e2e.ps1`：L1 单元（846）、L2 诊断语料（669）、L3 项目字节（Gson/okhttp）、L4 CLI、L5 性能。
- 每次提交 L1 全绿；每日 L2+L3+L4；发布前 L1-L5。

### 2.18 诊断顺序攻坚（本会话，2026-11-01）

问题：okhttp 诊断内容/位置/数量已一致，但 2 条 warning 相对顺序不同（`RecordedRequest.java:139:17`、`FakeSSLSession.java:73:10`）。

**此前的"HashMap 迭代序 → 跨语言不可复现"结论被证明是错的。** 真实根因（按官方源码修复）：

1. 端口缺 `TypeEnter` 阶段队列语义：`Phase.completeEnvs` 的 `firstToComplete`、`AbstractMembersPhase` 的 `todo/completing`、`HierarchyPhase` 作为 class completer（`TypeEnter.java:249-1022`）；`Phase` 结构、`topLevelPhase`、`HierarchyCompleter` 全部落地。
2. `Resolve.member_scope` 未实现 `Symbol.members()`（任何带 completer 的符号先 complete），`Http2Reader` 因此延迟到测试类才 append（#153 vs javac #73）。
3. `TypeEnter.complete` 在 completion 关闭时未重装 completer（官方 `sym.completer = this`）。
4. `MemberEnter` 未完成成员类型引用的类（官方 `attribType` 的完成副作用）。
5. 延迟 lint 队列键：官方以 `JCTree` 对象身份（`tree.pos()`）为键，端口用数值位置，跨文件同偏移会误 flush（出现 `FileSystem.java:57:9` 虚假诊断）。
6. `Check.checkDeprecated` 应为 deferred；`Attr.visitMethodDef/visitVarDef` 需在声明期 `setPos(tree.pos())` 并在 finally `flush(tree.pos())`；类 flush 移到 `attribClassBody` 之后。
7. `warnOnExplicitStrictfp` 去掉端口自加的 `setPos`（官方只 report，键由调用方设置）。

结果：okhttp `todo` append 顺序 **229/229**、`-XDrawDiagnostics` **11/11 行逐行一致（positional diff 0）**；Gson 207/207、okhttp 547/547、846 lib、E2E 8/8 全部保持。

## 3. 关键技术决策

| 决策 | 内容 | 依据 |
|---|---|---|
| arena id | `TreeId`/`SymbolId`/`TypeId` 代替对象引用 | ADR 0001 |
| 服务显式传参 | 无全局 Context；关键服务显式传入 | ADR 0001 |
| 诊断资源 | 官方 `.properties` `include_str!` 嵌入 + 类型化工厂 | `compiler.properties` |
| 中文补丁 | `compiler_zh_CN.properties` 追加 24 条，覆盖 936/936 | 本地化 |
| 重入适配 | 字段级 `Cell/RefCell` + 短借用；`complete_with_attr` 贯穿 Attr 链 | Java 单线程重入 vs Rust 借用 |
| 官方源码标注 | 文件头 `Origin: … @ 982d53c`；`docs/PORTING_MAP.md` 记录映射 | 可审计 |
| 资源读取 | JRT/ct.sym/jmod/zip 常驻内存 + 索引 | 性能 |

## 4. 验证矩阵（当前）

| 项目 | 结果 |
|---|---|
| Gson 字节 | **207/207** |
| okhttp 字节 | **547/547** |
| okhttp `-XDrawDiagnostics` | **11 行，逐行一致** |
| okhttp append 顺序（探针） | **229/229** |
| `javac-core --lib` | **846 passed, 0 failed** |
| Java 9–17 feature-matrix（23 单元） | **21/23**（2 个已知差异见 `testdata/feature-matrix/README.md`） |
| CLI 集成（cli/cli_parity/diag_examples/diagnostic_corpus） | 5 / 18 / 1 / 1 全过 |
| diag-corpus | **602 pass, 0 fail, 67 skip**（共 669） |
| `scripts/e2e.ps1 -IncludeOkhttp` | **8/8** |
| Gson release 性能 | **0.82 s vs javac 2.44 s**（5 次中位数） |

## 5. 已知缺口（明确不实现或暂不纳入门禁）

1. JVM 宿主机制：注解处理器（`-processor`/`-proc:only`）、`-Xplugin`、`-doe` JVM 栈、自定义 `-filemanager`——本质不可行，语料 skip。
2. 官方废弃机制：`-profile`、`-source 7`——不实现，语料 skip。
3. 67 个 diag-corpus skip（见 §2.13）。
4. 性能：okhttp release 仍慢于 javac（24.28 s vs 8.17 s，2026-10-31 口径），Gson 已反超。
5. 可选增强：okhttp 11 行诊断加入 L2 门禁命令；实时对照模式（不依赖固化 expected）；CI。

## 6. 方法论与教训

- **根因优先**：先证据链（javac 侧补丁打印 + 端口探针），再改代码；一次只动一个变量。
- **官方源码是唯一裁判**：所有修复都指向具体文件/方法/行为；端口自加适配必须标注理由，能被官方语义推翻就推翻。
- **不迷信已有结论**：`HashMap` 排序结论被本会话推翻——诊断顺序实际由 TypeEnter 阶段队列与延迟 lint 键决定。
- **探针用完即删**：临时 `eprintln!`、`DSH_TRACE_*` 必须清理，避免污染诊断/性能。
- **隔离 cwd 再测性能**：`-cp .` 会扫描仓库参考树。
- **历史债务逐批清零**：`unimplemented`/`unreachable`/静默占位不留尾巴。
- **测试与文档同步**：E2E 计划、门禁、skip 白名单、方案失败记录（PGO/mimalloc/samply）都落盘。

## 7. 提交索引（按主题）

| 主题 | 代表提交 |
|---|---|
| P0 基础设施 | `4b08a1e` |
| 解析器/tree oracle | `fe31f28`、`9a04e88` |
| ClassReader/ClassFinder | `76d0437`、`4972554`、`ac2da35` |
| Enter/TypeEnter | `c242a3c`、`50aedec`、`4af90e3` |
| Resolve/Types/Infer | `1be2159`、`62a76ec`、`ff75f6d` |
| Flow/Lint/Annotate | `b2d12f2`、`66cfcbe`、`a66c58c` |
| Attr 全量 | `3c17e8e`、`68eefac` |
| 后端 | `b34ebd2`、`5778054`、`d7650b7`、`2f8f51d` |
| 债务清零 | `f496fe9`、`5b664fe` |
| Lambda/Pattern | `4973fa9`、`9356682` |
| Gson 字节一致 | `b979d25` |
| okhttp 全绿 | `e0a5d1f`、`f0f0894` |
| CLI | `3758e21`、`ce38d06` |
| 性能 | `393a588`、`b9a47e5`、`b34685e` |
| 字节差异攻坚 | `77dac48`、`b4eb1f4`、`b02f67a` |
| 诊断顺序攻坚 | `8459da1`、`0e16acd` |
| 测试 harness | `8486e92` |

## 8. 下一步（建议，非承诺）

- 把 okhttp `-XDrawDiagnostics` 11 行逐行对比加入 L2 门禁命令。
- okhttp release 性能基准纳入 L5，建立回归阈值。
- diag-corpus 实时对照模式（防止 JDK 升级后 expected 漂移）。
- feature-matrix 的 2 个已知差异：TYPE_USE 参数注解的调用点参数计数、默认构造器 LineNumberTable 行号。
- CI 串联 L1-L4。
