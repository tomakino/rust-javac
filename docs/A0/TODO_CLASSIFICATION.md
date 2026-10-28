# TODO 分类清单（前置债务批次，2026-10-28）

## 统计口径

- 范围：`crates/**/*.rs`（大小写敏感 `TODO`，排除标识符 `todo`/`Todo`）。
- 结果：TODO 注释行 **247**；其中带括号批次标签（`TODO(...)`）**166**，无括号标签 **81**。
- 同批统计：FIXME **2**、HACK **4**、`unimplemented!` **0**、`todo!()` **0**。
- 说明：`TODO_CLASSIFICATION` 只处理无批次标签的 81 条；带标签条目由各自批次负责。

## 分类结果

### A. 真实缺口（代码路径未实现或降级；需后续批次）

| 位置 | 摘要 | 依据 |
| --- | --- | --- |
| tree/pretty.rs:25,405 | `printUnit(tree, cdef)` 的 import 过滤需要 `isUsed`/`TreeScanner`，当前 panic；编译器主流程只用 `cdef=None`（javadoc 路径未使用） | 代码中 `panic!("unimplemented: printUnit...") `；仅 `tests/tree_oracle.rs` 调用且传 None |
| parser/parser_factory.rs:13 | `ParserFactory.instance()` 未实现（待 Log/Lint/Options 批次） | 文件头明确指出 instance 未实现 |
| parser/lazy_doc_comment_table.rs:68 | 依赖 ParserFactory（S7） | 注释与 ParserFactory 缺口一致 |
| util/convert.rs:128 | `Character.digit` 仅 ASCII（非 ASCII 数字降级） | 注释 "ASCII-only for now" |
| util/message_format.rs:20 | MessageFormat 的 number/date/time/choice 子格式未实现 | 注释 "Not implemented (registered TODO)" |
| util/module_helper.rs:8 | `addExports(Module, Module)` 未 port | 文件头 NOT PORTED |
| file/base_file_manager.rs:30,32,38,410 | Log error 入口/流解码/`PathFileObject.openReader` 未 port | 文件头 TODO 列表 |
| file/javac_file_manager.rs:54,1508 | zip/jar FileSystem、Locations PathFactory 未 port（S3c） | 文件头与 1508 注释 |
| file/jrt_index.rs:27 | 资源缺失时对每个 package 走 missing-key 结果 | 文件头注释 |
| jvm/class_reader.rs:65,68,140,709,2943 | readClass 及部分类读取路径、WriteableScope、noType 表示等未 port（P3 批次） | 文件头 "NOT ported yet" 与行内 TODO |
| jvm/module_name_reader.rs:21 | 部分读取语义未 port | 文件头 "NOT ported yet" |
| jvm/pool_reader.rs:15,39 | `getClass/getModule/getPackage/getType/getNameAndType` 未 port | 文件头与 39 行注释 |
| platform/jdk_platform_provider.rs:53 | "Not ported yet" 列表（依赖各自批次） | 文件头小节 |
| code/types.rs:109,2360,3631,3681,4155,4241,4257 | Types-2 收窄、lint 接线、Symbol 辅助、glb 等未完成 | 行内 TODO 与 `NOT PORTED (TODO Types)` 列表 |
| code/symtab.rs:37,54,56,677,1079,1121 | Types/Symbol/Options 依赖项（部分降级） | 文件头与行内 TODO |
| util/log.rs:31,37,421,1015 | 日志参数类型、Main 接线、Option 语义 | 文件头与行内 TODO |
| util/javac_messages.rs:19,105 | 环境 locale 检测未实现（固定 en） | 行内 TODO |

### B. 已过期（功能已实现，注释需删除/更新；本批次已修正）

| 位置 | 处理 |
| --- | --- |
| code/flags.rs:16,371 | 删除 `TODO(P3)`：is_static/is_enum/is_constant 已在同文件 710-757 实现 |
| parser/doc_comment_parser.rs:30-36 | `erroneous()`/`ReferenceParser` 已 port；空引用改为空 DCReference（adapter），不再 panic；文件头已更新 |
| tree/tree_info.rs:1476 | `Type.constValue` 已 port（type_.rs:343）；删除过期 TODO |
| util/raw_diagnostic_formatter.rs:22-34 | formatSource 已走 AbstractDiagnosticFormatter（无 source 时为上游 IllegalArgumentException 的 panic，section 6/9 已登记）；文件头已更新 |
| file/locations.rs:86-88 | MODULE_PATH/UPGRADE_MODULE_PATH 已实现（module_path_init_locations/ModulePathIterator，含 jar/jmod）；文件头已更新 |
| docs/AUDIT_FINDINGS.md F16/F22 | F16：`unimplemented!` 全仓为 0，test fixture 已为真实最小实现；F22：FilterImportScope 走 `_full` API，基础 API 为显式注册错误而非 `unimplemented!` |
| docs/KEYWORD_AUDIT.md | 计数更新为 TODO 251/FIXME 2/HACK 4，删除 flags/raw formatter 的过期条目 |

