# rust-javac end-to-end test plan

## 1. Goals and scope

**E2E definition**: from the command-line entry point (`rust-javac.exe`) to the
final artifacts (`.class` files, diagnostic text, exit codes), compared item by
item with the official `javac`; internal functions are out of scope (that is the
job of the `--lib` unit tests).

**Covered**: CLI parsing -> file lookup/reading -> parse/enter/attr/flow/
transTypes/lower/gen -> output/diagnostics/exit codes -> performance.

**Not covered (not feasible by design)**: JVM plugins (`-Xplugin`), annotation
processors (`-processor`/`-proc:only`), JVM stack traces for `-doe`, custom
`-filemanager`, and deprecated mechanisms such as `-profile`/`-source 7`.

## 2. Test layers

| Layer            | Content                                  | Command                          | Time  |
|------------------|------------------------------------------|----------------------------------|-------|
| L1 unit          | 743 lib tests                            | `cargo test -p javac-core --lib` | ~3 s  |
| L2 diagnostics   | 602 comparable upstream examples         | `py scripts/check_examples.py`   | ~50 s |
| L3 project bytes | Gson 208 classes, OkHttp 542, Guava 1970 | `py scripts/check_gson.py` etc.  | ~50 s |
| L4 CLI behavior  | help/version/errors/exit codes           | `py scripts/check_cli.py`        | ~5 s  |
| L5 performance   | release benchmarks                       | `py scripts/bench_rust_javac.py` | ~10 s |

## 3. L4 CLI behavior matrix

| Case                                              | Expected (aligned with javac)                                                                |
|---------------------------------------------------|----------------------------------------------------------------------------------------------|
| `--help`                                          | standard option table (including the `-J<flag>` line)                                        |
| `--help-extra`                                    | extended option table, **without** `-J`                                                      |
| `--help-lint`                                     | **byte-identical** to javac (31 keys plus `all`/`none`)                                      |
| `--version` / `--full-version`                    | `javac 17.0.19`                                                                              |
| no arguments                                      | standard help, exit code **2**                                                               |
| `-J<flag>`                                        | consumed by the launcher (`-J-Duser.language=en` takes effect)                               |
| `-Xstdout <file>`                                 | diagnostics go to the file, console empty, **file byte-identical** to javac (including CRLF) |
| `@argfile`                                        | reads the argument file (`#` comments, whitespace-separated)                                 |
| invalid option                                    | `error: invalid flag: <arg>` plus usage, exit code **2**                                     |
| missing source file                               | `error: file not found: <path>` plus usage, exit code **2**                                  |
| `-Werror`                                         | `error: warnings found and -Werror specified` when warnings exist, exit code **1**           |
| `-Xmaxerrs N` / `-Xmaxwarns N`                    | truncation after the limit plus a summary line                                               |
| `-encoding`                                       | correct UTF-8/GBK reading; an invalid encoding is an error                                   |
| `-Xlint:all` / `-Xlint:none` / `-Xlint:-rawtypes` | correct warning set                                                                          |

## 4. L3 project bytes and diagnostics

| Case                                | Pass criterion                                                |
|-------------------------------------|---------------------------------------------------------------|
| Gson (80 sources -> 208 classes)    | **208/208 SHA-256 identical**; `-XDrawDiagnostics` diff empty |
| OkHttp (229 sources -> 542 classes) | **542/542 identical**; diagnostic diff empty                  |
| Guava (630 sources -> 1970 classes) | **1970/1970 identical**                                       |
| Produced classes run (optional)     | run JUnit with classes compiled by rust-javac                 |
| Incremental/repeat runs             | the same input twice produces stable bytes                    |

## 5. L2 diagnostic corpus gate

- Target: `fail=0`; current: 588 pass / 14 fail / 67 skip out of 602.
- The `skip` whitelist is fixed: annotation processing, custom file managers,
  JVM stack traces, automatic modules, plugins, `-verbose`, and deprecated
  `-profile`/`-source` mechanisms.
- Live reference mode (recommended): generate the reference with `javac` at
  run time instead of relying only on checked-in `expected/` files, so a JDK
  upgrade cannot silently rot the baselines.

## 6. L5 performance baseline (release)

| Case                    | Baseline                                    | Threshold                         |
|-------------------------|---------------------------------------------|-----------------------------------|
| Gson compilation        | rust release **~0.8 s** vs javac **~2.4 s** | rust <= javac x 1.2               |
| OkHttp compilation      | see the benchmark script                    | same, plus a regression threshold |
| Cold start vs warm runs | median of 5 runs                            | record the trend                  |

## 7. Known gaps (outside the gate)

1. Module-system diagnostics: ten `check_examples` cases fail (see
   `docs/PORTING_NOTES.md`, PN-003).
2. `IllegalSignature` generation error is missing (PN-004).
3. 67 diagnostic cases are skipped: annotation processing, plugin,
   `-verbose`, `-doe`, automatic modules, custom file managers, deprecated
   mechanisms.
4. `-proc:none`/`-proc:only` are not supported.
5. `expected/` is checked-in text and can go stale after a JDK upgrade; add
   live reference mode.

## 8. Execution cadence

- **Every code change**: L0 (`cargo test -p javac-core --lib`).
- **Daily/large changes**: L2 + L3 (Gson/OkHttp/Guava) + L4.
- **Before release**: full L0-L5 plus benchmarks.
- **Failure triage**: L2 with `--filter` or verbose output for the first
  diff; L3 with `javap -v` for constant-pool/instruction diffs.

## 9. Scripts

```text
py scripts/gate.py                 # select the level for the current diff
py scripts/check_examples.py       # diagnostic corpus (L2), with --filter
py scripts/check_gson.py           # Gson byte parity (L3)
py scripts/check_okhttp.py         # OkHttp byte parity (L3)
py scripts/check_guava.py          # Guava byte parity (L3)
py scripts/check_cli.py            # CLI behavior (L4)
```