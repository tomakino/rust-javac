# 官方/本地文件与方法覆盖对比（A3l 准备）

Status: reference（只读分析，不参与构建）；A3l 裁决 B 回退 open addressing 中，方法数字是 chaining 工作树快照，待回退后复跑。
Date: 2026-10-28
Scripts: target/tools/compare_api.py、target/tools/compare_api_origin.py（临时）
Data: target/api-diff.csv、target/api-origin-diff.csv

## 1. 口径

- 官方：reference/jdk17u/src/jdk.compiler/share/classes/com/sun/tools/javac/**/*.java，共 **189** 个文件。
- 本地：crates/javac-core/src/**/*.rs，共 **189** 个文件（另有其他 crate 的 rs，对应 javax.tools/com.sun.source 等）。
- 文件映射：优先用本地文件头 // Origin: <path> @ 982d53c；不是按文件名猜。
- 方法计数：
  - Java：源文件中的方法/构造器声明（正则；包含内部类方法、接口方法；会含少量噪音）；
  - Rust：fn 声明（排除 mod tests）；包含适配 helper/辅助函数。
  - **两者不是一一等价**，只用于发现数量级差异，不能直接判定“缺失”。

## 2. 文件级结果

~~~
ref_files=189
local_core_rs=189
origin_mapped=142
missing_ref=47
multi_mapped=1
extra_origin=251（其他 crate 的 com.sun.source/java.* 等）
~~~

- 142 个官方文件有本地 // Origin: 对应；
- 47 个官方文件没有 // Origin: 对应；
- 1 个官方文件对应多个本地文件：platform/JDKPlatformProvider.java → ct_sym_java_file_manager.rs + jdk_platform_provider.rs。

### 47 个无 Origin 的官方文件（需逐个归类）

~~~
Main.java
api/BasicJavacTask.java
api/ClientCodeWrapper.java
api/Entity.java
api/JavacScope.java
api/JavacTaskImpl.java
api/JavacTaskPool.java
api/JavacTool.java
api/JavacTrees.java
api/MultiTaskListener.java
api/WrappingJavaFileManager.java
code/AnnoConstruct.java
code/MissingInfoHandler.java
comp/AttrContextEnv.java
comp/InferenceContext.java
comp/Todo.java
jvm/CRTFlags.java
jvm/CRTable.java
jvm/ClassWriter.java
jvm/Code.java
jvm/Gen.java
jvm/Items.java
jvm/PoolWriter.java
jvm/StringConcat.java
jvm/UninitializedType.java
launcher/Main.java
main/Arguments.java
main/CommandLine.java
main/DelegatingJavaFileManager.java
main/JavaCompiler.java
main/JavacToolProvider.java
main/Main.java
model/FilteredMemberList.java
model/JavacElements.java
model/JavacTypes.java
package-info.java
platform/package-info.java
processing/AnnotationProcessingError.java
processing/JavacFiler.java
processing/JavacMessager.java
processing/JavacProcessingEnvironment.java
processing/JavacRoundEnvironment.java
processing/PrintingProcessor.java
processing/ServiceProxy.java
util/DefinedBy.java
util/MatchingUtils.java
util/RichDiagnosticFormatter.java
~~~

已知分类（抽样核对）：

- **真实未 1:1 移植**：main/JavaCompiler.java。main/mini_compiler.rs 文件头写的是
  // Source: ... (minimal wiring subset)，并明确写“1:1 JavaCompiler.java port is deferred”。
  这不是命名问题，是 A3k/A3l 范围内的 adapter。
- **用 // Source: 而非 // Origin:**：jvm/Gen.java（jvm/codegen.rs）、jvm/Items.java（jvm/items.rs）等；
  说明文件头不符合 §35，需审计是 1:1 移植还是 adapter。
- **可能在其他 crate / 未移植**：api/*、processing/*、main/*、model/*、launcher/*；
  需要逐个查 crates/javac-tools、javac-source 等是否有 Origin 或实现。

## 3. 方法级结果（仅 142 个已映射文件）

~~~
java_methods=8821
rust_fn=7339
~~~

重点文件：

| 文件 | Java 方法 | Rust fn | 说明 |
|---|---:|---:|---|
| tree/JCTree | 758 | 81 | Java 每个节点类很多方法；Rust 用 enum/struct 数据，方法数不可直接比 |
| comp/LambdaToMethod | 130 | 501 | Rust 适配/helper 多 |
| code/Symbol | 341 | 166 | 需按方法名核对 |
| tree/DCTree | 174 | 12 | 同上，数据类差异 |
| comp/Flow | 208 | 368 | Rust 辅助多 |
| comp/Resolve | 382 | 266 | **A3l 重点** |
| code/Types | 450 | 338 | 需按方法名核对 |
| jvm/ClassReader | 245 | 118 | 需按方法名核对 |
| file/Locations | 195 | 82 | 需按方法名核对 |
| code/Scope | 137 | 45 | **A3l 重点** |
| comp/TypeEnter | 96 | 113 | **A3l 重点** |
| comp/Attr | 285 | 310 | A3l 相关 |
| comp/Enter | 30 | 61 | A3l 相关 |
| comp/MemberEnter | 26 | 49 | A3l 相关 |

脚本给出的方法名差异示例（含内部类/构造器噪音，需人工过滤）：

- Scope：import_all/finalize_scope/import_by_name/import_type/single_entry_scope/...
- TypeEnter：imports_phase/hierarchy_phase/members_phase/header_phase/permits_phase/...
- Resolve：find_field/basic_lookup_helper/filter_candidates/...
- Enter：enter/visit_package_def/...
- MemberEnter：visit_ident/visit_select/...

## 4. 结论

1. 文件数“189 vs 189”只是表面齐平；按 Origin 只有 **142** 个官方文件真正映射，**47 个没有 Origin**，其中至少 main/JavaCompiler 明确是 deferred minimal subset，不是 1:1。
2. 方法数差异（8821 vs 7339）不能直接当缺失数：Java 含内部类/构造器，Rust 含适配 helper；但 Scope/Resolve/Symbol/Types/ClassReader/Locations 的量级差异需要按方法名逐项核对。
3. 当前脚本是粗粒度审计，下一步应：
   - 把 47 个文件逐个归类：真实缺失 / 改名 / 合并 / 其他 crate / 文件头不合规；
   - 对 Scope/TypeEnter/Resolve 生成方法名级清单（排除内部类、构造器、纯数据类），作为 A3l 审核 checklist；
   - 把 // Source: 头统一改为 §35 要求的 // Origin:，或登记为 adapter。

## 5. 边界

- 脚本不修改源码；数据来自当前工作树，执行者改动期间可能有波动。
- 方法正则不是编译器级解析，数字只用于定位差异，不作为验收断言。
