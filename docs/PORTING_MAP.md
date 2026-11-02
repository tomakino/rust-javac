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
