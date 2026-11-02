# Repository layout

```text
crates/                 compiler and support crates
  javac-core/           parser, enter, attr, flow, lower, code generation, diagnostics
  rust-javac/           CLI binary and CLI tests
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
xtask/                  cargo xtask generators
tools/                  Java oracle generators
docs/                   documentation
```

`crates/javac-core/resources/l10n/` holds the official `.properties`
language bundles, embedded with `include_str!` and parsed lazily at runtime.