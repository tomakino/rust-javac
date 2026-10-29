use javac_core::main::mini_compiler::MiniCompiler;

/// Regression: an interface field initialized by a lambda must be lowered
/// without re-entering visit_var_def_l2m on the VARDEF itself.
#[test]
fn interface_field_lambda_desugar() {
    let dir = std::env::temp_dir().join(format!("rust-javac-dns-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Dns.java");
    std::fs::write(
        &path,
        r#"import java.net.*;
import java.util.*;
interface Dns {
  Dns SYSTEM = hostname -> {
    if (hostname == null) throw new UnknownHostException("hostname == null");
    return Arrays.asList(InetAddress.getAllByName(hostname));
  };
  List<InetAddress> lookup(String hostname) throws UnknownHostException;
}"#,
    )
    .unwrap();
    let mut compiler = MiniCompiler::new(&dir);
    let result = compiler.compile_files(std::slice::from_ref(&path));
    assert!(result.is_ok(), "compile failed: {:?}", result.err());
    assert_eq!(compiler.error_count(), 0);
}
