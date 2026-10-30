// Origin: com/sun/tools/javac/file/Locations.java @ 982d53c (jdk-17.0.19-ga)
// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P2 ClassReader / API.
// Method-level status: the location-handler/search-path implementation is ported
// in this file; the remaining registered gaps are:
// - module-source-path brace/wildcard expansion (java.util.regex dependency);
// - the jar FileSystemProvider branch of contains() (see the file header);
// - BootClassPath options trimmed in task-48; close() is a no-op (no java.nio FS);
// - setPathFactory: the Java factory hook is replaced by the explicit
//   Locations::new(java_home)/LocationCtx services (ADR 0001).
// End of Gap registration.

// PORTING_RULES: the handler code keeps the upstream deeply nested `if let` structure
// (Java style); the collapsible-if/match suggestions are allowed rather than rewriting it.
#![allow(clippy::collapsible_if, clippy::collapsible_match)]

//! This class converts command line arguments, environment variables and system
//! properties (in File.pathSeparator-separated String form) into a boot class path,
//! user class path, and source path.
//!
//! <p><b>This is NOT part of any supported API.</b>
//!
//! PORTING_RULES (S3b):
//! * Java class plus its nested LocationHandler hierarchy become one Locations struct
//!   holding a Vec<Handler>; a Handler keeps the upstream (location name, options,
//!   explicit) triple plus a HandlerKind state enum (the Java handler subclasses), and
//!   dispatch is a match on the kind instead of virtual calls.
//! * java.nio.file.Path -> javac-fs Path; File.pathSeparator -> the platform separator;
//!   Files queries -> javac-fs + FSInfo/CacheFSInfo.  Handler helpers live on a small
//!   LocationCtx (fsInfo/javaHome) so the mutable handler state and the shared context can
//!   be borrowed separately.
//! * the JDK home (Java static FileSystems.getDefault().getPath(
//!   System.getProperty("java.home"))) is injected through Locations::new(java_home)
//!   (registered deviation, same as platform).  System modules come from
//!   <home>/lib/modules read with javac-jimage (the jrt: FileSystem stand-in; module
//!   locations carry the jrt-style path /modules/<module>), or from <home>/modules for an
//!   exploded image.
//! * option values may be absent (Java null): handleOption takes Option<&str>.
//! * implemented path semantics:
//!   - user class path jars expand their manifest Class-Path recursively
//!     (SearchPath.addJarClassPath; ClassPath handler passes expand=true);
//!   - invalid/nonexistent path elements report Warnings.InvalidPath /
//!     PathElementNotFound / UnexpectedArchiveFile / InvalidArchiveFile through
//!     the injected Log (warn flag honored);
//!   - non .jar/.zip files are probed with javac_zip::ZipArchive (the port's
//!     archive reader); a readable but unexpected archive warns and is accepted,
//!     an unreadable one warns InvalidArchiveFile and is skipped;
//!   - --patch-module invalid arguments report Errors.LocnInvalidArgForXpatch;
//!   - module path scanning for MODULE_PATH/UPGRADE_MODULE_PATH is implemented with
//!     javac-fs + javac-zip: exploded modules, directories of modules, jar modules
//!     (module-info.class, Automatic-Module-Name, automatic name derivation) and jmod
//!     archives; the jmod entry path resolves through the archive adapter (ModuleLocation
//!     carries the jmod file itself, see module_path_infer_module_name);
//!   - --module-source-path pattern forms (wildcard/brace expansion) need java.util.regex
//!     and the upstream expansion; only the module=path form is ported;
//!   - --patch-module accepts the NUL-separated module=path form and reports
//!     invalid arguments (Errors.LocnInvalidArgForXpatch); the brace pattern form of
//!     module-source-path remains registered (java.util.regex-dependent);
//!   - BootClassPath options were trimmed in task-48, so the handler has no options and
//!     only the default platform class path (system modules) is ported;
//!   - close() is a no-op: no java.nio FileSystem is opened in this (jar/ct.sym file
//!     systems are S3c).
//! * JavacFileManager consumes this API. Handler locations are looked up by name,
//!   equal to the upstream identity/equals contract for StandardLocation.
//! * REGISTERED dependency: the enclosing-jar branch of contains() needs the
//!   java.nio jar FileSystemProvider (upstream file.getFileSystem().provider() ==
//!   fsInfo.getJarFSProvider()); the port's FSInfo has no jar FS provider, and
//!   classpath entries are plain javac-fs paths (no jar-FS member paths), so the
//!   branch is not reachable. The directory/prefix containment is implemented.

use std::cell::Cell;
use std::option::Option as StdOption;
use std::rc::Rc;

use javac_fs::{Files, Path as FsPath};
use javac_jimage::JImage;
use javac_strings::JavaString;
use javac_tools::error::Error;
use javac_tools::tools::java_file_manager::Location;
use javac_tools::tools::standard_java_file_manager::PathFactory;
use javac_tools::tools::standard_location::StandardLocation;

use crate::file::cache_fs_info::CacheFSInfo;
use crate::main::option::Option as JavacOption;

/// Java: File.pathSeparator (platform dependent).
fn path_separator() -> char {
    if cfg!(windows) { ';' } else { ':' }
}
/// Java: ModuleSourcePathLocationHandler.isSeparator(char): both separators on
/// Windows, the platform separator and '/'.
fn is_separator_char(ch: char) -> bool {
    ch == std::path::MAIN_SEPARATOR || ch == '/'
}

/// The shared context used by the handler helpers (Java: the outer Locations fields).
struct LocationCtx<'a> {
    fs_info: &'a CacheFSInfo,
    /// Java: outer Locations.log (for path/option warnings).
    log: StdOption<&'a Rc<std::cell::RefCell<crate::util::log::Log>>>,
    /// Java: outer Locations.pathFactory.
    path_factory: StdOption<&'a Rc<dyn PathFactory>>,
    java_home: &'a FsPath,
}

impl<'a> LocationCtx<'a> {
    fn get_path(&self, first: &str) -> FsPath {
        if let Some(factory) = self.path_factory {
            return FsPath::from_path_buf(factory.get_path(&JavaString::from(first), &[]));
        }
        match FsPath::new(first) {
            Ok(path) => path,
            Err(error) => panic!("IllegalArgumentException: {error:?}"),
        }
    }

    /// Java: Locations.getPathEntries(String, Path); an invalid path is
    /// reported as Warnings.InvalidPath when warn is set (Locations.java:215-232).
    fn get_path_entries(
        &self,
        search_path: &str,
        empty_path_default: StdOption<&FsPath>,
        warn: bool,
    ) -> Vec<FsPath> {
        let mut entries = Vec::new();
        for entry in search_path.split(path_separator()) {
            if entry.is_empty() {
                if let Some(default) = empty_path_default {
                    entries.push(default.clone());
                }
            } else {
                match FsPath::new(entry) {
                    Ok(path) => entries.push(path),
                    Err(_) => {
                        if warn {
                            self.warning(
                                crate::resources::compiler_properties::warnings::invalid_path(
                                    JavaString::from(entry),
                                ),
                            );
                        }
                    }
                }
            }
        }
        entries
    }

    /// Reports a path warning through the injected Log (Locations.log).
    fn warning(&self, info: crate::resources::diagnostic_contract::WarningInfo) {
        if let Some(log) = self.log {
            use crate::util::mandatory_warning_handler::MandatoryWarningLog;
            log.borrow_mut().warning(
                Some(crate::code::lint::LintCategory::PATH),
                None,
                crate::comp::check::warning_from_info(info),
            );
        }
    }

