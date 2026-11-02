# rust-javac 移植偏差登记（PORTING_NOTES）

本文件登记 rust-javac 与官方 javac（jdk17u @ 982d53c，jdk-17.0.19-ga）的已知行为偏差。

格式：每条一个编号（PN-xxx），包含：现象、官方行为、本地行为、影响、状态、计划阶段、验证方式。
源码中只保留编号引用（见 docs/STYLE.md），详细内容集中在这里。

## PN-001 字段/参数的声明位置与 LineNumberTable 双来源

- 现象：3 个诊断用例的 caret 列偏左（DeprecatedAnnotationHasNoEffect、IllegalComboModifiers、VarNotIntializedInDefaultConstructor）。
- 官方行为：JavacParser.variableDeclarators / variableDeclaratorId 取 token.pos（名字 token）；诊断位置来自 JCVariableDecl.pos（名字）；但字段初始化器在 LineNumberTable 的行号来自声明起点（带注解字段时是注解行，如 Guava CacheBuilder 的 295 行）。
- 本地行为：parser 的字段/参数路径使用 mods.pos（声明起点），导致诊断 caret 指向修饰符；若改为名字，则 Guava 10 个类的字段初始化器行号差 1。
- 影响：诊断语料 3 个 fail；或 Guava 10 类字节不一致（二者只能取一）。
- 状态：已定位根因，已回滚改动（保持 Guava 位对位）。待配套修 Lower/Gen 的字段初始化器 statBegin 来源为 TreeInfo.getStartPos。
- 计划：P1/P2（先用 getStartPos 修行号，再改 parser 位置）。
- 验证：check_examples --filter <3 个用例>；check_guava。

## PN-002 dupTo 的 clone 标志

- 现象：InferenceContext.dupTo 生成的副本与官方在 clone 标志/状态上存在差异（历史审计发现）。
- 官方行为：dupTo 复制推理上下文并保持 clone 语义。
- 本地行为：副本的部分标志未完全对齐。
- 影响：暂未在现有语料中观察到字节差异；属于潜在风险。
- 状态：已登记，未修复。
- 计划：P3（错误模型/推断清理时一并审计）。
- 验证：lib 单元测试 + Guava/Gson 位对位。

## PN-003 模块系统诊断缺口（10 个语料用例）

- 现象：check_examples 失败 10 个：HasBeenDeprecatedModule、HasBeenDeprecatedRemovalModule、MultiModuleOutputCannotBeExplodedModule、NotDefAccessDoesNotReadFromUnnamed、NotDefAccessDoesNotReadUnnamed、NotDefAccessNotExportedFromUnnamed、NotDefAccessNotExportedToModuleFromUnnamed、PackageClashFromRequiresInUnnamed、PackageEmptyOrNotFoundWarning、WarnModuleNotFound。
- 官方行为：模块解析/可见性/lint/多模块输出校验的完整诊断集合。
- 本地行为：模块系统部分实现，缺少上述诊断路径。
- 影响：诊断语料 10 个 fail；模块相关项目行为不完整。
- 状态：已登记，未修复。
- 计划：功能补齐（独立于重构阶段；建议在 P3/P4 后单独立项）。
- 验证：check_examples --filter <模块用例>。

## PN-004 生成阶段 IllegalSignature 错误缺失

- 现象：check_examples 的 IllegalSignature 用例失败：缺 error while generating class ... (illegal signature attribute for type CAP#1)。
- 官方行为：Signature 属性生成失败时由 Gen/ClassWriter 报错并结束。
- 本地行为：未检测/未上报该错误路径。
- 影响：诊断语料 1 个 fail；异常类型的 class 生成行为不一致。
- 状态：已登记，未修复。
- 计划：P3（错误通道）之后补生成错误路径。
- 验证：check_examples --filter IllegalSignature。

## PN-005 注解处理、插件、FileManager、JVM 栈帧不支持

- 现象：check_examples 有 67 个 skip；javac-tools/javac-source 中存在 unimplemented 的模拟 API。
- 官方行为：支持 -processor、-Xplugin、自定义 JavaFileManager、-doe 等。
- 本地行为：未实现（模拟层返回 UnsupportedOperationException 或跳过）。
- 影响：带注解处理器的项目不可用；工具 API 不完整。
- 状态：明确不支持（登记）。
- 计划：不在 P0–P5 范围内；未来单独立项。
- 验证：无（skip 覆盖）。

## PN-006 Rc<RefCell> 适配层

- 现象：javac-core 中 Rc<RefCell< 530 处、borrow_mut() 2130 处。
- 原因：1:1 移植 Java 的共享可变对象图（Log/Warner/Scope/推断上下文等）。
- 影响：借用重入 panic 风险；可读性与并发扩展性差。
- 状态：技术债（适配层）。
- 计划：P2 按热点分批收敛（lambda_to_method → codegen → argument_attr → check → attr）。
- 验证：每批 L1/L2；BorrowMutError 零容忍。

## PN-007 panic 模拟异常通道

- 现象：InapplicableMethodException / InferenceException 用 panic + catch_unwind 实现控制流；expect() 3008 处。
- 官方行为：Java 异常（throw/catch）。
- 本地行为：panic payload + 线程局部栈。
- 影响：用户可触发路径可能 panic；错误传播不可读。
- 状态：技术债。
- 计划：P3（先 resolve/infer，再 attr/check）；expect() 分类清理。
- 验证：负例输入不 panic；examples fail 不增加。

## PN-008 Windows launcher 行过滤

- 现象：javac launcher 在内部错误（exit 4）时打印 printing javac parameters to: <cwd>\javac.<timestamp>.args，内容不可重复。
- 处理：expected_gen.py 的 normalize() 过滤该行；expected 基线已清理。
- 状态：已处理。
- 验证：check_examples --filter DuplicateModule。

## PN-009 Guava warnings 数差异

- 现象：javac 41 条警告 vs rust 53 条（Guava 编译时）。
- 原因：诊断位对位未完成（部分警告重复/位置不同）。
- 影响：stderr 不完全一致；不影响 class 字节。
- 状态：已登记，未修复。
- 计划：诊断位对位专项（与 PN-001 相关）。
- 验证：check_guava 的 warnings 比较。

## PN-010 NOT PORTED 方法清单

- 现象：239 行 NOT PORTED 标记，分布在 29 个文件；343 处 unimplemented/todo（多在 javac-tools/javac-source 模拟层）。
- 影响：边界场景可能缺功能或 panic；主编译路径已覆盖。
- 状态：登记在 docs/PORTING_MAP.md（每文件 details）与源码注释。
- 计划：随对应阶段逐步清零；不承诺一次性全部移植。
- 验证：对应语料/测试。

## 模板（新增偏差时复制）

## PN-xxx 标题

- 现象：
- 官方行为：
- 本地行为：
- 影响：
- 状态：
- 计划：
- 验证：
