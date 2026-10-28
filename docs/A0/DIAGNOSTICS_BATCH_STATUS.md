# 诊断接线批次状态(D0-D9,防压缩保护)

维护:util-porter;提交:Lead。最后更新:本轮(D1 返工完成,工作树全绿)。

## D0 审计 v3(完成,Lead 已提交)
- 产物:`docs/AUDIT_DIAGNOSTICS_TABLE.md` 第三版(935 工厂全量五类状态 + 清单)。
- 统计:已接线 639;待接线 67;未移植 40;范围外 24(processing,PLAN §3.1);无上游触发 165;合计 935。
- D 批次映射:D1 Attr.multicatch;D2 Attr.try-with-resources;D3 Flow guard/inner.cls;D4 Resolve 8;D5 ClassReader 5;D6 Locations 8;D7 其余 42。

## D1 多 catch + 返工(完成,工作树全绿,待 Lead 审核提交)
### 功能实现(1:1)
- `comp/attr.rs`:visit_type_union(Attr.visitTypeUnion 全部控制流)+ Tag::TYPEUNION dispatch。
- `code/type_.rs`:Type::new_union_class_type(Type.UnionClassType 构造器 1:1)。
- `comp/check.rs`:e2e multicatch_disjoint_types_compile、multicatch_related_types_report_disjoint_error;新增单元断言 supertype_completion_registers_class_in_real_symtab(supertype 触发 completion 后真实 Symtab 可见 completed Exception/IOException)。
- `jvm/codegen.rs`:TryFinalizer.has_finalizer 按上游 tree.finalizer!=null 修复 + after_body(BODY_ONLY_FINALIZE)。
### B 变体返工(Lead 审核:clone Symtab 适配无效,已按要求重做)
- 根因:上游 Symbol.complete 不需要 Symtab 可变引用;本地把完成建模为 (&mut Symtab) 才是偏差。
- `code/symtab.rs`:可变表 SymtabMaps/class_fields 改为 Rc<RefCell<...>> 内部可变(不 clone);Symtab/SymtabMaps 删除 Clone derive;写方法 enter_class_flat/enter_class/lookup_package*/enter_package/enter_module/define_class/get_class_field/remove_class 等改 &self 短借;读方法 get_class/get_package/get_module 等短借后释放。
- 完成链签名还原上游语义:`Completer::complete(ctx,syms:&Symtab,sym)`;ClassFinder(14)/ClassReader(55)/ModuleFinder(7)/Symbol(8) 等 &mut Symtab→&Symtab;CompilerContext::complete_symbol/complete_class_symbol 接受 &Symtab;全项目类型级 `&mut Symtab` 归零(799→0);Completer 各实现(Flag/ClassFinder/Deferred/TypeEnter/Main/Source/Unnamed/UsesProvides)同步。
- `code/types.rs`:Types.supertype 在读取 superclass 前调用 ctx.complete_class_symbol(syms, tsym)(删除 clone 适配);completion 写入真实 Symtab maps。
- 变更面:30 文件,2434 insertions/2184 deletions。
### 验证(本轮实际执行)
- cargo check -p javac-core → 0 errors
- cargo test --workspace --all-targets → 1114 passed / 0 failed / 无 already borrowed panic
- e2e:multicatch 2 ok;supertype_completion 1 ok
- clippy --workspace --all-targets → clippy_errors=0,warnings=207(低于 D1 前 214 基线;本批 30 文件 60 条,均为既有风格类/测试目标差异,已登记)
- unimplemented!/unreachable! 全项目 0
- JDK17 oracle 重跑:合法 multi-catch exit=0;非法报 compiler.err.multicatch.types.must.be.disjoint(参数顺序一致)。
### 过程事故申报
中途被外部停止后恢复工作树时,一次基于 clippy 建议的批量自动应用因字符串/字节偏移错误与 --all-targets 的 lib/test 目标差异,损坏了 type_/attr/check/symtab/symbol/type_annotations 若干文件与 module_finder 的 test 依赖 import;已通过 git HEAD 原文 + 确定性重放(含 D1 代码块重建)恢复,未使用 git checkout/reset,未回退他人改动;误建两个空文件(code/modules.rs、code/resolve.rs)已删除。最终全绿。

