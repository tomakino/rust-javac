# Development notes

Historical notes from the porting and performance work. Test counts and
timings reflect the state at the time of writing; current numbers are in
`docs/TESTING.md`.

## Pitfall: single-file compile in the repository root took 45 seconds

### Symptom

Running `target\debug\rust-javac.exe -d out Hello.java` from the repository
root took about 45 seconds. Copying the binary to a clean temporary directory
and running it there reduced the same compilation to 0.7-1.7 seconds.

### Root cause

The default classpath contains `.` (the current directory).
`add_classpath_packages_to_unnamed_module` in `crates/javac-core/src/main/
mini_compiler.rs` recursively scans classpath directories to collect packages;
the repository root contains `internal/reference` (a JDK source tree with tens
of thousands of files), so the scan dominated the run.

### Fix

The scan skip list grew from `target | build | .git | .gradle | node_modules |
out` to also include `internal` and `reference`.

After the fix: 1.26 s in the repository root, 0.70 s in a clean directory.

### Lessons

1. **Isolate the working directory before measuring performance**; otherwise
   `-cp .` scans the reference tree and test data.
2. **Use phase timing instead of guessing**: parse -> modules -> enter -> attr
   -> flow -> desugar/gen. In this case: parse 1.3 ms, modules 353 ms, enter
   468 ms, attr 622 ms, about 1.7 s total.
3. Remove temporary `eprintln!` calls after debugging (17 were removed here).
4. Stop commands that exceed 30 seconds instead of waiting.

## Related pitfall: default CLI language and `-J-Duser.language`

- The official javac takes its default locale from the JVM's
  `user.language`; on Windows this comes from `GetUserDefaultUILanguage()`
  (a Chinese system defaults to `zh_CN`).
- The port originally read only `LC_ALL`/`LC_MESSAGES`/`LANG` and fell back
  to `en` on Windows, so the CLI was always English. `Locale::
  default_locale()` now detects the Windows UI language.
- `-J-Duser.language` originally affected help text only, not compilation
  diagnostics; `set_default_locale()` now installs the process-wide default
  after CLI parsing, and diagnostics follow it.
- Unit tests pin `default_locale()` to `en` so results do not depend on the
  host language.

## 2026-10-31: OkHttp compiles with zero errors; P5 root cause

### Results

- OkHttp (229 sources) compiled with **zero errors**; the OkHttp JUnit suites
  passed (4 classes / 416 tests, internal 15 classes / 256 tests).
- Gson 2.10.1: **208/208 classes byte-identical**, diagnostics IDENTICAL; the
  Gson JUnit suite reported 1258/2, matching javac.
- `javac-core --lib` 846 tests; `diag_examples` 612/3/54 (3 accepted known
  cases).

### Root causes and fixes (upstream references)

1. The final-argument check added at the end of `Infer.instantiateMethod` had
   no upstream counterpart and added constraints for bounded parameters;
   removing it fixed P5 (`Infer.java:215-224` only completes the deferred
   context and solves).
2. `TypeEnter.finishImports` must run after the completion cascade returns;
   the port ran it during nested completion, so `TlsUtil` static imports saw
   only 1 of 7 members. It is now deferred to the outermost boundary
   (`TypeEnter.java:204-217/223-237/249-297`).
3. `ResolvedConstructorType.overloadCheck` replaced TYPEVAR/UNDETVAR
   `localInfo.pt` with `noType`, a non-upstream special case; removing it let
   the diamond return constraint reach the main inference context
   (`ArgumentAttr.java:704-707`).
4. Upstream `InferenceException extends InapplicableMethodException`
   (`Infer.java:138`); `catch_inapplicable_method_exception` must also catch
   `InferenceExceptionPayload` (e.g. `Resolve.instantiate`, `Resolve.java:
   673-690`).
5. `ArgumentAttr.speculativeTypes` now shares `dup` state and keys entries by
   `ResultInfo.id` instead of stack addresses; `ResolvedMethod` reuses the
   resultInfo identity (`ArgumentAttr.java:343-348/357/675-678`).

### Performance (compile only, no execution)

| Dataset | debug | release | javac (after warmup) |
|---|---|---|---|
| Gson | mean 23.21 s | mean **6.47 s** | 2.40 s |
| OkHttp | mean 150.12 s | mean **51.84 s** | 8.17 s |

Release was about 3.6x/2.9x faster than debug; it remained about 2.7x/6.3x
slower than javac, pending profiling (`samply`/`cargo flamegraph`).

### Known deviations (registered, not implemented)

- The full `TypeEnter` phase tree (`HierarchyPhase` completer ordering, the
  `AbstractMembersPhase` todo/completing reentrant queue) is not implemented;
  only the observable `finishImports` ordering is aligned.
- `speculativeTypes` keeps insertion order in a `Vec` (upstream uses an
  identity-hashed `HashMap`); puts overwrite by `ResultInfo` identity and the
  semantics match, but iteration order differs.
- Rust panics model Java checked exceptions; the panic hook prints a
  `resolve.rs:4220` line on catch. It is noise, not a behavioral failure.

## Performance rounds (2026-10-31)

### Round 2 (samply profiling)

- samply crashed because ETW reported a GBK `ImageFileName` and samply 0.13.1
  parsed it as strict UTF-8; a local patch switched `String::from_utf8` to
  `from_utf8_lossy` (tool fix, not committed).
