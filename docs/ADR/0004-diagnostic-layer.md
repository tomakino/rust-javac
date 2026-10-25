# ADR 0004:JCDiagnostic / Log 诊断层设计

- 状态:已接受(Lead 审核通过;修订 A-G 见文末"审核修订记录")
- 基线:openjdk/jdk17u @ 982d53c (jdk-17.0.19-ga)
- 相关:ADR 0001(arena + ID)、ADR 0002(诊断资源类型化访问器,含审核修订 A-G)、
  ADR 0003(ClassReader)、docs/PLAN.md

## 背景

javac 的诊断层负责:把资源键(compiler.err/warn/note/misc)与参数构造成诊断对象,
交给处理器(handler)/监听器(DiagnosticListener)/格式化器(formatter),最终写到
NOTICE/WARNING/ERROR/STDOUT/STDERR 五个输出通道或 API 收集器。ADR 0002 已决定
诊断资源访问器(936 个类型化工厂,task-19 已生成);本 ADR 覆盖诊断对象模型、工厂、
格式化与报告层(JCDiagnostic/Log 及其全家)。

上游文件与规模(reference/jdk17u,982d53c 检出):

| Java 文件 | 行数 | 职责 |
|---|---:|---|
| util/JCDiagnostic.java | 834 | 诊断对象/DiagnosticInfo/DiagnosticType/DiagnosticFlag/DiagnosticPosition/SourcePosition/Factory |
| util/Log.java | 858 | 报告器:writer 通道/handler/计数/限制/格式化/本地化 |
| util/AbstractLog.java | 262 | 组件基类:error/warning/note/mandatoryWarning/directError 等 |
| util/JavacMessages.java | 275 | 资源表/本地化/MessageFormat 驱动 |
| util/DiagnosticSource.java | 219 | 诊断源(文件 + 行表 + EndPosTable) |
| api/DiagnosticFormatter.java | 219 | 格式化器接口 |
| util/AbstractDiagnosticFormatter.java | 552 | 格式化器公共部分(位置/源码行/参数显示) |
| util/BasicDiagnosticFormatter.java | 414 | 默认格式(模板替换) |
| util/RichDiagnosticFormatter.java | 727 | -Xdiags:verbose 格式(带类型归属/源码行) |
| util/RawDiagnosticFormatter.java | 194 | -Xdiags:compact/raw 风格 |
| util/ForwardingDiagnosticFormatter.java | 138 | 委托包装 |
| util/MandatoryWarningHandler.java | 284 | 强制警告去重/抑制/最大数量 |
| code/Lint.java | 440 | LintCategory(诊断的 lint 类别) |
| tree/EndPosTable.java | 62 | 诊断结束位置表 |

数据流(上游):
1. 组件调用 JCDiagnostic.Factory(error/warning/note/mandatoryWarning/fragment)或
   AbstractLog 的 error/warning 便捷方法,构造 JCDiagnostic(带 source/position/
   DiagnosticInfo/flags/lintCategory;DiagnosticInfo 由生成访问器或 key 构造)。
2. Log.report(JCDiagnostic) -> 当前 DiagnosticHandler(默认 DefaultDiagnosticHandler;
   可被 DiscardDiagnosticHandler/DeferredDiagnosticHandler 替换)。
3. DefaultDiagnosticHandler:按 DiagnosticType 计数/限制(MaxErrors/MaxWarnings/
   nsuppressed*)、emitWarnings/suppressNotes/mandatory、expectDiagKeys 移除、
   COMPRESSED 标志;ERROR 走 shouldReport(recorded/recordedSourceLevelErrors 去重)。
4. writeDiagnostic:若设置 DiagnosticListener 则交给监听器(javax.tools API 路径),
   否则按类型取 writer,diagFormatter.format(diag, locale) 后 printRawLines 输出,
   promptOnError/dumpOnError/flush。
5. Log.flush()/DeferredDiagnosticHandler.reportDeferredDiagnostics() 在编译阶段边界
   触发(ClassFinder.complete 的 annotate.flush 类似机制)。

当前 rust-javac 现状:
- javac-core 无诊断层(无 JCDiagnostic/Log/DiagnosticSource/JavacMessages/Lint);
- resources::diagnostic_contract(P4 前占位契约,ADR 0002 修订 C 规定的替换点)与
  resources::compiler_properties(task-19 生成的 936 个类型化访问器 + COMPILER_MESSAGES/
  COMPILER_KEYS)已就绪;
