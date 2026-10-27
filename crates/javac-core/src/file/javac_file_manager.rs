// Origin: com/sun/tools/javac/file/JavacFileManager.java @ 982d53c (jdk-17.0.19-ga)
// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P2 ClassReader / API.
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: preRegister (JavacFileManager.java:150)
// - NOT PORTED: isValidName (JavacFileManager.java:235)
// - NOT PORTED: validateClassName (JavacFileManager.java:248)
// - NOT PORTED: validatePackageName (JavacFileManager.java:253)
// - NOT PORTED: testName (JavacFileManager.java:258)
// - NOT PORTED: printAscii (JavacFileManager.java:284)
// - NOT PORTED: preVisitDirectory (JavacFileManager.java:581)
// - NOT PORTED: preVisitDirectory (JavacFileManager.java:612)
// - NOT PORTED: visitFile (JavacFileManager.java:621)
// - NOT PORTED: hasExplicitLocation (JavacFileManager.java:819)
// - NOT PORTED: setLocationFromPaths (JavacFileManager.java:999)
// - NOT PORTED: compareTo (JavacFileManager.java:1101)
// - NOT PORTED: hashCode (JavacFileManager.java:1114)
// - NOT PORTED: getServiceLoader (JavacFileManager.java:1145)
// - NOT PORTED: getRelativeName (JavacFileManager.java:1240)
// - NOT PORTED: getMessage (JavacFileManager.java:1258)
// - NOT PORTED: hasNext (JavacFileManager.java:1300)
// - NOT PORTED: asFiles (JavacFileManager.java:1311)
// - NOT PORTED: hasNext (JavacFileManager.java:1319)
// End of Gap registration.
//
// clippy debt registration (pre-existing file-manager items, NOT S7b;
// cleanup in the file-manager/classpath): capitalized-acronym names
// FORWARD/REVERSE (upstream names kept, section 3), match-to-let 497,
// sort_by_key 611, &PathBuf 814, ?-operator 836, length-comparison 947/971,
// unused imports/path_buf/clone-to-slice in the test module (1230-1458).

//! A file manager for javac: the file objects are PathFileObject values over javac-fs paths, and
//! each search-path element is served by a Container (directory / archive / system image).
//!
//! <p><b>This is NOT part of any supported API.</b>
//!
//! PORTING_RULES (S3c):
//! * Java JavacFileManager extends BaseFileManager implements StandardJavaFileManager; the Rust
//!   value composes BaseFileManager (base: RefCell<...>) and implements the JavaFileManager and
//!   StandardJavaFileManager traits (javac-tools).  The mutable shared object of javac is exposed
//!   as Rc<JavacFileManager>; the PathFileObject back-pointer is a Weak<JavacFileManager>
//!   (Rc::new_cyclic) so that file objects can call back without a reference cycle.
//! * Java File / nio Path -> javac-fs Path (PathBuf only at the javac-tools trait boundary).
//!   File.pathSeparator-based class paths are handled by Locations; the JavaFileManager methods
//!   receive already-split paths through getLocationAsPaths.
//! * Context-based service lookup is not ported: Java new JavacFileManager(Context, register,
//!   charset) + setContext(Context) fetch Log/Options/FSInfo from the Context; the Rust
//!   constructor takes these services and java.home explicitly (registered deviation, same as
//!   platform/TreeMaker).  BaseFileManager::set_context(Context) is still ported and delegates
//!   to the same init_services body.
//! * org.openjdk.javac's container abstraction: Java private interface Container with the nested
//!   DirectoryContainer/JRTImageContainer/ArchiveContainer classes becomes a private Rust trait
//!   plus structs.  Directory and JRT image containers are ported here; ArchiveContainer
//!   (java.nio zip/jar FileSystem) is the second S3c (TODO in getContainer).
//! * FileObject downcast: Java uses instanceof PathFileObject to implement isSameFile /
//!   inferBinaryName / asPath / contains.  Rust trait objects cannot be downcast, so every
//!   PathFileObject created by this manager is registered in a thread-local name -> Weak table
//!   (the path name is unique per file object; the PathFileObject PartialEq compares paths too).
//!   Foreign file objects are compared by Rc identity (Java: Object.equals) and rejected by
//!   asPath/inferBinaryName with IllegalArgumentException (registered adapter).
//! * io PathFactory: StandardJavaFileManager.setPathFactory stores an Option<Rc<dyn PathFactory>>;
//!   the Locations-side PathFactory (Java locations.setPathFactory) is TODO(S3b follow-up).
//! * jar/jmod/ct.sym containers, PathFileObject JAR/JRT content readers and SigJavaFileObject are
//!   the second S3c (reported).  The directory and system-image containers, the container
//!   cache, location/search-path APIs, module location delegation and the whole JavaFileManager
//!   surface are ported and tested here.
//! * known gaps: getClassLoader (URLClassLoader/reflection, TODO(P6)); handleOption(String,
//!   Iterator) (OptionHelper not ported, TODO(OptionHelper)); ServiceLoader; close's
//!   deferredClose path (daemon thread, TODO(P6)); multiReleaseValue archive file systems
//!   (second).  Java warnings are not emitted (TODO(Log)).
//! * user.dir (getFileForOutput fallback) -> std::env::current_dir (no JVM system property).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::option::Option as StdOption;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use javac_fs::{Files as FsFiles, Path as FsPath};
use javac_strings::JavaString;
use javac_tools::error::Error;
use javac_tools::tools::option_checker::OptionChecker;
use javac_tools::tools::java_file_manager::{
    FileObjectRef, JavaFileManager, JavaFileObjectRef, Location, LocationRef,
};
use javac_tools::tools::java_file_object::Kind;
use javac_tools::tools::standard_java_file_manager::{PathFactory, StandardJavaFileManager};

use std::sync::Arc;
use std::sync::Mutex;
use javac_charset::Charset;
use crate::file::base_file_manager::BaseFileManager;
use crate::file::base_file_manager_api::BaseFileManagerApi;
use crate::file::cache_fs_info::CacheFSInfo;
use crate::file::jrt_index::JRTIndex;
use crate::file::path_file_object::PathFileObject;
use crate::file::relative_path::{RelativeDirectory, RelativeFile};
use crate::util::log::Log;
use crate::util::options::Options;

/// Java: protected enum SortFiles implements Comparator<Path>.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SortFiles {
    FORWARD,
    REVERSE,
}

impl SortFiles {
    fn compare(&self, f1: &FsPath, f2: &FsPath) -> std::cmp::Ordering {
        let n1 = file_name_string(f1);
        let n2 = file_name_string(f2);
        match self {
            SortFiles::FORWARD => n1.cmp(&n2),
            SortFiles::REVERSE => n2.cmp(&n1),
        }
    }
}

// FileObject refs already created by a manager, for the Java instanceof-based methods.
thread_local! {
    // Java sees the concrete PathFileObject through instanceof; Rust trait objects cannot be
    // downcast, so created file objects are recorded by their name (see the file header).
    static CREATED_FILE_OBJECTS: RefCell<HashMap<JavaString, Weak<PathFileObject>>> =
        RefCell::new(HashMap::new());
}

fn file_name_string(path: &FsPath) -> String {
    path.file_name()
        .map(|name| name.to_java_string())
        .unwrap_or_default()
}

fn jstr<T: AsRef<str>>(value: T) -> JavaString {
    JavaString::from(value.as_ref())
}

/// Java: private interface Container.
trait Container: Send + Sync {
    /// Java: void list(Path userPath, RelativeDirectory subdirectory,
    /// Set<JavaFileObject.Kind> fileKinds, boolean recurse, ListBuffer<JavaFileObject>).
    fn list(
        &self,
        manager: &JavacFileManager,
        user_path: &FsPath,
        subdirectory: &RelativeDirectory,
        kinds: &[Kind],
        recurse: bool,
        results: &mut Vec<JavaFileObjectRef>,
    ) -> Result<(), Error>;

    /// Java: JavaFileObject getFileObject(Path userPath, RelativeFile name).
    fn get_file_object(
        &self,
        manager: &JavacFileManager,
        user_path: &FsPath,
        name: &RelativeFile,
    ) -> Result<StdOption<JavaFileObjectRef>, Error>;

    /// Java: void close().
    fn close(&self) -> Result<(), Error>;

    /// Java: boolean maintainsDirectoryIndex().
    fn maintains_directory_index(&self) -> bool;

    /// Java: Iterable<RelativeDirectory> indexedDirectories().
    fn indexed_directories(&self) -> Vec<RelativeDirectory>;
}

/// Java: private static final Container MISSING_CONTAINER.
struct MissingContainer;

