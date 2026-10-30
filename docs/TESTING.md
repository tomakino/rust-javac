# 测试

## 分层

1. **单元测试** —— `cargo test -p javac-core --lib`（846 个）：解析、类型、流分析、诊断格式化、properties 解析、oracle 比对。
2. **CLI 一致性** —— `cargo test -p rust-javac --test cli_parity`（18 个）：选项处理、`-XDrawDiagnostics`、帮助输出、退出码。
3. **官方诊断示例** —— `cargo test -p rust-javac --test diag_examples`：运行上游 langtools 语料中的 669 个示例，与固化的 expected 输出归一化比对。
   - 环境变量：`DIAG_EXAMPLES_FILTER`、`DIAG_EXAMPLES_VERBOSE`、`DIAG_EXAMPLES_TIMEOUT_SECS`。
   - 当前：612 通过 / 3 失败 / 54 跳过。
4. **语料 / oracle 工具** —— `crates/javac-tests` 提供 `gen-diagnostics-oracle`、`gen-corpus-baselines`、`diff_runner`，用于和本地安装的 JDK 比对。
5. **真实项目** —— Gson 2.10.1 与 OkHttp 3.14.9 会被编译，并与官方 javac 输出比对。

## 规则

- 测试不得修改固化的 expected 输出。
- 依赖 JVM 宿主机制（注解处理器、插件、模块化 jar、JVM 栈帧）的用例标记为 `skip`，不算通过。
- 比对诊断文本前会归一化与环境相关的临时路径（`<tmp>`）。
