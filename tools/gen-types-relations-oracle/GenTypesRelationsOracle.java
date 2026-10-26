// Development-time Types-relations oracle for the Rust port tests.
//
// Uses the baseline JDK 17.0.19 internal javac API (com.sun.tools.javac.code.*)
// to compute the exact isSubtype/isConvertible/isCastable/isParameterized
// results for a set of sample types, then writes them to the oracle data file.
// The Rust test (code/types.rs, zero JVM at runtime) rebuilds the same types
// through the ported Symtab/Enter and compares.
//
// Run with:
//   java --add-exports jdk.compiler/com.sun.tools.javac.code=ALL-UNNAMED \
//        --add-exports jdk.compiler/com.sun.tools.javac.util=ALL-UNNAMED \
//        --add-exports jdk.compiler/com.sun.tools.javac.api=ALL-UNNAMED \
//        --add-exports jdk.compiler/com.sun.tools.javac.tree=ALL-UNNAMED \
//        tools/gen-types-relations-oracle/GenTypesRelationsOracle.java <out>

import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.ClassTree;
import com.sun.source.tree.Tree;
import com.sun.source.tree.VariableTree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.TreePath;
import com.sun.source.util.Trees;

import com.sun.tools.javac.api.JavacTaskImpl;
import com.sun.tools.javac.code.Type;
import com.sun.tools.javac.code.Types;
import com.sun.tools.javac.util.Context;

import javax.lang.model.type.TypeMirror;
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

public class GenTypesRelationsOracle {
    static final String SAMPLE = String.join("\n",
        "import java.util.*;",
        "class RelSample {",
        "    int i;",
        "    long l;",
        "    String s;",
        "    Object o;",
        "    List<String> ls;",
        "    List<Object> lo;",
        "    List<?> lu;",
        "    List<? extends String> les;",
        "    List<? super String> lss;",
        "    String[] sa;",
        "    int[] ia;",
        "    Object[] oa;",
        "    class Inner {}",
        "    Inner inner;",
        "}",
        "");

    public static void main(String[] args) throws Exception {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        StandardJavaFileManager fm = compiler.getStandardFileManager(null, null, null);
        JavaFileObject file = new SimpleJavaFileObject(
                URI.create("string:///RelSample.java"), JavaFileObject.Kind.SOURCE) {
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
        Context context = ((JavacTaskImpl) task).getContext();
        Types types = Types.instance(context);

        Map<String, Type> byName = new LinkedHashMap<>();
        for (CompilationUnitTree unit : units) {
            for (Tree decl : unit.getTypeDecls()) {
                if (!(decl instanceof ClassTree classTree)) {
                    continue;
                }
                for (Tree member : classTree.getMembers()) {
                    if (member instanceof VariableTree varTree) {
                        TypeMirror mirror = trees.getTypeMirror(
                                TreePath.getPath(unit, varTree.getType()));
                        if (mirror instanceof Type type) {
                            byName.put(varTree.getName().toString(), type);
                        }
                    }
                }
            }
        }

        StringBuilder out = new StringBuilder();
        out.append("SAMPLE RelSample\n");
        for (Map.Entry<String, Type> entry : byName.entrySet()) {
            out.append("PARAM ").append(entry.getKey()).append(' ')
               .append(entry.getValue().isParameterized() ? "true" : "false").append('\n');
        }
        String[][] subPairs = {
            {"i", "l"}, {"i", "i"}, {"i", "s"}, {"s", "o"}, {"ls", "lo"}, {"ls", "lu"},
            {"ls", "les"}, {"ls", "lss"}, {"ls", "o"}, {"sa", "oa"}, {"sa", "o"},
            {"ia", "oa"}, {"ia", "o"}, {"inner", "inner"}, {"les", "ls"}, {"lss", "ls"},
        };
        for (String[] pair : subPairs) {
            out.append("SUB ").append(pair[0]).append(' ').append(pair[1]).append(' ')
               .append(types.isSubtype(byName.get(pair[0]), byName.get(pair[1])) ? "true" : "false")
               .append('\n');
        }
        String[][] convPairs = {
            {"i", "l"}, {"l", "i"}, {"i", "s"}, {"s", "o"}, {"ls", "lo"}, {"ls", "lu"},
            {"sa", "oa"}, {"ia", "oa"}, {"i", "s"}, {"s", "i"},
        };
        for (String[] pair : convPairs) {
            out.append("CONV ").append(pair[0]).append(' ').append(pair[1]).append(' ')
               .append(types.isConvertible(byName.get(pair[0]), byName.get(pair[1])) ? "true" : "false")
               .append('\n');
        }
        String[][] castPairs = {
            {"s", "o"}, {"ls", "lo"}, {"ls", "lu"}, {"ia", "sa"}, {"sa", "ia"},
            {"i", "l"}, {"inner", "o"},
        };
        for (String[] pair : castPairs) {
            out.append("CAST ").append(pair[0]).append(' ').append(pair[1]).append(' ')
               .append(types.isCastable(byName.get(pair[0]), byName.get(pair[1])) ? "true" : "false")
               .append('\n');
        }
        String target = args.length == 0
                ? "crates/javac-core/tests/data/types-relations-oracle.txt"
                : args[0];
        java.nio.file.Files.writeString(java.nio.file.Path.of(target), out.toString());
        System.out.println("gen-types-relations-oracle: wrote " + target + " (" + out.length() + " chars)");
        fm.close();
    }
}
