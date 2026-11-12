# 全局索引系统重构计划（无兼容设计）

状态：2026-11-06，已批准；M1 完成；M2 已完成查询切换与旧索引删除，结构清理进行中。

## 实施进度

- 2026-11-06 M1：ProjectIndex、StringInterner、SymbolKey、FileIndex、WorkspaceModel、IndexBuilder；project_index_spec 7 项通过。
- 2026-11-06 M2：删除旧 flat index、index_lookup、ProjectCache；definition/references/documentSymbol/completion 改查 ProjectIndex；新增 workspace/symbol API 与 LSP handler；project_index 模块拆成 model/builder/session/document_symbols/workspace_symbols/symbols。
- 质量门禁：新增 completion_project_spec，真实 source_path 下验证补全索引分支；Partial 查询拒绝；源文件 hash 失效；overlay 只单独编译 overlay 文件。
- 仍未完成：WorkspaceService 抽象；prepareRename 仍走 resident compiler；completion/navigation 仍有超大文件；索引构建同步阻塞；M3/M4 未开始。

前提：项目未发布，不保留旧索引、旧 AnalysisHost 默认方法、旧 ProjectCache，也不做双跑兼容层。直接替换为最合适的架构，测试随新实现重写。

## 1. 目标

把“编译器会话即索引”改为独立、紧凑、可增量、可持久化的项目索引。

成功标准：

1. references、rename、definition、documentSymbol、workspace symbol 全部从索引读取。
2. 连续两次跨文件 references，第二次不编译项目。
3. 索引不持有 AnalysisSession、AST、类型缓存。
4. 修改一个文件后只重建该文件索引；声明/签名变化才重建依赖文件。
5. 进程重启后可从磁盘索引恢复，不重新编译全项目。
6. 大项目有明确阈值和 Partial 降级，不静默 OOM。
7. 现有 completion_spec 130、navigation_spec、diagnostics_spec、maven_project、LSP 冒烟在新架构下重写并通过。

## 2. 目标分层

    LSP adapter
      -> WorkspaceService (API)
        -> Workspace (配置、overlay、生命周期)
          -> IndexStore (Arc 快照 + 磁盘)
          -> IndexBuilder / IncrementalUpdater
          -> FileAnalysis (临时编译器，按文件)

    javac-analysis-api
      -> 纯 DTO + WorkspaceService trait

编译器只在 FileAnalysis 和 IndexBuilder 的临时批次中出现，索引层不依赖编译器。

## 3. Workspace 与 FileAnalysis

### Workspace

    pub struct Workspace {
      config: WorkspaceConfig,
      model: WorkspaceModel,
      index: IndexStore,
      overlays: HashMap<PathBuf, String>,
      analyses: LruCache<FileId, FileAnalysis>, // 最多 4 项
    }

负责：

- 解析 source sets、classpath、toolchain
- 管理 overlay 和文件版本
- 发布索引快照
- 提供 completion/hover/diagnostics 等需要编译器的查询入口

### FileAnalysis

    pub struct FileAnalysis {
      file: FileId,
      text_hash: u64,
      compiler: AnalysisSession,
      diagnostics: Vec<Diagnostic>,
      declarations: Vec<SymbolKey>,
      occurrences: Vec<Occurrence>,
    }

只覆盖一个文件；查询结束可丢弃。完成类型推断、诊断、inlay hints、completion 的 receiver/expected type 计算。

不允许把 FileAnalysis 放进项目索引。

## 4. 数据模型

### SymbolKey

    pub struct SymbolKey {
      owner: Option<SymbolId>,
      name: NameId,
      kind: SymbolKind,
      signature: SignatureId,
    }

- 类/接口/枚举/记录：owner + name + kind。
- 字段：owner + name。
- 方法/构造器：owner + name + 参数类型指纹 + arity；返回类型不参与重载身份。
- 局部变量/参数：key 带 FileId + 声明 range；只允许文件内引用。
- NameId/SignatureId 由 StringInterner 产生，索引内部用 u32。

