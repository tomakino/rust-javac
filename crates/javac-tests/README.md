# javac-tests(差分测试基座)

用途锚点:`docs/PLAN.md §7`(验证与差分体系:oracle = 本机 javac 17.0.19)。
本 crate 是测试基础设施(适配工具),不是 javac 源码的移植,因此不写移植来源锚点、不写版权头。

## 组成

- `src/javac.rs`:定位本机 javac 17,在全新临时目录编译源文件,捕获原始
  stdout/stderr 字节、退出码、生成的 class 清单与路径替换上下文;固定
  `-encoding UTF-8`、`-J-Dfile.encoding=UTF-8`、`-J-Duser.language=en`、
  `-J-Duser.country=US`。
- `src/normalize.rs`:固定归一化规则(绝对路径 -> 占位符、CRLF -> LF、反斜杠 -> '/'、
  JVM 噪声行剥离)与差分对比(`compare`/`DiffReport`)。
- `src/corpus.rs`:langtools javac 测试语料抽取(jtreg 头解析、分类、索引、oracle 基线)。
- `src/oracle_data.rs`、`src/diagnostics_oracle.rs`:入库 oracle 数据(tree/token/
  diagnostics)的结构与完整性测试。测试只读 `data/` 下的文本,运行期零 JVM 依赖。
- `src/diagnostics_data.rs`:诊断 oracle 数据与 builtin 用例源码文件的解析/还原(供测试、
  差分运行器、数据生成器共用)。
- `src/diff.rs`:差分运行器核心(目标命令、oracle 三态、用例加载、比较与报告)。
- `src/bin/gen_diagnostics_oracle.rs`:诊断 oracle 数据生成器(开发期工具)。
- `src/bin/diff_runner.rs`:差分运行器命令行。
- `src/bin/gen_corpus_baselines.rs`:corpus 基线生成命令行(开发期工具)。

## 诊断 oracle 数据(data/diagnostics-oracle.txt)

内容:每个用例分别用默认诊断格式与 `-XDrawDiagnostics` 运行 javac,记录退出码、
规范化后的 stdout/stderr、生成的 class 清单、draw 模式诊断 key 列表。默认格式保留
`path:line: error: msg` 文本与源码行/`^` 定位行;draw 格式保留诊断 key、位置
(`line:col`)与参数,供 P1(parser 诊断)与 P4(语义诊断)逐字对照。

用例来源:

- `builtin:` 项目自建的失败用例(源码入库于 `data/diagnostics-builtin-cases.txt`,
  35 个),精确控制选项组合:语法错误各形态、类型错误、`-Xlint`/`-Werror`/note、
  `-XDshould-stop.at=PARSE`、`-Xmaxerrs`、多文件诊断、`--release` 版本错误、
  preview API,以及成功编译的负对照。
- `corpus:` `reference/jdk17u/test/langtools/tools/javac/diags/examples`(上游按诊断
  key 组织的示例集合,README.examples.txt 说明其约定)。生成器解析示例文件中的
  `// options:` 与 `// key:` 元信息。

重新生成(需要本机 JDK 17 与 reference 语料;数据入库后运行期不再需要 JVM):

    cargo run -p javac-tests --bin gen_diagnostics_oracle

可选参数:`[OUTPUT]`(默认 `crates/javac-tests/data/diagnostics-oracle.txt`)、
`--corpus-root DIR`(默认 `reference/jdk17u/test/langtools/tools/javac/diags/examples`)、
`--builtin-cases PATH`(默认 `crates/javac-tests/data/diagnostics-builtin-cases.txt`,
即入库的 builtin 用例源码,生成器从中读取全部 35 个自建用例)。
找不到 JDK 17 javac 时生成器以退出码 2 失败,不会写出空数据。语料缺失的示例会跳过并记录
在数据头的 `# corpus-missing:` 行。

## 数据格式

    # diagnostics oracle data ...
    # generated-by: crates/javac-tests/src/bin/gen_diagnostics_oracle.rs
    # jdk-version: 17.0.19
    # baseline: 982d53c (jdk-17.0.19-ga)
    # total-cases: N
    === CASE: <name> ===
    # source: builtin:<name> | corpus:<relative path>
    # args: - | <javac options, space separated>
    # corpus-keys: <example declared keys, comma separated>   (仅 corpus 用例)
    # exit-code: N
    # draw-exit-code: N
    # draw-diagnostics: N
    # draw-keys: - | <comma separated compiler resource keys>
    # classes: - | <class files>
    --- stdout-default ---
    --- stderr-default ---
    --- stdout-draw ---
    --- stderr-draw ---
    === END ===

所有路径已归一化为 `<src>`/`<out>`/`<tmp>`/`<jdk>` 占位符,行尾统一为 LF。

## 语料来源与许可

`reference/jdk17u/test/langtools/tools/javac` 来自 OpenJDK jdk17u(基线
`982d53c`,jdk-17.0.19-ga),许可 GPLv2+Classpath Exception,与项目一致;
`reference/` 不入库,数据文件只保存诊断输出文本。

`data/diagnostics-builtin-cases.txt` 与 `data/diagnostics-oracle.txt` 中的 builtin
用例是项目自建的 Java 片段(非从 JDK 复制),随项目以同一许可证分发;corpus 部分只保存
参考 javac 对上游示例的诊断输出文本。

## 差分运行器(diff_runner)

