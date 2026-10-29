//! Real-trigger regression tests for the Attr check call sites wired in
//! `crates/javac-core/src/comp/attr.rs`:
//!
//! 1. `Attr.attribTree` (attr.rs:663-681; upstream
//!    `reference/jdk17u/.../comp/Attr.java:679-684`):
//!    `MatchBindingsComputer.finishBindings` plus the `breakTree` check.
//! 2. `Attr.visitWildcard` (attr.rs:1184-1185; upstream Attr.java:5104):
//!    `chk.checkRefType`.
//! 3. `Attr.attribTypeVariables` (attr.rs:2899-2911; upstream Attr.java:828-832):
//!    `chk.checkNonCyclic(pos, (TypeVar)tvar.type)`, the `checkCyclic == true`
//!    path. Only `MemberEnter.java:106` passes `true` (method type
//!    parameters); `TypeEnter.java:887` passes `false` for class type
//!    parameters, whose class-level cyclic diagnostic comes from
//!    `Check.checkNonCyclicDecl` (`TypeEnter.java:796`). The trigger is
//!    therefore a generic *method* `<U extends U>`, not a class declaration.
//!    (The class-level form is not used here: in the port it currently hits an
//!    unrelated stack overflow in the class attribution path
//!    (Attr.attribClass, not this `checkCyclic == true` branch); reported
//!    separately.)
//!
//! # Oracle
//!
//! The expected values below are the recorded output of the official
//! JDK 17 javac (17.0.19), collected while this test was authored; the test
//! itself runs zero JVM:
//!
//! ```text
//! $ javac -J-Duser.language=en -J-Duser.country=US -XDrawDiagnostics WildcardRef.java
//! WildcardRef.java:3:10: compiler.err.type.found.req: int, (compiler.misc.type.req.ref)
//! $ javac -J-Duser.language=en -J-Duser.country=US WildcardRef.java
//! WildcardRef.java:3: error: unexpected type
//!     List<? extends int> field;
//!          ^
//!   required: reference
//!   found:    int
//!
//! $ javac -J-Duser.language=en -J-Duser.country=US -XDrawDiagnostics MethodCyclicInheritance.java
//! MethodCyclicInheritance.java:2:6: compiler.err.cyclic.inheritance: U
//! $ javac -J-Duser.language=en -J-Duser.country=US MethodCyclicInheritance.java
//! MethodCyclicInheritance.java:2: error: cyclic inheritance involving U
//!     <U extends U> void m() {}
//!      ^
//!   where U is a type-variable:
//!     U extends U declared in method m
//!
//! $ javac -J-Duser.language=en -J-Duser.country=US -XDrawDiagnostics BindingCallLeak.java
//! BindingCallLeak.java:5:20: compiler.err.cant.resolve.location: kindname.variable, s, , , (compiler.misc.location: kindname.class, BindingCallLeak, null)
//! $ javac -J-Duser.language=en -J-Duser.country=US BindingCallLeak.java
//! BindingCallLeak.java:5: error: cannot find symbol
//!             return s.length();
//!                    ^
//!   symbol:   variable s
//!   location: class BindingCallLeak
//!
//! $ javac -J-Duser.language=en -J-Duser.country=US BindingOk.java    # exit 0
//! $ javac -J-Duser.language=en -J-Duser.country=US SwitchBreak.java  # exit 0
//! ```
//!
//! NOTE on the diagnostic key: `chk.checkRefType` reports through
//! `Check.typeTagError` -> `Errors.TypeFoundReq` (Check.java:319-328), whose
//! resource key is `compiler.err.type.found.req` (compiler.properties:2463-2466,
//! text "unexpected type / required: ... / found: ..."). The official javac
//! `-XDrawDiagnostics` output above confirms that key; the separate
//! `compiler.err.unexpected.type` key (compiler.properties:2640) belongs to
//! `Check.checkType` and is not used by `checkRefType`.
//!
//! # breakTree
//!
//! `Attr.breakTree` is only set by `attribToTree` (Attr.java:425-448), whose
//! only callers are `attribStatToTree`/`attribExprToTree` (Attr.java:417-423)
//! from `JavacTrees.getEnv`-style compiler-API attribution of copied trees
//! (JavacTrees.java:912-933). A `break` statement is unrelated to it, and no
//! source file can reach it in either javac's command line compiler or the
//! port (JavacTrees is deferred to P6, see
//! `crates/javac-source/src/util/trees.rs:7-9`). The switch/break test below
//! therefore only checks that such a source still behaves like javac; it does
//! not claim to trigger `breakTree`.

