//! Formatting through google-java-format.

use super::super::AnalysisSession;

impl AnalysisSession {
    /// Format the whole document with google-java-format.
    pub(crate) fn format_impl(&mut self, _file: &std::path::Path, text: &str) -> Option<String> {
        format_with_google_java_format(text)
    }
}

/// Format Java source through google-java-format when available.
fn format_with_google_java_format(text: &str) -> Option<String> {
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::Stdio;

    let jar = std::env::var_os("RAVAC_FORMAT_JAR")
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| {
            let exe = std::env::current_exe().ok()?;
            let candidate = exe.parent()?.join("tools").join("google-java-format.jar");
            candidate.exists().then_some(candidate)
        })
        .or_else(|| {
            let cwd = std::env::current_dir().ok()?;
            let candidate = cwd
                .join("target")
                .join("tools")
                .join("google-java-format.jar");
            candidate.exists().then_some(candidate)
        })?;
    let java = std::env::var_os("JAVA_HOME")
        .map(|home| PathBuf::from(home).join("bin").join("java.exe"))
        .filter(|java| java.exists())
        .unwrap_or_else(|| PathBuf::from("java"));
    let mut child = std::process::Command::new(java)
        .arg("-jar")
        .arg(&jar)
        .arg("--aosp")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    {
        let stdin = child.stdin.as_mut()?;
        stdin.write_all(text.as_bytes()).ok()?;
    }
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}