## D2 try-with-resources(完成,工作树全绿,待 Lead 审核提交)
### 实现(1:1)
- comp/member_enter.rs:initEnv 对齐上游(新环境的 tree=变量声明树、next=外层环境、outer=外层 outer;原实现用 dup_with_outer(env.tree,…,Some(env)) 造成 tree/outer/next 偏差,改用 Env::dupto 语义);visitVarDef 的 lazy-const 分支补 initEnv.info.enclVar=v(上游记录被初始化变量);对应注释按 §35 重写(无 Java 签名/行号)。
- comp/attr.rs:checkAssignable 的 FINAL 分支按 v.isResourceVariable() 分流(TWR 资源→Errors.TryResourceMayNotBeAssigned;普通 final→CantAssignValToFinalVar)。
- jvm/codegen.rs:genTry 的 afterBody 调用前先结束环境 finalize 临时借用(本地 RefCell 适配;上游字段直写无借用域),修复 TWR body-only finalizer 的 already-borrowed panic。
- comp/check.rs:3 个 e2e(try_with_resources_compiles、try_with_multiple_resources_compiles、try_resource_assignment_reports_error)。
### 验证(本轮实际执行)
- cargo check → 0 errors;cargo test --workspace --all-targets → 1117 passed/0 failed;e2e 3 passed;clippy errors=0(warnings 207 与 D1 基线一致,无新增);unimplemented/unreachable 0。
- JDK17 oracle 重跑:合法 TWR exit=0;try (Res2 r = ...) { r = ...; } → compiler.err.try.resource.may.not.be.assigned: r(与本地一致)。
- git:4 文件(attr/check/member_enter/codegen),122 insertions/12 deletions。
### 剩余/D3
- D3:Flow guard 诊断(compiler.misc.guard、compiler.misc.inner.cls 接线)。

## D3 guard 诊断接线(部分完成,工作树全绿;inner-class e2e 阻塞待裁决)
### 实现(1:1 接线 + 依赖修复)
- comp/flow.rs:report_effectively_final_error 改为真实 fragment 参数(Lambda/Guard/InnerCls;default 按上游 AssertionError);Flow.error 保留嵌套 JCDiagnostic 参数对象(ADR 0002);check_effectively_final 的 CLASSDEF 分支补 allow_effectively_final_in_inner_classes 的 fallthrough(上游 2915-2927);is_pattern_tree 只认 BINDINGPATTERN/PARENTHESIZEDPATTERN/GUARDPATTERN(上游 JCDefaultCaseLabel.isPattern=false,JCTree.java:2323)。
- comp/attr.rs:check_method 用 catch_inference_exception 包裹 rs.check_method,将 InferenceException 转为 MethodCheckFailure::Inference(上游 Attr.checkMethod catch,Attr.java:4866)。
- main/mini_compiler.rs:ParserFactory 注入 Preview(上游 ParserFactory.preview=Preview.instance,ParserFactory.java:66/81);完整实现 shouldStop:策略解析(should-stop.at 作为 ifError 的 backwards-compatible 拼写、should-stop.ifError、should-stop.ifNoError 默认 GENERATE;JavaCompiler.java:440-448)、按错误选择策略、严格 isAfter(JavaCompiler.java:557-561)、unrecoverableError 读 deferred 诊断(1251-1262)、阶段边界 PARSE/ENTER/ENTER(enterTrees)/PROCESS/ATTR 门控(930)/FLOW(两处:1368-1369 与 1383-1384,flow.analyze_tree + compileStates.put 之后)/TRANSTYPES/TRANSPATTERNS/UNLAMBDA/LOWER/GENERATE(997-998、1148、1500、1518、1535、1542、1549、1592)。
- util/log.rs:新增 deferred_diagnostics() 为 **adapter**:上游 DeferredDiagnosticHandler.getDiagnostics() 返回 queue 引用,本地因借用模型返回快照(Vec clone),仅用于 unrecoverableError 判据读取,不消费队列(不影响 reportDeferredDiagnostics 语义)。
- comp/check.rs:guard e2e(断言 compiler.err.cant.ref.non.effectively.final.var 的嵌套 fragment 键 compiler.misc.guard)+ lambda e2e(compiler.misc.lambda);删除临时 probe。guard e2e 用 should-stop.ifNoError=FLOW(deferred handler 下 errorCount=0,无错误策略生效),在 Flow 后停止,不进入 TransPatterns/Gen。
- 测试:mini_compiler 单测 4 个(should-stop.at 仅影响 ifError、ifError/ifNoError 按错误选择、默认 INIT/GENERATE、非法状态 panic);check.rs e2e 3 个(ifNoError=ATTR/FLOW 跳过生成、默认 GENERATE 生成类);guard/lambda fragment e2e 2 个。
- comp/member_enter.rs:删除既有调试 eprintln(不改变行为)。
### 未完成(阻塞,需 Lead 裁决)
- inner_class_effectively_final_uses_inner_cls_fragment:local class 完成链在 Attr 借用期间触发 TypeEnter→Attr 重入(RefCell already borrowed:type_enter.rs:1458 attrib_type_variables / member_enter.rs:212)。已尝试给 TypeEnter 注入当前 Attr(complete_with_attr)并推进到 member_enter 的 attr 借用,说明需整链注入(TypeEnter+MemberEnter+Annotate)或 Attr 借用模型重构;为不提交半成品,已撤回 type_enter.rs 改造(恢复 HEAD)并移除该未通过 e2e。上游语义(Flow fragment inner.cls)已在代码层实现,缺的是 local class 编译链的借用适配。请 Lead 裁决后续批次范围。
### 验证(本轮实际执行)
- cargo check 0 errors;cargo test --workspace --all-targets → 1119 passed / 0 failed;clippy errors=0(warnings 208);unimplemented/unreachable=0。
- JDK17 oracle:guard → compiler.err.cant.ref.non.effectively.final.var: x, (compiler.misc.guard);inner class → (compiler.misc.inner.cls);lambda → (compiler.misc.lambda)。guard/lambda 本地 e2e 与之一致。
- git:5 文件(flow/attr/check/mini_compiler/member_enter),166 insertions/34 deletions。

