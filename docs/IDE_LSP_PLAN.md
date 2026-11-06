# Ravac LSP 项目计划

状态：2026-11-06
范围：crates/javac-ide-core、crates/ravac-lsp、editors/vscode/ravac-lsp
目标：把 ravac-lsp 从“能演示的 Java LSP”推进到“能在真实 Java 项目里日常使用”。

核心索引架构重构计划： [IDE_INDEX_PLAN.md](IDE_INDEX_PLAN.md)

## 1. 目标与非目标

### 目标

1. 真实 Maven 项目离线打开后，传递依赖中的类型能补全、能跳转、不报“找不到符号”。
2. 引用和重命名是符号级的，不误改注释、字符串、同名局部变量。
3. 诊断能定位到范围，常见错误能一键修复。
4. 补全覆盖继承成员和方法链，达到日常写代码可用的程度。
5. 协议层能取消长请求、增量同步、响应工作区变化。

### 非目标

- 不做 Java 编译器代码生成；只做分析。
- 不替代 Maven/Gradle 构建、测试、调试。
- 不追求完整 IntelliJ 重构集合；只做高频且可验证的功能。
- 不引入网络依赖；Maven 解析只读本地 ~/.m2。

## 2. 当前状态

已经实现并通过测试：

- 补全：IDEA MinusculeMatcher 移植、项目符号、自动 import additionalTextEdits、Javadoc Markdown。
- 导航：definition、typeDefinition、implementation、references、documentSymbol、hover。
- 编辑：signatureHelp、inlayHint、rename、codeAction（仅 auto-import）、全文格式化。
- 层级：typeHierarchy、callHierarchy。
- 诊断：push diagnostics，单点位置，无范围。
- Maven：从 initialize 的 rootUri 发现 pom.xml；解析 parent/BOM/dependencyManagement，加入直接依赖、传递依赖 jar 和 source roots。
- 客户端：VS Code 扩展改为只使用 vscode-languageclient 内置特性，补全用 middleware 保留服务器顺序。

已知边界：

- Maven 解析是离线的，只读本地 ~/.m2；不支持 Gradle、profiles、注解处理/Lombok、生成源码；test_source_roots 已解析但未加入分析；dependencyManagement 的 scope 尚未应用。
- references/rename 已使用 SymbolId；同文件精确，跨文件通过 source_path 项目级编译后收集。项目级 AnalysisSession 已缓存，按源文件 mtime/length 与当前 overlay hash 失效；pom/classpath 变化仍需重启语言服务器。
- 诊断已带 UTF-16 range（编译器错误定位到 token）；未使用 import 提供删除 quick fix，auto-import 保留；relatedInformation 尚未产生。
- 补全已覆盖继承成员、方法链和泛型链（I1/I2/I3/I4/I5、R11、R14、R15）；override 之外的 postfix/generate 仍在 P2。
- 不支持 Gradle、Lombok、注解处理、生成源码；test_source_roots 未加入分析。
- 没有 workspace/symbol、documentHighlight、semanticTokens、codeLens、documentLink、foldingRange、selectionRange。
- 协议声明全文同步；没有 $/cancelRequest、进度、partial result、workspace/configuration、didChangeWatchedFiles、pull diagnostics。
- 单线程同步循环；长请求会阻塞后续请求。
- 格式化依赖外部 google-java-format jar；没有 range/on-type formatting。
- classpath jar 无源码时 definition 返回 null；JDK 跳转依赖 JAVA_HOME/lib/src.zip。

## 3. 优先级

P0 决定真实项目是否可用，必须先做。
P1 决定日常体验是否顺手。
P2 是补齐和优化，不阻塞 P0/P1。

## 4. P0

### P0-1 Maven 依赖解析补全

问题：当前只把 pom.xml 的直接依赖 jar 加入 classpath。真实项目大量类型来自传递依赖、parent POM 的 dependencyManagement、BOM，当前会大面积报未解析。

改动：

- crates/javac-ide-core/src/project/maven.rs
  - 递归读取 parent POM，合并 properties。
  - 解析 dependencyManagement、import scope 的 BOM。
  - 解析传递依赖，处理 exclusions、optional、scope。
  - 版本冲突采用最近优先；同层冲突按声明顺序。
  - 读取 sourceDirectory/testSourceDirectory，替代固定 src/main/java。
  - 只读本地 ~/.m2；缺失 artifact 记录警告，不访问网络。
- test_source_roots 的独立 classpath/source path 留到 P1-3；当前不混入主 source path。
- initialize 的 Maven 发现结果缓存，didChangeWatchedFiles 时失效。

