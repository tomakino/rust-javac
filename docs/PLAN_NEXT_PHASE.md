# rust-javac 下一阶段实施计划（已批准）

日期：2026-10-30；状态：已由用户批准，执行中。

## 一、目标与成功标准

目标：在现有编译核心 + CLI 基础之上，补齐三类缺口：CLI 未接线选项、官方 API 实现层（为 LSP 打基础）、诊断契约完整化；不改变已验收的编译行为。

成功标准（每项都有真实 javac 17 对照）：

1. CLI：-sourcepath / -nowarn / -Werror / -deprecation / -implicit 五项从“解析但未生效”变为“与 javac 行为一致”，并进入 cli_parity 逐项对比；
2. API：外部代码可按官方习惯调用——JavacTaskImpl.parse/analyze/generate、JavacTrees.getTree/getElement/getTypeMirror/getScope/getPath、TreePath.getPath、DocTrees 基本方法；新增 API 测试与 javac 行为对照；
3. 诊断：NameSimplifier 四场景（java.lang 顶层/嵌套/显式 import/冲突）与 javac 逐字一致；诊断即时输出顺序与 javac 一致；
4. 回归：cargo test -p javac-core --lib 834 全绿；cargo test -p rust-javac 全绿；OkHttp 编译 errors=0；OkHttp JUnit 结果不低于 1976/1999（环境敏感类除外）。

## 二、现状（代码核查结论）

