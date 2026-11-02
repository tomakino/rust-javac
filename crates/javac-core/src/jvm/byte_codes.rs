//! Bytecode instruction codes, as well as typecodes used as
//! instruction modifiers.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: Java's interface ByteCodes is a pure constant container (no imports, no
//! methods), so it maps to the constants of this module. Constant names follow
//! UPPER_SNAKE_CASE (PORTING_RULES section 3); Java names that carry a trailing underscore
//! because they collide with Java keywords (goto_, return_, new_, instanceof_) become
//! GOTO / RETURN / NEW / INSTANCEOF here, since those are not Rust keywords.

/// Byte code instruction codes.
///
/// Java: int illegal = -1, nop = 0, ...
pub const ILLEGAL: i32 = -1;
pub const NOP: i32 = 0;
pub const ACONST_NULL: i32 = 1;
pub const ICONST_M1: i32 = 2;
pub const ICONST_0: i32 = 3;
pub const ICONST_1: i32 = 4;
pub const ICONST_2: i32 = 5;
pub const ICONST_3: i32 = 6;
pub const ICONST_4: i32 = 7;
pub const ICONST_5: i32 = 8;
pub const LCONST_0: i32 = 9;
pub const LCONST_1: i32 = 10;
pub const FCONST_0: i32 = 11;
pub const FCONST_1: i32 = 12;
pub const FCONST_2: i32 = 13;
pub const DCONST_0: i32 = 14;
pub const DCONST_1: i32 = 15;
pub const BIPUSH: i32 = 16;
pub const SIPUSH: i32 = 17;
pub const LDC1: i32 = 18;
pub const LDC2: i32 = 19;
pub const LDC2W: i32 = 20;
pub const ILOAD: i32 = 21;
pub const LLOAD: i32 = 22;
pub const FLOAD: i32 = 23;
pub const DLOAD: i32 = 24;
pub const ALOAD: i32 = 25;
pub const ILOAD_0: i32 = 26;
pub const ILOAD_1: i32 = 27;
pub const ILOAD_2: i32 = 28;
pub const ILOAD_3: i32 = 29;
pub const LLOAD_0: i32 = 30;
pub const LLOAD_1: i32 = 31;
pub const LLOAD_2: i32 = 32;
pub const LLOAD_3: i32 = 33;
pub const FLOAD_0: i32 = 34;
pub const FLOAD_1: i32 = 35;
pub const FLOAD_2: i32 = 36;
pub const FLOAD_3: i32 = 37;
pub const DLOAD_0: i32 = 38;
pub const DLOAD_1: i32 = 39;
pub const DLOAD_2: i32 = 40;
pub const DLOAD_3: i32 = 41;
pub const ALOAD_0: i32 = 42;
pub const ALOAD_1: i32 = 43;
pub const ALOAD_2: i32 = 44;
pub const ALOAD_3: i32 = 45;
pub const IALOAD: i32 = 46;
pub const LALOAD: i32 = 47;
pub const FALOAD: i32 = 48;
pub const DALOAD: i32 = 49;
pub const AALOAD: i32 = 50;
pub const BALOAD: i32 = 51;
pub const CALOAD: i32 = 52;
pub const SALOAD: i32 = 53;
pub const ISTORE: i32 = 54;
pub const LSTORE: i32 = 55;
pub const FSTORE: i32 = 56;
pub const DSTORE: i32 = 57;
pub const ASTORE: i32 = 58;
pub const ISTORE_0: i32 = 59;
pub const ISTORE_1: i32 = 60;
pub const ISTORE_2: i32 = 61;
pub const ISTORE_3: i32 = 62;
pub const LSTORE_0: i32 = 63;
pub const LSTORE_1: i32 = 64;
pub const LSTORE_2: i32 = 65;
pub const LSTORE_3: i32 = 66;
pub const FSTORE_0: i32 = 67;
pub const FSTORE_1: i32 = 68;
pub const FSTORE_2: i32 = 69;
pub const FSTORE_3: i32 = 70;
pub const DSTORE_0: i32 = 71;
pub const DSTORE_1: i32 = 72;
pub const DSTORE_2: i32 = 73;
pub const DSTORE_3: i32 = 74;
pub const ASTORE_0: i32 = 75;
pub const ASTORE_1: i32 = 76;
pub const ASTORE_2: i32 = 77;
pub const ASTORE_3: i32 = 78;
pub const IASTORE: i32 = 79;
pub const LASTORE: i32 = 80;
pub const FASTORE: i32 = 81;
pub const DASTORE: i32 = 82;
pub const AASTORE: i32 = 83;
pub const BASTORE: i32 = 84;
pub const CASTORE: i32 = 85;
pub const SASTORE: i32 = 86;
pub const POP: i32 = 87;
pub const POP2: i32 = 88;
pub const DUP: i32 = 89;
pub const DUP_X1: i32 = 90;
pub const DUP_X2: i32 = 91;
pub const DUP2: i32 = 92;
pub const DUP2_X1: i32 = 93;
pub const DUP2_X2: i32 = 94;
pub const SWAP: i32 = 95;
pub const IADD: i32 = 96;
pub const LADD: i32 = 97;
pub const FADD: i32 = 98;
pub const DADD: i32 = 99;
pub const ISUB: i32 = 100;
pub const LSUB: i32 = 101;
pub const FSUB: i32 = 102;
pub const DSUB: i32 = 103;
pub const IMUL: i32 = 104;
pub const LMUL: i32 = 105;
pub const FMUL: i32 = 106;
pub const DMUL: i32 = 107;
pub const IDIV: i32 = 108;
pub const LDIV: i32 = 109;
pub const FDIV: i32 = 110;
pub const DDIV: i32 = 111;
pub const IMOD: i32 = 112;
pub const LMOD: i32 = 113;
pub const FMOD: i32 = 114;
pub const DMOD: i32 = 115;
pub const INEG: i32 = 116;
pub const LNEG: i32 = 117;
pub const FNEG: i32 = 118;
pub const DNEG: i32 = 119;
pub const ISHL: i32 = 120;
pub const LSHL: i32 = 121;
pub const ISHR: i32 = 122;
pub const LSHR: i32 = 123;
pub const IUSHR: i32 = 124;
pub const LUSHR: i32 = 125;
pub const IAND: i32 = 126;
pub const LAND: i32 = 127;
pub const IOR: i32 = 128;
pub const LOR: i32 = 129;
pub const IXOR: i32 = 130;
pub const LXOR: i32 = 131;
pub const IINC: i32 = 132;
pub const I2L: i32 = 133;
pub const I2F: i32 = 134;
pub const I2D: i32 = 135;
pub const L2I: i32 = 136;
pub const L2F: i32 = 137;
pub const L2D: i32 = 138;
pub const F2I: i32 = 139;
pub const F2L: i32 = 140;
pub const F2D: i32 = 141;
pub const D2I: i32 = 142;
pub const D2L: i32 = 143;
pub const D2F: i32 = 144;
pub const INT2BYTE: i32 = 145;
pub const INT2CHAR: i32 = 146;
pub const INT2SHORT: i32 = 147;
pub const LCMP: i32 = 148;
pub const FCMPL: i32 = 149;
pub const FCMPG: i32 = 150;
pub const DCMPL: i32 = 151;
pub const DCMPG: i32 = 152;
pub const IFEQ: i32 = 153;
pub const IFNE: i32 = 154;
pub const IFLT: i32 = 155;
pub const IFGE: i32 = 156;
pub const IFGT: i32 = 157;
pub const IFLE: i32 = 158;
pub const IF_ICMPEQ: i32 = 159;
pub const IF_ICMPNE: i32 = 160;
pub const IF_ICMPLT: i32 = 161;
pub const IF_ICMPGE: i32 = 162;
pub const IF_ICMPGT: i32 = 163;
pub const IF_ICMPLE: i32 = 164;
pub const IF_ACMPEQ: i32 = 165;
pub const IF_ACMPNE: i32 = 166;
/// Java: goto_ (Java keyword collision; no collision in Rust)
pub const GOTO: i32 = 167;
pub const JSR: i32 = 168;
pub const RET: i32 = 169;
pub const TABLESWITCH: i32 = 170;
pub const LOOKUPSWITCH: i32 = 171;
pub const IRETURN: i32 = 172;
pub const LRETURN: i32 = 173;
pub const FRETURN: i32 = 174;
pub const DRETURN: i32 = 175;
pub const ARETURN: i32 = 176;
/// Java: return_ (Java keyword collision; no collision in Rust)
pub const RETURN: i32 = 177;
pub const GETSTATIC: i32 = 178;
pub const PUTSTATIC: i32 = 179;
pub const GETFIELD: i32 = 180;
pub const PUTFIELD: i32 = 181;
pub const INVOKEVIRTUAL: i32 = 182;
pub const INVOKESPECIAL: i32 = 183;
pub const INVOKESTATIC: i32 = 184;
pub const INVOKEINTERFACE: i32 = 185;
pub const INVOKEDYNAMIC: i32 = 186;
/// Java: new_ (Java keyword collision; no collision in Rust)
pub const NEW: i32 = 187;
pub const NEWARRAY: i32 = 188;
pub const ANEWARRAY: i32 = 189;
pub const ARRAYLENGTH: i32 = 190;
pub const ATHROW: i32 = 191;
pub const CHECKCAST: i32 = 192;
/// Java: instanceof_ (Java keyword collision; no collision in Rust)
pub const INSTANCEOF: i32 = 193;
pub const MONITORENTER: i32 = 194;
pub const MONITOREXIT: i32 = 195;
pub const WIDE: i32 = 196;
pub const MULTIANEWARRAY: i32 = 197;
pub const IF_ACMP_NULL: i32 = 198;
pub const IF_ACMP_NONNULL: i32 = 199;
pub const GOTO_W: i32 = 200;
pub const JSR_W: i32 = 201;
pub const BREAKPOINT: i32 = 202;
pub const BYTE_CODE_COUNT: i32 = 203;

