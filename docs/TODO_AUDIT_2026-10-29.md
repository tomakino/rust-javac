# TODO/FIXME 审计台账

生成时间：2026-10-29

本次清理只删除源码中的整行 TODO/FIXME/XXX 注释，不改动任何代码。完整原始条目如下（行号是清理前的位置）。真实未实现项仍需按本台账逐项偿还，不得因为源码中不再出现 TODO 标记而视为完成。

- 清理条目：237
- 涉及文件：74

## crates/javac-core/src/code/class_finder.rs

- L11: // getSupplementaryFlags (jrtIndex==null path; Some path registered TODO),
- L113: /// Java: protected Location currentLoc; // FIXME
- L821: // TODO(S3/verbose): the upstream prints the source/class paths through

## crates/javac-core/src/code/lint.rs

- L61: // TODO(Attribute/Symtab): the AugmentVisitor needs Attribute.Compound and
- L76: /// TODO(Options): the constructor reads Options (-Xlint/-Xlint:all/none/

## crates/javac-core/src/code/preview.rs

- L25: //! * TODO(Log/Lint/Options): the constructor reads Options (enabled,
- L137: // TODO(Log/Lint/Options): the P6 wiring builds Preview from the ported
- L232: /// TODO(Log/Lint/Options): the constructor reads Options/Log/Lint; the

## crates/javac-core/src/code/source.rs

- L31: //! * TODO(Options/Option): `instance(Context)` uses the not yet ported

## crates/javac-core/src/code/symtab.rs

- L37: //!   values (registered; the localized toString is a Symbol TODO);
- L54: //!   Types methods are NOT PORTED (TODO Types; see types.rs).
- L56: //!   anonymous override is a Symbol TODO);
- L677: /// (PackageSymbol completer TODO), the members scope is created here.
- L1079: /// not ported yet (Options/Option integration TODO), so the caller passes
- L1104: // TODO(MissingInfoHandler): MissingInfoHandler.instance(context) is
- L1121: // anonymous override is not expressible (Symbol TODO).

## crates/javac-core/src/code/types.rs

- L87: //      predicates at the consuming sites; full narrowing is TODO Types-2.
- L109: //! remaining Types methods are NOT PORTED (TODO Types):
- L751: // FIXME, this might be leftovers from before capture conversion
- L2392: /// glb (TODO Types): a type that does not need capture conversion is
- L3325: /// TODO(Types): the strict=false HasSameArgs variant is not ported.
- L3670: /// subtype relation (registered lint wiring TODO with Attr); the boolean
- L3720: /// emitted here (the lint wiring is TODO with the Attr); the boolean
- L4200: /// ported, so the nil default is used (TODO Symbol; registered).
- L4286: /// PORTING_RULES: reproduced here because type_.rs has not ported it yet (TODO
- L4302: // helpers because type_.rs/symbol.rs have not ported them yet (TODO Type/Symbol
- L4728: /// TODO(Attr/Type.contains): not wired yet. Keep until that wires

## crates/javac-core/src/comp/analyzer.rs

- L645: // TODO(ArgumentAttr): thread the local cache context into the speculative

## crates/javac-core/src/comp/annotate.rs

- L1780: // TODO(TypeAnnotations): Check.isTypeAnnotation is not ported.

## crates/javac-core/src/comp/attr_context.rs

- L10: //!   fields are omitted and dup() skips them; registered TODO, no substitute;

## crates/javac-core/src/comp/attr.rs

- L659: // TODO(MatchBindingsComputer): matchBindings =
- L661: // TODO(DeferredAttr): if (tree == breakTree &&
- L1158: // TODO(Check): chk.checkRefType(tree.pos(), type)
- L1446: // TODO(Symbol/Types): sym.isMemberOf(symEnv.enclClass.sym,
- L1891: // registered with a not-yet-ported marker + TODO at the exact call site.
- L2876: // TODO(Check): chk.checkNonCyclic(tvar.pos(), (TypeVar)tvar.type)
- L9147: // TODO(Preview): the source-level preview check is registered with the
- L9217: // TODO(Preview): source-level CASE_NULL check registered with the
- L9263: // TODO(Resolve): the Resolve LogResolveHelper swap is registered
- L16814: /// path calls Attr.attribType (TODO(Attr), not ported yet), so the

