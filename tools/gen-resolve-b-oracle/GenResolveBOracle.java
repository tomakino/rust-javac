// Development-time Resolve B oracle for the Rust port tests.
//
// Uses the baseline JDK 17.0.19 internal javac API to sample:
//   1. Resolve.MethodResolutionPhase.values() behavior (isBoxingRequired /
//      isVarargsRequired / ordinal), directly compared by the Rust test
//      (crates/javac-core/tests/resolve_b_oracle.rs, zero JVM at test time);
//   2. Overload-resolution outcomes for a sample source (the chosen method
//      element and the javac diagnostic keys), captured here so the data is
//      available for end-to-end replay once the Attr batch lands. The Rust
//      test records those rows as SKIPPED (registration: the end-to-end
//      comparison needs Attr.attribInvocation wiring).
//
// Output format:
//   ORACLE resolve-b
//   PHASE <name> <isBoxingRequired> <isVarargsRequired> <ordinal>
//   CALL <snippet> -> <owner>.<name>(<param types>)
//   ERROR <diagnostic-code>
package com.sun.tools.javac.comp;

import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.tree.Tree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.TreePath;
import com.sun.source.util.TreePathScanner;
import com.sun.source.util.Trees;

import javax.lang.model.element.Element;
import javax.lang.model.element.ExecutableElement;
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
import java.util.List;

public class GenResolveBOracle {
    static final String SAMPLE = String.join("\n",
        "class Sample {",
        "    void f(int x) {}",
        "    void f(String s) {}",
        "    void g() {",
        "        f(1);",
        "        f(\"a\");",
        "        f(1.0);",
        "    }",
        "    void h(long l) {}",
        "    void h(int i) {}",
        "    void k() { h(1); }",
        "    void boxed(Integer v) {}",
        "    void boxed(long v) {}",
        "    void k2() { boxed(1); }",
        "}",
        "");

    public static void main(String[] args) throws Exception {
        StringBuilder out = new StringBuilder();
        out.append("ORACLE resolve-b\n");
        for (Resolve.MethodResolutionPhase phase : Resolve.MethodResolutionPhase.values()) {
            out.append("PHASE ").append(phase.name())
               .append(' ').append(phase.isBoxingRequired())
               .append(' ').append(phase.isVarargsRequired())
               .append(' ').append(phase.ordinal())
               .append('\n');
        }

        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        StandardJavaFileManager fm = compiler.getStandardFileManager(null, null, null);
        JavaFileObject file = new SimpleJavaFileObject(
                URI.create("string:///Sample.java"), JavaFileObject.Kind.SOURCE) {
            @Override
            public CharSequence getCharContent(boolean ignoreEncodingErrors) {
                return SAMPLE;
            }
        };
        List<JavaFileObject> files = new ArrayList<>();
        files.add(file);
        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
        JavacTask task = (JavacTask) compiler.getTask(null, fm, diagnostics,
                List.of("-proc:none"), null, files);
        Iterable<? extends CompilationUnitTree> units = task.parse();
        task.analyze();
        Trees trees = Trees.instance(task);

        for (CompilationUnitTree unit : units) {
            new TreePathScanner<Void, Void>() {
                @Override
                public Void visitMethodInvocation(MethodInvocationTree node, Void p) {
                    TreePath path = getCurrentPath();
                    Element element = trees.getElement(path);
                    String snippet = node.toString().replace('\n', ' ');
                    if (element instanceof ExecutableElement method) {
                        Element owner = method.getEnclosingElement();
                        String ownerName = owner instanceof TypeElement type
                                ? type.getQualifiedName().toString()
                                : String.valueOf(owner);
                        out.append("CALL ").append(snippet)
                           .append(" -> ").append(ownerName)
                           .append('.').append(method.getSimpleName())
                           .append('(').append(method.getParameters()).append(')')
                           .append('\n');
                    } else {
                        out.append("CALL ").append(snippet).append(" -> none\n");
                    }
                    return super.visitMethodInvocation(node, p);
                }
            }.scan(unit, null);
        }
        for (Diagnostic<? extends JavaFileObject> diagnostic : diagnostics.getDiagnostics()) {
            out.append("ERROR ").append(diagnostic.getCode()).append('\n');
        }
        System.out.print(out);
    }
}
