//! JDK source lookup for go-to-definition.

use std::path::{Path, PathBuf};

use javac_zip::ZipArchive;

use super::positions::{byte_to_utf16, offset_to_line_col};
use super::source_files::{simple_class_name, source_relative_path};

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
    let extracted = super::source_files::extract(&bytes, "jdk-src", &entry_name, output_dir)?;
    Some(location_at_offset(&text, &extracted, offset))
}

/// Find a member declaration in the JDK src.zip for `class_flat`.
///
/// JDK 9+ src.zip entries are prefixed with the module name
/// (`java.base/java/lang/Object.java`), so match the class path as a suffix.
pub(crate) fn jdk_member_location(
    jdk_src: &Path,
    class_flat: &str,
    member: &str,
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    let (text, extracted) = class_source_text(jdk_src, class_flat, output_dir)?;
    let offset = super::external_sources::locate_member(&text, member)?;
    Some(location_at_offset(&text, &extracted, offset))
}

/// Find a constructor declaration in the JDK src.zip for `class_flat`.
///
/// `arity` selects the overload by declared parameter count; the caller
/// degrades to [`jdk_class_location`] when the source text cannot pick a
/// unique declaration.
pub(crate) fn jdk_constructor_location(
    jdk_src: &Path,
    class_flat: &str,
    arity: usize,
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    let (text, extracted) = class_source_text(jdk_src, class_flat, output_dir)?;
    let simple = simple_class_name(class_flat);
    let offset = super::external_sources::locate_constructor(&text, simple, Some(arity))?;
    Some(location_at_offset(&text, &extracted, offset))
}

/// Find the type declaration of `class_flat` in the JDK src.zip.
pub(crate) fn jdk_class_location(
    jdk_src: &Path,
    class_flat: &str,
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    let (text, extracted) = class_source_text(jdk_src, class_flat, output_dir)?;
    let simple = simple_class_name(class_flat);
    // `locate_member` prefers the type declaration (`class X`) over members.
    let offset = super::external_sources::locate_member(&text, simple)?;
    Some(location_at_offset(&text, &extracted, offset))
}

/// Read `class_flat`'s source from the JDK src.zip and extract it under the
/// session output directory. Returns the source text and the extracted path.
fn class_source_text(
    jdk_src: &Path,
    class_flat: &str,
    output_dir: &Path,
) -> Option<(String, PathBuf)> {
    if !jdk_src.exists() {
        return None;
    }
    let relative = source_relative_path(class_flat)?;
    let archive = ZipArchive::open(jdk_src).ok()?;
    let suffix = format!("/{relative}");
    let entry_name = archive
        .entries()
        .iter()
        .map(|entry| entry.name().to_string())
        .find(|name| name == &relative || name.ends_with(&suffix))?;
    let bytes = archive.get_entry(&entry_name)?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let extracted = super::source_files::extract(&bytes, "jdk-src", &entry_name, output_dir)?;
    Some((text, extracted))
}

/// LSP location of a byte offset in an extracted JDK source file.
///
/// The byte offset becomes a UTF-16 offset before the line/column conversion,
/// so non-ASCII source lines keep the editor cursor on the identifier.
fn location_at_offset(
    text: &str,
    file: &Path,
    offset: usize,
) -> javac_analysis_api::Location {
    let offset = byte_to_utf16(text, offset);
    let (line, column) = offset_to_line_col(text, offset);
    javac_analysis_api::Location {
        file: file.to_path_buf(),
        range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(line, column)),
    }
}

/// The default JDK src.zip location. Delegates to the single locator in
/// [`super::source_files`] (union of RAVAC_PLATFORM_DIR / RAVAC_JDK_SRC /
/// JAVA_HOME); no second locator is kept here.
pub(crate) fn default_jdk_src() -> Option<PathBuf> {
    super::source_files::jdk_src_zip()
}
