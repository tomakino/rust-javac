# A0: TransPatterns 1:1 移植状态(压缩保护文档)

- 上游:reference/jdk17u/src/jdk.compiler/share/classes/com/sun/tools/javac/comp/TransPatterns.java(871 行)
- 本地目标:crates/javac-core/src/comp/trans_patterns.rs(新增);接线 crates/javac-core/src/main/mini_compiler.rs(desugar:trans_types → TRANSPATTERNS → lower);compile_states 已有 TRANSPATTERNS 枚举值。
- 上游单元清单(全部须在本文件落地):构造/instance;字段 syms/attr/rs/types/operators/names/target/preview/make/env/bindingContext/pendingMatchLabel/debugTransPatterns/currentClass/currentMethodSym/currentValue;translateTopLevelClass;visitTypeTest;visitBindingPattern;visitParenthesizedPattern;visitGuardPattern;visitSwitch;visitSwitchExpression;handleSwitch;principalType;toLoadableConstant;visitBinary;visitConditional;visitIf;visitForLoop;visitWhileLoop;visitDoLoop;visitMethodDef;visitIdent;visitBlock;visitLambda;visitClassDef;visitVarDef;makeTypeTest;makeBinary;makeUnary;convert;BindingContext(abstract);BasicBindingContext;BindingDeclarationFenceBindingContext;makeLit。
- 依赖核查(已核对源码):本地存在=TreeVisitor/accept 框架(trans_types.rs:140-247 translate/translateList)、TreeMaker(at/var_def/let_expr/assign/binary/unary/type_test/type_cast/literal/switch/switch_expression/case/default_case_label/select/ident/type_from_type/qual_ident/apply/block/exec/if_/break_/continue_)、SymbolKind::DynamicMethodSymbol/DynamicVarSymbol、SymbolKind::BindingSymbol、TreeKind::LetExpr、Preview.is_enabled/uses_preview、Target.synthetic_name_char、tree_info::get_start_pos/primary_pattern_tree、CompileStates::TRANSPATTERNS。
- 需补 API(实施时按上游 1:1):TreeInfo.primaryPatternType、Attr.makeNullCheck、Types.boxedTypeOrType、Operators.resolveBinary/resolveUnary(本地 operators.rs 是否有对应公共函数待核对)、Resolve.resolveInternalMethod 适配调用形态、LoadableConstant 构造(indy static args;本地 jvm pool_constant 待核对)、DynamicMethodSymbol 构造参数(bsm/asHandle/type/staticArgs)与 make.QualIdent/Select/Apply 组合、Type.MethodType/ClassType/WildcardType/BoundKind 构造。
- 接线步骤:1) 新增 trans_patterns.rs;2) comp/mod.rs 注册;3) mini_compiler desugar 在 trans_types 之后调用 trans_patterns.translate_top_level_class(env,cdef,make),随后 compile_states.set(TRANSPATTERNS);4) lower 顺序不变。
- 验证计划:官方样例 InstanceofPatternNoSubtype.java / InstanceofReifiableNotSafe.java 真实 e2e 断言诊断;合法 `o instanceof String s && s.length()>0` 编译成功;随后 A0b LambdaToMethod(lambda e2e);每步 cargo check;完成后全 target+clippy+unimplemented 0。
- 当前工作树:attr.rs(visit_type_test/reifiable)与 member_enter.rs(methodEnv returnResult)已作为 A1 checkpoint 提交;本轮尚未新增代码(仅完成前置核查)。
## 子步拆分(实施顺序,防压缩丢状态)

A0.1 共享 TreeTranslator 默认导航:上游 TreeTranslator.java:139-355 的默认 visitor 行为在本地目前分别内联在 lower.rs::navigate(6994+)与 trans_types.rs 的逐方法实现中。TransPatterns 需要同样的默认子节点翻译;方案:新增 crates/javac-core/src/tree/tree_translator.rs 提供 tree_translator_navigate(ctx, tree, translator) 通用函数(从 lower.rs::navigate 1:1 搬移并对 translate 调用走 TreeTranslator trait),TransPatterns 覆盖 visit_tree 调用它;TransTypes/Lower 复用登记。

A0.2 comp/trans_patterns.rs 主体(按上游逐方法;构造、翻译框架、除 switch 外全部方法先落,switch 部分随后同批完成)。

