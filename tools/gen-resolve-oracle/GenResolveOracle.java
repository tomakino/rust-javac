// Development-time Resolve oracle for the Rust port tests.
//
// Compiles the sample source with the baseline JDK 17.0.19 (public
// javax.tools/com.sun.source API: parse + analyze) and prints, for every
// identifier reference inside the method bodies, the resolved symbol name, kind
// and qualified owner. The Rust test (comp/resolve.rs, zero JVM at runtime)
// runs the ported Enter + Resolve.findIdentInType on the same source and
// compares the symbol name/kind/owner.
//
// Output format (one line per reference):
//   REF <name> <kind> <owner-qualified-name>

import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.ClassTree;
import com.sun.source.tree.IdentifierTree;
import com.sun.source.tree.MethodTree;
import com.sun.source.tree.Tree;
import com.sun.source.tree.VariableTree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.TreePath;
import com.sun.source.util.TreePathScanner;
import com.sun.source.util.Trees;

import javax.lang.model.element.Element;
import javax.lang.model.element.ElementKind;
import javax.lang.model.element.TypeElement;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.SimpleJavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;
import java.net.URI;
import java.util.ArrayList;
import java.util.List;

public class GenResolveOracle {
    static final String SAMPLE = String.join("\n",
        "class Sample {",
        "    int f1;",
        "    String f2;",
        "    static int f3;",
        "    class Inner {}",
        "    int m() {",
        "        return f1 + f3;",
        "    }",
        "    Inner m2() {",
        "        return null;",
        "    }",
        "}",
        "");

    public static void main(String[] args) throws Exception {
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
        JavacTask task = (JavacTask) compiler.getTask(null, fm, null, List.of("-proc:none"), null, files);
        Iterable<? extends CompilationUnitTree> units = task.parse();
        task.analyze();
        Trees trees = Trees.instance(task);

        StringBuilder out = new StringBuilder();
        out.append("SAMPLE Sample.java\n");
        for (CompilationUnitTree unit : units) {
            for (Tree decl : unit.getTypeDecls()) {
                if (!(decl instanceof ClassTree classTree)) {
                    continue;
                }
                new TreePathScanner<Void, Void>() {
                    @Override
                    public Void visitMethod(MethodTree node, Void p) {
                        return super.visitMethod(node, p);
                    }

                    @Override
                    public Void visitVariable(VariableTree node, Void p) {
                        return super.visitVariable(node, p);
                    }

                    @Override
                    public Void visitIdentifier(IdentifierTree node, Void p) {
                        if (node.getName().contentEquals("f1") || node.getName().contentEquals("f2")
                                || node.getName().contentEquals("f3") || node.getName().contentEquals("Inner")) {
                            Element element = trees.getElement(getCurrentPath());
                            if (element != null) {
                                out.append("REF ").append(element.getSimpleName());
                                out.append(' ').append(element.getKind());
                                out.append(' ').append(ownerName(element));
                                out.append('\n');
                            }
                        }
                        return super.visitIdentifier(node, p);
                    }
                }.scan(new TreePath(new TreePath(unit), classTree), null);
            }
        }
        String target = args.length == 0
                ? "crates/javac-core/tests/data/resolve-oracle.txt"
                : args[0];
        java.nio.file.Files.writeString(java.nio.file.Path.of(target), out.toString());
        System.out.println("gen-resolve-oracle: wrote " + target + " (" + out.length() + " chars)");
        fm.close();
    }

    static String ownerName(Element element) {
        Element enclosing = element.getEnclosingElement();
        if (enclosing instanceof TypeElement type) {
            return type.getQualifiedName().toString();
        }
        return enclosing == null ? "" : enclosing.getSimpleName().toString();
    }
}