use javac_core::main::mini_compiler::MiniCompiler;
use javac_core::util::log::{WriterHandle, WriterKind};
use std::cell::RefCell;
use std::collections::HashSet;
use std::io::Write;
use std::rc::Rc;

/// A `Write` sink whose bytes stay readable after the compile call (the Log
/// holds the other handle).
#[derive(Clone)]
struct CapturedWriter(Rc<RefCell<Vec<u8>>>);

impl Write for CapturedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// One compile run: the compiler result (class names), the error count, the
/// exact text written to the diagnostic (ERROR) stream, and the expected
/// diagnostic keys that were never reported.
struct CompileCase {
    result: Result<Vec<String>, String>,
    errors: i32,
    diagnostics: String,
    unreported_keys: Vec<String>,
}

fn compile_case(
    label: &str,
    file_name: &str,
    source: &str,
    expected_keys: &[&str],
) -> CompileCase {
    let dir = std::env::temp_dir().join(format!(
        "rust-javac-attr-wiring-{}-{label}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("output directory");
    let source_path = dir.join(file_name);
    std::fs::write(&source_path, source).expect("source file");

    let mut compiler = MiniCompiler::new(&dir);
    let captured = Rc::new(RefCell::new(Vec::new()));
    let writer: WriterHandle = Rc::new(RefCell::new(Box::new(CapturedWriter(Rc::clone(&captured)))));
    {
        let mut log = compiler.log.borrow_mut();
        log.set_writer(WriterKind::ERROR, writer);
        log.expect_diag_keys = Some(
            expected_keys
                .iter()
                .map(|key| key.to_string())
                .collect::<HashSet<_>>(),
        );
    }

    let result = compiler
        .compile_files(std::slice::from_ref(&source_path))
        .map(|classes| classes.into_iter().map(|class| class.name).collect::<Vec<_>>());
    let errors = compiler.error_count();
    let diagnostics = String::from_utf8_lossy(&captured.borrow()).into_owned();
    let remaining = compiler
        .log
        .borrow()
        .expect_diag_keys
        .clone()
        .unwrap_or_default();
    let unreported_keys = expected_keys
        .iter()
        .filter(|key| remaining.contains(**key))
        .map(|key| key.to_string())
        .collect();
    let _ = std::fs::remove_dir_all(&dir);
    CompileCase {
        result,
        errors,
        diagnostics,
        unreported_keys,
    }
}

/// Assert the javac-parity error case: the compiler reported exactly one
/// error and produced no class, every expected diagnostic code was reported,
/// and the formatted text contains the official javac message fragments.
///
/// `MiniCompiler::compile_files` returns `Ok(no classes)` when an early
/// `shouldStop` phase (JavaCompiler.compile2) stops on errors, so the error
/// channel is `log.nerrors` (as in javac), not necessarily the `Result`.
fn assert_error_like_javac(case: &CompileCase, fragments: &[&str]) {
    let classes: &[String] = match &case.result {
        Ok(classes) => classes,
        Err(_) => &[],
    };
    assert_eq!(
        case.errors, 1,
        "exactly one error expected (javac reports one); classes: {classes:?}\n{}",
        case.diagnostics
    );
    assert!(
        classes.is_empty(),
        "no class may be produced for a source that javac rejects: {classes:?}"
    );
    assert!(
        case.unreported_keys.is_empty(),
        "expected diagnostic code(s) {:?} were not reported; diagnostics:\n{}",
        case.unreported_keys,
        case.diagnostics
    );
    for fragment in fragments {
        assert!(
            case.diagnostics.contains(fragment),
            "diagnostics missing {:?}:\n{}",
            fragment,
            case.diagnostics
        );
    }
}

/// `Attr.visitWildcard` -> `chk.checkRefType` (`List<? extends int>`).
/// Oracle: `compiler.err.type.found.req` with
/// `required: reference` / `found:    int`.
#[test]
fn wildcard_primitive_bound_reports_type_found_req() {
    let source = "import java.util.List;\nclass WildcardRef {\n    List<? extends int> field;\n}\n";
    let case = compile_case(
        "wildcard",
        "WildcardRef.java",
        source,
        &["compiler.err.type.found.req"],
    );
    assert_error_like_javac(
        &case,
        &["unexpected type", "required: reference", "found:    int"],
    );
}

/// `Attr.attribTypeVariables` -> `chk.checkNonCyclic` for a *method* type
/// parameter (`<U extends U> void m()`), the only upstream call site with
/// `checkCyclic == true` (MemberEnter.java:106 -> Attr.java:828-832).
/// Oracle: `compiler.err.cyclic.inheritance: U`, text
/// "cyclic inheritance involving U".
#[test]
fn method_type_parameter_cycle_reports_cyclic_inheritance() {
    let source = "class MethodCyclicInheritance {\n    <U extends U> void m() {}\n}\n";
    let case = compile_case(
        "cyclic",
        "MethodCyclicInheritance.java",
        source,
        &["compiler.err.cyclic.inheritance"],
    );
    assert_error_like_javac(&case, &["cyclic inheritance involving U"]);
}

/// `Attr.attribTree` -> `finishBindings` reset: the binding of
/// `o instanceof String s` used as a *method-call argument* of the condition is
/// not in scope in the then-branch, so `s` must not resolve (javac: cannot find
/// symbol). `visitIf` reads `matchBindings` right after attributing the
/// condition (attr.rs:7837-7838) and enters those bindings into the then/else
/// environments (attr.rs:7839-7845); `visitApply` never rewrites the field, and
/// `finishBindings` only keeps the bindings for the tags listed in
/// match_bindings.rs:274-287 -- APPLY is not one of them. This case is therefore
/// closed only by the `attribTree` finishBindings call: the `==` variant is
/// already reset inside `MatchBindingsComputer::binary`
/// (`_ => MatchBindings::empty()`, match_bindings.rs:195) and would not prove
/// the wiring.
///
/// Known pre-existing difference (reported, not caused by this wiring): the
/// port renders the detail lines of `compiler.err.cant.resolve.location` as
/// "symbol:   kindname.variable s" / "location: location", while javac renders
/// "symbol:   variable s" / "location: class BindingCallLeak". The assertions
/// below therefore cover the diagnostic code and the primary
/// "cannot find symbol" message of the same diagnostic.
#[test]
fn binding_from_method_call_argument_is_out_of_scope() {
    let source = "class BindingCallLeak {\n    static boolean test(boolean b) { return b; }\n    int leak(Object o) {\n        if (test(o instanceof String s)) {\n            return s.length();\n        }\n        return 0;\n    }\n}\n";
    let case = compile_case(
        "binding-leak",
        "BindingCallLeak.java",
        source,
        &["compiler.err.cant.resolve.location"],
    );
    assert_error_like_javac(&case, &["cannot find symbol", "variable s"]);
}

/// Control case for the `finishBindings` retention path: the binding of
/// `o instanceof String s` inside a `&&` condition is definitely matched in
/// the then-branch, so the source compiles (javac exit 0).
#[test]
fn binding_in_logical_and_condition_stays_in_scope() {
    let source = "class BindingOk {\n    int ok(Object o) {\n        if (o instanceof String s && s.length() > 0) {\n            return s.length();\n        }\n        return 0;\n    }\n}\n";
    let case = compile_case("binding-ok", "BindingOk.java", source, &[]);
    assert!(
        case.result.is_ok(),
        "compilation must succeed: {:?}\n{}",
        case.result,
        case.diagnostics
    );
    assert_eq!(case.errors, 0, "no errors expected:\n{}", case.diagnostics);
    assert!(
        case.result.as_ref().unwrap().iter().any(|name| name == "BindingOk"),
        "BindingOk class must be generated: {:?}",
        case.result
    );
}

/// Switch statement + `break`: compiles exactly like javac. This is the
/// switch/break attempt requested by the batch brief; it does NOT trigger
/// `breakTree` (see the module docs: `breakTree` is the JavacTrees
/// copied-tree attribution target, not a break statement).
#[test]
fn switch_statement_with_break_compiles_like_javac() {
    let source = "class SwitchBreak {\n    int m(int x) {\n        switch (x) {\n            case 1:\n                return 10;\n            default:\n                break;\n        }\n        return 0;\n    }\n}\n";
    let case = compile_case("switch-break", "SwitchBreak.java", source, &[]);
    assert!(
        case.result.is_ok(),
        "compilation must succeed: {:?}\n{}",
        case.result,
        case.diagnostics
    );
    assert_eq!(case.errors, 0, "no errors expected:\n{}", case.diagnostics);
    assert!(
        case.result.as_ref().unwrap().iter().any(|name| name == "SwitchBreak"),
        "SwitchBreak class must be generated: {:?}",
        case.result
    );
}
