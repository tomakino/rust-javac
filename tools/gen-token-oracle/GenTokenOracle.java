// Source: tools/gen-token-oracle/GenTokenOracle.java (development-time oracle tool).
// Purpose: generate token streams from the baseline JDK 17 javac Scanner for the
// rust-javac parser/lexer tests. The generated data file is checked into the
// repository; the tests never start a JVM.
//
// Usage (development time only; see tools/gen-token-oracle/README.md):
//   javac --add-exports ... -d target/gen-token-oracle-classes tools/gen-token-oracle/GenTokenOracle.java
//   java --add-exports ... -cp target/gen-token-oracle-classes GenTokenOracle \
//        crates/javac-tests/data/token-oracle.txt 5 \
//        reference/jdk17u/test/langtools/tools/javac tools/gen-tree-oracle/samples
//
// The data contains the full TokenKind enumeration (118 values), an all-tokens
// sample, fuzz-free error samples and file samples, plus the observed kind
// coverage and the kinds that a normal scanner run never produces (CUSTOM etc.).

import com.sun.tools.javac.parser.ScannerFactory;
import com.sun.tools.javac.parser.Tokens;
import com.sun.tools.javac.parser.Tokens.TokenKind;
import com.sun.tools.javac.util.Context;
import com.sun.tools.javac.util.Log;
import com.sun.tools.javac.util.Name;
import com.sun.tools.javac.util.Options;

import javax.tools.JavaFileObject;
import javax.tools.SimpleJavaFileObject;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import java.util.stream.Stream;

public final class GenTokenOracle {
    private static final int MAX_TOKENS_PER_SAMPLE = 600;
    private static final long MAX_SOURCE_BYTES = 60_000;

    private static final Set<String> observed = new LinkedHashSet<>();

