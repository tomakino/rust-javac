# 官方 javac 文件 ↔ 本地文件映射

Status: reference（只读生成，不参与构建）；文件映射不受 Scope 表结构影响；47 个无 Origin 文件仍需逐个归类。

Date: 2026-10-28

Data: target/file-map.csv（完整 189 行）

## 统计

~~~
ref_files=189 local_rs=499
INFERRED=3
MISSING=33
ORIGIN=142
SOURCE=11
~~~

## 状态说明

- ORIGIN：本地文件头 // Origin: 明确映射。
- SOURCE：本地文件头是 // Source:，需要改为 Origin 或登记 adapter。
- ALIAS：按特殊别名匹配（Gen→codegen、JavaCompiler→mini_compiler 等），必须人工确认 1:1/adapter。
- INFERRED：按文件名 stem 匹配，无 Origin/Source，需补头或确认。
- MULTI：多个候选。
- MISSING：无本地文件。

## MISSING

| 官方文件 | 本地文件 | 备注 |
|---|---|---|
| api/BasicJavacTask.java | - | no local file by Origin/Source/stem/alias |
| api/ClientCodeWrapper.java | - | no local file by Origin/Source/stem/alias |
| api/Entity.java | - | no local file by Origin/Source/stem/alias |
| api/JavacScope.java | - | no local file by Origin/Source/stem/alias |
| api/JavacTaskImpl.java | - | no local file by Origin/Source/stem/alias |
| api/JavacTaskPool.java | - | no local file by Origin/Source/stem/alias |
| api/JavacTool.java | - | no local file by Origin/Source/stem/alias |
| api/JavacTrees.java | - | no local file by Origin/Source/stem/alias |
| api/MultiTaskListener.java | - | no local file by Origin/Source/stem/alias |
| api/WrappingJavaFileManager.java | - | no local file by Origin/Source/stem/alias |
| code/AnnoConstruct.java | - | no local file by Origin/Source/stem/alias |
| code/MissingInfoHandler.java | - | no local file by Origin/Source/stem/alias |
| comp/AttrContextEnv.java | - | no local file by Origin/Source/stem/alias |
| comp/InferenceContext.java | - | no local file by Origin/Source/stem/alias |
| main/Arguments.java | - | no local file by Origin/Source/stem/alias |
| main/CommandLine.java | - | no local file by Origin/Source/stem/alias |
| main/DelegatingJavaFileManager.java | - | no local file by Origin/Source/stem/alias |
| main/JavacToolProvider.java | - | no local file by Origin/Source/stem/alias |
| model/FilteredMemberList.java | - | no local file by Origin/Source/stem/alias |
| model/JavacElements.java | - | no local file by Origin/Source/stem/alias |
| model/JavacTypes.java | - | no local file by Origin/Source/stem/alias |
| package-info.java | - | no local file by Origin/Source/stem/alias |
| platform/package-info.java | - | no local file by Origin/Source/stem/alias |
| processing/AnnotationProcessingError.java | - | no local file by Origin/Source/stem/alias |
| processing/JavacFiler.java | - | no local file by Origin/Source/stem/alias |
| processing/JavacMessager.java | - | no local file by Origin/Source/stem/alias |
| processing/JavacProcessingEnvironment.java | - | no local file by Origin/Source/stem/alias |
| processing/JavacRoundEnvironment.java | - | no local file by Origin/Source/stem/alias |
| processing/PrintingProcessor.java | - | no local file by Origin/Source/stem/alias |
| processing/ServiceProxy.java | - | no local file by Origin/Source/stem/alias |
| util/DefinedBy.java | - | no local file by Origin/Source/stem/alias |
| util/MatchingUtils.java | - | no local file by Origin/Source/stem/alias |
| util/RichDiagnosticFormatter.java | - | no local file by Origin/Source/stem/alias |

## SOURCE

| 官方文件 | 本地文件 | 备注 |
|---|---|---|
| comp/Todo.java | crates/javac-core/src/comp/todo.rs | file header Source (must be Origin) |
| jvm/CRTFlags.java | crates/javac-core/src/jvm/crt_flags.rs | file header Source (must be Origin) |
| jvm/CRTable.java | crates/javac-core/src/jvm/cr_table.rs | file header Source (must be Origin) |
| jvm/ClassWriter.java | crates/javac-core/src/jvm/class_writer.rs | file header Source (must be Origin) |
| jvm/Code.java | crates/javac-core/src/jvm/code.rs|crates/javac-core/src/jvm/stack_map.rs | file header Source (must be Origin) |
| jvm/Gen.java | crates/javac-core/src/jvm/codegen.rs | file header Source (must be Origin) |
| jvm/Items.java | crates/javac-core/src/jvm/items.rs | file header Source (must be Origin) |
| jvm/PoolWriter.java | crates/javac-core/src/jvm/pool_writer.rs | file header Source (must be Origin) |
| jvm/StringConcat.java | crates/javac-core/src/jvm/string_concat.rs | file header Source (must be Origin) |
| jvm/UninitializedType.java | crates/javac-core/src/jvm/uninitialized_type.rs | file header Source (must be Origin) |
| main/JavaCompiler.java | crates/javac-core/src/main/mini_compiler.rs | file header Source (must be Origin) |

## ALIAS

| 官方文件 | 本地文件 | 备注 |
|---|---|---|

## INFERRED

| 官方文件 | 本地文件 | 备注 |
|---|---|---|
| Main.java | crates/rust-javac/src/main.rs | same stem, no Origin/Source |
| launcher/Main.java | crates/rust-javac/src/main.rs | same stem, no Origin/Source |
| main/Main.java | crates/rust-javac/src/main.rs | same stem, no Origin/Source |

## MULTI

| 官方文件 | 本地文件 | 备注 |
|---|---|---|
