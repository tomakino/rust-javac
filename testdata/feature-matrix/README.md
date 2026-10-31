# Java 9–17 feature matrix

Compile-and-compare corpus for the rust-javac port: every unit is compiled by the
official `javac` and by `rust-javac` with the same arguments, then the produced
`.class` files are compared by relative path and SHA-256, and the diagnostics
and exit codes must match.

Run it from the repository root:

```powershell
pwsh ./scripts/feature-matrix.ps1
pwsh ./scripts/feature-matrix.ps1 -EnablePreview   # include --enable-preview units (none yet)
```

## Layout

- `javaN/<feature>/` — one compilation unit per directory (`*.java` directly or
  recursively below it).
- `java9/modules/` — a single named module below `src/feature.matrix/`; compiled
  with `--module-source-path src`.

## Coverage

| Release | Feature | Unit |
|---|---|---|
| 9 | private interface methods | `java9/interface-private-methods` |
| 9 | enhanced try-with-resources | `java9/twr-enhanced` |
| 9 | modules (`module-info`, `exports`, `uses`, `provides`) | `java9/modules` |
| 10 | local variable type inference | `java10/var-inference` |
| 11 | `var` in lambda parameters | `java11/lambda-var` |
| 14 | records (finalized in 16) | `java14/records` |
| 14 | `instanceof` pattern (finalized in 16) | `java14/instanceof-pattern` |
| 15 | text blocks | `java15/text-blocks` |
| 16 | record + pattern combination | `java16/records-patterns` |
| 17 | sealed types / permits | `java17/sealed`, `java17/sealed-advanced` |
| 17 | switch expressions (`yield`, arrow) | `java17/switch-expression` |
| 17 | type annotations | `java17/type-annotations` |
| 17 | nestmates | `java17/nestmates` |
| 17 | generics / signatures | `java17/generics` |
| 17 | lambda / method references | `java17/lambdas` |
| 17 | indified string concatenation | `java17/string-concat` |
| 17 | enum switch | `java17/enum-switch` |
| 17 | try-with-resources | `java17/twr` |
| 17 | class literals | `java17/class-literals` |
| 17 | runtime annotations | `java17/annotations` |
| 17 | records: generic / varargs / empty | `java17/records-advanced` |
| 17 | lambda capture | `java17/lambda-capture` |
| 17 | extreme: literal / unicode / numeric limits | `java17/edge-literals` |
| 17 | extreme: recursive generics / bridges | `java17/edge-generics` |
| 17 | extreme: repeatable + TYPE_USE annotations | `java17/edge-annotations` |
| 17 | extreme: generic / varargs records | `java17/edge-records` |
| 17 | extreme: nested sealed hierarchy | `java17/edge-sealed` |
| 17 | extreme: method / constructor references | `java17/edge-lambdas` |
| 17 | extreme: string / enum / nested switch | `java17/edge-switches` |
| 17 | extreme: try-with-resources / finally order | `java17/edge-twr` |
| 17 | extreme: inner / local / anonymous classes | `java17/edge-inner` |
| 17 | extreme: initialization order | `java17/edge-init` |
| 17 | extreme: text block escapes | `java17/edge-textblocks` |

Preview features (17 pattern switch, 15/16 preview syntax) are intentionally
not included by default; `-EnablePreview` is reserved for those units.

## Known gaps

None: all 34 units currently compile to byte-identical classes, with identical
diagnostics and exit codes.
