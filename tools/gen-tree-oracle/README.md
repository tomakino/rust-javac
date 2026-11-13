# tree/token oracle generator (development time)

These tools generate oracle data with the baseline JDK 17 `javac` internal APIs
and check it in under `crates/javac-tests/data/`; the Rust tests only read the
data files (no JVM at test runtime).

## GenTreeOracle (pretty-print reference data)

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

The output records the JDK version, baseline commit, generation command and
print mode; each record has the form
`=== FILE: <relative path> ===  ...pretty output...  === END ===`.

## GenTokenOracle (token-stream reference data)

    javac --add-exports jdk.compiler/com.sun.tools.javac.parser=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.util=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.file=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.code=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.main=ALL-UNNAMED \
          --add-exports jdk.compiler/com.sun.tools.javac.resources=ALL-UNNAMED \
          -d target/gen-token-oracle-classes tools/gen-token-oracle/GenTokenOracle.java

    java --add-exports ...(as above) -cp target/gen-token-oracle-classes GenTokenOracle \
         reference/jdk17u/test/langtools/tools/javac \
         crates/javac-tests/data/token-oracle.txt 10

The output contains the full TokenKind list (118 entries), the all-tokens
sample, corpus samples and error-token samples, plus kind coverage statistics
and unreachable tokens (kinds normal scanning never produces).
