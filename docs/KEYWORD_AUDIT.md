# keyword audit (workspace) — accurate

Scan: all workspace .rs, target/.git/reference excluded.

## counts (actual)
- unimplemented!: 0
- unreachable!: 0
- todo!(): 0
- panic!: 1831 total (production 1541, test 290)
- TODO/FIXME/HACK comments: TODO 277, FIXME 2, HACK 4
- #[allow(dead_code)]: 67
- `let _ =`: 32
- real unsafe: 2 (Windows GetACP, approved)
- not-yet-ported / NOT PORTED comment markers: 791

## high-risk panic sites whose message says unimplemented/TODO (18)
- types.rs:873 unimplemented: null enclosing type in isSameType
- types.rs:1222 unimplemented: UnionClassType.isCompound (needs getLub)
- types.rs:1441 unimplemented: subst on compound types (needs makeIntersectionType)
- types.rs:3760 unimplemented: getReturnType (TODO Type batch)
- types.rs:3981 unimplemented: elemtype of a non-array type
- abstract_diagnostic_formatter.rs:163/294/368/381/412 (formatSource/localize/JCExpression/Formattable/String.valueOf)
- log.rs:1030 unsupported log message argument type (String.valueOf)
- raw_diagnostic_formatter.rs:101/106/217 (source name/class file source name/JCExpression positions)
- message_format.rs:86/89/112 malformed placeholder (Java MessageFormat exception semantics)
- doc_tree_factory.rs:352 UnsupportedOperationException (upstream default, legal)

## also unimplemented but only marked by TODO comments
- flags.rs: isStatic/isEnum/isConstant not ported (TODO(P3)) — symbol.rs has is_static/is_enum, is_constant missing
- Symbol.complete()/getTypeArguments completion TODO (registered comments)
- ClassFinder module completer TODO (Modules)
- etc: 277 TODO comments to audit individually