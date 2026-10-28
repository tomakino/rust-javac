# A3l：Scope/导入作用域独立对齐批次（v2，裁决 B）

Status: 回退 open addressing（2026-10-28 18:19）
Baseline: HEAD 301edb8 + target/a3l-base.patch（139777 bytes）
Chaining archive: target/a3l-chaining.patch（235880 bytes，废弃，仅存档）
Executor: 87815a06-70c5-4824-b711-76dbdfcc06a1
Date: 2026-10-28

## 0. 裁决记录（重要）

- reference/jdk17u@982d53c 的 Scope.java **是 open addressing**：
  - Scope.java:542 明确写 “We use open addressing with double hashing”；
  - getIndex 是探测式；enter 用 table[hash] + sentinel；dble 只搬 slot head。
- 原计划写的 “separate chaining / 只搬槽头 / masked Name.getIndex” 是 Lead 的依据错误，不是上游事实。
- 执行者按错误计划实现了 chaining，并做了“dble 重散列整链 / 按名过滤”等自创适配；这会让 @Deprecated 回归（ABSENT_TYP），也违反“不得自创偏离”。
- **裁决 B**：回退 chaining，恢复 982d53c 的 open addressing；保留与表结构无关的真实修复；gson 68 归 A3k，不作为本批清零目标。

## 1. 目标与完成定义

**目标**：Scope 表、Scope 子类按名查找、TypeEnter imports、Resolve.find_type 按 982d53c 1:1 对齐；行为等价 + 基本映射。

**本批完成定义**：

1. Scope 表是 **open addressing**：TableSlot 保留 Empty/Deleted/Entry，get_index 走 probe_index/sentinel 语义，dble 只搬 slot head；不保留 chaining。
2. cargo test --workspace --all-targets 全绿（834/826），clippy errors=0，unimplemented/unreachable/todo=0。
3. @Deprecated、ElementType、static import、star import、package scope 测试通过。
4. 保留本批与表无关的真实修复：resolveSelf、handleSwitch 顺序、annotate/check metadata 转发、lookupPackage complete、lower.accessBase，以及 e2e 5 个测试。
5. gson 结果只记录（当前 68 errors / 0 classes，分类归 A3k），不作为本批清零条件；gson 0/207 属于 A3k 类型系统。
6. 无临时 probe、无机器路径测试；未 commit，Lead 审核。

## 2. 当前证据

- gson 的 leave 失败来自 Scope 生命周期/调用配对，不是“open addressing 与 dble 不兼容”；之前把表改成 chaining 是错误方向。
- chaining 实验（废弃）：gson 68 errors，但 @Deprecated 回归；不能作为实现依据。
- 官方调用链（docs/A0/OFFICIAL_CALL_TRACE.md）：
  - TypeEnter.markDeprecated → Attr.attribAnnotationTypes → attribType → attribTree；
  - ClassReader.readClass → Scope$ScopeImpl.leave（classfile 临时 scope）。
- 官方 imports：resolve_imports 先 importAll(java.lang)，再处理 imports；ensure_imports_checked 只对未 filled star 调 finish_imports。

## 3. 实施步骤

### 步骤 1：回退 Scope 表到 982d53c open addressing
- scope.rs：恢复 HEAD 的 TableSlot/Empty/Deleted/Entry、probe_index、get_index、enter/leave/dble/remove/lookup 语义；保留 Scope::owner()（Enter/MemberEnter 使用）。
- compiler_context.rs：scope_leave 恢复 probe_index。
- 删除 chaining 专用实现：重散列整链、next_shadowed_matching 按名过滤、TableSlot 仅 Empty/Entry、简单 hash。
- 删除/改写 8 个 chaining 专用测试；新增 open addressing 语义测试（探测、Deleted、dble 只搬 slot head、dup/dupUnshared/leave）。

### 步骤 2：保留真实修复
- resolve.rs resolveSelf（class tree.sym）、lookup_package 显式 complete；
- attr.rs handle_switch addVars 顺序；
- check.rs/annotate.rs annotation metadata 转发；
- lower.rs access_base static 符号；
- e2e 5 个导入/override/qualified-this 测试。
这些修复必须有上游对应位置或 Port adapter 注释。

### 步骤 3：Scope 子类查找按 open addressing 语义
- ScopeImpl lookup + Entry.next(sf) + NON_RECURSIVE 边界；
- shadowed 是同名链；**不新增按名过滤/next_shadowed_matching**；
- Named/Import/Star/Compound/SingleEntry/Filter 逐分支按 Scope.java 820/858/955/1089。

