// Origin: com/sun/tools/javac/comp/ConstFold.java @ 982d53c (jdk-17.0.19-ga)
//
// PORTING_RULES:
// - ADR 0001: the ported ConstFold takes the services explicitly; the upstream
// ConstFold.instance(Context)/(Context) constructor is registered with the
// Context registration.
// - adapter adapter: the Java boolean constant value is Integer(0/1) upstream
// (ConstFold.b2i); the ported closed ConstantValue domain has a Boolean
// variant, so boolean results use ConstantValue::Boolean (same value domain).
// - number conversions implement the java.lang.Number narrowing semantics used
// by the upstream intValue/longValue/floatValue/doubleValue helpers.

use crate::code::symtab::Symtab;
use crate::code::type_tag::TypeTag;
use crate::compiler_context::CompilerContext;
use crate::ids::TypeId;
use crate::jvm::byte_codes;
use crate::util::constants::ConstantValue;
use crate::util::java_string::JavaString;

/// Java: strictfp class ConstFold (comp/ConstFold.java:63)
#[derive(Default, Clone, Copy)]
pub struct ConstFold;

impl ConstFold {
    /// Java: private ConstFold(Context context) — explicit services (ADR 0001).
    pub fn new() -> ConstFold {
        ConstFold
    }

    /// Java: private static Integer b2i(boolean b) (ConstFold.java:88-90)
    fn b2i(b: bool) -> ConstantValue {
        ConstantValue::Boolean(b)
    }

    /// Java: private static int intValue(Object x) (ConstFold.java:91)
    fn int_value(value: &ConstantValue) -> i32 {
        match value {
            ConstantValue::Integer(v) => *v,
            ConstantValue::Byte(v) => *v as i32,
            ConstantValue::Short(v) => *v as i32,
            ConstantValue::Long(v) => *v as i32,
            ConstantValue::Float(v) => *v as i32,
            ConstantValue::Double(v) => *v as i32,
            ConstantValue::Character(v) => *v as i32,
            ConstantValue::Boolean(v) => {
                if *v {
                    1
                } else {
                    0
                }
            }
            ConstantValue::String(_) => panic!("ClassCastException: String is not a Number"),
        }
    }

    /// Java: private static long longValue(Object x) (ConstFold.java:92)
    fn long_value(value: &ConstantValue) -> i64 {
        match value {
            ConstantValue::Integer(v) => *v as i64,
            ConstantValue::Byte(v) => *v as i64,
            ConstantValue::Short(v) => *v as i64,
            ConstantValue::Long(v) => *v,
            ConstantValue::Float(v) => *v as i64,
            ConstantValue::Double(v) => *v as i64,
            ConstantValue::Character(v) => *v as i64,
            ConstantValue::Boolean(v) => {
                if *v {
                    1
                } else {
                    0
                }
            }
            ConstantValue::String(_) => panic!("ClassCastException: String is not a Number"),
        }
    }

    /// Java: private static float floatValue(Object x) (ConstFold.java:93)
    fn float_value(value: &ConstantValue) -> f32 {
        match value {
            ConstantValue::Integer(v) => *v as f32,
            ConstantValue::Byte(v) => *v as f32,
            ConstantValue::Short(v) => *v as f32,
            ConstantValue::Long(v) => *v as f32,
            ConstantValue::Float(v) => *v,
            ConstantValue::Double(v) => *v as f32,
            ConstantValue::Character(v) => *v as f32,
            ConstantValue::Boolean(_) => panic!("ClassCastException: Boolean is not a Number"),
            ConstantValue::String(_) => panic!("ClassCastException: String is not a Number"),
        }
    }

    /// Java: private static double doubleValue(Object x) (ConstFold.java:94)
    fn double_value(value: &ConstantValue) -> f64 {
        match value {
            ConstantValue::Integer(v) => *v as f64,
            ConstantValue::Byte(v) => *v as f64,
            ConstantValue::Short(v) => *v as f64,
            ConstantValue::Long(v) => *v as f64,
            ConstantValue::Float(v) => *v as f64,
            ConstantValue::Double(v) => *v,
            ConstantValue::Character(v) => *v as f64,
            ConstantValue::Boolean(_) => panic!("ClassCastException: Boolean is not a Number"),
            ConstantValue::String(_) => panic!("ClassCastException: String is not a Number"),
        }
    }