/// Virtual instruction codes; used for constant folding.
pub const STRING_ADD: i32 = 256; // string +
pub const BOOL_NOT: i32 = 257; // boolean !
pub const BOOL_AND: i32 = 258; // boolean &&
pub const BOOL_OR: i32 = 259; // boolean ||

/// Virtual opcodes; used for shifts with long shiftcount.
pub const ISHLL: i32 = 270; // int shift left with long count
pub const LSHLL: i32 = 271; // long shift left with long count
pub const ISHRL: i32 = 272; // int shift right with long count
pub const LSHRL: i32 = 273; // long shift right with long count
pub const IUSHRL: i32 = 274; // int unsigned shift right with long count
pub const LUSHRL: i32 = 275; // long unsigned shift right with long count

/// Virtual opcode for null reference checks.
pub const NULLCHK: i32 = 276; // return operand if non-null,
// otherwise throw NullPointerException.

/// Virtual opcode for disallowed operations.
pub const ERROR: i32 = 277;

/// All conditional jumps come in pairs. To streamline the
/// treatment of jumps, we also introduce a negation of an
/// unconditional jump. That opcode happens to be jsr.
pub const DONTGOTO: i32 = JSR;

/// Shift and mask constants for shifting prefix instructions.
/// a pair of instruction codes such as LCMP ; IFEQ is encoded
/// in Symtab as {@literal (LCMP << preShift) + IFEQ }.
pub const PRE_SHIFT: i32 = 9;
pub const PRE_MASK: i32 = (1 << PRE_SHIFT) - 1;