- Hotspots and fixes:
  1. Ten `name.to_java_string() == other.to_java_string()` comparisons in
     `symtab.rs` became direct `Name` comparisons (the old code decoded
     UTF-8 to UTF-16 and allocated on every comparison, over 22% of the
     profile).
  2. `enter.rs` borrows `visible_packages` instead of cloning it;
     `modules.rs` updates `module_visible_packages()` in place via `sym_mut`.
  3. `javac-zip` `entry_index` uses an FxHasher (lookup semantics unchanged).
- Results (compile only): Gson release **0.973 s** (javac 2.40 s); OkHttp
  release **24.28 s** (previously 51.84 s; javac 8.17 s); Gson debug
  23.2 -> 6.48 s.

### Round 3

- Kept `[profile.release] lto = "fat"` and `codegen-units = 1`.
- **PGO is not feasible**: `-Cprofile-generate` needs `profiler_builtins`, which
  the windows-gnu target does not ship (only android/linux targets do);
  installing `rustc-dev` did not help, and the msvc toolchain lacks the
  Windows SDK (`kernel32.lib`).
- **mimalloc is not feasible**: `libmimalloc-sys` needs a full MinGW `gcc`
  (only the rust-mingw driver is present, without `cc1.exe`); the dependency
  and `global_allocator` were reverted.
- **`Name` without bytes (upstream `NameImpl` shape)**: the first attempt
  caused Gson `exit=4` (`RefCell` borrow reentrancy) and 72 lib failures, so
  it was reverted; a redo should avoid reentrant borrow tables (for example,
  hold `Rc<RefCell<Vec<u8>>>` in `Name`, or forbid `from_*` reentrancy).
- Conclusion: the low-risk optimization space was exhausted; the remaining
  gap is JVM JIT/mature caches versus the Rust runtime model.

### Round 4

- **`Name` without bytes**: `Name` became `(table, index, length)` and reads
  bytes through `NameTableHandle::with_bytes`, removing a byte copy and an
  `Rc` allocation per interned name. The corrected version (copy bytes at
  reentrancy-prone call sites) passes Gson 208/208 + IDENTICAL and lib 846.
- **`FxNameMap`**: an Fx-style hasher for `Name`-keyed maps
  (`visible_packages`, `NamedImportScope` `name2_scopes`, parser
  `tag_parsers`, module seen-packages); lookup/insert semantics unchanged.
- Result (Gson debug, 3 CPU runs): min **5.531 s**, mean 5.615 s.

### Round 5

- **Inapplicable failure channel (Result-ized)**:
  `MethodCheckContext.report` no longer panics; it records a thread-local
  failure, and `catch_inapplicable_method_exception` checks the record after
  the body returns and yields `MethodCheckFailure::Inapplicable` (the panic
  channel remains for the other throw sites). Gson debug CPU **5.531 ->
  4.562 s (-17.5%)**.
- **Symtab class index**: upstream `classes` is `HashMap<Name, Map<
  ModuleSymbol, ClassSymbol>>`; the port used a linear `Vec`. A
  `class_index: FxNameMap<Name, usize>` keeps `classes` insertion order for
  traversal while `get_class_in`/`do_enter_class`/`get_classes_for_name` use
  the index. Gson debug CPU **4.562 -> 3.984 s (-12.7%)**.
- Remaining hotspots: `miniz_oxide` inflate, id `index` calls (not inlined in
  debug), `TypeEnter::resolve_imports`, and `FxNameMap` rehashing.
- Cumulative: Gson debug CPU 5.531 -> **3.984 s (-28%)**; all behavior checks
  passed (Gson 208/208, diagnostics IDENTICAL, lib 846).

### Round 6

- **FxNameMap preallocation**: `fx_map_with_capacity` for the class index
  (1024), visible packages (32), name2_scopes (8), tag parsers (25), and
  module seen-packages (64), removing `reserve_rehash`. Gson debug CPU
  **3.984 -> 3.406 s (-14%)**.
- Cumulative: Gson debug CPU **5.531 -> 3.406 s (-38%)**.

### Round 7-10: javac_zip

- Adopted `zlib-rs` (pure Rust, zlib-compatible inflate) for the DEFLATED
  branch (`Inflate::new(false, 15)` plus `decompress(..., Finish)`),
  preallocating by `uncompressed_size` and checking `total_out`.
- `rawzip` was evaluated; a pure-rawzip version failed 6 of 10 zip tests
  (zip64, CP437 names, `archive_offset`, unknown compression methods) and was
  reverted.
- `libdeflater` could not be built (needs a C toolchain and the Windows SDK).
- Final design: `ZipArchive` holds a rawzip reader plus a wayfinder per
  central-directory entry; `read_entry` resolves the entry and inflates with
  `zlib-rs`. Central-directory parsing uses rawzip records (UTF-8/CP437
  names, STORE/DEFLATE, sizes, CRC, raw flags); zip64 is now accepted.
- Whole-file residency: `ZipArchive::open` reads the archive once and parses
  with `rawzip::ZipArchive::<()>::from_slice`; `read_entry` slices memory
  with no file I/O. Gson debug CPU min **3.828 s** (mean 3.953).

### Round 11-12

- `.cargo/config.toml` adds `-C target-cpu=native` for
  `x86_64-pc-windows-gnu` (local machine only).
- `PoolWriter.put_signature` merges `SignatureGenerator` class references
  into `inner_classes` (upstream `SignatureGenerator.classReference` ->
  `enterInner`); Gson remained 208/208 + IDENTICAL.
- Remaining OkHttp byte differences: missing referenced member-class entries
  in `InnerClasses`, and lambda synthetic methods carrying `line 1` instead
  of the lambda's line. `PoolWriter.keys_to_pos` is a linear `Vec` scan
  (upstream uses a `HashMap`), making constant-pool lookup O(n^2).