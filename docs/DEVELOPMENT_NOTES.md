# 开发笔记

## 坑点：在项目根目录编译单文件需要 45 秒

### 现象

在仓库根目录执行：

```bash
target\debug\rust-javac.exe -d out Hello.java
```

单文件编译耗时约 **45 秒**；把 `rust-javac.exe` 复制到干净的临时目录，
`Push-Location` 进去再运行，同一文件降到 **0.7–1.7 秒**。

### 根因

classpath 默认包含 `.`（当前目录）。`crates/javac-core/src/main/mini_compiler.rs` 的
`add_classpath_packages_to_unnamed_module` 会递归扫描 classpath 目录收集“包”，
而仓库根包含 `internal/reference`（JDK 源码树，几万文件），扫描全部文件导致数十秒开销。

### 修复

classpath 扫描的跳过列表从：

```rust
"target" | "build" | ".git" | ".gradle" | "node_modules" | "out"
```

扩展为：

```rust
"target" | "build" | ".git" | ".gradle" | "node_modules" | "out"
    | "internal" | "reference"
```

修复后：仓库根 **1.26 秒**，干净目录 **0.70 秒**。

### 教训

1. **测性能前先隔离 cwd**：把二进制放进干净目录并用 `Push-Location` 切换，
   否则 `-cp .` 会把仓库里的参考树/测试数据全扫一遍。
2. **用阶段计时定位，不要靠猜**：`parse` → `modules` → `enter` → `attr` → `flow` →
   `desugar`/`gen`。本次：parse 1.3ms / modules 353ms / enter 468ms / attr 622ms，
   总计约 1.7 秒。
3. 临时 `eprintln!` 定位后必须清理（本次清理了 17 处）。
4. 超过 30 秒的命令直接停止，不要等。

## 相关坑点：CLI 默认语言与 `-J-Duser.language`

- 官方 javac 的默认语言来自 JVM 的 `user.language`，Windows 上由
  `GetUserDefaultUILanguage()` 决定（中文系统默认 `zh_CN`）。
- 移植版原来只读 `LC_ALL/LC_MESSAGES/LANG`，Windows 上回退 `en`，导致 CLI 全英文。
  修复：`Locale::default_locale()` 增加 Windows UI 语言检测。
- `-J-Duser.language` 原来只影响帮助文本，不影响编译诊断；修复：`set_default_locale()`
  在 CLI 解析后注入进程级默认 locale，编译器诊断跟随。
- 单元测试里 `default_locale()` 固定返回 `en`，避免测试结果随宿主语言变化。
