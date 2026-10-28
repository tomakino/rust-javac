# Porting map (upstream locations for local symbols)

This file is the single place where upstream `File.java:line` references are recorded.
Source code must not contain such references in new/rewritten code (PORTING_RULES 26.2).
Entries are added per batch, for newly written or rewritten symbols only.

| Local symbol | Upstream location | Notes |
|---|---|---|
| `jvm/pool_constant.rs::LoadableConstant` | `PoolConstant.java:33-127` | trait adapter; Dynamic/NameAndType pending |
| `code/symbol.rs::OperatorSymbol::AccessCode` | `Symbol.java:2462-2513` | enum adaptation |
| `code/types.rs::SignatureGenerator` | `Types.java:5044-5240` | abstract class → trait |

Status: template only; entries are appended by the executing agent and reviewed by the Lead.

## Batch 3a entries (new code only)

| Local symbol | Upstream location | Notes |
|---|---|---|
| `jvm/pool_constant.rs::PoolConstant/LoadableConstant/BasicConstant` | `PoolConstant.java:33-127` | trait adapter; Dynamic/BsmKey/NameAndType pending |
| `code/symbol.rs::OperatorSymbol::AccessCode` + `Symbol::get_access_code` | `Symbol.java:2442-2513` | enum adaptation, cached access code |
| `jvm/class_file.rs::{internalize_name, externalize_name}` | `ClassFile.java:164-166 / 190-192` | Name overloads |
| `code/types.rs::InvalidSignatureException/type_/SignatureGenerator` | `Types.java:5044-5240` | abstract class → trait; exception → Result |

## A0b entries: LambdaToMethod (new code)

| Local symbol | Upstream location | Notes |
|---|---|---|
| `comp/lambda_to_method.rs::LambdaToMethod` (all inner classes) | `LambdaToMethod.java` (whole file) | 1:1 port; TreeTranslator base embedded (registered adapter) |
| `comp/lambda_to_method.rs::contains_lambda` | `JavaCompiler.desugar` ScanNested/hasLambdas | recursive LAMBDA/REFERENCE scan adapter |
| `jvm/pool_constant.rs::LoadableConstantRef::MethodType` | `PoolConstant.java` (Type implements LoadableConstant) | CONSTANT_MethodType |
| `comp/trans_patterns.rs` (whole file) | `TransPatterns.java` | 1:1 port (A0.2) |
| `comp/tree_hasher.rs`, `comp/tree_differ.rs` | `TreeHasher.java`, `TreeDiffer.java` | 1:1 ports |

## D4 entries: Resolve verbose resolution + fatal diagnostics (new code)

| Local symbol | Upstream location | Notes |
|---|---|---|
| `comp/resolve.rs::VerboseResolutionMode` | `Resolve.java:180-213` | enum + debug.verboseResolution parsing |
| `comp/resolve.rs::Resolve::report_verbose_resolution_diagnostic` | `Resolve.java:215-262` | verbose note with candidate subdiagnostics |
| `comp/resolve.rs::Resolve::get_verbose_applicable_candidate_diag` | `Resolve.java:264-275` | applicable candidate fragment |
| `comp/resolve.rs::Resolve::get_verbose_inapplicable_candidate_diag` | `Resolve.java:277-279` | inapplicable candidate fragment |
| `comp/resolve.rs::Resolve::dump_method_reference_search_results` | `Resolve.java:3133-3167` | debug.dumpMethodReferenceSearchResults |
| `comp/resolve.rs::Resolve::inapplicable_candidate_detail` | `Resolve.java:4280-4295` | InapplicableMethod fragments (Port adapter: declared-type display) |
| `code/symbol.rs::InapplicableSymbolCandidate` | `Resolve.java:4938-5006` (Candidate) | step/mtype/details snapshot for diagnostics |
| `comp/resolve.rs::resolve_map_candidates` / `resolve_filter_candidates` | `Resolve.java:4239-4278` | MostSpecificMap/filterCandidates adapters (code-suffix arity match) |
| `comp/argument_attr.rs` Log parameter (`Rc<RefCell<Log>>`) | `ArgumentAttr.java:232-242` | Port adapter: no long-lived Log borrow across attribution |
| `util/fatal_error.rs::FatalError::from_diagnostic` / `fatal_error_diagnostic` | `FatalError.java:42-55` | diagnostic message construction |
| `comp/resolve.rs::Resolve::resolve_internal_field` | `Resolve.java:1471-1477` | fatal error when the field cannot be located |
| `code/types.rs::Types::get_return_type_public` ERROR case | `Type.java:2393` | ErrorType.getReturnType returns the type itself |

