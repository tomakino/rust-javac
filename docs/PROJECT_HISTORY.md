# rust-javac project history

> 2026-10-25 to 2026-11-01, 282 commits (`4b08a1e` to `8486e92`).
> Goal: a 1:1 behavior-equivalent reimplementation of JDK 17 javac
> (jdk17u@982d53c, 17.0.19+10) in Rust.

## 1. Goals and red lines

- **Acceptance**: byte-identical `.class` files, line-identical
  `-XDrawDiagnostics` output (content, position, count, order), and identical
  CLI output and exit codes.
- **Single source of truth**: the official JDK 17 sources (`internal/reference/jdk17u`). No approximations, fallbacks,
  or special
  cases; any deviation found in the port is corrected against the official
  source.
- **Do not reproduce deprecated or incorrect upstream paths**: `-profile`
  and `-source 7` are not implemented; the CLI only adds non-deprecated
  options with correct semantics.
- **Pay down debt**: unimplemented points (`unimplemented!`/`unreachable!`/
  silent placeholders) are audited and cleared in batches, not left as
  "later".

## 2. Timeline

### 2.1 Infrastructure and parallel porting (P0/P0.5, 10-25/26)

- Multi-crate layout: `javac-core` (pipeline + diagnostics), `rust-javac`
  (CLI), `javac-tools` (javax.tools-style file manager), `javac-source`
  (com.sun.source), `javac-unicode`, `javac-floating`, `javac-jimage`/`jmod`/
  `zip`/`ctsym`/`classpath`/`charset`/`fs`, `javac-tests` (corpus/oracle),
  `xtask` (resource generation).
- Parallel "util-porter" batches; checkpoint recovery after network
  interruptions.
- Early contracts: arena ids (`TreeId`/`SymbolId`/`TypeId`, ADR 0001),
  explicit service parameters, embedded `.properties` resources via
  `include_str!`.

### 2.2 Parser and tree oracle (P1)

- Full `JavacParser` grammar: records, sealed types, patterns, lambdas,
  method references, switch expressions, modules, annotations.
- `TreeMaker`, `Pretty`, `TreeInfo`, and `TreeCopier` (1849 lines, about 75
  visitors) ported 1:1.
- Layered tree-oracle comparison ended at **37/37**.
- Pitfall: nested generic `>>>` failed because `Scanner.split` must return
  the right half and store the left half in `prev_token`; fixing it restored
  parsing.

### 2.3 ClassReader and ClassFinder (P2)

- `ClassReader` with all 26 attribute readers (Signature, Record,
  PermittedSubclasses, MethodParameters, LVT, Module, ModuleResolution,
  Runtime*Annotations, AnnotationDefault, TypeAnnotations,
  TypeAnnotationPosition, InnerClasses, EnclosingMethod, SourceFile,
  ConstantValue, Code, ...), verified against `javap`.
- `ClassFinder`: `complete`/`completeOwners`/`fillIn`/`loadClass`/
  `includeClassFile`/`scan`, bound to `Symbol.completer`/`DCFH`/`Symtab`.
- Annotation proxies (`AnnotationProxyMaker`), `BadEnclosingMethod`,
  `ProxyType` deproxy, classfile version/preview diagnostics.

### 2.4 Enter / MemberEnter / TypeEnter (P3)

- Full `Env`/`AttrContext`/`TypeEnvs`/`Enter`/`MemberEnter`/`TypeEnter`;
  `Scope` `dup`/`leave`/`import` semantics.
- `TypeEnter` phases (Imports/Hierarchy/Permits/Header/Record/Members),
  default constructors, record/enum members, `finishClass`.
- Module system (`Modules`, 3435 lines), `ModuleFinder`, `Locations`
  (`getLocationForModule(Path)`, `--patch-module`, `--module-source-path`),
  JRT/ct.sym/jmod readers.

### 2.5 Resolve / Types / Infer / DeferredAttr (late P3)

- `Resolve` A+B: `findField`/`findVar`/`findType`/`findIdent`, access checks,
  `selectBest`, `mostSpecific`, `mergeAbstracts`, member-reference family,
  polymorphic signatures, verbose diagnostics, `code/Printer` 1:1.
- `Types`: relations (adapt/rewriteSupers/capture/glb/closure/asSub/...),
  projections, `SignatureGenerator`, 126 audited items.
- `Infer`: InferenceContext, graph solver, `instantiateMethod`/
  `instantiateFunctionalInterface`, polymorphic signatures.
- `DeferredAttr`: DeferredType/Node/Context/Map, speculative and stuck
  strategies; `ArgumentAttr` (six argument kinds, `UniquePos`);
  `AttrRecover`; `StructuralStuckChecker`.
- Key semantics: `InferenceException extends InapplicableMethodException`;
  `ArgumentAttr.speculativeTypes` keyed by `ResultInfo.id`.

