// PORTING ADAPTER (not a 1:1 port)
//
// Adapter for: java.nio.file (Path / Files subset used by javac.file.*).
// Reference behavior: java.nio.file.Path/Files of the default FileSystem, as
// used by com.sun.tools.javac.file (BaseFileManager, JavacFileManager,
// Locations, PathFileObject, FSInfo, RelativePath, CacheFSInfo).  The oracle
// for the Windows behavior is the baseline JDK 17.0.19 (Temurin 17.0.19+10),
// recorded through a development-time Java program; the recorded expectations
// are embedded in the tests, so the tests run without a JVM.
//
//! Minimal java.nio.file platform layer for the P2 file layer.
//!
//! PORTING_RULES:
//! * Path wraps [std::path::PathBuf]; the API keeps the Java method names in
//!   snake_case (is_absolute, to_absolute_path, normalize, relativize, resolve,
//!   starts_with, ends_with, file_name, parent, root, to_uri, from_uri).
//! * Windows semantics are implemented and oracle-tested (separator '\\',
//!   case-insensitive starts_with/ends_with/relativize/equality, drive-relative
//!   paths, empty path).  Linux differences are documented per method and the
//!   Windows-only oracle tests are gated with #[cfg(windows)]; the generic
//!   path-syntax tests also run on Linux (relative paths, normalize, resolve).
//! * declared IOException -> Result<_, Error>; InvalidPathException is
//!   reported for NUL-containing input (matching java.nio.file); relativize
//!   across different roots reports IllegalArgumentException.
//! * no FileSystem/FileSystemProvider/PathMatcher virtual machine: javac's
//!   uses of those are out of scope for this layer (they stay in the P2 file
//!   layer, using javac-jimage/javac-zip/this crate).

pub mod error;
pub mod files;
pub mod path;

pub use error::Error;
pub use files::Files;
pub use path::Path;

