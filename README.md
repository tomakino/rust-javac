# rust-javac

A Rust reimplementation of the JDK 17 `javac`, aiming for behavior
equivalence with the official compiler (baseline: jdk17u `982d53c`,
17.0.19+10).

## Build and run

Requires JDK 17 (`JAVA_HOME` pointing at the JDK root) and Rust 1.99:

```bash
cargo build -p rust-javac
target/debug/ravac -d out Hello.java
```

The common options match javac 17, including `-d`, `-cp`/`-classpath`,
`-sourcepath`, `-source`/`-target`, `-Xlint`, `-XDrawDiagnostics`, and
`-J-Duser.language=<locale>`.

## Status

- Upstream diagnostic examples: **588 / 602** comparable cases pass, with
  67 cases skipped (annotation processors, modular jars, plugins, JVM stack
  frames, and other JVM-host mechanisms).
- `javac-core` unit tests: **743 passing**.
- Gson 2.10.1: **208/208 classes byte-identical** to javac; OkHttp 3.14.9:
  **542/542**; Guava 33.4.0: **1970/1970**.

## Testing

```bash
cargo test -p javac-core --lib
cargo test -p rust-javac --test cli_parity
py scripts/gate.py          # select the verification level for the diff
```

See `docs/TESTING.md` for the full matrix and `docs/STYLE.md` for the
coding conventions.

## License

GPL-2.0-only WITH Classpath-exception-2.0; see [LICENSE](LICENSE).