### 2.6 Flow / Lint / Annotate (batches 1-3.5)

- `Flow`: AliveAnalyzer/CaptureAnalyzer/AssignAnalyzer, match bindings,
  effectively-final analysis with nested JCDiagnostic.
- `Lint`: `augment`/AugmentVisitor, `-Xlint` parsing and defaults.
- `Annotate`: annotateLater/annotateNow/TypeAnnotate/queue/flush, repeatable
  containers, synthetic annotation types.

### 2.7 Full Attr (batches 5-6)

- Expressions: Ident/Select/Apply/NewArray/Assert/Conditional/Assignop/
  Unary/Binary/TypeCast/Lambda/Reference/NewClass/anonymous classes.
- Statements: Block/loops/Try/synchronized/Switch/SwitchExpr/patterns (type test/binding/guard)/Return/Throw/Yield.
- Types: attribClass/attribClassBody, visitTypeParameter/checkIntersection,
  type-annotation validation.
- Supporting families: `checkAllDefined`/`nonCyclic`/cyclic constructors,
  override/hide/clash, `checkDeprecatedAnnotation`/
  `checkFunctionalInterface`/`checkLeaksNotAccessible`,
  `PostAttrAnalyzer`, `Analyzer`.

### 2.8 TypeAnnotations / Analyzer / B7

- `TypeAnnotations` (~2200 lines): annotation types, targets, positions,
  separateAnnotationsKinds, resolveFrame, visitors, organizeBodies+Signatures.
- Anonymous-class diamond listener (snapshot/replay),
  `NewClass.classDeclRemoved`.

### 2.9 Backend

- `TransTypes` (2328 lines: cast/coerce/retype/bridges/translateClass).
- `TransPatterns` (~2780 lines) and `LambdaToMethod` (10241 lines;
  indy/metafactory, dedup, static capture).
- `Lower` (8775 lines, all visitors: StringConcat, TWR, enum/record, class
  literals, assert, conditionals, accessors).
- `Code` (2974), `Items` (1066), `StringConcat` (778), `Gen` (4333),
  `ClassWriter` (2261), `PoolWriter`, `CRTable`, `StackMapTableFrame` (370).
- `JNIWriter` 1:1.

### 2.10 Assembly and modules

- `mini_compiler` wires the JavaCompiler chain: parse -> modules -> enter ->
  process -> attr -> flow -> desugar (transTypes -> TransPatterns ->
  LambdaToMethod -> Lower) -> gen; `should-stop` supports PARSE/ENTER/
  PROCESS/ATTR/FLOW/TRANSTYPES/TRANSPATTERNS/UNLAMBDA/LOWER/GENERATE.
- Module tests (module-info uses/provides) pass end to end; `java.home`
  discovery order fixed (JAVA_HOME -> PATH -> baseline JDK 17).

### 2.11 Debt cleanup (10-27/28)

- `unimplemented!` reduced from ~349 to **0** in batches; `unreachable!`
  102 -> **0** (unified `util/assert` Assert.error semantics).
- `let _` cleanup 424 -> 412 (justified exceptions kept); panic
  classification table (`AUDIT_PANIC_TABLE.md`, 237 lines).
- Diagnostic audit (935 factories): wired 639 / wireable 67 / not ported 40 /
  out of scope 24 / no upstream trigger 165.

### 2.12 Real projects (Gson / OkHttp)

- **Gson 2.10.1 (80 sources)**: from 161 diagnostics/0 classes to 13 -> 10 ->
  **0 errors and 207 byte-identical classes**; the Gson JUnit suite then
  reported 1258/2, matching javac.
- **OkHttp 3.14.9 (229 sources)**: 0 errors; the official JUnit suites passed (4 classes / 416 tests; internal 15
  classes / 256 tests).
- Root causes: the extra parameter check in `Infer.instantiateMethod`, the
  `finishImports` ordering, the non-upstream `ResolvedConstructorType.
  overloadCheck` special case, `InferenceException` catching, and the
  `speculativeTypes` identity key.

### 2.13 Diagnostic corpus

- 669 upstream langtools examples compared against checked-in expected
  output: 547 -> 612 -> **602 pass / 0 fail / 67 skip** at the time.
- Skip whitelist: annotation processing 47, deprecated source 12, custom file
  manager 2, automatic modules 2, JVM stack trace 1, plugin 1, verbose 1,
  deprecated profile 1.

### 2.14 CLI parity

- Official help/extended help/version/no-args/invalid option/file not found/
  exit codes; `@argfile` (`#` comments, whitespace-separated).
- `--help-lint` (31 keys plus all/none, byte-identical), `-Xstdout`
  (`LineSeparatorWriter`, including CRLF), `-J<flag>` (including
  `-J-Duser.language` affecting diagnostics).
