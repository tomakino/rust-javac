# dead_code 审计表(只读分析,待负责人批准后处置)

生成时间: 2026-10-27;范围: crates/ tools/ xtask/。

## 口径与统计

- 实测 `#[allow(dead_code)]` 抑制项 **68 处**(Lead 旧统计 67;本次实测差异 1,以本次为准;主动更正);
- 编译器 `dead_code` 警告(未加 allow)**6 项唯一**(cargo check --workspace --all-targets;多 target 重复 10 条已去重);
- 合计 **74 项**;分类:{"未接线(依赖批次)":13,"已接线(allow 冗余)":48,"测试用":3,"零引用(遗留/未接线)":4}
- 警告 6 项:未接线 4(charset Unmappable、base_file_manager deferred_close_timeout、ct_sym delegate/get_delegate)、测试位置错误 2(annotate 两个 #[test] 函数);

## allow(dead_code) 抑制项(68)

| # | 文件:行 | 符号 | 类型 | 分类 | 生产/测试引用 | 上游对应 | 处置建议 |
|---:|---|---|---|---|---|---|---|
| 1 | crates/javac-core/src/code/preview.rs:140 | new | method/fn | 未接线(依赖批次) | 3344/1895 | 注释: src/jdk.compiler/share/classes/com/sun/tools/javac/code/Preview.java:90 | 保留;按 对应批次 接线(上游 src/jdk.compiler/share/classes/com/sun/tools/javac/code/Preview.java:90) |
| 2 | crates/javac-core/src/code/types.rs:3872 | get_thrown_types | method/fn | 已接线(allow 冗余) | 37/8 | 注释: Type.java:549 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 3 | crates/javac-core/src/code/types.rs:3990 | cvar_upper_bound | method/fn | 已接线(allow 冗余) | 11/3 | 注释: Types.java:149 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 4 | crates/javac-core/src/code/types.rs:4091 | new_type_var_type | method/fn | 已接线(allow 冗余) | 1/2 | 注释: Types.java:3427 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 5 | crates/javac-core/src/code/types.rs:4110 | new_captured_type | method/fn | 已接线(allow 冗余) | 1/2 | 注释: Types.java:4465 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 6 | crates/javac-core/src/code/types.rs:4139 | type_contains | method/fn | 未接线(依赖批次) | 11/3 | 注释: Type.java:1148 | 保留;按 对应批次 接线(上游 Type.java:1148) |
| 7 | crates/javac-core/src/comp/annotate.rs:2327 | AnnotateHarness | struct | 测试用 | 0/3 | — | 保留(测试专用);建议移入 cfg(test) 或注明测试用途 |
| 8 | crates/javac-core/src/comp/attr.rs:4762 | is_assignable_as_blank_final | method/fn | 已接线(allow 冗余) | 2/0 | 候选(待核): src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:283 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 9 | crates/javac-core/src/comp/attr.rs:4807 | resolve_is_static | method/fn | 已接线(allow 冗余) | 3/0 | — | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 10 | crates/javac-core/src/comp/attr.rs:4852 | check_assignable | method/fn | 已接线(allow 冗余) | 3/0 | 候选(待核): src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:308 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 11 | crates/javac-core/src/comp/attr.rs:4933 | enclosing_init_env | method/fn | 已接线(allow 冗余) | 2/0 | 注释: Attr.java:4692 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 12 | crates/javac-core/src/comp/attr.rs:4994 | is_static_enum_field | method/fn | 已接线(allow 冗余) | 3/0 | 注释: Attr.java:4764 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 13 | crates/javac-core/src/comp/attr.rs:5029 | check_enum_initializer | method/fn | 已接线(allow 冗余) | 2/0 | 注释: Attr.java:4728 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 14 | crates/javac-core/src/comp/attr.rs:5075 | resolve_is_initializer | method/fn | 已接线(allow 冗余) | 2/0 | — | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 15 | crates/javac-core/src/comp/attr.rs:5114 | check_init | method/fn | 已接线(allow 冗余) | 10/0 | 注释: Attr.java:4653 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 16 | crates/javac-core/src/comp/attr.rs:10840 | get_diagnostic_position | method/fn | 已接线(allow 冗余) | 3/3 | 注释: Attr.java:5455 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 17 | crates/javac-core/src/comp/attr.rs:10892 | check_serial_version_uid | method/fn | 已接线(allow 冗余) | 2/0 | 注释: Attr.java:5495 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 18 | crates/javac-core/src/comp/deferred_attr.rs:1618 | scan_lambda_return | method/fn | 已接线(allow 冗余) | 17/0 | 注释: DeferredAttr.java:1100 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 19 | crates/javac-core/src/comp/deferred_attr.rs:2042 | scan_lambda_body | method/fn | 已接线(allow 冗余) | 2/0 | 注释: DeferredAttr.java:1247 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 20 | crates/javac-core/src/comp/flow.rs:2501 | return_type | field/const/variant | 已接线(allow 冗余) | 12/1 | 注释: Flow.java:964 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 21 | crates/javac-core/src/comp/flow.rs:5040 | lint | field/const/variant | 已接线(allow 冗余) | 211/11 | — | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 22 | crates/javac-core/src/comp/lower.rs:4625 | args_type_sig | method/fn | 已接线(allow 冗余) | 1/0 | 候选(待核): src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Lower.java:2521 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 23 | crates/javac-core/src/comp/member_enter.rs:611 | check_type | method/fn | 已接线(allow 冗余) | 13/1 | 注释: src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:478 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 24 | crates/javac-core/src/comp/operators.rs:99 | UnaryEntry | struct | 已接线(allow 冗余) | 6/0 | — | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 25 | crates/javac-core/src/comp/operators.rs:110 | BinaryEntry | struct | 已接线(allow 冗余) | 6/0 | — | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 26 | crates/javac-core/src/comp/operators.rs:927 | Harness | struct | 测试用 | 0/20 | — | 保留(测试专用);建议移入 cfg(test) 或注明测试用途 |
| 27 | crates/javac-core/src/comp/trans_types.rs:438 | translate_args_with_env | method/fn | 已接线(allow 冗余) | 1/0 | — | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 28 | crates/javac-core/src/file/base_file_manager.rs:83 | (impl/字段) | item | 未接线(依赖批次) | 0/0 | — | 保留;按 S3 接线 |
| 29 | crates/javac-core/src/file/base_file_manager.rs:92 | options | field/const/variant | 已接线(allow 冗余) | 73/42 | 注释: src/jdk.compiler/share/classes/com/sun/tools/sjavac/options/Options.java:328 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 30 | crates/javac-core/src/file/base_file_manager.rs:96 | class_loader_class | field/const/variant | 已接线(allow 冗余) | 2/0 | — | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 31 | crates/javac-core/src/file/base_file_manager.rs:100 | locations | field/const/variant | 已接线(allow 冗余) | 16/0 | 注释: src/java.compiler/share/classes/javax/annotation/processing/Filer.java:48 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 32 | crates/javac-core/src/file/base_file_manager.rs:116 | encoding_name | field/const/variant | 已接线(allow 冗余) | 10/1 | — | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 33 | crates/javac-core/src/file/base_file_manager.rs:122 | multi_release_value | field/const/variant | 未接线(依赖批次) | 6/3 | 注释: src/jdk.compiler/share/classes/com/sun/tools/javac/api/JavacTool.java:196 | 保留;按 对应批次 接线(上游 src/jdk.compiler/share/classes/com/sun/tools/javac/api/JavacTool.java:196) |
| 34 | crates/javac-core/src/file/base_file_manager.rs:137 | new | method/fn | 已接线(allow 冗余) | 3344/1895 | 注释: src/jdk.compiler/share/classes/com/sun/tools/javac/file/BaseFileManager.java:74 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 35 | crates/javac-core/src/file/base_file_manager.rs:158 | create_locations | method/fn | 已接线(allow 冗余) | 2/0 | 注释: src/jdk.compiler/share/classes/com/sun/tools/javac/file/BaseFileManager.java:77 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 36 | crates/javac-core/src/file/base_file_manager.rs:198 | locations | method/fn | 已接线(allow 冗余) | 6/0 | 注释: src/java.compiler/share/classes/javax/annotation/processing/Filer.java:48 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 37 | crates/javac-core/src/file/base_file_manager.rs:693 | new | method/fn | 已接线(allow 冗余) | 3344/1895 | 候选(待核): src/jdk.compiler/share/classes/com/sun/tools/javac/tree/JCTree.java:1827 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 38 | crates/javac-core/src/file/jrt_index.rs:97 | get_file | method/fn | 未接线(依赖批次) | 8/9 | 注释: src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Analyzer.java:580 | 保留;按 S3 接线(上游 src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Analyzer.java:580) |
| 39 | crates/javac-core/src/file/locations.rs:343 | ModuleLocation | struct | 已接线(allow 冗余) | 30/0 | — | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 40 | crates/javac-core/src/file/locations.rs:359 | (impl/字段) | impl-block | 零引用(遗留/未接线) | 0/0 | — | 确认无引用后删除(无上游对应/适配遗留) |
| 41 | crates/javac-core/src/file/locations.rs:420 | (impl/字段) | impl-block | 零引用(遗留/未接线) | 0/0 | — | 确认无引用后删除(无上游对应/适配遗留) |
| 42 | crates/javac-core/src/file/locations.rs:508 | log | field/const/variant | 未接线(依赖批次) | 555/69 | 注释: make/langtools/tools/anttasks/CompilePropertiesTask.java:60 | 保留;按 对应批次 接线(上游 make/langtools/tools/anttasks/CompilePropertiesTask.java:60) |
| 43 | crates/javac-core/src/file/locations.rs:513 | warn | field/const/variant | 已接线(allow 冗余) | 604/18 | 注释: src/jdk.compiler/share/classes/com/sun/tools/javac/code/Types.java:1036 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 44 | crates/javac-core/src/file/locations.rs:518 | multi_release_value | field/const/variant | 已接线(allow 冗余) | 6/3 | 注释: src/jdk.compiler/share/classes/com/sun/tools/javac/api/JavacTool.java:196 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 45 | crates/javac-core/src/jvm/class_reader.rs:120 | ClassReader | struct | 已接线(allow 冗余) | 126/22 | 注释: src/java.base/share/classes/java/lang/Module.java:1550 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 46 | crates/javac-core/src/jvm/class_reader.rs:409 | get_module | method/fn | 未接线(依赖批次) | 11/2 | 注释: make/langtools/tools/crules/CodingRulesAnalyzerPlugin.java:84 | 保留;按 S3 接线(上游 make/langtools/tools/crules/CodingRulesAnalyzerPlugin.java:84) |
| 47 | crates/javac-core/src/jvm/class_reader.rs:436 | get_package | method/fn | 未接线(依赖批次) | 10/5 | 注释: make/langtools/src/classes/build/tools/symbolgenerator/JavadocElementList.java:109 | 保留;按 S3 接线(上游 make/langtools/src/classes/build/tools/symbolgenerator/JavadocElementList.java:109) |
| 48 | crates/javac-core/src/jvm/class_reader.rs:501 | get_name_and_type | method/fn | 未接线(依赖批次) | 2/0 | 注释: src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/PoolReader.java:176 | 保留;按 S3 接线(上游 src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/PoolReader.java:176) |
| 49 | crates/javac-core/src/jvm/class_reader.rs:530 | get_constant | method/fn | 未接线(依赖批次) | 5/3 | 注释: src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:809 | 保留;按 S3 接线(上游 src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:809) |
| 50 | crates/javac-core/src/jvm/class_reader.rs:3689 | AnnotationValue | enum | 已接线(allow 冗余) | 99/22 | 注释: test/langtools/tools/javac/classfiles/attributes/annotations/WorkAnnotations.java:43 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 51 | crates/javac-core/src/jvm/class_reader.rs:3705 | TypeAnnotationProxy | struct | 已接线(allow 冗余) | 7/0 | 注释: src/jdk.compiler/share/classes/com/sun/tools/javac/jvm/ClassReader.java:1567 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 52 | crates/javac-core/src/jvm/pool_reader.rs:662 | _unused_path | method/fn | 测试用 | 0/1 | — | 保留(测试专用);建议移入 cfg(test) 或注明测试用途 |
| 53 | crates/javac-core/src/main/option.rs:688 | OPERAND | field/const/variant | 零引用(遗留/未接线) | 0/0 | — | 确认无引用后删除(无上游对应/适配遗留) |
| 54 | crates/javac-core/src/parser/doc_comment_parser.rs:100 | RetainAll | field/const/variant | 未接线(依赖批次) | 0/0 | — | 保留;按 S8 接线 |
| 55 | crates/javac-core/src/parser/javac_parser.rs:539 | peek_token2 | method/fn | 已接线(allow 冗余) | 1/0 | — | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 56 | crates/javac-core/src/parser/javac_parser.rs:551 | peek_token3 | method/fn | 已接线(allow 冗余) | 1/0 | — | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 57 | crates/javac-core/src/parser/javac_parser.rs:558 | peek_token_at3 | method/fn | 已接线(allow 冗余) | 2/0 | — | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 58 | crates/javac-core/src/parser/javac_parser.rs:768 | check_no_mods | method/fn | 已接线(allow 冗余) | 1/0 | 注释: src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:493 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 59 | crates/javac-core/src/parser/javac_parser.rs:1198 | lookahead_kind | method/fn | 已接线(allow 冗余) | 1/0 | — | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 60 | crates/javac-core/src/parser/javac_parser.rs:1204 | earlier | method/fn | 已接线(allow 冗余) | 3/0 | 注释: src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:4777 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 61 | crates/javac-core/src/parser/parser_factory.rs:40 | (impl/字段) | item | 未接线(依赖批次) | 0/0 | — | 保留;按 对应批次 接线 |
| 62 | crates/javac-core/src/platform/jdk_platform_provider.rs:307 | ct_sym_version | field/const/variant | 未接线(依赖批次) | 1/5 | — | 保留;按 S3 接线 |
| 63 | crates/javac-core/src/tree/doc_tree_maker.rs:732 | is_text_tree | method/fn | 已接线(allow 冗余) | 1/0 | 候选(待核): src/jdk.compiler/share/classes/com/sun/tools/javac/tree/DocTreeMaker.java:594 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 64 | crates/javac-floating/src/fdbig_integer.rs:1062 | to_hex_string | method/fn | 已接线(allow 冗余) | 1/0 | 注释: make/langtools/src/classes/build/tools/symbolgenerator/CreateSymbols.java:2558 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 65 | crates/javac-floating/src/floating_decimal.rs:76 | BinaryToASCIIConverter | trait | 已接线(allow 冗余) | 9/0 | — | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 66 | crates/javac-floating/src/floating_decimal.rs:648 | insignificant_digits | method/fn | 已接线(allow 冗余) | 1/0 | 候选(待核): src/java.base/share/classes/jdk/internal/math/FloatingDecimal.java:490 | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 67 | crates/javac-floating/src/mutable_big_integer.rs:2140 | to_compact_value_unused | method/fn | 已接线(allow 冗余) | 1/0 | — | 移除 allow(编译器验证,若仍有警告则接线或补测试) |
| 68 | crates/javac-source/src/util/dispatch.rs:21 | dispatch_tree | method/fn | 零引用(遗留/未接线) | 0/0 | — | 确认无引用后删除(无上游对应/适配遗留) |

## 编译器 dead_code 警告(6)

| # | 文件:行 | 符号/消息 | 分类 | 上游对应 | 处置建议 |
|---:|---|---|---|---|---|
| 1 | crates/javac-charset/src/coder.rs:35 | variant `Unmappable` is never constructed | 未接线(错误路径) | charset 适配(上游 sun.nio.cs 不在 reference 镜像) | 接线解码错误路径,或确认不可达后删除 |
| 2 | crates/javac-core/src/file/base_file_manager.rs:209 | method `deferred_close_timeout` is never used | 未接线(配置项) | 注释: BaseFileManager(上游超时配置读取) | 在 setContext 接线读取超时选项 |
| 3 | crates/javac-core/src/platform/ct_sym_java_file_manager.rs:52 | field `delegate` is never read | 未接线(委托对象) | SigJavaFileObject/上游 JavaFileObject 委托 | 接线 getDelegate/委托字段,或删除(适配不需要) |
| 4 | crates/javac-core/src/platform/ct_sym_java_file_manager.rs:392 | method `get_delegate` is never used | 未接线(委托对象) | SigJavaFileObject/上游 JavaFileObject 委托 | 接线 getDelegate/委托字段,或删除(适配不需要) |
| 5 | crates/javac-core/src/comp/annotate.rs:2340 | function `instance_returns_the_registered_service` is never used | 测试项(位置错误) | — | 移动到正确 #[cfg(test)] 模块,使测试可运行 |
| 6 | crates/javac-core/src/comp/annotate.rs:2350 | function `instance_requires_registration` is never used | 测试项(位置错误) | — | 移动到正确 #[cfg(test)] 模块,使测试可运行 |

## 高影响项(核心编译链优先)

- attr.rs 系列(4762/4807/4852/4933/4994/5029/5075/5114/10840/10892):已接线但 allow 抑制;建议移除 allow 由编译器验证,若确为未使用则按上游 Attr.java 调用点接线;
- deferred_attr.rs 1618/2042(scanLambdaReturn/scanLambdaBody)、member_enter.rs 611(checkType)、lower.rs 4625(argsTypeSig)、trans_types.rs 438(translateArgsWithEnv):已接线但 allow;同上;
- jvm/class_reader.rs 409/436/501/530(pool accessors getModule/getPackage/getNameAndType/getConstant):注释 Unwired,依赖 ClassReader 批次接线;
- file/locations.rs 343/359/420(ModuleLocation/ModuleTable impl allow):移除 impl 级 allow 并由编译器验证;
- main/option.rs 688(OPERAND 枚举值零引用):接线 main option 解析(上游 Option.OPERAND 使用点);
- parser/javac_parser.rs 539/551/558(peek_token2/3/at3)、768(checkNoMods)、1198(lookahead_kind)、1204(earlier):零/仅内部引用,按上游 Parser 调用点接线或删除(上游对应已列);
- parser/parser_factory.rs 40(tokens 字段)、parser/doc_comment_parser.rs 100(RetainAll):依赖 parser/doc-comment wiring 批次;
- file/base_file_manager.rs 83/92/96/100/116/122(log/options/class_loader_class/locations/encoding_name/multi_release_value):S3b/option restore 批次接线;

## 处置建议汇总
- 移除 allow 并编译器验证(可能冗余):48 项(已接线);
- 接线:未接线(依赖批次)13 项 + 零引用中有上游对应项;给出上游行号;
- 删除:零引用且无上游对应/适配遗留(dispatch_tree 等,需负责人批准后删除);
- 测试专用:3 项(AnnotateHarness/Harness/_unused_path)+ 2 个警告测试函数(移动到 cfg(test));
