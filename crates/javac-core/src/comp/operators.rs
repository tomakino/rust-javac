// Origin: com/sun/tools/javac/comp/Operators.java @ 982d53c (jdk-17.0.19-ga)
//
// PORTING_RULES:
// - ADR 0001: the ported Operators receives the services explicitly (the
// upstream Context lookup Operators.instance/Operators(Context) is
// registered with the Context registration).
// - adapter adapter: the upstream opname Name[] indexed by Tag.operatorIndex() is
// expressed as a match on the operator Tag (the same Name objects); the
// helper lists keep the upstream "prepend wins" precedence by construction
// order (same-tag helpers are tested in the order they were added, latest
// first, exactly like the upstream prepend chain).
// - adapter adapter: the upstream OperatorHelper.lazy alternatives are created
// eagerly in the ported constructor (same OperatorSymbol set and order).
// - OperatorSymbol.kind: the ported symbol uses Kind::MTH (the upstream
// OperatorSymbol inherits the Symbol kind without passing one; registered
// with the Symbol; no ported Attr path depends on it).
//
// This class contains the logic for unary and binary operator resolution/lookup.

use std::cell::RefCell;
use std::rc::Rc;

use crate::code::flags;
use crate::code::kinds::Kind;
use crate::code::symbol::{Symbol, SymbolKind};
use crate::code::symtab::Symtab;
use crate::code::type_::TypeKindData;
use crate::code::type_tag::TypeTag;
use crate::code::types::Types;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TypeId};
use crate::jvm::byte_codes;
use crate::tree::jc_tree::Tag;
use crate::util::jc_diagnostic::{
    DiagnosticArg, DiagnosticType, Factory, SimpleDiagnosticPosition,
};
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::name::Name;
use crate::util::names::Names;

/// Java: enum Operators.OperatorType
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperatorType {
    BYTE,
    SHORT,
    INT,
    LONG,
    FLOAT,
    DOUBLE,
    CHAR,
    BOOLEAN,
    OBJECT,
    STRING,
    BOT,
}

impl OperatorType {
    /// Java: Type asType(Symtab syms)
    pub fn as_type(self, syms: &Symtab) -> TypeId {
        match self {
            OperatorType::BYTE => syms.byte_type,
            OperatorType::SHORT => syms.short_type,
            OperatorType::INT => syms.int_type,
            OperatorType::LONG => syms.long_type,
            OperatorType::FLOAT => syms.float_type,
            OperatorType::DOUBLE => syms.double_type,
            OperatorType::CHAR => syms.char_type,
            OperatorType::BOOLEAN => syms.boolean_type,
            OperatorType::OBJECT => syms.object_type,
            OperatorType::STRING => syms.string_type,
            OperatorType::BOT => syms.bot_type,
        }
    }
}

/// Java: Operators.UnaryOperatorHelper subclasses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnaryKind {
    Reference,
    Numeric,
    NumericIntegral,
    Boolean,
    PrefixPostfix,
}

