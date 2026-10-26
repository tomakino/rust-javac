// Origin: com/sun/tools/javac/file/PathFileObject.java @ 982d53c (jdk-17.0.19-ga)
// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P2 ClassReader / API.
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: hashCode (PathFileObject.java:539)
// End of Gap registration.

//! Implementation of JavaFileObject using java.nio.file API.
//!
//! <p>PathFileObjects are, for the most part, straightforward wrappers around
//! immutable absolute Path objects.  Different subtypes are used to provide
//! specialized implementations of "inferBinaryName" and "getName" that capture
//! additional information available at the time the object is created.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! * the Java abstract class plus its four private nested subclasses become one
//!   struct whose PathFileObjectKind selects the behavior (the factories keep
//!   the upstream signatures); the common mutable flag hasParents stays a Cell;
//! * java.nio.file.Path -> javac-fs Path; java.io streams -> std::io;
//!   declared IOException -> javac_tools::error::Error;
//! * deferred (TODO(P2), reported):
//!   - openReader/getCharContent/openWriter need java.nio.charset and the
//!     BaseFileManager decoder/content cache (not ported);
//!   - JarFileObject.inferBinaryName/toUri need a java.nio.file FileSystem for
//!     jar entries (zipfs, not ported);
//!   - JRTFileObject.toUri needs the jrt FileSystem (not ported);
//!   - createJarUri/getSimpleName need java.net.URI.normalize /
//!     URI.getSchemeSpecificPart (the javac-tools URI adapter is minimal);
//!   - the macOS Normalizer.isNormalized branch of isPathNameCompatible is not
//!     ported (java.text.Normalizer) and is skipped.

use crate::file::base_file_manager_api::BaseFileManagerApi;
use crate::file::relative_path::{RelativeFile, RelativePath};
use javac_fs::{Files as FsFiles, Path as FsPath};
use javac_strings::JavaString;
use javac_tools::error::Error;
use javac_tools::java_io::{Reader, Writer};
use javac_tools::lang::model::element::modifier::Modifier;
use javac_tools::lang::model::element::nesting_kind::NestingKind;
use javac_tools::tools::file_object::FileObject;
use javac_tools::tools::java_file_object::{JavaFileObject, Kind};
use javac_tools::uri::URI;
use std::cell::Cell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::rc::Rc;

/// The upstream nested subclasses (Java: DirectoryFileObject, JarFileObject,
/// JRTFileObject, SimpleFileObject).
pub enum PathFileObjectKind {
    /// Java: forDirectoryPath / DirectoryFileObject.
    Directory {
        user_package_root_dir: FsPath,
        relative_path: RelativePath,
    },
    /// Java: forJarPath / JarFileObject.
    Jar { user_jar_path: FsPath },
    /// Java: forJRTPath / JRTFileObject.
    Jrt,
    /// Java: forSimplePath / SimpleFileObject.
    Simple { user_path: FsPath },
}

/// Java: public abstract class PathFileObject implements JavaFileObject.
pub struct PathFileObject {
    file_manager: Rc<dyn BaseFileManagerApi>,
    path: FsPath,
    /// Java: private boolean hasParents;
    has_parents: Cell<bool>,
    kind: PathFileObjectKind,
}

impl PathFileObject {
    /// Shared constructor (Java: protected PathFileObject(BaseFileManager, Path)).
    fn new(
        file_manager: Rc<dyn BaseFileManagerApi>,
        path: FsPath,
        kind: PathFileObjectKind,
    ) -> PathFileObject {
        // Java: if (Files.isDirectory(path)) throw new IllegalArgumentException(
        // "directories not supported");
        if FsFiles::is_directory(&path) {
            panic!("java.lang.IllegalArgumentException: directories not supported");
        }
        PathFileObject {
            file_manager,
            path,
            has_parents: Cell::new(false),
            kind,
        }
    }

    /// Java: static PathFileObject forDirectoryPath(BaseFileManager, Path,
    /// Path userPackageRootDir, RelativePath relativePath).
    pub fn for_directory_path(
        file_manager: Rc<dyn BaseFileManagerApi>,
        path: &FsPath,
        user_package_root_dir: &FsPath,
        relative_path: RelativePath,
    ) -> Rc<PathFileObject> {
        Rc::new(PathFileObject::new(
            file_manager,
            path.clone(),
            PathFileObjectKind::Directory {
                user_package_root_dir: user_package_root_dir.clone(),
                relative_path,
            },
        ))
    }