### ProjectIndex

    pub struct ProjectIndex {
      generation: u64,
      workspace: WorkspaceFingerprint,
      strings: StringInterner,
      symbols: Vec<SymbolRecord>,
      by_key: HashMap<SymbolKey, SymbolId>,
      by_name: HashMap<NameId, Vec<SymbolId>>,
      by_file: HashMap<FileId, FileIndex>,
      by_supertype: HashMap<SymbolId, Vec<SymbolId>>,
      occurrences: HashMap<SymbolId, Vec<Occurrence>>,
      files: Vec<FileRecord>,
      status: IndexStatus,
    }

    pub struct SymbolRecord {
      key: SymbolKey,
      kind: SymbolKind,
      file: FileId,
      range: Range,
      detail: String,
      modifiers: u32,
      supertypes: Vec<TypeRef>,
      return_type: Option<TypeRef>,
    }

    pub struct Occurrence {
      file: FileId,
      range: Range,
      symbol: SymbolId,
      role: Declaration | Reference | Import | TypeUse | Override,
    }

    pub enum TypeRef {
      Project(SymbolId),
      External(String),
    }

### 外部类型

JDK 和 classpath jar 不进入全量声明索引，只以 TypeRef::External 保存。查询 supertypes/成员时通过编译器/classpath 解析，避免索引所有 jar。

### IndexStatus

    Building
    Ready
    Partial { failed_files, reason }
    Failed

Partial 是显式状态，不冒充 Ready。

## 5. 索引构建

### WorkspaceModel

    pub struct WorkspaceModel {
      root: PathBuf,
      sources: Vec<SourceSet>,
      classpath_hash: u64,
      source_level: String,
      jdk_home: Option<PathBuf>,
      platform_dir: Option<PathBuf>,
      generation: u64,
    }

    SourceSet { kind: Main|Test|Generated, roots, classpath }

由 AnalysisConfig + Maven 发现构建；classpath/source root 变化使索引全量失效。

### IndexBuilder

    build(model, overlays, batch_size=128) -> IndexBuildResult

流程：

1. 枚举 source set 下 .java；计算 canonical path + 内容 hash；overlay 优先。
2. 按 128 文件一批。
3. 每批构建临时 AnalysisSession（完整 classpath/sourcepath），前端分析该批文件。
4. 只提取该批 CompilationUnit 的声明/occurrence；不索引被按需解析的依赖文件。
5. 构建 SymbolKey；生成 SymbolRecord/Occurrence。
6. supertypes 先存 pending key；全部批次后解析为 Project(SymbolId) 或 External。
7. 每批结束立即 drop 编译器。
8. 全部批次成功后原子发布 ProjectIndex。

失败处理：

- 单批失败：该批文件标记 failed，其他批继续。
- 单文件失败：FileRecord.status=Failed，索引 Partial。
- 不允许旧 generation 冒充新 generation。

## 6. 查询

    definition          -> occurrence/declaration -> SymbolRecord.range
    references          -> occurrences_by_symbol
    rename_workspace    -> occurrences_by_symbol 按文件分组
    prepareRename       -> occurrence range
    documentSymbol      -> by_file declarations
    workspace/symbol    -> by_name + IDEA matcher
    type hierarchy      -> by_supertype
    hover               -> SymbolRecord.detail + Javadoc
    completion          -> FileAnalysis 负责 receiver/expected type；索引负责项目候选和继承成员
    diagnostics         -> FileAnalysis，不进入索引

当前文件 overlay 未入索引时：

- 对该文件做一次 FileAnalysis，得到临时 FileIndex，再查询；
- 不把 FileAnalysis 留在索引里；
- 不允许回退到文本匹配。

## 7. 增量与快照

### 文件变化

- hash/overlay 变化 -> 重新 FileAnalysis。
- 声明集合、签名、supertypes 未变：替换该文件 FileIndex，其他文件不动。
- 声明/签名/supertypes 变化：标记依赖文件 dirty，后台重建。
- 新增/删除/重命名：更新 FileRecord、by_name、by_key、occurrences；删除文件时清除其全部索引项。

