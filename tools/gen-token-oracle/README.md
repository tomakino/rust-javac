# token oracle 生成工具(开发期)

用基线 JDK 17 的 javac 内部 Scanner 生成 token 流对照数据,入库到
crates/javac-tests/data/token-oracle.txt;Rust 测试只读数据文件(运行期零 JVM)。

    javac --add-exports jdk.compiler/com.sun.tools.javac.parser=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.util=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.file=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.code=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.main=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.resources=ALL-UNNAMED \
          -d target/gen-token-oracle-classes tools/gen-token-oracle/GenTokenOracle.java

    java --add-exports jdk.compiler/com.sun.tools.javac.parser=ALL-UNNAMED \
         --add-exports jdk.compiler/com.sun.tools.javac.util=ALL-UNNAMED \
         --add-exports jdk.compiler/com.sun.tools.javac.file=ALL-UNNAMED \
         --add-exports jdk.compiler/com.sun.tools.javac.code=ALL-UNNAMED \
         --add-exports jdk.compiler/com.sun.tools.javac.main=ALL-UNNAMED \
         --add-exports jdk.compiler/com.sun.tools.javac.resources=ALL-UNNAMED \
         -cp target/gen-token-oracle-classes GenTokenOracle \
         crates/javac-tests/data/token-oracle.txt 5 \
         reference/jdk17u/test/langtools/tools/javac tools/gen-tree-oracle/samples

数据包含:TokenKind 全量清单(118 项,含量名/文本/内部 tag)、all-tokens 样例、
错误 token 样例(ERROR/未闭合字符串/未闭合字符/非法 unicode 转义)、语料抽样文件
的 token 流(kind/pos/endPos/name/stringVal/radix),以及 observed/missing kind 统计。
