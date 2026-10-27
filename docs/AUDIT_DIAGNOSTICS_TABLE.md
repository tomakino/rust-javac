# 编译器诊断资源接线审计(第三版:A0/A1/A0b 后按最新代码重算)

审计对象:`crates/javac-core/src/resources/compiler_properties.rs` 生成的全部诊断资源工厂(935)及本地 crates 调用接线、上游 `reference/jdk17u` 诊断点。第三版在第二版口径上做了两项修正:(1) 删除"同名裸调用"回退——第二版把 `errors::X(...)` 在导入 `fragments` 模块的文件中同名匹配、或把本地同名函数(如 `TypeAnnotationPosition::type_parameter`)误判为已接线,第三版改为**模块精确匹配**(显式 `module::factory(` 或文件导入该模块时的裸调用,且裸调用前不得为 `::`);(2) 上游诊断点扫描不再要求引用后紧跟 `(`,覆盖 `Errors.X` 形式,并重扫 5902 个 Java 文件。

## 审计口径

- 资源键:工厂 `(ErrorInfo|FragmentInfo|NoteInfo|WarningInfo)::new("<key>", ...)` 按模块加前缀(errors→`compiler.err.`,warnings→`compiler.warn.`,notes→`compiler.note.`,fragments→`compiler.misc.`);共解析出 **935** 个工厂(errors 471 / warnings 129 / notes 32 / fragments 303),与第二版一致。
- 已接线:本地非资源代码存在精确调用(显式 `<module>::<factory>(`;或文件导入 `compiler_properties::<module>` 时裸调用 `<factory>(`,裸调用不得以 `::` 前缀,避免把其它模块同名调用误计)。
- 上游诊断点:上游 Java 源码中 `Errors.X`/`Warnings.X`/`Notes.X`/`Fragments.X` 的引用(不论是否后跟括号),记录 file:line(每键最多保留 5 处)。
- 功能已具备待接线:本地未接线;存在上游诊断点;至少一个上游诊断点的 Java 文件在本地有对应实现文件(优先同包 `<pkg>/<snake>.rs`,否则本地任意目录存在同名 `<snake>.rs`)。
- 功能未移植(批次):本地未接线;上游诊断点所在功能文件在本地无对应实现文件;批次取第一处上游诊断点所在包。
- 范围外(引 PLAN §3.1/§3.2):上游诊断点**仅**落在 `processing/**`(PLAN §3.1:APT 不移植)或 `launcher/**`(PLAN §3.2:launcher 已裁剪)。
- 无上游触发:上游 5902 个 Java 文件无该工厂名引用(properties 有定义也无代码触发);生产接线前需先核对是否仅为历史键。
- 限制:同步诊断的键以生成表为准,少量属性文件键(920 个 properties 键 vs 935 工厂)存在同键多工厂/键名冲突;第二版第 935 行 `compiler.misc.null` ↔ `Fragments.WrongVersion` 系其解析错位,第三版按工厂实际键 `compiler.misc.wrong.version` 更正。

## 统计(第三版)

| 指标 | 数量 | 说明 |
| --- | --- | --- |
| 资源工厂总数 | 935 | errors 471 / warnings 129 / notes 32 / fragments 303 |
| 已接线 | 639 | 精确模块匹配(第二版 647 中 11 条为同名跨模块误报,已更正;A0/A1/A0b 新增 3 条真实接线) |
| 功能已具备待接线 | 67 | 本地目标文件存在,按 D1-D7 批次接线 |
| 功能未移植(批次) | 40 | Main/Options 32、API 5、Model 1、Util 2 |
| 范围外(PLAN §3.1/§3.2) | 24 | 上游诊断点仅在 processing(APT) |
| 无上游触发 | 165 | 上游代码无引用/无触发 |
| 合计 | 935 | 五类互斥 |

与第二版对比:已接线 647→639(更正 11 条同名跨模块误报 + 新增 3 条 A0/A1/A0b 接线:`compiler.err.instanceof.pattern.no.subtype`、`compiler.err.instanceof.reifiable.not.safe`、`compiler.misc.varargs.trustme.on.non.varargs.accessor`);待接线 67→67;"功能未移植"66→40,并把其中仅落在 processing/launcher 的 24 条拆为"范围外"(PLAN §3.1/§3.2);无上游触发 155→165(11 条误报更正后进入本类)。

## 范围外清单(24 条:上游诊断点仅落在 processing/launcher;PLAN §3.1 APT 不移植、§3.2 launcher 裁剪)

| # | 资源键 | 工厂 | 上游诊断点 | 依据 |
| --- | --- | --- | --- | --- |
| 1 | `compiler.err.cant.read.file` | `Errors.CantReadFile` | src/jdk.compiler/share/classes/com/sun/tools/javac/launcher/Main.java:279 | PLAN §3.2:launcher 已裁剪 |
| 2 | `compiler.err.proc.bad.config.file` | `Errors.ProcBadConfigFile` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:446 ; src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:455 | PLAN §3.1:APT 不移植 |
| 3 | `compiler.err.proc.cant.access` | `Errors.ProcCantAccess` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:1030 | PLAN §3.1:APT 不移植 |
| 4 | `compiler.err.proc.cant.access.1` | `Errors.ProcCantAccess1` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:1025 | PLAN §3.1:APT 不移植 |
| 5 | `compiler.err.proc.cant.load.class` | `Errors.ProcCantLoadClass` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:449 ; src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:452 | PLAN §3.1:APT 不移植 |
| 6 | `compiler.err.proc.processor.bad.option.name` | `Errors.ProcProcessorBadOptionName` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:774 | PLAN §3.1:APT 不移植 |
| 7 | `compiler.err.proc.processor.cant.instantiate` | `Errors.ProcProcessorCantInstantiate` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:593 | PLAN §3.1:APT 不移植 |
| 8 | `compiler.err.proc.processor.not.found` | `Errors.ProcProcessorNotFound` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:533 ; src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:587 | PLAN §3.1:APT 不移植 |
| 9 | `compiler.err.proc.processor.wrong.type` | `Errors.ProcProcessorWrongType` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:590 | PLAN §3.1:APT 不移植 |
| 10 | `compiler.err.proc.service.problem` | `Errors.ProcServiceProblem` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:430 | PLAN §3.1:APT 不移植 |
| 11 | `compiler.warn.proc.annotations.without.processors` | `Warnings.ProcAnnotationsWithoutProcessors` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:962 | PLAN §3.1:APT 不移植 |
| 12 | `compiler.warn.proc.duplicate.option.name` | `Warnings.ProcDuplicateOptionName` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:741 | PLAN §3.1:APT 不移植 |
| 13 | `compiler.warn.proc.duplicate.supported.annotation` | `Warnings.ProcDuplicateSupportedAnnotation` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:719 | PLAN §3.1:APT 不移植 |
| 14 | `compiler.warn.proc.file.create.last.round` | `Warnings.ProcFileCreateLastRound` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacFiler.java:513 | PLAN §3.1:APT 不移植 |
| 15 | `compiler.warn.proc.file.reopening` | `Warnings.ProcFileReopening` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacFiler.java:765 | PLAN §3.1:APT 不移植 |
| 16 | `compiler.warn.proc.illegal.file.name` | `Warnings.ProcIllegalFileName` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacFiler.java:702 | PLAN §3.1:APT 不移植 |
| 17 | `compiler.warn.proc.malformed.supported.string` | `Warnings.ProcMalformedSupportedString` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:1787 | PLAN §3.1:APT 不移植 |
| 18 | `compiler.warn.proc.processor.incompatible.source.version` | `Warnings.ProcProcessorIncompatibleSourceVersion` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:765 | PLAN §3.1:APT 不移植 |
| 19 | `compiler.warn.proc.redundant.types.with.wildcard` | `Warnings.ProcRedundantTypesWithWildcard` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:733 | PLAN §3.1:APT 不移植 |
| 20 | `compiler.warn.proc.suspicious.class.name` | `Warnings.ProcSuspiciousClassName` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacFiler.java:495 | PLAN §3.1:APT 不移植 |
| 21 | `compiler.warn.proc.type.already.exists` | `Warnings.ProcTypeAlreadyExists` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacFiler.java:735 | PLAN §3.1:APT 不移植 |
| 22 | `compiler.warn.proc.type.recreate` | `Warnings.ProcTypeRecreate` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacFiler.java:731 | PLAN §3.1:APT 不移植 |
| 23 | `compiler.warn.proc.unclosed.type.files` | `Warnings.ProcUnclosedTypeFiles` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacFiler.java:825 | PLAN §3.1:APT 不移植 |
| 24 | `compiler.warn.proc.unmatched.processor.options` | `Warnings.ProcUnmatchedProcessorOptions` | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:1459 | PLAN §3.1:APT 不移植 |

## 功能已具备待接线完整清单(67 条:本地目标文件存在,待接线)

排序:按上游诊断点文件/批次分组;`本地工厂位置` 为 compiler_properties.rs 行号,`本地目标文件` 为映射到的本地实现文件。

| # | 资源键 | 工厂 | 本地工厂位置 | 本地目标文件 | 上游诊断点(最多 2 处) | 建议批次 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `compiler.err.cant.attach.type.annotations` | `Errors.CantAttachTypeAnnotations` | compiler_properties.rs:378 | jvm/class_reader.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:2218 | JVM 批 |
| 2 | `compiler.err.cant.resolve` | `Errors.CantResolve` | compiler_properties.rs:455 | comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:551 | Comp 批 |
| 3 | `compiler.err.cant.type.annotate.scoping.1` | `Errors.CantTypeAnnotateScoping1` | compiler_properties.rs:511 | code/type_annotations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/TypeAnnotations.java:500 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5747 | Code 批 |
| 4 | `compiler.err.class.cant.write` | `Errors.ClassCantWrite` | compiler_properties.rs:532 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1626 | Main/Options 批 |
| 5 | `compiler.err.dot.class.expected` | `Errors.DotClassExpected` | compiler_properties.rs:791 | parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2264 | Parser 批 |
| 6 | `compiler.err.empty.A.argument` | `Errors.EmptyAArgument` | compiler_properties.rs:889 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:440 | Main/Options 批 |
| 7 | `compiler.err.error.reading.file` | `Errors.ErrorReadingFile` | compiler_properties.rs:980 | file/javac_file_manager.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/JavacFileManager.java:421 ; src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:431 | FileManager/Locations 批 |
| 8 | `compiler.err.error.writing.file` | `Errors.ErrorWritingFile` | compiler_properties.rs:987 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:560 | Main/Options 批 |
| 9 | `compiler.err.file.not.file` | `Errors.FileNotFile` | compiler_properties.rs:1085 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:769 | Main/Options 批 |
| 10 | `compiler.err.file.not.found` | `Errors.FileNotFound` | compiler_properties.rs:1092 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/launcher/Main.java:215 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/Main.java:227 | Main/Options 批 |
| 11 | `compiler.err.illegal.argument.for.option` | `Errors.IllegalArgumentForOption` | compiler_properties.rs:1190 | file/base_file_manager.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/BaseFileManager.java:293 | FileManager/Locations 批 |
| 12 | `compiler.err.invalid.A.key` | `Errors.InvalidAKey` | compiler_properties.rs:1519 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:445 | Main/Options 批 |
| 13 | `compiler.err.invalid.module.specifier` | `Errors.InvalidModuleSpecifier` | compiler_properties.rs:1589 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:683 | Main/Options 批 |
| 14 | `compiler.err.invalid.path` | `Errors.InvalidPath` | compiler_properties.rs:1603 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:773 | Main/Options 批 |
| 15 | `compiler.err.invalid.profile` | `Errors.InvalidProfile` | compiler_properties.rs:1617 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:385 | Main/Options 批 |
| 16 | `compiler.err.invalid.repeatable.annotation.not.applicable.in.context` | `Errors.InvalidRepeatableAnnotationNotApplicableInContext` | compiler_properties.rs:1673 | comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:834 | Comp 批 |
| 17 | `compiler.err.invalid.source` | `Errors.InvalidSource` | compiler_properties.rs:1715 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:318 | Main/Options 批 |
| 18 | `compiler.err.invalid.target` | `Errors.InvalidTarget` | compiler_properties.rs:1729 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:339 | Main/Options 批 |
| 19 | `compiler.err.limit.string.overflow` | `Errors.LimitStringOverflow` | compiler_properties.rs:1834 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:743 | Main/Options 批 |
| 20 | `compiler.err.multicatch.types.must.be.disjoint` | `Errors.MulticatchTypesMustBeDisjoint` | compiler_properties.rs:2016 | comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4987 | Comp 批 |
| 21 | `compiler.err.no.zipfs.for.archive` | `Errors.NoZipfsForArchive` | compiler_properties.rs:2177 | file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:404 ; src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:1391 | FileManager/Locations 批 |
| 22 | `compiler.err.option.too.many` | `Errors.OptionTooMany` | compiler_properties.rs:2345 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:458 | Main/Options 批 |
| 23 | `compiler.err.orphaned` | `Errors.Orphaned` | compiler_properties.rs:2352 | parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2542 | Parser 批 |
| 24 | `compiler.err.proc.cant.find.class` | `Errors.ProcCantFindClass` | compiler_properties.rs:2548 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1202 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1220 | Main/Options 批 |
| 25 | `compiler.err.proc.no.explicit.annotation.processing.requested` | `Errors.ProcNoExplicitAnnotationProcessingRequested` | compiler_properties.rs:2569 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1175 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1190 | Main/Options 批 |
| 26 | `compiler.err.source.cant.overwrite.input.file` | `Errors.SourceCantOverwriteInputFile` | compiler_properties.rs:2891 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:762 | Main/Options 批 |
| 27 | `compiler.err.try.resource.may.not.be.assigned` | `Errors.TryResourceMayNotBeAssigned` | compiler_properties.rs:3010 | comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:318 | Comp 批 |
| 28 | `compiler.err.two.class.loaders.1` | `Errors.TwoClassLoaders1` | compiler_properties.rs:3038 | main/main.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Main.java:411 | Main/Options 批 |
| 29 | `compiler.err.two.class.loaders.2` | `Errors.TwoClassLoaders2` | compiler_properties.rs:3045 | main/main.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Main.java:408 | Main/Options 批 |
| 30 | `compiler.err.unmatched.quote` | `Errors.UnmatchedQuote` | compiler_properties.rs:3171 | main/main.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Main.java:224 | Main/Options 批 |
| 31 | `compiler.err.warnings.and.werror` | `Errors.WarningsAndWerror` | compiler_properties.rs:3297 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:568 | Main/Options 批 |
| 32 | `compiler.warn.annotation.method.not.found` | `Warnings.AnnotationMethodNotFound` | compiler_properties.rs:3344 | jvm/class_reader.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:1953 | JVM 批 |
| 33 | `compiler.warn.annotation.method.not.found.reason` | `Warnings.AnnotationMethodNotFoundReason` | compiler_properties.rs:3351 | jvm/class_reader.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:1955 | JVM 批 |
| 34 | `compiler.warn.dir.path.element.not.directory` | `Warnings.DirPathElementNotDirectory` | compiler_properties.rs:3414 | file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:1663 | FileManager/Locations 批 |
| 35 | `compiler.warn.dir.path.element.not.found` | `Warnings.DirPathElementNotFound` | compiler_properties.rs:3421 | file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:323 ; src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:1664 | FileManager/Locations 批 |
| 36 | `compiler.warn.invalid.archive.file` | `Warnings.InvalidArchiveFile` | compiler_properties.rs:3547 | file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:398 | FileManager/Locations 批 |
| 37 | `compiler.warn.invalid.path` | `Warnings.InvalidPath` | compiler_properties.rs:3554 | file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:227 | FileManager/Locations 批 |
| 38 | `compiler.warn.locn.unknown.file.on.module.path` | `Warnings.LocnUnknownFileOnModulePath` | compiler_properties.rs:3624 | file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:1493 | FileManager/Locations 批 |
| 39 | `compiler.warn.path.element.not.found` | `Warnings.PathElementNotFound` | compiler_properties.rs:3764 | file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:369 | FileManager/Locations 批 |
| 40 | `compiler.warn.proc.package.does.not.exist` | `Warnings.ProcPackageDoesNotExist` | compiler_properties.rs:3897 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1217 | Main/Options 批 |
| 41 | `compiler.warn.proc.proc-only.requested.no.procs` | `Warnings.ProcProcOnlyRequestedNoProcs` | compiler_properties.rs:3904 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1170 | Main/Options 批 |
| 42 | `compiler.warn.proc.use.implicit` | `Warnings.ProcUseImplicit` | compiler_properties.rs:3960 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1713 | Main/Options 批 |
| 43 | `compiler.warn.proc.use.proc.or.implicit` | `Warnings.ProcUseProcOrImplicit` | compiler_properties.rs:3967 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1715 | Main/Options 批 |
| 44 | `compiler.warn.unexpected.archive.file` | `Warnings.UnexpectedArchiveFile` | compiler_properties.rs:4170 | file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:392 | FileManager/Locations 批 |
| 45 | `compiler.warn.unknown.enum.constant` | `Warnings.UnknownEnumConstant` | compiler_properties.rs:4177 | jvm/class_reader.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:2038 | JVM 批 |
| 46 | `compiler.warn.unknown.enum.constant.reason` | `Warnings.UnknownEnumConstantReason` | compiler_properties.rs:4184 | jvm/class_reader.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:2033 | JVM 批 |
| 47 | `compiler.note.compressed.diags` | `Notes.CompressedDiags` | compiler_properties.rs:4231 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1720 | Main/Options 批 |
| 48 | `compiler.note.deferred.method.inst` | `Notes.DeferredMethodInst` | compiler_properties.rs:4238 | comp/infer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Infer.java:236 | Comp 批 |
| 49 | `compiler.note.mref.stat` | `Notes.MrefStat` | compiler_properties.rs:4294 | comp/lambda_to_method.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/LambdaToMethod.java:1535 | Comp 批 |
| 50 | `compiler.note.verbose.l2m.deduplicate` | `Notes.VerboseL2mDeduplicate` | compiler_properties.rs:4434 | comp/lambda_to_method.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/LambdaToMethod.java:400 | Comp 批 |
| 51 | `compiler.misc.cant.resolve.modules` | `Fragments.CantResolveModules` | compiler_properties.rs:4824 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:818 | Main/Options 批 |
| 52 | `compiler.misc.fatal.err.cant.close` | `Fragments.FatalErrCantClose` | compiler_properties.rs:5006 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1801 | Main/Options 批 |
| 53 | `compiler.misc.fatal.err.cant.locate.ctor` | `Fragments.FatalErrCantLocateCtor` | compiler_properties.rs:5013 | comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:2884 | Comp 批 |
| 54 | `compiler.misc.fatal.err.cant.locate.field` | `Fragments.FatalErrCantLocateField` | compiler_properties.rs:5020 | comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:1476 | Comp 批 |
| 55 | `compiler.misc.fatal.err.cant.locate.meth` | `Fragments.FatalErrCantLocateMeth` | compiler_properties.rs:5027 | comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:2832 | Comp 批 |
| 56 | `compiler.misc.file.does.not.contain.package` | `Fragments.FileDoesNotContainPackage` | compiler_properties.rs:5237 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:844 | Main/Options 批 |
| 57 | `compiler.misc.file.doesnt.contain.class` | `Fragments.FileDoesntContainClass` | compiler_properties.rs:5244 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:849 | Main/Options 批 |
| 58 | `compiler.misc.guard` | `Fragments.Guard` | compiler_properties.rs:5251 | comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:2957 | Comp 批 |
| 59 | `compiler.misc.illegal.signature` | `Fragments.IllegalSignature` | compiler_properties.rs:5258 | comp/lambda_to_method.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/LambdaToMethod.java:2396 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/LambdaToMethod.java:2408 | Comp 批 |
| 60 | `compiler.misc.inapplicable.method` | `Fragments.InapplicableMethod` | compiler_properties.rs:5286 | comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:4285 | Comp 批 |
| 61 | `compiler.misc.inner.cls` | `Fragments.InnerCls` | compiler_properties.rs:5447 | comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:2958 | Comp 批 |
| 62 | `compiler.misc.non.static` | `Fragments.NonStatic` | compiler_properties.rs:5853 | comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:3153 | Comp 批 |
| 63 | `compiler.misc.not.applicable.method.found` | `Fragments.NotApplicableMethodFound` | compiler_properties.rs:5881 | comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:278 | Comp 批 |
| 64 | `compiler.misc.partial.inst.sig` | `Fragments.PartialInstSig` | compiler_properties.rs:5979 | comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:267 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:3146 | Comp 批 |
| 65 | `compiler.misc.synthetic.name.conflict` | `Fragments.SyntheticNameConflict` | compiler_properties.rs:6063 | comp/lower.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Lower.java:774 | Comp 批 |
| 66 | `compiler.misc.type.parameter` | `Fragments.TypeParameter` | compiler_properties.rs:6182 | comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:735 | Comp 批 |
| 67 | `compiler.misc.unbound` | `Fragments.Unbound` | compiler_properties.rs:6238 | comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:3163 | Comp 批 |