    /// Java: public static PathFileObject forJarPath(BaseFileManager, Path,
    /// Path userJarPath).
    pub fn for_jar_path(
        file_manager: Rc<dyn BaseFileManagerApi>,
        path: &FsPath,
        user_jar_path: &FsPath,
    ) -> Rc<PathFileObject> {
        Rc::new(PathFileObject::new(
            file_manager,
            path.clone(),
            PathFileObjectKind::Jar {
                user_jar_path: user_jar_path.clone(),
            },
        ))
    }

    /// Java: public static PathFileObject forJRTPath(BaseFileManager, Path).
    pub fn for_jrt_path(
        file_manager: Rc<dyn BaseFileManagerApi>,
        path: &FsPath,
    ) -> Rc<PathFileObject> {
        Rc::new(PathFileObject::new(
            file_manager,
            path.clone(),
            PathFileObjectKind::Jrt,
        ))
    }

    /// Java: static PathFileObject forSimplePath(BaseFileManager, Path, Path userPath).
    pub fn for_simple_path(
        file_manager: Rc<dyn BaseFileManagerApi>,
        path: &FsPath,
        user_path: &FsPath,
    ) -> Rc<PathFileObject> {
        Rc::new(PathFileObject::new(
            file_manager,
            path.clone(),
            PathFileObjectKind::Simple {
                user_path: user_path.clone(),
            },
        ))
    }

    /// Java: public Path getPath().
    pub fn get_path(&self) -> &FsPath {
        &self.path
    }

    /// Java: public String getShortName().
    pub fn get_short_name(&self) -> JavaString {
        match self.path.file_name() {
            Some(name) => jstr(name.to_java_string()),
            None => jstr(self.path.to_java_string()),
        }
    }

    /// Java: public String getName() (overridden by the subclasses).
    pub fn get_name(&self) -> JavaString {
        match &self.kind {
            PathFileObjectKind::Directory {
                user_package_root_dir,
                relative_path,
            } => {
                // Java: relativePath.resolveAgainst(userPackageRootDir).toString()
                let resolved = relative_path
                    .resolve_against(user_package_root_dir)
                    .expect("InvalidPathException in DirectoryFileObject.getName");
                jstr(resolved.to_java_string())
            }
            PathFileObjectKind::Jar { user_jar_path } => jstr(format!(
                "{}({})",
                user_jar_path.to_java_string(),
                self.path.to_java_string()
            )),
            PathFileObjectKind::Jrt => jstr(self.path.to_java_string()),
            PathFileObjectKind::Simple { user_path } => jstr(user_path.to_java_string()),
        }
    }

    /// Java: abstract String inferBinaryName(Iterable<? extends Path> paths).
    pub fn infer_binary_name(&self, paths: &[FsPath]) -> Option<JavaString> {
        match &self.kind {
            PathFileObjectKind::Directory { relative_path, .. } => {
                Some(to_binary_name_relative(relative_path))
            }
            PathFileObjectKind::Jar { .. } => {
                unimplemented!(
                    "TODO(P2): JarFileObject.inferBinaryName needs the jar FileSystem root directories"
                )
            }
            PathFileObjectKind::Jrt => {
                // Java: use subpath to ignore the leading /modules/MODULE-NAME
                match self.path.subpath(2, self.path.name_count()) {
                    Ok(relative) => Some(to_binary_name_path(&relative)),
                    Err(error) => panic!("java.lang.IllegalArgumentException: {}", error),
                }
            }
            PathFileObjectKind::Simple { .. } => {
                let abs_path = self.path.to_absolute_path().ok()?;
                for path in paths {
                    let ap = match path.to_absolute_path() {
                        Ok(absolute) => absolute,
                        Err(_) => continue,
                    };
                    if abs_path.starts_with(&ap) {
                        // Java: try { Path rp = ap.relativize(absPath); if (rp != null)... }
                        // catch (IllegalArgumentException e) { /* ignore this p */ }
                        if let Ok(relative) = ap.relativize(&abs_path) {
                            return Some(to_binary_name_path(&relative));
                        }
                    }
                }
                None
            }
        }
    }