A0.3 API 补齐(按编译错误逐项,1:1):primaryPatternType/makeNullCheck/boxedTypeOrType/Operators.resolve*/LoadableConstant/DynamicMethodSymbol 构造/make.QualIdent/Select/Apply/Type 构造。

A0.4 接线 mini_compiler.desugar(trans_types → TransPatterns.translate_top_level_class → compile_states TRANSPATTERNS → lower)与 comp/mod.rs 注册。

A0.5 官方样例 e2e + 合法 pattern 成功 + 全量验证。

A0b LambdaToMethod(下一批,同样先补默认导航复用)。

## 进度快照(压缩保护)

- 已创建 crates/javac-core/src/comp/trans_patterns.rs(未跟踪,约 3200 行):
  - 结构/构造/translate 框架/translateTopLevelClass/principal_type/make_type_test/make_binary/make_unary/convert/BindingContext(Default/Basic/Fence,含 binding_declared/get_binding_for/binding_vars/decorate_statement/decorate_expression/try_prepend/pop)/make_lit/to_loadable_constant;
  - impl TreeVisitor for TransPatterns:复制 TreeTranslator 全部 68 个默认导航方法,其中 18 个特殊方法已按上游替换:visit_type_test/visit_binding_pattern/visit_parenthesized_pattern/visit_guard_pattern/visit_binary/visit_conditional/visit_if/visit_for_loop/visit_while_loop/visit_do_loop/visit_method_def/visit_ident/visit_block/visit_lambda/visit_class_def/visit_var_def/visit_switch/visit_switch_expression;handle_switch 已按上游 275-486 编写;
- 当前状态:文件中间损坏(一次方法删除误删 handle_switch 930 行之后片段:unwrap_or('$') 后内容丢失),未注册模块(comp/mod.rs 已回退,工作树保持可编译/全绿);trans_patterns.rs 为未跟踪新文件;
- 下一步:1) 修复 trans_patterns.rs 中 handle_switch 截断(从 unwrap_or('$'); 后补回 temp_name/temp VarSymbol/temp_def/index/index_def/staticArgTypes/staticArgValues/enum_selector/bsm/indy/qualifier/selector_expr/case 重写循环/最终 LetExpr 块,按上游 358-478);2) cargo check;3) 注册 comp/mod.rs;4) mini_compiler.desugar 接线(trans_types → TransPatterns.translate_top_level_class → CompileStates::TRANSPATTERNS → lower);5) e2e 官方样例;6) A0b LambdaToMethod。

## 本轮实施进度(trans_patterns.rs 完成+接线验证)

- trans_patterns.rs 已完整(约 2780 行):结构/字段/构造/translate/translate_top_level_class/principal_type/make_type_test/make_binary/make_unary/convert/BindingContext(Default/Basic/Fence+decorate_statement/decorate_expression/binding_vars)/make_lit/to_loadable_constant;impl TreeVisitor 复用 68 默认导航方法+18 特殊方法按上游替换(含 handle_switch 完整);自由函数(new_var_symbol/case helpers/is_pattern_label/is_null_pattern/literal_value_constant/binding alias helpers/make_hoisted_var_decl)。cargo check 通过。
- 辅助 API 补齐:TreeMaker::literal_object 按上游 TreeMaker.java:899-934 设置常量类型(Boolean→booleanType.constType(1));Java(jvm)Code::emitop2_type 补 INSTANCEOF 栈效果(Code.java:1068-1071:pop1 push int)。
- 接线:mini_compiler.desugar 在 trans_types 后插入 TransPatterns.translate_top_level_class + CompileStates::TRANSTYPES/TRANSPATTERNS,再 lower(已实现并通过编译;因剩余 Gen/Items 适配问题本轮暂回退接线以保持工作树全绿)。
- 实测 Java e2e 官方样例(instanceof pattern)链路:Attr 诊断、TransTypes、TransPatterns(desugar LetExpr/Assign)均已执行;当前阻断:jvm/items.rs:220 make_local_item 要求 VarSymbol,但上游 Symbol.BindingSymbol extends VarSymbol(本地为独立 SymbolKind::BindingSymbol,无 adr 字段),需 1:1 补齐 BindingSymbol 的 VarSymbol 继承语义(adr/data/flags 访问)并让 Items/Gen 处理;另有 jvm/code.rs:2120 locals 栈帧适配(BindingSymbol 作为局部变量槽)。下一步:补 BindingSymbol 继承适配 → 恢复接线 → e2e 通过 → A0b LambdaToMethod。
- 当前工作树:trans_patterns.rs 未注册(未跟踪保留),mini_compiler/check.rs/mod.rs 已恢复 HEAD;待全量验证。

