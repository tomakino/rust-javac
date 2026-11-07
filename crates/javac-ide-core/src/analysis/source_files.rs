//! Shared source-file lookup for JDK `src.zip` and class-path `-sources.jar`
//! archives (SSOT: document links, hover/super-method locations and external
//! source navigation all go through these helpers).

use std::path::{Path, PathBuf};

use javac_zip::ZipArchive;

/// Zip entry name for `relative`; JDK 9+ `src.zip` entries carry a module
/// prefix (`java.base/java/util/List.java`), so match as a suffix too.
pub(crate) fn zip_entry(archive: &ZipArchive, relative: &str) -> Option<String> {
    let suffix = format!("/{relative}");
    archive
        .entries()
        .iter()
        .map(|entry| entry.name().to_string())
        .find(|name| name == relative || name.ends_with(&suffix))
}

/// Source-level simple name of a binary class name:
/// `java.util.Map$Entry` -> `Entry`.
pub(crate) fn simple_class_name(class_flat: &str) -> &str {
    class_flat.rsplit(['.', '$']).next().unwrap_or(class_flat)
}

/// Source file holding `class_flat`'s declaration, relative to a source root:
/// `java.lang.RuntimeException` -> `java/lang/RuntimeException.java`;
/// a nested class lives in its outermost class's file (`Map$Entry` ->
/// `java/util/Map.java`).
pub(crate) fn source_relative_path(class_flat: &str) -> Option<String> {
    // Nested classes live in the outer class's source file.
    let file_class = class_flat.split('$').next().unwrap_or(class_flat);
    if file_class.is_empty() {
        return None;
    }
    Some(format!("{}.java", file_class.replace('.', "/")))
}

/// Extract `bytes` under `<cwd>/<output_dir>/<base_dir>/<entry_name>`.
pub(crate) fn extract(
    bytes: &[u8],
    base_dir: &str,
    entry_name: &str,
    output_dir: &Path,
) -> Option<PathBuf> {
    let extracted = std::env::current_dir()
        .ok()?
        .join(output_dir)
        .join(base_dir)
        .join(entry_name);
    if let Some(parent) = extracted.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    std::fs::write(&extracted, bytes).ok()?;
    Some(extracted)
}

/// The `-sources.jar` next to a binary jar, when it exists.
pub(crate) fn sources_jar(entry: &Path) -> Option<PathBuf> {
    let name = entry.file_name()?.to_string_lossy();
    if !name.ends_with(".jar") || name.ends_with("-sources.jar") {
        return None;
    }
    let stem = name.strip_suffix(".jar")?;
    let candidate = entry.with_file_name(format!("{stem}-sources.jar"));
    candidate.is_file().then_some(candidate)
}

/// The single JDK `src.zip` locator. Union of the layouts used by the
/// platform bundle, tests and a standard JDK installation:
/// `RAVAC_PLATFORM_DIR/src.zip`, then `RAVAC_JDK_SRC`, then
/// `JAVA_HOME/lib/src.zip`. `jdk_sources::default_jdk_src` forwards here.
pub(crate) fn jdk_src_zip() -> Option<PathBuf> {
    if let Some(platform) = std::env::var_os("RAVAC_PLATFORM_DIR") {
        let candidate = PathBuf::from(platform).join("src.zip");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Some(path) = std::env::var_os("RAVAC_JDK_SRC") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }
    let home = std::env::var_os("JAVA_HOME")?;
    let candidate = PathBuf::from(home).join("lib").join("src.zip");
    candidate.is_file().then_some(candidate)
}
