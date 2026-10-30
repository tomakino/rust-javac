use std::path::PathBuf;

use javac_core::main::mini_compiler::MiniCompiler;
use javac_core::main::option::Option;

fn compile_case(name: &str, source: &str) {
    let dir = std::env::temp_dir().join(format!("rust-javac-probe-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("T.java");
    std::fs::write(&src, source).unwrap();
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let mut compiler = MiniCompiler::new_with_options(&out, |_options| {});
    let compiled = compiler.compile_files(&[src]);
    eprintln!(
        "CASE {name}: errors={} classes={} result={:?}",
        compiler.error_count(),
        compiled.as_ref().map(|c| c.len()).unwrap_or(0),
        compiled.as_ref().err().map(|e| format!("{e:?}"))
    );
}

#[test]
fn local_record_class_literal() {
    compile_case(
        "local-record",
        "class T {\n  void m() {\n    record LocalRecord(int i) {}\n    Class<?> c = LocalRecord.class;\n  }\n}\n",
    );
}

#[test]
fn local_record_in_lambda() {
    compile_case(
        "local-record-lambda",
        "import java.util.function.*;\nclass T {\n  void m() {\n    record LocalRecord(int i) {}\n    Supplier<Class<?>> s = () -> LocalRecord.class;\n    Supplier<String> n = () -> LocalRecord.class.getName();\n  }\n}\n",
    );
}

#[test]
fn local_record_annotated_lambda() {
    compile_case(
        "local-record-annotated",
        "import java.util.function.*;\nclass T {\n  void m() {\n    record LocalRecord(int i) {\n      @Deprecated public int i() { return i; }\n    }\n    Supplier<Class<?>> s = () -> LocalRecord.class;\n  }\n}\n",
    );
}

#[test]
fn local_record_in_generic_call_lambda() {
    compile_case(
        "local-record-generic",
        "class T {\n  static <E extends Throwable> E assertThrows(Class<E> c, Runnable r) { return null; }\n  void m() {\n    record LocalRecord(int i) {}\n    Object o = assertThrows(RuntimeException.class, () -> LocalRecord.class.getName());\n  }\n}\n",
    );
}

#[test]
fn local_appendable_class() {
    compile_case(
        "local-appendable",
        "import java.io.*;\nclass T {\n  void m() throws Exception {\n    class CustomAppendable implements Appendable {\n      final StringBuilder stringBuilder = new StringBuilder();\n      int toStringCallCount = 0;\n      @Override public Appendable append(char c) throws IOException { stringBuilder.append(c); return this; }\n      @Override public Appendable append(CharSequence csq) throws IOException { if (csq == null) csq = \"null\"; append(csq, 0, csq.length()); return this; }\n      @Override public Appendable append(CharSequence csq, int start, int end) throws IOException { String s = csq.toString(); toStringCallCount++; stringBuilder.append(s, start, end); return this; }\n    }\n  }\n}\n",
    );
}

fn compile_case_cp(name: &str, source: &str, classpath: &str) {
    let dir = std::env::temp_dir().join(format!("rust-javac-probe-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("T.java");
    std::fs::write(&src, source).unwrap();
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let mut compiler = MiniCompiler::new_with_options(&out, |options| {
        options.put_option(Option::CLASS_PATH, classpath);
    });
    let compiled = compiler.compile_files(&[src]);
    eprintln!(
        "CASE {name}: errors={} classes={} result={:?}",
        compiler.error_count(),
        compiled.as_ref().map(|c| c.len()).unwrap_or(0),
        compiled.as_ref().err().map(|e| format!("{e:?}"))
    );
}

#[test]
fn anonymous_type_token() {
    let Ok(gson_out) = std::env::var("GSON_OUT") else { return; };
    compile_case_cp(
        "anonymous-type-token",
        "import com.google.gson.reflect.TypeToken;\nimport java.util.Map;\nclass T {\n  void m() {\n    Object x = new TypeToken<Map<String, String>>() {}.getType();\n  }\n}\n",
        &gson_out,
    );
}

#[test]
fn local_appendable_gson_simple() {
    let Ok(gson_out) = std::env::var("GSON_OUT") else { return; };
    compile_case_cp(
        "local-appendable-gson-simple",
        "import com.google.gson.*;\nimport java.io.*;\nclass T {\n  void m() {\n    class C implements Appendable {\n      @Override public Appendable append(char c) { return this; }\n      @Override public Appendable append(CharSequence csq) { return this; }\n      @Override public Appendable append(CharSequence csq, int start, int end) { return this; }\n    }\n    C c = new C();\n    new Gson().toJson(\"x\", c);\n  }\n}\n",
        &gson_out,
    );
}

#[test]
fn gson_list_appendable() {
    let Ok(gson_out) = std::env::var("GSON_OUT") else { return; };
    compile_case_cp(
        "gson-list-appendable",
        "import com.google.gson.*;\nimport java.io.*;\nimport java.util.*;\nclass T {\n  void m() {\n    class C implements Appendable {\n      @Override public Appendable append(char c) { return this; }\n      @Override public Appendable append(CharSequence csq) { return this; }\n      @Override public Appendable append(CharSequence csq, int start, int end) { return this; }\n    }\n    C c = new C();\n    new Gson().toJson(Arrays.asList(\"test\", 123, true), c);\n  }\n}\n",
        &gson_out,
    );
}

#[test]
fn gson_list_var_appendable() {
    let Ok(gson_out) = std::env::var("GSON_OUT") else { return; };
    compile_case_cp(
        "gson-list-var-appendable",
        "import com.google.gson.*;\nimport java.io.*;\nimport java.util.*;\nclass T {\n  void m() {\n    class C implements Appendable {\n      @Override public Appendable append(char c) { return this; }\n      @Override public Appendable append(CharSequence csq) { return this; }\n      @Override public Appendable append(CharSequence csq, int start, int end) { return this; }\n    }\n    C c = new C();\n    List<Object> l = Arrays.asList(\"test\", 123, true);\n    new Gson().toJson(l, c);\n  }\n}\n",
        &gson_out,
    );
}

#[test]
fn local_appendable_gson() {
    let Ok(gson_out) = std::env::var("GSON_OUT") else { return; };
    compile_case_cp(
        "local-appendable-gson",
        "import com.google.gson.*;\nimport java.io.*;\nimport java.util.*;\nclass T {\n  void m() {\n    class C implements Appendable {\n      final StringBuilder sb = new StringBuilder();\n      int count = 0;\n      @Override public Appendable append(char c) { sb.append(c); return this; }\n      @Override public Appendable append(CharSequence csq) { if (csq == null) csq = \"null\"; append(csq, 0, csq.length()); return this; }\n      @Override public Appendable append(CharSequence csq, int start, int end) { String s = csq.toString(); count++; sb.append(s, start, end); return this; }\n    }\n    C c = new C();\n    new Gson().toJson(Arrays.asList(\"test\", 123, true), c);\n  }\n}\n",
        &gson_out,
    );
}

#[test]
fn readers_writers_body() {
    let Ok(gson_out) = std::env::var("GSON_OUT") else { return; };
    let Ok(junit) = std::env::var("JUNIT_JAR") else { return; };
    let classpath = format!("{gson_out};{junit}");
    compile_case_cp(
        "readers-writers-body",
        "import com.google.gson.*;\nimport com.google.gson.reflect.TypeToken;\nimport static org.junit.Assert.*;\nimport java.io.*;\nimport java.util.*;\nclass T {\n  void m() throws Exception {\n    Gson gson = new Gson();\n    StringWriter writer = new StringWriter();\n    gson.toJson(null, writer);\n    assertEquals(\"null\", writer.toString());\n    try {\n      gson.fromJson(\"true\", new TypeToken<Map<String, String>>() {}.getType());\n      fail();\n    } catch (JsonSyntaxException expected) {}\n  }\n}\n",
        &classpath,
    );
}

#[test]
fn bag_of_primitives_helper() {
    let Ok(gson_out) = std::env::var("GSON_OUT") else { return; };
    let Ok(junit) = std::env::var("JUNIT_JAR") else { return; };
    let Ok(test_src) = std::env::var("GSON_TEST_SRC") else { return; };
    let dir = std::env::temp_dir().join(format!("rust-javac-probe-bag-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let src = dir.join("T.java");
    std::fs::write(
        &src,
        "import com.google.gson.common.TestTypes.BagOfPrimitives;\nclass T {\n  void m() {\n    BagOfPrimitives b = new BagOfPrimitives();\n    String s = b.getExpectedJson();\n  }\n}\n",
    )
    .unwrap();
    let test_types = std::path::PathBuf::from(&test_src).join("com/google/gson/common/TestTypes.java");
    let classpath = format!("{gson_out};{junit}");
    let mut compiler = MiniCompiler::new_with_options(&out, |options| {
        options.put_option(Option::CLASS_PATH, &classpath);
    });
    let compiled = compiler.compile_files(&[test_types, src]);
    eprintln!(
        "CASE bag: errors={} classes={} result={:?}",
        compiler.error_count(),
        compiled.as_ref().map(|c| c.len()).unwrap_or(0),
        compiled.as_ref().err().map(|e| format!("{e:?}"))
    );
}

#[test]
fn rw_variant() {
    let Ok(gson_out) = std::env::var("GSON_OUT") else { return; };
    let Ok(junit) = std::env::var("JUNIT_JAR") else { return; };
    let Ok(test_src) = std::env::var("GSON_TEST_SRC") else { return; };
    let Ok(rw_src) = std::env::var("RW_SRC") else { return; };
    let dir = std::env::temp_dir().join(format!("rust-javac-probe-rwv-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let test_types = std::path::PathBuf::from(&test_src).join("com/google/gson/common/TestTypes.java");
    let classpath = format!("{gson_out};{junit}");
    let mut compiler = MiniCompiler::new_with_options(&out, |options| {
        options.put_option(Option::CLASS_PATH, &classpath);
    });
    let compiled = compiler.compile_files(&[test_types, std::path::PathBuf::from(rw_src)]);
    eprintln!(
        "CASE rw-variant: errors={} classes={} result={:?}",
        compiler.error_count(),
        compiled.as_ref().map(|c| c.len()).unwrap_or(0),
        compiled.as_ref().err().map(|e| format!("{e:?}"))
    );
}

#[test]
fn source_env() {
    let Ok(gson_out) = std::env::var("GSON_OUT") else { return; };
    let Ok(source) = std::env::var("JAVA_SRC") else { return; };
    compile_case_cp("source-env", &source, &gson_out);
}

#[test]
fn readers_writers_single() {
    let Ok(gson_out) = std::env::var("GSON_OUT") else { return; };
    let Ok(junit) = std::env::var("JUNIT_JAR") else { return; };
    let Ok(test_src) = std::env::var("GSON_TEST_SRC") else { return; };
    let dir = std::env::temp_dir().join(format!("rust-javac-probe-rw-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let sources = vec![
        std::path::PathBuf::from(&test_src).join("com/google/gson/common/TestTypes.java"),
        std::path::PathBuf::from(&test_src).join("com/google/gson/functional/ReadersWritersTest.java"),
    ];
    let classpath = format!("{gson_out};{junit}");
    let mut compiler = MiniCompiler::new_with_options(&out, |options| {
        options.put_option(Option::CLASS_PATH, &classpath);
    });
    let compiled = compiler.compile_files(&sources);
    eprintln!(
        "CASE readers-writers: errors={} classes={} result={:?}",
        compiler.error_count(),
        compiled.as_ref().map(|c| c.len()).unwrap_or(0),
        compiled.as_ref().err().map(|e| format!("{e:?}"))
    );
}

#[test]
fn diamond_in_argument() {
    compile_case(
        "diamond",
        "import java.util.*;\nclass T {\n  void m(Set<String> s) {\n    Object o = new ArrayList<>(s);\n  }\n}\n",
    );
}
#[test]
fn collections_unmodifiable_generic() {
    compile_case(
        "collections-unmodifiable",
        "import java.util.*;\nclass T {\n  static <T> List<T> f(List<T> list) { return Collections.unmodifiableList(new ArrayList<>(list)); }\n  static <K,V> Map<K,V> g(Map<K,V> map) { return map.isEmpty() ? Collections.emptyMap() : Collections.unmodifiableMap(new LinkedHashMap<>(map)); }\n}\n",
    );
}
#[test]
fn wildcard_inference_variants() {
    compile_case("wc-plain-list", "import java.util.*;\nclass T { static <T> List<T> f(List<T> l) { return Collections.unmodifiableList(l); } }\n");
    compile_case("wc-diamond-list", "import java.util.*;\nclass T { static <T> List<T> f(List<T> l) { return Collections.unmodifiableList(new ArrayList<>(l)); } }\n");
    compile_case("wc-plain-map", "import java.util.*;\nclass T { static <K,V> Map<K,V> f(Map<K,V> m) { return Collections.unmodifiableMap(m); } }\n");
    compile_case("wc-diamond-map", "import java.util.*;\nclass T { static <K,V> Map<K,V> f(Map<K,V> m) { return Collections.unmodifiableMap(new LinkedHashMap<>(m)); } }\n");
}
#[test]
fn wildcard_diamond_assignments() {
    compile_case("wc-assign-plain", "import java.util.*;\nclass T { static <T> void f(List<T> l) { List<T> x = new ArrayList<>(l); } }\n");
    compile_case("wc-assign-wild", "import java.util.*;\nclass T { static <T> void f(List<T> l) { List<? extends T> x = new ArrayList<>(l); } }\n");
    compile_case("wc-assign-explicit", "import java.util.*;\nclass T { static <T> void f(List<T> l) { List<? extends T> x = new ArrayList<T>(l); } }\n");
}
#[test]
fn wildcard_diamond_calls() {
    compile_case("wc-call-explicit-new", "import java.util.*;\nclass T { static <T> List<T> f(List<T> l) { return Collections.unmodifiableList(new ArrayList<T>(l)); } }\n");
    compile_case("wc-call-explicit-method", "import java.util.*;\nclass T { static <T> List<T> f(List<T> l) { return Collections.<T>unmodifiableList(new ArrayList<>(l)); } }\n");
    compile_case("wc-call-cast", "import java.util.*;\nclass T { static <T> List<T> f(List<T> l) { return Collections.unmodifiableList((List<T>) new ArrayList<>(l)); } }\n");
}
#[test]
fn classpath_jar_okio() {
    compile_case_cp("jar-okio", "import okio.Buffer; class T { Buffer b; }\n", "C:/Users/Lin/RustroverProjects/javac-testdata/okhttp-deps/okio-1.17.2.jar");
}
#[test]
fn classpath_jar_package_forms() {
    let cp="C:/Users/Lin/RustroverProjects/javac-testdata/okhttp-deps/okio-1.17.2.jar";
    compile_case_cp("jar-star", "import okio.*; class T { Buffer b; }\n", cp);
    compile_case_cp("jar-qualified", "class T { okio.Buffer b; }\n", cp);
}

#[test]
fn duplicate_field_diagnostic_carries_source_and_position() {
    let dir = std::env::temp_dir().join(format!("rust-javac-probe-dupfield-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("T.java");
    std::fs::write(&src, "class T {\n    int i;\n    int i;\n}\n").unwrap();
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let mut compiler = MiniCompiler::new_with_options(&out, |_options| {});
    compiler.log.borrow_mut().install_diagnostic_handler(
        javac_core::util::log::DiagnosticHandler::Deferred {
            prev: None,
            deferred: std::collections::VecDeque::new(),
            filter: None,
        },
    );
    let _ = compiler.compile_files(&[src]);
    let diagnostics = compiler.log.borrow_mut().take_deferred_diagnostics();
    for diagnostic in &diagnostics {
        eprintln!(
            "DIAG {} source={:?} pos={}",
            diagnostic.get_code_string(),
            diagnostic.get_source().map(|file| file.get_name().to_string_lossy()),
            diagnostic.get_int_position()
        );
    }
    let duplicate = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.get_code_string().contains("already.defined"))
        .expect("already.defined diagnostic");
    assert!(duplicate.get_source().is_some(), "duplicate field diagnostic lost its source");
    assert_ne!(duplicate.get_int_position(), javac_core::util::position::NOPOS);
}

#[test]
fn sealed_diagnostic_position_probe() {
    let dir = std::env::temp_dir().join(format!("rust-javac-probe-sealedpos-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("T.java");
    std::fs::write(&src, "sealed class Sealed {}\n").unwrap();
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let mut compiler = MiniCompiler::new_with_options(&out, |_options| {});
    compiler.log.borrow_mut().install_diagnostic_handler(
        javac_core::util::log::DiagnosticHandler::Deferred {
            prev: None,
            deferred: std::collections::VecDeque::new(),
            filter: None,
        },
    );
    let _ = compiler.compile_files(&[src]);
    for (index, tree) in compiler.ctx.trees.iter().enumerate() {
        if tree.tag == javac_core::tree::jc_tree::Tag::CLASSDEF {
            eprintln!("CLASS TREE {} pos={}", index, tree.pos);
        }
    }
    for diagnostic in compiler.log.borrow_mut().take_deferred_diagnostics() {
        eprintln!(
            "DIAG {} pos={} line={} col={}",
            diagnostic.get_code_string(),
            diagnostic.get_int_position(),
            diagnostic.get_line_number(),
            diagnostic.get_column_number()
        );
    }
}
