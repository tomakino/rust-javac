public class ConstFold {
    static final int MAX = Integer.MAX_VALUE;
    static final int MIN = Integer.MIN_VALUE;
    static final long LMAX = Long.MAX_VALUE;
    static final long LMIN = Long.MIN_VALUE;
    static final int SHIFT = 1 << 31;
    static final long LSHIFT = 1L << 63;
    static final int OVERFLOW = MAX + 1;
    static final long LOVERFLOW = LMAX + 1L;
    static final int MASK = 0xFFFF_FFFF;
    static final int OCTAL = 017777777777;
    static final double NAN = 0.0 / 0.0;
    static final double INF = 1.0 / 0.0;
    static final float FMIN = Float.MIN_VALUE;
    static final double SUB = Double.MIN_VALUE;
    static final int DIV = MIN / -1;
    static final int REM = MIN % -1;

    static final int FOLD = (1 + 2) * (3 + 4) - (5 + 6);
    static final String CONCAT = "a" + "b" + 1 + 2 + 3 + "c" + (4 + 5);
    static final char CHAR = '\u0041' + 1;
    static final boolean BOOL = 1 < 2 && 3 > 2 || false;
    static final long MIX = 1 + 2L * 3 + (int) 4L + 'a';

    public static void main(String[] args) {
        System.out.println(MAX + " " + MIN + " " + LMAX + " " + LMIN);
        System.out.println(SHIFT + " " + LSHIFT + " " + OVERFLOW + " " + LOVERFLOW);
        System.out.println(MASK + " " + OCTAL + " " + DIV + " " + REM);
        System.out.println(NAN + " " + INF + " " + FMIN + " " + SUB);
        System.out.println(FOLD + " " + CONCAT + " " + CHAR + " " + BOOL + " " + MIX);
        System.out.println(NAN == NAN);
        System.out.println(NAN != NAN);
        System.out.println(0.0 == -0.0);
        System.out.println(Double.compare(NAN, NAN));
        System.out.println(1 + 2 + "" + 3 + 4);
        System.out.println("" + 1 + 2 + 3 + 4);
    }
}