    public static void main(String[] args) throws Exception {
        if (args.length < 3) {
            System.err.println("usage: GenTokenOracle <outputFile> <maxFilesPerRoot> <root1> [root2 ...]");
            System.exit(2);
        }
        Path outputFile = Path.of(args[0]);
        int maxFiles = Integer.parseInt(args[1]);
        List<Path> roots = new ArrayList<>();
        for (int i = 2; i < args.length; i++) {
            roots.add(Path.of(args[i]));
        }

        Context context = new Context();
        Options options = Options.instance(context);
        options.put("source", "17");
        Log log = Log.instance(context);
        // Diagnostics reported by the lexer (error tokens, invalid unicode escapes)
        // require a source object; the scanner input itself is provided directly.
        log.useSource(new DummySource());
        ScannerFactory scannerFactory = ScannerFactory.instance(context);

        StringBuilder out = new StringBuilder();
        out.append("# token-stream oracle data (development-time generated; tests never start a JVM)\n");
        out.append("# generated-by: tools/gen-token-oracle/GenTokenOracle.java\n");
        out.append("# jdk-version: ").append(System.getProperty("java.version")).append("\n");
        out.append("# baseline: 982d53c (jdk-17.0.19-ga)\n");
        out.append("# scanner: ScannerFactory.newScanner(text, keepDocComments=true); scanner.nextToken()\n");
        out.append("# roots:");
        for (Path root : roots) {
            out.append(' ').append(root);
        }
        out.append('\n');
        out.append("# command: GenTokenOracle ").append(args[0]).append(' ').append(args[1]);
        for (int i = 2; i < args.length; i++) {
            out.append(' ').append(args[i]);
        }
        out.append('\n');

        // 1) Full TokenKind enumeration: 118 values with their text and internal tag.
        out.append("=== TOKENKINDS ===\n");
        TokenKind[] kinds = TokenKind.values();
        out.append("# count: ").append(kinds.length).append('\n');
        for (TokenKind kind : kinds) {
            out.append("KIND ").append(kind.name())
                    .append(" name=").append(kind.name == null ? "-" : kind.name)
                    .append(" tag=").append(kind.tag).append('\n');
        }
        out.append("=== END ===\n");

        // 2) all-tokens sample: every keyword, literal form, operator and comment style.
        List<String> allTokens = new ArrayList<>();
        allTokens.add("abstract"); allTokens.add("assert"); allTokens.add("boolean");
        allTokens.add("break"); allTokens.add("byte"); allTokens.add("case");
        allTokens.add("catch"); allTokens.add("char"); allTokens.add("class");
        allTokens.add("const"); allTokens.add("continue"); allTokens.add("default");
        allTokens.add("do"); allTokens.add("double"); allTokens.add("else");
        allTokens.add("enum"); allTokens.add("extends"); allTokens.add("final");
        allTokens.add("finally"); allTokens.add("float"); allTokens.add("for");
        allTokens.add("goto"); allTokens.add("if"); allTokens.add("implements");
        allTokens.add("import"); allTokens.add("instanceof"); allTokens.add("int");
        allTokens.add("interface"); allTokens.add("long"); allTokens.add("native");
        allTokens.add("new"); allTokens.add("package"); allTokens.add("private");
        allTokens.add("protected"); allTokens.add("public"); allTokens.add("return");
        allTokens.add("short"); allTokens.add("static"); allTokens.add("strictfp");
        allTokens.add("super"); allTokens.add("switch"); allTokens.add("synchronized");
        allTokens.add("this"); allTokens.add("throw"); allTokens.add("throws");
        allTokens.add("transient"); allTokens.add("try"); allTokens.add("void");
        allTokens.add("volatile"); allTokens.add("while"); allTokens.add("true");
        allTokens.add("false"); allTokens.add("null"); allTokens.add("_");
        allTokens.add("identifier");
        allTokens.add("123"); allTokens.add("0x1F"); allTokens.add("0b101");
        allTokens.add("017"); allTokens.add("1.5"); allTokens.add("1.5e3");
        allTokens.add("1L"); allTokens.add("1.5f"); allTokens.add("1.5d");
        allTokens.add("0x1.8p1");
        allTokens.add("'c'"); allTokens.add("\"str\"");
        allTokens.add("->"); allTokens.add("::"); allTokens.add("(");
        allTokens.add(")"); allTokens.add("{"); allTokens.add("}");
        allTokens.add("["); allTokens.add("]"); allTokens.add(";");
        allTokens.add(","); allTokens.add("."); allTokens.add("...");
        allTokens.add("="); allTokens.add(">"); allTokens.add("<");
        allTokens.add("!"); allTokens.add("~"); allTokens.add("?");
        allTokens.add(":"); allTokens.add("=="); allTokens.add("<=");
        allTokens.add(">="); allTokens.add("!="); allTokens.add("&&");
        allTokens.add("||"); allTokens.add("++"); allTokens.add("--");
        allTokens.add("+"); allTokens.add("-"); allTokens.add("*");
        allTokens.add("/"); allTokens.add("&"); allTokens.add("|");
        allTokens.add("^"); allTokens.add("%"); allTokens.add("<<");
        allTokens.add(">>"); allTokens.add(">>>"); allTokens.add("+=");
        allTokens.add("-="); allTokens.add("*="); allTokens.add("/=");
        allTokens.add("&="); allTokens.add("|="); allTokens.add("^=");
        allTokens.add("%="); allTokens.add("<<="); allTokens.add(">>=");
        allTokens.add(">>>="); allTokens.add("@");
        allTokens.add("// line comment\n");
        allTokens.add("/* block comment */");
        allTokens.add("/** javadoc comment */");
        allTokens.add("\"\"\"\n  text block\n  \"\"\"");
        appendSample(out, scannerFactory, "all-tokens", String.join(" ", allTokens));

        // 3) error samples: error tokens produced by the recovery paths.
        appendSample(out, scannerFactory, "error-tokens",
                "int # = ;\n" + "\"unterminated\n" + "'x\n" + "\\u" + "ZZZZ\n");
        // NOTE: "\\u" + "ZZZZ" builds the literal backslash-u-ZZZZ input at runtime;
        // a Java source literal cannot contain an invalid unicode escape directly.

        // 4) file samples: deterministic per-root selection.
        for (Path root : roots) {
            List<Path> files = new ArrayList<>();
            try (Stream<Path> stream = Files.walk(root)) {
                stream.filter(path -> path.toString().endsWith(".java"))
                        .filter(path -> {
                            try {
                                return Files.size(path) <= MAX_SOURCE_BYTES;
                            } catch (Exception e) {
                                return false;
                            }
                        })
                        .forEach(files::add);
            }
            files.sort(Comparator.comparing(Path::toString));
            for (Path file : files.subList(0, Math.min(maxFiles, files.size()))) {
                String text = Files.readString(file, StandardCharsets.UTF_8);
                String relative = root.getFileName() + "/"
                        + root.relativize(file).toString().replace('\\', '/');
                appendSample(out, scannerFactory, relative, text);
            }
        }

        // 5) Coverage summary: observed kinds vs the full enumeration.
        out.append("# observed-kinds: ").append(observed.size()).append('/')
                .append(kinds.length).append('\n');
        out.append("# missing-kinds:");
        for (TokenKind kind : kinds) {
            if (!observed.contains(kind.name())) {
                out.append(' ').append(kind.name());
            }
        }
        out.append('\n');
        if (outputFile.getParent() != null) {
            Files.createDirectories(outputFile.getParent());
        }
        Files.writeString(outputFile, out.toString(), StandardCharsets.UTF_8);
        System.out.println("token oracle: " + observed.size() + "/" + kinds.length
                + " kinds observed -> " + outputFile);
    }