- 编译流水线完整：Parse→Enter→TypeEnter→MemberEnter→Attr→Flow→TransTypes→Lower→Gen；
- CLI 已生效：-d/-cp/--module-path/--release/-encoding/-parameters/-verbose/-Xmaxerrs/-Xmaxwarns/--add-*/--patch-module/--module-source-path/-Xlint/-source/-target/-g*/@argfile/-version/-help；
- CLI 解析但未生效：-sourcepath/-nowarn/-Werror/-deprecation/-implicit（Option::X 在 core 无读取）；
- API：javax.tools / com.sun.source 接口齐全，Compiler 实现 Tool/JavaCompiler（含 get_task，java_compiler.rs:226）；缺 JavacTrees/JavacTaskImpl/DocTreesImpl 实现与 TreePath.getPath；
- 诊断：格式/位置/计数/locale/类型简名（java.lang 顶层）已对齐；NameSimplifier（import/嵌套/冲突）与即时输出未做；
- 代码级：unimplemented! = 0、TODO( = 0。

## 三、阶段划分

### 阶段 A：CLI 选项接线

改动（javac-core 的选项读取 + 既有服务接线，不新增架构）：

- -sourcepath：文件管理器设置 SOURCE_PATH（对照 JavacFileManager 的 setLocation/sourcepath 语义）；搜索按需、不得启动全递归（此前卡死教训）；
- -nowarn：Log/警告处理：关闭 mandatory warning（对照 Log.nerrors/nwarnings 与 MandatoryWarningHandler）；
- -Werror：Log：警告升级为错误（对照 Log.werror）；
- -deprecation：Option.DEPRECATION.process：把 LintCategory.DEPRECATION 加入 -Xlint 并启用 verbose 弃用；
- -implicit：sourcepath 搜索路径：按 none/class 决定是否隐式编译发现的源文件。

实测基线（javac 17，已由执行者验证）：
- 默认：隐式编译 sourcepath 源文件，生成 Main.class + Dep.class，EXIT 0；
- -implicit:none：只生成 Main.class（类型仍可解析），EXIT 0；
- -implicit:class：同默认，EXIT 0。

注意：任务中“默认 none 报找不到符号”的假设是错的，以实测为准。

验收：cli_parity 增补对应用例（真实 javac 对比：输出/退出码/产物）；834 与现有 15 项不回归。

### 阶段 A0（前置，优先）：缺失符号错误恢复

当前缺陷：无效代码 `no.Such.v()` 触发 types.rs:7778 panic（no return type tag=CLASS），exit 4；javac 应报 cannot find symbol、exit 1。

修复：Attr/Resolve 对解析失败返回 errType；TransTypes/Gen 对 ErrorType 短路（不 panic）；诊断与 javac 一致。

验收：no.Such.v() 与 javac 逐字对比诊断/退出码；cli_parity 增补；无回归。

### 阶段 B：官方 API 实现层（核心，最大）

改动：

1. javac-core 新增 api 实现模块（或 javac-source 的 impl 子模块）：
   - JavacTrees：getTree(Element)、getElement(TreePath)、getTypeMirror(TreePath)、getScope(TreePath)、getPath(Element)/getPath(CompilationUnitTree, Tree)、getDocCommentTree、getSourcePositions；
   - JavacTaskImpl：parse()/analyze()/generate() 接到现有流水线阶段；getElements()/getTypes()；addTaskListener/removeTaskListener；
   - JavacTool（或复用 Compiler）：getTask(...) 返回 JavacTaskImpl；
   - TreePath.getPath 静态搜索（TreePathScanner）；
2. 数据流：Element↔Symbol、Tree↔JCTree 的映射表（现有 arena 索引可直接用）；
3. 边界：getTree 对合成树返回 None；getPath 对非本编译单元返回 None；getTypeMirror 对未 attr 树返回 None（与上游一致）。

验收：新增 tests/api_surface.rs（官方调用方式：parse→analyze→generate、Trees.getTree/getElement/getTypeMirror、TreePath 遍历、TaskListener 事件）；与 javac 同源对比 AST/类型映射；OkHttp 编译不回归。

### 阶段 C：诊断契约完整化（NameSimplifier + 即时输出）

1. DiagnosticArg 的 TypeArg/SymbolArg 携带 SymbolId/TypeId（不再提前字符串化）；
2. formatter 在 format 前按诊断收集 usage（ClassNameSimplifier.addUsage），再 simplify（RichDiagnosticFormatter.java:285-331/384-395）；
3. CLI 诊断改为即时输出（Log 默认 handler），不再统一 deferred；保持计数与顺序；
4. 验收：四场景（java.lang 顶层/嵌套/import/冲突）与 javac 逐字一致；cli_parity 增补；834 回归（更新期望需附 javac 基线）。

### 阶段 D：性能与清理

1. ClassFinder 包发现惰性化 + 目录索引缓存（对照上游 JavacFileManager.list/DirectoryIndex；目标：仓库根默认 -cp . 编译 <1s，javac 基线 468ms）；
2. 清理调试 eprintln 与过时 adapter 注释；
3. （可后置）javac-floating 的 BigDecimal 相关、javac-charset 剩余名称、ct.sym 的 ct.properties。

验收：性能用例（javac vs rust-javac 同目录同输入计时）；清理后 834 + cli 全绿。

## 四、顺序与依赖

A0（错误恢复，优先）→ A（CLI 选项接线）→ B（API 层）→ C（诊断契约）→ D（性能/清理）。

每阶段完成后：S0 cargo check → 定向测试 → S4 834 → 相关 cli/repro → 阶段汇报。

## 五、风险与回退

- 错误恢复：可能触及 Attr/Resolve 的多个错误分支；以 no.Such.v() 最小复现驱动，逐分支修；
- API 层：Element/Tree 映射可能与现有 model 层（javac-tools 的 Element 实现）重复；先复用；
- 诊断契约：字符串化改动面大，可能导致 834 诊断期望变化——每步附 javac 基线更新；
- 即时输出：可能影响 cli_parity 顺序断言——先加测试再改；
- 性能：惰性索引不能改默认 classpath 语义（曾犯过），验收必须含“子目录源 + 当前目录依赖类”；
- 所有阶段遵守 10 条约束与零信任审核；2 轮无进展停下给方案。

## 六、假设

1. 以 reference/jdk17u（982d53c）为唯一行为基准，不使用 docs/*.md 作为依据（已过期）；
2. CLI 只显示实际实现的选项；未实现项使用时报错，不假装支持；
3. 无法实现项（APT/ServiceLoader/反射工厂/launcher）保持“注入替代 + 登记”，不在本计划内；
4. 不改变已验收的编译/运行行为；任何诊断文本变化必须有 javac 基线对照。