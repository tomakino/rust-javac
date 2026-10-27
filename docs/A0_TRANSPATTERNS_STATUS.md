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