### C. 应登记（移植规则允许的 adapter/差异；保留注释）

| 位置 | 摘要 |
| --- | --- |
| comp/attr.rs:1887 | Attr 第一组 visitor 的"未 port 服务 + 精确位置 TODO"登记 |
| comp/attr_context.rs:10 | Env 字段省略/dup 语义登记 |
| comp/deferred_attr.rs:19 | DeferredAttr service 构造为 ADR 0001 显式上下文（TODO 表示构造方式） |
| comp/check.rs:25,1951 | 未 port 分支的显式 TODO 登记 |
| comp/enter.rs:11-17 | Annotate/TypeEnter/MemberEnter/Todo/Modules 调用的登记 |
| comp/member_enter.rs:15-32,639 | 同上（Annotate/Lint/Attr 依赖） |
| comp/type_enter.rs:27,39 | Annotate/TypeAnnotations/Todo/Preview 接入的登记 |
| comp/resolve.rs:40,1764 | 未 port 方法列表与显式 TODO 登记 |
| code/class_finder.rs:11 | jrtIndex==null 路径的登记 |
| code/symbol.rs:13 | 缺口清单引用（AUDIT_TODO_TABLE） |
| file/mod.rs:11 | charset/URL 等差异登记 |
| file/locations.rs:79 | reported gaps 列表（模块扫描已实现，剩余项为 Log 警告等） |
| comp/compiler_context.rs:431 | Scope 查询的 TODO 说明 |
| tree/tree_maker.rs:91 | Types/Symtab 字段未声明的说明 |
| tree/doc_tree_maker.rs:62 | accessor 差异说明 |
| util/abstract_diagnostic_formatter.rs:42,44,115 | Options 缺失的登记 |
| util/basic_diagnostic_formatter.rs:36 | diags 选项解码登记 |
| parser (文件头) | parser S7 相关登记的其余条目 |

### D. 上游原注释（非移植债务，保留）

| 位置 | 说明 |
| --- | --- |
| crates/javac-tools/src/lang/model/util/element_scanner9.rs:33 | 上游 `TODO: Hmmm, this might not be right` |
| crates/javac-tools/src/tools/java_file_manager.rs:477,536,554,578 | 上游接口文档 `TODO: describe failure modes` |
| crates/javac-tools/src/uri.rs:7 | 上游移植说明 |
| jvm/class_reader.rs:9 | 上游 Code 属性 TODO 的移植说明 |

## 附录：81 条无括号标签 TODO（原始清单）

