# Rust Purity

## Goal and acceptance

The port keeps javac's shared mutable object graph where the Java design
requires sharing, but it must not hide accidental aliasing behind
`Rc<RefCell<_>>`. A remaining occurrence is accepted only when its record
states:

- why the object must be shared across the call sites that reach it;
- where the mutable borrow starts and ends, and what invariant prevents a
  nested borrow;
- whether the occurrence is a real object, a public API contract, or a
  test-only value.

The target is therefore not zero occurrences. The target is a complete,
reviewable ledger with no unexplained `try_borrow` fallback and no
fabricated second `&mut`.

## Measurement

The count is syntactic: every occurrence of `Rc<RefCell<` plus
`Rc<std::cell::RefCell<` under `crates/`:

```python
import pathlib, re

p1 = re.compile(r"Rc<RefCell<")
p2 = re.compile(r"Rc<std::cell::RefCell<")
o1 = o2 = 0
for path in pathlib.Path("crates").rglob("*.rs"):
    text = path.read_text(encoding="utf-8", errors="replace")
    o1 += len(p1.findall(text))
    o2 += len(p2.findall(text))
print(o1, o2, o1 + o2)
```

The starting count was **541** (`479` + `62`). The current count is **225** (`213` + `12`).

The ledger below also lists eleven textual fragments that the fixed script
counts but that are not runtime types: seven `...`, three `..`, and one
fragment embedded in a comment. Excluding them leaves **214** semantic
occurrences; the table keeps the placeholder row so that it reconciles with
the script.

## Implemented changes

- **Unsafe aliasing removed.** The fabricated second `&mut` used while an
  outer `RefMut` was live is gone. Call sites that only need observation use
  a shared borrow or a `&self` method.
- **Resolve, Symtab, Log, TreeMaker converged.** Their mutable state is
  field-level or phase-owned; the outer `Rc<RefCell<T>>` was replaced by
  `Rc<T>` where no cross-phase mutable borrow remained.
- **TreeMaker state made explicit.** Position and top-level construction state
  are fields rather than an outer cell; all construction methods take
  `&self`.
- **Dead Attr reverse edges removed.** MemberEnter and TypeAnnotations held
  Attr cells that were only injected, never read. Flow's cell was replaced by
  an explicit `&mut Attr` argument on the resource-close path.
- **Annotation flushing became epoch-based.** A flush requested without an
  Attr records a snapshot of the four queues. A later Attr boundary drains
  the epochs in FIFO order, preserving the relative order of validate and
  normal tasks instead of merging or reordering them.
- **Class entry carries Attr when available.** `class_enter_list` accepts an
  optional `&mut Attr`; the MemberEnter signature path passes it, while
  genuinely Attr-less entry points record their import checks for the next
  Attr boundary.

## Remaining ledger