验收：

- 新增真实 Maven fixture，至少包含 parent POM、BOM、两个传递依赖、一个 exclusion。
- 离线环境打开 fixture：传递依赖类型可补全、可跳转，诊断无“找不到符号”。
- 现有 maven 测试和 completion_spec 不回归。

风险：POM 语义范围大；先覆盖 Maven 常用子集，未支持的字段写入文档，不静默猜。

### P0-2 符号级引用与重命名

问题：references 和 rename 目前基于文本边界匹配，会命中注释、字符串、同名局部变量，跨文件会漏改。

改动：

- crates/javac-ide-core/src/analysis/index.rs
  - 索引从“名字 -> 位置”升级为“符号身份 -> 声明/引用位置”。
  - 保留 flat name，但增加 owner、kind、参数签名，避免同名冲突。
- crates/javac-ide-core/src/analysis/features/navigation/mod.rs
  - references 使用编译器 attribution 结果，只返回同一 SymbolId 的引用。
  - 支持跨文件；索引未命中时回退到现有文本搜索，并在结果里标记非精确。
- crates/javac-ide-core/src/analysis/features/actions/mod.rs
  - rename 复用符号级 references。
  - 增加 prepareRename：不可重命名时返回明确错误。
  - 重命名类型时同步 import。
- crates/ravac-lsp/src/lib.rs
  - 增加 textDocument/prepareRename handler。

验收：

- fixture 同时包含：同名字段、局部变量、方法、注释、字符串、另一个类的同名字段。
- references 只返回目标符号；rename 只改目标符号，跨文件不漏。
- 注释和字符串不被修改。
- prepareRename 对包名、关键字、不可重命名位置返回拒绝。

风险：编译器符号到源码 occurrence 的映射需要覆盖泛型、lambda、方法引用、继承成员；先用 fixture 锁定行为。

### P0-3 诊断与代码动作闭环

问题：诊断只有单点位置，codeAction 只有 auto-import；编辑器无法定位错误范围，也无法一键修常见问题。

改动：

- crates/javac-ide-core/src/analysis/diagnostics.rs
  - Diagnostic 增加 end position、code、source、relatedInformation。
  - 保持现有单点诊断兼容，逐步迁移。
- crates/javac-ide-core/src/analysis/features/actions/mod.rs
  - 未使用 import：提供删除 action。
  - 缺失 import：复用 auto-import 逻辑，处理同名冲突，让用户选择。
  - 诊断位置对应的 quick fix 挂到 codeAction 的 diagnostics 字段。
- crates/ravac-lsp/src/lib.rs
  - codeAction 返回 WorkspaceEdit，支持 resolve（可选）。
  - 诊断 publish 时带 range。

验收：

- Problems 面板显示范围，不是单点。
- 未使用 import 可一键删除。
- 同名类 import 冲突时给出选择，而不是随便选一个。
- 修复后诊断立即消失，不需要手动保存。

### P0-4 补全语义深度

问题：补全只有当前作用域和项目符号；继承成员、方法链返回类型不完整。

改动：

- crates/javac-ide-core/src/analysis/features/completion/
  - 继承成员：沿 ClassSymbol 的 supertypes 展开方法和字段，按可见性过滤。
  - 方法链：解析调用表达式的返回类型，再给成员；支持泛型替换。
  - override：在子类中提示可重写方法。
  - postfix/generate 放在 P2，不阻塞。
- 补全排序保持现有 IDEA matcher + start-match + degree 规则。

验收：

- 子类中能补出父类 public/protected 方法和字段。
- app.service.build().name(...) 这类链式调用能补出返回类型成员。
- 泛型返回类型不丢类型参数。
- completion_spec 新增对应用例。

## 5. P1

### P1-1 协议健壮性

- textDocumentSync 改为增量同步，处理 range change。
- 处理 $/cancelRequest：至少取消排队中的请求；长分析不阻塞后续轻量请求。
- 增加 $/progress：Maven 扫描、项目索引可见进度。
- 支持 workspace/configuration、workspace/didChangeWatchedFiles：pom 变化后重建 classpath。
- 评估 pull diagnostics（textDocument/diagnostic）；保留 push 兼容。
- 单线程循环改为请求队列 + 可中断分析，或明确记录阻塞边界。

验收：

- 取消补全后服务器不再返回该请求。
- pom.xml 修改后 classpath 自动更新。
- 大项目扫描时编辑器仍能响应 hover/definition。

### P1-2 编辑器语义