## D4 Resolve verbose resolution + fatal diagnostics (完成)
### 实现
- comp/resolve.rs:VerboseResolutionMode(debug.verboseResolution 解析:all/逗号分隔/-mode);report_verbose_resolution_diagnostic(SUCCESS/FAILURE、Object init、predef、internal 过滤;candidates 的 applicable/inapplicable 子诊断;verbose.resolve.multi(.1) note + MultilineDiagnostic);get_verbose_applicable/inapplicable_candidate_diag;dump_method_reference_search_results(debug.dumpMethodReferenceSearchResults,bound/unbound、applicable.method.found.2/.3 + partial.inst.sig/static/non_static/bound/unbound);InapplicableSymbolsError 多候选诊断(MostSpecificMap/filterCandidates/candidateDetails 的 InapplicableMethod 片段,COMPRESSED flag)。
- comp/resolve.rs:fatal 3 条 — resolve_internal_field 新增;resolve_internal_method/constructorfatal 改为 Fragments.FatalErrCantLocate* + FatalError(JCDiagnostic) 构造;util/fatal_error.rs 增加 from_diagnostic/fatal_error_diagnostic。
- 依赖修复(测试触发):Attr.visit_apply 的 ERROR 类型返回类型(types.get_return_type_public 增加 ErrorType 分支);ArgumentAttr 的 Log 参数改为 Rc<RefCell<Log>>(避免 attribution 期间长期借用 Log);find_fun_in_import_scope/diamond 的 Scope 查询改用 get_symbols_by_name_full(FilterImportScope 要求)。
### 验证
- cargo check 0;cargo test --workspace --all-targets:1132 passed/0 failed;clippy errors=0(warnings 208,与 D3 基线一致);unimplemented/unreachable=0。
- 真实触发 e2e:verbose success/failure note + applicable/inapplicable 子诊断 + partial.inst.sig;方法引用 bound dump(non.static/bound fragments);多候选 cant.apply.symbols + inapplicable.method 子诊断;fatal 单元(消息 "Fatal Error: Unable to find method f")。
- fatal 触发路径登记:resolve_internal_field/constructor 本地无调用者(不可从源码触发);resolve_internal_method 仅编译器合成路径(失败不可从源码构造)→ 单元构造验证 FatalError 消息 + 代码路径审计。
### 注释执行清单(§35)
- 新增/修改文件:7 个(resolve.rs、symbol.rs、fatal_error.rs、types.rs、argument_attr.rs、attr.rs、check.rs)+ docs 2 个。
- 上游 javadoc 原文保留:3 处(FatalError 构造器、resolveInternalField 的英文 javadoc;其余新增单元以英文职责说明 + Port note)。
- 模板噪音清理:删除 D4 新增/触及处的 `Java: <signature>`/文件行号锚点 16 处(enum/report/dump/detail/fatal/field/return-type/access/getDiagnostic doc);新增 Port note/Port adapter 11 处;测试内调试输出全部移除(0 eprintln)。
- PORTING_MAP 新增条目:12 条(D4 entries)。

