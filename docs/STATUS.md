# 项目状态(STATUS)

> 由 Lead 维护;最后更新于本文件当前内容。逐项验收记录见各任务汇报;本文件只给全局状态。

## 阶段进度

- **P0 / P0.5**:util 基础设施、javax.tools / javax.lang.model / com.sun.source 接口层、诊断资源生成、
  code 骨架(Symbol/Type/Scope/Attribute/Directive/Kinds/Flags)、tree 骨架(JCTree/Tag/TreeKind/
  Visitor/Scanner/Translator/Copier/TreeInfo/Pretty/EndPosTable)、CompilerContext、诊断层
  (JCDiagnostic/Formatter 家族/Log/MandatoryWarningHandler)、Options/Option、Source/Target/Profile/
  Lint/Preview、platform(S1-S2)、file 层 S3a+S3b、charset、ct.sym、jimage packages、classpath、
  javac-fs、差分测试基础设施 —— 完成。
- **P1(parser/AST)**:词法层 S6(3550 行)、TreeMaker(1703 行)、Types 最小前置(1090 行)、
  Doc 树族 S8(2534 行)—— 完成;**JavacParser S7a 进行中**(tools-api),S7b 待做;
  DocCommentParser/LazyDocCommentTable/ReferenceParser 进行中(ctsym-porter)。
- **P2(file/ClassReader)**:file 层 S3a(BaseFileManager/JRTIndex)+ S3b(Locations 1586 行)完成;
  S3c(JavacFileManager)进行中;ClassReader/PoolReader/ModuleNameReader(PoolReader+ModuleNameReader
  已完成;ClassReader 待 code 服务)。
- **P3(symbols/types)**:Symtab(1745 行)+ Types 最小完成;Types 批次 2(isSubtype/containsType/
  asSuper/memberType/subst)进行中;Types 全量与 Attr/Check/Flow/TransTypes/Lower/Gen 待 P3-P5。
- **P4-P9**:待(诊断对象契约 P4 替换、模块系统、多版本、差分全量)。

## 已验证产物(测试数)

| crate | tests | 说明 |
|---|---|---|
| javac-core | 518+ | util/code/tree/parser 词法/诊断/resource;全量绿(clippy 零) |
| javac-tools | 108 | javax.tools + javax.lang.model 完整接口 |
| javac-source | 48 | com.sun.source tree/doctree/util |
| javac-strings / javac-unicode | 8 / 8 | JavaString(UTF-16)/ Unicode 表与查询 |
| javac-jimage / javac-jmod / javac-zip | 11 / 4 / 10 | 容器读取 + /packages |
| javac-ctsym | 11 | ct.sym 容器/版本选择(与 javac --release 对照) |
| javac-classpath | 4 | 系统类库字节源(jimage→jmods) |
| javac-charset | 5 | java.nio.charset 适配(6 编码 + oracle) |
| javac-fs | 15 | java.nio.file 路径/文件查询平台层(Windows oracle) |
| javac-tests | 61 | oracle harness/corpus/差分运行器/诊断 oracle(80 用例) |
| xtask | 14 | gen-resources(936 访问器 + 本地化)/gen-manifest/gen-unicode |

## 团队工作约定

- 上下文限制由系统处理(压缩/续接);teammate 不得以此为由提前收尾、交接或停在半成品。
- 只在真实阻塞(依赖未移植且规则禁止造假/需裁决的设计冲突)时停干净状态报告。
- 恢复后先读 STATUS 与实际文件状态,从断点继续,不重做、不重复汇报。
- 干部分工:util-porter 独占全部 Rust 代码修改(含 mod.rs/lib.rs);Lead 只做指派、审核、docs、git 提交与决策。
- 硬规则:文件 1:1(适配层登记)、来源锚点、英文 Javadoc、不写版权头;不 git;
  不改任务范围外文件;每批 cargo test + clippy 零诊断;未移植依赖暂缺 + TODO 并报告,不写替代。

## 执行模式

