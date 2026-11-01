public class TbExtreme {
    static final String SIMPLE = """
        hello
        world""";
    static final String QUOTE = """
        quote\"
        """;
    static final String TAB = """
        tab	here""";
    static final String CONT = """
        line1 \
        line2""";
    static final String SPACES = """
        trailing \s
        spaces""";
    static final String UNICODE = """
        \u00e9\u4e2d""";
    static final String EMPTY = """
        """;

    public static void main(String[] args) {
        System.out.print(SIMPLE + "|" + QUOTE + "|" + TAB + "|" + CONT + "|" + SPACES + "|" + UNICODE + "|" + EMPTY.length());
    }
}
