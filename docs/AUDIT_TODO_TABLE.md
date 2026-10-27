# TODO / NOT PORTED 审计表(只读分析,待负责人批准)

生成时间: 2026-10-27;范围: crates/ tools/ xtask/。

## 口径与统计

- 实测命中 1030 行(含模块头 Gap registration 列表、方法注释、TODO 标记);按规范化文本+文件去重后 **890 条目**;
- 与 Lead 旧统计(TODO 286 / not-ported 561)口径不同(旧统计可能按独立条目/排除模块头列表);本表以本次实测为准并保留全部条目,主动更正;
- 分类统计:{"真实未实现":314,"过时(疑似本地已实现,更新注释)":251,"纯文档说明":84,"真实未实现(待核)":189,"真实未实现(依赖批次)":38,"过时(本地已实现,更新注释)":2,"测试或工具":12}
- 影响度统计:{"中":340,"高":460,"高·核心缺口":50,"低":40}

分类定义:
- 真实未实现:上游存在、本地无对应实现(含 NOT PORTED 登记与 TODO 功能缺口);
- 过时(疑似本地已实现):注释声称 NOT PORTED 的方法名在本地已有定义(snake_case 同名),注释需更新;
- 真实未实现(依赖批次):TODO(S3*/S8/parser wiring 等)标注的阶段项;
- 纯文档说明:adapter/registered/unused/debug 等移植说明,可保留;
- 测试或工具:测试/工具代码中的 TODO;

## 重点缺口(按负责人指定优先项核对上游行号)

| # | 本地落点 | 缺口 | 上游 javac | 依赖/方案 | 影响 |
|---:|---|---|---|---|---|
| 1 | code/class_finder.rs:30,1432 | ClassFinder module completer 未移植(Modules 未移植;msym.complete() TODO) | ClassFinder.java:170 getCompleter()/282-310 complete();Modules.java:281 getCompleter() | 前置移植 comp/Modules.java(ModuleSymbol/模块图) + Completer 链;当前平台类位置为直装 | 高·核心 |
| 2 | code/symbol.rs 模块头 | Symbol.complete()/Completer 未移植(completer 字段存在) | Symbol.java:128 completer;678 complete();779/1407 重写;2524-2528 Completer 接口 | 实现 Completer trait + complete()/apiComplete/isCompleted 等;symtab 引用 | 高·核心 |
| 3 | code/symtab.rs:514,560,652,875,900,986,993,1241,1256,1262,1919 | Symtab initialCompleter/moduleCompleter/packageCompleter/predefClassCompleter 未移植 | Symtab.java:111-112,311-325,491,519;ClassFinder.java:170;Modules.java:281 | 依赖 ClassFinder/Modules/Completer 实现后接线 | 高·核心 |
| 4 | file/locations.rs S3c 条目(见完整表) | Locations S3c:by-path lookup/ModuleTable.update 未移植 | Locations.java:173 update();482/489/623 getLocationForModule;583/618 moduleTable | 移植 Locations.update/ModuleTable(依赖 FileManager/module scan) | 高·核心 |
| 5 | code/class_finder.rs:264,270,277 | ClassFinder complete() 的 annotate.blockAnnotations/unblockAnnotationsNoFlush/flush 调用未接线 | ClassFinder.java:282-310 | 接线 Annotate 服务(本地已移植 Annotate) | 高·核心 |
| 6 | code/symbol.rs 模块头 133 条 | Symbol 的属性/Annotation API 批量未移植(getRawAttributes/set*/isCompleted 等) | Symbol.java:146-273,443-872 等(条目内逐行号) | 按上游逐个实现;优先被 ClassReader/annotation 处理调用的方法 | 高 |
| 7 | code/types.rs 126 条 | Type/Types 辅助方法批量未移植 | Type.java/Types.java(条目内逐行号) | 按上游实现;优先被 attr/resolve/infer 调用项 | 高 |
| 8 | file/javac_file_manager.rs 28 条 + base_file_manager.rs 17 条 | FileManager API 缺口(module paths/list/contains 等) | JavacFileManager.java/BaseFileManager.java(条目内行号) | 按上游实现;依赖 Locations S3c | 中高 |
| 9 | jvm/class_reader.rs 35 条 | ClassReader 池访问器/annotation proxy 缺口 | ClassReader.java(条目内行号) | 按上游实现(ClassReader 批次) | 中 |
| 10 | code/scope.rs 21 条 | Scope.isStaticallyImported 及各子类重写未移植 | Scope.java:162/672/871/984/988/1110 | 实现方法(小);与 import scope 使用点接线 | 中 |
| 11 | tree/tree_info.rs 21 条 | TreeInfo 方法缺口 | TreeInfo.java(条目内逐行号) | 按上游实现 | 中 |
| 12 | comp/attr.rs 23 条 / comp/enter.rs 16 条 | attr/enter TODO(annotations、TypeEnter、模块等) | Attr.java/Enter.java(条目内逐行号) | 逐条按上游接线;优先 annotations/TypeEnter | 高 |
| 13 | code/flags.rs:16,371 | 过时:isStatic/isEnum/isConstant 已在 flags.rs 实现(本会话) | Flags.java:457-467 | 更新/删除注释(过时) | 低 |
| 14 | parser/javac_parser.rs TODO + doc_comment_parser RetainAll | parser/doc-comment wiring 阶段项 | JavacParser.java/DocCommentParser.java | 依赖 parser/doc-comment S8 wiring | 中 |

## 高影响核心缺口 Top 40(完整表筛选,按影响度)

