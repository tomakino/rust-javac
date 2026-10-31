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
### 性能榨取尝试（2026-10-31 第三轮）

- 保留：`[profile.release] lto = "fat"` + `codegen-units = 1`（仅 release 构建生效）。
- **PGO 不可行**：`-Cprofile-generate` 需要 `profiler_builtins`；rustup 的 windows-gnu 目标未提供该运行时（只有 android/linux 目标有 `libprofiler_builtins`），安装 `rustc-dev` 后仍缺失；msvc 工具链缺 Windows SDK（`kernel32.lib`），无法替代。
- **mimalloc 不可行**：`libmimalloc-sys` 需要完整 MinGW `gcc`（本机只有 rust-mingw 的驱动，缺 `cc1.exe`），已回滚依赖与 `global_allocator`。
- **`Name` 去 bytes（对齐官方 `NameImpl`）**：实现后 Gson `exit=4`（`RefCell` 借用重入）、`javac-core --lib` 72 fail，已回滚；后续如需重做，应改用不重入借用表的方案（例如 `Name` 持有 `Rc<RefCell<Vec<u8>>>` 或表访问闭包严格禁止重入 `from_*`）。
- 结论：当前可复现的低风险优化空间已基本用尽；剩余差距主要是 JVM JIT/成熟缓存与 Rust 运行模型差异。
### 性能榨取第二阶段（2026-10-31 第四轮）

- **`Name` 去 bytes（对齐官方 `NameImpl`）**：`Name` 改为 `(table, index, length)`，字节经 `NameTableHandle::with_bytes` 读取；消除每次驻留的字节拷贝与一个 `Rc` 分配。首次实现因 `RefCell` 借用重入回归（Gson exit=4、lib 72 fail），修正版（可能重入的调用点先复制字节）通过：Gson 208/208 + IDENTICAL、lib 846。
- **`FxNameMap`**：为 `Name` 键的高频 map（visible_packages、NamedImportScope name2_scopes、parser tag_parsers、module seen-packages）引入 Fx 风格 hasher（查找/插入语义不变）。
- 结果（Gson debug，3 次 CPU）：min **5.531 s**、mean 5.615 s。
### 性能榨取第三阶段（2026-10-31 第五轮）

- **inapplicable 失败通道（Result 化）**：`MethodCheckContext.report` 不再 panic，改为记录 thread-local 失败；`catch_inapplicable_method_exception` 在 body 返回后检查记录并返回 `MethodCheckFailure::Inapplicable`（panic 通道保留给其余 throw 点）。Gson debug CPU **5.531 → 4.562 s（-17.5%）**。
- **Symtab 类索引**：官方 `Symtab.java:250` 的 `classes` 是 `HashMap<Name, Map<ModuleSymbol,ClassSymbol>>`；port 原为 `Vec` 线性查找。增加 `class_index: FxNameMap<Name, usize>`（保持 `classes` Vec 的插入顺序用于遍历），`get_class_in`/`do_enter_class`/`get_classes_for_name` 走索引。Gson debug CPU **4.562 → 3.984 s（-12.7%）**。
- 采样残留热点：`miniz_oxide` inflate（javac_zip，三方库内部）、`SymbolId/TypeId/TreeId::index`（debug 未内联，release 会内联）、`TypeEnter::resolve_imports`、`FxNameMap` rehash（少量）。
- 累计：Gson debug CPU 5.531 → **3.984 s（-28%）**；行为验证全过（Gson 208/208、诊断 IDENTICAL、lib 846）。
### 性能榨取第四阶段（2026-10-31 第六轮）

- **FxNameMap 预分配**：`fx_map_with_capacity` 用于 class 索引（1024）、visible_packages（32）、name2_scopes（8）、tag_parsers（25）、module seen（64），消除 `reserve_rehash`。Gson debug CPU **3.984 → 3.406 s（-14%）**。
- 累计：Gson debug CPU **5.531 → 3.406 s（-38%）**。
- 采样残留（debug 固有/三方库）：`miniz_oxide` inflate（javac_zip 依赖）、`SymbolId/TypeId/TreeId::index` 与 `copy_nonoverlapping::precondition_check`（debug 未内联/边界检查，release 会优化）、`TypeEnter::finish_class`、`FxNameMap` 插入。
### javac_zip 解压层（2026-10-31 第七轮）