- 自本文件当前版本起:只有 util-porter 一名 teammate 执行编码;Lead 负责指派、审核、共享文件
  (所有 mod.rs/lib.rs/Cargo.toml/docs)维护与 git 提交。其余 teammate 停止。
- 任务板:task-66(单人执行:修复 5 个失败测试 + JavacParser S7a);历史任务由 Lead 接管/关闭。

## 并行任务(Agent Teams)(历史)

| task | owner | 任务 | 状态 |
|---|---|---|---|
| task-62 | source-finisher | file S3c:JavacFileManager(1190 行) | 进行中 |
| task-63 | tools-api | parser S7:JavacParser S7a(基础语法 + EndPosTable) | 进行中 |
| task-64 | util-porter | code/Types 批次 2(isSubtype/asSuper/memberType/subst) | 进行中 |
| task-65 | ctsym-porter | parser S8:DocCommentParser/LazyDocCommentTable/ReferenceParser | 进行中 |

(已完成任务见团队历史;关键里程碑:util 批 B、JCDiagnostic、Formatter、Log、Options、Source/Lint/Target/Profile、
platform、ct.sym、charset、file 基础、Locations、Symtab、Types 最小、词法层、TreeMaker、Doc 树族、差分基础设施。)

## 关键 TODO(下一阶段)

1) JavacParser S7a/S7b:解析 67 节点树 + 错误恢复 + EndPosTable;里程碑 M3 = 首个简单文件 parse 后 Pretty 与
   crates/javac-tests/data/tree-oracle.txt 对照通过。
2) DocCommentParser 接线后补 doc 注释端到端对照(Pretty/DocPretty oracle)。
3) Types 全量(P3):isSubtype/subst/capture/lub/glb/UndetVar/metadata;Symtab 剩余 Completer/
   MissingInfoHandler/Target-Source instance 集成。
4) P2:JavacFileManager S3c、ClassReader(依赖 code 服务)、ct.sym/jrt 数据源接线。
5) P4+:Attr/Check/Flow/TransTypes/Lower/Gen/字节码写出、模块系统、多版本、诊断对象契约替换(P4)、
   差分全量(P9)。
6) 资源批:ct.properties(JRTIndex.getCtInfo: hidden/proprietary/minProfile)静态表生成。

## 已批准偏离(摘要;详见 docs/PORTING_RULES.md)

- Arena+ID(SymbolId/TypeId/TreeId/ScopeId/AttributeId/DirectiveId);CompilerContext 持六 arena。
- Java 继承 -> enum/match、组合、trait;重载按 §3 改名;集合/可空按 §14。
- 适配层:MessageFormat、Locale/ResourceBundle 静态表、java.nio.charset(javac-charset)、
  java.nio.file(javac-fs)、ct.sym(javac-ctsym)、系统类库(javac-classpath)、jimage/jmod/zip。
- 内部可变性白名单:JavacMessages bundleCache、CacheFSInfo/LazyDocCommentTable、Log 输出句柄、
  Options(listener/RefCell)、BaseFileManager contentCache 等(逐项 §9)。
- 运行期零 JVM:所有 oracle 数据开发期采集入库。

## 全量状态

- 全 workspace:cargo test 全绿(最近:javac-core 518+、其余见上表);clippy 各 crate 零诊断。
- 共享工作树规则:并行任务按文件范围隔离;每批提交前 cargo check 保持 crate 可编译。

## 当前阻塞(进行中)

- 无长期阻塞;并行批次中间态可能短时不可编译(owner 负责恢复,他人用子集验证)。

## 文档索引

- docs/PLAN.md(P0-P9 计划)、docs/PORTING_RULES.md(移植规则 §1-§14.9 + 适配登记 §9)、
  docs/SUPPORT_MATRIX.md(范围/裁剪)、docs/DEPENDENCIES.md(依赖许可)、
  docs/PORTING_MANIFEST.md(xtask 生成,Java→Rust 映射)。
- docs/ADR/0001-arena-ids.md、0002-diagnostic-resources.md、0003-class-reader.md、
  0004-diagnostic-layer.md、0005-jctree-design.md、0006-parser-design.md。