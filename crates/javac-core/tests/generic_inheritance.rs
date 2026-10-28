//! Regression test for generic inheritance through a JDK interface hierarchy.
//!
//! The source below exercises three type-system paths that must agree with
//! javac: (1) `List<E>` is a generic classfile symbol whose type arguments are
//! completed lazily, (2) the declared interfaces of a symbol are substituted
//! with the actual type arguments of the instantiated type, and (3) capture
//! conversion of a method return type keeps the type arguments.  A failure in
//! any of them makes `Arrays.asList(...)` incompatible with
//! `Collection<Annotation>`.

use javac_core::main::mini_compiler::MiniCompiler;

#[test]
fn generic_interface_supertypes_are_substituted() {
    let source = r#"
import java.lang.annotation.Annotation;
import java.lang.reflect.Field;
import java.util.Arrays;
import java.util.Collection;
import java.util.List;

class GenericInheritance {
    Field field;

    Collection<Annotation> getAnnotations() {
        return Arrays.asList(field.getAnnotations());
    }

    List<String> strings() {
        return Arrays.asList("a", "b");
    }
}
"#;
    let root = std::env::temp_dir().join(format!(
        "rust-javac-generic-inheritance-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("output directory");
    let source_path = root.join("GenericInheritance.java");
    std::fs::write(&source_path, source).expect("source file");
    let mut compiler = MiniCompiler::new(&root);
    let compiled = compiler
        .compile_files(std::slice::from_ref(&source_path))
        .expect("generic inheritance compilation");
    assert_eq!(
        compiler.error_count(),
        0,
        "diagnostics: {}",
        compiler.error_count()
    );
    assert!(
        compiled.iter().any(|class| class.name == "GenericInheritance"),
        "generated: {:?}",
        compiled.iter().map(|class| class.name.clone()).collect::<Vec<_>>()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn primitive_array_copyof_selects_the_concrete_overload() {
    let source = r#"
class CopyOfCall {
    static <X> X[] copyOf(X[] a, int n) { return a; }
    static int[] copyOf(int[] a, int n) { return a; }

    int[] copy() {
        return copyOf(new int[0], 0);
    }
}
"#;
    let root = std::env::temp_dir().join(format!(
        "rust-javac-copyof-overload-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("output directory");
    let source_path = root.join("CopyOfCall.java");
    std::fs::write(&source_path, source).expect("source file");
    let mut compiler = MiniCompiler::new(&root);
    let compiled = compiler
        .compile_files(std::slice::from_ref(&source_path))
        .expect("copyOf compilation");
    assert_eq!(compiler.error_count(), 0, "errors: {}", compiler.error_count());
    assert!(
        compiled.iter().any(|class| class.name == "CopyOfCall"),
        "generated: {:?}",
        compiled.iter().map(|class| class.name.clone()).collect::<Vec<_>>()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn incompatible_generic_array_call_is_diagnosed() {
    let source = r#"
class BadArrayCall {
    static <X> void f(X[] a) {}

    void g() {
        f(new int[0]);
    }
}
"#;
    let root = std::env::temp_dir().join(format!(
        "rust-javac-bad-array-call-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("output directory");
    let source_path = root.join("BadArrayCall.java");
    std::fs::write(&source_path, source).expect("source file");
    let mut compiler = MiniCompiler::new(&root);
    // The call is not applicable: a diagnostic must be reported, and the
    // inference failure must not escape as a panic.
    let _ = compiler.compile_files(std::slice::from_ref(&source_path));
    assert!(
        compiler.error_count() > 0,
        "expected at least one diagnostic for the incompatible generic array call"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn array_foreach_loops_are_attributed_in_scope() {
    let source = r#"
class ForEachScopes {
    int sum(int[] values) {
        int total = 0;
        for (int value : values) {
            for (int inner : values) {
                total += value + inner;
            }
        }
        return total;
    }
}
"#;
    let root = std::env::temp_dir().join(format!(
        "rust-javac-foreach-scopes-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("output directory");
    let source_path = root.join("ForEachScopes.java");
    std::fs::write(&source_path, source).expect("source file");
    let mut compiler = MiniCompiler::new(&root);
    let compiled = compiler
        .compile_files(std::slice::from_ref(&source_path))
        .expect("foreach compilation");
    assert_eq!(compiler.error_count(), 0, "errors: {}", compiler.error_count());
    assert!(
        compiled.iter().any(|class| class.name == "ForEachScopes"),
        "generated: {:?}",
        compiled.iter().map(|class| class.name.clone()).collect::<Vec<_>>()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn foreach_over_inapplicable_type_is_diagnosed() {
    let source = r#"
class BadForEach {
    void f() {
        for (int value : 1) {
        }
    }
}
"#;
    let root = std::env::temp_dir().join(format!(
        "rust-javac-bad-foreach-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("output directory");
    let source_path = root.join("BadForEach.java");
    std::fs::write(&source_path, source).expect("source file");
    let mut compiler = MiniCompiler::new(&root);
    // The loop expression is not an array or an Iterable: a diagnostic is
    // reported instead of the loop crashing the compiler.
    let result = compiler.compile_files(std::slice::from_ref(&source_path));
    assert!(result.is_ok(), "foreach diagnostic must not panic: {result:?}");
    assert!(
        compiler.error_count() > 0,
        "expected at least one diagnostic for an inapplicable foreach"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn local_variable_after_nested_scope_is_attributed() {
    let source = r#"
class NestedScopeBody {
    void forThenLocal() {
        for (int i = 0; i < 1; i++) {
        }
        int afterFor = 1;
    }

    void blockThenLocal() {
        {
        }
        int afterBlock = 1;
    }

    void ifThenLocal() {
        if (true) {
        }
        int afterIf = 1;
    }
}
"#;
    let root = std::env::temp_dir().join(format!(
        "rust-javac-nested-scope-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("output directory");
    let source_path = root.join("NestedScopeBody.java");
    std::fs::write(&source_path, source).expect("source file");
    let mut compiler = MiniCompiler::new(&root);
    let compiled = compiler
        .compile_files(std::slice::from_ref(&source_path))
        .expect("nested scope compilation");
    assert_eq!(compiler.error_count(), 0, "errors: {}", compiler.error_count());
    assert!(
        compiled.iter().any(|class| class.name == "NestedScopeBody"),
        "generated: {:?}",
        compiled.iter().map(|class| class.name.clone()).collect::<Vec<_>>()
    );
    let _ = std::fs::remove_dir_all(&root);
}