impl Container for MissingContainer {
    fn list(
        &self,
        _manager: &JavacFileManager,
        _user_path: &FsPath,
        _subdirectory: &RelativeDirectory,
        _kinds: &[Kind],
        _recurse: bool,
        _results: &mut Vec<JavaFileObjectRef>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn get_file_object(
        &self,
        _manager: &JavacFileManager,
        _user_path: &FsPath,
        _name: &RelativeFile,
    ) -> Result<StdOption<JavaFileObjectRef>, Error> {
        Ok(None)
    }

    fn close(&self) -> Result<(), Error> {
        Ok(())
    }

    fn maintains_directory_index(&self) -> bool {
        false
    }

    fn indexed_directories(&self) -> Vec<RelativeDirectory> {
        Vec::new()
    }
}

/// Java: private final class DirectoryContainer.
struct DirectoryContainer {
    directory: FsPath,
}

impl Container for DirectoryContainer {
    fn list(
        &self,
        manager: &JavacFileManager,
        user_path: &FsPath,
        subdirectory: &RelativeDirectory,
        kinds: &[Kind],
        recurse: bool,
        results: &mut Vec<JavaFileObjectRef>,
    ) -> Result<(), Error> {
        let d = match subdirectory.as_relative_path().resolve_against(user_path) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        if !manager.fs_info.exists(&d) {
            return Ok(());
        }
        if !manager.case_map_check(&d, subdirectory) {
            return Ok(());
        }
        let mut files = match FsFiles::list_directory(&d) {
            Ok(files) => files,
            Err(_) => return Ok(()),
        };
        if let Some(sort_files) = manager.sort_files.get() {
            files.sort_by(|a, b| sort_files.compare(a, b));
        }
        for f in files {
            let fname = file_name_string(&f);
            if manager.fs_info.is_directory(&f) {
                if recurse && is_identifier(&fname) {
                    let rd = RelativeDirectory::with_parent(subdirectory, &jstr(&fname));
                    self.list(manager, user_path, &rd, kinds, recurse, results)?;
                }
            } else if manager.is_valid_file(&fname, kinds) {
                let file = RelativeFile::with_directory(subdirectory, &jstr(&fname));
                let path = match file.as_relative_path().resolve_against(&self.directory) {
                    Ok(path) => path,
                    Err(_) => return Err(Error::io(jstr("error accessing directory"))),
                };
                let relative_path = file.as_relative_path().clone();
                let fo = PathFileObject::for_directory_path(
                    manager.manager_api(),
                    &path,
                    user_path,
                    relative_path,
                );
                manager.register(&fo);
                results.push(fo.clone() as JavaFileObjectRef);
            }
        }
        Ok(())
    }

    fn get_file_object(
        &self,
        manager: &JavacFileManager,
        user_path: &FsPath,
        name: &RelativeFile,
    ) -> Result<StdOption<JavaFileObjectRef>, Error> {
        let f = match name.as_relative_path().resolve_against(user_path) {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        if !manager.fs_info.exists(&f) {
            return Ok(None);
        }
        let canonical = manager.fs_info.get_canonical_file(&f);
        let fo = PathFileObject::for_simple_path(manager.manager_api(), &canonical, &f);
        manager.register(&fo);
        Ok(Some(fo.clone() as JavaFileObjectRef))
    }

    fn close(&self) -> Result<(), Error> {
        Ok(())
    }

    fn maintains_directory_index(&self) -> bool {
        false
    }

    fn indexed_directories(&self) -> Vec<RelativeDirectory> {
        Vec::new()
    }
}

/// A URLClassLoader-equivalent over the ported file layer: the class loader
/// returned by JavacFileManager.getClassLoader (registered adapter: Java returns
/// a java.net.URLClassLoader; the port loads the class/resource bytes from the
/// location paths, directories and archives, because it does not run a JVM).
pub struct PathClassLoader {
    /// Java: the URLClassLoader search path (the location paths).
    paths: Vec<FsPath>,
}

impl PathClassLoader {
    /// Creates a class loader over the given paths (directories and archives).
    pub fn new(paths: Vec<FsPath>) -> PathClassLoader {
        PathClassLoader { paths }
    }

    /// Reads the given relative resource from the search path (directories or
    /// jar/jmod/zip archives).
    fn read_resource(&self, relative: &str) -> StdOption<Vec<u8>> {
        for path in &self.paths {
            if path.as_path().is_dir() {
                let file = path.as_path().join(relative);
                if let Ok(bytes) = std::fs::read(file) {
                    return Some(bytes);
                }
            } else {
                let Ok(archive) = javac_zip::ZipArchive::open(&path.to_path_buf()) else {
                    continue;
                };
                if let Some(bytes) = archive.get_entry(relative) {
                    return Some(bytes);
                }
            }
        }
        None
    }
}

impl javac_tools::jdk::ClassLoader for PathClassLoader {
    fn find_class(&self, name: &JavaString) -> StdOption<Vec<u8>> {
        let relative = format!("{}.class", name.to_string_lossy().replace('.', "/"));
        self.read_resource(&relative)
    }

    fn find_resource(&self, name: &JavaString) -> StdOption<Vec<u8>> {
        self.read_resource(&name.to_string_lossy())
    }
}


/// Java: private final class JavacFileManager.ArchiveContainer
/// (JavacFileManager.java:557-672): a read-only container over a jar/zip/jmod
/// archive. The upstream uses the java.nio zip/jmod FileSystem; the port walks
/// the archive through javac-zip and keeps the same package index (registered
/// adapter).
struct ArchiveContainer {
    /// Java: private final Path archivePath;
    archive_path: FsPath,
    /// Java: private final Map<RelativeDirectory, Path> packages; the port keeps
    /// the valid package directories and the file entries (jar paths).
    files: Vec<String>,
    packages: Vec<String>,
}

impl ArchiveContainer {
    /// Java: ArchiveContainer(Path archivePath) throws IOException,
    /// ProviderNotFoundException, SecurityException.
    fn new(archive_path: &FsPath) -> Result<ArchiveContainer, Error> {
        let archive = javac_zip::ZipArchive::open(&archive_path.to_path_buf()).map_err(|error| {
            Error::io(JavaString::from(error.to_string().as_str()))
        })?;
        let mut files: Vec<String> = Vec::new();
        let mut packages: Vec<String> = Vec::new();
        let mut seen_packages: std::collections::HashSet<String> = std::collections::HashSet::new();
        for entry in archive.entries() {
            let name = entry.name().to_string();
            if entry.is_directory() {
                continue;
            }
            let dirname = match name.rfind('/') {
                Some(index) => &name[..index],
                None => "",
            };
            // Java: walkFileTree preVisitDirectory with isValid(fileName); a
            // non-identifier directory skips its whole subtree.
            let valid = dirname
                .split('/')
                .all(|segment| segment.is_empty() || is_identifier(segment));
            if !valid {
                continue;
            }
            let mut current = dirname;
            loop {
                if seen_packages.insert(current.to_string()) {
                    packages.push(current.to_string());
                }
                match current.rfind('/') {
                    Some(index) => current = &current[..index],
                    None => break,
                }
            }
            if seen_packages.insert(String::new()) {
                packages.push(String::new());
            }
            files.push(name);
        }
        Ok(ArchiveContainer {
            archive_path: archive_path.clone(),
            files,
            packages,
        })
    }

    /// Java: list(Path userPath, RelativeDirectory subdirectory,
    /// Set<Kind>, boolean recurse, ListBuffer<JavaFileObject>)
    /// (JavacFileManager.java:597-631).
    fn list(
        &self,
        manager: &JavacFileManager,
        _user_path: &FsPath,
        subdirectory: &RelativeDirectory,
        kinds: &[Kind],
        recurse: bool,
        results: &mut Vec<JavaFileObjectRef>,
    ) -> Result<(), Error> {
        let prefix = subdirectory.as_relative_path().get_path().to_string_lossy();
        if !self.packages.iter().any(|package| package == &prefix) {
            return Ok(());
        }
        for name in &self.files {
            let Some(rest) = name.strip_prefix(&prefix) else {
                continue;
            };
            if !recurse && rest.contains('/') {
                continue;
            }
            let entry_path = FsPath::new(&format!("/{name}"))
                .map_err(|error| Error::io(JavaString::from(error.to_string().as_str())))?;
            if manager.kind_in(kinds, &entry_path) {
                let fo = PathFileObject::for_jar_path(
                    manager.manager_api(),
                    &entry_path,
                    &self.archive_path,
                );
                manager.register(&fo);
                results.push(fo as JavaFileObjectRef);
            }
        }
        Ok(())
    }

    /// Java: getFileObject(Path userPath, RelativeFile name)
    /// (JavacFileManager.java:646-656).
    fn get_file_object(
        &self,
        manager: &JavacFileManager,
        user_path: &FsPath,
        name: &RelativeFile,
    ) -> Result<StdOption<JavaFileObjectRef>, Error> {
        let dirname = name.dirname().as_relative_path().get_path().to_string_lossy();
        let basename = name.basename().to_string_lossy();
        let entry = format!("{dirname}{basename}");
        if !self.files.iter().any(|candidate| candidate == &entry) {
            return Ok(None);
        }
        let entry_path = FsPath::new(&format!("/{entry}"))
            .map_err(|error| Error::io(JavaString::from(error.to_string().as_str())))?;
        let fo = PathFileObject::for_jar_path(manager.manager_api(), &entry_path, user_path);
        Ok(Some(fo as JavaFileObjectRef))
    }

