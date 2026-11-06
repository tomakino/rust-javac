# IDE completion rules

Single source of truth for ravac-lsp completion matching and ranking.
Each rule has an evidence note; untestable rules are marked NOT IMPLEMENTED.

## 1. Search (matching)

The matcher is a direct Rust port of IntelliJ IDEA's name matcher
(`matcher.rs`, from `MinusculeMatcherImpl.kt` and
`CamelHumpMatcher.java`), not a custom five-level heuristic.

- Completion builds CamelHumpMatcher's middle-matching prefix:
  `"*" + prefix.replace('.', ". ").trim()` (IDEA
  `applyMiddleMatching`, `ide.completion.middle.matching`).
- Matching produces IDEA `MatchedFragment` ranges and uses
  `calculateHumpedMatchingScore` for ranking. Word starts, acronyms,
  consecutive uppercase letters, digits, wildcards, spaces, dots and
  separators follow the Java/Kotlin implementation.
- Middle matches must be at least 3 characters
  (`isMiddleMatch`/`matchInsideFragment`); a fragment that reaches the
  end of the pattern is still allowed.
- Query parsing splits the prefix at '.' when the receiver is a package or a
  qualified class ("java.util.Ba").
- Ranking uses IDEA's start-match preference before matching degree:
  start matches first, then `matching_degree` descending. This mirrors
  `PreferStartMatching`; it is not a match-level enum.

### new expressions

Class candidates come from a short-name catalog built once per analysis round
and reused by later completion requests, mirroring IDEA's short-name index
instead of rescanning every package per request. Candidates are filtered by
prefix, visibility, scope, and internal-package rules before rendering.

Matching classes are expanded to public constructors
("new Zip" / "new zie" / "new zipf" / "new Filein" / "new Fileinp").
Package-private classes and JDK-internal packages are filtered out.
Constructor items insert `Class($0)` as a snippet; the label carries the real
parameter names (`StringBuffer(int capacity)`), which are display-only and are
never inserted into the source.

When the nested `new` expression has an array expected type, constructor
expansion is disabled (IDEA `isArrayTypeExpected`): the list contains class
items instead, and the expected array type (for example `byte[]`) is inserted
first. Constructor expansion is controlled by
`AnalysisSession::set_completion_show_constructors` and is **off by default**
(IDEA's normal type lookup). In plain type mode a class item inserts
`Class($0)`; in array mode it inserts only the class name. Primitive array
types (`byte[]`, `int[]`, ...) are offered for a matching `new` prefix even
without an expected array type. Expected types are read from:

- the enclosing constructor or method parameter at the current argument index;
- a cast immediately before the `new` expression;
- an explicit local-variable initializer or assignment LHS;
- the enclosing method return type.

For non-array expected types, the expected class item is inserted before other
candidates; the attributed method's parameter type is used first, while other
overloads still contribute array expectations. Varargs arguments use the component type, so an element argument
does not spuriously count as an array expectation.

Method overloads and constructor overloads all contribute expected parameter
types at the current argument index, instead of using only the attributed
callee. Explicit constructor/method type arguments are substituted through the
declared type variables before the array check. For generic methods and
constructors without explicit type arguments, type variables are inferred from
the types of the preceding actual arguments; diamond constructors use the same
inference before the expected-array check.

## 2. Ranking

One sort key, evaluated exactly once (rank.rs):

    (start match, IDEA matching degree descending, exact, context kind,
     name, arity, signature length, signature, label)

name is the part before '(' so overloads group; signature length is
short-first (IDEA PreferShorter): FileInputStream(File file) before
FileInputStream(FileDescriptor fdObj).

### Context kind weight

| Context | order |
|---|---|
| New | classes/interfaces/enums/records only |
| Receiver | fields, methods, subpackages, types (IDEA keeps fields first) |
| Statement | locals/parameters, fields, methods, types, packages |
| Import | packages, types |
| Package | subpackages, types |
| ClassBody | types, methods, fields |

## 3. NOT IMPLEMENTED (cannot be reproduced without a compile index)

- PreferMostUsedWeigher (compile-time reference counts). Evidence:
  IDEA lists System.err last regardless of name order.
- Proximity/stats part of PreferByKindWeigher.
- Live templates and intention actions.

Do not emulate these with name heuristics.

### Auto-import

A completion item for an unimported top-level public type carries an LSP
`additionalTextEdits` entry that inserts `import <qualified name>;` after the
package/import block (imports.rs). No edit is added when:

- the type is in the same package or `java.lang`;
- the exact type or its package (wildcard) is already imported;
- another imported type or a type declared in this file has the same simple
  name (the client would create an ambiguity);
- the class is not public;
- completion is on an `import` line, or the candidate came from a
  package-qualified completion (`java.util.`) that already spells the name.

Nested types are not auto-imported yet.

### Completion documentation

`CompletionItem.documentation` is Markdown, shown in the completion right
pane. Project declarations use the parser's doc-comment table (the same text as
hover). Class files carry no Javadoc, so JDK elements are read lazily from
`JAVA_HOME/lib/src.zip` (`RAVAC_JDK_SRC` overrides the path) and cached per
class. The source scan associates each `/** ... */` block with the following
declaration: methods match by name and parameter count, fields and nested types
by name. Elements without source evidence get no documentation.

## 4. Known defects

None. Two former defects are fixed and covered by tests:

- Module coverage: the unnamed-module completer now enters every JDK module,
  so java.desktop/jdk.compiler types take part in "new Fil" (t66).
- Package completion no longer reads or lists nested class files entered under
  their flat binary name, so a later "java.util.Base64" expansion keeps
  Decoder/Encoder (t65).

## 5. Regression matrix

Fixed prefixes, run after any completion change
(target/tmp/regression_matrix.py):

    java.util.Base64, java.util.Base64., java.util.Base64.get,
    new Zip, new zie, new zipf, new Filein, new Fileinp,
    System., System.., java., java.util.

Unit suites: sort/search/rank (matcher + ranking), completion_spec
(108 end-to-end cases, including t65 package->qualified regression, t66
all-module "new Fil" coverage, t67/t68 documentation, t69-t74 auto-import,
and t75 constructor placeholder insertion).
