# Porting Map

This file records the per-module mapping between rust-javac and OpenJDK
javac, together with the porting registry moved out of source headers.
Source headers are ordinary module documentation; all porting metadata
(upstream classes, status, original registries) is recorded here.

## Overview

| Rust file | Upstream | Status | Original header lines |
|---|---|---|---|
| comp/infer.rs | com/sun/tools/javac/comp/Infer.java | ported | 48 |
| comp/attr.rs | com/sun/tools/javac/comp/Attr.java | ported | 39 |
| comp/check.rs | com/sun/tools/javac/comp/Check.java | ported | 26 |
| comp/resolve.rs | com/sun/tools/javac/comp/Resolve.java | ported | 62 |
| comp/lower.rs | com/sun/tools/javac/comp/Lower.java | ported | 14 |
| comp/flow.rs | com/sun/tools/javac/comp/Flow.java | ported | 22 |
| jvm/codegen.rs | com/sun/tools/javac/jvm/Gen.java | ported | 4 |
| parser/javac_parser.rs | com/sun/tools/javac/parser/JavacParser.java | ported | 46 |
| code/types.rs | com/sun/tools/javac/code/Types.java | ported | 134 |
| main/mini_compiler.rs | com/sun/tools/javac/main/JavaCompiler.java | adapter | 13 |

Notes:
- Ten representative modules are registered here; their complete original
  headers are preserved in the details blocks below.
- Header migration is batched: infer.rs, mini_compiler.rs, and codegen.rs
  are in the first batch; the remaining seven follow in the second P0
  batch.
- New modules follow the template in docs/STYLE.md and append their
  registry here.

## compinfer

- Rust: crates/javac-core/src/comp/infer.rs
- Upstream: com/sun/tools/javac/comp/Infer.java
- Status: ported (registry below)
- Original registry: com/sun/tools/javac/comp/Infer.java @ 982d53c (jdk-17.0.19-ga) (header migrated in P0 first batch)

<details><summary>Original file header (migrated from source)</summary>

    #![allow(clippy::result_large_err, clippy::needless_pass_by_ref_mut)]
    // Origin: com/sun/tools/javac/comp/Infer.java @ 982d53c (jdk-17.0.19-ga)
    // com/sun/tools/javac/comp/InferenceContext.java @ 982d53c (jdk-17.0.19-ga)
    //
    // Registration mapping: the InferenceContext.java unit maps to the InferenceContext
    // type below (same upstream class, ported into one Rust module because it and
    // Infer are mutually dependent); Infer.java maps to the Infer/Incorporation*/
    // GraphSolver/InferenceStep types below.
    //
    // Ported in this file (Infer):
    // - InferenceContext: addVar/inferenceVars/undetVars/restvars/instvars/
    // boundedVars/filterVars/free/freeVarsIn/asUndetVar(s)/instTypes/asInstType(s)/
    // addFreeTypeListener/notifyChange/save/rollback/dupTo/min (ReachabilityVisitor)/
    // solve/solveAny/solveBasic/solveLegacy/cachedCapture (InferenceContext.java:
    // 76-606);
    // - Infer: allowGraphInference/fromTypeVarFun/incorporationEngine (graph+legacy)/
    // doIncorporation/doIncorporationOp/IncorporationBinaryOp/
    // getParameterizedSupers/asSuper/reportInstError/reportBoundError/
    // getBoundFragment/BoundFilter/instantiateAsUninferredVars (Infer.java:82-140,
    // 519-545, 616-630, 692-1290);
    // - the inference engine: GraphStrategy/LeafSolver/BestLeafSolver/InferenceStep/
    // LegacyInferenceSteps/GraphInferenceSteps/DependencyKind/GraphSolver/
    // InferenceGraph/Node (Infer.java:1299-1945); the Tarjan reduction is a
    // faithful re-implementation of GraphUtils.tarjan (GraphUtils.java:171-235)
    // over the graph's node indices, because the ported GraphUtils.tarjan takes
    // references into an immutable node collection while the inference graph
    // mutates its nodes (PORTING_RULES adapter adapter, registered);
    //
    // NOT PORTED IN THIS FILE (registered: dependency batches; no substitutes are
    // written, the upstream signatures depend on services that are not ported yet):
    // Ported in the Resolve B (this file; Registration rows in the registration):
    // - Infer.instantiateMethod (Infer.java:166-260) with the graph/legacy solver
    // paths, PartiallyInferredMethodType/check (287-360);
    // - generateReturnConstraints/needsEagerInstantiation/
    // generateReferenceToTargetConstraint/shouldPropagate/roots
    // (Infer.java:257-520) — Log.note(Notes.DeferredMethodInst) remains registered
    // (Log/Notes).
    // NOT PORTED (registered):
    // - Infer.instantiatePolymorphicSignatureInstance/ImplicitArgType (Infer.java:
    // 555-630): needs Attr tree tags and DeferredAttr.DeferredTypeMap wiring
    // (Attr);
    // - Infer.instantiateFunctionalInterface (Infer.java:633-690): needs
    // Types.findDescriptorType and Check.checkValidGenericType (FunctionDescriptor
    //);
    // - Infer.dumpGraphsIfNeeded (Infer.java:360-390): file dumping (Options);
    // - Infer.instance(Context)/Infer(Context) constructor: the port uses explicit
    // service construction (ADR 0001), registered.


</details>

## compattr

- Rust: crates/javac-core/src/comp/attr.rs
- Upstream: com/sun/tools/javac/comp/Attr.java
- Status: ported (registry below)
- Original registry: com/sun/tools/javac/comp/Attr.java @ 982d53c (jdk-17.0.19-ga) (header migration pending (P0 second batch))

<details><summary>Original file header (migrated from source)</summary>

    #![allow(clippy::result_large_err)]
    // Origin: com/sun/tools/javac/comp/Attr.java @ 982d53c (jdk-17.0.19-ga)
    //
    // Attr A (structure + type/identifier/expression entry points).
    //
    // Registration (A):
    // Ported in this file: the Attr structure/instance equivalent (new with the
    // ported dependencies), the allow* feature switches, ResultInfo (the checked
    // subset), attribTree (the visitor dispatch frame), attribExpr (both
    // overloads), attribType (both overloads), attribType(node, TypeSymbol),
    // attribTypeVariables, attribAnnotationTypes, attribIdent (both overloads) and
    // attribIdentAsEnumType, plus the visitor methods for the type trees
    // (visitTypeIdent/visitTypeArray/visitTypeApply/visitWildcard/
    // visitTypeParameter) and the basic expression leaves (visitLiteral/visitIdent/
    // visitParens), with the exact upstream control flow.
    //
    // NOT PORTED IN THIS (registered; dependency batches not ported yet, no
    // substitute in this file):
    // - the CheckContext/CheckMode/checkType pipeline: ported in the dependency
    // (Check::check_type/CheckContext/CheckMode/NestedCheckContext/
    // BasicCheckContext; the compatible/report/checkWarner leaves that need
    // Types.isAssignable/convertWarner are registered);
    // - Infer.InferenceContext: the context structure/free/asInstType/
    // addFreeTypeListener/notifyChange are ported (comp/infer.rs); the free-type
    // listener recursion inside check() remains registered (Infer);
    // - DeferredAttr.DeferredAttrContext/AttrMode (DeferredAttr): the
    // attribTree deferred-mode/breakTree/matchBindings parts are registered;
    // - Resolve (Resolve): attribIdent/visitIdent/visitSelect resolution is
    // registered;
    // - Annotate/TypeAnnotations (Annotate): annotateTypeParameterSecondStage/
    // annotateTypeSecondStage are registered;
    // - Analyzer/AttrRecover/ArgumentAttr/MatchBindingsComputer (their batches):
    // attribStat/attribTree glue registered;
    // - Types: isAssignable/checkClassType call sites are registered until the
    // Check/Types batches provide them (Types.isConvertible/setBounds/erasure are
    // ported and used where upstream uses them);
    // - visitApply/visitSelect/visitExec/... the remaining Attr visitors: registered
    // for Attr B/C (the default visitTree keeps the upstream behavior:
    // the tree is not attributed by A yet -> explicit unimplemented).

