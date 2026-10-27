# rust-javac 实施计划

- 状态:待项目负责人确认
- 版本:v0.1(草案)
- 基线与参考:openjdk/jdk17u commit `982d53c227943a511e8d61694feced764b71a6c1`(tag `jdk-17.0.19-ga`,即 `jdk-17.0.19+10`),与本机 javac 17.0.19(Temurin)同源。
- 唯一参考实现:上述 commit 的 `src/jdk.compiler`、`src/java.compiler`。

## 1. 目标

1. 用 Rust 实现与 JDK 17 javac **行为完全一致**的 Java 编译器。
2. 纯 Rust、零 JVM/JDK 进程依赖:不调用 `java`/`javap`/jrt-fs,不加载 JVM;所有类库符号来自文件(jimage/jmods/ct.sym/jar/目录)。
3. 可作为库被其它程序调用(对齐 javax.tools API)。
4. 开源:GPLv2-only + Classpath Exception;达到首个可编译里程碑后公开(远程仓库暂不发布)。
5. 可升级:所有代码可回指上游文件/行,升级 = 上游 diff 驱动的重移植,不靠记忆重写。

## 2. 完全对齐的判定标准

### 2.1 必须对齐
- 命令行:选项集合(按 §3 裁剪清单)、退出码、stdout/stderr 输出、诊断文本(逐字)、诊断位置(行/列/行列范围)、诊断顺序。
- 编译语义:解析、错误恢复、符号解析、类型检查、类型推断、重载解析、常量折叠、流分析、泛型擦除、lambda/方法引用、内部类、record/sealed/switch 表达式/文本块/模式匹配 instanceof、preview 特性、模块系统、注解处理(不做,见 §3)。
- 语言版本:`-source`/`-target`/`--release` 支持 7..17,以及各版本差异与错误提示。
- 生成 class:语义与 javac 输出等价——JDK 17 `java` 运行结果一致、`javap` 可解析;类/方法/字段/属性/指令序列按移植逻辑生成,不做额外优化或改动。
- API 形态:`javax.tools.JavaCompiler`/`JavacTask`(com.sun.tools.javac.api)行为对齐。

### 2.2 不要求 / 例外
- 不要求 .class 与 javac 逐字节一致(差分测试以行为等价 + javap 归一化对比为主)。
- 诊断文本比较允许固定规则的归一化:绝对路径、临时目录名、平台换行符、平台编码、时间戳等平台噪声(规则固定并测试)。
- 已明确裁剪的功能(§3)属于登记在案的偏离,不与 javac 对齐。
- 崩溃/安全类上游缺陷不照抄,需登记并加测试。

### 2.3 已知 bug/怪癖策略
javac 17 的错误恢复、警告去重、流分析边界、接受/拒绝边界、字节码生成的实现细节等,全部照抄;仅崩溃/安全例外。理由:升级路径依赖上游 diff 对照。

## 3. 范围与裁剪清单

### 3.1 不做
- 注解处理器(APT):不移植 `com.sun.tools.javac.processing`,不实现 `javax.annotation.processing` 的处理器加载与运行。javac 无处理器路径的行为需模拟对齐(默认不扫描处理器),差异登记。
- 注解反射代理(`AnnotationProxyMaker`/`java.lang.reflect.Array`/`java.lang.annotation.Annotation` 动态代理):以 §9 显式适配实现。`javac_tools::jdk::Class` 句柄携带注解类型 binary name(方法可见行为:getName 匹配);`AnnotationProxy` 以 (typeName, 有序 element values) 表示 dynamic-proxy return form(primitive/String/Class 名/枚举/嵌套注解/数组/ExceptionProxy);`java.lang.reflect.Array` 以 `Vec` 表示。不引入 JVM 反射,行为按 AnnoConstruct/Symbol.java 分支等价。
- 远程 Git 发布(暂缓,里程碑后决定)。
- crates.io 发布(暂缓)。

