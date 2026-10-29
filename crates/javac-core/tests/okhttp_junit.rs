//! Run OkHttp's official JUnit tests against classes produced by MiniCompiler.
//!
//! Requires OKHTTP_ROOT and OKHTTP_DEPS. The first batch is the small core:
//! UtilTest, HeadersTest, HttpUrlTest, RequestTest (plus the
//! UrlComponentEncodingTester helper needed by HttpUrlTest).

use std::path::{Path, PathBuf};
use std::process::Command;

use javac_core::main::mini_compiler::MiniCompiler;
use javac_core::main::option::Option as JavacOption;

fn collect_java(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_java(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "java")
            && path.file_name().is_some_and(|name| name != "module-info.java")
        {
            out.push(path);
        }
    }
}

fn collect_test_classes(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_test_classes(&path, root, out);
        } else if path.extension().is_some_and(|ext| ext == "class") {
            let name = path.file_name().unwrap().to_string_lossy();
            if name.ends_with("Test.class") && !name.contains('$') {
                let rel = path.strip_prefix(root).expect("class under output");
                let mut class = rel.to_string_lossy().replace('\\', ".").replace('/', ".");
                class.truncate(class.len() - ".class".len());
                out.push(class);
            }
        }
    }
}

fn class_summary(stdout: &str) -> (bool, Option<u32>, Option<u32>) {
    if let Some(idx) = stdout.find("OK (") {
        let rest = &stdout[idx + 4..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        return (true, digits.parse().ok(), Some(0));
    }
    if let Some(idx) = stdout.find("Tests run: ") {
        let rest = &stdout[idx + "Tests run: ".len()..];
        let mut parts = rest.split(',');
        let tests = parts.next().and_then(|s| s.trim().parse().ok());
        let failures = parts
            .next()
            .and_then(|s| s.trim().strip_prefix("Failures: "))
            .and_then(|s| s.trim().parse().ok());
        return (false, tests, failures);
    }
    (false, None, None)
}

#[test]
fn okhttp_small_core_runs_under_junit() {
    let Some(root) = std::env::var_os("OKHTTP_ROOT") else {
        return;
    };
    let Some(deps) = std::env::var_os("OKHTTP_DEPS") else {
        return;
    };
    let root = PathBuf::from(root);
    let deps = PathBuf::from(deps);

    // HttpUrlTest needs okhttp/src/main/resources on the runtime classpath
    // (publicsuffixes.gz); otherwise PublicSuffixDatabase waits forever.
    let all_mode = std::env::var_os("OKHTTP_JUNIT_ALL").is_some();
    let mut all_classes: Vec<String> = Vec::new();
    let mut classes = vec![
        "okhttp3.internal.UtilTest",
        "okhttp3.HeadersTest",
        "okhttp3.HttpUrlTest",
        "okhttp3.RequestTest",
    ];
    if all_mode {
        classes.clear();
    }
    if std::env::var_os("OKHTTP_JUNIT_NETWORK").is_some() {
        classes.extend([
            "okhttp3.CallTest",
            "okhttp3.URLConnectionTest",
            "okhttp3.internal.ws.WebSocketHttpTest",
            "okhttp3.internal.tls.CertificatePinnerChainValidationTest",
        ]);
    }
    if std::env::var_os("OKHTTP_JUNIT_EXTRA").is_some() {
        // DiskLruCacheTest and HandshakeCertificatesTest fail identically on
        // the javac 17 baseline (JDK certificate store / timeout semantics),
        // so they are environment-sensitive and not part of this batch.
        classes.extend([
            "okhttp3.internal.cache2.RelayTest",
            "okhttp3.internal.connection.RouteExceptionTest",
            "okhttp3.internal.platform.PlatformTest",
            "okhttp3.mockwebserver.MockWebServerTest",
            "okhttp3.mockwebserver.RecordedRequestTest",
            "okhttp3.mockwebserver.CustomDispatcherTest",
            "okhttp3.tls.HeldCertificateTest",
        ]);
    }
    if std::env::var_os("OKHTTP_JUNIT_INTERNAL").is_some() {
        classes.extend([
            "okhttp3.internal.http2.HpackTest",
            "okhttp3.internal.http2.SettingsTest",
            "okhttp3.internal.http2.Http2ConnectionTest",
            "okhttp3.internal.http2.HuffmanTest",
            "okhttp3.internal.http2.FrameLogTest",
            "okhttp3.internal.ws.WebSocketReaderTest",
            "okhttp3.internal.ws.RealWebSocketTest",
            "okhttp3.internal.ws.WebSocketWriterTest",
            "okhttp3.internal.tls.DistinguishedNameParserTest",
            "okhttp3.internal.tls.HostnameVerifierTest",
            "okhttp3.internal.publicsuffix.PublicSuffixDatabaseTest",
            "okhttp3.internal.http.HttpDateTest",
            "okhttp3.internal.http.StatusLineTest",
            "okhttp3.internal.cache2.FileOperatorTest",
            "okhttp3.internal.connection.ConnectionSpecSelectorTest",
        ]);
    }

    let mut sources = Vec::new();
    if !all_mode {
    collect_java(&root.join("okhttp/src/main/java"), &mut sources);
    collect_java(&root.join("okhttp-generated"), &mut sources);
    collect_java(&root.join("okhttp-testing-support/src/main/java"), &mut sources);
    collect_java(&root.join("okhttp-tls/src/main/java"), &mut sources);
    for rel in [
        "okhttp-tests/src/test/java/okhttp3/internal/UtilTest.java",
        "okhttp-tests/src/test/java/okhttp3/HeadersTest.java",
        "okhttp-tests/src/test/java/okhttp3/HttpUrlTest.java",
        "okhttp-tests/src/test/java/okhttp3/RequestTest.java",
        "okhttp-tests/src/test/java/okhttp3/UrlComponentEncodingTester.java",
    ] {
        sources.push(root.join(rel));
    }
    if std::env::var_os("OKHTTP_JUNIT_INTERNAL").is_some() {
        for rel in [
            "okhttp-tests/src/test/java/okhttp3/internal/http2/HpackTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/http2/SettingsTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/http2/Http2ConnectionTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/http2/MockHttp2Peer.java",
            "okhttp-tests/src/test/java/okhttp3/internal/http2/HuffmanTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/http2/FrameLogTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/ws/WebSocketReaderTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/ws/WebSocketRecorder.java",
            "okhttp-tests/src/test/java/okhttp3/internal/ws/RealWebSocketTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/ws/WebSocketWriterTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/tls/DistinguishedNameParserTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/tls/HostnameVerifierTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/publicsuffix/PublicSuffixDatabaseTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/http/HttpDateTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/http/StatusLineTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/cache2/FileOperatorTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/connection/ConnectionSpecSelectorTest.java",
        ] {
            sources.push(root.join(rel));
        }
    }
    if std::env::var_os("OKHTTP_JUNIT_EXTRA").is_some() {
        collect_java(&root.join("mockwebserver/src/main/java"), &mut sources);
        for rel in [
            "okhttp-tests/src/test/java/okhttp3/internal/cache/DiskLruCacheTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/io/FaultyFileSystem.java",
            "okhttp-tests/src/test/java/okhttp3/internal/cache2/RelayTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/connection/RouteExceptionTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/platform/PlatformTest.java",
            "mockwebserver/src/test/java/okhttp3/mockwebserver/MockWebServerTest.java",
            "mockwebserver/src/test/java/okhttp3/mockwebserver/RecordedRequestTest.java",
            "mockwebserver/src/test/java/okhttp3/mockwebserver/CustomDispatcherTest.java",
            "okhttp-tls/src/test/java/okhttp3/tls/HandshakeCertificatesTest.java",
            "okhttp-tls/src/test/java/okhttp3/tls/HeldCertificateTest.java",
        ] {
            sources.push(root.join(rel));
        }
    }
    if std::env::var_os("OKHTTP_JUNIT_NETWORK").is_some() {
        collect_java(&root.join("mockwebserver/src/main/java"), &mut sources);
        collect_java(&root.join("okhttp-urlconnection/src/main/java"), &mut sources);
        for rel in [
            "okhttp-tests/src/test/java/okhttp3/RecordingEventListener.java",
            "okhttp-tests/src/test/java/okhttp3/TestLogHandler.java",
            "okhttp-tests/src/test/java/okhttp3/internal/DoubleInetAddressDns.java",
            "okhttp-tests/src/test/java/okhttp3/internal/RecordingOkAuthenticator.java",
            "okhttp-tests/src/test/java/okhttp3/internal/RecordingAuthenticator.java",
            "okhttp-tests/src/test/java/okhttp3/internal/http/RecordingProxySelector.java",
            "okhttp-tests/src/test/java/okhttp3/internal/platform/PlatformTest.java",
            "okhttp-tests/src/test/java/okhttp3/RecordingCallback.java",
            "okhttp-tests/src/test/java/okhttp3/RecordedResponse.java",
            "okhttp-tests/src/test/java/okhttp3/FallbackTestClientSocketFactory.java",
            "okhttp-tests/src/test/java/okhttp3/DelegatingSSLSocketFactory.java",
            "okhttp-tests/src/test/java/okhttp3/DelegatingSSLSocket.java",
            "okhttp-tests/src/test/java/okhttp3/DelegatingServerSocketFactory.java",
            "okhttp-tests/src/test/java/okhttp3/DelegatingSocketFactory.java",
            "okhttp-tests/src/test/java/okhttp3/internal/ws/WebSocketRecorder.java",
            "okhttp-tests/src/test/java/okhttp3/CallTest.java",
            "okhttp-tests/src/test/java/okhttp3/URLConnectionTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/ws/WebSocketHttpTest.java",
            "okhttp-tests/src/test/java/okhttp3/internal/tls/CertificatePinnerChainValidationTest.java",
        ] {
            sources.push(root.join(rel));
        }
    }
    }
    if all_mode {
        for module in [
            "okhttp/src/main/java",
            "okhttp-generated",
            "okhttp-testing-support/src/main/java",
            "okhttp-tls/src/main/java",
            "mockwebserver/src/main/java",
            "okhttp-urlconnection/src/main/java",
            "okhttp-logging-interceptor/src/main/java",
        ] {
            collect_java(&root.join(module), &mut sources);
        }
        for tests in [
            "okhttp-tests/src/test/java",
            "mockwebserver/src/test/java",
            "okhttp-tls/src/test/java",
            "okhttp-urlconnection/src/test/java",
        ] {
            collect_java(&root.join(tests), &mut sources);
        }
    }
    sources.sort();

    let out = std::env::temp_dir().join(format!("rust-javac-okhttp-junit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("junit output directory");

    let mut jars = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&deps) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "jar") {
                jars.push(path.to_string_lossy().to_string());
            }
        }
    }
    jars.sort();
    assert!(!jars.is_empty(), "no dependency jars in {deps:?}");
    let compile_cp = jars.join(";");

    let mut compiler = MiniCompiler::new_with_options(&out, |options| {
        options.put_option(JavacOption::CLASS_PATH, compile_cp.clone());
    });
    let compiled = compiler
        .compile_files(&sources)
        .expect("small-core compilation");
    eprintln!(
        "[JUNIT] compile files={} errors={} classes={}",
        sources.len(),
        compiler.error_count(),
        compiled.len()
    );
    assert_eq!(compiler.error_count(), 0);
    assert!(!compiled.is_empty());
    if all_mode {
        collect_test_classes(&out, &out, &mut all_classes);
        all_classes.sort();
        eprintln!("[JUNIT] all_mode test classes={}", all_classes.len());
    }

    let mut run_cp = out.to_string_lossy().to_string();
    for jar in &jars {
        run_cp.push(';');
        run_cp.push_str(jar);
    }
    for resources in [root.join("okhttp/src/main/resources")] {
        if resources.is_dir() {
            run_cp.push(';');
            run_cp.push_str(&resources.to_string_lossy());
        }
    }
    let java = std::env::var("JAVA_HOME")
        .map(|home| PathBuf::from(home).join("bin/java.exe"))
        .unwrap_or_else(|_| PathBuf::from("java"));

    let mut failures = Vec::new();
    let class_list: Vec<String> = if all_mode {
        all_classes.clone()
    } else {
        classes.iter().map(|class| class.to_string()).collect()
    };
    for class in &class_list {
        let output = Command::new(&java)
            .arg("-cp")
            .arg(&run_cp)
            .arg("org.junit.runner.JUnitCore")
            .arg(class)
            .output()
            .expect("run JUnitCore");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let (ok, tests, failed) = class_summary(&stdout);
        eprintln!(
            "[JUNIT] class={class} status={} tests={:?} failures={:?} exit={:?}",
            if ok { "ok" } else { "failed" },
            tests,
            failed,
            output.status
        );
        if !ok {
            eprintln!("[JUNIT-FAILURE] class={class}\n{stdout}\n{stderr}");
            failures.push(class.to_string());
        }
    }
    assert!(failures.is_empty(), "failed test classes: {failures:?}");
}
