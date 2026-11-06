//! Build-system provider registry and the shared load contract.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::model::ProjectModel;

/// Identity of a build system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuildSystemId(pub &'static str);

impl BuildSystemId {
    pub const MAVEN: Self = Self("maven");
    pub const NONE: Self = Self("none");

    pub fn as_str(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for BuildSystemId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Cooperative cancellation for long-running model loads.
#[derive(Debug, Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Inputs every provider receives.
#[derive(Debug, Clone)]
pub struct LoadContext {
    pub cancel: CancelFlag,
    pub timeout: Duration,
}

impl Default for LoadContext {
    fn default() -> Self {
        Self {
            cancel: CancelFlag::new(),
            timeout: Duration::from_secs(30),
        }
    }
}

/// Why a provider could not produce a model.
#[derive(Debug)]
pub enum LoadError {
    /// The provider does not apply to this root.
    NotDetected,
    Unsupported {
        detail: String,
    },
    Invalid(String),
    External(String),
    Timeout,
    Io(std::io::Error),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotDetected => formatter.write_str("build system not detected"),
            Self::Unsupported { detail } => write!(formatter, "unsupported: {detail}"),
            Self::Invalid(detail) => write!(formatter, "invalid project: {detail}"),
            Self::External(detail) => write!(formatter, "external model failed: {detail}"),
            Self::Timeout => formatter.write_str("timed out"),
            Self::Io(error) => write!(formatter, "io error: {error}"),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for LoadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// One build system integration.
pub trait BuildSystemProvider: Send + Sync {
    fn id(&self) -> BuildSystemId;

    /// Whether this provider applies to `root`.
    fn detect(&self, root: &Path) -> bool;

    /// Build the project model. Synchronous; the caller owns threading.
    fn load(&self, root: &Path, ctx: &LoadContext) -> Result<ProjectModel, LoadError>;

    /// Files whose changes should invalidate the model.
    fn watch_paths(&self, root: &Path) -> Vec<PathBuf>;
}

/// Load with a caller-supplied registry, most specific provider first.
///
/// Undetected or failing providers fall through to the next one; an empty
/// result produces an empty model.
pub fn load_with(
    registry: &[Box<dyn BuildSystemProvider>],
    root: &Path,
    ctx: &LoadContext,
) -> ProjectModel {
    for provider in registry {
        if !provider.detect(root) {
            continue;
        }
        if let Ok(model) = provider.load(root, ctx) {
            return model;
        }
    }
    ProjectModel::empty(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Failing;

    impl BuildSystemProvider for Failing {
        fn id(&self) -> BuildSystemId {
            BuildSystemId("failing")
        }
        fn detect(&self, _root: &Path) -> bool {
            true
        }
        fn load(&self, _root: &Path, _ctx: &LoadContext) -> Result<ProjectModel, LoadError> {
            Err(LoadError::Invalid("boom".to_string()))
        }
        fn watch_paths(&self, _root: &Path) -> Vec<PathBuf> {
            Vec::new()
        }
    }

    struct Fallback;

    impl BuildSystemProvider for Fallback {
        fn id(&self) -> BuildSystemId {
            BuildSystemId::NONE
        }
        fn detect(&self, _root: &Path) -> bool {
            true
        }
        fn load(&self, root: &Path, _ctx: &LoadContext) -> Result<ProjectModel, LoadError> {
            Ok(ProjectModel::empty(root))
        }
        fn watch_paths(&self, _root: &Path) -> Vec<PathBuf> {
            Vec::new()
        }
    }

    #[test]
    fn load_with_falls_through_failing_providers() {
        let registry: Vec<Box<dyn BuildSystemProvider>> =
            vec![Box::new(Failing), Box::new(Fallback)];
        let model = load_with(&registry, Path::new("target/tmp"), &LoadContext::default());
        assert_eq!(model.provider, BuildSystemId::NONE);
    }

    #[test]
    fn load_with_returns_an_empty_model_for_an_empty_registry() {
        let model = load_with(&[], Path::new("target/tmp"), &LoadContext::default());
        assert_eq!(model.provider, BuildSystemId::NONE);
        assert!(model.modules.is_empty());
    }
}