- **采用 `zlib-rs`**（纯 Rust，zlib 兼容 inflate）：`ZipArchive::read_entry` 的 DEFLATED 分支用 `zlib_rs::Inflate::new(false, 15)`（raw deflate）+ `decompress(..., InflateFlush::Finish)`，输出按 `uncompressed_size` 预分配并校验 `total_out`。
- **`rawzip` 解析层**：已接入验证，但 10 个 port zip 测试中 6 个失败（zip64 拒绝、CP437 名称、`archive_offset`、未知压缩方法值），按行为等价要求回滚。
- **`libdeflater`**：无法构建（`libdeflate-sys` 需要 C 工具链；本机缺 MinGW `cc1`/Windows SDK，便携 MinGW 下载不可用）。
- 验证：`javac-zip` 10/10、Gson 208/208 + IDENTICAL、`javac-core --lib` 846；Gson debug CPU 3.953–4.516 s（当前负载下与 zune/miniz 同档，历史低负载 3.406 s）。
### javac_zip：rawzip + zlib-rs 落地（2026-10-31 第八轮）

- `ZipArchive` 现在持有 **rawzip reader**（`rawzip::ZipArchive<FileReader>`）与每个 central-directory 条目的 `wayfinder`；`read_entry` 用 `raw.get_entry(wayfinder)` + `ZipEntry::reader()` 读取压缩字节，再用 **`zlib_rs::Inflate`（raw deflate）解压**。
- **central directory 仍由 port 的解析器读取**（`parse_central_directory`）：保留既有 zip 契约（zip64 拒绝、CP437/UTF-8 名称、压缩方法原值、`archive_offset`、`NotAZip` 错误），这正是之前纯 rawzip 版本 6/10 测试失败的原因；rawzip 负责 local header/数据定位读取。
- 验证：`javac-zip` 10/10、Gson 208/208 + IDENTICAL、`javac-core --lib` 846；Gson debug CPU 4.078–4.359 s（与 zlib-rs-only 3.953 同档；历史低负载 3.406 s）。
### javac_zip 整体重构（2026-10-31 第九轮）

- 模块化：`error.rs`（Error）、`entry.rs`（Entry + UTF-8/CP437 名称解码）、`decompress.rs`（zlib-rs 解压）、`archive.rs`（ZipArchive：rawzip 解析 central directory 与 local header，zlib-rs 解压）、`cp437.rs`。
- **central directory 改为 rawzip 解析**（此前保留 port 的解析器）：条目名（UTF-8 flag / CP437）、压缩方法（STORE/DEFLATE 映射，未知 → `u16::MAX`）、sizes、crc、flags（原始位）均来自 rawzip `ZipFileHeaderRecord`；`read_entry` 用 wayfinder + `ZipEntry::reader()`。
- **zip64 现在被接受**（rawzip 支持；`java.util.zip.ZipFile` 也支持），port 的 `UnsupportedZip64` 仅保留 API 变体；对应测试改为 `supports_zip64_central_directory_fields`。
- `archive_offset` 仍由 tail EOCD 位置推导（jmod 前缀 = 4），保持 port 的 jmod 契约。
- 验证：`javac-zip` 10/10、Gson 208/208 + IDENTICAL、`javac-core --lib` 846；Gson debug CPU 4.5–5.1 s（当前负载波动大，与历史 3.4–5.8 范围重叠）。
### javac_zip：整文件驻留 + rawzip slice（2026-10-31 第十轮）