    /// Java: addJarClassPath's log.error(Errors.ErrorReadingFile(...)).
    fn error_reading_file(&self, file: &FsPath, error: &javac_fs::Error) {
        if let Some(log) = self.log {
            use crate::util::abstract_log::AbstractLog;
            let info = crate::resources::compiler_properties::errors::error_reading_file(
                crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                    file.to_java_string(),
                ),
                JavaString::from(format!("{error:?}").as_str()),
            );
            log.borrow_mut()
                .error_pos(None, crate::util::jc_diagnostic::Error::from(info));
        }
    }

    /// Java: static Path normalize(Path p) (toRealPath, else toAbsolutePath.normalize()).
    fn normalize(&self, path: &FsPath) -> FsPath {
        self.fs_info.get_canonical_file(path)
    }

    /// Java: SearchPath.addFiles(String files) (plus the inherited warn behavior).
    fn add_files(
        &self,
        files: StdOption<&str>,
        empty_default: StdOption<&FsPath>,
        warn: bool,
        expand_jar_class_paths: bool,
    ) -> Vec<FsPath> {
        let mut result: Vec<FsPath> = Vec::new();
        let Some(files) = files else {
            return result;
        };
        for file in self.get_path_entries(files, empty_default, warn) {
            self.add_file(&mut result, &file, warn, expand_jar_class_paths);
        }
        result
    }

    /// Java: SearchPath.addDirectories(Path dir, boolean warn)
    /// (Locations.java:300-333): every directory entry is listed and archive
    /// files are added recursively. A non-directory path reports
    /// Warnings.DirPathElementNotFound and is skipped.
    #[allow(dead_code)]
    fn add_directories(&self, result: &mut Vec<FsPath>, dirs: &str, warn: bool) {
        for dir in self.get_path_entries(dirs, StdOption::None, warn) {
            self.add_directory(result, &dir, warn);
        }
    }

    fn add_directory(&self, result: &mut Vec<FsPath>, dir: &FsPath, warn: bool) {
        if !self.fs_info.is_directory(dir) {
            self.dir_path_warning(dir, warn, true);
            return;
        }
        if let Ok(entries) = Files::list_directory(dir) {
            for entry in entries {
                if self.is_archive(&entry) {
                    self.add_file(result, &entry, warn, true);
                }
            }
        }
    }

    /// Reports DirPathElementNotFound (not_found=true) or
    /// DirPathElementNotDirectory (not_found=false) for a dir path.
    fn dir_path_warning(&self, path: &FsPath, warn: bool, not_found: bool) {
        if !warn {
            return;
        }
        if not_found {
            self.warning(
                crate::resources::compiler_properties::warnings::dir_path_element_not_found(
                    crate::resources::diagnostic_contract::PathArg::placeholder(
                        path.to_java_string(),
                    ),
                ),
            );
        } else {
            self.warning(
                crate::resources::compiler_properties::warnings::dir_path_element_not_directory(
                    crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                        path.to_java_string(),
                    ),
                ),
            );
        }
    }

    /// Java: SearchPath.addFile(Path file, boolean warn)
    /// (Locations.java:359-419).
    fn add_file(
        &self,
        result: &mut Vec<FsPath>,
        file: &FsPath,
        warn: bool,
        expand_jar_class_paths: bool,
    ) {
        if result.contains(file) {
            return;
        }
        if !self.fs_info.exists(file) {
            // Java: warning(Warnings.PathElementNotFound(file)).
            if warn {
                self.warning(
                    crate::resources::compiler_properties::warnings::path_element_not_found(
                        crate::resources::diagnostic_contract::PathArg::placeholder(
                            file.to_java_string(),
                        ),
                    ),
                );
            }
            result.push(file.clone());
            return;
        }
        let canonical = self.fs_info.get_canonical_file(file);
        if result
            .iter()
            .any(|existing| self.fs_info.get_canonical_file(existing) == canonical)
        {
            return;
        }
        let name = file
            .file_name()
            .map(|name| name.to_java_string())
            .unwrap_or_default();
        if self.fs_info.is_file(file) {
            let is_jmod = name.ends_with(".jmod");
            let is_modules_image = name.ends_with("modules");
            if !is_jmod && !is_modules_image {
                if !self.is_archive(file) {
                    // Java: FileSystems.newFileSystem(file).close() probe
                    // (Locations.java:385-401). javac_zip::ZipArchive is the
                    // port's archive reader; a readable non-archive extension
                    // warns UnexpectedArchiveFile, an unreadable file warns
                    // InvalidArchiveFile and is skipped.
                    match javac_zip::ZipArchive::open(&file.as_path()) {
                        Ok(_) => {
                            if warn {
                                self.warning(
                                    crate::resources::compiler_properties::warnings::unexpected_archive_file(
                                        crate::resources::diagnostic_contract::PathArg::placeholder(
                                            file.to_java_string(),
                                        ),
                                    ),
                                );
                            }
                        }
                        Err(_) => {
                            if warn {
                                self.warning(
                                    crate::resources::compiler_properties::warnings::invalid_archive_file(
                                        crate::resources::diagnostic_contract::PathArg::placeholder(
                                            file.to_java_string(),
                                        ),
                                    ),
                                );
                            }
                            return;
                        }
                    }
                }
            }
        }
        result.push(file.clone());
        // Java: if (expandJarClassPaths && fsInfo.isFile(file) &&
        // !file.endsWith("modules")) addJarClassPath(file, warn);
        if expand_jar_class_paths
            && self.fs_info.is_file(file)
            && !name.ends_with("modules")
        {
            match self.fs_info.get_jar_class_path(file) {
                Ok(paths) => {
                    for class_path_file in paths {
                        self.add_file(result, &class_path_file, warn, true);
                    }
                }
                Err(error) => self.error_reading_file(file, &error),
            }
        }
    }

    /// Java: private boolean isArchive(Path file).
    fn is_archive(&self, file: &FsPath) -> bool {
        let Some(name) = file.file_name() else {
            return false;
        };
        let name = name.to_java_string().to_lowercase();
        self.fs_info.is_file(file) && (name.ends_with(".jar") || name.ends_with(".zip"))
    }

    /// Java: BasicLocationHandler.checkDirectory(Path).
    fn check_directory(&self, path: &FsPath) -> Result<FsPath, Error> {
        if !self.fs_info.exists(path) {
            return Err(Error::io(JavaString::from(
                format!("{path}: does not exist").as_str(),
            )));
        }
        if !self.fs_info.is_directory(path) {
            return Err(Error::io(JavaString::from(
                format!("{path}: not a directory").as_str(),
            )));
        }
        Ok(path.clone())
    }

    /// Java: BasicLocationHandler.checkSingletonDirectory(Iterable).
    fn check_singleton_directory(&self, paths: &[FsPath]) -> Result<FsPath, Error> {
        if paths.is_empty() {
            panic!("IllegalArgumentException: empty path for directory");
        }
        if paths.len() > 1 {
            panic!("IllegalArgumentException: path too long for directory");
        }
        self.check_directory(&paths[0])
    }

    /// Java: private boolean contains(Collection<Path> searchPath, Path file).
    fn contains_paths(&self, search_path: &[FsPath], file: &FsPath) -> bool {
        let normalized_file = self.normalize(file);
        for path in search_path {
            let normalized_path = self.normalize(path);
            if self.fs_info.is_directory(&normalized_path)
                && normalized_file.starts_with(&normalized_path)
            {
                return true;
            }
        // REGISTERED: the enclosing-jar case needs the java.nio jar
        // FileSystemProvider; the port's FSInfo has no jar FS (see file header).
        }
        false
    }
}

/// Java: private static abstract class LocationHandler + BasicLocationHandler.
struct Handler {
    location: JavaString,
    options: Vec<JavacOption>,
    explicit: bool,
    kind: HandlerKind,
}

fn handler(location: StandardLocation, options: Vec<JavacOption>, kind: HandlerKind) -> Handler {
    Handler {
        location: location.name(),
        options,
        explicit: false,
        kind,
    }
}

/// Java: the LocationHandler subclasses (state only; behavior is dispatched in Locations).
enum HandlerKind {
    BootClassPath(BootClassPathState),
    ClassPath(SimpleState),
    Simple(SimpleState),
    Output(OutputState),
    ModuleSourcePath(ModuleSourcePathState),
    PatchModules(PatchModulesState),
    ModulePath(ModulePathState),
    SystemModules(SystemModulesState),
}

/// Java: SimpleLocationHandler state (and ClassPathLocationHandler, which extends it).
struct SimpleState {
    /// Java: protected Collection<Path> searchPath; (null = not set).
    search_path: StdOption<Vec<FsPath>>,
}

/// Java: BootClassPathLocationHandler state.
struct BootClassPathState {
    search_path: StdOption<Vec<FsPath>>,
    /// Java: private boolean isDefault;
    is_default: bool,
}

/// Java: OutputLocationHandler state.
struct OutputState {
    /// Java: private Path outputDir;
    output_dir: StdOption<FsPath>,
    /// Java: private ModuleTable moduleTable;
    module_table: StdOption<ModuleTable>,
    /// Java: private boolean listed;
    listed: bool,
}

/// Java: ModulePathLocationHandler state.
struct ModulePathState {
    search_path: StdOption<Vec<FsPath>>,
    module_table: StdOption<ModuleTable>,
}

/// Java: ModuleSourcePathLocationHandler state.
struct ModuleSourcePathState {
    module_table: StdOption<ModuleTable>,
    /// Java: private List<Path> paths; (null when a pattern form was used).
    paths: StdOption<Vec<FsPath>>,
}

/// Java: PatchModulesLocationHandler state.
struct PatchModulesState {
    module_table: ModuleTable,
}

/// Java: SystemModulesLocationHandler state.
struct SystemModulesState {
    system_java_home: StdOption<FsPath>,
    /// Java: private Path modules;
    modules: StdOption<FsPath>,
    module_table: StdOption<ModuleTable>,
}

/// Java: private class ModuleLocationHandler implements Location (a module in a
/// module-oriented location).
#[allow(dead_code)]
pub(crate) struct ModuleLocation {
    /// Java: private final LocationHandler parent;
    parent: JavaString,
    /// Java: private final String name;
    name: JavaString,
    /// Java: private final String moduleName;
    module_name: String,
    /// Java: private final boolean output;
    output: bool,
    /// Java: Collection<Path> searchPath;
    search_path: std::cell::RefCell<Vec<FsPath>>,
    /// Java: boolean explicit;
    explicit: Cell<bool>,
}

#[allow(dead_code)]
impl ModuleLocation {
    fn new(
        parent: &JavaString,
        name: JavaString,
        module_name: &str,
        search_path: Vec<FsPath>,
        output: bool,
    ) -> Rc<ModuleLocation> {
        Rc::new(ModuleLocation {
            parent: parent.clone(),
            name,
            module_name: module_name.to_string(),
            output,
            search_path: std::cell::RefCell::new(search_path),
            explicit: Cell::new(true),
        })
    }

    fn get_module_name(&self) -> &str {
        &self.module_name
    }

    fn search_path(&self) -> Vec<FsPath> {
        self.search_path.borrow().clone()
    }

    fn set_search_path(&self, paths: Vec<FsPath>) {
        *self.search_path.borrow_mut() = paths;
    }

    fn get_parent(&self) -> &JavaString {
        &self.parent
    }

    fn is_explicit(&self) -> bool {
        self.explicit.get()
    }

    /// Java: boolean explicit (Locations.java:1855): set by setPathsForModule.
    fn set_explicit(&self, value: bool) {
        self.explicit.set(value);
    }
}

impl Location for ModuleLocation {
    fn get_name(&self) -> JavaString {
        self.name.clone()
    }

    fn is_output_location(&self) -> bool {
        self.output
    }
}

/// Java: private class ModuleTable.
struct ModuleTable {
    names: Vec<(String, Rc<ModuleLocation>)>,
    paths: Vec<(FsPath, Rc<ModuleLocation>)>,
}

#[allow(dead_code)]
impl ModuleTable {
    fn new() -> ModuleTable {
        ModuleTable {
            names: Vec::new(),
            paths: Vec::new(),
        }
    }

    fn add(&mut self, location: Rc<ModuleLocation>) {
        self.names
            .retain(|(module_name, _)| *module_name != location.module_name);
        for path in location.search_path() {
            self.paths.push((path, location.clone()));
        }
        self.names.push((location.module_name.clone(), location));
    }

    fn update_paths(&mut self, location: &Rc<ModuleLocation>) {
        self.paths
            .retain(|(_, existing)| !Rc::ptr_eq(existing, location));
        for path in location.search_path() {
            self.paths.push((path, location.clone()));
        }
    }

    fn get(&self, name: &str) -> StdOption<Rc<ModuleLocation>> {
        self.names
            .iter()
            .find(|(module_name, _)| module_name == name)
            .map(|(_, location)| location.clone())
    }

    fn get_by_path(&self, ctx: &LocationCtx<'_>, file: &FsPath) -> StdOption<Rc<ModuleLocation>> {
        let normalized = ctx.normalize(file);
        for (path, location) in &self.paths {
            if ctx.normalize(path) == normalized {
                return Some(location.clone());
            }
        }
        let mut current = file.parent();
        while let Some(path) = current {
            let normalized = ctx.normalize(&path);
            for (candidate, location) in &self.paths {
                if ctx.normalize(candidate) == normalized {
                    return Some(location.clone());
                }
            }
            current = path.parent();
        }
        None
    }

    fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    fn clear(&mut self) {
        self.names.clear();
        self.paths.clear();
    }

    fn locations(&self) -> Vec<Rc<dyn Location>> {
        self.names
            .iter()
            .map(|(_, location)| location.clone() as Rc<dyn Location>)
            .collect()
    }

    fn explicit_locations(&self) -> Vec<Rc<dyn Location>> {
        self.names
            .iter()
            .filter(|(_, location)| location.is_explicit())
            .map(|(_, location)| location.clone() as Rc<dyn Location>)
            .collect()
    }

    fn contains(&self, ctx: &LocationCtx<'_>, file: &FsPath) -> bool {
        let search_path: Vec<FsPath> = self.paths.iter().map(|(path, _)| path.clone()).collect();
        ctx.contains_paths(&search_path, file)
    }
}

