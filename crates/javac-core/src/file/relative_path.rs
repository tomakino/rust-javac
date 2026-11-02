//! Used to represent a platform-neutral path within a platform-specific
//! container, such as a directory or zip file.
//! Internally, the file separator is always '/'.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: the Java class hierarchy (abstract RelativePath with the
//! nested RelativeDirectory/RelativeFile subclasses) becomes a base struct plus
//! two wrapper structs holding the base.  The base keeps the dynamic kind
//! (RelativePathKind) so the abstract dirname()/basename() dispatch stays 1:1;
//! composition replaces inheritance and the base state/methods are 1:1.
//! Not ported dependencies:
//! * java.nio.file.FileSystem (resolveAgainst(FileSystem)) is REGISTERED: the
//!   port has no FileSystem object; the resolveAgainst(Path) overload is ported
//!   on javac-fs and the archive adapter is used for zip entries;
//! * java.util.zip.ZipFile/ZipEntry (RelativeFile.getZipEntry) is REGISTERED:
//!   javac-zip works on ZipArchive/entry names instead of ZipFile.
//! * equals/compareTo/hashCode/toString map onto PartialEq/Ord/Hash/Display.

use javac_fs::Path as FsPath;
use javac_strings::JavaString;
use javac_tools::tools::java_file_object::Kind;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

/// The dynamic kind of a RelativePath (Java: the concrete subclass).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RelativePathKind {
    Directory,
    File,
}

/// Java: public abstract class RelativePath implements Comparable<RelativePath>.
#[derive(Clone, Debug)]
pub struct RelativePath {
    /// Java: protected final String path; (always '/'-separated)
    pub(crate) path: JavaString,
    /// The dynamic subclass (Java: this.getClass()).
    pub(crate) kind: RelativePathKind,
}

impl RelativePath {
    /// Java: protected RelativePath(String p) — p must use '/' as internal separator.
    pub(crate) fn new_internal(p: &JavaString, kind: RelativePathKind) -> RelativePath {
        RelativePath {
            path: p.clone(),
            kind,
        }
    }

    /// Java: public String getPath().
    pub fn get_path(&self) -> &JavaString {
        &self.path
    }

    /// Java: public abstract RelativeDirectory dirname() — virtual dispatch.
    pub fn dirname(&self) -> RelativeDirectory {
        match self.kind {
            RelativePathKind::Directory => directory_dirname(&self.path),
            RelativePathKind::File => file_dirname(&self.path),
        }
    }

    /// Java: public abstract String basename() — virtual dispatch.
    pub fn basename(&self) -> JavaString {
        match self.kind {
            RelativePathKind::Directory => directory_basename(&self.path),
            RelativePathKind::File => file_basename(&self.path),
        }
    }

    /// Java: public Path resolveAgainst(Path directory) throws InvalidPathException.
    ///
    /// PORTING_RULES: java.nio.file.Path is the javac-fs Path; the separator is
    /// the platform separator (Java: directory.getFileSystem().getSeparator()).
    pub fn resolve_against(&self, directory: &FsPath) -> Result<FsPath, javac_fs::Error> {
        let separator = std::path::MAIN_SEPARATOR;
        let relative = self.path.replace_char('/' as u16, separator as u16);
        let relative = FsPath::new(&relative.to_string_lossy())?;
        Ok(directory.resolve(&relative))
    }

    /// Java: public Path resolveAgainst(FileSystem fs) — REGISTERED: the port has
    /// no java.nio.file.FileSystem object; callers use resolveAgainst(Path).
    /// Java: public Path resolveAgainst(FileSystem fs) throws InvalidPathException.
    ///
    /// PORTING_RULES: java.nio.file.FileSystem is REGISTERED as unavailable; the caller supplies
    /// the root directory and separator (Java: fs.getRootDirectories().iterator().next()
    /// and fs.getSeparator()).  The resolution algorithm is the upstream one.
    pub fn resolve_against_fs(
        &self,
        fs_root: &FsPath,
        separator: char,
    ) -> Result<FsPath, javac_fs::Error> {
        let relative = self.path.replace_char('/' as u16, separator as u16);
        let relative = FsPath::new(&relative.to_string_lossy())?;
        Ok(fs_root.resolve(&relative))
    }
}