| # | 文件:行 | 类别 | 摘要 | 上游锚点 | 处置建议 |
|---:|---|---|---|---|---|
| 1 | crates/javac-core/src/code/types.rs:3900 | 真实未实现 | Java: Symbol.ClassSymbol.getSuperclass() (Symbol.complete() is not ported: a | — | 按上游实现(核心编译链,优先) |
| 2 | crates/javac-core/src/code/types.rs:3919 | 真实未实现 | Java: Symbol.ClassSymbol.getInterfaces() (Symbol.complete() is not ported; a | — | 按上游实现(核心编译链,优先) |
| 3 | crates/javac-core/src/code/class_finder.rs:30 | 真实未实现(待核) | - NOT PORTED: module completer (Modules not ported; msym.complete() TODO). | — | 按上游实现(核心编译链,优先) |
| 4 | crates/javac-core/src/code/symtab.rs:514 | 真实未实现 | NOT PORTED (Completer is not ported). | — | 按上游实现(核心编译链,优先) |
| 5 | crates/javac-core/src/code/symtab.rs:560 | 真实未实现 | Java: c.completer = initialCompleter; - NOT PORTED (Completer). | — | 按上游实现(核心编译链,优先) |
| 6 | crates/javac-core/src/code/symtab.rs:652 | 真实未实现 | WriteableScope.create(this); the rest of complete() is not ported | — | 按上游实现(核心编译链,优先) |
| 7 | crates/javac-core/src/code/symtab.rs:875 | 真实未实现 | Java: msym.completer = s -> moduleCompleter.complete(s); - NOT PORTED. | — | 按上游实现(核心编译链,优先) |
| 8 | crates/javac-core/src/code/symtab.rs:986 | 真实未实现 | NOT PORTED: needs the anonymous Completer of the upstream synthesize | — | 按上游实现(核心编译链,优先) |
| 9 | crates/javac-core/src/code/symtab.rs:993 | 真实未实现 | Java: private final Completer moduleCompleter (from Modules; not ported -> None) | — | 按上游实现(核心编译链,优先) |
| 10 | crates/javac-core/src/code/symtab.rs:1241 | 真实未实现 | NOT PORTED (ClassFinder/Completer). | — | 按上游实现(核心编译链,优先) |
| 11 | crates/javac-core/src/code/symtab.rs:1262 | 真实未实现 | NOT PORTED (Modules/Completer). | — | 按上游实现(核心编译链,优先) |
| 12 | crates/javac-core/src/code/symtab.rs:1919 | 真实未实现 | Modules.instance(context).getCompleter() (Modules is not ported: the | — | 按上游实现(核心编译链,优先) |
| 13 | crates/javac-core/src/comp/enter.rs:17 | 真实未实现(待核) | usesProvidesCompleter and isInModuleGraph are TODO (Modules); | — | 按上游实现(核心编译链,优先) |
| 14 | crates/javac-core/src/comp/enter.rs:362 | 真实未实现(待核) | TODO(Modules): tree.modle.usesProvidesCompleter = | — | 按上游实现(核心编译链,优先) |
| 15 | crates/javac-core/src/code/types.rs:3770 | 纯文档说明 | ; registered). Symbol.complete() is not ported, so the lazy completion | — | 保留(移植说明) |
| 16 | crates/javac-core/src/code/class_finder.rs:22 | 纯文档说明 | calls in complete() — Annotate (registered TODO in the method); | — | 保留(移植说明) |
| 17 | crates/javac-core/src/code/symbol.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 按上游实现(核心编译链,优先) |
| 18 | crates/javac-core/src/code/symbol.rs:19 | 真实未实现 | - NOT PORTED: isCompleted (Symbol.java:232) | Symbol.java:232 | 按上游实现(核心编译链,优先) |
| 19 | crates/javac-core/src/code/symbol.rs:29 | 真实未实现 | - NOT PORTED: isInner (Symbol.java:490) | Symbol.java:490 | 按上游实现(核心编译链,优先) |
| 20 | crates/javac-core/src/code/symbol.rs:37 | 真实未实现 | - NOT PORTED: apiComplete (Symbol.java:686) | Symbol.java:686 | 按上游实现(核心编译链,优先) |
| 21 | crates/javac-core/src/code/symbol.rs:51 | 真实未实现 | - NOT PORTED: getUnderlyingSymbol (Symbol.java:790) | Symbol.java:790 | 按上游实现(核心编译链,优先) |
| 22 | crates/javac-core/src/code/symbol.rs:54 | 真实未实现 | - NOT PORTED: precedes (Symbol.java:835) | Symbol.java:835 | 按上游实现(核心编译链,优先) |
| 23 | crates/javac-core/src/code/symbol.rs:61 | 真实未实现 | - NOT PORTED: getAttribute (Symbol.java:933) | Symbol.java:933 | 按上游实现(核心编译链,优先) |
| 24 | crates/javac-core/src/code/symbol.rs:62 | 真实未实现 | - NOT PORTED: isCurrentSymbolsAnnotation (Symbol.java:948) | Symbol.java:948 | 按上游实现(核心编译链,优先) |
| 25 | crates/javac-core/src/code/symbol.rs:67 | 真实未实现 | - NOT PORTED: isNoModule (Symbol.java:1034) | Symbol.java:1034 | 按上游实现(核心编译链,优先) |
| 26 | crates/javac-core/src/code/symbol.rs:68 | 真实未实现 | - NOT PORTED: completeUsesProvides (Symbol.java:1050) | Symbol.java:1050 | 按上游实现(核心编译链,优先) |
| 27 | crates/javac-core/src/code/symbol.rs:73 | 真实未实现 | - NOT PORTED: mergeAttributes (Symbol.java:1200) | Symbol.java:1200 | 按上游实现(核心编译链,优先) |
| 28 | crates/javac-core/src/code/symbol.rs:82 | 真实未实现 | - NOT PORTED: getSuperClassToSearchForAnnotations (Symbol.java:1453) | Symbol.java:1453 | 按上游实现(核心编译链,优先) |
| 29 | crates/javac-core/src/code/symbol.rs:83 | 真实未实现 | - NOT PORTED: getInheritedAnnotations (Symbol.java:1465) | Symbol.java:1465 | 按上游实现(核心编译链,优先) |
| 30 | crates/javac-core/src/code/symbol.rs:93 | 真实未实现 | - NOT PORTED: clearAnnotationMetadata (Symbol.java:1608) | Symbol.java:1608 | 按上游实现(核心编译链,优先) |
| 31 | crates/javac-core/src/code/symbol.rs:103 | 真实未实现 | - NOT PORTED: setLazyConstValue (Symbol.java:1735) | Symbol.java:1735 | 按上游实现(核心编译链,优先) |
| 32 | crates/javac-core/src/code/symbol.rs:104 | 真实未实现 | - NOT PORTED: isExceptionParameter (Symbol.java:1751) | Symbol.java:1751 | 按上游实现(核心编译链,优先) |
| 33 | crates/javac-core/src/code/symbol.rs:105 | 真实未实现 | - NOT PORTED: isResourceVariable (Symbol.java:1755) | Symbol.java:1755 | 按上游实现(核心编译链,优先) |
| 34 | crates/javac-core/src/code/symbol.rs:108 | 真实未实现 | - NOT PORTED: getOriginalAnnos (Symbol.java:1826) | Symbol.java:1826 | 按上游实现(核心编译链,优先) |
| 35 | crates/javac-core/src/code/symbol.rs:111 | 真实未实现 | - NOT PORTED: isAliasFor (Symbol.java:1880) | Symbol.java:1880 | 按上游实现(核心编译链,优先) |
| 36 | crates/javac-core/src/code/symbol.rs:113 | 真实未实现 | - NOT PORTED: preserveBinding (Symbol.java:1888) | Symbol.java:1888 | 按上游实现(核心编译链,优先) |
| 37 | crates/javac-core/src/code/symbol.rs:114 | 真实未实现 | - NOT PORTED: isPreserved (Symbol.java:1892) | Symbol.java:1892 | 按上游实现(核心编译链,优先) |
| 38 | crates/javac-core/src/code/symbol.rs:117 | 真实未实现 | - NOT PORTED: isHandle (Symbol.java:1974) | Symbol.java:1974 | 按上游实现(核心编译链,优先) |
| 39 | crates/javac-core/src/code/symbol.rs:118 | 真实未实现 | - NOT PORTED: asHandle (Symbol.java:1979) | Symbol.java:1979 | 按上游实现(核心编译链,优先) |
| 40 | crates/javac-core/src/code/symbol.rs:119 | 真实未实现 | - NOT PORTED: implemented (Symbol.java:1987) | Symbol.java:1987 | 按上游实现(核心编译链,优先) |

## 完整条目表(890 条;按文件+行排序)

| # | 文件:行 | 类别 | 摘要 | 上游锚点 | 影响 | 处置建议 |
|---:|---|---|---|---|---|---|
| 1 | crates/javac-core/src/code/attribute.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 2 | crates/javac-core/src/code/attribute.rs:5 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getValue() (Attribute.java:57 and the overrides) — Java Object | Attribute.java:57 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 3 | crates/javac-core/src/code/attribute.rs:8 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: accept(AnnotationValueVisitor, P) (Attribute.java:62 and the | Attribute.java:62 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 4 | crates/javac-core/src/code/attribute.rs:10 | 真实未实现 | - NOT PORTED: Compound.toString's type part / Enum.toString (Attribute.java:258/ | Attribute.java:258 | 中 | 按上游实现 |
| 5 | crates/javac-core/src/code/attribute.rs:11 | 真实未实现 | 366) call Type.toString / Symbol.toString, which are not ported yet; the | — | 中 | 按上游实现 |
| 6 | crates/javac-core/src/code/attribute.rs:253 | 纯文档说明 | The Type/Symbol display parts are not ported yet (registered helpers). | — | 中 | 保留(移植说明) |
| 7 | crates/javac-core/src/code/class_finder.rs:11 | 纯文档说明 | getSupplementaryFlags (jrtIndex==null path; Some path registered TODO), | — | 高 | 保留(移植说明) |
| 8 | crates/javac-core/src/code/class_finder.rs:19 | 真实未实现 | Still not ported (no substitutes): | — | 高 | 按上游实现(核心编译链,优先) |
| 9 | crates/javac-core/src/code/class_finder.rs:20 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: instance(Context) — the port injects dependencies in new(); | — | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 10 | crates/javac-core/src/code/class_finder.rs:21 | 真实未实现 | - NOT PORTED: annotate.blockAnnotations/unblockAnnotationsNoFlush/flush | — | 高 | 按上游实现(核心编译链,优先) |
| 11 | crates/javac-core/src/code/class_finder.rs:22 | 纯文档说明 | calls in complete() — Annotate (registered TODO in the method); | — | 高·核心缺口 | 保留(移植说明) |
| 12 | crates/javac-core/src/code/class_finder.rs:23 | 真实未实现 | - NOT PORTED: getSupplementaryFlags jrtIndex!=null path (JRTIndex.isInJRT | — | 高 | 按上游实现(核心编译链,优先) |
| 13 | crates/javac-core/src/code/class_finder.rs:25 | 真实未实现 | - NOT PORTED: verbose path printing (StandardJavaFileManager.getLocationAsPaths | — | 高 | 按上游实现(核心编译链,优先) |
| 14 | crates/javac-core/src/code/class_finder.rs:27 | 真实未实现 | - NOT PORTED: newCompletionFailure cached CompletionFailure reuse (the port | — | 高 | 按上游实现(核心编译链,优先) |
| 15 | crates/javac-core/src/code/class_finder.rs:30 | 真实未实现(待核) | - NOT PORTED: module completer (Modules not ported; msym.complete() TODO). | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 16 | crates/javac-core/src/code/class_finder.rs:264 | 真实未实现(待核) | TODO(Annotate): annotate.blockAnnotations(); | — | 高 | 按上游实现(核心编译链,优先) |
| 17 | crates/javac-core/src/code/class_finder.rs:270 | 真实未实现(待核) | TODO(Annotate): annotate.unblockAnnotationsNoFlush(); | — | 高 | 按上游实现(核心编译链,优先) |
| 18 | crates/javac-core/src/code/class_finder.rs:277 | 真实未实现(待核) | TODO(Annotate): if (!reader.filling) annotate.flush(); | — | 高 | 按上游实现(核心编译链,优先) |
| 19 | crates/javac-core/src/code/class_finder.rs:660 | 纯文档说明 | is not ported yet (registered TODO). | — | 高 | 保留(移植说明) |
| 20 | crates/javac-core/src/code/class_finder.rs:749 | 真实未实现(依赖批次) | TODO(S3/verbose): the upstream prints the source/class paths through | — | 高 | 依赖批次接线(按上游) |
| 21 | crates/javac-core/src/code/class_finder.rs:1432 | 真实未实现 | Modules is not ported yet, so the platform class location is installed | — | 高 | 按上游实现(核心编译链,优先) |
| 22 | crates/javac-core/src/code/dependencies.rs:12 | 纯文档说明 | NOT PORTED IN THIS STEP (registered, no substitutes): | — | 中 | 保留(移植说明) |
| 23 | crates/javac-core/src/code/dependencies.rs:52 | 真实未实现 | NOT PORTED: GraphDependencies (Dependencies.java:100-430) — needs GraphUtils, | Dependencies.java:100-430 | 中 | 按上游实现 |
| 24 | crates/javac-core/src/code/directive.rs:18 | 真实未实现 | NOT PORTED IN THIS STEP (see docs/STATUS.md; no placeholder logic): | — | 中 | 按上游实现 |
| 25 | crates/javac-core/src/code/flags.rs:16 | 过时(本地已实现,更新注释) | TODO(P3): isStatic/isEnum/isConstant need Symbol/Symbol.VarSymbol and are | — | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 26 | crates/javac-core/src/code/flags.rs:371 | 过时(本地已实现,更新注释) | TODO(P3): isStatic(Symbol), isEnum(Symbol), isConstant(Symbol.VarSymbol) are | — | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 27 | crates/javac-core/src/code/lint.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 高 | 按上游实现(核心编译链,优先) |
| 28 | crates/javac-core/src/code/lint.rs:5 | 真实未实现 | - NOT PORTED: initSyms (Lint.java:391) | Lint.java:391 | 高 | 按上游实现(核心编译链,优先) |
| 29 | crates/javac-core/src/code/lint.rs:6 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitConstant (Lint.java:403) | Lint.java:403 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 30 | crates/javac-core/src/code/lint.rs:7 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitClass (Lint.java:411) | Lint.java:411 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 31 | crates/javac-core/src/code/lint.rs:8 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitCompound (Lint.java:417) | Lint.java:417 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 32 | crates/javac-core/src/code/lint.rs:9 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitArray (Lint.java:429) | Lint.java:429 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 33 | crates/javac-core/src/code/lint.rs:10 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitEnum (Lint.java:434) | Lint.java:434 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 34 | crates/javac-core/src/code/lint.rs:11 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitError (Lint.java:437) | Lint.java:437 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 35 | crates/javac-core/src/code/lint.rs:61 | 真实未实现(待核) | TODO(Attribute/Symtab): the AugmentVisitor needs Attribute.Compound and | — | 高 | 按上游实现(核心编译链,优先) |
| 36 | crates/javac-core/src/code/lint.rs:62 | 真实未实现 | Symtab.instance, which are not ported yet; no field is represented until then. | — | 高 | 按上游实现(核心编译链,优先) |
| 37 | crates/javac-core/src/code/lint.rs:76 | 真实未实现(待核) | TODO(Options): the constructor reads Options (-Xlint/-Xlint:all/none/ | — | 高 | 按上游实现(核心编译链,优先) |
| 38 | crates/javac-core/src/code/lint.rs:77 | 真实未实现 | custom/-preview) and Source.instance; both depend on the not yet ported | — | 高 | 按上游实现(核心编译链,优先) |
| 39 | crates/javac-core/src/code/preview.rs:25 | 真实未实现(依赖批次) | * TODO(Log/Lint/Options): the constructor reads Options (enabled, | — | 中 | 依赖批次接线(按上游) |
| 40 | crates/javac-core/src/code/preview.rs:132 | 真实未实现 | instead of reading Options (not ported yet). | — | 中 | 按上游实现 |
| 41 | crates/javac-core/src/code/preview.rs:136 | 真实未实现 | MandatoryWarningHandler; those dependencies are not ported yet. | — | 中 | 按上游实现 |
| 42 | crates/javac-core/src/code/preview.rs:137 | 真实未实现(依赖批次) | TODO(Log/Lint/Options): the P6 wiring builds Preview from the ported | — | 中 | 依赖批次接线(按上游) |
| 43 | crates/javac-core/src/code/preview.rs:232 | 真实未实现(依赖批次) | TODO(Log/Lint/Options): the constructor reads Options/Log/Lint; the | — | 中 | 依赖批次接线(按上游) |
| 44 | crates/javac-core/src/code/scope.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 高 | 按上游实现(核心编译链,优先) |
| 45 | crates/javac-core/src/code/scope.rs:5 | 真实未实现 | - NOT PORTED: isStaticallyImported (Scope.java:162) | Scope.java:162 | 高 | 按上游实现(核心编译链,优先) |
| 46 | crates/javac-core/src/code/scope.rs:6 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: create (Scope.java:268) | Scope.java:268 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 47 | crates/javac-core/src/code/scope.rs:7 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: hasNext (Scope.java:581) | Scope.java:581 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 48 | crates/javac-core/src/code/scope.rs:8 | 真实未实现 | - NOT PORTED: doNext (Scope.java:598) | Scope.java:598 | 高 | 按上游实现(核心编译链,优先) |
| 49 | crates/javac-core/src/code/scope.rs:9 | 真实未实现 | - NOT PORTED: skipToNextMatchingEntry (Scope.java:619) | Scope.java:619 | 高 | 按上游实现(核心编译链,优先) |
| 50 | crates/javac-core/src/code/scope.rs:13 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: toString (Scope.java:676) | Scope.java:676 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 51 | crates/javac-core/src/code/scope.rs:14 | 真实未实现 | - NOT PORTED: finalizeScope (Scope.java:749) | Scope.java:749 | 高 | 按上游实现(核心编译链,优先) |
| 52 | crates/javac-core/src/code/scope.rs:15 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: finalizeSingleScope (Scope.java:755) | Scope.java:755 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 53 | crates/javac-core/src/code/scope.rs:16 | 真实未实现 | - NOT PORTED: importByName (Scope.java:799) | Scope.java:799 | 高 | 按上游实现(核心编译链,优先) |
| 54 | crates/javac-core/src/code/scope.rs:17 | 真实未实现 | - NOT PORTED: importType (Scope.java:803) | Scope.java:803 | 高 | 按上游实现(核心编译链,优先) |
| 55 | crates/javac-core/src/code/scope.rs:18 | 真实未实现 | - NOT PORTED: appendScope (Scope.java:807) | Scope.java:807 | 高 | 按上游实现(核心编译链,优先) |
| 56 | crates/javac-core/src/code/scope.rs:21 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: importAll (Scope.java:884) | Scope.java:884 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 57 | crates/javac-core/src/code/scope.rs:22 | 真实未实现 | - NOT PORTED: isFilled (Scope.java:898) | Scope.java:898 | 高 | 按上游实现(核心编译链,优先) |
| 58 | crates/javac-core/src/code/scope.rs:23 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: accepts (Scope.java:905) | Scope.java:905 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 59 | crates/javac-core/src/code/scope.rs:24 | 真实未实现 | - NOT PORTED: doLookup (Scope.java:939) | Scope.java:939 | 高 | 按上游实现(核心编译链,优先) |
| 60 | crates/javac-core/src/code/scope.rs:28 | 真实未实现 | - NOT PORTED: importFrom (Scope.java:999) | Scope.java:999 | 高 | 按上游实现(核心编译链,优先) |
| 61 | crates/javac-core/src/code/scope.rs:30 | 真实未实现 | - NOT PORTED: prependSubScope (Scope.java:1033) | Scope.java:1033 | 高 | 按上游实现(核心编译链,优先) |
| 62 | crates/javac-core/src/code/scope.rs:31 | 真实未实现 | - NOT PORTED: appendSubScope (Scope.java:1042) | Scope.java:1042 | 高 | 按上游实现(核心编译链,优先) |
| 63 | crates/javac-core/src/code/scope.rs:32 | 真实未实现 | - NOT PORTED: getMark (Scope.java:1061) | Scope.java:1061 | 高 | 按上游实现(核心编译链,优先) |
| 64 | crates/javac-core/src/code/scope.rs:70 | 真实未实现 | NOT PORTED IN THIS STEP (see docs/STATUS.md; no placeholder logic): | — | 高 | 按上游实现(核心编译链,优先) |
| 65 | crates/javac-core/src/code/source.rs:31 | 真实未实现(待核) | * TODO(Options/Option): `instance(Context)` uses the not yet ported | — | 中 | 按上游实现 |
| 66 | crates/javac-core/src/code/symbol.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 高 | 按上游实现(核心编译链,优先) |
| 67 | crates/javac-core/src/code/symbol.rs:5 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getRawAttributes (Symbol.java:146) | Symbol.java:146 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 68 | crates/javac-core/src/code/symbol.rs:6 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getRawTypeAttributes (Symbol.java:156) | Symbol.java:156 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 69 | crates/javac-core/src/code/symbol.rs:7 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: annotationsPendingCompletion (Symbol.java:170) | Symbol.java:170 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 70 | crates/javac-core/src/code/symbol.rs:8 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: appendAttributes (Symbol.java:174) | Symbol.java:174 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 71 | crates/javac-core/src/code/symbol.rs:9 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: appendClassInitTypeAttributes (Symbol.java:180) | Symbol.java:180 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 72 | crates/javac-core/src/code/symbol.rs:10 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: appendInitTypeAttributes (Symbol.java:186) | Symbol.java:186 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 73 | crates/javac-core/src/code/symbol.rs:11 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: appendUniqueTypeAttributes (Symbol.java:192) | Symbol.java:192 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 74 | crates/javac-core/src/code/symbol.rs:12 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getClassInitTypeAttributes (Symbol.java:198) | Symbol.java:198 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 75 | crates/javac-core/src/code/symbol.rs:13 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getInitTypeAttributes (Symbol.java:204) | Symbol.java:204 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 76 | crates/javac-core/src/code/symbol.rs:14 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: setInitTypeAttributes (Symbol.java:210) | Symbol.java:210 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 77 | crates/javac-core/src/code/symbol.rs:15 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: setClassInitTypeAttributes (Symbol.java:214) | Symbol.java:214 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 78 | crates/javac-core/src/code/symbol.rs:16 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getDeclarationAttributes (Symbol.java:218) | Symbol.java:218 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 79 | crates/javac-core/src/code/symbol.rs:17 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: hasAnnotations (Symbol.java:224) | Symbol.java:224 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 80 | crates/javac-core/src/code/symbol.rs:18 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: hasTypeAnnotations (Symbol.java:228) | Symbol.java:228 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 81 | crates/javac-core/src/code/symbol.rs:19 | 真实未实现 | - NOT PORTED: isCompleted (Symbol.java:232) | Symbol.java:232 | 高 | 按上游实现(核心编译链,优先) |
| 82 | crates/javac-core/src/code/symbol.rs:20 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: prependAttributes (Symbol.java:236) | Symbol.java:236 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 83 | crates/javac-core/src/code/symbol.rs:21 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: resetAnnotations (Symbol.java:242) | Symbol.java:242 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 84 | crates/javac-core/src/code/symbol.rs:22 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: setAttributes (Symbol.java:246) | Symbol.java:246 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 85 | crates/javac-core/src/code/symbol.rs:23 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: setDeclarationAttributes (Symbol.java:252) | Symbol.java:252 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 86 | crates/javac-core/src/code/symbol.rs:24 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: setTypeAttributes (Symbol.java:258) | Symbol.java:258 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 87 | crates/javac-core/src/code/symbol.rs:25 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: initedMetadata (Symbol.java:266) | Symbol.java:266 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 88 | crates/javac-core/src/code/symbol.rs:26 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getMetadata (Symbol.java:273) | Symbol.java:273 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 89 | crates/javac-core/src/code/symbol.rs:27 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isDirectlyOrIndirectlyLocal (Symbol.java:443) | Symbol.java:443 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 90 | crates/javac-core/src/code/symbol.rs:28 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isConstructor (Symbol.java:458) | Symbol.java:458 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 91 | crates/javac-core/src/code/symbol.rs:29 | 真实未实现 | - NOT PORTED: isInner (Symbol.java:490) | Symbol.java:490 | 高 | 按上游实现(核心编译链,优先) |
| 92 | crates/javac-core/src/code/symbol.rs:30 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: hasOuterInstance (Symbol.java:502) | Symbol.java:502 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 93 | crates/javac-core/src/code/symbol.rs:31 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: hiddenIn (Symbol.java:567) | Symbol.java:567 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 94 | crates/javac-core/src/code/symbol.rs:32 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: hiddenInInternal (Symbol.java:582) | Symbol.java:582 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 95 | crates/javac-core/src/code/symbol.rs:33 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isAccessibleIn (Symbol.java:616) | Symbol.java:616 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 96 | crates/javac-core/src/code/symbol.rs:34 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isInheritedIn (Symbol.java:651) | Symbol.java:651 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 97 | crates/javac-core/src/code/symbol.rs:35 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: asMemberOf (Symbol.java:659) | Symbol.java:659 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 98 | crates/javac-core/src/code/symbol.rs:36 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: overrides (Symbol.java:672) | Symbol.java:672 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 99 | crates/javac-core/src/code/symbol.rs:37 | 真实未实现 | - NOT PORTED: apiComplete (Symbol.java:686) | Symbol.java:686 | 高 | 按上游实现(核心编译链,优先) |
| 100 | crates/javac-core/src/code/symbol.rs:38 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: exists (Symbol.java:696) | Symbol.java:696 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 101 | crates/javac-core/src/code/symbol.rs:39 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: asType (Symbol.java:701) | Symbol.java:701 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 102 | crates/javac-core/src/code/symbol.rs:40 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getEnclosingElement (Symbol.java:706) | Symbol.java:706 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 103 | crates/javac-core/src/code/symbol.rs:41 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getModifiers (Symbol.java:716) | Symbol.java:716 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 104 | crates/javac-core/src/code/symbol.rs:42 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getSimpleName (Symbol.java:722) | Symbol.java:722 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 105 | crates/javac-core/src/code/symbol.rs:43 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getAnnotationMirrors (Symbol.java:731) | Symbol.java:731 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 106 | crates/javac-core/src/code/symbol.rs:44 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getTypeParameters (Symbol.java:743) | Symbol.java:743 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 107 | crates/javac-core/src/code/symbol.rs:51 | 真实未实现 | - NOT PORTED: getUnderlyingSymbol (Symbol.java:790) | Symbol.java:790 | 高 | 按上游实现(核心编译链,优先) |
| 108 | crates/javac-core/src/code/symbol.rs:52 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: formFullName (Symbol.java:803) | Symbol.java:803 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 109 | crates/javac-core/src/code/symbol.rs:53 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: formFlatName (Symbol.java:818) | Symbol.java:818 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 110 | crates/javac-core/src/code/symbol.rs:54 | 真实未实现 | - NOT PORTED: precedes (Symbol.java:835) | Symbol.java:835 | 高 | 按上游实现(核心编译链,优先) |
| 111 | crates/javac-core/src/code/symbol.rs:55 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getEnclosedElements (Symbol.java:852) | Symbol.java:852 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 112 | crates/javac-core/src/code/symbol.rs:56 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getAnnotationTypeMetadata (Symbol.java:867) | Symbol.java:867 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 113 | crates/javac-core/src/code/symbol.rs:57 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isAnnotationType (Symbol.java:872) | Symbol.java:872 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 114 | crates/javac-core/src/code/symbol.rs:58 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getGenericElement (Symbol.java:896) | Symbol.java:896 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 115 | crates/javac-core/src/code/symbol.rs:59 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getBounds (Symbol.java:901) | Symbol.java:901 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 116 | crates/javac-core/src/code/symbol.rs:61 | 真实未实现 | - NOT PORTED: getAttribute (Symbol.java:933) | Symbol.java:933 | 高 | 按上游实现(核心编译链,优先) |
| 117 | crates/javac-core/src/code/symbol.rs:62 | 真实未实现 | - NOT PORTED: isCurrentSymbolsAnnotation (Symbol.java:948) | Symbol.java:948 | 高 | 按上游实现(核心编译链,优先) |
| 118 | crates/javac-core/src/code/symbol.rs:63 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: create (Symbol.java:993) | Symbol.java:993 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 119 | crates/javac-core/src/code/symbol.rs:65 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isOpen (Symbol.java:1020) | Symbol.java:1020 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 120 | crates/javac-core/src/code/symbol.rs:66 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isUnnamed (Symbol.java:1025) | Symbol.java:1025 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 121 | crates/javac-core/src/code/symbol.rs:67 | 真实未实现 | - NOT PORTED: isNoModule (Symbol.java:1034) | Symbol.java:1034 | 高 | 按上游实现(核心编译链,优先) |
| 122 | crates/javac-core/src/code/symbol.rs:68 | 真实未实现 | - NOT PORTED: completeUsesProvides (Symbol.java:1050) | Symbol.java:1050 | 高 | 按上游实现(核心编译链,优先) |
| 123 | crates/javac-core/src/code/symbol.rs:70 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: reset (Symbol.java:1088) | Symbol.java:1088 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 124 | crates/javac-core/src/code/symbol.rs:73 | 真实未实现 | - NOT PORTED: mergeAttributes (Symbol.java:1200) | Symbol.java:1200 | 高 | 按上游实现(核心编译链,优先) |
| 125 | crates/javac-core/src/code/symbol.rs:80 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getInterfaces (Symbol.java:1421) | Symbol.java:1421 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 126 | crates/javac-core/src/code/symbol.rs:81 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getSuperclass (Symbol.java:1435) | Symbol.java:1435 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 127 | crates/javac-core/src/code/symbol.rs:82 | 真实未实现 | - NOT PORTED: getSuperClassToSearchForAnnotations (Symbol.java:1453) | Symbol.java:1453 | 高 | 按上游实现(核心编译链,优先) |
| 128 | crates/javac-core/src/code/symbol.rs:83 | 真实未实现 | - NOT PORTED: getInheritedAnnotations (Symbol.java:1465) | Symbol.java:1465 | 高 | 按上游实现(核心编译链,优先) |
| 129 | crates/javac-core/src/code/symbol.rs:85 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getRecordComponent (Symbol.java:1497) | Symbol.java:1497 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 130 | crates/javac-core/src/code/symbol.rs:86 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: findRecordComponentToRemove (Symbol.java:1506) | Symbol.java:1506 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 131 | crates/javac-core/src/code/symbol.rs:87 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: createRecordComponent (Symbol.java:1522) | Symbol.java:1522 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 132 | crates/javac-core/src/code/symbol.rs:88 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: setRecordComponents (Symbol.java:1539) | Symbol.java:1539 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 133 | crates/javac-core/src/code/symbol.rs:89 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getNestingKind (Symbol.java:1544) | Symbol.java:1544 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 134 | crates/javac-core/src/code/symbol.rs:91 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: markAbstractIfNeeded (Symbol.java:1580) | Symbol.java:1580 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 135 | crates/javac-core/src/code/symbol.rs:93 | 真实未实现 | - NOT PORTED: clearAnnotationMetadata (Symbol.java:1608) | Symbol.java:1608 | 高 | 按上游实现(核心编译链,优先) |
| 136 | crates/javac-core/src/code/symbol.rs:96 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: setAnnotationTypeMetadata (Symbol.java:1623) | Symbol.java:1623 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 137 | crates/javac-core/src/code/symbol.rs:97 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isRecord (Symbol.java:1629) | Symbol.java:1629 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 138 | crates/javac-core/src/code/symbol.rs:98 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getPermittedSubclasses (Symbol.java:1634) | Symbol.java:1634 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 139 | crates/javac-core/src/code/symbol.rs:99 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: asMethodHandle (Symbol.java:1670) | Symbol.java:1670 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 140 | crates/javac-core/src/code/symbol.rs:100 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: poolKey (Symbol.java:1684) | Symbol.java:1684 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 141 | crates/javac-core/src/code/symbol.rs:102 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getConstantValue (Symbol.java:1731) | Symbol.java:1731 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 142 | crates/javac-core/src/code/symbol.rs:103 | 真实未实现 | - NOT PORTED: setLazyConstValue (Symbol.java:1735) | Symbol.java:1735 | 高 | 按上游实现(核心编译链,优先) |
| 143 | crates/javac-core/src/code/symbol.rs:104 | 真实未实现 | - NOT PORTED: isExceptionParameter (Symbol.java:1751) | Symbol.java:1751 | 高 | 按上游实现(核心编译链,优先) |
| 144 | crates/javac-core/src/code/symbol.rs:105 | 真实未实现 | - NOT PORTED: isResourceVariable (Symbol.java:1755) | Symbol.java:1755 | 高 | 按上游实现(核心编译链,优先) |
| 145 | crates/javac-core/src/code/symbol.rs:106 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getConstValue (Symbol.java:1759) | Symbol.java:1759 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 146 | crates/javac-core/src/code/symbol.rs:107 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: setData (Symbol.java:1777) | Symbol.java:1777 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 147 | crates/javac-core/src/code/symbol.rs:108 | 真实未实现 | - NOT PORTED: getOriginalAnnos (Symbol.java:1826) | Symbol.java:1826 | 高 | 按上游实现(核心编译链,优先) |
| 148 | crates/javac-core/src/code/symbol.rs:109 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getAccessor (Symbol.java:1838) | Symbol.java:1838 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 149 | crates/javac-core/src/code/symbol.rs:111 | 真实未实现 | - NOT PORTED: isAliasFor (Symbol.java:1880) | Symbol.java:1880 | 高 | 按上游实现(核心编译链,优先) |
| 150 | crates/javac-core/src/code/symbol.rs:112 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: aliases (Symbol.java:1884) | Symbol.java:1884 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 151 | crates/javac-core/src/code/symbol.rs:113 | 真实未实现 | - NOT PORTED: preserveBinding (Symbol.java:1888) | Symbol.java:1888 | 高 | 按上游实现(核心编译链,优先) |
| 152 | crates/javac-core/src/code/symbol.rs:114 | 真实未实现 | - NOT PORTED: isPreserved (Symbol.java:1892) | Symbol.java:1892 | 高 | 按上游实现(核心编译链,优先) |
| 153 | crates/javac-core/src/code/symbol.rs:117 | 真实未实现 | - NOT PORTED: isHandle (Symbol.java:1974) | Symbol.java:1974 | 高 | 按上游实现(核心编译链,优先) |
| 154 | crates/javac-core/src/code/symbol.rs:118 | 真实未实现 | - NOT PORTED: asHandle (Symbol.java:1979) | Symbol.java:1979 | 高 | 按上游实现(核心编译链,优先) |
| 155 | crates/javac-core/src/code/symbol.rs:119 | 真实未实现 | - NOT PORTED: implemented (Symbol.java:1987) | Symbol.java:1987 | 高 | 按上游实现(核心编译链,优先) |
| 156 | crates/javac-core/src/code/symbol.rs:120 | 真实未实现 | - NOT PORTED: implementedIn (Symbol.java:2000) | Symbol.java:2000 | 高 | 按上游实现(核心编译链,优先) |
| 157 | crates/javac-core/src/code/symbol.rs:121 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: binaryOverrides (Symbol.java:2017) | Symbol.java:2017 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 158 | crates/javac-core/src/code/symbol.rs:122 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: binaryImplementation (Symbol.java:2042) | Symbol.java:2042 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 159 | crates/javac-core/src/code/symbol.rs:125 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isOverridableIn (Symbol.java:2120) | Symbol.java:2120 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 160 | crates/javac-core/src/code/symbol.rs:127 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isLambdaMethod (Symbol.java:2153) | Symbol.java:2153 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 161 | crates/javac-core/src/code/symbol.rs:128 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: originalEnclosingMethod (Symbol.java:2160) | Symbol.java:2160 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 162 | crates/javac-core/src/code/symbol.rs:129 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: implementation (Symbol.java:2168) | Symbol.java:2168 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 163 | crates/javac-core/src/code/symbol.rs:132 | 真实未实现 | - NOT PORTED: isStaticOrInstanceInit (Symbol.java:2221) | Symbol.java:2221 | 高 | 按上游实现(核心编译链,优先) |
| 164 | crates/javac-core/src/code/symbol.rs:133 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getDefaultValue (Symbol.java:2227) | Symbol.java:2227 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 165 | crates/javac-core/src/code/symbol.rs:134 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getParameters (Symbol.java:2232) | Symbol.java:2232 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 166 | crates/javac-core/src/code/symbol.rs:135 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isVarArgs (Symbol.java:2237) | Symbol.java:2237 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 167 | crates/javac-core/src/code/symbol.rs:136 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isDefault (Symbol.java:2242) | Symbol.java:2242 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 168 | crates/javac-core/src/code/symbol.rs:137 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getReceiverType (Symbol.java:2256) | Symbol.java:2256 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 169 | crates/javac-core/src/code/symbol.rs:138 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getReturnType (Symbol.java:2262) | Symbol.java:2262 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 170 | crates/javac-core/src/code/symbol.rs:139 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getThrownTypes (Symbol.java:2267) | Symbol.java:2267 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 171 | crates/javac-core/src/code/symbol.rs:140 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: staticArgs (Symbol.java:2291) | Symbol.java:2291 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 172 | crates/javac-core/src/code/symbol.rs:141 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: bootstrapMethod (Symbol.java:2296) | Symbol.java:2296 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 173 | crates/javac-core/src/code/symbol.rs:142 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: dynamicType (Symbol.java:2306) | Symbol.java:2306 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 174 | crates/javac-core/src/code/symbol.rs:146 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: referenceKind (Symbol.java:2369) | Symbol.java:2369 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 175 | crates/javac-core/src/code/symbol.rs:150 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getAccessCode (Symbol.java:2442) | Symbol.java:2442 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 176 | crates/javac-core/src/code/symbol.rs:151 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getFromCode (Symbol.java:2481) | Symbol.java:2481 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 177 | crates/javac-core/src/code/symbol.rs:152 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isTerminal (Symbol.java:2525) | Symbol.java:2525 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 178 | crates/javac-core/src/code/symbol.rs:154 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getDiagnostic (Symbol.java:2562) | Symbol.java:2562 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 179 | crates/javac-core/src/code/symbol.rs:155 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getMessage (Symbol.java:2570) | Symbol.java:2570 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 180 | crates/javac-core/src/code/symbol.rs:156 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getDetailValue (Symbol.java:2574) | Symbol.java:2574 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 181 | crates/javac-core/src/code/symbol.rs:157 | 真实未实现 | - NOT PORTED: initCause (Symbol.java:2579) | Symbol.java:2579 | 高 | 按上游实现(核心编译链,优先) |
| 182 | crates/javac-core/src/code/symbol.rs:158 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: resetDiagnostic (Symbol.java:2584) | Symbol.java:2584 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 183 | crates/javac-core/src/code/symbol.rs:159 | 真实未实现 | - NOT PORTED: visitClassSymbol (Symbol.java:2606) | Symbol.java:2606 | 高 | 按上游实现(核心编译链,优先) |
| 184 | crates/javac-core/src/code/symbol.rs:160 | 真实未实现 | - NOT PORTED: visitMethodSymbol (Symbol.java:2607) | Symbol.java:2607 | 高 | 按上游实现(核心编译链,优先) |
| 185 | crates/javac-core/src/code/symbol.rs:161 | 真实未实现 | - NOT PORTED: visitPackageSymbol (Symbol.java:2608) | Symbol.java:2608 | 高 | 按上游实现(核心编译链,优先) |
| 186 | crates/javac-core/src/code/symbol.rs:162 | 真实未实现 | - NOT PORTED: visitOperatorSymbol (Symbol.java:2609) | Symbol.java:2609 | 高 | 按上游实现(核心编译链,优先) |
| 187 | crates/javac-core/src/code/symbol.rs:163 | 真实未实现 | - NOT PORTED: visitVarSymbol (Symbol.java:2610) | Symbol.java:2610 | 高 | 按上游实现(核心编译链,优先) |
| 188 | crates/javac-core/src/code/symbol.rs:164 | 真实未实现 | - NOT PORTED: visitTypeSymbol (Symbol.java:2611) | Symbol.java:2611 | 高 | 按上游实现(核心编译链,优先) |
| 189 | crates/javac-core/src/code/symbol.rs:165 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitSymbol (Symbol.java:2612) | Symbol.java:2612 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 190 | crates/javac-core/src/code/symbol.rs:189 | 真实未实现 | NOT PORTED IN THIS STEP (see docs/STATUS.md; no placeholder logic): | — | 高 | 按上游实现(核心编译链,优先) |
| 191 | crates/javac-core/src/code/symbol.rs:203 | 真实未实现 | value stored in data is not ported yet) and MethodHandleSymbol private | — | 高 | 按上游实现(核心编译链,优先) |
| 192 | crates/javac-core/src/code/symbol.rs:204 | 真实未实现 | fields are not represented (dependencies not ported yet; reported); | — | 高 | 按上游实现(核心编译链,优先) |
| 193 | crates/javac-core/src/code/symbol.rs:205 | 真实未实现 | - RootPackageSymbol.missingInfoHandler (MissingInfoHandler is not ported; | — | 高 | 按上游实现(核心编译链,优先) |
| 194 | crates/javac-core/src/code/symbol.rs:481 | 真实未实现(待核) | TODO(Annotate): SymbolMetadata is not ported yet; there is | — | 高 | 按上游实现(核心编译链,优先) |
| 195 | crates/javac-core/src/code/symbol.rs:559 | 真实未实现 | subclasses, which are not ported yet; the base behavior is false). | — | 高 | 按上游实现(核心编译链,优先) |
| 196 | crates/javac-core/src/code/symbol.rs:720 | 真实未实现 | that are not ported yet. | — | 高 | 按上游实现(核心编译链,优先) |
| 197 | crates/javac-core/src/code/symbol.rs:945 | 真实未实现 | value stored in data is not ported yet, see the module doc). | — | 高 | 按上游实现(核心编译链,优先) |
| 198 | crates/javac-core/src/code/symbol.rs:970 | 真实未实现 | (Attribute) are not ported yet (see the module doc). | — | 高 | 按上游实现(核心编译链,优先) |
| 199 | crates/javac-core/src/code/symtab.rs:4 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isTerminal (Symtab.java:324) | Symtab.java:324 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 200 | crates/javac-core/src/code/symtab.rs:6 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: accept (Symtab.java:447) | Symtab.java:447 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 201 | crates/javac-core/src/code/symtab.rs:8 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getClassesForName (Symtab.java:875) | Symtab.java:875 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 202 | crates/javac-core/src/code/symtab.rs:9 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getPackagesForName (Symtab.java:879) | Symtab.java:879 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 203 | crates/javac-core/src/code/symtab.rs:37 | 纯文档说明 | values (registered; the localized toString is a Symbol TODO); | — | 高 | 保留(移植说明) |
| 204 | crates/javac-core/src/code/symtab.rs:42 | 真实未实现 | NOT PORTED IN THIS STEP (dependencies not ported yet; no substitute logic): | — | 高 | 按上游实现(核心编译链,优先) |
| 205 | crates/javac-core/src/code/symtab.rs:54 | 真实未实现(待核) | Types methods are NOT PORTED (TODO Types; see types.rs). | — | 高 | 按上游实现(核心编译链,优先) |
| 206 | crates/javac-core/src/code/symtab.rs:514 | 真实未实现 | NOT PORTED (Completer is not ported). | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 207 | crates/javac-core/src/code/symtab.rs:560 | 真实未实现 | Java: c.completer = initialCompleter; - NOT PORTED (Completer). | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 208 | crates/javac-core/src/code/symtab.rs:652 | 真实未实现 | WriteableScope.create(this); the rest of complete() is not ported | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 209 | crates/javac-core/src/code/symtab.rs:653 | 真实未实现(待核) | (PackageSymbol completer TODO), the members scope is created here. | — | 高 | 按上游实现(核心编译链,优先) |
| 210 | crates/javac-core/src/code/symtab.rs:875 | 真实未实现 | Java: msym.completer = s -> moduleCompleter.complete(s); - NOT PORTED. | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 211 | crates/javac-core/src/code/symtab.rs:986 | 真实未实现 | NOT PORTED: needs the anonymous Completer of the upstream synthesize | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 212 | crates/javac-core/src/code/symtab.rs:989 | 真实未实现 | NOT PORTED: see profile_type. | — | 高 | 按上游实现(核心编译链,优先) |
| 213 | crates/javac-core/src/code/symtab.rs:993 | 真实未实现 | Java: private final Completer moduleCompleter (from Modules; not ported -> None) | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 214 | crates/javac-core/src/code/symtab.rs:1042 | 真实未实现(待核) | not ported yet (Options/Option integration TODO), so the caller passes | — | 高 | 按上游实现(核心编译链,优先) |
| 215 | crates/javac-core/src/code/symtab.rs:1067 | 真实未实现(待核) | TODO(MissingInfoHandler): MissingInfoHandler.instance(context) is | — | 高 | 按上游实现(核心编译链,优先) |
| 216 | crates/javac-core/src/code/symtab.rs:1068 | 真实未实现 | not ported; the root package is created without it. | — | 高 | 按上游实现(核心编译链,优先) |
| 217 | crates/javac-core/src/code/symtab.rs:1084 | 真实未实现(待核) | anonymous override is not expressible (Symbol TODO). | — | 高 | 按上游实现(核心编译链,优先) |
| 218 | crates/javac-core/src/code/symtab.rs:1241 | 真实未实现 | NOT PORTED (ClassFinder/Completer). | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 219 | crates/javac-core/src/code/symtab.rs:1262 | 真实未实现 | NOT PORTED (Modules/Completer). | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 220 | crates/javac-core/src/code/symtab.rs:1403 | 真实未实现 | NOT PORTED: valueBasedInternalType = enterSyntheticAnnotation(...) | — | 高 | 按上游实现(核心编译链,优先) |
| 221 | crates/javac-core/src/code/symtab.rs:1405 | 真实未实现 | NOT PORTED: synthesizeEmptyInterfaceIfMissing(...) and | — | 高 | 按上游实现(核心编译链,优先) |
| 222 | crates/javac-core/src/code/symtab.rs:1919 | 真实未实现 | Modules.instance(context).getCompleter() (Modules is not ported: the | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 223 | crates/javac-core/src/code/type_.rs:32 | 真实未实现 | Option<SymbolId> (Symbol is not ported yet, so only its arena ID is used); | — | 中 | 按上游实现 |
| 224 | crates/javac-core/src/code/type_.rs:39 | 纯文档说明 | annotation-based metadata machinery) is not ported yet, so the adapter | — | 中 | 保留(移植说明) |
| 225 | crates/javac-core/src/code/type_.rs:43 | 真实未实现 | NOT PORTED IN THIS STEP (see docs/STATUS.md; do not add placeholder logic): | — | 中 | 按上游实现 |
| 226 | crates/javac-core/src/code/type_.rs:60 | 真实未实现 | getThrownTypes, allparams) are not ported yet: they need the arena/ID | — | 中 | 按上游实现 |
| 227 | crates/javac-core/src/code/type_.rs:159 | 真实未实现 | which is not ported yet (see the module doc). | — | 中 | 按上游实现 |
| 228 | crates/javac-core/src/code/type_.rs:453 | 真实未实现 | override is not ported yet because it needs the arena). | — | 中 | 按上游实现 |
| 229 | crates/javac-core/src/code/types.rs:7 | 真实未实现 | - NOT PORTED: needsStripping (Types.java:276) | Types.java:276 | 高 | 按上游实现(核心编译链,优先) |
| 230 | crates/javac-core/src/code/types.rs:10 | 真实未实现 | - NOT PORTED: visitCapturedType (Types.java:478) | Types.java:478 | 高 | 按上游实现(核心编译链,优先) |
| 231 | crates/javac-core/src/code/types.rs:11 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: asSub (Types.java:539) | Types.java:539 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 232 | crates/javac-core/src/code/types.rs:12 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: setMessage (Types.java:649) | Types.java:649 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 233 | crates/javac-core/src/code/types.rs:13 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getDiagnostic (Types.java:654) | Types.java:654 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 234 | crates/javac-core/src/code/types.rs:14 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getSymbol (Types.java:674) | Types.java:674 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 235 | crates/javac-core/src/code/types.rs:15 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: findDescriptorInternal (Types.java:732) | Types.java:732 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 236 | crates/javac-core/src/code/types.rs:16 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: mergeDescriptors (Types.java:793) | Types.java:793 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 237 | crates/javac-core/src/code/types.rs:17 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: failure (Types.java:804) | Types.java:804 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 238 | crates/javac-core/src/code/types.rs:19 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: findDescriptorSymbol (Types.java:819) | Types.java:819 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 239 | crates/javac-core/src/code/types.rs:20 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: findDescriptorType (Types.java:827) | Types.java:827 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 240 | crates/javac-core/src/code/types.rs:21 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isFunctionalInterface (Types.java:834) | Types.java:834 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 241 | crates/javac-core/src/code/types.rs:23 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: removeWildcards (Types.java:852) | Types.java:852 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 242 | crates/javac-core/src/code/types.rs:24 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: makeFunctionalInterfaceClass (Types.java:900) | Types.java:900 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 243 | crates/javac-core/src/code/types.rs:25 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: functionalInterfaceBridges (Types.java:926) | Types.java:926 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 244 | crates/javac-core/src/code/types.rs:26 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: pendingBridges (Types.java:959) | Types.java:959 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 245 | crates/javac-core/src/code/types.rs:27 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: checkUnsafeVarargsConversion (Types.java:1044) | Types.java:1044 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 246 | crates/javac-core/src/code/types.rs:28 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitUndetVar (Types.java:1214) | Types.java:1214 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 247 | crates/javac-core/src/code/types.rs:29 | 真实未实现 | - NOT PORTED: isSubtypes (Types.java:1250) | Types.java:1250 | 高 | 按上游实现(核心编译链,优先) |
| 248 | crates/javac-core/src/code/types.rs:30 | 真实未实现 | - NOT PORTED: isSubtypesUnchecked (Types.java:1266) | Types.java:1266 | 高 | 按上游实现(核心编译链,优先) |
| 249 | crates/javac-core/src/code/types.rs:31 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isSignaturePolymorphic (Types.java:1327) | Types.java:1327 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 250 | crates/javac-core/src/code/types.rs:32 | 真实未实现 | - NOT PORTED: visitPackageType (Types.java:1447) | Types.java:1447 | 高 | 按上游实现(核心编译链,优先) |
| 251 | crates/javac-core/src/code/types.rs:35 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: disjointTypes (Types.java:1888) | Types.java:1888 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 252 | crates/javac-core/src/code/types.rs:36 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: disjointType (Types.java:1906) | Types.java:1906 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 253 | crates/javac-core/src/code/types.rs:37 | 真实未实现 | - NOT PORTED: isCastableRecursive (Types.java:1922) | Types.java:1922 | 高 | 按上游实现(核心编译链,优先) |
| 254 | crates/javac-core/src/code/types.rs:38 | 真实未实现 | - NOT PORTED: notSoftSubtypeRecursive (Types.java:1935) | Types.java:1935 | 高 | 按上游实现(核心编译链,优先) |
| 255 | crates/javac-core/src/code/types.rs:39 | 真实未实现 | - NOT PORTED: cvarLowerBounds (Types.java:1978) | Types.java:1978 | 高 | 按上游实现(核心编译链,优先) |
| 256 | crates/javac-core/src/code/types.rs:41 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: relaxBound (Types.java:2012) | Types.java:2012 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 257 | crates/javac-core/src/code/types.rs:42 | 真实未实现 | - NOT PORTED: isArray (Types.java:2059) | Types.java:2059 | 高 | 按上游实现(核心编译链,优先) |
| 258 | crates/javac-core/src/code/types.rs:43 | 真实未实现 | - NOT PORTED: elemtypeOrType (Types.java:2083) | Types.java:2083 | 高 | 按上游实现(核心编译链,优先) |
| 259 | crates/javac-core/src/code/types.rs:44 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: dimensions (Types.java:2108) | Types.java:2108 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 260 | crates/javac-core/src/code/types.rs:45 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: makeArrayType (Types.java:2123) | Types.java:2123 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 261 | crates/javac-core/src/code/types.rs:46 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isAssignable (Types.java:2338) | Types.java:2338 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 262 | crates/javac-core/src/code/types.rs:48 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isDirectSuperInterface (Types.java:2659) | Types.java:2659 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 263 | crates/javac-core/src/code/types.rs:49 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isDerivedRaw (Types.java:2670) | Types.java:2670 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 264 | crates/javac-core/src/code/types.rs:50 | 真实未实现 | - NOT PORTED: isDerivedRawInternal (Types.java:2679) | Types.java:2679 | 高 | 按上游实现(核心编译链,优先) |
| 265 | crates/javac-core/src/code/types.rs:52 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: setBounds (Types.java:2706) | Types.java:2706 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 266 | crates/javac-core/src/code/types.rs:54 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getBounds (Types.java:2731) | Types.java:2731 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 267 | crates/javac-core/src/code/types.rs:55 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isSubSignature (Types.java:2795) | Types.java:2795 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 268 | crates/javac-core/src/code/types.rs:57 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: overrideEquivalent (Types.java:2816) | Types.java:2816 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 269 | crates/javac-core/src/code/types.rs:58 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: overridesObjectMethod (Types.java:2821) | Types.java:2821 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 270 | crates/javac-core/src/code/types.rs:59 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: mergeAbstracts (Types.java:2874) | Types.java:2874 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 271 | crates/javac-core/src/code/types.rs:60 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: baseSymbol (Types.java:2921) | Types.java:2921 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 272 | crates/javac-core/src/code/types.rs:61 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: implementationInternal (Types.java:2978) | Types.java:2978 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 273 | crates/javac-core/src/code/types.rs:62 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: implementation (Types.java:3003) | Types.java:3003 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 274 | crates/javac-core/src/code/types.rs:63 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getSymbols (Types.java:3029) | Types.java:3029 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 275 | crates/javac-core/src/code/types.rs:64 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getSymbolsByName (Types.java:3034) | Types.java:3034 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 276 | crates/javac-core/src/code/types.rs:65 | 真实未实现 | - NOT PORTED: getMark (Types.java:3039) | Types.java:3039 | 高 | 按上游实现(核心编译链,优先) |
| 277 | crates/javac-core/src/code/types.rs:66 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: membersClosure (Types.java:3091) | Types.java:3091 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 278 | crates/javac-core/src/code/types.rs:67 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: firstUnimplementedAbstract (Types.java:3101) | Types.java:3101 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 279 | crates/javac-core/src/code/types.rs:68 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: firstUnimplementedAbstractImpl (Types.java:3110) | Types.java:3110 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 280 | crates/javac-core/src/code/types.rs:69 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: interfaceCandidates (Types.java:3187) | Types.java:3187 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 281 | crates/javac-core/src/code/types.rs:70 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: prune (Types.java:3206) | Types.java:3206 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 282 | crates/javac-core/src/code/types.rs:73 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: substBounds (Types.java:3407) | Types.java:3407 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 283 | crates/javac-core/src/code/types.rs:74 | 真实未实现 | - NOT PORTED: substBound (Types.java:3448) | Types.java:3448 | 高 | 按上游实现(核心编译链,优先) |
| 284 | crates/javac-core/src/code/types.rs:75 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: newInstances (Types.java:3487) | Types.java:3487 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 285 | crates/javac-core/src/code/types.rs:76 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: createMethodTypeWithParameters (Types.java:3503) | Types.java:3503 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 286 | crates/javac-core/src/code/types.rs:77 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: createMethodTypeWithThrown (Types.java:3519) | Types.java:3519 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 287 | crates/javac-core/src/code/types.rs:78 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: createMethodTypeWithReturn (Types.java:3535) | Types.java:3535 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 288 | crates/javac-core/src/code/types.rs:79 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: baseType (Types.java:3546) | Types.java:3546 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 289 | crates/javac-core/src/code/types.rs:81 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: createErrorType (Types.java:3562) | Types.java:3562 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 290 | crates/javac-core/src/code/types.rs:84 | 真实未实现 | - NOT PORTED: typaramsString (Types.java:3656) | Types.java:3656 | 高 | 按上游实现(核心编译链,优先) |
| 291 | crates/javac-core/src/code/types.rs:85 | 真实未实现 | - NOT PORTED: appendTyparamString (Types.java:3668) | Types.java:3668 | 高 | 按上游实现(核心编译链,优先) |
| 292 | crates/javac-core/src/code/types.rs:86 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: closure (Types.java:3710) | Types.java:3710 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 293 | crates/javac-core/src/code/types.rs:87 | 真实未实现 | - NOT PORTED: closureCollector (Types.java:3735) | Types.java:3735 | 高 | 按上游实现(核心编译链,优先) |
| 294 | crates/javac-core/src/code/types.rs:88 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: merge (Types.java:3757) | Types.java:3757 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 295 | crates/javac-core/src/code/types.rs:91 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: compoundMin (Types.java:3900) | Types.java:3900 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 296 | crates/javac-core/src/code/types.rs:92 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: closureMin (Types.java:3915) | Types.java:3915 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 297 | crates/javac-core/src/code/types.rs:93 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: glbFlattened (Types.java:4124) | Types.java:4124 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 298 | crates/javac-core/src/code/types.rs:95 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: resultSubtype (Types.java:4249) | Types.java:4249 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 299 | crates/javac-core/src/code/types.rs:96 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: returnTypeSubstitutable (Types.java:4261) | Types.java:4261 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 300 | crates/javac-core/src/code/types.rs:98 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: covariantReturnType (Types.java:4292) | Types.java:4292 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 301 | crates/javac-core/src/code/types.rs:99 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: boxedTypeOrType (Types.java:4312) | Types.java:4312 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 302 | crates/javac-core/src/code/types.rs:100 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: unboxedTypeOrType (Types.java:4336) | Types.java:4336 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 303 | crates/javac-core/src/code/types.rs:101 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: sideCast (Types.java:4479) | Types.java:4479 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 304 | crates/javac-core/src/code/types.rs:102 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: sideCastFinal (Types.java:4509) | Types.java:4509 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 305 | crates/javac-core/src/code/types.rs:103 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: giveWarning (Types.java:4535) | Types.java:4535 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 306 | crates/javac-core/src/code/types.rs:104 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: superClosure (Types.java:4550) | Types.java:4550 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 307 | crates/javac-core/src/code/types.rs:105 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: adaptRecursive (Types.java:4664) | Types.java:4664 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 308 | crates/javac-core/src/code/types.rs:107 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: rewriteQuantifiers (Types.java:4720) | Types.java:4720 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 309 | crates/javac-core/src/code/types.rs:109 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: rewriteAsWildcardType (Types.java:4784) | Types.java:4784 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 310 | crates/javac-core/src/code/types.rs:110 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: makeExtendsWildcard (Types.java:4822) | Types.java:4822 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 311 | crates/javac-core/src/code/types.rs:111 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: makeSuperWildcard (Types.java:4844) | Types.java:4844 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 312 | crates/javac-core/src/code/types.rs:115 | 真实未实现 | - NOT PORTED: visitClassSymbol (Types.java:4928) | Types.java:4928 | 高 | 按上游实现(核心编译链,优先) |
| 313 | crates/javac-core/src/code/types.rs:116 | 真实未实现 | - NOT PORTED: visitMethodSymbol (Types.java:4929) | Types.java:4929 | 高 | 按上游实现(核心编译链,优先) |
| 314 | crates/javac-core/src/code/types.rs:117 | 真实未实现 | - NOT PORTED: visitOperatorSymbol (Types.java:4930) | Types.java:4930 | 高 | 按上游实现(核心编译链,优先) |
| 315 | crates/javac-core/src/code/types.rs:118 | 真实未实现 | - NOT PORTED: visitPackageSymbol (Types.java:4931) | Types.java:4931 | 高 | 按上游实现(核心编译链,优先) |
| 316 | crates/javac-core/src/code/types.rs:119 | 真实未实现 | - NOT PORTED: visitTypeSymbol (Types.java:4932) | Types.java:4932 | 高 | 按上游实现(核心编译链,优先) |
| 317 | crates/javac-core/src/code/types.rs:120 | 真实未实现 | - NOT PORTED: visitVarSymbol (Types.java:4933) | Types.java:4933 | 高 | 按上游实现(核心编译链,优先) |
| 318 | crates/javac-core/src/code/types.rs:123 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: apply (Types.java:5005) | Types.java:5005 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 319 | crates/javac-core/src/code/types.rs:125 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getRetention (Types.java:5021) | Types.java:5021 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 320 | crates/javac-core/src/code/types.rs:127 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: append (Types.java:5062) | Types.java:5062 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 321 | crates/javac-core/src/code/types.rs:130 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: classReference (Types.java:5065) | Types.java:5065 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 322 | crates/javac-core/src/code/types.rs:131 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: reportIllegalSignature (Types.java:5071) | Types.java:5071 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 323 | crates/javac-core/src/code/types.rs:132 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: assembleSig (Types.java:5078) | Types.java:5078 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 324 | crates/javac-core/src/code/types.rs:133 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: hasTypeVar (Types.java:5170) | Types.java:5170 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 325 | crates/javac-core/src/code/types.rs:134 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: assembleClassSig (Types.java:5180) | Types.java:5180 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 326 | crates/javac-core/src/code/types.rs:135 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: assembleParamsSig (Types.java:5207) | Types.java:5207 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 327 | crates/javac-core/src/code/types.rs:137 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: constantType (Types.java:5231) | Types.java:5231 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 328 | crates/javac-core/src/code/types.rs:138 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: newRound (Types.java:5257) | Types.java:5257 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 329 | crates/javac-core/src/code/types.rs:145 | 真实未实现 | reports 164 not-yet-ported semantic methods. Representative groups (the | — | 高 | 按上游实现(核心编译链,优先) |
| 330 | crates/javac-core/src/code/types.rs:171 | 真实未实现(待核) | remaining Types methods are NOT PORTED (TODO Types): | — | 高 | 按上游实现(核心编译链,优先) |
| 331 | crates/javac-core/src/code/types.rs:175 | 真实未实现 | is NOT PORTED (containsType needs isSubtype): the ported | — | 高 | 按上游实现(核心编译链,优先) |
| 332 | crates/javac-core/src/code/types.rs:179 | 真实未实现 | - isSameType ForAll branch: hasSameBounds + subst are NOT PORTED (panic); | — | 高 | 按上游实现(核心编译链,优先) |
| 333 | crates/javac-core/src/code/types.rs:182 | 真实未实现 | - UndetVar: the TypeKindData variant is not ported, the branch is | — | 高 | 按上游实现(核心编译链,优先) |
| 334 | crates/javac-core/src/code/types.rs:187 | 纯文档说明 | PORTING_RULES (upstream helpers that live in Type.java but are not ported in | — | 高 | 保留(移植说明) |
| 335 | crates/javac-core/src/code/types.rs:871 | 真实未实现 | NOT PORTED: a null enclosing type is an upstream assertion | — | 高 | 按上游实现(核心编译链,优先) |
| 336 | crates/javac-core/src/code/types.rs:1243 | 真实未实现 | both are reproduced here (TypeMetadata is not ported). | — | 高 | 按上游实现(核心编译链,优先) |
| 337 | crates/javac-core/src/code/types.rs:2161 | 真实未实现(待核) | glb (TODO Types): a type that does not need capture conversion is | — | 高 | 按上游实现(核心编译链,优先) |
| 338 | crates/javac-core/src/code/types.rs:2845 | 纯文档说明 | PORTING_RULES: the upstream rank_field caches (Type, not ported) | — | 高 | 保留(移植说明) |
| 339 | crates/javac-core/src/code/types.rs:3026 | 真实未实现(待核) | TODO(Types): the strict=false HasSameArgs variant is not ported. | — | 高 | 按上游实现(核心编译链,优先) |
| 340 | crates/javac-core/src/code/types.rs:3264 | 真实未实现(待核) | emitted here (the lint wiring is TODO with the Attr); the boolean | — | 高 | 按上游实现(核心编译链,优先) |
| 341 | crates/javac-core/src/code/types.rs:3332 | 纯文档说明 | (Types.java:1663) is not ported yet (Varargs, registered). | Types.java:1663 | 高 | 保留(移植说明) |
| 342 | crates/javac-core/src/code/types.rs:3585 | 真实未实现 | is not ported yet); the boolean relation is unchanged. | — | 高 | 按上游实现(核心编译链,优先) |
| 343 | crates/javac-core/src/code/types.rs:3719 | 纯文档说明 | PORTING_RULES: reproduced here because type_.rs has not ported getEnclosingType | — | 高 | 保留(移植说明) |
| 344 | crates/javac-core/src/code/types.rs:3720 | 纯文档说明 | yet (TODO Type; registered). | — | 高 | 保留(移植说明) |
| 345 | crates/javac-core/src/code/types.rs:3737 | 纯文档说明 | ported, so the nil default is used (TODO Symbol; registered). | — | 高 | 保留(移植说明) |
| 346 | crates/javac-core/src/code/types.rs:3753 | 纯文档说明 | PORTING_RULES: reproduced here because type_.rs has not ported it yet (TODO | — | 高 | 保留(移植说明) |
| 347 | crates/javac-core/src/code/types.rs:3769 | 真实未实现(待核) | helpers because type_.rs/symbol.rs have not ported them yet (TODO Type/Symbol | — | 高 | 按上游实现(核心编译链,优先) |
| 348 | crates/javac-core/src/code/types.rs:3770 | 纯文档说明 | ; registered). Symbol.complete() is not ported, so the lazy completion | — | 高·核心缺口 | 保留(移植说明) |
| 349 | crates/javac-core/src/code/types.rs:3900 | 真实未实现 | Java: Symbol.ClassSymbol.getSuperclass() (Symbol.complete() is not ported: a | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 350 | crates/javac-core/src/code/types.rs:3919 | 真实未实现 | Java: Symbol.ClassSymbol.getInterfaces() (Symbol.complete() is not ported; a | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 351 | crates/javac-core/src/code/types.rs:4135 | 真实未实现 | upstream uses are in Attr and other not-yet-ported callers, not Types.java). | — | 高 | 按上游实现(核心编译链,优先) |
| 352 | crates/javac-core/src/code/types.rs:4137 | 真实未实现(依赖批次) | TODO(Attr/Type.contains): not wired yet. Keep until that wires | — | 高 | 依赖批次接线(按上游) |
| 353 | crates/javac-core/src/code/types.rs:4386 | 真实未实现 | Java: t.rank_field = -1; — TypeVar.rank_field is not ported yet | — | 高 | 按上游实现(核心编译链,优先) |
| 354 | crates/javac-core/src/code/types.rs:6907 | 真实未实现(待核) | .expect("TODO(Types batch): lub needs the Names service (Types::with_names)"); | — | 高 | 按上游实现(核心编译链,优先) |
| 355 | crates/javac-core/src/comp/analyzer.rs:637 | 真实未实现(待核) | TODO(ArgumentAttr): thread the local cache context into the speculative | — | 中 | 按上游实现 |
| 356 | crates/javac-core/src/comp/annotate.rs:1609 | 真实未实现(待核) | TODO(TypeAnnotations): Check.isTypeAnnotation is not ported. | — | 中 | 按上游实现 |
| 357 | crates/javac-core/src/comp/attr_context.rs:9 | 真实未实现 | - Attr.ResultInfo (returnResult/yieldResult) is not ported (Attr): the | — | 高 | 按上游实现(核心编译链,优先) |
| 358 | crates/javac-core/src/comp/attr_context.rs:10 | 纯文档说明 | fields are omitted and dup() skips them; registered TODO, no substitute; | — | 高 | 保留(移植说明) |
| 359 | crates/javac-core/src/comp/attr.rs:17 | 纯文档说明 | NOT PORTED IN THIS (registered; dependency batches not ported yet, no | — | 高 | 保留(移植说明) |
| 360 | crates/javac-core/src/comp/attr.rs:634 | 真实未实现(待核) | TODO(MatchBindingsComputer): matchBindings = | — | 高 | 按上游实现(核心编译链,优先) |
| 361 | crates/javac-core/src/comp/attr.rs:636 | 真实未实现(待核) | TODO(DeferredAttr): if (tree == breakTree && | — | 高 | 按上游实现(核心编译链,优先) |
| 362 | crates/javac-core/src/comp/attr.rs:708 | 纯文档说明 | registered adapter). The visitor methods not ported in A are | — | 高 | 保留(移植说明) |
| 363 | crates/javac-core/src/comp/attr.rs:1010 | 真实未实现(待核) | TODO(Check): chk.checkRefType(tree.pos(), type) | — | 高 | 按上游实现(核心编译链,优先) |
| 364 | crates/javac-core/src/comp/attr.rs:1293 | 真实未实现(待核) | TODO(Symbol/Types): sym.isMemberOf(symEnv.enclClass.sym, | — | 高 | 按上游实现(核心编译链,优先) |
| 365 | crates/javac-core/src/comp/attr.rs:1357 | 真实未实现 | NOT PORTED: chk.checkAccessFromSerializableElement(tree, | — | 高 | 按上游实现(核心编译链,优先) |
| 366 | crates/javac-core/src/comp/attr.rs:1462 | 真实未实现(待核) | TODO(Lint/Resolve): chk.checkForBadAuxiliaryClassAccess( | — | 高 | 按上游实现(核心编译链,优先) |
| 367 | crates/javac-core/src/comp/attr.rs:1653 | 真实未实现(待核) | TODO(Check): chk.checkDeprecated/checkSunAPI/checkProfile/ | — | 高 | 按上游实现(核心编译链,优先) |
| 368 | crates/javac-core/src/comp/attr.rs:1654 | 纯文档说明 | checkPreview(pos,...) are not ported yet (registered). | — | 高 | 保留(移植说明) |
| 369 | crates/javac-core/src/comp/attr.rs:1722 | 真实未实现 | 3922-4182). Methods whose upstream body needs a not-yet-ported service are | — | 高 | 按上游实现(核心编译链,优先) |
| 370 | crates/javac-core/src/comp/attr.rs:1723 | 纯文档说明 | registered with a not-yet-ported marker + TODO at the exact call site. | — | 高 | 保留(移植说明) |
| 371 | crates/javac-core/src/comp/attr.rs:2549 | 真实未实现(待核) | TODO(Check): chk.checkNonCyclic(tvar.pos(), (TypeVar)tvar.type) | — | 高 | 按上游实现(核心编译链,优先) |
| 372 | crates/javac-core/src/comp/attr.rs:6037 | 真实未实现 | NOT PORTED: the source-level feature error logging (Features); | — | 高 | 按上游实现(核心编译链,优先) |
| 373 | crates/javac-core/src/comp/attr.rs:8546 | 纯文档说明 | TODO(Preview): the source-level preview check is registered with the | — | 高 | 保留(移植说明) |
| 374 | crates/javac-core/src/comp/attr.rs:8548 | 真实未实现 | pattern switch (the Preview.checkSourceLevel call is not ported). | — | 高 | 按上游实现(核心编译链,优先) |
| 375 | crates/javac-core/src/comp/attr.rs:8616 | 纯文档说明 | TODO(Preview): source-level CASE_NULL check registered with the | — | 高 | 保留(移植说明) |
| 376 | crates/javac-core/src/comp/attr.rs:8662 | 纯文档说明 | TODO(Resolve): the Resolve LogResolveHelper swap is registered | — | 高 | 保留(移植说明) |
| 377 | crates/javac-core/src/comp/attr.rs:8663 | 真实未实现 | with the Resolve service integration and is not ported. | — | 高 | 按上游实现(核心编译链,优先) |
| 378 | crates/javac-core/src/comp/attr.rs:9071 | 真实未实现(待核) | TODO(Check): the unchecked-cast warning in checkCastablePattern is | — | 高 | 按上游实现(核心编译链,优先) |
| 379 | crates/javac-core/src/comp/attr.rs:11706 | 真实未实现 | - the module/package declaration branches need Modules (not ported yet). | — | 高 | 按上游实现(核心编译链,优先) |
| 380 | crates/javac-core/src/comp/attr.rs:15533 | 测试或工具 | Resolve is not ported yet (registered: Attr.visitIdent identifier | — | 高 | 不改(测试/工具) |
| 381 | crates/javac-core/src/comp/attr.rs:15725 | 测试或工具 | path calls Attr.attribType (TODO(Attr), not ported yet), so the | — | 高 | 不改(测试/工具) |
| 382 | crates/javac-core/src/comp/check.rs:12 | 纯文档说明 | STILL NOT PORTED (registered; dependencies in brackets): | — | 高 | 保留(移植说明) |
| 383 | crates/javac-core/src/comp/check.rs:24 | 真实未实现(待核) | is not ported the branch is marked TODO and not executed (no substitute). | — | 高 | 按上游实现(核心编译链,优先) |
| 384 | crates/javac-core/src/comp/check.rs:762 | 真实未实现(待核) | TODO(Check): warnOnExplicitStrictfp uses the | — | 高 | 按上游实现(核心编译链,优先) |
| 385 | crates/javac-core/src/comp/check.rs:763 | 纯文档说明 | DeferredLintHandler, which is not ported (registered). | — | 高 | 保留(移植说明) |
| 386 | crates/javac-core/src/comp/check.rs:859 | 真实未实现(待核) | TODO(Check): warnOnExplicitStrictfp (DeferredLintHandler) | — | 高 | 按上游实现(核心编译链,优先) |
| 387 | crates/javac-core/src/comp/check.rs:956 | 真实未实现 | containsType, which are not ported yet; those branches are explicitly | — | 高 | 按上游实现(核心编译链,优先) |
| 388 | crates/javac-core/src/comp/check.rs:1893 | 真实未实现 | ports the Annotate-facing part; the branches that need not-yet-ported units | — | 高 | 按上游实现(核心编译链,优先) |
| 389 | crates/javac-core/src/comp/check.rs:1895 | 真实未实现(待核) | types.getRetention/overrideEquivalent) are explicit TODO registrations. | — | 高 | 按上游实现(核心编译链,优先) |
| 390 | crates/javac-core/src/comp/check.rs:3792 | 真实未实现(待核) | TODO(Check): checkDeprecated is not ported yet (Check.java:3520-3560, | Check.java:3520-3560 | 高 | 按上游实现(核心编译链,优先) |
| 391 | crates/javac-core/src/comp/deferred_attr.rs:8 | 纯文档说明 | here. NOT PORTED (registered): the DeferredAttr service and | — | 高 | 保留(移植说明) |
| 392 | crates/javac-core/src/comp/deferred_attr.rs:19 | 真实未实现(待核) | TODO. | — | 高 | 按上游实现(核心编译链,优先) |
| 393 | crates/javac-core/src/comp/deferred_attr.rs:939 | 真实未实现 | visitor (not ported as a trait); the port reproduces the visitor's dispatch | — | 高 | 按上游实现(核心编译链,优先) |
| 394 | crates/javac-core/src/comp/enter.rs:9 | 纯文档说明 | NOT PORTED IN THIS STEP (registered; no substitute): | — | 高 | 保留(移植说明) |
| 395 | crates/javac-core/src/comp/enter.rs:11 | 真实未实现(待核) | calls are TODO in the corresponding methods (Annotate); | — | 高 | 按上游实现(核心编译链,优先) |
| 396 | crates/javac-core/src/comp/enter.rs:13 | 真实未实现(待核) | ensureImportsChecked are TODO (TypeEnter/MemberEnter); visitClassDef | — | 高 | 按上游实现(核心编译链,优先) |
| 397 | crates/javac-core/src/comp/enter.rs:15 | 真实未实现(待核) | - Todo instance: todo.append of package/module envs is TODO (Todo); | — | 高 | 按上游实现(核心编译链,优先) |
| 398 | crates/javac-core/src/comp/enter.rs:17 | 真实未实现(待核) | usesProvidesCompleter and isInModuleGraph are TODO (Modules); | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 399 | crates/javac-core/src/comp/enter.rs:21 | 纯文档说明 | which are not ported yet: the field stays None (registered); | — | 高 | 保留(移植说明) |
| 400 | crates/javac-core/src/comp/enter.rs:22 | 真实未实现 | - Scope.NamedImportScope/StarImportScope are not ported (Scope): the | — | 高 | 按上游实现(核心编译链,优先) |
| 401 | crates/javac-core/src/comp/enter.rs:66 | 真实未实现(待核) | TODO(TreeMaker): TreeMaker.ClassDef/Modifiers factories are not | — | 高 | 按上游实现(核心编译链,优先) |
| 402 | crates/javac-core/src/comp/enter.rs:87 | 纯文档说明 | TypeEnter/Modules/Todo are not ported (TODO in the methods, registered). | — | 高 | 保留(移植说明) |
| 403 | crates/javac-core/src/comp/enter.rs:278 | 真实未实现(待核) | TODO(Annotate): annotate.blockAnnotations(); | — | 高 | 按上游实现(核心编译链,优先) |
| 404 | crates/javac-core/src/comp/enter.rs:295 | 真实未实现(待核) | TODO(Annotate): annotate.unblockAnnotations(); | — | 高 | 按上游实现(核心编译链,优先) |
| 405 | crates/javac-core/src/comp/enter.rs:362 | 真实未实现(待核) | TODO(Modules): tree.modle.usesProvidesCompleter = | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 406 | crates/javac-core/src/comp/enter.rs:394 | 真实未实现(待核) | TODO(Modules): dependencyWithPackage / PackageInOtherModule | — | 高 | 按上游实现(核心编译链,优先) |
| 407 | crates/javac-core/src/comp/enter.rs:654 | 真实未实现(待核) | TODO(Annotate): c.clearAnnotationMetadata(); | — | 高 | 按上游实现(核心编译链,优先) |
| 408 | crates/javac-core/src/comp/enter.rs:691 | 真实未实现(待核) | TODO(TypeEnter/MemberEnter): c.completer = typeEnter; | — | 高 | 按上游实现(核心编译链,优先) |
| 409 | crates/javac-core/src/comp/enter.rs:771 | 真实未实现(待核) | TODO(Modules): if (modules.isInModuleGraph(tree.sym)) | — | 高 | 按上游实现(核心编译链,优先) |
| 410 | crates/javac-core/src/comp/infer.rs:29 | 纯文档说明 | NOT PORTED IN THIS FILE (registered: dependency batches; no substitutes are | — | 高 | 保留(移植说明) |
| 411 | crates/javac-core/src/comp/infer.rs:30 | 真实未实现 | written, the upstream signatures depend on services that are not ported yet): | — | 高 | 按上游实现(核心编译链,优先) |
| 412 | crates/javac-core/src/comp/infer.rs:38 | 纯文档说明 | NOT PORTED (registered): | — | 高 | 保留(移植说明) |
| 413 | crates/javac-core/src/comp/infer.rs:1871 | 真实未实现(待核) | .expect("TODO(Types batch): lub needs the Names service"); | — | 高 | 按上游实现(核心编译链,优先) |
| 414 | crates/javac-core/src/comp/infer.rs:2790 | 真实未实现(待核) | TODO(Lint): the deferred solve callback does not propagate the | — | 高 | 按上游实现(核心编译链,优先) |
| 415 | crates/javac-core/src/comp/infer.rs:3329 | 真实未实现(待核) | TODO(Attr): the CompletionFailure channel of | — | 高 | 按上游实现(核心编译链,优先) |
| 416 | crates/javac-core/src/comp/member_enter.rs:9 | 纯文档说明 | NOT PORTED IN THIS STEP (registered; dependencies not ported yet, no | — | 高 | 保留(移植说明) |
| 417 | crates/javac-core/src/comp/member_enter.rs:13 | 真实未实现 | (signature/visitVarDef/checkReceiver), because the Attr is not ported; | — | 高 | 按上游实现(核心编译链,优先) |
| 418 | crates/javac-core/src/comp/member_enter.rs:15 | 真实未实现(待核) | annotateDefaultValueLater/unfinishedDefaultValue calls are TODO (Annotate | — | 高 | 按上游实现(核心编译链,优先) |
| 419 | crates/javac-core/src/comp/member_enter.rs:23 | 真实未实现 | - ArrayType.makeVarargs and VarSymbol.setLazyConstValue are not ported | — | 高 | 按上游实现(核心编译链,优先) |
| 420 | crates/javac-core/src/comp/member_enter.rs:25 | 真实未实现(待核) | constant-value computation are TODO in visitVarDef; | — | 高 | 按上游实现(核心编译链,优先) |
| 421 | crates/javac-core/src/comp/member_enter.rs:26 | 真实未实现(待核) | - getMethodEnv mEnv.info.lint.augment(sym) is TODO (Lint), the scope | — | 高 | 按上游实现(核心编译链,优先) |
| 422 | crates/javac-core/src/comp/member_enter.rs:32 | 纯文档说明 | registered separately (the Enter leaves the TypeEnter completer TODO). | — | 高 | 保留(移植说明) |
| 423 | crates/javac-core/src/comp/member_enter.rs:449 | 真实未实现(待核) | TODO(Attr): localEnv.info.returnResult = attr.new ResultInfo( | — | 高 | 按上游实现(核心编译链,优先) |
| 424 | crates/javac-core/src/comp/member_enter.rs:455 | 真实未实现(待核) | TODO(Attr): localEnv.info.yieldResult = null; | — | 高 | 按上游实现(核心编译链,优先) |
| 425 | crates/javac-core/src/comp/member_enter.rs:514 | 真实未实现(待核) | TODO(Types): replace the plain array type with | — | 高 | 按上游实现(核心编译链,优先) |
| 426 | crates/javac-core/src/comp/member_enter.rs:555 | 真实未实现(待核) | TODO(Symbol/Attr): v.setLazyConstValue(initEnv(tree, | — | 高 | 按上游实现(核心编译链,优先) |
| 427 | crates/javac-core/src/comp/member_enter.rs:557 | 真实未实现 | not ported and initEnv needs AttrContextEnv (Attr). | — | 高 | 按上游实现(核心编译链,优先) |
| 428 | crates/javac-core/src/comp/member_enter.rs:608 | 真实未实现(待核) | TODO(Attr): the upstream call sites are checkReceiver's | — | 高 | 按上游实现(核心编译链,优先) |
| 429 | crates/javac-core/src/comp/member_enter.rs:610 | 真实未实现(待核) | TODO until Attr is ported; the method is kept for that. | — | 高 | 按上游实现(核心编译链,优先) |
| 430 | crates/javac-core/src/comp/member_enter.rs:835 | 真实未实现(待核) | TODO(Lint): mEnv.info.lint = mEnv.info.lint.augment(tree.sym); | — | 高 | 按上游实现(核心编译链,优先) |
| 431 | crates/javac-core/src/comp/resolve.rs:40 | 纯文档说明 | NOT YET PORTED IN THIS FILE (registered; each with a not-yet-ported marker + TODO): | — | 高 | 保留(移植说明) |
| 432 | crates/javac-core/src/comp/resolve.rs:117 | 真实未实现 | flags (not ported yet). | — | 高 | 按上游实现(核心编译链,优先) |
| 433 | crates/javac-core/src/comp/resolve.rs:346 | 真实未实现 | package owner may be unset (Modules not ported), in which case the | — | 高 | 按上游实现(核心编译链,优先) |
| 434 | crates/javac-core/src/comp/resolve.rs:1004 | 真实未实现(待核) | TODO(Resolve diagnostics detail): the upstream creates a | — | 高 | 按上游实现(核心编译链,优先) |
| 435 | crates/javac-core/src/comp/resolve.rs:1546 | 真实未实现(待核) | upstream body; unimplemented dependencies are explicit TODO registrations. | — | 高 | 按上游实现(核心编译链,优先) |
| 436 | crates/javac-core/src/comp/resolve.rs:2304 | 真实未实现(待核) | TODO(ClassFinder): the BadClassFile distinction is not | — | 高 | 按上游实现(核心编译链,优先) |
| 437 | crates/javac-core/src/comp/resolve.rs:3934 | 真实未实现(待核) | TODO(Check diagnostics , ADR 0002 P4): the upstream reportMC builds | — | 高 | 按上游实现(核心编译链,优先) |
| 438 | crates/javac-core/src/comp/resolve.rs:4012 | 真实未实现(待核) | TODO(Lint/Check): the upstream returns the shared rsWarner; the | — | 高 | 按上游实现(核心编译链,优先) |
| 439 | crates/javac-core/src/comp/resolve.rs:4378 | 真实未实现(待核) | TODO(Attr): the CompletionFailure channel of | — | 高 | 按上游实现(核心编译链,优先) |
| 440 | crates/javac-core/src/comp/resolve.rs:5052 | 真实未实现(待核) | TODO(Scope/Resolve): upstream getSymbolsByName applies the | — | 高 | 按上游实现(核心编译链,优先) |
| 441 | crates/javac-core/src/comp/resolve.rs:6195 | 真实未实现(待核) | TODO(Check): FindConstructor also runs | — | 高 | 按上游实现(核心编译链,优先) |
| 442 | crates/javac-core/src/comp/resolve.rs:9322 | 测试或工具 | Java: currModule.complete() — TODO(Completer): the | — | 高·核心缺口 | 不改(测试/工具) |
| 443 | crates/javac-core/src/comp/resolve.rs:9323 | 测试或工具 | completer chain is not ported; entered modules are complete. | — | 高 | 不改(测试/工具) |
| 444 | crates/javac-core/src/comp/resolve.rs:9399 | 测试或工具 | TODO(Types): t.isUnion() / UnionClassType alternative types. | — | 高 | 不改(测试/工具) |
| 445 | crates/javac-core/src/comp/type_enter.rs:1 | 纯文档说明 | #![allow(dead_code)] // TODO(Attr/TreeMaker/Check batches): helpers waiting for their batches (registered in the file he | — | 中 | 保留(移植说明) |
| 446 | crates/javac-core/src/comp/type_enter.rs:21 | 纯文档说明 | NOT PORTED IN THIS FILE (registered; dependencies not ported yet, no | — | 中 | 保留(移植说明) |
| 447 | crates/javac-core/src/comp/type_enter.rs:27 | 真实未实现(待核) | typeAnnotations.validate*, todo.append, preview checks -> TODO comments | — | 中 | 按上游实现 |
| 448 | crates/javac-core/src/comp/type_enter.rs:30 | 纯文档说明 | VarDef/Return/Block/Type/Select/Ident/Assign...) are not ported (registered | — | 中 | 保留(移植说明) |
| 449 | crates/javac-core/src/comp/type_enter.rs:36 | 真实未实现 | - Lint: Lint.suppress is not ported (Lint) -> unimplemented helper; | — | 中 | 按上游实现 |
| 450 | crates/javac-core/src/comp/type_enter.rs:236 | 真实未实现(待核) | TODO(TypeEnter/Completer): the completer registration is an | — | 中 | 按上游实现 |
| 451 | crates/javac-core/src/comp/type_enter.rs:374 | 真实未实现(待核) | TODO(Lint): Lint prevLint = chk.setLint(lint); | — | 中 | 按上游实现 |
| 452 | crates/javac-core/src/comp/type_enter.rs:468 | 真实未实现(待核) | TODO(Lint): chk.setLint(prevLint); | — | 中 | 按上游实现 |
| 453 | crates/javac-core/src/comp/type_enter.rs:502 | 真实未实现(待核) | TODO(Modules): syms.lookupPackage(toplevel(&env).modle, | — | 中 | 按上游实现 |
| 454 | crates/javac-core/src/comp/type_enter.rs:573 | 真实未实现(待核) | TODO(Check): chk.checkCanonical(imp); | — | 中 | 按上游实现 |
| 455 | crates/javac-core/src/comp/type_enter.rs:1004 | 真实未实现(待核) | TODO(Synthesizer): the local ErrorType modelType hook and the | — | 中 | 按上游实现 |
| 456 | crates/javac-core/src/comp/type_enter.rs:1005 | 纯文档说明 | Synthesizer visitor are not ported; registered. | — | 中 | 保留(移植说明) |
| 457 | crates/javac-core/src/compiler_context.rs:393 | 真实未实现(待核) | TODO for the Scope query). | — | 中 | 按上游实现 |
| 458 | crates/javac-core/src/file/base_file_manager.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 459 | crates/javac-core/src/file/base_file_manager.rs:5 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: run (BaseFileManager.java:148) | BaseFileManager.java:148 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 460 | crates/javac-core/src/file/base_file_manager.rs:6 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: handleFileManagerOption (BaseFileManager.java:230) | BaseFileManager.java:230 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 461 | crates/javac-core/src/file/base_file_manager.rs:7 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getValue (BaseFileManager.java:484) | BaseFileManager.java:484 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 462 | crates/javac-core/src/file/base_file_manager.rs:30 | 真实未实现(待核) | TODO (the Log error entry point and a streaming decoder are not available). | — | 中 | 按上游实现 |
| 463 | crates/javac-core/src/file/base_file_manager.rs:32 | 真实未实现(待核) | provide yet (TODO, reported). | — | 中 | 按上游实现 |
| 464 | crates/javac-core/src/file/base_file_manager.rs:37 | 真实未实现 | java.lang.ref.SoftReference (content cache) are not ported: getClassLoader is | — | 中 | 按上游实现 |
| 465 | crates/javac-core/src/file/base_file_manager.rs:38 | 真实未实现(待核) | unimplemented + TODO; the content cache keeps the decoded JavaString directly | — | 中 | 按上游实现 |
| 466 | crates/javac-core/src/file/base_file_manager.rs:44 | 真实未实现(待核) | * TODO(main/OptionHelper): handleOption(String, Iterator) needs Option.process and | — | 中 | 按上游实现 |
| 467 | crates/javac-core/src/file/base_file_manager.rs:45 | 真实未实现 | main.OptionHelper (not ported); handleOptions needs Log.error and the Errors | — | 中 | 按上游实现 |
| 468 | crates/javac-core/src/file/base_file_manager.rs:82 | 真实未实现(依赖批次) | TODO(S3b): set by setContext once Locations.update is ported. | — | 中 | 依赖批次接线(按上游) |
| 469 | crates/javac-core/src/file/base_file_manager.rs:121 | 真实未实现(依赖批次) | TODO(main option restore): set by the MULTIRELEASE branch of handleOption(Option,String). | — | 中 | 依赖批次接线(按上游) |
| 470 | crates/javac-core/src/file/base_file_manager.rs:165 | 真实未实现(依赖批次) | TODO(S3b): the body needs Locations.update(log, warn, FSInfo.instance(context)) and | — | 中 | 依赖批次接线(按上游) |
| 471 | crates/javac-core/src/file/base_file_manager.rs:166 | 真实未实现 | the FSInfo Context registration (not ported); no substitute state is set. | — | 中 | 按上游实现 |
| 472 | crates/javac-core/src/file/base_file_manager.rs:170 | 真实未实现 | Java: FSInfo.instance(context); the FSInfo Context factory is not ported, so the | — | 中 | 按上游实现 |
| 473 | crates/javac-core/src/file/base_file_manager.rs:197 | 真实未实现(依赖批次) | Not wired yet (TODO(file manager wiring)). | — | 中 | 依赖批次接线(按上游) |
| 474 | crates/javac-core/src/file/base_file_manager.rs:268 | 真实未实现(待核) | TODO(P6): java.net.URL/URLClassLoader and the reflective custom loader lookup are not | — | 中 | 按上游实现 |
| 475 | crates/javac-core/src/file/cache_fs_info.rs:17 | 真实未实现(待核) | * not ported (TODO(P2), reported): preRegister(Context) (Context factory for | — | 中 | 按上游实现 |
| 476 | crates/javac-core/src/file/cache_fs_info.rs:19 | 真实未实现 | and super.getJarClassPath (java.util.jar.JarFile/Manifest are not ported). | — | 中 | 按上游实现 |
| 477 | crates/javac-core/src/file/fs_info.rs:13 | 真实未实现(待核) | * not ported dependencies (TODO(P2), reported): java.util.jar.JarFile/ | — | 中 | 按上游实现 |
| 478 | crates/javac-core/src/file/fs_info.rs:28 | 纯文档说明 | PORTING_RULES: java.nio.file.attribute is not ported; this struct carries | — | 中 | 保留(移植说明) |
| 479 | crates/javac-core/src/file/fs_info.rs:108 | 真实未实现 | java.net.URL are not ported as public types; the jar manifest main | — | 中 | 按上游实现 |
| 480 | crates/javac-core/src/file/fs_info.rs:147 | 真实未实现(待核) | TODO(P2): the upstream getJarFSProvider() (FileSystemProvider lookup) and | — | 中 | 按上游实现 |
| 481 | crates/javac-core/src/file/fs_info.rs:148 | 真实未实现 | the static tryResolveFile(URL, String) are not ported: their java.nio.file | — | 中 | 按上游实现 |
| 482 | crates/javac-core/src/file/javac_file_manager.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 483 | crates/javac-core/src/file/javac_file_manager.rs:5 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: preRegister (JavacFileManager.java:150) | JavacFileManager.java:150 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 484 | crates/javac-core/src/file/javac_file_manager.rs:6 | 真实未实现 | - NOT PORTED: isValidName (JavacFileManager.java:235) | JavacFileManager.java:235 | 中 | 按上游实现 |
| 485 | crates/javac-core/src/file/javac_file_manager.rs:7 | 真实未实现 | - NOT PORTED: validateClassName (JavacFileManager.java:248) | JavacFileManager.java:248 | 中 | 按上游实现 |
| 486 | crates/javac-core/src/file/javac_file_manager.rs:8 | 真实未实现 | - NOT PORTED: validatePackageName (JavacFileManager.java:253) | JavacFileManager.java:253 | 中 | 按上游实现 |
| 487 | crates/javac-core/src/file/javac_file_manager.rs:9 | 真实未实现 | - NOT PORTED: testName (JavacFileManager.java:258) | JavacFileManager.java:258 | 中 | 按上游实现 |
| 488 | crates/javac-core/src/file/javac_file_manager.rs:10 | 真实未实现 | - NOT PORTED: printAscii (JavacFileManager.java:284) | JavacFileManager.java:284 | 中 | 按上游实现 |
| 489 | crates/javac-core/src/file/javac_file_manager.rs:11 | 真实未实现 | - NOT PORTED: preVisitDirectory (JavacFileManager.java:581) | JavacFileManager.java:581 | 中 | 按上游实现 |
| 490 | crates/javac-core/src/file/javac_file_manager.rs:13 | 真实未实现 | - NOT PORTED: visitFile (JavacFileManager.java:621) | JavacFileManager.java:621 | 中 | 按上游实现 |
| 491 | crates/javac-core/src/file/javac_file_manager.rs:14 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: hasExplicitLocation (JavacFileManager.java:819) | JavacFileManager.java:819 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 492 | crates/javac-core/src/file/javac_file_manager.rs:15 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: setLocationFromPaths (JavacFileManager.java:999) | JavacFileManager.java:999 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 493 | crates/javac-core/src/file/javac_file_manager.rs:16 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: compareTo (JavacFileManager.java:1101) | JavacFileManager.java:1101 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 494 | crates/javac-core/src/file/javac_file_manager.rs:17 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: hashCode (JavacFileManager.java:1114) | JavacFileManager.java:1114 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 495 | crates/javac-core/src/file/javac_file_manager.rs:18 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getServiceLoader (JavacFileManager.java:1145) | JavacFileManager.java:1145 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 496 | crates/javac-core/src/file/javac_file_manager.rs:19 | 真实未实现 | - NOT PORTED: getRelativeName (JavacFileManager.java:1240) | JavacFileManager.java:1240 | 中 | 按上游实现 |
| 497 | crates/javac-core/src/file/javac_file_manager.rs:20 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getMessage (JavacFileManager.java:1258) | JavacFileManager.java:1258 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 498 | crates/javac-core/src/file/javac_file_manager.rs:21 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: hasNext (JavacFileManager.java:1300) | JavacFileManager.java:1300 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 499 | crates/javac-core/src/file/javac_file_manager.rs:22 | 真实未实现 | - NOT PORTED: asFiles (JavacFileManager.java:1311) | JavacFileManager.java:1311 | 中 | 按上游实现 |
| 500 | crates/javac-core/src/file/javac_file_manager.rs:46 | 真实未实现 | * Context-based service lookup is not ported: Java new JavacFileManager(Context, register, | — | 中 | 按上游实现 |
| 501 | crates/javac-core/src/file/javac_file_manager.rs:54 | 真实未实现(待核) | (java.nio zip/jar FileSystem) is the second S3c (TODO in getContainer). | — | 中 | 按上游实现 |
| 502 | crates/javac-core/src/file/javac_file_manager.rs:62 | 真实未实现(依赖批次) | the Locations-side PathFactory (Java locations.setPathFactory) is TODO(S3b follow-up). | — | 中 | 依赖批次接线(按上游) |
| 503 | crates/javac-core/src/file/javac_file_manager.rs:67 | 真实未实现(待核) | * known gaps: getClassLoader (URLClassLoader/reflection, TODO(P6)); handleOption(String, | — | 中 | 按上游实现 |
| 504 | crates/javac-core/src/file/javac_file_manager.rs:68 | 真实未实现(待核) | Iterator) (OptionHelper not ported, TODO(OptionHelper)); ServiceLoader; close's | — | 中 | 按上游实现 |
| 505 | crates/javac-core/src/file/javac_file_manager.rs:70 | 真实未实现(依赖批次) | (second).  Java warnings are not emitted (TODO(Log)). | — | 中 | 依赖批次接线(按上游) |
| 506 | crates/javac-core/src/file/javac_file_manager.rs:969 | 真实未实现(待核) | depend on the source level; TODO(P6) for the exact SourceVersion port). | — | 中 | 按上游实现 |
| 507 | crates/javac-core/src/file/javac_file_manager.rs:1479 | 真实未实现(依赖批次) | Java: locations.setPathFactory(f); the Locations-side PathFactory is TODO(S3b | — | 中 | 依赖批次接线(按上游) |
| 508 | crates/javac-core/src/file/javac_file_manager.rs:1552 | 真实未实现 | java.net.URI parsing is not ported; this covers the upstream rejection conditions | — | 中 | 按上游实现 |
| 509 | crates/javac-core/src/file/javac_file_manager.rs:1793 | 测试或工具 | on the Locations module-path mapping, which is not ported yet.) | — | 中 | 不改(测试/工具) |
| 510 | crates/javac-core/src/file/jrt_index.rs:26 | 真实未实现 | java.util.ResourceBundle is not ported: getCtInfo returns CtSym::empty() (the upstream | — | 中 | 按上游实现 |
| 511 | crates/javac-core/src/file/jrt_index.rs:27 | 真实未实现(待核) | missing-key result) for every package until the resource lands (TODO resources, | — | 中 | 按上游实现 |
| 512 | crates/javac-core/src/file/jrt_index.rs:96 | 真实未实现(依赖批次) | TODO(S3c): get_file is used by JavacFileManager once that lands. | — | 中 | 依赖批次接线(按上游) |
| 513 | crates/javac-core/src/file/jrt_index.rs:337 | 真实未实现(待核) | TODO(resources): the upstream reads the ResourceBundle | — | 中 | 按上游实现 |
| 514 | crates/javac-core/src/file/jrt_index.rs:339 | 真实未实现 | resource is not generated by xtask yet and java.util.ResourceBundle is not ported, so | — | 中 | 按上游实现 |
| 515 | crates/javac-core/src/file/locations.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 高 | 按上游实现(核心编译链,优先) |
| 516 | crates/javac-core/src/file/locations.rs:5 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: setPathFactory (Locations.java:179) | Locations.java:179 | 高·核心缺口 | 更新/删除注释(本地已实现,需人工确认) |
| 517 | crates/javac-core/src/file/locations.rs:6 | 真实未实现 | - NOT PORTED: addDirectories (Locations.java:300) | Locations.java:300 | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 518 | crates/javac-core/src/file/locations.rs:8 | 真实未实现 | - NOT PORTED: addDirectory (Locations.java:319) | Locations.java:319 | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 519 | crates/javac-core/src/file/locations.rs:9 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isSet (Locations.java:457) | Locations.java:457 | 高·核心缺口 | 更新/删除注释(本地已实现,需人工确认) |
| 520 | crates/javac-core/src/file/locations.rs:10 | 真实未实现 | - NOT PORTED: setPaths (Locations.java:471) | Locations.java:471 | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 521 | crates/javac-core/src/file/locations.rs:11 | 真实未实现 | - NOT PORTED: setPathsForModule (Locations.java:476) | Locations.java:476 | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 522 | crates/javac-core/src/file/locations.rs:16 | 真实未实现 | - NOT PORTED: computePath (Locations.java:733) | Locations.java:733 | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 523 | crates/javac-core/src/file/locations.rs:17 | 真实未实现 | - NOT PORTED: createPath (Locations.java:737) | Locations.java:737 | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 524 | crates/javac-core/src/file/locations.rs:20 | 真实未实现 | - NOT PORTED: canonicalize (Locations.java:854) | Locations.java:854 | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 525 | crates/javac-core/src/file/locations.rs:27 | 真实未实现 | - NOT PORTED: checkPaths (Locations.java:1217) | Locations.java:1217 | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 526 | crates/javac-core/src/file/locations.rs:28 | 真实未实现 | - NOT PORTED: initModuleLocations (Locations.java:1226) | Locations.java:1226 | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 527 | crates/javac-core/src/file/locations.rs:29 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: hasNext (Locations.java:1273) | Locations.java:1273 | 高·核心缺口 | 更新/删除注释(本地已实现,需人工确认) |
| 528 | crates/javac-core/src/file/locations.rs:30 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: next (Locations.java:1293) | Locations.java:1293 | 高·核心缺口 | 更新/删除注释(本地已实现,需人工确认) |
| 529 | crates/javac-core/src/file/locations.rs:31 | 真实未实现 | - NOT PORTED: scanDirectory (Locations.java:1303) | Locations.java:1303 | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 530 | crates/javac-core/src/file/locations.rs:32 | 真实未实现 | - NOT PORTED: scanFile (Locations.java:1361) | Locations.java:1361 | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 531 | crates/javac-core/src/file/locations.rs:33 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: readModuleName (Locations.java:1498) | Locations.java:1498 | 高·核心缺口 | 更新/删除注释(本地已实现,需人工确认) |
| 532 | crates/javac-core/src/file/locations.rs:34 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isModuleName (Locations.java:1509) | Locations.java:1509 | 高·核心缺口 | 更新/删除注释(本地已实现,需人工确认) |
| 533 | crates/javac-core/src/file/locations.rs:35 | 真实未实现 | - NOT PORTED: initFromPattern (Locations.java:1594) | Locations.java:1594 | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 534 | crates/javac-core/src/file/locations.rs:36 | 真实未实现 | - NOT PORTED: initModuleTable (Locations.java:1637) | Locations.java:1637 | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 535 | crates/javac-core/src/file/locations.rs:37 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: isSeparator (Locations.java:1654) | Locations.java:1654 | 高·核心缺口 | 更新/删除注释(本地已实现,需人工确认) |
| 536 | crates/javac-core/src/file/locations.rs:38 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: expandBraces (Locations.java:1686) | Locations.java:1686 | 高·核心缺口 | 更新/删除注释(本地已实现,需人工确认) |
| 537 | crates/javac-core/src/file/locations.rs:39 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getMatchingBrace (Locations.java:1731) | Locations.java:1731 | 高·核心缺口 | 更新/删除注释(本地已实现,需人工确认) |
| 538 | crates/javac-core/src/file/locations.rs:47 | 真实未实现 | - NOT PORTED: isCurrentPlatform (Locations.java:1916) | Locations.java:1916 | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 539 | crates/javac-core/src/file/locations.rs:79 | 真实未实现(待核) | * reported gaps (TODO, no substitute data): | — | 高 | 按上游实现(核心编译链,优先) |
| 540 | crates/javac-core/src/file/locations.rs:80 | 真实未实现(待核) | - jar Class-Path expansion (java.util.jar Manifest; FSInfo.getJarClassPath TODO(P2)): | — | 高 | 按上游实现(核心编译链,优先) |
| 541 | crates/javac-core/src/file/locations.rs:82 | 真实未实现(依赖批次) | - Log.warning diagnostics for path problems (TODO(Log)): invalid/nonexistent | — | 高 | 依赖批次接线(按上游) |
| 542 | crates/javac-core/src/file/locations.rs:85 | 真实未实现(依赖批次) | probe (java.nio FileSystem); TODO(S3c); | — | 高·核心缺口 | 依赖批次接线(按上游) |
| 543 | crates/javac-core/src/file/locations.rs:88 | 真实未实现(待核) | MODULE_PATH/UPGRADE_MODULE_PATH are TODO panics; the path bookkeeping is ported; | — | 高 | 按上游实现(核心编译链,优先) |
| 544 | crates/javac-core/src/file/locations.rs:92 | 真实未实现(依赖批次) | argument reporting is TODO(Log)); | — | 高 | 依赖批次接线(按上游) |
| 545 | crates/javac-core/src/file/locations.rs:97 | 真实未实现(依赖批次) | * TODO(S3c): JavacFileManager consumes this API.  Handler locations are looked up by | — | 高·核心缺口 | 依赖批次接线(按上游) |
| 546 | crates/javac-core/src/file/locations.rs:182 | 真实未实现(依赖批次) | Java: warning(PathElementNotFound) — TODO(Log): no Log warning entry point. | — | 高 | 依赖批次接线(按上游) |
| 547 | crates/javac-core/src/file/locations.rs:202 | 真实未实现(依赖批次) | FileSystem does not exist in the port: skip (TODO(S3c); the upstream | — | 高·核心缺口 | 依赖批次接线(按上游) |
| 548 | crates/javac-core/src/file/locations.rs:203 | 真实未实现(依赖批次) | warning is a TODO(Log) diagnostic). | — | 高 | 依赖批次接线(按上游) |
| 549 | crates/javac-core/src/file/locations.rs:209 | 真实未实现(待核) | ported (FSInfo.getJarClassPath TODO(P2)), so the Class-Path attribute is not expanded. | — | 高 | 按上游实现(核心编译链,优先) |
| 550 | crates/javac-core/src/file/locations.rs:257 | 真实未实现(依赖批次) | Java: the enclosing-jar case needs a java.nio jar file system (TODO(S3c)). | — | 高·核心缺口 | 依赖批次接线(按上游) |
| 551 | crates/javac-core/src/file/locations.rs:749 | 真实未实现(待核) | not available without a JVM (the launcher provides them, TODO(P6)); the | — | 高 | 按上游实现(核心编译链,优先) |
| 552 | crates/javac-core/src/file/locations.rs:943 | 真实未实现(依赖批次) | path warnings are registered as TODO(Log) in this file. | — | 高 | 依赖批次接线(按上游) |
| 553 | crates/javac-core/src/file/locations.rs:1538 | 真实未实现(依赖批次) | TODO(S3c): the by-path lookup (ModuleTable.get(Path) with parent walking) is used by | — | 高·核心缺口 | 依赖批次接线(按上游) |
| 554 | crates/javac-core/src/file/mod.rs:11 | 真实未实现(待核) | charset/URL/thread-dependent methods are TODO and reported) and the Locations | — | 中 | 按上游实现 |
| 555 | crates/javac-core/src/file/mod.rs:13 | 真实未实现 | * not ported yet (reported to the Lead): Locations handlers (S3b), JavacFileManager | — | 中 | 按上游实现 |
| 556 | crates/javac-core/src/file/mod.rs:16 | 真实未实现(待核) | * TODO(P2): the deferred methods listed in the individual file headers. | — | 中 | 按上游实现 |
| 557 | crates/javac-core/src/file/path_file_object.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 558 | crates/javac-core/src/file/path_file_object.rs:5 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: hashCode (PathFileObject.java:539) | PathFileObject.java:539 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 559 | crates/javac-core/src/file/path_file_object.rs:26 | 真实未实现(待核) | * deferred (TODO(P2), reported): | — | 中 | 按上游实现 |
| 560 | crates/javac-core/src/file/path_file_object.rs:28 | 真实未实现 | BaseFileManager decoder/content cache (not ported); | — | 中 | 按上游实现 |
| 561 | crates/javac-core/src/file/path_file_object.rs:339 | 真实未实现(待核) | TODO(P2): the macOS Normalizer.isNormalized branch is not ported. | — | 中 | 按上游实现 |
| 562 | crates/javac-core/src/file/path_file_object.rs:379 | 真实未实现(待核) | logic (BaseFileManager itself is not ported; TODO(P2): call the ported | — | 中 | 按上游实现 |
| 563 | crates/javac-core/src/file/path_file_object.rs:639 | 纯文档说明 | PORTING_RULES adapter: java.net.URI is not ported; this implements the | — | 中 | 保留(移植说明) |
| 564 | crates/javac-core/src/file/relative_path.rs:17 | 真实未实现 | Not ported dependencies: | — | 中 | 按上游实现 |
| 565 | crates/javac-core/src/file/relative_path.rs:18 | 真实未实现(待核) | * java.nio.file.FileSystem (resolveAgainst(FileSystem)) -> TODO(P2); the resolveAgainst(Path) overload is ported on top  | — | 中 | 按上游实现 |
| 566 | crates/javac-core/src/file/relative_path.rs:19 | 真实未实现(待核) | * java.util.zip.ZipFile/ZipEntry (RelativeFile.getZipEntry) -> TODO(P2); | — | 中 | 按上游实现 |
| 567 | crates/javac-core/src/file/relative_path.rs:86 | 真实未实现(待核) | Java: public Path resolveAgainst(FileSystem fs) — TODO(P2): java.nio.file | — | 中 | 按上游实现 |
| 568 | crates/javac-core/src/file/relative_path.rs:87 | 真实未实现 | FileSystem is not ported (the P2 file layer has no FileSystem object). | — | 中 | 按上游实现 |
| 569 | crates/javac-core/src/file/relative_path.rs:90 | 纯文档说明 | PORTING_RULES: java.nio.file.FileSystem is not ported; the caller supplies | — | 中 | 保留(移植说明) |
| 570 | crates/javac-core/src/file/relative_path.rs:256 | 真实未实现(待核) | Java: ZipEntry getZipEntry(ZipFile zip) — TODO(P2): java.util.zip.ZipFile/ | — | 中 | 按上游实现 |
| 571 | crates/javac-core/src/file/relative_path.rs:257 | 真实未实现 | ZipEntry are not ported (javac-zip works on ZipArchive/entry names). | — | 中 | 按上游实现 |
| 572 | crates/javac-core/src/file/relative_path.rs:260 | 纯文档说明 | PORTING_RULES: java.util.zip.ZipEntry is not ported; the archive entry lookup | — | 中 | 保留(移植说明) |
| 573 | crates/javac-core/src/jvm/class_file.rs:148 | 真实未实现(待核) | TODO(P0+): internalize(Name) / externalize(Name) overloads are added together with | — | 中 | 按上游实现 |
| 574 | crates/javac-core/src/jvm/class_reader.rs:8 | 真实未实现(待核) | (readCode structure; Code object attach TODO), ConstantValue, Deprecated, | — | 高 | 按上游实现(核心编译链,优先) |
| 575 | crates/javac-core/src/jvm/class_reader.rs:24 | 纯文档说明 | Annotate (P3), which is not ported yet: attach path TODO (registered). | — | 高 | 保留(移植说明) |
| 576 | crates/javac-core/src/jvm/class_reader.rs:26 | 真实未实现 | Still not ported upstream methods (no substitutes in this file): | — | 高 | 按上游实现(核心编译链,优先) |
| 577 | crates/javac-core/src/jvm/class_reader.rs:27 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: instance (ClassReader.java:257) | ClassReader.java:257 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 578 | crates/javac-core/src/jvm/class_reader.rs:28 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getEnclosingType (ClassReader.java:575) | ClassReader.java:575 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 579 | crates/javac-core/src/jvm/class_reader.rs:29 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: setEnclosingType (ClassReader.java:600) | ClassReader.java:600 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 580 | crates/javac-core/src/jvm/class_reader.rs:30 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: setFlagIfAttributeTrue (ClassReader.java:1479) — Annotate | ClassReader.java:1479 | 高 | 更新/删除注释(本地已实现,需人工确认) |
| 581 | crates/javac-core/src/jvm/class_reader.rs:31 | 真实未实现 | - NOT PORTED: ProxyType (ClassReader.java:1840-1895) — module annotation | ClassReader.java:1840-1895 | 高 | 按上游实现(核心编译链,优先) |
| 582 | crates/javac-core/src/jvm/class_reader.rs:32 | 真实未实现 | - NOT PORTED: AttributeReader.accept/toString of proxy classes (1834-1878) — Annotate | — | 高 | 按上游实现(核心编译链,优先) |
| 583 | crates/javac-core/src/jvm/class_reader.rs:33 | 真实未实现 | - NOT PORTED: visitConstant/visitClass/visitEnum/visitCompound/visitArray/ | — | 高 | 按上游实现(核心编译链,优先) |
| 584 | crates/javac-core/src/jvm/class_reader.rs:36 | 真实未实现 | - NOT PORTED: AnnotationCompleter/AnnotationDefaultCompleter/TypeAnnotationCompleter/ | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 585 | crates/javac-core/src/jvm/class_reader.rs:38 | 真实未实现 | - NOT PORTED: addTypeAnnotationsToSymbol/visitClassSymbol/visitMethodSymbol/ | — | 高 | 按上游实现(核心编译链,优先) |
| 586 | crates/javac-core/src/jvm/class_reader.rs:42 | 真实未实现 | - NOT PORTED: adjustMethodParams (2588) — readMethod this$n stripping | — | 高 | 按上游实现(核心编译链,优先) |
| 587 | crates/javac-core/src/jvm/class_reader.rs:43 | 真实未实现 | - NOT PORTED: parameter (2707) — initParameterNames | — | 高 | 按上游实现(核心编译链,优先) |
| 588 | crates/javac-core/src/jvm/class_reader.rs:44 | 真实未实现 | - NOT PORTED: AnnotationTypeMetadata/CompleterDeproxy (annotation) | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 589 | crates/javac-core/src/jvm/class_reader.rs:45 | 真实未实现 | - NOT PORTED: UsesProvidesCompleter class (3173) — result-equivalent tail path | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 590 | crates/javac-core/src/jvm/class_reader.rs:47 | 真实未实现 | - NOT PORTED: ProxyType.getTag/cloneWithMetadata/resolve/toString (3209-3223) | — | 高 | 按上游实现(核心编译链,优先) |
| 591 | crates/javac-core/src/jvm/class_reader.rs:48 | 真实未实现 | - NOT PORTED: PackageInfoCompleter.complete (3261) | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 592 | crates/javac-core/src/jvm/class_reader.rs:49 | 真实未实现 | - NOT PORTED: SourceFileObject.equals/hashCode (3146/3154: value equality; | — | 高 | 按上游实现(核心编译链,优先) |
| 593 | crates/javac-core/src/jvm/class_reader.rs:55 | 真实未实现 | kept in ClassReader pending_* fields (Symbol fields/Annotate not ported); | — | 高 | 按上游实现(核心编译链,优先) |
| 594 | crates/javac-core/src/jvm/class_reader.rs:75 | 真实未实现(待核) | * NOT ported yet (not-yet-ported marker with TODO below): readClass and the | — | 高 | 按上游实现(核心编译链,优先) |
| 595 | crates/javac-core/src/jvm/class_reader.rs:78 | 真实未实现(待核) | These methods carry explicit TODO markers; no substitutes are provided. | — | 高 | 按上游实现(核心编译链,优先) |
| 596 | crates/javac-core/src/jvm/class_reader.rs:150 | 纯文档说明 | PORTING_RULES: Scope.WriteableScope is not ported yet (TODO(P3 scope | — | 高 | 保留(移植说明) |
| 597 | crates/javac-core/src/jvm/class_reader.rs:211 | 纯文档说明 | is not ported yet, so the read values are kept here, registered).", | — | 高 | 保留(移植说明) |
| 598 | crates/javac-core/src/jvm/class_reader.rs:268 | 纯文档说明 | Annotate/fileManager/Lint are not ported yet (S3 wiring, registered). | — | 高 | 保留(移植说明) |
| 599 | crates/javac-core/src/jvm/class_reader.rs:290 | 纯文档说明 | Lint is not ported yet (TODO(Lint), registered); false keeps | — | 高 | 保留(移植说明) |
| 600 | crates/javac-core/src/jvm/class_reader.rs:716 | 真实未实现(待核) | constant here; Types.noType is not represented in Symtab yet (TODO P3), | — | 高 | 按上游实现(核心编译链,优先) |
| 601 | crates/javac-core/src/jvm/class_reader.rs:1130 | 真实未实现 | Java: target = null; repeatable = null; (S3 annotation proxies, not ported) | — | 高 | 按上游实现(核心编译链,优先) |
| 602 | crates/javac-core/src/jvm/class_reader.rs:2600 | 真实未实现(待核) | TODO(Annotate): attachTypeAnnotations(sym, proxies) | — | 高 | 按上游实现(核心编译链,优先) |
| 603 | crates/javac-core/src/jvm/class_reader.rs:2601 | 真实未实现 | (TypeAnnotationCompleter is not ported yet); the read proxies are kept. | — | 高·核心缺口 | 按上游实现(核心编译链,优先) |
| 604 | crates/javac-core/src/jvm/class_reader.rs:2781 | 纯文档说明 | mapping is a registered TODO for the hardening). | — | 高 | 保留(移植说明) |
| 605 | crates/javac-core/src/jvm/class_reader.rs:3176 | 真实未实现(待核) | TODO(P6/Code): jvm.Code is not ported, so the structure is read | — | 高 | 按上游实现(核心编译链,优先) |
| 606 | crates/javac-core/src/jvm/class_reader.rs:3556 | 真实未实现(待核) | TODO(P2/S3): parameterAnnotations/parameterNameIndices/parameterAccessFlags | — | 高 | 按上游实现(核心编译链,优先) |
| 607 | crates/javac-core/src/jvm/class_reader.rs:3557 | 真实未实现 | fields are not ported yet; parameterNames are synthesized exactly like | — | 高 | 按上游实现(核心编译链,优先) |
| 608 | crates/javac-core/src/jvm/class_reader.rs:3688 | 纯文档说明 | Annotate (not ported yet); they are kept as read values (registered). | — | 高 | 保留(移植说明) |
| 609 | crates/javac-core/src/jvm/codegen.rs:4114 | 真实未实现 | The method symbol code handle is not ported; the generated code | — | 中 | 按上游实现 |
| 610 | crates/javac-core/src/jvm/module_name_reader.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 611 | crates/javac-core/src/jvm/module_name_reader.rs:5 | 真实未实现 | - NOT PORTED: utf8Mapper (ModuleNameReader.java:149) | ModuleNameReader.java:149 | 中 | 按上游实现 |
| 612 | crates/javac-core/src/jvm/module_name_reader.rs:21 | 真实未实现(待核) | * NOT ported yet (TODO, no substitute semantics): | — | 中 | 按上游实现 |
| 613 | crates/javac-core/src/jvm/pool_reader.rs:15 | 真实未实现(待核) | * NOT ported yet (TODO, no substitute semantics): | — | 中 | 按上游实现 |
| 614 | crates/javac-core/src/jvm/pool_reader.rs:18 | 真实未实现 | ClassSymbol/ModuleSymbol/PackageSymbol/Type/NameAndType) are not ported; | — | 中 | 按上游实现 |
| 615 | crates/javac-core/src/jvm/pool_reader.rs:39 | 真实未实现(待核) | (getClass/getModule/getPackage/getType/getNameAndType are still TODO; the raw | — | 中 | 按上游实现 |
| 616 | crates/javac-core/src/jvm/pool_reader.rs:62 | 纯文档说明 | are not ported yet; this adapter carries the raw value (see the file header | — | 中 | 保留(移植说明) |
| 617 | crates/javac-core/src/jvm/pool_reader.rs:73 | 真实未实现 | (the Type produced by sigToType is not ported). | — | 中 | 按上游实现 |
| 618 | crates/javac-core/src/jvm/pool_reader.rs:95 | 纯文档说明 | PORTING_RULES: TODO(P2): ClassReader.badClassFile throws a ClassFileError | — | 中 | 保留(移植说明) |
| 619 | crates/javac-core/src/jvm/pool_reader.rs:173 | 真实未实现 | — the ClassReader form is not ported yet (see the file header). | — | 中 | 按上游实现 |
| 620 | crates/javac-core/src/jvm/pool_reader.rs:306 | 真实未实现(待核) | TODO(P2): Symtab/currentModule are not ported; the port keeps the | — | 中 | 按上游实现 |
| 621 | crates/javac-core/src/jvm/pool_reader.rs:320 | 真实未实现(待核) | TODO(P2): Type/sigToType is not ported; the raw descriptor Name is | — | 中 | 按上游实现 |
| 622 | crates/javac-core/src/jvm/pool_reader.rs:354 | 真实未实现(待核) | TODO(P2): Symtab is not ported; the port keeps the name. | — | 中 | 按上游实现 |
| 623 | crates/javac-core/src/jvm/profile.rs:20 | 真实未实现 | constant is not ported yet (option.rs has the "-profile" primary name | — | 中 | 按上游实现 |
| 624 | crates/javac-core/src/jvm/profile.rs:131 | 真实未实现(待核) | TODO(Options/Option): Options.instance/options.get and | — | 中 | 按上游实现 |
| 625 | crates/javac-core/src/jvm/profile.rs:132 | 真实未实现 | Option.PROFILE ("-profile") are not ported; the entry point stays | — | 中 | 按上游实现 |
| 626 | crates/javac-core/src/jvm/target.rs:17 | 真实未实现(待核) | * TODO(Options/Option): `instance(Context)` uses the not yet ported | — | 中 | 按上游实现 |
| 627 | crates/javac-core/src/jvm/target.rs:268 | 真实未实现(待核) | TODO(Options/Option): Options.instance/options.get and | — | 中 | 按上游实现 |
| 628 | crates/javac-core/src/jvm/target.rs:269 | 真实未实现 | Option.TARGET ("--target -target") are not ported; the entry point stays | — | 中 | 按上游实现 |
| 629 | crates/javac-core/src/launcher/mod.rs:1 | 真实未实现 | com.sun.tools.javac.launcher (clipped: not ported, see docs/SUPPORT_MATRIX.md) | — | 中 | 按上游实现 |
| 630 | crates/javac-core/src/main/mini_compiler.rs:492 | 真实未实现 | is CLASS_PATH (Modules.java:440-500; Modules is not ported yet, so this | Modules.java:440-500 | 中 | 按上游实现 |
| 631 | crates/javac-core/src/main/mini_compiler.rs:585 | 真实未实现 | JavaCompiler.enterTrees -> InitModules is not ported (single module). | — | 中 | 按上游实现 |
| 632 | crates/javac-core/src/main/option.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 高 | 按上游实现(核心编译链,优先) |
| 633 | crates/javac-core/src/main/option.rs:5 | 真实未实现 | - NOT PORTED: getSupportedRuntimeOptions (Option.java:820) | Option.java:820 | 高 | 按上游实现(核心编译链,优先) |
| 634 | crates/javac-core/src/main/option.rs:6 | 真实未实现 | - NOT PORTED: showHelp (Option.java:1003) | Option.java:1003 | 高 | 按上游实现(核心编译链,优先) |
| 635 | crates/javac-core/src/main/option.rs:7 | 真实未实现 | - NOT PORTED: getPrimaryName (Option.java:1071) | Option.java:1071 | 高 | 按上游实现(核心编译链,优先) |
| 636 | crates/javac-core/src/main/option.rs:37 | 真实未实现(待核) | TODO(main/OptionHelper + Arguments + Log): process/handleOption/help | — | 高 | 按上游实现(核心编译链,优先) |
| 637 | crates/javac-core/src/main/option.rs:594 | 真实未实现(待核) | TODO(main/OptionHelper): the upstream (String, Throwable) constructor is | — | 高 | 按上游实现(核心编译链,优先) |
| 638 | crates/javac-core/src/main/option.rs:595 | 真实未实现 | not ported; the project error model carries the cause separately (Porting rules). | — | 高 | 按上游实现(核心编译链,优先) |
| 639 | crates/javac-core/src/main/option.rs:785 | 真实未实现 | BOOT_CLASS_PATH/XBOOTCLASSPATH/EXTDIRS/ENDORSEDDIRS are not ported, see | — | 高 | 按上游实现(核心编译链,优先) |
| 640 | crates/javac-core/src/main/option.rs:903 | 真实未实现 | The java.util.regex Pattern is not ported; this function implements the | — | 高 | 按上游实现(核心编译链,优先) |
| 641 | crates/javac-core/src/parser/doc_comment_parser.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 642 | crates/javac-core/src/parser/doc_comment_parser.rs:5 | 真实未实现 | - NOT PORTED: getTreeKind (DocCommentParser.java:1106) | DocCommentParser.java:1106 | 中 | 按上游实现 |
| 643 | crates/javac-core/src/parser/doc_comment_parser.rs:15 | 真实未实现 | - upstream holds ParserFactory fac; ParserFactory is not ported yet | — | 中 | 按上游实现 |
| 644 | crates/javac-core/src/parser/doc_comment_parser.rs:20 | 真实未实现(待核) | from Log.currentSource() (TODO(parser S7)); | — | 中 | 按上游实现 |
| 645 | crates/javac-core/src/parser/doc_comment_parser.rs:30 | 真实未实现(待核) | - TODO(tree): erroneous() calls DocTreeMaker.newErroneousTree(text, | — | 中 | 按上游实现 |
| 646 | crates/javac-core/src/parser/doc_comment_parser.rs:31 | 真实未实现 | diagSource, code), which is still not ported in tree/doc_tree_maker.rs | — | 中 | 按上游实现 |
| 647 | crates/javac-core/src/parser/doc_comment_parser.rs:33 | 真实未实现(待核) | input panics with that TODO; well-formed comments do not take this path. | — | 中 | 按上游实现 |
| 648 | crates/javac-core/src/parser/doc_comment_parser.rs:34 | 真实未实现(待核) | - TODO(parser S7): reference() delegates to ReferenceParser, whose fragment | — | 中 | 按上游实现 |
| 649 | crates/javac-core/src/parser/doc_comment_parser.rs:36 | 真实未实现 | ReferenceParser not-yet-ported marker until parser S7 lands. | — | 中 | 按上游实现 |
| 650 | crates/javac-core/src/parser/doc_comment_parser.rs:99 | 真实未实现(依赖批次) | Not wired yet (TODO(doc comment S8 wiring)). | — | 中 | 依赖批次接线(按上游) |
| 651 | crates/javac-core/src/parser/doc_comment_parser.rs:938 | 真实未实现(待核) | TODO(tree): DocTreeMaker.newErroneousTree(text, diagSource, code) | — | 中 | 按上游实现 |
| 652 | crates/javac-core/src/parser/doc_comment_parser.rs:939 | 真实未实现 | is still not ported (the upstream diagnostic position is the node | — | 中 | 按上游实现 |
| 653 | crates/javac-core/src/parser/doc_comment_parser.rs:1459 | 纯文档说明 | (registered TODO(tree): null description fields). | — | 中 | 保留(移植说明) |
| 654 | crates/javac-core/src/parser/doc_comment_parser.rs:1544 | 纯文档说明 | non-null reference node (registered TODO(tree): null reference fields). | — | 中 | 保留(移植说明) |
| 655 | crates/javac-core/src/parser/doc_comment_parser.rs:1546 | 真实未实现(待核) | .expect("null reference node is not representable in the port (TODO)") | — | 中 | 按上游实现 |
| 656 | crates/javac-core/src/parser/javac_parser.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 高 | 按上游实现(核心编译链,优先) |
| 657 | crates/javac-core/src/parser/javac_parser.rs:5 | 真实未实现 | - NOT PORTED: newEndPosTable (JavacParser.java:193) | JavacParser.java:193 | 高 | 按上游实现(核心编译链,优先) |
| 658 | crates/javac-core/src/parser/javac_parser.rs:6 | 真实未实现 | - NOT PORTED: newOdStack (JavacParser.java:1101) | JavacParser.java:1101 | 高 | 按上游实现(核心编译链,优先) |
| 659 | crates/javac-core/src/parser/javac_parser.rs:7 | 真实未实现 | - NOT PORTED: newOpStack (JavacParser.java:1107) | JavacParser.java:1107 | 高 | 按上游实现(核心编译链,优先) |
| 660 | crates/javac-core/src/parser/javac_parser.rs:8 | 真实未实现 | - NOT PORTED: setParser (JavacParser.java:5035) | JavacParser.java:5035 | 高 | 按上游实现(核心编译链,优先) |
| 661 | crates/javac-core/src/parser/javac_parser.rs:34 | 纯文档说明 | TODO(parser S7) marker (doc-comment reference parsing only; registered). | — | 高 | 保留(移植说明) |
| 662 | crates/javac-core/src/parser/javac_parser.rs:541 | 真实未实现(依赖批次) | TODO(parser wiring): wire the remaining term3Rest/annotations call sites. | — | 高 | 依赖批次接线(按上游) |
| 663 | crates/javac-core/src/parser/javac_parser.rs:553 | 真实未实现(依赖批次) | TODO(parser wiring). | — | 高 | 依赖批次接线(按上游) |
| 664 | crates/javac-core/src/parser/javadoc_tokenizer.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 665 | crates/javac-core/src/parser/javadoc_tokenizer.rs:5 | 真实未实现 | - NOT PORTED: ensure (JavadocTokenizer.java:398) | JavadocTokenizer.java:398 | 中 | 按上游实现 |
| 666 | crates/javac-core/src/parser/lazy_doc_comment_table.rs:14 | 真实未实现 | not ported yet (parser S7), so the same dependencies are injected: | — | 中 | 按上游实现 |
| 667 | crates/javac-core/src/parser/lazy_doc_comment_table.rs:16 | 真实未实现(待核) | took from Log.currentSource() (TODO(parser S7): use ParserFactory directly); | — | 中 | 按上游实现 |
| 668 | crates/javac-core/src/parser/lazy_doc_comment_table.rs:68 | 真实未实现(待核) | ParserFactory is available (parser S7; TODO). | — | 中 | 按上游实现 |
| 669 | crates/javac-core/src/parser/mod.rs:20 | 真实未实现 | registrations that are not ported yet (Names has no Context registration, | — | 中 | 按上游实现 |
| 670 | crates/javac-core/src/parser/mod.rs:21 | 真实未实现(待核) | Source.instance is TODO(Options)): the dependencies are injected | — | 中 | 按上游实现 |
| 671 | crates/javac-core/src/parser/mod.rs:22 | 纯文档说明 | (registered GAP/TODO(P6)); | — | 中 | 保留(移植说明) |
| 672 | crates/javac-core/src/parser/parser_factory.rs:13 | 真实未实现(依赖批次) | injection (its instance() is not implemented yet, TODO(Log/Lint/Options | — | 中 | 依赖批次接线(按上游) |
| 673 | crates/javac-core/src/parser/parser_factory.rs:17 | 纯文档说明 | * Locale is not ported (registered GAP, no field); | — | 中 | 保留(移植说明) |
| 674 | crates/javac-core/src/parser/parser_factory.rs:39 | 真实未实现(依赖批次) | Upstream ParserFactory.tokens (unwired yet: TODO(parser wiring)). | — | 中 | 依赖批次接线(按上游) |
| 675 | crates/javac-core/src/parser/reference_parser.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 676 | crates/javac-core/src/parser/scanner_factory.rs:13 | 真实未实现(待核) | implemented yet (TODO(Options)), so new()/instance() take Names and | — | 中 | 按上游实现 |
| 677 | crates/javac-core/src/parser/scanner_factory.rs:45 | 真实未实现(依赖批次) | implemented yet (TODO(Log/Lint/Options)), so the dependency is an | — | 中 | 依赖批次接线(按上游) |
| 678 | crates/javac-core/src/parser/scanner_factory.rs:50 | 真实未实现(待核) | yet (TODO(Options)); None is the default (no -Xlint categories), | — | 中 | 按上游实现 |
| 679 | crates/javac-core/src/parser/tokens.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 680 | crates/javac-core/src/parser/tokens.rs:5 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getKind (Tokens.java:258) | Tokens.java:258 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 681 | crates/javac-core/src/parser/tokens.rs:27 | 纯文档说明 | keyword table; the Context-based instance is a registered GAP (TODO(P6)). | — | 中 | 保留(移植说明) |
| 682 | crates/javac-core/src/parser/unicode_reader.rs:22 | 纯文档说明 | Unicode digit tables are not ported; registered GAP, the Java corpus does | — | 中 | 保留(移植说明) |
| 683 | crates/javac-core/src/parser/unicode_reader.rs:424 | 真实未实现 | Handle other digits (the full Unicode digit tables are not ported). | — | 中 | 按上游实现 |
| 684 | crates/javac-core/src/platform/jdk_platform_provider.rs:36 | 真实未实现 | JVM (P6 launcher not ported), so the JDK home is injected: | — | 中 | 按上游实现 |
| 685 | crates/javac-core/src/platform/jdk_platform_provider.rs:53 | 真实未实现(待核) | ## Not ported yet (TODO, with the dependency that unblocks each) | — | 中 | 按上游实现 |
| 686 | crates/javac-core/src/platform/jdk_platform_provider.rs:60 | 真实未实现 | but JavacFileManager and its Locations are not ported), | — | 中 | 按上游实现 |
| 687 | crates/javac-core/src/platform/jdk_platform_provider.rs:305 | 真实未实现(依赖批次) | TODO(S3): read by getFileManager() (the ct.sym section lookup), which is not | — | 中 | 依赖批次接线(按上游) |
| 688 | crates/javac-core/src/platform/jdk_platform_provider.rs:313 | 真实未实现(依赖批次) | TODO(S3): the map is only filled by getFileManager(); no FileSystem handle is kept in | — | 中 | 依赖批次接线(按上游) |
| 689 | crates/javac-core/src/platform/jdk_platform_provider.rs:432 | 真实未实现(依赖批次) | TODO(S3)), so the upstream loop over an empty map is the Ok(()) case here.  When S3 | — | 中 | 依赖批次接线(按上游) |
| 690 | crates/javac-core/src/platform/platform_utils.rs:18 | 真实未实现 | `com.sun.tools.javac.main.Arguments` (the main package is not ported) exists, so | — | 中 | 按上游实现 |
| 691 | crates/javac-core/src/platform/platform_utils.rs:33 | 真实未实现 | `ServiceLoader` discovery is not ported (see the file header); P6 passes the | — | 中 | 按上游实现 |
| 692 | crates/javac-core/src/resources/diagnostic_contract.rs:11 | 真实未实现 | - the other *Arg types correspond to javac types not ported yet (Symbol/Type/File/...). | — | 中 | 按上游实现 |
| 693 | crates/javac-core/src/resources/diagnostic_contract.rs:213 | 真实未实现 | Generate opaque placeholders for javac diagnostic argument types not ported yet (replaced by the real types at P4). | — | 中 | 按上游实现 |
| 694 | crates/javac-core/src/tree/dc_tree.rs:20 | 真实未实现(待核) | - TODO(P4/P6): upstream DCErroneous implements JCDiagnostic.DiagnosticPosition | — | 中 | 按上游实现 |
| 695 | crates/javac-core/src/tree/dc_tree.rs:24 | 真实未实现 | factory-based constructor is not ported (no placeholder is introduced). | — | 中 | 按上游实现 |
| 696 | crates/javac-core/src/tree/doc_pretty.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 697 | crates/javac-core/src/tree/doc_pretty.rs:5 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: println (DocPretty.java:104) | DocPretty.java:104 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 698 | crates/javac-core/src/tree/doc_pretty.rs:6 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitComment (DocPretty.java:172) | DocPretty.java:172 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 699 | crates/javac-core/src/tree/doc_pretty.rs:7 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitDeprecated (DocPretty.java:182) | DocPretty.java:182 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 700 | crates/javac-core/src/tree/doc_pretty.rs:8 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitDocRoot (DocPretty.java:211) | DocPretty.java:211 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 701 | crates/javac-core/src/tree/doc_pretty.rs:9 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitDocType (DocPretty.java:223) | DocPretty.java:223 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 702 | crates/javac-core/src/tree/doc_pretty.rs:10 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitErroneous (DocPretty.java:257) | DocPretty.java:257 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 703 | crates/javac-core/src/tree/doc_pretty.rs:11 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitHidden (DocPretty.java:267) | DocPretty.java:267 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 704 | crates/javac-core/src/tree/doc_pretty.rs:12 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitInheritDoc (DocPretty.java:309) | DocPretty.java:309 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 705 | crates/javac-core/src/tree/doc_pretty.rs:13 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitText (DocPretty.java:547) | DocPretty.java:547 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 706 | crates/javac-core/src/tree/doc_tree_maker.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 707 | crates/javac-core/src/tree/doc_tree_maker.rs:5 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: cast (DocTreeMaker.java:729) | DocTreeMaker.java:729 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 708 | crates/javac-core/src/tree/doc_tree_maker.rs:19 | 真实未实现(待核) | - TODO(P6): upstream instance(Context) needs JavacTrees and | — | 中 | 按上游实现 |
| 709 | crates/javac-core/src/tree/doc_tree_maker.rs:20 | 真实未实现 | ParserFactory/ReferenceParser, which are not ported yet; the ported | — | 中 | 按上游实现 |
| 710 | crates/javac-core/src/tree/doc_tree_maker.rs:21 | 真实未实现(待核) | factory methods are usable through [DocTreeMaker::new]. TODO(parser S8): | — | 中 | 按上游实现 |
| 711 | crates/javac-core/src/tree/doc_tree_maker.rs:25 | 纯文档说明 | a not-yet-ported marker (registered, no replacement algorithm is written); | — | 中 | 保留(移植说明) |
| 712 | crates/javac-core/src/tree/doc_tree_maker.rs:26 | 真实未实现(待核) | - TODO(P6): getSentenceBreak uses trees.getBreakIterator() upstream; until | — | 中 | 按上游实现 |
| 713 | crates/javac-core/src/tree/doc_tree_maker.rs:62 | 真实未实现(待核) | args) method (TODO, see the file header); the accessor keeps the | — | 中 | 按上游实现 |
| 714 | crates/javac-core/src/tree/doc_tree_maker.rs:68 | 真实未实现(待核) | TODO(P6): trees: JavacTrees (used by getSentenceBreak) | — | 中 | 按上游实现 |
| 715 | crates/javac-core/src/tree/doc_tree_maker.rs:69 | 真实未实现(待核) | TODO(parser S8): reference_parser: ReferenceParser (used by newReferenceTree) | — | 中 | 按上游实现 |
| 716 | crates/javac-core/src/tree/doc_tree_maker.rs:79 | 真实未实现(待核) | TODO(P6)/(parser S8), see the file header). | — | 中 | 按上游实现 |
| 717 | crates/javac-core/src/tree/doc_tree_maker.rs:423 | 真实未实现(待核) | TODO(parser S8): needs parser.ReferenceParser (not ported yet); upstream | — | 中 | 按上游实现 |
| 718 | crates/javac-core/src/tree/doc_tree_maker.rs:771 | 真实未实现(待核) | * TODO(P6): upstream consults trees.getBreakIterator(); JavacTrees and | — | 中 | 按上游实现 |
| 719 | crates/javac-core/src/tree/doc_tree_maker.rs:772 | 真实未实现 | * BreakIterator are not ported yet, so the upstream breakIterator == null | — | 中 | 按上游实现 |
| 720 | crates/javac-core/src/tree/jc_tree.rs:46 | 真实未实现 | Not ported in this step (tracked in docs/STATUS.md): | — | 中 | 按上游实现 |
| 721 | crates/javac-core/src/tree/pretty.rs:24 | 真实未实现 | - NOT PORTED: the printUnit(tree, cdef) import filtering (isUsed needs the | — | 中 | 按上游实现 |
| 722 | crates/javac-core/src/tree/pretty.rs:25 | 真实未实现(待核) | internal TreeScanner). The cdef != null path panics with a TODO message; | — | 中 | 按上游实现 |
| 723 | crates/javac-core/src/tree/pretty.rs:28 | 真实未实现(待核) | TODO(parser): compare the full Pretty output against the oracle data | — | 中 | 按上游实现 |
| 724 | crates/javac-core/src/tree/pretty.rs:405 | 真实未实现(待核) | TreeScanner, not ported yet); it panics with a TODO message. | — | 中 | 按上游实现 |
| 725 | crates/javac-core/src/tree/tree_info.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 726 | crates/javac-core/src/tree/tree_info.rs:5 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getTree (TreeInfo.java:663) | TreeInfo.java:663 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 727 | crates/javac-core/src/tree/tree_info.rs:6 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getStartPosition (TreeInfo.java:664) | TreeInfo.java:664 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 728 | crates/javac-core/src/tree/tree_info.rs:7 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getPreferredPosition (TreeInfo.java:665) | TreeInfo.java:665 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 729 | crates/javac-core/src/tree/tree_info.rs:8 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getEndPosition (TreeInfo.java:666) | TreeInfo.java:666 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 730 | crates/javac-core/src/tree/tree_info.rs:9 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitIdent (TreeInfo.java:721) | TreeInfo.java:721 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 731 | crates/javac-core/src/tree/tree_info.rs:10 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitSelect (TreeInfo.java:725) | TreeInfo.java:725 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 732 | crates/javac-core/src/tree/tree_info.rs:11 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: scan (TreeInfo.java:749) | TreeInfo.java:749 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 733 | crates/javac-core/src/tree/tree_info.rs:12 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitTopLevel (TreeInfo.java:753) | TreeInfo.java:753 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 734 | crates/javac-core/src/tree/tree_info.rs:13 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitModuleDef (TreeInfo.java:757) | TreeInfo.java:757 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 735 | crates/javac-core/src/tree/tree_info.rs:14 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitPackageDef (TreeInfo.java:761) | TreeInfo.java:761 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 736 | crates/javac-core/src/tree/tree_info.rs:15 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitClassDef (TreeInfo.java:765) | TreeInfo.java:765 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 737 | crates/javac-core/src/tree/tree_info.rs:16 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitMethodDef (TreeInfo.java:769) | TreeInfo.java:769 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 738 | crates/javac-core/src/tree/tree_info.rs:17 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitVarDef (TreeInfo.java:773) | TreeInfo.java:773 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 739 | crates/javac-core/src/tree/tree_info.rs:18 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitTypeParameter (TreeInfo.java:777) | TreeInfo.java:777 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 740 | crates/javac-core/src/tree/tree_info.rs:21 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: visitAnnotation (TreeInfo.java:1314) | TreeInfo.java:1314 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 741 | crates/javac-core/src/tree/tree_info.rs:43 | 真实未实现 | NOT PORTED IN THIS STEP (dependencies are not ported yet; no placeholder | — | 中 | 按上游实现 |
| 742 | crates/javac-core/src/tree/tree_info.rs:50 | 真实未实现 | JCDiagnostic/DiagnosticPosition layer is not ported yet); | — | 中 | 按上游实现 |
| 743 | crates/javac-core/src/tree/tree_info.rs:52 | 真实未实现 | - flagNames (Flags.toString is not ported yet). | — | 中 | 按上游实现 |
| 744 | crates/javac-core/src/tree/tree_info.rs:1000 | 真实未实现 | for every ported subclass except the not-yet-ported delegated symbols. | — | 中 | 按上游实现 |
| 745 | crates/javac-core/src/tree/tree_info.rs:1473 | 真实未实现(待核) | TODO: the constant value (Type.constValue) is not | — | 中 | 按上游实现 |
| 746 | crates/javac-core/src/tree/tree_maker.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 747 | crates/javac-core/src/tree/tree_maker.rs:5 | 真实未实现 | - NOT PORTED: SpeculativeNewClass (TreeMaker.java:407) / classDeclRemoved | TreeMaker.java:407 | 中 | 按上游实现 |
| 748 | crates/javac-core/src/tree/tree_maker.rs:8 | 真实未实现 | - NOT PORTED: Literal(Object) Byte/Short branches (TreeMaker.java:899-996): | TreeMaker.java:899-996 | 中 | 按上游实现 |
| 749 | crates/javac-core/src/tree/tree_maker.rs:41 | 纯文档说明 | remain registered NOT PORTED, see the Registration list above). | — | 中 | 保留(移植说明) |
| 750 | crates/javac-core/src/tree/tree_maker.rs:89 | 真实未实现(待核) | Java: Types types; Symtab syms; — not declared yet, see the file header (TODO). | — | 中 | 按上游实现 |
| 751 | crates/javac-core/src/tree/tree_maker.rs:125 | 真实未实现 | Java: names = Names.instance(context); Types/Symtab are not ported yet (file header). | — | 中 | 按上游实现 |
| 752 | crates/javac-core/src/tree/tree_maker.rs:215 | 纯文档说明 | "TODO(Names batch): TreeMaker.names is None (Names.instance adapter not ported)" | — | 中 | 保留(移植说明) |
| 753 | crates/javac-core/src/tree/tree_maker.rs:1685 | 纯文档说明 | AnnotationBuilder visitor) stay registered as NOT PORTED: they need the | — | 中 | 保留(移植说明) |
| 754 | crates/javac-core/src/tree/tree_maker.rs:2298 | 纯文档说明 | registered NOT PORTED, see the file header; the constant type is not set | — | 中 | 保留(移植说明) |
| 755 | crates/javac-core/src/util/abstract_diagnostic_formatter.rs:4 | 真实未实现 | - NOT PORTED: expr2String (AbstractDiagnosticFormatter.java:231) | AbstractDiagnosticFormatter.java:231 | 中 | 按上游实现 |
| 756 | crates/javac-core/src/util/abstract_diagnostic_formatter.rs:5 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getConfiguration (AbstractDiagnosticFormatter.java:400) | AbstractDiagnosticFormatter.java:400 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 757 | crates/javac-core/src/util/abstract_diagnostic_formatter.rs:6 | 真实未实现 | - NOT PORTED: getPrinter (AbstractDiagnosticFormatter.java:517) | AbstractDiagnosticFormatter.java:517 | 中 | 按上游实现 |
| 758 | crates/javac-core/src/util/abstract_diagnostic_formatter.rs:7 | 真实未实现 | - NOT PORTED: setPrinter (AbstractDiagnosticFormatter.java:521) | AbstractDiagnosticFormatter.java:521 | 中 | 按上游实现 |
| 759 | crates/javac-core/src/util/abstract_diagnostic_formatter.rs:8 | 真实未实现 | - NOT PORTED: capturedVarId (AbstractDiagnosticFormatter.java:541) | AbstractDiagnosticFormatter.java:541 | 中 | 按上游实现 |
| 760 | crates/javac-core/src/util/abstract_diagnostic_formatter.rs:9 | 真实未实现 | - NOT PORTED: visitCapturedType (AbstractDiagnosticFormatter.java:545) | AbstractDiagnosticFormatter.java:545 | 中 | 按上游实现 |
| 761 | crates/javac-core/src/util/abstract_diagnostic_formatter.rs:41 | 真实未实现 | - SimpleConfiguration(Options) needs the not-yet-ported Options: it is not | — | 中 | 按上游实现 |
| 762 | crates/javac-core/src/util/abstract_diagnostic_formatter.rs:42 | 真实未实现(待核) | provided (TODO in the that ports Options); the report-style option | — | 中 | 按上游实现 |
| 763 | crates/javac-core/src/util/abstract_diagnostic_formatter.rs:44 | 真实未实现(待核) | - NOT PORTED IN THIS STEP (dependencies are not ported yet; TODO, no | — | 中 | 按上游实现 |
| 764 | crates/javac-core/src/util/abstract_diagnostic_formatter.rs:47 | 真实未实现 | (not ported); | — | 中 | 按上游实现 |
| 765 | crates/javac-core/src/util/abstract_diagnostic_formatter.rs:115 | 真实未实现(待核) | ported yet, TODO). | — | 中 | 按上游实现 |
| 766 | crates/javac-core/src/util/abstract_diagnostic_formatter.rs:382 | 真实未实现 | NOT PORTED: Type / Symbol (code/Printer), Profile / Option / Target / | — | 中 | 按上游实现 |
| 767 | crates/javac-core/src/util/abstract_diagnostic_formatter.rs:396 | 真实未实现 | NOT PORTED before the code/Source; now ported. | — | 中 | 按上游实现 |
| 768 | crates/javac-core/src/util/abstract_log.rs:88 | 真实未实现(依赖批次) | TODO(Log): pass the log itself as DiagnosticSourceLog once the | — | 中 | 依赖批次接线(按上游) |
| 769 | crates/javac-core/src/util/basic_diagnostic_formatter.rs:35 | 真实未实现 | - BasicConfiguration(Options) needs the not-yet-ported Options and is not | — | 中 | 按上游实现 |
| 770 | crates/javac-core/src/util/basic_diagnostic_formatter.rs:36 | 纯文档说明 | provided (TODO: the diags.* option decoding is registered in the Options | — | 中 | 保留(移植说明) |
| 771 | crates/javac-core/src/util/context.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 772 | crates/javac-core/src/util/context.rs:5 | 真实未实现 | - NOT PORTED: uncheckedCast (Context.java:201) | Context.java:201 | 中 | 按上游实现 |
| 773 | crates/javac-core/src/util/convert.rs:18 | 真实未实现(待核) | TODO(P1): Unicode digits in Character.digit; the parser only feeds ASCII | — | 中 | 按上游实现 |
| 774 | crates/javac-core/src/util/convert.rs:128 | 真实未实现(待核) | Java: Character.digit(char, radix); ASCII-only for now (see TODO above). | — | 中 | 按上游实现 |
| 775 | crates/javac-core/src/util/diagnostic_source.rs:21 | 真实未实现 | expressed because java.nio.CharBuffer is not ported; the JavaString units | — | 中 | 按上游实现 |
| 776 | crates/javac-core/src/util/fatal_error.rs:33 | 真实未实现(待核) | TODO(P0+): Java constructors FatalError(JCDiagnostic) and FatalError(JCDiagnostic, Throwable) | — | 中 | 按上游实现 |
| 777 | crates/javac-core/src/util/graph_utils.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 778 | crates/javac-core/src/util/graph_utils.rs:5 | 真实未实现 | - NOT PORTED: nodeAttributes (GraphUtils.java:84) | GraphUtils.java:84 | 中 | 按上游实现 |
| 779 | crates/javac-core/src/util/graph_utils.rs:6 | 真实未实现 | - NOT PORTED: dependencyAttributes (GraphUtils.java:88) | GraphUtils.java:88 | 中 | 按上游实现 |
| 780 | crates/javac-core/src/util/graph_utils.rs:7 | 真实未实现 | - NOT PORTED: formatProperties (GraphUtils.java:252) | GraphUtils.java:252 | 中 | 按上游实现 |
| 781 | crates/javac-core/src/util/graph_utils.rs:8 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: wrap (GraphUtils.java:258) | GraphUtils.java:258 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 782 | crates/javac-core/src/util/graph_utils.rs:33 | 真实未实现 | DottableNode, DotVisitor and toDot are not ported. Their output depends on Object.hashCode() | — | 中 | 按上游实现 |
| 783 | crates/javac-core/src/util/iterators.rs:16 | 纯文档说明 | no callers; std::iter::Iterator has no remove, so it is not ported (registered in STATUS.md). | — | 中 | 保留(移植说明) |
| 784 | crates/javac-core/src/util/javac_messages.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 785 | crates/javac-core/src/util/javac_messages.rs:5 | 真实未实现 | - NOT PORTED: getDefaultMessages (JavacMessages.java:196) | JavacMessages.java:196 | 中 | 按上游实现 |
| 786 | crates/javac-core/src/util/javac_messages.rs:19 | 纯文档说明 | locale is "en" (environment detection is a registered TODO); | — | 中 | 保留(移植说明) |
| 787 | crates/javac-core/src/util/javac_messages.rs:34 | 真实未实现(待核) | TODO(D5): the JCDiagnostic-dependent members of JavacMessages | — | 中 | 按上游实现 |
| 788 | crates/javac-core/src/util/javac_messages.rs:105 | 真实未实现(待核) | Java: Locale.getDefault() -- TODO: environment locale detection is a | — | 中 | 按上游实现 |
| 789 | crates/javac-core/src/util/javac_messages.rs:207 | 真实未实现(待核) | TODO(D5): the upstream Context constructor also initializes Options | — | 中 | 按上游实现 |
| 790 | crates/javac-core/src/util/jc_diagnostic.rs:44 | 真实未实现 | NOT PORTED IN THIS STEP (dependencies are not ported yet; no substitute | — | 中 | 按上游实现 |
| 791 | crates/javac-core/src/util/jc_diagnostic.rs:693 | 纯文档说明 | PORTING_RULES: the source (DiagnosticSource) is not ported yet, so | — | 中 | 保留(移植说明) |
| 792 | crates/javac-core/src/util/jc_diagnostic.rs:792 | 真实未实现(待核) | TODO(Options): the upstream protected Factory(Context) initializes | — | 中 | 按上游实现 |
| 793 | crates/javac-core/src/util/jc_diagnostic.rs:794 | 真实未实现 | Options are not ported yet, so instance() only creates/registers the | — | 中 | 按上游实现 |
| 794 | crates/javac-core/src/util/list_buffer.rs:16 | 真实未实现 | (remove/removeAll/retainAll) are not ported: remove(Object) throws | — | 中 | 按上游实现 |
| 795 | crates/javac-core/src/util/list.rs:38 | 真实未实现 | retainAll/subList are not ported: every one of them throws UnsupportedOperationException in | — | 中 | 按上游实现 |
| 796 | crates/javac-core/src/util/list.rs:542 | 真实未实现(待核) | TODO(P3): public static <T> List<T> convert(Class<T> klass, List<?> list) | — | 中 | 按上游实现 |
| 797 | crates/javac-core/src/util/log.rs:31 | 纯文档说明 | TODO for the remaining types; registered); | — | 中 | 保留(移植说明) |
| 798 | crates/javac-core/src/util/log.rs:35 | 真实未实现 | - NOT PORTED IN THIS STEP (dependencies not ported yet; no substitute): | — | 中 | 按上游实现 |
| 799 | crates/javac-core/src/util/log.rs:36 | 真实未实现 | * Main.javacBundleName (main/Main is not ported): the javac resource bundle | — | 中 | 按上游实现 |
| 800 | crates/javac-core/src/util/log.rs:37 | 真实未实现(待核) | is not added to JavacMessages by the ported Log (TODO Main); | — | 中 | 按上游实现 |
| 801 | crates/javac-core/src/util/log.rs:386 | 真实未实现(待核) | TODO(Main): messages.add(Main.javacBundleName); (main/Main is | — | 中 | 按上游实现 |
| 802 | crates/javac-core/src/util/log.rs:387 | 真实未实现 | not ported yet, so the javac resources are not added here). | — | 中 | 按上游实现 |
| 803 | crates/javac-core/src/util/log.rs:421 | 真实未实现(待核) | (equivalently, TODO Option). | — | 中 | 按上游实现 |
| 804 | crates/javac-core/src/util/log.rs:1006 | 纯文档说明 | rendered (the remaining types are TODO, registered section 9). | — | 中 | 保留(移植说明) |
| 805 | crates/javac-core/src/util/mandatory_warning_handler.rs:19 | 真实未实现 | - the Java Log dependency (Log is not ported yet) becomes the | — | 中 | 按上游实现 |
| 806 | crates/javac-core/src/util/mandatory_warning_handler.rs:32 | 真实未实现 | NOT PORTED IN THIS STEP: | — | 中 | 按上游实现 |
| 807 | crates/javac-core/src/util/message_format.rs:20 | 纯文档说明 | Not implemented (registered TODO): number/date/time/choice sub-formats | — | 中 | 保留(移植说明) |
| 808 | crates/javac-core/src/util/module_helper.rs:8 | 真实未实现(待核) | addExports(Module, Module) is therefore NOT PORTED (TODO, no substitute | — | 中 | 按上游实现 |
| 809 | crates/javac-core/src/util/module_helper.rs:39 | 真实未实现 | NOT PORTED: java.lang.Module is a JDK platform type, not part of the javac | — | 中 | 按上游实现 |
| 810 | crates/javac-core/src/util/options.rs:37 | 真实未实现(待核) | * TODO(Arguments/CommandLine): the table is filled by Arguments/CommandLine | — | 中 | 按上游实现 |
| 811 | crates/javac-core/src/util/options.rs:38 | 真实未实现 | (not ported yet); isLintSet's callers (Lint/Preview) are ported but the | — | 中 | 按上游实现 |
| 812 | crates/javac-core/src/util/propagated_exception.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 813 | crates/javac-core/src/util/propagated_exception.rs:5 | 真实未实现 | - NOT PORTED: getCause (PropagatedException.java:47) | PropagatedException.java:47 | 中 | 按上游实现 |
| 814 | crates/javac-core/src/util/raw_diagnostic_formatter.rs:20 | 真实未实现 | parts)); Options are not ported yet, so only the option-less construction | — | 中 | 按上游实现 |
| 815 | crates/javac-core/src/util/raw_diagnostic_formatter.rs:21 | 真实未实现(待核) | is provided (TODO(Options)); | — | 中 | 按上游实现 |
| 816 | crates/javac-core/src/util/raw_diagnostic_formatter.rs:22 | 真实未实现 | - NOT PORTED IN THIS STEP (dependencies are not ported yet; no substitute | — | 中 | 按上游实现 |
| 817 | crates/javac-core/src/util/raw_diagnostic_formatter.rs:26 | 真实未实现(待核) | file diagnostic) panics with a TODO message; the position-less raw | — | 中 | 按上游实现 |
| 818 | crates/javac-core/src/util/shared_name_table.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 819 | crates/javac-core/src/util/shared_name_table.rs:5 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getIndex (SharedNameTable.java:171) | SharedNameTable.java:171 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 820 | crates/javac-core/src/util/shared_name_table.rs:6 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getByteLength (SharedNameTable.java:176) | SharedNameTable.java:176 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 821 | crates/javac-core/src/util/shared_name_table.rs:7 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getByteAt (SharedNameTable.java:181) | SharedNameTable.java:181 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 822 | crates/javac-core/src/util/shared_name_table.rs:8 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getByteOffset (SharedNameTable.java:191) | SharedNameTable.java:191 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 823 | crates/javac-core/src/util/shared_name_table.rs:9 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: hashCode (SharedNameTable.java:198) | SharedNameTable.java:198 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 824 | crates/javac-core/src/util/string_utils.rs:11 | 真实未实现(待核) | TODO(P0): verify case mapping against JDK 17 String.toLowerCase(Locale.US) | — | 中 | 按上游实现 |
| 825 | crates/javac-core/src/util/unshared_name_table.rs:4 | 真实未实现 | Not yet ported upstream methods (no substitutes in this file): | — | 中 | 按上游实现 |
| 826 | crates/javac-core/src/util/unshared_name_table.rs:5 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: create (UnsharedNameTable.java:41) | UnsharedNameTable.java:41 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 827 | crates/javac-core/src/util/unshared_name_table.rs:6 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getIndex (UnsharedNameTable.java:153) | UnsharedNameTable.java:153 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 828 | crates/javac-core/src/util/unshared_name_table.rs:7 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getByteLength (UnsharedNameTable.java:158) | UnsharedNameTable.java:158 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 829 | crates/javac-core/src/util/unshared_name_table.rs:8 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getByteAt (UnsharedNameTable.java:163) | UnsharedNameTable.java:163 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 830 | crates/javac-core/src/util/unshared_name_table.rs:9 | 过时(疑似本地已实现,更新注释) | - NOT PORTED: getByteOffset (UnsharedNameTable.java:173) | UnsharedNameTable.java:173 | 中 | 更新/删除注释(本地已实现,需人工确认) |
| 831 | crates/javac-core/tests/e2e_compile.rs:200 | 测试或工具 | #[ignore = "E2E blocked: TypeEnter phases (Header/Hierarchy/Permits/Members/Record/finishClass) not yet ported — tracked | — | 低 | 不改(测试/工具) |
| 832 | crates/javac-core/tests/tree_oracle.rs:12 | 测试或工具 | explicitly not ported yet (S7b: records/sealed/patterns/switch | — | 低 | 不改(测试/工具) |
| 833 | crates/javac-core/tests/tree_oracle.rs:16 | 真实未实现(依赖批次) | (TODO(S8 DocCommentParser batch)). Any other difference is a FAIL with the | — | 低 | 依赖批次接线(按上游) |
| 834 | crates/javac-core/tests/tree_oracle.rs:115 | 测试或工具 | if message.contains("TODO(S7b)") { | — | 低 | 不改(测试/工具) |
| 835 | crates/javac-core/tests/tree_oracle.rs:116 | 测试或工具 | Some("S7b syntax not ported yet") | — | 低 | 不改(测试/工具) |
| 836 | crates/javac-core/tests/tree_oracle.rs:117 | 真实未实现(依赖批次) | } else if message.contains("TODO(S8") \|\| message.contains("keepDocComments") { | — | 低 | 依赖批次接线(按上游) |
| 837 | crates/javac-core/tests/tree_oracle.rs:118 | 真实未实现(依赖批次) | Some("keepDocComments=false (TODO(S8 DocCommentParser))") | — | 低 | 依赖批次接线(按上游) |
| 838 | crates/javac-floating/src/big_integer.rs:19 | 纯文档说明 | NOT PORTED (registered): object serialization (`readObject`, `writeObject`, | — | 中 | 保留(移植说明) |
| 839 | crates/javac-floating/src/big_integer.rs:3415 | 真实未实现 | `multiply(long)`; BigDecimal itself is not ported). | — | 中 | 按上游实现 |
| 840 | crates/javac-floating/src/mutable_big_integer.rs:13 | 真实未实现 | * `toBigDecimal`/`toCompactValue` are not ported: they depend on | — | 中 | 按上游实现 |
| 841 | crates/javac-floating/src/mutable_big_integer.rs:2139 | 真实未实现 | Java: long toCompactValue(int sign) — used only by BigDecimal (not ported). | — | 中 | 按上游实现 |
| 842 | crates/javac-floating/src/random.rs:12 | 纯文档说明 | thread-local generator is not ported; registered: only the randomness | — | 中 | 保留(移植说明) |
| 843 | crates/javac-floating/src/random.rs:15 | 纯文档说明 | NOT PORTED (registered): the `java.util.stream` based methods (`ints`, | — | 中 | 保留(移植说明) |
| 844 | crates/javac-source/src/util/doc_trees.rs:9 | 真实未实现(待核) | are deferred to P6 (TODO(P6)). | — | 中 | 按上游实现 |
| 845 | crates/javac-source/src/util/javac_task.rs:8 | 真实未实现 | javax.annotation.processing (not ported) and on the internal | — | 中 | 按上游实现 |
| 846 | crates/javac-source/src/util/javac_task.rs:9 | 真实未实现(待核) | BasicJavacTask/Context; it is deferred to P6 (TODO(P6)). | — | 中 | 按上游实现 |
| 847 | crates/javac-source/src/util/simple_tree_visitor.rs:17 | 纯文档说明 | convenience overloads are not ported (as registered). | — | 中 | 保留(移植说明) |
| 848 | crates/javac-source/src/util/tree_path.rs:14 | 真实未实现(待核) | static getPath searches rely on TreePathScanner (next batch) and are TODO(P1). | — | 中 | 按上游实现 |
| 849 | crates/javac-source/src/util/tree_path.rs:42 | 纯文档说明 | PORTING_RULES: TODO(P1) — the Java implementation searches with a | — | 中 | 保留(移植说明) |
| 850 | crates/javac-source/src/util/trees.rs:10 | 真实未实现(待核) | (TODO(P6), not expressible without the javax.annotation.processing port). | — | 中 | 按上游实现 |
| 851 | crates/javac-strings/src/lib.rs:193 | 真实未实现(待核) | TODO(P2): match java.nio.charset.StandardCharsets.UTF_8 CharsetDecoder | — | 中 | 按上游实现 |
| 852 | crates/javac-tools/src/error.rs:25 | 真实未实现(待核) | TODO(P6): replace this placeholder with the error type introduced by the | — | 低 | 按上游实现 |
| 853 | crates/javac-tools/src/jdk.rs:27 | 真实未实现(待核) | TODO(P6): these placeholders are finalized by the API phase; annotation | — | 低 | 按上游实现 |
| 854 | crates/javac-tools/src/jdk.rs:28 | 真实未实现 | processing is not ported, so class loading is never observed by the | — | 低 | 按上游实现 |
| 855 | crates/javac-tools/src/lang/model/element/annotation_value_visitor.rs:13 | 纯文档说明 | forwards `null` upstream and is not ported (registered difference). | — | 低 | 保留(移植说明) |
| 856 | crates/javac-tools/src/lang/model/element/element_visitor.rs:12 | 纯文档说明 | convenience overloads are not ported (registered difference).  Callers | — | 低 | 保留(移植说明) |
| 857 | crates/javac-tools/src/lang/model/element/module_element.rs:18 | 纯文档说明 | convenience methods are not ported (registered difference).  Callers use | — | 低 | 保留(移植说明) |
| 858 | crates/javac-tools/src/lang/model/element/unknown_annotation_value_exception.rs:6 | 真实未实现 | the `Object p` argument/`getArgument` are not ported (no Rust Object); the | — | 低 | 按上游实现 |
| 859 | crates/javac-tools/src/lang/model/element/unknown_directive_exception.rs:6 | 真实未实现 | the `Object p` argument/`getArgument` are not ported (no Rust Object); the | — | 低 | 按上游实现 |
| 860 | crates/javac-tools/src/lang/model/element/unknown_element_exception.rs:9 | 纯文档说明 | field and `getArgument` are not ported (registered difference). | — | 低 | 保留(移植说明) |
| 861 | crates/javac-tools/src/lang/model/type_/mirrored_type_exception.rs:8 | 纯文档说明 | type, exactly as upstream.  Serialization (`readObject`) is not ported. | — | 低 | 保留(移植说明) |
| 862 | crates/javac-tools/src/lang/model/type_/mirrored_types_exception.rs:12 | 纯文档说明 | the transient-field reset is not ported (registered difference). | — | 低 | 保留(移植说明) |
| 863 | crates/javac-tools/src/lang/model/type_/type_mirror.rs:13 | 真实未实现 | not ported yet. | — | 低 | 按上游实现 |
| 864 | crates/javac-tools/src/lang/model/type_/type_visitor.rs:9 | 纯文档说明 | forwards `null` upstream and is not ported (registered difference). | — | 低 | 保留(移植说明) |
| 865 | crates/javac-tools/src/lang/model/type_/unknown_type_exception.rs:6 | 真实未实现 | the `Object p` argument/`getArgument` are not ported (no Rust Object); the | — | 低 | 按上游实现 |
| 866 | crates/javac-tools/src/lang/model/util/abstract_annotation_value_visitor6.rs:14 | 真实未实现 | null upstream and is not ported. | — | 中 | 按上游实现 |
| 867 | crates/javac-tools/src/lang/model/util/abstract_type_visitor6.rs:13 | 真实未实现 | upstream and is not ported. | — | 中 | 按上游实现 |
| 868 | crates/javac-tools/src/lang/model/util/element_scanner6.rs:16 | 纯文档说明 | upstream and is not ported (registered difference). | — | 中 | 保留(移植说明) |
| 869 | crates/javac-tools/src/lang/model/util/element_scanner9.rs:30 | 真实未实现(待核) | TODO(upstream): the Java source carries the same "Hmmm, this might not be | — | 中 | 按上游实现 |
| 870 | crates/javac-tools/src/lang/model/util/element_scanner9.rs:33 | 真实未实现(待核) | TODO: Hmmm, this might not be right | — | 中 | 按上游实现 |
| 871 | crates/javac-tools/src/lib.rs:58 | 纯文档说明 | TODO(P0): register the adapter files `error.rs`, `uri.rs`, `java_io.rs`, | — | 低 | 保留(移植说明) |
| 872 | crates/javac-tools/src/lib.rs:60 | 真实未实现(待核) | TODO(P2): `java.net.URI` is a minimal mapping; the remaining | — | 低 | 按上游实现 |
| 873 | crates/javac-tools/src/lib.rs:63 | 真实未实现(待核) | TODO(P6): annotation processing is out of scope (docs/PLAN.md §3.1); | — | 低 | 按上游实现 |
| 874 | crates/javac-tools/src/tools/java_file_manager.rs:477 | 真实未实现(待核) | TODO: describe failure modes | — | 低 | 按上游实现 |
| 875 | crates/javac-tools/src/tools/standard_java_file_manager.rs:108 | 真实未实现 | Path and vice versa); they are therefore not ported as functions. | — | 低 | 按上游实现 |
| 876 | crates/javac-tools/src/tools/standard_java_file_manager.rs:422 | 真实未实现(待核) | boundary handled by the file layer (TODO(P2): platform path encoding). | — | 低 | 按上游实现 |
| 877 | crates/javac-tools/src/tools/standard_java_file_manager.rs:754 | 测试或工具 | platform-encoding boundary of the file layer (TODO(P2)); the | — | 低 | 不改(测试/工具) |
| 878 | crates/javac-tools/src/tools/tool_provider.rs:27 | 真实未实现 | getSystemTool/matches private static helpers are therefore not ported; | — | 低 | 按上游实现 |
| 879 | crates/javac-tools/src/tools/tool_provider.rs:29 | 真实未实现(待核) | compiler entry point (TODO(P6)). | — | 低 | 按上游实现 |
| 880 | crates/javac-tools/src/tools/tool_provider.rs:76 | 真实未实现(待核) | TODO(P6): the Rust compiler entry point is created by the API phase | — | 低 | 按上游实现 |
| 881 | crates/javac-tools/src/tools/tool_provider.rs:151 | 测试或工具 | javadoc is not ported: no documentation tool is provided. | — | 低 | 不改(测试/工具) |
| 882 | crates/javac-tools/src/uri.rs:22 | 真实未实现(待核) | TODO(P2): this is not the complete `java.net.URI` port. | — | 低 | 按上游实现 |
| 883 | crates/javac-tools/src/uri.rs:168 | 真实未实现(待核) | TODO(P2): `java.net.URI` validates per component (for example '[' and ']' | — | 低 | 按上游实现 |
| 884 | src/main.rs:8 | 真实未实现 | eprintln!("rust-javac: driver not ported yet (P0 scaffolding)"); | — | 中 | 按上游实现 |
| 885 | xtask/src/audit_porting.rs:6 | 真实未实现(待核) | 对应函数定义(fn <snake_case>)、或在该文件有 TODO/unimplemented 登记; | — | 低 | 按上游实现 |
| 886 | xtask/src/audit_porting.rs:43 | 真实未实现(待核) | No function definition, but a TODO/unimplemented registration names it. | — | 低 | 按上游实现 |
| 887 | xtask/src/audit_porting.rs:304 | 真实未实现(待核) | let is_registration = line.contains("TODO") | — | 低 | 按上游实现 |
| 888 | xtask/src/audit_porting.rs:306 | 真实未实现 | \|\| line.contains("NOT PORTED"); | — | 低 | 按上游实现 |
| 889 | xtask/src/audit_porting.rs:750 | 真实未实现(依赖批次) | let rust = "//! doc\nfn get_thrown_types(ctx: &CompilerContext) -> List<TypeId> { }\n// TODO(Types batch): is_capture_of | — | 低 | 依赖批次接线(按上游) |
| 890 | xtask/src/audit_porting.rs:810 | 真实未实现(依赖批次) | let rust = "fn covered() {}\n// TODO(demo): registered is not wired yet\n"; | — | 低 | 依赖批次接线(按上游) |

## 处置汇总
- 过时(疑似本地已实现):更新/删除注释(约 253 条;需逐项人工确认后清理,禁止直接批量删);
- 真实未实现(含依赖批次):按上游实现/接线;核心缺口见重点表;
- 纯文档说明:保留;
- 测试或工具:不改;
- 完整条目中的上游锚点来自条目文本内嵌 `X.java:NNN`(可靠);无锚点条目需在实现批重新定位上游行。