## D4 type display (diagnostic form)

| Local symbol | Upstream location | Notes |
|---|---|---|
| `code/printer.rs::visit_type` | `Printer.visit(Type)` (`code/Printer.java`) | qualified class names, enclosing types, type args, array/wildcard/captured/method-type rules |
| `code/printer.rs::visit_symbol` | `Printer.visitMethodSymbol` (`code/Printer.java`) | constructors/initializers use the owner name; `<type-vars>name(args)`; varargs `...` |

## D5 ClassReader diagnostic channels

| Local symbol | Upstream location | Notes |
|---|---|---|
| `jvm/class_reader.rs::find_access_method` warning | `ClassReader.java:1938-1959` | annotation.method.not.found (.reason registered) |
| `jvm/class_reader.rs::find_enum_constant` warning | `ClassReader.java:2015-2044` | unknown.enum.constant (.reason registered) |
| `jvm/class_reader.rs::add_type_annotations_to_symbol` catch | `ClassReader.java:2212-2223` | cant.attach.type.annotations |
| `jvm/class_reader.rs::DeproxyContext` | `ClassReader.AnnotationDeproxy` | explicit log/lintClassfile/currentClassFile (Port adapter) |
| `util/options.rs::Options::lint_enabled` | `Lint.isEnabled(LintCategory)` | Port adapter (Lint service not ported): parses -Xlint/-Xlint:all/-Xlint:none/comma lists |

## D3b inner-class Attr injection

| Local symbol | Upstream location | Notes |
|---|---|---|
| `comp/type_enter.rs::TypeEnter::complete_with_attr` | `TypeEnter.complete` | explicit &mut Attr (upstream TypeEnter.attr field) |
| `comp/attr.rs::Attr::complete_local_class` | `ClassSymbol.complete` from Attr | passes the Attr reference into TypeEnter (Port adapter for the RefCell model) |
| `comp/type_enter.rs::TypeEnter::finish_imports` | `TypeEnter.finishImports` | explicit &mut Attr (upstream TypeEnter.attr field) |
| `comp/check.rs::diagnostics_tests::complete_wrapper_propagates_completion_failure` | — | completion-failure propagation/recovery test (D3b second review) |

## D3f TypeEnter reentrancy (A + A2)

| Local symbol | Upstream location | Notes |
|---|---|---|
| `code/symbol.rs::Completer::complete_with_attr` | `Completer.complete` call sites inside Attr | default falls back to complete; TypeEnterCompleter overrides with the enclosing Attr |
| `compiler_context.rs::CompilerContext::complete_symbol_with_attr` | `Symbol.complete` | optional Attr context for the reentrant completion chain |
| `compiler_context.rs::CompilerContext::complete_class_symbol_with_attr` | `ClassSymbol.complete` | same failure recovery as complete_class_symbol |

## A3a Types/Resolve Attr threading

| Local symbol | Upstream location | Notes |
|---|---|---|
| `code/types.rs::Types::supertype` (+`class_bound`/`subst_type`/`subst_visit`) | `Types.supertype` | optional `&mut Attr` for the reentrant completion point; None keeps the Attr-less entry points |
| `comp/resolve.rs::Resolve::find_member_type` / `find_inherited_member_type` / `find_type` / `find_ident` / `find_ident_internal` / `resolve_ident` / `find_member_type_pub` | `Resolve.find*` | optional `&mut Attr`, `Some(self)` from Attr-inside callers |

## A3g import-scope Attr threading

