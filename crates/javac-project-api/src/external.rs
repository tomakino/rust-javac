//! Generic external build-model process.
//!
//! Some build systems (for example Gradle's Tooling API) produce the model in
//! an external process. [`ExternalModelRunner`] runs a program in the project
//! root, waits with cancellation/timeout, and decodes its JSON stdout into
//! [`serde::de::DeserializeOwned`] types. No provider in this crate uses it
//! yet; it exists so a future integration stays inside the same contract.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;

use super::provider::{CancelFlag, LoadError};

pub struct ExternalModelRunner {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub timeout: Duration,
}

impl ExternalModelRunner {
    pub fn new(program: impl Into<PathBuf>, args: Vec<String>, timeout: Duration) -> Self {
        Self {
            program: program.into(),
            args,
            timeout,
        }
    }

    /// Run the program and decode its JSON stdout.
    pub fn run<T: DeserializeOwned>(
        &self,
        root: &Path,
        cancel: &CancelFlag,
    ) -> Result<T, LoadError> {
        if cancel.is_cancelled() {
            return Err(LoadError::Timeout);
        }
        let mut child = Command::new(&self.program)
            .args(&self.args)
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(LoadError::Io)?;

        let started = Instant::now();
        loop {
            if cancel.is_cancelled() {
                let _ = child.kill();
                return Err(LoadError::Timeout);
            }
            match child.try_wait().map_err(LoadError::Io)? {
                Some(status) => {
                    let mut stdout = Vec::new();
                    if let Some(mut pipe) = child.stdout.take() {
                        pipe.read_to_end(&mut stdout).map_err(LoadError::Io)?;
                    }
                    let mut stderr = String::new();
                    if let Some(mut pipe) = child.stderr.take() {
                        pipe.read_to_string(&mut stderr).map_err(LoadError::Io)?;
                    }
                    if !status.success() {
                        let detail = if stderr.trim().is_empty() {
                            format!("external process exited with {status}")
                        } else {
                            stderr.trim().to_string()
                        };
                        return Err(LoadError::External(detail));
                    }
                    return serde_json::from_slice(&stdout).map_err(|error| {
                        LoadError::External(format!("invalid external model: {error}"))
                    });
                }
                None => {
                    if started.elapsed() >= self.timeout {
                        let _ = child.kill();
                        return Err(LoadError::Timeout);
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Payload {
        value: u32,
    }

    fn echo_runner(value: &str) -> ExternalModelRunner {
        #[cfg(windows)]
        {
            ExternalModelRunner::new(
                "powershell",
                vec![
                    "-NoProfile".into(),
                    "-Command".into(),
                    format!("Write-Output '{value}'"),
                ],
                Duration::from_secs(5),
            )
        }
        #[cfg(not(windows))]
        {
            ExternalModelRunner::new(
                "sh",
                vec!["-c".into(), format!("echo '{value}'")],
                Duration::from_secs(5),
            )
        }
    }

    #[test]
    fn decodes_json_stdout() {
        let runner = echo_runner("{\"value\":7}");
        let payload: Payload = runner.run(Path::new("."), &CancelFlag::new()).expect("payload");
        assert_eq!(payload.value, 7);
    }

    #[test]
    fn missing_program_is_an_io_error() {
        let runner = ExternalModelRunner::new(
            "definitely-not-a-real-program-xyz",
            Vec::new(),
            Duration::from_secs(1),
        );
        let result: Result<Payload, LoadError> =
            runner.run(Path::new("."), &CancelFlag::new());
        assert!(matches!(result, Err(LoadError::Io(_))));
    }

    #[test]
    fn cancelled_before_spawn_returns_timeout() {
        let cancel = CancelFlag::new();
        cancel.cancel();
        let runner = echo_runner("{\"value\":1}");
        let result: Result<Payload, LoadError> = runner.run(Path::new("."), &cancel);
        assert!(matches!(result, Err(LoadError::Timeout)));
    }
}
