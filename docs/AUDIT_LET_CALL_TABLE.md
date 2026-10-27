# let_ = <call> 审计分析表(批 B,只读分析)

生成时间: 2026-10-27;范围: crates/ tools/ xtask/ 全部 .rs;扫描模式: `let _ = <expr>;`(跨行拼接);排除纯标识符/引用类(属批 A)。

## 汇总统计

| 类别 | 数量 | 说明 |
|---|---:|---|
| 总计 | 178 | 全部 let _ = 调用/宏 项 |
| 可安全忽略 | 100 | 上游候选为语句调用(忽略返回值)、宏写入副作用、或测试辅助 |
| 必须使用(候选值使用) | 7 | 上游候选行存在赋值/return/条件使用,需人工确认对应关系后按项修复 |
| 需证据 | 71 | 上游定位不唯一/找不到/通用名/诊断副作用调用,需人工核对 |

统计细分:宏 49;测试辅助 2;候选值使用但通用名/副作用降级 27。

方法:本地扫描 → 由本地文件映射上游 javac 文件(路径与文件名去下划线匹配)→ 在映射文件内按本地所在函数名定位并搜索同名调用 → 行级判定上游是否使用返回值(赋值/return/条件)→ 失败时全局搜索前 3 候选。判定为启发式;表中保留候选行供人工核对,`必须使用(候选值使用)` 仅为高风险标记,不代表最终裁决。

## 分析表(按判定排序:必须使用 → 需证据 → 可安全忽略)

