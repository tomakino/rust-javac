# Testing

## Layers

1. **Unit tests** - `cargo test -p javac-core --lib` (743 tests): parsing,
   types, flow analysis, diagnostic formatting, properties parsing, and
   oracle comparisons.
2. **CLI parity** - `cargo test -p rust-javac --test cli_parity` (18 tests):
   option handling, `-XDrawDiagnostics`, help output, and exit codes.
3. **Upstream diagnostic examples** - `cargo test -p rust-javac --test
   diag_examples`: runs the langtools corpus and compares normalized output
   with the checked-in baselines.
    - Environment: `DIAG_EXAMPLES_FILTER`, `DIAG_EXAMPLES_VERBOSE`,
      `DIAG_EXAMPLES_TIMEOUT_SECS`.
    - Current: 602 comparable cases, 588 pass / 14 fail / 67 skip.
4. **Corpus and oracle tooling** - `crates/javac-tests` provides
   `gen-corpus-baselines` and `diff_runner` for comparison against a local
   JDK installation.
5. **Real projects** - Gson 2.10.1, OkHttp 3.14.9, and Guava 33.4.0 are
   compiled and compared byte-for-byte with javac.

## Rules

- Tests must not modify the checked-in expected output.
- Cases that require JVM-host mechanisms (annotation processors, plugins,
  modular jars, JVM stack frames) are marked `skip` and do not count as
  passes.
- Environment-specific temporary paths are normalized (`<tmp>`) before
  diagnostic text comparison.