```text
crates\javac-core\src\compiler_context.rs:431: // TODO for the Scope query).
crates\javac-core\src\code\class_finder.rs:11: // getSupplementaryFlags (jrtIndex==null path; Some path registered TODO),
crates\javac-core\src\code\symbol.rs:13: // gap list is docs/AUDIT_TODO_TABLE.md):
crates\javac-core\src\code\symtab.rs:37: //!   values (registered; the localized toString is a Symbol TODO);
crates\javac-core\src\code\symtab.rs:54: //!   Types methods are NOT PORTED (TODO Types; see types.rs).
crates\javac-core\src\code\symtab.rs:56: //!   anonymous override is a Symbol TODO);
crates\javac-core\src\code\symtab.rs:677: /// (PackageSymbol completer TODO), the members scope is created here.
crates\javac-core\src\code\symtab.rs:1079: /// not ported yet (Options/Option integration TODO), so the caller passes
crates\javac-core\src\code\symtab.rs:1121: // anonymous override is not expressible (Symbol TODO).
crates\javac-core\src\code\types.rs:87: //      predicates at the consuming sites; full narrowing is TODO Types-2.
crates\javac-core\src\code\types.rs:109: //! remaining Types methods are NOT PORTED (TODO Types):
crates\javac-core\src\code\types.rs:2360: /// glb (TODO Types): a type that does not need capture conversion is
crates\javac-core\src\code\types.rs:3631: /// subtype relation (registered lint wiring TODO with Attr); the boolean
crates\javac-core\src\code\types.rs:3681: /// emitted here (the lint wiring is TODO with the Attr); the boolean
crates\javac-core\src\code\types.rs:4155: /// ported, so the nil default is used (TODO Symbol; registered).
crates\javac-core\src\code\types.rs:4241: /// PORTING_RULES: reproduced here because type_.rs has not ported it yet (TODO
crates\javac-core\src\code\types.rs:4257: // helpers because type_.rs/symbol.rs have not ported them yet (TODO Type/Symbol
crates\javac-core\src\comp\attr_context.rs:10: //!   fields are omitted and dup() skips them; registered TODO, no substitute;
crates\javac-core\src\comp\attr.rs:1887: // registered with a not-yet-ported marker + TODO at the exact call site.
crates\javac-core\src\comp\check.rs:25: // is not ported the branch is marked TODO and not executed (no substitute).
crates\javac-core\src\comp\check.rs:1951: // types.getRetention/overrideEquivalent) are explicit TODO registrations.
crates\javac-core\src\comp\deferred_attr.rs:19: // TODO.
crates\javac-core\src\comp\enter.rs:11: // calls are TODO in the corresponding methods (Annotate);
crates\javac-core\src\comp\enter.rs:13: // ensureImportsChecked are TODO (TypeEnter/MemberEnter); visitClassDef
crates\javac-core\src\comp\enter.rs:15: // - Todo instance: todo.append of package/module envs is TODO (Todo);
crates\javac-core\src\comp\enter.rs:17: // usesProvidesCompleter and isInModuleGraph are TODO (Modules);
crates\javac-core\src\comp\member_enter.rs:15: // annotateDefaultValueLater/unfinishedDefaultValue calls are TODO (Annotate
crates\javac-core\src\comp\member_enter.rs:24: // type refinement is TODO in visitVarDef. VarSymbol.setLazyConstValue is
crates\javac-core\src\comp\member_enter.rs:26: // - getMethodEnv mEnv.info.lint.augment(sym) is TODO (Lint), the scope
crates\javac-core\src\comp\member_enter.rs:32: // registered separately (the Enter leaves the TypeEnter completer TODO).
crates\javac-core\src\comp\member_enter.rs:639: /// TODO until Attr is ported; the method is kept for that.
crates\javac-core\src\comp\resolve.rs:40: //! NOT YET PORTED IN THIS FILE (registered; each with a not-yet-ported marker + TODO):
crates\javac-core\src\comp\resolve.rs:1764: // upstream body; unimplemented dependencies are explicit TODO registrations.
crates\javac-core\src\comp\type_enter.rs:27: // typeAnnotations.validate*, todo.append, preview checks -> TODO comments
crates\javac-core\src\comp\type_enter.rs:39: // registered TODOs where used (symbol/record batches);
crates\javac-core\src\file\base_file_manager.rs:30: //!   TODO (the Log error entry point and a streaming decoder are not available).
crates\javac-core\src\file\base_file_manager.rs:32: //!   provide yet (TODO, reported).
crates\javac-core\src\file\base_file_manager.rs:38: //!   unimplemented + TODO; the content cache keeps the decoded JavaString directly
crates\javac-core\src\file\base_file_manager.rs:410: /// below is expressed (PathFileObject.openReader is a separate TODO).
crates\javac-core\src\file\javac_file_manager.rs:54: //!   (java.nio zip/jar FileSystem) is the second S3c (TODO in getContainer).
crates\javac-core\src\file\javac_file_manager.rs:1508: // Java: locations.setPathFactory(f); the Locations-side PathFactory is TODO(S3b
crates\javac-core\src\file\jrt_index.rs:27: //!   missing-key result) for every package until the resource lands (TODO resources,
crates\javac-core\src\file\locations.rs:79: //! * reported gaps (TODO, no substitute data):
crates\javac-core\src\file\mod.rs:11: //!   charset/URL/thread-dependent methods are TODO and reported) and the Locations
crates\javac-core\src\jvm\class_reader.rs:9: //   Code object attach TODO), ConstantValue, Deprecated, Exceptions,
crates\javac-core\src\jvm\class_reader.rs:65: //! * NOT ported yet (not-yet-ported marker with TODO below): readClass and the
crates\javac-core\src\jvm\class_reader.rs:68: //!   These methods carry explicit TODO markers; no substitutes are provided.
crates\javac-core\src\jvm\class_reader.rs:140: /// PORTING_RULES: Scope.WriteableScope is not ported yet (TODO(P3 scope
crates\javac-core\src\jvm\class_reader.rs:709: /// constant here; Types.noType is not represented in Symtab yet (TODO P3),
crates\javac-core\src\jvm\class_reader.rs:2943: // mapping is a registered TODO for the hardening).
crates\javac-core\src\jvm\module_name_reader.rs:21: //! * NOT ported yet (TODO, no substitute semantics):
crates\javac-core\src\jvm\pool_reader.rs:15: //! * NOT ported yet (TODO, no substitute semantics):
crates\javac-core\src\jvm\pool_reader.rs:39: //! (getClass/getModule/getPackage/getType/getNameAndType are still TODO; the raw
crates\javac-core\src\parser\doc_comment_parser.rs:33: //!   input panics with that TODO; well-formed comments do not take this path.
crates\javac-core\src\parser\lazy_doc_comment_table.rs:68: /// ParserFactory is available (parser S7; TODO).
crates\javac-core\src\parser\parser_factory.rs:13: //!   injection (its instance() is not implemented yet, TODO(Log/Lint/Options
crates\javac-core\src\platform\jdk_platform_provider.rs:53: //! ## Not ported yet (TODO, with the dependency that unblocks each)
crates\javac-core\src\tree\doc_tree_maker.rs:62: /// args) method (TODO, see the file header); the accessor keeps the
crates\javac-core\src\tree\pretty.rs:25: //!   internal TreeScanner). The cdef != null path panics with a TODO message;
crates\javac-core\src\tree\pretty.rs:405: /// TreeScanner, not ported yet); it panics with a TODO message.
crates\javac-core\src\tree\tree_info.rs:1476: // TODO: the constant value (Type.constValue) is not
crates\javac-core\src\tree\tree_maker.rs:91: // Java: Types types; Symtab syms; — not declared yet, see the file header (TODO).
crates\javac-core\src\util\abstract_diagnostic_formatter.rs:42: //!   provided (TODO in the that ports Options); the report-style option
crates\javac-core\src\util\abstract_diagnostic_formatter.rs:44: //! - NOT PORTED IN THIS STEP (dependencies are not ported yet; TODO, no
crates\javac-core\src\util\abstract_diagnostic_formatter.rs:115: // ported yet, TODO).
crates\javac-core\src\util\basic_diagnostic_formatter.rs:36: //!   provided (TODO: the diags.* option decoding is registered in the Options
crates\javac-core\src\util\convert.rs:128: /// Java: Character.digit(char, radix); ASCII-only for now (see TODO above).
crates\javac-core\src\util\javac_messages.rs:19: //!   locale is "en" (environment detection is a registered TODO);
crates\javac-core\src\util\javac_messages.rs:105: /// Java: Locale.getDefault() -- TODO: environment locale detection is a
crates\javac-core\src\util\log.rs:31: //!     TODO for the remaining types; registered);
crates\javac-core\src\util\log.rs:37: //!       is not added to JavacMessages by the ported Log (TODO Main);
crates\javac-core\src\util\log.rs:421: // (equivalently, TODO Option).
crates\javac-core\src\util\log.rs:1015: /// rendered (the remaining types are TODO, registered section 9).
crates\javac-core\src\util\message_format.rs:20: // Not implemented (registered TODO): number/date/time/choice sub-formats
crates\javac-core\src\util\module_helper.rs:8: //!   addExports(Module, Module) is therefore NOT PORTED (TODO, no substitute
crates\javac-tools\src\uri.rs:7: //              (com.sun.tools.javac.file), see the TODOs below.
crates\javac-tools\src\lang\model\util\element_scanner9.rs:33: // TODO: Hmmm, this might not be right
crates\javac-tools\src\tools\java_file_manager.rs:477: // TODO: describe failure modes
crates\javac-tools\src\tools\java_file_manager.rs:536: // TODO: describe failure modes
crates\javac-tools\src\tools\java_file_manager.rs:554: // TODO: describe failure modes
crates\javac-tools\src\tools\java_file_manager.rs:578: // TODO: describe failure modes
```

## 结论与后续

- A 类是本批次无法一次完成的真实缺口；按依赖批次推进（Options/Log/Lint → Types-2/Symbol → S3c 文件层 → javadoc Pretty）。
- B 类已在本批次修正（源码注释或文档），不再保留过期标记。
- C 类为移植规则明确允许的 adapter/差异，注释保留并需在 PORTING_MAP 中可追溯。
- D 类来自上游源码，不属于移植债务。
