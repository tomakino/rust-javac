// Origin: com/sun/tools/javac/jvm/Code.java @ 982d53c (jdk-17.0.19-ga)
// PORTING_RULES §19 gap registration (method-level, extracted from the reference file):
// Dependency/phase: P6 code-generation batch (Code/StackMap/CDS/emit*).
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: checkLimits (Code.java:111)
// - NOT PORTED: arraycode (Code.java:299)
// - NOT PORTED: curCP (Code.java:322)
// - NOT PORTED: emit1 (Code.java:341)
// - NOT PORTED: emit2 (Code.java:349)
// - NOT PORTED: emit4 (Code.java:362)
// - NOT PORTED: emitop (Code.java:379)
// - NOT PORTED: emitLdc (Code.java:402)
// - NOT PORTED: emitMultianewarray (Code.java:414)
// - NOT PORTED: emitNewarray (Code.java:425)
// - NOT PORTED: emitAnewarray (Code.java:435)
// - NOT PORTED: emitInvokeinterface (Code.java:445)
// - NOT PORTED: emitInvokespecial (Code.java:458)
// - NOT PORTED: emitInvokestatic (Code.java:472)
// - NOT PORTED: emitInvokevirtual (Code.java:483)
// - NOT PORTED: emitInvokedynamic (Code.java:494)
// - NOT PORTED: emitop0 (Code.java:506)
// - NOT PORTED: emitop1 (Code.java:908)
// - NOT PORTED: emitop1 (Code.java:912)
// - NOT PORTED: emitop1w (Code.java:932)
// - NOT PORTED: emitop1w (Code.java:979)
// - NOT PORTED: emitop2 (Code.java:1001)
// - NOT PORTED: emitop2 (Code.java:1006)
// - NOT PORTED: emitop2 (Code.java:1010)
// - NOT PORTED: emitop4 (Code.java:1085)
// - NOT PORTED: align (Code.java:1103)
// - NOT PORTED: put1 (Code.java:1111)
// - NOT PORTED: put2 (Code.java:1118)
// - NOT PORTED: put4 (Code.java:1127)
// - NOT PORTED: get1 (Code.java:1137)
// - NOT PORTED: get2 (Code.java:1143)
// - NOT PORTED: get4 (Code.java:1149)
// - NOT PORTED: isAlive (Code.java:1160)
// - NOT PORTED: markDead (Code.java:1166)
// - NOT PORTED: entryPoint (Code.java:1172)
// - NOT PORTED: entryPoint (Code.java:1182)
// - NOT PORTED: entryPoint (Code.java:1197)
// - NOT PORTED: setLetExprStackPos (Code.java:1210)
// - NOT PORTED: isStatementStart (Code.java:1216)
// - NOT PORTED: emitStackMap (Code.java:1248)
// - NOT PORTED: getLocalsSize (Code.java:1268)
// - NOT PORTED: negate (Code.java:1430)
// - NOT PORTED: emitJump (Code.java:1439)
// - NOT PORTED: branch (Code.java:1459)
// - NOT PORTED: resolve (Code.java:1477)
// - NOT PORTED: resolve (Code.java:1540)
// - NOT PORTED: resolvePending (Code.java:1551)
// - NOT PORTED: mergeChains (Code.java:1559)
// - NOT PORTED: addCatch (Code.java:1584)
// - NOT PORTED: compressCatchTable (Code.java:1590)
// - NOT PORTED: addLineNumber (Code.java:1617)
// - NOT PORTED: statBegin (Code.java:1628)
// - NOT PORTED: markStatBegin (Code.java:1636)
// - NOT PORTED: toString (Code.java:1905)
// - NOT PORTED: dup (Code.java:1918)
// - NOT PORTED: toString (Code.java:1938)
// - NOT PORTED: openRange (Code.java:1952)
// - NOT PORTED: closeRange (Code.java:1958)
// - NOT PORTED: hasOpenRange (Code.java:1971)
// - NOT PORTED: isLastRangeInitialized (Code.java:1978)
// - NOT PORTED: getWidestRange (Code.java:1985)
// - NOT PORTED: addLocalVar (Code.java:2002)
// - NOT PORTED: getLVTSize (Code.java:2028)
// - NOT PORTED: setDefined (Code.java:2038)
// - NOT PORTED: setDefined (Code.java:2055)
// - NOT PORTED: setUndefined (Code.java:2068)
// - NOT PORTED: endScope (Code.java:2087)
// - NOT PORTED: fillLocalVarPosition (Code.java:2108)
// - NOT PORTED: appendArray (Code.java:2126)
// - NOT PORTED: fillExceptionParameterPositions (Code.java:2139)
// - NOT PORTED: findExceptionIndex (Code.java:2160)
// - NOT PORTED: newLocal (Code.java:2205)
// - NOT PORTED: newLocal (Code.java:2213)
// - NOT PORTED: newLocal (Code.java:2217)
// - NOT PORTED: newRegSegment (Code.java:2225)
// - NOT PORTED: endScopes (Code.java:2231)
// - NOT PORTED: mnem (Code.java:2241)
// Ported here: typecode(TypeTag/Type), truncate, width(int/Type/List<Type>).
// End of §19 gap registration.