| Type or group                                                                                                                                                                                                         | Count | Category                  | Why it is retained; borrow boundary                                                                                                                                                        | Prerequisite to remove                                                                                                    |
|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|------:|---------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------|
| `Attr`                                                                                                                                                                                                                |    26 | Real sharing              | Phase-wide attribution service; a `&mut Attr` is passed explicitly through one operation, and the cell only installs/reaches the service. Nested calls must not re-borrow it.              | Replace the service graph with phase-owned Attr instances; not a local ownership change.                                  |
| Attr nested/optional forms                                                                                                                                                                                            |     5 | Real sharing              | `Option<Rc<RefCell<Attr>>>` reaches the same service through completion and analysis cells.                                                                                                | Same service-graph redesign as the Attr row.                                                                              |
| `Option<Rc<Annotate>>` and qualified forms                                                                                                                                                                            |    15 | Real sharing              | Attr and Annotate form a genuine construction cycle; Annotate is needed by completion and queued annotation work. Borrows are taken per callback, never held across a flush.               | Split the Attr/Annotate construction cycle or make annotation scheduling own its services.                                |
| `TypeEnter` and qualified/nested forms                                                                                                                                                                                |     7 | Real sharing              | Class completion is synchronous and needs Attr for imports, hierarchy, and members; the cell is the service slot, not a transient borrow.                                                  | Completion must carry or defer Attr without returning an incomplete symbol.                                               |
| `HashMap<TreeId, TranslationContext>`                                                                                                                                                                                 |     8 | Real sharing              | Shared by LambdaToMethod and its clone; map borrows are short lookup/insert scopes.                                                                                                        | Detach the cloned translator's context ownership.                                                                         |
| Other service cells (ArgumentAttr, DeferredAttrService, AttrRecover, Operators, Analyzer, CompileStates, Flow, Modules, TypeAnnotations, Preview, Profile, ModuleFinder, JavaFileManager, Env, TreeMaker, SymtabMaps) |    38 | Real sharing              | Optional service slots are filled during wiring and read during their phase; each borrow is short and never spans a nested service construction.                                           | Remove the corresponding optional service slot or make the phase own the service.                                         |
| Lower                                                                                                                                                                                                                 |     9 | State machine             | A lowering workspace: current class/method, environments, access caches, maps and stacks. It is used across the whole lowering traversal and is carried by codegen and lambda translation. | Own the lower phase as a single owned object; 226 methods and 27 written fields make field-level cells counterproductive. |
| PoolWriter                                                                                                                                                                                                            |    11 | State machine             | Constant-pool and bootstrap-method state is mutated across writer helpers; borrows must cover a complete pool mutation.                                                                    | Rewrite the writer helpers around an owned mutable writer.                                                                |
| Code                                                                                                                                                                                                                  |     8 | State machine             | Bytecode buffers, stack map state, locals, labels and line tables are one mutable emission workspace.                                                                                      | Own code generation as a phase object; helper calls must accept the same `&mut`.                                          |
| ClassReader                                                                                                                                                                                                           |     6 | State machine             | Reader cursor, buffer, signature buffer and current class/owner state span one class-file read.                                                                                            | A lifecycle/self-reference redesign for the reader and its proxies.                                                       |
| TransTypes                                                                                                                                                                                                            |     7 | Trait receiver constraint | The translator implements a `TreeVisitor` whose methods require `&mut self`; a shared proxy would need 68 forwarding methods for seven counted occurrences.                                | Change the visitor contract or accept the forwarding adapter; both are larger than the benefit.                           |
| AnnotateQueues                                                                                                                                                                                                        |     2 | State machine             | Scheduler queues and pending epochs are mutated by enqueue and drain; the lock is per queue operation.                                                                                     | Own the scheduler inside the Annotate phase.                                                                              |
| EnumMapping                                                                                                                                                                                                           |     3 | State machine             | Enum switch mapping state is accumulated during lowering.                                                                                                                                  | Fold into the owned lowering workspace.                                                                                   |
| `HashMap<SymbolId, Rc<RefCell<Code>>>`                                                                                                                                                                                |     3 | State machine             | Per-method code buffers are shared between generation and later writes.                                                                                                                    | Redesign method-code ownership.                                                                                           |
| Bits                                                                                                                                                                                                                  |     2 | State machine             | Inference/attribute bitset state is updated in place.                                                                                                                                      | Replace with a phase-owned bitset if the owning service is threaded.                                                      |
| `Vec<u8>`, `Vec<TreeId>`, `Vec<TypeId>`, other Vec/Map state                                                                                                                                                          |    18 | State machine             | Buffers and worklists are accumulated across one traversal or one emitted artifact; shared because helper calls mutate them.                                                               | Convert each to a field of an owned phase object, one family at a time.                                                   |
| `dyn TaskListener`                                                                                                                                                                                                    |    12 | Public API                | Public javac listener interface; `started`/`finished` take `&mut self`, and add/remove compares `Rc` identity.                                                                             | Changing the public listener contract.                                                                                    |
| `dyn JavacTask`                                                                                                                                                                                                       |     4 | Public API                | Public task interface exposed to callers.                                                                                                                                                  | Outside compiler internals; not a local refactor.                                                                         |
| `dyn DocCommentTable`                                                                                                                                                                                                 |     5 | Public API                | Doc-comment table interface with mutable updates; shared by parser and tree access.                                                                                                        | Redesign the parser/tree doc-comment ownership.                                                                           |
| `dyn DiagnosticFormatter<D>` (including `u32`)                                                                                                                                                                        |     6 | Public API                | Formatter trait object exposed by the diagnostics API.                                                                                                                                     | Change the public diagnostic API.                                                                                         |
| `dyn Writer`, `dyn std::io::Write`, boxed write forms                                                                                                                                                                 |     7 | Trait receiver constraint | Output sinks are trait objects whose write methods require mutable access; identity is shared by writers and codegen.                                                                      | Redesign the writer abstraction around owned sinks.                                                                       |
| `dyn GenFinalizer`                                                                                                                                                                                                    |     3 | Trait receiver constraint | Codegen finalizer callback is invoked mutably per generated class.                                                                                                                         | Own finalizers in the generation phase.                                                                                   |
| `dyn Completer`                                                                                                                                                                                                       |     2 | Trait receiver constraint | Completion callbacks are invoked with `&mut self` and stored in symbol slots.                                                                                                              | Change the completion contract.                                                                                           |
| Local/test candidates (`A`, `Vec<&str>`, `Vec<String>`, `Vec<Entry>`, `Vec<TableSlot>`)                                                                                                                               |     7 | Locally actionable        | Small test fixtures or parser/generator-local buffers; no cross-phase service identity was found. Each still needs an ownership check before changing.                                     | Audit the concrete owner; replace the shared cell with an owned value or `&`.                                             |
| Textual placeholders (`...`, `..`, comment fragment)                                                                                                                                                                  |    11 | Not runtime               | Documentation/comments; the fixed script counts them, no object exists.                                                                                                                    | None.                                                                                                                     |

