// Source: tools/gen-tree-oracle/GenTreeOracle.java (development-time oracle tool).
// Purpose: generate Pretty-printed parse trees from the baseline JDK 17 javac for
// the rust-javac tree layer tests. The generated data file is checked into the
// repository; the tests never start a JVM.
//
// Usage (development time only; see tools/gen-tree-oracle/README.md):
//   javac --add-exports ... -d target/gen-tree-oracle-classes tools/gen-tree-oracle/GenTreeOracle.java
//   java --add-exports ... -cp target/gen-tree-oracle-classes GenTreeOracle \
//        crates/javac-tests/data/tree-oracle.txt 40 \
//        reference/jdk17u/test/langtools/tools/javac tools/gen-tree-oracle/samples
//
// The output records the JDK version, the baseline commit, the generation command,
// the roots and the Pretty print mode so the data can be audited.

import com.sun.source.util.JavacTask;
import com.sun.tools.javac.api.JavacTool;
import com.sun.tools.javac.tree.JCTree;
import com.sun.tools.javac.tree.Pretty;

import javax.tools.DiagnosticCollector;
import javax.tools.JavaFileObject;
import javax.tools.StandardJavaFileManager;
import java.io.StringWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.stream.Stream;

public final class GenTreeOracle {
    private static final long MAX_SOURCE_BYTES = 60_000;

    public static void main(String[] args) throws Exception {
        if (args.length < 3) {
            System.err.println("usage: GenTreeOracle <outputFile> <maxRecordsPerRoot> <root1> [root2 ...]");
            System.exit(2);
        }
        Path outputFile = Path.of(args[0]);
        int maxRecords = Integer.parseInt(args[1]);
        List<Path> roots = new ArrayList<>();
        for (int i = 2; i < args.length; i++) {
            roots.add(Path.of(args[i]));
        }

        // Deterministic selection: per root, recursively collected .java files sorted
        // by path, limited to maxRecords per root.
        List<Path[]> selection = new ArrayList<>();
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
            for (Path file : files.subList(0, Math.min(maxRecords, files.size()))) {
                selection.add(new Path[]{root, file});
            }
        }

        JavacTool tool = JavacTool.create();
        StringBuilder out = new StringBuilder();
        out.append("# tree-pretty oracle data (development-time generated; tests never start a JVM)\n");
        out.append("# generated-by: tools/gen-tree-oracle/GenTreeOracle.java\n");
        out.append("# jdk-version: ").append(System.getProperty("java.version")).append("\n");
        out.append("# baseline: 982d53c (jdk-17.0.19-ga)\n");
        out.append("# print-mode: new Pretty(writer, false).printUnit(unit, null)\n");
        out.append("# parse-mode: JavacTask.parse() with -proc:none -XDshould-stop.at=PARSE\n");
        out.append("# doc-comments: kept (JavacTask.parse sets keepComments=true; Pretty(false) prints doc comments)\n");
        out.append("# roots:");
        for (Path root : roots) {
            out.append(' ').append(root);
        }
        out.append('\n');
        out.append("# command: GenTreeOracle ").append(args[0]).append(' ').append(args[1]);
        for (int i = 2; i < args.length; i++) {
            out.append(' ').append(args[i]);
        }
        out.append('\n');

        int written = 0;
        int skipped = 0;
        try (StandardJavaFileManager fileManager =
                     tool.getStandardFileManager(null, null, null)) {
            for (Path[] entry : selection) {
                Path root = entry[0];
                Path file = entry[1];
                try {
                    DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
                    List<String> options = List.of("-proc:none", "-XDshould-stop.at=PARSE");
                    JavacTask task = tool.getTask(null, fileManager, diagnostics, options, null,
                            fileManager.getJavaFileObjects(file));
                    // Parse only: no class files are generated.
                    Iterable<? extends com.sun.source.tree.CompilationUnitTree> units = task.parse();
                    String pretty = null;
                    for (com.sun.source.tree.CompilationUnitTree unit : units) {
                        StringWriter writer = new StringWriter();
                        new Pretty(writer, false).printUnit((JCTree.JCCompilationUnit) unit, null);
                        pretty = writer.toString();
                        break;
                    }
                    if (pretty == null) {
                        skipped++;
                        continue;
                    }
                    String relative = root.getFileName() + "/"
                            + root.relativize(file).toString().replace('\\', '/');
                    out.append("=== FILE: ").append(relative).append(" ===\n");
                    out.append(pretty);
                    if (!pretty.endsWith("\n")) {
                        out.append('\n');
                    }
                    out.append("=== END ===\n");
                    written++;
                } catch (Throwable t) {
                    skipped++;
                }
            }
        }
        out.append("# records: ").append(written).append('\n');
        out.append("# skipped: ").append(skipped).append('\n');
        if (outputFile.getParent() != null) {
            Files.createDirectories(outputFile.getParent());
        }
        Files.writeString(outputFile, out.toString(), StandardCharsets.UTF_8);
        System.out.println("tree oracle: " + written + " records, " + skipped
                + " skipped -> " + outputFile);
    }
}
