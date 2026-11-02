# Porting Deviations

This registry lists the known behavioral differences between rust-javac and
OpenJDK javac at `jdk17u @ 982d53c` (jdk-17.0.19-ga).

Each entry has a stable id (`PN-xxx`), the observed behavior, the upstream
behavior, the local behavior, the impact, the current status, and the plan.
Source comments reference the id; the full record stays here.

## PN-001 Field and parameter declaration positions

- Observed: three diagnostic examples place the caret on the first modifier
  instead of the declared name: `DeprecatedAnnotationHasNoEffect`,
  `IllegalComboModifiers`, `VarNotIntializedInDefaultConstructor`.
- Upstream: `JavacParser.variableDeclarators` and `variableDeclaratorId`
  take `token.pos` (the name token); `JCVariableDecl.pos` is the name, and
  diagnostics use it. Line-number tables for field initializers, however,
  are emitted from the declaration start (`TreeInfo.getStartPos`), which is
  the annotation line for annotated fields (e.g. Guava `CacheBuilder` 295).
- Local: the parser uses `mods.pos` for field and parameter declarations, so
  diagnostics point at the modifiers. Switching to the name token aligns the
  diagnostics but shifts the field-initializer line numbers in 10 Guava
  classes by one.
- Impact: either three diagnostic examples fail, or 10 Guava classes differ.
- Status: root cause identified; the change was reverted to keep Guava
  byte-identical. The missing piece is emitting field-initializer line
  numbers from `TreeInfo.getStartPos` while keeping `VarDecl.pos` at the
  name.
- Plan: P1/P2 (fix the line-number source first, then the parser positions).
- Verification: `check_examples.py --filter` for the three cases;
  `check_guava.py`.

## PN-002 `dupTo` clone flags

- Observed: `InferenceContext.dupTo` produces copies whose clone flags and
  cached state are not fully aligned with upstream.
- Upstream: `dupTo` duplicates an inference context while preserving clone
  semantics.
- Local: some flags are not copied.
- Impact: no byte difference has been observed in the current corpora; it is
  a latent correctness risk.
- Status: registered, not fixed.
- Plan: audit together with P3 (inference/error model work).
- Verification: library tests plus Guava/Gson parity.

## PN-003 Module-system diagnostics (10 corpus cases)

- Observed: ten `check_examples.py` cases fail:
  `HasBeenDeprecatedModule`, `HasBeenDeprecatedRemovalModule`,
  `MultiModuleOutputCannotBeExplodedModule`,
  `NotDefAccessDoesNotReadFromUnnamed`, `NotDefAccessDoesNotReadUnnamed`,
  `NotDefAccessNotExportedFromUnnamed`,
  `NotDefAccessNotExportedToModuleFromUnnamed`,
  `PackageClashFromRequiresInUnnamed`, `PackageEmptyOrNotFoundWarning`,
  `WarnModuleNotFound`.
- Upstream: the complete module resolution, readability, lint, and
  multi-module output checks.
- Local: the module system is partially implemented; these diagnostic paths
  are missing.
- Impact: ten diagnostic examples fail; module-heavy projects behave
  differently.
- Status: registered, not fixed.
- Plan: separate feature work after the P3/P4 refactors; not part of the
  refactoring itself.
- Verification: `check_examples.py --filter` for the module cases.

## PN-004 Missing `IllegalSignature` generation error

- Observed: the `IllegalSignature` corpus case misses
  `error while generating class ... (illegal signature attribute for type CAP#1)`.
- Upstream: `Gen`/`ClassWriter` reports the failure while emitting the
  Signature attribute.
- Local: the failure path is not detected or not reported.
- Impact: one diagnostic example fails; class generation for such types
  diverges.
- Status: registered, not fixed.
- Plan: add the generation error path after P3 (error channel).
- Verification: `check_examples.py --filter IllegalSignature`.

## PN-005 Annotation processing, plugins, file managers, stack traces

- Observed: 67 `check_examples.py` cases are skipped; `javac-tools` and
  `javac-source` contain unimplemented simulation APIs.
- Upstream: `-processor`, `-Xplugin`, custom `JavaFileManager`, `-doe`.
- Local: not implemented.
- Impact: projects with annotation processors cannot be compiled; the tool
  API surface is incomplete.
- Status: explicitly unsupported and registered.
- Plan: out of scope for P0-P5; separate feature work.
- Verification: covered by the skip list.

## PN-006 `Rc<RefCell<...>>` adaptation layer

- Observed: `Rc<RefCell<` appears 530 times and `borrow_mut()` 2130 times in
  `javac-core`.
- Cause: a 1:1 port of Java's shared mutable object graph (log, warner,
  scopes, inference contexts, lambda translation state).