/// This class converts command line arguments, environment variables and system properties
/// (in File.pathSeparator-separated String form) into a boot class path, user class path,
/// and source path (in Collection<String> form).
pub struct Locations {
    /// Java: private Log log; used for path warnings/errors (SearchPath warnings).
    #[allow(dead_code)]
    log: StdOption<Rc<std::cell::RefCell<crate::util::log::Log>>>,
    /// Java: private FSInfo fsInfo;
    fs_info: StdOption<Rc<CacheFSInfo>>,
    /// Java: private boolean warn;
    #[allow(dead_code)]
    warn: bool,
    /// Java: static final Path javaHome (injected; see the file header).
    java_home: FsPath,
    /// Java: private String multiReleaseValue / fsEnv.
    #[allow(dead_code)]
    multi_release_value: StdOption<JavaString>,
    /// Java: PathFactory pathFactory (StandardJavaFileManager.setPathFactory).
    path_factory: StdOption<Rc<dyn PathFactory>>,
    /// Java: Map<Location, LocationHandler> handlersForLocation.
    handlers: Vec<Handler>,
    /// Java: Map<Option, LocationHandler> handlersForOption.
    handlers_for_option: Vec<(JavacOption, usize)>,
}

impl Locations {
    /// Java: public Locations() — installs the default handler chain.
    pub fn new(java_home: FsPath) -> Locations {
        let mut locations = Locations {
            log: None,
            fs_info: None,
            warn: false,
            java_home,
            multi_release_value: None,
            path_factory: None,
            handlers: Vec::new(),
            handlers_for_option: Vec::new(),
        };
        locations.init_handlers();
        locations
    }

    /// Java: void update(Log log, boolean warn, FSInfo fsInfo).
    pub fn update(
        &mut self,
        log: Rc<std::cell::RefCell<crate::util::log::Log>>,
        warn: bool,
        fs_info: Rc<CacheFSInfo>,
    ) {
        self.log = Some(log);
        self.warn = warn;
        self.fs_info = Some(fs_info);
    }

    /// Java: public void setPathFactory(PathFactory f) —
    /// StandardJavaFileManager.setPathFactory sets the Locations factory too.
    pub fn set_path_factory(&mut self, factory: Rc<dyn PathFactory>) {
        self.path_factory = Some(factory);
    }

    /// Java: public void setMultiReleaseValue(String multiReleaseValue).
    pub fn set_multi_release_value(&mut self, value: &str) {
        self.multi_release_value = Some(JavaString::from(value));
    }

    /// Java: public void close() throws IOException — no file system is opened by this.
    pub fn close(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// Java: boolean isDefaultBootClassPath().
    pub fn is_default_boot_class_path(&mut self) -> bool {
        let Some(index) = self.handler_index_by_name("PLATFORM_CLASS_PATH") else {
            return false;
        };
        self.prepare(index);
        match &self.handlers[index].kind {
            HandlerKind::BootClassPath(state) => state.is_default,
            _ => crate::util::assert::error_msg("PLATFORM_CLASS_PATH handler"),
        }
    }

    /// Java: boolean isDefaultSystemModulesPath() { return !h.isExplicit(); }
    pub fn is_default_system_modules_path(&self) -> bool {
        match self.handler_index_by_name("SYSTEM_MODULES") {
            Some(index) => !self.handlers[index].explicit,
            None => true,
        }
    }

    /// Java: void initHandlers().
    fn init_handlers(&mut self) {
        let handlers = vec![
            handler(
                StandardLocation::PLATFORM_CLASS_PATH,
                Vec::new(),
                HandlerKind::BootClassPath(BootClassPathState {
                    search_path: None,
                    is_default: true,
                }),
            ),
            handler(
                StandardLocation::CLASS_PATH,
                vec![JavacOption::CLASS_PATH],
                HandlerKind::ClassPath(SimpleState { search_path: None }),
            ),
            handler(
                StandardLocation::SOURCE_PATH,
                vec![JavacOption::SOURCE_PATH],
                HandlerKind::Simple(SimpleState { search_path: None }),
            ),
            handler(
                StandardLocation::ANNOTATION_PROCESSOR_PATH,
                Vec::new(),
                HandlerKind::Simple(SimpleState { search_path: None }),
            ),
            handler(
                StandardLocation::ANNOTATION_PROCESSOR_MODULE_PATH,
                Vec::new(),
                HandlerKind::Simple(SimpleState { search_path: None }),
            ),
            handler(
                StandardLocation::CLASS_OUTPUT,
                vec![JavacOption::D],
                HandlerKind::Output(OutputState {
                    output_dir: None,
                    module_table: None,
                    listed: false,
                }),
            ),
            handler(
                StandardLocation::SOURCE_OUTPUT,
                Vec::new(),
                HandlerKind::Output(OutputState {
                    output_dir: None,
                    module_table: None,
                    listed: false,
                }),
            ),
            handler(
                StandardLocation::NATIVE_HEADER_OUTPUT,
                vec![JavacOption::H],
                HandlerKind::Output(OutputState {
                    output_dir: None,
                    module_table: None,
                    listed: false,
                }),
            ),
            handler(
                StandardLocation::MODULE_SOURCE_PATH,
                vec![JavacOption::MODULE_SOURCE_PATH],
                HandlerKind::ModuleSourcePath(ModuleSourcePathState {
                    module_table: None,
                    paths: None,
                }),
            ),
            handler(
                StandardLocation::PATCH_MODULE_PATH,
                vec![JavacOption::PATCH_MODULE],
                HandlerKind::PatchModules(PatchModulesState {
                    module_table: ModuleTable::new(),
                }),
            ),
            handler(
                StandardLocation::UPGRADE_MODULE_PATH,
                vec![JavacOption::UPGRADE_MODULE_PATH],
                HandlerKind::ModulePath(ModulePathState {
                    search_path: None,
                    module_table: None,
                }),
            ),
            handler(
                StandardLocation::MODULE_PATH,
                vec![JavacOption::MODULE_PATH],
                HandlerKind::ModulePath(ModulePathState {
                    search_path: None,
                    module_table: None,
                }),
            ),
            handler(
                StandardLocation::SYSTEM_MODULES,
                vec![JavacOption::SYSTEM],
                HandlerKind::SystemModules(SystemModulesState {
                    system_java_home: Some(self.java_home.clone()),
                    modules: None,
                    module_table: None,
                }),
            ),
        ];
        self.handlers_for_option = Vec::new();
        for (index, handler) in handlers.iter().enumerate() {
            for option in &handler.options {
                self.handlers_for_option.push((*option, index));
            }
        }
        self.handlers = handlers;
    }

    fn handler_index_by_name(&self, name: &str) -> StdOption<usize> {
        self.handlers
            .iter()
            .position(|handler| handler.location.to_string_lossy() == name)
    }

    /// Java: protected LocationHandler getHandler(Location location).
    fn handler_index(&self, location: &dyn Location) -> StdOption<usize> {
        self.handler_index_by_name(&location.get_name().to_string_lossy())
    }

    fn find_module_location(&self, name: &str) -> StdOption<Rc<ModuleLocation>> {
        for handler in &self.handlers {
            let table = match &handler.kind {
                HandlerKind::PatchModules(state) => Some(&state.module_table),
                HandlerKind::Output(state) => state.module_table.as_ref(),
                HandlerKind::ModulePath(state) => state.module_table.as_ref(),
                HandlerKind::ModuleSourcePath(state) => state.module_table.as_ref(),
                HandlerKind::SystemModules(state) => state.module_table.as_ref(),
                _ => None,
            };
            if let Some(table) = table {
                if let Some(location) = table.get(name) {
                    return Some(location);
                }
                if let Some((_, location)) = table
                    .names
                    .iter()
                    .find(|(_, location)| location.get_name().to_string() == name)
                {
                    return Some(Rc::clone(location));
                }
            }
        }
        None
    }

    /// Java: the lazy() paths of the simple/classpath/boot/system handlers.
    fn prepare(&mut self, index: usize) {
        let fs_info = self
            .fs_info
            .as_ref()
            .expect("NullPointerException: Locations.update has not been called");
        let ctx = LocationCtx {
            fs_info,
            java_home: &self.java_home,
            log: self.log.as_ref(),
            path_factory: self.path_factory.as_ref(),
        };
        match &mut self.handlers[index].kind {
            HandlerKind::BootClassPath(state) => {
                if state.search_path.is_none() {
                    state.search_path = Some(ctx.system_classes());
                }
            }
            HandlerKind::ClassPath(state) => {
                if state.search_path.is_none() {
                    // Java: env.class.path / java.class.path / application.home properties are
                    // REGISTERED: env.class.path/java.class.path/application.home come
                    // from the launcher (no JVM here); the upstream final fallback
                    // (the current directory) is used.
                    state.search_path = Some(ctx.add_files(Some("."), StdOption::None, self.warn, true));
                }
            }
            HandlerKind::ModulePath(state) => {
                if state.search_path.is_none() {
                    state.search_path = Some(Vec::new());
                }
            }
            _ => {}
        }
    }

    /// Java: boolean handleOption(Option option, String value).
    pub fn handle_option(&mut self, option: JavacOption, value: StdOption<&str>) -> bool {
        let Some(index) = self
            .handlers_for_option
            .iter()
            .find(|(candidate, _)| *candidate == option)
            .map(|(_, index)| *index)
        else {
            return false;
        };
        self.handlers[index].explicit = true;
        let fs_info = self
            .fs_info
            .as_ref()
            .expect("NullPointerException: Locations.update has not been called");
        let ctx = LocationCtx {
            fs_info,
            java_home: &self.java_home,
            log: self.log.as_ref(),
            path_factory: self.path_factory.as_ref(),
        };
        match &mut self.handlers[index].kind {
            HandlerKind::BootClassPath(_) => false,
            HandlerKind::ClassPath(state) => {
                // Java: user class path expands each jar's Class-Path manifest
                // entry (Locations.java:789 .expandJarClassPaths(true)).
                state.search_path =
                    value.map(|value| ctx.add_files(Some(value), StdOption::None, self.warn, true));
                true
            }
            HandlerKind::Simple(state) => {
                state.search_path =
                    value.map(|value| ctx.add_files(Some(value), StdOption::None, self.warn, false));
                true
            }
            HandlerKind::Output(state) => {
                state.output_dir = value.map(|value| ctx.get_path(value));
                true
            }
            HandlerKind::ModulePath(state) => {
                if let Some(files) =
                    value.map(|value| ctx.get_path_entries(value, StdOption::None, self.warn))
                {
                    for file in &files {
                        ctx.check_module_path_entry(file);
                    }
                    state.search_path = Some(files);
                } else {
                    state.search_path = None;
                }
                state.module_table = None;
                true
            }
            HandlerKind::ModuleSourcePath(state) => {
                ctx.module_source_path_init(state, value.unwrap_or(""), self.warn);
                true
            }
            HandlerKind::PatchModules(state) => {
                ctx.patch_modules_init(state, value.unwrap_or(""));
                true
            }
            HandlerKind::SystemModules(state) => {
                match value {
                    None => state.system_java_home = Some(ctx.java_home.clone()),
                    Some("none") => state.system_java_home = None,
                    Some(value) => {
                        let path = ctx.get_path(value);
                        ctx.system_modules_update(state, &path);
                    }
                }
                state.modules = None;
                true
            }
        }
    }
}

impl<'a> LocationCtx<'a> {
    /// Java: BootClassPathLocationHandler.systemClasses().
    fn system_classes(&self) -> Vec<FsPath> {
        let modules = self.java_home.as_path().join("lib").join("modules");
        let modules = FsPath::from_path_buf(modules);
        if Files::is_regular_file(&modules) {
            return vec![modules];
        }
        let exploded = FsPath::from_path_buf(self.java_home.as_path().join("modules"));
        if Files::exists(&FsPath::from_path_buf(exploded.as_path().join("java.base"))) {
            if let Ok(entries) = Files::list_directory(&exploded) {
                return entries;
            }
        }
        Vec::new()
    }