### D4 参数占位偏差登记(逐条;diagnostic_contract 仅 placeholder,后续 P4)
- `site`(verbose note 参数): 用 Printer.visit_type/class 文本,上游传 TypeSymbol 对象;显示文本与真实 oracle 一致(qualified)。
- `inst`(partial.inst.sig 参数): 用类型显示文本,上游传实例化 Type 对象。
- `methodArguments`(verbose/错误诊断参数): 用现有 method_arguments_display 文本("no arguments"/类型逗号分隔),上游为 Type/树对象的 LocalizedString/列表。
- 候选 `sym`(not.applicable.method.found/inapplicable.method): SymbolArg placeholder 只含名字(如 "f"),上游 Symbol 显示带参数签名("f(int)");本地 asMemberOf placeholder 用成员签名文本(name(params))替代 Types.memberType(后者在 ERROR 类型上 panic,既有缺口)。
- `kindName`: 用 log.localize_key(kindname.*) 的本地化文本,上游传 KindName 对象。
- 以上均要求 diagnostic_contract 支持真 Symbol/Type/KindName 参数对象(ADR 0002 P4);已列入后续修复项,当前不伪造对象。

### D4 显示层(Printer.visit(Type)/visitMethodSymbol)
- 新增 `code/printer.rs`(1:1 移植 code/Printer.java 相关 visit 方法):visitClassType(qualified longform、enclosing `T.In`/`T<G>.NonStatic`、type args `<...>` join(",")、compound/anonymous)、visitMethodType、visitWildcardType、visitArrayType(`[]`)、visitCapturedType(本地化 captureof、稳定 arena id)、visitForAll、visitUndetVar、visitErrorType/visitTypeVar/visitModuleType/visitType(tsym.name/type.none)、visitMethodSymbol(构造器/初始化器用 owner 名、`<tvars>name(args)`、varargs `...`)。
- `resolve.rs::type_display_str` → `printer::visit_type`;`method_symbol_display` → `printer::visit_symbol`(传 names);删除 earlier simple 文本实现(type_.rs simple helpers 已移除)。
- 非 verbose 单候选错误 kindName 已本地化(`kindname.method`→method、`kindname.class`→class),与多候选一致。
- 真实 oracle 命令:`javac -J-Duser.language=en -XDrawDiagnostics -XDdebug.verboseResolution=all -XDshould-stop.ifNoError=FLOW -d <tmp> T.java`(JDK17 17.0.19);原始输出类型文本:java.lang.Object、java.util.List<java.lang.String>、java.util.List<? extends java.lang.Number>、java.util.Map<java.lang.String,java.lang.String>、java.lang.String[]、T.In、T<G>.NonStatic、f(java.lang.String...)、f(int)。测试 `diagnostic_type_display_covers_all_type_forms` 断言这些文本(verbose with actuals + 候选;非 verbose 首行 + 候选行)。
- 普通模式(无 -XDrawDiagnostics)与 raw 模式的参数显示不同;按 Lead 指定以 raw/Printer 规则为准(qualified)。
- 依赖/后续:diagnostic_contract 仍只有 placeholder(Symbol/Type/KindName 对象无);Printer 映射登记 PORTING_MAP;typed-arg(P4)后续。captured 实参显示(capture#id of ...)出现在本地 wildcard 实参上(oracle raw 该 actuals 显示通配符文本),属既有 attr 捕获差异,登记观察项;Deferred/Proxy 文本保留本地实现并登记。
- 残留显示适配(登记,后续 P4/类型注解批次):Printer.printAnnotations 尚未移植,类型参数上的类型注解前缀当前不显示;visitClassSymbol/visitPackageSymbol 等符号级 printer 未实现(当前诊断符号参数仍走 SymbolArg 文本/placeholder,不调用它们)。
- 显示层调用点:`resolve.rs` type_display_str(inapplicable 错误 found args/owner type)→printer::visit_type;method_arguments_display(错误 required/found、verbose actuals)→printer 类型文本 join(",")(formatIterable 一致);method_symbol_display(verbose 候选/InapplicableMethod 详情)→printer::visit_symbol。

### D5 ClassReader 警告/错误通道(2 wired / 3 dependency-blocked)
- wired 2: `annotation.method.not.found`(find_access_method;lintClassfile 条件;source=requestingOwner.classfile(C.class);args=TypeArg 注解类型文本+Name)、`unknown.enum.constant`(find_enum_constant;无 lint 条件;source 无,与 oracle raw "-" 一致;args=FileObjectArg 枚举类文件(E.class,Port adapter:上游 currentClassFile 在枚举完成时为枚举类文件)+SymbolArg 枚举名+Name)。
- blocked 3(依赖 CompletionFailure 通道,后续批次):`annotation.method.not.found.reason`、`unknown.enum.constant.reason`(上游需成员加载 CompletionFailure,ClassReader.java:1945-1957/2028-2040;本地 completion 经 Result 返回、成员查询不产生 CompletionFailure)、`cant.attach.type.annotations`(上游默认 addTypeAnnotationsToSymbol=false,ClassReader.java:296;本地 catch/error 通道已实现,参数构造为 Port adapter:AnnotationArg 文本+SymbolArg placeholder 替代上游 attributes 列表+s.owner Symbol+s.name+detailValue,待 completion-failure 批次改真对象)。
- lintClassfile:Lint 服务未移植(TODO(Lint));`Options::lint_enabled` 解析 -Xlint/-Xlint:all/-Xlint:none/逗号列表(Port adapter);测试用 -Xlint:all 触发(不写死 Xlint:classfile)。
- 真实 fixture 用例 2 条(真实 javac class 字节+字节补丁):A_nomethod.class+C.class(annotation.method.not.found;source C.class,args A/value);E.class(enter)+C2_bad.class(unknown.enum.constant;source 无,args E.class/E/Z)。deproxy 由 pending 注解值手动驱动(构造 Annotate 需 17 依赖,登记)。
- oracle 原始命令/输出(JDK17 17.0.19):javac -J-Duser.language=en -XDrawDiagnostics -Xlint:classfile -cp <dir> -d <dir> Use.java;case1 raw "C.class:-:-: compiler.warn.annotation.method.not.found: A, value"(普通 "... C.class: warning: Cannot find annotation method 'value()' in type 'A'");case2 raw "- compiler.warn.unknown.enum.constant: E.class, E, Z"(普通 "warning: unknown enum constant E.Z")。

### D3b inner-class 借用专项(方案 A:整链 &mut Attr 注入)
- Attr 的 local-class 完成改为 `complete_local_class(ctx,syms,sym)`:调用 `TypeEnter::complete_with_attr(..., self)` 并保留上游失败恢复;不再在 Attr 借用内 `complete_class_symbol` 重借。
- TypeEnter:`complete_with_attr` + phase(imports/hierarchy/header/record/members)与 helper(attrib_super_types/attrib_import_*/attrib_annotation_types/attrib_type_variables/add_accessor/add_enum/add_record/finish_class)接收 `&mut Attr`;正常入口 `complete` 包装 borrow 一次(self.attr.borrow_mut 仅 1 处)。
- MemberEnter:member_enter/member_enter_list/visit_method_def/visit_var_def/signature/attrib_* 接收 `&mut Attr`;删除 attr_service(重借 0 处)。
- Annotate:AnnotateRunnable/flush/unblock 增加 `&mut Attr`;队列闭包、TypeAnnotations.validate、AnnotateServices(attribute_annotation*/get_annotation_*/annotation_value_info/process_repeated/make_container)显式传 attr;TypeAnnotate 持 `&mut Attr`;正常路径 wrapper borrow 一次。
- 审计(真实计数,二审修正):type_enter=3——complete 包装 `attr_rc.borrow_mut()`(正常入口,Attr 内走 complete_with_attr)、ensure_imports_checked `attr_rc.borrow_mut()`(正常 Enter 入口,不在 complete_with_attr 链)、setter `self.attr.borrow_mut() = Some(attr)`(wiring 注入,非 attrib 路径);member_enter=0;annotate=2——flush 包装(正常入口,Attr 内走 flush_with_attr)、AnnotationTypeCompleter 的 `self.services.attr.borrow_mut()`(ClassReader 注解类型 completer 正常路径)。TypeEnter/MemberEnter 的 Attr 内路径均不二次借用;finish_imports 已改为接收 `&mut Attr`(complete_with_attr 链传现有 attr,ensure_imports_checked 正常入口 borrow 一次)。
- 测试:局部类(inner_class_effectively_final_uses_inner_cls_fragment)通过——诊断 code `compiler.err.cant.ref.non.effectively.final.var`、文本与 JDK17 oracle 一致、含 `compiler.misc.inner.cls` fragment;匿名类(anonymous_class_compiles)通过(T+T$1,2 个编译类)。
- 阻塞(失败停手,未提交半成品):成员内部类实例化(`class T { class In {} In i = new In(); }`)panic 于既有 `Resolve::resolve_self_containing` 的 "enclClass sym"(`env.enclClass` 的 tree.sym 为 None,backtrace resolve.rs:9503);该测试已移除并登记,待 Resolve/Enter 批次修复;D3b 的 Attr/Annotate 链由局部类/匿名类 e2e 验证。
- 验证:1138 passed / clippy 0/208 / unimplemented 0;git 7 files 329 insertions(+),196 deletions(-)(未提交,待 Lead 审核)。

#### D3b 二审修复
- complete 包装不再吞错:`pub fn complete` 返回 `Result<(), CompletionFailure>`(上游 throws 语义),`TypeEnterCompleter::complete` 直接传播;新增 `complete_wrapper_propagates_completion_failure`(mock Completer 返回 Err → `complete_class_symbol` 返回 Err 且恢复为 error type,诊断 supplier 未被使用)。
- finish_imports 改为接收 `&mut Attr`(`complete_with_attr` 链传现有 attr;`ensure_imports_checked` 正常入口 borrow 一次),Attrs 内路径无二次借用。
- 成员内部类 panic 归属:在 HEAD bc5d915(D3b stash 后)复现 `class T { class In { int v; } In i = new In(); }` 仍 panic 于 `resolve.rs:9499` `resolve_self_containing` 的 "enclClass sym" → 既有缺口,D3b 未引入;登记 D3c(Resolve/Enter 专项),测试不入库。
- 验证:1139 passed(含新增完成失败用例) / clippy 0/208 / unimplemented 0。

### D3c 成员内部类 resolve/生成缺口(修复)
- 根因:`Resolve::resolve_self_containing`/`resolve_self` 用 `tree_info::symbol`(上游 TreeInfo.symbol,仅处理 Ident/FieldAccess/TypeApply/AnnotatedType/MemberReference)读取 `env.enclClass` 的 ClassDecl 符号 → 恒为 None → panic "enclClass sym";上游读 `env.enclClass.sym`(TreeInfo.symbolFor 对 CLASSDEF 有效)。修复:4 处改用 `tree_info::symbol_for`(上游 TreeInfo.symbolFor),无 dummy sym/无绕过。
- 次生:Codegen 遍历 class defs 时对嵌套类(ClassDecl)触发 `panic!("unexpected class member")`;上游 `Gen.normalizeDefs`(JDK17 Gen.java:421-476)只接受 BLOCK/METHODDEF/VARDEF 并对其它 tag `Assert.error()`,故嵌套类不会进入 genClass 的 defs 循环(Gen.java:2419-2421;每个嵌套类由独立 genClass 生成)→ 本地仅忽略 `ClassDecl`,其余 tag 仍 panic(不掩盖未实现分支)。
- 测试:member_inner_class_field_initializer_compiles(`class T { class In { int v; } In i = new In(); }`)、member_inner_class_method_new_compiles(`In make() { return new In(); }`)、outer_inner_qualification_compiles(`class Outer {...} class Use { Outer.Inner f; }`)全部通过;JDK17 javac oracle(T1/T2/Outer.java)`javac_exit=0`;D3b 的 local/anonymous e2e 保留。
- 二审修复:撤销 `check_non_abstract` 的 `AbstractCantBeAccessedDirectly` 第一个 SymbolArg 误改(D3b 调试时全局替换误伤,已恢复 `ctx.sym(sym).name`);codegen 证据化(见上,仅忽略 ClassDecl,其余仍 panic)。
- 验证:workspace 1142 passed / 0 failed(gson 临时测试已由 Lead 移出);clippy errors=0/warnings=208;unimplemented 0;debug grep 0。
