# AGENTS.md — Collaboration rules

Applies to every AI agent and contributor working in this repository.

## 1. Define the goal first

State the goal in one sentence and list three non-negotiable constraints:
safety, minimal change, consistency. If the goal is unclear, ask before
touching anything.

## 2. Find every touch point

Before any change, grep for the concept across code, configuration, scripts,
docs, tests and examples; turn the result into a checklist and handle every
item. Never change only the one place that was named.

## 3. Single source of truth (SSOT)

A concept has one source of truth. Whenever a second form appears
(compatibility layer, alias, duplicate layout, duplicate command), first ask
whether it can be merged; only fall back to duplication when merging is
impossible, and write down why.

## 4. Evidence threshold

"Supported" and "done" require verification. Work that is not verified is
marked "unverified" or deleted. Verify in increasing cost order:
compile → smoke → targeted tests → full; skipping a level needs explicit
approval.

## 5. Root cause and siblings

Reproduce before fixing; fix the cause, not the symptom. When changing one
place, check the same pattern elsewhere and fix it in the same batch.

## 6. Impact report

Every delivery carries three lines: what changed, what may be affected, what
was not verified.

## 7. Stay in scope

Do only what was asked. Long tasks, full test runs and release operations need
an explicit instruction.

## 8. Daily development and quick verification

Each change is verified in increasing cost order, stopping at L1 by default;
full L3 and cross builds run only when explicitly requested or before a merge.

```powershell
cargo check -p javac-core              # type check, fastest
cargo build -p rust-javac              # produces target\debug\ravac.exe (platform data is compiled in)
cargo test -p javac-core --lib         # L0: unit tests
py scripts/gate.py --run               # picks a level from the changed paths and runs it
py scripts/gate.py --files a.rs b.rs   # only the listed files
py scripts/gate.py --level L2 --run    # force a level
```

Build dependency: `rust-javac` routes every allocation through `mimalloc`
(`java.base` full run: 29.5 s → 23.3 s). `mimalloc` is C, so the GNU target
needs a MinGW C compiler:

```powershell
winget install MartinStorsjo.LLVM-MinGW.UCRT
$env:CC_x86_64_pc_windows_gnu = "<install dir>\llvm-mingw-*\bin\clang.exe"
$env:AR_x86_64_pc_windows_gnu = "<install dir>\llvm-mingw-*\bin\llvm-ar.exe"
```

Do not add that `bin` directory to `PATH`: its `x86_64-w64-mingw32-gcc` alias
shadows the Rust linker and produces `unable to find library -lgcc`, an error
that points nowhere near the real cause.

Smoke test: `target\debug\ravac.exe -d out Hello.java`; or run a single corpus
checker, e.g. `py scripts/check_gson.py --build` (exit code 0 = identical,
1 = differences, 2 = environment missing).

Platform data: checked in under `internal/platform/17.0.19+10/` and embedded
into the binary by `include_bytes!`. Regenerate it with
`cargo xtask gen-platform --out internal\platform\17.0.19+10\modules` and
`cargo xtask gen-ct-sym --out internal\platform\17.0.19+10\ct.sym`;
`--check` verifies the local files. Committing a new image requires the user's
explicit request.

Levels: L0 unit tests; L1 = L0 + repro + Gson (~15 s); L2 = L1 + jdk.compiler +
Guava + OkHttp + RxJava + Spring (~85 s); L3 = every test, corpus and module
check. Use L1 for parser/CLI changes and L2 for attr/flow/lower/codegen changes;
doc-only changes need no tests. The gate accepts registered known failures; a
new failing name counts as a failure.

## Project rules

- Declare only verified target platforms: `windows-amd64`, `linux-amd64`,
  `linux-arm64`, `android-arm64`.
- Linux links statically (musl) with Rust's bundled `rust-lld`; no zig.
- Platform data (`modules`, `ct.sym`) is compiled into the binary by
  `include_bytes!`: at run time the compiler reads memory only and never reads
  an external file or `RAVAC_PLATFORM_DIR`.
- Cross builds and packaging: `python scripts/package.py --build --release
  --platforms android-arm64` (Linux: `--platforms linux-amd64,linux-arm64`).
- Test data lives under `internal/test`; never edit the `expected/` baselines
  by hand.
- Run `git status` before committing and confirm there are no unexpected files;
  never commit `dist/` or `target/`.