    /// Java: abstract PathFileObject getSibling(String basename).
    pub fn get_sibling(&self, base_name: &JavaString) -> Rc<PathFileObject> {
        let base_name_path = FsPath::new(&base_name.to_string_lossy())
            .expect("InvalidPathException in PathFileObject.getSibling");
        match &self.kind {
            PathFileObjectKind::Directory {
                user_package_root_dir,
                relative_path,
            } => PathFileObject::for_directory_path(
                self.file_manager.clone(),
                &self.path.resolve_sibling(&base_name_path),
                user_package_root_dir,
                RelativeFile::with_directory(&relative_path.dirname(), base_name)
                    .as_relative_path()
                    .clone(),
            ),
            PathFileObjectKind::Jar { user_jar_path } => PathFileObject::for_jar_path(
                self.file_manager.clone(),
                &self.path.resolve_sibling(&base_name_path),
                user_jar_path,
            ),
            PathFileObjectKind::Jrt => PathFileObject::for_jrt_path(
                self.file_manager.clone(),
                &self.path.resolve_sibling(&base_name_path),
            ),
            PathFileObjectKind::Simple { user_path } => PathFileObject::for_simple_path(
                self.file_manager.clone(),
                &self.path.resolve_sibling(&base_name_path),
                &user_path.resolve_sibling(&base_name_path),
            ),
        }
    }

    /// Java: public boolean isSameFile(PathFileObject other).
    pub fn is_same_file(&self, other: &PathFileObject) -> bool {
        self.path == other.path
    }

    /// Java: public Kind getKind() — the SimpleFileObject override uses userPath.
    /// PORTING_RULES: JRTIndex.isInJRT tests `path.getFileSystem() == jrtfs` upstream;
    /// the jrt: FileSystem does not exist in the port, so the JRTFileObject construction kind
    /// is the marker (adapter; registered).
    pub fn is_jrt(&self) -> bool {
        matches!(self.kind, PathFileObjectKind::Jrt)
    }

    pub fn get_kind_value(&self) -> Kind {
        match &self.kind {
            PathFileObjectKind::Simple { user_path } => get_kind_from_path(user_path),
            _ => get_kind_from_path(&self.path),
        }
    }

    /// Java: public boolean isNameCompatible(String simpleName, Kind kind).
    pub fn is_name_compatible_value(&self, simple_name: &JavaString, kind: Kind) -> bool {
        match &self.kind {
            PathFileObjectKind::Simple { user_path } => {
                self.is_path_name_compatible(user_path, simple_name, kind)
            }
            _ => self.is_path_name_compatible(&self.path, simple_name, kind),
        }
    }

    /// Java: protected boolean isPathNameCompatible(Path p, String simpleName, Kind kind).
    pub fn is_path_name_compatible(
        &self,
        path: &FsPath,
        simple_name: &JavaString,
        kind: Kind,
    ) -> bool {
        if kind == Kind::OTHER && get_kind_from_path(path) != Kind::OTHER {
            return false;
        }
        let sn = simple_name.concat(&kind.extension());
        let pn = match path.file_name() {
            Some(name) => name.to_java_string(),
            None => return false,
        };
        if pn == sn.to_string_lossy() {
            return true;
        }
        // Java: if (p.getFileSystem() == defaultFileSystem) {... }
        // The JRT/Jar paths belong to non-default file systems.
        let default_file_system = matches!(
            self.kind,
            PathFileObjectKind::Directory { .. } | PathFileObjectKind::Simple { .. }
        );
        if default_file_system {
            // TODO(P2): the macOS Normalizer.isNormalized branch is not ported.
            // Java: if (pn.equalsIgnoreCase(sn)) { try { return
            // p.toRealPath(NOFOLLOW_LINKS).getFileName().toString().equals(sn); } catch... }
            if pn.eq_ignore_ascii_case(&sn.to_string_lossy())
                && let Ok(raw) = std::fs::canonicalize(path.as_path())
                && let real = crate::file::fs_info::strip_verbatim_prefix(raw)
                && let Some(name) = real.file_name()
            {
                return name.to_string_lossy() == sn.to_string_lossy();
            }
        }
        false
    }

