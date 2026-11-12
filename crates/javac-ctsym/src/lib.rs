//! Read the JDK's `ct.sym` platform signatures.
//!
//! `ct.sym` is the compiler's view of every Java platform release the installed JDK
//! supports. It stores one signed class per (release, class) pair, but no
//! implementations: each entry is a class file with method bodies stripped.
//!
//! # Layout
//!
//! Entries are grouped by *section*, a directory named after the releases it serves.
//! Each character is a release number in base 36 — `8` for 8, `A` for 10, `H` for
//! 17 — so `879A` holds the signatures shared by releases 7, 8, 9 and 10. A section
//! name containing `-` is ignored.
//!
//! Below a section, entries are laid out by module and package:
//!
//! ```text
//! <section>/<module>/<package path>/<Class>.sig     releases 9 and newer
//! <section>/<package path>/<Class>.sig              releases 7 and 8
//! ```
//!
//! A section may instead hold a single `system-modules` list. Such a section
//! describes the running JDK image rather than a fixed signature set: callers must
//! read the image for that release, and [`CtSym::has_system_modules`] reports when.
//!
//! # Reading
//!
//! [`CtSym::open`] indexes the archive once; [`CtSym::get_sig`] returns the bytes for
//! a (release, class) pair. Class lookup is by release, never by "newest signature at
//! or below the release": each pair lives in exactly one section, and a class
//! introduced in 14 has no entry in 13.
//!
//! # Containers
//!
//! The container is detected from the file magic. The JDK's zip and ravac's RCS1
//! form are both accepted and behave identically; the compact form stores identical
//! payloads once and compresses them in blocks.
//!
//! # Concurrency and cost
//!
//! Opening reads the archive index once (about 2 MB for the JDK 17 archive). Payloads
//! are read on demand: `RCS1` decompresses at most 256 KiB per touched block and keeps
//! up to 64 blocks in an interior cache. That cache makes `CtSym` `!Sync`; use one
//! instance per thread, or share it behind a lock.
//!
//! # Example
//!
//! ```no_run
//! use javac_ctsym::CtSym;
//! use std::path::Path;
//!
//! let ct_sym = CtSym::open(Path::new("/path/to/jdk/lib/ct.sym"))?;
//! let class = ct_sym.get_sig(11, "java.lang.String");
//! assert!(class.is_some());
//! # Ok::<(), javac_ctsym::Error>(())
//! ```

#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use javac_zip::ZipArchive;

mod rcs;
pub use rcs::{RcsArchive, RcsBuilder, RcsError};

/// File name of the signature archive inside a JDK installation.
pub const CT_SYM_FILE_NAME: &str = "ct.sym";

/// Suffix of the signature class files.
pub const SIG_EXTENSION: &str = ".sig";

/// Name of the per-release module list that describes a running JDK image.
pub const SYSTEM_MODULES_NAME: &str = "system-modules";

/// The base-36 release character used in section names (`8` -> `8`, `10` -> `A`,
/// `17` -> `H`). `None` when the release is negative or needs more than one base-36
/// digit.
pub fn release_version_char(release: i32) -> Option<char> {
    let digit = u32::try_from(release).ok()?;
    char::from_digit(digit, 36).map(|ch| ch.to_ascii_uppercase())
}

/// Errors produced while opening or reading a ct.sym archive.
#[derive(Debug)]
pub enum Error {
    /// The file could not be read.
    Io(io::Error),
    /// The file header matches neither a zip archive nor an RCS1 container.
    NotAZip(String),
    /// The zip container is damaged or truncated.
    Corrupted(String),
    /// The RCS1 container is damaged or unsupported.
    Rcs(RcsError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(error) => write!(f, "I/O error: {}", error),
            Error::NotAZip(message) => write!(f, "invalid ct.sym archive: {}", message),
            Error::Corrupted(message) => write!(f, "corrupted ct.sym archive: {}", message),
            Error::Rcs(error) => write!(f, "{}", error),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(error) => Some(error),
            Error::Rcs(error) => Some(error),
            _ => None,
        }
    }
}

impl Error {
    /// Maps an RCS1 error onto the ct.sym error type.
    fn from_rcs(error: RcsError) -> Error {
        match error {
            RcsError::Io(error) => Error::Io(error),
            RcsError::NotAnArchive(message) => Error::NotAZip(message),
            RcsError::WrongVersion { .. } | RcsError::Corrupted(_) => {
                Error::Rcs(error)
            }
        }
    }
}

impl Error {
    /// Maps a javac-zip error onto the ct.sym error type.
    ///
    /// A compression method or zip64 extension outside the archive's supported shape means the
    /// container itself cannot be honoured, so it is reported as Corrupted.
    fn from_zip(error: javac_zip::Error) -> Error {
        match error {
            javac_zip::Error::Io(error) => Error::Io(error),
            javac_zip::Error::NotAZip(message) => Error::NotAZip(message),
            javac_zip::Error::Corrupted(message) => Error::Corrupted(message),
            javac_zip::Error::UnsupportedCompression(method) => {
                Error::Corrupted(format!("unsupported zip compression method {}", method))
            }
            javac_zip::Error::UnsupportedZip64(message) => {
                Error::Corrupted(format!("unsupported zip64 ct.sym: {}", message))
            }
        }
    }
}

/// One release directory of ct.sym.
#[derive(Debug, Clone)]
pub struct Section {
    name: String,
    releases: Vec<i32>,
    module_dirs: Vec<String>,
    sig_count: usize,
    system_modules: bool,
}

impl Section {
    /// The directory name: one character per release sharing the stored signatures
    /// (for example "879A").
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The releases described by the archive, ascending.
    pub fn releases(&self) -> &[i32] {
        &self.releases
    }

    /// The module directories below this release directory, in archive order.
    ///
    /// For release 9 and newer these name the modules of the system image. For release 8
    /// and older the layout has no module directory, so this list holds the leading
    /// package components (`java`, `javax`, ...) of the platform class path instead.
    pub fn module_dirs(&self) -> &[String] {
        &self.module_dirs
    }

    /// The number of .sig entries below this release directory.
    pub fn sig_count(&self) -> usize {
        self.sig_count
    }

    /// Whether this directory contains a "system-modules" file, i.e. describes the running JDK
    /// image instead of holding .sig files.
    pub fn has_system_modules(&self) -> bool {
        self.system_modules
    }
}

/// One signature entry (.sig class file) of ct.sym.
#[derive(Debug, Clone)]
pub struct SigEntry {
    section: String,
    module: String,
    class_name: String,
}

impl SigEntry {
    /// The release directory that stores the entry.
    pub fn section(&self) -> &str {
        &self.section
    }

    /// The module directory that stores the entry.
    pub fn module(&self) -> &str {
        &self.module
    }

    /// The binary (dot separated) class name of the entry, nested classes separated by '$'.
    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    /// The entry path inside the archive.
    pub fn entry_name(&self) -> String {
        format!(
            "{}/{}/{}{}",
            self.section,
            self.module,
            self.class_name.replace('.', "/"),
            SIG_EXTENSION
        )
    }
}

