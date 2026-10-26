// Development-time inference oracle for the Rust port tests.
//
// Uses the baseline JDK 17.0.19 internal javac API to run InferenceContext.solve
// on a set of hand-built constraints (the same constraints the Rust tests
// rebuild) and writes the instantiated type names to the oracle data file.
// The tool lives in com.sun.tools.javac.comp so that the package-private
// InferenceContext.instTypes() can be read like the javac tests do.
//
// Run with:
//   java --add-exports jdk.compiler/com.sun.tools.javac.code=ALL-UNNAMED \
//        --add-exports jdk.compiler/com.sun.tools.javac.util=ALL-UNNAMED \
//        --add-exports jdk.compiler/com.sun.tools.javac.api=ALL-UNNAMED \
//        --add-exports jdk.compiler/com.sun.tools.javac.comp=ALL-UNNAMED \
//        --add-exports jdk.compiler/com.sun.tools.javac.tree=ALL-UNNAMED \
//        tools/gen-infer-oracle/GenInferOracle.java <out>

package com.sun.tools.javac.comp;

import com.sun.source.util.JavacTask;
import com.sun.tools.javac.api.JavacTaskImpl;
import com.sun.tools.javac.code.Flags;
import com.sun.tools.javac.code.Symtab;
import com.sun.tools.javac.code.Type;
import com.sun.tools.javac.code.Type.TypeVar;
import com.sun.tools.javac.code.Type.UndetVar;
import com.sun.tools.javac.code.Type.UndetVar.InferenceBound;
import com.sun.tools.javac.code.Types;
import com.sun.tools.javac.util.Context;
import com.sun.tools.javac.util.List;
import com.sun.tools.javac.util.Names;
import com.sun.tools.javac.util.Warner;

import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.SimpleJavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;
import java.net.URI;
import java.util.ArrayList;

public class GenInferOracle {

    static Context context;
    static Symtab syms;
    static Types types;
    static Names names;
    static Infer infer;
    static StringBuilder out = new StringBuilder();

    static TypeVar newTvar(String name, long flags, Type bound) {
        com.sun.tools.javac.code.Symbol.TypeVariableSymbol tsym =
                new com.sun.tools.javac.code.Symbol.TypeVariableSymbol(
                        flags, names.fromString(name), null, syms.noSymbol);
        TypeVar tv = new TypeVar(tsym, bound, syms.botType);
        tsym.type = tv;
        return tv;
    }

    static void scenario(String label, String expected, TypeVar tvar, InferenceBound ib, Type bound) {
        try {
            InferenceContext ic = new InferenceContext(infer, List.of(tvar));
            UndetVar uv = (UndetVar) ic.asUndetVar(tvar);
            uv.addBound(ib, bound, types);
            Warner warn = new Warner();
            ic.solve(warn);
            List<Type> insts = ic.instTypes();
            Type inst = insts.head;
            String actual = inst.tsym != null ? inst.tsym.getQualifiedName().toString() : String.valueOf(inst);
            out.append("SCENARIO ").append(label).append(" EXPECT ").append(expected)
               .append(" ACTUAL ").append(actual).append('\n');
        } catch (Throwable t) {
            out.append("SCENARIO ").append(label).append(" EXPECT ").append(expected)
               .append(" ACTUAL <error:").append(t.getClass().getSimpleName()).append(">")
               .append('\n');
        }
    }

    public static void main(String[] args) throws Exception {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        StandardJavaFileManager fm = compiler.getStandardFileManager(null, null, null);
        JavaFileObject file = new SimpleJavaFileObject(
                URI.create("string:///InferSample.java"), JavaFileObject.Kind.SOURCE) {
            @Override
            public CharSequence getCharContent(boolean ignoreEncodingErrors) {
                return "class InferSample {}";
            }
        };
        ArrayList<JavaFileObject> files = new ArrayList<>();
        files.add(file);
        JavacTask task = (JavacTask) compiler.getTask(null, fm, null, List.of("-proc:none"), null, files);
        task.parse();
        task.analyze();
        context = ((JavacTaskImpl) task).getContext();
        syms = Symtab.instance(context);
        types = Types.instance(context);
        names = Names.instance(context);
        infer = Infer.instance(context);

        out.append("ORACLE infer-constraints\n");
        scenario("eq_string", "java.lang.String", newTvar("T", 0, syms.objectType),
                InferenceBound.EQ, syms.stringType);
        scenario("lower_string", "java.lang.String", newTvar("T", 0, syms.objectType),
                InferenceBound.LOWER, syms.stringType);
        scenario("upper_string", "java.lang.String", newTvar("T", 0, syms.objectType),
                InferenceBound.UPPER, syms.stringType);
        scenario("upper_object", "java.lang.Object", newTvar("T", 0, syms.objectType),
                InferenceBound.UPPER, syms.objectType);
        scenario("throws_runtime", "java.lang.RuntimeException",
                newTvar("T", Flags.THROWS, syms.objectType),
                InferenceBound.UPPER, syms.runtimeExceptionType);

        String target = args.length == 0
                ? "crates/javac-core/tests/data/infer-oracle.txt"
                : args[0];
        java.nio.file.Files.writeString(java.nio.file.Path.of(target), out.toString());
        System.out.println("gen-infer-oracle: wrote " + target + " (" + out.length() + " chars)");
        fm.close();
    }
}