</details>

## compcheck

- Rust: crates/javac-core/src/comp/check.rs
- Upstream: com/sun/tools/javac/comp/Check.java
- Status: ported (registry below)
- Original registry: com/sun/tools/javac/comp/Check.java @ 982d53c (jdk-17.0.19-ga) (header migration pending (P0 second batch))

<details><summary>Original file header (migrated from source)</summary>


    // Origin: com/sun/tools/javac/comp/Check.java @ 982d53c (jdk-17.0.19-ga)
    //
    // Registration: this file ports the Check subset required by
    // the Enter/Flow batches (class enter symbol/flags/duplicate checks, the
    // exception set utilities, and the -3 methods below).
    //
    // PORTED IN THE -3 STEP:
    // - typeTagError / checkNonVoid / checkClassOrArrayType / asTypeParam /
    // checkNullOrRefType / checkEmptyIf / checkDivZero / warnDivZero /
    // duplicateErasureError / varargsDuplicateError (Check.java:288-291,
    // 700-717, 723-727, 814-822, 1007-1016, 371-375, 3718-3737, 3783-3787).
    // STILL NOT PORTED (registered; dependencies in brackets):
    // - checkCastable (Check.java:632-645) [Types.isCastable(Type, Type, Warner)];
    // - checkRedundantCast (Check.java:647-660) [TreeInfo.containsTypeAnnotation,
    // DeferredLintHandler wiring via Attr];
    // - validate/Validator (Check.java:1397-1525) [firstIncompatibleTypeArg,
    // checkClassBounds, TreeInfo.containsTypeAnnotation, JCTree.Visitor scanner];
    // - checkUnique MTH branches [Check holds no Symtab service yet; the caller
    // passes no Symtab (ADR 0001 service wiring pending)];
    // - the rest of the upstream Check body (type checking beyond the ported
    // subset, annotation checks, lambda/method checks, warnings, sunApiHandler,
    //...).
    // The ported methods keep the upstream bodies and error keys; where a dependency
    // is not ported the branch is marked REGISTERED and not executed (no substitute).


</details>

## compresolve

- Rust: crates/javac-core/src/comp/resolve.rs
- Upstream: com/sun/tools/javac/comp/Resolve.java
- Status: ported (registry below)
- Original registry: com/sun/tools/javac/comp/Resolve.java @ 982d53c (jdk-17.0.19-ga) (header migration pending (P0 second batch))

<details><summary>Original file header (migrated from source)</summary>

    // PORTING_RULES: the upstream-shaped method-check code keeps large error
    // enums and nested conditionals; the lints are allowed locally (registered).
    #![allow(clippy::result_large_err, clippy::collapsible_if)]
    // Origin: com/sun/tools/javac/comp/Resolve.java @ 982d53c (jdk-17.0.19-ga)

    //! Name/method resolution (Java: com.sun.tools.javac.comp.Resolve).
    //!
    //! PORTING_RULES:
    //! * the resolution services are injected through the explicit-context adapter
    //!   (ADR 0001): Symtab/Types are passed as parameters, not stored global
    //!   singletons; the ported Resolve state keeps names/log/types/diags/source/
    //!   options only.
    //! * the Java overload sets become distinct Rust names (PORTING_RULES Porting rules):
    //!   isAccessible(Env, TypeSymbol) -> is_accessible_class,
    //!   isAccessible(Env, Type) -> is_accessible_type,
    //!   isAccessible(Env, Type, Symbol) -> is_accessible_member.
    //! * Resolve.instance(Context) is registered for the Context registration.
    //!
    //! Ported in the Resolve B (this file, Porting rules rows in the report):
    //! - Resolve.methodResolutionSteps/MethodResolutionPhase (BASIC/BOX/VARARITY,
    //!   mergeResults), MethodResolutionContext/Candidate, InapplicableMethodException,
    //!   MethodCheckDiag, LookupFilter, InterfaceLookupPhase, superclasses;
    //! - rawInstantiate/checkMethod/instantiate (Resolve.java:568-733);
    //! - MethodCheck implementations: NilMethodCheck/ArityMethodCheck/
    //!   ResolveMethodCheck/MostSpecificCheck/MethodReferenceCheck (class + entry),
    //!   AbstractMethodCheck.argumentsAcceptable, MethodCheckContext, MethodResultInfo,
    //!   adjustArgs (varargs branch), varargsAccessible (access check +
    //!   INACCESSIBLE_VARARGS), reportMC method-check fragments (MethodCheckDiag
    //!   basic/infer key + argument selection);
    //! - selectBest/mostSpecific/signatureMoreSpecific/ambiguityError/
    //!   findMethodInScope/findMethod/findFun/findConstructor/findDiamond;
    //! - lookupMethod/LookupHelper/BasicLookupHelper and the resolve* family:
    //!   resolveMethod/resolveQualifiedMethod/resolveInternalMethod/
    //!   resolveConstructor/resolveInternalConstructor/resolveDiamond;
    //! - ResolveError data + diagnostics: InapplicableSymbolError/
    //!   InapplicableSymbolsError/DiamondError/BadVarargsMethod/BadClassFileError
    //!   added to the existing SymbolNotFound/Access/Invisible/Static/Ambiguity/
    //!   BadRestrictedType hierarchy (Resolve.java:3904-4720);
    //! - access errors/ambiguity helpers (new_access_error/new_ambiguity_error/
    //!   new_static_error/new_bad_class_file_error/resolve_error_access/
    //!   resolve_error_diagnostic);
    //! - loadClass BadClassFile distinction, findConstructor deprecated/preview
    //!   checks, isAccessible union alternatives, currModule.complete, and the
    //!   in-query LookupFilter (Resolve.java:345/390-392/1787/2054/2896-2897).
    //!
    //! NOT YET PORTED IN THIS FILE (registered; each with a not-yet-ported marker + REGISTERED):
    //! - member-reference resolution: makeReferenceLookupHelper/
    //!   MethodReferenceLookupHelper/UnboundMethodReferenceLookupHelper/
    //!   ArrayConstructorReferenceLookupHelper/ConstructorReferenceLookupHelper/
    //!   ReferenceLookupResult/getMemberReference/resolveMemberReference/
    //!   ReferenceLookupHelper (Resolve.java:3000-3650; needs
    //!   Types.findDescriptorType + Attr member-reference wiring, FunctionDescriptor
    //!   );
    //! - MethodReferenceCheck.argumentsAcceptable body (Resolve.java:1000-1070,
    //!   same dependency);
    //! - selectBest inference-exception propagation (Resolve.java:1592-1593) and
    //!   AmbiguityError.mergeAbstracts (Types);
    //! - verbose resolution diagnostics (reportVerboseResolutionDiagnostic/
    //!   VerboseResolutionMode): Log/Options;
    //! - MethodSymbol.implementation(hiddenIn path): Symbol;
    //! - Types.overrideEquivalent/Types.mergeAbstracts: Types.


</details>

## complower

- Rust: crates/javac-core/src/comp/lower.rs
- Upstream: com/sun/tools/javac/comp/Lower.java
- Status: ported (registry below)
- Original registry: com/sun/tools/javac/comp/Lower.java @ 982d53c (jdk-17.0.19-ga) (header migration pending (P0 second batch))

<details><summary>Original file header (migrated from source)</summary>

    // Origin: com/sun/tools/javac/comp/Lower.java @ 982d53c (jdk-17.0.19-ga)

    //! Translates away syntactic sugar before code generation: inner classes and
    //! their access bridges, class literals, assertions, foreach loops, enum
    //! switches, string concatenation and assignment operators.
    //!
    //! The Java base class TreeTranslator is expressed here through the same visitor
    //! framework methods used by the other lowering pass: a translate/result pair,
    //! prototype-type scoped helpers, and the standard tree navigation helpers.
    //!
    //! The pass keeps mutable lower state (current class/method, proxy map, access
    //! method cache, enum switch maps) on the Lower value itself, as upstream does;
    //! service handles are injected explicitly.