    private static void appendSample(StringBuilder out, ScannerFactory scannerFactory,
                                     String name, String text) {
        out.append("=== SAMPLE: ").append(name).append(" ===\n");
        com.sun.tools.javac.parser.Scanner scanner = scannerFactory.newScanner(text, true);
        scanner.nextToken();
        int count = 0;
        while (scanner.token().kind != TokenKind.EOF) {
            if (count >= MAX_TOKENS_PER_SAMPLE) {
                out.append("# truncated at ").append(MAX_TOKENS_PER_SAMPLE).append(" tokens\n");
                break;
            }
            Tokens.Token token = scanner.token();
            observed.add(token.kind.name());
            out.append("TOKEN ").append(token.kind.name())
                    .append(' ').append(token.pos)
                    .append(' ').append(token.endPos);
            try {
                Name tokenName = token.name();
                if (tokenName != null) {
                    out.append(" name=").append(tokenName);
                }
            } catch (UnsupportedOperationException ignored) {
                // The base token kind carries no name.
            }
            try {
                String stringValue = token.stringVal();
                if (stringValue != null) {
                    out.append(" string=").append(escape(stringValue));
                }
            } catch (UnsupportedOperationException ignored) {
                // The base token kind carries no string value.
            }
            try {
                int radix = token.radix();
                if (radix != 0) {
                    out.append(" radix=").append(radix);
                }
            } catch (UnsupportedOperationException ignored) {
                // The base token kind carries no radix.
            }
            out.append('\n');
            count++;
            scanner.nextToken();
        }
        out.append("# tokens: ").append(count).append('\n');
        out.append("=== END ===\n");
    }

    private static final class DummySource extends SimpleJavaFileObject {
        DummySource() {
            super(URI.create("file:///Dummy.java"), JavaFileObject.Kind.SOURCE);
        }

        @Override
        public CharSequence getCharContent(boolean ignoreEncodingErrors) {
            return "";
        }
    }

    private static String escape(String value) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < value.length(); i++) {
            char ch = value.charAt(i);
            switch (ch) {
                case '\\' -> sb.append("\\\\");
                case '\n' -> sb.append("\\n");
                case '\r' -> sb.append("\\r");
                case '\t' -> sb.append("\\t");
                default -> sb.append(ch);
            }
        }
        return sb.toString();
    }
}