### 3.2 命令行裁剪(SUPPORT_MATRIX 将逐项维护)
保留:源文件、@argfile、`-d`、`-classpath/-cp`、`-sourcepath`、`--module-path/-p`、`--upgrade-module-path`、`--system`、`--release`、`-source`、`-target`、`-encoding`、`-g`/`-g:none`、`-parameters`、`-verbose`、`-nowarn`、`-deprecation`、`-Xlint`/`-Xlint:all`、`-Werror`、`-Xmaxerrs`、`-Xmaxwarns`、`-Xdiags`、`-XdrawDiagnostics`、`-XDshould-stop.at` 等测试必需 -XD 项、`-version`、`-fullversion`、`--help`、`--help-extra`、`-X`、`--enable-preview`、`--module/-m`、`--add-modules`、`--add-exports`、`--add-reads`、`--patch-module`、`--module-source-path`、`-implicit`。
裁剪:APT 相关(`-processor*`、`-proc:*`)、`-h`、`-s`、`-J*`、javadoc 专有/废弃项(如 `-bootclasspath`、`-profile`、`-doclet` 等,以 `Option.java` 逐项核对)、其余 `-X` 内部调试项、`com.sun.tools.javac.launcher`(单文件源码启动器,不属于 javac 命令;且需 JVM 执行能力,超出编译器范围)。
约束:被裁剪选项不伪造兼容;传入时明确报"rust-javac 未支持"并退出;清单随 P6 依据上游 `Option.java` 定稿。

### 3.3 待确认
- 无(launcher 已确认裁剪,见 §3.2)。

## 4. 工程结构与目录

```
Cargo.toml                workspace,resolver 3,edition 2024
crates/
  javac-tools/            javax.tools 接口与常量(trait)
  javac-core/             核心;src 下按 javac 13 个包 1:1 建模块:
                          api code comp file jvm launcher main model
                          parser platform processing tree util
  rust-javac/             命令行入口(bin)
  javac-ffi/              cdylib(后期;预留)
  javac-tests/            差分测试 harness
docs/                     中文文档:PLAN/ARCHITECTURE/ROADMAP/PORTING_RULES/
                          UPGRADE/SUPPORT_MATRIX/API_MAPPING/STATUS/ADR
tools/                    脚本:拉取上游、生成诊断资源、生成映射清单、差分测试
reference/                上游源码(不入库,脚本按 commit 拉取)
```

- `javac-util` 不单独成 crate:util 包多个文件反向依赖 `code`/`tree`/`main`/`api`(`Options`→`main.Option`、`Log`→`main.Main`、`JCDiagnostic`→`tree`、`AbstractDiagnosticFormatter`→`code`/`file`/`jvm`),独立 crate 会成环。
- `javac-core` 内部模块与包一一对应;单个 Java 源文件对应单个 Rust 文件(如 `util/Context.java`→`core/src/util/context.rs`),文件头只写来源锚点(版权头由项目负责人统一添加)。

## 5. 核心设计决策(映射规则摘要,细则见 PORTING_RULES.md)

| Java 表达 | Rust 表达 |
|---|---|
| 类/继承(`JCTree`/`Symbol`/`Type` 层次) | 外层 `struct` 放公共字段 + `enum` kind 放子类字段 + `match`;访问者→trait |
| 对象引用图(共享可变/循环) | arena + ID:`SymbolArena/SymbolId`、`TypeArena/TypeId`、`TreeArena/TreeId`、`ScopeArena/ScopeId` |
| Context 服务单例 | `CompilerContext` 持有全部状态;核心禁止 `Rc<RefCell<...>>`(CI 检查),仅 api/ffi 层白名单 |
| `CompletionFailure`(可捕获/重试) | `Result<T, CompileError>` 显式传播 |
| `Abort`/`FatalError`(不可恢复) | `panic!`;仅入口与 FFI 边界 `catch_unwind` |
| `javac.util.List` | 自实现同语义不可变链表(Rc 节点,共享 tail),算法照抄 |
| `HashMap`/`LinkedHashMap` | 对应语义;迭代顺序影响输出处必须保持 |
| `String`/`char` | 内部统一 UTF-16 code unit(`Vec<u16>`),位置/长度/字符判断按 Java 语义 |
| 诊断消息 | 从上游 `compiler.properties` 等生成 Rust 资源表;Formatter 类逐类移植 |
| 内部类/匿名类 | 嵌套 struct + 显式 `ctx` 参数;单方法匿名类→闭包/函数 |
| 反射/ServiceLoader | 不依赖;平台描述符用注册表 |
| 未实现路径 | `unimplemented!` + manifest 标记;禁止静默降级 |

## 6. 阶段计划

每阶段完成定义:上游对应文件全部移植(或按裁剪登记)、模块单测、差分子集通过、manifest 更新、无 TODO、STATUS.md 更新。