    /// Java: ModulePathLocationHandler.checkValidModulePathEntry(Path).
    fn check_module_path_entry(&self, path: &FsPath) {
        if !self.fs_info.exists(path) {
            return;
        }
        if self.fs_info.is_directory(path) {
            return;
        }
        let name = path
            .file_name()
            .map(|name| name.to_java_string())
            .unwrap_or_default();
        if !(name.ends_with(".jar") || name.ends_with(".jmod")) {
            panic!("IllegalArgumentException: {path}");
        }
    }

    /// Java: ModuleSourcePathLocationHandler.initFromPattern
    /// (Locations.java:1594-1635).
    fn module_source_path_pattern_init(
        &self,
        state: &mut ModuleSourcePathState,
        value: &str,
        warn: bool,
    ) {
        let mut segments: Vec<String> = Vec::new();
        for segment in value.split(path_separator()) {
            self.expand_braces(segment, &mut segments);
        }
        let mut map: Vec<(String, Vec<FsPath>)> = Vec::new();
        let mut no_suffix_paths: Vec<FsPath> = Vec::new();
        let mut any_suffix = false;
        const MARKER: char = '*';
        for segment in &segments {
            let mark_start = segment.find(MARKER);
            let Some(mark_start) = mark_start else {
                let path = self.get_path(segment);
                self.module_source_path_add_path(&mut map, &path, StdOption::None, warn);
                no_suffix_paths.push(path);
                continue;
            };
            if mark_start == 0
                || !is_separator_char(segment.as_bytes()[mark_start - 1] as char)
            {
                panic!("java.lang.IllegalArgumentException: illegal use of {MARKER} in {segment}");
            }
            let prefix = self.get_path(&segment[..mark_start - 1]);
            let mark_end = mark_start + 1;
            let suffix = if mark_end == segment.len() {
                StdOption::None
            } else if !is_separator_char(segment.as_bytes()[mark_end] as char)
                || segment[mark_end + 1..].contains(MARKER)
            {
                panic!("java.lang.IllegalArgumentException: illegal use of {MARKER} in {segment}");
            } else {
                any_suffix = true;
                StdOption::Some(self.get_path(&segment[mark_end + 1..]))
            };
            self.module_source_path_add_path(&mut map, &prefix, suffix.as_ref(), warn);
            if suffix.is_none() {
                no_suffix_paths.push(prefix);
            }
        }
        // Java: initModuleTable(map): keep the modules with a module-info.java.
        let mut table = ModuleTable::new();
        let parent = JavaString::from("MODULE_SOURCE_PATH");
        for (module_name, paths) in map {
            let module_info = self.get_path("module-info.java");
            let has_module_info = paths
                .iter()
                .any(|path| self.fs_info.exists(&path.resolve(&module_info)));
            if has_module_info {
                let name = format!("MODULE_SOURCE_PATH[{module_name}]");
                table.add(ModuleLocation::new(
                    &parent,
                    JavaString::from(name.as_str()),
                    &module_name,
                    paths,
                    false,
                ));
            }
        }
        state.module_table = StdOption::Some(table);
        // Java: paths = anySuffix ? null : noSuffixPaths;
        state.paths = if any_suffix {
            StdOption::None
        } else {
            StdOption::Some(no_suffix_paths)
        };
    }

    /// Java: ModuleSourcePathLocationHandler.add(Map, Path prefix, Path suffix)
    /// (Locations.java:1659-1684).
    fn module_source_path_add_path(
        &self,
        map: &mut Vec<(String, Vec<FsPath>)>,
        prefix: &FsPath,
        suffix: StdOption<&FsPath>,
        warn: bool,
    ) {
        if !self.fs_info.is_directory(prefix) {
            // Java: warn(Files.exists(prefix) ? DirPathElementNotDirectory :
            // DirPathElementNotFound) (Locations.java:1661-1664).
            self.dir_path_warning(prefix, warn, !self.fs_info.exists(prefix));
            return;
        }
        let entries = match Files::list_directory(prefix) {
            Ok(entries) => entries,
            Err(_) => return,
        };
        for entry in entries {
            if !self.fs_info.is_directory(&entry) {
                continue;
            }
            let path = match suffix {
                StdOption::Some(suffix) => entry.resolve(suffix),
                StdOption::None => entry.clone(),
            };
            if !self.fs_info.is_directory(&path) {
                continue;
            }
            let name = entry
                .file_name()
                .map(|name| name.to_java_string())
                .unwrap_or_default();
            match map.iter_mut().find(|(existing, _)| *existing == name) {
                StdOption::Some((_, paths)) => paths.push(path),
                StdOption::None => map.push((name, vec![path])),
            }
        }
    }

    /// Java: ModuleSourcePathLocationHandler.expandBraces(String, Collection)
    /// (Locations.java:1686-1729).
    fn expand_braces(&self, value: &str, results: &mut Vec<String>) {
        let mut depth = 0i32;
        let mut start = -1i32;
        let mut prefix = "";
        let mut suffix = "";
        let bytes = value.as_bytes();
        for (i, byte) in bytes.iter().enumerate() {
            match *byte {
                b'{' => {
                    depth += 1;
                    if depth == 1 {
                        prefix = &value[..i];
                        suffix = &value[self.get_matching_brace(value, i) + 1..];
                        start = i as i32 + 1;
                    }
                }
                b',' => {
                    if depth == 1 {
                        let element = &value[start as usize..i];
                        self.expand_braces(&format!("{prefix}{element}{suffix}"), results);
                        start = i as i32 + 1;
                    }
                }
                b'}' => match depth {
                    0 => panic!("java.lang.IllegalArgumentException: mismatched braces"),
                    1 => {
                        let element = &value[start as usize..i];
                        self.expand_braces(&format!("{prefix}{element}{suffix}"), results);
                        return;
                    }
                    _ => depth -= 1,
                },
                _ => {}
            }
        }
        if depth > 0 {
            panic!("java.lang.IllegalArgumentException: mismatched braces");
        }
        results.push(value.to_string());
    }

    /// Java: ModuleSourcePathLocationHandler.getMatchingBrace(String, int)
    /// (Locations.java:1731-1746).
    fn get_matching_brace(&self, value: &str, offset: usize) -> usize {
        let mut depth = 1i32;
        let bytes = value.as_bytes();
        for (i, byte) in bytes.iter().enumerate().skip(offset + 1) {
            match *byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return i;
                    }
                }
                _ => {}
            }
        }
        panic!("java.lang.IllegalArgumentException: mismatched braces");
    }


    /// Java: PatchModulesLocationHandler.handleOption.
    fn patch_modules_init(&self, state: &mut PatchModulesState, value: &str) {
        state.module_table.clear();
        let parent = JavaString::from("PATCH_MODULE_PATH");
        for entry in value.split('\0') {
            match entry.find('=') {
                Some(equal) if equal > 0 => {
                    let module_name = &entry[..equal];
                    let paths = self.add_files(Some(&entry[equal + 1..]), StdOption::None, false, false);
                    let name = format!("PATCH_MODULE_PATH[{module_name}]");
                    state.module_table.add(ModuleLocation::new(
                        &parent,
                        JavaString::from(name.as_str()),
                        module_name,
                        paths,
                        false,
                    ));
                }
                _ => {
                    // Java: log.error(Errors.LocnInvalidArgForXpatch(value)); the
                    // should-not-get-here branch of PatchModulesLocationHandler.
                    if let Some(log) = &self.log {
                        use crate::util::abstract_log::AbstractLog;
                        log.borrow_mut().error_pos(
                            None,
                            crate::util::jc_diagnostic::Error::from(
                                crate::resources::compiler_properties::errors::locn_invalid_arg_for_xpatch(
                                    JavaString::from(value),
                                ),
                            ),
                        );
                    }
                }
            }
        }
    }

    /// Java: ModuleSourcePathLocationHandler.init / initForModule (module=path form only).
    fn module_source_path_init(
        &self,
        state: &mut ModuleSourcePathState,
        value: &str,
        warn: bool,
    ) {
        let mut module_paths: Vec<(String, Vec<FsPath>)> = Vec::new();
        let mut pattern: StdOption<&str> = None;
        for entry in value.split('\0') {
            let mut is_module_specific = false;
            if let Some(equal) = entry.find('=') {
                let name = &entry[..equal];
                if !name.is_empty()
                    && name
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' || ch == '.')
                {
                    let paths = entry[equal + 1..]
                        .split(path_separator())
                        .map(|path| self.get_path(path))
                        .collect();
                    module_paths.push((name.to_string(), paths));
                    is_module_specific = true;
                }
            }
            if !is_module_specific {
                pattern = Some(entry);
            }
        }
        if let Some(pattern_entry) = pattern {
            // Java: initFromPattern(modulePattern), then the module-specific
            // initForModule overrides (ModuleSourcePathLocationHandler.init).
            self.module_source_path_pattern_init(state, pattern_entry, warn);
            if !module_paths.is_empty() {
                let table = state.module_table.get_or_insert_with(ModuleTable::new);
                let parent = JavaString::from("MODULE_SOURCE_PATH");
                for (module_name, paths) in module_paths {
                    let name = format!("MODULE_SOURCE_PATH[{module_name}]");
                    table.add(ModuleLocation::new(
                        &parent,
                        JavaString::from(name.as_str()),
                        &module_name,
                        paths,
                        false,
                    ));
                }
            }
            return;
        }
        let mut table = ModuleTable::new();
        let parent = JavaString::from("MODULE_SOURCE_PATH");
        for (module_name, paths) in module_paths {
            let name = format!("MODULE_SOURCE_PATH[{module_name}]");
            table.add(ModuleLocation::new(
                &parent,
                JavaString::from(name.as_str()),
                &module_name,
                paths,
                false,
            ));
        }
        state.module_table = Some(table);
        state.paths = None;
    }

    /// Java: SystemModulesLocationHandler.update(Path).
    fn system_modules_update(&self, state: &mut SystemModulesState, path: &FsPath) {
        let has_jrt_fs = Files::exists(&FsPath::from_path_buf(
            path.as_path().join("lib").join("jrt-fs.jar"),
        ));
        let has_modules = Files::exists(&FsPath::from_path_buf(path.as_path().join("modules")));
        if path != self.java_home && !has_jrt_fs && !has_modules {
            panic!("IllegalArgumentException: {path}");
        }
        state.system_java_home = Some(path.clone());
        state.modules = None;
    }

    /// Java: SystemModulesLocationHandler.initSystemModules().
    fn init_system_modules(&self, state: &mut SystemModulesState) -> Result<(), Error> {
        if state.module_table.is_some() {
            return Ok(());
        }
        let Some(system_java_home) = state.system_java_home.clone() else {
            state.module_table = Some(ModuleTable::new());
            return Ok(());
        };
        let parent = JavaString::from("SYSTEM_MODULES");
        let mut table = ModuleTable::new();
        let lib_modules =
            FsPath::from_path_buf(system_java_home.as_path().join("lib").join("modules"));
        let modules_path: StdOption<FsPath> = if Files::is_regular_file(&lib_modules) {
            // The jrt: FileSystem stand-in: javac-jimage over lib/modules; module locations
            // carry the jrt-style textual path /modules/<module> (see the file header).
            let image = JImage::open(&lib_modules.as_path())
                .map_err(|error| Error::io(JavaString::from(format!("{error:?}").as_str())))?;
            for module in image.modules() {
                let name = format!("SYSTEM_MODULES[{module}]");
                table.add(ModuleLocation::new(
                    &parent,
                    JavaString::from(name.as_str()),
                    module,
                    vec![FsPath::from_path_buf(std::path::PathBuf::from(format!(
                        "/modules/{module}"
                    )))],
                    false,
                ));
            }
            Some(lib_modules)
        } else {
            let modules = FsPath::from_path_buf(system_java_home.as_path().join("modules"));
            if !Files::exists(&modules) {
                return Err(Error::io(JavaString::from("can't find system classes")));
            }
            if let Ok(entries) = Files::list_directory(&modules) {
                for entry in entries {
                    if !self.fs_info.is_directory(&entry) {
                        continue;
                    }
                    let Some(name) = entry.file_name() else {
                        continue;
                    };
                    let module_name = name.to_java_string();
                    let location_name = format!("SYSTEM_MODULES[{module_name}]");
                    table.add(ModuleLocation::new(
                        &parent,
                        JavaString::from(location_name.as_str()),
                        &module_name,
                        vec![entry],
                        false,
                    ));
                }
            }
            Some(modules)
        };
        state.modules = modules_path;
        state.module_table = Some(table);
        Ok(())
    }
}

