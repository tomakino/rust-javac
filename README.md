# rust-javac

A Rust reimplementation of the JDK 17 `javac`, aiming for behavior
equivalence with the official compiler (baseline: jdk17u `982d53c`,
17.0.19+10).

## Requirements

| Tool | Version | Notes |
|---|---|---|
| OS | Windows 10/11 x86_64 | primary target; the compiler also builds on Linux |
| Rust | `nightly-2026-11-01` | pinned by `rust-toolchain.toml`; the pinned nightly is needed for the `-Z` flags in `.cargo/config.toml` |
| JDK | Temurin 17.0.19+10 | development and tests only (comparison baseline, `lib/src.zip`, platform data source); the packaged binary is self-contained |
| Python | 3.8+ | test scripts (`py` on Windows, `python3` elsewhere) |
| Git | any | |

The test corpora are vendored in the repository; nothing is downloaded:

- `internal/test/real-projects/` — Gson, Guava, OkHttp, commons-lang3, JUnit 5
- `internal/test/diag-corpus/` — upstream diagnostic examples with frozen baselines
- `internal/test/feature-matrix/` — Java 9–17 feature matrix
- `internal/reference/jdk17u/` — JDK 17 source snapshot used as the porting reference

See `docs/SETUP.md` for the full environment walkthrough.

## Quick start

```powershell
git clone <repo>
cd rust-javac
cargo build -p rust-javac

# compile one file
target\debug\ravac.exe -d out Hello.java
```

The CLI matches javac 17: `-d`, `-cp`/`-classpath`, `-sourcepath`,
`--release`, `-source`/`-target`, `-Xlint`, `-XDrawDiagnostics`,
`-J-Duser.language=<locale>`, `-s`, `-h`, `-g[:...]`, `-verbose` and
`@argfile`. `--enable-preview` is refused (see Known limitations).

## Distribution

```powershell
py scripts/package.py --build --release
```

Produces `dist/ravac-17.0.19-win64.zip` (about 63 MB) containing
`ravac.exe`, `modules` (the jimage) and `ct.sym`. Extract it anywhere:
the compiler uses only these two data files and needs no JDK installation.
`RAVAC_PLATFORM_DIR` may point at another directory holding `modules`.
The compiler never scans the machine for a JDK: `JAVA_HOME`, `PATH` and
well-known install locations are not consulted at run time.

Verified: with no JDK on the machine (no `JAVA_HOME`, no `PATH` entry),
the packaged binary compiles Gson 2.10.1 to **208/208 byte-identical**
classes, including `--release 11`.

## Testing

```powershell
py scripts/gate.py --run              # pick the level from the current git diff
py scripts/gate.py --level L3 --run   # full gate
```

The levels are L0 (unit tests), L1 (L0 + repros + Gson), L2 (L1 +
jdk.compiler + Guava + OkHttp) and L3 (workspace tests + every corpus and
module check). See `docs/TESTING.md` for the command list.

## Status

Numbers from the last full runs (see `docs/TESTING.md`):

- `javac-core` unit tests: **750 passing**.
- Gson 2.10.1: **208/208 classes byte-identical** (also with
  `--release 11`/`17` and every `-g` variant).
- OkHttp 3.14.9: **542/542**; Guava 33.4.0: **1970/1970**;
  commons-lang3 3.12.0: **359/359**.
- jdk.compiler corpus: **1637/1637**; `java.base`: **3067/3067**;
  `java.desktop`: **2617/2617**.
- Diagnostic examples: **602/602** comparable cases at commit `d723adf2`;
  the `--enable-preview` cases are now skipped, so run
  `scripts/check_examples.py` for the current count.

## Known limitations

- Annotation processing is refused (`-proc:*`, `-processor*`, `-A*`):
  `javax.annotation.processing` is not implemented.
- `--enable-preview` is refused: preview codegen does not match javac
  byte-for-byte (guard cases differ in the LineNumberTable).
- `-Xplugin`, custom file managers and JVM stack traces (`-doe`) are not
  supported; the diagnostic corpus skips the cases that need them.

## License

GPL-2.0-only WITH Classpath-exception-2.0; see [LICENSE](LICENSE).