- Impact: runtime borrow panics, reduced readability, no `Send`/`Sync`.
- Status: technical debt, explicitly an adaptation layer.
- Plan: P2, batch by hotspot (`lambda_to_method` -> `codegen` ->
  `argument_attr` -> `check` -> `attr`).
- Verification: each batch runs L1/L2; `BorrowMutError` is a hard failure.

### P2 progress

- P2 has converged thirteen types so far: MemberEnter, Warner, TypeEnvs, Enter,
  FileQueue, Todo, ModuleFinder, DeferredLintHandler, SpeculativeCache,
  AnnotationTypeMetadata, Items, ArgumentTypeData and ClassWriter.
  `Rc<RefCell<` dropped from 530 to 424 occurrences. The L2 gate
  (Gson/Guava/OkHttp byte parity) passes.
- The unsafe `with_reentrant_mut` helper (a second `&mut T` fabricated while an
  outer `RefMut` was live) was removed; its only caller invokes a `&self`
  method through a shared borrow.
- `ClassWriter` converged through the three-step pattern: buffers, hooks and
  flags made interiorly mutable; the `StackMapTableWriter` trait converted to
  `&self`; then all 44 writer methods took `&self` and the type is shared as
  `Rc<ClassWriter>`.
- `Resolve` is the next low-risk target: it has no `&mut self` methods (all
  state is already in `Cell`/`RefCell` fields), so `Rc<RefCell<Resolve>>` is
  vestigial. The type appears in six field sites -- `flow.rs` (`rs`),
  `lambda_to_method.rs` (`rs`), `lower.rs` (`rs`), `trans_patterns.rs`
  (`rs`), `trans_types.rs` (`resolve`) and `mini_compiler.rs`
  (`resolve`) -- plus `Resolve::instance`'s return type. `Attr.rs` stores it
  as `Rc<RefCell<Option<Rc<RefCell<Resolve>>>>>` (the outer RefCell makes the
  service installable), which becomes `Rc<RefCell<Option<Rc<Resolve>>>>`; the
  two shapes must not be mixed in one bulk replacement.
- Two integration shapes need interface-first work rather than bulk
  replacement:
  - `Warner` is now interiorly mutable and shared as `Rc<Warner>`; callers
    that need the copy-back semantics use `Warner::copy_from`.
  - `TypeEnter` is nested (`Enter.type_enter:
    Rc<RefCell<Option<Rc<RefCell<TypeEnter>>>>>`, `self_ref:
    RefCell<Weak<RefCell<TypeEnter>>>`) and its helper trait uses `&mut self`;
    migrate the three storage sites and the trait before changing the type.
- `PoolWriter` keeps `pool: WriteablePoolHelper` as a mutable field and mutates
  several counters; internalize the helper first.

## PN-007 Panic-based exception channel

- Observed: `InapplicableMethodException` and `InferenceException` are
  implemented with panic payloads and `catch_unwind`; `expect()` appears
  3008 times.
- Upstream: Java exceptions (`throw`/`catch`).
- Local: panics plus thread-local stacks.
- Impact: user-reachable paths can panic; propagation is opaque.
- Status: technical debt.
- Plan: P3 (resolve/infer first, then attr/check); classify and reduce
  `expect()`.
- Verification: negative-input tests must not panic; example failures must
  not increase.

## PN-008 Windows launcher line filtering

- Observed: on internal errors (exit 4) the javac launcher prints
  `printing javac parameters to: <cwd>\javac.<timestamp>.args`, whose content
  is not reproducible.
- Handling: `expected_gen.py` filters the line during normalization; the
  baseline was cleaned.
- Status: handled.
- Verification: `check_examples.py --filter DuplicateModule`.

## PN-009 Guava warning count

- Observed: javac emits 41 warnings while rust-javac emits 53 when compiling
  Guava.
- Cause: diagnostic-position and lint parity is incomplete.
- Impact: stderr differs; class files are unaffected.
- Status: registered, not fixed.
- Plan: diagnostic parity work, related to PN-001.
- Verification: the warning comparison in `check_guava.py`.

## PN-010 Not-ported method inventory

- Observed: 239 `NOT PORTED` lines across 29 files; 343
  `unimplemented!`/`todo!`/`UnsupportedOperationException` markers, mostly in
  the `javac-tools`/`javac-source` simulation layer.
- Impact: edge cases may be missing or panic; the main compilation path is
  covered.
- Status: tracked in `docs/PORTING_MAP.md` per file and in source comments.
- Plan: clear incrementally with the owning phase; no promise of a one-shot
  port.
- Verification: the corpus and library tests for the owning area.

## Template

## PN-xxx Title

- Observed:
- Upstream:
- Local:
- Impact:
- Status:
- Plan:
- Verification:
