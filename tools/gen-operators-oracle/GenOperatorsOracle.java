// Development-time Operators/ConstFold oracle for the Rust port tests.
//
// Uses the baseline JDK 17.0.19 internal javac API to sample the observable
// behavior of Operators.resolveUnary/resolveBinary and ConstFold.fold1/fold2/
// coerce. The Rust test (tests/operators_oracle.rs, zero JVM at test time)
// replays each row against the ported Operators/ConstFold.
//
// Output format (one row per case):
//   BIN <tag> <type1> <type2> <opcode> <returntag>
//   UN  <tag> <type> <opcode> <returntag>
//   FOLD <opcode> <int|long|float|double|string> <lhs> <rhs> <result>
package com.sun.tools.javac.comp;

import com.sun.tools.javac.api.JavacTaskImpl;
import com.sun.tools.javac.code.Symbol.OperatorSymbol;
import com.sun.tools.javac.code.Symtab;
import com.sun.tools.javac.code.Type;
import com.sun.tools.javac.code.TypeTag;
import com.sun.tools.javac.jvm.ByteCodes;
import com.sun.tools.javac.tree.JCTree;
import com.sun.tools.javac.util.Context;
import com.sun.tools.javac.util.List;
import com.sun.tools.javac.util.Names;

import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.SimpleJavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;
import java.net.URI;
import java.util.ArrayList;

public class GenOperatorsOracle {
    static Context context;
    static Symtab syms;
    static Operators ops;
    static ConstFold cf;
    static StringBuilder out = new StringBuilder();

    static Type type(String name) {
        switch (name) {
            case "byte": return syms.byteType;
            case "short": return syms.shortType;
            case "int": return syms.intType;
            case "long": return syms.longType;
            case "float": return syms.floatType;
            case "double": return syms.doubleType;
            case "char": return syms.charType;
            case "boolean": return syms.booleanType;
            case "object": return syms.objectType;
            case "string": return syms.stringType;
            case "bot": return syms.botType;
            default: throw new AssertionError(name);
        }
    }

    static String tagName(TypeTag tag) {
        return tag.name().toLowerCase();
    }

    static void bin(JCTree.Tag tag, String t1, String t2) {
        OperatorSymbol op = ops.resolveBinary(null, tag, type(t1), type(t2));
        if (op == ops.noOpSymbol) {
            out.append("BIN ").append(tag).append(' ').append(t1).append(' ')
               .append(t2).append(" NOOP\n");
        } else {
            out.append("BIN ").append(tag).append(' ').append(t1).append(' ')
               .append(t2).append(' ').append(op.opcode).append(' ')
               .append(tagName(op.type.getReturnType().getTag())).append('\n');
        }
    }

    static void un(JCTree.Tag tag, String t) {
        OperatorSymbol op = ops.resolveUnary(null, tag, type(t));
        if (op == ops.noOpSymbol) {
            out.append("UN ").append(tag).append(' ').append(t).append(" NOOP\n");
        } else {
            out.append("UN ").append(tag).append(' ').append(t).append(' ')
               .append(op.opcode).append(' ')
               .append(tagName(op.type.getReturnType().getTag())).append('\n');
        }
    }

    static void foldInt(int opcode, int l, int r) {
        Type result = cf.fold2(opcode, syms.intType.constType(l), syms.intType.constType(r));
        out.append("FOLD ").append(opcode).append(" int ").append(l).append(' ')
           .append(r).append(' ');
        if (result == null || result.constValue() == null) {
            out.append("null\n");
        } else {
            out.append(result.constValue()).append('\n');
        }
    }

    static void foldString(int opcode, String l, String r) {
        Type result = cf.fold2(opcode, syms.stringType.constType(l), syms.stringType.constType(r));
        out.append("FOLD ").append(opcode).append(" string ").append(l).append(' ')
           .append(r).append(' ');
        if (result == null || result.constValue() == null) {
            out.append("null\n");
        } else {
            out.append(result.constValue()).append('\n');
        }
    }

    public static void main(String[] args) throws Exception {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        StandardJavaFileManager fm = compiler.getStandardFileManager(null, null, null);
        JavaFileObject file = new SimpleJavaFileObject(
                URI.create("string:///OperatorSample.java"), JavaFileObject.Kind.SOURCE) {
            @Override
            public CharSequence getCharContent(boolean ignoreEncodingErrors) {
                return "class OperatorSample {}";
            }
        };
        ArrayList<JavaFileObject> files = new ArrayList<>();
        files.add(file);
        com.sun.source.util.JavacTask task = (com.sun.source.util.JavacTask) compiler.getTask(null, fm, null, List.of("-proc:none"), null, files);
        task.parse();
        context = ((JavacTaskImpl) task).getContext();
        syms = Symtab.instance(context);
        ops = Operators.instance(context);
        cf = ConstFold.instance(context);
        out.append("ORACLE operators\n");

        bin(JCTree.Tag.PLUS, "int", "int");
        bin(JCTree.Tag.PLUS, "string", "string");
        bin(JCTree.Tag.PLUS, "long", "long");
        bin(JCTree.Tag.MINUS, "long", "long");
        bin(JCTree.Tag.SL, "int", "long");
        bin(JCTree.Tag.SR, "long", "int");
        bin(JCTree.Tag.USR, "int", "int");
        bin(JCTree.Tag.EQ, "object", "object");
        bin(JCTree.Tag.EQ, "int", "int");
        bin(JCTree.Tag.LT, "double", "double");
        bin(JCTree.Tag.BITAND, "boolean", "boolean");
        bin(JCTree.Tag.BITAND, "int", "int");
        bin(JCTree.Tag.AND, "boolean", "boolean");
        bin(JCTree.Tag.OR, "boolean", "boolean");
        bin(JCTree.Tag.PLUS, "boolean", "boolean");
        un(JCTree.Tag.POS, "int");
        un(JCTree.Tag.NEG, "int");
        un(JCTree.Tag.NEG, "double");
        un(JCTree.Tag.COMPL, "long");
        un(JCTree.Tag.NOT, "boolean");
        un(JCTree.Tag.POSTINC, "byte");
        un(JCTree.Tag.NOT, "int");

        foldInt(ByteCodes.iadd, 2, 3);
        foldInt(ByteCodes.isub, 2, 3);
        foldInt(ByteCodes.imul, 7, 6);
        foldInt(ByteCodes.idiv, 7, 2);
        foldInt(ByteCodes.idiv, 7, 0);
        foldInt(ByteCodes.imod, 7, 2);
        foldInt(ByteCodes.ishl, 1, 5);
        foldInt(ByteCodes.iand, 6, 3);
        foldInt(ByteCodes.if_icmplt, 1, 2);
        foldInt(ByteCodes.if_icmpge, 1, 2);
        foldString(ByteCodes.string_add, "a", "b");

        System.out.print(out);
    }
}
