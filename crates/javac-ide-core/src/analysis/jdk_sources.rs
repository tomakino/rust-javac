//! JDK source lookup for go-to-definition.

use std::path::{Path, PathBuf};

use javac_zip::ZipArchive;

use super::positions::offset_to_line_col;

/// Find the JDK source declaration of a simple name in the JDK src.zip.
///
/// The matching entry is extracted under the session output directory so the
/// editor can open it as a normal file.
pub(crate) fn jdk_source_location(
    jdk_src: &Path,
    word: &str,
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    if !jdk_src.exists() {
        return None;
    }
    let archive = ZipArchive::open(jdk_src).ok()?;
    let suffix = format!("/{word}.java");
    let entry_name = archive
        .entries()
        .iter()
        .map(|entry| entry.name().to_string())
        .find(|name| name.ends_with(&suffix) || name == &format!("{word}.java"))?;
    let bytes = archive.get_entry(&entry_name)?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let markers = [
        format!("class {word}"),
        format!("interface {word}"),
        format!("enum {word}"),
        format!("record {word}"),
    ];
    let offset = text
        .lines()
        .enumerate()
        .find(|(_, line)| markers.iter().any(|marker| line.contains(marker.as_str())))
        .map(|(index, _)| {
            text.lines()
                .take(index)
                .map(|line| line.len() + 1)
                .sum::<usize>()
        })?;
    let extracted = std::env::current_dir()
        .ok()?
        .join(output_dir)
        .join("jdk-src")
        .join(&entry_name);
    if let Some(parent) = extracted.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    std::fs::write(&extracted, &bytes).ok()?;
    let (line, column) = offset_to_line_col(&text, offset);
    Some(javac_analysis_api::Location {
        file: PathBuf::from(extracted),
        range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(line, column)),
    })
}

/// The default JDK src.zip location (JAVA_HOME/lib/src.zip).
///
/// RAVAC_JDK_SRC overrides the location for tests and non-standard layouts.
pub(crate) fn default_jdk_src() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("RAVAC_JDK_SRC") {
        let path = PathBuf::from(path);
        return path.exists().then_some(path);
    }
    let home = std::env::var_os("JAVA_HOME")?;
    let path = PathBuf::from(home).join("lib").join("src.zip");
    path.exists().then_some(path)
}
