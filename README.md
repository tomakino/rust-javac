# rust-javac

A Rust reimplementation of the JDK 17 `javac`, byte-for-byte compatible with the
official compiler (baseline: jdk17u `982d53c`, Temurin 17.0.19+10).

```text
 source .java ──► ravac ──► .class files identical to javac 17
```

## Highlights

- **Byte-identical output** — every generated `.class` file matches javac
  (SHA-256), including `LineNumberTable`, `StackMapTable`, `BootstrapMethods`
  and constant-pool ordering.
- **Identical diagnostics** — messages, positions, `-XDrawDiagnostics`,
  `-Xlint`, note/warning/error counters and exit codes.
- **Self-contained runtime** — the distribution is `ravac` plus two data files
  (`modules`, `ct.sym`). No JDK, no `JAVA_HOME`, no scanning of the machine.
- **Drop-in CLI** — `-d`, `-cp`, `-sourcepath`, `--release`, `-source`/`-target`,
  `-g[:...]`, `-Xlint`, `-verbose`, `-s`, `-h`, `@argfile` and more.

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

One zip per verified platform:

| Platform | Linkage | Archive |
|---|---|---|
| `windows-amd64` | native | `ravac-17.0.19-windows-amd64.zip` |
| `linux-amd64` | static musl | `ravac-17.0.19-linux-amd64.zip` |
| `linux-arm64` | static musl | `ravac-17.0.19-linux-arm64.zip` |
| `android-arm64` | Bionic (NDK) | `ravac-17.0.19-android-arm64.zip` |

Each archive contains the binary, `modules` (the jimage) and `ct.sym`.
Extract anywhere and run:

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

`JAVA_HOME`, `PATH` and well-known install locations are never consulted.

## Requirements

| Tool | Version | Scope |
|---|---|---|
| Rust | `nightly-2026-11-01` (pinned) | build |
| JDK 17 | Temurin 17.0.19+10 | build/test data only |
| Python | 3.8+ | test and packaging scripts |
| Android NDK | API 21+ | only for `android-arm64` builds |

Linux cross builds use Rust's bundled `rust-lld`; no zig or MSYS2 needed.

## Testing

```powershell
py scripts/gate.py --run              # pick the level from the current git diff
py scripts/gate.py --level L3 --run   # full gate
```

| Level | Scope |
|---|---|
| L0 | `cargo test -p javac-core --lib` (750 tests) |
| L1 | L0 + repro programs + Gson |
| L2 | L1 + jdk.compiler + Guava + OkHttp |
| L3 | workspace tests + every corpus and module check |

## Status

Numbers from the last full runs:

| Check | Result |
|---|---|
| `javac-core` unit tests | **750 / 750** |
| Gson 2.10.1 | **208 / 208** classes identical |
| Guava 33.4.0 | **1970 / 1970** |
| OkHttp 3.14.9 | **542 / 542** |
| commons-lang3 3.12.0 | **359 / 359** |
| jdk.compiler corpus | **1637 / 1637** |
| `java.base` module | **3067 / 3067** |
| `java.desktop` module | **2617 / 2617** |
| Diagnostic examples | **602 / 602** comparable cases (last recorded run; \`--enable-preview\` cases are skipped now) |

## Known limitations

- Annotation processing (`-proc:*`, `-processor*`, `-A*`) is refused:
  `javax.annotation.processing` is not implemented.
- `--enable-preview` is refused: preview codegen is not byte-compatible.
- `-Xplugin`, custom file managers and JVM stack traces (`-doe`) are not
  supported; the diagnostic corpus skips those cases.

## Documentation

| Document | Contents |
|---|---|
| [docs/SETUP.md](docs/SETUP.md) | environment reproduction, cross builds |
| [docs/TESTING.md](docs/TESTING.md) | gate levels and corpora |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | crate layout and porting design |
| [docs/STYLE.md](docs/STYLE.md) | coding conventions |
| [AGENTS.md](AGENTS.md) | collaboration rules |

## License

GPL-2.0-only WITH Classpath-exception-2.0; see [LICENSE](LICENSE).