## crates/javac-core/src/comp/check.rs

- L25: // is not ported the branch is marked TODO and not executed (no substitute).
- L1956: // types.getRetention/overrideEquivalent) are explicit TODO registrations.
- L3962: // TODO(Check): checkDeprecated is not ported yet (Check.java:3520-3560,

## crates/javac-core/src/comp/deferred_attr.rs

- L19: // TODO.

## crates/javac-core/src/comp/enter.rs

- L11: // calls are TODO in the corresponding methods (Annotate);
- L13: // ensureImportsChecked are TODO (TypeEnter/MemberEnter); visitClassDef
- L15: // - Todo instance: todo.append of package/module envs is TODO (Todo);
- L17: // usesProvidesCompleter and isInModuleGraph are TODO (Modules);
- L66: /// TODO(TreeMaker): TreeMaker.ClassDef/Modifiers factories are not
- L319: // TODO(Annotate): annotate.blockAnnotations();
- L336: // TODO(Annotate): annotate.unblockAnnotations();
- L763: // TODO(Annotate): c.clearAnnotationMetadata();

## crates/javac-core/src/comp/infer.rs

- L2812: // TODO(Lint): the deferred solve callback does not propagate the
- L3395: // TODO(Attr): the CompletionFailure channel of
- L3410: // TODO(Attr): the CompletionFailure channel of

## crates/javac-core/src/comp/member_enter.rs

- L15: // annotateDefaultValueLater/unfinishedDefaultValue calls are TODO (Annotate
- L24: // type refinement is TODO in visitVarDef. VarSymbol.setLazyConstValue is
- L26: // - getMethodEnv mEnv.info.lint.augment(sym) is TODO (Lint), the scope
- L32: // registered separately (the Enter leaves the TypeEnter completer TODO).
- L634: /// TODO(Attr): the upstream call sites are checkReceiver's
- L636: /// TODO until Attr is ported; the method is kept for that.
- L886: // TODO(Lint): mEnv.info.lint = mEnv.info.lint.augment(tree.sym);

## crates/javac-core/src/comp/resolve.rs

- L40: //! NOT YET PORTED IN THIS FILE (registered; each with a not-yet-ported marker + TODO):
- L1764: // upstream body; unimplemented dependencies are explicit TODO registrations.
- L2534: // TODO(ClassFinder): the BadClassFile distinction is not
- L4239: // TODO(Check diagnostics , ADR 0002 P4): the upstream reportMC builds
- L4327: // TODO(Lint/Check): the upstream returns the shared rsWarner; the
- L4700: // TODO(Attr): the CompletionFailure channel of
- L5393: // TODO(Scope/Resolve): upstream getSymbolsByName applies the
- L6606: // TODO(Check): FindConstructor also runs
- L10221: // Java: currModule.complete() — TODO(Completer): the
- L10298: // TODO(Types): t.isUnion() / UnionClassType alternative types.

## crates/javac-core/src/comp/type_enter.rs

- L27: // typeAnnotations.validate*, todo.append, preview checks -> TODO comments
- L275: // TODO(TypeEnter/Completer): the completer registration is an
- L416: // TODO(Lint): Lint prevLint = chk.setLint(lint);
- L508: // TODO(Lint): chk.setLint(prevLint);
- L644: // TODO(Check): chk.checkCanonical(imp);
- L1094: // TODO(Synthesizer): the local ErrorType modelType hook and the

## crates/javac-core/src/compiler_context.rs

- L431: // TODO for the Scope query).

## crates/javac-core/src/file/base_file_manager.rs

