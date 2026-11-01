public class LitExtreme {
    static final int MIN_I = Integer.MIN_VALUE;
    static final int MAX_I = 0x7fff_ffff;
    static final int HEX = 0xCAFE_BABE;
    static final int BIN = 0b1010_1010_1010;
    static final int OCT = 0_777;
    static final long MAX_L = 0xffff_ffff_ffff_ffffL;
    static final long MIN_L = Long.MIN_VALUE;
    static final double NEG_ZERO = -0.0;
    static final double NAN = Double.NaN;
    static final float FMIN = Float.MIN_VALUE;
    static final double DSUB = 1.0e-323;
    static final char NUL = '\u0000';
    static final char MAX_C = '\uffff';
    static final String SURROGATE = "\ud83d\ude00";
    static final String ESCAPES = "\t\n\r\f\b\\\"\'";
    static final String OCTAL = "\101\102";
    static final String UNICODE = "\u0041\u00e9\u4e2d";
    static final int SHIFT = 1 << 31 >> 31;
    static final long MIX = (5L << 62) | (MIN_L + 1);

    public static void main(String[] args) {
        System.out.println(MIN_I + MAX_I + HEX + BIN + OCT + MAX_L + MIN_L);
        System.out.println(NEG_ZERO + NAN + FMIN + DSUB + NUL + MAX_C);
        System.out.println(SURROGATE + ESCAPES + OCTAL + UNICODE + SHIFT + MIX);
    }
}
