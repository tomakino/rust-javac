// Development-time Attr type-attribution oracle for the Rust port tests.
//
// Compiles the sample source with the baseline JDK 17.0.19 (public
// javax.tools/com.sun.source API: parse + analyze), then for every type tree
// in the analyzed AST prints the attributed type encoded structurally:
//
//   PRIM:<TypeKind>            primitive types
//   ARRAY[<encoded component>] arrays
//   DECL:<qualified name>      declared types (with <arg,...> type arguments)
//   TYPEVAR:<name>             type variables
//   WILD:<...>                 wildcards
//   ERROR                      error types
//
// The encoded text is compared by the Rust attribution oracle test
// (crates/javac-core/src/comp/attr.rs tests, zero JVM at runtime) after the
// ported parser + Enter + Attr pipeline attributes the same type trees.
//
// Output: crates/javac-core/tests/data/attr-oracle.txt

import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.ClassTree;
import com.sun.source.tree.MethodTree;
import com.sun.source.tree.Tree;
import com.sun.source.tree.VariableTree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.TreePath;
import com.sun.source.util.TreePathScanner;
import com.sun.source.util.Trees;

import javax.lang.model.type.ArrayType;
import javax.lang.model.type.DeclaredType;
import javax.lang.model.type.TypeKind;
import javax.lang.model.type.TypeMirror;
import javax.lang.model.element.TypeElement;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.SimpleJavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;
import java.net.URI;
import java.util.ArrayList;
import java.util.List;

public class GenAttrOracle {
    static final String SAMPLE = String.join("\n",
        "package p;",
        "import java.util.List;",
        "class Sample {",
        "    int f1;",
        "    int[] f2;",
        "    String f3;",
        "    String[] f4;",
        "    List<String> f5;",
        "    List<String[]> f6;",
        "    List<List<String>> f7;",
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
        List<String> options = List.of("-proc:none");
        JavacTask task = (JavacTask) compiler.getTask(null, fm, null, options, null, files);
        Iterable<? extends CompilationUnitTree> units = task.parse();
        task.analyze();
        Trees trees = Trees.instance(task);

        StringBuilder out = new StringBuilder();
        out.append("SAMPLE Sample.java\n");
        final int[] sequence = {0};
        for (CompilationUnitTree unit : units) {
            for (Tree decl : unit.getTypeDecls()) {
                if (!(decl instanceof ClassTree classTree)) {
                    continue;
                }
                // Walk the member declarations in source order; the Rust test
                // collects the same type trees from the parsed members.
                for (Tree member : classTree.getMembers()) {
                    if (member instanceof VariableTree varTree) {
                        TypeMirror type = trees.getTypeMirror(TreePath.getPath(unit, varTree.getType()));
                        out.append("TYPE ").append(sequence[0]++).append(" ")
                                .append(encode(type)).append("\n");
                    } else if (member instanceof MethodTree methodTree) {
                        if (methodTree.getReturnType() != null) {
                            TypeMirror type = trees.getTypeMirror(
                                    TreePath.getPath(unit, methodTree.getReturnType()));
                            out.append("TYPE ").append(sequence[0]++).append(" ")
                                    .append(encode(type)).append("\n");
                        }
                        for (VariableTree param : methodTree.getParameters()) {
                            TypeMirror type = trees.getTypeMirror(
                                    TreePath.getPath(unit, param.getType()));
                            out.append("TYPE ").append(sequence[0]++).append(" ")
                                    .append(encode(type)).append("\n");
                        }
                    }
                }
            }
        }
        String target = args.length == 0
                ? "crates/javac-core/tests/data/attr-oracle.txt"
                : args[0];
        java.nio.file.Files.writeString(java.nio.file.Path.of(target), out.toString());
        System.out.println("gen-attr-oracle: wrote " + target + " (" + out.length() + " chars)");
        fm.close();
    }

    static String encode(TypeMirror type) {
        if (type == null) {
            return "<null>";
        }
        switch (type.getKind()) {
            case ARRAY:
                return "ARRAY[" + encode(((ArrayType) type).getComponentType()) + "]";
            case DECLARED: {
                DeclaredType declared = (DeclaredType) type;
                TypeElement element = (TypeElement) declared.asElement();
                StringBuilder builder = new StringBuilder("DECL:")
                        .append(element.getQualifiedName());
                List<? extends TypeMirror> arguments = declared.getTypeArguments();
                if (!arguments.isEmpty()) {
                    builder.append("<");
                    for (int i = 0; i < arguments.size(); i++) {
                        if (i > 0) {
                            builder.append(",");
                        }
                        builder.append(encode(arguments.get(i)));
                    }
                    builder.append(">");
                }
                return builder.toString();
            }
            case TYPEVAR:
                return "TYPEVAR:" + type.toString();
            case WILDCARD:
                return "WILD:" + type.toString();
            case ERROR:
                return "ERROR";
            default:
                if (type.getKind().isPrimitive()) {
                    return "PRIM:" + type.getKind();
                }
                return type.getKind() + ":" + type.toString();
        }
    }
}