- javac-tools 已移植 javax.tools 的 Diagnostic/DiagnosticListener/DiagnosticCollector
  (trait/struct),可直接对接;
- util/Context 已按 task-21 移植(TypeId + Any,工厂惰性,重复 put 断言,dump);
- Options/EndPosTable/Lint/JavaFileObject 等依赖部分未移植(见依赖顺序)。

## 决策

### 1. Rust 模块映射

| Java | Rust |
|---|---|
| util/JCDiagnostic.java | crates/javac-core/src/util/jc_diagnostic.rs |
| util/Log.java | crates/javac-core/src/util/log.rs |
| util/AbstractLog.java | crates/javac-core/src/util/abstract_log.rs |
| util/JavacMessages.java | crates/javac-core/src/util/javac_messages.rs |
| util/DiagnosticSource.java | crates/javac-core/src/util/diagnostic_source.rs |
| api/DiagnosticFormatter.java | crates/javac-core/src/api/diagnostic_formatter.rs |
| util/AbstractDiagnosticFormatter.java | crates/javac-core/src/util/abstract_diagnostic_formatter.rs |
| util/BasicDiagnosticFormatter.java | crates/javac-core/src/util/basic_diagnostic_formatter.rs |
| util/RichDiagnosticFormatter.java | crates/javac-core/src/util/rich_diagnostic_formatter.rs |
| util/RawDiagnosticFormatter.java | crates/javac-core/src/util/raw_diagnostic_formatter.rs |
| util/ForwardingDiagnosticFormatter.java | crates/javac-core/src/util/forwarding_diagnostic_formatter.rs |
| util/MandatoryWarningHandler.java | crates/javac-core/src/util/mandatory_warning_handler.rs |
| code/Lint.java | crates/javac-core/src/code/lint.rs |
| tree/EndPosTable.java | crates/javac-core/src/tree/end_pos_table.rs |
| java.text.MessageFormat(适配) | crates/javac-core/src/util/message_format.rs(PORTING_RULES §9 登记) |

1:1 规则同上(来源锚点/英文 Javadoc/snake_case/控制流照抄);PrintWriter ->
共享可变输出通道(Rc<RefCell<dyn Write>>,登记为 ADR 0001 白名单式适配);
Queue/Set/Map -> VecDeque/BTreeSet 或 HashSet(按上游顺序要求选择)。

### 2. 诊断对象模型(JCDiagnostic)

- DiagnosticType:FRAGMENT(misc)/NOTE(note)/WARNING(warn)/ERROR(err),key 字段用于
  组装资源键(与 ADR 0002 生成器的 kind 一一对应);
- DiagnosticFlag:MANDATORY/RESOLVE_ERROR/SYNTAX/RECOVERABLE/NON_DEFERRABLE/
  COMPRESSED/API/SOURCE_LEVEL(Set -> 位标志或 BTreeSet);
- DiagnosticPosition 接口(getTree/getStartPosition/getPreferredPosition/
  getEndPosition(EndPosTable))+ SimpleDiagnosticPosition(pos);
- DiagnosticInfo(abstract:type/prefix/code/args)、Error/Warning/Note/Fragment 子类;
  key() = prefix + "." + type.key + "." + code(与 ADR 0002 契约一致);
- JCDiagnostic 字段:source(DiagnosticSource)/position(DiagnosticPosition)/
  diagnosticInfo/flags/lintCategory/sourcePosition(惰性行列表位置);方法:
  getType/getSubdiagnostics/isMultiline/getLintCategory/getSource/getDiagnosticSource/
  getStartPosition/getPosition/getEndPosition/getDiagnosticPosition/getLineNumber/
  getColumnNumber/getArgs/getPrefix/getKind/getCode/getMessage/setFlag/isFlagSet/
  toString/MultilineDiagnostic;
- 参数表示(落实 ADR 0002 修订 C):参数必须是原始对象语义(arena ID/真实值类型),
  禁止预格式化文本。Rust 定义 DiagnosticArgument(值类型枚举,后续可扩展):
  Name(Name)、Symbol(SymbolId)、Type(TypeId)、JavaString(JavaString)、
  File(Rc<dyn JavaFileObject>)、FormatInt(i32)/等等;Fragment/诊断子对象用
  Rc<JCDiagnostic>(诊断可作为参数);格式化时由 Formatter 按模板 {n} 渲染;