impl Locations {
    fn handler_paths(&self, index: usize) -> StdOption<Vec<FsPath>> {
        match &self.handlers[index].kind {
            HandlerKind::BootClassPath(state) => state.search_path.clone(),
            HandlerKind::ClassPath(state) | HandlerKind::Simple(state) => state.search_path.clone(),
            HandlerKind::Output(state) => state.output_dir.clone().map(|dir| vec![dir]),
            HandlerKind::ModulePath(state) => state.search_path.clone(),
            HandlerKind::ModuleSourcePath(state) => match &state.paths {
                Some(paths) => Some(paths.clone()),
                // Java: throw new IllegalStateException("paths not available").
                None => panic!("IllegalStateException: paths not available"),
            },
            HandlerKind::PatchModules(_) => panic!("UnsupportedOperationException: getPaths"),
            HandlerKind::SystemModules(state) => {
                state.system_java_home.clone().map(|home| vec![home])
            }
        }
    }

    /// Java: boolean hasLocation(Location location).
    pub fn has_location(&mut self, location: &dyn Location) -> bool {
        if let Some(module) = self.find_module_location(&location.get_name().to_string_lossy()) {
            return !module.search_path().is_empty();
        }
        let Some(index) = self.handler_index(location) else {
            return false;
        };
        self.prepare(index);
        match &self.handlers[index].kind {
            HandlerKind::PatchModules(state) => !state.module_table.is_empty(),
            HandlerKind::ModuleSourcePath(state) => state.module_table.is_some(),
            HandlerKind::Output(state) => {
                state.output_dir.is_some() || state.module_table.is_some()
            }
            HandlerKind::ModulePath(state) => state.search_path.is_some(),
            HandlerKind::Simple(state) => state.search_path.is_some(),
            HandlerKind::SystemModules(state) => state.system_java_home.is_some(),
            _ => true,
        }
    }

    /// Java: boolean hasExplicitLocation(Location location).
    pub fn has_explicit_location(&self, location: &dyn Location) -> bool {
        match self.handler_index(location) {
            None => false,
            Some(index) => self.handlers[index].explicit,
        }
    }

    /// Java: Collection<Path> getLocation(Location location).
    pub fn get_location(&mut self, location: &dyn Location) -> StdOption<Vec<FsPath>> {
        if let Some(module) = self.find_module_location(&location.get_name().to_string_lossy()) {            return Some(module.search_path());
        }        let index = self.handler_index(location)?;
        self.prepare(index);
        if let HandlerKind::PatchModules(_) = &self.handlers[index].kind {
            panic!("UnsupportedOperationException: getPaths");
        }
        self.handler_paths(index)
    }

    /// Java: Path getOutputLocation(Location location).
    pub fn get_output_location(&mut self, location: &dyn Location) -> StdOption<FsPath> {
        if !location.is_output_location() {
            panic!("IllegalArgumentException: not an output location");
        }
        let index = self
            .handler_index(location)
            .expect("IllegalArgumentException: unknown location");
        let HandlerKind::Output(state) = &self.handlers[index].kind else {
            panic!("IllegalArgumentException: not an output location");
        };
        state.output_dir.clone()
    }

    /// Java: void setLocation(Location location, Iterable<? extends Path> files).
    pub fn set_location(
        &mut self,
        location: &dyn Location,
        files: StdOption<Vec<FsPath>>,
    ) -> Result<(), Error> {
        let Some(index) = self.handler_index(location) else {
            let kind = if location.is_output_location() {
                HandlerKind::Output(OutputState {
                    output_dir: None,
                    module_table: None,
                    listed: false,
                })
            } else {
                HandlerKind::Simple(SimpleState { search_path: None })
            };
            self.handlers.push(Handler {
                location: location.get_name(),
                options: Vec::new(),
                explicit: true,
                kind,
            });
            let index = self.handlers.len() - 1;
            self.handlers[index].explicit = true;
            return self.set_handler_paths(index, files);
        };
        self.handlers[index].explicit = true;
        self.prepare(index);
        self.set_handler_paths(index, files)
    }

    fn set_handler_paths(
        &mut self,
        index: usize,
        files: StdOption<Vec<FsPath>>,
    ) -> Result<(), Error> {
        let fs_info = self
            .fs_info
            .as_ref()
            .expect("NullPointerException: Locations.update has not been called");
        let ctx = LocationCtx {
            fs_info,
            java_home: &self.java_home,
            log: self.log.as_ref(),
            path_factory: self.path_factory.as_ref(),
        };
        match &mut self.handlers[index].kind {
            HandlerKind::BootClassPath(state) => {
                match files {
                    None => state.search_path = None,
                    Some(files) => {
                        state.is_default = false;
                        state.search_path = Some(files);
                    }
                }
                Ok(())
            }
            HandlerKind::ClassPath(state) | HandlerKind::Simple(state) => {
                state.search_path = match files {
                    None => Some(Vec::new()),
                    Some(files) => Some(files),
                };
                Ok(())
            }
            HandlerKind::Output(state) => {
                match files {
                    None => state.output_dir = None,
                    Some(files) => state.output_dir = Some(ctx.check_singleton_directory(&files)?),
                }
                state.module_table = None;
                state.listed = false;
                Ok(())
            }
            HandlerKind::ModulePath(state) => {
                state.search_path = files;
                state.module_table = None;
                Ok(())
            }
            HandlerKind::ModuleSourcePath(state) => {
                let files = files.unwrap_or_default();
                let mut table = ModuleTable::new();
                for file in &files {
                    ctx.module_source_path_add(&mut table, file, StdOption::None, self.warn);
                }
                state.module_table = Some(table);
                state.paths = Some(files);
                Ok(())
            }
            HandlerKind::PatchModules(_) => panic!("UnsupportedOperationException: setLocation"),
            HandlerKind::SystemModules(state) => {
                match files {
                    None => state.system_java_home = None,
                    Some(files) => {
                        let home = ctx.check_singleton_directory(&files)?;
                        ctx.system_modules_update(state, &home);
                    }
                }
                Ok(())
            }
        }
    }

    /// Java: void setLocationForModule(Location location, String moduleName, Iterable<Path>).
    pub fn set_location_for_module(
        &mut self,
        location: &dyn Location,
        module_name: &str,
        files: StdOption<Vec<FsPath>>,
    ) -> Result<(), Error> {
        let fs_info = self
            .fs_info
            .as_ref()
            .expect("NullPointerException: Locations.update has not been called");
        let ctx = LocationCtx {
            fs_info,
            java_home: &self.java_home,
            log: self.log.as_ref(),
            path_factory: self.path_factory.as_ref(),
        };
        let index = match self.handler_index(location) {
            Some(index) => index,
            None => {
                let kind = if location.is_output_location() {
                    HandlerKind::Output(OutputState {
                        output_dir: None,
                        module_table: None,
                        listed: false,
                    })
                } else {
                    HandlerKind::ModulePath(ModulePathState {
                        search_path: None,
                        module_table: None,
                    })
                };
                self.handlers.push(Handler {
                    location: location.get_name(),
                    options: Vec::new(),
                    explicit: true,
                    kind,
                });
                self.handlers.len() - 1
            }
        };
        let is_output = self.handlers[index].kind.is_output_location();
        let parent = self.handlers[index].location.clone();
        let files = files.unwrap_or_default();
        let name = format!("{}[{}]", parent, module_name);
        let child = ModuleLocation::new(
            &parent,
            JavaString::from(name.as_str()),
            module_name,
            files,
            is_output,
        );
        match &mut self.handlers[index].kind {
            HandlerKind::Output(state) => {
                state
                    .module_table
                    .get_or_insert_with(ModuleTable::new)
                    .add(child);
            }
            HandlerKind::ModulePath(state) => {
                state
                    .module_table
                    .get_or_insert_with(ModuleTable::new)
                    .add(child);
            }
            HandlerKind::ModuleSourcePath(state) => {
                state
                    .module_table
                    .get_or_insert_with(ModuleTable::new)
                    .add(child);
            }
            HandlerKind::SystemModules(state) => {
                ctx.init_system_modules(state)?;
                state
                    .module_table
                    .get_or_insert_with(ModuleTable::new)
                    .add(child);
            }
            HandlerKind::PatchModules(_) => {
                panic!("UnsupportedOperationException: setLocationForModule")
            }
            _ => panic!("UnsupportedOperationException: setLocationForModule"),
        }
        Ok(())
    }

