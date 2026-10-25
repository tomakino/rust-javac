// Development-time ENTER-stage symbol oracle generator for the Rust port tests.
//
// Uses the public javax.tools / com.sun.source API of the baseline JDK 17.0.19
// (no --add-exports): parse + analyze, then print the TypeElement structure
// (kind/name/qualified name/nesting/modifiers and nested type elements) and the
// collected diagnostics per sample.
//
// The written text is checked in as crates/javac-core/tests/data/enter-oracle.txt;
// the Rust tests never invoke a JVM.

import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.Tree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.TreePath;
import com.sun.source.util.Trees;

import javax.lang.model.element.Element;
import javax.lang.model.element.ElementKind;
import javax.lang.model.element.Modifier;
import javax.lang.model.element.NestingKind;
import javax.lang.model.element.TypeElement;
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
import java.util.Locale;
import java.util.Map;
import java.util.stream.Collectors;

public class GenEnterOracle {
    static final Map<String, String> SOURCES = new LinkedHashMap<>();

    static {
        SOURCES.put("Sample.java", String.join("\n",
            "package p;",
            "class A {",
            "    class B {",
            "        class C {}",
            "    }",
            "    interface I {",
            "        class Nested {}",
            "    }",
            "    int f;",
            "    void m() {}",
            "}",
            "interface Top {}",
            ""));
        SOURCES.put("Unnamed.java", String.join("\n",
            "class U {",
            "    enum E { X, Y }",
            "}",
            ""));
        SOURCES.put("PublicMismatch.java", String.join("\n",
            "public class Foo {",
            "}",
            ""));
        SOURCES.put("Duplicate.java", String.join("\n",
            "package d;",
            "class D {}",
            "class D {}",
            ""));
    }

    public static void main(String[] args) throws Exception {
        Locale.setDefault(Locale.ENGLISH);
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        for (Map.Entry<String, String> entry : SOURCES.entrySet()) {
            runSample(compiler, entry.getKey(), entry.getValue());
        }
    }

    static void runSample(JavaCompiler compiler, String name, String text) throws Exception {
        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
        StandardJavaFileManager fm = compiler.getStandardFileManager(diagnostics, Locale.ENGLISH, null);
        List<JavaFileObject> files = new ArrayList<>();
        files.add(new SimpleJavaFileObject(URI.create("string:///" + name), JavaFileObject.Kind.SOURCE) {
            @Override
            public CharSequence getCharContent(boolean ignoreEncodingErrors) {
                return text;
            }
        });
        JavacTask task = (JavacTask) compiler.getTask(
            null, fm, diagnostics, List.of("-proc:none"), null, files);
        Iterable<? extends CompilationUnitTree> units = task.parse();
        task.analyze();
        long errors = diagnostics.getDiagnostics().stream()
            .filter(d -> d.getKind() == Diagnostic.Kind.ERROR).count();
        System.out.println("SAMPLE " + name + " errors=" + errors);
        for (Diagnostic<? extends JavaFileObject> d : diagnostics.getDiagnostics()) {
            if (d.getKind() == Diagnostic.Kind.ERROR) {
                System.out.println("ERROR " + escape(d.getMessage(Locale.ENGLISH)));
            }
        }
        Trees trees = Trees.instance(task);
        for (CompilationUnitTree unit : units) {
            String pkg = unit.getPackageName() == null ? "" : unit.getPackageName().toString();
            System.out.println("UNIT " + name + " package=" + pkg);
            for (Tree decl : unit.getTypeDecls()) {
                printType(trees, TreePath.getPath(unit, decl), 0);
            }
        }
        fm.close();
    }

    static void printType(Trees trees, TreePath path, int depth) {
        Element element = trees.getElement(path);
        if (!(element instanceof TypeElement te)) {
            return;
        }
        StringBuilder indent = new StringBuilder();
        for (int i = 0; i < depth; i++) indent.append("  ");
        String modifiers = te.getModifiers().stream().map(Modifier::toString)
            .sorted().collect(Collectors.joining(","));
        NestingKind nesting = te.getNestingKind();
        ElementKind kind = te.getKind();
        String owner = ownerKind(element);
        System.out.println(indent + "TYPE flatname=" + te.getQualifiedName()
            + " name=" + te.getSimpleName()
            + " kind=" + kind
            + " nesting=" + nesting
            + " owner=" + owner
            + " modifiers=[" + modifiers + "]");
        if (path.getLeaf() instanceof com.sun.source.tree.ClassTree ct) {
            for (Tree member : ct.getMembers()) {
                if (member instanceof com.sun.source.tree.ClassTree) {
                    printType(trees, new TreePath(path, member), depth + 1);
                }
            }
        }
    }

    static String ownerKind(Element element) {
        Element owner = element.getEnclosingElement();
        if (owner == null) return "<none>";
        return owner.getKind() + ":" + owner;
    }

    static String escape(String text) {
        return text.replace("\n", "\\n").replace("\r", "");
    }
}
