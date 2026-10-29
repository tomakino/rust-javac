use javac_core::main::mini_compiler::MiniCompiler;
#[test]
fn diamond_in_method_argument_uses_target_type() {
 let dir=std::env::temp_dir().join(format!("rust-javac-diamond-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);std::fs::create_dir_all(&dir).unwrap();let path=dir.join("T.java");std::fs::write(&path,"import java.util.*; import java.util.concurrent.*; class T { Set<String> s = Collections.newSetFromMap(new ConcurrentHashMap<>()); }").unwrap();let mut c=MiniCompiler::new(&dir);if let Err(m)=c.compile_files(std::slice::from_ref(&path)){panic!("{m}");}
}
