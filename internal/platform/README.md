# Bundled platform data

参考实现的平台数据，构建与分发直接使用，不依赖本机 JDK 的 `lib/modules`。

- `modules`：Windows 版 Temurin 17.0.19+10 的 `lib/modules` 经 `cargo xtask gen-platform`
  生成（RPI1 格式，已去除方法体与无引用常量池），约 17 MiB。
- `ct.sym`：原样复制自同一 JDK 的 `lib/ct.sym`，约 8 MiB。

用途：`crates/rust-javac/build.rs` 把这两个文件链接到 `target/<profile>/`；
`scripts/package.py` 把它们打进分发包。

校验：`cargo xtask gen-platform --check` 会重新生成并与已入库文件逐字节比较，
不一致即失败。修改 `javac-platform` 的生成逻辑或升级 JDK 后，必须重新生成并提交。

重新生成：

```powershell
cargo xtask gen-platform --modules "$env:JAVA_HOME\lib\modules" --out "internal\platform\17.0.19+10\modules"
```

边界：这是一套 Windows 平台视图。公开 API 代码跨平台通用；引用平台特定内部类
（如 `sun.nio.fs.UnixFileSystemProvider`、`sun.awt.X11.*`）的代码需在对应平台的
JDK 上重新生成数据。

来源与许可：数据来自 Eclipse Temurin JDK 17.0.19+10，许可与仓库一致，
为 GPL-2.0-only WITH Classpath-exception-2.0，见仓库根目录 `LICENSE`。
