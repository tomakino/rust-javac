use javac_core::main::mini_compiler::MiniCompiler;
#[test]
fn unbound_method_reference_compareto() {
    let dir=std::env::temp_dir().join(format!("rust-javac-mref-{}",std::process::id()));
    let _=std::fs::remove_dir_all(&dir);std::fs::create_dir_all(&dir).unwrap();
    let path=dir.join("U.java");
    std::fs::write(&path,"import java.util.*; class U { Comparator<String> c = String::compareTo; }").unwrap();
    let mut comp=MiniCompiler::new(&dir);
    if let Err(m)=comp.compile_files(std::slice::from_ref(&path)){panic!("{m}");}
}
