// Development-time Resolve error-key oracle for the Rust port tests.
//
// Compiles the sample sources with the baseline JDK 17.0.19 (public
// javax.tools API) and collects the compiler diagnostic codes (keys) emitted
// for each erroneous sample. The Rust test (comp/resolve.rs, zero JVM at
// runtime) triggers the corresponding ResolveError diagnostics through the
// ported Resolve and compares the diagnostic keys.
//
// Output: ERR <sample> <code>

import javax.tools.Diagnostic;
import javax.tools.DiagnosticCollector;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.SimpleJavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;
import java.net.URI;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public class GenResolveErrorOracle {
    static final Map<String, String> SAMPLES = new LinkedHashMap<>();
    static {
        SAMPLES.put("E1", "class E1 { int m() { return unknownVar; } }");
        SAMPLES.put("E2", "class E2 { void m() { UnknownType x = null; } }");
        SAMPLES.put("E3", "class E3 { int f; static int m() { return f; } }");
        SAMPLES.put("E4", "class E4 { private int f; } class E4User { int m(E4 e) { return e.f; } }");
    }

    public static void main(String[] args) throws Exception {
        StringBuilder out = new StringBuilder();
        for (Map.Entry<String, String> entry : SAMPLES.entrySet()) {
            out.append("SAMPLE ").append(entry.getKey()).append('\n');
            for (String code : compileAndCollect(entry.getValue())) {
                out.append("ERR ").append(entry.getKey()).append(' ').append(code).append('\n');
            }
        }
        String target = args.length == 0
                ? "crates/javac-core/tests/data/resolve-error-oracle.txt"
                : args[0];
        java.nio.file.Files.writeString(java.nio.file.Path.of(target), out.toString());
        System.out.println("gen-resolve-error-oracle: wrote " + target + " (" + out.length() + " chars)");
    }

    static List<String> compileAndCollect(String source) throws Exception {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        StandardJavaFileManager fm = compiler.getStandardFileManager(null, null, null);
        JavaFileObject file = new SimpleJavaFileObject(
                URI.create("string:///Sample.java"), JavaFileObject.Kind.SOURCE) {
            @Override
            public CharSequence getCharContent(boolean ignoreEncodingErrors) {
                return source;
            }
        };
        DiagnosticCollector<JavaFileObject> collector = new DiagnosticCollector<>();
        List<JavaFileObject> files = new ArrayList<>();
        files.add(file);
        JavaCompiler.CompilationTask task = compiler.getTask(
                null, fm, collector, List.of("-proc:none"), null, files);
        task.call();
        List<String> codes = new ArrayList<>();
        for (Diagnostic<? extends JavaFileObject> diagnostic : collector.getDiagnostics()) {
            if (diagnostic.getKind() == Diagnostic.Kind.ERROR) {
                codes.add(diagnostic.getCode());
            }
        }
        fm.close();
        return codes;
    }
}
