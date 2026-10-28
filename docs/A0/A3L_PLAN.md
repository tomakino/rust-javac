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