//! PORTING_RULES batch status (P2 ClassReader batch): only the pure static
//! helpers needed by ClassReader.readMethod (typecode/truncate/width) are
//! ported 1:1. The code emitter, StackMapTable/CDS support and the
//! Code/StackMap/CDS inner classes are NOT ported yet (P6 code-generation
//! batch) and are registered in the §19 gap block below; no substitute code
//! is provided.

use crate::code::type_tag::TypeTag;
use crate::compiler_context::CompilerContext;
use crate::ids::TypeId;
use crate::jvm::byte_codes;


/// Java: public static int typecode(TypeTag tag)
pub fn typecode_tag(tag: TypeTag) -> i32 {
    match tag {
        TypeTag::BYTE => byte_codes::BYTE_CODE,
        TypeTag::SHORT => byte_codes::SHORT_CODE,
        TypeTag::CHAR => byte_codes::CHAR_CODE,
        TypeTag::INT => byte_codes::INT_CODE,
        TypeTag::LONG => byte_codes::LONG_CODE,
        TypeTag::FLOAT => byte_codes::FLOAT_CODE,
        TypeTag::DOUBLE => byte_codes::DOUBLE_CODE,
        TypeTag::BOOLEAN => byte_codes::BYTE_CODE,
        TypeTag::VOID => byte_codes::VOID_CODE,
        TypeTag::CLASS
        | TypeTag::ARRAY
        | TypeTag::METHOD
        | TypeTag::BOT
        | TypeTag::TYPEVAR
        | TypeTag::UNINITIALIZED_THIS
        | TypeTag::UNINITIALIZED_OBJECT => byte_codes::OBJECT_CODE,
        other => panic!("typecode {other:?}"),
    }
}

/// Java: public static int typecode(Type type)
pub fn typecode(ctx: &CompilerContext, ty: TypeId) -> i32 {
    typecode_tag(ctx.typ(ty).get_tag())
}

/// Java: public static int truncate(int tc)
pub fn truncate(tc: i32) -> i32 {
    match tc {
        byte_codes::BYTE_CODE | byte_codes::SHORT_CODE | byte_codes::CHAR_CODE => {
            byte_codes::INT_CODE
        }
        other => other,
    }
}

/// Java: public static int width(int typecode)
pub fn width_typecode(typecode: i32) -> i32 {
    match typecode {
        byte_codes::LONG_CODE | byte_codes::DOUBLE_CODE => 2,
        byte_codes::VOID_CODE => 0,
        _ => 1,
    }
}

/// Java: public static int width(Type type) — the Java null Type maps to
/// Option::None (width 1).
pub fn width_type(ctx: &CompilerContext, ty: Option<TypeId>) -> i32 {
    match ty {
        None => 1,
        Some(ty) => width_typecode(typecode(ctx, ty)),
    }
}

/// Java: public static int width(List<Type> types)
pub fn width_list(ctx: &CompilerContext, types: crate::util::list::List<TypeId>) -> i32 {
    let mut w = 0;
    let mut current = types;
    while current.non_empty() {
        w += width_type(ctx, Some(*current.head()));
        current = current.tail();
    }
    w
}