## 本轮结论与回退

- trans_patterns.rs 已完整编译通过并完成 mini_compiler.desugar 接线验证;官方 instanceof e2e 链路实测:Attr 诊断→TransTypes→TransPatterns(desugar)→Lower 全部通过;剩余阻断在 Gen/Items:jvm/items.rs:220 make_local_item 要求 VarSymbol,而上游 Symbol.BindingSymbol extends VarSymbol(本地为独立 SymbolKind::BindingSymbol,无 adr),需要 1:1 补齐 BindingSymbol 的 VarSymbol 继承语义;以及 code.rs locals 帧(BindingSymbol 局部槽)适配。
- 本轮临时补的 TreeMaker::literal_object 常量类型(TreeMaker.java:899-934)导致 4 个既有 tree_maker 测试断言失败(测试编码旧的不完整行为);按"不得改测试迁就实现"规则,已回退 literal_object 改动,待 Lead 审核测试期望后随 A0.3 提交。
- jvm/code.rs emitop2_type 的 INSTANCEOF 栈效果补丁(Code.java:1068-1071)保留(正确且不影响测试)。
- 当前工作树:trans_patterns.rs 未注册(未跟踪);code.rs 修改保留;其余回退;全量测试预期 1105 绿。

## A0.3 完成:BindingSymbol 继承 1:1 + 注册/接线 + 官方 e2e 通过

### 本批实现(全部上游 1:1)
1. Symbol.BindingSymbol 继承 VarSymbol(Symbol.java:1861-1902):
   - `SymbolKind::BindingSymbol` 增加 VarSymbol payload(lazy_const_value/pos/adr/data_kind/data);
   - 统一访问器 `Symbol::local_adr/set_local_adr/var_pos/set_var_pos`(VarSymbol 与 BindingSymbol 共用);
   - `new_binding_symbol` 构造补 payload(构造器语义 flags|HASINIT|MATCH_BINDING,Symbol.java:1874-1876);
   - Attr.visitBindingPattern 创建点(attr.rs:8584)改走 `new_binding_symbol(flags, name, type, scope.owner)` 并设置 `v.pos = tree.pos`(Attr.java:4135-4136);TypeAnnotationsValidator 创建点(attr.rs:4213)flags 补 HASINIT|MATCH_BINDING(Attr.java:5915);
   - Items.make_local_item(220)、Code::var_adr/set_var_adr 使用统一访问器(BindingSymbol 作为 VarSymbol 可读 adr)。
2. 注册/接线:comp/mod.rs 注册 trans_patterns;mini_compiler.desugar 在 trans_types 后插入 TransPatterns.translate_top_level_class,并写 CompileStates::TRANSTYPES(transTypes 后)/TRANSPATTERNS(TransPatterns 后),再 lower(JavaCompiler.java:1516-1560)。
3. Gen/Code 根因修复(§17,均对照上游):
   - Code.emitStackMapFrame:未定义局部槽 locals[i] 允许 null,width(null)=1(Code.java:1335-1340, width Code.java:284-286);
   - Code.resolve(chain,target):断言与状态更新按上游引用语义(Code.java:1530-1535;newState 在 changed 时必为新对象);
   - StackMapTableFrame Append locals 允许 None(ClassWriter.java:1401-1408, local_diff[j]=locals[i] 可为 null);
   - Code.emitop2_type INSTANCEOF 栈效果(Code.java:1068-1071,上一批已提交)。
4. trans_patterns.rs 质量:删除未使用的 default_visit_* 5 个(binding/parenthesized/guard/block/var_def,特殊方法已直接实现上游逻辑)、unused import/variable、collapsible_if 2 处、binding_declared 去掉仅递归参数、make_hoisted_var_decl 直接返回、to_loadable_constant 增加 wrong_self_convention allow(上游命名保持);clippy 对 trans_patterns.rs 零警告。

### 官方 e2e(本轮实跑,真实断言)
- `instanceof_pattern_no_subtype_is_error`:compiler.err.instanceof.pattern.no.subtype ✓
- `instanceof_reifiable_not_safe_is_error`:compiler.err.instanceof.reifiable.not.safe ✓
- `instanceof_pattern_legal_compiles`:无 compiler.err ✓
- 命令:cargo test -p javac-core --lib comp::check::diagnostics_tests::instanceof → 3 passed;0 failed。

