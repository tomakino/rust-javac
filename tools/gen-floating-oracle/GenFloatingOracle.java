import java.util.*;

public class GenFloatingOracle {
    static final String[] INPUTS = {
        "0", "-0", "0.0", "-0.0", "1", "-1", "0.1", "0.5", "0.25", "0.3",
        "1e10", "1e-10", "1e300", "1e-300", "1.7976931348623157e308",
        "4.9e-324", "5e-324", "2.2250738585072014e-308", "1.7976931348623159e308",
        "3.141592653589793", "2.718281828459045", "9007199254740993",
        "0.30000000000000004", "1.23456789012345678e-300", "123456789012345678",
        "1e7", "1e8", "1e-6", "1e-7", "9999999999999999", "1.0000000000000002",
        "7.038531e-26", "1.23456789012345678901234567890",
        "3.33333333333333333333333333333333e-200", "1e-45", "1.4e-45", "3.4028235e38",
        "1.17549435e-38", "1.1754944e-38", "1e39", "1e-46",
        "0x1.8p1", "0x1p-1074", "0x1.fffffffffffffp1023", "0x1.0000000000001p0",
        "-0x1.0p-1", "0x0.8p-1022", "0x1p0", "0x1.921fb54442d18p1", "0x1.0p-1022",
        "0x1.fffffep127", "0x1p-149", "0x1.8p-148", "0x1.fffffffffffff8p0",
        "NaN", "-NaN", "Infinity", "-Infinity", "+Infinity", " 1.5 ", "1.5f", "1.5d",
        "-1.5E+3", "1e-323", "1e-324", "1e-325", "1e308", "1e309", "1e-400"
    };
    static final char NL = (char) 10;
    public static void main(String[] args) {
        StringBuilder sb = new StringBuilder();
        for (String s : INPUTS) {
            try {
                double d = Double.parseDouble(s);
                sb.append("D|").append(s).append("|")
                  .append(Long.toHexString(Double.doubleToRawLongBits(d))).append("|")
                  .append(Double.toString(d)).append(NL);
            } catch (NumberFormatException e) {
                sb.append("D|").append(s).append("|NaN|null").append(NL);
            }
            try {
                float f = Float.parseFloat(s);
                sb.append("F|").append(s).append("|")
                  .append(Integer.toHexString(Float.floatToRawIntBits(f))).append("|")
                  .append(Float.toString(f)).append(NL);
            } catch (NumberFormatException e) {
                sb.append("F|").append(s).append("|NaN|null").append(NL);
            }
        }
        long[] dBits = {0x3ff0000000000000L, 0x3fb999999999999aL, 0x7ff0000000000000L,
                        0xfff0000000000000L, 0x7ff8000000000000L, 0x1L, 0x8000000000000001L,
                        0x3ff199999999999aL, 0x4340000000000000L, 0x4000000000000000L,
                        0x3fe0000000000000L, 0x41dfffffffc00000L, 0x3f847ae147ae147bL,
                        0x7fefffffffffffffL, 0x0010000000000000L, 0x3eb0c6f7a0b5ed8dL};
        for (long b : dBits) {
            double d = Double.longBitsToDouble(b);
            sb.append("DT|").append(Long.toHexString(b)).append("|")
              .append(Double.toString(d)).append(NL);
        }
        int[] fBits = {0x3f800000, 0x3dcccccd, 0x7f800000, 0xff800000, 0x7fc00000, 0x1,
                       0x80000001, 0x40000000, 0x3f000000, 0x4b7fffff, 0x7f7fffff,
                       0x00800000, 0x3eaaaaab};
        for (int b : fBits) {
            float f = Float.intBitsToFloat(b);
            sb.append("FT|").append(Integer.toHexString(b)).append("|")
              .append(Float.toString(f)).append(NL);
        }
        System.out.print(sb);
    }
}