### P0 基础设施
- workspace、trait 接口层(`javac-tools`:FileObject/JavaFileObject/JavaFileManager/Diagnostic/StandardLocation/SourceVersion 等)、`util` 包 41 文件(Context/List/ListBuffer/Pair/Name/Names/SharedNameTable/UnsharedNameTable/Position/Options/Diagnostics/Log/Formatter 系列/JavacMessages/Convert/FloatUtils 等)。
- 工具链:拉取上游、诊断资源生成器、Java↔Rust 映射清单生成器、差分测试基座、lint 规则。
- 验证:单测;诊断格式化输出与 javac 对照(利用上游 `-XDrawDiagnostics`/资源表)。

### P1 词法与语法
- `parser` 14 文件(UnicodeReader/JavaTokenizer/Tokens/Scanner/JavacParser/TextBlockSupport/DocCommentParser/JavadocTokenizer/ReferenceParser/ParserFactory/Lexer/Parser)。
- `tree` 12 文件(JCTree/TreeMaker/TreeInfo/TreeScanner/TreeTranslator/TreeCopier/Pretty/DCTree/DocTreeMaker/DocCommentTable/EndPosTable/LazyDocCommentTable/DocPretty)。
- `com.sun.source` 接口(trees/doctrees/util)。
- 验证:用 `-XDshould-stop.at=PARSE` 对照语法错误诊断;Pretty 输出对照 AST。

### P2 类文件读取与类路径
- `jvm`:ClassReader/ClassFile/ByteCodes/PoolReader/ModuleNameReader/Target/Profile/CRTFlags 等。
- `file`:JavacFileManager/Locations/PathFileObject/BaseFileManager/FSInfo/CacheFSInfo/JRTIndex/RelativePath。
- jimage(lib/modules)解析、jar/目录类路径、platform 包。
- 验证:读取本机 JDK 17 全部类签名,与 `javap`/javac 行为对照;类路径诊断对照。

### P3 符号与类型系统
- `code` 包 26 文件(Symbol/Type/Types/Symtab/Scope/Flags/Kinds/Source/Lint/Attribute/AnnoConstruct/ClassFinder/ModuleFinder/Directive/TypeAnnotations/TypeAnnotationPosition/TargetType/Preview/Printer/…)。
- 验证:类型/符号与 javac 内部行为对照(经 `-XDshould-stop.at=ENTER/PROCESS` 诊断);类型系统单测。

### P4 语义分析
- `comp` 包 30 文件(Enter/MemberEnter/TypeEnter/Attr/ArgumentAttr/Check/Flow/Resolve/Infer/InferenceContext/DeferredAttr/Operators/ConstFold/Annotate/Analyzer/AttrRecover/MatchBindingsComputer/TransTypes/Lower/TransPatterns/LambdaToMethod/TreeDiffer/TreeHasher/Todo/CompileStates/AttrContext/AttrContextEnv/Env/TypeEnvs/Conditional…)。
- 验证:`-XDshould-stop.at=ATTR/FLOW/TRANSTYPES/LOWER` 逐阶段诊断对照。

### P5 代码生成
- `jvm` 其余(Gen/Code/PoolWriter/PoolConstant/StringConcat/CRTable/ClassWriter/Items/JNIWriter/UninitializedType)+ `main/JavaCompiler` 驱动。
- 验证:端到端产出 class;JDK 17 java 运行结果对照;javap 归一化对比。

### P6 命令行与 API
- `main` 8 文件(Arguments/Option/CommandLine/Main/JavacToolProvider/OptionHelper/DelegatingJavaFileManager)+ `api` 13 文件(BasicJavacTask/ClientCodeWrapper/DiagnosticFormatter/Entity/Formattable/JavacScope/JavacTaskImpl/JavacTaskPool/JavacTool/JavacTrees/Messages/MultiTaskListener/WrappingJavaFileManager)+ `model` 4 文件。
- CLI 裁剪清单定稿;SUPPORT_MATRIX 发布。
- 验证:命令行行为差分(退出码/stdout/stderr);API 冒烟测试。

### P7 模块系统联调
- module-info、--module/--module-path/--add-*、模块图与导出/opens 校验、JDK 平台模块。
- 验证:模块编译用例与 javac 对照。

### P8 多 release
- ct.sym 解析(7..16)、`--release`/`-source`/`-target` 版本校验、Source/Target/Preview 完整语义、平台 API 校验。
- 验证:每个 release 的编译对照(成功/失败诊断)。

### P9 全量差分
- langtools javac 测试语料抽取与运行(先编译类用例,再诊断类用例);全量回归;诊断逐字对齐修复;边界补齐。
- 验证:差分通过率报告;零未登记偏离。

## 7. 验证与差分体系

