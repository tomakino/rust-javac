# AGENTS.md — 协作规则

适用于本仓库的所有 AI agent 与贡献者。

## 1. 先定义目标

用一句话写清目标，并列出三条不可妥协的约束：安全、最小改动、一致性。目标不清楚先问，不要动手。

## 2. 摸清全部落点

任何改动前，用 grep 找出这个概念在代码、配置、脚本、文档、测试、示例中的所有位置，列成清单逐项处理。禁止只改被点名的一处。

## 3. 单一标准（SSOT）

一个概念只能有一个事实来源。出现第二种形式（兼容层、别名、双份布局、双命令）时，先问能否合并；确实无法合并才允许回退，并写明原因。

## 4. 证据门槛

「支持」和「完成」必须有验证。未验证的工作标注「未验证」或删除。按成本递增验证：编译 → 冒烟 → 定向测试 → 全量；跳级需明确批准。

## 5. 根因与同类

先复现再修，修根因不修症状。改一处时，检查同类位置并在同一批修复。

## 6. 影响报告

每次交付附三行：改了什么、可能影响什么、哪些没验证。

## 7. 守住边界

只完成被要求的动作，不扩大范围。长任务、全量测试、发布操作必须有明确指令。

## 8. 日常开发与快速验证

日常改动按成本递增验证，默认做到 L1 就停；全量 L3 与交叉构建只在明确要求或合并前执行。

```powershell
cargo check -p javac-core              # 类型检查，最快
cargo build -p rust-javac              # 产出 target\debug\ravac.exe（build.rs 链接本地平台数据）
cargo test -p javac-core --lib         # L0：单元测试
py scripts/gate.py --run               # 按改动路径自动选级别并执行
py scripts/gate.py --files a.rs b.rs   # 只看指定文件
py scripts/gate.py --level L2 --run    # 强制级别
```

构建依赖：`rust-javac` 用 `mimalloc` 接管全局分配器（`java.base` 全量从 29.5s 降到 23.3s）。`mimalloc` 是 C 库，GNU 目标需要一个 MinGW C 编译器：

```powershell
winget install MartinStorsjo.LLVM-MinGW.UCRT
$env:CC_x86_64_pc_windows_gnu = "<安装目录>\llvm-mingw-*\bin\clang.exe"
$env:AR_x86_64_pc_windows_gnu = "<安装目录>\llvm-mingw-*\bin\llvm-ar.exe"
```

不要把那个 `bin` 目录加进 `PATH`：它自带的 `x86_64-w64-mingw32-gcc` 别名会顶掉 Rust 的 linker，报 `unable to find library -lgcc`，而错误信息指不到真正原因。

冒烟：`target\debug\ravac.exe -d out Hello.java`；或跑单个语料检查器，如 `py scripts/check_gson.py --build`（退出码 0 一致、1 有差异、2 环境缺失）。

平台数据：不自动提交，也不加入 `.gitignore`；生成后保持 untracked（出现在 `git status`），提交这两个二进制必须由用户明确要求。首次构建前用 `cargo xtask gen-platform --out internal\platform\17.0.19+10\modules` 和 `cargo xtask gen-ct-sym --out internal\platform\17.0.19+10\ct.sym` 生成；`--check` 校验本地文件。

级别：L0 单测；L1 = L0 + repro + Gson（约 15s）；L2 = L1 + jdk.compiler + Guava + OkHttp + RxJava + Spring（约 85s）；L3 = 全部测试、全部语料与模块检查。改 parser/CLI 用 L1，改 attr/flow/lower/codegen 用 L2，纯文档改动无需测试。gate 接受已登记的已知失败，出现新的失败名即视为失败。

## 项目规则

- 只声明已验证的目标平台：`windows-amd64`、`linux-amd64`、`linux-arm64`、`android-arm64`。
- Linux 静态链接（musl），使用 Rust 自带的 `rust-lld`，不用 zig。
- 只有一种运行时布局：`modules` 与 `ct.sym` 放在可执行文件旁（`RAVAC_PLATFORM_DIR` 可覆盖）。
- 测试数据在 `internal/test` 下；禁止手改 `expected/` 基线。
- 提交前运行 `git status`，确认没有意外文件；禁止提交 `dist/`、`target/`。