### 待接线汇总(按上游诊断点文件)

| 上游文件 | 数量 | 对应批次 |
| --- | --- | --- |
| `main/JavaCompiler.java` | 16 | Main/Options 批 |
| `main/Option.java` | 10 | Main/Options 批 |
| `file/Locations.java` | 8 | FileManager/Locations 批 |
| `comp/Resolve.java` | 8 | Comp 批 |
| `jvm/ClassReader.java` | 5 | JVM 批 |
| `main/Main.java` | 3 | Main/Options 批 |
| `comp/LambdaToMethod.java` | 3 | Comp 批 |
| `comp/Annotate.java` | 2 | Comp 批 |
| `parser/JavacParser.java` | 2 | Parser 批 |
| `comp/Attr.java` | 2 | Comp 批 |
| `comp/Flow.java` | 2 | Comp 批 |
| `code/TypeAnnotations.java` | 1 | Code 批 |
| `file/JavacFileManager.java` | 1 | FileManager/Locations 批 |
| `file/BaseFileManager.java` | 1 | FileManager/Locations 批 |
| `comp/Infer.java` | 1 | Comp 批 |
| `comp/Lower.java` | 1 | Comp 批 |
| `comp/Check.java` | 1 | Comp 批 |

## 功能未移植清单(40 条,上游诊断点所在文件本地无对应实现)

| # | 资源键 | 工厂 | 本地上游诊断点 | 未移植批次 |
| --- | --- | --- | --- | --- |
| 1 | `compiler.err.bad.name.for.option` | `Errors.BadNameForOption` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:743 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:767 | Main/Options 批 |
| 2 | `compiler.err.file.not.directory` | `Errors.FileNotDirectory` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:866 | Main/Options 批 |
| 3 | `compiler.err.invalid.flag` | `Errors.InvalidFlag` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:366 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:395 | Main/Options 批 |
| 4 | `compiler.err.module.not.found.in.module.source.path` | `Errors.ModuleNotFoundInModuleSourcePath` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:422 | Main/Options 批 |
| 5 | `compiler.err.modulesourcepath.must.be.specified.with.dash.m.option` | `Errors.ModulesourcepathMustBeSpecifiedWithDashMOption` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:415 | Main/Options 批 |
| 6 | `compiler.err.multi-module.outdir.cannot.be.exploded.module` | `Errors.MultiModuleOutdirCannotBeExplodedModule` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:488 | Main/Options 批 |
| 7 | `compiler.err.no.output.dir` | `Errors.NoOutputDir` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:619 | Main/Options 批 |
| 8 | `compiler.err.no.source.files` | `Errors.NoSourceFiles` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:463 | Main/Options 批 |
| 9 | `compiler.err.no.source.files.classes` | `Errors.NoSourceFilesClasses` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:461 | Main/Options 批 |
| 10 | `compiler.err.option.not.allowed.with.target` | `Errors.OptionNotAllowedWithTarget` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:597 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:605 | Main/Options 批 |
| 11 | `compiler.err.option.removed.source` | `Errors.OptionRemovedSource` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:582 | Main/Options 批 |
| 12 | `compiler.err.option.removed.target` | `Errors.OptionRemovedTarget` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:589 | Main/Options 批 |
| 13 | `compiler.err.output.dir.must.be.specified.with.dash.m.option` | `Errors.OutputDirMustBeSpecifiedWithDashMOption` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:413 | Main/Options 批 |
| 14 | `compiler.err.plugin.not.found` | `Errors.PluginNotFound` | src/jdk.compiler/share/classes/com/sun/tools/javac/api/BasicJavacTask.java:238 | API 批 |
| 15 | `compiler.err.preview.not.latest` | `Errors.PreviewNotLatest` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:542 | Main/Options 批 |
| 16 | `compiler.err.preview.without.source.or.release` | `Errors.PreviewWithoutSourceOrRelease` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:538 | Main/Options 批 |
| 17 | `compiler.err.proc.messager` | `Errors.ProcMessager` | src/jdk.compiler/share/classes/com/sun/tools/javac/api/JavacTrees.java:1266 ; src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacMessager.java:121 | API 批 |
| 18 | `compiler.err.processorpath.no.processormodulepath` | `Errors.ProcessorpathNoProcessormodulepath` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:625 | Main/Options 批 |
| 19 | `compiler.err.profile.bootclasspath.conflict` | `Errors.ProfileBootclasspathConflict` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:558 | Main/Options 批 |
| 20 | `compiler.err.release.bootclasspath.conflict` | `Errors.ReleaseBootclasspathConflict` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:298 | Main/Options 批 |
| 21 | `compiler.err.sourcepath.modulesourcepath.conflict` | `Errors.SourcepathModulesourcepathConflict` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:563 | Main/Options 批 |
| 22 | `compiler.err.unsupported.release.version` | `Errors.UnsupportedReleaseVersion` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:311 | Main/Options 批 |
| 23 | `compiler.warn.addopens.ignored` | `Warnings.AddopensIgnored` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:640 | Main/Options 批 |
| 24 | `compiler.warn.bad.name.for.option` | `Warnings.BadNameForOption` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:660 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:665 | Main/Options 批 |
| 25 | `compiler.warn.doclint.not.available` | `Warnings.DoclintNotAvailable` | src/jdk.compiler/share/classes/com/sun/tools/javac/api/BasicJavacTask.java:265 | API 批 |
| 26 | `compiler.warn.option.obsolete.source` | `Warnings.OptionObsoleteSource` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:584 | Main/Options 批 |
| 27 | `compiler.warn.option.obsolete.suppression` | `Warnings.OptionObsoleteSuppression` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:629 | Main/Options 批 |
| 28 | `compiler.warn.option.obsolete.target` | `Warnings.OptionObsoleteTarget` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:591 | Main/Options 批 |
| 29 | `compiler.warn.option.parameters.unsupported` | `Warnings.OptionParametersUnsupported` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:613 | Main/Options 批 |
| 30 | `compiler.warn.outdir.is.in.exploded.module` | `Warnings.OutdirIsInExplodedModule` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:497 | Main/Options 批 |
| 31 | `compiler.warn.proc.messager` | `Warnings.ProcMessager` | src/jdk.compiler/share/classes/com/sun/tools/javac/api/JavacTrees.java:1270 ; src/jdk.compiler/share/classes/com/sun/tools/javac/api/JavacTrees.java:1274 | API 批 |
| 32 | `compiler.warn.profile.target.conflict` | `Warnings.ProfileTargetConflict` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:552 | Main/Options 批 |
| 33 | `compiler.warn.source.no.bootclasspath` | `Warnings.SourceNoBootclasspath` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:571 | Main/Options 批 |
| 34 | `compiler.warn.source.no.system.modules.path` | `Warnings.SourceNoSystemModulesPath` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:574 | Main/Options 批 |
| 35 | `compiler.warn.source.target.conflict` | `Warnings.SourceTargetConflict` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:525 | Main/Options 批 |
| 36 | `compiler.warn.target.default.source.conflict` | `Warnings.TargetDefaultSourceConflict` | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:523 | Main/Options 批 |
| 37 | `compiler.note.multiple.elements` | `Notes.MultipleElements` | src/jdk.compiler/share/classes/com/sun/tools/javac/model/JavacElements.java:236 | Model 批 |
| 38 | `compiler.note.proc.messager` | `Notes.ProcMessager` | src/jdk.compiler/share/classes/com/sun/tools/javac/api/JavacTrees.java:1278 ; src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacMessager.java:135 | API 批 |
| 39 | `compiler.misc.where.fresh.typevar` | `Fragments.WhereFreshTypevar` | src/jdk.compiler/share/classes/com/sun/tools/javac/util/RichDiagnosticFormatter.java:585 | Util 批 |
| 40 | `compiler.misc.where.intersection` | `Fragments.WhereIntersection` | src/jdk.compiler/share/classes/com/sun/tools/javac/util/RichDiagnosticFormatter.java:524 | Util 批 |

## 全量资源清单(935)