    fn const_value(ctx: &CompilerContext, t: TypeId) -> Option<&ConstantValue> {
        ctx.typ(t).const_value()
    }

    fn const_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ty: TypeId,
        value: ConstantValue,
    ) -> TypeId {
        let _ = syms;
        ctx.type_const_type(ty, value)
    }

    /// Java: Type fold1(int opcode, Type operand) (ConstFold.java:104-153)
    pub fn fold1(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        opcode: i32,
        operand: TypeId,
    ) -> Option<TypeId> {
        let value = Self::const_value(ctx, operand)?;
        let result: Option<(TypeId, ConstantValue)> = match opcode {
            byte_codes::NOP => return Some(operand),
            byte_codes::INEG => Some((
                syms.int_type,
                ConstantValue::Integer(Self::int_value(value).wrapping_neg()),
            )),
            byte_codes::IXOR => Some((
                syms.int_type,
                ConstantValue::Integer(!Self::int_value(value)),
            )),
            byte_codes::BOOL_NOT => Some((
                syms.boolean_type,
                Self::b2i(Self::int_value(value) == 0),
            )),
            byte_codes::IFEQ => Some((
                syms.boolean_type,
                Self::b2i(Self::int_value(value) == 0),
            )),
            byte_codes::IFNE => Some((
                syms.boolean_type,
                Self::b2i(Self::int_value(value) != 0),
            )),
            byte_codes::IFLT => Some((
                syms.boolean_type,
                Self::b2i(Self::int_value(value) < 0),
            )),
            byte_codes::IFGT => Some((
                syms.boolean_type,
                Self::b2i(Self::int_value(value) > 0),
            )),
            byte_codes::IFLE => Some((
                syms.boolean_type,
                Self::b2i(Self::int_value(value) <= 0),
            )),
            byte_codes::IFGE => Some((
                syms.boolean_type,
                Self::b2i(Self::int_value(value) >= 0),
            )),
            byte_codes::LNEG => Some((
                syms.long_type,
                ConstantValue::Long(Self::long_value(value).wrapping_neg()),
            )),
            byte_codes::LXOR => Some((
                syms.long_type,
                ConstantValue::Long(!Self::long_value(value)),
            )),
            byte_codes::FNEG => Some((
                syms.float_type,
                ConstantValue::Float(-Self::float_value(value)),
            )),
            byte_codes::DNEG => Some((
                syms.double_type,
                ConstantValue::Double(-Self::double_value(value)),
            )),
            _ => None,
        };
        result.map(|(ty, value)| self.const_type(ctx, syms, ty, value))
    }

    /// Java: Type fold2(int opcode, Type left, Type right)
    /// (ConstFold.java:165-337)
    pub fn fold2(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        opcode: i32,
        left: TypeId,
        right: TypeId,
    ) -> Option<TypeId> {
        if opcode > byte_codes::PRE_MASK {
            // we are seeing a composite instruction of the form xcmp; ifxx.
            let t1 = self.fold2(ctx, syms, opcode >> byte_codes::PRE_SHIFT, left, right)?;
            if Self::const_value(ctx, t1).is_none() {
                return Some(t1);
            }
            return self.fold1(ctx, syms, opcode & byte_codes::PRE_MASK, t1);
        }
        let l = Self::const_value(ctx, left).expect("constant lhs").clone();
        let r = Self::const_value(ctx, right).expect("constant rhs").clone();
        let result: Option<(TypeId, ConstantValue)> = match opcode {
            byte_codes::IADD => Some((
                syms.int_type,
                ConstantValue::Integer(Self::int_value(&l).wrapping_add(Self::int_value(&r))),
            )),
            byte_codes::ISUB => Some((
                syms.int_type,
                ConstantValue::Integer(Self::int_value(&l).wrapping_sub(Self::int_value(&r))),
            )),
            byte_codes::IMUL => Some((
                syms.int_type,
                ConstantValue::Integer(Self::int_value(&l).wrapping_mul(Self::int_value(&r))),
            )),
            byte_codes::IDIV => Self::int_value(&l)
                .checked_div(Self::int_value(&r))
                .map(|v| (syms.int_type, ConstantValue::Integer(v))),
            byte_codes::IMOD => Self::int_value(&l)
                .checked_rem(Self::int_value(&r))
                .map(|v| (syms.int_type, ConstantValue::Integer(v))),
            byte_codes::IAND => {
                let ty = if ctx.typ(left).has_tag(TypeTag::BOOLEAN) {
                    syms.boolean_type
                } else {
                    syms.int_type
                };
                Some((
                    ty,
                    ConstantValue::Integer(Self::int_value(&l) & Self::int_value(&r)),
                ))
            }
            byte_codes::BOOL_AND => Some((
                syms.boolean_type,
                Self::b2i((Self::int_value(&l) & Self::int_value(&r)) != 0),
            )),
            byte_codes::IOR => {
                let ty = if ctx.typ(left).has_tag(TypeTag::BOOLEAN) {
                    syms.boolean_type
                } else {
                    syms.int_type
                };
                Some((
                    ty,
                    ConstantValue::Integer(Self::int_value(&l) | Self::int_value(&r)),
                ))
            }
            byte_codes::BOOL_OR => Some((
                syms.boolean_type,
                Self::b2i((Self::int_value(&l) | Self::int_value(&r)) != 0),
            )),
            byte_codes::IXOR => {
                let ty = if ctx.typ(left).has_tag(TypeTag::BOOLEAN) {
                    syms.boolean_type
                } else {
                    syms.int_type
                };
                Some((
                    ty,
                    ConstantValue::Integer(Self::int_value(&l) ^ Self::int_value(&r)),
                ))
            }
            byte_codes::ISHL | byte_codes::ISHLL => Some((
                syms.int_type,
                ConstantValue::Integer(Self::int_value(&l) << (Self::int_value(&r) & 0x1f)),
            )),
            byte_codes::ISHR | byte_codes::ISHRL => Some((
                syms.int_type,
                ConstantValue::Integer(Self::int_value(&l) >> (Self::int_value(&r) & 0x1f)),
            )),
            byte_codes::IUSHR | byte_codes::IUSHRL => Some((
                syms.int_type,
                ConstantValue::Integer(
                    ((Self::int_value(&l) as u32) >> (Self::int_value(&r) & 0x1f)) as i32,
                ),
            )),
            byte_codes::IF_ICMPEQ => Some((
                syms.boolean_type,
                Self::b2i(Self::int_value(&l) == Self::int_value(&r)),
            )),
            byte_codes::IF_ICMPNE => Some((
                syms.boolean_type,
                Self::b2i(Self::int_value(&l) != Self::int_value(&r)),
            )),
            byte_codes::IF_ICMPLT => Some((
                syms.boolean_type,
                Self::b2i(Self::int_value(&l) < Self::int_value(&r)),
            )),
            byte_codes::IF_ICMPGT => Some((
                syms.boolean_type,
                Self::b2i(Self::int_value(&l) > Self::int_value(&r)),
            )),
            byte_codes::IF_ICMPLE => Some((
                syms.boolean_type,
                Self::b2i(Self::int_value(&l) <= Self::int_value(&r)),
            )),
            byte_codes::IF_ICMPGE => Some((
                syms.boolean_type,
                Self::b2i(Self::int_value(&l) >= Self::int_value(&r)),
            )),
            byte_codes::LADD => Some((
                syms.long_type,
                ConstantValue::Long(Self::long_value(&l).wrapping_add(Self::long_value(&r))),
            )),
            byte_codes::LSUB => Some((
                syms.long_type,
                ConstantValue::Long(Self::long_value(&l).wrapping_sub(Self::long_value(&r))),
            )),
            byte_codes::LMUL => Some((
                syms.long_type,
                ConstantValue::Long(Self::long_value(&l).wrapping_mul(Self::long_value(&r))),
            )),
            byte_codes::LDIV => Self::long_value(&l)
                .checked_div(Self::long_value(&r))
                .map(|v| (syms.long_type, ConstantValue::Long(v))),
            byte_codes::LMOD => Self::long_value(&l)
                .checked_rem(Self::long_value(&r))
                .map(|v| (syms.long_type, ConstantValue::Long(v))),
            byte_codes::LAND => Some((
                syms.long_type,
                ConstantValue::Long(Self::long_value(&l) & Self::long_value(&r)),
            )),
            byte_codes::LOR => Some((
                syms.long_type,
                ConstantValue::Long(Self::long_value(&l) | Self::long_value(&r)),
            )),
            byte_codes::LXOR => Some((
                syms.long_type,
                ConstantValue::Long(Self::long_value(&l) ^ Self::long_value(&r)),
            )),
            byte_codes::LSHL | byte_codes::LSHLL => Some((
                syms.long_type,
                ConstantValue::Long(Self::long_value(&l) << (Self::int_value(&r) & 0x3f)),
            )),
            byte_codes::LSHR | byte_codes::LSHRL => Some((
                syms.long_type,
                ConstantValue::Long(Self::long_value(&l) >> (Self::int_value(&r) & 0x3f)),
            )),
            byte_codes::LUSHR => Some((
                syms.long_type,
                ConstantValue::Long(
                    ((Self::long_value(&l) as u64) >> (Self::int_value(&r) & 0x3f)) as i64,
                ),
            )),
            byte_codes::LCMP => {
                if Self::long_value(&l) < Self::long_value(&r) {
                    Some((syms.int_type, ConstantValue::Integer(-1)))
                } else if Self::long_value(&l) > Self::long_value(&r) {
                    Some((syms.int_type, ConstantValue::Integer(1)))
                } else {
                    Some((syms.int_type, ConstantValue::Integer(0)))
                }
            }
            byte_codes::FADD => Some((
                syms.float_type,
                ConstantValue::Float(Self::float_value(&l) + Self::float_value(&r)),
            )),
            byte_codes::FSUB => Some((
                syms.float_type,
                ConstantValue::Float(Self::float_value(&l) - Self::float_value(&r)),
            )),
            byte_codes::FMUL => Some((
                syms.float_type,
                ConstantValue::Float(Self::float_value(&l) * Self::float_value(&r)),
            )),
            byte_codes::FDIV => Some((
                syms.float_type,
                ConstantValue::Float(Self::float_value(&l) / Self::float_value(&r)),
            )),
            byte_codes::FMOD => Some((
                syms.float_type,
                ConstantValue::Float(Self::float_value(&l) % Self::float_value(&r)),
            )),
            byte_codes::FCMPG | byte_codes::FCMPL => {
                let lv = Self::float_value(&l);
                let rv = Self::float_value(&r);
                if lv < rv {
                    Some((syms.int_type, ConstantValue::Integer(-1)))
                } else if lv > rv {
                    Some((syms.int_type, ConstantValue::Integer(1)))
                } else if lv == rv {
                    Some((syms.int_type, ConstantValue::Integer(0)))
                } else if opcode == byte_codes::FCMPG {
                    Some((syms.int_type, ConstantValue::Integer(1)))
                } else {
                    Some((syms.int_type, ConstantValue::Integer(-1)))
                }
            }
            byte_codes::DADD => Some((
                syms.double_type,
                ConstantValue::Double(Self::double_value(&l) + Self::double_value(&r)),
            )),
            byte_codes::DSUB => Some((
                syms.double_type,
                ConstantValue::Double(Self::double_value(&l) - Self::double_value(&r)),
            )),
            byte_codes::DMUL => Some((
                syms.double_type,
                ConstantValue::Double(Self::double_value(&l) * Self::double_value(&r)),
            )),
            byte_codes::DDIV => Some((
                syms.double_type,
                ConstantValue::Double(Self::double_value(&l) / Self::double_value(&r)),
            )),
            byte_codes::DMOD => Some((
                syms.double_type,
                ConstantValue::Double(Self::double_value(&l) % Self::double_value(&r)),
            )),
            byte_codes::DCMPG | byte_codes::DCMPL => {
                let lv = Self::double_value(&l);
                let rv = Self::double_value(&r);
                if lv < rv {
                    Some((syms.int_type, ConstantValue::Integer(-1)))
                } else if lv > rv {
                    Some((syms.int_type, ConstantValue::Integer(1)))
                } else if lv == rv {
                    Some((syms.int_type, ConstantValue::Integer(0)))
                } else if opcode == byte_codes::DCMPG {
                    Some((syms.int_type, ConstantValue::Integer(1)))
                } else {
                    Some((syms.int_type, ConstantValue::Integer(-1)))
                }
            }
            byte_codes::IF_ACMPEQ => Some((syms.boolean_type, Self::b2i(l == r))),
            byte_codes::IF_ACMPNE => Some((syms.boolean_type, Self::b2i(l != r))),
            byte_codes::STRING_ADD => {
                let ls = ConstFold::string_value(&l);
                let rs = ConstFold::string_value(&r);
                Some((
                    syms.string_type,
                    ConstantValue::String(JavaString::from(
                        format!("{ls}{rs}").as_str(),
                    )),
                ))
            }
            _ => None,
        };
        result.map(|(ty, value)| self.const_type(ctx, syms, ty, value))
    }

    /// Java: Type.stringValue() for the constant String value.
    fn string_value(value: &ConstantValue) -> JavaString {
        match value {
            ConstantValue::String(s) => s.clone(),
            _ => panic!("ClassCastException: String constant expected"),
        }
    }

    /// Java: Type coerce(Type etype, Type ttype) (ConstFold.java:345-372)
    pub fn coerce(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        etype: TypeId,
        ttype: TypeId,
    ) -> TypeId {
        let etsym_type = ctx
            .typ(etype)
            .tsym
            .and_then(|tsym| ctx.sym(tsym).ty);
        let ttsym_type = ctx
            .typ(ttype)
            .tsym
            .and_then(|tsym| ctx.sym(tsym).ty);
        if etsym_type == ttsym_type {
            return etype;
        }
        if ctx.typ(etype).is_numeric() {
            let n = Self::const_value(ctx, etype)
                .expect("numeric constant type")
                .clone();
            let value = match ctx.typ(ttype).get_tag() {
                TypeTag::BYTE => Some((syms.byte_type, Self::int_value(&n) as i8 as i32)),
                TypeTag::CHAR => Some((syms.char_type, Self::int_value(&n) as u16 as i32)),
                TypeTag::SHORT => Some((syms.short_type, Self::int_value(&n) as i16 as i32)),
                TypeTag::INT => Some((syms.int_type, Self::int_value(&n))),
                TypeTag::LONG => None,
                TypeTag::FLOAT => None,
                TypeTag::DOUBLE => None,
                _ => None,
            };
            match ctx.typ(ttype).get_tag() {
                TypeTag::BYTE | TypeTag::CHAR | TypeTag::SHORT | TypeTag::INT => {
                    let (ty, v) = value.expect("integer coercion");
                    return self.const_type(ctx, syms, ty, ConstantValue::Integer(v));
                }
                TypeTag::LONG => {
                    return self.const_type(
                        ctx,
                        syms,
                        syms.long_type,
                        ConstantValue::Long(Self::long_value(&n)),
                    );
                }
                TypeTag::FLOAT => {
                    return self.const_type(
                        ctx,
                        syms,
                        syms.float_type,
                        ConstantValue::Float(Self::float_value(&n)),
                    );
                }
                TypeTag::DOUBLE => {
                    return self.const_type(
                        ctx,
                        syms,
                        syms.double_type,
                        ConstantValue::Double(Self::double_value(&n)),
                    );
                }
                _ => {}
            }
        }
        ttype
    }
}