    /// Java: Location getLocationForModule(Location location, String name).
    pub fn get_location_for_module(
        &mut self,
        location: &dyn Location,
        module_name: &str,
    ) -> Result<StdOption<Rc<dyn Location>>, Error> {
        let Some(index) = self.handler_index(location) else {
            return Ok(None);
        };
        let fs_info = self
            .fs_info
            .as_ref()
            .expect("NullPointerException: Locations.update has not been called");
        let ctx = LocationCtx {
            fs_info,
            java_home: &self.java_home,
            log: self.log.as_ref(),
            path_factory: self.path_factory.as_ref(),
        };
        let parent = self.handlers[index].location.clone();
        let result: StdOption<Rc<ModuleLocation>> = match &mut self.handlers[index].kind {
            HandlerKind::Output(state) => {
                let table = state.module_table.get_or_insert_with(ModuleTable::new);
                if table.get(module_name).is_none() {
                    let output_dir = state
                        .output_dir
                        .clone()
                        .expect("NullPointerException: outputDir");
                    let name = format!("{parent}[{module_name}]");
                    let child = ModuleLocation::new(
                        &parent,
                        JavaString::from(name.as_str()),
                        module_name,
                        vec![FsPath::from_path_buf(
                            output_dir.as_path().join(module_name),
                        )],
                        true,
                    );
                    table.add(child);
                }
                table.get(module_name)
            }
            HandlerKind::PatchModules(state) => state.module_table.get(module_name),
            HandlerKind::ModuleSourcePath(state) => match &state.module_table {
                Some(table) => table.get(module_name),
                None => None,
            },
            HandlerKind::ModulePath(state) => {
                // Java: initModuleLocations(); moduleTable.get(moduleName)
                module_path_init_locations(fs_info, self.log.as_ref(), state, &parent);
                state.module_table.as_ref().and_then(|table| table.get(module_name))
            }
            HandlerKind::SystemModules(state) => {
                ctx.init_system_modules(state)?;
                state
                    .module_table
                    .as_ref()
                    .and_then(|table| table.get(module_name))
            }
            _ => None,
        };
        Ok(result.map(|location| location as Rc<dyn Location>))
    }

    /// Java: Location getLocationForModule(Location location, Path file)
    /// (Locations.java:2164-2167; handler overrides: Output 656, ModulePath 1156,
    /// ModuleSourcePath 1814, SystemModules 1931, PatchModules 2072).
    pub fn get_location_for_module_path(
        &mut self,
        location: &dyn Location,
        file: &FsPath,
    ) -> Result<StdOption<Rc<dyn Location>>, Error> {
        let Some(index) = self.handler_index(location) else {
            return Ok(None);
        };
        let fs_info = self
            .fs_info
            .as_ref()
            .expect("NullPointerException: Locations.update has not been called");
        let ctx = LocationCtx {
            fs_info,
            java_home: &self.java_home,
            log: self.log.as_ref(),
            path_factory: self.path_factory.as_ref(),
        };
        let parent = self.handlers[index].location.clone();
        let result: StdOption<Rc<ModuleLocation>> = match &mut self.handlers[index].kind {
            HandlerKind::Output(state) => state
                .module_table
                .as_ref()
                .and_then(|table| table.get_by_path(&ctx, file)),
            HandlerKind::PatchModules(state) => state.module_table.get_by_path(&ctx, file),
            HandlerKind::ModuleSourcePath(state) => state
                .module_table
                .as_ref()
                .and_then(|table| table.get_by_path(&ctx, file)),
            HandlerKind::ModulePath(state) => {
                // Java: initModuleLocations(); moduleTable.get(file)
                module_path_init_locations(fs_info, self.log.as_ref(), state, &parent);
                state
                    .module_table
                    .as_ref()
                    .and_then(|table| table.get_by_path(&ctx, file))
            }
            HandlerKind::SystemModules(state) => {
                ctx.init_system_modules(state)?;
                state
                    .module_table
                    .as_ref()
                    .and_then(|table| table.get_by_path(&ctx, file))
            }
            _ => None,
        };
        Ok(result.map(|location| location as Rc<dyn Location>))
    }

    /// Java: Iterable<Set<Location>> listLocationsForModules(Location location).
    pub fn list_locations_for_modules(
        &mut self,
        location: &dyn Location,
    ) -> Result<Vec<Vec<Rc<dyn Location>>>, Error> {
        let Some(index) = self.handler_index(location) else {
            return Ok(Vec::new());
        };
        let fs_info = self
            .fs_info
            .as_ref()
            .expect("NullPointerException: Locations.update has not been called");
        let ctx = LocationCtx {
            fs_info,
            java_home: &self.java_home,
            log: self.log.as_ref(),
            path_factory: self.path_factory.as_ref(),
        };
        let parent = self.handlers[index].location.clone();
        let result = match &mut self.handlers[index].kind {
            HandlerKind::Output(state) => {
                if !state.listed {
                    if let Some(output_dir) = state.output_dir.clone() {
                        if let Ok(entries) = Files::list_directory(&output_dir) {
                            for entry in entries {
                                let Some(name) = entry.file_name() else {
                                    continue;
                                };
                                let module_name = name.to_java_string();
                                let table = state.module_table.get_or_insert_with(ModuleTable::new);
                                if table.get(&module_name).is_none() {
                                    let location_name = format!("{parent}[{module_name}]");
                                    let child = ModuleLocation::new(
                                        &parent,
                                        JavaString::from(location_name.as_str()),
                                        &module_name,
                                        vec![entry],
                                        true,
                                    );
                                    table.add(child);
                                }
                            }
                        }
                    }
                    state.listed = true;
                }
                match &state.module_table {
                    None => Vec::new(),
                    Some(table) if table.is_empty() => Vec::new(),
                    Some(table) => vec![table.locations()],
                }
            }
            HandlerKind::PatchModules(state) => vec![state.module_table.locations()],
            HandlerKind::ModuleSourcePath(state) => match &state.module_table {
                None => Vec::new(),
                Some(table) => vec![table.locations()],
            },
            HandlerKind::ModulePath(state) => {
                // Java: explicitLocations + ModulePathIterator sets (Locations.java:1161-1177).
                let mut paths: Vec<Vec<Rc<dyn Location>>> = Vec::new();
                let explicit = state
                    .module_table
                    .as_ref()
                    .map(|table| table.explicit_locations())
                    .unwrap_or_default();
                if !explicit.is_empty() {
                    paths.push(explicit);
                }
                if state.search_path.is_some() {
                    for set in module_path_scan_sets(fs_info, self.log.as_ref(), state, &parent) {
                        paths.push(
                            set.into_iter()
                                .map(|location| location as Rc<dyn Location>)
                                .collect(),
                        );
                    }
                }
                paths
            }
            HandlerKind::SystemModules(state) => {
                ctx.init_system_modules(state)?;
                match &state.module_table {
                    None => Vec::new(),
                    Some(table) => vec![table.locations()],
                }
            }
            _ => Vec::new(),
        };
        Ok(result)
    }

    /// Java: String inferModuleName(Location location).
    pub fn infer_module_name(&self, location: &dyn Location) -> StdOption<String> {
        self.find_module_location(&location.get_name().to_string_lossy())
            .map(|module| module.get_module_name().to_string())
    }

    /// Java: boolean contains(Location location, Path file).
    pub fn contains(&mut self, location: &dyn Location, file: &FsPath) -> Result<bool, Error> {
        let Some(index) = self.handler_index(location) else {
            panic!("IllegalArgumentException: unknown location");
        };
        self.prepare(index);
        let fs_info = self
            .fs_info
            .as_ref()
            .expect("NullPointerException: Locations.update has not been called");
        let ctx = LocationCtx {
            fs_info,
            java_home: &self.java_home,
            log: self.log.as_ref(),
            path_factory: self.path_factory.as_ref(),
        };
        if let Some(module) = self.find_module_location(&location.get_name().to_string_lossy()) {
            return Ok(ctx.contains_paths(&module.search_path(), file));
        }
        if let Some(module) = self.handler_kind_module_location(index, &ctx, file) {
            return Ok(module);
        }
        match self.handler_paths(index) {
            Some(paths) => Ok(ctx.contains_paths(&paths, file)),
            None => Ok(false),
        }
    }

    /// The module-table based containment of an output/system/module handler.
    fn handler_kind_module_location(
        &self,
        index: usize,
        ctx: &LocationCtx<'_>,
        file: &FsPath,
    ) -> StdOption<bool> {
        match &self.handlers[index].kind {
            HandlerKind::Output(state) => state
                .module_table
                .as_ref()
                .map(|table| table.contains(ctx, file)),
            HandlerKind::PatchModules(state) => Some(state.module_table.contains(ctx, file)),
            HandlerKind::ModuleSourcePath(state) => state
                .module_table
                .as_ref()
                .map(|table| table.contains(ctx, file)),
            _ => None,
        }
    }
}

impl HandlerKind {
    fn is_output_location(&self) -> bool {
        matches!(self, HandlerKind::Output(_))
    }
}

impl<'a> LocationCtx<'a> {
    /// Java: ModuleSourcePathLocationHandler.add(Map, prefix, suffix).
    fn module_source_path_add(
        &self,
        table: &mut ModuleTable,
        prefix: &FsPath,
        suffix: StdOption<&FsPath>,
        warn: bool,
    ) {
        if !self.fs_info.is_directory(prefix) {
            self.dir_path_warning(prefix, warn, !self.fs_info.exists(prefix));
            return;
        }
        let Ok(entries) = Files::list_directory(prefix) else {
            return;
        };
        let parent = JavaString::from("MODULE_SOURCE_PATH");
        for entry in entries {
            if !self.fs_info.is_directory(&entry) {
                continue;
            }
            let path = match suffix {
                None => entry.clone(),
                Some(suffix) => FsPath::from_path_buf(entry.as_path().join(suffix.as_path())),
            };
            if !self.fs_info.is_directory(&path) {
                continue;
            }
            let Some(name) = entry.file_name() else {
                continue;
            };
            let module_name = name.to_java_string();
            let location_name = format!("MODULE_SOURCE_PATH[{module_name}]");
            table.add(ModuleLocation::new(
                &parent,
                JavaString::from(location_name.as_str()),
                &module_name,
                vec![path],
                false,
            ));
        }
    }
}

