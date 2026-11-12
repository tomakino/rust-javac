# rust-javac

用 Rust 重写的 JDK 17 `javac` 编译器。

解析、符号录入、标注、流分析、transTypes、lower、字节码生成、class 文件写出和诊断机制全部由 Rust 实现，不调用也不包装 javac。验收标准是行为等价：class 文件 SHA-256 一致、`-XDrawDiagnostics` 输出逐行一致（文本、位置、数量、顺序）、命令输出与退出码一致。

- 参考实现：jdk17u `982d53c`（Temurin 17.0.19+10）
- 语言级别：Java 9–17，模块系统，`--release` 通过 ct.sym 提供 7–17 平台视图
- 对比语料：Gson、Guava、OkHttp、commons-lang3、JUnit 5，JDK 的 `jdk.compiler` / `java.base` / `java.desktop`，langtools 诊断样例，43 个语言特性用例

## 快速开始

```powershell
cargo build -p rust-javac
target\debug\ravac.exe -d out Hello.java
```

`build.rs` 会从 `JAVA_HOME` 硬链接 `lib/modules` 和 `lib/ct.sym` 到 `target\<profile>\`。运行时只读可执行文件旁的 `modules`、`ct.sym`（`bin/` 布局则读上一级），不查 `JAVA_HOME`、`PATH`，也不扫描文件系统；`RAVAC_PLATFORM_DIR` 可覆盖平台目录。

## 架构

14 个 crate 对应 `jdk.compiler` 的子系统：

| Crate             | 对应的 javac 组件                                     |
|:------------------|:------------------------------------------------------|
| `javac-core`      | `com.sun.tools.javac.{parser, comp, jvm, code, main}` |
| `javac-tools`     | `javax.tools`（文件管理器 API）                       |
| `javac-source`    | `com.sun.source`（tree API）                          |
| `javac-jimage`    | `jdk.internal.jimage`（`lib/modules` 读取）           |
| `javac-ctsym`     | ct.sym 读取（`--release` 平台视图）                   |
| `javac-classpath` | class path / module path                              |
| `javac-charset`   | 字符集                                                |
| `javac-unicode`   | Unicode 表                                            |
| `javac-floating`  | 浮点与 `BigDecimal` 辅助                              |
| `javac-jmod`      | jmod 读取                                             |
| `javac-zip`       | zip 读取                                              |
| `javac-fs`        | 文件系统抽象                                          |
| `javac-strings`   | 字符串工具                                            |
| `rust-javac`      | CLI（对应 `com.sun.tools.javac.Main`）                |

流水线沿用 javac 的阶段划分：parse → enter → attribute → flow → transTypes → lower → generate。诊断经过移植的 `Log` 与资源包层，官方 `.properties` 语言包内嵌并延迟解析。

## 命令行

```text
ravac [options] <source files>
ravac @argfile
```

| 选项                                       | 含义                              |
|:-------------------------------------------|:----------------------------------|
| `-d <dir>`                                 | class 文件输出目录                |
| `-cp`, `-classpath`, `--class-path <path>` | class 搜索路径                    |
| `-sourcepath <path>`                       | 源码搜索路径                      |
| `--release <n>`                            | 平台与语言级别（7–17）            |
| `-source <n>`, `-target <n>`               | 语言级别 / class 文件版本         |
| `-g`, `-g:none`, `-g:source,lines,vars`    | 调试信息                          |
| `-Xlint[:<group>]`, `-nowarn`              | lint 警告                         |
| `-verbose`                                 | 打印加载的文件与模块              |
| `-s <dir>`, `-h <dir>`                     | 源码输出 / 本地头文件输出         |
| `@<file>`                                  | 从参数文件读取选项和源文件        |
| `-J-Duser.language=<locale>`               | 诊断语言                          |
| `-XDrawDiagnostics`                        | 原始诊断格式                      |

```powershell
ravac -d out -cp "lib/*" src/A.java src/B.java
ravac --release 11 -d out src/Main.java
```

| 退出码 | 含义           |
|:------:|:---------------|
|   0    | 成功           |
|   1    | 编译错误       |
|   2    | 命令行参数错误 |
|   3    | 系统错误       |
|   4    | 内部错误       |

`ravac --help` 列出完整选项。注解处理、`--enable-preview`、`-Xplugin`、`-Xdoclint`、`-Xprint` 未实现（见“限制”）。

## 构建与分发

需要 Rust nightly-2026-11-01（`rust-toolchain.toml` 固定）、JDK 17（提供平台数据和参考编译器）、Python 3.8+（脚本）；打 `android-arm64` 包另需 Android NDK（API 21+）。

```powershell
cargo build -p rust-javac
py scripts/package.py --build --release --platforms all
```

| 平台            | 链接方式 | 运行时库     |
|:----------------|:---------|:-------------|
| `windows-amd64` | 原生     | msvcrt       |
| `linux-amd64`   | 静态     | musl         |
| `linux-arm64`   | 静态     | musl         |
| `android-arm64` | 动态     | Bionic（NDK）|

分发包包含编译器及它读取的两个数据文件：

```text
ravac-17.0.19-windows-amd64/
├─ ravac.exe
├─ modules      jimage：各模块的系统类
└─ ct.sym       --release 7–17 的平台签名
```

## 验证结果

以下为固定语料、固定选项、固定参考编译器（Temurin 17.0.19+10，Windows x64）下的采样结果，不代表任意程序都等价。对比的选项组合包括默认 `-source`/`-target 17`、`-g` 系列、`--release 11/17`、`-parameters`、`-encoding UTF-8`、`-Xmaxerrs`、`-Xdiags:compact`、`-Xlint` 系列、`-nowarn`、`-sourcepath`、`-s`、`-h`、`@argfile`，以及 Gson 上的 `-source 8`。

| 对象                    | 规模                             | 结果             |
|:------------------------|:---------------------------------|:-----------------|
| `javac-core` 单元测试   | 750                              | 750 通过         |
| Gson 2.10.1             | 208 个 class                     | 全部一致         |
| Guava 33.4.0            | 1970 个 class                    | 全部一致         |
| OkHttp 3.14.9           | 542 个 class                     | 全部一致         |
| commons-lang3 3.12.0    | 359 个 class                     | 全部一致         |
| JUnit 5.10.2            | 625 个 class                     | 全部一致         |
| `jdk.compiler`          | 1637 个 class                    | 全部一致         |
| `java.base`             | 3067 个源文件（6410 个 class）   | 全部一致         |
| `java.desktop`          | 2617 个源文件（5393 个 class）   | 全部一致         |
| 诊断样例语料            | 602 个可对比用例                 | 602 一致         |

## 测试

```powershell
py scripts/gate.py --run              # 按工作区改动推断级别
py scripts/gate.py --level L3 --run   # 全量
```

| 级别 | 覆盖                                             |
|:----:|:-------------------------------------------------|
| L0   | `cargo test -p javac-core --lib`                 |
| L1   | L0、repro 程序、Gson                             |
| L2   | L1、`jdk.compiler`、Guava、OkHttp                |
| L3   | 工作区全部测试、所有语料与模块检查               |

## 限制

以下能力有意不实现，命令行直接拒绝：

| 选项                               | 原因                                       |
|:-----------------------------------|:-------------------------------------------|
| `-proc:*`, `-processor*`, `-A*`    | 未实现 `javax.annotation.processing`       |
| `--enable-preview`                 | preview class 文件不保证字节一致           |
| `-Xplugin`, `-Xdoclint`, `-Xprint` | 未实现                                     |

自定义文件管理器和 JVM 栈追踪诊断（`-doe`）同样不支持，诊断语料跳过对应用例。

## 文档

协作规则与仓库约定见 [AGENTS.md](AGENTS.md)。

## 免责声明

独立重实现，与 Oracle、OpenJDK、Eclipse Adoptium 无关。它不是 javac 的完整替代：注解处理、preview 特性和插件均未实现，不追求覆盖 javac 的所有模式。验证结果只描述上文列出的语料、选项、参考编译器与宿主平台，不构成对任意程序的保证，也不适用于安全关键或受监管场景。

## 许可证

GPL-2.0-only WITH Classpath-exception-2.0，见 [LICENSE](LICENSE)。