    fn close(&self) -> Result<(), Error> {
        // Java: fileSystem.close(); the ported archive reader owns no FileSystem.
        Ok(())
    }

    fn maintains_directory_index(&self) -> bool {
        true
    }

    fn indexed_directories(&self) -> Vec<RelativeDirectory> {
        self.packages
            .iter()
            .map(|package| RelativeDirectory::new(&JavaString::from(package.as_str())))
            .collect()
    }
}

impl Container for ArchiveContainer {
    fn list(
        &self,
        manager: &JavacFileManager,
        user_path: &FsPath,
        subdirectory: &RelativeDirectory,
        kinds: &[Kind],
        recurse: bool,
        results: &mut Vec<JavaFileObjectRef>,
    ) -> Result<(), Error> {
        ArchiveContainer::list(self, manager, user_path, subdirectory, kinds, recurse, results)
    }

    fn get_file_object(
        &self,
        manager: &JavacFileManager,
        user_path: &FsPath,
        name: &RelativeFile,
    ) -> Result<StdOption<JavaFileObjectRef>, Error> {
        ArchiveContainer::get_file_object(self, manager, user_path, name)
    }

    fn close(&self) -> Result<(), Error> {
        ArchiveContainer::close(self)
    }

    fn maintains_directory_index(&self) -> bool {
        ArchiveContainer::maintains_directory_index(self)
    }

    fn indexed_directories(&self) -> Vec<RelativeDirectory> {
        ArchiveContainer::indexed_directories(self)
    }
}


/// Java: private final class JRTImageContainer (the system modules image via JRTIndex).
struct JRTImageContainer;

impl Container for JRTImageContainer {
    fn list(
        &self,
        manager: &JavacFileManager,
        _user_path: &FsPath,
        subdirectory: &RelativeDirectory,
        kinds: &[Kind],
        recurse: bool,
        results: &mut Vec<JavaFileObjectRef>,
    ) -> Result<(), Error> {
        let index = manager.get_jrt_index();
        let entry = index
            .get_entry(subdirectory)
            .map_err(|error| Error::io(jstr(format!("error reading system modules: {error:?}"))))?;
        if manager.symbol_file_enabled.get() && entry.ct_sym.hidden {
            return Ok(());
        }
        for (_, file) in &entry.files {
            if manager.kind_in(kinds, file) {
                let fo = PathFileObject::for_jrt_path(manager.manager_api(), file);
                manager.register(&fo);
                results.push(fo.clone() as JavaFileObjectRef);
            }
        }
        if recurse {
            for rd in &entry.subdirs {
                self.list(manager, _user_path, rd, kinds, recurse, results)?;
            }
        }
        Ok(())
    }

    fn get_file_object(
        &self,
        manager: &JavacFileManager,
        _user_path: &FsPath,
        name: &RelativeFile,
    ) -> Result<StdOption<JavaFileObjectRef>, Error> {
        let index = manager.get_jrt_index();
        let entry = index
            .get_entry(&name.dirname())
            .map_err(|error| Error::io(jstr(format!("error reading system modules: {error:?}"))))?;
        if manager.symbol_file_enabled.get() && entry.ct_sym.hidden {
            return Ok(None);
        }
        let basename = name.basename().to_string_lossy();
        match entry.files.iter().find(|(file_name, _)| *file_name == basename) {
            None => Ok(None),
            Some((_, path)) => {
                let fo = PathFileObject::for_jrt_path(manager.manager_api(), path);
                manager.register(&fo);
                Ok(Some(fo.clone() as JavaFileObjectRef))
            }
        }
    }

    fn close(&self) -> Result<(), Error> {
        Ok(())
    }

    fn maintains_directory_index(&self) -> bool {
        false
    }

    fn indexed_directories(&self) -> Vec<RelativeDirectory> {
        Vec::new()
    }
}

/// Java: private static class PathAndContainer implements Comparable<PathAndContainer>.
#[derive(Clone)]
struct PathAndContainer {
    path: FsPath,
    container: Arc<dyn Container>,
    index: usize,
}

/// A file manager for javac.
pub struct JavacFileManager {
    /// Java: the inherited BaseFileManager state.
    base: RefCell<BaseFileManager>,
    /// The Rc back-pointer handed to PathFileObject (Java: the manager reference).
    self_weak: Weak<JavacFileManager>,
    /// Java: private FSInfo fsInfo;
    fs_info: Rc<CacheFSInfo>,
    /// Java: protected boolean symbolFileEnabled;
    symbol_file_enabled: Cell<bool>,
    /// Java: private PathFactory pathFactory = Paths::get;
    path_factory: RefCell<StdOption<Rc<dyn PathFactory>>>,
    /// Java: protected SortFiles sortFiles;
    sort_files: Cell<StdOption<SortFiles>>,
    /// The injected java.home (see the file header).
    java_home: FsPath,
    /// Java: Locations.thisSystemModules.
    this_system_modules: FsPath,
    /// Java: private JRTIndex jrtIndex;
    jrt_index: RefCell<StdOption<Rc<JRTIndex>>>,
    /// Java: private final Map<Path, Container> containers.
    containers: Arc<Mutex<HashMap<FsPath, Arc<dyn Container>>>>,
    /// Java: pathsAndContainersByLocationAndRelativeDirectory (two-layer cache).
    paths_and_containers: Arc<std::sync::Mutex<HashMap<String, HashMap<RelativeDirectory, Vec<PathAndContainer>>>>>,
    /// Java: private Map<Location, List<PathAndContainer>> nonIndexingContainersByLocation.
    non_indexing_containers: Arc<std::sync::Mutex<HashMap<String, Vec<PathAndContainer>>>>,
}

impl JavacFileManager {
    /// The immediate close action (the non-deferred branch of JavacFileManager.close).
    fn close_now(&self) -> Result<(), Error> {
        self.base.borrow_mut().locations_mut().close()?;
        let containers: Vec<Arc<dyn Container>> = self
            .containers
            .lock()
            .expect("containers")
            .values()
            .cloned()
            .collect();
        for container in containers {
            container.close()?;
        }
        self.containers.lock().expect("containers").clear();
        self.paths_and_containers.lock().expect("paths").clear();
        self.non_indexing_containers
            .lock()
            .expect("non indexing")
            .clear();
        self.base.borrow_mut().clear_content_cache();
        Ok(())
    }
    /// Java: public JavacFileManager(Context, boolean, Charset) + setContext.
    ///
    /// PORTING_RULES: the services and java.home are injected (see the file header).
    pub fn new(
        charset: StdOption<Charset>,
        java_home: FsPath,
        log: Rc<RefCell<Log>>,
        options: Rc<Options>,
        fs_info: Rc<CacheFSInfo>,
    ) -> Rc<JavacFileManager> {
        let symbol_file_enabled = !options.is_set("ignore.symbol.file");
        let sort_files = options.get("sortFiles").map(|value| {
            if value == "reverse" {
                SortFiles::REVERSE
            } else {
                SortFiles::FORWARD
            }
        });
        Rc::new_cyclic(|self_weak| {
            let mut base = BaseFileManager::new(charset, java_home.clone());
            base.init_services(Rc::clone(&log), Rc::clone(&options), fs_info.clone());
            let this_system_modules = compute_this_system_modules(&java_home);
            JavacFileManager {
                base: RefCell::new(base),
                self_weak: self_weak.clone(),
                fs_info,
                symbol_file_enabled: Cell::new(symbol_file_enabled),
                path_factory: RefCell::new(None),
                sort_files: Cell::new(sort_files),
                java_home,
                this_system_modules,
                jrt_index: RefCell::new(None),
                containers: Arc::new(std::sync::Mutex::new(HashMap::new())),
                paths_and_containers: Arc::new(std::sync::Mutex::new(HashMap::new())),
                non_indexing_containers: Arc::new(std::sync::Mutex::new(HashMap::new())),
            }
        })
    }

    /// The PathFileObject back-pointer to this manager (Java: the BaseFileManager reference).
    pub fn manager_api(&self) -> Rc<dyn BaseFileManagerApi> {
        let manager: Rc<JavacFileManager> = self
            .self_weak
            .upgrade()
            .expect("JavacFileManager: strong reference gone");
        manager
    }

    /// Java: public void setSymbolFileEnabled(boolean b).
    pub fn set_symbol_file_enabled(&self, enabled: bool) {
        self.symbol_file_enabled.set(enabled);
    }

    /// Java: public boolean isSymbolFileEnabled().
    pub fn is_symbol_file_enabled(&self) -> bool {
        self.symbol_file_enabled.get()
    }

