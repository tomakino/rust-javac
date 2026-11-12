# rust-javac

用 Rust 重写的 JDK 17 `javac` 编译器。

从源码解析、符号录入、标注、流分析、字节码生成到诊断输出，整条编译流程都由 Rust 实现，不调用也不包装 javac。目标是与参考编译器行为一致：class 文件逐字节相同、诊断文本与位置相同、退出码相同。

参考实现为 jdk17u（Temurin 17.0.19+10）。支持 Java 9–17 语言特性、模块系统，以及通过 ct.sym 提供 `--release 7–17` 平台视图。

## 获取与运行

目前没有预编译发布包，从源码构建：

```powershell
cargo build -p rust-javac --release
```

构建需要 JDK 17，并把 `JAVA_HOME` 指向它：构建脚本会将 `lib/modules` 和 `lib/ct.sym` 放到可执行文件旁。分发包里的 `modules` 是去掉方法体的精简平台镜像（约 36 MiB，JDK 原文件约 123 MiB），由 `cargo xtask gen-platform` 生成。运行时 `ravac.exe` 只读同目录（`bin/` 布局则读上一级）的 `modules` 与 `ct.sym`，不查 `JAVA_HOME`、`PATH`，也不扫描文件系统。

```powershell
target\release\ravac.exe -d out Hello.java
```

## 用法

```text
ravac [options] <source files>
ravac @argfile
```

| 选项                                       | 含义                       |
|:-------------------------------------------|:---------------------------|
| `-d <dir>`                                 | class 文件输出目录         |
| `-cp`, `-classpath`, `--class-path <path>` | class 搜索路径             |
| `-sourcepath <path>`                       | 源码搜索路径               |
| `--release <n>`                            | 平台与语言级别（7–17）     |
| `-source <n>`, `-target <n>`               | 语言级别 / class 文件版本  |
| `-g`, `-g:none`, `-g:source,lines,vars`    | 调试信息                   |
| `-Xlint[:<group>]`, `-nowarn`              | lint 警告                  |
| `-verbose`                                 | 打印加载的文件与模块       |
| `-s <dir>`, `-h <dir>`                     | 源码输出 / 本地头文件输出  |
| `@<file>`                                  | 从参数文件读取选项和源文件 |
| `-J-Duser.language=<locale>`               | 诊断语言                   |
| `-XDrawDiagnostics`                        | 原始诊断格式               |

```powershell
ravac -d out -cp "lib/*" src/A.java src/B.java
ravac --release 11 -d out src/Main.java
```

`ravac --help` 列出完整选项。

| 退出码 | 含义           |
|:------:|:---------------|
|   0    | 成功           |
|   1    | 编译错误       |
|   2    | 命令行参数错误 |
|   3    | 系统错误       |
|   4    | 内部错误       |

## 质量

在 Gson、Guava、OkHttp、commons-lang3、JUnit 5，以及 JDK 自身的 `jdk.compiler`、`java.base`、`java.desktop` 上，与 Temurin 17.0.19+10 对比：产出的 class 文件 SHA-256 全部一致；602 个 langtools 诊断样例逐行一致（文本、位置、数量、顺序）。覆盖范围还包括 43 个 Java 9–17 语言特性用例。

这些是固定语料、固定选项下的采样结果，不代表任意程序都等价。

## 限制

以下能力有意不实现，命令行直接拒绝：

| 选项                               | 原因                                 |
|:-----------------------------------|:-------------------------------------|
| `-proc:*`, `-processor*`, `-A*`    | 未实现 `javax.annotation.processing` |
| `--enable-preview`                 | preview class 文件不保证字节一致     |
| `-Xplugin`, `-Xdoclint`, `-Xprint` | 未实现                               |

自定义文件管理器和 JVM 栈追踪诊断（`-doe`）同样不支持。

## 免责声明

独立重实现，与 Oracle、OpenJDK、Eclipse Adoptium 无关。它不是 javac 的完整替代：注解处理、preview 特性与插件均未实现，也不追求覆盖 javac 的所有模式。验证结果只描述上文列出的语料与选项，不构成对任意程序的保证，不适用于安全关键或受监管场景。

## 许可证

GPL-2.0-only WITH Classpath-exception-2.0，见 [LICENSE](LICENSE)。
