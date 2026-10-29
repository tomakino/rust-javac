use javac_core::main::mini_compiler::MiniCompiler;
#[test]
fn generic_socket_option_override() {
 let dir=std::env::temp_dir().join(format!("rust-javac-override-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);std::fs::create_dir_all(&dir).unwrap();let p=dir.join("D.java");
 std::fs::write(&p,r#"import java.net.*; import java.io.*;
class D extends Socket {
  public <T> Socket setOption(SocketOption<T> name, T value) throws IOException { return this; }
  public <T> T getOption(SocketOption<T> name) throws IOException { return null; }
}"#).unwrap();
 let mut c=MiniCompiler::new(&dir);let _=c.compile_files(std::slice::from_ref(&p));
}