### 步骤 4：TypeEnter imports / Resolve.find_type
- 保持当前实现，但逐项核对 TypeEnter.java 335-386 / Resolve.java 2329-2400；
- 不因 gson 诊断数加特判。

### 步骤 5：测试与验收
- 834/826 全量；clippy；e2e；@Deprecated/static/star/package。
- gson opt-in 仅记录 errors/classes，报告是否 leave panic；不要求 0/207。
- 官方 oracle：docs/A0/OFFICIAL_CALL_TRACE.md；javap 207 classes 仅作参考。

### 步骤 6：审核与提交
- Lead 读 scope.rs/compiler_context.rs 的 diff，确认没有 chaining 残留；
- 清理 probe；更新 PORTING_MAP/STATUS；工作树干净后提交。

## 4. 禁止

- 不得把 TableSlot 改成 chaining，不得删除 probe_index，不得用简单 hash 替代探测。
- 不得为了绕开 @Deprecated/导入问题加按名过滤、重散列、特判。
- 不得用 gson 诊断数/classes 作为本批完成条件。
- 不得把 “能跑通” 当成 open addressing 1:1；必须以 Scope.java 源码为准。

## 5. 验收口径

行为等价（官方 oracle 差分）+ 基本映射（核心类/主流程可定位、Origin/Port note）；文件数/方法数只作导航，不作为完成门槛。

## 6. 进展（2026-10-28 18:55，Lead 本地复跑）

- **leave 墓碑修复**：`compiler_context.scope_leave` 删除链尾由 `Empty` 改为 `Deleted`（对应上游 `elems.shadowed` 的 sentinel）；gson 的 `leave(): entry not found` panic 消失。
- **as_super 接口完成适配**（Port adapter）：classfile stub 的 `INTERFACE` flag 未完成，`Types::as_super` 读取目标接口 flags 前先 `complete_class_symbol`；`EnumDesc`/`ConstantDesc` 的 5 条 override 诊断消失。
- `is_overrider` 前增加 `complete_for_members`（保留），但未单独解决剩余 override。
- 当前真实结果：`cargo test --workspace --all-targets` **828 passed / 0 failed**；clippy errors=0；gson **80 files / 63 errors / 0 classes**（无 panic）。
- 剩余 63 条分类：31 `method.does.not.override.superclass`、14 `override.incompatible.ret`、11 `prob.found.req`、7 `cant.resolve.location.args`、5 `cant.apply.symbol`、3 `cant.apply.symbols`、2 `cant.resolve.args`、2 `report.access`。
- 下一步线索：`is_overrider` 对接口继承方法仍返回 false（`List.addAll(int,Collection)`、`Collection.removeAll`、`TypeAdapterFactory.create` 等）；`Symbol.overrides` 的 `member_type`/`is_sub_signature` 对接口泛型替换不正确，应优先对照上游 `MethodSymbol.overrides`/`Types.memberType`。
- 临时 `[GSON-DIAG]`/`[OVERRIDE*]`/`[OVERRIDES-MISMATCH]` 打印已全部删除（grep=0）。

## 7. A3k 进展（2026-10-28 19:40，执行者）

在 §6 基础上继续（Lead 的 leave 墓碑/as_super 完成/is_overrider complete_for_members 均保留未回退），本批关闭以下根因：