/// One archive entry, normalized across the zip and RCS1 containers.
struct EntryView {
    name: String,
    is_directory: bool,
}

/// The container holding the entries: the JDK's zip or the compact RCS1 format.
enum ArchiveBackend {
    Zip(ZipArchive),
    Rcs(RcsArchive),
}

impl ArchiveBackend {
    fn open(path: &Path) -> Result<ArchiveBackend, Error> {
        use std::io::Read;
        let mut magic = [0u8; 4];
        if let Ok(mut file) = std::fs::File::open(path)
            && file.read_exact(&mut magic).is_ok()
            && magic == rcs::MAGIC
        {
            return Ok(ArchiveBackend::Rcs(
                RcsArchive::open(path).map_err(Error::from_rcs)?,
            ));
        }
        Ok(ArchiveBackend::Zip(
            ZipArchive::open(path).map_err(Error::from_zip)?,
        ))
    }

    fn entries(&self) -> Vec<EntryView> {
        match self {
            ArchiveBackend::Zip(archive) => archive
                .entries()
                .iter()
                .map(|entry| EntryView {
                    name: entry.name().to_string(),
                    is_directory: entry.is_directory(),
                })
                .collect(),
            ArchiveBackend::Rcs(archive) => archive
                .entries()
                .iter()
                .map(|entry| EntryView {
                    name: entry.name().to_string(),
                    is_directory: entry.is_directory(),
                })
                .collect(),
        }
    }

    fn entry_count(&self) -> usize {
        match self {
            ArchiveBackend::Zip(archive) => archive.entries().len(),
            ArchiveBackend::Rcs(archive) => archive.entry_count(),
        }
    }

    fn get_entry(&self, name: &str) -> Option<Vec<u8>> {
        match self {
            ArchiveBackend::Zip(archive) => archive.get_entry(name),
            ArchiveBackend::Rcs(archive) => archive.get_entry(name),
        }
    }
}

/// A read-only view of a JDK ct.sym archive.
pub struct CtSym {
    path: PathBuf,
    archive: ArchiveBackend,
    sections: Vec<Section>,
    section_index: HashMap<String, usize>,
    releases: Vec<i32>,
    sig_entries: Vec<SigEntry>,
    class_index: HashMap<String, Vec<usize>>,
}

impl fmt::Debug for CtSym {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CtSym")
            .field("path", &self.path)
            .field("entries", &self.archive.entry_count())
            .field("sections", &self.sections.len())
            .field("releases", &self.releases)
            .field("sig_entries", &self.sig_entries.len())
            .finish()
    }
}

impl CtSym {
    /// Opens a ct.sym archive and builds the section and class indexes.
    ///
    /// Section names are decoded one base-36 character at a time; a name containing `-` is
    /// ignored and contributes no releases.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the file cannot be read, [`Error::NotAZip`] when the magic
    /// matches neither a zip archive nor a compact container, and [`Error::Corrupted`] or
    /// [`Error::Rcs`] when the index is inconsistent.
    ///
    /// # Panics
    ///
    /// Panics when a section name contains a character that is not a base-36 digit: the
    /// section cannot select a release, and dropping it silently would hide a broken
    /// archive.
    pub fn open(path: &Path) -> Result<CtSym, Error> {
        let archive = ArchiveBackend::open(path)?;
        Self::from_archive(path.to_path_buf(), archive)
    }

    /// Opens a `ct.sym` embedded in the binary instead of a file.
    ///
    /// The embedded form is the compact `RCS1` container (`cargo xtask
    /// gen-ct-sym`), never a zip.
    pub fn from_bytes(data: &'static [u8]) -> Result<CtSym, Error> {
        if data.len() < 4 || data[..4] != rcs::MAGIC {
            return Err(Error::NotAZip("<embedded ct.sym>".to_string()));
        }
        let archive = ArchiveBackend::Rcs(RcsArchive::from_bytes(data).map_err(Error::from_rcs)?);
        Self::from_archive(PathBuf::from("<embedded ct.sym>"), archive)
    }

    fn from_archive(path: PathBuf, archive: ArchiveBackend) -> Result<CtSym, Error> {
        let entries = archive.entries();

        // First pass: section names in archive order. Root-level files are visited too.
        let mut section_names: Vec<String> = Vec::new();
        let mut seen_sections: HashSet<&str> = HashSet::new();
        for entry in &entries {
            let name = entry.name.as_str();
            let section_name = name.split('/').next().unwrap_or(name);
            if seen_sections.insert(section_name) {
                section_names.push(section_name.to_string());
            }
        }

        // Decode every section; an invalid name panics here.
        let mut sections = Vec::with_capacity(section_names.len());
        let mut section_index = HashMap::with_capacity(section_names.len());
        for name in &section_names {
            let releases = decode_section_releases(name);
            section_index.insert(name.clone(), sections.len());
            sections.push(Section {
                name: name.clone(),
                releases,
                module_dirs: Vec::new(),
                sig_count: 0,
                system_modules: false,
            });
        }

        // Second pass: fill the release directories and index every .sig entry.
        let mut sig_entries: Vec<SigEntry> = Vec::new();
        let mut class_index: HashMap<String, Vec<usize>> = HashMap::new();
        for entry in &entries {
            if entry.is_directory {
                continue;
            }
            let name = entry.name.as_str();
            let Some((section_name, relative)) = name.split_once('/') else {
                continue;
            };
            let index = *section_index
                .get(section_name)
                .expect("section index is consistent with the section list");
            if let Some((module, path)) = relative.split_once('/') {
                if !name.ends_with(SIG_EXTENSION) {
                    // The layout only holds .sig files and the system-modules list.
                    continue;
                }
                let section = &mut sections[index];
                section.sig_count += 1;
                if !section.module_dirs.iter().any(|known| known == module) {
                    section.module_dirs.push(module.to_string());
                }
                let class_path = &path[..path.len() - SIG_EXTENSION.len()];
                let class_name = class_path.replace('/', ".");
                let entry_index = sig_entries.len();
                class_index
                    .entry(class_name.clone())
                    .or_default()
                    .push(entry_index);
                sig_entries.push(SigEntry {
                    section: section_name.to_string(),
                    module: module.to_string(),
                    class_name,
                });
            } else if relative == SYSTEM_MODULES_NAME {
                sections[index].system_modules = true;
            }
        }

        // The archive's release set, ascending.
        let mut release_set = BTreeSet::new();
        for section in &sections {
            release_set.extend(section.releases.iter().copied());
        }

        Ok(CtSym {
            path,
            archive,
            sections,
            section_index,
            releases: release_set.into_iter().collect(),
            sig_entries,
            class_index,
        })
    }

    /// The file this view was opened from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The number of archive entries, directories included.
    pub fn entry_count(&self) -> usize {
        self.archive.entry_count()
    }

    /// The sections in archive order.
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// The release numbers described by this archive, ascending.
    pub fn releases(&self) -> &[i32] {
        &self.releases
    }

