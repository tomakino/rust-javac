# rust-javac 端到端测试计划

## 1. 目标与范围

**E2E 定义**：从命令行入口（`rust-javac.exe`）到最终产物（`.class`、诊断文本、退出码），与官方 `javac` 逐项对照；不测内部函数（那是 `--lib` 单元测试的职责）。

**覆盖**：CLI 解析 → 文件查找/读取 → 解析/进入/属性/流程/转译/降级/生成 → 输出/诊断/退出码 → 性能。

**不覆盖（本质不可行）**：JVM 插件（`-Xplugin`）、注解处理器（`-processor`/`-proc:only`）、`-doe` 的 JVM 堆栈、自定义 `-filemanager`、`-profile`/`-source 7` 等废弃机制。

## 2. 测试分层

| 层 | 内容 | 命令 | 耗时 |
|---|---|---|---|
| L1 单元 | 846 个 lib 测试 | `cargo test -p javac-core --lib` | ~3s |
| L2 诊断语料 | 669 官方示例（全文对比） | `cargo test -p rust-javac --test diag_examples` | ~40s |
| L3 项目字节 | Gson 207 class、okhttp 547 class | `scripts/e2e.ps1` | ~10s / ~90s |
| L4 CLI 行为 | help/版本/错误/退出码 | `scripts/e2e.ps1` | ~5s |
| L5 性能 | release 基准 | `scripts/e2e.ps1 -Perf` | ~10s |

## 3. L4 CLI 行为用例矩阵

| 用例 | 预期（对齐 javac） |
|---|---|
| `--help` | 标准选项表（含 `-J<flag>` 行） |
| `--help-extra` | 扩展选项表，**不含** `-J` |
| `--help-lint` | 与 javac **逐字节一致**（31 key + `all`/`none`） |
| `--version` / `--full-version` | `javac 17.0.19` |
| 无参数 | 打印标准 help，退出码 **2** |
| `-J<flag>` | 被 launcher 吞掉（`-J-Duser.language=en` 生效） |
| `-Xstdout <file>` | 诊断写入文件、控制台空、**文件与 javac 字节一致**（含 CRLF） |
| `@argfile` | 读参数文件（`#` 注释、空白分隔） |
| 非法选项 | `error: invalid flag: <arg>` + usage，退出码 **2** |
| 不存在的源文件 | `error: file not found: <path>` + usage，退出码 **2** |
| `-Werror` | 有警告时 `error: warnings found and -Werror specified`，退出码 **1** |
| `-Xmaxerrs N` / `-Xmaxwarns N` | 超过上限后截断 + 汇总行 |
| `-encoding` | UTF-8 / GBK 正确读源；无效编码报错 |
| `-Xlint:all` / `-Xlint:none` / `-Xlint:-rawtypes` | 警告集合正确 |

## 4. L3 项目字节与诊断

| 用例 | 通过标准 |
|---|---|
| Gson（80 源 → 207 class） | **207/207 SHA-256 一致**；`-XDrawDiagnostics` diff 空 |
| okhttp（229 源 → 547 class） | **547/547 一致**；诊断 diff 空（当前有 3 类已知差异，见 §7） |
| 编译产物可运行（可选） | 用 `java` 跑 rust 编译出的 class（如 JUnit），验证运行时行为等价 |
| 增量/重复运行 | 同一输入跑 2 次，输出字节稳定 |

## 5. L2 诊断语料门禁

- `fail=0`；
- `skip` **白名单固定为 67**：annotation-processing=47、custom-file-manager=2、jvm-stacktrace=1、modules-automatic=2、plugin=1、verbose=1、deprecated-profile=1、deprecated-source=12；
- **实时对照模式（建议新增）**：不依赖固化 `expected/`，用 `javac` 现场生成参考，防止 JDK 升级后 expected 漂移。

## 6. L5 性能基准（release）

| 用例 | 当前基线 | 阈值 |
|---|---|---|
| Gson 编译 | rust release **0.82s** vs javac **2.44s** | rust ≤ javac × 1.2 |
| okhttp 编译 | 待测（release） | 同上，另设回归阈值（如 ≤ 上次 × 1.1） |
| 冷启动 vs 热运行 | 各跑 5 次取中位数 | 记录趋势 |

## 7. 已知缺口（不纳入门禁）

1. okhttp 诊断的**内容、位置、数量**已与 javac 完全一致；仅剩 **2 条 warning 的相对顺序**不同（`RecordedRequest.java:139:17` 与 `FakeSSLSession.java:73:10`）。根因：javac 的 Attr 处理顺序来自 `TypeEnvs.values()`，而官方 `TypeEnvs.java:52` 用 `HashMap`，其迭代序取决于 JVM 的 `identityHashCode`（对象身份），**跨语言不可复现**；端口 `type_envs.rs:19` 同样用 `HashMap`（源码层面与官方一致）。此项不纳入门禁，也不应通过排序等特判手段"修复"；
2. diag-corpus 的 67 个 SKIP：47 个注解处理、1 个插件、1 个 `-verbose`、1 个 `-doe`、2 个自动模块、2 个自定义 FileManager、2 个废弃机制；
3. `-proc:none`/`-proc:only` 未支持（JVM 处理器）；
4. expected 为固化文本，随 JDK 升级可能过时（建议加实时对照）。

## 8. 执行节奏与门禁

- **每次提交**：L1（846）必须全绿；
- **每日/大改**：L2（fail=0、skip=67）+ L3（Gson 207/207）+ L4 CLI 用例；
- **发布前**：全量 L1–L5 + 性能基准 + 冷启动；
- **失败处理**：L2 失败按 `DIAG_EXAMPLES_VERBOSE=1` 看首个差异行；L3 失败用 `javap -v` 做常量池/指令 diff。

## 9. 脚本

`scripts/e2e.ps1`：

```powershell
pwsh ./scripts/e2e.ps1                 # L3(Gson) + L4
pwsh ./scripts/e2e.ps1 -IncludeOkhttp  # 加上 okhttp 字节对比
pwsh ./scripts/e2e.ps1 -Perf           # 加上 L5 性能基准
```