- Rust 映射:JCDiagnostic 字段与 P4 替换点:resources::diagnostic_contract 的
  DiagnosticInfo/ErrorInfo/WarningInfo/NoteInfo/FragmentInfo 在 P4 由
  util/jc_diagnostic.rs 的真实类型替代(命名保持一致:DiagnosticInfo/DiagnosticType/
  DiagnosticArg 等,避免生成访问器大改);生成器(xtask)只需调整 use 路径与返回类型
  映射表(ADR 0002 修订 C 的替换点),访问器调用点不变;
- DiagnosticInfo.args 从 Vec<DiagnosticArg>(P4 前占位文本)改为
  Vec<Rc<dyn Any/DiagnosticArgument>>(P4 真实对象);ErrorInfo 等 newtype 或直接
  返回 JCDiagnostic 子类对象(实现时以最小改动为准,保持生成器 API 稳定)。

### 3. 诊断工厂(JCDiagnostic.Factory)

- Context 键(diagnosticFactoryKey;Rust 用 TypeId 键,util/context 已就绪);
- 构造:Factory(context) 用 JavacMessages + prefix "compiler",注册进 Context,
  Options 监听器(onlySyntaxErrorsUnrecoverable -> defaultErrorFlags 加 RECOVERABLE);
- 方法:error/warning/note/mandatoryWarning/mandatoryNote/fragment 及 key 版本
  (errorKey/warningKey/noteKey/fragmentKey 返回 Error/Warning/Note/Fragment 诊断键)、
  create(内部组装 JCDiagnostic);默认错误 flags 含 MANDATORY;
- Rust 映射:Factory 由 CompilerContext/Context 持有(工厂函数 + &mut Context);
  Options 的 addListener(选项变化重建 flags)保留(Options 移植后);
  诊断键类型与 ADR 0002 生成器返回类型对接(生成访问器已把 prefix="compiler"/
  code 写入,Factory 直接收 DiagnosticInfo);
- fragment/key 的默认格式化器(BasicDiagnosticFormatter + JavacMessages)按上游静态
  初始化(fragmentFormatter)。

### 4. 格式化层(DiagnosticFormatter)

- api/DiagnosticFormatter 接口:format(diag, locale);getArgumentFormatter 等
  (完整 1:1);
- AbstractDiagnosticFormatter:格式化公共(位置前缀/源码行/参数格式化/本地化消息);
- BasicDiagnosticFormatter(默认,-Xdiags:basic 等价):JavacMessages 取模板 +
  MessageFormat 替换参数(含 {n,choice,...} 等语义;模板来自 COMPILER_MESSAGES 表);
- RawDiagnosticFormatter(-Xdiags:compact/raw 风格,含位置紧凑显示);
- RichDiagnosticFormatter(-Xdiags:verbose):依赖类型/符号信息(带类型归属)、
  源码行、位置;P4 语义分析就绪后完整;P2/P3 期间可移植但部分方法依赖未移植的
  Types/Printer(STATUS 逐项登记未完成,不写简化替代);
- ForwardingDiagnosticFormatter:委托包装;
- JavacMessages:资源表(默认英文 compiler.properties;本地化变体后续按 PLAN)、
  getLocalizedString/MessageFormat 驱动;Rust 侧资源来源 = task-19 生成的
  COMPILER_MESSAGES/COMPILER_KEYS(替代 ListResourceBundle 运行时查找);
  本地化多语言资源(P4 之后)按 PLAN 处理;
- java.text.MessageFormat 适配(util/message_format.rs):实现 Java MessageFormat 的
  {n}/{n,number}/{n,choice}/单引号转义 语义(以 JDK MessageFormat 为 oracle 对照,
  ADR 登记适配);JavacMessages 与各 Formatter 只依赖该适配层。

### 5. 报告层(Log/AbstractLog)

- AbstractLog:组件基类,error/warning/note/mandatoryWarning 等便捷方法(转 Log
  diagFactory),directError/rawError 等;Rust 用 trait/struct 组合(Java 继承 ->
  嵌入 base 或 trait 默认方法,按 PORTING_RULES 映射);
- Log:
  * Context 单例(logKey/outKey/errKey);
  * PrefixKind(JAVAC "javac."/COMPILER_MISC "compiler.misc.");
  * WriterKind(NOTICE/WARNING/ERROR/STDOUT/STDERR)与 writers Map;Rust 用
    [Option<SharedWriter>; 5] 或 HashMap<WriterKind, SharedWriter>;
    SharedWriter = Rc<RefCell<dyn Write>>(适配;登记 ADR 0001 白名单);
  * DiagnosticHandler(install/prev/report)+ DefaultDiagnosticHandler(计数/限制/
    shouldReport/COMPRESSED)+ DiscardDiagnosticHandler + DeferredDiagnosticHandler
    (VecDeque 缓冲、filter、NON_DEFERRABLE、reportDeferredDiagnostics);
  * 计数/限制:nerrors/nwarnings/nsuppressederrors/nsuppressedwarns、MaxErrors/
    MaxWarnings、emitWarnings/suppressNotes、compressedOutput;
  * shouldReport(recorded/recordedSourceLevelErrors 去重;按 source/pos);
  * source 管理:currentSourceFile/setEndPosTable/useSource(Rust 用字段 +
    借用守卫或显式 save/restore;上游 try/finally 语义用 RAII guard);
  * 本地化:localize(key/args)、localize(DiagnosticInfo)、getLocalizedString;
  * 输出:printLines/printRawLines/printVerbose/printNewline/prompt/printErrLine;
  * 报告:report(diag)->handler;writeDiagnostic(listener 优先,否则 formatter +
    writer + flush);getWriterForDiagnosticType;rawError/rawWarning;
  * 诊断监听器:javax.tools DiagnosticListener(已移植 trait)在 writeDiagnostic 中
    优先(javac-tools::tools::diagnostic_listener::DiagnosticListener);
  * MandatoryWarningHandler:强制警告去重/分组/最大数量(P3/P4 用);
  * LintLanguage/LintCategory:code/lint.rs;诊断 flags 与 suppress 逻辑按上游。

### 6. 诊断处理流程(Rust 侧)

create(工厂构造 JCDiagnostic/Rc<JCDiagnostic>) -> report -> diagnosticHandler.report
-> DefaultDiagnosticHandler(计数/去重/限制/COMPRESSED) -> writeDiagnostic
(DiagnosticListener 优先;否则 formatter.format -> writer) -> flush。
DeferredDiagnosticHandler 在解析/注解处理边界使用(与上游 annotate.flush 时序一致)。
计划采用 Rc<JCDiagnostic>(诊断只读共享,可作参数/子诊断);DiagnosticHandler trait
对象(Box<dyn DiagnosticHandler>)或枚举(Default/Discard/Deferred),不引入 RefCell
对象图(Deferred 队列为字段 VecDeque,安装/弹出由 Log 字段管理)。

### 7. 与生成访问器/资源契约的集成(落实 ADR 0002 修订 C)

- P4 替换点:resources/diagnostic_contract.rs 的类型定义移入/替换为
  util/jc_diagnostic.rs 的真实类型(名称保持 DiagnosticInfo/DiagnosticType/
  DiagnosticArg/ErrorInfo 等);xtask 生成器的 SIMPLE_TYPES 映射表与 use 路径同步调整;
- 参数语义:生成访问器的 arg 类型(opaque newtype P4 前)在 P4 时改为真实对象类型
  (TypeId/SymbolId/Name/JavaFileObject 等),错误/警告/注释/片段访问器返回
  JCDiagnostic 的 Error/Warning/Note/Fragment 键对象;参数不得预格式化;
- 验收:诊断格式化与 javac 逐字对齐(见测试策略),javac-tests 差分基座字节级比较
  stdout/stderr;任何格式差异必须修复或登记(格式化器/MessageFormat/模板)。

### 8. 测试策略

