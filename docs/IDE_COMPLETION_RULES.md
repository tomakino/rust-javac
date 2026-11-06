# IDE completion rules

Single source of truth for ravac-lsp completion matching and ranking.
Each rule has an evidence note; untestable rules are marked NOT IMPLEMENTED.

## 1. Search (matching)

Five levels, best first (search.rs):

1. ExactStart - case-exact prefix ("File" -> FileInputStream).
2. Start - case-insensitive prefix ("file" -> FileInputStream).
3. Camel - camel-hump subsequence; a character is accepted on a word
   boundary (start, after '_'/'$'/'.', or uppercase) or when it extends the
   previous contiguous run ("zie" -> ZipEntry, "gSM" -> getSecurityManager).
4. Contiguous - substring anywhere ("co" -> arraycopy).
5. Fuzzy - loose subsequence; only used as a fallback, never in the
   standard path.

Query parsing splits the prefix at '.' when the receiver is a package or a
qualified class ("java.util.Ba").

### new expressions

Every match quality expands constructors of matching classes:
"new Zip" / "new zie" / "new zipf" / "new Filein" / "new Fileinp".
Package-private classes and JDK-internal packages are filtered out.
Constructor items insert `Class($0)` as a snippet; the label carries the real
parameter names (`StringBuffer(int capacity)`), which are display-only and are
never inserted into the source.

## 2. Ranking

One sort key, evaluated exactly once (rank.rs):

    (match quality, exact, context kind, name, arity, signature length,
     signature, label)

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
