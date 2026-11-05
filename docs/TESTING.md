# Testing

## Layers

| Level | Scope | Command |
|---|---|---|
| L0 | `javac-core` unit tests (750) | `cargo test -p javac-core --lib` |
| L1 | L0 + repro programs + Gson | `py scripts/gate.py --level L1 --run` |
| L2 | L1 + jdk.compiler + Guava + OkHttp | `py scripts/gate.py --level L2 --run` |
| L3 | workspace tests + every corpus and module check | `py scripts/gate.py --level L3 --run` |

`py scripts/gate.py --run` chooses a level from the current `git diff`.
The gate stops at the first failing command.

## What the checks compare

- `check_repros.py` — 56 small repro programs; class bytes and diagnostics.
- `check_gson.py`, `check_guava.py`, `check_okhttp.py`,
  `check_commons_lang3.py` — real projects; every class file by SHA-256,
  plus stderr and exit code. `check_gson.py` also accepts `--release N`.
- `check_jdkcompiler.py` — the jdk.compiler sources; 1637 classes.
- `check_junit5.py` — JUnit 5 modules; 625 classes.
- `check_java_base.py` — the java.base source tree compiled with
  `--patch-module java.base=<src>` (3067 files). Needs
  `target/tmp/jdk-src/java.base`, produced by
  `check_jdk_module.py --module java.base`.
- `check_jdk_module.py --module java.desktop` — whole-module compile from
  `$JAVA_HOME/lib/src.zip` (2617 files, 5393 classes); excludes
  `javax/swing/plaf/gtk` and `PangoFonts.java` because `src.zip` lacks the
  Unix platform classes.
- `check_cli.py` — CLI contract: no-argument exit code, unknown option,
  the `--help` `-J<flag>` line, and `-Xstdout`.
- `check_examples.py` — upstream diagnostic examples compared with the
  frozen baselines in `internal/test/diag-corpus/expected/`.
- `check_feature_matrix.py` — 43 Java 9–17 language feature cases.

## Current numbers

| Check | Result |
|---|---|
| `javac-core` unit tests | 750 passing |
| Gson 2.10.1 | 208/208 classes identical |
| Guava 33.4.0 | 1970/1970 |
| OkHttp 3.14.9 | 542/542 |
| commons-lang3 3.12.0 | 359/359 |
| jdk.compiler corpus | 1637/1637 |
| java.base | 3067/3067 |
| java.desktop | 2617/2617 |
| JUnit 5 | 625 classes |
| feature matrix | 43/43 |
| CLI checks | 4/4 |
| repro programs | 56/56 |
| diagnostic examples | 602/602 (last run at commit `d723adf2`) |

The `--enable-preview` diagnostic examples are skipped now (the option is
refused), so `check_examples.py` reports a different total than the
602 above.

## Skips

`check_examples.py` skips cases that require JVM-host mechanisms or
options the port refuses. Current reasons:

```
annotation-processing=47  preview=15            deprecated-source=12
custom-file-manager=2     modules-automatic=2   deprecated-profile=1
jvm-stacktrace=1          plugin=1              verbose=1
```

Skips count as neither passes nor failures. `expected_gen.py` records the
same skip set when regenerating baselines.

## Rules

- Tests must not modify the checked-in `expected/` output.
- Environment-specific paths are normalized (`<src>`, `<tmp>`) and
  `[Nms]` timings are replaced before comparison; line endings are
  normalized too.
- Cases that need annotation processors, plugins, custom file managers,
  JVM stack frames, automatic modules or `--enable-preview` are skipped.
- Regenerate `internal/test/diag-corpus/expected/` with
  `expected_gen.py` after changing the baseline JDK.
- Run `cargo test --workspace --no-fail-fast` with a writable `TEMP`
  on Windows (see `docs/SETUP.md`).
