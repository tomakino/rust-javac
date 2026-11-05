//! Minimal platform data layout.
//!
//! The compiler needs exactly two files from the platform: modules (the
//! jimage with the system classes) and, only for --release, ct.sym.  Both
//! live directly next to the executable (flat layout).  The compiler never
//! scans the machine for a JDK: no JAVA_HOME, no PATH, no registry, no
//! well-known install directories.  RAVAC_PLATFORM_DIR may point at another
//! directory, but that directory must contain modules directly.

use std::path::{Path, PathBuf};

/// The modules image: <dir>/modules.
pub(crate) fn modules_file(dir: &Path) -> PathBuf {
    dir.join("modules")
}

/// The ct.sym signatures: <dir>/ct.sym.
pub(crate) fn ct_sym_file(dir: &Path) -> PathBuf {
    dir.join("ct.sym")
}

/// True when the directory holds the modules image.
pub(crate) fn has_modules(dir: &Path) -> bool {
    modules_file(dir).is_file()
}