| Local symbol | Upstream location | Notes |
|---|---|---|
| `code/scope.rs::Scope::get_symbols_full` / `get_symbols_by_name_full` / `includes_full` | `Scope.getSymbols` / `Scope.getSymbolsByName` / `Scope.includes` | optional `&mut Attr` for the FilterImportScope completion; `None` keeps the Attr-less behaviour |
| `code/scope.rs::symbol_importer_import_from` / `query_filter_import_scope_members` | `FilterImportScope.SymbolImporter.importFrom` / `FilterImportScope.getSymbols*` | forwards the Attr into the supertype completion |
| `compiler_context.rs::CompilerContext::scope_get_symbols_ctx` / `scope_get_symbols_by_name_full` / `scope_get_symbols_by_name_ctx` / `scope_finalize` | `Scope.getSymbols*` / `ImportScope.finalizeScope` | optional Attr parameter, threaded from `finish_imports` |
| `comp/check.rs::Check::check_imports_unique` | `Check.checkImportsUnique` | optional Attr; the static-import FilterImportScope query completes the imported class |
| `comp/type_enter.rs::TypeEnter::check_imports_unique` | `TypeEnter.finishImports` completion path | forwards `Some(attr)`; the normal entry keeps the Attr from `finish_imports` |
| `comp/check.rs::diagnostics_tests::cross_package_static_import_scope_completes_with_attr` | — | regression: a cross-package static import triggers the scope query and the Attr-aware class completion |

## A3h absent enclosing type (noType)

| Local symbol | Upstream location | Notes |
|---|---|---|
| `code/types.rs::Types::is_same_type_enclosing` | `Types.isSameTypeVisitor.visitClassType` | `None` is `Type.noType`; two absent enclosing types are equal, an absent and a present one differ; no panic |
| `code/types.rs::Types::hash_class_type` | `Types.HashCodeVisitor.visitClassType` | hashes the no-type value for a top-level class instead of panicking |
| `code/types.rs::get_enclosing_type` / `enclosing_of` | `Type.getEnclosingType` | documentation clarification: `None` is the local `Type.noType`, not null |
| `code/types.rs::tests::is_same_type_handles_absent_enclosing_type_as_no_type` | — | regression: top-level class equality, noType/class inequality, hash path |

## A3i annotation-type metadata single completion

| Local symbol | Upstream location | Notes |
|---|---|---|
| `code/symtab.rs::Symtab::enter_class` | `Symtab.enterClass` | installs the initial completer only for a newly created symbol; an existing symbol keeps its completer, so a completed class is not re-read by a later package scan |
| `code/symtab.rs::tests::enter_class_keeps_the_completer_of_an_existing_symbol` | — | regression: re-entering an existing symbol does not re-arm its completer |
| `comp/check.rs::diagnostics_tests::annotation_type_target_metadata_compiles` | — | classfile annotation `@Deprecated` exercises the `@Target` metadata path (`AnnotationTypeMetadata::getTarget`) |

## A3j platform/annotation/module triage

| Local symbol | Upstream location | Notes |
|---|---|---|
| `code/symtab.rs` boxed-class pre-entry + `code/types.rs::Types::boxed_class` | `Types.boxedClass` / `Symtab.initType` | boxed names are entered with the short name and the `java.lang` package owner (flat entry), not the full name with `no_symbol`; `BOT` has no boxed class |
| `code/symtab.rs::Symtab` enum symbol | `Symtab.enterClass(java_base, java_lang_Enum)` | package-owned `Enum` symbol |
| `comp/modules.rs::Modules::get_unnamed_module_completer` | `Modules.getUnnamedModuleCompleter` | calls `ModuleFinder.findAllModules` before installing the unnamed-module completer, making `java.se`/`java.sql` observable |
| `comp/annotate.rs` elided annotation value | `Annotate.attributeAnnotation` | writes the synthesized `value=` Assign back into the annotation tree so `Check.validateAnnotation` sees it |
| tests `platform_boxed_types_resolve` / `enum_type_parameters_resolve` / `sql_package_is_visible` / `elided_annotation_value_is_not_a_missing_default` | — | regressions for platform boxed types, Enum type parameters, java.sql visibility, elided values |

## A3k entries: lazy type completion and shared-service reentrancy