- L30: //!   TODO (the Log error entry point and a streaming decoder are not available).
- L32: //!   provide yet (TODO, reported).
- L38: //!   unimplemented + TODO; the content cache keeps the decoded JavaString directly
- L44: //! * TODO(main/OptionHelper): handleOption(String, Iterator) needs Option.process and
- L82: /// TODO(S3b): set by setContext once Locations.update is ported.
- L121: /// TODO(main option restore): set by the MULTIRELEASE branch of handleOption(Option,String).
- L165: /// TODO(S3b): the body needs Locations.update(log, warn, FSInfo.instance(context)) and
- L197: /// Not wired yet (TODO(file manager wiring)).
- L268: // TODO(P6): java.net.URL/URLClassLoader and the reflective custom loader lookup are not
- L347: /// was trimmed in task-48 (reported; TODO(main option restore)).
- L410: /// below is expressed (PathFileObject.openReader is a separate TODO).

## crates/javac-core/src/file/cache_fs_info.rs

- L17: //! * not ported (TODO(P2), reported): preRegister(Context) (Context factory for

## crates/javac-core/src/file/fs_info.rs

- L13: //! * not ported dependencies (TODO(P2), reported): java.util.jar.JarFile/
- L147: // TODO(P2): the upstream getJarFSProvider() (FileSystemProvider lookup) and

## crates/javac-core/src/file/javac_file_manager.rs

- L54: //!   (java.nio zip/jar FileSystem) is the second S3c (TODO in getContainer).
- L62: //!   the Locations-side PathFactory (Java locations.setPathFactory) is TODO(S3b follow-up).
- L67: //! * known gaps: getClassLoader (URLClassLoader/reflection, TODO(P6)); handleOption(String,
- L68: //!   Iterator) (OptionHelper not ported, TODO(OptionHelper)); ServiceLoader; close's
- L70: //!   (second).  Java warnings are not emitted (TODO(Log)).
- L1008: /// depend on the source level; TODO(P6) for the exact SourceVersion port).
- L1518: // Java: locations.setPathFactory(f); the Locations-side PathFactory is TODO(S3b
- L1592: /// (registered deviation, TODO(P6)).

## crates/javac-core/src/file/jrt_index.rs

- L27: //!   missing-key result) for every package until the resource lands (TODO resources,
- L96: /// TODO(S3c): get_file is used by JavacFileManager once that lands.
- L337: /// TODO(resources): the upstream reads the ResourceBundle

## crates/javac-core/src/file/locations.rs

- L79: //! * reported gaps (TODO, no substitute data):
- L80: //!   - jar Class-Path expansion (java.util.jar Manifest; FSInfo.getJarClassPath TODO(P2)):
- L82: //!   - Log.warning diagnostics for path problems (TODO(Log)): invalid/nonexistent
- L85: //!     probe (java.nio FileSystem); TODO(S3c);
- L94: //!     argument reporting is TODO(Log));
- L99: //! * TODO(S3c): JavacFileManager consumes this API.  Handler locations are looked up by
- L184: // Java: warning(PathElementNotFound) — TODO(Log): no Log warning entry point.
- L204: // FileSystem does not exist in the port: skip (TODO(S3c); the upstream
- L205: // warning is a TODO(Log) diagnostic).
- L211: // ported (FSInfo.getJarClassPath TODO(P2)), so the Class-Path attribute is not expanded.
- L259: // Java: the enclosing-jar case needs a java.nio jar file system (TODO(S3c)).
- L509: /// Java: private Log log; (not used by this; path warnings are TODO(Log)).
- L751: // not available without a JVM (the launcher provides them, TODO(P6)); the
- L945: // path warnings are registered as TODO(Log) in this file.

## crates/javac-core/src/file/mod.rs

- L11: //!   charset/URL/thread-dependent methods are TODO and reported) and the Locations
- L16: //! * TODO(P2): the deferred methods listed in the individual file headers.

## crates/javac-core/src/file/path_file_object.rs

- L26: //! * deferred (TODO(P2), reported):
- L339: // TODO(P2): the macOS Normalizer.isNormalized branch is not ported.
- L379: /// logic (BaseFileManager itself is not ported; TODO(P2): call the ported

## crates/javac-core/src/file/relative_path.rs

