// Operators/ConstFold oracle test (zero JVM at test time).
//
// The data file is produced by tools/gen-operators-oracle/GenOperatorsOracle.java
// against the baseline JDK 17.0.19. Each BIN/UN row is replayed against the
// ported Operators; each FOLD row against the ported ConstFold. The upstream
// boolean constant is Integer(0/1) (ConstFold.b2i) while the port has a Boolean
// variant, so the comparison maps Boolean -> 1/0 (§9 adapter registered).

use javac_core::code::kinds::Kind;
use javac_core::code::symbol::{SymbolKind};
use javac_core::code::source::Source;
use javac_core::code::symtab::Symtab;
use javac_core::comp::const_fold::ConstFold;
use javac_core::comp::operators::Operators;
use javac_core::compiler_context::CompilerContext;
use javac_core::ids::{SymbolId, TypeId};
use javac_core::jvm::byte_codes;
use javac_core::jvm::target::Target;
use javac_core::tree::jc_tree::Tag;
use javac_core::util::constants::ConstantValue;
use javac_core::util::context::Context;
use javac_core::util::javac_messages::JavacMessages;
use javac_core::util::list::List;
use javac_core::util::log::Log;
use javac_core::util::names::Names;
use javac_core::util::jc_diagnostic::Factory;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

fn oracle_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/operators-oracle.txt")
}

struct Harness {
    ctx: CompilerContext,
    syms: Symtab,
    operators: Operators,
    types: javac_core::code::types::Types,
    _names: Rc<Names>,
    _log: Rc<RefCell<Log>>,
}

fn harness() -> Harness {
    let mut ctx = CompilerContext::new();
    let names = Rc::new(Names::new(false));
    let syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, Source::JDK17);
    let mut jcontext = Context::new();
    let messages = JavacMessages::instance(&mut jcontext);
    let diags = Rc::new(Factory::new(messages, "compiler"));
    let log = Log::instance(&mut jcontext);
    let types = javac_core::code::types::Types::with_names(Rc::clone(&names));
    let operators = Operators::new(
        &mut ctx,
        &syms,
        Rc::clone(&names),
        Rc::clone(&log),
        Rc::clone(&diags),
    );
    Harness {
        ctx,
        syms,
        operators,
        types,
        _names: names,
        _log: log,
    }
}

fn type_by_name(h: &Harness, name: &str) -> TypeId {
    match name {
        "byte" => h.syms.byte_type,
        "short" => h.syms.short_type,
        "int" => h.syms.int_type,
        "long" => h.syms.long_type,
        "float" => h.syms.float_type,
        "double" => h.syms.double_type,
        "char" => h.syms.char_type,
        "boolean" => h.syms.boolean_type,
        "object" => h.syms.object_type,
        "string" => h.syms.string_type,
        "bot" => h.syms.bot_type,
        other => panic!("unknown oracle type {other}"),
    }
}

fn tag_by_name(name: &str) -> Tag {
    match name {
        "PLUS" => Tag::PLUS,
        "MINUS" => Tag::MINUS,
        "SL" => Tag::SL,
        "SR" => Tag::SR,
        "USR" => Tag::USR,
        "EQ" => Tag::EQ,
        "LT" => Tag::LT,
        "BITAND" => Tag::BITAND,
        "AND" => Tag::AND,
        "OR" => Tag::OR,
        "POS" => Tag::POS,
        "NEG" => Tag::NEG,
        "COMPL" => Tag::COMPL,
        "NOT" => Tag::NOT,
        "POSTINC" => Tag::POSTINC,
        other => panic!("unknown oracle tag {other}"),
    }
}

fn type_tag_name(ctx: &CompilerContext, ty: TypeId) -> String {
    format!("{:?}", ctx.typ(ty).get_tag()).to_lowercase()
}

fn operator_opcode(ctx: &CompilerContext, sym: SymbolId) -> i32 {
    match &ctx.sym(sym).kind_data {
        SymbolKind::OperatorSymbol { opcode, .. } => *opcode,
        _ => panic!("expected an operator symbol"),
    }
}

fn operator_return(ctx: &CompilerContext, sym: SymbolId) -> TypeId {
    let ty = ctx.sym(sym).ty.expect("operator type");
    match &ctx.typ(ty).kind {
        javac_core::code::type_::TypeKindData::MethodType { restype, .. } => {
            restype.expect("operator return type")
        }
        _ => panic!("expected a method type"),
    }
}