- workspace/symbol：基于索引的全局符号搜索。
- documentHighlight：光标下符号的所有 occurrence。
- semanticTokens：类、方法、字段、参数、局部变量；与 TextMate 语法共存。
- foldingRange、selectionRange：先实现基础版；VS Code 内置 fallback 保留。
- codeLens、documentLink：P2，按需求再做。

验收：

- Ctrl+T 能搜到项目类和方法。
- 光标放到符号上，同文件同名引用高亮准确。
- 语义着色不改变现有主题可读性。

### P1-3 项目类型与生成源码

- Gradle：解析 build.gradle 的 sourceSets、dependencies；先做只读本地缓存。
- Lombok：至少识别 @Data/@Getter/@Setter 生成的访问器；不修改编译器。
- 注解处理：读取 META-INF/services，生成源码目录加入 source path。
- JPMS：module-info 的 requires/exports 参与解析。
- test source roots 完整加入测试 classpath。

验收：对应 fixture 能补全/跳转，不误报。

## 6. P2

- range formatting、on-type formatting；格式化 jar 缺失时明确提示。
- inlayHint resolve、tooltip；参数名/var 类型之外的提示。
- signatureHelp 的 overload 文档和 activeParameter 精确化。
- jar 源码附件、反编译回退。
- codeLens（引用计数）、documentLink。
- completionItem/resolve、codeAction/resolve。
- 性能：大项目索引持久化、增量重建。

## 7. 里程碑

### M1：真实 Maven 可用

- P0-1 完成。
- 真实 fixture 通过；离线；无“找不到符号”。
- 更新 docs/IDE_LSP.md 的 Maven 能力和限制。

### M2：编辑安全

- P0-2 完成。
- 符号级 references/rename 通过同名/注释/字符串/跨文件 fixture。
- prepareRename 接入 LSP。

### M3：诊断闭环与补全深度

- P0-3、P0-4 完成。
- Problems 可一键修；继承成员和方法链补全通过。

### M4：协议与编辑器体验

- P1-1、P1-2 完成。
- 取消、增量同步、workspace/symbol、documentHighlight、semanticTokens 有测试。

### M5：项目类型

- P1-3 按目标项目推进；Gradle/Lombok 至少各有一个 fixture。

## 8. 测试与门禁

每个 P0 项必须同时有：

- 单元测试：crates/javac-ide-core/tests/ 或模块内测试。
- LSP 端到端：scripts/lsp/run_lsp_tests.py 新增检查。
- 真实项目 fixture：internal/test 下可离线运行。
- 文档同步：docs/IDE_LSP.md 的能力表和 Known limitations。

最低门禁：

    cargo test -p javac-ide-core
    cargo test -p javac-ide-core --test completion_spec
    cargo test -p javac-core --lib
    py scripts/lsp/run_lsp_tests.py

不通过不得标记完成。未验证项在文档中明确写“未验证”。

## 9. 暂不做

- 完整 IntelliJ 重构（extract method/variable、inline、move class）。
- Java 代码生成、构建、运行、调试。
- 远程/多语言服务器编排。
- 网络下载 Maven artifact。
- 语义 tokens 的全量语法着色替代 TextMate。

## 10. 跟踪表

| ID | 项目 | 优先级 | 状态 | 验收信号 |
|---|---|---|---|---|
| P0-1 | Maven 传递依赖/parent/BOM | P0 | 完成（核心；test roots 在 P1-3） | 6 tests + 本地 JUnit/Hamcrest 传输依赖探针 |
| P0-2 | 符号级 references/rename | P0 | 完成（同文件 + 跨文件项目级 + prepareRename + 项目缓存） | 同名/注释/字符串/跨文件 fixture 通过 |
| P0-3 | 诊断范围 + quick fix | P0 | 完成（range + unused import/auto-import；relatedInformation 待补） | 2 个 diagnostics tests 通过 |
| P0-4 | 继承成员/方法链补全 | P0 | 完成 | completion_spec 130/130 通过 |
| P1-1 | 增量同步/取消/进度/配置 | P1 | 未开始 | 取消与 pom 变更测试通过 |
| P1-2 | workspace symbol/highlight/semantic tokens | P1 | 未开始 | LSP 端到端检查通过 |
| P1-3 | Gradle/Lombok/注解处理/test roots | P1 | 未开始 | 对应 fixture 通过 |

## 11. 完成定义

一个项目只有同时满足以下条件才可标记完成：

1. 代码已合入，测试在 CI 或本地门禁通过。
2. 验收场景有可重复的 fixture 或脚本。
3. docs/IDE_LSP.md 的能力表和限制已同步。
4. 没有引入未记录的近似实现；不支持的行为明确报错或返回空，不静默猜测。
