# Doc style demo (new / rewritten code only)

This file is the reference example for PORTING_RULES 26.2. It applies to code that is
newly written or rewritten; existing legacy code is not changed.

## A. Accepted Rust code

```rust
//! Type lowering support. Upstream locations live in docs/PORTING_MAP.md —
//! never in source comments.

/// A navigator assembling the mapping from local class symbols to class
/// definition trees.
///
/// Only one case is handled; all other cases simply traverse down the tree.
pub struct ClassMap {
    /// All encountered class definitions, keyed by their symbol.
    classdefs: HashMap<SymbolId, TreeId>,
}

impl ClassMap {
    /// Records every class definition encountered during the traversal.
    ///
    /// # Arguments
    /// - `tree` — the class definition being visited
    pub fn visit_class_def(&mut self, tree: TreeId) {
        self.classdefs.insert(class_symbol(tree), tree);
        self.scan(tree);
    }
}

/// Maps a class symbol to its definition tree.
///
/// # Arguments
/// - `c` — the class symbol whose definition is requested
///
/// # Returns
/// The recorded class definition tree for `c`
///
/// # Panics
/// Panics when `c` has no recorded definition (upstream `AssertionError`)
pub fn class_def(&self, c: SymbolId) -> TreeId {
    ...
}

/// Translates one switch or switch-expression.
///
/// # Arguments
/// - `tree` — the switch tree being lowered
/// - `selector` — the selector expression
/// - `cases` — the case clauses
///
/// # Panics
/// Panics when `tree` is neither SWITCH nor SWITCH_EXPRESSION
/// (upstream `AssertionError`).
fn handle_switch(&mut self, tree: TreeId, selector: TreeId, cases: List<TreeId>) {
    // expand multiple label cases:
    let mut converted_cases = ListBuffer::new();

    // default / single label: keep as-is; multiple labels expand:
    //   case C1, C2, C3: ...
    //   =>
    //   case C1:
    //   case C2:
    //   case C3: ...
    ...
    // A switch over a boxed primitive. Pattern matching switches are already
    // translated by TransPatterns, so all non-primitive types are boxed
    // primitives:
    ...
}
```

Required properties:
- upstream Javadoc/comment semantics are preserved as `///` or English inline comments;
- every public item has a `///` doc of at least 3 non-empty lines;
- `# Arguments` for parameters, `# Returns` for non-unit results, `# Panics` when it can panic;
- zero position references in source (no `File.java:line`, no `Java:` signature copies, no per-member upstream anchors);
- upstream locations are recorded only in docs/PORTING_MAP.md.

## B. Position mapping file (docs/PORTING_MAP.md) — line numbers are allowed there only

| Local symbol | Upstream location | Notes |
|---|---|---|
| `comp/lower.rs::ClassMap::visit_class_def` | `Lower.java:212` | direct |
| `comp/lower.rs::handle_switch` | `Lower.java:2105-2205` | branch order preserved |
| `jvm/pool_constant.rs::LoadableConstant` | `PoolConstant.java:33-127` | trait adapter |

## C. Forbidden (noise) in new source

```rust
/// Upstream: PoolConstant.java:72-74            // forbidden: position reference
```

## D. Audit

For added/modified public items only:
- doc lines must not match `[A-Za-z0-9_]+\.java:[0-9]+`, `Java:` prefix, or internal batch/section markers;
- doc must provide summary + `# Arguments`/`# Returns`/`# Panics` as applicable;
- each new public item gets a `PORTING_MAP.md` entry (checked by the reviewer, not by the compiler).