- Diagnostic language: Windows UI language detection; `-J-Duser.language`
  installs the process default locale.
- Deprecated mechanisms removed: `-profile` and `-source 7` are skipped in
  the corpus and not implemented.

### 2.15 Performance

- Pitfall: `-cp .` in the repository root scanned `internal/reference` and
  made a single-file compile take 45 s; skipping it yields 1.26 s (0.70 s in
  a clean directory).
- Release: fat LTO + codegen-units=1; `target-cpu=native` for local
  benchmarks.
- Optimizations: `Name` without bytes (shared name table, corrected
  version), `FxNameMap` (preallocated), flat-name index in `Symtab`,
  Result-based inapplicable channel (-17.5%), `javac-zip` whole-file
  residency + rawzip slice + zlib-rs (-16 to -21%).
- Results (2026-10-31): Gson release 0.973 s (javac 2.40 s); OkHttp release
  24.28 s (javac 8.17 s). The E2E plan records a Gson release median of **0.82 s vs javac 2.44 s**.
- Infeasible approaches: PGO (missing `profiler_builtins`), mimalloc (needs
  a full MinGW/Windows SDK), samply (ETW GBK crash; local patch not
  committed).

### 2.16 Byte-parity effort (OkHttp 69 -> 0)

- `InnerClasses` missing referenced member classes (`SignatureGenerator.classReference` -> `enterInner`, merging
  signature
  class references).
- Lambda synthetic methods' `LineNumberTable` (lambda body positioned at the
  lambda expression; upstream dedup disabled by default).
- `EnclosingMethod` uses the lambda's original enclosing method; local class
  name index keys use the enclosing class flat name.
- Constant-pool key: `Symbol.poolKey = Pair(newOwner, baseSymbol())`;
  inherited interface methods are cloned per call site, `mergeAbstracts`
  uses identity keys, and import-scope clones get `baseSymbol`.
- `try-with-resources` restores `make.pos` on exception paths; string-switch
  synthetic nodes are positioned at the switch statement.
- Final: **OkHttp 547/547, Gson 207/207** byte-identical at the time.

### 2.17 E2E plan and gates

- `docs/e2e-test-plan.md` plus `scripts/e2e.ps1`: L1 unit (846), L2
  diagnostics (669), L3 project bytes (Gson/OkHttp), L4 CLI, L5 performance.
- L1 green on every commit; L2+L3+L4 daily; L1-L5 before release.

### 2.18 Diagnostic-order effort (2026-11-01)

Problem: OkHttp diagnostics matched in content/position/count, but two
warnings were ordered differently (`RecordedRequest.java:139:17`,
`FakeSSLSession.java:73:10`). The earlier "HashMap iteration order"
conclusion was wrong; the real causes were:

1. The port lacked the `TypeEnter` phase-queue semantics (`Phase.completeEnvs` `firstToComplete`, `AbstractMembersPhase`
   `todo/completing`, `HierarchyPhase` as class completer, `TypeEnter.java:
   249-1022`); the `Phase` structure, `topLevelPhase`, and
   `HierarchyCompleter` were added.