Category totals: **99** real sharing, **62** state machine, **27** public API, **19** trait receiver constraint, **7**
local candidates, **11** textual
placeholders; total **225**.

### Textual placeholder sources

The fixed script counts these eleven fragments; none is a runtime object. The
last entry is a documentation reference to a real type whose code occurrences
are counted separately.

| Location                                                              | Fragment                                          |
|-----------------------------------------------------------------------|---------------------------------------------------|
| `crates/javac-core/src/compiler_context.rs:10`                        | `Rc<RefCell<...>>`                                |
| `crates/javac-core/src/parser/doc_comment_parser.rs:18`               | `Rc<RefCell<...>>`                                |
| `crates/javac-core/src/util/forwarding_diagnostic_formatter.rs:16`    | `Rc<RefCell<...>>`                                |
| `crates/javac-core/src/util/name.rs:14`                               | `Rc<RefCell<...>>`                                |
| `crates/javac-core/src/code/scope.rs:169`                             | `Rc<RefCell<...>>`                                |
| `crates/javac-core/src/code/scope.rs:414`                             | `Rc<RefCell<...>>`                                |
| `crates/javac-core/src/comp/argument_attr.rs:93`                      | `Rc<RefCell<...>>`                                |
| `crates/javac-tools/src/tools/java_compiler.rs:177`                   | `Rc<RefCell<..>>`                                 |
| `crates/javac-tools/src/tools/java_compiler.rs:178`                   | `Rc<RefCell<..>>`                                 |
| `crates/javac-tools/src/lib.rs:28`                                    | `Rc<RefCell<..>>`                                 |
| `crates/javac-core/src/util/forwarding_diagnostic_formatter.rs:11-12` | `Rc<RefCell<dyn` / `//! DiagnosticFormatter<D>>>` |

The split fragment describes `Rc<RefCell<dyn DiagnosticFormatter<D>>>`; the
actual code uses of that type are counted in the `dyn DiagnosticFormatter<D>`
and `dyn DiagnosticFormatter<u32>` rows, so it is not a missed occurrence.

## Conclusion

- The local candidates (7) are the only remaining occurrences that can be
  removed without redesigning a service, a phase workspace, or a public/trait
  contract. They should be audited and, if genuinely local, collapsed to
  owned values.
- The 99 real-sharing occurrences are not debt. They exist because Attr,
  Annotate, TypeEnter, completion, flow, type annotations, and the optional
  service slots form a real object graph, and their borrow boundaries are
  explicit.
- The 62 state-machine occurrences are phase workspaces. Replacing them with
  field-level cells would multiply mutable fields and borrow guards without
  proving ownership; they should remain until the owning phase is redesigned.
- The 46 public/trait occurrences are interface constraints. Their
  `&mut self` methods are part of the javac API; changing them would change
  the contract, not remove debt.
- The 11 placeholder matches are a measurement artifact.
- The project should treat **225 as the honest adaptation surface**, not as a
  failure to reach zero.

## Verification baseline

- `cargo test -p javac-core --lib`: 747 passed.
- Gson: 208/208 classes.
- Guava: 1970/1970 classes.
- OkHttp: 542/542 classes.
- `check_examples.py`: 602 total, 588 pass, 14 fail, 67 skip.