### 全量验证(本轮实跑)
- cargo test --workspace --all-targets:1108 passed / 0 failed / 3 ignored;
- cargo clippy --workspace --all-targets:errors=0(警告 211 为既有基线;trans_patterns.rs 无警告);
- unimplemented!/unreachable! = 0。

### literal_object 审批材料(未改测试,等 Lead 批准)
- 实现依据:TreeMaker.Literal(Object)(TreeMaker.java:899-934)按 value 设置常量类型(Boolean→booleanType.constType(1) 等);本地 TreeMaker::literal_object 当前未设置 type(已登记 NOT PORTED 注释)。
- 临时补齐后 4 个既有测试失败(测试构造的 TreeMaker 未注入 syms):
  1) tree::tree_maker::tests::annotations_and_modifiers
  2) tree::tree_maker::tests::method_body_expression_positions_literals_and_lambda
  3) tree::tree_maker::tests::pretty_prints_maker_built_expressions_and_statements
  4) tree::tree_maker::tests::top_level_and_class_members_keep_pos_and_shape
  失败输出(临时补丁版):4 个测试均 panic at tree_maker.rs:2336(`self.syms.as_ref().expect("TreeMaker.syms")`),原因:测试用 `TreeMaker::new(names)` 构造,未安装 syms(上游 TreeMaker 总持有 syms)。
- 建议(不改断言强度):测试 setup 给 TreeMaker 注入 syms(字段 `pub syms` 可直接赋 `Some`),然后按上游核对 literal literal 的 type 期望;生产路径当前由 TransPatterns::make_boolean_literal 在调用点设置布尔常量类型(等价上游 TreeMaker.Literal(Object),已登记适配)。

## A0b LambdaToMethod 移植:进度记录(压缩保护)

### 已完成的准备工作(本轮实跑)
- 上游 LambdaToMethod.java(2468 行)全文已重读并记录:字段/构造(96-180)、DedupedLambda/KlassInfo(182-254)、translate/translateTopLevelClass(256-289)、visitClassDef(298-327)、visitLambda(335-447)、apportionTypeAnnotations/makeThis(449-478)、visitReference(485-526)、visitIdent/Select/NewClass/VarDef(528-615)、makeLambdaBody/ExpressionBody/StatementBody(618-697)、makeDeserializeMethod/makeNewClass/addDeserializationCase/eqTest/deserTest/deserGetter(699-836)、makePrivateSyntheticMethod/makeSyntheticVar/setVarargsIfNeeded/convertArgs(838-880)、MemberReferenceToLambda(884-1089)、typeToMethodType/makeMetafactoryIndyCall/makeIndyCall(1091-1213)、LambdaAnalyzerPreprocessor(1215-2353,含 Frame/TranslationContext/LambdaTranslationContext/ReferenceTranslationContext/LambdaSymbolKind)、typeSig/classSig/L2MSignatureGenerator(2379-2467)。
- 接线条件确认:JavaCompiler.java:1541-1547 —— `Feature.LAMBDA.allowedInSource(source) && scanner.hasLambdas` 时 LambdaToMethod.translateTopLevelClass,写入 CompileState.UNLAMBDA。
- 依赖盘点:本地已有 Names(deserializeLambda/clinit/init/_this/_super/lambda/dollarThis/altMetafactory/metafactory)、Symtab(lambda_metafactory/serializable_type/illegal_argument_exception_type/serialized_lambda_type/method_handle_lookup_type/method_type_type/array_class/no_symbol 等)、Types(make_functional_interface_class/functional_interface_bridges/find_descriptor_symbol/create_method_type_with_parameters/as_super/direct_supertypes/boxed_class/elemtype/constant_type)、TransTypes(coerce/translate_args/translate_args_with_env)、Lower(FreeVarCollector 私有,需要公开适配器)、TreeScanner(struct+TreeVisitor 模式)、TreeInfo(name/symbol/types/set_symbol/is_null)。
- 已补基础 API(本轮,编译通过):
  - SymbolKind::MethodSymbol 增加 `original_enclosing_method: Option<SymbolId>`(None = this;上游 MethodSymbol.originalEnclosingMethod Symbol.java:2295-2298),全部构造点补 None;
  - 新增 `symbol::encl_class(ctx, sym)`(Symbol.java:512-519)与 `symbol::original_enclosing_method(ctx, sym)`;
  - `Attr::remove_clinit`(Attr.java:3499-3503;Attr.clinits 已有)。