/// Java: ModulePathLocationHandler.initModuleLocations (Locations.java:1226-1242).
fn module_path_init_locations(
    fs_info: &CacheFSInfo,
    log: StdOption<&Rc<std::cell::RefCell<crate::util::log::Log>>>,
    state: &mut ModulePathState,
    parent: &JavaString,
) {
    if state.module_table.is_some() {
        return;
    }
    let mut table = ModuleTable::new();
    if let Some(search_path) = state.search_path.clone() {
        for (index, path) in search_path.iter().enumerate() {
            let set = module_path_scan_path(fs_info, log, path, index, parent);
            for location in set {
                if table.get(location.get_module_name()).is_none() {
                    table.add(location);
                }
            }
        }
    }
    state.module_table = Some(table);
}

/// Java: ModulePathIterator (Locations.java:1267-1502): one set per search path entry.
fn module_path_scan_sets(
    fs_info: &CacheFSInfo,
    log: StdOption<&Rc<std::cell::RefCell<crate::util::log::Log>>>,
    state: &ModulePathState,
    parent: &JavaString,
) -> Vec<Vec<Rc<ModuleLocation>>> {
    let mut result = Vec::new();
    let Some(search_path) = state.search_path.as_ref() else {
        return result;
    };
    for (index, path) in search_path.iter().enumerate() {
        result.push(module_path_scan_path(fs_info, log, path, index, parent));
    }
    result
}

/// Java: ModulePathIterator.scanDirectory/scanFile dispatch (Locations.java:1303-1375).
fn module_path_scan_path(
    fs_info: &CacheFSInfo,
    log: StdOption<&Rc<std::cell::RefCell<crate::util::log::Log>>>,
    path: &FsPath,
    path_index: usize,
    parent: &JavaString,
) -> Vec<Rc<ModuleLocation>> {
    if fs_info.is_directory(path) {
        module_path_scan_directory(fs_info, log, path, path_index, parent)
    } else {
        match module_path_infer_module_name(fs_info, log, path) {
            Some((module_name, module_paths)) => {
                let name = format!("{parent}[{path_index}:{module_name}]");
                let location = ModuleLocation::new(
                    parent,
                    JavaString::from(name.as_str()),
                    &module_name,
                    module_paths,
                    false,
                );
                // Locations.java:1279: the scanned locations are not explicit.
                location.set_explicit(false);
                vec![location]
            }
            None => Vec::new(),
        }
    }
}

/// Java: ModulePathIterator.scanDirectory (Locations.java:1303-1359).
fn module_path_scan_directory(
    fs_info: &CacheFSInfo,
    log: StdOption<&Rc<std::cell::RefCell<crate::util::log::Log>>>,
    path: &FsPath,
    path_index: usize,
    parent: &JavaString,
) -> Vec<Rc<ModuleLocation>> {
    let entries = match Files::list_directory(path) {
        Ok(entries) => entries,
        Err(_) => {
            module_path_error(
                log,
                crate::resources::compiler_properties::errors::locn_cant_read_directory(
                    crate::resources::diagnostic_contract::PathArg::placeholder(path.to_java_string()),
                ),
            );
            return Vec::new();
        }
    };
    let mut paths: Vec<FsPath> = Vec::new();
    let mut module_info_class: StdOption<FsPath> = None;
    for entry in entries {
        let name = entry
            .file_name()
            .map(|name| name.to_java_string())
            .unwrap_or_default();
        if name.ends_with("module-info.class") {
            module_info_class = Some(entry);
            break;
        }
        paths.push(entry);
    }
    if let Some(module_info_class) = module_info_class {
        // It's an exploded module directly on the module path.
        match module_path_read_module_name_file(&module_info_class) {
            Ok(module_name) => {
                let name = format!("{parent}[{path_index}:{module_name}]");
                let location = ModuleLocation::new(
                    parent,
                    JavaString::from(name.as_str()),
                    &module_name,
                    vec![path.clone()],
                    false,
                );
                // Locations.java:1279: the scanned locations are not explicit.
                location.set_explicit(false);
                return vec![location];
            }
            Err(error) => {
                module_path_error(
                    log,
                    module_name_reader_error_info(&error, path.to_java_string()),
                );
                return Vec::new();
            }
        }
    }
    // A directory of modules.
    let mut result: Vec<Rc<ModuleLocation>> = Vec::new();
    for (index, entry) in paths.iter().enumerate() {
        let Some((module_name, module_paths)) = module_path_infer_module_name(fs_info, log, entry)
        else {
            continue;
        };
        let name = format!("{parent}[{path_index}.{index}:{module_name}]");
        let location = ModuleLocation::new(
            parent,
            JavaString::from(name.as_str()),
            &module_name,
            module_paths,
            false,
        );
        // Locations.java:1279: the scanned locations are not explicit.
        location.set_explicit(false);
        result.push(location);
    }
    result
}

/// Java: ModulePathIterator.inferModuleName (Locations.java:1377-1496).
fn module_path_infer_module_name(
    fs_info: &CacheFSInfo,
    log: StdOption<&Rc<std::cell::RefCell<crate::util::log::Log>>>,
    p: &FsPath,
) -> StdOption<(String, Vec<FsPath>)> {
    if fs_info.is_directory(p) {
        let module_info_class = FsPath::from_path_buf(p.as_path().join("module-info.class"));
        let module_info_sig = FsPath::from_path_buf(p.as_path().join("module-info.sig"));
        if fs_info.exists(&module_info_class) || fs_info.exists(&module_info_sig) {
            let name = p
                .file_name()
                .map(|name| name.to_java_string())
                .unwrap_or_default();
            if is_module_name(&name) {
                return Some((name, vec![p.clone()]));
            }
        }
        return None;
    }
    let file_name = p
        .file_name()
        .map(|name| name.to_java_string())
        .unwrap_or_default();
    if file_name.ends_with(".jar") && fs_info.exists(p) {
        let archive = match javac_zip::ZipArchive::open(&p.to_path_buf()) {
            Ok(archive) => archive,
            Err(_) => {
                module_path_error(
                    log,
                    crate::resources::compiler_properties::errors::locn_cant_read_file(
                        crate::resources::diagnostic_contract::PathArg::placeholder(
                            p.to_java_string(),
                        ),
                    ),
                );
                return None;
            }
        };
        if let Some(bytes) = archive.get_entry("module-info.class") {
            match module_path_read_module_name_bytes(&bytes) {
                Ok(module_name) => {
                    if is_module_name(&module_name) {
                        return Some((module_name, vec![p.clone()]));
                    }
                    module_path_error(
                        log,
                        crate::resources::compiler_properties::errors::locn_bad_module_info(
                            crate::resources::diagnostic_contract::PathArg::placeholder(
                                p.to_java_string(),
                            ),
                        ),
                    );
                    return None;
                }
                Err(error) => {
                    module_path_error(
                        log,
                        module_name_reader_error_info(&error, p.to_java_string()),
                    );
                    return None;
                }
            }
        }
        if let Some(bytes) = archive.get_entry("META-INF/MANIFEST.MF") {
            if let Ok(attributes) = crate::file::fs_info::parse_manifest_main_attributes(&bytes) {
                if let Some(module_name) = attributes.get("automatic-module-name") {
                    if is_module_name(module_name) {
                        return Some((module_name.clone(), vec![p.clone()]));
                    }
                    module_path_error(
                        log,
                        crate::resources::compiler_properties::errors::locn_cant_get_module_name_for_jar(
                            crate::resources::diagnostic_contract::PathArg::placeholder(
                                p.to_java_string(),
                            ),
                        ),
                    );
                    return None;
                }
            }
        }
        // automatic module
        let module_name = automatic_module_name(&file_name);
        if !module_name.is_empty() {
            return Some((module_name, vec![p.clone()]));
        }
        module_path_error(
            log,
            crate::resources::compiler_properties::errors::locn_cant_get_module_name_for_jar(
                crate::resources::diagnostic_contract::PathArg::placeholder(p.to_java_string()),
            ),
        );
        return None;
    }
    if file_name.ends_with(".jmod") {
        let archive = match javac_zip::ZipArchive::open(&p.to_path_buf()) {
            Ok(archive) => archive,
            Err(_) => {
                module_path_error(
                    log,
                    crate::resources::compiler_properties::errors::locn_cant_read_file(
                        crate::resources::diagnostic_contract::PathArg::placeholder(
                            p.to_java_string(),
                        ),
                    ),
                );
                return None;
            }
        };
        let Some(bytes) = archive.get_entry("classes/module-info.class") else {
            return None;
        };
        match module_path_read_module_name_bytes(&bytes) {
            Ok(module_name) => {
                if is_module_name(&module_name) {
                    // PORTING_RULES: the upstream returns the jmod file system
                    // "classes" path; the ported file layer has no jmod
                    // FileSystem, so the module path is the jmod file itself
                    // (registered adapter; the container layer resolves entries).
                    return Some((module_name, vec![p.clone()]));
                }
                None
            }
            Err(error) => {
                module_path_error(
                    log,
                    module_name_reader_error_info(&error, p.to_java_string()),
                );
                None
            }
        }
    } else {
        None
    }
}

/// Java: ModulePathIterator.readModuleName(Path) (Locations.java:1498-1502).
fn module_path_read_module_name_file(
    path: &FsPath,
) -> Result<String, crate::jvm::module_name_reader::Error> {
    let mut reader = crate::jvm::module_name_reader::ModuleNameReader::new();
    let path = path.to_path_buf();
    reader
        .read_module_name(&path)
        .map(|name| name.to_string_lossy())
}

/// Java: ModuleNameReader.readModuleName(InputStream) over the zip entry bytes.
fn module_path_read_module_name_bytes(
    bytes: &[u8],
) -> Result<String, crate::jvm::module_name_reader::Error> {
    let mut reader = crate::jvm::module_name_reader::ModuleNameReader::new();
    let mut cursor = std::io::Cursor::new(bytes);
    reader
        .read_module_name_from_reader(&mut cursor)
        .map(|name| name.to_string_lossy())
}

fn module_path_error(
    log: StdOption<&Rc<std::cell::RefCell<crate::util::log::Log>>>,
    info: crate::resources::diagnostic_contract::ErrorInfo,
) {
    if let Some(log) = log {
        use crate::util::abstract_log::AbstractLog;
        log.borrow_mut()
            .error_pos(None, crate::util::jc_diagnostic::Error::from(info));
    }
}

/// Java: the ModuleNameReader error reporting of ModulePathIterator.
fn module_name_reader_error_info(
    error: &crate::jvm::module_name_reader::Error,
    path: String,
) -> crate::resources::diagnostic_contract::ErrorInfo {
    let path_arg = || {
        crate::resources::diagnostic_contract::PathArg::placeholder(path.clone())
    };
    match error {
        crate::jvm::module_name_reader::Error::BadClassFile(_) => {
            crate::resources::compiler_properties::errors::locn_bad_module_info(path_arg())
        }
        _ => crate::resources::compiler_properties::errors::locn_cant_read_file(path_arg()),
    }
}