### classpath/pom

- classpath_hash 变化 -> 全量重建。
- pom 变化由 didChangeWatchedFiles 触发；不静默复用旧 classpath。

### 快照

IndexStore 使用：

    RwLock<Arc<ProjectIndex>>

- 写入新 generation 后整体替换。
- 查询 clone Arc，看到完整快照。
- 构建期间保留旧快照，状态 Building；查询可返回旧结果但标记状态。

### 构建线程

IndexBuilder 放到工作线程；Workspace 不共享 AnalysisSession。LSP 请求在 Building 时可回退 FileAnalysis。

## 8. 磁盘索引

只序列化 ProjectIndex，不序列化编译器。

    cache dir:
      Windows: %LOCALAPPDATA%/ravac/index/
      Unix: $XDG_CACHE_HOME/ravac/index/ 或 ~/.cache/ravac/index/
      file: <workspace-hash>-v1.bin

文件格式自制紧凑二进制（varint + 字符串表），不新增外部依赖：

    header:
      magic, schema_version, toolchain_hash, classpath_hash,
      workspace_hash, file_count, generation
    body:
      files, symbols, keys, occurrences, by_name/by_supertype 的紧凑表

规则：

- 原子写：临时文件 + rename。
- 启动加载前校验 workspace/classpath/toolchain/file hashes。
- 任一不匹配 -> 丢弃缓存，重建。
- schema 版本不匹配 -> 丢弃。
- 不做跨版本迁移。
- 磁盘索引只作为冷启动加速；损坏时回退重建，不阻塞。

## 9. API 与模块

删除：

- AnalysisHost 旧 trait 及默认方法
- analysis/index.rs
- ProjectCache
- index_built / IndexEntry / index_lookup
- 基于 flat name 的索引键

新增：

    crates/javac-ide-core/src/workspace/mod.rs
    crates/javac-ide-core/src/workspace/model.rs
    crates/javac-ide-core/src/workspace/analysis.rs
    crates/javac-ide-core/src/workspace/index/mod.rs
    crates/javac-ide-core/src/workspace/index/key.rs
    crates/javac-ide-core/src/workspace/index/build.rs
    crates/javac-ide-core/src/workspace/index/store.rs
    crates/javac-ide-core/src/workspace/index/dump.rs
    crates/javac-ide-core/src/workspace/index/query.rs
    crates/javac-ide-core/src/workspace/index/update.rs

javac-analysis-api：

- 保留/重定义纯 DTO：Position/Range/Location/CompletionItem/Diagnostic/Symbol/TextEdit/WorkspaceSymbol。
- 用 WorkspaceService trait 替换 AnalysisHost：

    fn completion(&mut self, file, position) -> Vec<CompletionItem>;
    fn definition(...) -> Option<Location>;
    fn references(...) -> Vec<Location>;
    fn rename_workspace(...) -> Vec<TextDocumentEdit>;
    fn workspace_symbols(&mut self, query: &str) -> Vec<WorkspaceSymbol>;
    fn document_symbols(...);
    fn hover(...);
    fn diagnostics(...);
    fn inlay_hints(...);
    fn code_actions(...);
    fn signature_help(...);
    fn format(...);

- 不提供旧方法兼容；ravac-lsp 同步改。

ravac-lsp：

- 持有 Box<dyn WorkspaceService>
- 增加 workspace/symbol
- didChangeWatchedFiles -> invalidate
- 查询失败/Partial 明确记录

## 10. 测试与门禁

新增 index_spec：

- 同名类、字段、方法、重载键冲突
- 注释/字符串/遮蔽变量不产生错误 occurrence
- 三文件 references/rename/definition
- 接口继承、外部 JDK supertype
- workspace/symbol
- 修改签名/删除文件/新增文件后的增量结果
- overlay 索引
- 磁盘 roundtrip、版本/hash 不匹配回退
- 大项目阈值 Partial

改写：

