// Origin: com/sun/tools/javac/comp/ArgumentAttr.java @ 982d53c (jdk-17.0.19-ga)
//
// PORTING_RULES §19 mapping (batch 5): this file ports
// - ArgumentAttr (instance/constructor/setResult/checkSpeculative/
//   withLocalCacheContext/LocalCacheContext/attribArg/visitTree/processArg/
//   visitParens/visitConditional/visitSwitchExpression/visitReference/
//   visitLambda/visitApply/visitNewClass),
// - the ArgumentType base class and its six concrete subclasses
//   (ParensType/ConditionalType/SwitchExpressionType/ExplicitLambdaType/
//   ResolvedMethodType/ResolvedConstructorType),
// - UniquePos.
// §9 registered adapters:
// - Java class inheritance (ArgumentType extends DeferredType, and the six
//   subclasses) is expressed by the ArgumentTypeData payload carried by
//   TypeKindData::Deferred plus kind dispatch in this file; the overridden
//   DeferredType methods (complete/speculativeType/speculativeTree) dispatch on
//   that payload in deferred_attr.rs;
// - Java Map<ResultInfo,Type> object identity is keyed by ResultInfo.id
//   (attr.rs) in a Vec of (id, TypeId) pairs, preserving insertion order;
// - Java Optional fields are Option fields; Java HashMap/LinkedHashMap caches
//   are Vec/RefCell maps preserving the port cache policy (entries are removed
//   on completion, see LocalCacheContext);
// - the Java visitor dispatch becomes a match in ArgumentAttr::visit (the
//   ported tree model has no visitor base class of its own);
// - explicit ctx/syms/env parameters follow ADR 0001;
// - UniquePos identity uses (pos, DiagnosticSource identity); the port keys it
//   by the source file identity (Rc::as_ptr of the JavaFileObject of
//   log.current_source) because DiagnosticSource is value-copied in the port.

use crate::ids::TreeId;
use crate::ids::TypeId;
use crate::util::list::List;

/// Java: the six concrete ArgumentAttr.ArgumentType subclasses (ArgumentAttr.java:
/// ParensType 405-425, ConditionalType 430-456, SwitchExpressionType 461-513,
/// ExplicitLambdaType 518-640, ResolvedMemberType/ResolvedMethodType/
/// ResolvedConstructorType 645-735).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgumentKind {
    Parens,
    Conditional,
    SwitchExpression,
    ExplicitLambda,
    ResolvedMethod,
    ResolvedConstructor,
}

/// Java: ArgumentAttr.ArgumentType<T> state (ArgumentAttr.java:641-740), carried
/// by TypeKindData::Deferred::argument.
///
/// The parent DeferredType state (tree/env/mode/notPertinentToApplicability/
/// speculativeCache) stays in the Deferred variant; this payload adds the
/// speculative tree, the ResultInfo-keyed speculative types map and the lazily
/// populated scratch lists of SwitchExpressionType/ExplicitLambdaType.
pub struct ArgumentTypeData {
    /// The concrete subclass.
    pub kind: ArgumentKind,
    /// Java: T speculativeTree
    pub speculative_tree: TreeId,
    /// Java: Map<ResultInfo, Type> speculativeTypes — keyed by ResultInfo.id
    /// (§9); insertion order preserved (upstream HashMap iteration order is
    /// unspecified, the port keeps insertion order for the speculativeType
    /// scan).
    pub speculative_types: Vec<(u64, TypeId)>,
    /// Java: SwitchExpressionType.yieldExpressions (Optional<List<JCYield>>)
    pub yield_expressions: Option<List<TreeId>>,
    /// Java: ExplicitLambdaType.argtypes (Optional<List<Type>>)
    pub argtypes: Option<List<TypeId>>,
    /// Java: ExplicitLambdaType.returnExpressions (Optional<List<JCReturn>>)
    pub return_expressions: Option<List<TreeId>>,
}