    /// Java: private JRTIndex getJRTIndex().
    fn get_jrt_index(&self) -> Rc<JRTIndex> {
        let mut slot = self.jrt_index.borrow_mut();
        if slot.is_none() {
            *slot = Some(JRTIndex::get_shared_instance(&self.java_home));
        }
        slot.as_ref().expect("jrt index").clone()
    }

    /// Records a created file object for the instanceof-based JavaFileManager methods.
    fn register(&self, file: &Rc<PathFileObject>) {
        CREATED_FILE_OBJECTS.with(|map| {
            map.borrow_mut()
                .insert(file.get_name(), Rc::downgrade(file));
        });
    }

    /// Downcast helper for JavaFileObjectRef (called through JavaFileObject.getName, which is
    /// also a FileObject method — no trait-object upcast needed).
    pub(crate) fn lookup_by_name(&self, name: &JavaString) -> StdOption<Rc<PathFileObject>> {
        CREATED_FILE_OBJECTS.with(|map| map.borrow().get(name).and_then(|weak| weak.upgrade()))
    }

    /// Downcast helper (see the file header).
    pub(crate) fn lookup_path_file_object(&self, fo: &FileObjectRef) -> StdOption<Rc<PathFileObject>> {
        CREATED_FILE_OBJECTS.with(|map| map.borrow().get(&fo.get_name()).and_then(|weak| weak.upgrade()))
    }

    /// Java: private Path getPath(String first, String... more).
    fn get_path(&self, first: &JavaString, more: &[JavaString]) -> FsPath {
        let factory = self.path_factory.borrow();
        match factory.as_ref() {
            Some(factory) => FsPath::from_path_buf(factory.get_path(first, more)),
            None => {
                let mut buffer = PathBuf::from(first.to_string_lossy());
                for part in more {
                    buffer.push(part.to_string_lossy());
                }
                FsPath::from_path_buf(buffer)
            }
        }
    }

    /// Convenience for the ported callers that pass a Rust string.
    fn get_path_str(&self, first: &str) -> FsPath {
        self.get_path(&jstr(first), &[])
    }

    /// Java: private boolean isValidFile(String s, Set<Kind> fileKinds).
    fn is_valid_file(&self, name: &str, kinds: &[Kind]) -> bool {
        kinds.contains(&BaseFileManager::get_kind_name(name))
    }

    /// True when the file's kind is one of the requested kinds.
    fn kind_in(&self, kinds: &[Kind], path: &FsPath) -> bool {
        kinds.contains(&BaseFileManager::get_kind(path))
    }

    /// Java: private boolean caseMapCheck(Path f, RelativePath name).
    fn case_map_check(&self, f: &FsPath, name: &RelativeDirectory) -> bool {
        if cfg!(unix) {
            return true;
        }
        let path = match self.fs_info.get_canonical_file(f) {
            path => path.to_java_string(),
        };
        let path = path.replace('\\', "/");
        let name_path = name.as_relative_path().get_path().to_string_lossy();
        let path_chars: Vec<char> = path.chars().collect();
        let name_chars: Vec<char> = name_path.chars().collect();
        let mut i = path_chars.len() as isize - 1;
        let mut j = name_chars.len() as isize - 1;
        while i >= 0 && j >= 0 {
            while i >= 0 && path_chars[i as usize] == '/' {
                i -= 1;
            }
            while j >= 0 && name_chars[j as usize] == '/' {
                j -= 1;
            }
            if i >= 0 && j >= 0 {
                if path_chars[i as usize] != name_chars[j as usize] {
                    return false;
                }
                i -= 1;
                j -= 1;
            }
        }
        j < 0
    }

    /// Java: synchronized Container getContainer(Path path) throws IOException.
    fn get_container(&self, path: &FsPath) -> Result<Arc<dyn Container>, Error> {
        if let Some(container) = self.containers.lock().unwrap().get(path) {
            return Ok(container.clone());
        }
        if self.fs_info.is_file(path) && path == &self.this_system_modules {
            let container: Arc<dyn Container> = Arc::new(JRTImageContainer);
            self.containers.lock().unwrap()
                .insert(path.clone(), container.clone());
            return Ok(container);
        }
        // Java: the jrt: filesystem paths are served by the JRT image container;
        // without a jrt FileSystem the port represents them as the jrt-style
        // textual paths /modules/<module> (see the file header), so those virtual
        // paths must map to the same container.
        if path
            .to_java_string()
            .to_string()
            .replace('\\', "/")
            .starts_with("/modules/")
        {
            let container: Arc<dyn Container> = Arc::new(JRTImageContainer);
            self.containers.lock().unwrap()
                .insert(path.clone(), container.clone());
            return Ok(container);
        }
        let real_path = self.fs_info.get_canonical_file(path);
        if let Some(container) = self.containers.lock().unwrap().get(&real_path) {
            self.containers.lock().unwrap()
                .insert(path.clone(), container.clone());
            return Ok(container.clone());
        }
        let container: Arc<dyn Container> = if !self.fs_info.exists(&real_path) {
            Arc::new(MissingContainer)
        } else if self.fs_info.is_directory(&real_path) {
            Arc::new(DirectoryContainer {
                directory: real_path.clone(),
            })
        } else {
            Arc::new(ArchiveContainer::new(&real_path)?)
        };
        self.containers.lock().unwrap()
            .insert(real_path, container.clone());
        self.containers.lock().unwrap()
            .insert(path.clone(), container.clone());
        Ok(container)
    }

    /// Java: private List<PathAndContainer> pathsAndContainers(Location, RelativeDirectory).
    fn paths_and_containers(
        &self,
        location: &dyn Location,
        relative_directory: &RelativeDirectory,
    ) -> Result<Vec<PathAndContainer>, Error> {
        let key = location.get_name().to_string_lossy();
        if !self.paths_and_containers.lock().unwrap().contains_key(&key) {
            let index = self.index_paths_and_containers_by_relative_directory(location)?;
            self.paths_and_containers.lock().unwrap().insert(key.clone(), index);
        }
        let cache = self.paths_and_containers.lock().unwrap();
        let index = cache.get(&key).expect("index");
        match index.get(relative_directory) {
            Some(list) => Ok(list.clone()),
            None => Ok(self
                .non_indexing_containers
                .lock().unwrap()
                .get(&key)
                .cloned()
                .unwrap_or_default()),
        }
    }

    /// Java: private Map<RelativeDirectory, List<PathAndContainer>>
    /// indexPathsAndContainersByRelativeDirectory(Location).
    fn index_paths_and_containers_by_relative_directory(
        &self,
        location: &dyn Location,
    ) -> Result<HashMap<RelativeDirectory, Vec<PathAndContainer>>, Error> {
        let all = self.paths_and_containers_for_location(location)?;
        let mut non_indexing: Vec<PathAndContainer> = Vec::new();
        for path_and_container in &all {
            if !path_and_container.container.maintains_directory_index() {
                non_indexing.push(path_and_container.clone());
            }
        }
        let mut result: HashMap<RelativeDirectory, Vec<PathAndContainer>> = HashMap::new();
        for path_and_container in &all {
            if path_and_container.container.maintains_directory_index() {
                for directory in path_and_container.container.indexed_directories() {
                    let entry = result.entry(directory).or_insert_with(|| non_indexing.clone());
                    entry.push(path_and_container.clone());
                }
            }
        }
        for list in result.values_mut() {
            list.sort_by(|a, b| a.index.cmp(&b.index));
        }
        let key = location.get_name().to_string_lossy();
        self.non_indexing_containers.lock().unwrap().insert(key, non_indexing);
        Ok(result)
    }

    /// Java: private List<PathAndContainer> pathsAndContainers(Location).
    fn paths_and_containers_for_location(
        &self,
        location: &dyn Location,
    ) -> Result<Vec<PathAndContainer>, Error> {
        let paths = self.get_location_as_paths_impl(location);
        let Some(paths) = paths else {
            return Ok(Vec::new());
        };
        let mut result = Vec::new();
        for path in paths {
            let container = self.get_container(&path)?;
            let index = result.len();
            result.push(PathAndContainer {
                path,
                container,
                index,
            });
        }
        Ok(result)
    }

    /// Java: Collection<Path> getLocationAsPaths(Location) via Locations.
    fn get_location_as_paths_impl(&self, location: &dyn Location) -> StdOption<Vec<FsPath>> {
        self.base.borrow_mut().locations_mut().get_location(location)
    }

    /// Java: private void clearCachesForLocation(Location location).
    fn clear_caches_for_location(&self, location: &dyn Location) {
        let key = location.get_name().to_string_lossy();
        self.paths_and_containers.lock().unwrap().remove(&key);
        self.non_indexing_containers.lock().unwrap().remove(&key);
    }

    /// Java: private void clearCaches() (used by handleOption).
    fn clear_caches(&self) {
        self.paths_and_containers.lock().unwrap().clear();
        self.non_indexing_containers.lock().unwrap().clear();
    }