| # | 本地文件:行 | 被调函数 | 本地函数 | 上游候选(file:line) | 上游候选行文本 | 判定 | 依据 |
|---:|---|---|---|---|---|---|---|
| 1 | crates/javac-core/src/comp/annotate.rs:1091 | attribute_annotation_value | attribute_annotation_name_value_pair | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:512 \| src | attributeAnnotationValue(nameValuePair.type = syms.errType, nameValuePair, env); | 必须使用(候选值使用) | upstream candidate uses return value |
| 2 | crates/javac-core/src/comp/annotate.rs:1105 | attribute_annotation_value | attribute_annotation_name_value_pair | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:512 \| src | attributeAnnotationValue(nameValuePair.type = syms.errType, nameValuePair, env); | 必须使用(候选值使用) | upstream candidate uses return value |
| 3 | crates/javac-core/src/comp/annotate.rs:1245 | attribute_annotation_value | attribute_annotation_value | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:534 \| src | Attribute value = attributeAnnotationValue(resultType, assign.rhs, env); | 必须使用(候选值使用) | upstream candidate uses return value |
| 4 | crates/javac-core/src/comp/annotate.rs:1282 | attribute_annotation | attribute_annotation_value | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:589 \| src | attributeAnnotation((JCAnnotation)tree, syms.errType, env); | 必须使用(候选值使用) | upstream candidate uses return value |
| 5 | crates/javac-core/src/comp/lower.rs:2510 | translate | translate_top_level_class | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Lower.java:4103 \| src/j | result = translate(tree.body); | 必须使用(候选值使用) | upstream candidate uses return value |
| 6 | crates/javac-core/src/comp/resolve.rs:9054 | merge_results | method_resolution_phase_merge_results_checks_the_invariant | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:3708 | bestSoFar = phase.mergeResults(bestSoFar, sym); | 必须使用(候选值使用) | upstream candidate uses return value |
| 7 | crates/javac-core/src/comp/resolve.rs:9063 | merge_results | method_resolution_phase_merge_results_rejects_ambiguous_best | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:3708 | bestSoFar = phase.mergeResults(bestSoFar, sym); | 必须使用(候选值使用) | upstream candidate uses return value |
| 8 | crates/javac-classpath/src/lib.rs:433 | remove_dir_all | missing_system_library_is_reported | NOT FOUND |  | 需证据 | no upstream call candidate |
| 9 | crates/javac-core/src/api/diagnostic_formatter.rs:214 | get_configuration | dyn_formatter_is_object_safe | src/jdk.compiler/share/classes/com/sun/tools/javac/util/BasicDiagnosticFormatter | if (getConfiguration().getVisible().contains(DiagnosticPart.SUMMARY)) { | 需证据(候选值使用) | upstream candidate uses return value |
| 10 | crates/javac-core/src/code/class_finder.rs:1610 | io_failure_message | io_failure_message_panics_for_unsupported_operation | NOT FOUND |  | 需证据 | no upstream call candidate |
| 11 | crates/javac-core/src/code/directive.rs:308 | is_static | requires_methods_panic_for_other_directives | src/java.base/share/classes/java/lang/Class.java:3443 \| src/java.base/share/cla | if (!Modifier.isStatic(m.getModifiers())) { | 需证据(候选值使用) | upstream candidate uses return value |
| 12 | crates/javac-core/src/code/preview.rs:564 | disabled_error_classfile | disabled_error_classfile_panics_for_an_unknown_major_version | NOT FOUND |  | 需证据 | no upstream call candidate |
| 13 | crates/javac-core/src/code/source.rs:767 | is_plural | is_plural_requires_a_diag_kind | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Preview.java:141 \| src/ | previewHandler.report(pos, feature.isPlural() ? | 需证据(候选值使用) | upstream candidate uses return value |
| 14 | crates/javac-core/src/code/symbol.rs:3463 | get_diagnostic | completion_failure_reset_diagnostic_recomputes | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/ArgumentAttr.java:570 \| | resultInfo.checkContext.report(null, ex.getDiagnostic()); | 需证据(候选值使用) | upstream candidate uses return value |
| 15 | crates/javac-core/src/code/symbol.rs:3469 | get_diagnostic | completion_failure_reset_diagnostic_recomputes | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/ArgumentAttr.java:570 \| | resultInfo.checkContext.report(null, ex.getDiagnostic()); | 需证据(候选值使用) | upstream candidate uses return value |
| 16 | crates/javac-core/src/code/symbol.rs:3680 | kind_name | kind_name_panics_for_invalid_kind | make/langtools/tools/propertiesparser/parser/MessageInfo.java:104 \| src/jdk.com | if (text.equals(st.kindName())) { | 需证据(候选值使用) | upstream candidate uses return value |
| 17 | crates/javac-core/src/code/type_annotations.rs:1163 | clone | separate_annotations_kinds | src/java.base/share/classes/java/lang/Class.java:1157 \| src/java.base/share/cla | return cloneArray ? interfaces.clone() : interfaces; | 需证据(候选值使用) | upstream candidate uses return value |
| 18 | crates/javac-core/src/comp/annotate.rs:2352 | instance | instance_requires_registration | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:110 \| src | attr = Attr.instance(context); | 需证据(候选值使用) | upstream candidate uses return value |
| 19 | crates/javac-core/src/comp/attr.rs:1655 | clone | check_id_internal | src/java.base/share/classes/java/lang/Class.java:1157 \| src/java.base/share/cla | return cloneArray ? interfaces.clone() : interfaces; | 需证据(候选值使用) | upstream candidate uses return value |
| 20 | crates/javac-core/src/comp/attr.rs:1712 | attrib_tree_with | visit_erroneous | NOT FOUND |  | 需证据 | no upstream call candidate |
| 21 | crates/javac-core/src/comp/attr.rs:5197 | get_const_value | check_init | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4683 \| src/jd | v.getConstValue(); // ensure initializer is evaluated | 需证据(候选值使用) | upstream candidate uses return value |
| 22 | crates/javac-core/src/comp/attr.rs:9001 | get_const_value | enum_constant | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1861 \| src/jd | ((VarSymbol)s).getConstValue(); // ensure initializer is evaluated | 需证据(候选值使用) | upstream candidate uses return value |
| 23 | crates/javac-core/src/comp/attr.rs:9519 | attrib_speculative_with_diag | attrib_to_tree | NOT FOUND |  | 需证据 | no upstream call candidate |
| 24 | crates/javac-core/src/comp/attr.rs:10468 | check_after_inference | types_inferred | NOT FOUND |  | 需证据 | no upstream call candidate |
| 25 | crates/javac-core/src/comp/attr.rs:14549 | get_const_value | visit_var_def_result | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1296 \| src/jd | v.getConstValue(); // ensure compile-time constant initializer is evaluated | 需证据(候选值使用) | upstream candidate uses return value |
| 26 | crates/javac-core/src/comp/check.rs:180 | clone | types_inferred | src/java.base/share/classes/java/lang/Class.java:1157 \| src/java.base/share/cla | return cloneArray ? interfaces.clone() : interfaces; | 需证据(候选值使用) | upstream candidate uses return value |
| 27 | crates/javac-core/src/comp/check.rs:1931 | validate_annotation_values | visit_annotation | NOT FOUND |  | 需证据 | no upstream call candidate |
| 28 | crates/javac-core/src/comp/deferred_attr.rs:1315 | deferred_type_check | recover | NOT FOUND |  | 需证据 | no upstream call candidate |
| 29 | crates/javac-core/src/comp/enter.rs:824 | class_enter_list | complete | NOT FOUND |  | 需证据 | no upstream call candidate |
| 30 | crates/javac-core/src/comp/infer.rs:2035 | upgrade | done | NOT FOUND |  | 需证据 | no upstream call candidate |
| 31 | crates/javac-core/src/comp/infer.rs:2129 | upgrade | done | NOT FOUND |  | 需证据 | no upstream call candidate |
| 32 | crates/javac-core/src/comp/infer.rs:4005 | compatible | instantiate_functional_interface | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Infer.java:677 \| src/jd | checkContext.compatible(owntype, funcInterface, types.noWarnings); | 需证据(候选值使用) | upstream candidate uses return value |
| 33 | crates/javac-core/src/comp/lower.rs:4002 | get_const_value | class_of_type | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Lower.java:1880 \| src/j | ((VarSymbol)typeSym).getConstValue(); // ensure initializer is evaluated | 需证据(候选值使用) | upstream candidate uses return value |
| 34 | crates/javac-core/src/comp/match_bindings.rs:475 | clone | intersection_reports_a_name_clash_once | src/java.base/share/classes/java/lang/Class.java:1157 \| src/java.base/share/cla | return cloneArray ? interfaces.clone() : interfaces; | 需证据(候选值使用) | upstream candidate uses return value |
| 35 | crates/javac-core/src/comp/member_enter.rs:387 | scope_leave | visit_method_def | NOT FOUND |  | 需证据 | no upstream call candidate |
| 36 | crates/javac-core/src/comp/resolve.rs:2592 | complete_symbol | lookup_package | NOT FOUND |  | 需证据 | no upstream call candidate |
| 37 | crates/javac-core/src/comp/resolve.rs:5209 | clone | check_arg | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:1543 \| src | return bestSoFar.clone(origin); | 需证据(候选值使用) | upstream candidate uses return value |
| 38 | crates/javac-core/src/comp/resolve.rs:8973 | instance | instance_requires_registration | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:122 \| src/ | syms = Symtab.instance(context); | 需证据(候选值使用) | upstream candidate uses return value |
| 39 | crates/javac-core/src/file/base_file_manager.rs:817 | wait_timeout | spawn_deferred_close | NOT FOUND |  | 需证据 | no upstream call candidate |
| 40 | crates/javac-core/src/file/javac_file_manager.rs:1769 | js | system_modules_image_is_listed_and_module_locations_resolve | NOT FOUND |  | 需证据 | no upstream call candidate |
| 41 | crates/javac-core/src/file/locations.rs:1540 | handler_index | get_location_for_module_path | NOT FOUND |  | 需证据 | no upstream call candidate |
| 42 | crates/javac-core/src/file/path_file_object.rs:1091 | remove_dir_all | directories_are_not_supported | NOT FOUND |  | 需证据 | no upstream call candidate |
| 43 | crates/javac-core/src/jvm/module_name_reader.rs:338 | remove_file | reads_the_module_name_from_a_path | NOT FOUND |  | 需证据 | no upstream call candidate |
| 44 | crates/javac-core/src/parser/javac_parser.rs:1633 | illegal_at_token | type_arguments_opt_list_mode | NOT FOUND |  | 需证据 | no upstream call candidate |
| 45 | crates/javac-core/src/parser/javac_parser.rs:1785 | restricted_type_not_allowed_here | unannotated_type | NOT FOUND |  | 需证据 | no upstream call candidate |
| 46 | crates/javac-core/src/parser/javac_parser.rs:2001 | token_arg | arguments | NOT FOUND |  | 需证据 | no upstream call candidate |
| 47 | crates/javac-core/src/parser/javac_parser.rs:2148 | illegal_at_token | creator | NOT FOUND |  | 需证据 | no upstream call candidate |
| 48 | crates/javac-core/src/parser/javac_parser.rs:3317 | illegal | term3_rest | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:1422  | t = illegal(); | 需证据(候选值使用) | upstream candidate uses return value |
| 49 | crates/javac-core/src/parser/javac_parser.rs:4050 | token_arg | variable_declarator_rest | NOT FOUND |  | 需证据 | no upstream call candidate |
| 50 | crates/javac-core/src/parser/javac_parser.rs:4077 | syntax_error | variable_declarator_rest | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3449  | else if (reqInit) syntaxError(token.pos, Errors.Expected(EQ)); | 需证据(候选值使用) | upstream candidate uses return value |
| 51 | crates/javac-core/src/parser/javac_parser.rs:4083 | syntax_error | variable_declarator_rest | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3449  | else if (reqInit) syntaxError(token.pos, Errors.Expected(EQ)); | 需证据(候选值使用) | upstream candidate uses return value |
| 52 | crates/javac-core/src/parser/javac_parser.rs:4091 | syntax_error | variable_declarator_rest | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3449  | else if (reqInit) syntaxError(token.pos, Errors.Expected(EQ)); | 需证据(候选值使用) | upstream candidate uses return value |
| 53 | crates/javac-core/src/parser/javac_parser.rs:4636 | try_without_catch_finally_or_resource_decls | parse_simple_statement | NOT FOUND |  | 需证据 | no upstream call candidate |
| 54 | crates/javac-core/src/parser/javac_parser.rs:4873 | token_arg | switch_block_statement_groups | NOT FOUND |  | 需证据 | no upstream call candidate |
| 55 | crates/javac-core/src/parser/javac_parser.rs:5767 | report_syntax_error_pos | formal_parameters_impl | NOT FOUND |  | 需证据 | no upstream call candidate |
| 56 | crates/javac-core/src/parser/javac_parser.rs:6026 | expected_module | parse_compilation_unit_impl | NOT FOUND |  | 需证据 | no upstream call candidate |
| 57 | crates/javac-core/src/parser/javac_parser.rs:6197 | syntax_error | module_directive_list | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:3879  | JCErroneous erroneousTree = syntaxError(token.pos, List.of(mods), Errors.RecordH | 需证据(候选值使用) | upstream candidate uses return value |
| 58 | crates/javac-core/src/platform/jdk_platform_provider.rs:683 | js | a_non_numeric_platform_name_throws_number_format | NOT FOUND |  | 需证据 | no upstream call candidate |
| 59 | crates/javac-core/src/platform/platform_utils.rs:203 | js | lookup_splits_at_the_first_colon_only | NOT FOUND |  | 需证据 | no upstream call candidate |
| 60 | crates/javac-core/src/util/arena.rs:128 | get | get_out_of_range_panics | make/langtools/src/classes/build/tools/symbolgenerator/CreateSymbols.java:228 \| | LoadDescriptions data = load(ctDescriptionFileExtra != null ? Paths.get(ctDescri | 需证据(候选值使用) | upstream candidate uses return value |
| 61 | crates/javac-core/src/util/arena.rs:135 | get_mut | get_mut_out_of_range_panics | NOT FOUND |  | 需证据 | no upstream call candidate |
| 62 | crates/javac-core/src/util/forwarding_diagnostic_formatter.rs:220 | get_configuration | forwards_formatting_calls | src/jdk.compiler/share/classes/com/sun/tools/javac/util/ForwardingDiagnosticForm | this.configuration = new ForwardingConfiguration(formatter.getConfiguration()); | 需证据(候选值使用) | upstream candidate uses return value |
| 63 | crates/javac-core/src/util/forwarding_diagnostic_formatter.rs:253 | get_configuration | forwards_configuration | src/jdk.compiler/share/classes/com/sun/tools/javac/util/ForwardingDiagnosticForm | this.configuration = new ForwardingConfiguration(formatter.getConfiguration()); | 需证据(候选值使用) | upstream candidate uses return value |
| 64 | crates/javac-source/src/doctree/doc_tree.rs:623 | as_doc_comment_tree | doc_tree_is_object_safe_with_conversion_accessors | NOT FOUND |  | 需证据 | no upstream call candidate |
| 65 | crates/javac-source/src/tree/tree.rs:1332 | as_annotation_tree | tree_is_object_safe_with_conversion_accessors | NOT FOUND |  | 需证据 | no upstream call candidate |
| 66 | crates/javac-source/src/util/doc_tree_path.rs:63 | scan_path | get_path_in_path | NOT FOUND |  | 需证据 | no upstream call candidate |
| 67 | crates/javac-source/src/util/doc_tree_path.rs:208 | new_child | doc_comment_child_is_rejected | NOT FOUND |  | 需证据 | no upstream call candidate |
| 68 | crates/javac-source/src/util/tree_path.rs:63 | scan_path | get_path_in_path | NOT FOUND |  | 需证据 | no upstream call candidate |
| 69 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:229 | display | run | NOT FOUND |  | 需证据 | no upstream call candidate |
| 70 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:239 | display | run | NOT FOUND |  | 需证据 | no upstream call candidate |
| 71 | crates/javac-tests/src/corpus.rs:1143 | remove_dir_all | generates_sample_baselines_incrementally | NOT FOUND |  | 需证据 | no upstream call candidate |
| 72 | crates/javac-tests/src/diff.rs:651 | differs | write_text_diff | NOT FOUND |  | 需证据 | no upstream call candidate |
| 73 | crates/javac-tests/src/javac.rs:290 | remove_dir_all | drop | NOT FOUND |  | 需证据 | no upstream call candidate |
| 74 | crates/javac-tools/src/tools/forwarding_java_file_manager.rs:316 | next | handle_option | make/langtools/src/classes/build/tools/symbolgenerator/CreateSymbols.java:468 \| | ClassDescription desc = it.next(); | 需证据(候选值使用) | upstream candidate uses return value |
| 75 | crates/javac-tools/src/tools/tool_provider.rs:83 | system_java_compiler_name | get_system_java_compiler | NOT FOUND |  | 需证据 | no upstream call candidate |
| 76 | crates/javac-tools/src/tools/tool_provider.rs:110 | system_documentation_tool_name | get_system_documentation_tool | NOT FOUND |  | 需证据 | no upstream call candidate |
| 77 | xtask/src/gen_resources.rs:569 | push_str | generate_rust | NOT FOUND |  | 需证据 | no upstream call candidate |
| 78 | xtask/src/gen_resources.rs:600 | len | generate_rust | NOT FOUND |  | 需证据 | no upstream call candidate |
| 79 | crates/javac-core/src/code/preview.rs:531 | disabled_error | disabled_error_requires_preview_to_be_disabled | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Preview.java:247 | log.error(JCDiagnostic.DiagnosticFlag.SOURCE_LEVEL, pos, disabledError(feature)) | 可安全忽略 | upstream statement call candidates |
| 80 | crates/javac-core/src/code/source.rs:786 | name_fragment | name_fragment_requires_a_fragment | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Preview.java:142 \| src/ | Warnings.PreviewFeatureUsePlural(feature.nameFragment()) : | 可安全忽略 | upstream statement call candidates |
| 81 | crates/javac-core/src/code/type_.rs:1881 | get_components | components_panics_for_non_intersection | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Type.java:1283 | getComponents(); | 可安全忽略 | upstream statement call candidates |
| 82 | crates/javac-core/src/code/type_.rs:1887 | get_alternatives | alternatives_panics_for_non_union | src/jdk.compiler/share/classes/com/sun/tools/javac/code/Type.java:1210 | public java.util.List<? extends TypeMirror> getAlternatives() { | 可安全忽略 | upstream statement call candidates |
| 83 | crates/javac-core/src/comp/annotate.rs:1982 | annotate_now | enter_type_annotations | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:1031 \| sr | annotateNow(s, annotations, env, true, isTypeParam); | 可安全忽略 | upstream statement call candidates |
| 84 | crates/javac-core/src/comp/attr_recover.rs:469 | visit | basic_method_invocation_recovery | src/java.base/share/classes/java/lang/Module.java:1525 \| src/java.compiler/shar | cw.visit(version, | 可安全忽略 | upstream statement call candidates |
| 85 | crates/javac-core/src/comp/attr.rs:7493 | flush | visit_block | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:1439 \| src/jd | annotate.flush(); | 可安全忽略 | upstream statement call candidates |
| 86 | crates/javac-core/src/comp/attr.rs:8335 | flush | visit_binding_pattern | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:4143 \| src/jd | annotate.flush(); | 可安全忽略 | upstream statement call candidates |
| 87 | crates/javac-core/src/comp/attr.rs:12199 | take | attrib_class | test/langtools/tools/javac/flow/T8042741/LambdaArgumentsTest.java:32 \| test/lan | interface Consumer<E> { void take(E arg); } | 可安全忽略 | upstream statement call candidates |
| 88 | crates/javac-core/src/comp/member_enter.rs:269 | tree | member_enter | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/LambdaToMethod.java:853 | * Set varargsElement field on a given tree (must be either a new class tree | 可安全忽略 | upstream statement call candidates |
| 89 | crates/javac-core/src/comp/member_enter.rs:643 | attrib_expr | check_receiver | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/MemberEnter.java:321 | attr.attribExpr(tree.nameexpr, localEnv); | 可安全忽略 | upstream statement call candidates |
| 90 | crates/javac-core/src/compiler_context.rs:1057 | new | out_of_range_access_panics | src/jdk.compiler/share/classes/com/sun/tools/javac/tree/JCTree.java:1827 \| src/ | * A new(...) operation. | 可安全忽略 | upstream statement call candidates |
| 91 | crates/javac-core/src/file/javac_file_manager.rs:1306 | close | close | src/jdk.compiler/share/classes/com/sun/tools/javac/file/JavacFileManager.java:66 | fileSystem.close(); | 可安全忽略 | upstream statement call candidates |
| 92 | crates/javac-core/src/file/path_file_object.rs:1090 | manager | directories_are_not_supported | src/java.base/share/classes/java/lang/RuntimePermission.java:109 | *   <td>Setting of the security manager (possibly replacing an existing one) | 可安全忽略 | upstream statement call candidates |
| 93 | crates/javac-core/src/parser/javac_parser.rs:783 | syntax_error | check_no_mods_at | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:825 \ | syntaxError(result.pos, Errors.RestrictedTypeNotAllowedHere(restrictedTypeName)) | 可安全忽略 | upstream statement call candidates |
| 94 | crates/javac-core/src/parser/javac_parser.rs:3148 | illegal | term3 | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:1173  | } else return illegal(); | 可安全忽略 | upstream statement call candidates |
| 95 | crates/javac-core/src/parser/javac_parser.rs:3170 | illegal | term3 | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:1173  | } else return illegal(); | 可安全忽略 | upstream statement call candidates |
| 96 | crates/javac-core/src/parser/javac_parser.rs:3213 | illegal | term3 | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:1173  | } else return illegal(); | 可安全忽略 | upstream statement call candidates |
| 97 | crates/javac-core/src/parser/javac_parser.rs:3228 | illegal | term3 | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:1173  | } else return illegal(); | 可安全忽略 | upstream statement call candidates |
| 98 | crates/javac-core/src/parser/javac_parser.rs:3236 | illegal | term3 | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:1173  | } else return illegal(); | 可安全忽略 | upstream statement call candidates |
| 99 | crates/javac-core/src/parser/javac_parser.rs:3297 | syntax_error | term3 | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:825 \ | syntaxError(result.pos, Errors.RestrictedTypeNotAllowedHere(restrictedTypeName)) | 可安全忽略 | upstream statement call candidates |
| 100 | crates/javac-core/src/parser/javac_parser.rs:5497 | illegal | class_or_interface_or_record_body_declaration | src/jdk.compiler/share/classes/com/sun/tools/javac/parser/JavacParser.java:4312  | illegal(annosAfterParams.head.pos); | 可安全忽略 | upstream statement call candidates |
| 101 | crates/javac-core/src/parser/javac_parser.rs:5577 | new | class_or_interface_or_record_body_declaration | src/jdk.compiler/share/classes/com/sun/tools/javac/tree/JCTree.java:1827 \| src/ | * A new(...) operation. | 可安全忽略 | upstream statement call candidates |
| 102 | crates/javac-core/src/platform/jdk_platform_provider.rs:376 | from | get_file_manager | src/java.base/share/classes/java/lang/ProcessBuilder.java:460 \| src/java.base/s | *     {@link Redirect#from Redirect.from(File)} | 可安全忽略 | upstream statement call candidates |
| 103 | crates/javac-core/src/tree/jc_tree.rs:1579 | no_assign_op | tag_no_assign_op_panics_for_non_assignment_tags | src/jdk.compiler/share/classes/com/sun/tools/javac/tree/JCTree.java:401 | throw new AssertionError("noAssignOp() method is not available for non assignmen | 可安全忽略 | upstream statement call candidates |
| 104 | crates/javac-core/src/util/graph_utils.rs:473 | tarjan | tarjan_state_is_filled_in | src/jdk.compiler/share/classes/com/sun/tools/javac/util/GraphUtils.java:153 | public static <D, N extends TarjanNode<D, N>> List<? extends List<? extends N>>  | 可安全忽略 | upstream statement call candidates |
| 105 | crates/javac-core/src/util/graph_utils.rs:533 | tarjan | compare_to_uses_tarjan_index | src/jdk.compiler/share/classes/com/sun/tools/javac/util/GraphUtils.java:153 | public static <D, N extends TarjanNode<D, N>> List<? extends List<? extends N>>  | 可安全忽略 | upstream statement call candidates |
| 106 | crates/javac-core/src/util/javac_messages.rs:463 | new | missing_bundle_panics_like_internal_error | src/jdk.compiler/share/classes/com/sun/tools/javac/tree/JCTree.java:1827 \| src/ | * A new(...) operation. | 可安全忽略 | upstream statement call candidates |
| 107 | crates/javac-core/tests/e2e_compile.rs:184 | remove_dir_all | compile | NOT FOUND |  | 可安全忽略(测试辅助) | no upstream call candidate |
| 108 | crates/javac-core/tests/operators_oracle.rs:240 | nil | operators_oracle_matches_the_port | make/langtools/tools/genstubs/GenStubs.java:249 \| src/jdk.compiler/share/classe | tree.annotations = com.sun.tools.javac.util.List.nil(); | 可安全忽略(测试辅助) | upstream candidate uses return value |
| 109 | crates/javac-ctsym/src/lib.rs:1680 | open | rejects_release_directories_with_invalid_characters | src/java.base/share/classes/java/lang/Class.java:517 \| src/jdk.compiler/share/c | *         {@link ModuleReader#open(String)} to read the bytes of a class file | 可安全忽略 | upstream statement call candidates |
| 110 | crates/javac-source/src/util/doc_tree_factory.rs:772 | from | at_sets_the_position_recorded_in_subsequent_nodes | src/java.base/share/classes/java/lang/ProcessBuilder.java:460 \| src/java.base/s | *     {@link Redirect#from Redirect.from(File)} | 可安全忽略 | upstream statement call candidates |
| 111 | crates/javac-source/src/util/simple_doc_tree_visitor.rs:639 | visit | default_action_panics_when_default_value_is_null | src/java.base/share/classes/java/lang/Module.java:1525 \| src/java.compiler/shar | cw.visit(version, | 可安全忽略 | upstream statement call candidates |
| 112 | crates/javac-source/src/util/simple_tree_visitor.rs:1016 | visit_binary | default_action_panics_when_default_value_is_null | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Attr.java:5948 \| src/jd | super.visitBinary(that); | 可安全忽略 | upstream statement call candidates |
| 113 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:202 | data | run | src/java.base/share/classes/java/lang/ClassLoader.java:99 \| src/java.base/share | * locating resources. A resource is some data (a "{@code .class}" file, | 可安全忽略 | upstream statement call candidates |
| 114 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:206 |  | run | make/langtools/src/classes/build/tools/symbolgenerator/CreateSymbols.java:2 \| m | * Copyright (c) 2006, 2021, Oracle and/or its affiliates. All rights reserved. | 可安全忽略 | upstream statement call candidates |
| 115 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:210 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 116 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:211 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 117 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:212 |  | run | make/langtools/src/classes/build/tools/symbolgenerator/CreateSymbols.java:2 \| m | * Copyright (c) 2006, 2021, Oracle and/or its affiliates. All rights reserved. | 可安全忽略 | upstream statement call candidates |
| 118 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:216 |  | run | make/langtools/src/classes/build/tools/symbolgenerator/CreateSymbols.java:2 \| m | * Copyright (c) 2006, 2021, Oracle and/or its affiliates. All rights reserved. | 可安全忽略 | upstream statement call candidates |
| 119 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:220 |  | run | make/langtools/src/classes/build/tools/symbolgenerator/CreateSymbols.java:2 \| m | * Copyright (c) 2006, 2021, Oracle and/or its affiliates. All rights reserved. | 可安全忽略 | upstream statement call candidates |
| 120 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:224 |  | run | make/langtools/src/classes/build/tools/symbolgenerator/CreateSymbols.java:2 \| m | * Copyright (c) 2006, 2021, Oracle and/or its affiliates. All rights reserved. | 可安全忽略 | upstream statement call candidates |
| 121 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:228 | len |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 122 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:234 | display |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 123 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:235 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 124 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:237 | join |  | make/langtools/tools/propertiesparser/gen/ClassGenerator.java:180 \| make/langto |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 125 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:244 | len |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 126 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:251 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 127 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:252 | len |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 128 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:397 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 129 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:398 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 130 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:399 | list_or_dash |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 131 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:401 | join |  | make/langtools/tools/propertiesparser/gen/ClassGenerator.java:180 \| make/langto |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 132 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:403 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 133 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:404 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 134 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:405 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 135 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:406 | list_or_dash |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 136 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:407 | list_or_dash |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 137 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:408 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 138 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:410 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 139 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:412 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 140 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:414 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 141 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:416 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 142 | crates/javac-tests/src/bin/gen_diagnostics_oracle.rs:421 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 143 | crates/javac-tests/src/corpus.rs:392 | display |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 144 | crates/javac-tests/src/corpus.rs:393 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 145 | crates/javac-tests/src/corpus.rs:395 | as_str |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 146 | crates/javac-tests/src/corpus.rs:398 | as_str |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 147 | crates/javac-tests/src/corpus.rs:400 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 148 | crates/javac-tests/src/corpus.rs:403 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 149 | crates/javac-tests/src/corpus.rs:406 | join |  | make/langtools/tools/propertiesparser/gen/ClassGenerator.java:180 \| make/langto |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 150 | crates/javac-tests/src/corpus.rs:409 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 151 | crates/javac-tests/src/corpus.rs:412 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 152 | crates/javac-tests/src/corpus.rs:415 | join |  | make/langtools/tools/propertiesparser/gen/ClassGenerator.java:180 \| make/langto |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 153 | crates/javac-tests/src/corpus.rs:418 | join |  | make/langtools/tools/propertiesparser/gen/ClassGenerator.java:180 \| make/langto |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 154 | crates/javac-tests/src/corpus.rs:752 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 155 | crates/javac-tests/src/corpus.rs:753 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 156 | crates/javac-tests/src/corpus.rs:754 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 157 | crates/javac-tests/src/diff.rs:611 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 158 | crates/javac-tests/src/diff.rs:620 | join |  | make/langtools/tools/propertiesparser/gen/ClassGenerator.java:180 \| make/langto |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 159 | crates/javac-tests/src/diff.rs:622 |  | format_details | make/langtools/src/classes/build/tools/symbolgenerator/CreateSymbols.java:2 \| m | * Copyright (c) 2006, 2021, Oracle and/or its affiliates. All rights reserved. | 可安全忽略 | upstream statement call candidates |
| 160 | crates/javac-tests/src/diff.rs:631 |  | format_details | make/langtools/src/classes/build/tools/symbolgenerator/CreateSymbols.java:2 \| m | * Copyright (c) 2006, 2021, Oracle and/or its affiliates. All rights reserved. | 可安全忽略 | upstream statement call candidates |
| 161 | crates/javac-tests/src/diff.rs:659 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 162 | crates/javac-tests/src/diff.rs:660 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 163 | crates/javac-tests/src/diff.rs:661 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 164 | crates/javac-tests/src/diff.rs:664 |  | write_text_diff | make/langtools/src/classes/build/tools/symbolgenerator/CreateSymbols.java:2 \| m | * Copyright (c) 2006, 2021, Oracle and/or its affiliates. All rights reserved. | 可安全忽略 | upstream statement call candidates |
| 165 | crates/javac-tests/src/diff.rs:671 |  | write_text_diff | make/langtools/src/classes/build/tools/symbolgenerator/CreateSymbols.java:2 \| m | * Copyright (c) 2006, 2021, Oracle and/or its affiliates. All rights reserved. | 可安全忽略 | upstream statement call candidates |
| 166 | crates/javac-tools/src/lang/model/util/elements.rs:385 | record_components_in | record_component_for | src/java.compiler/share/classes/javax/lang/model/util/ElementFilter.java:118 \|  | recordComponentsIn(Iterable<? extends Element> elements) { | 可安全忽略 | upstream statement call candidates |
| 167 | crates/javac-tools/src/tools/forwarding_java_file_manager.rs:412 | new | sized_generic_forwarding | src/jdk.compiler/share/classes/com/sun/tools/javac/tree/JCTree.java:1827 \| src/ | * A new(...) operation. | 可安全忽略 | upstream statement call candidates |
| 168 | crates/javac-tools/src/tools/java_file_manager.rs:842 | new | sized_generic_default_method | src/jdk.compiler/share/classes/com/sun/tools/javac/tree/JCTree.java:1827 \| src/ | * A new(...) operation. | 可安全忽略 | upstream statement call candidates |
| 169 | xtask/src/gen_resources.rs:527 |  | generate_rust | make/langtools/src/classes/build/tools/symbolgenerator/CreateSymbols.java:2 \| m | * Copyright (c) 2006, 2021, Oracle and/or its affiliates. All rights reserved. | 可安全忽略 | upstream statement call candidates |
| 170 | xtask/src/gen_resources.rs:547 | rust_module |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 171 | xtask/src/gen_resources.rs:560 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 172 | xtask/src/gen_resources.rs:562 | join | generate_rust | make/langtools/tools/propertiesparser/gen/ClassGenerator.java:180 \| make/langto | String.join("\n", generateImports(importedTypes)), | 可安全忽略 | upstream statement call candidates |
| 173 | xtask/src/gen_resources.rs:578 | join | generate_rust | make/langtools/tools/propertiesparser/gen/ClassGenerator.java:180 \| make/langto | String.join("\n", generateImports(importedTypes)), | 可安全忽略 | upstream statement call candidates |
| 174 | xtask/src/gen_resources.rs:593 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 175 | xtask/src/gen_resources.rs:620 |  | generate_message_table | make/langtools/src/classes/build/tools/symbolgenerator/CreateSymbols.java:2 \| m | * Copyright (c) 2006, 2021, Oracle and/or its affiliates. All rights reserved. | 可安全忽略 | upstream statement call candidates |
| 176 | xtask/src/gen_resources.rs:627 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 177 | xtask/src/gen_resources.rs:633 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |
| 178 | xtask/src/gen_resources.rs:635 |  |  | NOT FOUND |  | 可安全忽略(宏) | macro (write/format side effect; return value ignored) |

## 高风险 Top 20(供人工抽查)

| 排名 | 本地位置 | 被调函数 | 判定 | 上游候选 | 风险理由 |
|---:|---|---|---|---|---|
| 1 | crates/javac-core/src/comp/annotate.rs:1091 | attribute_annotation_value | 必须使用(候选值使用) | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:512 \| src | 上游候选值使用; 表达式含 ?(Result 传播但成功值被丢弃); 关键编译阶段调用 |
| 2 | crates/javac-core/src/comp/annotate.rs:1105 | attribute_annotation_value | 必须使用(候选值使用) | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:512 \| src | 上游候选值使用; 表达式含 ?(Result 传播但成功值被丢弃); 关键编译阶段调用 |
| 3 | crates/javac-core/src/comp/annotate.rs:1245 | attribute_annotation_value | 必须使用(候选值使用) | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:534 \| src | 上游候选值使用; 表达式含 ?(Result 传播但成功值被丢弃); 关键编译阶段调用 |
| 4 | crates/javac-core/src/comp/annotate.rs:1282 | attribute_annotation | 必须使用(候选值使用) | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Annotate.java:589 \| src | 上游候选值使用; 表达式含 ?(Result 传播但成功值被丢弃); 关键编译阶段调用 |
| 5 | crates/javac-core/src/comp/lower.rs:2510 | translate | 必须使用(候选值使用) | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Lower.java:4103 \| src/j | 上游候选值使用; 关键编译阶段调用 |
| 6 | crates/javac-core/src/comp/resolve.rs:9054 | merge_results | 必须使用(候选值使用) | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:3708 | 上游候选值使用; 关键编译阶段调用 |
| 7 | crates/javac-core/src/comp/resolve.rs:9063 | merge_results | 必须使用(候选值使用) | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:3708 | 上游候选值使用; 关键编译阶段调用 |
| 8 | crates/javac-core/src/comp/attr.rs:1712 | attrib_tree_with | 需证据 | NOT FOUND | 上游定位需证据; 关键编译阶段调用 |
| 9 | crates/javac-core/src/comp/attr.rs:10468 | check_after_inference | 需证据 | NOT FOUND | 上游定位需证据; 关键编译阶段调用 |
| 10 | crates/javac-core/src/comp/check.rs:1931 | validate_annotation_values | 需证据 | NOT FOUND | 上游定位需证据; 关键编译阶段调用 |
| 11 | crates/javac-core/src/comp/enter.rs:824 | class_enter_list | 需证据 | NOT FOUND | 上游定位需证据; 关键编译阶段调用 |
| 12 | crates/javac-core/src/parser/javac_parser.rs:5767 | report_syntax_error_pos | 需证据 | NOT FOUND | 上游定位需证据; 关键编译阶段调用 |
| 13 | xtask/src/gen_resources.rs:569 | push_str | 需证据 | NOT FOUND | 上游定位需证据; 表达式含 ?(Result 传播但成功值被丢弃) |
| 14 | xtask/src/gen_resources.rs:600 | len | 需证据 | NOT FOUND | 上游定位需证据; 表达式含 ?(Result 传播但成功值被丢弃); 通用名候选不可靠 |
| 15 | crates/javac-core/src/code/type_annotations.rs:1163 | clone | 需证据(候选值使用) | src/java.base/share/classes/java/lang/Class.java:1157 \| src/java.base/share/cla | 上游定位需证据; 通用名候选不可靠 |
| 16 | crates/javac-core/src/comp/check.rs:180 | clone | 需证据(候选值使用) | src/java.base/share/classes/java/lang/Class.java:1157 \| src/java.base/share/cla | 上游定位需证据; 通用名候选不可靠 |
| 17 | crates/javac-core/src/comp/infer.rs:2035 | upgrade | 需证据 | NOT FOUND | 上游定位需证据 |
| 18 | crates/javac-core/src/comp/infer.rs:2129 | upgrade | 需证据 | NOT FOUND | 上游定位需证据 |
| 19 | crates/javac-core/src/comp/resolve.rs:5209 | clone | 需证据(候选值使用) | src/jdk.compiler/share/classes/com/sun/tools/javac/comp/Resolve.java:1543 \| src | 上游定位需证据; 通用名候选不可靠 |
| 20 | crates/javac-core/src/file/locations.rs:1540 | handler_index | 需证据 | NOT FOUND | 上游定位需证据 |

## 限制与后续
- 上游定位基于方法名映射;通用名(clone/get_const_value/is_static/translate 等)与诊断副作用名(syntax_error/illegal/report*)候选不可靠,已降级为需证据;
- 宏类与测试辅助为原有显式忽略语义,未改动;
- 待负责人批准后,仅对最终确认的必须使用项逐项修复(附上游行号),其余保持显式忽略并补注释,不做批量删除。

## 上游核对清单(负责人批准后的人工核对,本轮)

核对范围:分析表中 78 项生产候选(已排除 28 项测试函数/测试辅助与 17 项宏/工具/元组误报);逐项打开 reference 上游方法核对调用点是否使用返回值。结论:**无判定为必须使用的项,本轮无代码修改**。

| # | 本地位置 | 上游证据(方法:行) | 上游调用用法 | 判定 | 处置 |
|---:|---|---|---|---|---|
| 1 | comp/annotate.rs:1091 | comp/Annotate.java:512,518 | attributeAnnotationValue(...); 语句,且 return null | 误报(Lead 已标) | 不改 |
| 2 | comp/annotate.rs:1105 | comp/Annotate.java:512,518 | 同上 | 误报(Lead 已标) | 不改 |
| 3 | comp/annotate.rs:1245 | comp/Annotate.java:568-572 | for (...) attributeAnnotationValue(syms.errType, l.head, env); | 等价忽略 | 保留显式忽略 |
| 4 | comp/annotate.rs:1282 | comp/Annotate.java:589 | attributeAnnotation((JCAnnotation)tree, syms.errType, env); | 误报(Lead 已标) | 不改 |
| 5 | comp/attr.rs:1712 | comp/Attr.java:5128-5129 | for (JCTree err : tree.errs) attribTree(err, errEnv, new ResultInfo(...)); | 等价忽略 | 保留 |
| 6 | comp/attr.rs:5197 | comp/Attr.java:1861 | ((VarSymbol)s).getConstValue(); // ensure initializer is evaluated | 等价忽略 | 保留 |
| 7 | comp/attr.rs:9001 | comp/Attr.java:1861 | 同上(enum constant init evaluation) | 等价忽略 | 保留 |
| 8 | comp/attr.rs:14549 | comp/Attr.java:1296 | v.getConstValue(); // ensure compile-time constant initializer is evaluated | 等价忽略 | 保留 |
| 9 | comp/attr.rs:9519 | comp/Attr.java:429-431 | deferredAttr.attribSpeculative(root, env, resultInfo, ...); | 等价忽略 | 保留 |
| 10 | comp/attr.rs:10468 | comp/Attr.java:264 | check(tree, inferenceContext.asInstType(found), ownkind, pendingResult); (lambda 语句) | 等价忽略 | 保留 |
| 11 | comp/check.rs:180 | comp/Check.java:604-608 | solvedContext -> checkType(pos, ..., checkContext) (lambda 语句) | 等价忽略 | 保留 |
| 12 | comp/check.rs:1931 | comp/Check.java:2863 | validateAnnotation(tree); | 等价忽略 | 保留 |
| 13 | comp/deferred_attr.rs:1315 | comp/DeferredAttr.java:500 | attr.attribTree(tree, speculativeEnv, resultInfo); | 等价忽略(异常传播差异既有登记) | 保留 |
| 14 | comp/enter.rs:824 | comp/Enter.java:603 | classEnter(trees, null); | 等价忽略 | 保留 |
| 15 | comp/infer.rs:2035 | 无上游对应(Rust Weak::upgrade 适配) | - | 适配层 | 不改 |
| 16 | comp/infer.rs:2129 | 无上游对应(Rust Weak::upgrade 适配) | - | 适配层 | 不改 |
| 17 | comp/infer.rs:4005 | comp/Infer.java:677 | checkContext.compatible(owntype, funcInterface, types.noWarnings); | 等价忽略 | 保留 |
| 18 | comp/lower.rs:2510 | comp/Lower.java:4144 | translate(cdef, (JCExpression)null); | 等价忽略 | 保留 |
| 19 | comp/lower.rs:4002 | comp/Lower.java:1879-1880 | ((VarSymbol)typeSym).getConstValue(); // ensure initializer is evaluated | 等价忽略 | 保留 |
| 20 | comp/member_enter.rs:387 | comp/MemberEnter.java:218 | localEnv.info.scope.leave(); | 等价忽略 | 保留 |
| 21 | comp/resolve.rs:2592 | comp/Resolve.java:2128 | sym.complete(); | 等价忽略(异常传播差异既有登记) | 保留 |
| 22 | comp/resolve.rs:5209 | 无上游对应(本地 Rc clone 冗余) | - | 误报 | 不改 |
| 23 | file/base_file_manager.rs:817 | 上游 Condition.await 无返回值使用 | Rust Mutex/Condvar poison 适配 | 适配层 | 保留显式忽略 |
| 24 | file/locations.rs:1540 | 本地方法 TODO(S3c)未实现(ModuleTable.get(Path) 未移植) | - | 需证据/未实现 | 不改(登记既有 TODO) |
| 25 | parser/javac_parser.rs:1633 | comp/JavacParser.java:2088 | illegal(); | 等价忽略 | 保留 |
| 26 | parser/javac_parser.rs:2148 | comp/JavacParser.java:2331 | illegal(); | 等价忽略 | 保留 |
| 27 | parser/javac_parser.rs:3317 | comp/JavacParser.java:1541 | if (typeArgs != null) illegal(); | 等价忽略 | 保留 |
| 28 | parser/javac_parser.rs:4077 | comp/JavacParser.java:3449 | else if (reqInit) syntaxError(token.pos, Errors.Expected(EQ)); | 等价忽略 | 保留 |
| 29 | parser/javac_parser.rs:4083 | comp/JavacParser.java:3449 | 同上 | 等价忽略 | 保留 |
| 30 | parser/javac_parser.rs:4091 | comp/JavacParser.java:3449 | 同上 | 等价忽略 | 保留 |
| 31 | parser/javac_parser.rs:5767 | comp/JavacParser.java:4611 | reportSyntaxError(S.prevToken().endPos, Errors.Expected3(...)); | 等价忽略(本地返回 Option 适配) | 保留 |
| 32 | parser/javac_parser.rs:6197 | comp/JavacParser.java:3812-3815 | setErrorEndPos(pos); reportSyntaxError(pos, Errors.InvalidModuleDirective); break; | 等价忽略 | 保留 |
| 33 | code/type_annotations.rs:1163 | comp/TypeAnnotations.java:350 | typeWithAnnotations(typetree, type, typeAnnotations, typeAnnotations, pos); | 等价忽略(副作用设置 type annotation positions) | 保留 |
| 34 | javac-source/src/util/tree_path.rs:63 | reference 未镜像 com/sun/source.util(TreePath.scanPath) | 本地 API 内部实现 | 适配层 | 不改 |
| 35 | javac-source/src/util/doc_tree_path.rs:63 | 同上 | 本地 API 内部实现 | 适配层 | 不改 |
| 36 | javac-tests/src/javac.rs:290 | 测试辅助(移除临时目录) | - | 测试辅助 | 不改 |

分类统计(核对候选 78 项):测试函数/测试辅助 28;宏/工具/元组误报 17;生产项 33,其中等价忽略 25、误报 4、适配层 3、TODO 未实现 1;必须使用 0。

结论:分析表中所有重点候选(annotate 1245、lower 2510、enter 824、infer 2035/2129、attr 1712/10468、check 1931、resolve 2592/5209/8973 等)经上游行号核对均为语句调用忽略返回值、误报、测试项、适配层或既有登记项;按负责人要求仅修必须使用项,本轮无代码修改。剩余风险:deferred_attr.rs:1315 与 resolve.rs:2592 的 CompletionFailure/异常传播差异为既有登记偏差,不属于"返回值使用"问题,留待后续专项。