1. **ForAll.getTypeArguments**（`code/type_.rs`、`code/types.rs`）：上游 `ForAll.getTypeArguments()` 返回 tvars；本地两个实现都只处理 class 类型，导致 `Types.resultSubtype` 无法按位置重命名多态方法的类型变量，接口泛型方法（`TypeAdapterFactory.create`、`List.addAll`、`Collection.removeAll` 等）的 override 判定失败。补上 ForAll 分支后 31 条 `method.does.not.override` 与 14 条 `override.incompatible.ret` 全部消失。
2. **class 类型 outer 的 noType 编码**（`code/types.rs::subst_visit`、`code/symtab.rs::new_class_type`、`jvm/class_reader.rs::class_sig_to_type`）：上游 `ClassType.outer` 恒为 `Type.noType` 单例；本地存在 None 与 Some(noType) 两种编码，subst 重建后编码丢失，`isSameType` 的 enclosing 比较把同一类型判为不同（`LinkedTreeMap.entrySet`、`NonNullElementWrapperList.toArray` 等 override 失败）。修复：Symtab 初始类型保持 Some(noType)、classfile 顶层参数化类显式带 noType outer、Subst 保留源类型的编码。未采用「比较时把 None 与 Some(noType) 归一化」，因为 A3h 回归测试明确要求二者不同。
3. **Resolve.notOverriddenIn 完整语义**（`comp/resolve.rs`）：上游条件为 `s2 == null || s2 == sym || sym.owner == s2.owner || !isSubSignature(memberType(site,s2), memberType(site,sym))`；本地只保留了 `s2 == sym`，把「没有实现」的抽象接口成员误判为「已被覆盖」而跳过候选，产生 `Deque.isEmpty`、`Map.get`、交集类型 `Member.getModifiers` 等 5 条 cannot find symbol。补齐后消失。
4. **TransTypes.addBridge 的元数据回写**（`comp/trans_types.rs`）：`Symbol::clone` 按 Port adapter 清空 arena id，clone 后写回已分配的 bridge 槽位时 `setAttributes` 的 initedMetadata 取不到 id 而 panic（`AssertionError: symbol is not allocated`）。写回前恢复 bridge 的 arena id。
5. **匿名类调用 protected 超类构造器**（`comp/attr.rs::visit_new_class`）：上游用 `rsEnv`（selectSuper = cdef != null）解析并 check 构造器；本地 resolve_constructor 传了 `local_env`，protected 访问分支看不到 selectSuper，`new Reader(){}`、DateType 匿名子类等 5 条 report.access。改为 resolve_constructor/checkId 都使用 rsEnv。
6. **erasue 未处理方法类型**（`code/types.rs::erasure_visit`）：上游 erasure 的 StructuralTypeMapping 会擦除 MethodType 的参数/返回/throws；本地 `_ => t` 使 raw 构造器成员保留类类型变量（`new ArrayTypeAdapter(...)` 等 4 条 constructor cannot be applied、`DefaultDateTypeAdapter.write` override、`LinkedTreeMap.compareTo` 的 raw Comparable 共 7 条）。补上 MethodType/PartiallyInferredMethodType 分支后消失。

最终本地结果（真实命令输出）：

- `cargo test --workspace --all-targets`：29 个测试套件全部 ok，其中 lib **828 passed / 0 failed**；无 FAILED。
- `cargo clippy --workspace --all-targets`：Finished，errors=0（仅既有 warnings）。
- `unimplemented!/unreachable!/todo!(` 源码计数 0。
- gson opt-in（80 files）：**10 errors / 0 classes**，gson_probe 的 `errors == 0` 断言仍失败（未达成 0/207）。63 → 10 的 53 条消失对应上述 6 个根因。

剩余 10 条是同一个根因（diamond 的目标类型推断在「方法调用实参」上下文未完成）：

- GsonBuilder:762/766/767/768、DefaultDateTypeAdapter:70/74/78/82、TypeToken:242/249（共 10 条，全部 `cannot infer type arguments for kindname.class<>`）。
- 最小复现：`return new CtorTarget(new HashMap<>(m));`（m: Map<String,String>）。
- 证据：内层 `new HashMap<>(m)` 的 speculative 树类型仍是 `HashMap<K,V>`（类类型变量，非实例化 String）；`DiamondCheckContext.compatible` 收到 found=`HashMap<K,V>`、req=`Map<String,String>` 失败，report 后产生 `cant.apply.diamond.1`。
- 已定位到 `Infer.instantiateMethod` 的 `shouldPropagate` 分支（返回 `HashMap<K?,V?>` 后，外层 `Attr.check` 的 inferenceContext 没有通过 `minContext.dupTo` 获得内层变量，`check_after_inference` 的 `free(found)` 为 false，于是立即 compatible 并报错）。下一步需按 Infer.java:194-233 对齐 `shouldPropagate`/`minContext.dupTo` 与外层 free 监听（下一批，非本批可交）。

未提交（工作树保留全部修复）；临时探针 `[RAW]/[FD]/[SB]/[DIA]/[ARG]/[MT]/[OVR]/[NOI]` 打印已全部删除（grep=0），临时测试 `tests/a3k_probe.rs` 已删除。

## 8. A3k 后续：diamond 推断与 codegen（2026-10-28 20:20，执行者）

§7 的 10 条 diamond 错误已在本轮关闭（Attr 阶段 0 errors）。关闭的根因：

