# Code and Documentation Style

rust-javac is an open-source port of OpenJDK javac. All repository text is
written in English: source comments, module documentation, `docs/`, commit
messages, and issue/PR descriptions.

## 1. Keep porting metadata out of the source

Source files contain conventional Rust documentation: what the module does,
the key types and invariants, and non-obvious design decisions. They do **not**
contain:

- upstream source paths, method anchors, line numbers, or commit hashes;
- porting status, "ported in this file" inventories, or NOT PORTED lists;
- links to `docs/PORTING_MAP.md` or `docs/PORTING_NOTES.md`.

Per-module mapping, status, and deviation records live in `docs/` only. A
reader of the source should never need the porting paperwork to understand
the code.

## 2. Module documentation

Every module starts with a `//!` block of two to six lines:

1. **What it does** (one sentence).
2. **Key types and flow** (one or two sentences): the main entry points and
   how data moves through them.
3. **Non-obvious constraints** (optional): ordering, ownership, invariants,
   or a Rust-specific representation choice.

Example:

```rust
//! Bytecode generation.
//!
//! `Gen` lowers attributed and desugared trees to JVM instructions: it walks
//! statements and expressions, emits code through `Code`, and records line
//! numbers, local variable tables, and stack-map frames. Constant-pool
//! management and class-file serialization live in `ClassWriter` and
//! `PoolWriter`.
```

A one-line header such as `//! Infer: inference.` is not documentation and
is rejected in review.

## 3. Comments

A comment must state something the code does not: an invariant, a
non-obvious reason, a constraint, or the meaning of a magic value. Do not
restate the next line of code.

### 3.1 Implementation rationale

Explain why the Rust code takes this shape:

```rust
// Rust: trees live in an arena and are addressed by `TreeId`, so phases can
// borrow disjoint fields instead of sharing `Rc<RefCell<_>>`.
```

### 3.2 Behavior deviations

When local behavior intentionally differs from javac, write one concise
note next to the code and keep the full record in `docs/PORTING_NOTES.md`:

```rust
// The declaration position is the name token; initializer line numbers use
// the declaration start. See PN-001.
```

## 4. Forbidden patterns

- Upstream paths, class names, or commit hashes in source comments.
- Links to local documentation from source comments.
- Template one-liners and bare pointers.
- Historical narration (`previously X failed because Y, so now Z`).
- Duplicating the same fact in the module header, the item, and inline.
- Non-English text in code, comments, or `docs/`.

## 5. Naming

- Types mirror the upstream class names (`Attr`, `Check`, `TreeInfo`).
- Functions and fields use snake_case.

## 6. Errors and ownership

- User-reachable failures return `PResult<T>` (`Result<T, Diagnostic>`).
- `expect()` is reserved for internal invariants and must state the
  invariant.
- Panics are for impossible states only; `catch_unwind` is a last-resort
  boundary, not control flow.
- Cross-phase mutable state lives in `Session`; single-phase mutation uses
  `&mut`; read-only sharing uses `&T` or `Rc<T>` without `RefCell`.
- Callbacks use `FnMut`; do not store `Rc<RefCell<dyn Fn>>`.

## 7. Documentation layout

- `docs/PORTING_MAP.md`: per-module mapping, status, and the original
  registry moved out of headers.
- `docs/PORTING_NOTES.md`: behavioral deviations with stable ids (`PN-xxx`).
- `docs/STYLE.md`: this file.

## 8. Verification gates

`scripts/gate.py` selects the minimal suite from the current diff.

| Level | Commands                                  | When                                   |
|-------|-------------------------------------------|----------------------------------------|
| none  | nothing                                   | docs/scripts/comment-only changes      |
| L0    | `cargo test -p javac-core --lib`          | any code change                        |
| L1    | L0 + `check_repros.py` + `check_gson.py`  | parser, diagnostics, CLI               |
| L2    | L1 + `check_jdkcompiler.py` + `check_guava.py` + `check_okhttp.py` | types, inference, Attr, Lower, codegen |
| L3    | full suite                                | merge points, daily, large refactors   |

Run the level declared by the diff; do not escalate on failure. Full L3 is
reserved for merge points and refactor boundaries.

## 9. Commits

- One logical change per commit; do not commit per line.
- State what changed, which gate ran, and the result.
- Do not rewrite published history; do not squash unrelated work.