- 尚缺/待移植:
  - comp/TreeHasher.java(114 行)与 comp/TreeDiffer.java(658 行)未移植(DedupedLambda dedup 依赖);
  - Lower.BasicFreeVarCollector 需公开适配(captureLocalClassDefs 依赖);
  - LambdaToMethod 主文件全部;
  - JCLambda bodyKind 辅助(上游 getBodyKind)、JCMemberReference 字段已确认(mode/kind/name/expr/typeargs/sym/varargs_element/owner_accessible/referent_type/target);
  - kInfo.dynMethSyms 的 poolKey 语义:上游 Symbol 基类未在 Symbol.java 中定义 poolKey(仅 VarSymbol.clone/MethodSymbol.clone/MethodHandleSymbol 覆盖),本地实现需以 dynSym 内容键等价(登记适配);
  - e2e class 结构断言(indy/MethodHandle)需要读取生成 class 字节/类文件结构(已有 classfile 解析?待查)。

## A0b 进度快照(压缩保护)

- 已完成并提交上游依赖:comp/tree_hasher.rs(897 行,TreeHasher.java:46-113)、comp/tree_differ.rs(~1350 行,TreeDiffer.java:104-658)已注册编译通过;Symbol::MethodSymbol.original_enclosing_method 字段 + symbol::encl_class/original_enclosing_method + Attr::remove_clinit;Lower 重构出 pub trait BasicFreeVarCollectorVisitor + pub fn collect_free_vars(Lower.java:228-272);TreeMaker::literal_object 常量类型(TreeMaker.java:899-934)+ 4 个 tree_maker 测试 harness 注入 syms(断言未变;tree_maker 6 passed)。
- comp/lambda_to_method.rs 正在写入(已写 part1~373 行:字段/常量/enums/structs/new/translate 系列/translateTopLevelClass;part2~386 行:visitClassDef/lambda/reference/ident/select/newClass/varDef 主 visitor)。尚缺:analyzer(LambdaAnalyzerPreprocessor)、TranslationContext/LambdaTranslationContext/ReferenceTranslationContext 方法、lambda body 辅助、deserialize/indy/metafactory/signature、TreeVisitor impl(复制 TreeTranslator 68 方法+替换 8 特殊)、注册/接线/e2e。
- 本地 API 映射已确认:make.method_def(10 参数)/var_def_symbol/ident_symbol/qual_ident/select/apply/new_class/new_array/switch/case/if_/return_/exec/block/break_/modifiers/type_from_type/let_expr/at;ctx.scope_enter;TreeInfo.name/symbol/types/set_symbol/skip_parens;types.erasure/is_same_type/get_return_type_public/get_thrown_types/boxed_class/find_descriptor_symbol(diags/chk/origin)/functional_interface_bridges(需 Enter)/create_method_type_with_parameters/constant_type/direct_supertypes/as_super/elemtype;transTypes.coerce(需 env)/translate_args_with_env(待 pub);rs.resolve_internal_method/qualified_method/constructor;symbol::as_method_handle/new_dynamic_method_symbol/new_method_symbol/encl_class;SignatureGenerator trait(append_char/append_bytes/append_name/report_illegal_signature/assemble_sig/assemble_class_sig)可移植 L2MSignatureGenerator。
- 下一步:补 trans_types translate_args_with_env 为 pub;写 analyzer/contexts/辅助;复制 TreeVisitor 导航;注册;接线(JavaCompiler.java:1541-1547,条件 Feature.LAMBDA.allowedInSource && scanner.hasLambdas);e2e(lambda 表达式/方法引用 + indy/MethodHandle 结构断言);全量验证;§25 报告。

## A0b 进度快照 v2(压缩保护)