    /// The section with exactly this directory name.
    pub fn section(&self, name: &str) -> Option<&Section> {
        self.section_index
            .get(name)
            .map(|&index| &self.sections[index])
    }

    /// Every signature entry, in archive order.
    pub fn all_sig_entries(&self) -> &[SigEntry] {
        &self.sig_entries
    }

    /// The signature entries available to a release, in archive order.
    ///
    /// Includes each module's `module-info.sig`; [`classes`](Self::classes) returns only
    /// class names.
    pub fn sig_entries(&self, release: i32) -> Vec<&SigEntry> {
        self.sig_entries
            .iter()
            .filter(|entry| self.section_releases(&entry.section).contains(&release))
            .collect()
    }

    /// The class names available to a release, in archive order, without duplicates.
    ///
    /// Module descriptors (`module-info`) are excluded; an unknown release yields an empty
    /// list.
    pub fn classes(&self, release: i32) -> Vec<String> {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut classes = Vec::new();
        for entry in &self.sig_entries {
            if entry.class_name == "module-info" {
                continue;
            }
            if !self.section_releases(&entry.section).contains(&release) {
                continue;
            }
            if seen.insert(entry.class_name.as_str()) {
                classes.push(entry.class_name.clone());
            }
        }
        classes
    }

    /// The module directories available to a release, in archive order, without
    /// duplicates.
    ///
    /// Before release 9 the archive has no module directories, so the returned names are
    /// package roots rather than modules; see [`Section::module_dirs`].
    pub fn modules(&self, release: i32) -> Vec<&str> {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut modules = Vec::new();
        for section in &self.sections {
            if !section.releases.contains(&release) {
                continue;
            }
            for module in &section.module_dirs {
                if seen.insert(module.as_str()) {
                    modules.push(module.as_str());
                }
            }
        }
        modules
    }

    /// The entry for `class` in `release`.
    ///
    /// `class` may be a binary name (`java.lang.Object`) or an internal name
    /// (`java/lang/Object`). A well-formed archive stores each (class, release) pair in
    /// exactly one section, so the result is normally unique; the first match in archive
    /// order is returned and duplicate entries are not detected. `None` when the release
    /// has no entry.
    pub fn find_sig(&self, release: i32, class: &str) -> Option<&SigEntry> {
        let key = class_key(class);
        self.class_index
            .get(key.as_ref())?
            .iter()
            .find_map(|&index| {
                let entry = &self.sig_entries[index];
                self.section_releases(&entry.section)
                    .contains(&release)
                    .then_some(entry)
            })
    }

    /// The signature bytes of `class` for `release`.
    ///
    /// The bytes are a class file with method bodies stripped.
    ///
    /// `None` when the release has no such class, or when the payload cannot be read: a
    /// damaged entry is reported exactly like a missing one. Each call returns a new `Vec`.
    /// With the RCS1 container the first read of a block decompresses up to 256 KiB and
    /// later reads copy from an in-memory block cache.
    pub fn get_sig(&self, release: i32, class: &str) -> Option<Vec<u8>> {
        let entry_name = self.find_sig(release, class)?.entry_name();
        self.archive.get_entry(&entry_name)
    }

    /// The signature bytes of `class` for `release`, restricted to `module`.
    ///
    /// Otherwise identical to [`get_sig`](Self::get_sig).
    pub fn get_sig_in_module(&self, release: i32, module: &str, class: &str) -> Option<Vec<u8>> {
        let key = class_key(class);
        let entry = self
            .class_index
            .get(key.as_ref())?
            .iter()
            .find_map(|&index| {
                let entry = &self.sig_entries[index];
                (entry.module.as_str() == module
                    && self.section_releases(&entry.section).contains(&release))
                .then_some(entry)
            })?;
        self.archive.get_entry(&entry.entry_name())
    }

    /// Whether `release` is described by the running JDK image (`system-modules`) instead
    /// of signatures.
    pub fn has_system_modules(&self, release: i32) -> bool {
        self.image_section(release)
            .is_some_and(|section| section.system_modules)
    }

    /// The module names the running JDK image lists for `release`.
    ///
    /// `None` when the release has no running-image section or no list.
    ///
    /// # Panics
    ///
    /// Panics when the list is not valid UTF-8; a malformed archive is not silently
    /// repaired.
    pub fn system_modules(&self, release: i32) -> Option<Vec<String>> {
        let section = self.image_section(release)?;
        if !section.system_modules {
            return None;
        }
        let entry_name = format!("{}/{}", section.name, SYSTEM_MODULES_NAME);
        let bytes = self.archive.get_entry(&entry_name)?;
        Some(read_lines(&bytes).unwrap_or_else(|error| {
            panic!(
                "ct.sym entry {} is not valid UTF-8: {}",
                entry_name, error
            )
        }))
    }

    /// The releases that provide a signature for `class`, ascending.
    pub fn releases_containing(&self, class: &str) -> Vec<i32> {
        let key = class_key(class);
        let Some(indices) = self.class_index.get(key.as_ref()) else {
            return Vec::new();
        };
        let mut releases = BTreeSet::new();
        for &index in indices {
            let entry = &self.sig_entries[index];
            releases.extend(self.section_releases(&entry.section).iter().copied());
        }
        releases.into_iter().collect()
    }

    /// The newest release that provides a signature for `class`.
    pub fn find_class_any_release(&self, class: &str) -> Option<i32> {
        self.releases_containing(class).into_iter().next_back()
    }

    /// The releases of a section name, from the section table.
    fn section_releases(&self, name: &str) -> &[i32] {
        let index = self
            .section_index
            .get(name)
            .expect("section index is consistent with the stored entries");
        &self.sections[*index].releases
    }

    /// The single-character section of a release, where the running-image module list lives.
    fn image_section(&self, release: i32) -> Option<&Section> {
        let version_char = release_version_char(release)?;
        self.sections.iter().find(|section| {
            let mut chars = section.name.chars();
            chars.next() == Some(version_char) && chars.next().is_none()
        })
    }
}

/// The binary (dot-separated) name form used by the class index; slash-separated names are
/// accepted and converted.
fn class_key(class: &str) -> Cow<'_, str> {
    if class.contains('/') {
        Cow::Owned(class.replace('/', "."))
    } else {
        Cow::Borrowed(class)
    }
}

/// Decodes the release numbers of a section name.
///
/// A name containing `-` is ignored and yields no releases. A character that is not a
/// base-36 digit panics. A base-36 digit that this crate does not model as a release (for
/// example `1` or `I`) is skipped.
fn decode_section_releases(name: &str) -> Vec<i32> {
    if name.contains('-') {
        return Vec::new();
    }
    let mut releases = Vec::new();
    for ch in name.chars() {
        let value = base36_value(ch).unwrap_or_else(|| {
            panic!(
                "ct.sym section {:?} is not a base-36 name",
                name
            )
        });
        if let Some(release) = target_numeric_version(value) {
            releases.push(release);
        }
    }
    releases.sort_unstable();
    releases.dedup();
    releases
}