- L18: //! * java.nio.file.FileSystem (resolveAgainst(FileSystem)) -> TODO(P2); the resolveAgainst(Path) overload is ported on top of javac-fs;
- L19: //! * java.util.zip.ZipFile/ZipEntry (RelativeFile.getZipEntry) -> TODO(P2);
- L86: /// Java: public Path resolveAgainst(FileSystem fs) — TODO(P2): java.nio.file
- L256: /// Java: ZipEntry getZipEntry(ZipFile zip) — TODO(P2): java.util.zip.ZipFile/

## crates/javac-core/src/jvm/class_file.rs

- L148: // TODO(P0+): internalize(Name) / externalize(Name) overloads are added together with

## crates/javac-core/src/jvm/class_reader.rs

- L9: //   Code object attach TODO), ConstantValue, Deprecated, Exceptions,
- L65: //! * NOT ported yet (not-yet-ported marker with TODO below): readClass and the
- L68: //!   These methods carry explicit TODO markers; no substitutes are provided.
- L140: /// PORTING_RULES: Scope.WriteableScope is not ported yet (TODO(P3 scope
- L287: // TODO(Lint)); the -Xlint:classfile option is read directly.
- L713: /// constant here; Types.noType is not represented in Symtab yet (TODO P3),
- L2946: // mapping is a registered TODO for the hardening).
- L3341: /// TODO(P6/Code): jvm.Code is not ported, so the structure is read

## crates/javac-core/src/jvm/module_name_reader.rs

- L21: //! * NOT ported yet (TODO, no substitute semantics):

## crates/javac-core/src/jvm/pool_reader.rs

- L15: //! * NOT ported yet (TODO, no substitute semantics):
- L39: //! (getClass/getModule/getPackage/getType/getNameAndType are still TODO; the raw
- L95: /// PORTING_RULES: TODO(P2): ClassReader.badClassFile throws a ClassFileError
- L306: // TODO(P2): Symtab/currentModule are not ported; the port keeps the
- L320: // TODO(P2): Type/sigToType is not ported; the raw descriptor Name is
- L341: // TODO(P2): Symtab/currentModule are not ported; the port keeps the
- L354: // TODO(P2): Symtab is not ported; the port keeps the name.

## crates/javac-core/src/jvm/profile.rs

- L131: /// TODO(Options/Option): Options.instance/options.get and

## crates/javac-core/src/jvm/target.rs

- L17: //! * TODO(Options/Option): `instance(Context)` uses the not yet ported
- L268: /// TODO(Options/Option): Options.instance/options.get and

## crates/javac-core/src/main/option.rs

- L37: //!   TODO(main/OptionHelper + Arguments + Log): process/handleOption/help
- L614: /// TODO(main/OptionHelper): the upstream (String, Throwable) constructor is

## crates/javac-core/src/parser/doc_comment_parser.rs

- L20: //!   from Log.currentSource() (TODO(parser S7));
- L97: /// Not wired yet (TODO(doc comment S8 wiring)).
- L936: // TODO(tree): DocTreeMaker.newErroneousTree(text, diagSource, code)
- L1457: // (registered TODO(tree): null description fields).

## crates/javac-core/src/parser/javac_parser.rs

- L34: //!   TODO(parser S7) marker (doc-comment reference parsing only; registered).
- L541: /// TODO(parser wiring): wire the remaining term3Rest/annotations call sites.
- L553: /// TODO(parser wiring).
- L560: /// TODO(parser wiring).
- L770: /// TODO(parser wiring).
- L1206: /// TODO(parser wiring).

## crates/javac-core/src/parser/lazy_doc_comment_table.rs

- L16: //!   took from Log.currentSource() (TODO(parser S7): use ParserFactory directly);
- L68: /// ParserFactory is available (parser S7; TODO).

## crates/javac-core/src/parser/mod.rs

- L21: //!   Source.instance is TODO(Options)): the dependencies are injected
- L22: //!   (registered GAP/TODO(P6));

## crates/javac-core/src/parser/parser_factory.rs