| # | 资源键 | 工厂 | 状态 | 本地接线/本地目标 | 上游诊断点(最多 2 处) | 备注 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `compiler.err.abstract.cant.be.accessed.directly` | `Errors.AbstractCantBeAccessedDirectly` | 已接线 | javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:2667 |  |
| 2 | `compiler.err.abstract.cant.be.instantiated` | `Errors.AbstractCantBeInstantiated` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/comp/check.rs(等 3 个文件) | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2784 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:745 |  |
| 3 | `compiler.err.abstract.meth.cant.have.body` | `Errors.AbstractMethCantHaveBody` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/resources/mod.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1193 |  |
| 4 | `compiler.err.add.exports.with.release` | `Errors.AddExportsWithRelease` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1652 |  |
| 5 | `compiler.err.add.reads.with.release` | `Errors.AddReadsWithRelease` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1732 |  |
| 6 | `compiler.err.addmods.all.module.path.invalid` | `Errors.AddmodsAllModulePathInvalid` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:622 |  |
| 7 | `compiler.err.already.annotated` | `Errors.AlreadyAnnotated` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:262 |  |
| 8 | `compiler.err.already.defined` | `Errors.AlreadyDefined` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:360 |  |
| 9 | `compiler.err.already.defined.in.clinit` | `Errors.AlreadyDefinedInClinit` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:347 |  |
| 10 | `compiler.err.already.defined.single.import` | `Errors.AlreadyDefinedSingleImport` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3844 |  |
| 11 | `compiler.err.already.defined.static.single.import` | `Errors.AlreadyDefinedStaticSingleImport` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3846 |  |
| 12 | `compiler.err.already.defined.this.unit` | `Errors.AlreadyDefinedThisUnit` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3851 |  |
| 13 | `compiler.err.annotation.decl.not.allowed.here` | `Errors.AnnotationDeclNotAllowedHere` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1273 |  |
| 14 | `compiler.err.annotation.missing.default.value` | `Errors.AnnotationMissingDefaultValue` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3503 |  |
| 15 | `compiler.err.annotation.missing.default.value.1` | `Errors.AnnotationMissingDefaultValue1` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3502 |  |
| 16 | `compiler.err.annotation.not.valid.for.type` | `Errors.AnnotationNotValidForType` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:588 |  |
| 17 | `compiler.err.annotation.type.not.applicable` | `Errors.AnnotationTypeNotApplicable` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2960 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3065 |  |
| 18 | `compiler.err.annotation.type.not.applicable.to.type` | `Errors.AnnotationTypeNotApplicableToType` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5805 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3094 |  |
| 19 | `compiler.err.annotation.value.must.be.annotation` | `Errors.AnnotationValueMustBeAnnotation` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:580 |  |
| 20 | `compiler.err.annotation.value.must.be.class.literal` | `Errors.AnnotationValueMustBeClassLiteral` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:659 |  |
| 21 | `compiler.err.annotation.value.must.be.name.value` | `Errors.AnnotationValueMustBeNameValue` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:511 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:517 |  |
| 22 | `compiler.err.annotation.value.not.allowable.type` | `Errors.AnnotationValueNotAllowableType` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:563 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:623 |  |
| 23 | `compiler.err.anon.class.impl.intf.no.args` | `Errors.AnonClassImplIntfNoArgs` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2790 |  |
| 24 | `compiler.err.anon.class.impl.intf.no.qual.for.new` | `Errors.AnonClassImplIntfNoQualForNew` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2751 |  |
| 25 | `compiler.err.anon.class.impl.intf.no.typeargs` | `Errors.AnonClassImplIntfNoTypeargs` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2793 |  |
| 26 | `compiler.err.anonymous.diamond.method.does.not.override.superclass` | `Errors.AnonymousDiamondMethodDoesNotOverrideSuperclass` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2146 |  |
| 27 | `compiler.err.array.and.receiver` | `Errors.ArrayAndReceiver` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3558 |  |
| 28 | `compiler.err.array.and.varargs` | `Errors.ArrayAndVarargs` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:373 |  |
| 29 | `compiler.err.array.dimension.missing` | `Errors.ArrayDimensionMissing` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2434 |  |
| 30 | `compiler.err.array.req.but.found` | `Errors.ArrayReqButFound` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4176 |  |
| 31 | `compiler.err.assert.as.identifier` | `Errors.AssertAsIdentifier` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:580 |  |
| 32 | `compiler.err.attribute.value.must.be.constant` | `Errors.AttributeValueMustBeConstant` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:672 |  |
| 33 | `compiler.err.bad.functional.intf.anno` | `Errors.BadFunctionalIntfAnno` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3080 |  |
| 34 | `compiler.err.bad.functional.intf.anno.1` | `Errors.BadFunctionalIntfAnno1` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3082 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3997 |  |
| 35 | `compiler.err.bad.initializer` | `Errors.BadInitializer` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3164 |  |
| 36 | `compiler.err.bad.name.for.option` | `Errors.BadNameForOption` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:743 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:767 |  |
| 37 | `compiler.err.bad.value.for.option` | `Errors.BadValueForOption` | 已接线 | javac-core/src/main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:243 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:464 |  |
| 38 | `compiler.err.break.outside.switch.expression` | `Errors.BreakOutsideSwitchExpression` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2391 |  |
| 39 | `compiler.err.break.outside.switch.loop` | `Errors.BreakOutsideSwitchLoop` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2411 |  |
| 40 | `compiler.err.call.must.be.first.stmt.in.ctor` | `Errors.CallMustBeFirstStmtInCtor` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2639 |  |
| 41 | `compiler.err.call.to.super.not.allowed.in.enum.ctor` | `Errors.CallToSuperNotAllowedInEnumCtor` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1216 |  |
| 42 | `compiler.err.cannot.create.array.with.diamond` | `Errors.CannotCreateArrayWithDiamond` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2357 |  |
| 43 | `compiler.err.cannot.create.array.with.type.arguments` | `Errors.CannotCreateArrayWithTypeArguments` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2370 |  |
| 44 | `compiler.err.cannot.generate.class` | `Errors.CannotGenerateClass` | 已接线 | javac-core/src/comp/lower.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/LambdaToMethod.java:2396 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/LambdaToMethod.java:2408 |  |
| 45 | `compiler.err.cant.access` | `Errors.CantAccess` | 已接线 | javac-core/src/comp/annotate.rs, javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:309 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:394 |  |
| 46 | `compiler.err.cant.apply.diamond` | `Errors.CantApplyDiamond` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 47 | `compiler.err.cant.apply.diamond.1` | `Errors.CantApplyDiamond1` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2941 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:850 |  |
| 48 | `compiler.err.cant.apply.symbol` | `Errors.CantApplySymbol` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 49 | `compiler.err.cant.apply.symbols` | `Errors.CantApplySymbols` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 50 | `compiler.err.cant.assign.val.to.final.var` | `Errors.CantAssignValToFinalVar` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:320 |  |
| 51 | `compiler.err.cant.assign.val.to.this` | `Errors.CantAssignValToThis` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:310 |  |
| 52 | `compiler.err.cant.attach.type.annotations` | `Errors.CantAttachTypeAnnotations` | 功能已具备待接线 | jvm/class_reader.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:2218 |  |
| 53 | `compiler.err.cant.deref` | `Errors.CantDeref` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4474 |  |
| 54 | `compiler.err.cant.extend.intf.annotation` | `Errors.CantExtendIntfAnnotation` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5422 |  |
| 55 | `compiler.err.cant.infer.local.var.type` | `Errors.CantInferLocalVarType` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1262 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1269 |  |
| 56 | `compiler.err.cant.inherit.diff.arg` | `Errors.CantInheritDiffArg` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2828 |  |
| 57 | `compiler.err.cant.inherit.from.anon` | `Errors.CantInheritFromAnon` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:907 |  |
| 58 | `compiler.err.cant.inherit.from.final` | `Errors.CantInheritFromFinal` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:935 |  |
| 59 | `compiler.err.cant.inherit.from.sealed` | `Errors.CantInheritFromSealed` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5316 |  |
| 60 | `compiler.err.cant.read.file` | `Errors.CantReadFile` | 范围外(PLAN §3.2(launcher 裁剪)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/launcher/Main.java:279 |  |
| 61 | `compiler.err.cant.ref.before.ctor.called` | `Errors.CantRefBeforeCtorCalled` | 已接线 | javac-core/src/comp/check.rs, javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:336 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:3851 |  |
| 62 | `compiler.err.cant.ref.non.effectively.final.var` | `Errors.CantRefNonEffectivelyFinalVar` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:2961 |  |
| 63 | `compiler.err.cant.resolve` | `Errors.CantResolve` | 功能已具备待接线 | comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:551 |  |
| 64 | `compiler.err.cant.resolve.args` | `Errors.CantResolveArgs` | 无上游触发 | — | — | 上游无引用 |
| 65 | `compiler.err.cant.resolve.args.params` | `Errors.CantResolveArgsParams` | 无上游触发 | — | — | 上游无引用 |
| 66 | `compiler.err.cant.resolve.location` | `Errors.CantResolveLocation` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:4014 |  |
| 67 | `compiler.err.cant.resolve.location.args` | `Errors.CantResolveLocationArgs` | 无上游触发 | — | — | 上游无引用 |
| 68 | `compiler.err.cant.resolve.location.args.params` | `Errors.CantResolveLocationArgsParams` | 无上游触发 | — | — | 上游无引用 |
| 69 | `compiler.err.cant.select.static.class.from.param.type` | `Errors.CantSelectStaticClassFromParamType` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1497 |  |
| 70 | `compiler.err.cant.type.annotate.scoping` | `Errors.CantTypeAnnotateScoping` | 已接线 | javac-core/src/code/type_annotations.rs, javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/TypeAnnotations.java:504 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5753 |  |
| 71 | `compiler.err.cant.type.annotate.scoping.1` | `Errors.CantTypeAnnotateScoping1` | 功能已具备待接线 | code/type_annotations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/TypeAnnotations.java:500 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5747 |  |
| 72 | `compiler.err.catch.without.try` | `Errors.CatchWithoutTry` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2912 |  |
| 73 | `compiler.err.clash.with.pkg.of.same.name` | `Errors.ClashWithPkgOfSameName` | 已接线 | javac-core/src/comp/type_enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/TypeEnter.java:898 |  |
| 74 | `compiler.err.class.cant.write` | `Errors.ClassCantWrite` | 功能已具备待接线 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1626 |  |
| 75 | `compiler.err.class.in.module.cant.extend.sealed.in.diff.module` | `Errors.ClassInModuleCantExtendSealedInDiffModule` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5270 |  |
| 76 | `compiler.err.class.in.unnamed.module.cant.extend.sealed.in.diff.package` | `Errors.ClassInUnnamedModuleCantExtendSealedInDiffPackage` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5265 |  |
| 77 | `compiler.err.class.not.allowed` | `Errors.ClassNotAllowed` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2603 |  |
| 78 | `compiler.err.class.public.should.be.in.file` | `Errors.ClassPublicShouldBeInFile` | 已接线 | javac-core/src/comp/enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Enter.java:439 |  |
| 79 | `compiler.err.concrete.inheritance.conflict` | `Errors.ConcreteInheritanceConflict` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1963 |  |
| 80 | `compiler.err.conflicting.exports` | `Errors.ConflictingExports` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:860 |  |
| 81 | `compiler.err.conflicting.exports.to.module` | `Errors.ConflictingExportsToModule` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:875 |  |
| 82 | `compiler.err.conflicting.opens` | `Errors.ConflictingOpens` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:919 |  |
| 83 | `compiler.err.conflicting.opens.to.module` | `Errors.ConflictingOpensToModule` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:934 |  |
| 84 | `compiler.err.const.expr.req` | `Errors.ConstExprReq` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1757 |  |
| 85 | `compiler.err.constant.label.not.compatible` | `Errors.ConstantLabelNotCompatible` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1760 |  |
| 86 | `compiler.err.cont.outside.loop` | `Errors.ContOutsideLoop` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2409 |  |
| 87 | `compiler.err.continue.outside.switch.expression` | `Errors.ContinueOutsideSwitchExpression` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2393 |  |
| 88 | `compiler.err.cyclic.annotation.element` | `Errors.CyclicAnnotationElement` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3630 |  |
| 89 | `compiler.err.cyclic.inheritance` | `Errors.CyclicInheritance` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2390 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2442 |  |
| 90 | `compiler.err.cyclic.requires` | `Errors.CyclicRequires` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1779 |  |
| 91 | `compiler.err.dc.bad.entity` | `Errors.DcBadEntity` | 无上游触发 | — | — | 上游无引用 |
| 92 | `compiler.err.dc.bad.inline.tag` | `Errors.DcBadInlineTag` | 无上游触发 | — | — | 上游无引用 |
| 93 | `compiler.err.dc.gt.expected` | `Errors.DcGtExpected` | 无上游触发 | — | — | 上游无引用 |
| 94 | `compiler.err.dc.identifier.expected` | `Errors.DcIdentifierExpected` | 无上游触发 | — | — | 上游无引用 |
| 95 | `compiler.err.dc.malformed.html` | `Errors.DcMalformedHtml` | 无上游触发 | — | — | 上游无引用 |
| 96 | `compiler.err.dc.missing.semicolon` | `Errors.DcMissingSemicolon` | 无上游触发 | — | — | 上游无引用 |
| 97 | `compiler.err.dc.no.content` | `Errors.DcNoContent` | 无上游触发 | — | — | 上游无引用 |
| 98 | `compiler.err.dc.no.tag.name` | `Errors.DcNoTagName` | 无上游触发 | — | — | 上游无引用 |
| 99 | `compiler.err.dc.ref.annotations.not.allowed` | `Errors.DcRefAnnotationsNotAllowed` | 无上游触发 | — | — | 上游无引用 |
| 100 | `compiler.err.dc.ref.bad.parens` | `Errors.DcRefBadParens` | 无上游触发 | — | — | 上游无引用 |
| 101 | `compiler.err.dc.ref.syntax.error` | `Errors.DcRefSyntaxError` | 无上游触发 | — | — | 上游无引用 |
| 102 | `compiler.err.dc.ref.unexpected.input` | `Errors.DcRefUnexpectedInput` | 无上游触发 | — | — | 上游无引用 |
| 103 | `compiler.err.dc.unexpected.content` | `Errors.DcUnexpectedContent` | 无上游触发 | — | — | 上游无引用 |
| 104 | `compiler.err.dc.unterminated.inline.tag` | `Errors.DcUnterminatedInlineTag` | 无上游触发 | — | — | 上游无引用 |
| 105 | `compiler.err.dc.unterminated.signature` | `Errors.DcUnterminatedSignature` | 无上游触发 | — | — | 上游无引用 |
| 106 | `compiler.err.dc.unterminated.string` | `Errors.DcUnterminatedString` | 无上游触发 | — | — | 上游无引用 |
| 107 | `compiler.err.default.allowed.in.intf.annotation.member` | `Errors.DefaultAllowedInIntfAnnotationMember` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1184 |  |
| 108 | `compiler.err.default.overrides.object.member` | `Errors.DefaultOverridesObjectMember` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1019 |  |
| 109 | `compiler.err.does.not.override.abstract` | `Errors.DoesNotOverrideAbstract` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2257 |  |
| 110 | `compiler.err.doesnt.exist` | `Errors.DoesntExist` | 已接线 | javac-core/src/comp/check.rs, javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:4032 |  |
| 111 | `compiler.err.dot.class.expected` | `Errors.DotClassExpected` | 功能已具备待接线 | parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2264 |  |
| 112 | `compiler.err.duplicate.annotation.invalid.repeated` | `Errors.DuplicateAnnotationInvalidRepeated` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:805 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:830 |  |
| 113 | `compiler.err.duplicate.annotation.member.value` | `Errors.DuplicateAnnotationMemberValue` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3484 |  |
| 114 | `compiler.err.duplicate.annotation.missing.container` | `Errors.DuplicateAnnotationMissingContainer` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:885 |  |
| 115 | `compiler.err.duplicate.case.label` | `Errors.DuplicateCaseLabel` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1717 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1729 |  |
| 116 | `compiler.err.duplicate.class` | `Errors.DuplicateClass` | 已接线 | javac-core/src/comp/enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Enter.java:548 |  |
| 117 | `compiler.err.duplicate.default.label` | `Errors.DuplicateDefaultLabel` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1770 |  |
| 118 | `compiler.err.duplicate.module` | `Errors.DuplicateModule` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:327 |  |
| 119 | `compiler.err.duplicate.module.on.path` | `Errors.DuplicateModuleOnPath` | 已接线 | javac-core/src/code/module_finder.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/ModuleFinder.java:340 |  |
| 120 | `compiler.err.duplicate.provides` | `Errors.DuplicateProvides` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1125 |  |
| 121 | `compiler.err.duplicate.requires` | `Errors.DuplicateRequires` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:798 |  |
| 122 | `compiler.err.duplicate.total.pattern` | `Errors.DuplicateTotalPattern` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1791 |  |
| 123 | `compiler.err.duplicate.uses` | `Errors.DuplicateUses` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1160 |  |
| 124 | `compiler.err.else.without.if` | `Errors.ElseWithoutIf` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2906 |  |
| 125 | `compiler.err.empty.A.argument` | `Errors.EmptyAArgument` | 功能已具备待接线 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:440 |  |
| 126 | `compiler.err.empty.char.lit` | `Errors.EmptyCharLit` | 已接线 | javac-core/src/parser/java_tokenizer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:977 |  |
| 127 | `compiler.err.encl.class.required` | `Errors.EnclClassRequired` | 已接线 | javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:3799 |  |
| 128 | `compiler.err.enum.annotation.must.be.enum.constant` | `Errors.EnumAnnotationMustBeEnumConstant` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:634 |  |
| 129 | `compiler.err.enum.as.identifier` | `Errors.EnumAsIdentifier` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:584 |  |
| 130 | `compiler.err.enum.cant.be.instantiated` | `Errors.EnumCantBeInstantiated` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2775 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:748 |  |
| 131 | `compiler.err.enum.constant.expected` | `Errors.EnumConstantExpected` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:4107 |  |
| 132 | `compiler.err.enum.constant.not.expected` | `Errors.EnumConstantNotExpected` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:4086 |  |
| 133 | `compiler.err.enum.label.must.be.unqualified.enum` | `Errors.EnumLabelMustBeUnqualifiedEnum` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1727 |  |
| 134 | `compiler.err.enum.no.finalize` | `Errors.EnumNoFinalize` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2107 |  |
| 135 | `compiler.err.enum.no.subclassing` | `Errors.EnumNoSubclassing` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5350 |  |
| 136 | `compiler.err.enum.types.not.extensible` | `Errors.EnumTypesNotExtensible` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5356 |  |
| 137 | `compiler.err.error` | `Errors.Error` | 已接线 | javac-core/src/code/preview.rs, javac-core/src/comp/annotate.rs(等 11 个文件) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Main.java:162 ; src/jdk.compiler/share/classes/com/sun/tools/javac/util/JavacMessages.java:248 |  |
| 138 | `compiler.err.error.reading.file` | `Errors.ErrorReadingFile` | 功能已具备待接线 | file/javac_file_manager.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/JavacFileManager.java:421 ; src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:431 |  |
| 139 | `compiler.err.error.writing.file` | `Errors.ErrorWritingFile` | 功能已具备待接线 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:560 |  |
| 140 | `compiler.err.except.already.caught` | `Errors.ExceptAlreadyCaught` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:1375 |  |
| 141 | `compiler.err.except.never.thrown.in.try` | `Errors.ExceptNeverThrownInTry` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:1379 |  |
| 142 | `compiler.err.expected` | `Errors.Expected` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2040 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2126 |  |
| 143 | `compiler.err.expected.module` | `Errors.ExpectedModule` | 已接线 | javac-core/src/comp/modules.rs, javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:348 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3697 |  |
| 144 | `compiler.err.expected.module.or.open` | `Errors.ExpectedModuleOrOpen` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3890 |  |
| 145 | `compiler.err.expected.str` | `Errors.ExpectedStr` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3804 |  |
| 146 | `compiler.err.expected2` | `Errors.Expected2` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:1531 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2382 |  |
| 147 | `compiler.err.expected3` | `Errors.Expected3` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:1484 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2166 |  |
| 148 | `compiler.err.expected4` | `Errors.Expected4` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3893 |  |
| 149 | `compiler.err.expression.not.allowable.as.annotation.value` | `Errors.ExpressionNotAllowableAsAnnotationValue` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:603 |  |
| 150 | `compiler.err.feature.not.supported.in.source` | `Errors.FeatureNotSupportedInSource` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:286 |  |
| 151 | `compiler.err.feature.not.supported.in.source.plural` | `Errors.FeatureNotSupportedInSourcePlural` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:287 |  |
| 152 | `compiler.err.file.not.directory` | `Errors.FileNotDirectory` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:866 |  |
| 153 | `compiler.err.file.not.file` | `Errors.FileNotFile` | 功能已具备待接线 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:769 |  |
| 154 | `compiler.err.file.not.found` | `Errors.FileNotFound` | 功能已具备待接线 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/launcher/Main.java:215 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/Main.java:227 |  |
| 155 | `compiler.err.file.patched.and.msp` | `Errors.FilePatchedAndMsp` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:381 |  |
| 156 | `compiler.err.file.sb.on.source.or.patch.path.for.module` | `Errors.FileSbOnSourceOrPatchPathForModule` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:560 |  |
| 157 | `compiler.err.final.parameter.may.not.be.assigned` | `Errors.FinalParameterMayNotBeAssigned` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:1890 |  |
| 158 | `compiler.err.finally.without.try` | `Errors.FinallyWithoutTry` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2910 |  |
| 159 | `compiler.err.first.statement.must.be.call.to.another.constructor` | `Errors.FirstStatementMustBeCallToAnotherConstructor` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1093 |  |
| 160 | `compiler.err.flows.through.from.pattern` | `Errors.FlowsThroughFromPattern` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:4313 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:4318 |  |
| 161 | `compiler.err.flows.through.to.pattern` | `Errors.FlowsThroughToPattern` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:4331 |  |
| 162 | `compiler.err.foreach.not.applicable.to.type` | `Errors.ForeachNotApplicableToType` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1547 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1562 |  |
| 163 | `compiler.err.fp.number.too.large` | `Errors.FpNumberTooLarge` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:689 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:708 |  |
| 164 | `compiler.err.fp.number.too.small` | `Errors.FpNumberTooSmall` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:687 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:706 |  |
| 165 | `compiler.err.generic.array.creation` | `Errors.GenericArrayCreation` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3065 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:755 |  |
| 166 | `compiler.err.generic.throwable` | `Errors.GenericThrowable` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5461 |  |
| 167 | `compiler.err.icls.cant.have.static.decl` | `Errors.IclsCantHaveStaticDecl` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5484 |  |
| 168 | `compiler.err.illegal.argument.for.option` | `Errors.IllegalArgumentForOption` | 功能已具备待接线 | file/base_file_manager.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/BaseFileManager.java:293 |  |
| 169 | `compiler.err.illegal.array.creation.both.dimension.and.initialization` | `Errors.IllegalArrayCreationBothDimensionAndInitialization` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2470 |  |
| 170 | `compiler.err.illegal.char` | `Errors.IllegalChar` | 已接线 | javac-core/src/parser/java_tokenizer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:1033 |  |
| 171 | `compiler.err.illegal.char.for.encoding` | `Errors.IllegalCharForEncoding` | 已接线 | javac-core/src/file/base_file_manager.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/BaseFileManager.java:361 |  |
| 172 | `compiler.err.illegal.combination.of.modifiers` | `Errors.IllegalCombinationOfModifiers` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:833 |  |
| 173 | `compiler.err.illegal.default.super.call` | `Errors.IllegalDefaultSuperCall` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4803 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:3758 |  |
| 174 | `compiler.err.illegal.dot` | `Errors.IllegalDot` | 已接线 | javac-core/src/parser/java_tokenizer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:871 |  |
| 175 | `compiler.err.illegal.enum.static.ref` | `Errors.IllegalEnumStaticRef` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4757 |  |
| 176 | `compiler.err.illegal.esc.char` | `Errors.IllegalEscChar` | 已接线 | javac-core/src/parser/java_tokenizer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:378 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:383 |  |
| 177 | `compiler.err.illegal.forward.ref` | `Errors.IllegalForwardRef` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:915 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4674 |  |
| 178 | `compiler.err.illegal.initializer.for.type` | `Errors.IllegalInitializerForType` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3055 |  |
| 179 | `compiler.err.illegal.line.end.in.char.lit` | `Errors.IllegalLineEndInCharLit` | 已接线 | javac-core/src/parser/java_tokenizer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:980 |  |
| 180 | `compiler.err.illegal.nonascii.digit` | `Errors.IllegalNonasciiDigit` | 已接线 | javac-core/src/parser/unicode_reader.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/UnicodeReader.java:532 |  |
| 181 | `compiler.err.illegal.parenthesized.expression` | `Errors.IllegalParenthesizedExpression` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3927 |  |
| 182 | `compiler.err.illegal.qual.not.icls` | `Errors.IllegalQualNotIcls` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2533 |  |
| 183 | `compiler.err.illegal.record.component.name` | `Errors.IllegalRecordComponentName` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1326 |  |
| 184 | `compiler.err.illegal.ref.to.restricted.type` | `Errors.IllegalRefToRestrictedType` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 185 | `compiler.err.illegal.self.ref` | `Errors.IllegalSelfRef` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4674 |  |
| 186 | `compiler.err.illegal.start.of.expr` | `Errors.IllegalStartOfExpr` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:480 |  |
| 187 | `compiler.err.illegal.start.of.stmt` | `Errors.IllegalStartOfStmt` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2596 |  |
| 188 | `compiler.err.illegal.start.of.type` | `Errors.IllegalStartOfType` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:323 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:482 |  |
| 189 | `compiler.err.illegal.static.intf.meth.call` | `Errors.IllegalStaticIntfMethCall` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4814 |  |
| 190 | `compiler.err.illegal.text.block.open` | `Errors.IllegalTextBlockOpen` | 已接线 | javac-core/src/parser/java_tokenizer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:415 |  |
| 191 | `compiler.err.illegal.underscore` | `Errors.IllegalUnderscore` | 已接线 | javac-core/src/parser/java_tokenizer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:491 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:493 |  |
| 192 | `compiler.err.illegal.unicode.esc` | `Errors.IllegalUnicodeEsc` | 已接线 | javac-core/src/parser/unicode_reader.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/UnicodeReader.java:281 |  |
| 193 | `compiler.err.import.requires.canonical` | `Errors.ImportRequiresCanonical` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3862 |  |
| 194 | `compiler.err.improperly.formed.type.inner.raw.param` | `Errors.ImproperlyFormedTypeInnerRawParam` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1461 |  |
| 195 | `compiler.err.improperly.formed.type.param.missing` | `Errors.ImproperlyFormedTypeParamMissing` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1487 |  |
| 196 | `compiler.err.incomparable.types` | `Errors.IncomparableTypes` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4035 |  |
| 197 | `compiler.err.incompatible.thrown.types.in.mref` | `Errors.IncompatibleThrownTypesInMref` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3195 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3811 |  |
| 198 | `compiler.err.incorrect.constructor.receiver.name` | `Errors.IncorrectConstructorReceiverName` | 已接线 | javac-core/src/comp/member_enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/MemberEnter.java:331 |  |
| 199 | `compiler.err.incorrect.constructor.receiver.type` | `Errors.IncorrectConstructorReceiverType` | 已接线 | javac-core/src/comp/member_enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/MemberEnter.java:330 |  |
| 200 | `compiler.err.incorrect.receiver.name` | `Errors.IncorrectReceiverName` | 已接线 | javac-core/src/comp/member_enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/MemberEnter.java:337 |  |
| 201 | `compiler.err.incorrect.receiver.type` | `Errors.IncorrectReceiverType` | 已接线 | javac-core/src/comp/member_enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/MemberEnter.java:336 |  |
| 202 | `compiler.err.initializer.must.be.able.to.complete.normally` | `Errors.InitializerMustBeAbleToCompleteNormally` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:493 |  |
| 203 | `compiler.err.initializer.not.allowed` | `Errors.InitializerNotAllowed` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:4273 |  |
| 204 | `compiler.err.instance.initializer.not.allowed.in.records` | `Errors.InstanceInitializerNotAllowedInRecords` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:4275 |  |
| 205 | `compiler.err.instanceof.pattern.no.subtype` | `Errors.InstanceofPatternNoSubtype` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4087 |  |
| 206 | `compiler.err.instanceof.reifiable.not.safe` | `Errors.InstanceofReifiableNotSafe` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4125 |  |
| 207 | `compiler.err.int.number.too.large` | `Errors.IntNumberTooLarge` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:663 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:672 |  |
| 208 | `compiler.err.intf.annotation.cant.have.type.params` | `Errors.IntfAnnotationCantHaveTypeParams` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5425 |  |
| 209 | `compiler.err.intf.annotation.member.clash` | `Errors.IntfAnnotationMemberClash` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2912 |  |
| 210 | `compiler.err.intf.annotation.members.cant.have.params` | `Errors.IntfAnnotationMembersCantHaveParams` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1033 |  |
| 211 | `compiler.err.intf.annotation.members.cant.have.type.params` | `Errors.IntfAnnotationMembersCantHaveTypeParams` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1166 |  |
| 212 | `compiler.err.intf.expected.here` | `Errors.IntfExpectedHere` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:922 |  |
| 213 | `compiler.err.intf.meth.cant.have.body` | `Errors.IntfMethCantHaveBody` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1191 |  |
| 214 | `compiler.err.intf.not.allowed.here` | `Errors.IntfNotAllowedHere` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1273 |  |
| 215 | `compiler.err.invalid.A.key` | `Errors.InvalidAKey` | 功能已具备待接线 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:445 |  |
| 216 | `compiler.err.invalid.accessor.method.in.record` | `Errors.InvalidAccessorMethodInRecord` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1066 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1069 |  |
| 217 | `compiler.err.invalid.annotation.member.type` | `Errors.InvalidAnnotationMemberType` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2894 |  |
| 218 | `compiler.err.invalid.binary.number` | `Errors.InvalidBinaryNumber` | 已接线 | javac-core/src/parser/java_tokenizer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:636 |  |
| 219 | `compiler.err.invalid.canonical.constructor.in.record` | `Errors.InvalidCanonicalConstructorInRecord` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1106 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1111 |  |
| 220 | `compiler.err.invalid.flag` | `Errors.InvalidFlag` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:366 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:395 |  |
| 221 | `compiler.err.invalid.hex.number` | `Errors.InvalidHexNumber` | 已接线 | javac-core/src/parser/java_tokenizer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:590 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:639 |  |
| 222 | `compiler.err.invalid.lambda.parameter.declaration` | `Errors.InvalidLambdaParameterDeclaration` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:1896 |  |
| 223 | `compiler.err.invalid.meth.decl.ret.type.req` | `Errors.InvalidMethDeclRetTypeReq` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:4310 |  |
| 224 | `compiler.err.invalid.module.directive` | `Errors.InvalidModuleDirective` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3814 |  |
| 225 | `compiler.err.invalid.module.specifier` | `Errors.InvalidModuleSpecifier` | 功能已具备待接线 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:683 |  |
| 226 | `compiler.err.invalid.mref` | `Errors.InvalidMref` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3562 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3647 |  |
| 227 | `compiler.err.invalid.path` | `Errors.InvalidPath` | 功能已具备待接线 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:773 |  |
| 228 | `compiler.err.invalid.permits.clause` | `Errors.InvalidPermitsClause` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5248 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5258 |  |
| 229 | `compiler.err.invalid.profile` | `Errors.InvalidProfile` | 功能已具备待接线 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:385 |  |
| 230 | `compiler.err.invalid.repeatable.annotation` | `Errors.InvalidRepeatableAnnotation` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:913 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:919 |  |
| 231 | `compiler.err.invalid.repeatable.annotation.elem.nondefault` | `Errors.InvalidRepeatableAnnotationElemNondefault` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3275 |  |
| 232 | `compiler.err.invalid.repeatable.annotation.incompatible.target` | `Errors.InvalidRepeatableAnnotationIncompatibleTarget` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3222 |  |
| 233 | `compiler.err.invalid.repeatable.annotation.invalid.value` | `Errors.InvalidRepeatableAnnotationInvalidValue` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:971 |  |
| 234 | `compiler.err.invalid.repeatable.annotation.multiple.values` | `Errors.InvalidRepeatableAnnotationMultipleValues` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:958 |  |
| 235 | `compiler.err.invalid.repeatable.annotation.no.value` | `Errors.InvalidRepeatableAnnotationNoValue` | 已接线 | javac-core/src/comp/annotate.rs, javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:963 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3142 |  |
| 236 | `compiler.err.invalid.repeatable.annotation.not.applicable` | `Errors.InvalidRepeatableAnnotationNotApplicable` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:833 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:852 |  |
| 237 | `compiler.err.invalid.repeatable.annotation.not.applicable.in.context` | `Errors.InvalidRepeatableAnnotationNotApplicableInContext` | 功能已具备待接线 | comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:834 |  |
| 238 | `compiler.err.invalid.repeatable.annotation.not.documented` | `Errors.InvalidRepeatableAnnotationNotDocumented` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3174 |  |
| 239 | `compiler.err.invalid.repeatable.annotation.not.inherited` | `Errors.InvalidRepeatableAnnotationNotInherited` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3182 |  |
| 240 | `compiler.err.invalid.repeatable.annotation.repeated.and.container.present` | `Errors.InvalidRepeatableAnnotationRepeatedAndContainerPresent` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:1005 |  |
| 241 | `compiler.err.invalid.repeatable.annotation.retention` | `Errors.InvalidRepeatableAnnotationRetention` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3164 |  |
| 242 | `compiler.err.invalid.repeatable.annotation.value.return` | `Errors.InvalidRepeatableAnnotationValueReturn` | 已接线 | javac-core/src/comp/annotate.rs, javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:982 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3137 |  |
| 243 | `compiler.err.invalid.source` | `Errors.InvalidSource` | 功能已具备待接线 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:318 |  |
| 244 | `compiler.err.invalid.supertype.record` | `Errors.InvalidSupertypeRecord` | 已接线 | javac-core/src/comp/type_enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/TypeEnter.java:688 |  |
| 245 | `compiler.err.invalid.target` | `Errors.InvalidTarget` | 功能已具备待接线 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:339 |  |
| 246 | `compiler.err.invalid.yield` | `Errors.InvalidYield` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2051 |  |
| 247 | `compiler.err.io.exception` | `Errors.IoException` | 无上游触发 | — | — | 上游无引用 |
| 248 | `compiler.err.is.preview` | `Errors.IsPreview` | 已接线 | javac-core/src/code/preview.rs, javac-core/src/comp/attr.rs(等 3 个文件) | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3584 |  |
| 249 | `compiler.err.label.already.in.use` | `Errors.LabelAlreadyInUse` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1588 |  |
| 250 | `compiler.err.lambda.body.neither.value.nor.void.compatible` | `Errors.LambdaBodyNeitherValueNorVoidCompatible` | 已接线 | javac-core/src/comp/deferred_attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/DeferredAttr.java:862 |  |
| 251 | `compiler.err.limit.code` | `Errors.LimitCode` | 已接线 | javac-core/src/jvm/code.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/Code.java:113 |  |
| 252 | `compiler.err.limit.code.too.large.for.try.stmt` | `Errors.LimitCodeTooLargeForTryStmt` | 已接线 | javac-core/src/jvm/codegen.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/Gen.java:1707 |  |
| 253 | `compiler.err.limit.dimensions` | `Errors.LimitDimensions` | 已接线 | javac-core/src/jvm/codegen.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/Gen.java:289 ; src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/Gen.java:2003 |  |
| 254 | `compiler.err.limit.locals` | `Errors.LimitLocals` | 已接线 | javac-core/src/jvm/code.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/Code.java:117 |  |
| 255 | `compiler.err.limit.parameters` | `Errors.LimitParameters` | 已接线 | javac-core/src/jvm/codegen.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/Gen.java:958 |  |
| 256 | `compiler.err.limit.pool` | `Errors.LimitPool` | 已接线 | javac-core/src/jvm/codegen.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/Gen.java:2423 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:740 |  |
| 257 | `compiler.err.limit.pool.in.class` | `Errors.LimitPoolInClass` | 无上游触发 | — | — | 上游无引用 |
| 258 | `compiler.err.limit.stack` | `Errors.LimitStack` | 已接线 | javac-core/src/jvm/code.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/Code.java:121 |  |
| 259 | `compiler.err.limit.string` | `Errors.LimitString` | 已接线 | javac-core/src/jvm/codegen.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/Gen.java:536 |  |
| 260 | `compiler.err.limit.string.overflow` | `Errors.LimitStringOverflow` | 功能已具备待接线 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:743 |  |
| 261 | `compiler.err.local.classes.cant.extend.sealed` | `Errors.LocalClassesCantExtendSealed` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5251 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5310 |  |
| 262 | `compiler.err.local.enum` | `Errors.LocalEnum` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2658 |  |
| 263 | `compiler.err.local.var.accessed.from.icls.needs.final` | `Errors.LocalVarAccessedFromIclsNeedsFinal` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:2966 |  |
| 264 | `compiler.err.locn.bad.module-info` | `Errors.LocnBadModuleInfo` | 已接线 | javac-core/src/file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:1332 ; src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:1419 |  |
| 265 | `compiler.err.locn.cant.get.module.name.for.jar` | `Errors.LocnCantGetModuleNameForJar` | 已接线 | javac-core/src/file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:1411 ; src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:1452 |  |
| 266 | `compiler.err.locn.cant.read.directory` | `Errors.LocnCantReadDirectory` | 已接线 | javac-core/src/file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:1315 |  |
| 267 | `compiler.err.locn.cant.read.file` | `Errors.LocnCantReadFile` | 已接线 | javac-core/src/file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:1335 ; src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:1422 |  |
| 268 | `compiler.err.locn.invalid.arg.for.xpatch` | `Errors.LocnInvalidArgForXpatch` | 已接线 | javac-core/src/file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:2039 |  |
| 269 | `compiler.err.locn.module-info.not.allowed.on.patch.path` | `Errors.LocnModuleInfoNotAllowedOnPatchPath` | 无上游触发 | — | — | 上游无引用 |
| 270 | `compiler.err.malformed.fp.lit` | `Errors.MalformedFpLit` | 已接线 | javac-core/src/parser/java_tokenizer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:511 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:514 |  |
| 271 | `compiler.err.match.binding.exists` | `Errors.MatchBindingExists` | 已接线 | javac-core/src/comp/check.rs, javac-core/src/comp/match_bindings.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3768 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/MatchBindingsComputer.java:179 |  |
| 272 | `compiler.err.method.does.not.override.superclass` | `Errors.MethodDoesNotOverrideSuperclass` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2145 |  |
| 273 | `compiler.err.missing.meth.body.or.decl.abstract` | `Errors.MissingMethBodyOrDeclAbstract` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1187 |  |
| 274 | `compiler.err.missing.ret.stmt` | `Errors.MissingRetStmt` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:572 |  |
| 275 | `compiler.err.mod.not.allowed.here` | `Errors.ModNotAllowedHere` | 已接线 | javac-core/src/comp/check.rs, javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1278 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:811 |  |
| 276 | `compiler.err.modifier.not.allowed.here` | `Errors.ModifierNotAllowedHere` | 已接线 | javac-core/src/comp/modules.rs, javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:804 |  |
| 277 | `compiler.err.module.decl.sb.in.module-info.java` | `Errors.ModuleDeclSbInModuleInfoJava` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:313 |  |
| 278 | `compiler.err.module.name.mismatch` | `Errors.ModuleNameMismatch` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:322 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:398 |  |
| 279 | `compiler.err.module.non.zero.opens` | `Errors.ModuleNonZeroOpens` | 无上游触发 | — | — | 上游无引用 |
| 280 | `compiler.err.module.not.found` | `Errors.ModuleNotFound` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:795 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1405 |  |
| 281 | `compiler.err.module.not.found.in.module.source.path` | `Errors.ModuleNotFoundInModuleSourcePath` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:422 |  |
| 282 | `compiler.err.module.not.found.on.module.source.path` | `Errors.ModuleNotFoundOnModuleSourcePath` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:389 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:428 |  |
| 283 | `compiler.err.modulesourcepath.must.be.specified.with.dash.m.option` | `Errors.ModulesourcepathMustBeSpecifiedWithDashMOption` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:415 |  |
| 284 | `compiler.err.multi-module.outdir.cannot.be.exploded.module` | `Errors.MultiModuleOutdirCannotBeExplodedModule` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:488 |  |
| 285 | `compiler.err.multicatch.parameter.may.not.be.assigned` | `Errors.MulticatchParameterMayNotBeAssigned` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:1886 |  |
| 286 | `compiler.err.multicatch.types.must.be.disjoint` | `Errors.MulticatchTypesMustBeDisjoint` | 功能已具备待接线 | comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4987 |  |
| 287 | `compiler.err.multiple.values.for.module.source.path` | `Errors.MultipleValuesForModuleSourcePath` | 已接线 | javac-core/src/main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:197 |  |
| 288 | `compiler.err.name.clash.same.erasure` | `Errors.NameClashSameErasure` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3785 |  |
| 289 | `compiler.err.name.clash.same.erasure.no.hide` | `Errors.NameClashSameErasureNoHide` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2597 |  |
| 290 | `compiler.err.name.clash.same.erasure.no.override` | `Errors.NameClashSameErasureNoOverride` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2068 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2553 |  |
| 291 | `compiler.err.name.clash.same.erasure.no.override.1` | `Errors.NameClashSameErasureNoOverride1` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2559 |  |
| 292 | `compiler.err.name.reserved.for.internal.use` | `Errors.NameReservedForInternalUse` | 无上游触发 | — | — | 上游无引用 |
| 293 | `compiler.err.native.meth.cant.have.body` | `Errors.NativeMethCantHaveBody` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1196 |  |
| 294 | `compiler.err.new.not.allowed.in.annotation` | `Errors.NewNotAllowedInAnnotation` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:566 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:710 |  |
| 295 | `compiler.err.no.annotation.member` | `Errors.NoAnnotationMember` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:530 |  |
| 296 | `compiler.err.no.annotations.on.dot.class` | `Errors.NoAnnotationsOnDotClass` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:1275 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2256 |  |
| 297 | `compiler.err.no.encl.instance.of.type.in.scope` | `Errors.NoEnclInstanceOfTypeInScope` | 已接线 | javac-core/src/comp/lower.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Lower.java:1725 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Lower.java:1736 |  |
| 298 | `compiler.err.no.intf.expected.here` | `Errors.NoIntfExpectedHere` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:929 |  |
| 299 | `compiler.err.no.match.entry` | `Errors.NoMatchEntry` | 无上游触发 | — | — | 上游无引用 |
| 300 | `compiler.err.no.opens.unless.strong` | `Errors.NoOpensUnlessStrong` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:885 |  |
| 301 | `compiler.err.no.output.dir` | `Errors.NoOutputDir` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:619 |  |
| 302 | `compiler.err.no.pkg.in.module-info.java` | `Errors.NoPkgInModuleInfoJava` | 已接线 | javac-core/src/comp/enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Enter.java:319 |  |
| 303 | `compiler.err.no.source.files` | `Errors.NoSourceFiles` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:463 |  |
| 304 | `compiler.err.no.source.files.classes` | `Errors.NoSourceFilesClasses` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:461 |  |
| 305 | `compiler.err.no.superclass` | `Errors.NoSuperclass` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2501 |  |
| 306 | `compiler.err.no.switch.expression` | `Errors.NoSwitchExpression` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2302 |  |
| 307 | `compiler.err.no.switch.expression.qualify` | `Errors.NoSwitchExpressionQualify` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2301 |  |
| 308 | `compiler.err.no.value.for.option` | `Errors.NoValueForOption` | 已接线 | javac-core/src/main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/launcher/Main.java:326 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:176 |  |
| 309 | `compiler.err.no.zipfs.for.archive` | `Errors.NoZipfsForArchive` | 功能已具备待接线 | file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:404 ; src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:1391 |  |
| 310 | `compiler.err.non-static.cant.be.ref` | `Errors.NonStaticCantBeRef` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 311 | `compiler.err.non.sealed.or.sealed.expected` | `Errors.NonSealedOrSealedExpected` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5322 |  |
| 312 | `compiler.err.non.sealed.sealed.or.final.expected` | `Errors.NonSealedSealedOrFinalExpected` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5323 |  |
| 313 | `compiler.err.non.sealed.with.no.sealed.supertype` | `Errors.NonSealedWithNoSealedSupertype` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5305 |  |
| 314 | `compiler.err.not.annotation.type` | `Errors.NotAnnotationType` | 已接线 | javac-core/src/comp/annotate.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:482 |  |
| 315 | `compiler.err.not.def.access.class.intf.cant.access` | `Errors.NotDefAccessClassIntfCantAccess` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 316 | `compiler.err.not.def.access.class.intf.cant.access.reason` | `Errors.NotDefAccessClassIntfCantAccessReason` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 317 | `compiler.err.not.def.access.package.cant.access` | `Errors.NotDefAccessPackageCantAccess` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 318 | `compiler.err.not.def.public` | `Errors.NotDefPublic` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1096 |  |
| 319 | `compiler.err.not.def.public.cant.access` | `Errors.NotDefPublicCantAccess` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 320 | `compiler.err.not.encl.class` | `Errors.NotEnclClass` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4309 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:3765 |  |
| 321 | `compiler.err.not.exhaustive` | `Errors.NotExhaustive` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:730 |  |
| 322 | `compiler.err.not.exhaustive.statement` | `Errors.NotExhaustiveStatement` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:695 |  |
| 323 | `compiler.err.not.in.module.on.module.source.path` | `Errors.NotInModuleOnModuleSourcePath` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:430 |  |
| 324 | `compiler.err.not.in.profile` | `Errors.NotInProfile` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3576 |  |
| 325 | `compiler.err.not.loop.label` | `Errors.NotLoopLabel` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2368 |  |
| 326 | `compiler.err.not.stmt` | `Errors.NotStmt` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:4758 ; test/langtools/tools/javac/6304921/TestLog.java:128 |  |
| 327 | `compiler.err.not.within.bounds` | `Errors.NotWithinBounds` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1438 |  |
| 328 | `compiler.err.operator.cant.be.applied` | `Errors.OperatorCantBeApplied` | 已接线 | javac-core/src/comp/operators.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Operators.java:216 |  |
| 329 | `compiler.err.operator.cant.be.applied.1` | `Errors.OperatorCantBeApplied1` | 已接线 | javac-core/src/comp/operators.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Operators.java:217 |  |
| 330 | `compiler.err.option.not.allowed.with.target` | `Errors.OptionNotAllowedWithTarget` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:597 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:605 |  |
| 331 | `compiler.err.option.removed.source` | `Errors.OptionRemovedSource` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:582 |  |
| 332 | `compiler.err.option.removed.target` | `Errors.OptionRemovedTarget` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:589 |  |
| 333 | `compiler.err.option.too.many` | `Errors.OptionTooMany` | 功能已具备待接线 | main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:458 |  |
| 334 | `compiler.err.orphaned` | `Errors.Orphaned` | 功能已具备待接线 | parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2542 |  |
| 335 | `compiler.err.output.dir.must.be.specified.with.dash.m.option` | `Errors.OutputDirMustBeSpecifiedWithDashMOption` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:413 |  |
| 336 | `compiler.err.override.incompatible.ret` | `Errors.OverrideIncompatibleRet` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1840 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1845 |  |
| 337 | `compiler.err.override.meth` | `Errors.OverrideMeth` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1800 |  |
| 338 | `compiler.err.override.meth.doesnt.throw` | `Errors.OverrideMethDoesntThrow` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1861 |  |
| 339 | `compiler.err.override.static` | `Errors.OverrideStatic` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1789 |  |
| 340 | `compiler.err.override.weaker.access` | `Errors.OverrideWeakerAccess` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1815 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1817 |  |
| 341 | `compiler.err.package.clash.from.requires` | `Errors.PackageClashFromRequires` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1598 |  |
| 342 | `compiler.err.package.clash.from.requires.in.unnamed` | `Errors.PackageClashFromRequiresInUnnamed` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1595 |  |
| 343 | `compiler.err.package.empty.or.not.found` | `Errors.PackageEmptyOrNotFound` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1044 |  |
| 344 | `compiler.err.package.in.other.module` | `Errors.PackageInOtherModule` | 已接线 | javac-core/src/comp/enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Enter.java:353 |  |
| 345 | `compiler.err.package.not.visible` | `Errors.PackageNotVisible` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 346 | `compiler.err.pattern.dominated` | `Errors.PatternDominated` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1719 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1848 |  |
| 347 | `compiler.err.pattern.expected` | `Errors.PatternExpected` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1754 |  |
| 348 | `compiler.err.pkg.annotations.sb.in.package-info.java` | `Errors.PkgAnnotationsSbInPackageInfoJava` | 已接线 | javac-core/src/comp/enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Enter.java:337 |  |
| 349 | `compiler.err.pkg.clashes.with.class.of.same.name` | `Errors.PkgClashesWithClassOfSameName` | 已接线 | javac-core/src/comp/type_enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/TypeEnter.java:399 |  |
| 350 | `compiler.err.plugin.not.found` | `Errors.PluginNotFound` | 功能未移植(API 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/api/BasicJavacTask.java:238 |  |
| 351 | `compiler.err.premature.eof` | `Errors.PrematureEof` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:441 |  |
| 352 | `compiler.err.preview.feature.disabled` | `Errors.PreviewFeatureDisabled` | 已接线 | javac-core/src/code/preview.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Preview.java:208 |  |
| 353 | `compiler.err.preview.feature.disabled.classfile` | `Errors.PreviewFeatureDisabledClassfile` | 已接线 | javac-core/src/code/preview.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Preview.java:219 |  |
| 354 | `compiler.err.preview.feature.disabled.plural` | `Errors.PreviewFeatureDisabledPlural` | 已接线 | javac-core/src/code/preview.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Preview.java:207 |  |
| 355 | `compiler.err.preview.not.latest` | `Errors.PreviewNotLatest` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:542 |  |
| 356 | `compiler.err.preview.without.source.or.release` | `Errors.PreviewWithoutSourceOrRelease` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:538 |  |
| 357 | `compiler.err.prob.found.req` | `Errors.ProbFoundReq` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:570 |  |
| 358 | `compiler.err.proc.bad.config.file` | `Errors.ProcBadConfigFile` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:446 ; src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:455 |  |
| 359 | `compiler.err.proc.cant.access` | `Errors.ProcCantAccess` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:1030 |  |
| 360 | `compiler.err.proc.cant.access.1` | `Errors.ProcCantAccess1` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:1025 |  |
| 361 | `compiler.err.proc.cant.create.loader` | `Errors.ProcCantCreateLoader` | 无上游触发 | — | — | 上游无引用 |
| 362 | `compiler.err.proc.cant.find.class` | `Errors.ProcCantFindClass` | 功能已具备待接线 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1202 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1220 |  |
| 363 | `compiler.err.proc.cant.load.class` | `Errors.ProcCantLoadClass` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:449 ; src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:452 |  |
| 364 | `compiler.err.proc.messager` | `Errors.ProcMessager` | 功能未移植(API 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/api/JavacTrees.java:1266 ; src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacMessager.java:121 |  |
| 365 | `compiler.err.proc.no.explicit.annotation.processing.requested` | `Errors.ProcNoExplicitAnnotationProcessingRequested` | 功能已具备待接线 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1175 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1190 |  |
| 366 | `compiler.err.proc.no.service` | `Errors.ProcNoService` | 无上游触发 | — | — | 上游无引用 |
| 367 | `compiler.err.proc.processor.bad.option.name` | `Errors.ProcProcessorBadOptionName` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:774 |  |
| 368 | `compiler.err.proc.processor.cant.instantiate` | `Errors.ProcProcessorCantInstantiate` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:593 |  |
| 369 | `compiler.err.proc.processor.not.found` | `Errors.ProcProcessorNotFound` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:533 ; src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:587 |  |
| 370 | `compiler.err.proc.processor.wrong.type` | `Errors.ProcProcessorWrongType` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:590 |  |
| 371 | `compiler.err.proc.service.problem` | `Errors.ProcServiceProblem` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:430 |  |
| 372 | `compiler.err.processorpath.no.processormodulepath` | `Errors.ProcessorpathNoProcessormodulepath` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:625 |  |
| 373 | `compiler.err.profile.bootclasspath.conflict` | `Errors.ProfileBootclasspathConflict` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:558 |  |
| 374 | `compiler.err.qualified.new.of.static.class` | `Errors.QualifiedNewOfStaticClass` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2753 |  |
| 375 | `compiler.err.receiver.parameter.not.applicable.constructor.toplevel.class` | `Errors.ReceiverParameterNotApplicableConstructorToplevelClass` | 已接线 | javac-core/src/comp/member_enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/MemberEnter.java:333 |  |
| 376 | `compiler.err.record.cannot.declare.instance.fields` | `Errors.RecordCannotDeclareInstanceFields` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:4350 |  |
| 377 | `compiler.err.record.cant.declare.field.modifiers` | `Errors.RecordCantDeclareFieldModifiers` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:4719 |  |
| 378 | `compiler.err.record.component.and.old.array.syntax` | `Errors.RecordComponentAndOldArraySyntax` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3582 |  |
| 379 | `compiler.err.record.header.expected` | `Errors.RecordHeaderExpected` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3879 |  |
| 380 | `compiler.err.recursive.ctor.invocation` | `Errors.RecursiveCtorInvocation` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3698 |  |
| 381 | `compiler.err.ref.ambiguous` | `Errors.RefAmbiguous` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 382 | `compiler.err.release.bootclasspath.conflict` | `Errors.ReleaseBootclasspathConflict` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:298 |  |
| 383 | `compiler.err.repeated.annotation.target` | `Errors.RepeatedAnnotationTarget` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3530 |  |
| 384 | `compiler.err.repeated.interface` | `Errors.RepeatedInterface` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2844 |  |
| 385 | `compiler.err.repeated.modifier` | `Errors.RepeatedModifier` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3263 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3766 |  |
| 386 | `compiler.err.repeated.provides.for.service` | `Errors.RepeatedProvidesForService` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1082 |  |
| 387 | `compiler.err.repeated.value.for.module.source.path` | `Errors.RepeatedValueForModuleSourcePath` | 已接线 | javac-core/src/main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:189 |  |
| 388 | `compiler.err.repeated.value.for.patch.module` | `Errors.RepeatedValueForPatchModule` | 已接线 | javac-core/src/main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:237 |  |
| 389 | `compiler.err.report.access` | `Errors.ReportAccess` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 390 | `compiler.err.req.arg` | `Errors.ReqArg` | 已接线 | javac-core/src/main/option.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Option.java:1155 |  |
| 391 | `compiler.err.restricted.type.not.allowed` | `Errors.RestrictedTypeNotAllowed` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3983 |  |
| 392 | `compiler.err.restricted.type.not.allowed.array` | `Errors.RestrictedTypeNotAllowedArray` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:1888 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3459 |  |
| 393 | `compiler.err.restricted.type.not.allowed.compound` | `Errors.RestrictedTypeNotAllowedCompound` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3464 |  |
| 394 | `compiler.err.restricted.type.not.allowed.here` | `Errors.RestrictedTypeNotAllowedHere` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:825 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:1888 |  |
| 395 | `compiler.err.ret.outside.meth` | `Errors.RetOutsideMeth` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2418 |  |
| 396 | `compiler.err.return.outside.switch.expression` | `Errors.ReturnOutsideSwitchExpression` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2420 |  |
| 397 | `compiler.err.rule.completes.normally` | `Errors.RuleCompletesNormally` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:721 |  |
| 398 | `compiler.err.same.binary.name` | `Errors.SameBinaryName` | 已接线 | javac-core/src/comp/enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Enter.java:452 |  |
| 399 | `compiler.err.sealed.class.must.have.subclasses` | `Errors.SealedClassMustHaveSubclasses` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5237 |  |
| 400 | `compiler.err.sealed.or.non.sealed.local.classes.not.allowed` | `Errors.SealedOrNonSealedLocalClassesNotAllowed` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2712 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2719 |  |
| 401 | `compiler.err.service.definition.is.enum` | `Errors.ServiceDefinitionIsEnum` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1152 |  |
| 402 | `compiler.err.service.implementation.doesnt.have.a.no.args.constructor` | `Errors.ServiceImplementationDoesntHaveANoArgsConstructor` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1115 |  |
| 403 | `compiler.err.service.implementation.is.abstract` | `Errors.ServiceImplementationIsAbstract` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1109 |  |
| 404 | `compiler.err.service.implementation.is.inner` | `Errors.ServiceImplementationIsInner` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1111 |  |
| 405 | `compiler.err.service.implementation.must.be.subtype.of.service.interface` | `Errors.ServiceImplementationMustBeSubtypeOfServiceInterface` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1107 |  |
| 406 | `compiler.err.service.implementation.no.args.constructor.not.public` | `Errors.ServiceImplementationNoArgsConstructorNotPublic` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1117 |  |
| 407 | `compiler.err.service.implementation.not.in.right.module` | `Errors.ServiceImplementationNotInRightModule` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1176 |  |
| 408 | `compiler.err.service.implementation.provider.return.must.be.subtype.of.service.interface` | `Errors.ServiceImplementationProviderReturnMustBeSubtypeOfServiceInterface` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1103 |  |
| 409 | `compiler.err.signature.doesnt.match.intf` | `Errors.SignatureDoesntMatchIntf` | 无上游触发 | — | — | 上游无引用 |
| 410 | `compiler.err.signature.doesnt.match.supertype` | `Errors.SignatureDoesntMatchSupertype` | 无上游触发 | — | — | 上游无引用 |
| 411 | `compiler.err.source.cant.overwrite.input.file` | `Errors.SourceCantOverwriteInputFile` | 功能已具备待接线 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:762 |  |
| 412 | `compiler.err.sourcepath.modulesourcepath.conflict` | `Errors.SourcepathModulesourcepathConflict` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:563 |  |
| 413 | `compiler.err.stack.sim.error` | `Errors.StackSimError` | 已接线 | javac-core/src/jvm/codegen.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/Gen.java:975 |  |
| 414 | `compiler.err.static.declaration.not.allowed.in.inner.classes` | `Errors.StaticDeclarationNotAllowedInInnerClasses` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1241 |  |
| 415 | `compiler.err.static.imp.only.classes.and.interfaces` | `Errors.StaticImpOnlyClassesAndInterfaces` | 已接线 | javac-core/src/comp/type_enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/TypeEnter.java:491 |  |
| 416 | `compiler.err.static.methods.cannot.be.annotated.with.override` | `Errors.StaticMethodsCannotBeAnnotatedWithOverride` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2145 |  |
| 417 | `compiler.err.string.const.req` | `Errors.StringConstReq` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1757 |  |
| 418 | `compiler.err.switch.case.unexpected.statement` | `Errors.SwitchCaseUnexpectedStatement` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3018 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3043 |  |
| 419 | `compiler.err.switch.expression.completes.normally` | `Errors.SwitchExpressionCompletesNormally` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:724 |  |
| 420 | `compiler.err.switch.expression.empty` | `Errors.SwitchExpressionEmpty` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1643 |  |
| 421 | `compiler.err.switch.expression.no.result.expressions` | `Errors.SwitchExpressionNoResultExpressions` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1646 |  |
| 422 | `compiler.err.switch.mixing.case.types` | `Errors.SwitchMixingCaseTypes` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1706 |  |
| 423 | `compiler.err.this.as.identifier` | `Errors.ThisAsIdentifier` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:595 |  |
| 424 | `compiler.err.throws.not.allowed.in.intf.annotation` | `Errors.ThrowsNotAllowedInIntfAnnotation` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1161 |  |
| 425 | `compiler.err.too.many.modules` | `Errors.TooManyModules` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:338 |  |
| 426 | `compiler.err.too.many.patched.modules` | `Errors.TooManyPatchedModules` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:591 |  |
| 427 | `compiler.err.total.pattern.and.default` | `Errors.TotalPatternAndDefault` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1772 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1793 |  |
| 428 | `compiler.err.try.resource.may.not.be.assigned` | `Errors.TryResourceMayNotBeAssigned` | 功能已具备待接线 | comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:318 |  |
| 429 | `compiler.err.try.with.resources.expr.effectively.final.var` | `Errors.TryWithResourcesExprEffectivelyFinalVar` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:3047 |  |
| 430 | `compiler.err.try.with.resources.expr.needs.var` | `Errors.TryWithResourcesExprNeedsVar` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3626 |  |
| 431 | `compiler.err.try.without.catch.finally.or.resource.decls` | `Errors.TryWithoutCatchFinallyOrResourceDecls` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2851 |  |
| 432 | `compiler.err.two.class.loaders.1` | `Errors.TwoClassLoaders1` | 功能已具备待接线 | main/main.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Main.java:411 |  |
| 433 | `compiler.err.two.class.loaders.2` | `Errors.TwoClassLoaders2` | 功能已具备待接线 | main/main.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Main.java:408 |  |
| 434 | `compiler.err.type.doesnt.take.params` | `Errors.TypeDoesntTakeParams` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4960 |  |
| 435 | `compiler.err.type.found.req` | `Errors.TypeFoundReq` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:326 |  |
| 436 | `compiler.err.type.var.cant.be.deref` | `Errors.TypeVarCantBeDeref` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4289 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4454 |  |
| 437 | `compiler.err.type.var.may.not.be.followed.by.other.bounds` | `Errors.TypeVarMayNotBeFollowedByOtherBounds` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5043 |  |
| 438 | `compiler.err.type.var.more.than.once` | `Errors.TypeVarMoreThanOnce` | 无上游触发 | — | — | 上游无引用 |
| 439 | `compiler.err.type.var.more.than.once.in.result` | `Errors.TypeVarMoreThanOnceInResult` | 无上游触发 | — | — | 上游无引用 |
| 440 | `compiler.err.types.incompatible` | `Errors.TypesIncompatible` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2062 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2663 |  |
| 441 | `compiler.err.unclosed.char.lit` | `Errors.UnclosedCharLit` | 已接线 | javac-core/src/parser/java_tokenizer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:988 |  |
| 442 | `compiler.err.unclosed.comment` | `Errors.UnclosedComment` | 已接线 | javac-core/src/parser/java_tokenizer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:962 |  |
| 443 | `compiler.err.unclosed.str.lit` | `Errors.UnclosedStrLit` | 已接线 | javac-core/src/parser/java_tokenizer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:461 |  |
| 444 | `compiler.err.unclosed.text.block` | `Errors.UnclosedTextBlock` | 已接线 | javac-core/src/parser/java_tokenizer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:461 |  |
| 445 | `compiler.err.undef.label` | `Errors.UndefLabel` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2405 |  |
| 446 | `compiler.err.underscore.as.identifier` | `Errors.UnderscoreAsIdentifier` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:603 |  |
| 447 | `compiler.err.underscore.as.identifier.in.lambda` | `Errors.UnderscoreAsIdentifierInLambda` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3540 |  |
| 448 | `compiler.err.unexpected.lambda` | `Errors.UnexpectedLambda` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3081 |  |
| 449 | `compiler.err.unexpected.mref` | `Errors.UnexpectedMref` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3545 |  |
| 450 | `compiler.err.unexpected.type` | `Errors.UnexpectedType` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:252 |  |
| 451 | `compiler.err.unmatched.quote` | `Errors.UnmatchedQuote` | 功能已具备待接线 | main/main.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Main.java:224 |  |
| 452 | `compiler.err.unnamed.pkg.not.allowed.named.modules` | `Errors.UnnamedPkgNotAllowedNamedModules` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:402 |  |
| 453 | `compiler.err.unreachable.stmt` | `Errors.UnreachableStmt` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:501 |  |
| 454 | `compiler.err.unreported.exception.default.constructor` | `Errors.UnreportedExceptionDefaultConstructor` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:1006 |  |
| 455 | `compiler.err.unreported.exception.implicit.close` | `Errors.UnreportedExceptionImplicitClose` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:1010 |  |
| 456 | `compiler.err.unreported.exception.need.to.catch.or.throw` | `Errors.UnreportedExceptionNeedToCatchOrThrow` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:1014 |  |
| 457 | `compiler.err.unsupported.encoding` | `Errors.UnsupportedEncoding` | 已接线 | javac-core/src/file/base_file_manager.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/BaseFileManager.java:322 |  |
| 458 | `compiler.err.unsupported.release.version` | `Errors.UnsupportedReleaseVersion` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:311 |  |
| 459 | `compiler.err.var.might.already.be.assigned` | `Errors.VarMightAlreadyBeAssigned` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:1900 |  |
| 460 | `compiler.err.var.might.be.assigned.in.loop` | `Errors.VarMightBeAssignedInLoop` | 无上游触发 | — | — | 上游无引用 |
| 461 | `compiler.err.var.might.not.have.been.initialized` | `Errors.VarMightNotHaveBeenInitialized` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:1932 |  |
| 462 | `compiler.err.var.not.initialized.in.default.constructor` | `Errors.VarNotInitializedInDefaultConstructor` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:2174 |  |
| 463 | `compiler.err.varargs.and.old.array.syntax` | `Errors.VarargsAndOldArraySyntax` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3579 |  |
| 464 | `compiler.err.varargs.and.receiver` | `Errors.VarargsAndReceiver` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3555 |  |
| 465 | `compiler.err.varargs.invalid.trustme.anno` | `Errors.VarargsInvalidTrustmeAnno` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:951 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:955 |  |
| 466 | `compiler.err.varargs.must.be.last` | `Errors.VarargsMustBeLast` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:4601 |  |
| 467 | `compiler.err.variable.not.allowed` | `Errors.VariableNotAllowed` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2606 |  |
| 468 | `compiler.err.void.not.allowed.here` | `Errors.VoidNotAllowedHere` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:702 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1512 |  |
| 469 | `compiler.err.warnings.and.werror` | `Errors.WarningsAndWerror` | 功能已具备待接线 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:568 |  |
| 470 | `compiler.err.wrong.number.type.args` | `Errors.WrongNumberTypeArgs` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4958 |  |
| 471 | `compiler.err.wrong.receiver` | `Errors.WrongReceiver` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3561 |  |
| 472 | `compiler.warn.access.to.member.from.serializable.element` | `Warnings.AccessToMemberFromSerializableElement` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2773 |  |
| 473 | `compiler.warn.access.to.member.from.serializable.lambda` | `Warnings.AccessToMemberFromSerializableLambda` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2769 |  |
| 474 | `compiler.warn.addopens.ignored` | `Warnings.AddopensIgnored` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:640 |  |
| 475 | `compiler.warn.annotation.method.not.found` | `Warnings.AnnotationMethodNotFound` | 功能已具备待接线 | jvm/class_reader.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:1953 |  |
| 476 | `compiler.warn.annotation.method.not.found.reason` | `Warnings.AnnotationMethodNotFoundReason` | 功能已具备待接线 | jvm/class_reader.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:1955 |  |
| 477 | `compiler.warn.attempt.to.synchronize.on.instance.of.value.based.class` | `Warnings.AttemptToSynchronizeOnInstanceOfValueBasedClass` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1874 |  |
| 478 | `compiler.warn.auxiliary.class.accessed.from.outside.of.its.source.file` | `Warnings.AuxiliaryClassAccessedFromOutsideOfItsSourceFile` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3884 |  |
| 479 | `compiler.warn.bad.name.for.option` | `Warnings.BadNameForOption` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:660 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:665 |  |
| 480 | `compiler.warn.big.major.version` | `Warnings.BigMajorVersion` | 已接线 | javac-core/src/jvm/class_reader.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:2927 |  |
| 481 | `compiler.warn.constant.SVUID` | `Warnings.ConstantSVUID` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5570 |  |
| 482 | `compiler.warn.declared.using.preview` | `Warnings.DeclaredUsingPreview` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:260 |  |
| 483 | `compiler.warn.deprecated.annotation.has.no.effect` | `Warnings.DeprecatedAnnotationHasNoEffect` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3548 |  |
| 484 | `compiler.warn.diamond.redundant.args` | `Warnings.DiamondRedundantArgs` | 已接线 | javac-core/src/comp/analyzer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Analyzer.java:269 |  |
| 485 | `compiler.warn.dir.path.element.not.directory` | `Warnings.DirPathElementNotDirectory` | 功能已具备待接线 | file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:1663 |  |
| 486 | `compiler.warn.dir.path.element.not.found` | `Warnings.DirPathElementNotFound` | 功能已具备待接线 | file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:323 ; src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:1664 |  |
| 487 | `compiler.warn.div.zero` | `Warnings.DivZero` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:290 ; test/langtools/tools/javac/6304921/TestLog.java:133 |  |
| 488 | `compiler.warn.doclint.not.available` | `Warnings.DoclintNotAvailable` | 功能未移植(API 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/api/BasicJavacTask.java:265 |  |
| 489 | `compiler.warn.empty.if` | `Warnings.EmptyIf` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3736 |  |
| 490 | `compiler.warn.file.from.future` | `Warnings.FileFromFuture` | 无上游触发 | — | — | 上游无引用 |
| 491 | `compiler.warn.finally.cannot.complete` | `Warnings.FinallyCannotComplete` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:838 |  |
| 492 | `compiler.warn.forward.ref` | `Warnings.ForwardRef` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4678 |  |
| 493 | `compiler.warn.future.attr` | `Warnings.FutureAttr` | 已接线 | javac-core/src/jvm/class_reader.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:768 |  |
| 494 | `compiler.warn.has.been.deprecated` | `Warnings.HasBeenDeprecated` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:240 |  |
| 495 | `compiler.warn.has.been.deprecated.for.removal` | `Warnings.HasBeenDeprecatedForRemoval` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:233 |  |
| 496 | `compiler.warn.has.been.deprecated.for.removal.module` | `Warnings.HasBeenDeprecatedForRemovalModule` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:231 |  |
| 497 | `compiler.warn.has.been.deprecated.module` | `Warnings.HasBeenDeprecatedModule` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:238 |  |
| 498 | `compiler.warn.illegal.char.for.encoding` | `Warnings.IllegalCharForEncoding` | 无上游触发 | — | — | 上游无引用 |
| 499 | `compiler.warn.illegal.ref.to.restricted.type` | `Warnings.IllegalRefToRestrictedType` | 已接线 | javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:2504 |  |
| 500 | `compiler.warn.improper.SVUID` | `Warnings.ImproperSVUID` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5560 |  |
| 501 | `compiler.warn.inconsistent.white.space.indentation` | `Warnings.InconsistentWhiteSpaceIndentation` | 已接线 | javac-core/src/parser/java_tokenizer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:1059 |  |
| 502 | `compiler.warn.incubating.modules` | `Warnings.IncubatingModules` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1350 |  |
| 503 | `compiler.warn.inexact.non-varargs.call` | `Warnings.InexactNonVarargsCall` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1045 |  |
| 504 | `compiler.warn.invalid.archive.file` | `Warnings.InvalidArchiveFile` | 功能已具备待接线 | file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:398 |  |
| 505 | `compiler.warn.invalid.path` | `Warnings.InvalidPath` | 功能已具备待接线 | file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:227 |  |
| 506 | `compiler.warn.invalid.yield` | `Warnings.InvalidYield` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:2062 |  |
| 507 | `compiler.warn.is.preview` | `Warnings.IsPreview` | 已接线 | javac-core/src/code/preview.rs, javac-core/src/comp/attr.rs(等 3 个文件) | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3587 |  |
| 508 | `compiler.warn.is.preview.reflective` | `Warnings.IsPreviewReflective` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3590 |  |
| 509 | `compiler.warn.leaks.not.accessible` | `Warnings.LeaksNotAccessible` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:4218 |  |
| 510 | `compiler.warn.leaks.not.accessible.not.required.transitive` | `Warnings.LeaksNotAccessibleNotRequiredTransitive` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:4255 |  |
| 511 | `compiler.warn.leaks.not.accessible.unexported` | `Warnings.LeaksNotAccessibleUnexported` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:4227 |  |
| 512 | `compiler.warn.leaks.not.accessible.unexported.qualified` | `Warnings.LeaksNotAccessibleUnexportedQualified` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:4233 |  |
| 513 | `compiler.warn.lintOption` | `Warnings.LintOption` | 无上游触发 | — | — | 上游无引用 |
| 514 | `compiler.warn.local.redundant.type` | `Warnings.LocalRedundantType` | 已接线 | javac-core/src/comp/analyzer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Analyzer.java:382 |  |
| 515 | `compiler.warn.locn.unknown.file.on.module.path` | `Warnings.LocnUnknownFileOnModulePath` | 功能已具备待接线 | file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:1493 |  |
| 516 | `compiler.warn.long.SVUID` | `Warnings.LongSVUID` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5565 |  |
| 517 | `compiler.warn.method.redundant.typeargs` | `Warnings.MethodRedundantTypeargs` | 已接线 | javac-core/src/comp/analyzer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Analyzer.java:349 |  |
| 518 | `compiler.warn.missing-explicit-ctor` | `Warnings.MissingExplicitCtor` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3929 |  |
| 519 | `compiler.warn.missing.SVUID` | `Warnings.MissingSVUID` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5545 |  |
| 520 | `compiler.warn.missing.deprecated.annotation` | `Warnings.MissingDeprecatedAnnotation` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3542 |  |
| 521 | `compiler.warn.module.for.option.not.found` | `Warnings.ModuleForOptionNotFound` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1246 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1688 |  |
| 522 | `compiler.warn.module.not.found` | `Warnings.ModuleNotFound` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:4263 |  |
| 523 | `compiler.warn.option.obsolete.source` | `Warnings.OptionObsoleteSource` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:584 |  |
| 524 | `compiler.warn.option.obsolete.suppression` | `Warnings.OptionObsoleteSuppression` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:629 |  |
| 525 | `compiler.warn.option.obsolete.target` | `Warnings.OptionObsoleteTarget` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:591 |  |
| 526 | `compiler.warn.option.parameters.unsupported` | `Warnings.OptionParametersUnsupported` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:613 |  |
| 527 | `compiler.warn.outdir.is.in.exploded.module` | `Warnings.OutdirIsInExplodedModule` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:497 |  |
| 528 | `compiler.warn.override.bridge` | `Warnings.OverrideBridge` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1883 |  |
| 529 | `compiler.warn.override.equals.but.not.hashcode` | `Warnings.OverrideEqualsButNotHashcode` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2199 |  |
| 530 | `compiler.warn.override.unchecked.ret` | `Warnings.OverrideUncheckedRet` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1851 |  |
| 531 | `compiler.warn.override.unchecked.thrown` | `Warnings.OverrideUncheckedThrown` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1867 |  |
| 532 | `compiler.warn.override.varargs.extra` | `Warnings.OverrideVarargsExtra` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1877 |  |
| 533 | `compiler.warn.override.varargs.missing` | `Warnings.OverrideVarargsMissing` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1876 |  |
| 534 | `compiler.warn.package.empty.or.not.found` | `Warnings.PackageEmptyOrNotFound` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:4273 |  |
| 535 | `compiler.warn.path.element.not.found` | `Warnings.PathElementNotFound` | 功能已具备待接线 | file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:369 |  |
| 536 | `compiler.warn.pkg-info.already.seen` | `Warnings.PkgInfoAlreadySeen` | 已接线 | javac-core/src/comp/enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Enter.java:368 |  |
| 537 | `compiler.warn.poor.choice.for.module.name` | `Warnings.PoorChoiceForModuleName` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2231 |  |
| 538 | `compiler.warn.position.overflow` | `Warnings.PositionOverflow` | 已接线 | javac-core/src/jvm/cr_table.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/CRTable.java:178 |  |
| 539 | `compiler.warn.possible.fall-through.into.case` | `Warnings.PossibleFallThroughIntoCase` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:690 |  |
| 540 | `compiler.warn.potential.lambda.found` | `Warnings.PotentialLambdaFound` | 已接线 | javac-core/src/comp/analyzer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Analyzer.java:319 |  |
| 541 | `compiler.warn.potentially.ambiguous.overload` | `Warnings.PotentiallyAmbiguousOverload` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2738 |  |
| 542 | `compiler.warn.preview.feature.use` | `Warnings.PreviewFeatureUse` | 已接线 | javac-core/src/code/preview.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Preview.java:143 |  |
| 543 | `compiler.warn.preview.feature.use.classfile` | `Warnings.PreviewFeatureUseClassfile` | 已接线 | javac-core/src/code/preview.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Preview.java:157 |  |
| 544 | `compiler.warn.preview.feature.use.plural` | `Warnings.PreviewFeatureUsePlural` | 已接线 | javac-core/src/code/preview.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Preview.java:142 |  |
| 545 | `compiler.warn.prob.found.req` | `Warnings.ProbFoundReq` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3959 |  |
| 546 | `compiler.warn.proc.annotations.without.processors` | `Warnings.ProcAnnotationsWithoutProcessors` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:962 |  |
| 547 | `compiler.warn.proc.duplicate.option.name` | `Warnings.ProcDuplicateOptionName` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:741 |  |
| 548 | `compiler.warn.proc.duplicate.supported.annotation` | `Warnings.ProcDuplicateSupportedAnnotation` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:719 |  |
| 549 | `compiler.warn.proc.file.create.last.round` | `Warnings.ProcFileCreateLastRound` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacFiler.java:513 |  |
| 550 | `compiler.warn.proc.file.reopening` | `Warnings.ProcFileReopening` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacFiler.java:765 |  |
| 551 | `compiler.warn.proc.illegal.file.name` | `Warnings.ProcIllegalFileName` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacFiler.java:702 |  |
| 552 | `compiler.warn.proc.malformed.supported.string` | `Warnings.ProcMalformedSupportedString` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:1787 |  |
| 553 | `compiler.warn.proc.messager` | `Warnings.ProcMessager` | 功能未移植(API 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/api/JavacTrees.java:1270 ; src/jdk.compiler/share/classes/com/sun/tools/javac/api/JavacTrees.java:1274 |  |
| 554 | `compiler.warn.proc.package.does.not.exist` | `Warnings.ProcPackageDoesNotExist` | 功能已具备待接线 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1217 |  |
| 555 | `compiler.warn.proc.proc-only.requested.no.procs` | `Warnings.ProcProcOnlyRequestedNoProcs` | 功能已具备待接线 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1170 |  |
| 556 | `compiler.warn.proc.processor.incompatible.source.version` | `Warnings.ProcProcessorIncompatibleSourceVersion` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:765 |  |
| 557 | `compiler.warn.proc.redundant.types.with.wildcard` | `Warnings.ProcRedundantTypesWithWildcard` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:733 |  |
| 558 | `compiler.warn.proc.suspicious.class.name` | `Warnings.ProcSuspiciousClassName` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacFiler.java:495 |  |
| 559 | `compiler.warn.proc.type.already.exists` | `Warnings.ProcTypeAlreadyExists` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacFiler.java:735 |  |
| 560 | `compiler.warn.proc.type.recreate` | `Warnings.ProcTypeRecreate` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacFiler.java:731 |  |
| 561 | `compiler.warn.proc.unclosed.type.files` | `Warnings.ProcUnclosedTypeFiles` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacFiler.java:825 |  |
| 562 | `compiler.warn.proc.unmatched.processor.options` | `Warnings.ProcUnmatchedProcessorOptions` | 范围外(PLAN §3.1(APT 不移植)) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacProcessingEnvironment.java:1459 |  |
| 563 | `compiler.warn.proc.use.implicit` | `Warnings.ProcUseImplicit` | 功能已具备待接线 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1713 |  |
| 564 | `compiler.warn.proc.use.proc.or.implicit` | `Warnings.ProcUseProcOrImplicit` | 功能已具备待接线 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1715 |  |
| 565 | `compiler.warn.profile.target.conflict` | `Warnings.ProfileTargetConflict` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:552 |  |
| 566 | `compiler.warn.raw.class.use` | `Warnings.RawClassUse` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1554 |  |
| 567 | `compiler.warn.redundant.cast` | `Warnings.RedundantCast` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:655 |  |
| 568 | `compiler.warn.requires.automatic` | `Warnings.RequiresAutomatic` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:4284 |  |
| 569 | `compiler.warn.requires.transitive.automatic` | `Warnings.RequiresTransitiveAutomatic` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:4282 |  |
| 570 | `compiler.warn.restricted.type.not.allowed` | `Warnings.RestrictedTypeNotAllowed` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3496 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3503 |  |
| 571 | `compiler.warn.restricted.type.not.allowed.preview` | `Warnings.RestrictedTypeNotAllowedPreview` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3510 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3517 |  |
| 572 | `compiler.warn.self.ref` | `Warnings.SelfRef` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4678 |  |
| 573 | `compiler.warn.service.provided.but.not.exported.or.used` | `Warnings.ServiceProvidedButNotExportedOrUsed` | 已接线 | javac-core/src/comp/modules.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Modules.java:1205 |  |
| 574 | `compiler.warn.source.no.bootclasspath` | `Warnings.SourceNoBootclasspath` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:571 |  |
| 575 | `compiler.warn.source.no.system.modules.path` | `Warnings.SourceNoSystemModulesPath` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:574 |  |
| 576 | `compiler.warn.source.target.conflict` | `Warnings.SourceTargetConflict` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:525 |  |
| 577 | `compiler.warn.static.not.qualified.by.type` | `Warnings.StaticNotQualifiedByType` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4381 |  |
| 578 | `compiler.warn.strictfp` | `Warnings.Strictfp` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1332 |  |
| 579 | `compiler.warn.sun.proprietary` | `Warnings.SunProprietary` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3569 |  |
| 580 | `compiler.warn.target.default.source.conflict` | `Warnings.TargetDefaultSourceConflict` | 功能未移植(Main/Options 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/main/Arguments.java:523 |  |
| 581 | `compiler.warn.trailing.white.space.will.be.removed` | `Warnings.TrailingWhiteSpaceWillBeRemoved` | 已接线 | javac-core/src/parser/java_tokenizer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavaTokenizer.java:1063 |  |
| 582 | `compiler.warn.try.explicit.close.call` | `Warnings.TryExplicitCloseCall` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4344 |  |
| 583 | `compiler.warn.try.resource.not.referenced` | `Warnings.TryResourceNotReferenced` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:2520 |  |
| 584 | `compiler.warn.try.resource.throws.interrupted.exc` | `Warnings.TryResourceThrowsInterruptedExc` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1980 |  |
| 585 | `compiler.warn.unchecked.assign` | `Warnings.UncheckedAssign` | 无上游触发 | — | — | 上游无引用 |
| 586 | `compiler.warn.unchecked.assign.to.var` | `Warnings.UncheckedAssignToVar` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4596 |  |
| 587 | `compiler.warn.unchecked.call.mbr.of.raw.type` | `Warnings.UncheckedCallMbrOfRawType` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4789 |  |
| 588 | `compiler.warn.unchecked.cast.to.type` | `Warnings.UncheckedCastToType` | 无上游触发 | — | — | 上游无引用 |
| 589 | `compiler.warn.unchecked.generic.array.creation` | `Warnings.UncheckedGenericArrayCreation` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1053 |  |
| 590 | `compiler.warn.unchecked.meth.invocation.applied` | `Warnings.UncheckedMethInvocationApplied` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4839 |  |
| 591 | `compiler.warn.unchecked.varargs.non.reifiable.type` | `Warnings.UncheckedVarargsNonReifiableType` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:966 |  |
| 592 | `compiler.warn.underscore.as.identifier` | `Warnings.UnderscoreAsIdentifier` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:601 |  |
| 593 | `compiler.warn.unexpected.archive.file` | `Warnings.UnexpectedArchiveFile` | 功能已具备待接线 | file/locations.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/file/Locations.java:392 |  |
| 594 | `compiler.warn.unknown.enum.constant` | `Warnings.UnknownEnumConstant` | 功能已具备待接线 | jvm/class_reader.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:2038 |  |
| 595 | `compiler.warn.unknown.enum.constant.reason` | `Warnings.UnknownEnumConstantReason` | 功能已具备待接线 | jvm/class_reader.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:2033 |  |
| 596 | `compiler.warn.unreachable.catch` | `Warnings.UnreachableCatch` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:1389 |  |
| 597 | `compiler.warn.unreachable.catch.1` | `Warnings.UnreachableCatch1` | 已接线 | javac-core/src/comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:1390 |  |
| 598 | `compiler.warn.varargs.redundant.trustme.anno` | `Warnings.VarargsRedundantTrustmeAnno` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:960 |  |
| 599 | `compiler.warn.varargs.unsafe.use.varargs.param` | `Warnings.VarargsUnsafeUseVarargsParam` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3966 |  |
| 600 | `compiler.warn.warning` | `Warnings.Warning` | 已接线 | javac-core/src/code/preview.rs, javac-core/src/comp/analyzer.rs(等 7 个文件) | — |  |
| 601 | `compiler.note.compressed.diags` | `Notes.CompressedDiags` | 功能已具备待接线 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1720 |  |
| 602 | `compiler.note.deferred.method.inst` | `Notes.DeferredMethodInst` | 功能已具备待接线 | comp/infer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Infer.java:236 |  |
| 603 | `compiler.note.deprecated.filename` | `Notes.DeprecatedFilename` | 无上游触发 | — | — | 上游无引用 |
| 604 | `compiler.note.deprecated.filename.additional` | `Notes.DeprecatedFilenameAdditional` | 无上游触发 | — | — | 上游无引用 |
| 605 | `compiler.note.deprecated.plural` | `Notes.DeprecatedPlural` | 无上游触发 | — | — | 上游无引用 |
| 606 | `compiler.note.deprecated.plural.additional` | `Notes.DeprecatedPluralAdditional` | 无上游触发 | — | — | 上游无引用 |
| 607 | `compiler.note.deprecated.recompile` | `Notes.DeprecatedRecompile` | 无上游触发 | — | — | 上游无引用 |
| 608 | `compiler.note.lambda.stat` | `Notes.LambdaStat` | 无上游触发 | — | — | 上游无引用 |
| 609 | `compiler.note.method.ref.search.results.multi` | `Notes.MethodRefSearchResultsMulti` | 无上游触发 | — | — | 上游无引用 |
| 610 | `compiler.note.mref.stat` | `Notes.MrefStat` | 功能已具备待接线 | comp/lambda_to_method.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/LambdaToMethod.java:1535 |  |
| 611 | `compiler.note.mref.stat.1` | `Notes.MrefStat1` | 无上游触发 | — | — | 上游无引用 |
| 612 | `compiler.note.multiple.elements` | `Notes.MultipleElements` | 功能未移植(Model 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/model/JavacElements.java:236 |  |
| 613 | `compiler.note.note` | `Notes.Note` | 无上游触发 | — | — | 上游无引用 |
| 614 | `compiler.note.preview.filename` | `Notes.PreviewFilename` | 无上游触发 | — | — | 上游无引用 |
| 615 | `compiler.note.preview.filename.additional` | `Notes.PreviewFilenameAdditional` | 无上游触发 | — | — | 上游无引用 |
| 616 | `compiler.note.preview.plural` | `Notes.PreviewPlural` | 无上游触发 | — | — | 上游无引用 |
| 617 | `compiler.note.preview.plural.additional` | `Notes.PreviewPluralAdditional` | 无上游触发 | — | — | 上游无引用 |
| 618 | `compiler.note.preview.recompile` | `Notes.PreviewRecompile` | 无上游触发 | — | — | 上游无引用 |
| 619 | `compiler.note.proc.messager` | `Notes.ProcMessager` | 功能未移植(API 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/api/JavacTrees.java:1278 ; src/jdk.compiler/share/classes/com/sun/tools/javac/processing/JavacMessager.java:135 |  |
| 620 | `compiler.note.removal.filename` | `Notes.RemovalFilename` | 无上游触发 | — | — | 上游无引用 |
| 621 | `compiler.note.removal.filename.additional` | `Notes.RemovalFilenameAdditional` | 无上游触发 | — | — | 上游无引用 |
| 622 | `compiler.note.removal.plural` | `Notes.RemovalPlural` | 无上游触发 | — | — | 上游无引用 |
| 623 | `compiler.note.removal.plural.additional` | `Notes.RemovalPluralAdditional` | 无上游触发 | — | — | 上游无引用 |
| 624 | `compiler.note.removal.recompile` | `Notes.RemovalRecompile` | 无上游触发 | — | — | 上游无引用 |
| 625 | `compiler.note.unchecked.filename` | `Notes.UncheckedFilename` | 无上游触发 | — | — | 上游无引用 |
| 626 | `compiler.note.unchecked.filename.additional` | `Notes.UncheckedFilenameAdditional` | 无上游触发 | — | — | 上游无引用 |
| 627 | `compiler.note.unchecked.plural` | `Notes.UncheckedPlural` | 无上游触发 | — | — | 上游无引用 |
| 628 | `compiler.note.unchecked.plural.additional` | `Notes.UncheckedPluralAdditional` | 无上游触发 | — | — | 上游无引用 |
| 629 | `compiler.note.unchecked.recompile` | `Notes.UncheckedRecompile` | 无上游触发 | — | — | 上游无引用 |
| 630 | `compiler.note.verbose.l2m.deduplicate` | `Notes.VerboseL2mDeduplicate` | 功能已具备待接线 | comp/lambda_to_method.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/LambdaToMethod.java:400 |  |
| 631 | `compiler.note.verbose.resolve.multi` | `Notes.VerboseResolveMulti` | 无上游触发 | — | — | 上游无引用 |
| 632 | `compiler.note.verbose.resolve.multi.1` | `Notes.VerboseResolveMulti1` | 无上游触发 | — | — | 上游无引用 |
| 633 | `compiler.misc.accessor.method.cant.throw.exception` | `Fragments.AccessorMethodCantThrowException` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1074 |  |
| 634 | `compiler.misc.accessor.method.must.not.be.generic` | `Fragments.AccessorMethodMustNotBeGeneric` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1078 |  |
| 635 | `compiler.misc.accessor.method.must.not.be.static` | `Fragments.AccessorMethodMustNotBeStatic` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1082 |  |
| 636 | `compiler.misc.accessor.return.type.doesnt.match` | `Fragments.AccessorReturnTypeDoesntMatch` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1070 |  |
| 637 | `compiler.misc.anachronistic.module.info` | `Fragments.AnachronisticModuleInfo` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 638 | `compiler.misc.anonymous` | `Fragments.Anonymous` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5251 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5310 |  |
| 639 | `compiler.misc.anonymous.class` | `Fragments.AnonymousClass` | 无上游触发 | — | — | 上游无引用 |
| 640 | `compiler.misc.applicable.method.found` | `Fragments.ApplicableMethodFound` | 无上游触发 | — | — | 上游无引用 |
| 641 | `compiler.misc.applicable.method.found.1` | `Fragments.ApplicableMethodFound1` | 无上游触发 | — | — | 上游无引用 |
| 642 | `compiler.misc.applicable.method.found.2` | `Fragments.ApplicableMethodFound2` | 无上游触发 | — | — | 上游无引用 |
| 643 | `compiler.misc.applicable.method.found.3` | `Fragments.ApplicableMethodFound3` | 无上游触发 | — | — | 上游无引用 |
| 644 | `compiler.misc.arg.length.mismatch` | `Fragments.ArgLengthMismatch` | 无上游触发 | — | — | 上游无引用 |
| 645 | `compiler.misc.bad.class.file` | `Fragments.BadClassFile` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 646 | `compiler.misc.bad.class.file.header` | `Fragments.BadClassFileHeader` | 已接线 | javac-core/src/code/class_finder.rs | — |  |
| 647 | `compiler.misc.bad.class.signature` | `Fragments.BadClassSignature` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 648 | `compiler.misc.bad.const.pool.entry` | `Fragments.BadConstPoolEntry` | 无上游触发 | — | — | 上游无引用 |
| 649 | `compiler.misc.bad.const.pool.index` | `Fragments.BadConstPoolIndex` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 650 | `compiler.misc.bad.const.pool.tag` | `Fragments.BadConstPoolTag` | 无上游触发 | — | — | 上游无引用 |
| 651 | `compiler.misc.bad.const.pool.tag.at` | `Fragments.BadConstPoolTagAt` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 652 | `compiler.misc.bad.constant.range` | `Fragments.BadConstantRange` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 653 | `compiler.misc.bad.constant.value` | `Fragments.BadConstantValue` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 654 | `compiler.misc.bad.constant.value.type` | `Fragments.BadConstantValueType` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 655 | `compiler.misc.bad.enclosing.class` | `Fragments.BadEnclosingClass` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 656 | `compiler.misc.bad.enclosing.method` | `Fragments.BadEnclosingMethod` | 已接线 | javac-core/src/jvm/class_reader.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:327 |  |
| 657 | `compiler.misc.bad.instance.method.in.unbound.lookup` | `Fragments.BadInstanceMethodInUnboundLookup` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 658 | `compiler.misc.bad.intersection.target.for.functional.expr` | `Fragments.BadIntersectionTargetForFunctionalExpr` | 无上游触发 | — | — | 上游无引用 |
| 659 | `compiler.misc.bad.module-info.name` | `Fragments.BadModuleInfoName` | 无上游触发 | — | — | 上游无引用 |
| 660 | `compiler.misc.bad.requires.flag` | `Fragments.BadRequiresFlag` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 661 | `compiler.misc.bad.runtime.invisible.param.annotations` | `Fragments.BadRuntimeInvisibleParamAnnotations` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 662 | `compiler.misc.bad.signature` | `Fragments.BadSignature` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 663 | `compiler.misc.bad.source.file.header` | `Fragments.BadSourceFileHeader` | 已接线 | javac-core/src/code/class_finder.rs | — |  |
| 664 | `compiler.misc.bad.static.method.in.bound.lookup` | `Fragments.BadStaticMethodInBoundLookup` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 665 | `compiler.misc.bad.static.method.in.unbound.lookup` | `Fragments.BadStaticMethodInUnboundLookup` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 666 | `compiler.misc.bad.type.annotation.value` | `Fragments.BadTypeAnnotationValue` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 667 | `compiler.misc.base.membership` | `Fragments.BaseMembership` | 无上游触发 | — | — | 上游无引用 |
| 668 | `compiler.misc.bound` | `Fragments.Bound` | 已接线 | javac-core/src/code/types.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:3163 |  |
| 669 | `compiler.misc.canonical` | `Fragments.Canonical` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1107 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1112 |  |
| 670 | `compiler.misc.canonical.cant.have.return.statement` | `Fragments.CanonicalCantHaveReturnStatement` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2426 |  |
| 671 | `compiler.misc.canonical.must.not.contain.explicit.constructor.invocation` | `Fragments.CanonicalMustNotContainExplicitConstructorInvocation` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1126 |  |
| 672 | `compiler.misc.canonical.must.not.declare.type.variables` | `Fragments.CanonicalMustNotDeclareTypeVariables` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1133 |  |
| 673 | `compiler.misc.canonical.must.not.have.stronger.access` | `Fragments.CanonicalMustNotHaveStrongerAccess` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1109 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1114 |  |
| 674 | `compiler.misc.canonical.with.name.mismatch` | `Fragments.CanonicalWithNameMismatch` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1223 |  |
| 675 | `compiler.misc.cant.access.inner.cls.constr` | `Fragments.CantAccessInnerClsConstr` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 676 | `compiler.misc.cant.apply.diamond.1` | `Fragments.CantApplyDiamond1` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3012 |  |
| 677 | `compiler.misc.cant.apply.symbol` | `Fragments.CantApplySymbol` | 无上游触发 | — | — | 上游无引用 |
| 678 | `compiler.misc.cant.apply.symbols` | `Fragments.CantApplySymbols` | 无上游触发 | — | — | 上游无引用 |
| 679 | `compiler.misc.cant.hide` | `Fragments.CantHide` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1840 |  |
| 680 | `compiler.misc.cant.implement` | `Fragments.CantImplement` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1716 |  |
| 681 | `compiler.misc.cant.override` | `Fragments.CantOverride` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1714 |  |
| 682 | `compiler.misc.cant.resolve.args` | `Fragments.CantResolveArgs` | 无上游触发 | — | — | 上游无引用 |
| 683 | `compiler.misc.cant.resolve.location.args` | `Fragments.CantResolveLocationArgs` | 无上游触发 | — | — | 上游无引用 |
| 684 | `compiler.misc.cant.resolve.location.args.params` | `Fragments.CantResolveLocationArgsParams` | 无上游触发 | — | — | 上游无引用 |
| 685 | `compiler.misc.cant.resolve.modules` | `Fragments.CantResolveModules` | 功能已具备待接线 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:818 |  |
| 686 | `compiler.misc.captured.type` | `Fragments.CapturedType` | 无上游触发 | — | — | 上游无引用 |
| 687 | `compiler.misc.clashes.with` | `Fragments.ClashesWith` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1718 |  |
| 688 | `compiler.misc.class.file.not.found` | `Fragments.ClassFileNotFound` | 已接线 | javac-core/src/code/class_finder.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/ClassFinder.java:396 |  |
| 689 | `compiler.misc.class.file.wrong.class` | `Fragments.ClassFileWrongClass` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 690 | `compiler.misc.class.is.not.sealed` | `Fragments.ClassIsNotSealed` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:4019 |  |
| 691 | `compiler.misc.compact` | `Fragments.Compact` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1228 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1231 |  |
| 692 | `compiler.misc.conditional.target.cant.be.void` | `Fragments.ConditionalTargetCantBeVoid` | 已接线 | javac-core/src/comp/argument_attr.rs, javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/ArgumentAttr.java:442 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1996 |  |
| 693 | `compiler.misc.count.error` | `Fragments.CountError` | 无上游触发 | — | — | 上游无引用 |
| 694 | `compiler.misc.count.error.plural` | `Fragments.CountErrorPlural` | 无上游触发 | — | — | 上游无引用 |
| 695 | `compiler.misc.count.error.recompile` | `Fragments.CountErrorRecompile` | 无上游触发 | — | — | 上游无引用 |
| 696 | `compiler.misc.count.warn` | `Fragments.CountWarn` | 无上游触发 | — | — | 上游无引用 |
| 697 | `compiler.misc.count.warn.plural` | `Fragments.CountWarnPlural` | 无上游触发 | — | — | 上游无引用 |
| 698 | `compiler.misc.count.warn.recompile` | `Fragments.CountWarnRecompile` | 无上游触发 | — | — | 上游无引用 |
| 699 | `compiler.misc.descriptor` | `Fragments.Descriptor` | 已接线 | javac-core/src/code/types.rs, javac-core/src/jvm/class_reader.rs | — |  |
| 700 | `compiler.misc.descriptor.throws` | `Fragments.DescriptorThrows` | 无上游触发 | — | — | 上游无引用 |
| 701 | `compiler.misc.diamond` | `Fragments.Diamond` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:53 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3012 |  |
| 702 | `compiler.misc.diamond.and.explicit.params` | `Fragments.DiamondAndExplicitParams` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:861 |  |
| 703 | `compiler.misc.diamond.anonymous.methods.implicitly.override` | `Fragments.DiamondAnonymousMethodsImplicitlyOverride` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2146 |  |
| 704 | `compiler.misc.diamond.invalid.arg` | `Fragments.DiamondInvalidArg` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:54 |  |
| 705 | `compiler.misc.diamond.invalid.args` | `Fragments.DiamondInvalidArgs` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:55 |  |
| 706 | `compiler.misc.diamond.non.generic` | `Fragments.DiamondNonGeneric` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:855 |  |
| 707 | `compiler.misc.doesnt.extend.sealed` | `Fragments.DoesntExtendSealed` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5287 |  |
| 708 | `compiler.misc.eq.bounds` | `Fragments.EqBounds` | 已接线 | javac-core/src/comp/infer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Infer.java:1284 |  |
| 709 | `compiler.misc.exception.message` | `Fragments.ExceptionMessage` | 已接线 | javac-core/src/code/class_finder.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/ClassFinder.java:304 ; src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:3190 |  |
| 710 | `compiler.misc.explicit.param.do.not.conform.to.bounds` | `Fragments.ExplicitParamDoNotConformToBounds` | 已接线 | javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:598 |  |
| 711 | `compiler.misc.fatal.err.cant.close` | `Fragments.FatalErrCantClose` | 功能已具备待接线 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:1801 |  |
| 712 | `compiler.misc.fatal.err.cant.locate.ctor` | `Fragments.FatalErrCantLocateCtor` | 功能已具备待接线 | comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:2884 |  |
| 713 | `compiler.misc.fatal.err.cant.locate.field` | `Fragments.FatalErrCantLocateField` | 功能已具备待接线 | comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:1476 |  |
| 714 | `compiler.misc.fatal.err.cant.locate.meth` | `Fragments.FatalErrCantLocateMeth` | 功能已具备待接线 | comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:2832 |  |
| 715 | `compiler.misc.fatal.err.no.java.lang` | `Fragments.FatalErrNoJavaLang` | 已接线 | javac-core/src/comp/type_enter.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/TypeEnter.java:360 |  |
| 716 | `compiler.misc.feature.annotations.after.type.params` | `Fragments.FeatureAnnotationsAfterTypeParams` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:205 |  |
| 717 | `compiler.misc.feature.case.null` | `Fragments.FeatureCaseNull` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:227 |  |
| 718 | `compiler.misc.feature.default.methods` | `Fragments.FeatureDefaultMethods` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:199 |  |
| 719 | `compiler.misc.feature.diamond` | `Fragments.FeatureDiamond` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:192 |  |
| 720 | `compiler.misc.feature.diamond.and.anon.class` | `Fragments.FeatureDiamondAndAnonClass` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:213 |  |
| 721 | `compiler.misc.feature.intersection.types.in.cast` | `Fragments.FeatureIntersectionTypesInCast` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:207 |  |
| 722 | `compiler.misc.feature.lambda` | `Fragments.FeatureLambda` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:197 |  |
| 723 | `compiler.misc.feature.method.references` | `Fragments.FeatureMethodReferences` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:198 |  |
| 724 | `compiler.misc.feature.modules` | `Fragments.FeatureModules` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:193 |  |
| 725 | `compiler.misc.feature.multiple.case.labels` | `Fragments.FeatureMultipleCaseLabels` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:219 |  |
| 726 | `compiler.misc.feature.not.supported.in.source` | `Fragments.FeatureNotSupportedInSource` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:279 |  |
| 727 | `compiler.misc.feature.not.supported.in.source.plural` | `Fragments.FeatureNotSupportedInSourcePlural` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:280 |  |
| 728 | `compiler.misc.feature.pattern.matching.instanceof` | `Fragments.FeaturePatternMatchingInstanceof` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:223 |  |
| 729 | `compiler.misc.feature.pattern.switch` | `Fragments.FeaturePatternSwitch` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:228 |  |
| 730 | `compiler.misc.feature.private.intf.methods` | `Fragments.FeaturePrivateIntfMethods` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:215 |  |
| 731 | `compiler.misc.feature.records` | `Fragments.FeatureRecords` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:225 |  |
| 732 | `compiler.misc.feature.reifiable.types.instanceof` | `Fragments.FeatureReifiableTypesInstanceof` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:224 |  |
| 733 | `compiler.misc.feature.repeatable.annotations` | `Fragments.FeatureRepeatableAnnotations` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:206 |  |
| 734 | `compiler.misc.feature.sealed.classes` | `Fragments.FeatureSealedClasses` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:226 |  |
| 735 | `compiler.misc.feature.static.intf.method.invoke` | `Fragments.FeatureStaticIntfMethodInvoke` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:201 |  |
| 736 | `compiler.misc.feature.static.intf.methods` | `Fragments.FeatureStaticIntfMethods` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:200 |  |
| 737 | `compiler.misc.feature.switch.expressions` | `Fragments.FeatureSwitchExpressions` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:221 |  |
| 738 | `compiler.misc.feature.switch.rules` | `Fragments.FeatureSwitchRules` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:220 |  |
| 739 | `compiler.misc.feature.text.blocks` | `Fragments.FeatureTextBlocks` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:222 |  |
| 740 | `compiler.misc.feature.type.annotations` | `Fragments.FeatureTypeAnnotations` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:204 |  |
| 741 | `compiler.misc.feature.var.in.try.with.resources` | `Fragments.FeatureVarInTryWithResources` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:194 |  |
| 742 | `compiler.misc.feature.var.syntax.in.implicit.lambda` | `Fragments.FeatureVarSyntaxInImplicitLambda` | 已接线 | javac-core/src/code/source.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Source.java:217 |  |
| 743 | `compiler.misc.file.does.not.contain.module` | `Fragments.FileDoesNotContainModule` | 已接线 | javac-core/src/code/module_finder.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/ModuleFinder.java:231 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:838 |  |
| 744 | `compiler.misc.file.does.not.contain.package` | `Fragments.FileDoesNotContainPackage` | 功能已具备待接线 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:844 |  |
| 745 | `compiler.misc.file.doesnt.contain.class` | `Fragments.FileDoesntContainClass` | 功能已具备待接线 | main/java_compiler.rs (同名文件,不同目录) | src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:849 |  |
| 746 | `compiler.misc.guard` | `Fragments.Guard` | 功能已具备待接线 | comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:2957 |  |
| 747 | `compiler.misc.illegal.signature` | `Fragments.IllegalSignature` | 功能已具备待接线 | comp/lambda_to_method.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/LambdaToMethod.java:2396 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/LambdaToMethod.java:2408 |  |
| 748 | `compiler.misc.illegal.start.of.class.file` | `Fragments.IllegalStartOfClassFile` | 已接线 | javac-core/src/code/deferred_completion_failure_handler.rs, javac-core/src/code/symbol.rs(等 3 个文件) | — |  |
| 749 | `compiler.misc.implicit.and.explicit.not.allowed` | `Fragments.ImplicitAndExplicitNotAllowed` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:61 |  |
| 750 | `compiler.misc.inaccessible.varargs.type` | `Fragments.InaccessibleVarargsType` | 已接线 | javac-core/src/comp/resolve.rs | — |  |
| 751 | `compiler.misc.inapplicable.method` | `Fragments.InapplicableMethod` | 功能已具备待接线 | comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:4285 |  |
| 752 | `compiler.misc.incompatible.abstract.default` | `Fragments.IncompatibleAbstractDefault` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2659 |  |
| 753 | `compiler.misc.incompatible.abstracts` | `Fragments.IncompatibleAbstracts` | 已接线 | javac-core/src/code/types.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Types.java:754 |  |
| 754 | `compiler.misc.incompatible.arg.types.in.lambda` | `Fragments.IncompatibleArgTypesInLambda` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/comp/deferred_attr.rs(等 3 个文件) | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3153 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3472 |  |
| 755 | `compiler.misc.incompatible.arg.types.in.mref` | `Fragments.IncompatibleArgTypesInMref` | 已接线 | javac-core/src/comp/deferred_attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/DeferredAttr.java:941 |  |
| 756 | `compiler.misc.incompatible.bounds` | `Fragments.IncompatibleBounds` | 已接线 | javac-core/src/comp/infer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Infer.java:1276 |  |
| 757 | `compiler.misc.incompatible.descs.in.functional.intf` | `Fragments.IncompatibleDescsInFunctionalIntf` | 已接线 | javac-core/src/code/types.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Types.java:777 |  |
| 758 | `compiler.misc.incompatible.diff.ret` | `Fragments.IncompatibleDiffRet` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2063 |  |
| 759 | `compiler.misc.incompatible.eq.bounds` | `Fragments.IncompatibleEqBounds` | 已接线 | javac-core/src/comp/infer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Infer.java:1264 |  |
| 760 | `compiler.misc.incompatible.ret.type.in.lambda` | `Fragments.IncompatibleRetTypeInLambda` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/comp/attr_recover.rs(等 3 个文件) | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3403 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3465 |  |
| 761 | `compiler.misc.incompatible.ret.type.in.mref` | `Fragments.IncompatibleRetTypeInMref` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3795 |  |
| 762 | `compiler.misc.incompatible.type.in.conditional` | `Fragments.IncompatibleTypeInConditional` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2099 |  |
| 763 | `compiler.misc.incompatible.type.in.switch.expression` | `Fragments.IncompatibleTypeInSwitchExpression` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1661 |  |
| 764 | `compiler.misc.incompatible.unrelated.defaults` | `Fragments.IncompatibleUnrelatedDefaults` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:2653 |  |
| 765 | `compiler.misc.incompatible.upper.bounds` | `Fragments.IncompatibleUpperBounds` | 已接线 | javac-core/src/comp/infer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Infer.java:1266 |  |
| 766 | `compiler.misc.inconvertible.types` | `Fragments.InconvertibleTypes` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3795 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4121 |  |
| 767 | `compiler.misc.infer.arg.length.mismatch` | `Fragments.InferArgLengthMismatch` | 无上游触发 | — | — | 上游无引用 |
| 768 | `compiler.misc.infer.no.conforming.assignment.exists` | `Fragments.InferNoConformingAssignmentExists` | 无上游触发 | — | — | 上游无引用 |
| 769 | `compiler.misc.infer.no.conforming.instance.exists` | `Fragments.InferNoConformingInstanceExists` | 已接线 | javac-core/src/comp/infer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Infer.java:428 |  |
| 770 | `compiler.misc.infer.varargs.argument.mismatch` | `Fragments.InferVarargsArgumentMismatch` | 无上游触发 | — | — | 上游无引用 |
| 771 | `compiler.misc.inferred.do.not.conform.to.eq.bounds` | `Fragments.InferredDoNotConformToEqBounds` | 已接线 | javac-core/src/comp/infer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Infer.java:1250 |  |
| 772 | `compiler.misc.inferred.do.not.conform.to.lower.bounds` | `Fragments.InferredDoNotConformToLowerBounds` | 已接线 | javac-core/src/comp/infer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Infer.java:1252 |  |
| 773 | `compiler.misc.inferred.do.not.conform.to.upper.bounds` | `Fragments.InferredDoNotConformToUpperBounds` | 已接线 | javac-core/src/comp/infer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Infer.java:1254 |  |
| 774 | `compiler.misc.inner.cls` | `Fragments.InnerCls` | 功能已具备待接线 | comp/flow.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:2958 |  |
| 775 | `compiler.misc.intersection.type` | `Fragments.IntersectionType` | 无上游触发 | — | — | 上游无引用 |
| 776 | `compiler.misc.invalid.default.interface` | `Fragments.InvalidDefaultInterface` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 777 | `compiler.misc.invalid.generic.lambda.target` | `Fragments.InvalidGenericLambdaTarget` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3115 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3282 |  |
| 778 | `compiler.misc.invalid.mref` | `Fragments.InvalidMref` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3646 |  |
| 779 | `compiler.misc.invalid.static.interface` | `Fragments.InvalidStaticInterface` | 无上游触发 | — | — | 上游无引用 |
| 780 | `compiler.misc.is.a.type.variable` | `Fragments.IsATypeVariable` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5248 |  |
| 781 | `compiler.misc.is.duplicated` | `Fragments.IsDuplicated` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5258 |  |
| 782 | `compiler.misc.kindname.annotation` | `Fragments.KindnameAnnotation` | 无上游触发 | — | — | 上游无引用 |
| 783 | `compiler.misc.kindname.class` | `Fragments.KindnameClass` | 无上游触发 | — | — | 上游无引用 |
| 784 | `compiler.misc.kindname.constructor` | `Fragments.KindnameConstructor` | 无上游触发 | — | — | 上游无引用 |
| 785 | `compiler.misc.kindname.enum` | `Fragments.KindnameEnum` | 无上游触发 | — | — | 上游无引用 |
| 786 | `compiler.misc.kindname.instance.init` | `Fragments.KindnameInstanceInit` | 无上游触发 | — | — | 上游无引用 |
| 787 | `compiler.misc.kindname.interface` | `Fragments.KindnameInterface` | 无上游触发 | — | — | 上游无引用 |
| 788 | `compiler.misc.kindname.method` | `Fragments.KindnameMethod` | 无上游触发 | — | — | 上游无引用 |
| 789 | `compiler.misc.kindname.module` | `Fragments.KindnameModule` | 无上游触发 | — | — | 上游无引用 |
| 790 | `compiler.misc.kindname.package` | `Fragments.KindnamePackage` | 无上游触发 | — | — | 上游无引用 |
| 791 | `compiler.misc.kindname.record` | `Fragments.KindnameRecord` | 无上游触发 | — | — | 上游无引用 |
| 792 | `compiler.misc.kindname.record.component` | `Fragments.KindnameRecordComponent` | 无上游触发 | — | — | 上游无引用 |
| 793 | `compiler.misc.kindname.static` | `Fragments.KindnameStatic` | 无上游触发 | — | — | 上游无引用 |
| 794 | `compiler.misc.kindname.static.init` | `Fragments.KindnameStaticInit` | 无上游触发 | — | — | 上游无引用 |
| 795 | `compiler.misc.kindname.type.variable` | `Fragments.KindnameTypeVariable` | 无上游触发 | — | — | 上游无引用 |
| 796 | `compiler.misc.kindname.type.variable.bound` | `Fragments.KindnameTypeVariableBound` | 无上游触发 | — | — | 上游无引用 |
| 797 | `compiler.misc.kindname.value` | `Fragments.KindnameValue` | 无上游触发 | — | — | 上游无引用 |
| 798 | `compiler.misc.kindname.variable` | `Fragments.KindnameVariable` | 无上游触发 | — | — | 上游无引用 |
| 799 | `compiler.misc.lambda` | `Fragments.Lambda` | 已接线 | javac-core/src/comp/deferred_attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Flow.java:2956 |  |
| 800 | `compiler.misc.local` | `Fragments.Local` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5310 |  |
| 801 | `compiler.misc.local.array.missing.target` | `Fragments.LocalArrayMissingTarget` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1368 |  |
| 802 | `compiler.misc.local.cant.infer.null` | `Fragments.LocalCantInferNull` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:980 |  |
| 803 | `compiler.misc.local.cant.infer.void` | `Fragments.LocalCantInferVoid` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:983 |  |
| 804 | `compiler.misc.local.lambda.missing.target` | `Fragments.LocalLambdaMissingTarget` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1375 |  |
| 805 | `compiler.misc.local.missing.init` | `Fragments.LocalMissingInit` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1262 |  |
| 806 | `compiler.misc.local.mref.missing.target` | `Fragments.LocalMrefMissingTarget` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1393 |  |
| 807 | `compiler.misc.local.self.ref` | `Fragments.LocalSelfRef` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4581 |  |
| 808 | `compiler.misc.location` | `Fragments.Location` | 已接线 | javac-core/src/code/symbol.rs, javac-core/src/comp/check.rs(等 3 个文件) | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:4018 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:4092 |  |
| 809 | `compiler.misc.location.1` | `Fragments.Location1` | 已接线 | javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:4088 |  |
| 810 | `compiler.misc.locn.module_path` | `Fragments.LocnModule_path` | 已接线 | javac-core/src/code/module_finder.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/ModuleFinder.java:402 |  |
| 811 | `compiler.misc.locn.module_source_path` | `Fragments.LocnModule_source_path` | 已接线 | javac-core/src/code/module_finder.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/ModuleFinder.java:403 |  |
| 812 | `compiler.misc.locn.system_modules` | `Fragments.LocnSystem_modules` | 已接线 | javac-core/src/code/module_finder.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/ModuleFinder.java:404 |  |
| 813 | `compiler.misc.locn.upgrade_module_path` | `Fragments.LocnUpgrade_module_path` | 已接线 | javac-core/src/code/module_finder.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/ModuleFinder.java:405 |  |
| 814 | `compiler.misc.lower.bounds` | `Fragments.LowerBounds` | 已接线 | javac-core/src/comp/infer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Infer.java:1285 |  |
| 815 | `compiler.misc.malformed.vararg.method` | `Fragments.MalformedVarargMethod` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 816 | `compiler.misc.method.descriptor.invalid` | `Fragments.MethodDescriptorInvalid` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 817 | `compiler.misc.method.must.be.public` | `Fragments.MethodMustBePublic` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1066 |  |
| 818 | `compiler.misc.missing.ret.val` | `Fragments.MissingRetVal` | 已接线 | javac-core/src/comp/argument_attr.rs, javac-core/src/comp/attr.rs(等 4 个文件) | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2439 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3465 |  |
| 819 | `compiler.misc.module.info.definition.expected` | `Fragments.ModuleInfoDefinitionExpected` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 820 | `compiler.misc.module.info.invalid.super.class` | `Fragments.ModuleInfoInvalidSuperClass` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 821 | `compiler.misc.module.name.mismatch` | `Fragments.ModuleNameMismatch` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 822 | `compiler.misc.module.non.zero.opens` | `Fragments.ModuleNonZeroOpens` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 823 | `compiler.misc.mref.infer.and.explicit.params` | `Fragments.MrefInferAndExplicitParams` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3563 |  |
| 824 | `compiler.misc.must.not.be.same.class` | `Fragments.MustNotBeSameClass` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5277 |  |
| 825 | `compiler.misc.must.not.be.supertype` | `Fragments.MustNotBeSupertype` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5278 |  |
| 826 | `compiler.misc.no.abstracts` | `Fragments.NoAbstracts` | 已接线 | javac-core/src/code/types.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Types.java:760 |  |
| 827 | `compiler.misc.no.args` | `Fragments.NoArgs` | 无上游触发 | — | — | 上游无引用 |
| 828 | `compiler.misc.no.conforming.assignment.exists` | `Fragments.NoConformingAssignmentExists` | 无上游触发 | — | — | 上游无引用 |
| 829 | `compiler.misc.no.suitable.functional.intf.inst` | `Fragments.NoSuitableFunctionalIntfInst` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/comp/infer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Types.java:686 ; src/jdk.compiler/share/classes/com/sun/tools/javac/code/Types.java:693 |  |
| 830 | `compiler.misc.no.unique.maximal.instance.exists` | `Fragments.NoUniqueMaximalInstanceExists` | 已接线 | javac-core/src/comp/infer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Infer.java:1484 |  |
| 831 | `compiler.misc.no.unique.minimal.instance.exists` | `Fragments.NoUniqueMinimalInstanceExists` | 已接线 | javac-core/src/comp/infer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Infer.java:1443 |  |
| 832 | `compiler.misc.non.static` | `Fragments.NonStatic` | 功能已具备待接线 | comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:3153 |  |
| 833 | `compiler.misc.not.a.functional.intf` | `Fragments.NotAFunctionalIntf` | 已接线 | javac-core/src/code/types.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3082 |  |
| 834 | `compiler.misc.not.a.functional.intf.1` | `Fragments.NotAFunctionalIntf1` | 无上游触发 | — | — | 上游无引用 |
| 835 | `compiler.misc.not.an.intf.component` | `Fragments.NotAnIntfComponent` | 无上游触发 | — | — | 上游无引用 |
| 836 | `compiler.misc.not.applicable.method.found` | `Fragments.NotApplicableMethodFound` | 功能已具备待接线 | comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:278 |  |
| 837 | `compiler.misc.not.def.access.class.intf.cant.access` | `Fragments.NotDefAccessClassIntfCantAccess` | 无上游触发 | — | — | 上游无引用 |
| 838 | `compiler.misc.not.def.access.class.intf.cant.access.reason` | `Fragments.NotDefAccessClassIntfCantAccessReason` | 无上游触发 | — | — | 上游无引用 |
| 839 | `compiler.misc.not.def.access.does.not.read` | `Fragments.NotDefAccessDoesNotRead` | 已接线 | javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:4482 |  |
| 840 | `compiler.misc.not.def.access.does.not.read.from.unnamed` | `Fragments.NotDefAccessDoesNotReadFromUnnamed` | 已接线 | javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:4486 |  |
| 841 | `compiler.misc.not.def.access.does.not.read.unnamed` | `Fragments.NotDefAccessDoesNotReadUnnamed` | 已接线 | javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:4490 |  |
| 842 | `compiler.misc.not.def.access.not.exported` | `Fragments.NotDefAccessNotExported` | 已接线 | javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:4507 |  |
| 843 | `compiler.misc.not.def.access.not.exported.from.unnamed` | `Fragments.NotDefAccessNotExportedFromUnnamed` | 已接线 | javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:4510 |  |
| 844 | `compiler.misc.not.def.access.not.exported.to.module` | `Fragments.NotDefAccessNotExportedToModule` | 已接线 | javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:4497 |  |
| 845 | `compiler.misc.not.def.access.not.exported.to.module.from.unnamed` | `Fragments.NotDefAccessNotExportedToModuleFromUnnamed` | 已接线 | javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:4501 |  |
| 846 | `compiler.misc.not.def.access.package.cant.access` | `Fragments.NotDefAccessPackageCantAccess` | 无上游触发 | — | — | 上游无引用 |
| 847 | `compiler.misc.not.def.public.cant.access` | `Fragments.NotDefPublicCantAccess` | 无上游触发 | — | — | 上游无引用 |
| 848 | `compiler.misc.overridden.default` | `Fragments.OverriddenDefault` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4803 |  |
| 849 | `compiler.misc.package.not.visible` | `Fragments.PackageNotVisible` | 无上游触发 | — | — | 上游无引用 |
| 850 | `compiler.misc.partial.inst.sig` | `Fragments.PartialInstSig` | 功能已具备待接线 | comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:267 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:3146 |  |
| 851 | `compiler.misc.possible.loss.of.precision` | `Fragments.PossibleLossOfPrecision` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:618 |  |
| 852 | `compiler.misc.prob.found.req` | `Fragments.ProbFoundReq` | 无上游触发 | — | — | 上游无引用 |
| 853 | `compiler.misc.redundant.supertype` | `Fragments.RedundantSupertype` | 已接线 | javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:3759 |  |
| 854 | `compiler.misc.ref.ambiguous` | `Fragments.RefAmbiguous` | 无上游触发 | — | — | 上游无引用 |
| 855 | `compiler.misc.report.access` | `Fragments.ReportAccess` | 无上游触发 | — | — | 上游无引用 |
| 856 | `compiler.misc.resume.abort` | `Fragments.ResumeAbort` | 无上游触发 | — | — | 上游无引用 |
| 857 | `compiler.misc.source.unavailable` | `Fragments.SourceUnavailable` | 无上游触发 | — | — | 上游无引用 |
| 858 | `compiler.misc.stat.expr.expected` | `Fragments.StatExprExpected` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3420 |  |
| 859 | `compiler.misc.static.mref.with.targs` | `Fragments.StaticMrefWithTargs` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:3697 |  |
| 860 | `compiler.misc.switch.expression.target.cant.be.void` | `Fragments.SwitchExpressionTargetCantBeVoid` | 已接线 | javac-core/src/comp/argument_attr.rs, javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/ArgumentAttr.java:478 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1611 |  |
| 861 | `compiler.misc.synthetic.name.conflict` | `Fragments.SyntheticNameConflict` | 功能已具备待接线 | comp/lower.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Lower.java:774 |  |
| 862 | `compiler.misc.throws.clause.not.allowed.for.canonical.constructor` | `Fragments.ThrowsClauseNotAllowedForCanonicalConstructor` | 已接线 | javac-core/src/comp/attr.rs, javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1230 ; src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:4325 |  |
| 863 | `compiler.misc.token.bad-symbol` | `Fragments.TokenBadSymbol` | 无上游触发 | — | — | 上游无引用 |
| 864 | `compiler.misc.token.character` | `Fragments.TokenCharacter` | 无上游触发 | — | — | 上游无引用 |
| 865 | `compiler.misc.token.double` | `Fragments.TokenDouble` | 无上游触发 | — | — | 上游无引用 |
| 866 | `compiler.misc.token.end-of-input` | `Fragments.TokenEndOfInput` | 无上游触发 | — | — | 上游无引用 |
| 867 | `compiler.misc.token.float` | `Fragments.TokenFloat` | 无上游触发 | — | — | 上游无引用 |
| 868 | `compiler.misc.token.identifier` | `Fragments.TokenIdentifier` | 无上游触发 | — | — | 上游无引用 |
| 869 | `compiler.misc.token.integer` | `Fragments.TokenInteger` | 无上游触发 | — | — | 上游无引用 |
| 870 | `compiler.misc.token.long-integer` | `Fragments.TokenLongInteger` | 无上游触发 | — | — | 上游无引用 |
| 871 | `compiler.misc.token.string` | `Fragments.TokenString` | 无上游触发 | — | — | 上游无引用 |
| 872 | `compiler.misc.try.not.applicable.to.type` | `Fragments.TryNotApplicableToType` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1900 |  |
| 873 | `compiler.misc.type.captureof` | `Fragments.TypeCaptureof` | 无上游触发 | — | — | 上游无引用 |
| 874 | `compiler.misc.type.captureof.1` | `Fragments.TypeCaptureof1` | 无上游触发 | — | — | 上游无引用 |
| 875 | `compiler.misc.type.must.be.identical.to.corresponding.record.component.type` | `Fragments.TypeMustBeIdenticalToCorrespondingRecordComponentType` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1147 |  |
| 876 | `compiler.misc.type.none` | `Fragments.TypeNone` | 无上游触发 | — | — | 上游无引用 |
| 877 | `compiler.misc.type.null` | `Fragments.TypeNull` | 无上游触发 | — | — | 上游无引用 |
| 878 | `compiler.misc.type.parameter` | `Fragments.TypeParameter` | 功能已具备待接线 | comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:735 |  |
| 879 | `compiler.misc.type.req.array.or.iterable` | `Fragments.TypeReqArrayOrIterable` | 已接线 | javac-core/src/comp/attr.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1548 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1562 |  |
| 880 | `compiler.misc.type.req.class` | `Fragments.TypeReqClass` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:726 |  |
| 881 | `compiler.misc.type.req.class.array` | `Fragments.TypeReqClassArray` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:712 |  |
| 882 | `compiler.misc.type.req.exact` | `Fragments.TypeReqExact` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:774 |  |
| 883 | `compiler.misc.type.req.ref` | `Fragments.TypeReqRef` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:792 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:819 |  |
| 884 | `compiler.misc.type.var` | `Fragments.TypeVar` | 无上游触发 | — | — | 上游无引用 |
| 885 | `compiler.misc.unable.to.access.file` | `Fragments.UnableToAccessFile` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 886 | `compiler.misc.unbound` | `Fragments.Unbound` | 功能已具备待接线 | comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:3163 |  |
| 887 | `compiler.misc.unchecked.assign` | `Fragments.UncheckedAssign` | 无上游触发 | — | — | 上游无引用 |
| 888 | `compiler.misc.unchecked.cast.to.type` | `Fragments.UncheckedCastToType` | 无上游触发 | — | — | 上游无引用 |
| 889 | `compiler.misc.unchecked.clash.with` | `Fragments.UncheckedClashWith` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1735 |  |
| 890 | `compiler.misc.unchecked.implement` | `Fragments.UncheckedImplement` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1733 |  |
| 891 | `compiler.misc.unchecked.override` | `Fragments.UncheckedOverride` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1731 |  |
| 892 | `compiler.misc.undecl.type.var` | `Fragments.UndeclTypeVar` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 893 | `compiler.misc.unexpected.const.pool.tag.at` | `Fragments.UnexpectedConstPoolTagAt` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 894 | `compiler.misc.unexpected.ret.val` | `Fragments.UnexpectedRetVal` | 已接线 | javac-core/src/comp/argument_attr.rs, javac-core/src/comp/attr.rs(等 3 个文件) | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:2433 ; src/jdk.compiler/share/classes/com/sun/tools/javac/comp/DeferredAttr.java:855 |  |
| 895 | `compiler.misc.unicode.str.not.supported` | `Fragments.UnicodeStrNotSupported` | 无上游触发 | — | — | 上游无引用 |
| 896 | `compiler.misc.unnamed.module` | `Fragments.UnnamedModule` | 无上游触发 | — | — | 上游无引用 |
| 897 | `compiler.misc.unnamed.package` | `Fragments.UnnamedPackage` | 无上游触发 | — | — | 上游无引用 |
| 898 | `compiler.misc.upper.bounds` | `Fragments.UpperBounds` | 已接线 | javac-core/src/comp/infer.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Infer.java:1286 |  |
| 899 | `compiler.misc.user.selected.completion.failure` | `Fragments.UserSelectedCompletionFailure` | 已接线 | javac-core/src/code/class_finder.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/code/ClassFinder.java:344 ; src/jdk.compiler/share/classes/com/sun/tools/javac/main/JavaCompiler.java:790 |  |
| 900 | `compiler.misc.var.and.explicit.not.allowed` | `Fragments.VarAndExplicitNotAllowed` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:62 |  |
| 901 | `compiler.misc.var.and.implicit.not.allowed` | `Fragments.VarAndImplicitNotAllowed` | 已接线 | javac-core/src/parser/javac_parser.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:63 |  |
| 902 | `compiler.misc.varargs.argument.mismatch` | `Fragments.VarargsArgumentMismatch` | 无上游触发 | — | — | 上游无引用 |
| 903 | `compiler.misc.varargs.clash.with` | `Fragments.VarargsClashWith` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1752 |  |
| 904 | `compiler.misc.varargs.implement` | `Fragments.VarargsImplement` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1750 |  |
| 905 | `compiler.misc.varargs.override` | `Fragments.VarargsOverride` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:1748 |  |
| 906 | `compiler.misc.varargs.trustme.on.non.varargs.accessor` | `Fragments.VarargsTrustmeOnNonVarargsAccessor` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:3073 |  |
| 907 | `compiler.misc.varargs.trustme.on.non.varargs.meth` | `Fragments.VarargsTrustmeOnNonVarargsMeth` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:956 |  |
| 908 | `compiler.misc.varargs.trustme.on.reifiable.varargs` | `Fragments.VarargsTrustmeOnReifiableVarargs` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:962 |  |
| 909 | `compiler.misc.varargs.trustme.on.virtual.varargs` | `Fragments.VarargsTrustmeOnVirtualVarargs` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:948 |  |
| 910 | `compiler.misc.varargs.trustme.on.virtual.varargs.final.only` | `Fragments.VarargsTrustmeOnVirtualVarargsFinalOnly` | 已接线 | javac-core/src/comp/check.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Check.java:949 |  |
| 911 | `compiler.misc.verbose.checking.attribution` | `Fragments.VerboseCheckingAttribution` | 无上游触发 | — | — | 上游无引用 |
| 912 | `compiler.misc.verbose.classpath` | `Fragments.VerboseClasspath` | 无上游触发 | — | — | 上游无引用 |
| 913 | `compiler.misc.verbose.loading` | `Fragments.VerboseLoading` | 无上游触发 | — | — | 上游无引用 |
| 914 | `compiler.misc.verbose.parsing.done` | `Fragments.VerboseParsingDone` | 无上游触发 | — | — | 上游无引用 |
| 915 | `compiler.misc.verbose.parsing.started` | `Fragments.VerboseParsingStarted` | 无上游触发 | — | — | 上游无引用 |
| 916 | `compiler.misc.verbose.sourcepath` | `Fragments.VerboseSourcepath` | 无上游触发 | — | — | 上游无引用 |
| 917 | `compiler.misc.verbose.total` | `Fragments.VerboseTotal` | 无上游触发 | — | — | 上游无引用 |
| 918 | `compiler.misc.verbose.wrote.file` | `Fragments.VerboseWroteFile` | 无上游触发 | — | — | 上游无引用 |
| 919 | `compiler.misc.version.not.available` | `Fragments.VersionNotAvailable` | 无上游触发 | — | — | 上游无引用 |
| 920 | `compiler.misc.where.captured` | `Fragments.WhereCaptured` | 无上游触发 | — | — | 上游无引用 |
| 921 | `compiler.misc.where.captured.1` | `Fragments.WhereCaptured1` | 无上游触发 | — | — | 上游无引用 |
| 922 | `compiler.misc.where.description.captured` | `Fragments.WhereDescriptionCaptured` | 无上游触发 | — | — | 上游无引用 |
| 923 | `compiler.misc.where.description.captured.1` | `Fragments.WhereDescriptionCaptured1` | 无上游触发 | — | — | 上游无引用 |
| 924 | `compiler.misc.where.description.intersection` | `Fragments.WhereDescriptionIntersection` | 无上游触发 | — | — | 上游无引用 |
| 925 | `compiler.misc.where.description.intersection.1` | `Fragments.WhereDescriptionIntersection1` | 无上游触发 | — | — | 上游无引用 |
| 926 | `compiler.misc.where.description.typevar` | `Fragments.WhereDescriptionTypevar` | 无上游触发 | — | — | 上游无引用 |
| 927 | `compiler.misc.where.description.typevar.1` | `Fragments.WhereDescriptionTypevar1` | 无上游触发 | — | — | 上游无引用 |
| 928 | `compiler.misc.where.fresh.typevar` | `Fragments.WhereFreshTypevar` | 功能未移植(Util 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/util/RichDiagnosticFormatter.java:585 |  |
| 929 | `compiler.misc.where.intersection` | `Fragments.WhereIntersection` | 功能未移植(Util 批) | — | src/jdk.compiler/share/classes/com/sun/tools/javac/util/RichDiagnosticFormatter.java:524 |  |
| 930 | `compiler.misc.where.typevar` | `Fragments.WhereTypevar` | 无上游触发 | — | — | 上游无引用 |
| 931 | `compiler.misc.where.typevar.1` | `Fragments.WhereTypevar1` | 无上游触发 | — | — | 上游无引用 |
| 932 | `compiler.misc.wrong.number.type.args` | `Fragments.WrongNumberTypeArgs` | 已接线 | javac-core/src/comp/resolve.rs | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:589 |  |
| 933 | `null` | `Fragments.WrongVersion` | 已接线 | javac-core/src/jvm/class_reader.rs | — |  |
| 934 | `compiler.misc.x.print.processor.info` | `Fragments.XPrintProcessorInfo` | 无上游触发 | — | — | 上游无引用 |
| 935 | `compiler.misc.x.print.rounds` | `Fragments.XPrintRounds` | 无上游触发 | — | — | 上游无引用 |

## 结论

1. 已接线 639 条;真实待接线 67 条(本地功能文件存在,按批次 D1-D7 接线);功能未移植 40 条(Main/Options 32、API 5、Model 1、Util 2);范围外 24 条(processing,PLAN §3.1);无上游触发 165 条(不得盲目接线,需先核对 properties/历史键)。
2. 待接线优先级(按计划批次):D1 Attr.multicatch(`compiler.err.multicatch.types.must.be.disjoint`);D2 Attr try-with-resources(`compiler.err.try.resource.may.not.be.assigned`);D3 Flow guard/`inner.cls`(2);D4 Resolve verbose 5 + fatal 3(8);D5 ClassReader(5);D6 Locations(8);D7 其余 42 条(JavaCompiler 16、Option 10、LambdaToMethod 3、Annotate 2、JavacParser 2、Main 3、FileManager 2、Infer/Check/Lower/Flow 等)。
3. 范围外条目(processing 24 条)按 PLAN §3.1 不接线;若未来恢复 APT 需整体重开批次。
4. 本表由脚本重算:解析 compiler_properties.rs(935 工厂)、本地 crates/**/*.rs(496 文件,排除生成资源文件)、上游 reference/jdk17u(5902 Java 文件)诊断引用。第二版全量行已由本版替代;两者差异见统计节与更正说明。

重读依据:本表生成前重新读取 compiler_properties.rs 全量工厂、A0/A1/A0b 提交后的本地调用点、reference/jdk17u 诊断引用与 docs/PLAN.md §3.1/§3.2;统计数字由脚本一次运行输出(不手改)。