/// Java: ModulePathLocationHandler.isModuleName(String) (Locations.java:1509-1520).
fn is_module_name(name: &str) -> bool {
    use javac_tools::lang::model::source_version::SourceVersion;
    for id in name.split('.') {
        if !SourceVersion::is_name(&javac_strings::JavaString::from(id)) {
            return false;
        }
    }
    true
}

/// Java: ModulePath.deriveModuleDescriptor automatic module name
/// (Locations.java:1426-1450).
fn automatic_module_name(file_name: &str) -> String {
    let mut mn = &file_name[..file_name.len() - 4];
    let bytes = mn.as_bytes();
    let mut found: StdOption<usize> = None;
    let mut index = 0usize;
    while index + 1 < bytes.len() {
        if bytes[index] == b'-' && bytes[index + 1].is_ascii_digit() {
            let mut end = index + 1;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end == bytes.len() || bytes[end] == b'.' {
                found = Some(index);
                break;
            }
        }
        index += 1;
    }
    if let Some(start) = found {
        mn = &mn[..start];
    }
    let mut result = String::new();
    let mut last_dot = false;
    for ch in mn.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch);
            last_dot = false;
        } else if !result.is_empty() && !last_dot {
            result.push('.');
            last_dot = true;
        }
    }
    while result.ends_with('.') {
        result.pop();
    }
    result
}

#[cfg(test)]


mod tests {
    use super::{LocationCtx, Locations};
    use crate::file::cache_fs_info::CacheFSInfo;
    use crate::util::context::Context;
    use crate::util::log::Log;
    use javac_fs::Path as FsPath;
    use javac_tools::tools::standard_location::StandardLocation;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "javac-locations-{}-{}-{}",
            tag,
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn fs_path(path: &std::path::Path) -> FsPath {
        FsPath::from_path_buf(path.to_path_buf())
    }

    fn java_home() -> FsPath {
        match std::env::var_os("JAVA_HOME") {
            Some(home) => FsPath::from_path_buf(PathBuf::from(home)),
            None => fs_path(&std::env::temp_dir()),
        }
    }

    fn new_locations() -> Locations {
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let mut locations = Locations::new(java_home());
        locations.update(log, false, Rc::new(CacheFSInfo::new()));
        locations
    }

    fn has_jdk_home() -> bool {
        match std::env::var_os("JAVA_HOME") {
            Some(home) => PathBuf::from(home).join("lib").join("modules").is_file(),
            None => false,
        }
    }

    #[test]
    fn class_path_defaults_to_the_current_directory() {
        let mut locations = new_locations();
        let paths = locations
            .get_location(&StandardLocation::CLASS_PATH)
            .expect("default class path");
        assert!(!paths.is_empty());
        assert!(locations.has_location(&StandardLocation::CLASS_PATH));
    }

    #[test]
    fn set_location_replaces_the_class_path() {
        let mut locations = new_locations();
        let dir = temp_dir("cp");
        locations
            .set_location(&StandardLocation::CLASS_PATH, Some(vec![fs_path(&dir)]))
            .expect("set class path");
        assert_eq!(
            locations.get_location(&StandardLocation::CLASS_PATH),
            Some(vec![fs_path(&dir)])
        );
        assert!(locations.has_explicit_location(&StandardLocation::CLASS_PATH));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn output_location_option_and_validation() {
        let mut locations = new_locations();
        let dir = temp_dir("out");
        assert!(locations.handle_option(
            crate::main::option::Option::D,
            Some(dir.to_str().expect("utf8"))
        ));
        assert!(locations.has_location(&StandardLocation::CLASS_OUTPUT));
        assert_eq!(
            locations.get_output_location(&StandardLocation::CLASS_OUTPUT),
            Some(fs_path(&dir))
        );
        let missing = dir.join("missing");
        assert!(
            locations
                .set_location(
                    &StandardLocation::CLASS_OUTPUT,
                    Some(vec![fs_path(&missing)])
                )
                .is_err()
        );
        locations
            .set_location(&StandardLocation::CLASS_OUTPUT, Some(vec![fs_path(&dir)]))
            .expect("set output");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn module_source_path_module_form() {
        let mut locations = new_locations();
        let dir = temp_dir("msp");
        let value = format!("m1={}", dir.to_str().expect("utf8"));
        assert!(locations.handle_option(
            crate::main::option::Option::MODULE_SOURCE_PATH,
            Some(value.as_str())
        ));
        assert!(locations.has_location(&StandardLocation::MODULE_SOURCE_PATH));
        let location = locations
            .get_location_for_module(&StandardLocation::MODULE_SOURCE_PATH, "m1")
            .expect("module source path")
            .expect("m1 location");
        assert_eq!(
            location.get_name().to_string_lossy(),
            "MODULE_SOURCE_PATH[m1]"
        );
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn patch_module_nul_form() {
        let mut locations = new_locations();
        let dir = temp_dir("patch");
        let value = format!("java.base={}", dir.to_str().expect("utf8"));
        assert!(locations.handle_option(
            crate::main::option::Option::PATCH_MODULE,
            Some(value.as_str())
        ));
        assert!(locations.has_location(&StandardLocation::PATCH_MODULE_PATH));
        let location = locations
            .get_location_for_module(&StandardLocation::PATCH_MODULE_PATH, "java.base")
            .expect("patch module")
            .expect("java.base location");
        assert_eq!(
            location.get_name().to_string_lossy(),
            "PATCH_MODULE_PATH[java.base]"
        );
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn module_path_bookkeeping_and_invalid_entries() {
        let mut locations = new_locations();
        let dir = temp_dir("mp");
        assert!(locations.handle_option(
            crate::main::option::Option::MODULE_PATH,
            Some(dir.to_str().expect("utf8"))
        ));
        assert_eq!(
            locations.get_location(&StandardLocation::MODULE_PATH),
            Some(vec![fs_path(&dir)])
        );
        let invalid = dir.join("notes.txt");
        std::fs::write(&invalid, b"x").expect("write");
        let value = invalid.to_str().expect("utf8").to_string();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut locations = new_locations();
            locations.handle_option(
                crate::main::option::Option::MODULE_PATH,
                Some(value.as_str()),
            )
        }));
        assert!(result.is_err());
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn system_modules_defaults_to_the_injected_home() {
        let mut locations = new_locations();
        let home = java_home();
        assert_eq!(
            locations.get_location(&StandardLocation::SYSTEM_MODULES),
            Some(vec![home.clone()])
        );
        assert!(locations.is_default_system_modules_path());
        if !has_jdk_home() {
            return;
        }
        let location = locations
            .get_location_for_module(&StandardLocation::SYSTEM_MODULES, "java.base")
            .expect("system modules")
            .expect("java.base system module");
        assert_eq!(
            location.get_name().to_string_lossy(),
            "SYSTEM_MODULES[java.base]"
        );
        assert!(
            locations
                .get_location(&StandardLocation::SYSTEM_MODULES)
                .is_some()
        );
    }

    #[test]
    fn boot_class_path_default_is_the_system_modules_image() {
        let mut locations = new_locations();
        assert!(locations.is_default_boot_class_path());
        let paths = locations
            .get_location(&StandardLocation::PLATFORM_CLASS_PATH)
            .expect("platform class path");
        if has_jdk_home() {
            assert!(!paths.is_empty());
            assert!(
                paths[0].as_path().ends_with("lib/modules")
                    || paths[0].as_path().ends_with("lib\\modules")
            );
        }
    }

    #[test]
    fn get_path_entries_splits_on_the_platform_separator() {
        let fs_info = CacheFSInfo::new();
        let home = java_home();
        let ctx = LocationCtx {
            fs_info: &fs_info,
            java_home: &home,
            log: None,
            path_factory: None,
        };
        let separator = if cfg!(windows) { ';' } else { ':' };
        let value = format!("a{separator}b");
        let entries = ctx.get_path_entries(&value, None, false);
        assert_eq!(entries.len(), 2);
        let default = FsPath::from_path_buf(PathBuf::from("."));
        let entries =
            ctx.get_path_entries(&format!("a{separator}{separator}b"), Some(&default), false);
        assert_eq!(entries.len(), 3);
    }

    fn module_info_bytes(module: &str) -> Option<Vec<u8>> {
        let home = java_home().to_path_buf();
        let class_path = javac_classpath::ClassPath::system(&home).ok()?;
        class_path.read_class_in_module(module, "module-info")
    }

    #[test]
    fn module_path_scan_reads_an_exploded_module() {
        let Some(bytes) = module_info_bytes("java.base") else {
            eprintln!("skipping: no baseline JDK 17 installation found");
            return;
        };
        let parent = temp_dir("mpscan");
        let module_dir = parent.join("java.base");
        std::fs::create_dir_all(&module_dir).expect("module dir");
        std::fs::write(module_dir.join("module-info.class"), &bytes).expect("write module-info");
        let mut locations = new_locations();
        assert!(locations.handle_option(
            crate::main::option::Option::MODULE_PATH,
            Some(parent.to_str().expect("utf8"))
        ));
        let location = locations
            .get_location_for_module(&StandardLocation::MODULE_PATH, "java.base")
            .expect("module path")
            .expect("java.base location");
        assert_eq!(
            location.get_name().to_string_lossy(),
            "MODULE_PATH[0.0:java.base]"
        );
        let sets = locations
            .list_locations_for_modules(&StandardLocation::MODULE_PATH)
            .expect("module path sets");
        assert_eq!(sets.len(), 1);
        assert!(sets[0]
            .iter()
            .any(|location| location.get_name().to_string_lossy() == "MODULE_PATH[0.0:java.base]"));
        std::fs::remove_dir_all(&parent).expect("cleanup");
    }

}

#[cfg(test)]
mod module_source_path_pattern_tests {
    use super::*;

    fn ctx() -> (CacheFSInfo, FsPath) {
        (CacheFSInfo::new(), FsPath::from_path_buf(std::env::temp_dir()))
    }

    #[test]
    fn expand_braces_matches_the_upstream_algorithm() {
        let (fs_info, home) = ctx();
        let ctx = LocationCtx {
            fs_info: &fs_info,
            log: None,
            path_factory: None,
            java_home: &home,
        };
        let mut results = Vec::new();
        ctx.expand_braces("a{b,c}d", &mut results);
        assert_eq!(results, vec!["abd".to_string(), "acd".to_string()]);
        let mut nested = Vec::new();
        ctx.expand_braces("x{a{b,c},d}y", &mut nested);
        assert_eq!(
            nested,
            vec!["xaby".to_string(), "xacy".to_string(), "xdy".to_string()]
        );
    }

}