| Local symbol | Upstream location | Notes |
|---|---|---|
| `code/types.rs::Types::get_type_arguments_completed` | ClassType.getTypeArguments (Type.java:1082-1089) | lazy class-symbol completion; optional explicit `&mut Attr` |
| `code/types.rs::allparams_of_completed` | Type.allparams (Type.java:1060-1070) | lazy completion adapter over `getTypeArguments` |
| `code/types.rs::Types::interfaces` | InterfaceVisitor.visitClassType (Types.java:1195-1215) | declared interfaces substituted with the actual type arguments of the instantiated type |
| `code/types.rs::Types::capture` symbol completion | Types.java:4430-4500, Type.java:1082 | completes `tsym` before reading the formal list; Port adapter for the lazy `getTypeArguments` contract |
| `code/types.rs::Types::subst_visit` METHOD branch | Types.subst (Types.java:1900-1960) | `PartiallyInferredMethodType` handled as a MethodType |
| `comp/annotate.rs::AnnotationTypeCompleter::complete` | Annotate.AnnotationTypeCompleter | explicit `&mut Attr` (upstream reads the Attr service) |
| `comp/argument_attr.rs::ArgumentAttr` session | ArgumentAttr.java:246-260 | Port adapter: cloned per attribution session; `Rc<RefCell<HashMap>>` cache |
| `comp/deferred_attr.rs::DeferredAttrService` speculative methods | DeferredAttr.java:900-1000 | `&self` methods (single shared service; no re-borrow) |
| `comp/flow.rs::AliveAnalyzer/FlowAnalyzer/AssignAnalyzer` | Flow.java analyzers | explicit `&Symtab` parameter (Port adapter for the service model) |
| `code/scope.rs::Scope::owner` / `Scope::is_shared` | Scope.java (owner/shared) | accessors used by the Enter/MemberEnter scope duplication |
| `comp/member_enter.rs::visit_var_def` | MemberEnter.visitVarDef | unshared scope duplicate before publishing |
| `comp/enter.rs::visit_class_def` | Enter.visitClassDef | unshared scope duplicate before `scope.enter` |
| `tree/tree_maker.rs::TreeMaker::type_from_type` | TreeMaker.Type(t).setType(t) | every branch stores `tree.ty`; TYPEVAR sets `tree.sym` |
| `comp/check.rs::firstIncompatibleTypeArg` | Check.firstIncompatibleTypeArg (Check.java:2582-2606) | uses `Types.withTypeVar` instead of mutating the declared bound |
| `comp/resolve.rs::select_best` inference failure | Resolve.selectBest `catch (InapplicableMethodException)` | an inference failure is recorded as an inapplicable candidate with its nullable diagnostic (no synthesized diagnostic) |
| `comp/resolve.rs::InapplicableMethodException` | Resolve.InapplicableMethodException | the diagnostic is `Option` because the upstream null diagnostic is legal |
| `comp/operators.rs::resolve_unary` | Operators.resolveUnary | helpers are looked up by operator name (`opname`), so PREINC shares the POSTINC helper |
| `comp/attr.rs::make_null_check` | Attr.makeNullCheck | resolves the NULLCHK operator and sets the operator slot and `tree.type` |
| `comp/attr.rs::visit_literal` null literal | Attr.visitLiteral | `JCLiteral(BOT, null)` returns the literal type without a constant value |
| `comp/member_enter.rs::method_env` | MemberEnter.methodEnv | `next` points at the enclosing env and `outer` inherits the enclosing outer |
| `comp/member_enter.rs::visit_var_def` / `comp/enter.rs::visit_class_def` | MemberEnter.visitVarDef / Enter.visitClassDef | no shared-scope reconstruction; members are entered into the scope selected by `enterScope` |
| `code/scope.rs::Scope::enter` | ScopeImpl.enter | unconditional shared-table assertion, matching the upstream assertion helper |
| `comp/attr.rs::copy_scope` | Attr.copyScope | collects symbols with the recursive lookup kind so outward scope symbols are copied into the fresh scope |
| `comp/attr.rs::visit_block` / `visit_for_loop` / `visit_foreach_loop` / `visit_try` | Attr scope leaves | scope.leave() is called at the same finally/caller positions as the upstream leaves |
| `comp/attr.rs::binding_env` / `binding_env_with_tree` | Attr.bindingEnv / switch case environment | the environment scope is duplicated with the sharing dup and released by the caller |
| `comp/attr.rs::handle_switch` | Attr handleSwitch | switch and case environments use the sharing dup and are released after attribution |
| `code/scope.rs::Scope::is_shared` | — | removed: the shared-scope detection adapter is no longer used |

