<h1 align="center">rust-javac</h1>

<p align="center">
  <b>A Rust reimplementation of the JDK 17 <code>javac</code></b><br>
  Byte-for-byte identical <code>.class</code> files, diagnostics and exit codes.<br>
  Self-contained runtime — no JDK, no <code>JAVA_HOME</code>, no machine scanning.
</p>

<p align="center">
  <a href="#highlights">Highlights</a> ·
  <a href="#why-it-is-not-a-toy">Why</a> ·
  <a href="#scale">Scale</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="#distribution">Distribution</a> ·
  <a href="#testing">Testing</a> ·
  <a href="#status">Status</a> ·
  <a href="#documentation">Documentation</a>
</p>

---

## Highlights

> **The compiler is a drop-in replacement for javac 17.** Feed it Java sources,
> get the same bytes and the same diagnostics — verified against real projects
> and the JDK's own modules.

|                           |                                                                                                                                                     |
|---------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------|
| **Identical output**      | Every generated `.class` is SHA-256 identical to javac's, including `LineNumberTable`, `StackMapTable`, `BootstrapMethods` and constant-pool order. |
| **Identical diagnostics** | Messages, positions, `-XDrawDiagnostics`, `-Xlint`, note/warning/error counts and exit codes all match.                                             |
| **Self-contained**        | The distribution is `ravac` plus two data files (`modules`, `ct.sym`). The binary never looks for a JDK.                                            |
| **Drop-in CLI**           | `-d`, `-cp`, `-sourcepath`, `--release`, `-source`/`-target`, `-g[:...]`, `-Xlint`, `-verbose`, `-s`, `-h`, `@argfile` and more.                    |

## Why it is not a toy

- **The compiler is reimplemented, not wrapped.** `ravac` does not shell out to
  javac or run a JVM. Parsing, symbol entering, attribution, flow analysis,
  lowering, bytecode generation and class-file writing are Rust code across **14 crates and 500+ source files**.
- **Correctness is measured, not claimed.** Every generated class is compared
  by SHA-256 with javac's output; a single differing byte fails the check.
  Diagnostics are diffed line by line, including positions and exit codes.
- **It compiles the JDK itself.** `java.base` (3067 files), `java.desktop`
  (2617) and `jdk.compiler` (1637) are compiled module-wide and compared
  byte-for-byte with javac.
- **The runtime is self-contained.** No JVM, no JDK, no `JAVA_HOME`: a static
  binary plus `modules` and `ct.sym`. It runs in containers, CI and Termux.
- **The port is broad, not a demo.** Java 9–17 language features, the module
  system, ct.sym `--release` views, `@argfile`, the full diagnostic catalog
  and javac 17's option surface.

## Scale

|                       |                                                                                                                                           |
|:----------------------|:------------------------------------------------------------------------------------------------------------------------------------------|
| Compiler crates       | **14** (`core`, `tools`, `source`, `unicode`, `floating`, `jimage`, `jmod`, `zip`, `ctsym`, `classpath`, `charset`, `fs`, `strings`, CLI) |
| Rust sources          | **500+ files**                                                                                                                            |
| Unit tests            | **750**                                                                                                                                   |
| Byte-compared classes | **10,000+** (Gson, Guava, OkHttp, commons-lang3, JUnit 5, jdk.compiler, java.base, java.desktop)                                          |
| Diagnostic cases      | **602**                                                                                                                                   |
| Language features     | **43** (Java 9–17)                                                                                                                        |

## Quick start

```powershell
git clone <repo>
cd rust-javac
cargo build -p rust-javac

target\debug\ravac.exe -d out Hello.java
```

The development binary is self-contained too: the build script links the
baseline JDK data into `target/<profile>/`.

## Distribution

```powershell
py scripts/package.py --build --release --platforms all
```