- **问题**：旧实现每次 `read_entry` 都通过 `FileReader` 读 local header + 压缩数据（同一 jar 被间断频繁读取时每条目两次系统调用）。
- **优化**：`ZipArchive::open` 用 `std::fs::read` 一次读入整个 zip，`rawzip::ZipArchive::<()>::from_slice`（零拷贝 slice）解析；`read_entry` 用 `ZipSliceEntry::data()` 直接切片，**运行时零文件读取**；`archive_offset` 直接在内存字节上计算。
- 验证：`javac-zip` 10/10、Gson 208/208 + IDENTICAL、`javac-core --lib` 846；Gson debug CPU **3.828–4.094 s（min 3.828，mean 3.953）**，此前切片前为 4.688/5.052（-16%～-21%）。
### okhttp 字节差异调查（2026-10-31 第十二轮）

- okhttp（229 源、547 class）与官方 javac 对比：**69 个 class 字节不一致**（此前只验证了“0 错误”，未比对字节）。
- 差异类型（抽样确认）：
  - **`InnerClasses` 缺少被引用成员类条目**：如 `Cache$2` 引用 `DiskLruCache$Snapshot`，javac 的 `InnerClasses` 含 `Snapshot`，port 缺（`Cache$2` 2074 vs javac 2097 字节）。
  - **lambda 合成方法的 `LineNumberTable`**：`lambda$static$0` 的 port 写 `line 1`，javac 写 lambda 所在行（如 `line 100`）。
- 已修：`PoolWriter.put_signature` 现在把 `SignatureGenerator` 收集的类引用合并进 `inner_classes`（官方 `SignatureGenerator.classReference` → `enterInner`）；Gson 仍 208/208 + IDENTICAL、lib 846。**尚不足以修复 `Cache$2`**（需要继续查 `write_member_attrs`/`Signature` 路径）。
- 另发现性能问题：`PoolWriter.keys_to_pos` 是 `Vec` 线性扫描（官方 `HashMap`，`PoolWriter.java:334`），常量池查找 O(n²)。
### 性能：本机 CPU 优化（2026-10-31 第十一轮）

- `.cargo/config.toml` 增加 `[target.x86_64-pc-windows-gnu] rustflags = ["-C", "target-cpu=native"]`（本机专用；与同机 javac 对比一致）。
- Gson debug CPU：3.656 / 3.943 / 4.047（min 3.656，mean 3.943），此前 3.828/3.953。
- `ids::index` 的 `#[inline(always)]` 在 debug 下实测更慢（4.000 vs 3.828），未采用；release 由编译器内联。
- 行为验证：Gson 208/208 字节一致 + 诊断 IDENTICAL；`javac-core --lib` 846 passed。
- PGO 与 mimalloc 仍不可行（见上一轮记录）。
### 性能优化（2026-10-31 第二轮，samply profiling）

- samply 在本机崩溃根因：ETW `ImageFileName` 为 GBK（“哔哩哔哩.exe”），samply 0.13.1 用严格 UTF-8 解析导致 panic；本地补丁版 samply 把 `parser.rs` 的 `String::from_utf8` 改为 `from_utf8_lossy`（工具修复，不入库）。
- 热点与修复（官方依据：`Symtab` 直接比较 `Name`；`Modules.addVisiblePackages` 原地 `visiblePackages.put`；zip entry 查找语义不变）：
  1. `symtab.rs` 10 处 `name.to_java_string() == other.to_java_string()` 改为直接比较 `Name`（原实现每次比较都解码 UTF-8→UTF-16 并分配，profile 占 22%+）；
  2. `enter.rs` 的 `visible_packages.clone()` 改为借用；`modules.rs` 的 `module_visible_packages()` clone+insert+写回改为 `sym_mut` 原地 insert；
  3. `javac-zip` 的 `entry_index` 改用 FxHasher（查找语义不变）。
- 结果（仅编译）：Gson release **0.973 s**（官方 2.40 s）；okhttp release **24.28 s**（旧 release 51.84 s，官方 8.17 s）；Gson debug 23.2→6.48 s。
