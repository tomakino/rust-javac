# rust-javac

JDK 17 `javac` 的 Rust 重实现，目标是诊断与编译行为对齐官方 javac（基线：jdk17u `982d53c`，17.0.19+10）。

## 构建与运行

需要 JDK 17（`JAVA_HOME` 指向 JDK 根目录）和 Rust 1.99：

```bash
cargo build -p rust-javac
target/debug/rust-javac -d out Hello.java
```

常用选项与 javac 17 一致，例如 `-d`、`-cp`/`-classpath`、`-sourcepath`、`-source`/`-target`、`-Xlint`、`-XDrawDiagnostics`、`-J-Duser.language=zh_CN`。

## 现状

- 官方诊断示例 **612 / 669** 通过（3 个已知差异；54 个跳过：注解处理器、模块化 jar、插件、JVM 栈帧等依赖 JVM 宿主机制的用例）；
- `javac-core` 单元测试 **846 通过**；
- Gson 2.10.1 的诊断输出与 javac 逐字一致；OkHttp 3.14.9 由移植的官方测试套件覆盖。

## 测试

```bash
cargo test -p javac-core --lib
cargo test -p rust-javac --test cli_parity
```

## 许可

GPL-2.0-only WITH Classpath-exception-2.0，详见 [LICENSE](LICENSE)。