1) 单元测试:
   - JCDiagnostic:DiagnosticInfo.key()、DiagnosticType/Flag、SimpleDiagnosticPosition、
     SourcePosition 惰性行列表(无源/有源/位置 NOPOS)、getLineNumber/ColumnNumber;
   - Factory:error/warning/note/mandatoryWarning/fragment 构造与 flags
     (onlySyntaxErrorsUnrecoverable 监听器)、默认 MANDATORY;
   - MessageFormat 适配:对照 JDK java.text.MessageFormat(用 javac-tests 的 Javac
     oracle harness 跑 Java 小程序输出预期,或内嵌已知期望),覆盖 {n}、引号转义、
     choice/number 子格式;
   - Formatter:Basic/Raw 对固定诊断的格式化文本(模板/参数/位置行/源码行);Rich
     在依赖就绪后补测;Forwarding 委托;
   - JavacMessages:模板查找/默认 locale/非存在 key 行为(与上游一致);
   - Log:writers 分流(NOTICE/WARNING/ERROR/STDOUT/STDERR)、nerrors/nwarnings 计数、
     MaxErrors/MaxWarnings 抑制、emitWarnings/suppressNotes、recorded 去重、
     shouldReport、expectDiagKeys、COMPRESSED、rawError/rawWarning、flush、
     useSource/currentSourceFile 恢复、DeferredDiagnosticHandler(缓冲/过滤/
     NON_DEFERRABLE/reportDeferredDiagnostics)、DiscardDiagnosticHandler;
   - AbstractLog:error/warning/note 便捷方法映射;
   - MandatoryWarningHandler:去重/分组/上限;
   - DiagnosticSource/EndPosTable:行表位置(与 util/position 已有实现对接)。
2) 集成/差分(javac-tests 差分基座):
   - 同一源文件用 javac oracle 与 rust-javac 编译,比较 stdout/stderr 字节级
     (\r\n 归一化后),三种格式各测:-XDrawDiagnostics(机器格式)、默认(basic)、
     -Xdiags:verbose(rich,待依赖就绪);
   - 错误计数/退出码/Locale(默认英文)对照;DiagnosticListener 路径(API 编译
     收集诊断)对照;
   - 强制警告/去重场景(多次相同警告、-Xlint 类别、-nowarn);
   - 资源键覆盖率:编译语料(test/langtools/tools/javac 的 compile-only 基线)时
     触发的诊断走生成访问器,断言 arg 个数/模板替换无 panic。
3) 回归:所有诊断相关单测在 cargo test -p javac-core 下保持绿;clippy 零诊断。

### 9. 依赖顺序建议

| 顺序 | 内容 | 说明 |
|---|---|---|
| D1 | util/context(已完成,task-21) | Log/Factory 的组件注册基础 |
| D2 | util/message_format(适配)+ util/javac_messages + api 诊断接口(api/Messages、api/DiagnosticFormatter、api/Formattable)+ util/options + main/Option(完整移植) | Log/Formatter 的模板/本地化/选项基础;Options 是 Log 构造前置(修订 D),必须完整移植不得简化;JavacMessages 需要的资源表由生成数据提供 |
| D3 | tree/end_pos_table + util/position(已有)+ util/diagnostic_source | JCDiagnostic 的 source/position 基础;DiagnosticPosition 与树解耦方案见修订 E |
| D4 | code/lint(最小完整 1:1) | JCDiagnostic 的 LintCategory/flags |
| D5 | util/jc_diagnostic(DiagnosticInfo/Factory/对象模型)+ util/abstract/basic/raw/forwarding + api/DiagnosticFormatter 实现类 | 对象 + 格式化;Basic 是默认路径 |
| D6 | util/abstract_log + util/log(handlers/writers/计数/去重/监听器)+ mandatory_warning_handler | 报告层;接收完整诊断对象 |
| D7 | rich_diagnostic_formatter + 本地化资源 | 依赖 P4 的类型/符号信息;P4 内完成 |
| D8 | -Xdiags/-nowarn/-Xlint/-verbose/diag listener 等选项联动(在 D2 Options 之上) | 与 Log 构造/flags 联动 |

与 ClassReader 的关系(ADR 0003):ClassReader 的 P2 需要 Log(完整移植,不写简化版,
Lead 裁决)与 JCDiagnostic 对象;因此 D1-D6 应在 P2 的主链路(ClassReader 之前或并行)
完成;依赖未移植组件的方法(如 RichDiagnosticFormatter 的类型归属、MandatoryWarningHandler
的部分 lint 逻辑)允许暂缺但 STATUS 逐项登记,后续在同一文件补全,不得另写替代实现。

### 10. 差异/风险登记

