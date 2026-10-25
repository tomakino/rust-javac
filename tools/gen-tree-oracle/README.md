# tree/token oracle 生成工具(开发期)

这些工具用基线 JDK 17 的 javac 内部 API 生成 oracle 数据,数据入库到
crates/javac-tests/data/,Rust 测试只读数据文件(运行期零 JVM 依赖)。

## GenTreeOracle(Pretty 打印对照数据)

    javac --add-exports jdk.compiler/com.sun.tools.javac.api=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.tree=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.util=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.file=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.code=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.main=ALL-UNNAMED \
          -d target/gen-tree-oracle-classes tools/gen-tree-oracle/GenTreeOracle.java

    java --add-exports jdk.compiler/com.sun.tools.javac.api=ALL-UNNAMED \
         --add-exports jdk.compiler/com.sun.tools.javac.tree=ALL-UNNAMED \
         --add-exports jdk.compiler/com.sun.tools.javac.util=ALL-UNNAMED \
         --add-exports jdk.compiler/com.sun.tools.javac.file=ALL-UNNAMED \
         --add-exports jdk.compiler/com.sun.tools.javac.code=ALL-UNNAMED \
         --add-exports jdk.compiler/com.sun.tools.javac.main=ALL-UNNAMED \
         -cp target/gen-tree-oracle-classes GenTreeOracle \
         reference/jdk17u/test/langtools/tools/javac \
         crates/javac-tests/data/tree-oracle.txt 40

输出记录 JDK 版本、基线 commit、生成命令与打印模式;每个记录形如
=== FILE: <相对路径> ===  ...Pretty 输出... === END ===。

## GenTokenOracle(token 流对照数据)

    javac --add-exports jdk.compiler/com.sun.tools.javac.parser=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.util=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.file=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.code=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.main=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.resources=ALL-UNNAMED \
          -d target/gen-token-oracle-classes tools/gen-token-oracle/GenTokenOracle.java

    java --add-exports ...(同上) -cp target/gen-token-oracle-classes GenTokenOracle \
         reference/jdk17u/test/langtools/tools/javac \
         crates/javac-tests/data/token-oracle.txt 10

输出包含 TokenKind 全量清单(118 项)、all-tokens 样例、语料抽样与错误 token 样例,
并记录 kind 覆盖统计与不可达 token(正常扫描不会产生的 kind)。