    /// Java: private void ensureParentDirectoriesExist() throws IOException.
    fn ensure_parent_directories_exist(&self) -> Result<(), Error> {
        if !self.has_parents.get() {
            if let Some(parent) = self.path.parent()
                && !FsFiles::is_directory(&parent)
            {
                std::fs::create_dir_all(parent.as_path()).map_err(|error| {
                    Error::io(jstr(format!(
                        "could not create parent directories: {}",
                        error
                    )))
                })?;
            }
            self.has_parents.set(true);
        }
        Ok(())
    }

    /// The file manager that created this object (Java: protected final
    /// BaseFileManager fileManager).
    pub fn file_manager(&self) -> &Rc<dyn BaseFileManagerApi> {
        &self.file_manager
    }
}

/// Java: private static Kind getKind(Path path) — the BaseFileManager.getKind
/// logic (BaseFileManager itself is not ported; TODO(P2): call the ported
/// BaseFileManager.getKind once it exists).
pub fn get_kind_from_path(path: &FsPath) -> Kind {
    let file_name = match path.file_name() {
        Some(name) => name.to_java_string(),
        None => return Kind::OTHER,
    };
    for kind in [Kind::SOURCE, Kind::CLASS, Kind::HTML] {
        if file_name.ends_with(&kind.extension().to_string_lossy()) {
            return kind;
        }
    }
    Kind::OTHER
}

/// Java: protected static String toBinaryName(RelativePath relativePath).
pub fn to_binary_name_relative(relative_path: &RelativePath) -> JavaString {
    to_binary_name(&relative_path.get_path().clone(), '/' as u16)
}

/// Java: protected static String toBinaryName(Path relativePath).
pub fn to_binary_name_path(relative_path: &FsPath) -> JavaString {
    to_binary_name(
        &jstr(relative_path.to_java_string()),
        std::path::MAIN_SEPARATOR as u16,
    )
}

/// Java: private static String toBinaryName(String relativePath, String sep).
fn to_binary_name(relative_path: &JavaString, separator: u16) -> JavaString {
    remove_extension(relative_path).replace_char(separator, '.' as u16)
}

/// Java: private static String removeExtension(String fileName).
fn remove_extension(file_name: &JavaString) -> JavaString {
    let units = file_name.as_units();
    if let Some(last_dot) = units.iter().rposition(|unit| *unit == '.' as u16) {
        file_name.substring(0, last_dot)
    } else {
        file_name.clone()
    }
}

impl fmt::Debug for PathFileObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl fmt::Display for PathFileObject {
    /// Java: public String toString() — getClass().getSimpleName() + "[" + path + "]".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let class_name = match &self.kind {
            PathFileObjectKind::Directory { .. } => "DirectoryFileObject",
            PathFileObjectKind::Jar { .. } => "JarFileObject",
            PathFileObjectKind::Jrt => "JRTFileObject",
            PathFileObjectKind::Simple { .. } => "SimpleFileObject",
        };
        write!(f, "{}[{}]", class_name, self.path.to_java_string())
    }
}

impl PartialEq for PathFileObject {
    /// Java: public boolean equals(Object other) -> path.equals(other.path).
    fn eq(&self, other: &PathFileObject) -> bool {
        self.path == other.path
    }
}

impl Eq for PathFileObject {}

impl Hash for PathFileObject {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

impl FileObject for PathFileObject {
    /// Java: public URI toUri() (Jar/JRT override in the subclasses).
    fn to_uri(&self) -> URI {
        match &self.kind {
            PathFileObjectKind::Jar { .. } => {
                unimplemented!(
                    "TODO(P2): JarFileObject.toUri needs java.net.URI.normalize/createJarUri"
                )
            }
            PathFileObjectKind::Jrt => {
                unimplemented!("TODO(P2): JRTFileObject.toUri needs the jrt FileSystem")
            }
            PathFileObjectKind::Directory { .. } => {
                let uri = self.path.to_uri().expect("PathFileObject.toUri");
                URI::create(&jstr(uri))
            }
            PathFileObjectKind::Simple { user_path } => {
                let uri = user_path.to_uri().expect("PathFileObject.toUri");
                URI::create(&jstr(&uri))
            }
        }
    }

    fn get_name(&self) -> JavaString {
        PathFileObject::get_name(self)
    }