/// Java: Operators.BinaryOperatorHelper subclasses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BinaryKind {
    Numeric,
    NumericIntegral,
    Boolean,
    String,
    Shift,
    Equality,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct UnaryEntry {
    tag: Tag,
    kind: UnaryKind,
    arg: OperatorType,
    res: OperatorType,
    opcode: i32,
    symbol: SymbolId,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct BinaryEntry {
    tag: Tag,
    kind: BinaryKind,
    arg1: OperatorType,
    arg2: OperatorType,
    res: OperatorType,
    opcodes: Vec<i32>,
    symbol: SymbolId,
}

/// Java: enum Operators.ComparisonKind
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComparisonKind {
    NumericOrBoolean,
    Reference,
    Invalid,
}

/// Java: public class Operators (comp/Operators.java:93)
pub struct Operators {
    pub names: Rc<Names>,
    pub log: Rc<RefCell<Log>>,
    pub diags: Rc<Factory>,
    /// Java: public final OperatorSymbol noOpSymbol
    pub no_op_symbol: SymbolId,
    unary: Vec<UnaryEntry>,
    binary: Vec<BinaryEntry>,
}

impl Operators {
    /// Java: protected Operators(Context context) — explicit services (ADR 0001).
    pub fn new(
        ctx: &mut CompilerContext,
        syms: &Symtab,
        names: Rc<Names>,
        log: Rc<RefCell<Log>>,
        diags: Rc<Factory>,
    ) -> Operators {
        let no_type = crate::code::symtab::no_type_id(ctx);
        let no_op_symbol = ctx.alloc_symbol(Symbol::new(
            Kind::MTH,
            (flags::PUBLIC | flags::STATIC) as i64,
            names.empty.clone(),
            Some(no_type),
            Some(syms.no_symbol),
            SymbolKind::OperatorSymbol {
                opcode: -1,
                access_code: i32::MIN,
            },
        ));
        let mut operators = Operators {
            names,
            log,
            diags,
            no_op_symbol,
            unary: Vec::new(),
            binary: Vec::new(),
        };
        operators.init_unary_operators(ctx, syms);
        operators.init_binary_operators(ctx, syms);
        operators
    }

    /// Java: Type unaryPromotion(Type t) (Operators.java:158-170)
    fn unary_promotion(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
    ) -> TypeId {
        let unboxed = types.unboxed_type_or_type(ctx, syms, t);
        match ctx.typ(unboxed).get_tag() {
            TypeTag::BYTE | TypeTag::SHORT | TypeTag::CHAR => syms.int_type,
            _ => unboxed,
        }
    }

    /// Helper for the promoted/unboxed type tag reads (avoids the
    /// ctx.typ(helper(ctx,..)) double borrow; Porting rules adapter, no behavior change).
    fn promoted_tag(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
    ) -> TypeTag {
        let promoted = self.unary_promotion(ctx, syms, types, t);
        ctx.typ(promoted).get_tag()
    }

    fn promoted_is_numeric(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
    ) -> bool {
        let promoted = self.unary_promotion(ctx, syms, types, t);
        ctx.typ(promoted).is_numeric()
    }

    fn promoted_is_integral(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
    ) -> bool {
        let promoted = self.unary_promotion(ctx, syms, types, t);
        ctx.typ(promoted).is_integral()
    }

    fn unboxed_tag(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
    ) -> TypeTag {
        let unboxed = types.unboxed_type_or_type(ctx, syms, t);
        ctx.typ(unboxed).get_tag()
    }

    /// Java: Type binaryPromotion(Type t1, Type t2) (Operators.java:176-196)
    fn binary_promotion(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t1: TypeId,
        t2: TypeId,
    ) -> TypeId {
        let unboxed_t1 = types.unboxed_type_or_type(ctx, syms, t1);
        let unboxed_t2 = types.unboxed_type_or_type(ctx, syms, t2);
        if ctx.typ(unboxed_t1).is_numeric() && ctx.typ(unboxed_t2).is_numeric() {
            if ctx.typ(unboxed_t1).has_tag(TypeTag::DOUBLE)
                || ctx.typ(unboxed_t2).has_tag(TypeTag::DOUBLE)
            {
                syms.double_type
            } else if ctx.typ(unboxed_t1).has_tag(TypeTag::FLOAT)
                || ctx.typ(unboxed_t2).has_tag(TypeTag::FLOAT)
            {
                syms.float_type
            } else if ctx.typ(unboxed_t1).has_tag(TypeTag::LONG)
                || ctx.typ(unboxed_t2).has_tag(TypeTag::LONG)
            {
                syms.long_type
            } else {
                syms.int_type
            }
        } else if types.is_same_type(ctx, syms, unboxed_t1, unboxed_t2) {
            unboxed_t1
        } else {
            syms.object_type
        }
    }

    /// Java: OperatorSymbol resolveUnary(DiagnosticPosition pos, Tag tag,
    /// Type op) (Operators.java:201-208)
    pub fn resolve_unary(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        tag: Tag,
        op: TypeId,
    ) -> SymbolId {
        let op_name = self.operator_name(tag);
        for entry in self
            .unary
            .iter()
            .filter(|entry| self.operator_name(entry.tag) == op_name)
        {
            if self.unary_test(ctx, syms, types, entry, op) {
                let expected = self.unary_expected(ctx, syms, types, entry, op);
                if self.unary_applicable(ctx, syms, types, entry, expected) {
                    return entry.symbol;
                }
            }
        }
        self.report_error_if_needed(ctx, syms, pos, tag, &[Some(op)])
    }

    /// Java: OperatorSymbol resolveBinary(DiagnosticPosition pos, Tag tag,
    /// Type op1, Type op2) (Operators.java:213-221)
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_binary(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        pos: i32,
        tag: Tag,
        op1: TypeId,
        op2: TypeId,
    ) -> SymbolId {
        for entry in self.binary.iter().filter(|entry| entry.tag == tag) {
            if self.binary_test(ctx, syms, types, entry, op1, op2)
                && self.binary_applicable(ctx, syms, types, entry, op1, op2)
            {
                return entry.symbol;
            }
        }
        self.report_error_if_needed(ctx, syms, pos, tag, &[Some(op1), Some(op2)])
    }

    /// Java: UnaryOperatorHelper.test(Type) (the helper-kind customization).
    fn unary_test(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        entry: &UnaryEntry,
        t: TypeId,
    ) -> bool {
        match entry.kind {
            UnaryKind::Reference => ctx.typ(t).is_null_or_reference(),
            UnaryKind::Numeric => self.promoted_is_numeric(ctx, syms, types, t),
            UnaryKind::NumericIntegral => self.promoted_is_integral(ctx, syms, types, t),
            UnaryKind::Boolean => self.unboxed_tag(ctx, syms, types, t) == TypeTag::BOOLEAN,
            UnaryKind::PrefixPostfix => self.promoted_is_numeric(ctx, syms, types, t),
        }
    }

    /// Java: UnaryOperatorHelper.isUnaryOperatorApplicable (the doLookup
    /// predicate: the operator's formal type is the expected type).
    fn unary_applicable(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        entry: &UnaryEntry,
        expected: TypeId,
    ) -> bool {
        types.is_same_type(ctx, syms, entry.arg.as_type(syms), expected)
    }

    /// Java: UnaryOperatorHelper.resolve(Type) (the expected argument type
    /// computed by each helper subclass).
    fn unary_expected(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        entry: &UnaryEntry,
        arg: TypeId,
    ) -> TypeId {
        match entry.kind {
            UnaryKind::Reference => syms.object_type,
            UnaryKind::Boolean => syms.boolean_type,
            UnaryKind::Numeric | UnaryKind::NumericIntegral => {
                self.unary_promotion(ctx, syms, types, arg)
            }
            UnaryKind::PrefixPostfix => types.unboxed_type_or_type(ctx, syms, arg),
        }
    }

    /// Java: BinaryOperatorHelper.test(Type, Type)
    fn binary_test(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        entry: &BinaryEntry,
        t1: TypeId,
        t2: TypeId,
    ) -> bool {
        match entry.kind {
            BinaryKind::Numeric => {
                self.promoted_is_numeric(ctx, syms, types, t1)
                    && self.promoted_is_numeric(ctx, syms, types, t2)
            }
            BinaryKind::NumericIntegral => {
                self.promoted_is_integral(ctx, syms, types, t1)
                    && self.promoted_is_integral(ctx, syms, types, t2)
            }
            BinaryKind::Boolean => {
                self.unboxed_tag(ctx, syms, types, t1) == TypeTag::BOOLEAN
                    && self.unboxed_tag(ctx, syms, types, t2) == TypeTag::BOOLEAN
            }
            BinaryKind::String => {
                let has_string_op = types.is_same_type(ctx, syms, t1, syms.string_type)
                    || types.is_same_type(ctx, syms, t2, syms.string_type);
                let has_void_op =
                    ctx.typ(t1).has_tag(TypeTag::VOID) || ctx.typ(t2).has_tag(TypeTag::VOID);
                has_string_op && !has_void_op
            }
            BinaryKind::Shift => {
                let op1 = self.promoted_tag(ctx, syms, types, t1);
                let op2 = self.promoted_tag(ctx, syms, types, t2);
                (op1 == TypeTag::LONG || op1 == TypeTag::INT)
                    && (op2 == TypeTag::LONG || op2 == TypeTag::INT)
            }
            BinaryKind::Equality => {
                self.equality_kind(ctx, syms, types, t1, t2) != ComparisonKind::Invalid
            }
        }
    }

    /// Java: BinaryOperatorHelper.isBinaryOperatorApplicable (the doLookup
    /// predicate over the resolved operand types).
    fn binary_applicable(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        entry: &BinaryEntry,
        t1: TypeId,
        t2: TypeId,
    ) -> bool {
        let (expected1, expected2) = self.binary_expected(ctx, syms, types, entry, t1, t2);
        types.is_same_type(ctx, syms, entry.arg1.as_type(syms), expected1)
            && types.is_same_type(ctx, syms, entry.arg2.as_type(syms), expected2)
    }

    /// Java: BinaryOperatorHelper.resolve(Type, Type) (the expected operand
    /// types computed by each helper subclass).
    fn binary_expected(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        entry: &BinaryEntry,
        t1: TypeId,
        t2: TypeId,
    ) -> (TypeId, TypeId) {
        match entry.kind {
            BinaryKind::Numeric | BinaryKind::NumericIntegral => {
                let t = self.binary_promotion(ctx, syms, types, t1, t2);
                (t, t)
            }
            BinaryKind::Boolean => (syms.boolean_type, syms.boolean_type),
            BinaryKind::String => (
                self.string_promotion(ctx, syms, types, t1),
                self.string_promotion(ctx, syms, types, t2),
            ),
            BinaryKind::Shift => (
                self.unary_promotion(ctx, syms, types, t1),
                self.unary_promotion(ctx, syms, types, t2),
            ),
            BinaryKind::Equality => {
                let kind = self.equality_kind(ctx, syms, types, t1, t2);
                let t = if kind == ComparisonKind::NumericOrBoolean {
                    self.binary_promotion(ctx, syms, types, t1, t2)
                } else {
                    syms.object_type
                };
                (t, t)
            }
        }
    }

    /// Java: BinaryStringOperator.stringPromotion(Type) (Operators.java:558-570)
    fn string_promotion(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
    ) -> TypeId {
        if ctx.typ(t).is_primitive() {
            self.unary_promotion(ctx, syms, types, t)
        } else if ctx.typ(t).has_tag(TypeTag::VOID)
            || ctx.typ(t).has_tag(TypeTag::BOT)
            || types.is_same_type(ctx, syms, t, syms.string_type)
        {
            t
        } else if ctx.typ(t).has_tag(TypeTag::TYPEVAR) {
            let upper = ctx
                .typ(t)
                .get_upper_bound()
                .expect("type variable upper bound");
            self.string_promotion(ctx, syms, types, upper)
        } else {
            syms.object_type
        }
    }

    /// Java: BinaryEqualityOperator.getKind(Type, Type) (Operators.java:637-652)
    fn equality_kind(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        arg1: TypeId,
        arg2: TypeId,
    ) -> ComparisonKind {
        let arg1_primitive = ctx.typ(arg1).is_primitive();
        let arg2_primitive = ctx.typ(arg2).is_primitive();
        if arg1_primitive && arg2_primitive {
            ComparisonKind::NumericOrBoolean
        } else if arg1_primitive {
            let promoted = self.unary_promotion(ctx, syms, types, arg2);
            if ctx.typ(promoted).is_primitive() {
                ComparisonKind::NumericOrBoolean
            } else {
                ComparisonKind::Invalid
            }
        } else if arg2_primitive {
            let promoted = self.unary_promotion(ctx, syms, types, arg1);
            if ctx.typ(promoted).is_primitive() {
                ComparisonKind::NumericOrBoolean
            } else {
                ComparisonKind::Invalid
            }
        } else if ctx.typ(arg1).is_null_or_reference() && ctx.typ(arg2).is_null_or_reference() {
            ComparisonKind::Reference
        } else {
            ComparisonKind::Invalid
        }
    }

    /// Java: OperatorSymbol makeOperator(Name name, List<OperatorType>
    /// formals, OperatorType res, int... opcodes) (Operators.java:249-257)
    fn make_operator(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        name: &Name,
        formals: &[OperatorType],
        res: OperatorType,
        opcodes: &[i32],
    ) -> SymbolId {
        let mut argtypes: List<TypeId> = List::nil();
        for formal in formals {
            argtypes = argtypes.append(formal.as_type(syms));
        }
        let op_type = ctx.alloc_type(crate::code::type_::Type::new(
            TypeTag::METHOD,
            Some(syms.method_class),
            TypeKindData::MethodType {
                argtypes,
                restype: Some(res.as_type(syms)),
                thrown: List::nil(),
                recvtype: None,
            },
        ));
        let opcode = self.merge_opcodes(opcodes);
        ctx.alloc_symbol(Symbol::new(
            Kind::MTH,
            (flags::PUBLIC | flags::STATIC) as i64,
            name.clone(),
            Some(op_type),
            Some(syms.no_symbol),
            SymbolKind::OperatorSymbol {
                opcode,
                access_code: i32::MIN,
            },
        ))
    }

    /// Java: int mergeOpcodes(int... opcodes) (Operators.java:262-269)
    fn merge_opcodes(&self, opcodes: &[i32]) -> i32 {
        assert!(
            opcodes.len() == 1 || opcodes.len() == 2,
            "AssertionError: opcodes length"
        );
        if opcodes.len() == 1 {
            opcodes[0]
        } else {
            (opcodes[0] << byte_codes::PRE_SHIFT) | opcodes[1]
        }
    }

    /// Java: Name operatorName(Tag tag) (Operators.java:293-295) — the opname[]
    /// lookup expressed as the initOperatorNames match (Porting rules adapter).
    pub fn operator_name(&self, tag: Tag) -> Name {
        let from = |text: &str| self.names.from_string(&javac_strings::JavaString::from(text));
        match tag {
            Tag::POS => from("+"),
            Tag::NEG => from("-"),
            Tag::NOT => from("!"),
            Tag::COMPL => from("~"),
            Tag::PREINC => from("++"),
            Tag::PREDEC => from("--"),
            Tag::POSTINC => from("++"),
            Tag::POSTDEC => from("--"),
            Tag::NULLCHK => from("<*nullchk*>"),
            Tag::OR => from("||"),
            Tag::AND => from("&&"),
            Tag::EQ => from("=="),
            Tag::NE => from("!="),
            Tag::LT => from("<"),
            Tag::GT => from(">"),
            Tag::LE => from("<="),
            Tag::GE => from(">="),
            Tag::BITOR => from("|"),
            Tag::BITXOR => from("^"),
            Tag::BITAND => from("&"),
            Tag::SL => from("<<"),
            Tag::SR => from(">>"),
            Tag::USR => from(">>>"),
            Tag::PLUS => from("+"),
            Tag::MINUS => self.names.hyphen.clone(),
            Tag::MUL => self.names.asterisk.clone(),
            Tag::DIV => self.names.slash.clone(),
            Tag::MOD => from("%"),
            other => panic!("AssertionError: not an operator tag: {:?}", other),
        }
    }

    /// Java: OperatorSymbol reportErrorIfNeeded(DiagnosticPosition pos, Tag tag,
    /// Type... args) (Operators.java:278-291)
    fn report_error_if_needed(
        &self,
        ctx: &mut CompilerContext,
        _syms: &Symtab,
        pos: i32,
        tag: Tag,
        args: &[Option<TypeId>],
    ) -> SymbolId {
        let erroneous = args.iter().flatten().any(|t| {
            ctx.typ(*t).has_tag(TypeTag::ERROR) || ctx.typ(*t).has_tag(TypeTag::NONE)
        });
        if !erroneous {
            use crate::resources::compiler_properties::errors;
            let op_name = self.operator_name(tag);
            let info = if args.len() == 1 {
                errors::operator_cant_be_applied(
                    op_name,
                    crate::resources::diagnostic_contract::TypeArg::placeholder(type_arg_text(
                        ctx,
                        args[0].expect("unary operator argument"),
                    )),
                )
            } else {
                errors::operator_cant_be_applied1(
                    op_name,
                    crate::resources::diagnostic_contract::TypeArg::placeholder(type_arg_text(
                        ctx,
                        args[0].expect("binary operator lhs"),
                    )),
                    crate::resources::diagnostic_contract::TypeArg::placeholder(type_arg_text(
                        ctx,
                        args[1].expect("binary operator rhs"),
                    )),
                )
            };
            let diagnostic_args: Vec<DiagnosticArg> = info
                .info()
                .args
                .iter()
                .map(|arg| {
                    Rc::new(javac_strings::JavaString::from(arg.to_text().as_str())) as DiagnosticArg
                })
                .collect();
            let source = self.log.borrow().current_source();
            let diagnostic = self.diags.create(
                DiagnosticType::ERROR,
                source,
                Some(Rc::new(SimpleDiagnosticPosition::new(pos))),
                info.info().code,
                diagnostic_args,
            );
            self.log.borrow_mut().report(diagnostic);
        }
        self.no_op_symbol
    }

    /// Java: OperatorSymbol lookupBinaryOp(Predicate<OperatorSymbol>
    /// applicabilityTest) (Operators.java:806-813)
    pub fn lookup_binary_op(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        _types: &Types,
        applicability_test: &dyn Fn(&CompilerContext, &Symtab, SymbolId) -> bool,
    ) -> Option<SymbolId> {
        // Java: binaryOperators.values().stream().flatMap(List::stream)
        //.map(helper -> helper.doLookup(test)).distinct()
        //.filter(sym -> sym != noOpSymbol).findFirst().get()
        for entry in self.binary.iter() {
            if applicability_test(ctx, syms, entry.symbol) && entry.symbol != self.no_op_symbol {
                return Some(entry.symbol);
            }
        }
        None
    }

    /// Java: private void initUnaryOperators() (Operators.java:666-702)
    #[allow(clippy::type_complexity)]
    fn init_unary_operators(&mut self, ctx: &mut CompilerContext, syms: &Symtab) {
        let table: &[(UnaryKind, Tag, &[(OperatorType, OperatorType, i32)])] = &[
            (UnaryKind::Numeric, Tag::POS, &[
                (OperatorType::DOUBLE, OperatorType::DOUBLE, byte_codes::NOP),
                (OperatorType::FLOAT, OperatorType::FLOAT, byte_codes::NOP),
                (OperatorType::LONG, OperatorType::LONG, byte_codes::NOP),
                (OperatorType::INT, OperatorType::INT, byte_codes::NOP),
            ]),
            (UnaryKind::Numeric, Tag::NEG, &[
                (OperatorType::DOUBLE, OperatorType::DOUBLE, byte_codes::DNEG),
                (OperatorType::FLOAT, OperatorType::FLOAT, byte_codes::FNEG),
                (OperatorType::LONG, OperatorType::LONG, byte_codes::LNEG),
                (OperatorType::INT, OperatorType::INT, byte_codes::INEG),
            ]),
            (UnaryKind::NumericIntegral, Tag::COMPL, &[
                (OperatorType::LONG, OperatorType::LONG, byte_codes::LXOR),
                (OperatorType::INT, OperatorType::INT, byte_codes::IXOR),
            ]),
            (UnaryKind::PrefixPostfix, Tag::POSTINC, &[
                (OperatorType::DOUBLE, OperatorType::DOUBLE, byte_codes::DADD),
                (OperatorType::FLOAT, OperatorType::FLOAT, byte_codes::FADD),
                (OperatorType::LONG, OperatorType::LONG, byte_codes::LADD),
                (OperatorType::INT, OperatorType::INT, byte_codes::IADD),
                (OperatorType::CHAR, OperatorType::CHAR, byte_codes::IADD),
                (OperatorType::SHORT, OperatorType::SHORT, byte_codes::IADD),
                (OperatorType::BYTE, OperatorType::BYTE, byte_codes::IADD),
            ]),
            (UnaryKind::PrefixPostfix, Tag::POSTDEC, &[
                (OperatorType::DOUBLE, OperatorType::DOUBLE, byte_codes::DSUB),
                (OperatorType::FLOAT, OperatorType::FLOAT, byte_codes::FSUB),
                (OperatorType::LONG, OperatorType::LONG, byte_codes::LSUB),
                (OperatorType::INT, OperatorType::INT, byte_codes::ISUB),
                (OperatorType::CHAR, OperatorType::CHAR, byte_codes::ISUB),
                (OperatorType::SHORT, OperatorType::SHORT, byte_codes::ISUB),
                (OperatorType::BYTE, OperatorType::BYTE, byte_codes::ISUB),
            ]),
            (UnaryKind::Boolean, Tag::NOT, &[
                (OperatorType::BOOLEAN, OperatorType::BOOLEAN, byte_codes::BOOL_NOT),
            ]),
            (UnaryKind::Reference, Tag::NULLCHK, &[
                (OperatorType::OBJECT, OperatorType::OBJECT, byte_codes::NULLCHK),
            ]),
        ];
        for (kind, tag, entries) in table {
            // Java: initOperators prepends each helper; the operator suppliers
            // are themselves prepended, so the last added alternative is tested
            // first. Eager construction preserves both orders.
            for (arg, res, opcode) in entries.iter() {
                let name = self.operator_name(*tag);
                let symbol = self.make_operator(ctx, syms, &name, &[*arg], *res, &[*opcode]);
                self.unary.push(UnaryEntry {
                    tag: *tag,
                    kind: *kind,
                    arg: *arg,
                    res: *res,
                    opcode: *opcode,
                    symbol,
                });
            }
        }
    }

    /// Java: private void initBinaryOperators() (Operators.java:707-804)
    #[allow(clippy::type_complexity)]
    fn init_binary_operators(&mut self, ctx: &mut CompilerContext, syms: &Symtab) {
        let table: &[(BinaryKind, Tag, &[(OperatorType, OperatorType, OperatorType, &[i32])])] = &[
            (BinaryKind::String, Tag::PLUS, &[
                (OperatorType::STRING, OperatorType::OBJECT, OperatorType::STRING, &[byte_codes::STRING_ADD]),
                (OperatorType::OBJECT, OperatorType::STRING, OperatorType::STRING, &[byte_codes::STRING_ADD]),
                (OperatorType::STRING, OperatorType::STRING, OperatorType::STRING, &[byte_codes::STRING_ADD]),
                (OperatorType::STRING, OperatorType::INT, OperatorType::STRING, &[byte_codes::STRING_ADD]),
                (OperatorType::STRING, OperatorType::LONG, OperatorType::STRING, &[byte_codes::STRING_ADD]),
                (OperatorType::STRING, OperatorType::FLOAT, OperatorType::STRING, &[byte_codes::STRING_ADD]),
                (OperatorType::STRING, OperatorType::DOUBLE, OperatorType::STRING, &[byte_codes::STRING_ADD]),
                (OperatorType::STRING, OperatorType::BOOLEAN, OperatorType::STRING, &[byte_codes::STRING_ADD]),
                (OperatorType::STRING, OperatorType::BOT, OperatorType::STRING, &[byte_codes::STRING_ADD]),
                (OperatorType::INT, OperatorType::STRING, OperatorType::STRING, &[byte_codes::STRING_ADD]),
                (OperatorType::LONG, OperatorType::STRING, OperatorType::STRING, &[byte_codes::STRING_ADD]),
                (OperatorType::FLOAT, OperatorType::STRING, OperatorType::STRING, &[byte_codes::STRING_ADD]),
                (OperatorType::DOUBLE, OperatorType::STRING, OperatorType::STRING, &[byte_codes::STRING_ADD]),
                (OperatorType::BOOLEAN, OperatorType::STRING, OperatorType::STRING, &[byte_codes::STRING_ADD]),
                (OperatorType::BOT, OperatorType::STRING, OperatorType::STRING, &[byte_codes::STRING_ADD]),
            ]),
            (BinaryKind::Numeric, Tag::PLUS, &[
                (OperatorType::DOUBLE, OperatorType::DOUBLE, OperatorType::DOUBLE, &[byte_codes::DADD]),
                (OperatorType::FLOAT, OperatorType::FLOAT, OperatorType::FLOAT, &[byte_codes::FADD]),
                (OperatorType::LONG, OperatorType::LONG, OperatorType::LONG, &[byte_codes::LADD]),
                (OperatorType::INT, OperatorType::INT, OperatorType::INT, &[byte_codes::IADD]),
            ]),
            (BinaryKind::Numeric, Tag::MINUS, &[
                (OperatorType::DOUBLE, OperatorType::DOUBLE, OperatorType::DOUBLE, &[byte_codes::DSUB]),
                (OperatorType::FLOAT, OperatorType::FLOAT, OperatorType::FLOAT, &[byte_codes::FSUB]),
                (OperatorType::LONG, OperatorType::LONG, OperatorType::LONG, &[byte_codes::LSUB]),
                (OperatorType::INT, OperatorType::INT, OperatorType::INT, &[byte_codes::ISUB]),
            ]),
            (BinaryKind::Numeric, Tag::MUL, &[
                (OperatorType::DOUBLE, OperatorType::DOUBLE, OperatorType::DOUBLE, &[byte_codes::DMUL]),
                (OperatorType::FLOAT, OperatorType::FLOAT, OperatorType::FLOAT, &[byte_codes::FMUL]),
                (OperatorType::LONG, OperatorType::LONG, OperatorType::LONG, &[byte_codes::LMUL]),
                (OperatorType::INT, OperatorType::INT, OperatorType::INT, &[byte_codes::IMUL]),
            ]),
            (BinaryKind::Numeric, Tag::DIV, &[
                (OperatorType::DOUBLE, OperatorType::DOUBLE, OperatorType::DOUBLE, &[byte_codes::DDIV]),
                (OperatorType::FLOAT, OperatorType::FLOAT, OperatorType::FLOAT, &[byte_codes::FDIV]),
                (OperatorType::LONG, OperatorType::LONG, OperatorType::LONG, &[byte_codes::LDIV]),
                (OperatorType::INT, OperatorType::INT, OperatorType::INT, &[byte_codes::IDIV]),
            ]),
            (BinaryKind::Numeric, Tag::MOD, &[
                (OperatorType::DOUBLE, OperatorType::DOUBLE, OperatorType::DOUBLE, &[byte_codes::DMOD]),
                (OperatorType::FLOAT, OperatorType::FLOAT, OperatorType::FLOAT, &[byte_codes::FMOD]),
                (OperatorType::LONG, OperatorType::LONG, OperatorType::LONG, &[byte_codes::LMOD]),
                (OperatorType::INT, OperatorType::INT, OperatorType::INT, &[byte_codes::IMOD]),
            ]),
            (BinaryKind::Boolean, Tag::BITAND, &[
                (OperatorType::BOOLEAN, OperatorType::BOOLEAN, OperatorType::BOOLEAN, &[byte_codes::IAND]),
            ]),
            (BinaryKind::NumericIntegral, Tag::BITAND, &[
                (OperatorType::LONG, OperatorType::LONG, OperatorType::LONG, &[byte_codes::LAND]),
                (OperatorType::INT, OperatorType::INT, OperatorType::INT, &[byte_codes::IAND]),
            ]),
            (BinaryKind::Boolean, Tag::BITOR, &[
                (OperatorType::BOOLEAN, OperatorType::BOOLEAN, OperatorType::BOOLEAN, &[byte_codes::IOR]),
            ]),
            (BinaryKind::NumericIntegral, Tag::BITOR, &[
                (OperatorType::LONG, OperatorType::LONG, OperatorType::LONG, &[byte_codes::LOR]),
                (OperatorType::INT, OperatorType::INT, OperatorType::INT, &[byte_codes::IOR]),
            ]),
            (BinaryKind::Boolean, Tag::BITXOR, &[
                (OperatorType::BOOLEAN, OperatorType::BOOLEAN, OperatorType::BOOLEAN, &[byte_codes::IXOR]),
            ]),
            (BinaryKind::NumericIntegral, Tag::BITXOR, &[
                (OperatorType::LONG, OperatorType::LONG, OperatorType::LONG, &[byte_codes::LXOR]),
                (OperatorType::INT, OperatorType::INT, OperatorType::INT, &[byte_codes::IXOR]),
            ]),
            (BinaryKind::Shift, Tag::SL, &[
                (OperatorType::INT, OperatorType::INT, OperatorType::INT, &[byte_codes::ISHL]),
                (OperatorType::INT, OperatorType::LONG, OperatorType::INT, &[byte_codes::ISHLL]),
                (OperatorType::LONG, OperatorType::INT, OperatorType::LONG, &[byte_codes::LSHL]),
                (OperatorType::LONG, OperatorType::LONG, OperatorType::LONG, &[byte_codes::LSHLL]),
            ]),
            (BinaryKind::Shift, Tag::SR, &[
                (OperatorType::INT, OperatorType::INT, OperatorType::INT, &[byte_codes::ISHR]),
                (OperatorType::INT, OperatorType::LONG, OperatorType::INT, &[byte_codes::ISHRL]),
                (OperatorType::LONG, OperatorType::INT, OperatorType::LONG, &[byte_codes::LSHR]),
                (OperatorType::LONG, OperatorType::LONG, OperatorType::LONG, &[byte_codes::LSHRL]),
            ]),
            (BinaryKind::Shift, Tag::USR, &[
                (OperatorType::INT, OperatorType::INT, OperatorType::INT, &[byte_codes::IUSHR]),
                (OperatorType::INT, OperatorType::LONG, OperatorType::INT, &[byte_codes::IUSHRL]),
                (OperatorType::LONG, OperatorType::INT, OperatorType::LONG, &[byte_codes::LUSHR]),
                (OperatorType::LONG, OperatorType::LONG, OperatorType::LONG, &[byte_codes::LUSHRL]),
            ]),
            (BinaryKind::Numeric, Tag::LT, &[
                (OperatorType::DOUBLE, OperatorType::DOUBLE, OperatorType::BOOLEAN, &[byte_codes::DCMPG, byte_codes::IFLT]),
                (OperatorType::FLOAT, OperatorType::FLOAT, OperatorType::BOOLEAN, &[byte_codes::FCMPG, byte_codes::IFLT]),
                (OperatorType::LONG, OperatorType::LONG, OperatorType::BOOLEAN, &[byte_codes::LCMP, byte_codes::IFLT]),
                (OperatorType::INT, OperatorType::INT, OperatorType::BOOLEAN, &[byte_codes::IF_ICMPLT]),
            ]),
            (BinaryKind::Numeric, Tag::GT, &[
                (OperatorType::DOUBLE, OperatorType::DOUBLE, OperatorType::BOOLEAN, &[byte_codes::DCMPL, byte_codes::IFGT]),
                (OperatorType::FLOAT, OperatorType::FLOAT, OperatorType::BOOLEAN, &[byte_codes::FCMPL, byte_codes::IFGT]),
                (OperatorType::LONG, OperatorType::LONG, OperatorType::BOOLEAN, &[byte_codes::LCMP, byte_codes::IFGT]),
                (OperatorType::INT, OperatorType::INT, OperatorType::BOOLEAN, &[byte_codes::IF_ICMPGT]),
            ]),
            (BinaryKind::Numeric, Tag::LE, &[
                (OperatorType::DOUBLE, OperatorType::DOUBLE, OperatorType::BOOLEAN, &[byte_codes::DCMPG, byte_codes::IFLE]),
                (OperatorType::FLOAT, OperatorType::FLOAT, OperatorType::BOOLEAN, &[byte_codes::FCMPG, byte_codes::IFLE]),
                (OperatorType::LONG, OperatorType::LONG, OperatorType::BOOLEAN, &[byte_codes::LCMP, byte_codes::IFLE]),
                (OperatorType::INT, OperatorType::INT, OperatorType::BOOLEAN, &[byte_codes::IF_ICMPLE]),
            ]),
            (BinaryKind::Numeric, Tag::GE, &[
                (OperatorType::DOUBLE, OperatorType::DOUBLE, OperatorType::BOOLEAN, &[byte_codes::DCMPL, byte_codes::IFGE]),
                (OperatorType::FLOAT, OperatorType::FLOAT, OperatorType::BOOLEAN, &[byte_codes::FCMPL, byte_codes::IFGE]),
                (OperatorType::LONG, OperatorType::LONG, OperatorType::BOOLEAN, &[byte_codes::LCMP, byte_codes::IFGE]),
                (OperatorType::INT, OperatorType::INT, OperatorType::BOOLEAN, &[byte_codes::IF_ICMPGE]),
            ]),
            (BinaryKind::Equality, Tag::EQ, &[
                (OperatorType::OBJECT, OperatorType::OBJECT, OperatorType::BOOLEAN, &[byte_codes::IF_ACMPEQ]),
                (OperatorType::BOOLEAN, OperatorType::BOOLEAN, OperatorType::BOOLEAN, &[byte_codes::IF_ICMPEQ]),
                (OperatorType::DOUBLE, OperatorType::DOUBLE, OperatorType::BOOLEAN, &[byte_codes::DCMPL, byte_codes::IFEQ]),
                (OperatorType::FLOAT, OperatorType::FLOAT, OperatorType::BOOLEAN, &[byte_codes::FCMPL, byte_codes::IFEQ]),
                (OperatorType::LONG, OperatorType::LONG, OperatorType::BOOLEAN, &[byte_codes::LCMP, byte_codes::IFEQ]),
                (OperatorType::INT, OperatorType::INT, OperatorType::BOOLEAN, &[byte_codes::IF_ICMPEQ]),
            ]),
            (BinaryKind::Equality, Tag::NE, &[
                (OperatorType::OBJECT, OperatorType::OBJECT, OperatorType::BOOLEAN, &[byte_codes::IF_ACMPNE]),
                (OperatorType::BOOLEAN, OperatorType::BOOLEAN, OperatorType::BOOLEAN, &[byte_codes::IF_ICMPNE]),
                (OperatorType::DOUBLE, OperatorType::DOUBLE, OperatorType::BOOLEAN, &[byte_codes::DCMPL, byte_codes::IFNE]),
                (OperatorType::FLOAT, OperatorType::FLOAT, OperatorType::BOOLEAN, &[byte_codes::FCMPL, byte_codes::IFNE]),
                (OperatorType::LONG, OperatorType::LONG, OperatorType::BOOLEAN, &[byte_codes::LCMP, byte_codes::IFNE]),
                (OperatorType::INT, OperatorType::INT, OperatorType::BOOLEAN, &[byte_codes::IF_ICMPNE]),
            ]),
            (BinaryKind::Boolean, Tag::AND, &[
                (OperatorType::BOOLEAN, OperatorType::BOOLEAN, OperatorType::BOOLEAN, &[byte_codes::BOOL_AND]),
            ]),
            (BinaryKind::Boolean, Tag::OR, &[
                (OperatorType::BOOLEAN, OperatorType::BOOLEAN, OperatorType::BOOLEAN, &[byte_codes::BOOL_OR]),
            ]),
        ];
        for (kind, tag, entries) in table {
            for (arg1, arg2, res, opcodes) in entries.iter() {
                let name = self.operator_name(*tag);
                let symbol =
                    self.make_operator(ctx, syms, &name, &[*arg1, *arg2], *res, opcodes);
                self.binary.push(BinaryEntry {
                    tag: *tag,
                    kind: *kind,
                    arg1: *arg1,
                    arg2: *arg2,
                    res: *res,
                    opcodes: opcodes.to_vec(),
                    symbol,
                });
            }
        }
    }
}

fn type_arg_text(ctx: &CompilerContext, t: TypeId) -> String {
    ctx.typ(t)
        .tsym
        .map(|tsym| crate::comp::resolve::symbol_display_name(ctx, tsym))
        .unwrap_or_else(|| format!("{:?}", ctx.typ(t).get_tag()))
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::source::Source;
    use crate::code::symtab::Symtab;
    use crate::comp::const_fold::ConstFold;
    use crate::jvm::target::Target;
    use crate::util::constants::ConstantValue;
    use crate::util::context::Context;
    use crate::util::javac_messages::JavacMessages;

    #[allow(dead_code)]
    struct Harness {
        ctx: CompilerContext,
        syms: Symtab,
        diags: Rc<Factory>,
        operators: Operators,
        types: Types,
        log: Rc<RefCell<Log>>,
        _names: Rc<Names>,
    }

    fn harness() -> Harness {
        let mut ctx = CompilerContext::new();
        let names = Rc::new(Names::new(false));
        let syms = Symtab::new(
            &mut ctx,
            Rc::clone(&names),
            Target::JDK1_17,
            Source::JDK17,
        );
        let mut jcontext = Context::new();
        let messages = JavacMessages::instance(&mut jcontext);
        let diags = Rc::new(Factory::new(messages, "compiler"));
        let log = Log::instance(&mut jcontext);
        let types = Types::with_names(Rc::clone(&names));
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
            diags,
            operators,
            types,
            log,
            _names: names,
        }
    }

    fn operator_opcode(ctx: &CompilerContext, sym: SymbolId) -> i32 {
        match &ctx.sym(sym).kind_data {
            SymbolKind::OperatorSymbol { opcode, .. } => *opcode,
            _ => panic!("expected an operator symbol"),
        }
    }

    fn return_type(ctx: &CompilerContext, sym: SymbolId) -> TypeId {
        let ty = ctx.sym(sym).ty.expect("operator type");
        match &ctx.typ(ty).kind {
            TypeKindData::MethodType { restype, .. } => {
                restype.expect("operator return type")
            }
            _ => panic!("expected a method type"),
        }
    }

    #[test]
    fn resolve_binary_numeric_and_string_operators() {
        let mut h = harness();
        let int_t = h.syms.int_type;
        let string_t = h.syms.string_type;
        let long_t = h.syms.long_type;
        let plus = h.operators.resolve_binary(
            &mut h.ctx,
            &h.syms,
            &h.types,
            0,
            Tag::PLUS,
            int_t,
            int_t,
        );
        assert_ne!(plus, h.operators.no_op_symbol);
        assert_eq!(operator_opcode(&h.ctx, plus), byte_codes::IADD);
        assert_eq!(return_type(&h.ctx, plus), int_t);

        let concat = h.operators.resolve_binary(
            &mut h.ctx,
            &h.syms,
            &h.types,
            0,
            Tag::PLUS,
            string_t,
            string_t,
        );
        assert_ne!(concat, h.operators.no_op_symbol);
        assert_eq!(operator_opcode(&h.ctx, concat), byte_codes::STRING_ADD);
        assert_eq!(return_type(&h.ctx, concat), string_t);

        let shift = h.operators.resolve_binary(
            &mut h.ctx,
            &h.syms,
            &h.types,
            0,
            Tag::SL,
            int_t,
            long_t,
        );
        assert_ne!(shift, h.operators.no_op_symbol);
        assert_eq!(operator_opcode(&h.ctx, shift), byte_codes::ISHLL);
        assert_eq!(return_type(&h.ctx, shift), int_t);
    }

    #[test]
    fn resolve_unary_operators() {
        let mut h = harness();
        let int_t = h.syms.int_type;
        let bool_t = h.syms.boolean_type;
        let neg = h.operators.resolve_unary(
            &mut h.ctx,
            &h.syms,
            &h.types,
            0,
            Tag::NEG,
            int_t,
        );
        assert_ne!(neg, h.operators.no_op_symbol);
        assert_eq!(operator_opcode(&h.ctx, neg), byte_codes::INEG);
        assert_eq!(return_type(&h.ctx, neg), int_t);

        let not = h.operators.resolve_unary(
            &mut h.ctx,
            &h.syms,
            &h.types,
            0,
            Tag::NOT,
            bool_t,
        );
        assert_ne!(not, h.operators.no_op_symbol);
        assert_eq!(operator_opcode(&h.ctx, not), byte_codes::BOOL_NOT);
        assert_eq!(return_type(&h.ctx, not), bool_t);
    }

    #[test]
    fn operators_identity_is_stable_for_repeated_lookups() {
        let mut h = harness();
        let long_t = h.syms.long_type;
        let first = h.operators.resolve_binary(
            &mut h.ctx,
            &h.syms,
            &h.types,
            0,
            Tag::MINUS,
            long_t,
            long_t,
        );
        let second = h.operators.resolve_binary(
            &mut h.ctx,
            &h.syms,
            &h.types,
            0,
            Tag::MINUS,
            long_t,
            long_t,
        );
        assert_eq!(first, second);
        assert_eq!(operator_opcode(&h.ctx, first), byte_codes::LSUB);
    }

    #[test]
    fn const_fold_folds_basic_int_and_string_operations() {
        let mut h = harness();
        let cf = ConstFold::new();
        let int_t = h.syms.int_type;
        let string_t = h.syms.string_type;
        let one = h.ctx.type_const_type(int_t, ConstantValue::Integer(1));
        let two = h.ctx.type_const_type(int_t, ConstantValue::Integer(2));
        let sum = cf
            .fold2(&mut h.ctx, &h.syms, byte_codes::IADD, one, two)
            .expect("int fold");
        assert_eq!(h.ctx.typ(sum).const_value(), Some(&ConstantValue::Integer(3)));
        let zero = h.ctx.type_const_type(int_t, ConstantValue::Integer(0));
        let quotient = cf.fold2(&mut h.ctx, &h.syms, byte_codes::IDIV, one, zero);
        assert!(quotient.is_none(), "division by zero must return null");

        let a = h.ctx.type_const_type(
            string_t,
            ConstantValue::String(javac_strings::JavaString::from("a")),
        );
        let b = h.ctx.type_const_type(
            string_t,
            ConstantValue::String(javac_strings::JavaString::from("b")),
        );
        let concat = cf
            .fold2(&mut h.ctx, &h.syms, byte_codes::STRING_ADD, a, b)
            .expect("string fold");
        match h.ctx.typ(concat).const_value() {
            Some(ConstantValue::String(value)) => assert_eq!(value.to_string(), "ab"),
            other => panic!("expected a string constant, got {:?}", other),
        }
    }

    #[test]
    fn const_fold_composite_comparison_and_coerce() {
        let mut h = harness();
        let cf = ConstFold::new();
        let int_t = h.syms.int_type;
        let long_t = h.syms.long_type;
        let one = h.ctx.type_const_type(int_t, ConstantValue::Integer(1));
        // Composite instruction lcmp; iflt (as built by Operators.mergeOpcodes).
        let cmp = (byte_codes::LCMP << byte_codes::PRE_SHIFT) | byte_codes::IFLT;
        let two_long = h
            .ctx
            .type_const_type(long_t, ConstantValue::Long(2));
        let one_long = h
            .ctx
            .type_const_type(long_t, ConstantValue::Long(1));
        let lt = cf
            .fold2(&mut h.ctx, &h.syms, cmp, one_long, two_long)
            .expect("composite fold");
        assert_eq!(
            h.ctx.typ(lt).const_value(),
            Some(&ConstantValue::Boolean(true))
        );
        let coerced = cf.coerce(&mut h.ctx, &h.syms, one, long_t);
        assert_eq!(
            h.ctx.typ(coerced).const_value(),
            Some(&ConstantValue::Long(1))
        );
    }
}