</details>

## compflow

- Rust: crates/javac-core/src/comp/flow.rs
- Upstream: com/sun/tools/javac/comp/Flow.java
- Status: ported (registry below)
- Original registry: com/sun/tools/javac/comp/Flow.java @ 982d53c (jdk-17.0.19-ga) (header migration pending (P0 second batch))

<details><summary>Original file header (migrated from source)</summary>

    // Origin: com/sun/tools/javac/comp/Flow.java @ 982d53c (jdk-17.0.19-ga)
    //
    // PORTING_RULES:
    // - ADR 0001: the ported Flow takes the services explicitly (Flow.instance/
    // Flow(Context) are registered with the Context registration); the
    // visitor subclass hierarchy maps to the FlowVisitor trait (the default
    // visit methods are the ported TreeScanner navigational bodies, so a
    // specialized analyzer overrides only the visit methods it cares about,
    // exactly like the upstream TreeScanner subclasses);
    // - the upstream TreeScanner.scan(tree) -> tree.accept(this) becomes
    // FlowVisitor::scan (accept dispatch), and scan_list/scan_list_opt are the
    // ported list helpers;
    // - the Java ListBuffer<PendingExit> maps to the ported List<PendingExit> in
    // the analyzer state (same append/iterate order);
    // - the FlowAnalyzer.ThrownPendingExit / AssignAnalyzer.AssignPendingExit
    // subclasses map to PendingExit variants;
    // - adapter adapters: JCTree.type checks read the arena type slot; the diagnostic
    // position surface is the i32 preferred position; the Log error/warning
    // helpers (log.error(pos, fragment/key), log.warning(LintCategory,...)) are
    // implemented in this file on top of Factory::create + Log::report
    // (registered with the Log).


</details>

## jvmcodegen

- Rust: crates/javac-core/src/jvm/codegen.rs
- Upstream: com/sun/tools/javac/jvm/Gen.java
- Status: ported (registry below)
- Original registry: com/sun/tools/javac/jvm/Gen.java (header migrated in P0 first batch)

<details><summary>Original file header (migrated from source)</summary>

    // Source: com/sun/tools/javac/jvm/Gen.java

    //! This pass maps flat Java (without inner classes) to bytecodes.


</details>

## parserjavac-parser

- Rust: crates/javac-core/src/parser/javac_parser.rs
- Upstream: com/sun/tools/javac/parser/JavacParser.java
- Status: ported (registry below)
- Original registry: com/sun/tools/javac/parser/JavacParser.java @ 982d53c (jdk-17.0.19-ga) (header migration pending (P0 second batch))

<details><summary>Original file header (migrated from source)</summary>

    // Origin: com/sun/tools/javac/parser/JavacParser.java @ 982d53c (jdk-17.0.19-ga)
    // Gap registration (method-level audit of the ported upstream class):
    // Dependency/phase: dependent service (see docs/PLAN.md).
    // Not yet ported upstream methods (no substitutes in this file):
    // - NOT PORTED: newEndPosTable (JavacParser.java:193)
    // - NOT PORTED: newOdStack (JavacParser.java:1101)
    // - NOT PORTED: newOpStack (JavacParser.java:1107)
    // - NOT PORTED: setParser (JavacParser.java:5035)
    // End of Gap registration.

    //! The parser maps a token sequence into an abstract syntax tree.
    //! The parser is a hand-written recursive-descent parser that
    //! implements the grammar described in the Java Language Specification.
    //! For efficiency reasons, an operator precedence scheme is used
    //! for parsing binary operation expressions.
    //!
    //! <p><b>This is NOT part of any supported API.
    //! If you write code that depends on this, you do so at your own risk.
    //! This code and its internal interfaces are subject to change or
    //! deletion without notice.</b>
    //!
    //! PORTING_RULES / status (S7b):
    //! * JCTree values are arena TreeIds (ADR 0001); every parse method takes the
    //!   CompilerContext because tree creation needs the arena;
    //! * the Java nested Abstract/Simple/Empty EndPosTable classes are ported in
    //!   this file (Simple uses util::int_hash_table::IntHashTable);
    //! * the full Java 17 grammar is parsed: records, sealed types, patterns
    //!   (instanceof/switch), switch expressions/rules, lambdas, method
    //!   references, module declarations and annotation type declarations;
    //! * keepDocComments=true and keepDocComments=false both work: the
    //!   ported LazyDocCommentTable attaches comments lazily (JavacParser.attach)
    //!   and tree/Pretty prints them from the compilation unit doc comment table;
    //! * DocCommentParser's ReferenceParser is ported and used for doc-comment
    //!   references.
    //!
    //! clippy debt registration (pre-existing S6/S7a items, NOT from the S7b;
    //! cleanup happens with the parser-wiring/annotations batches):
    //! - collapsible-if at 631/1548/2093/2726/2727/3128/3375/4353;
    //! - unneeded-return at 2186/2192; unit-let at 4612;
    //! - manual isolate_lowest_one at 755 (check_no_mods_at);
    //! - to_* self-by-value at 796 (to_p); Result::unwrap_or at 976/1012 (literal);
    //! - &mut Vec vs slice at 2689 (fold helper); while-let at 3881
    //!   (insertAnnotationsToMostInner);
    //! - these are all equivalent upstream-structure ports; the remaining lint
    //!   changes are stylistic and are scheduled for the parser-wiring.


</details>

## codetypes

- Rust: crates/javac-core/src/code/types.rs
- Upstream: com/sun/tools/javac/code/Types.java
- Status: ported (registry below)
- Original registry: com/sun/tools/javac/code/Types.java @ 982d53c (jdk-17.0.19-ga) (header migration pending (P0 second batch))