    /// Java: private Path getClassOutDir().
    fn get_class_out_dir(&self) -> StdOption<FsPath> {
        self.base
            .borrow_mut()
            .locations_mut()
            .get_output_location(&javac_tools::tools::standard_location::StandardLocation::CLASS_OUTPUT)
    }

    /// Java: private Path getSourceOutDir().
    fn get_source_out_dir(&self) -> StdOption<FsPath> {
        self.base
            .borrow_mut()
            .locations_mut()
            .get_output_location(&javac_tools::tools::standard_location::StandardLocation::SOURCE_OUTPUT)
    }
}

/// Java: Locations.thisSystemModules (lib/modules file, or the exploded modules directory).
fn compute_this_system_modules(java_home: &FsPath) -> FsPath {
    let lib_modules = FsPath::from_path_buf(java_home.as_path().join("lib").join("modules"));
    if javac_fs::Files::is_regular_file(&lib_modules) {
        return lib_modules;
    }
    FsPath::from_path_buf(java_home.as_path().join("modules"))
}

/// Java: SourceVersion.isIdentifier(name) — ASCII subset (the full Unicode identifier rules
/// depend on the source level; TODO(P6) for the exact SourceVersion port).
fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        None => false,
        Some(first) => {
            (first.is_ascii_alphabetic() || first == '_' || first == '$')
                && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
        }
    }
}

use javac_tools::tools::standard_location::StandardLocation;

impl JavacFileManager {
    /// Java: private void checkOutputLocation(Location location).
    fn check_output_location(&self, location: &dyn Location) {
        if !location.is_output_location() {
            panic!(
                "IllegalArgumentException: location is not an output location: {}",
                location.get_name().to_string_lossy()
            );
        }
    }

    /// Java: private void checkModuleOrientedOrOutputLocation(Location location).
    fn check_module_oriented_or_output_location(&self, location: &dyn Location) {
        if !location.is_module_oriented_location() && !location.is_output_location() {
            panic!(
                "IllegalArgumentException: location is not an output location or a module-oriented location: {}",
                location.get_name().to_string_lossy()
            );
        }
    }

    /// Java: private void checkNotModuleOrientedLocation(Location location).
    fn check_not_module_oriented_location(&self, location: &dyn Location) {
        if location.is_module_oriented_location() {
            panic!(
                "IllegalArgumentException: location is module-oriented: {}",
                location.get_name().to_string_lossy()
            );
        }
    }

    /// Java: private JavaFileObject getFileForInput(Location, RelativeFile).
    fn get_file_for_input_relative(
        &self,
        location: &dyn Location,
        name: &RelativeFile,
    ) -> Result<StdOption<JavaFileObjectRef>, Error> {
        let Some(paths) = self.get_location_as_paths_impl(location) else {
            return Ok(None);
        };
        for path in paths {
            let container = self.get_container(&path)?;
            if let Some(fo) = container.get_file_object(self, &path, name)? {
                return Ok(Some(fo));
            }
        }
        Ok(None)
    }

    /// Java: private JavaFileObject getFileForOutput(Location, RelativeFile, FileObject).
    fn get_file_for_output_relative(
        &self,
        location: &dyn Location,
        name: &RelativeFile,
        sibling: StdOption<&FileObjectRef>,
    ) -> Result<JavaFileObjectRef, Error> {
        let dir: StdOption<FsPath> = if location_is(location, StandardLocation::CLASS_OUTPUT) {
            match self.get_class_out_dir() {
                Some(class_out_dir) => Some(class_out_dir),
                None => {
                    let base_name = name.basename();
                    if let Some(sibling) = sibling
                        && let Some(sibling) = self.lookup_path_file_object(sibling)
                    {
                        let fo = sibling.get_sibling(&base_name);
                        self.register(&fo);
                        return Ok(fo as JavaFileObjectRef);
                    }
                    let p = self.get_path(&base_name, &[]);
                    let real = self.fs_info.get_canonical_file(&p);
                    let fo = PathFileObject::for_simple_path(self.manager_api(), &real, &p);
                    self.register(&fo);
                    return Ok(fo as JavaFileObjectRef);
                }
            }
        } else if location_is(location, StandardLocation::SOURCE_OUTPUT) {
            self.get_source_out_dir().or_else(|| self.get_class_out_dir())
        } else {
            self.get_location_as_paths_impl(location)
                .and_then(|paths| paths.into_iter().next())
        };
        let dir = dir.unwrap_or_else(|| {
            let cwd = std::env::current_dir()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string());
            self.get_path_str(&cwd)
        });
        let canonical = self.fs_info.get_canonical_file(&dir);
        let path = match name.as_relative_path().resolve_against(&canonical) {
            Ok(path) => path,
            Err(_) => return Err(Error::io(jstr("bad filename"))),
        };
        let relative_path = name.as_relative_path().clone();
        let fo = PathFileObject::for_directory_path(
            self.manager_api(),
            &path,
            &dir,
            relative_path,
        );
        self.register(&fo);
        Ok(fo as JavaFileObjectRef)
    }

    /// Java: Path asPath(FileObject) (with the IllegalArgumentException of the upstream).
    fn as_path_or_panic(&self, file: &FileObjectRef) -> PathBuf {
        match self.lookup_path_file_object(file) {
            Some(path_file_object) => path_file_object.get_path().to_path_buf(),
            None => panic!(
                "IllegalArgumentException: not a PathFileObject: {}",
                file.get_name().to_string_lossy()
            ),
        }
    }

    /// Java: private JavaFileObject from a path (getJavaFileObjectsFromPaths).
    fn simple_path_file_object(&self, path: &PathBuf) -> JavaFileObjectRef {
        let p = FsPath::from_path_buf(path.clone());
        let canonical = self.fs_info.get_canonical_file(&p);
        let fo = PathFileObject::for_simple_path(self.manager_api(), &canonical, &p);
        self.register(&fo);
        fo as JavaFileObjectRef
    }
}

/// Java: location == StandardLocation.CLASS_OUTPUT (identity compared by name; the enum
/// constants are the ported identity for the standard locations).
fn location_is(location: &dyn Location, expected: StandardLocation) -> bool {
    location.get_name().to_string_lossy() == expected.name().to_string_lossy()
}

impl JavaFileManager for JavacFileManager {
    fn get_class_loader(
        &self,
        location: &dyn Location,
    ) -> StdOption<Rc<dyn javac_tools::jdk::ClassLoader>> {
        self.check_not_module_oriented_location(location);
        let paths = self.get_location_as_paths_impl(location);
        if paths.is_none() {
            return None;
        }
        // Java: new URLClassLoader over File.toURI().toURL() (JavacFileManager.java:
        // getClassLoader); the ported loader returns the class/resource bytes
        // (registered adapter: the port does not run a JVM).
        Some(Rc::new(PathClassLoader::new(paths.expect("location paths")))
            as Rc<dyn javac_tools::jdk::ClassLoader>)

    }
    fn list(
        &self,
        location: &dyn Location,
        package_name: &JavaString,
        kinds: &[Kind],
        recurse: bool,
    ) -> Result<Vec<JavaFileObjectRef>, Error> {
        self.check_not_module_oriented_location(location);
        let subdirectory = RelativeDirectory::for_package(package_name);
        let mut results: Vec<JavaFileObjectRef> = Vec::new();
        for path_and_container in self.paths_and_containers(location, &subdirectory)? {
            path_and_container.container.list(
                self,
                &path_and_container.path,
                &subdirectory,
                kinds,
                recurse,
                &mut results,
            )?;
        }
        Ok(results)
    }

    fn infer_binary_name(
        &self,
        location: &dyn Location,
        file: &JavaFileObjectRef,
    ) -> StdOption<JavaString> {
        self.check_not_module_oriented_location(location);
        let path_file_object = self.lookup_by_name(&file.get_name())?;
        let paths = self.get_location_as_paths_impl(location)?;
        path_file_object.infer_binary_name(&paths)
    }

    fn is_same_file(&self, a: &FileObjectRef, b: &FileObjectRef) -> bool {
        match (self.lookup_path_file_object(a), self.lookup_path_file_object(b)) {
            (Some(a), Some(b)) => a.is_same_file(&b),
            // Java: a.equals(b); the port falls back to Rc identity.
            _ => Rc::ptr_eq(a, b),
        }
    }

    fn handle_option(
        &self,
        current: &JavaString,
        remaining: &mut dyn Iterator<Item = JavaString>,
    ) -> bool {
        // Java: BaseFileManager.handleOption (inherited): the
        // IllegalArgumentException thrown for an invalid value is a runtime
        // exception that propagates to the caller.
        match self
            .base
            .borrow_mut()
            .handle_option(&current.to_string_lossy(), remaining)
        {
            Ok(handled) => handled,
            Err(message) => panic!("IllegalArgumentException: {}", message.to_string_lossy()),
        }
    }

