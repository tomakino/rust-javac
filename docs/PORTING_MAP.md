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