<details><summary>Original file header (migrated from source)</summary>

    #![allow(clippy::result_large_err)]
    #![allow(clippy::type_complexity)]

    // Origin: com/sun/tools/javac/code/Types.java @ 982d53c (jdk-17.0.19-ga)
    // Method-level audit status (Type/Types batch): the previous 123-method
    // NOT PORTED list is resolved below. Every unit maps to a local implementation,
    // a dispatch adapter, or an explicitly registered dependency.
    //
    // 1) Implemented in this batch (upstream line -> local unit):
    //    isSubtypes (1250-1259) -> is_subtypes; isSubtypesUnchecked (1266-1275) ->
    //    is_subtypes_unchecked; cvarLowerBounds (1978-1980) -> cvar_lower_bounds;
    //    isArray (2059-2063) -> is_array; elemtypeOrType (2083-2088) ->
    //    elemtype_or_type; isDerivedRaw/isDerivedRawInternal/isDerivedRaw(List)
    //    (2670-2692) -> is_derived_raw/is_derived_raw_internal/is_derived_raw_types
    //    (+ is_derived_raw_cache field); substBound (3448-3461) -> subst_bound;
    //    typaramsString/appendTyparamString (3656-3693) -> typarams_string/
    //    append_typaram_string (+ types_to_string for toString(Type));
    //    closureCollector/ClosureHolder (3735-3765) -> closure_collector/
    //    ClosureHolder/ClosureCollector (the Java Collector/Stream lifecycle is
    //    exposed explicitly, registered adapter); insert/union with BiPredicate
    //    (3772-3808) -> closure_insert_with/closure_union_with + ClosureSkip enum
    //    (BiPredicate represented by the two upstream predicates, registered);
    //    newRound (5257-5263) -> new_round.
    //
    // 2) Implemented before this batch under a different local name or by the
    //    port's match dispatch (audited, upstream line -> local unit):
    //    asSub (539) -> as_super/as_sub family; descriptor chain (640-885) ->
    //    FunctionDescriptorLookupError/descriptor_failure/find_descriptor_*/
    //    is_functional_interface/remove_wildcards/merge_descriptors; functional
    //    interface bridges (834-1010) -> functional_interface_bridges/
    //    pending_bridges; checkUnsafeVarargsConversion (1044) ->
    //    check_unsafe_varargs_conversion; isSignaturePolymorphic (1327) ->
    //    is_signature_polymorphic; disjointTypes/disjointType (1888-1974) ->
    //    disjoint_types/disjoint_type (+ recursive helpers folded into the
    //    cached match, registered); relaxBound (2012) -> relax_bound;
    //    dimensions/makeArrayType (2108-2128) -> dimensions/make_array_type;
    //    isAssignable (2338-2348) -> is_assignable*; isDirectSuperInterface (2659)
    //    -> is_direct_super_interface; setBounds/getBounds (2706-2750) ->
    //    set_bounds/get_bounds; isSubSignature (2795-2799) -> is_sub_signature*;
    //    overrideEquivalent (2816) -> override_equivalent; mergeAbstracts (2874) ->
    //    merge_abstracts (anonymous baseSymbol() carried as
    //    MergeAbstractsResult.base_symbol, registered); implementation/
    //    implementationInternal (2978-3005) -> implementation_filtered/
    //    implementation_internal; membersClosure (3091+) -> members_closure/
    //    members_closure_visit; firstUnimplementedAbstract/interfaceCandidates/
    //    prune (3101-3220) -> first_unimplemented_abstract*/interface_candidates/
    //    prune; substBounds (3407) -> subst_bounds; newInstances (3487) ->
    //    new_instances; createMethodTypeWith*(3503-3535) -> create_method_type_with*;
    //    baseType/createErrorType (3546-3570) -> base_type/create_error_type;
    //    closure/compoundMin/closureMin/glbFlattened (3710-4124) -> closure/
    //    compound_min/closure_min/glb_flattened; resultSubtype/
    //    returnTypeSubstitutable/covariantReturnType (4249-4292) ->
    //    result_subtype/return_type_substitutable/covariant_return_type;
    //    boxedTypeOrType/unboxedTypeOrType (4312-4336) -> boxed_type_or_type/
    //    unboxed_type_or_type; sideCast/sideCastFinal (4479-4509) -> side_cast/
    //    side_cast_final; giveWarning (4535) -> give_warning; superClosure (4550) ->
    //    super_closure; adaptRecursive (4664-4675) -> adapt_recursive;
    //    rewriteQuantifiers (4720) -> rewrite_quantifiers; rewriteAsWildcardType/
    //    makeExtendsWildcard/makeSuperWildcard (4784-4844) ->
    //    rewrite_as_wildcard_type/make_extends_wildcard/make_super_wildcard;
    //    getRetention (5021-5039) -> get_retention; SignatureGenerator
    //    assemble*/append*/classReference/hasTypeVar (5044-5229) ->
    //    SignatureGenerator trait (append_char/append_bytes/append_name/
    //    class_reference/assemble_sig/assemble_class_sig/assemble_params_sig/
    //    has_type_var/report_illegal_signature, registered adapter for the Java
    //    abstract callbacks); constantType (5231) -> constant_type.
    //    TypeRelation/TypeMapping/SimpleVisitor/Default(Simple)Visitor overrides
    //    (visitCapturedType/visitUndetVar/visitPackageType/visitClassSymbol/
    //    visitMethodSymbol/visitOperatorSymbol/visitPackageSymbol/visitTypeSymbol/
    //    visitVarSymbol/apply; 478/1214/1447/1463/1593/1983/4229/4758/4905-4962/
    //    5005/5012) are implemented by the equivalent TypeKindData match branches
    //    (the Java visitor hierarchy is a dispatch mechanism; registered adapter).
    //
    // 3) Registered adapters:
    //    - Java Collector/Stream closureCollector -> ClosureCollector builder.
    //    - BiPredicate<Type,Type> -> ClosureSkip (Basic/IsSameType, the only two
    //      upstream predicates).
    //    - Java visitor classes -> TypeKindData match dispatch.
    //    - TypeMapping.apply (5005) -> the explicit map methods.
    //
    // 4) Registered adapters/dependencies:
    //    - needsStripping (Types.java:276/295/404/3392/3399): the Type.java
    //      stripMetadata/stripMetadataIfNeeded visitor machinery is not ported
    //      (Type.java batch; API JavacTypes/JavacTrees also not ported).
    //    - MembersScope.combine (Types.java:3024-3042) is implemented by the
    //      MembersScope wrapper: every getSymbols/getSymbolsByName query applies
    //      !s.owner.isInterface() when skipInterface is set.
    //    - Type.contains / containsType are implemented by Types::contains and the
    //      type_contains method (live uses at 1997/5986/6065); the free helper below
    //      is retained for its unit test (registered adapter).
    //    - TypeVar.rank_field reset (Types.java:2723): Type.java batch.
    //    - lub Names service not injected in the current call path (local
    //      expect "lub needs the Names service"): service-wiring batch.
    //    - isSubtypeUnchecked(..., Warner) is implemented: the raw-conversion branch
    //      of isSubtypeUncheckedInternal emits LintCategory.UNCHECKED
    //      (silentWarn for reifiable targets) and the warn variant runs
    //      checkUnsafeVarargsConversion (Types.java:1011-1064).
    // End of Gap registration.

    //! Utility routines for the Type interface.
    //!
    //! <p><b>This is NOT part of any supported API.
    //! If you write code that depends on this, you do so at your own risk.
    //! This code and its internal interfaces are subject to change or
    //! deletion without notice.</b>
    //!
    //! PORTING_RULES (ADR 0001, arena + IDs): this ports ONLY the Types
    //! subset needed to unlock TreeMaker/class-fields (see the task):
    //! hashCode, isSameType, erasure, boxedClass, and the helpers they need.
    //! The Java Types instance state (Symtab, Names, Source, Check, Enter,
    //! JavacMessages, JCDiagnostic.Factory, Warner) is NOT stored in the Rust Types
    //! (which is a stateless service): the methods that need it take the ported
    //! Symtab explicitly (registered section 9; no Rc/RefCell is introduced). The
    //! The former method-level list (historical): isSubtype / isCastable / containsType /
    //! asSuper / memberType / subst / capture / supertype / interfaces / lub / glb /
    //! isReifiable / rank) is implemented in this file; see the audit at the top.
    //! Remaining dependencies are needsStripping, TypeVar.rank_field reset and lub
    //! Names injection (registered above).
    //!
    //! PORTING_RULES: the Type.java helpers getEnclosingType/getTypeArguments/
    //! getParameterTypes/getReturnType are ported as Type methods in type_.rs; the
    //! free helpers below delegate to those methods (no duplicate matching logic).
    //! TypeMetadata remains unported (equalsIgnoreMetadata == equals).
    //!
    //! PORTING_RULES (hashCode): the upstream Name.hashCode() is
    //! Object.identityHashCode (Name.java has no hashCode override), which is not
    //! reproducible across processes; the port uses the javac name-table hash
    //! (NameTable.hashValue over the name bytes) as a stable equivalent (any stable
    //! value consistent with identity equality is equivalent for the HashMap usage;
    //! registered). Likewise BoundKind.hashCode()/TypeVar identity hash are
    //! enum/identity hashes upstream; the port uses BoundKind.ordinal and the TypeId
    //! arena index (same instance gives the same value, different instances give
    //! different values; registered).


</details>

## mainmini-compiler

- Rust: crates/javac-core/src/main/mini_compiler.rs
- Upstream: com/sun/tools/javac/main/JavaCompiler.java
- Status: adapter (minimal driver; the full JavaCompiler port is deferred)
- Original registry: com/sun/tools/javac/main/JavaCompiler.java (minimal wiring subset) (header migrated in P0 first batch)