impl fmt::Display for RelativePath {
    /// Java: public String toString() { return "RelPath[" + path + "]"; }
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RelPath[{}]", self.path)
    }
}

impl PartialEq for RelativePath {
    /// Java: equals -> path.equals(other.path).
    fn eq(&self, other: &RelativePath) -> bool {
        self.path == other.path
    }
}

impl Eq for RelativePath {}

impl PartialOrd for RelativePath {
    fn partial_cmp(&self, other: &RelativePath) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RelativePath {
    /// Java: compareTo -> path.compareTo(other.path).
    fn cmp(&self, other: &RelativePath) -> Ordering {
        self.path.as_units().cmp(other.path.as_units())
    }
}

impl Hash for RelativePath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.as_units().hash(state);
    }
}

/// Java: public static class RelativeDirectory extends RelativePath.
#[derive(Clone, Debug)]
pub struct RelativeDirectory {
    base: RelativePath,
}

impl RelativeDirectory {
    /// Java: static RelativeDirectory forPackage(CharSequence packageName).
    pub fn for_package(package_name: &JavaString) -> RelativeDirectory {
        RelativeDirectory::new(&package_name.replace_char('.' as u16, '/' as u16))
    }

    /// Java: public RelativeDirectory(String p).
    pub fn new(p: &JavaString) -> RelativeDirectory {
        let path = if p.is_empty() || p.ends_with(&JavaString::from("/")) {
            p.clone()
        } else {
            p.concat(&JavaString::from("/"))
        };
        RelativeDirectory {
            base: RelativePath::new_internal(&path, RelativePathKind::Directory),
        }
    }

    /// Java: public RelativeDirectory(RelativeDirectory d, String p).
    pub fn with_parent(d: &RelativeDirectory, p: &JavaString) -> RelativeDirectory {
        RelativeDirectory::new(&d.base.path.concat(p))
    }

    /// Java: public RelativeDirectory dirname().
    pub fn dirname(&self) -> RelativeDirectory {
        directory_dirname(&self.base.path)
    }

    /// Java: public String basename().
    pub fn basename(&self) -> JavaString {
        directory_basename(&self.base.path)
    }

    /// Java: boolean contains(RelativePath other) — a subdirectory path does not
    /// contain itself.
    pub fn contains(&self, other: &RelativePath) -> bool {
        other.path.len() > self.base.path.len() && other.path.starts_with(&self.base.path)
    }

    /// The base RelativePath view (Java's inherited identity).
    pub fn as_relative_path(&self) -> &RelativePath {
        &self.base
    }
}

impl fmt::Display for RelativeDirectory {
    /// Java: public String toString() { return "RelativeDirectory[" + path + "]"; }
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RelativeDirectory[{}]", self.base.path)
    }
}

impl PartialEq for RelativeDirectory {
    fn eq(&self, other: &RelativeDirectory) -> bool {
        self.base == other.base
    }
}

impl Eq for RelativeDirectory {}

impl Hash for RelativeDirectory {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.base.hash(state);
    }
}

/// Java: public static class RelativeFile extends RelativePath.
#[derive(Clone, Debug)]
pub struct RelativeFile {
    base: RelativePath,
}

impl RelativeFile {
    /// Java: static RelativeFile forClass(CharSequence className, Kind kind).
    pub fn for_class(class_name: &JavaString, kind: Kind) -> RelativeFile {
        let path = class_name
            .replace_char('.' as u16, '/' as u16)
            .concat(&kind.extension());
        RelativeFile::new(&path)
    }

    /// Java: public RelativeFile(String p).
    pub fn new(p: &JavaString) -> RelativeFile {
        if p.ends_with(&JavaString::from("/")) {
            panic!("java.lang.IllegalArgumentException: {}", p);
        }
        RelativeFile {
            base: RelativePath::new_internal(p, RelativePathKind::File),
        }
    }

    /// Java: public RelativeFile(RelativeDirectory d, String p).
    pub fn with_directory(d: &RelativeDirectory, p: &JavaString) -> RelativeFile {
        RelativeFile::new(&d.base.path.concat(p))
    }

