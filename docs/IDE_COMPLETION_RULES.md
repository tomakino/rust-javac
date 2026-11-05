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

## 4. Known defects

- The resident compiler only enters java.base. Types in java.desktop
  (javax.imageio, javax.swing), java.awt, jdk.compiler and other modules
  are not in the candidate pool (e.g. FileImageInputStream, FileSystemView).
- After a package completion for "java.util.", a later "java.util.Base64"
  expansion can drop nested types (Decoder/Encoder) because the package
  completer pre-enters them under their flat name (Base64$Decoder).
  Independent sessions are unaffected.

## 5. Regression matrix

Fixed prefixes, run after any completion change
(target/tmp/regression_matrix.py):

    java.util.Base64, java.util.Base64., java.util.Base64.get,
    new Zip, new zie, new zipf, new Filein, new Fileinp,
    System., System.., java., java.util.

Unit suites: sort/search/rank (matcher + ranking), completion_spec
(97 end-to-end cases).