<details><summary>Original file header (migrated from source)</summary>

    // Source: com/sun/tools/javac/main/JavaCompiler.java (minimal wiring subset)

    //! Minimal compiler wiring for the compilation pipeline.
    //!
    //! PORTING ADAPTER (registered): the 1:1 JavaCompiler.java port is deferred to
    //! the batch that ports Modules/TransPatterns/LambdaToMethod/
    //! JavacProcessingEnvironment/TaskEvent+TaskListener/MultiTaskListener/
    //! DeferredCompletionFailureHandler/JNIWriter. This file wires the same phase
    //! sequence (parse -> enter -> attribute -> flow -> transTypes -> lower ->
    //! generate -> class writer) over the ported services for sources that use no
    //! pattern matching, lambdas, modules or annotation processing; every phase
    //! call below is the upstream JavaCompiler call of that phase.


</details>

## Full upstream mapping

| Rust file | Upstream |
|---|---|
| crates/javac-core/src/api/diagnostic_formatter.rs | com/sun/tools/javac/api/DiagnosticFormatter.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/api/formattable.rs | com/sun/tools/javac/api/Formattable.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/api/messages.rs | com/sun/tools/javac/api/Messages.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/attribute.rs | com/sun/tools/javac/code/Attribute.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/bound_kind.rs | com/sun/tools/javac/code/BoundKind.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/class_finder.rs | com/sun/tools/javac/code/ClassFinder.java @ 982d53c (jdk-17.0.19-ga) Gap registration (method-level audit of ClassFinder.java): |
| crates/javac-core/src/code/deferred_completion_failure_handler.rs | com/sun/tools/javac/code/DeferredCompletionFailureHandler.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/deferred_lint_handler.rs | com/sun/tools/javac/code/DeferredLintHandler.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/dependencies.rs | com/sun/tools/javac/util/Dependencies.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/directive.rs | com/sun/tools/javac/code/Directive.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/flags.rs | com/sun/tools/javac/code/Flags.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/kinds.rs | com/sun/tools/javac/code/Kinds.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/lint.rs | com/sun/tools/javac/code/Lint.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/missing_info_handler.rs | com/sun/tools/javac/code/MissingInfoHandler.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/module_finder.rs | com/sun/tools/javac/code/ModuleFinder.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/preview.rs | com/sun/tools/javac/code/Preview.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/printer.rs | com/sun/tools/javac/code/Printer.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/scope.rs | com/sun/tools/javac/code/Scope.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/source.rs | com/sun/tools/javac/code/Source.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/symbol.rs | com/sun/tools/javac/code/Symbol.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/symbol_metadata.rs | com/sun/tools/javac/code/SymbolMetadata.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/symtab.rs | com/sun/tools/javac/code/Symtab.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/target_type.rs | com/sun/tools/javac/code/TargetType.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/type_.rs | com/sun/tools/javac/code/Type.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/type_annotation_position.rs | com/sun/tools/javac/code/TypeAnnotationPosition.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/type_annotations.rs | com/sun/tools/javac/code/TypeAnnotations.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/type_metadata.rs | com/sun/tools/javac/code/TypeMetadata.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/type_tag.rs | com/sun/tools/javac/code/TypeTag.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/code/types.rs | com/sun/tools/javac/code/Types.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/analyzer.rs | com/sun/tools/javac/comp/Analyzer.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/annotate.rs | com/sun/tools/javac/comp/Annotate.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/argument_attr.rs | com/sun/tools/javac/comp/ArgumentAttr.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/attr.rs | com/sun/tools/javac/comp/Attr.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/attr_context.rs | com/sun/tools/javac/comp/AttrContext.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/attr_recover.rs | com/sun/tools/javac/comp/AttrRecover.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/check.rs | com/sun/tools/javac/comp/Check.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/compile_states.rs | com/sun/tools/javac/comp/CompileStates.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/const_fold.rs | com/sun/tools/javac/comp/ConstFold.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/deferred_attr.rs | com/sun/tools/javac/comp/DeferredAttr.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/enter.rs | com/sun/tools/javac/comp/Enter.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/env.rs | com/sun/tools/javac/comp/Env.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/flow.rs | com/sun/tools/javac/comp/Flow.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/lambda_to_method.rs | com/sun/tools/javac/comp/LambdaToMethod.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/lower.rs | com/sun/tools/javac/comp/Lower.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/match_bindings.rs | com/sun/tools/javac/comp/MatchBindingsComputer.java @ 982d53c |
| crates/javac-core/src/comp/member_enter.rs | com/sun/tools/javac/comp/MemberEnter.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/modules.rs | com/sun/tools/javac/comp/Modules.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/operators.rs | com/sun/tools/javac/comp/Operators.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/resolve.rs | com/sun/tools/javac/comp/Resolve.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/trans_patterns.rs | com/sun/tools/javac/comp/TransPatterns.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/trans_types.rs | com/sun/tools/javac/comp/TransTypes.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/tree_differ.rs | com/sun/tools/javac/comp/TreeDiffer.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/tree_hasher.rs | com/sun/tools/javac/comp/TreeHasher.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/type_enter.rs | com/sun/tools/javac/comp/TypeEnter.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/comp/type_envs.rs | com/sun/tools/javac/comp/TypeEnvs.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/file/base_file_manager.rs | com/sun/tools/javac/file/BaseFileManager.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/file/cache_fs_info.rs | com/sun/tools/javac/file/CacheFSInfo.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/file/fs_info.rs | com/sun/tools/javac/file/FSInfo.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/file/javac_file_manager.rs | com/sun/tools/javac/file/JavacFileManager.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/file/jrt_index.rs | com/sun/tools/javac/file/JRTIndex.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/file/locations.rs | com/sun/tools/javac/file/Locations.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/file/path_file_object.rs | com/sun/tools/javac/file/PathFileObject.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/file/relative_path.rs | com/sun/tools/javac/file/RelativePath.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/jvm/byte_codes.rs | com/sun/tools/javac/jvm/ByteCodes.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/jvm/class_file.rs | com/sun/tools/javac/jvm/ClassFile.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/jvm/class_reader.rs | com/sun/tools/javac/jvm/ClassReader.java @ 982d53c (jdk-17.0.19-ga) Gap registration (method-level audit of ClassReader.java): |
| crates/javac-core/src/jvm/jni_writer.rs | com/sun/tools/javac/jvm/JNIWriter.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/jvm/module_name_reader.rs | com/sun/tools/javac/jvm/ModuleNameReader.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/jvm/pool_constant.rs | com/sun/tools/javac/jvm/PoolConstant.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/jvm/pool_reader.rs | com/sun/tools/javac/jvm/PoolReader.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/jvm/profile.rs | com/sun/tools/javac/jvm/Profile.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/jvm/target.rs | com/sun/tools/javac/jvm/Target.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/main/option.rs | com/sun/tools/javac/main/Option.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/main/option_helper.rs | com/sun/tools/javac/main/OptionHelper.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/model/annotation_mirrors.rs | javax/lang/model (AnnotatedConstruct.getAnnotationMirrors) and the javax.lang.model mirror objects of com.sun.tools.javac.model.JavacElements / |
| crates/javac-core/src/model/annotation_proxy_maker.rs | com/sun/tools/javac/model/AnnotationProxyMaker.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/model/symbol_annotations.rs | com/sun/tools/javac/code/AnnoConstruct.java and the annotated API of com/sun/tools/javac/code/Symbol.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/parser/doc_comment_parser.rs | com/sun/tools/javac/parser/DocCommentParser.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/parser/java_tokenizer.rs | com/sun/tools/javac/parser/JavaTokenizer.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/parser/javac_parser.rs | com/sun/tools/javac/parser/JavacParser.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/parser/javadoc_tokenizer.rs | com/sun/tools/javac/parser/JavadocTokenizer.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/parser/lazy_doc_comment_table.rs | com/sun/tools/javac/parser/LazyDocCommentTable.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/parser/lexer.rs | com/sun/tools/javac/parser/Lexer.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/parser/parser.rs | com/sun/tools/javac/parser/Parser.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/parser/parser_factory.rs | com/sun/tools/javac/parser/ParserFactory.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/parser/reference_parser.rs | com/sun/tools/javac/parser/ReferenceParser.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/parser/scanner.rs | com/sun/tools/javac/parser/Scanner.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/parser/scanner_factory.rs | com/sun/tools/javac/parser/ScannerFactory.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/parser/text_block_support.rs | com/sun/tools/javac/parser/TextBlockSupport.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/parser/tokens.rs | com/sun/tools/javac/parser/Tokens.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/parser/unicode_reader.rs | com/sun/tools/javac/parser/UnicodeReader.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/platform/ct_sym_java_file_manager.rs | com/sun/tools/javac/platform/JDKPlatformProvider.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/platform/jdk_platform_provider.rs | com/sun/tools/javac/platform/JDKPlatformProvider.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/platform/platform_description.rs | com/sun/tools/javac/platform/PlatformDescription.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/platform/platform_provider.rs | com/sun/tools/javac/platform/PlatformProvider.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/platform/platform_utils.rs | com/sun/tools/javac/platform/PlatformUtils.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/tree/dc_tree.rs | com/sun/tools/javac/tree/DCTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/tree/doc_comment_table.rs | com/sun/tools/javac/tree/DocCommentTable.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/tree/doc_pretty.rs | com/sun/tools/javac/tree/DocPretty.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/tree/doc_tree_maker.rs | com/sun/tools/javac/tree/DocTreeMaker.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/tree/end_pos_table.rs | com/sun/tools/javac/tree/EndPosTable.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/tree/jc_tree.rs | com/sun/tools/javac/tree/JCTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/tree/pretty.rs | com/sun/tools/javac/tree/Pretty.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/tree/tree_copier.rs | com/sun/tools/javac/tree/TreeCopier.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/tree/tree_info.rs | com/sun/tools/javac/tree/TreeInfo.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/tree/tree_maker.rs | com/sun/tools/javac/tree/TreeMaker.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/tree/tree_scanner.rs | com/sun/tools/javac/tree/TreeScanner.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/tree/tree_translator.rs | com/sun/tools/javac/tree/TreeTranslator.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/abort.rs | com/sun/tools/javac/util/Abort.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/abstract_diagnostic_formatter.rs | com/sun/tools/javac/util/AbstractDiagnosticFormatter.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/abstract_log.rs | com/sun/tools/javac/util/AbstractLog.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/array_utils.rs | com/sun/tools/javac/util/ArrayUtils.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/assert.rs | com/sun/tools/javac/util/Assert.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/basic_diagnostic_formatter.rs | com/sun/tools/javac/util/BasicDiagnosticFormatter.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/bits.rs | com/sun/tools/javac/util/Bits.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/byte_buffer.rs | com/sun/tools/javac/util/ByteBuffer.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/client_code_exception.rs | com/sun/tools/javac/util/ClientCodeException.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/constants.rs | com/sun/tools/javac/util/Constants.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/context.rs | com/sun/tools/javac/util/Context.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/convert.rs | com/sun/tools/javac/util/Convert.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/diagnostic_source.rs | com/sun/tools/javac/util/DiagnosticSource.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/fatal_error.rs | com/sun/tools/javac/util/FatalError.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/forwarding_diagnostic_formatter.rs | com/sun/tools/javac/util/ForwardingDiagnosticFormatter.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/graph_utils.rs | com/sun/tools/javac/util/GraphUtils.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/int_hash_table.rs | com/sun/tools/javac/util/IntHashTable.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/iterators.rs | com/sun/tools/javac/util/Iterators.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/javac_messages.rs | com/sun/tools/javac/util/JavacMessages.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/jc_diagnostic.rs | com/sun/tools/javac/util/JCDiagnostic.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/layout_characters.rs | com/sun/tools/javac/util/LayoutCharacters.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/list_buffer.rs | com/sun/tools/javac/util/ListBuffer.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/log.rs | com/sun/tools/javac/util/Log.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/mandatory_warning_handler.rs | com/sun/tools/javac/util/MandatoryWarningHandler.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/module_helper.rs | com/sun/tools/javac/util/ModuleHelper.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/name.rs | com/sun/tools/javac/util/Name.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/names.rs | com/sun/tools/javac/util/Names.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/options.rs | com/sun/tools/javac/util/Options.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/pair.rs | com/sun/tools/javac/util/Pair.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/position.rs | com/sun/tools/javac/util/Position.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/propagated_exception.rs | com/sun/tools/javac/util/PropagatedException.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/raw_diagnostic_formatter.rs | com/sun/tools/javac/util/RawDiagnosticFormatter.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/shared_name_table.rs | com/sun/tools/javac/util/SharedNameTable.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/string_utils.rs | com/sun/tools/javac/util/StringUtils.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/unshared_name_table.rs | com/sun/tools/javac/util/UnsharedNameTable.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-core/src/util/warner.rs | com/sun/tools/javac/util/Warner.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-floating/src/big_integer.rs | java/math/BigInteger.java + java/math/BitSieve.java + java/math/SignedMutableBigInteger.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-floating/src/digit_helper.rs | java/lang/Character.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-floating/src/fdbig_integer.rs | jdk/internal/math/FDBigInteger.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-floating/src/floating_decimal.rs | jdk/internal/math/FloatingDecimal.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-floating/src/lib.rs | jdk/internal/math/FloatingDecimal.java + FDBigInteger.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-floating/src/mutable_big_integer.rs | java/math/MutableBigInteger.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-floating/src/random.rs | java/util/Random.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/attribute_tree.rs | com/sun/source/doctree/AttributeTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/author_tree.rs | com/sun/source/doctree/AuthorTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/block_tag_tree.rs | com/sun/source/doctree/BlockTagTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/comment_tree.rs | com/sun/source/doctree/CommentTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/deprecated_tree.rs | com/sun/source/doctree/DeprecatedTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/doc_comment_tree.rs | com/sun/source/doctree/DocCommentTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/doc_root_tree.rs | com/sun/source/doctree/DocRootTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/doc_tree.rs | com/sun/source/doctree/DocTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/doc_tree_visitor.rs | com/sun/source/doctree/DocTreeVisitor.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/doc_type_tree.rs | com/sun/source/doctree/DocTypeTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/end_element_tree.rs | com/sun/source/doctree/EndElementTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/entity_tree.rs | com/sun/source/doctree/EntityTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/erroneous_tree.rs | com/sun/source/doctree/ErroneousTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/hidden_tree.rs | com/sun/source/doctree/HiddenTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/identifier_tree.rs | com/sun/source/doctree/IdentifierTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/index_tree.rs | com/sun/source/doctree/IndexTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/inherit_doc_tree.rs | com/sun/source/doctree/InheritDocTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/inline_tag_tree.rs | com/sun/source/doctree/InlineTagTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/link_tree.rs | com/sun/source/doctree/LinkTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/literal_tree.rs | com/sun/source/doctree/LiteralTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/mod.rs | com/sun/source/doctree/package-info.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/param_tree.rs | com/sun/source/doctree/ParamTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/provides_tree.rs | com/sun/source/doctree/ProvidesTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/reference_tree.rs | com/sun/source/doctree/ReferenceTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/return_tree.rs | com/sun/source/doctree/ReturnTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/see_tree.rs | com/sun/source/doctree/SeeTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/serial_data_tree.rs | com/sun/source/doctree/SerialDataTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/serial_field_tree.rs | com/sun/source/doctree/SerialFieldTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/serial_tree.rs | com/sun/source/doctree/SerialTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/since_tree.rs | com/sun/source/doctree/SinceTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/start_element_tree.rs | com/sun/source/doctree/StartElementTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/summary_tree.rs | com/sun/source/doctree/SummaryTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/system_property_tree.rs | com/sun/source/doctree/SystemPropertyTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/text_tree.rs | com/sun/source/doctree/TextTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/throws_tree.rs | com/sun/source/doctree/ThrowsTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/unknown_block_tag_tree.rs | com/sun/source/doctree/UnknownBlockTagTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/unknown_inline_tag_tree.rs | com/sun/source/doctree/UnknownInlineTagTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/uses_tree.rs | com/sun/source/doctree/UsesTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/value_tree.rs | com/sun/source/doctree/ValueTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/doctree/version_tree.rs | com/sun/source/doctree/VersionTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/lib.rs | com/sun/source (module) @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/annotated_type_tree.rs | com/sun/source/tree/AnnotatedTypeTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/annotation_tree.rs | com/sun/source/tree/AnnotationTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/array_access_tree.rs | com/sun/source/tree/ArrayAccessTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/array_type_tree.rs | com/sun/source/tree/ArrayTypeTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/assert_tree.rs | com/sun/source/tree/AssertTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/assignment_tree.rs | com/sun/source/tree/AssignmentTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/binary_tree.rs | com/sun/source/tree/BinaryTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/binding_pattern_tree.rs | com/sun/source/tree/BindingPatternTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/block_tree.rs | com/sun/source/tree/BlockTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/break_tree.rs | com/sun/source/tree/BreakTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/case_label_tree.rs | com/sun/source/tree/CaseLabelTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/case_tree.rs | com/sun/source/tree/CaseTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/catch_tree.rs | com/sun/source/tree/CatchTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/class_tree.rs | com/sun/source/tree/ClassTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/compilation_unit_tree.rs | com/sun/source/tree/CompilationUnitTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/compound_assignment_tree.rs | com/sun/source/tree/CompoundAssignmentTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/conditional_expression_tree.rs | com/sun/source/tree/ConditionalExpressionTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/continue_tree.rs | com/sun/source/tree/ContinueTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/default_case_label_tree.rs | com/sun/source/tree/DefaultCaseLabelTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/directive_tree.rs | com/sun/source/tree/DirectiveTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/do_while_loop_tree.rs | com/sun/source/tree/DoWhileLoopTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/empty_statement_tree.rs | com/sun/source/tree/EmptyStatementTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/enhanced_for_loop_tree.rs | com/sun/source/tree/EnhancedForLoopTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/erroneous_tree.rs | com/sun/source/tree/ErroneousTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/exports_tree.rs | com/sun/source/tree/ExportsTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/expression_statement_tree.rs | com/sun/source/tree/ExpressionStatementTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/expression_tree.rs | com/sun/source/tree/ExpressionTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/for_loop_tree.rs | com/sun/source/tree/ForLoopTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/guarded_pattern_tree.rs | com/sun/source/tree/GuardedPatternTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/identifier_tree.rs | com/sun/source/tree/IdentifierTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/if_tree.rs | com/sun/source/tree/IfTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/import_tree.rs | com/sun/source/tree/ImportTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/instance_of_tree.rs | com/sun/source/tree/InstanceOfTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/intersection_type_tree.rs | com/sun/source/tree/IntersectionTypeTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/labeled_statement_tree.rs | com/sun/source/tree/LabeledStatementTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/lambda_expression_tree.rs | com/sun/source/tree/LambdaExpressionTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/line_map.rs | com/sun/source/tree/LineMap.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/literal_tree.rs | com/sun/source/tree/LiteralTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/member_reference_tree.rs | com/sun/source/tree/MemberReferenceTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/member_select_tree.rs | com/sun/source/tree/MemberSelectTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/method_invocation_tree.rs | com/sun/source/tree/MethodInvocationTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/method_tree.rs | com/sun/source/tree/MethodTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/mod.rs | com/sun/source/tree/package-info.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/modifiers_tree.rs | com/sun/source/tree/ModifiersTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/module_tree.rs | com/sun/source/tree/ModuleTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/new_array_tree.rs | com/sun/source/tree/NewArrayTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/new_class_tree.rs | com/sun/source/tree/NewClassTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/opens_tree.rs | com/sun/source/tree/OpensTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/package_tree.rs | com/sun/source/tree/PackageTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/parameterized_type_tree.rs | com/sun/source/tree/ParameterizedTypeTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/parenthesized_pattern_tree.rs | com/sun/source/tree/ParenthesizedPatternTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/parenthesized_tree.rs | com/sun/source/tree/ParenthesizedTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/pattern_tree.rs | com/sun/source/tree/PatternTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/primitive_type_tree.rs | com/sun/source/tree/PrimitiveTypeTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/provides_tree.rs | com/sun/source/tree/ProvidesTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/requires_tree.rs | com/sun/source/tree/RequiresTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/return_tree.rs | com/sun/source/tree/ReturnTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/scope.rs | com/sun/source/tree/Scope.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/statement_tree.rs | com/sun/source/tree/StatementTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/switch_expression_tree.rs | com/sun/source/tree/SwitchExpressionTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/switch_tree.rs | com/sun/source/tree/SwitchTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/synchronized_tree.rs | com/sun/source/tree/SynchronizedTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/throw_tree.rs | com/sun/source/tree/ThrowTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/tree.rs | com/sun/source/tree/Tree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/tree_visitor.rs | com/sun/source/tree/TreeVisitor.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/try_tree.rs | com/sun/source/tree/TryTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/type_cast_tree.rs | com/sun/source/tree/TypeCastTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/type_parameter_tree.rs | com/sun/source/tree/TypeParameterTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/unary_tree.rs | com/sun/source/tree/UnaryTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/union_type_tree.rs | com/sun/source/tree/UnionTypeTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/uses_tree.rs | com/sun/source/tree/UsesTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/variable_tree.rs | com/sun/source/tree/VariableTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/while_loop_tree.rs | com/sun/source/tree/WhileLoopTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/wildcard_tree.rs | com/sun/source/tree/WildcardTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/tree/yield_tree.rs | com/sun/source/tree/YieldTree.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/doc_source_positions.rs | com/sun/source/util/DocSourcePositions.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/doc_tree_factory.rs | com/sun/source/util/DocTreeFactory.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/doc_tree_path.rs | com/sun/source/util/DocTreePath.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/doc_tree_scanner.rs | com/sun/source/util/DocTreeScanner.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/doc_trees.rs | com/sun/source/util/DocTrees.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/javac_task.rs | com/sun/source/util/JavacTask.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/mod.rs | com/sun/source/util/package-info.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/parameter_name_provider.rs | com/sun/source/util/ParameterNameProvider.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/plugin.rs | com/sun/source/util/Plugin.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/simple_doc_tree_visitor.rs | com/sun/source/util/SimpleDocTreeVisitor.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/simple_tree_visitor.rs | com/sun/source/util/SimpleTreeVisitor.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/source_positions.rs | com/sun/source/util/SourcePositions.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/task_event.rs | com/sun/source/util/TaskEvent.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/task_listener.rs | com/sun/source/util/TaskListener.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/tree_path.rs | com/sun/source/util/TreePath.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/tree_path_scanner.rs | com/sun/source/util/TreePathScanner.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/tree_scanner.rs | com/sun/source/util/TreeScanner.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-source/src/util/trees.rs | com/sun/source/util/Trees.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/mod.rs | javax/lang (package) @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/annotated_construct.rs | javax/lang/model/AnnotatedConstruct.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/annotation_mirror.rs | javax/lang/model/element/AnnotationMirror.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/annotation_value.rs | javax/lang/model/element/AnnotationValue.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/annotation_value_visitor.rs | javax/lang/model/element/AnnotationValueVisitor.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/element.rs | javax/lang/model/element/Element.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/element_kind.rs | javax/lang/model/element/ElementKind.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/element_visitor.rs | javax/lang/model/element/ElementVisitor.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/executable_element.rs | javax/lang/model/element/ExecutableElement.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/mod.rs | javax/lang/model/element/package-info.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/modifier.rs | javax/lang/model/element/Modifier.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/module_element.rs | javax/lang/model/element/ModuleElement.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/name.rs | javax/lang/model/element/Name.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/nesting_kind.rs | javax/lang/model/element/NestingKind.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/package_element.rs | javax/lang/model/element/PackageElement.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/parameterizable.rs | javax/lang/model/element/Parameterizable.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/qualified_nameable.rs | javax/lang/model/element/QualifiedNameable.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/record_component_element.rs | javax/lang/model/element/RecordComponentElement.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/type_element.rs | javax/lang/model/element/TypeElement.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/type_parameter_element.rs | javax/lang/model/element/TypeParameterElement.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/unknown_annotation_value_exception.rs | javax/lang/model/element/UnknownAnnotationValueException.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/unknown_directive_exception.rs | javax/lang/model/element/UnknownDirectiveException.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/unknown_element_exception.rs | javax/lang/model/element/UnknownElementException.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/element/variable_element.rs | javax/lang/model/element/VariableElement.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/mod.rs | javax/lang/model/package-info.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/source_version.rs | javax/lang/model/SourceVersion.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/array_type.rs | javax/lang/model/type/ArrayType.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/declared_type.rs | javax/lang/model/type/DeclaredType.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/error_type.rs | javax/lang/model/type/ErrorType.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/executable_type.rs | javax/lang/model/type/ExecutableType.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/intersection_type.rs | javax/lang/model/type/IntersectionType.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/mirrored_type_exception.rs | javax/lang/model/type/MirroredTypeException.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/mirrored_types_exception.rs | javax/lang/model/type/MirroredTypesException.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/mod.rs | javax/lang/model/type/package-info.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/no_type.rs | javax/lang/model/type/NoType.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/null_type.rs | javax/lang/model/type/NullType.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/primitive_type.rs | javax/lang/model/type/PrimitiveType.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/reference_type.rs | javax/lang/model/type/ReferenceType.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/type_kind.rs | javax/lang/model/type/TypeKind.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/type_mirror.rs | javax/lang/model/type/TypeMirror.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/type_variable.rs | javax/lang/model/type/TypeVariable.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/type_visitor.rs | javax/lang/model/type/TypeVisitor.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/union_type.rs | javax/lang/model/type/UnionType.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/unknown_type_exception.rs | javax/lang/model/type/UnknownTypeException.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/type_/wildcard_type.rs | javax/lang/model/type/WildcardType.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/unknown_entity_exception.rs | javax/lang/model/UnknownEntityException.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/abstract_annotation_value_visitor14.rs | javax/lang/model/util/AbstractAnnotationValueVisitor14.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/abstract_annotation_value_visitor6.rs | javax/lang/model/util/AbstractAnnotationValueVisitor6.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/abstract_annotation_value_visitor7.rs | javax/lang/model/util/AbstractAnnotationValueVisitor7.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/abstract_annotation_value_visitor8.rs | javax/lang/model/util/AbstractAnnotationValueVisitor8.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/abstract_annotation_value_visitor9.rs | javax/lang/model/util/AbstractAnnotationValueVisitor9.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/abstract_element_visitor14.rs | javax/lang/model/util/AbstractElementVisitor14.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/abstract_element_visitor6.rs | javax/lang/model/util/AbstractElementVisitor6.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/abstract_element_visitor7.rs | javax/lang/model/util/AbstractElementVisitor7.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/abstract_element_visitor8.rs | javax/lang/model/util/AbstractElementVisitor8.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/abstract_element_visitor9.rs | javax/lang/model/util/AbstractElementVisitor9.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/abstract_type_visitor14.rs | javax/lang/model/util/AbstractTypeVisitor14.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/abstract_type_visitor6.rs | javax/lang/model/util/AbstractTypeVisitor6.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/abstract_type_visitor7.rs | javax/lang/model/util/AbstractTypeVisitor7.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/abstract_type_visitor8.rs | javax/lang/model/util/AbstractTypeVisitor8.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/abstract_type_visitor9.rs | javax/lang/model/util/AbstractTypeVisitor9.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/element_filter.rs | javax/lang/model/util/ElementFilter.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/element_kind_visitor14.rs | javax/lang/model/util/ElementKindVisitor14.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/element_kind_visitor6.rs | javax/lang/model/util/ElementKindVisitor6.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/element_kind_visitor7.rs | javax/lang/model/util/ElementKindVisitor7.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/element_kind_visitor8.rs | javax/lang/model/util/ElementKindVisitor8.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/element_kind_visitor9.rs | javax/lang/model/util/ElementKindVisitor9.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/element_scanner14.rs | javax/lang/model/util/ElementScanner14.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/element_scanner6.rs | javax/lang/model/util/ElementScanner6.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/element_scanner7.rs | javax/lang/model/util/ElementScanner7.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/element_scanner8.rs | javax/lang/model/util/ElementScanner8.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/element_scanner9.rs | javax/lang/model/util/ElementScanner9.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/elements.rs | javax/lang/model/util/Elements.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/mod.rs | javax/lang/model/util/package-info.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/simple_annotation_value_visitor14.rs | javax/lang/model/util/SimpleAnnotationValueVisitor14.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/simple_annotation_value_visitor6.rs | javax/lang/model/util/SimpleAnnotationValueVisitor6.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/simple_annotation_value_visitor7.rs | javax/lang/model/util/SimpleAnnotationValueVisitor7.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/simple_annotation_value_visitor8.rs | javax/lang/model/util/SimpleAnnotationValueVisitor8.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/simple_annotation_value_visitor9.rs | javax/lang/model/util/SimpleAnnotationValueVisitor9.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/simple_element_visitor14.rs | javax/lang/model/util/SimpleElementVisitor14.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/simple_element_visitor6.rs | javax/lang/model/util/SimpleElementVisitor6.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/simple_element_visitor7.rs | javax/lang/model/util/SimpleElementVisitor7.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/simple_element_visitor8.rs | javax/lang/model/util/SimpleElementVisitor8.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/simple_element_visitor9.rs | javax/lang/model/util/SimpleElementVisitor9.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/simple_type_visitor14.rs | javax/lang/model/util/SimpleTypeVisitor14.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/simple_type_visitor6.rs | javax/lang/model/util/SimpleTypeVisitor6.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/simple_type_visitor7.rs | javax/lang/model/util/SimpleTypeVisitor7.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/simple_type_visitor8.rs | javax/lang/model/util/SimpleTypeVisitor8.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/simple_type_visitor9.rs | javax/lang/model/util/SimpleTypeVisitor9.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/type_kind_visitor14.rs | javax/lang/model/util/TypeKindVisitor14.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/type_kind_visitor6.rs | javax/lang/model/util/TypeKindVisitor6.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/type_kind_visitor7.rs | javax/lang/model/util/TypeKindVisitor7.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/type_kind_visitor8.rs | javax/lang/model/util/TypeKindVisitor8.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/type_kind_visitor9.rs | javax/lang/model/util/TypeKindVisitor9.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lang/model/util/types.rs | javax/lang/model/util/Types.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/lib.rs | java.compiler/share/classes/javax/tools (package) @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/diagnostic.rs | javax/tools/Diagnostic.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/diagnostic_collector.rs | javax/tools/DiagnosticCollector.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/diagnostic_listener.rs | javax/tools/DiagnosticListener.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/documentation_tool.rs | javax/tools/DocumentationTool.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/file_object.rs | javax/tools/FileObject.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/forwarding_file_object.rs | javax/tools/ForwardingFileObject.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/forwarding_java_file_manager.rs | javax/tools/ForwardingJavaFileManager.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/forwarding_java_file_object.rs | javax/tools/ForwardingJavaFileObject.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/java_compiler.rs | javax/tools/JavaCompiler.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/java_file_manager.rs | javax/tools/JavaFileManager.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/java_file_object.rs | javax/tools/JavaFileObject.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/mod.rs | javax/tools/package-info.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/option_checker.rs | javax/tools/OptionChecker.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/simple_java_file_object.rs | javax/tools/SimpleJavaFileObject.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/standard_java_file_manager.rs | javax/tools/StandardJavaFileManager.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/standard_location.rs | javax/tools/StandardLocation.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/tool.rs | javax/tools/Tool.java @ 982d53c (jdk-17.0.19-ga) |
| crates/javac-tools/src/tools/tool_provider.rs | javax/tools/ToolProvider.java @ 982d53c (jdk-17.0.19-ga) |
