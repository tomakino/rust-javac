import com.sun.tools.javac.api.BasicJavacTask;
import com.sun.tools.javac.code.Type;
import com.sun.tools.javac.code.Types;
import com.sun.tools.javac.tree.JCTree;
import com.sun.tools.javac.tree.JCTree.JCClassDecl;
import com.sun.tools.javac.tree.JCTree.JCVariableDecl;
import com.sun.tools.javac.util.Context;

import javax.tools.*;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;

public class GenTypesOracle {
    public static void main(String[] args) throws Exception {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        StandardJavaFileManager fm = compiler.getStandardFileManager(null, null, null);
        com.sun.source.util.JavacTask task = (com.sun.source.util.JavacTask) compiler.getTask(
                null, fm, null, List.of("-proc:none"), null, fm.getJavaFileObjects(args[0]));
        Iterable<? extends com.sun.source.tree.CompilationUnitTree> parsed = task.parse();
        task.analyze();
        Context context = ((BasicJavacTask) task).getContext();
        Types types = Types.instance(context);
        Map<String, Type> byName = new LinkedHashMap<>();
        for (com.sun.source.tree.CompilationUnitTree unit : parsed) {
            for (com.sun.source.tree.Tree decl : unit.getTypeDecls()) {
                if (decl instanceof JCClassDecl cd) {
                    for (JCTree member : cd.getMembers()) {
                        if (member instanceof JCVariableDecl vd) {
                            byName.put(vd.name.toString(), vd.sym.type);
                        }
                    }
                }
            }
        }
        TreeSet<String> names = new TreeSet<>(byName.keySet());
        String nl = System.lineSeparator();
        StringBuilder sb = new StringBuilder();
        for (String a : names) {
            for (String b : names) {
                sb.append("SAME ").append(a).append(" ").append(b).append(" ")
                  .append(types.isSameType(byName.get(a), byName.get(b))).append(nl);
            }
        }
        for (String a : names) {
            sb.append("ERASE ").append(a).append(" ").append(types.erasure(byName.get(a))).append(nl);
        }
        System.out.print(sb);
    }
}