    fn has_location(&self, location: &dyn Location) -> bool {
        self.base.borrow_mut().locations_mut().has_location(location)
    }

    fn get_java_file_for_input(
        &self,
        location: &dyn Location,
        class_name: &JavaString,
        kind: Kind,
    ) -> Result<StdOption<JavaFileObjectRef>, Error> {
        self.check_not_module_oriented_location(location);
        if !matches!(kind, Kind::SOURCE | Kind::CLASS) {
            panic!("IllegalArgumentException: invalid kind: {kind}");
        }
        let name = RelativeFile::for_class(class_name, kind);
        self.get_file_for_input_relative(location, &name)
    }

    fn get_java_file_for_output(
        &self,
        location: &dyn Location,
        class_name: &JavaString,
        kind: Kind,
        sibling: StdOption<&FileObjectRef>,
    ) -> Result<JavaFileObjectRef, Error> {
        self.check_output_location(location);
        if !matches!(kind, Kind::SOURCE | Kind::CLASS) {
            panic!("IllegalArgumentException: invalid kind: {kind}");
        }
        let name = RelativeFile::for_class(class_name, kind);
        self.get_file_for_output_relative(location, &name, sibling)
    }

    fn get_file_for_input(
        &self,
        location: &dyn Location,
        package_name: &JavaString,
        relative_name: &JavaString,
    ) -> Result<StdOption<FileObjectRef>, Error> {
        self.check_not_module_oriented_location(location);
        if !is_relative_uri(&relative_name.to_string_lossy()) {
            panic!(
                "IllegalArgumentException: invalid relative name: {}",
                relative_name.to_string_lossy()
            );
        }
        let name = if package_name.len() == 0 {
            RelativeFile::new(relative_name)
        } else {
            RelativeFile::with_directory(&RelativeDirectory::for_package(package_name), relative_name)
        };
        Ok(self
            .get_file_for_input_relative(location, &name)?
            .map(|file| file as FileObjectRef))
    }

    fn get_file_for_output(
        &self,
        location: &dyn Location,
        package_name: &JavaString,
        relative_name: &JavaString,
        sibling: StdOption<&FileObjectRef>,
    ) -> Result<FileObjectRef, Error> {
        self.check_output_location(location);
        if !is_relative_uri(&relative_name.to_string_lossy()) {
            panic!(
                "IllegalArgumentException: invalid relative name: {}",
                relative_name.to_string_lossy()
            );
        }
        let name = if package_name.len() == 0 {
            RelativeFile::new(relative_name)
        } else {
            RelativeFile::with_directory(&RelativeDirectory::for_package(package_name), relative_name)
        };
        Ok(self.get_file_for_output_relative(location, &name, sibling)? as FileObjectRef)
    }

    fn flush(&self) -> Result<(), Error> {
        self.base.borrow_mut().clear_content_cache();
        self.clear_caches();
        Ok(())
    }

    /// Java: public void close() throws IOException (JavacFileManager.java:422-432):
    /// with a deferred-close timeout the cleanup runs on a daemon thread after the
    /// inactivity period (BaseFileManager.java:145-166).
    ///
    /// # Arguments
    ///   - none.
    ///
    /// # Returns
    ///   - Ok(()) when the close was scheduled or completed.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: JavacFileManager.java:422-432, BaseFileManager.java:145-175.
    fn close(&self) -> Result<(), Error> {
        if self.base.borrow().deferred_close_timeout() > 0 {
            let deferred = self.base.borrow().deferred_state();
            let containers = Arc::clone(&self.containers);
            let paths_and_containers = Arc::clone(&self.paths_and_containers);
            let non_indexing_containers = Arc::clone(&self.non_indexing_containers);
            let content_cache = self.base.borrow().content_cache_handle();
            // Java: BaseFileManager.deferredClose spawns the daemon thread; the
            // ported Locations.close is a no-op (no java.nio FileSystem is opened),
            // so the deferred action is the cache/container cleanup (registered adapter).
            deferred.spawn_deferred_close(move || {
                let opened: Vec<Arc<dyn Container>> = containers
                    .lock()
                    .expect("containers")
                    .values()
                    .cloned()
                    .collect();
                for container in opened {
                    let _ = container.close();
                }
                containers.lock().expect("containers").clear();
                paths_and_containers.lock().expect("paths").clear();
                non_indexing_containers.lock().expect("non indexing").clear();
                content_cache.lock().expect("content cache").clear();
            });
            return Ok(());
        }
        self.close_now()
    }


    fn get_location_for_module(
        &self,
        location: &dyn Location,
        module_name: &JavaString,
    ) -> Result<LocationRef, Error> {
        self.check_module_oriented_or_output_location(location);
        if location_is(location, StandardLocation::SOURCE_OUTPUT)
            && self.get_source_out_dir().is_none()
        {
            let class_output = StandardLocation::CLASS_OUTPUT;
            return self.module_location(&class_output, module_name);
        }
        self.module_location(location, module_name)
    }

    fn get_location_for_module_of_file(
        &self,
        location: &dyn Location,
        fo: &JavaFileObjectRef,
    ) -> Result<LocationRef, Error> {
        self.check_module_oriented_or_output_location(location);
        let Some(path_file_object) = self.lookup_by_name(&fo.get_name()) else {
            return Err(Error::io(jstr("IllegalArgumentException: not a PathFileObject")));
        };
        let path = path_file_object.get_path().clone();
        match self
            .base
            .borrow_mut()
            .locations_mut()
            .get_location_for_module_path(location, &path)?
        {
            Some(location) => Ok(location),
            None => Err(Error::io(jstr("module not found for the given file"))),
        }
    }

    fn infer_module_name(&self, location: &dyn Location) -> Result<JavaString, Error> {
        self.check_not_module_oriented_location(location);
        match self.base.borrow_mut().locations_mut().infer_module_name(location) {
            Some(name) => Ok(jstr(name)),
            None => Err(Error::io(jstr("IllegalArgumentException: not a module location"))),
        }
    }

    fn list_locations_for_modules(
        &self,
        location: &dyn Location,
    ) -> Result<Vec<Vec<LocationRef>>, Error> {
        self.check_module_oriented_or_output_location(location);
        self.base
            .borrow_mut()
            .locations_mut()
            .list_locations_for_modules(location)
    }

    fn contains(&self, location: &dyn Location, fo: &FileObjectRef) -> Result<bool, Error> {
        let path = self.as_path_or_panic(fo);
        let file = FsPath::from_path_buf(path);
        self.base.borrow_mut().locations_mut().contains(location, &file)
    }
}

impl JavacFileManager {
    /// Helper for getLocationForModule (the Locations API returns Option).
    fn module_location(
        &self,
        location: &dyn Location,
        module_name: &JavaString,
    ) -> Result<LocationRef, Error> {
        match self
            .base
            .borrow_mut()
            .locations_mut()
            .get_location_for_module(location, &module_name.to_string_lossy())?
        {
            Some(location) => Ok(location),
            None => Err(Error::io(jstr(format!(
                "module not found: {}",
                module_name.to_string_lossy()
            )))),
        }
    }
}

impl StandardJavaFileManager for JavacFileManager {
    fn get_java_file_objects_from_files(&self, files: &[PathBuf]) -> Vec<JavaFileObjectRef> {
        files.iter().map(|path| self.simple_path_file_object(path)).collect()
    }

    fn get_java_file_objects(&self, files: &[PathBuf]) -> Vec<JavaFileObjectRef> {
        self.get_java_file_objects_from_files(files)
    }

    fn get_java_file_objects_from_strings(&self, names: &[JavaString]) -> Vec<JavaFileObjectRef> {
        names
            .iter()
            .map(|name| {
                let path = self.get_path(name, &[]);
                self.simple_path_file_object(&path.to_path_buf())
            })
            .collect()
    }

    fn get_java_file_objects_strings(&self, names: &[JavaString]) -> Vec<JavaFileObjectRef> {
        self.get_java_file_objects_from_strings(names)
    }

    fn set_location(
        &self,
        location: &dyn Location,
        files: StdOption<&[PathBuf]>,
    ) -> Result<(), Error> {
        let paths = files.map(|files| {
            files
                .iter()
                .map(|path| FsPath::from_path_buf(path.clone()))
                .collect::<Vec<FsPath>>()
        });
        self.base
            .borrow_mut()
            .locations_mut()
            .set_location(location, paths)?;
        self.clear_caches_for_location(location);
        Ok(())
    }

    fn set_location_for_module(
        &self,
        location: &dyn Location,
        module_name: &JavaString,
        paths: &[PathBuf],
    ) -> Result<(), Error> {
        self.check_module_oriented_or_output_location(location);
        let files = paths
            .iter()
            .map(|path| FsPath::from_path_buf(path.clone()))
            .collect::<Vec<FsPath>>();
        self.base
            .borrow_mut()
            .locations_mut()
            .set_location_for_module(location, &module_name.to_string_lossy(), Some(files))?;
        self.clear_caches_for_location(location);
        Ok(())
    }

