# Reproducing the development environment

This page describes the environment the project is developed and verified
in, and the steps needed to reproduce it from a fresh checkout. Windows is
the primary platform; the compiler itself also builds on Linux.

## 1. Machine

- Windows 10/11, x86_64.
- 8 or more cores recommended. `.cargo/config.toml` runs the parallel front
  end with 16 jobs on the Windows GNU target; fewer cores still work.
- No services or databases are needed: all test data is in the repository.

## 2. Toolchain

### Rust

The repository pins the toolchain in `rust-toolchain.toml`:

```toml
[toolchain]
channel = "nightly-2026-11-01"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

`rustup` installs it on the first `cargo` invocation. The nightly is
required because `.cargo/config.toml` enables `-Zunstable-options` and
`--jobs-frontend=16` (parallel front end) for the
`x86_64-pc-windows-gnu` target, plus `-C target-cpu=native` for
benchmark-like builds.

Check:

```powershell
rustc --version
cargo --version
```

### JDK

The port targets jdk17u commit `982d53c` (Temurin **17.0.19+10**). Install
a JDK 17 of that build and point `JAVA_HOME` at it:

```powershell
# adjust the path to your own JDK installation
$env:JAVA_HOME = "C:\Program Files\Eclipse Adoptium\jdk-17.0.19.10-hotspot"
& "$env:JAVA_HOME\bin\javac.exe" -version   # javac 17.0.19
Test-Path "$env:JAVA_HOME\lib\src.zip"      # True (module checkers need it)
```

Notes:

- The JDK is used as the comparison baseline: the checkers run both
  `javac` and `ravac` and compare bytes/diagnostics.
- `lib/src.zip` is extracted by `scripts/check_jdk_module.py` to
  `target/tmp/jdk-src/<module>`.
- Other 17.0.x builds may produce different diagnostics or help text; the
  frozen `internal/test/diag-corpus/expected/` baselines were generated
  with 17.0.19+10.

### Python

Python 3.8 or newer. The scripts are invoked as `py scripts/...` on
Windows (the launcher) and `python3 scripts/...` elsewhere. No third-party
packages are needed.

### Git

Any recent Git; the repository is self-contained.

## 3. Clone and build

```powershell
git clone <repo-url> rust-javac
cd rust-javac
cargo build -p rust-javac
target\debug\ravac.exe -version
```

The first build compiles all workspace crates; expect a few minutes on the
pinned nightly. Network access is needed once, for `rustup` to install the
pinned toolchain and for Cargo to fetch the (few) crates.io dependencies;
later builds and all checks run offline.

## 4. First verification

```powershell
cargo test -p javac-core --lib     # 750 tests
py scripts/check_gson.py           # 81 sources -> 208/208 identical
```

To run the module-level checks, extract the JDK sources once:

```powershell
py scripts/check_jdk_module.py --module java.base
```

This writes `target/tmp/jdk-src/java.base` (reused by
`scripts/check_java_base.py`) and compares the whole module byte-for-byte.

## 5. Test data (vendored)

| Path | Content |
|---|---|
| `internal/test/real-projects/` | Gson 2.10.1, Guava 33.4.0, OkHttp 3.14.9, commons-lang3 3.12.0, JUnit 5.10.2 sources |
| `internal/test/diag-corpus/` | upstream javac diagnostic examples + frozen `expected/` output |
| `internal/test/feature-matrix/` | Java 9–17 language feature cases |
| `internal/reference/jdk17u/` | JDK 17 source snapshot (porting reference and oracle sources) |

No downloads or generation steps are required. `expected/` can be
regenerated with `internal/test/diag-corpus/expected_gen.py` when the
baseline JDK changes.

## 6. Running the gate

```powershell
py scripts/gate.py --run              # level from the current git diff
py scripts/gate.py --level L2 --run   # force a level
py scripts/gate.py --level L3 --run   # full gate
```

| Level | Commands |
|---|---|
| L0 | `cargo test -p javac-core --lib` |
| L1 | L0 + `check_repros.py` + `check_gson.py` |
| L2 | L1 + `check_jdkcompiler.py` + `check_guava.py` + `check_okhttp.py` |
| L3 | `cargo test --workspace --no-fail-fast` + `check_feature_matrix.py` + `check_cli.py` + `check_repros.py` + `check_gson.py` + `check_jdkcompiler.py` + `check_guava.py` + `check_okhttp.py` + `check_commons_lang3.py` + `check_junit5.py` + `check_java_base.py` + `check_jdk_module.py --module java.desktop` + `check_examples.py` |

Individual scripts accept `--help`; most support `--filter`, `--limit`
and `--verbose`.

## 7. Self-contained distribution

The release artifact is a zip, not a single exe:

```powershell
py scripts/package.py --build --release
```

It writes `dist/ravac-17.0.19-win64.zip` with:

```text
ravac.exe
modules         # jimage: system classes for every module
ct.sym          # --release 7-17 signatures
```

Extract anywhere and run `ravac.exe`; no JDK is required.  At run time the
platform directory is chosen deterministically:

1. `RAVAC_PLATFORM_DIR` (the only environment variable consulted; it must
   contain `modules` directly),
2. the executable's directory (or one level above for a `bin/` layout),
3. otherwise the compiler reports that no platform data was found.

The compiler never scans the machine for a JDK.  Development builds are
self-contained too: `crates/rust-javac/build.rs` hard-links the baseline
JDK's `lib/modules` and `lib/ct.sym` into `target/<profile>/`, so
`target/<profile>/ravac.exe` behaves exactly like the packaged binary.

`modules` is 128.7 MB and `ct.sym` is 8.3 MB; the zip is about 63 MB.

Verification:

```powershell
Remove-Item Env:JAVA_HOME
Remove-Item Env:RAVAC_PLATFORM_DIR
dist\ravac-win64\ravac.exe -d out @sources.txt
```

In this setup Gson 2.10.1 compiles to 208/208 byte-identical classes,
including `--release 11`.

## 8. Windows notes

- **Temporary directory permissions.** Some hosts deny writes under the
  user's `%TEMP%`; the checkers and `cargo test` then fail with
  `Os { code: 5, kind: PermissionDenied }`. Point the temp variables into
  the repository for the session:

  ```powershell
  $env:TEMP = "$PWD\target\tmp\tt"; $env:TMP = $env:TEMP
  ```

- **Long command lines.** Windows limits a command line to 32767
  characters; the scripts pass large source lists through `@argfile`.

- **PowerShell and `-J` options.** Quote JVM options explicitly, otherwise
  PowerShell splits them:

  ```powershell
  & javac "-J-Duser.language=en" "-J-Duser.country=US" -d out A.java
  ```

- **Path length.** The JUnit 5 corpus keeps only `src/main` trees; the
  test/fixture trees exceed the Windows path limit and are listed in
  `.gitignore`.

- **Locale.** `ravac` follows the Windows UI language for diagnostics;
  the test scripts pin English with `-J-Duser.language=en` so the frozen
  baselines are stable.

### Linux

The compiler and the unit tests build with the same toolchain. The Python
checkers were written for Windows: `check_java_base.py` invokes
`bin/javac.exe`, and the module checks read `$JAVA_HOME/lib/src.zip`.
Adjust those paths (or contribute the cross-platform fix) before running
the corpus checks on Linux.

## 9. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `javac not found` from a script | `JAVA_HOME` unset or wrong | set `JAVA_HOME` to the JDK root and restart the shell |
| `no src.zip under JAVA_HOME` | JDK installed without sources | install a full JDK (Temurin ships `lib/src.zip`) |
| `PermissionDenied` during `cargo test` | `%TEMP%` not writable | see the TEMP note above |
| `error: --enable-preview is not supported` | intentional: preview codegen is not byte-compatible | do not use `--enable-preview` |
| `error: ... is not supported ... annotation processing` | intentional: APT is not implemented | do not pass `-proc:*`/`-processor*` |
| diagnostic count differs from `expected/` | baseline JDK is not 17.0.19+10 | install the baseline JDK, or regenerate `expected/` |

## 10. Checklist

1. `rustc --version` uses `nightly-2026-11-01`.
2. `javac -version` prints `javac 17.0.19` and `JAVA_HOME` points at it.
3. `Test-Path "$env:JAVA_HOME\lib\src.zip"` is `True`.
4. `cargo build -p rust-javac` succeeds.
5. `cargo test -p javac-core --lib` passes.
6. `py scripts/check_gson.py` reports `result: PASS`.
