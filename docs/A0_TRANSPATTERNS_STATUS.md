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
