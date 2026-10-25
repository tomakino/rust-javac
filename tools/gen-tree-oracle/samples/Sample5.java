public class Sample5 {
    long hex = 0xDEAD_BEEFL;
    int binary = 0b1010_1010;
    int octal = 0777;
    double d = 1.5e-3;
    float f = .5f;
    char c = '\n';
    char unicodeEscape = '\u0041';
    String s = "a\u0041\n\t\"quoted\"";
    int shift = (1 << 4) | (0xFF & 7) ^ 3 >>> 1;
    boolean logic = true && false || !(1 > 2);
    long underscore = 1_000_000L;
}