    fn get_location(&self, location: &dyn Location) -> StdOption<Vec<PathBuf>> {
        self.get_location_as_paths_impl(location)
            .map(|paths| paths.iter().map(|path| path.to_path_buf()).collect())
    }

    fn get_location_as_paths(&self, location: &dyn Location) -> StdOption<Vec<PathBuf>> {
        self.get_location(location)
    }

    fn as_path(&self, file: &FileObjectRef) -> Result<PathBuf, Error> {
        Ok(self.as_path_or_panic(file))
    }

    fn set_path_factory(&self, f: Rc<dyn PathFactory>) {
        *self.path_factory.borrow_mut() = Some(f);
        // Java: locations.setPathFactory(f); the Locations-side PathFactory is TODO(S3b
        // follow-up); the file manager itself uses the injected factory for getPath.
    }
}

impl BaseFileManagerApi for JavacFileManager {
    fn update_last_used_time(&self) {
        self.base.borrow().update_last_used_time();
    }

    fn flush_cache(&self, file: &PathFileObject) {
        self.base.borrow().flush_cache_object(file);
    }

    fn jrt_read_resource(&self, path: &javac_fs::Path) -> Option<Vec<u8>> {
        self.get_jrt_index().get_resource(path)
    }
    fn decode(&self, inbuf: &[u8], ignore_encoding_errors: bool) -> JavaString {
        self.base.borrow().decode(inbuf, ignore_encoding_errors)
    }

    fn get_decoder(
        &self,
        encoding_name: &JavaString,
        ignore: bool,
    ) -> Result<javac_charset::CharsetDecoder, crate::util::jc_diagnostic::Error> {
        self.base.borrow().get_decoder(encoding_name, ignore)
    }

    fn get_encoding_name(&self) -> JavaString {
        self.base.borrow().get_encoding_name()
    }

    fn make_byte_buffer(
        &self,
        input: &mut dyn std::io::Read,
    ) -> Result<Vec<u8>, Error> {
        self.base.borrow().make_byte_buffer(input)
    }

    fn recycle_byte_buffer(&self, buffer: Vec<u8>) {
        self.base.borrow().recycle_byte_buffer(buffer);
    }

    fn get_cached_content(&self, file: &PathFileObject) -> Option<JavaString> {
        self.base.borrow().get_cached_content(file)
    }

    fn cache(&self, file: &PathFileObject, cb: JavaString) {
        self.base.borrow().cache(file, cb);
    }

    fn use_source(
        &self,
        file: &PathFileObject,
    ) -> std::option::Option<
        std::rc::Rc<dyn javac_tools::tools::java_file_object::JavaFileObject>,
    > {
        self.base.borrow().use_source(file)
    }

    fn restore_source(
        &self,
        previous: std::option::Option<
            std::rc::Rc<dyn javac_tools::tools::java_file_object::JavaFileObject>,
        >,
    ) {
        self.base.borrow().restore_source(previous);
    }
}

/// Java: protected static boolean isRelativeUri(String u) — the checks of the javadoc contract
/// (no scheme, non-empty, no leading slash/dot segments, no embedded dot segments).  The full
/// java.net.URI parsing is not ported; this covers the upstream rejection conditions
/// (registered deviation, TODO(P6)).
fn is_relative_uri(value: &str) -> bool {
    if value.is_empty() || value.contains('\\') {
        return false;
    }
    if value.starts_with('/')
        || value.starts_with("./")
        || value.starts_with("../")
        || value.ends_with('/')
    {
        return false;
    }
    // URI scheme detection: an absolute URI has 'scheme:' before any '/', '?', '#'.
    // Java: URI.isAbsolute() == (scheme != null); the scheme is the prefix before
    // the first ':', and the URI is relative when that prefix contains a path,
    // query or fragment delimiter. This mirrors the upstream
    // isRelativeUri(URI) check (`if (uri.isAbsolute()) return false;`).
    if let Some(colon) = value.find(':') {
        let rest = &value[..colon];
        if !rest.contains('/') && !rest.contains('?') && !rest.contains('#') {
            return false;
        }
    }
    for segment in value.split('/') {
        if segment == "." || segment == ".." {
            return false;
        }
    }
    true
}

impl OptionChecker for JavacFileManager {
    /// Java: BaseFileManager.isSupportedOption(String) (inherited).
    fn is_supported_option(&self, option: &JavaString) -> i32 {
        self.base
            .borrow()
            .is_supported_option(&option.to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::{is_relative_uri, JavacFileManager};
    use crate::file::cache_fs_info::CacheFSInfo;
    use crate::util::context::Context;
    use javac_fs::Path as FsPath;
    use crate::util::log::Log;
    use crate::util::options::Options;
    use javac_strings::JavaString;
    use javac_tools::tools::java_file_manager::JavaFileManager;
    use javac_tools::tools::java_file_object::Kind;
    use javac_tools::tools::standard_java_file_manager::StandardJavaFileManager;
    use javac_tools::tools::standard_location::StandardLocation;
    use std::path::PathBuf;
    use std::rc::Rc;
use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn class_loader_reads_class_and_resource_bytes() {
        let manager = manager();
        let dir = temp_dir("loader");
        let pkg = dir.join("pkg");
        std::fs::create_dir_all(&pkg).expect("package dir");
        let class_bytes = vec![0xCAu8, 0xFE, 0xBA, 0xBE];
        std::fs::write(pkg.join("A.class"), &class_bytes).expect("class file");
        std::fs::write(dir.join("resource.txt"), b"data").expect("resource file");
        manager
            .set_location(&StandardLocation::CLASS_PATH, Some(&[dir.clone()]))
            .expect("set class path");
        let loader = manager
            .get_class_loader(&StandardLocation::CLASS_PATH)
            .expect("class loader");
        assert_eq!(
            loader.find_class(&js("pkg.A")).expect("class bytes"),
            class_bytes
        );
        assert_eq!(
            loader.find_resource(&js("resource.txt")).expect("resource bytes"),
            b"data".to_vec()
        );
        assert!(loader.find_class(&js("pkg.Missing")).is_none());
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "javac-jfm-{}-{}-{}",
            tag,
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn java_home() -> FsPath {
        match std::env::var_os("JAVA_HOME") {
            Some(home) => FsPath::from_path_buf(PathBuf::from(home)),
            None => FsPath::from_path_buf(std::env::temp_dir()),
        }
    }

    fn has_jdk_home() -> bool {
        match std::env::var_os("JAVA_HOME") {
            Some(home) => PathBuf::from(home).join("lib").join("modules").is_file(),
            None => false,
        }
    }

    fn manager() -> Rc<JavacFileManager> {
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let options = Options::instance(&mut context);
        JavacFileManager::new(
            None,
            java_home(),
            log,
            options,
            Rc::new(CacheFSInfo::new()),
        )
    }

    fn js(s: &str) -> JavaString {
        JavaString::from(s)
    }

    #[test]
    fn output_location_creates_directory_file_objects() {
        let manager = manager();
        let dir = temp_dir("out");
        manager
            .set_location(&StandardLocation::CLASS_OUTPUT, Some(&[dir.clone()]))
            .expect("set class output");
        let out = manager
            .get_java_file_for_output(&StandardLocation::CLASS_OUTPUT, &js("p.C"), Kind::CLASS, None)
            .expect("class output");
        let path = manager.as_path(&(out.clone() as javac_tools::tools::java_file_manager::FileObjectRef)).expect("path");
        assert_eq!(path, dir.join("p").join("C.class"));
        assert_eq!(out.get_kind(), Kind::CLASS);
        assert!(out.get_name().to_string_lossy().ends_with("C.class"));
        assert!(manager.has_location(&StandardLocation::CLASS_OUTPUT));
        assert_eq!(manager.get_location(&StandardLocation::CLASS_OUTPUT), Some(vec![dir.clone()]));
        manager.close().expect("close");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn source_path_input_list_infer_and_contains() {
        let manager = manager();
        let dir = temp_dir("src");
        let package_dir = dir.join("p");
        std::fs::create_dir_all(&package_dir).expect("pkg dir");
        std::fs::write(package_dir.join("C.java"), b"class C {}").expect("write C");
        let sub_package = package_dir.join("q");
        std::fs::create_dir_all(&sub_package).expect("sub pkg");
        std::fs::write(sub_package.join("D.java"), b"class D {}").expect("write D");
        manager
            .set_location(&StandardLocation::SOURCE_PATH, Some(&[dir.clone()]))
            .expect("set source path");
        assert_eq!(
            manager.get_location_as_paths(&StandardLocation::SOURCE_PATH),
            Some(vec![dir.clone()])
        );

        let file = manager
            .get_java_file_for_input(&StandardLocation::SOURCE_PATH, &js("p.C"), Kind::SOURCE)
            .expect("source input")
            .expect("C.java");
        assert!(file.get_name().to_string_lossy().ends_with("C.java"));
        assert_eq!(
            manager.infer_binary_name(&StandardLocation::SOURCE_PATH, &file),
            Some(js("p.C"))
        );
        let file_ref: javac_tools::tools::java_file_manager::FileObjectRef = file.clone();
        assert!(manager.contains(&StandardLocation::SOURCE_PATH, &file_ref).expect("contains"));

        let other = manager
            .get_file_for_input(&StandardLocation::SOURCE_PATH, &js("p"), &js("C.java"))
            .expect("file input")
            .expect("C.java via file input");
        assert!(manager.is_same_file(&(file_ref.clone()), &other));

        let listed = manager
            .list(&StandardLocation::SOURCE_PATH, &js("p"), &[Kind::SOURCE], false)
            .expect("list p");
        assert_eq!(listed.len(), 1);
        assert_eq!(
            listed[0].get_kind(),
            Kind::SOURCE
        );
        let recursive = manager
            .list(&StandardLocation::SOURCE_PATH, &js("p"), &[Kind::SOURCE], true)
            .expect("list p recursive");
        assert_eq!(recursive.len(), 2);
        manager.close().expect("close");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn system_modules_image_is_listed_and_module_locations_resolve() {
        if !has_jdk_home() {
            return;
        }
        let manager = manager();
        assert!(manager.has_location(&StandardLocation::SYSTEM_MODULES));

        // Upstream: JavacFileManager.list first calls
        // checkNotModuleOrientedLocation, so the package-oriented APIs must
        // reject a module-oriented location.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let local = manager.clone();
            let _ = local.list(
                &StandardLocation::SYSTEM_MODULES,
                &js("java.lang"),
                &[Kind::CLASS],
                false,
            );
        }));
        assert!(result.is_err());

        let location = manager
            .get_location_for_module(&StandardLocation::SYSTEM_MODULES, &js("java.base"))
            .expect("java.base module location");
        assert_eq!(
            location.get_name().to_string_lossy(),
            "SYSTEM_MODULES[java.base]"
        );

        let sets = manager
            .list_locations_for_modules(&StandardLocation::SYSTEM_MODULES)
            .expect("list module locations");
        assert!(!sets.is_empty());
        // The java.base module location is part of the listed module locations.
        // (Java: the module-oriented location is enumerated through
        // listLocationsForModules; getFileForInput on a module location depends
        // on the Locations module-path mapping, which is not ported yet.)
        let names: Vec<String> = sets
            .iter()
            .flatten()
            .map(|module_location| module_location.get_name().to_string_lossy())
            .collect();
        assert!(names.iter().any(|name| name == "SYSTEM_MODULES[java.base]"));
        manager.close().expect("close");
    }

    #[test]
    fn input_objects_and_illegal_arguments() {
        let manager = manager();
        let dir = temp_dir("objs");
        let file = dir.join("X.java");
        std::fs::write(&file, b"class X {}").expect("write");
        let objects = manager.get_java_file_objects(&[file.clone()]);
        assert_eq!(objects.len(), 1);
        assert_eq!(
            manager.as_path(&(objects[0].clone() as javac_tools::tools::java_file_manager::FileObjectRef)).expect("path"),
            file
        );

        // Module-oriented locations are rejected by the package-oriented methods.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let local = manager.clone();
            local.get_java_file_for_input(
                &StandardLocation::SYSTEM_MODULES,
                &js("java.lang.Object"),
                Kind::CLASS,
            )
        }));
        assert!(result.is_err());

