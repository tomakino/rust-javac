# Repository layout

```text
crates/                 compiler and support crates
  javac-core/           parser, enter, attr, flow, lower, code generation, diagnostics
  rust-javac/           CLI binary, robustness and examples_parity tests
  javac-tools/          javax.tools-style file manager API
  javac-source/         com.sun.source tree API
  javac-unicode/        Unicode tables
  javac-floating/       floating-point/BigDecimal helpers
  javac-jimage/         jimage reader
  javac-jmod/           jmod reader
  javac-zip/            zip reader
  javac-ctsym/          ct.sym reader
  javac-classpath/      classpath handling
  javac-charset/        charset support
  javac-fs/             filesystem helpers
  javac-tests/          corpus/diff/oracle tooling
scripts/                Python parity checkers and gate.py
tools/                  Java oracle generators (javac-side probes)
xtask/                  cargo xtask generators
internal/
  reference/jdk17u/     JDK 17 source snapshot used as the oracle
  test/real-projects/   Gson, Guava, OkHttp, commons-lang3, JUnit 5
  test/diag-corpus/     diagnostic examples and frozen expected output
  test/feature-matrix/  Java 9-17 feature cases
docs/                   documentation (SETUP, TESTING, ARCHITECTURE, ...)
.cargo/config.toml      cargo alias and target flags
rust-toolchain.toml     pinned nightly toolchain
dist/                   local distribution output (ignored)
target/                 build output and temporary corpora (ignored)
```

Key entry points:

- `README.md` — project overview and quick start.
- `docs/SETUP.md` — reproducing the development environment.
- `docs/TESTING.md` — gate levels and corpus descriptions.
- `scripts/gate.py` — runs the verification level for the current diff.
- `crates/javac-core/resources/l10n/` — the official `.properties`
  language bundles, embedded with `include_str!` and parsed lazily.