- oracle:本机 javac 17.0.19(CI 用同版本,锁定 tag)。
- 对比维度:退出码;stdout/stderr(归一化后逐字);诊断位置与顺序;生成 class 的 javap 归一化视图;JDK17 运行输出;--release 各版本;preview;模块。
- 分阶段对比:利用 javac 的停止策略选项。注意实测语义:`-XDshould-stop.at` 控制"出错时"
  的停止阶段(shouldStopPolicyIfError,默认 INIT);无错误的停止阶段由
  `-XDshould-stop.ifNoError` 控制(默认 GENERATE)。未完成端到端前按阶段比较诊断时
  两个选项都要用。
- 差分基座已实现:crates/javac-tests(javac 17 oracle 调用、RawOutput、归一化、DiffReport);
  oracle 环境固定 -encoding UTF-8、-J-Dfile.encoding=UTF-8、-J-Duser.language=en/-country=US。
- 语料:OpenJDK langtools 测试抽取(许可 GPLv2+CPE,登记来源)+ 自建用例。
- 归一化规则(固定并测试):绝对路径→占位符;临时目录→占位符;CRLF/平台换行→LF;平台编码→UTF-8;时间戳剔除;Windows 路径分隔符。
- CI(本地脚本先行):build/test、clippy -D warnings、fmt、manifest 校验、资源生成校验、禁模式检查(核心无 Rc<RefCell>、无全局可变状态、无对 JDK 的进程依赖)、差分回归。

## 8. 工具链

- `tools/fetch-upstream`:按 commit sparse clone 到 reference/(不入库)。
- `tools/gen-resources`:从上游 properties 生成 Rust 诊断资源表。
- `tools/gen-manifest` / `tools/check-manifest`:生成并校验 Java↔Rust 文件/方法映射。
- `tools/diff-runner`:差分执行器(可指定参考 javac 版本、阶段停止点、归一化)。
- `tools/upgrade`:上游 tag diff → 受影响 Rust 文件与状态清单。

## 9. 协作与提交

- Lead 负责:架构契约、目录与骨架、核心难点、审查、差分与提交。
- Agent Teams 按包/文件分片并行(写作用域互不重叠),每阶段由 Lead 统一审查并本地 git 提交(远程仓库暂不发布)。
- 提交粒度:每阶段一次或多次小提交,提交信息含阶段与上游包名。
- 禁止:自创抽象/简化实现/重排上游控制流/未登记的偏离。

## 10. 里程碑与汇报

| 里程碑 | 内容 |
|---|---|
| M0 | 计划确认(本文档) |
| M1 | P0+P1:基础设施 + util + parser/tree |
| M2 | P2+P3:类文件/类路径 + 符号类型系统 |
| M3 | P4:语义分析 |
| M4 | P5+P6:首次端到端产出 class + 命令行/API |
| M5 | P7+P8:模块 + 多 release |
| M6 | P9:全量差分绿 |

每阶段汇报:完成清单、差分结果、偏离登记、STATUS.md 更新。

## 11. 合规与开源

- 许可证:GPLv2-only + Classpath Exception(`LICENSE`、CPE 文本、`NOTICE` 取自上游);每个 Rust 文件仅写来源锚点,版权头由项目负责人统一添加。
- README:英文为主,明确非 Oracle 官方、与 OpenJDK javac 的关系、支持范围、裁剪清单、商标声明(Java 是 Oracle 商标)。
- 依赖:仅使用 GPLv2 兼容许可(MIT/BSD/ISC/Zlib 等),`cargo-deny` 强制。
- 发布:暂不发布远程仓库与 crates.io;后期 Release 附对应源码。

## 12. 风险与对策

| 风险 | 对策 |
|---|---|
| 规模大(上游约 12 万行 javac 源码 + 接口层) | 按包分片 + Agent Teams + manifest 追踪 + 阶段汇报 |
| 对象图/借用设计 | arena+ID 先行在 P0/P3 验证;核心禁 Rc<RefCell> |
| UTF-16 语义 | 内部统一 UTF-16 code unit;词法/诊断专项测试 |
| 诊断逐字对齐 | 资源自动生成 + 格式化器逐类移植 + 差分 |
| 类库读取(jimage/ct.sym) | 先 jimage,再 jmods/ct.sym;javap 对照 |
| 性能 | 同量级目标;后期基准驱动优化,不改行为 |
| 升级偏离 | 锚点+manifest+差分三项守护;偏离登记 |

## 13. 未决事项

1. 其余实现期发现、需负责人决策的问题。