/// Type codes.
pub const INT_CODE: i32 = 0;
pub const LONG_CODE: i32 = 1;
pub const FLOAT_CODE: i32 = 2;
pub const DOUBLE_CODE: i32 = 3;
pub const OBJECT_CODE: i32 = 4;
pub const BYTE_CODE: i32 = 5;
pub const CHAR_CODE: i32 = 6;
pub const SHORT_CODE: i32 = 7;
pub const VOID_CODE: i32 = 8;
pub const TYPE_CODE_COUNT: i32 = 9;

/// Java: static final String[] typecodeNames
pub const TYPE_CODE_NAMES: [&str; 10] = [
    "int", "long", "float", "double", "object", "byte", "char", "short", "void", "oops",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The instruction codes in declaration order (Java: the first constant block).
    const ALL_OPCODES: [i32; 203] = [
        NOP, ACONST_NULL, ICONST_M1, ICONST_0, ICONST_1, ICONST_2, ICONST_3, ICONST_4, ICONST_5,
        LCONST_0, LCONST_1, FCONST_0, FCONST_1, FCONST_2, DCONST_0, DCONST_1, BIPUSH, SIPUSH,
        LDC1, LDC2, LDC2W, ILOAD, LLOAD, FLOAD, DLOAD, ALOAD, ILOAD_0, ILOAD_1, ILOAD_2, ILOAD_3,
        LLOAD_0, LLOAD_1, LLOAD_2, LLOAD_3, FLOAD_0, FLOAD_1, FLOAD_2, FLOAD_3, DLOAD_0, DLOAD_1,
        DLOAD_2, DLOAD_3, ALOAD_0, ALOAD_1, ALOAD_2, ALOAD_3, IALOAD, LALOAD, FALOAD, DALOAD,
        AALOAD, BALOAD, CALOAD, SALOAD, ISTORE, LSTORE, FSTORE, DSTORE, ASTORE, ISTORE_0,
        ISTORE_1, ISTORE_2, ISTORE_3, LSTORE_0, LSTORE_1, LSTORE_2, LSTORE_3, FSTORE_0, FSTORE_1,
        FSTORE_2, FSTORE_3, DSTORE_0, DSTORE_1, DSTORE_2, DSTORE_3, ASTORE_0, ASTORE_1, ASTORE_2,
        ASTORE_3, IASTORE, LASTORE, FASTORE, DASTORE, AASTORE, BASTORE, CASTORE, SASTORE, POP,
        POP2, DUP, DUP_X1, DUP_X2, DUP2, DUP2_X1, DUP2_X2, SWAP, IADD, LADD, FADD, DADD, ISUB,
        LSUB, FSUB, DSUB, IMUL, LMUL, FMUL, DMUL, IDIV, LDIV, FDIV, DDIV, IMOD, LMOD, FMOD, DMOD,
        INEG, LNEG, FNEG, DNEG, ISHL, LSHL, ISHR, LSHR, IUSHR, LUSHR, IAND, LAND, IOR, LOR, IXOR,
        LXOR, IINC, I2L, I2F, I2D, L2I, L2F, L2D, F2I, F2L, F2D, D2I, D2L, D2F, INT2BYTE,
        INT2CHAR, INT2SHORT, LCMP, FCMPL, FCMPG, DCMPL, DCMPG, IFEQ, IFNE, IFLT, IFGE, IFGT,
        IFLE, IF_ICMPEQ, IF_ICMPNE, IF_ICMPLT, IF_ICMPGE, IF_ICMPGT, IF_ICMPLE, IF_ACMPEQ,
        IF_ACMPNE, GOTO, JSR, RET, TABLESWITCH, LOOKUPSWITCH, IRETURN, LRETURN, FRETURN, DRETURN,
        ARETURN, RETURN, GETSTATIC, PUTSTATIC, GETFIELD, PUTFIELD, INVOKEVIRTUAL, INVOKESPECIAL,
        INVOKESTATIC, INVOKEINTERFACE, INVOKEDYNAMIC, NEW, NEWARRAY, ANEWARRAY, ARRAYLENGTH,
        ATHROW, CHECKCAST, INSTANCEOF, MONITORENTER, MONITOREXIT, WIDE, MULTIANEWARRAY,
        IF_ACMP_NULL, IF_ACMP_NONNULL, GOTO_W, JSR_W, BREAKPOINT,
    ];

    #[test]
    fn opcode_values_match_the_jvm_specification() {
        assert_eq!(ILLEGAL, -1);
        assert_eq!(ACONST_NULL, 1);
        assert_eq!(ICONST_M1, 2);
        assert_eq!(BIPUSH, 16);
        assert_eq!(ILOAD, 21);
        assert_eq!(ISTORE, 54);
        assert_eq!(POP, 87);
        assert_eq!(IADD, 96);
        assert_eq!(IINC, 132);
        assert_eq!(LCMP, 148);
        assert_eq!(IFEQ, 153);
        assert_eq!(GOTO, 167);
        assert_eq!(JSR, 168);
        assert_eq!(TABLESWITCH, 170);
        assert_eq!(IRETURN, 172);
        assert_eq!(RETURN, 177);
        assert_eq!(GETSTATIC, 178);
        assert_eq!(INVOKEDYNAMIC, 186);
        assert_eq!(NEW, 187);
        assert_eq!(WIDE, 196);
        assert_eq!(MULTIANEWARRAY, 197);
        assert_eq!(IF_ACMP_NULL, 198);
        assert_eq!(GOTO_W, 200);
        assert_eq!(JSR_W, 201);
        assert_eq!(BREAKPOINT, 202);
        assert_eq!(BYTE_CODE_COUNT, 203);
    }

    #[test]
    fn opcodes_are_contiguous_in_declaration_order() {
        assert_eq!(ALL_OPCODES.len() as i32, BYTE_CODE_COUNT);
        for (index, code) in ALL_OPCODES.iter().enumerate() {
            assert_eq!(*code, index as i32, "opcode at index {} is out of sequence", index);
        }
        assert_eq!(*ALL_OPCODES.first().expect("non-empty"), NOP);
        assert_eq!(*ALL_OPCODES.last().expect("non-empty"), BREAKPOINT);
    }

    #[test]
    fn virtual_opcodes_match_java() {
        assert_eq!(STRING_ADD, 256);
        assert_eq!(BOOL_NOT, 257);
        assert_eq!(BOOL_AND, 258);
        assert_eq!(BOOL_OR, 259);
        assert_eq!(ISHLL, 270);
        assert_eq!(LSHLL, 271);
        assert_eq!(ISHRL, 272);
        assert_eq!(LSHRL, 273);
        assert_eq!(IUSHRL, 274);
        assert_eq!(LUSHRL, 275);
        assert_eq!(NULLCHK, 276);
        assert_eq!(ERROR, 277);
    }

    #[test]
    fn modifiers_match_java() {
        assert_eq!(DONTGOTO, JSR);
        assert_eq!(PRE_SHIFT, 9);
        assert_eq!(PRE_MASK, (1 << PRE_SHIFT) - 1);
        assert_eq!(PRE_MASK, 511);
    }

    #[test]
    fn typecodes_match_java() {
        assert_eq!(INT_CODE, 0);
        assert_eq!(LONG_CODE, 1);
        assert_eq!(FLOAT_CODE, 2);
        assert_eq!(DOUBLE_CODE, 3);
        assert_eq!(OBJECT_CODE, 4);
        assert_eq!(BYTE_CODE, 5);
        assert_eq!(CHAR_CODE, 6);
        assert_eq!(SHORT_CODE, 7);
        assert_eq!(VOID_CODE, 8);
        assert_eq!(TYPE_CODE_COUNT, 9);
        assert_eq!(TYPE_CODE_NAMES.len(), 10);
        assert_eq!(
            &TYPE_CODE_NAMES[..9],
            &["int", "long", "float", "double", "object", "byte", "char", "short", "void"]
        );
        assert_eq!(TYPE_CODE_NAMES[9], "oops");
    }
}