- L13: //!   injection (its instance() is not implemented yet, TODO(Log/Lint/Options
- L39: /// Upstream ParserFactory.tokens (unwired yet: TODO(parser wiring)).

## crates/javac-core/src/parser/scanner_factory.rs

- L13: //!   implemented yet (TODO(Options)), so new()/instance() take Names and
- L45: /// implemented yet (TODO(Log/Lint/Options)), so the dependency is an
- L50: /// yet (TODO(Options)); None is the default (no -Xlint categories),

## crates/javac-core/src/parser/tokens.rs

- L27: //!   keyword table; the Context-based instance is a registered GAP (TODO(P6)).

## crates/javac-core/src/platform/jdk_platform_provider.rs

- L53: //! ## Not ported yet (TODO, with the dependency that unblocks each)
- L305: /// TODO(S3): read by getFileManager() (the ct.sym section lookup), which is not
- L313: // TODO(S3): the map is only filled by getFileManager(); no FileSystem handle is kept in
- L432: /// TODO(S3)), so the upstream loop over an empty map is the Ok(()) case here.  When S3

## crates/javac-core/src/tree/dc_tree.rs

- L20: //! - TODO(P4/P6): upstream DCErroneous implements JCDiagnostic.DiagnosticPosition

## crates/javac-core/src/tree/doc_tree_maker.rs

- L19: //! - TODO(P6): upstream instance(Context) needs JavacTrees and
- L21: //!   factory methods are usable through [DocTreeMaker::new]. TODO(parser S8):
- L26: //! - TODO(P6): getSentenceBreak uses trees.getBreakIterator() upstream; until
- L62: /// args) method (TODO, see the file header); the accessor keeps the
- L68: // TODO(P6): trees: JavacTrees (used by getSentenceBreak)
- L69: // TODO(parser S8): reference_parser: ReferenceParser (used by newReferenceTree)
- L79: /// TODO(P6)/(parser S8), see the file header).
- L423: /// TODO(parser S8): needs parser.ReferenceParser (not ported yet); upstream
- L771: * TODO(P6): upstream consults trees.getBreakIterator(); JavacTrees and

## crates/javac-core/src/tree/pretty.rs

- L25: //!   internal TreeScanner). The cdef != null path panics with a TODO message;
- L28: //! TODO(parser): compare the full Pretty output against the oracle data
- L405: /// TreeScanner, not ported yet); it panics with a TODO message.

## crates/javac-core/src/tree/tree_maker.rs

- L91: // Java: Types types; Symtab syms; — not declared yet, see the file header (TODO).

## crates/javac-core/src/util/abstract_diagnostic_formatter.rs

- L42: //!   provided (TODO in the that ports Options); the report-style option
- L44: //! - NOT PORTED IN THIS STEP (dependencies are not ported yet; TODO, no
- L115: // ported yet, TODO).

## crates/javac-core/src/util/abstract_log.rs

- L88: // TODO(Log): pass the log itself as DiagnosticSourceLog once the

## crates/javac-core/src/util/basic_diagnostic_formatter.rs

- L36: //!   provided (TODO: the diags.* option decoding is registered in the Options

## crates/javac-core/src/util/convert.rs

- L18: //! TODO(P1): Unicode digits in Character.digit; the parser only feeds ASCII
- L130: /// Java: Character.digit(char, radix); ASCII-only for now (see TODO above).

## crates/javac-core/src/util/fatal_error.rs

- L41: // TODO(P0+): Java constructor FatalError(JCDiagnostic, Throwable) is added

## crates/javac-core/src/util/javac_messages.rs

- L19: //!   locale is "en" (environment detection is a registered TODO);
- L34: //! TODO(D5): the JCDiagnostic-dependent members of JavacMessages
- L105: /// Java: Locale.getDefault() -- TODO: environment locale detection is a
- L207: // TODO(D5): the upstream Context constructor also initializes Options

## crates/javac-core/src/util/jc_diagnostic.rs

- L792: /// TODO(Options): the upstream protected Factory(Context) initializes

