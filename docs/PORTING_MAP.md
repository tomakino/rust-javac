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