        // Invalid relative names are rejected.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let local = manager.clone();
            local.get_file_for_input(&StandardLocation::SOURCE_PATH, &js("p"), &js("/abs.java"))
        }));
        assert!(result.is_err());
        manager.flush().expect("flush");
        manager.close().expect("close");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn output_location_validation() {
        let manager = manager();
        let dir = temp_dir("valid");
        // CLASS_OUTPUT requires an existing directory.
        let missing = dir.join("missing");
        assert!(manager
            .set_location(&StandardLocation::CLASS_OUTPUT, Some(&[missing]))
            .is_err());
        assert!(manager
            .set_location(&StandardLocation::CLASS_OUTPUT, Some(&[dir.clone()]))
            .is_ok());
        // getJavaFileForOutput rejects a non-output location.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let local = manager.clone();
            local.set_location(&StandardLocation::SOURCE_PATH, Some(&[dir.clone()])).expect("source path");
            local.get_java_file_for_output(
                &StandardLocation::SOURCE_PATH,
                &js("p.C"),
                Kind::CLASS,
                None,
            )
        }));
        assert!(result.is_err());
        manager.close().expect("close");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn is_relative_uri_matches_uri_semantics() {
        // Java: JavacFileManager.isRelativeUri(URI) semantics.
        assert!(!is_relative_uri(""));
        assert!(!is_relative_uri("/abs"));
        assert!(!is_relative_uri("./x"));
        assert!(!is_relative_uri("../x"));
        assert!(!is_relative_uri("C:/x"));
        assert!(!is_relative_uri("a:b"));
        assert!(!is_relative_uri("a:b/c"));
        assert!(is_relative_uri("a/b"));
        assert!(is_relative_uri("a/b:c"));
        assert!(is_relative_uri("a?b:c"));
        assert!(is_relative_uri("a#b:c"));
        assert!(is_relative_uri("pkg/File.java"));
    }


    fn manager_with_defer_close(seconds: &str) -> Rc<JavacFileManager> {
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let options = Options::instance(&mut context);
        options.put("fileManager.deferClose", seconds);
        JavacFileManager::new(
            None,
            java_home(),
            log,
            options,
            Rc::new(CacheFSInfo::new()),
        )
    }

    /// Java: BaseFileManager.deferredClose (BaseFileManager.java:145-166): close()
    /// with a deferred-close timeout schedules the cleanup after the inactivity
    /// period instead of running it immediately.
    #[test]
    fn deferred_close_cleans_the_caches_after_the_inactivity_timeout() {
        let manager = manager_with_defer_close("0.05");
        let dir = temp_dir("deferred");
        std::fs::write(dir.join("A.class"), b"bytes").expect("class file");
        manager
            .set_location(&StandardLocation::CLASS_PATH, Some(&[dir.clone()]))
            .expect("class path");
        let listed = manager
            .list(&StandardLocation::CLASS_PATH, &js(""), &[Kind::CLASS], false)
            .expect("list");
        assert!(!listed.is_empty(), "the test directory lists A.class");
        assert!(
            !manager.paths_and_containers.lock().unwrap().is_empty(),
            "the directory cache is populated"
        );
        manager.close().expect("close");
        assert!(
            !manager.paths_and_containers.lock().unwrap().is_empty(),
            "the cleanup is deferred"
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !manager.paths_and_containers.lock().unwrap().is_empty() {
            assert!(
                std::time::Instant::now() < deadline,
                "the deferred close thread did not clean the caches"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    /// Java: repeated close() calls with the timeout armed schedule independent
    /// deferred-close threads (BaseFileManager.java:145-166); the cleanup is
    /// idempotent and the timeout is reset by the first expiring thread.
    #[test]
    fn repeated_close_schedules_cleanups_without_panicking() {
        let manager = manager_with_defer_close("0.05");
        manager.close().expect("first close");
        manager.close().expect("second close");
        let deferred = manager.base.borrow().deferred_state();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while deferred.timeout_ms() > 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "the deferred close thread did not reset the timeout"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        // The timeout is now zero: a third close runs the immediate path.
        manager.close().expect("third close");
    }

    /// Java: BaseFileManager.updateLastUsedTime (BaseFileManager.java:168-172) —
    /// a use of the manager extends the inactivity window (PathFileObject calls
    /// it from openInputStream/openOutputStream).
    #[test]
    fn deferred_close_defers_when_the_manager_reports_usage() {
        use crate::file::base_file_manager_api::BaseFileManagerApi;

        let manager = manager_with_defer_close("0.2");
        let dir = temp_dir("deferred-used");
        std::fs::write(dir.join("A.class"), b"bytes").expect("class file");
        manager
            .set_location(&StandardLocation::CLASS_PATH, Some(&[dir.clone()]))
            .expect("class path");
        manager
            .list(&StandardLocation::CLASS_PATH, &js(""), &[Kind::CLASS], false)
            .expect("list");
        manager.close().expect("close");
        // A use after close() refreshes the last-used time and extends the window.
        manager.update_last_used_time();
        std::thread::sleep(std::time::Duration::from_millis(120));
        assert!(
            !manager.paths_and_containers.lock().unwrap().is_empty(),
            "the refreshed last-used time extends the deferred close"
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !manager.paths_and_containers.lock().unwrap().is_empty() {
            assert!(
                std::time::Instant::now() < deadline,
                "the deferred close runs after the extended window"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
