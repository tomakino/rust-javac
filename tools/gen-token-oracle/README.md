# token oracle generator (development time)

Generates token-stream reference data with the baseline JDK 17 `javac` internal
Scanner and checks it in as `crates/javac-tests/data/token-oracle.txt`; the Rust
tests only read that data file (no JVM at test runtime).

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

The data contains: the full TokenKind list (118 entries, with token name, text
and internal tag), an all-tokens sample, error-token samples (ERROR, unclosed
string, unclosed character, illegal unicode escape), token streams sampled from
the corpus (kind/pos/endPos/name/stringVal/radix), and observed/missing kind
statistics.
