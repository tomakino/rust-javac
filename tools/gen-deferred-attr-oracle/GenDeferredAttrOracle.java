// Development-time DeferredAttr oracle for the Rust port tests.
//
// Uses the baseline JDK 17.0.19 internal javac API to sample the observable
// behavior of DeferredAttr.DeferredAttrContext (defaults, overload-phase
// chain, the empty-context assertions), then writes the result to the oracle
// data file (no JVM at Rust test time).
package com.sun.tools.javac.comp;

import com.sun.source.util.JavacTask;
import com.sun.tools.javac.api.JavacTaskImpl;
import com.sun.tools.javac.util.Context;
import com.sun.tools.javac.util.List;

import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.SimpleJavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;
import java.net.URI;
import java.util.ArrayList;

public class GenDeferredAttrOracle {

    public static void main(String[] args) throws Exception {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        StandardJavaFileManager fm = compiler.getStandardFileManager(null, null, null);
        JavaFileObject file = new SimpleJavaFileObject(
                URI.create("string:///DeferredSample.java"), JavaFileObject.Kind.SOURCE) {
            @Override
            public CharSequence getCharContent(boolean ignoreEncodingErrors) {
                return "class DeferredSample {}";
            }
        };
        ArrayList<JavaFileObject> files = new ArrayList<>();
        files.add(file);
        JavacTask task = (JavacTask) compiler.getTask(null, fm, null, List.of("-proc:none"), null, files);
        task.parse();
        task.analyze();
        Context context = ((JavacTaskImpl) task).getContext();
        DeferredAttr deferredAttr = DeferredAttr.instance(context);
        Infer infer = Infer.instance(context);

        StringBuilder out = new StringBuilder();
        out.append("ORACLE deferred-attr\n");

        DeferredAttr.DeferredAttrContext empty = deferredAttr.emptyDeferredAttrContext;
        out.append("EMPTY mode ").append(empty.mode)
           .append(" phase ").append(empty.phase)
           .append(" msym ").append(empty.msym)
           .append(" insideOverloadPhase ").append(empty.insideOverloadPhase())
           .append('\n');

        DeferredAttr.DeferredAttrContext check =
                deferredAttr.new DeferredAttrContext(DeferredAttr.AttrMode.CHECK, null,
                        Resolve.MethodResolutionPhase.BOX, infer.emptyContext,
                        deferredAttr.emptyDeferredAttrContext, null);
        out.append("CHECK insideOverloadPhase ").append(check.insideOverloadPhase()).append('\n');

        DeferredAttr.DeferredAttrContext speculative =
                deferredAttr.new DeferredAttrContext(DeferredAttr.AttrMode.SPECULATIVE, null,
                        Resolve.MethodResolutionPhase.BOX, infer.emptyContext,
                        deferredAttr.emptyDeferredAttrContext, null);
        out.append("SPECULATIVE insideOverloadPhase ").append(speculative.insideOverloadPhase()).append('\n');

        DeferredAttr.DeferredAttrContext nested =
                deferredAttr.new DeferredAttrContext(DeferredAttr.AttrMode.CHECK, null,
                        Resolve.MethodResolutionPhase.BOX, infer.emptyContext, check, null);
        out.append("NESTED_CHECK insideOverloadPhase ").append(nested.insideOverloadPhase()).append('\n');

        String target = args.length == 0
                ? "crates/javac-core/tests/data/deferred-attr-oracle.txt"
                : args[0];
        java.nio.file.Files.writeString(java.nio.file.Path.of(target), out.toString());
        System.out.println("gen-deferred-attr-oracle: wrote " + target + " (" + out.length() + " chars)");
        fm.close();
    }
}