    /// Java: RelativeFile(RelativeDirectory d, RelativePath p).
    pub fn with_directory_path(d: &RelativeDirectory, p: &RelativePath) -> RelativeFile {
        RelativeFile::with_directory(d, &p.path)
    }

    /// Java: public RelativeDirectory dirname().
    pub fn dirname(&self) -> RelativeDirectory {
        file_dirname(&self.base.path)
    }

    /// Java: public String basename().
    pub fn basename(&self) -> JavaString {
        file_basename(&self.base.path)
    }

    /// Java: ZipEntry getZipEntry(ZipFile zip) — REGISTERED: javac-zip works on
    /// ZipArchive/entry names, not java.util.zip.ZipFile/ZipEntry.
    /// Java: public ZipEntry getZipEntry(ZipFile zip) { return zip.getEntry(path); }
    ///
    /// PORTING_RULES: java.util.zip.ZipEntry is REGISTERED as unavailable; the archive entry lookup
    /// returns the entry contents through javac-zip (registered adapter: the
    /// upstream ZipEntry object is replaced by its data).
    pub fn get_zip_entry(&self, archive: &javac_zip::ZipArchive) -> std::option::Option<Vec<u8>> {
        archive.get_entry(&self.base.path.to_string_lossy())
    }

    /// The base RelativePath view (Java's inherited identity).
    pub fn as_relative_path(&self) -> &RelativePath {
        &self.base
    }
}

impl fmt::Display for RelativeFile {
    /// Java: public String toString() { return "RelativeFile[" + path + "]"; }
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RelativeFile[{}]", self.base.path)
    }
}

impl PartialEq for RelativeFile {
    fn eq(&self, other: &RelativeFile) -> bool {
        self.base == other.base
    }
}

impl Eq for RelativeFile {}

impl Hash for RelativeFile {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.base.hash(state);
    }
}

/// Java: String.lastIndexOf(ch, fromIndex) semantics (fromIndex < 0 -> -1).
fn last_index_of(path: &JavaString, ch: u16, from_index: i32) -> i32 {
    if from_index < 0 {
        return -1;
    }
    let units = path.as_units();
    let start = (from_index as usize).min(units.len().saturating_sub(1));
    for index in (0..=start).rev() {
        if units[index] == ch {
            return index as i32;
        }
    }
    -1
}

/// Java: RelativeDirectory.dirname() body.
fn directory_dirname(path: &JavaString) -> RelativeDirectory {
    let length = path.len() as i32;
    if length == 0 {
        return RelativeDirectory::new(path);
    }
    let sep = last_index_of(path, '/' as u16, length - 2);
    let end = if sep < 0 { 0 } else { (sep + 1) as usize };
    RelativeDirectory::new(&path.substring(0, end))
}

/// Java: RelativeDirectory.basename() body.
fn directory_basename(path: &JavaString) -> JavaString {
    let length = path.len() as i32;
    if length == 0 {
        return path.clone();
    }
    let sep = last_index_of(path, '/' as u16, length - 2);
    let start = (sep + 1) as usize;
    let end = (length - 1) as usize;
    if end < start {
        return JavaString::new();
    }
    path.substring(start, end)
}

/// Java: RelativeFile.dirname() body.
fn file_dirname(path: &JavaString) -> RelativeDirectory {
    let sep = last_index_of(path, '/' as u16, path.len() as i32 - 1);
    let end = if sep < 0 { 0 } else { (sep + 1) as usize };
    RelativeDirectory::new(&path.substring(0, end))
}

/// Java: RelativeFile.basename() body.
fn file_basename(path: &JavaString) -> JavaString {
    let sep = last_index_of(path, '/' as u16, path.len() as i32 - 1);
    let start = (sep + 1) as usize;
    path.substring(start, path.len())
}

#[cfg(test)]
mod resolve_against_fs_tests {
    use super::*;

    #[test]
    fn resolve_against_fs_uses_the_root_and_separator() {
        let directory = RelativeDirectory::new(&JavaString::from("pkg/sub/"));
        let root = FsPath::new("/root").expect("root path");
        let resolved = directory
            .as_relative_path()
            .resolve_against_fs(&root, '/')
            .expect("resolve");
        assert_eq!(resolved, FsPath::new("/root/pkg/sub/").expect("expected path"));
    }
}

