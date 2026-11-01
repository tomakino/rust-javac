public class TextBlocks {
    public static void main(String[] args) {
        String block = """
                line one
                  indented line
                line three
                """;
        System.out.println(block.lines().count() + ":" + block.contains("indented line"));
    }
}