## crates/javac-core/src/util/list.rs

- L542: // TODO(P3): public static <T> List<T> convert(Class<T> klass, List<?> list)

## crates/javac-core/src/util/log.rs

- L31: //!     TODO for the remaining types; registered);
- L37: //!       is not added to JavacMessages by the ported Log (TODO Main);
- L386: // TODO(Main): messages.add(Main.javacBundleName); (main/Main is
- L421: // (equivalently, TODO Option).
- L1015: /// rendered (the remaining types are TODO, registered section 9).

## crates/javac-core/src/util/message_format.rs

- L20: // Not implemented (registered TODO): number/date/time/choice sub-formats

## crates/javac-core/src/util/module_helper.rs

- L8: //!   addExports(Module, Module) is therefore NOT PORTED (TODO, no substitute

## crates/javac-core/src/util/options.rs

- L37: //! * TODO(Arguments/CommandLine): the table is filled by Arguments/CommandLine
- L108: /// not ported yet (registered TODO(Lint)); the -Xlint options are parsed

## crates/javac-core/src/util/raw_diagnostic_formatter.rs

- L21: //!   is provided (TODO(Options));

## crates/javac-core/src/util/string_utils.rs

- L11: //! TODO(P0): verify case mapping against JDK 17 String.toLowerCase(Locale.US)

## crates/javac-core/tests/tree_oracle.rs

- L16: //! (TODO(S8 DocCommentParser batch)). Any other difference is a FAIL with the

## crates/javac-source/src/util/doc_trees.rs

- L9: //!   are deferred to P6 (TODO(P6)).
- L11: //!   the ported surface (java.text) — omitted, registered (TODO(P6/P2)).

## crates/javac-source/src/util/javac_task.rs

- L9: //!   BasicJavacTask/Context; it is deferred to P6 (TODO(P6)).

## crates/javac-source/src/util/tree_path.rs

- L14: //! static getPath searches rely on TreePathScanner (next batch) and are TODO(P1).
- L42: /// PORTING_RULES: TODO(P1) — the Java implementation searches with a

## crates/javac-source/src/util/trees.rs

- L10: //!   (TODO(P6), not expressible without the javax.annotation.processing port).

## crates/javac-strings/src/lib.rs

- L193: /// TODO(P2): match java.nio.charset.StandardCharsets.UTF_8 CharsetDecoder

## crates/javac-tools/src/error.rs

- L25: //! TODO(P6): replace this placeholder with the error type introduced by the

## crates/javac-tools/src/jdk.rs

- L31: //! TODO(P6): these placeholders are finalized by the API phase; annotation

## crates/javac-tools/src/lang/model/util/element_scanner9.rs

- L30: /// TODO(upstream): the Java source carries the same "Hmmm, this might not be
- L33: // TODO: Hmmm, this might not be right

## crates/javac-tools/src/lib.rs

- L58: //! TODO(P0): register the adapter files `error.rs`, `uri.rs`, `java_io.rs`,
- L60: //! TODO(P2): `java.net.URI` is a minimal mapping; the remaining
- L63: //! TODO(P6): annotation processing is out of scope (docs/PLAN.md §3.1);

## crates/javac-tools/src/tools/java_file_manager.rs

- L477: // TODO: describe failure modes
- L536: // TODO: describe failure modes
- L554: // TODO: describe failure modes
- L578: // TODO: describe failure modes

## crates/javac-tools/src/tools/standard_java_file_manager.rs

- L422: /// boundary handled by the file layer (TODO(P2): platform path encoding).
- L754: // platform-encoding boundary of the file layer (TODO(P2)); the

## crates/javac-tools/src/tools/tool_provider.rs

- L29: ///   compiler entry point (TODO(P6)).
- L76: /// TODO(P6): the Rust compiler entry point is created by the API phase

## crates/javac-tools/src/uri.rs

- L22: //! TODO(P2): this is not the complete `java.net.URI` port.
- L168: /// TODO(P2): `java.net.URI` validates per component (for example '[' and ']'
