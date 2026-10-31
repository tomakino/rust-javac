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

## 2026-10-31：okhttp3 全量编译 0 错误与 P5 根因

### 结果

- okhttp3 全量（229 个 Java 源）编译 **0 错误**；okhttp JUnit 默认 4 类 416 tests、internal 15 类 256 tests 全部 ok。
- Gson 2.10.1 主源码 **208/208 class 逐字节一致**、诊断 IDENTICAL；Gson JUnit 1258/2（与官方一致）。
- `javac-core --lib` 846；`diag_examples` 612/3/54（3 个已知接受项）。

### 根因与修复（官方依据）

1. `Infer.instantiateMethod` 末尾的“最终参数检查”是移植版自加适配块（无官方对应），会给有界形参额外加约束；删除后 P5 通过（官方 `Infer.java:215-224` 只有 `deferredAttrContext.complete()` 后 `solve`）。
2. `TypeEnter.finishImports` 应等完成级联返回后再执行；移植版在嵌套完成中提前执行，导致 `TlsUtil` 静态导入只看到 1/7 成员。改为按完成栈延迟到最外层边界（官方 `TypeEnter.java:204-217/223-237/249-297`）。
3. `ResolvedConstructorType.overloadCheck` 把 `localInfo.pt` 的 TYPEVAR/UNDETVAR 改成 `noType` 是非官方特判；删除后 diamond 的返回约束才能进入主推断上下文（官方 `ArgumentAttr.java:704-707`）。
4. 官方 `InferenceException extends InapplicableMethodException`（`Infer.java:138`）；`catch_inapplicable_method_exception` 必须同时接住 `InferenceExceptionPayload`（如 `Resolve.instantiate`，`Resolve.java:673-690`）。
5. `ArgumentAttr.speculativeTypes` 改为跨 `dup` 共享、按 `ResultInfo.id` 为键（替代栈指针地址），`ResolvedMethod` 复用 resultInfo 身份（官方 `ArgumentAttr.java:343-348/357/675-678`）。

### 性能（仅编译，不含运行）

| 数据集 | debug | release | 官方 javac（warmup 后） |
|---|---|---|---|
| Gson 全量 | mean 23.21 s | mean **6.47 s** | 2.40 s |
| okhttp 全量 | mean 150.12 s | mean **51.84 s** | 8.17 s |

- release 比 debug 快约 3.6×/2.9×；与官方 javac 比仍慢约 2.7×/6.3×，待 profiling（`samply`/`cargo flamegraph`）。

### 已知偏差（登记，未实现）

- 未实现官方完整 TypeEnter Phase 树（`HierarchyPhase` 的 Completer 排序、`AbstractMembersPhase` 的 todo/completing 重入队列）；只对齐可观察的 `finishImports` 时序。
- `speculativeTypes` 用 `Vec` 保留插入序（官方 `HashMap` 依 identity hash）；put 已按 `ResultInfo` 身份覆盖，当前语义对齐，迭代序差异已登记。
- Rust 用 panic 适配 Java 受检异常，catch 时 panic hook 会打印 `resolve.rs:4220` 噪音；非行为失败。
### 性能优化（2026-10-31 第二轮，samply profiling）

- samply 在本机崩溃根因：ETW `ImageFileName` 为 GBK（“哔哩哔哩.exe”），samply 0.13.1 用严格 UTF-8 解析导致 panic；本地补丁版 samply 把 `parser.rs` 的 `String::from_utf8` 改为 `from_utf8_lossy`（工具修复，不入库）。
- 热点与修复（官方依据：`Symtab` 直接比较 `Name`；`Modules.addVisiblePackages` 原地 `visiblePackages.put`；zip entry 查找语义不变）：
  1. `symtab.rs` 10 处 `name.to_java_string() == other.to_java_string()` 改为直接比较 `Name`（原实现每次比较都解码 UTF-8→UTF-16 并分配，profile 占 22%+）；
  2. `enter.rs` 的 `visible_packages.clone()` 改为借用；`modules.rs` 的 `module_visible_packages()` clone+insert+写回改为 `sym_mut` 原地 insert；
  3. `javac-zip` 的 `entry_index` 改用 FxHasher（查找语义不变）。
- 结果（仅编译）：Gson release **0.973 s**（官方 2.40 s）；okhttp release **24.28 s**（旧 release 51.84 s，官方 8.17 s）；Gson debug 23.2→6.48 s。