1. **diamond 构造器的部分推断类型**（`comp/attr.rs::visit_new_class`）：上游 `PartiallyInferredMethodType.check`（Infer.java:286-357）用每个候选目标重新推断并回滚；本地把 diamond 的 `constructorType` 用 `createMethodTypeWithReturn` 包成普通 MethodType，`ArgumentAttr.overloadCheck` 只做 `localInfo.check(HashMap<K,V>)` 并报 `cant.apply.diamond.1`。修复：`constructorType` 为 partial 时保留 partial 类型写入槽位（不 wrap）。
2. **合成 diamond 构造器的 base symbol**（`comp/resolve.rs::find_diamond`）：上游匿名 MethodSymbol 重写 `baseSymbol()` 返回原构造器；本地合成符号未设 `base_symbol_override`，`tree.constructor` 取到返回 HashMap 的合成符号，codegen `invoke` 误压返回值 → `stack sim error on f`。修复：`base_symbol_override = Some(原构造器)`。
3. **TreeMaker.type_cast 参数顺序**（`tree/tree_maker.rs`）：上游为 `TypeCast(Type, JCExpression)`；本地签名是 `(expr, type)`，调用点按上游顺序传参导致 cast 的 clazz/expr 颠倒（`(float) in.nextDouble()` 的 expr 变成类型树）→ `validateAnnotatedType` 遇 APPLY panic、codegen visitTypeArray panic。修复签名并同步 parser/lambda_to_method/trans_patterns/trans_types 调用点。
4. **Lower 合成 cast 的类型**（`comp/lower.rs::lower_type_cast`）：Lower 合成的 cast 不经过 Attr，`tree.ty` 为空；回退用 cast 目标类型（Port adapter）。

结果：§7 的 10 条 `cannot infer type arguments for kindname.class<>` 在 Attr 阶段全部消失；三个最小复现（plain diamond return / diamond 作为构造器实参 / diamond 局部变量）均 errors=0 并生成 1/2/1 个 class。**但 gson 仍在 CodeGen 阶段 panic**，gson_probe 仍 FAILED，未达到 0/207。本轮继续修掉两个 codegen 前置缺口：

5. **Lower 合成 cast 的类型记录**（`comp/lower.rs::lower_type_cast`）：Lower 合成的 cast 树不经过 Attr，`tree.ty` 为空，codegen 在 `gen_expr` 读不到类型；fallback 的同时把 cast 目标类型写回树（Port adapter）。
6. **Gen.concat 初始化缺失**（`jvm/codegen.rs::Gen::new`）：上游 `concat = StringConcat.instance(context)`（Gen.java:110），本地构造为 `None`，任何字符串拼接在 `gen_binary`/`gen_assignop` 的 `self.concat.take().expect(...)` 处 panic。改为构造时调用 `make_concat`（1:1 上游）。

修完后当前阻塞是 `stack sim error on read`：某个 `read` 方法生成后栈非 0（body pos=19398），说明还有 codegen 栈模拟/字符串拼接或树类型的缺口。剩余阻塞已从 Attr 推断完全转移到 CodeGen，属下一批 codegen 债务（不再属于 Infer）。

同轮完成 Lead 的前置 TODO/债务：

- **Types/Infer 的 Names 缺失消除**：`Types::new()` 改用共享 `DEFAULT_NAMES`，`names` 字段改为非 Option；删除 `names_or_default`、`lub_pair` 的 `expect("TODO ... Names")`、`Infer::solve` 的 lub expect、intersection projection 与 `attribute.retention` 的 names expect（`code/types.rs`、`comp/infer.rs`）。
- **locations 注释修正**：MODULE_PATH/UPGRADE_MODULE_PATH 实际已实现（`module_path_init_locations`/ModulePathIterator，含 exploded/dir-of-modules/jar/jmod），删除文件头「TODO panics」表述（`file/locations.rs`）。
- **doc_comment_parser 空引用**：`reference_required` 在空引用时返回空 DCReference（上游 null reference 的 Port adapter），不再 panic。
- **过期 TODO 清理**：`code/flags.rs` 的 TODO(P3)（is_static/is_enum/is_constant 已实现）、`tree/tree_info.rs` 的 constValue TODO、`util/raw_diagnostic_formatter.rs` 文件头（formatSource 已 port；无 source 为上游 IllegalArgumentException 的 panic，section 6/9）。
- **文档**：`docs/AUDIT_FINDINGS.md` F16/F22 关闭并改写；`docs/KEYWORD_AUDIT.md` 计数更新（TODO 251 / FIXME 2 / HACK 4）；新增 `docs/A0/TODO_CLASSIFICATION.md`（81 条无标签 TODO 的 A/B/C/D 分类与全量附录）。