## A3l entries: open-addressing Scope verification, imports, annotation Attr threading

| Local symbol | Upstream location | Notes |
|---|---|---|
| `code/scope.rs` Scope table (TableSlot/get_index/enter/dble/leave/remove/lookup) | `Scope.ScopeImpl` (Scope.java:274-448, 542-565) | kept as the upstream open-addressing table (probe_index, sentinel/deleted slots); the chaining experiment was reverted by Lead decision; only `Scope::owner` and the header notes remain |
| `compiler_context.rs::scope_leave` | `ScopeImpl.leave` (Scope.java:385-401) | upstream probing leave; the chaining variant was reverted |
| `compiler_context.rs::scope_finalize` | `ImportScope.finalizeScope` / `NamedImportScope.finalizeScope` (Scope.java:749-781, 830-837) | NamedImportScope finalizes subScopes and name2Scopes separately, matching upstream |
| `comp/check.rs::validate_annotations` / `get_target_names` / `get_applicable_targets` | `Check.validateAnnotations` / `getTargetNames` / `getApplicableTargets` | Port adapter: optional `&mut Attr` forwarded to annotation metadata completion |
| `comp/annotate.rs::SourceCompleter::complete` | `Annotate.SourceCompleter.complete` | Port adapter: forwards the caller Attr into the annotation-type visitor |
| `comp/lower.rs::Lower::access_base` | `Lower.accessBase` (Lower.java:1030-1034) | static symbols qualify with the owner symbol (`access(owner, QualIdent(owner))`), not the member |
| `comp/attr.rs::handle_switch` addVars order | `Attr.handleSwitch` (Attr.java:1804-1814) | case scope is left before the case variables enter the switch scope |
| `comp/resolve.rs::resolve_self` | `Resolve.resolveSelf` (Resolve.java:3724-3775) | the environment's enclosing class symbol is read from the class declaration (tree.sym), not `TreeInfo.symbol` (which has no CLASSDEF case); qualified this (`Outer.this`) now resolves |
| tests `three_hundred_symbols_survive_multiple_doublings` / `names_with_the_same_initial_low_bits_are_all_found` / e2e import, override, qualified-this | — | open-addressing and import-resolution regressions |

## A3k entries: interface/generic override resolution, erasure and diamond access

| Local symbol | Upstream location | Notes |
|---|---|---|
| `code/type_.rs::get_type_arguments` / `code/types.rs::get_type_arguments` | `ForAll.getTypeArguments` (Type.java:ForAll) | ForAll returns its tvars; `resultSubtype` needs that list to rename the method type variables before comparing returns |
| `code/types.rs::subst_visit` class-type branch | `Types.Subst.visitClassType` (Types.java:3353-3365) | the rebuilt class type keeps the encoding of the source enclosing type (None vs Some(noType)); both stand for the Type.noType sentinel |
| `code/symtab.rs::new_class_type` | `new ClassType(Type.noType, null, null)` (Symtab/ClassSymbol ctor) | initial class symbol type carries the noType enclosing type (Some(noType)) |
| `jvm/class_reader.rs::class_sig_to_type` | `ClassReader.classSigToType` (ClassReader.java) | a top-level parameterized class gets the explicit noType enclosing type, matching the erasure(sym.type) shape of the plain path |
| `code/types.rs::erasure_visit` MethodType branch | `StructuralTypeMapping.visitMethodType` (Types.java erasure visitor) | erase parameter/return/thrown types; raw constructor members then expose erasures instead of the class type variables |
| `comp/resolve.rs::not_overridden_in` | `Resolve.notOverriddenIn` (Resolve.java:467-474) | full upstream condition: s2 == null || s2 == sym || sym.owner == s2.owner || !isSubSignature(memberType(site, s2), memberType(site, sym)) |
| `comp/trans_types.rs::add_bridge` | `TransTypes.addBridge` (TransTypes.java) | Port adapter: the cloned bridge symbol restores its arena id before setAttributes writes metadata (Symbol::clone clears it) |
| `comp/attr.rs::visit_new_class` constructor resolution | `Attr.visitNewClass` (Attr.java:2844-2854) | resolveConstructor/checkId run in the duplicated env whose selectSuper = cdef != null; protected super constructors are then accessible from anonymous subclasses |