P9 差分基础设施:用目标编译器命令对语料/诊断 oracle 用例重跑编译,与参考 oracle 比较
退出码、规范化 stdout/stderr 与生成的 class 清单。

    cargo run -p javac-tests --bin diff_runner -- --target "C:/path/to/rust-javac.exe"

输入:

- 目标编译器命令:可执行路径 + 若干 --target-arg 固定参数(按 javac 兼容命令行调用:
  固定参数、用例参数、-encoding UTF-8、-d 输出目录、源文件绝对路径);
- oracle 三种来源:
  * --oracle diagnostics(默认):入库的 data/diagnostics-oracle.txt(运行期零 JVM;
    默认重建全部 80 个用例 = 45 corpus + 35 builtin,builtin 源码来自入库文件);
  * --oracle baselines:target/javac-corpus/baselines 下由参考 javac 生成的基线
    (基线带 source_fnv 指纹,语料变化导致的过期基线会自动跳过而不是误报);
  * --oracle javac:实时运行参考 javac 17(开发期/自测);
- 用例来源:--source diagnostics(默认)或 --source corpus(langtools compile-only 用例);
- 过滤/抽样:--filter SUBSTR、--limit N、--list;
- 诊断模式:--mode default(默认)或 --mode draw(--source diagnostics,对比 -XDrawDiagnostics);
- 其它:--diagnostics PATH、--builtin-cases PATH、--baselines DIR、--corpus-root DIR、
  --max-details N(默认 10)。

输出:source/oracle/mode 摘要、cases/run/passed/failed/skipped 统计、限量差异详情
(用例名、差异类别、期望/实际行),以及 result: PASS 或 FAIL。

退出码:0 全部比较通过;1 存在差分失败;2 环境缺失(目标不可运行、oracle 数据/基线
不存在、需要 JDK 17 但找不到、参数/数据配置错误)。缺 JDK 17 时打印 skipped 并以 2 退出。

自测(cargo test -p javac-tests 的 diff 模块,缺 JDK 17/语料时自动跳过):

- 参考 javac 自身作为目标(实时 oracle)应全过;
- 故意输出错误的目标脚本应被检出并给出差异;
- 参考 javac 重跑入库 diagnostics 数据应全过;
- 参考 javac 重跑新生成的 corpus 基线应全过;
- 缺 JDK 17、无 oracle 记录、过期基线指纹 → 跳过;环境缺失映射退出码 2。

## 生成 corpus 基线(gen_corpus_baselines)

用参考 javac 17 为 langtools 语料的 compile-only 用例生成差分基线(退出码、规范化
stdout/stderr、class 清单、源输入指纹),写入 target/ 下(不入库):

    cargo run -p javac-tests --bin gen_corpus_baselines -- --limit 50

参数:

- --corpus-root DIR:语料根(默认 reference/jdk17u/test/langtools/tools/javac);
- --output DIR:基线输出根(默认 target/javac-corpus;基线写到 <output>/baselines/);
- --filter SUBSTR:只处理相对路径包含 SUBSTR 的用例,用于按目录分批;
- --limit N:过滤后最多处理 N 个 compile-only 用例;
- --refresh:忽略源指纹强制重生成(默认增量:缺失或源文件/选项变化的用例才重生成)。

环境:需要本机 JDK 17;找不到 JDK 17 或语料目录时退出码 2 且不写空基线。退出码:
0 完成(允许 unsupported/up-to-date),1 有生成失败用例,2 环境缺失。

耗时参考(Windows/JDK 17.0.19):每个用例一次 javac 启动,compile-only 全量约 1955
个用例;建议分批执行(按 --filter 匹配相对路径子串,例如 parser/、lambda/、types/、flow/),
每批观察统计后再跑下一批。

## 全量 corpus 差分(P9 用法)

1. 生成基线(可分批;首次全量,后续增量):

       cargo run -p javac-tests --bin gen_corpus_baselines -- --filter parser/
       cargo run -p javac-tests --bin gen_corpus_baselines -- --filter lambda/
       cargo run -p javac-tests --bin gen_corpus_baselines            # 补齐剩余全部

2. 用目标编译器跑差分(与基线比较;过期基线按 source_fnv 自动跳过,不会误报):

       cargo run -p javac-tests --bin diff_runner -- \
           --target "path/to/rust-javac" --source corpus --oracle baselines

   分批/抽样:追加 --filter SUBSTR 或 --limit N;--list 可先确认选中的用例。

3. 也可以不用基线,直接以实时参考 javac 为 oracle(每个用例多一次 javac 启动,适合
   抽查少数用例;缺 JDK 17 时退出码 2):

       cargo run -p javac-tests --bin diff_runner -- \
           --target "path/to/rust-javac" --source corpus --oracle javac --limit 20

4. 结果与退出码:统计行 cases/run/passed/failed/skipped,失败详情列出用例名、差异类别
   (exit-code/stdout/stderr/classes)与期望/实际行;0 全过、1 有差异、2 环境缺失。
   P9 全量回归建议按目录分批记录通过率,并对失败用例单独复跑(--filter <用例名>)。

默认诊断数据差分(无需语料与基线,运行期零 JVM;需目标命令):

    cargo run -p javac-tests --bin diff_runner -- --target "path/to/rust-javac"

该命令默认 source=diagnostics、oracle=diagnostics,重建全部 80 个诊断用例。