验证：`cargo test --workspace --all-targets` 全部套件 ok（lib 828 passed / 0 failed）；`cargo clippy --workspace --all-targets` Finished errors=0；`unimplemented!/unreachable!/todo!()` 计数 0；临时 `tests/zz_scratch_a3l.rs` 已删除。未提交。

## 9. A3m 续：CodeGen 移植缺口（2026-10-28 21:10，执行者）

继续 Lead 的 A3m 指令（Types Names 注入 + gson 0/207）。在 §8 基础上又修掉 9 个 CodeGen 移植缺口（每个都有上游对应，无 gson 特判）：

1. **方法 env**（`Gen::gen_method`）：上游 `visitMethodDef` 创建 `localEnv = env.dup(tree); localEnv.enclMethod = tree`；本地直接把类 env 当方法 env，return/break 的 `unwind` 永远到不了方法树（`unwind environment` panic）。
2. **block/loop env 安装**（`gen_block`/`gen_loop`）：`genStats` 读 `Gen.env`，本地创建了 block/loop env 却没安装，循环内 break/continue 的 unwind 找不到 FORLOOP。
3. **try/synchronized env 安装**（`gen_try_stmt`/`gen_synchronized`）：body 必须在 try/sync env 内生成，否则 return 不经过 finally/monitorexit，`stack sim`/`locks active on return`。
4. **Lower 合成 var def 的类型**（`lower_var_def`）：用 `sym.ty` 回退（上游 `visitVarDef` 用符号类型），修复合成声明 `variable type` panic。
5. **StringConcat eager 判定**（`should_convert_to_string_eagerly`）：`unboxedType` 对 primitive 返回 noType，本地把 primitive double 误判为需要 eager String 转换，导致 indy 少弹 2 slot。
6. **常量池 Byte/Short/Char/Boolean**（`pool_writer::put_constant_value`）：JVMS 4.4.4 统一按 CONSTANT_Integer 处理（原先 panic）。
7. 以及 §8 的 cast 参数顺序、Lower 合成 cast 类型、Gen.concat 初始化、return boxing（erasure 返回类型）、find_diamond 的 base symbol。

**当前 gson 结果**：不再 panic，`gson files=80 errors=8 classes=0`。8 条错误是 TransTypes `logAccessErrorInternal` 对 JDK 类符号的误判：`Array`（owner 为 root package、flags=INTERFACE|PUBLIC）与 `AssertionError.<init>`（flags=2 PRIVATE）被判为 inaccessible。需要下一轮定位 ClassReader/ClassFinder 的符号 owner/flags 完成路径；**未达到 0/207**。

验证：`cargo test --workspace --all-targets` 828 passed / 0 failed；`cargo clippy --workspace --all-targets` Finished errors=0；临时探针已全部清理（grep=0）；未提交。

## 10. A3m 续（二）：8 条 access 错误根因（2026-10-28 21:31，执行者）

**8 条 `.Array` 的根因**：`Types::cvar_upper_bound` 只对 captured type 取 bound，普通 TypeVar 直接返回自身；`isAccessible(env, T[])` 展开 elemtype 得到 `T` 后再取 upper bound 仍是 `T`，`T` 的 flags 无访问位 → 判为 inaccessible。已改为上游 `Types.cvarUpperBound` 的循环取 bound；修复后 gson **不再有 `.Array` 诊断**。

**同轮修复**：
- `is_accessible_class_free`：unnamed module 的可见包（Modules.computeVisiblePackages 的 Port adapter；visiblePackages 未完成时按包所属模块的 exports 判定）；arrayClass 伪符号直接可访问（上游数组经元素类型检查）。
- `Gen.visitUnary` PREINC/PREDEC 的 AssignItem 用原始 item（od）而非 load 后的 stack 值。
- Lower 的 switch map 合成 Indexed/literal 补类型（合成树不过 Attr）。
- `Lower.lower_var_def` 的 init 用符号类型（合成 var def 无 tree.ty）。
- `PoolWriter.put_constant_value` 的 byte/short/char/boolean 按 CONSTANT_Integer。