2. `Resolve.member_scope` did not complete `Symbol.members()` first, so
   `Http2Reader` was appended late (port #153 vs javac #73).
3. `TypeEnter.complete` did not reinstall the completer when completion was
   closed (upstream `sym.completer = this`).
4. `MemberEnter` did not complete classes referenced by member types (upstream `attribType` completion side effect).
5. The deferred lint queue must be keyed by the `JCTree` object identity (`tree.pos()`), not by a numeric position; the
   port merged or flushed
   same-offset trees across files (a spurious `FileSystem.java:57:9`).
6. `Check.checkDeprecated` must be deferred; `Attr.visitMethodDef`/
   `visitVarDef` must `setPos(tree.pos())` at declaration and `flush(tree.
   pos())` in a finally block; class flush moved after `attribClassBody`.
7. `warnOnExplicitStrictfp` no longer sets a position (upstream only
   reports; the caller sets the key).

Result: OkHttp `todo` append order **229/229**, `-XDrawDiagnostics`
**11/11 lines positionally identical**, with Gson 207/207, OkHttp 547/547,
lib 846, and E2E 8/8 preserved.

## 3. Key technical decisions

| Decision              | Content                                                                                      | Rationale                                     |
|-----------------------|----------------------------------------------------------------------------------------------|-----------------------------------------------|
| arena ids             | `TreeId`/`SymbolId`/`TypeId` instead of object references                                    | ADR 0001                                      |
| explicit services     | no global context; key services passed explicitly                                            | ADR 0001                                      |
| diagnostic resources  | upstream `.properties` embedded with `include_str!` plus typed factories                     | `compiler.properties`                         |
| Chinese patches       | 24 entries appended to `compiler_zh_CN.properties`, 936/936 coverage                         | localization                                  |
| reentrancy adaptation | field-level `Cell`/`RefCell` with short borrows; `complete_with_attr` through the Attr chain | Java single-thread reentrancy vs Rust borrows |
| traceability          | per-file registry in `docs/PORTING_MAP.md` (headers are ordinary module docs)                | auditability                                  |
| resource reads        | JRT/ct.sym/jmod/zip resident in memory with indexes                                          | performance                                   |

## 4. Verification matrix (current)

| Area                        | Result                                          |
|-----------------------------|-------------------------------------------------|
| Gson bytes                  | **208/208**                                     |
| OkHttp bytes                | **542/542**                                     |
| Guava bytes                 | **1970/1970**                                   |
| OkHttp `-XDrawDiagnostics`  | 11 lines, positionally identical                |
| OkHttp append order (probe) | 229/229                                         |
| `javac-core --lib`          | **743 passed, 0 failed**                        |
| Java 9-17 feature matrix    | **43/43**                                       |
| CLI integration             | 5/5                                             |
| diag-corpus                 | **588 pass, 14 fail, 67 skip** (602 comparable) |
| Gson release performance    | ~0.82 s vs javac ~2.44 s (median of 5)          |

## 5. Known gaps

1. JVM-host mechanisms: annotation processors (`-processor`/`-proc:only`),
   `-Xplugin`, `-doe` stack traces, custom `-filemanager`; omitted by
   design and skipped in the corpus.
2. Deprecated mechanisms (`-profile`, `-source 7`) are not implemented.
3. 67 diagnostic skips (see 2.13).
4. Performance: OkHttp release remains slower than javac (24.28 s vs
   8.17 s at the 2026-10-31 measurement); Gson is faster.
5. Optional: add the 11 OkHttp diagnostic lines to the L2 gate; add live
   reference mode instead of checked-in expected output; add CI.
6. Declared-position semantics remain split between diagnostics (name token)
   and line-number tables (declaration start); see PN-001.

## 6. Methodology

- **Root cause first**: build an evidence chain (javac-side patched prints
  and port probes) before changing code; change one variable at a time.
- **Upstream source is the only judge**: every fix points to a concrete
  file/method/behavior; port-specific adaptations must justify themselves and
  are removed when upstream semantics contradict them.
- **Do not trust earlier conclusions**: the `HashMap` ordering theory was
  disproved; diagnostic order comes from the TypeEnter phase queue and the
  deferred-lint key.
- **Remove probes**: temporary `eprintln!` and `DSH_TRACE_*` must be cleaned
  up.
- **Isolate the working directory before measuring performance**: `-cp .`
  scans the reference tree.
- **Pay down debt in batches**: no leftover `unimplemented`/`unreachable`/
  silent placeholders.
- **Keep tests and docs in sync**: the E2E plan, gates, skip whitelist, and
  failed approaches (PGO/mimalloc/samply) are all recorded.

## 7. Commit index (by topic)

| Topic                   | Representative commits                     |
|-------------------------|--------------------------------------------|
| P0 infrastructure       | `4b08a1e`                                  |
| parser/tree oracle      | `fe31f28`, `9a04e88`                       |
| ClassReader/ClassFinder | `76d0437`, `4972554`, `ac2da35`            |
| Enter/TypeEnter         | `c242a3c`, `50aedec`, `4af90e3`            |
| Resolve/Types/Infer     | `1be2159`, `62a76ec`, `ff75f6d`            |
| Flow/Lint/Annotate      | `b2d12f2`, `66cfcbe`, `a66c58c`            |
| full Attr               | `3c17e8e`, `68eefac`                       |
| backend                 | `b34ebd2`, `5778054`, `d7650b7`, `2f8f51d` |
| debt cleanup            | `f496fe9`, `5b664fe`                       |
| lambda/patterns         | `4973fa9`, `9356682`                       |
| Gson byte parity        | `b979d25`                                  |
| OkHttp green            | `e0a5d1f`, `f0f0894`                       |
| CLI                     | `3758e21`, `ce38d06`                       |
| performance             | `393a588`, `b9a47e5`, `b34685e`            |
| byte-diff effort        | `77dac48`, `b4eb1f4`, `b02f67a`            |
| diagnostic order        | `8459da1`, `0e16acd`                       |
| test harness            | `8486e92`                                  |

## 8. Next steps (not commitments)

- Add the 11 OkHttp `-XDrawDiagnostics` lines to the L2 gate.
- Add an OkHttp release benchmark to L5 with a regression threshold.
- Add live reference mode to the diagnostic corpus.
- Resolve PN-001 (declaration positions and line-number sources).
- Wire L1-L4 into CI.