<table>
<tr><th>Platform</th><th>Linkage</th><th>Archive</th></tr>
<tr><td><code>windows-amd64</code></td><td>native</td><td><code>ravac-17.0.19-windows-amd64.zip</code></td></tr>
<tr><td><code>linux-amd64</code></td><td>static musl</td><td><code>ravac-17.0.19-linux-amd64.zip</code></td></tr>
<tr><td><code>linux-arm64</code></td><td>static musl</td><td><code>ravac-17.0.19-linux-arm64.zip</code></td></tr>
<tr><td><code>android-arm64</code></td><td>Bionic (NDK)</td><td><code>ravac-17.0.19-android-arm64.zip</code></td></tr>
</table>

Each archive contains the binary, `modules` (the jimage) and `ct.sym`:

```text
ravac-17.0.19-windows-amd64/
├─ ravac.exe
├─ modules      system classes for every module
└─ ct.sym       --release signatures
```

At run time the platform directory is chosen deterministically:

1. `RAVAC_PLATFORM_DIR` (must contain `modules` directly),
2. the executable's directory (or one level above for a `bin/` layout),
3. otherwise an error: no platform data.

`JAVA_HOME`, `PATH` and install locations are never consulted.

## Requirements

<table>
<tr><th>Tool</th><th>Version</th><th>Scope</th></tr>
<tr><td>Rust</td><td><code>nightly-2026-11-01</code> (pinned)</td><td>build</td></tr>
<tr><td>JDK 17</td><td>Temurin 17.0.19+10</td><td>build/test data only</td></tr>
<tr><td>Python</td><td>3.8+</td><td>tests, packaging</td></tr>
<tr><td>Android NDK</td><td>API 21+</td><td>only <code>android-arm64</code></td></tr>
</table>

Linux cross builds use Rust's bundled `rust-lld`; no zig or MSYS2 needed.

## Testing

```powershell
py scripts/gate.py --run              # pick the level from the current git diff
py scripts/gate.py --level L3 --run   # full gate
```

| Level  | Scope                                             |
|:------:|:--------------------------------------------------|
| **L0** | `cargo test -p javac-core --lib` — 750 unit tests |
| **L1** | L0 + repro programs + Gson                        |
| **L2** | L1 + jdk.compiler + Guava + OkHttp                |
| **L3** | workspace tests + every corpus and module check   |

## Status

<table>
<tr><th>Check</th><th align="right">Result</th></tr>
<tr><td><code>javac-core</code> unit tests</td><td align="right"><b>750 / 750</b></td></tr>
<tr><td>Gson 2.10.1</td><td align="right"><b>208 / 208</b> classes identical</td></tr>
<tr><td>Guava 33.4.0</td><td align="right"><b>1970 / 1970</b></td></tr>
<tr><td>OkHttp 3.14.9</td><td align="right"><b>542 / 542</b></td></tr>
<tr><td>commons-lang3 3.12.0</td><td align="right"><b>359 / 359</b></td></tr>
<tr><td>jdk.compiler corpus</td><td align="right"><b>1637 / 1637</b></td></tr>
<tr><td><code>java.base</code> module</td><td align="right"><b>3067 / 3067</b></td></tr>
<tr><td><code>java.desktop</code> module</td><td align="right"><b>2617 / 2617</b></td></tr>
<tr><td>Diagnostic examples</td><td align="right"><b>602 / 602</b> (last recorded run; <code>--enable-preview</code> skipped)</td></tr>
</table>

## Known limitations

- Annotation processing (`-proc:*`, `-processor*`, `-A*`) is refused:
  `javax.annotation.processing` is not implemented.
- `--enable-preview` is refused: preview codegen is not byte-compatible.
- `-Xplugin`, custom file managers and JVM stack traces (`-doe`) are not
  supported; the diagnostic corpus skips those cases.

## Documentation

| Document                                     | Contents                               |
|:---------------------------------------------|:---------------------------------------|
| [docs/SETUP.md](docs/SETUP.md)               | environment reproduction, cross builds |
| [docs/TESTING.md](docs/TESTING.md)           | gate levels and corpora                |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | crate layout and porting design        |
| [docs/STYLE.md](docs/STYLE.md)               | coding conventions                     |
| [AGENTS.md](AGENTS.md)                       | collaboration rules                    |

---

<p align="center">
  <sub>GPL-2.0-only WITH Classpath-exception-2.0 · see <a href="LICENSE">LICENSE</a></sub>
</p>