- PrintWriter -> Rc<RefCell<dyn Write>> 共享输出通道(适配,登记 PORTING_RULES §9);
- java.text.MessageFormat -> util/message_format 适配(以 JDK 为 oracle 对照);
- java.util.Locale -> 最小表示(JavaString/locale 名;本地化后续);
- JCDiagnostic 的 args Object[] -> DiagnosticArgument 值类型 + Rc<JCDiagnostic>
  (保留对象身份,禁止预格式化——ADR 0002 修订 C);
- RichDiagnosticFormatter 依赖 Types/Symbol/Printer(P3/P4),其完整对齐在 P4;
- 风险管理:先做 Basic/默认格式的逐字对齐(差分基座),再做 Rich;格式化输出按
  行归一化比较(CRLF/路径/编码,复用 javac-tests normalize)。

## 审核修订记录(Lead 审核通过时提出,已接受并落实到实施约束)

- **A. MessageFormat 特性清单与覆盖**:实现前必须扫描全部消息模板(compiler.properties、
  launcher.properties、javac.properties、ct.properties、legacy.properties 及翻译变体)中
  实际使用的 java.text.MessageFormat 特性,列出完整清单并全部覆盖;不得支持范围小于上游
  实际使用;以本机 JDK 17 MessageFormat 为 oracle 生成对照数据(开发期 Java 程序采集,
  测试数据入库,运行期零 JVM 依赖)。
- **B. 输出通道白名单**:PrintWriter -> Rc<RefCell<dyn Write>> 已登记 PORTING_RULES §9
  白名单(由 Lead 登记;本 ADR 引用)。
- **C. api 接口前置**:api/Messages、api/DiagnosticFormatter、api/Formattable 纳入 D2
  (已在上表),不推迟到 D5/D6。
- **D. Options/Option 前置**:Log 构造依赖 Options/Options.addListener;Options/Option
  从 D8 提前到 D2 之前/并列(已在上表 D2),且必须完整移植(不得简化);ClassReader 诊断
  链路的最小完整集合(逐项):api/Messages、api/DiagnosticFormatter、api/Formattable、
  util/options(main/Option)、util/message_format、util/javac_messages、util/context
  (已完成)、util/jc_diagnostic、util/abstract_log、util/log、util/diagnostic_source、
  tree/end_pos_table(P2 以简化位置实现)、code/lint、以及 resources::compiler_properties
  的 errors/warnings/fragments 访问器。
- **E. DiagnosticPosition/EndPosTable 与树解耦**:P2 使用 SimpleDiagnosticPosition(pos:
  i32)构造诊断;DiagnosticPosition.getTree 的 Rust 映射为 Option<TreeId>(树移植后接入
  JCTree arena);getEndPosition(endPosTable) 的 EndPosTable 以 trait 落地,树移植前用
  空实现(返回 preferred position);不阻塞 P2。
- **F. 替换策略批准**:resources::diagnostic_contract -> util/jc_diagnostic 真实类型,
  名称保持;xtask 生成器只调整 SIMPLE_TYPES 映射表与 use 路径;重新生成的
  compiler_properties.rs(8465 行)必须可编译并保留可审计的 GENERATED 文件头(含来源、
  基线 commit、生成命令)。
- **G. 阶段完成定义与验收**:D1-D8 每阶段完成定义与验收标准对齐 ADR 0003 的 P2 里程碑
  (P2a/P2b);-Xdiags 各模式(raw/basic/rich)与 Rich/verbose 覆盖在 D5-D7 明确:
  D5 完成 Basic/Raw 格式与 -Xdiags:basic/compact 对照,D7 完成 Rich 格式与
  -Xdiags:verbose 逐字对照;差分基座三种格式(默认/-XDrawDiagnostics/-Xdiags:verbose)
  各至少一组字节级对照测试。

## 结果

- 诊断层分三层移植:对象/工厂(JCDiagnostic)、格式化(DiagnosticFormatter 家族 +
  JavacMessages/MessageFormat)、报告(AbstractLog/Log/handlers/监听器);
- 与 task-19 生成的类型化访问器通过 ADR 0002 修订 C 的替换点对接,参数保留对象语义,
  P4 格式化与 javac 逐字对齐;
- 依赖顺序明确(D1-D8),P2 主链路所需的 Log/JCDiagnostic 按完整移植推进,未就绪依赖
  逐项 STATUS 登记、同文件补全;
- 测试策略覆盖单元(对象/格式化/消息/Log 行为)与差分(三种诊断格式字节级对照)。
