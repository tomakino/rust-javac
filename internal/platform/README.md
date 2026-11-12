# Platform data（本地生成，不入库）

`internal/platform/17.0.19+10/` 存放编译器运行时读取的平台数据。两个文件都由参考 JDK
生成，**不提交到仓库**：

- `modules`：`cargo xtask gen-platform` 的输出（RPI2：去方法体、无引用常量池、非 class
  资源与索引压缩），约 13 MiB。
- `ct.sym`：JDK `lib/ct.sym` 的原样副本，约 8 MiB。

克隆仓库后生成一次（需要 JDK 17，并设置 `JAVA_HOME`）：

```powershell
cargo xtask gen-platform --out "internal\platform\17.0.19+10\modules"
Copy-Item "$env:JAVA_HOME\lib\ct.sym" "internal\platform\17.0.19+10\ct.sym"
```

`crates/rust-javac/build.rs` 会把它们链接到 `target/<profile>/`；
`scripts/package.py` 把它们打进分发包。文件缺失时两者回退到 `$JAVA_HOME/lib`。

校验本地镜像与重新生成的结果一致：

```powershell
cargo xtask gen-platform --check
```

边界：这是一套 Windows 平台视图。公开 API 代码跨平台通用；引用平台内部类的代码
（`sun.nio.fs.UnixFileSystemProvider`、`sun.awt.X11.*`）需要在对应平台的 JDK 上重新生成。

来源与许可：数据来自 Eclipse Temurin 17.0.19+10，许可与仓库一致，
为 GPL-2.0-only WITH Classpath-exception-2.0，见仓库根目录 `LICENSE`。
