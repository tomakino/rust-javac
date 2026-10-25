# Class-file fixtures generator

Development-time only: compiles the Java samples below with the baseline JDK 17
javac and stores the resulting .class files under
`crates/javac-core/tests/data/fixtures/` for the ClassReader tests. Tests never
invoke a JVM at runtime; they read the checked-in .class bytes.

Generation command (baseline: Eclipse Adoptium JDK 17.0.19, `javac -version` =
javac 17.0.19):

```powershell
pwsh tools/gen-class-fixtures/gen.ps1
```

The fixtures are simple original samples (no third-party code); they are
licensed under GPLv2+CE, matching the repository's reference baseline.