/// The value of a base-36 digit character.
fn base36_value(ch: char) -> Option<u8> {
    match ch {
        '0'..='9' => Some(ch as u8 - b'0'),
        'a'..='z' => Some(ch as u8 - b'a' + 10),
        'A'..='Z' => Some(ch as u8 - b'A' + 10),
        _ => None,
    }
}

/// The release number modelled for a base-36 digit value.
///
/// This crate models releases 5 through 17. Releases before 5 are spelled `1.1`..`1.4` in
/// section names, and 18 and newer lie outside the modelled range, so both are skipped.
fn target_numeric_version(value: u8) -> Option<i32> {
    if (5..=17).contains(&value) {
        Some(i32::from(value))
    } else {
        None
    }
}

/// Splits text into lines on `\n`, `\r` and `\r\n`; a trailing terminator does not produce an
/// empty final line.
fn read_lines(bytes: &[u8]) -> Result<Vec<String>, std::str::Utf8Error> {
    let text = std::str::from_utf8(bytes)?;
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\n' => lines.push(std::mem::take(&mut current)),
            '\r' => {
                if chars.as_str().starts_with('\n') {
                    chars.next();
                }
                lines.push(std::mem::take(&mut current));
            }
            other => current.push(other),
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    // Recorded from the baseline JDK 17.0.19 (Temurin 17.0.19+10) lib/ct.sym: every sampled
    // entry below was read back and has exactly the recorded length and FNV-1a 64 value.
    const EXPECTED_ENTRY_COUNT: usize = 15710;
    const EXPECTED_SIG_COUNT: usize = 13652;
    const EXPECTED_RELEASES: [i32; 11] = [7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17];

    /// (release directory name, .sig entry count, FNV-1a 64 of the module directory names).
    /// A section with no module directory hashes to the FNV-1a offset basis,
    /// `-3750763034362895579`, which is why that value repeats below.
    const EXPECTED_SECTIONS: &[(&str, usize, i64)] = &[
        ("7", 326, 7792818733320166904),
        ("79A", 7, 8757203050431131106),
        ("8", 334, 8532651434829324),
        ("87", 1058, -2736047172782108242),
        ("879", 21, -292396228109880930),
        ("879A", 2922, -1038738797983208662),
        ("879AB", 2, 8757203050431131106),
        ("879ABC", 1, 8757203050431131106),
        ("879ABCD", 1, 8757203050431131106),
        ("879ABCDE", 1, 8757203050431131106),
        ("879ABCDEF", 4, -7386790205313625273),
        ("879ABCDEFG", 90, 7250595889213142289),
        ("89", 10, 8757203050431131106),
        ("89A", 182, 570962484516306742),
        ("89ABC", 1, 8757203050431131106),
        ("89ABCDEF", 4, 8757203050431131106),
        ("89ABCDEFG", 30, 8757203050431131106),
        ("9", 178, -8280845046694629346),
        ("9A", 1783, -8521503377795548872),
        ("9AB", 8, -3965427469478767907),
        ("9ABC", 20, -8636962377173915878),
        ("9ABCDE", 6, 6760493622339926202),
        ("9ABCDEF", 24, -2350716230094111235),
        ("9ABCDEFG", 183, -5425990566018445156),
        ("A", 172, 6813672009161469515),
        ("AB", 8, 6079295720706207954),
        ("ABC", 2, -8636962377173915878),
        ("ABCD", 2, 795144819590431177),
        ("ABCDEF", 1, -7386790205313625273),
        ("ABCDEFG", 16, 3904923253033284050),
        ("B", 80, 4976932462243244039),
        ("BC", 330, -1333038536174672657),
        ("BCD", 45, -2047110622842896610),
        ("BCDE", 116, 8792961806299346110),
        ("BCDEF", 1039, 403512697778647286),
        ("BCDEFG", 2748, 2889769768192010042),
        ("BCDG", 2, 4169008022551082254),
        ("BDEF", 1, 7328214563642397275),
        ("C", 33, 2107565965323933286),
        ("CD", 6, -2508752599149893319),
        ("CDE", 1, 8757203050431131106),
        ("CDEF", 16, 1178003395819695927),
        ("CDEFG", 65, -1464547515265358172),
        ("D", 26, 2091782134421934175),
        ("DE", 12, -8636962377173915878),
        ("DEF", 20, -7581071285721464824),
        ("DEFG", 299, -6814269898099906500),
        ("E", 34, -5008809175885664976),
        ("EF", 29, -3776632783209281518),
        ("EFG", 37, 4231315468668058633),
        ("F", 39, -8487782479609445294),
        ("FG", 63, 614743087525428810),
        ("G", 1214, 7769882600355422574),
        ("H", 0, -3750763034362895579),
    ];

    /// Per release: (.sig entries, FNV of their entry names, class names, FNV of the class
    /// names, module-info entries, module count, FNV of the module names).
    struct ExpectedRelease {
        release: i32,
        sig_entries: usize,
        sig_entries_fnv: i64,
        classes: usize,
        classes_fnv: i64,
        module_infos: usize,
        modules: usize,
        modules_fnv: i64,
    }

    const EXPECTED_RELEASES_TABLE: &[ExpectedRelease] = &[
        ExpectedRelease {
            release: 7,
            sig_entries: 4433,
            sig_entries_fnv: 8615239435891015708,
            classes: 4433,
            classes_fnv: 4968106950679544085,
            module_infos: 0,
            modules: 29,
            modules_fnv: -2265994986763012122,
        },
        ExpectedRelease {
            release: 8,
            sig_entries: 4661,
            sig_entries_fnv: -4903847196233257718,
            classes: 4661,
            classes_fnv: 8947010510533641026,
            module_infos: 0,
            modules: 30,
            modules_fnv: -6721757467674400871,
        },
        ExpectedRelease {
            release: 9,
            sig_entries: 5478,
            sig_entries_fnv: -4599902814794367768,
            classes: 5413,
            classes_fnv: 3383187479132315910,
            module_infos: 65,
            modules: 65,
            modules_fnv: 4837512625209421126,
        },
        ExpectedRelease {
            release: 10,
            sig_entries: 5470,
            sig_entries_fnv: -5374199880351154475,
            classes: 5406,
            classes_fnv: -2170881719611769110,
            module_infos: 64,
            modules: 64,
            modules_fnv: -327723386611205437,
        },
        ExpectedRelease {
            release: 11,
            sig_entries: 4765,
            sig_entries_fnv: 8448287124026350851,
            classes: 4705,
            classes_fnv: 6424821974009885356,
            module_infos: 60,
            modules: 60,
            modules_fnv: 2454269990958225735,
        },
        ExpectedRelease {
            release: 12,
            sig_entries: 4787,
            sig_entries_fnv: 2348334183814083478,
            classes: 4727,
            classes_fnv: 6239494893323560573,
            module_infos: 60,
            modules: 60,
            modules_fnv: -3388820469750179561,
        },
        ExpectedRelease {
            release: 13,
            sig_entries: 4758,
            sig_entries_fnv: 7548640608610301705,
            classes: 4698,
            classes_fnv: 1383530514434311723,
            module_infos: 60,
            modules: 60,
            modules_fnv: -192478988901972383,
        },
        ExpectedRelease {
            release: 14,
            sig_entries: 4776,
            sig_entries_fnv: -7091892342548704452,
            classes: 4715,
            classes_fnv: 8749399968903186345,
            module_infos: 61,
            modules: 61,
            modules_fnv: -8046479167228493243,
        },
        ExpectedRelease {
            release: 15,
            sig_entries: 4708,
            sig_entries_fnv: 4184875412362374053,
            classes: 4648,
            classes_fnv: 3245884733840092473,
            module_infos: 60,
            modules: 60,
            modules_fnv: -2260888531994082490,
        },
        ExpectedRelease {
            release: 16,
            sig_entries: 4747,
            sig_entries_fnv: 1160926093218335289,
            classes: 4686,
            classes_fnv: -544705826610566846,
            module_infos: 61,
            modules: 61,
            modules_fnv: 3045690628843231014,
        },
        ExpectedRelease {
            release: 17,
            sig_entries: 0,
            sig_entries_fnv: -3750763034362895579,
            classes: 0,
            classes_fnv: -3750763034362895579,
            module_infos: 0,
            modules: 0,
            modules_fnv: -3750763034362895579,
        },
    ];

    /// (release, class name, expected archive entry, byte length, FNV-1a 64 of the bytes);
    /// every row was read back from the baseline archive for that release.
    const EXPECTED_SAMPLES: &[(i32, &str, &str, usize, i64)] = &[
        (
            7,
            "java.lang.Object",
            "87/java.base/java/lang/Object.sig",
            657,
            -6819042849110505806,
        ),
        (
            8,
            "java.lang.Object",
            "87/java.base/java/lang/Object.sig",
            657,
            -6819042849110505806,
        ),
        (
            9,
            "java.lang.Object",
            "9A/java.base/java/lang/Object.sig",
            758,
            4084377364556998773,
        ),
        (
            10,
            "java.lang.Object",
            "9A/java.base/java/lang/Object.sig",
            758,
            4084377364556998773,
        ),
        (
            11,
            "java.lang.Object",
            "BCDEF/java.base/java/lang/Object.sig",
            671,
            5493193315078468195,
        ),
        (
            16,
            "java.lang.Object",
            "G/java.base/java/lang/Object.sig",
            671,
            7478274168258873765,
        ),
        (
            7,
            "java.util.List",
            "7/java.base/java/util/List.sig",
            1481,
            -6381576200970082607,
        ),
        (
            8,
            "java.util.List",
            "8/java.base/java/util/List.sig",
            1771,
            -3125682332734182537,
        ),
        (
            9,
            "java.util.List",
            "9/java.base/java/util/List.sig",
            4040,
            -3126946683705282822,
        ),
        (
            10,
            "java.util.List",
            "A/java.base/java/util/List.sig",
            4184,
            7171129518035454692,
        ),
        (
            16,
            "java.util.List",
            "BCDEFG/java.base/java/util/List.sig",
            4097,
            -2936110035978295474,
        ),
        (
            7,
            "java.lang.String",
            "7/java.base/java/lang/String.sig",
            3266,
            -5061971556790383590,
        ),
        (
            8,
            "java.lang.String",
            "8/java.base/java/lang/String.sig",
            3530,
            -6976675878030243290,
        ),
        (
            9,
            "java.lang.String",
            "9A/java.base/java/lang/String.sig",
            3641,
            5786329660325336703,
        ),
        (
            10,
            "java.lang.String",
            "9A/java.base/java/lang/String.sig",
            3641,
            5786329660325336703,
        ),
        (
            11,
            "java.lang.String",
            "B/java.base/java/lang/String.sig",
            3843,
            3935391556395909812,
        ),
        (
            12,
            "java.lang.String",
            "C/java.base/java/lang/String.sig",
            4580,
            5168126211325120982,
        ),
        (
            13,
            "java.lang.String",
            "D/java.base/java/lang/String.sig",
            4808,
            3777494150983502849,
        ),
        (
            14,
            "java.lang.String",
            "E/java.base/java/lang/String.sig",
            4928,
            -7876324863518074878,
        ),
        (
            15,
            "java.lang.String",
            "F/java.base/java/lang/String.sig",
            4691,
            -6731759512271481627,
        ),
        (
            16,
            "java.lang.String",
            "G/java.base/java/lang/String.sig",
            4691,
            -1102717903700749231,
        ),
        (
            9,
            "java.lang.Module",
            "9/java.base/java/lang/Module.sig",
            1687,
            -3864602836507431031,
        ),
        (
            10,
            "java.lang.Module",
            "ABCDEFG/java.base/java/lang/Module.sig",
            1687,
            -999777767697089591,
        ),
        (
            16,
            "java.lang.Module",
            "ABCDEFG/java.base/java/lang/Module.sig",
            1687,
            -999777767697089591,
        ),
        (
            9,
            "java.lang.ModuleLayer",
            "9A/java.base/java/lang/ModuleLayer.sig",
            1811,
            793528896039981018,
        ),
        (
            11,
            "java.lang.ModuleLayer",
            "BCDEFG/java.base/java/lang/ModuleLayer.sig",
            1835,
            -9079148850477975815,
        ),
        (
            14,
            "java.lang.Record",
            "EF/java.base/java/lang/Record.sig",
            374,
            -2755385534913670456,
        ),
        (
            15,
            "java.lang.Record",
            "EF/java.base/java/lang/Record.sig",
            374,
            -2755385534913670456,
        ),
        (
            16,
            "java.lang.Record",
            "G/java.base/java/lang/Record.sig",
            199,
            9181731050334200335,
        ),
        (
            7,
            "java.io.Serializable",
            "879A/java.base/java/io/Serializable.sig",
            159,
            -2174557176097563682,
        ),
        (
            9,
            "java.io.Serializable",
            "879A/java.base/java/io/Serializable.sig",
            159,
            -2174557176097563682,
        ),
        (
            11,
            "java.io.Serializable",
            "BCDEFG/java.base/java/io/Serializable.sig",
            72,
            3744534623470569336,
        ),
        (
            12,
            "java.lang.constant.Constable",
            "CDEFG/java.base/java/lang/constant/Constable.sig",
            214,
            1494539965668344630,
        ),
        (
            16,
            "java.lang.constant.Constable",
            "CDEFG/java.base/java/lang/constant/Constable.sig",
            214,
            1494539965668344630,
        ),
        (
            7,
            "java.lang.annotation.Retention",
            "879A/java.base/java/lang/annotation/Retention.sig",
            533,
            -6607189606811132024,
        ),
        (
            11,
            "java.lang.annotation.Retention",
            "BCDEFG/java.base/java/lang/annotation/Retention.sig",
            454,
            5275196054431271716,
        ),
    ];

    /// The module list of the running JDK image recorded from "<17>/system-modules".
    const EXPECTED_SYSTEM_MODULES: &[&str] = &[
        "java.base",
        "java.compiler",
        "java.datatransfer",
        "java.desktop",
        "java.instrument",
        "java.logging",
        "java.management",
        "java.management.rmi",
        "java.naming",
        "java.net.http",
        "java.prefs",
        "java.rmi",
        "java.scripting",
        "java.se",
        "java.security.jgss",
        "java.security.sasl",
        "java.smartcardio",
        "java.sql",
        "java.sql.rowset",
        "java.transaction.xa",
        "java.xml",
        "java.xml.crypto",
        "jdk.accessibility",
        "jdk.attach",
        "jdk.charsets",
        "jdk.compiler",
        "jdk.crypto.cryptoki",
        "jdk.crypto.ec",
        "jdk.dynalink",
        "jdk.editpad",
        "jdk.hotspot.agent",
        "jdk.httpserver",
        "jdk.incubator.foreign",
        "jdk.incubator.vector",
        "jdk.jartool",
        "jdk.javadoc",
        "jdk.jcmd",
        "jdk.jconsole",
        "jdk.jdeps",
        "jdk.jdi",
        "jdk.jdwp.agent",
        "jdk.jfr",
        "jdk.jlink",
        "jdk.jpackage",
        "jdk.jshell",
        "jdk.jsobject",
        "jdk.jstatd",
        "jdk.localedata",
        "jdk.management",
        "jdk.management.agent",
        "jdk.management.jfr",
        "jdk.naming.dns",
        "jdk.naming.rmi",
        "jdk.net",
        "jdk.nio.mapmode",
        "jdk.sctp",
        "jdk.security.auth",
        "jdk.security.jgss",
        "jdk.unsupported",
        "jdk.xml.dom",
        "jdk.zipfs",
    ];

    /// The ct.sym of the baseline JDK, if this machine has one.
    fn baseline_ct_sym() -> Option<PathBuf> {
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            let candidate = PathBuf::from(home).join("lib").join(CT_SYM_FILE_NAME);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    /// Opens the baseline ct.sym, or None when this machine has no matching baseline.
    fn open_baseline() -> Option<(PathBuf, CtSym)> {
        let path = baseline_ct_sym()?;
        let ct_sym = CtSym::open(&path)
            .unwrap_or_else(|error| panic!("cannot open {}: {}", path.display(), error));
        if ct_sym.entry_count() != EXPECTED_ENTRY_COUNT {
            eprintln!(
                "skipping baseline comparison: {} is not the recorded JDK 17.0.19 ct.sym",
                path.display()
            );
            return None;
        }
        Some((path, ct_sym))
    }

    fn fnv1a64_bytes(bytes: &[u8]) -> i64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash as i64
    }

    fn fnv1a64_names<I, S>(names: I) -> i64
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for name in names {
            for byte in name.as_ref().as_bytes() {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
            hash ^= u64::from(b'\n');
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash as i64
    }

    #[test]
    fn opens_baseline_ct_sym() {
        let Some((path, ct_sym)) = open_baseline() else {
            return;
        };
        assert_eq!(ct_sym.path(), path.as_path());
        assert_eq!(ct_sym.entry_count(), EXPECTED_ENTRY_COUNT);
        assert_eq!(ct_sym.releases(), &EXPECTED_RELEASES[..]);
        assert_eq!(
            ct_sym.all_sig_entries().len(),
            EXPECTED_SIG_COUNT,
            "every file entry except system-modules is a .sig"
        );
        let section_sigs: usize = ct_sym.sections().iter().map(Section::sig_count).sum();
        assert_eq!(section_sigs, EXPECTED_SIG_COUNT);
        assert!(
            ct_sym
                .sections()
                .iter()
                .all(|section| !section.name().contains('-')),
            "the baseline has no ignored (-) release directory"
        );
        // The newest release of the baseline has no signature class files at all.
        assert!(ct_sym.sig_entries(17).is_empty());
        assert!(ct_sym.classes(17).is_empty());
        assert!(ct_sym.get_sig(17, "java.lang.Object").is_none());
        assert!(ct_sym.has_system_modules(17));
    }

    #[test]
    fn matches_recorded_release_directories() {
        let Some((_, ct_sym)) = open_baseline() else {
            return;
        };
        assert_eq!(ct_sym.sections().len(), EXPECTED_SECTIONS.len());
        for (section, &(name, sig_count, module_fnv)) in
            ct_sym.sections().iter().zip(EXPECTED_SECTIONS)
        {
            assert_eq!(section.name(), name);
            assert_eq!(section.sig_count(), sig_count, "section {}", name);
            assert_eq!(
                fnv1a64_names(section.module_dirs().iter().map(String::as_str)),
                module_fnv,
                "module directories of section {}",
                name
            );
        }

        // Release decoding: one character per release, directories with "-" are ignored.
        let section_879a = ct_sym.section("879A").expect("section 879A");
        assert_eq!(section_879a.releases(), &[7, 8, 9, 10][..]);
        let section_7 = ct_sym.section("7").expect("section 7");
        assert_eq!(section_7.releases(), &[7][..]);
        let image = ct_sym.section("H").expect("section H");
        assert_eq!(image.releases(), &[17][..]);
        assert!(image.has_system_modules());
        assert_eq!(image.sig_count(), 0);
        assert!(image.module_dirs().is_empty());
        assert!(ct_sym.section("nope").is_none());
    }

    #[test]
    fn matches_recorded_release_tables() {
        let Some((_, ct_sym)) = open_baseline() else {
            return;
        };
        for expected in EXPECTED_RELEASES_TABLE {
            let entries = ct_sym.sig_entries(expected.release);
            assert_eq!(
                entries.len(),
                expected.sig_entries,
                "release {} signature entries",
                expected.release
            );
            assert_eq!(
                fnv1a64_names(entries.iter().map(|entry| entry.entry_name())),
                expected.sig_entries_fnv,
                "release {} signature entry names",
                expected.release
            );
            let module_infos = entries
                .iter()
                .filter(|entry| entry.class_name() == "module-info")
                .count();
            assert_eq!(
                module_infos, expected.module_infos,
                "release {} module-info entries",
                expected.release
            );

            let classes = ct_sym.classes(expected.release);
            assert_eq!(
                classes.len(),
                expected.classes,
                "release {} classes",
                expected.release
            );
            assert_eq!(
                fnv1a64_names(&classes),
                expected.classes_fnv,
                "release {} class names",
                expected.release
            );

            let modules = ct_sym.modules(expected.release);
            assert_eq!(
                modules.len(),
                expected.modules,
                "release {} modules",
                expected.release
            );
            assert_eq!(
                fnv1a64_names(&modules),
                expected.modules_fnv,
                "release {} module names",
                expected.release
            );
        }
    }

    #[test]
    fn matches_recorded_baseline_samples() {
        let Some((_, ct_sym)) = open_baseline() else {
            return;
        };
        for &(release, class, entry_name, length, fnv) in EXPECTED_SAMPLES {
            let entry = ct_sym
                .find_sig(release, class)
                .unwrap_or_else(|| panic!("no signature for {} in release {}", class, release));
            assert_eq!(entry.entry_name(), entry_name);
            assert_eq!(entry.section(), entry_name.split('/').next().unwrap());
            let bytes = ct_sym
                .get_sig(release, class)
                .unwrap_or_else(|| panic!("cannot read {} for release {}", class, release));
            assert_eq!(&bytes[..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
            assert_eq!(
                bytes.len(),
                length,
                "length of {} for release {}",
                class,
                release
            );
            assert_eq!(
                fnv1a64_bytes(&bytes),
                fnv,
                "bytes of {} for release {}",
                class,
                release
            );
            // Internal (slash) names and module-scoped lookup address the same entry.
            assert_eq!(
                ct_sym.get_sig(release, &class.replace('.', "/")),
                Some(bytes.clone())
            );
            assert_eq!(
                ct_sym.get_sig_in_module(release, entry.module(), class),
                Some(bytes)
            );
        }

        // Classes that do not exist in a release are absent, not taken from a newer directory:
        // each (class, release) pair belongs to exactly one section.
        assert_eq!(ct_sym.get_sig(13, "java.lang.Record"), None);
        assert_eq!(ct_sym.get_sig(7, "java.lang.Record"), None);
        assert_eq!(ct_sym.get_sig(8, "java.lang.Module"), None);
        assert_eq!(ct_sym.get_sig(11, "java.lang.constant.Constable"), None);
        assert_eq!(ct_sym.find_class_any_release("java.lang.Record"), Some(16));
        assert_eq!(
            ct_sym.releases_containing("java.lang.Record"),
            vec![14, 15, 16]
        );
        assert_eq!(
            ct_sym.releases_containing("java.lang.Module"),
            vec![9, 10, 11, 12, 13, 14, 15, 16]
        );
        assert_eq!(ct_sym.find_class_any_release("no.such.Class"), None);
        assert_eq!(ct_sym.classes(6), Vec::<String>::new());
        assert_eq!(ct_sym.get_sig(18, "java.lang.Object"), None);

        // Module descriptors are not classes; they are exposed through sig_entries instead.
        let release_9 = ct_sym.sig_entries(9);
        assert!(
            release_9
                .iter()
                .any(|entry| entry.class_name() == "module-info" && entry.module() == "java.base")
        );
        assert!(!ct_sym.classes(9).iter().any(|class| class == "module-info"));
    }

    #[test]
    fn system_modules_describe_the_running_image() {
        let Some((_, ct_sym)) = open_baseline() else {
            return;
        };
        for release in 7..=16 {
            assert!(
                !ct_sym.has_system_modules(release),
                "release {} must not be the running image",
                release
            );
            assert_eq!(ct_sym.system_modules(release), None);
        }
        assert!(ct_sym.has_system_modules(17));
        let modules = ct_sym
            .system_modules(17)
            .expect("system-modules of release 17");
        let expected: Vec<String> = EXPECTED_SYSTEM_MODULES
            .iter()
            .map(|module| module.to_string())
            .collect();
        assert_eq!(modules, expected);
        assert_eq!(fnv1a64_names(&modules), -1513895327547763668);
        assert_eq!(modules.len(), 61);
        assert_eq!(modules.first().map(String::as_str), Some("java.base"));
        assert_eq!(modules.last().map(String::as_str), Some("jdk.zipfs"));
    }

    #[test]
    fn version_characters_and_directory_decoding() {
        assert_eq!(release_version_char(7), Some('7'));
        assert_eq!(release_version_char(9), Some('9'));
        assert_eq!(release_version_char(10), Some('A'));
        assert_eq!(release_version_char(17), Some('H'));
        assert_eq!(release_version_char(35), Some('Z'));
        assert_eq!(release_version_char(36), None);
        assert_eq!(release_version_char(-1), None);

        assert_eq!(decode_section_releases("7"), vec![7]);
        assert_eq!(decode_section_releases("879A"), vec![7, 8, 9, 10]);
        assert_eq!(decode_section_releases("GH"), vec![16, 17]);
        assert_eq!(decode_section_releases("8-"), Vec::<i32>::new());
        // Target.lookup has no "1".."4" (those targets are named "1.1".."1.4") and no "18".
        assert_eq!(decode_section_releases("1"), Vec::<i32>::new());
        assert_eq!(decode_section_releases("I"), Vec::<i32>::new());
        assert_eq!(base36_value('0'), Some(0));
        assert_eq!(base36_value('a'), Some(10));
        assert_eq!(base36_value('Z'), Some(35));
        assert_eq!(base36_value('!'), None);
    }

    #[test]
    fn reads_lines_like_buffered_reader() {
        assert_eq!(
            read_lines(b"java.base\njava.compiler\n").unwrap(),
            vec!["java.base", "java.compiler"]
        );
        assert_eq!(read_lines(b"a\r\nb\r").unwrap(), vec!["a", "b"]);
        assert_eq!(read_lines(b"a\rb").unwrap(), vec!["a", "b"]);
        assert_eq!(read_lines(b"a\n\n").unwrap(), vec!["a", ""]);
        assert_eq!(read_lines(b"\n").unwrap(), vec![""]);
        assert_eq!(read_lines(b"").unwrap(), Vec::<String>::new());
        assert!(read_lines(b"\xFF").is_err());
    }

    struct TestEntry<'a> {
        name: &'a str,
        data: &'a [u8],
    }

    fn push_u16(out: &mut Vec<u8>, value: u16) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32(out: &mut Vec<u8>, value: u32) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    /// Builds a minimal stored-only zip archive around the given entries.
    fn build_zip(entries: &[TestEntry<'_>]) -> Vec<u8> {
        const LOCAL_HEADER_SIGNATURE: u32 = 0x0403_4b50;
        const CENTRAL_HEADER_SIGNATURE: u32 = 0x0201_4b50;
        const EOCD_SIGNATURE: u32 = 0x0605_4b50;
        let mut out = Vec::new();
        let mut central = Vec::new();
        for entry in entries {
            let name_len = entry.name.len() as u16;
            let local_offset = out.len() as u32;

            push_u32(&mut out, LOCAL_HEADER_SIGNATURE);
            push_u16(&mut out, 20);
            push_u16(&mut out, 0);
            push_u16(&mut out, 0);
            push_u16(&mut out, 0);
            push_u16(&mut out, 0);
            push_u32(&mut out, 0);
            push_u32(&mut out, entry.data.len() as u32);
            push_u32(&mut out, entry.data.len() as u32);
            push_u16(&mut out, name_len);
            push_u16(&mut out, 0);
            out.extend_from_slice(entry.name.as_bytes());
            out.extend_from_slice(entry.data);

            push_u32(&mut central, CENTRAL_HEADER_SIGNATURE);
            push_u16(&mut central, 20);
            push_u16(&mut central, 20);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u32(&mut central, 0);
            push_u32(&mut central, entry.data.len() as u32);
            push_u32(&mut central, entry.data.len() as u32);
            push_u16(&mut central, name_len);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u32(&mut central, 0);
            push_u32(&mut central, local_offset);
            central.extend_from_slice(entry.name.as_bytes());
        }
        let cd_offset = out.len() as u32;
        let cd_size = central.len() as u32;
        out.extend_from_slice(&central);

        push_u32(&mut out, EOCD_SIGNATURE);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);
        push_u16(&mut out, entries.len() as u16);
        push_u16(&mut out, entries.len() as u16);
        push_u32(&mut out, cd_size);
        push_u32(&mut out, cd_offset);
        push_u16(&mut out, 0);
        out
    }

    fn temp_dir() -> PathBuf {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("target")
            .join("test-tmp");
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn write_temp(tag: &str, bytes: &[u8]) -> PathBuf {
        let path = temp_dir().join(format!(
            "rust-javac-ctsym-test-{}-{}.sym",
            std::process::id(),
            tag
        ));
        std::fs::write(&path, bytes).expect("write temp ct.sym");
        path
    }

    #[test]
    fn reads_selection_from_a_synthetic_archive() {
        let entries = [
            TestEntry {
                name: "7/base/java/lang/Object.sig",
                data: b"\xCA\xFE\xBA\xBE-seven",
            },
            TestEntry {
                name: "8/base/java/lang/Object.sig",
                data: b"\xCA\xFE\xBA\xBE-eight",
            },
            TestEntry {
                name: "79A/base/java/lang/String.sig",
                data: b"\xCA\xFE\xBA\xBE-ninety-a",
            },
            TestEntry {
                name: "H/system-modules",
                data: b"java.base\n",
            },
        ];
        let path = write_temp("synthetic", &build_zip(&entries));
        let ct_sym = CtSym::open(&path).expect("open synthetic ct.sym");

        assert_eq!(ct_sym.entry_count(), 4);
        assert_eq!(ct_sym.releases(), &[7, 8, 9, 10, 17][..]);
        assert_eq!(ct_sym.sections().len(), 4);

        assert_eq!(
            ct_sym.get_sig(7, "java.lang.Object").as_deref(),
            Some(&b"\xCA\xFE\xBA\xBE-seven"[..])
        );
        assert_eq!(
            ct_sym.get_sig(8, "java/lang/Object").as_deref(),
            Some(&b"\xCA\xFE\xBA\xBE-eight"[..])
        );
        assert_eq!(
            ct_sym.get_sig(9, "java.lang.String").as_deref(),
            Some(&b"\xCA\xFE\xBA\xBE-ninety-a"[..])
        );
        assert_eq!(
            ct_sym.get_sig(10, "java.lang.String").as_deref(),
            Some(&b"\xCA\xFE\xBA\xBE-ninety-a"[..])
        );
        assert_eq!(ct_sym.get_sig(9, "java.lang.Object"), None);
        assert_eq!(ct_sym.get_sig(17, "java.lang.String"), None);
        assert_eq!(ct_sym.get_sig(11, "java.lang.String"), None);
        assert_eq!(ct_sym.classes(9), vec!["java.lang.String".to_string()]);
        assert_eq!(ct_sym.classes(8), vec!["java.lang.Object".to_string()]);
        assert_eq!(ct_sym.classes(11), Vec::<String>::new());
        assert_eq!(ct_sym.modules(9), vec!["base"]);
        assert_eq!(
            ct_sym.releases_containing("java.lang.String"),
            vec![7, 9, 10]
        );
        assert_eq!(ct_sym.find_class_any_release("java.lang.String"), Some(10));
        assert_eq!(ct_sym.find_class_any_release("java.lang.Object"), Some(8));

        let entry = ct_sym
            .find_sig(9, "java.lang.String")
            .expect("String entry");
        assert_eq!(entry.section(), "79A");
        assert_eq!(entry.module(), "base");
        assert_eq!(entry.class_name(), "java.lang.String");
        assert_eq!(entry.entry_name(), "79A/base/java/lang/String.sig");
        assert_eq!(
            ct_sym
                .get_sig_in_module(9, "base", "java.lang.String")
                .as_deref(),
            Some(&b"\xCA\xFE\xBA\xBE-ninety-a"[..])
        );
        assert_eq!(
            ct_sym.get_sig_in_module(9, "other", "java.lang.String"),
            None
        );

        assert!(ct_sym.has_system_modules(17));
        assert_eq!(
            ct_sym.system_modules(17),
            Some(vec!["java.base".to_string()])
        );
        assert!(!ct_sym.has_system_modules(7));
        assert_eq!(ct_sym.system_modules(7), None);
        assert_eq!(ct_sym.system_modules(10), None);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn skips_release_directories_that_contain_a_dash() {
        let entries = [
            TestEntry {
                name: "8-/base/java/lang/Object.sig",
                data: b"\xCA\xFE\xBA\xBE",
            },
            TestEntry {
                name: "9/base/java/lang/Object.sig",
                data: b"\xCA\xFE\xBA\xBE",
            },
        ];
        let path = write_temp("dash", &build_zip(&entries));
        let ct_sym = CtSym::open(&path).expect("open synthetic ct.sym");
        assert_eq!(ct_sym.releases(), &[9][..]);
        let ignored = ct_sym.section("8-").expect("ignored section");
        assert!(ignored.releases().is_empty());
        assert_eq!(ignored.sig_count(), 1);
        assert_eq!(ct_sym.get_sig(8, "java.lang.Object"), None);
        assert_eq!(
            ct_sym.get_sig(9, "java.lang.Object").as_deref(),
            Some(&b"\xCA\xFE\xBA\xBE"[..])
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    #[should_panic(expected = "not a base-36 name")]
    fn rejects_release_directories_with_invalid_characters() {
        let entries = [TestEntry {
            name: "8!/base/java/lang/Object.sig",
            data: b"\xCA\xFE\xBA\xBE",
        }];
        let path = write_temp("invalid-section", &build_zip(&entries));
        let _ = CtSym::open(&path);
    }

    #[test]
    fn rejects_missing_and_malformed_archives() {
        let missing = temp_dir().join("rust-javac-ctsym-no-such-file.sym");
        assert!(matches!(CtSym::open(&missing).unwrap_err(), Error::Io(_)));

        let short = write_temp("short", b"PK");
        assert!(matches!(
            CtSym::open(&short).unwrap_err(),
            Error::NotAZip(_)
        ));
        std::fs::remove_file(&short).ok();

        let garbage = write_temp("garbage", &[0u8; 128]);
        assert!(matches!(
            CtSym::open(&garbage).unwrap_err(),
            Error::NotAZip(_)
        ));
        std::fs::remove_file(&garbage).ok();

        // An empty archive opens and has no release at all, like a JDK platform provider whose
        // ct.sym holds no release directory.
        let empty = write_temp("empty", &build_zip(&[]));
        let ct_sym = CtSym::open(&empty).expect("open empty archive");
        assert!(ct_sym.releases().is_empty());
        assert!(ct_sym.sections().is_empty());
        assert_eq!(ct_sym.classes(7), Vec::<String>::new());
        assert_eq!(ct_sym.get_sig(7, "java.lang.Object"), None);
        assert_eq!(ct_sym.find_class_any_release("java.lang.Object"), None);
        std::fs::remove_file(&empty).ok();
    }
}