    /// Java: public InputStream openInputStream() throws IOException.
    fn open_input_stream(&self) -> Result<Box<dyn Read>, Error> {
        self.file_manager.update_last_used_time();
        let file = std::fs::File::open(self.path.as_path()).map_err(io_error)?;
        Ok(Box::new(file))
    }

    /// Java: public OutputStream openOutputStream() throws IOException.
    fn open_output_stream(&self) -> Result<Box<dyn Write>, Error> {
        self.file_manager.update_last_used_time();
        self.file_manager.flush_cache(self);
        self.ensure_parent_directories_exist()?;
        let file = std::fs::File::create(self.path.as_path()).map_err(io_error)?;
        Ok(Box::new(file))
    }

    /// Java: public Reader openReader(boolean ignoreEncodingErrors) throws IOException.
    fn open_reader(&self, _ignore_encoding_errors: bool) -> Result<Box<dyn Reader>, Error> {
        unimplemented!(
            "TODO(P2): PathFileObject.openReader needs java.nio.charset and the BaseFileManager decoder"
        )
    }

    /// Java: public CharSequence getCharContent(boolean ignoreEncodingErrors).
    fn get_char_content(&self, _ignore_encoding_errors: bool) -> Result<Option<JavaString>, Error> {
        unimplemented!(
            "TODO(P2): PathFileObject.getCharContent needs the BaseFileManager content cache and decoder"
        )
    }

    /// Java: public Writer openWriter() throws IOException.
    fn open_writer(&self) -> Result<Box<dyn Writer>, Error> {
        unimplemented!(
            "TODO(P2): PathFileObject.openWriter needs java.nio.charset and the BaseFileManager encoding name"
        )
    }

    /// Java: public long getLastModified().
    fn get_last_modified(&self) -> i64 {
        FsFiles::last_modified_millis(&self.path).unwrap_or(0)
    }

    /// Java: public boolean delete().
    fn delete(&self) -> bool {
        let is_directory = std::fs::metadata(self.path.as_path())
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false);
        let result = if is_directory {
            std::fs::remove_dir(self.path.as_path())
        } else {
            std::fs::remove_file(self.path.as_path())
        };
        result.is_ok()
    }
}

impl JavaFileObject for PathFileObject {
    fn get_kind(&self) -> Kind {
        PathFileObject::get_kind_value(self)
    }

    fn is_name_compatible(&self, simple_name: &JavaString, kind: Kind) -> bool {
        PathFileObject::is_name_compatible_value(self, simple_name, kind)
    }

    /// Java: public NestingKind getNestingKind() { return null; }
    fn get_nesting_kind(&self) -> Option<NestingKind> {
        None
    }

    /// Java: public Modifier getAccessLevel() { return null; }
    fn get_access_level(&self) -> Option<Modifier> {
        None
    }
}

/// Maps a std::io::Error to the Java IOException role (message preserved).
fn io_error(error: std::io::Error) -> Error {
    Error::io(jstr(error.to_string()))
}

/// JavaString from any UTF-8 text (the port has no JavaString from String/&String).
fn jstr<T: AsRef<str>>(value: T) -> JavaString {
    JavaString::from(value.as_ref())
}

#[cfg(test)]
mod tests {
    use super::{PathFileObject, get_kind_from_path};
    use crate::file::base_file_manager_api::BaseFileManagerApi;
    use crate::file::relative_path::RelativeFile;
    use javac_fs::Path as FsPath;
    use javac_strings::JavaString;
    use javac_tools::tools::file_object::FileObject;
    use javac_tools::tools::java_file_object::{JavaFileObject, Kind};
    use std::cell::Cell;
    use std::io::Read;
    use std::io::Write;
    use std::panic::AssertUnwindSafe;
    use std::rc::Rc;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct StubManager {
        calls: Cell<usize>,
    }

    impl StubManager {
        fn new() -> Rc<StubManager> {
            Rc::new(StubManager {
                calls: Cell::new(0),
            })
        }
    }

    impl BaseFileManagerApi for StubManager {
        fn update_last_used_time(&self) {
            self.calls.set(self.calls.get() + 1);
        }

        fn flush_cache(&self, _file: &PathFileObject) {
            self.calls.set(self.calls.get() + 1);
        }

        fn jrt_read_resource(&self, _path: &FsPath) -> Option<Vec<u8>> {
            None
        }
    }