fn constant_display(ctx: &CompilerContext, ty: TypeId) -> String {
    match ctx.typ(ty).const_value() {
        None => "null".to_string(),
        Some(ConstantValue::Integer(v)) => v.to_string(),
        Some(ConstantValue::Long(v)) => v.to_string(),
        Some(ConstantValue::Float(v)) => v.to_string(),
        Some(ConstantValue::Double(v)) => v.to_string(),
        Some(ConstantValue::Boolean(v)) => {
            if *v {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        Some(ConstantValue::String(v)) => v.to_string(),
        Some(other) => format!("{other:?}"),
    }
}

#[test]
fn operators_oracle_matches_the_port() {
    let data = std::fs::read_to_string(oracle_path()).expect("operators oracle data");
    let mut h = harness();
    let mut checked = 0usize;
    let mut skipped_noop = 0usize;
    for line in data.lines() {
        let parts: Vec<&str> = line.split(' ').collect();
        match parts.first().copied() {
            Some("BIN") => {
                let noop = parts.last() == Some(&"NOOP");
                assert_eq!(parts.len(), if noop { 5 } else { 6 }, "BIN row: {line}");
                let tag = tag_by_name(parts[1]);
                let t1 = type_by_name(&h, parts[2]);
                let t2 = type_by_name(&h, parts[3]);
                let actual = h.operators.resolve_binary(
                    &mut h.ctx,
                    &mut h.syms,
                    &h.types,
                    -1,
                    tag,
                    t1,
                    t2,
                );
                if noop {
                    if actual != h.operators.no_op_symbol {
                        eprintln!("NOOP mismatch row={line} actual_opcode={}", operator_opcode(&h.ctx, actual));
                    }
                    assert_eq!(actual, h.operators.no_op_symbol, "BIN row: {line}");
                    skipped_noop += 1;
                } else {
                    assert_ne!(actual, h.operators.no_op_symbol, "BIN row: {line}");
                    assert_eq!(operator_opcode(&h.ctx, actual), parts[4].parse::<i32>().unwrap(), "BIN row: {line}");
                    let ret = operator_return(&h.ctx, actual);
                    assert_eq!(type_tag_name(&h.ctx, ret), parts[5], "BIN row: {line}");
                    checked += 1;
                }
            }
            Some("UN") => {
                let noop = parts.last() == Some(&"NOOP");
                assert_eq!(parts.len(), if noop { 4 } else { 5 }, "UN row: {line}");
                let tag = tag_by_name(parts[1]);
                let t = type_by_name(&h, parts[2]);
                let actual = h.operators.resolve_unary(
                    &mut h.ctx,
                    &mut h.syms,
                    &h.types,
                    -1,
                    tag,
                    t,
                );
                if noop {
                    assert_eq!(actual, h.operators.no_op_symbol, "UN row: {line}");
                    skipped_noop += 1;
                } else {
                    assert_ne!(actual, h.operators.no_op_symbol, "UN row: {line}");
                    assert_eq!(operator_opcode(&h.ctx, actual), parts[3].parse::<i32>().unwrap(), "UN row: {line}");
                    let ret = operator_return(&h.ctx, actual);
                    assert_eq!(type_tag_name(&h.ctx, ret), parts[4], "UN row: {line}");
                    checked += 1;
                }
            }
            Some("FOLD") => {
                assert_eq!(parts.len(), 6, "FOLD row: {line}");
                let opcode: i32 = parts[1].parse().unwrap();
                let cf = ConstFold::new();
                let lhs: TypeId;
                let rhs: TypeId;
                match parts[2] {
                    "int" => {
                        lhs = h.ctx.type_const_type(h.syms.int_type, ConstantValue::Integer(parts[3].parse().unwrap()));
                        rhs = h.ctx.type_const_type(h.syms.int_type, ConstantValue::Integer(parts[4].parse().unwrap()));
                    }
                    "string" => {
                        lhs = h.ctx.type_const_type(h.syms.string_type, ConstantValue::String(javac_strings::JavaString::from(parts[3])));
                        rhs = h.ctx.type_const_type(h.syms.string_type, ConstantValue::String(javac_strings::JavaString::from(parts[4])));
                    }
                    other => panic!("unknown FOLD kind {other}"),
                }
                let result = cf.fold2(&mut h.ctx, &h.syms, opcode, lhs, rhs);
                let actual = match result {
                    None => "null".to_string(),
                    Some(ty) => constant_display(&h.ctx, ty),
                };
                assert_eq!(actual, parts[5], "FOLD row: {line}");
                checked += 1;
            }
            _ => {}
        }
    }
    assert!(checked >= 25, "checked rows: {checked}");
    eprintln!("operators oracle rows: {checked} checked, {skipped_noop} NOOP rows");
    let _ = (Kind::MTH, List::<TypeId>::nil(), byte_codes::IADD);
}
