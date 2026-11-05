# rust-javac

**A reimplementation of the JDK 17 `javac` compiler in Rust.**

This project reimplements the Java compiler rather than wrapping it. The parser, symbol
entering, attribution, flow analysis, lowering, bytecode generation, class-file writing
and diagnostic machinery are implemented in Rust. The objective is observational
equivalence with the reference compiler: identical class-file bytes, identical diagnostic
output and identical exit codes, established by automated comparison.

- Reference implementation: jdk17u commit `982d53c` (Temurin 17.0.19+10)
- Language levels: Java 9–17, module system, `--release` via ct.sym
- Verification: SHA-256 per class file; line-by-line diagnostic comparison

---

## Contents

1. [Architecture](#1-architecture)
2. [Compatibility and verification](#2-compatibility-and-verification)
3. [Build and distribution](#3-build-and-distribution)
4. [Validation results](#4-validation-results)
5. [Testing](#5-testing)
6. [Limitations](#6-limitations)
7. [Documentation](#7-documentation)
8. [License](#8-license)

---

## 1. Architecture

The implementation is organized as 14 crates mirroring the subsystems of jdk.compiler.

| Crate             | Corresponding javac component                         |
|:------------------|:------------------------------------------------------|
| `javac-core`      | `com.sun.tools.javac.{parser, comp, jvm, code, main}` |
| `javac-tools`     | `javax.tools` (file manager API)                      |
| `javac-source`    | `com.sun.source` (tree API)                           |
| `javac-jimage`    | `jdk.internal.jimage` (`lib/modules` reader)          |
| `javac-ctsym`     | ct.sym reader (`--release` platform view)             |
| `javac-classpath` | class path and module path handling                   |
| `javac-charset`   | charset support                                       |
| `javac-unicode`   | Unicode tables                                        |
| `javac-floating`  | floating-point and `BigDecimal` helpers               |
| `javac-jmod`      | jmod archive reader                                   |
| `javac-zip`       | zip reader                                            |
| `javac-fs`        | file-system abstraction                               |
| `javac-strings`   | string utilities                                      |
| `rust-javac`      | CLI (equivalent of `com.sun.tools.javac.Main`)        |

The compiler pipeline follows javac's phase structure: parse → enter → attribute → flow →
transTypes → lower → generate, with diagnostics emitted through a ported `Log` and
resource-bundle layer. The official `.properties` language bundles are embedded and parsed
lazily.

## 2. Compatibility and verification

**Class files.** Each output class is compared with the reference compiler by SHA-256.
Equivalence therefore covers constant-pool contents and ordering, attribute tables (`LineNumberTable`,
`LocalVariableTable`, `StackMapTable`, `BootstrapMethods`,
`InnerClasses`, `Signature`, `NestMembers`, `Record`, `PermittedSubclasses`,
module attributes) and the class-file version fields.

**Diagnostics.** The output of both compilers is normalized (temporary and source paths,
line endings, timing values) and compared line by line: message text, source position,
caret placement, note/warning/error counts and the process exit code (0/1/2/3/4).

**Scope.** Verification covers:

- real-world projects: Gson 2.10.1, Guava 33.4.0, OkHttp 3.14.9, commons-lang3 3.12.0,
  JUnit 5.10.2;
- the JDK's own sources: `jdk.compiler`, `java.base` and `java.desktop`, compiled
  module-wide with javac as the reference;
- the diagnostic example corpus (langtools), 602 comparable cases;
- 43 language-feature cases spanning Java 9–17.

## 3. Build and distribution

### Requirements

| Component   | Version                                                | Purpose                              |
|:------------|:-------------------------------------------------------|:-------------------------------------|
| Rust        | `nightly-2026-11-01` (pinned in `rust-toolchain.toml`) | build                                |
| JDK 17      | Temurin 17.0.19+10                                     | platform data and reference compiler |
| Python      | 3.8+                                                   | test and packaging scripts           |
| Android NDK | API 21+                                                | `android-arm64` packages only        |

### Build

```powershell
cargo build -p rust-javac
```

`build.rs` links the baseline JDK's `lib/modules` and `lib/ct.sym` into
`target/<profile>/`, so the development binary is self-contained as well.

### Distribution

```powershell
py scripts/package.py --build --release --platforms all
```

| Platform        | Linkage | Runtime libc |
|:----------------|:--------|:-------------|
| `windows-amd64` | native  | Windows      |
| `linux-amd64`   | static  | musl         |
| `linux-arm64`   | static  | musl         |
| `android-arm64` | dynamic | Bionic (NDK) |

A distribution contains the compiler and the two data files it reads:

```text
ravac-17.0.19-windows-amd64/
├─ ravac.exe
├─ modules      jimage: system classes for every module
└─ ct.sym       platform signatures for --release 7–17
```

The platform directory is resolved deterministically: `RAVAC_PLATFORM_DIR` if set, otherwise
the executable's directory (or its parent for a `bin/` layout), otherwise an error. The
compiler does not consult `JAVA_HOME`, `PATH` or installation directories, and does not
scan the file system for a JDK.

## 4. Validation results

Results of the last full runs:

| Subject                 | Scope                            | Result                          |
|:------------------------|:---------------------------------|:--------------------------------|
| `javac-core` unit tests | 750 tests                        | 750 passed                      |
| Gson 2.10.1             | 208 classes                      | 208 byte-identical              |
| Guava 33.4.0            | 1970 classes                     | 1970 byte-identical             |
| OkHttp 3.14.9           | 542 classes                      | 542 byte-identical              |
| commons-lang3 3.12.0    | 359 classes                      | 359 byte-identical              |
| JUnit 5.10.2            | 625 classes                      | 625 byte-identical              |
| `jdk.compiler`          | 1637 classes                     | all byte-identical              |
| `java.base`             | 3067 source files (6410 classes) | all byte-identical              |
| `java.desktop`          | 2617 source files (5393 classes) | all byte-identical              |
| Diagnostic corpus       | 602 comparable cases             | 602 matched (last recorded run) |

## 5. Testing

```powershell
py scripts/gate.py --run              # level inferred from the working-tree diff
py scripts/gate.py --level L3 --run   # full gate
```

| Level | Coverage                                       |
|:-----:|:-----------------------------------------------|
|  L0   | `cargo test -p javac-core --lib`               |
|  L1   | L0, repro programs, Gson                       |
|  L2   | L1, jdk.compiler, Guava, OkHttp                |
|  L3   | workspace tests, all corpora and module checks |

## 6. Limitations

The following are intentionally out of scope and rejected at the command line:

| Option                             | Reason                                           |
|:-----------------------------------|:-------------------------------------------------|
| `-proc:*`, `-processor*`, `-A*`    | `javax.annotation.processing` is not implemented |
| `--enable-preview`                 | preview class files are not byte-compatible      |
| `-Xplugin`, `-Xdoclint`, `-Xprint` | not implemented                                  |

Custom file managers and JVM stack-trace diagnostics (`-doe`) are likewise unsupported;
the diagnostic corpus skips the corresponding cases.

## 7. Documentation

| Document                                       | Contents                                  |
|:-----------------------------------------------|:------------------------------------------|
| [docs/SETUP.md](docs/SETUP.md)                 | environment reproduction and cross builds |
| [docs/TESTING.md](docs/TESTING.md)             | gate levels and corpus descriptions       |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)   | design and porting notes                  |
| [docs/PORTING_NOTES.md](docs/PORTING_NOTES.md) | per-file porting annotations              |
| [docs/STYLE.md](docs/STYLE.md)                 | coding conventions                        |
| [AGENTS.md](AGENTS.md)                         | collaboration rules                       |

## 8. License

GPL-2.0-only WITH Classpath-exception-2.0. See [LICENSE](LICENSE).