    fn manager() -> Rc<dyn BaseFileManagerApi> {
        StubManager::new()
    }

    fn js(s: &str) -> JavaString {
        JavaString::from(s)
    }

    fn fs_path(path: &std::path::Path) -> FsPath {
        FsPath::from_path_buf(path.to_path_buf())
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "javac-file-pfo-{}-{}-{}",
            tag,
            std::process::id(),
            unique
        ))
    }

    #[test]
    fn simple_file_object_names_uri_streams_and_flags() {
        let dir = temp_dir("simple");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("Hello.java");
        std::fs::write(&file, b"class Hello {}").expect("write");
        let path = fs_path(&file);
        let user_path = fs_path(&file);
        let fo = PathFileObject::for_simple_path(manager(), &path, &user_path);

        assert_eq!(fo.get_name().to_string_lossy(), user_path.to_java_string());
        assert_eq!(fo.get_short_name().to_string_lossy(), "Hello.java");
        assert_eq!(JavaFileObject::get_kind(&*fo), Kind::SOURCE);
        assert!(JavaFileObject::is_name_compatible(
            &*fo,
            &js("Hello"),
            Kind::SOURCE
        ));
        assert!(!JavaFileObject::is_name_compatible(
            &*fo,
            &js("Other"),
            Kind::SOURCE
        ));
        assert!(!JavaFileObject::is_name_compatible(
            &*fo,
            &js("Hello"),
            Kind::CLASS
        ));
        assert_eq!(fo.get_kind_value(), Kind::SOURCE);
        assert_eq!(fo.infer_binary_name(&[]), None);

        let uri = FileObject::to_uri(&*fo);
        assert!(uri.to_uri_string().to_string_lossy().starts_with("file:"));

        let mut content = String::new();
        FileObject::open_input_stream(&*fo)
            .expect("openInputStream")
            .read_to_string(&mut content)
            .expect("read");
        assert_eq!(content, "class Hello {}");
        assert!(FileObject::get_last_modified(&*fo) > 0);
        assert_eq!(fo.get_nesting_kind(), None);
        assert_eq!(fo.get_access_level(), None);

        // isSameFile/equals use the path (Java: canonical-by-construction).
        let same = PathFileObject::for_simple_path(manager(), &path, &user_path);
        assert!(fo.is_same_file(&same));
        assert!(fo.is_same_file(&fo));
        assert!(fo == same);
        assert_eq!(
            fo.to_string(),
            format!("SimpleFileObject[{}]", path.to_java_string())
        );

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn simple_file_object_output_creates_parents_and_delete() {
        let dir = temp_dir("output");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let target = dir.join("sub").join("Out.java");
        let path = fs_path(&target);
        let user_path = fs_path(&target);
        let fo = PathFileObject::for_simple_path(manager(), &path, &user_path);

        {
            let mut writer = FileObject::open_output_stream(&*fo).expect("openOutputStream");
            writer.write_all(b"abc").expect("write");
        }
        assert_eq!(std::fs::read(&target).expect("read back"), b"abc");
        // The parent directory was created by ensureParentDirectoriesExist.
        assert!(target.parent().expect("parent").is_dir());

        assert!(FileObject::delete(&*fo));
        assert!(!target.exists());
        assert!(!FileObject::delete(&*fo));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn simple_file_object_infer_binary_name_from_search_path() {
        let dir = temp_dir("infer");
        let root = dir.join("classes");
        std::fs::create_dir_all(root.join("pkg")).expect("dirs");
        let file = root.join("pkg").join("A.java");
        std::fs::write(&file, b"class A {}").expect("write");
        let path = fs_path(&file);
        let fo = PathFileObject::for_simple_path(manager(), &path, &path);
        let binary_name = fo
            .infer_binary_name(&[fs_path(&root)])
            .expect("binary name");
        assert_eq!(binary_name.to_string_lossy(), "pkg.A");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn directory_file_object_names_and_sibling() {
        let dir = temp_dir("directory");
        let root = dir.join("classes");
        std::fs::create_dir_all(root.join("pkg")).expect("dirs");
        let file = root.join("pkg").join("Hello.java");
        std::fs::write(&file, b"class Hello {}").expect("write");
        let path = fs_path(&file);
        let root_path = fs_path(&root);
        let relative = RelativeFile::for_class(&js("pkg.Hello"), Kind::SOURCE);
        let fo = PathFileObject::for_directory_path(
            manager(),
            &path,
            &root_path,
            relative.as_relative_path().clone(),
        );

        assert_eq!(fo.get_name().to_string_lossy(), path.to_java_string());
        assert_eq!(
            fo.infer_binary_name(&[])
                .expect("binary name")
                .to_string_lossy(),
            "pkg.Hello"
        );

        let other_path = path.resolve_sibling(&FsPath::new("Other.java").expect("path"));
        let sibling = fo.get_sibling(&js("Other.java"));
        assert_eq!(sibling.get_path(), &other_path);
        assert_eq!(
            sibling.get_name().to_string_lossy(),
            other_path.to_java_string()
        );
        assert_eq!(sibling.get_short_name().to_string_lossy(), "Other.java");

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn jrt_and_jar_file_objects() {
        let jrt_path = FsPath::new("/modules/java.base/java/lang/Object.class").expect("path");
        let jrt = PathFileObject::for_jrt_path(manager(), &jrt_path);
        assert_eq!(jrt.get_name().to_string_lossy(), jrt_path.to_java_string());
        assert_eq!(
            jrt.infer_binary_name(&[])
                .expect("binary name")
                .to_string_lossy(),
            "java.lang.Object"
        );

        let jar_path = FsPath::new("/x/y/A.class").expect("path");
        let jar = PathFileObject::for_jar_path(
            manager(),
            &jar_path,
            &FsPath::new("/x/y.jar").expect("path"),
        );
        let jar_file = FsPath::new("/x/y.jar").expect("path");
        assert_eq!(
            jar.get_name().to_string_lossy(),
            format!(
                "{}({})",
                jar_file.to_java_string(),
                jar_path.to_java_string()
            )
        );

        // Deferred upstream dependencies: jar/jrt URIs (and jar binary names).
        assert!(std::panic::catch_unwind(AssertUnwindSafe(|| FileObject::to_uri(&*jar))).is_err());
        assert!(std::panic::catch_unwind(AssertUnwindSafe(|| FileObject::to_uri(&*jrt))).is_err());
        assert!(std::panic::catch_unwind(AssertUnwindSafe(|| jar.infer_binary_name(&[]))).is_err());
    }

    #[test]
    fn deferred_reader_and_writer_methods_panic() {
        let dir = temp_dir("deferred");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("A.java");
        std::fs::write(&file, b"class A {}").expect("write");
        let path = fs_path(&file);
        let fo = PathFileObject::for_simple_path(manager(), &path, &path);

        assert!(
            std::panic::catch_unwind(AssertUnwindSafe(|| FileObject::open_reader(&*fo, false)))
                .is_err()
        );
        assert!(
            std::panic::catch_unwind(AssertUnwindSafe(|| FileObject::get_char_content(
                &*fo, false
            )))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(AssertUnwindSafe(|| FileObject::open_writer(&*fo))).is_err()
        );

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    #[should_panic(expected = "directories not supported")]
    fn directories_are_not_supported() {
        let dir = temp_dir("dirs-not-supported");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = fs_path(&dir);
        let _ = PathFileObject::for_simple_path(manager(), &path, &path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn kind_helper_and_binary_name_helpers() {
        assert_eq!(
            get_kind_from_path(&FsPath::new("a/A.java").expect("path")),
            Kind::SOURCE
        );
        assert_eq!(
            get_kind_from_path(&FsPath::new("a/A.class").expect("path")),
            Kind::CLASS
        );
        assert_eq!(
            get_kind_from_path(&FsPath::new("a/A.html").expect("path")),
            Kind::HTML
        );
        assert_eq!(
            get_kind_from_path(&FsPath::new("a/A.txt").expect("path")),
            Kind::OTHER
        );
        assert_eq!(
            super::to_binary_name_relative(
                RelativeFile::for_class(&js("pkg.A"), Kind::SOURCE).as_relative_path()
            )
            .to_string_lossy(),
            "pkg.A"
        );
        let separator = std::path::MAIN_SEPARATOR;
        let path = FsPath::new(&format!("pkg{separator}A.class")).expect("path");
        assert_eq!(super::to_binary_name_path(&path).to_string_lossy(), "pkg.A");
    }
}