**当前 gson 结果**：8 条 access 错误消失；gson_probe 仍以 STATUS_STACK_OVERFLOW 退出，**未达 0/207 classes**。本轮探针把溢出定位到 **`JsonTreeReader` 的 generate（CodeGen）阶段**：`[STAGE] lower done` 之后溢出，而 `gen_expr` 深度 <500（不深）。已排除：cvarUpperBound 自引用（[CVAR] 未触发，64MB/256MB 栈仍溢出）、is_accessible 的 ARRAY 递归（深度 0/1）、resolve_error 链（[ERRD] 未触发）、接口递归（[BRIDGES] 353 条有限）、`supertype(Object)`（no_type 正常）、`add_bridges`（`[END]` 已打印）。候选：`Gen::gen_class`/`normalize_defs`/`gen_stat` 的树遍历递归（`JsonTreeReader` 的树）；需下一轮在这些入口加深度诊断。**后续轮**：在 `gen_expr`/`gen_stat` 入口加了「当前递归栈上的 TreeId 集合」环检测（同一 TreeId 重入即打印 32 帧栈并 panic），**`[CYCLE]` 未触发**——说明溢出不是 `gen_expr`/`gen_stat` 的树环；溢出仍在 generate 阶段，候选改为 `gen_method`/`gen_class`/`Items`/`Code` 的递归（下一轮在这些入口加环检测/深度）。 **再后续轮**：按 Lead 要求改用「函数调用栈」探针（thread_local Vec<&'static str>，深度 >200 打印最后 100 标签 + 当前 TreeId/pos 并 panic），覆盖 `gen_cond`/`gen_args`/`gen_method`/`gen_new_class`/`gen_assign`/`gen_unary`/`gen_binary`/`gen_indexed`/`gen_class`/`gen_expr`/`gen_stat`/`normalize_defs`/`ClassReferenceVisitor` 的 `ref_scan`/`ref_binary`/`ref_conditional`/`ref_unary`/`ref_cast` 以及 `Items::store`/`Items::invoke`，**`[FNSTACK]` 未触发**（19 个入口深度均 <200）——溢出不在这些函数的递归；候选改为未覆盖的 `gen_type_test`/`gen_instanceof`/`gen_switch`/`gen_foreach`/`gen_try`/`gen_lambda`/`gen_reference` 与 `Items::load`/`coerce`/`duplicate`/`Code::emitop`（下一轮全覆盖）。

验证：828 passed / 0 failed；clippy Finished errors=0；探针 grep=0；未提交。

## 11. A3m 续（三）：gson 0 error / 207 classes 达成（2026-10-28 22:57，执行者）

§10 的 STATUS_STACK_OVERFLOW 已定位并修完，gson 达到 **0 error / 207 classes**。根因链与上游对照：

1. **栈溢出根因**：`Lower::make_empty_class`（enum switch 的 `$SwitchMap` 合成类）调用 `enter_synthetic(ctx, c, members)`，把合成类 c 进入了**它自己的** members scope（`members_field`），形成 `c.members ∋ c` 自引用；`ClassWriter::write_nest_members_if_needed` 的 `list_nested` 据此无限递归。上游 `Lower.enterSynthetic(owner, sym)` 进入的是 **owner 的** members，本轮按上游改为 `ctx.sym(owner).members()`。
2. **enum case 标签无类型**：`EnumMapping::for_constant` 用裸 `make.literal(TypeTag::INT, ...)` 造 case 标签，没有 type；上游 `make.Literal(value)`（Object 重载）会 `setType(intType.constType(value))`。改为 `Lower::make_int_literal`（等同上游 Object 重载）。
3. **char case 标签**：本地 char 常量保留 `ConstantValue::Character` 包装，上游 constValue 已是 Integer；`gen_switch` 接受 Character/Byte/Short 拓宽（Port adapter）。
4. **`break label` 的 unwind**：`gen_labelled` 建了 `dup_env` 却没安装为当前 env，body 在旧 env 下生成，`unwind(LABELLED)` 找不到 labeled env；按上游 `Gen.visitLabelled` 在 body 生成期间安装（与 loop/switch/block 同模式）。
5. **`Lower.unbox` 的合成 cast 无 ty**：上游 `Attr.visitTypeCast` 设 `tree.type = clazz.type`，合成 cast 在 Attr 之后构建，本地补设 `boxed_ty`（Port adapter）。

**验证**（真实输出）：`cargo test --workspace --all-targets` → `828 passed; 0 failed`；`cargo clippy --workspace --all-targets` → `Finished`（errors=0）；gson_probe → `gson files=80 errors=0 classes=207`，`test result: ok`。临时探针全部删除（grep `[ALLOC17141]/[SC30113]/[UNWIND]/[LAB]/[CASE]/[WC]/[LN]/[ES]/[SELFENTER]/[INNER]/FNSTACK/fn_enter/[GEN]/[STAGE]/[PHASE]` = 0）。未提交。