- navigation_spec 使用 Workspace API
- diagnostics_spec 使用 FileAnalysis
- completion_spec 保持语义，改调用入口
- maven_project 保持
- LSP 冒烟增加 workspace/symbol、跨文件 rename、缓存计数

门禁：

    cargo build -p ravac-lsp
    cargo test -p javac-ide-core --test index_spec
    cargo test -p javac-ide-core --test navigation_spec
    cargo test -p javac-ide-core --test diagnostics_spec
    cargo test -p javac-ide-core --test maven_project
    cargo test -p javac-ide-core --test completion_spec
    py scripts/lsp/run_lsp_tests.py

## 11. 实施顺序

### M1 核心模型

- WorkspaceModel、StringInterner、SymbolKey、ProjectIndex、FileIndex。
- 删除旧 index.rs/IndexEntry。
- IndexBuilder 批编译 + 提取 + 丢弃编译器。
- 不保留 ProjectCache。

### M2 查询切换

- 新 WorkspaceService。
- definition/references/rename/documentSymbol/workspace symbol 全部查索引。
- ravac-lsp 和测试同步改。
- 移除旧 AnalysisHost/IndexEntry/index_lookup。

### M3 生命周期

- FileAnalysis、overlay、单文件增量、依赖 dirty。
- RwLock 快照、后台构建、didChangeWatchedFiles。
- Partial/进度/失败回退。

### M4 持久化与边界

- 磁盘索引 dump/load、hash 校验、原子写。
- max_indexed_files、max_occurrence_bytes、batch_size 降级。
- completion 接入索引成员/项目候选。
- 文档、性能/内存测试。

## 12. 不在本计划

- Gradle、Lombok、注解处理/生成源码
- 语义高亮、code lens、document link
- 多 workspace root 并发
- 全量 jar 索引

## 13. 假设

- 单 workspace，单线程 LSP 请求；IndexBuilder 可工作线程。
- 外部 JDK/classpath 类型由编译器解析。
- 磁盘索引只加速冷启动，正确性始终以源文件 hash/classpath/toolchain 校验为准。
- 第一版索引只覆盖项目源码，不索引全部 jar。

## 14. 风险

- SymbolKey 对重载、泛型、局部变量的稳定性必须由 fixture 锁定。
- 批编译可能按 sourcepath 解析依赖；只索引批内编译单元，避免重复声明。
- 增量声明变化可能影响很多文件；宁可标记 dirty/重建，不返回错误结果。
- 磁盘格式必须版本化；不兼容直接丢弃，不做迁移。

## 15. 实施记录（2026-11-06）

本次按第 11 节顺序完成 M2 遗留清理、M3、M4；门禁与新增基准全部通过。

完成：

- M2 结构清理：`analysis/project_index` 迁移为 `workspace/index/{key,model,build,store,query,update,extract,dump}.rs`；`AnalysisHost` 删除并由 `WorkspaceService` 取代；prepareRename 改走索引 occurrence；`completion/mod.rs`（2400 行）与 `navigation/mod.rs`（550 行）按职责拆分，模块根文件只保留声明。
- M3：`Workspace` 持有 `IndexStore`（`RwLock<Arc<ProjectIndex>>`）与最多 4 项 `FileAnalysis` LRU；overlay 文本参与索引与查询；磁盘文件用 mtime/len + hash 检测变更；`workspace/didChangeWatchedFiles` 与 `invalidate` 接入；Partial 状态显式化，索引查询在 Partial 时拒绝。
- M3 增量：`index/extract.rs` 单文件提取声明/occurrence/supertype/Javadoc；`index/update.rs::apply_batch` 统一处理 Update/Add/Delete 三类任务，按 `(flat name, kind, signature)` 复用 `SymbolId` 并局部替换 `FileIndex`。声明/签名/supertype 变化时按 `ProjectIndex::dependent_closure` 重新提取该文件与反向依赖闭包；新增文件分配新 `FileId` 与占位记录；删除文件标记 `FileStatus::Deleted`，清除其符号与所有指向它的 occurrence。
- M3 快照/构建线程：`ensure_index` 不再 join 后台构建。构建进行中查询继续使用旧快照（`Workspace::is_building` 与 `WorkspaceService::is_index_building` 可观测，LSP `ravac/indexStats` 返回 `building` 与 `generation`）；无可用快照时索引查询回退 `FileAnalysis`。构建完成后由 `poll_build` 原子发布新快照（`IndexStore::publish`）。`Workspace::wait_for_index` 供需要新快照的调用方显式等待（测试、初始化）。
- M4：`dump.rs` 版本化紧凑二进制（varint + 字符串表）、原子写与加载校验（fingerprint + 逐文件 hash）；`IndexLimits` 支持 `max_indexed_files`/`max_occurrence_bytes` 降级；completion 项目候选改由索引提供。
- M4 Javadoc：批编译后按源文件关联 doc comment 表，`SymbolRecord.javadoc` 随索引持久化；`Workspace::hover` 优先返回索引 detail + Javadoc。

