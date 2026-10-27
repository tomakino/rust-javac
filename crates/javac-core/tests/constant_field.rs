//! End-to-end regression test for lazy constant-field evaluation:
//! `static final int A = 1 + 2;` is attributed through
//! VarSymbol.setLazyConstValue -> Attr.attribLazyConstantValue, and the
//! generated class file carries a ConstantValue attribute (javac 17 behavior;
//! zero JVM at test time).

use javac_core::main::mini_compiler::MiniCompiler;
use javac_tools::tools::standard_java_file_manager::StandardJavaFileManager;
use javac_tools::tools::standard_location::StandardLocation;
use std::path::{Path, PathBuf};

fn write_source(root: &Path, relative: &str, source: &str) -> PathBuf {
    let path = root.join("src").join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("source directory");
    }
    std::fs::write(&path, source).expect("source file");
    path
}

fn class_bytes<'a>(classes: &'a [javac_core::main::mini_compiler::CompiledClass], name: &str) -> &'a [u8] {
    classes
        .iter()
        .find(|class| class.name == name)
        .map(|class| class.bytes.as_slice())
        .expect("generated class")
}

fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

#[test]
fn lazy_constant_field_gets_a_constant_value_attribute() {
    let root = std::env::temp_dir().join(format!(
        "rust-javac-constant-field-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let out = root.join("out");
    std::fs::create_dir_all(&out).expect("output directory");
    let source = write_source(
        &root,
        "C.java",
        "class C {\n    static final int A = 1 + 2;\n    static final String S = \"x\" + \"y\";\n}\n",
    );
    let mut compiler = MiniCompiler::new(&out);
    let source_path =
        StandardLocation::location_for(&javac_strings::JavaString::from("SOURCE_PATH"));
    compiler
        .file_manager
        .set_location(&*source_path, Some(&[root.join("src")]))
        .expect("set source path");
    let compiled = compiler
        .compile_files(&[source])
        .expect("constant field compilation");
    let names: Vec<&str> = compiled.iter().map(|class| class.name.as_str()).collect();
    assert!(names.contains(&"C"), "generated: {names:?}");
    assert_eq!(compiler.error_count(), 0);
    let bytes = class_bytes(&compiled, "C");
    assert!(
        contains(bytes, b"ConstantValue"),
        "lazy-constant field A must produce a ConstantValue attribute"
    );
    let _ = std::fs::remove_dir_all(&root);
}