- 已提交/已就绪依赖(工作树编译绿):comp/tree_hasher.rs、comp/tree_differ.rs(注释按 §35 精简:类级锚点 + 上游原注释,无行号、无 none 模板)、Symbol.MethodSymbol.original_enclosing_method + symbol::encl_class/original_enclosing_method + Attr::remove_clinit、Lower BasicFreeVarCollectorVisitor + collect_free_vars、TreeMaker::literal_object(TreeMaker.java 常量类型)+ 4 个 tree_maker 测试 harness 注入 syms(tree_maker 6 passed)、trans_types translate_args/translate_args_with_env 改 pub。
- comp/lambda_to_method.rs 进行中(未注册,~2600 行,处于构建中;当前含部分已写实现与待补项)。已完成段落:字段/常量/enums/structs/new/translate 系列;主 visitor(visitClassDef/visitLambda/visitReference/visitIdent/visitSelect/visitNewClass/visitVarDef);analyzer 结构与方法(visitApply/visitBlock/visitClassDef/visitIdent/visitLambda/analyzeLambda/visitMethodDef/visitNewClass/captureLocalClassDefs 静态化/currentlyInClass/visitReference/visitSelect/visitVarDef/owner/initSym/directlyEnclosingLambda/inClassWithinLambda/capturedDecl/context/currentClass/filters/符号捕获/new_lambda_context/complete/names/disambiguation/序列化判断/translate ident/select/newclass/target 字段);主辅助(make_private_synthetic_method/make_synthetic_var/new_var_symbol/type_to_method_type/is_protected/find_descriptor_checked/type_sig/class_sig/make_metafactory_indy_call/make_indy_call/set_varargs_if_needed/convert_args/apportion_type_annotations/TypeAttrKind)。
- 仍缺:deserialize 系列(make_deserialize_method/make_new_class/add_deserialization_case/eq_test/deser_test/deser_getter)、dedup_lambda、L2MSignatureGenerator、MemberReferenceToLambda、main pass context_is_serializable 包装、make_lambda_method_decl 最终版、LambdaBodyTranslator 完整导航、三份 TreeVisitor 导航 impl(主 pass/analyzer/LambdaBodyTranslator,脚本从 tree_translator.rs 复制+替换特殊)、注册/接线(JavaCompiler.java:1541-1547 条件 Feature.LAMBDA.allowedInSource && scanner.hasLambdas;CompileStates::UNLAMBDA)、e2e(lambda 表达式/方法引用 + indy/MethodHandle 结构断言)、全量验证、§25 报告(含注释规范执行/清理清单)。
- 未注册原因:构建中文件不得进入 git 提交/编译;工作树保持全绿。下一步从本快照继续补上述缺项,注册→编译修错→接线→e2e→全量→报告。

## A0b 进度快照 v3(压缩保护)

- lambda_to_method.rs 已写到 ~4400 行,含:头/字段/枚举/结构/构造/translate 系列;主 pass 特殊 visitor(class/lambda/reference/ident/select/newClass/varDef)与全部辅助(make_private_synthetic_method/make_synthetic_var/new_var_symbol/type_to_method_type/convert_args/set_varargs_if_needed/apportion_type_annotations/TypeAttrKind/make_metafactory_indy_call/make_indy_call/make_deserialize_method/make_new_class/add_deserialization_case/eq_test/deser_test/deser_getter/class_sig/type_sig/dedup 相关);analyzer 全部方法(含 new_lambda_context/complete/names/disambiguation/序列化判断/translate ident/select/newclass/captureLocalClassDefs 静态化/owner/initSym/capturedDecl/filters);L2MSignatureGenerator;三份导航 impl 中的两份(主 pass/analyzer)已由 TreeTranslator 复制生成;free helpers(is_field_symbol/captured_symbols/context_symbol/target_tsym_is_intersection/reference_kind_of/java_string_hash_code/synthetic_method_name_component_ctx/new_var_symbol_static)。
- 当前编译状态:已注销(编译错误待修);剩余错误类别:ctx.attribute(...).type_compound_on_lambda() 需 Attribute helper;frame_class 占位需改为 ctx-based;Symbol.packge(ctx) 参数;ctx.scope_get_symbols_by_name 签名;Types.as_super 参数;find_descriptor_symbol 的 chk 借用冲突;L2MSignatureGenerator trait 方法返回 Result 需处理;make_lambda_method_decl 最终版;LambdaBodyTranslator 完整;MemberReferenceToLambda;dedup_lambda;主 pass context_is_serializable 包装;member_reference 转换;makeNewClass 的 attr_env_placeholder 清理;deser_getter 参数;若干 Option/借用错误。
- 下一步(压缩后):注销状态→逐批修复编译错误(每修一类 cargo check)→补齐上述缺失→注册→接线(JavaCompiler.java:1541-1547 条件;CompileStates::UNLAMBDA;MiniCompiler 字段)→e2e(lambda/方法引用 + indy/MethodHandle 结构断言)→全量+clippy+unimplemented→§25 报告(含注释规范执行/清理清单)。
- 工作树保持全绿(lambda_to_method 未注册;其它依赖改动已编译通过)。