测试：

- `tests/index_spec.rs`（14 项）：声明/occurrence/supertype、重载键、注释/字符串/遮蔽、跨包同名类、接口/外部 supertype、workspace/symbol、增删改文件、overlay、磁盘 roundtrip 与 hash 不匹配回退、文件数阈值 Partial、Javadoc 提取、索引 Javadoc hover、Javadoc 磁盘 roundtrip。
- `tests/index_incremental_spec.rs`（5 项）：声明变化经反向依赖闭包增量重建、新增文件增量、删除文件清除引用方 occurrence 且不触发全量、闭包外引用拒绝批次、闭包内不可编译时回退全量。
- `tests/index_snapshot_spec.rs`（2 项）：构建进行中查询返回旧快照且不阻塞；无快照时 `definition` 回退 `FileAnalysis`。
- `tests/index_perf_spec.rs`（5 项）：120 文件构建/查询时间预算、索引内存上限、方法体编辑走增量、声明改名 + 引用更新走增量、反向依赖表方向与闭包。
- `navigation_spec`、`diagnostics_spec`、`completion_spec`、`completion_project_spec`、`inlay_hints` 改走 `WorkspaceService`/`FileAnalysis`，行为保持。
- LSP 冒烟在 initialize 后轮询 `ravac/indexStats` 直到首个快照，并覆盖 workspace/symbol、跨文件 rename、index generation，17/17 通过。

偏差（显式记录）：

- 索引符号类型名保留 `IndexSymbolKind`，避免与编译器 `code::symbol::SymbolKind` 冲突。
- `ProjectIndex` 新增 `workspace: WorkspaceFingerprint` 与 `dependents`；`FileRecord` 用 `status: FileStatus` 替代 `failed: bool`。
- `IndexStatus::is_ready` 只认 `Ready`；Partial 时不返回索引结果，回退单文件分析。
- 增量边界：内容变化、声明变化（依赖闭包内）、新增、删除均走增量；闭包外仍有引用、闭包内文件提取失败、多个 changed 根文件相互引用时回退后台全量重建。
- 删除文件时，其他文件指向被删声明的 occurrence 直接清除：这些源文件已无法编译，全量重建只会得到 `Partial` 索引。
- 查询不等待后台构建；首次索引可用性由调用方通过 `is_index_building` / `ravac/indexStats` 轮询或 `wait_for_index` 等待。LSP 冒烟脚本按此轮询。
- 被移除声明保留 `SymbolId` 槽位（tombstone），本轮不做回收。
- hover 的 Javadoc 走索引；索引未命中时回退 `FileAnalysis`。
- `WorkspaceService` 除第 9 节列出的方法外，保留 LSP 必需的 type/call hierarchy、implementation、format、symbols/symbol_at、invalidate、configuration/reconfigure、index_generation、is_index_building。
- `set_build_delay_for_tests` / `clear_index_for_tests` 是 `#[doc(hidden)]` 的测试钩子，生产路径不设置。
