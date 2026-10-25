// Origin: com/sun/tools/javac/file/Locations.java @ 982d53c (jdk-17.0.19-ga)

// PORTING_RULES: the handler code keeps the upstream deeply nested `if let` structure
// (Java style); the collapsible-if/match suggestions are allowed rather than rewriting it.
#![allow(clippy::collapsible_if, clippy::collapsible_match)]

//! This class converts command line arguments, environment variables and system
//! properties (in File.pathSeparator-separated String form) into a boot class path,
//! user class path, and source path.
//!
//! <p><b>This is NOT part of any supported API.</b>
//!
//! PORTING_RULES (S3b batch):
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
//! * reported gaps (TODO, no substitute data):
//!   - jar Class-Path expansion (java.util.jar Manifest; FSInfo.getJarClassPath TODO(P2)):
//!     class path archives are accepted, their manifest Class-Path is not expanded;
//!   - Log.warning diagnostics for path problems (TODO(Log batch)): invalid/nonexistent
//!     elements are still skipped/added as upstream, only the warning is missing;
//!   - non .jar/.zip class path files are skipped without the upstream zip readability
//!     probe (java.nio FileSystem); TODO(S3c);
//!   - module path scanning (jar/jmod/exploded module names) needs Manifest and the
//!     javac-zip/javac-jmod adapters: getLocationForModule/listLocationsForModules for
//!     MODULE_PATH/UPGRADE_MODULE_PATH are TODO panics; the path bookkeeping is ported;
//!   - --module-source-path pattern forms (wildcard/brace expansion) need java.util.regex
//!     and the upstream expansion; only the module=path form is ported;
//!   - --patch-module accepts the NUL-separated module=path form only (upstream invalid
//!     argument reporting is TODO(Log batch));
//!   - BootClassPath options were trimmed in task-48, so the handler has no options and
//!     only the default platform class path (system modules) is ported;
//!   - close() is a no-op: no java.nio FileSystem is opened in this batch (jar/ct.sym file
//!     systems are S3c).
//! * TODO(S3c): JavacFileManager consumes this API.  Handler locations are looked up by
//!   name, equal to the upstream identity/equals contract for StandardLocation.

use std::cell::Cell;
use std::option::Option as StdOption;
use std::rc::Rc;

use javac_fs::{Files, Path as FsPath};
use javac_jimage::JImage;
use javac_strings::JavaString;
use javac_tools::error::Error;
use javac_tools::tools::java_file_manager::Location;
use javac_tools::tools::standard_location::StandardLocation;

use crate::file::cache_fs_info::CacheFSInfo;
use crate::main::option::Option as JavacOption;

/// Java: File.pathSeparator (platform dependent).
fn path_separator() -> char {
    if cfg!(windows) { ';' } else { ':' }
}

/// The shared context used by the handler helpers (Java: the outer Locations fields).
struct LocationCtx<'a> {
    fs_info: &'a CacheFSInfo,
    java_home: &'a FsPath,
}

impl<'a> LocationCtx<'a> {
    fn get_path(&self, first: &str) -> FsPath {
        match FsPath::new(first) {
            Ok(path) => path,
            Err(error) => panic!("IllegalArgumentException: {error:?}"),
        }
    }

    /// Java: Locations.getPathEntries(String, Path).
    fn get_path_entries(
        &self,
        search_path: &str,
        empty_path_default: StdOption<&FsPath>,
    ) -> Vec<FsPath> {
        let mut entries = Vec::new();
        for entry in search_path.split(path_separator()) {
            if entry.is_empty() {
                if let Some(default) = empty_path_default {
                    entries.push(default.clone());
                }
            } else {
                entries.push(self.get_path(entry));
            }
        }
        entries
    }

    /// Java: static Path normalize(Path p) (toRealPath, else toAbsolutePath.normalize()).
    fn normalize(&self, path: &FsPath) -> FsPath {
        self.fs_info.get_canonical_file(path)
    }

    /// Java: SearchPath.addFiles(String files) (plus the inherited warn behavior).
    fn add_files(&self, files: StdOption<&str>, empty_default: StdOption<&FsPath>) -> Vec<FsPath> {
        let mut result: Vec<FsPath> = Vec::new();
        let Some(files) = files else {
            return result;
        };
        for file in self.get_path_entries(files, empty_default) {
            self.add_file(&mut result, &file);
        }
        result
    }

    /// Java: SearchPath.addFile(Path file, boolean warn).
    fn add_file(&self, result: &mut Vec<FsPath>, file: &FsPath) {
        if result.contains(file) {
            return;
        }
        if !self.fs_info.exists(file) {
            // Java: warning(PathElementNotFound) — TODO(Log batch): no Log warning entry point.
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
        if self.fs_info.is_file(file) {
            let name = file
                .file_name()
                .map(|name| name.to_java_string())
                .unwrap_or_default();
            let is_jmod = name.ends_with(".jmod");
            let is_modules_image = name.ends_with("modules");
            if !is_jmod && !is_modules_image && !self.is_archive(file) {
                // Java: opens the file as a zip filesystem to validate it.  java.nio
                // FileSystem does not exist in the port: skip (TODO(S3c); the upstream
                // warning is a TODO(Log batch) diagnostic).
                return;
            }
        }
        result.push(file.clone());
        // Java: expandJarClassPaths -> addJarClassPath(file): java.util.jar Manifest is not
        // ported (FSInfo.getJarClassPath TODO(P2)), so the Class-Path attribute is not expanded.
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
            // Java: the enclosing-jar case needs a java.nio jar file system (TODO(S3c)).
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
    /// Java: private Log log; (not used by this batch; path warnings are TODO(Log batch)).
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

    /// Java: public void setMultiReleaseValue(String multiReleaseValue).
    pub fn set_multi_release_value(&mut self, value: &str) {
        self.multi_release_value = Some(JavaString::from(value));
    }

    /// Java: public void close() throws IOException — no file system is opened by this batch.
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
            _ => unreachable!("PLATFORM_CLASS_PATH handler"),
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
                Vec::new(),
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
                    // not available without a JVM (the launcher provides them, TODO(P6)); the
                    // upstream final fallback (the current directory) is used.
                    state.search_path = Some(ctx.add_files(Some("."), StdOption::None));
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
        };
        match &mut self.handlers[index].kind {
            HandlerKind::BootClassPath(_) => false,
            HandlerKind::ClassPath(state) | HandlerKind::Simple(state) => {
                state.search_path = value.map(|value| ctx.add_files(Some(value), StdOption::None));
                true
            }
            HandlerKind::Output(state) => {
                state.output_dir = value.map(|value| ctx.get_path(value));
                true
            }
            HandlerKind::ModulePath(state) => {
                if let Some(files) = value.map(|value| ctx.get_path_entries(value, StdOption::None))
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
                ctx.module_source_path_init(state, value.unwrap_or(""));
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

    /// Java: PatchModulesLocationHandler.handleOption.
    fn patch_modules_init(&self, state: &mut PatchModulesState, value: &str) {
        state.module_table.clear();
        let parent = JavaString::from("PATCH_MODULE_PATH");
        for entry in value.split('\0') {
            match entry.find('=') {
                Some(equal) if equal > 0 => {
                    let module_name = &entry[..equal];
                    let paths = self.add_files(Some(&entry[equal + 1..]), StdOption::None);
                    let name = format!("PATCH_MODULE_PATH[{module_name}]");
                    state.module_table.add(ModuleLocation::new(
                        &parent,
                        JavaString::from(name.as_str()),
                        module_name,
                        paths,
                        false,
                    ));
                }
                _ => unimplemented!(
                    "TODO(Log batch): PatchModulesLocationHandler invalid --patch-module argument reporting"
                ),
            }
        }
    }

    /// Java: ModuleSourcePathLocationHandler.init / initForModule (module=path form only).
    fn module_source_path_init(&self, state: &mut ModuleSourcePathState, value: &str) {
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
        if pattern.is_some() {
            // TODO(S3b): the --module-source-path pattern form (wildcard/brace expansion and
            // the root/dirs/*/suffix layout) needs the upstream expansion algorithm and
            // java.util.regex; not ported (reported).
            unimplemented!(
                "TODO(S3b): --module-source-path pattern form needs the upstream expansion algorithm and java.util.regex"
            );
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
        if let Some(module) = self.find_module_location(&location.get_name().to_string_lossy()) {
            return Some(module.search_path());
        }
        let index = self.handler_index(location)?;
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
                    ctx.module_source_path_add(&mut table, file, StdOption::None);
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
            HandlerKind::ModulePath(_) => {
                unimplemented!(
                    "TODO(S3c): ModulePathLocationHandler module scan needs java.util.jar Manifest and the zip/jmod containers"
                )
            }
            HandlerKind::ModuleSourcePath(state) => match &state.module_table {
                Some(table) => table.get(module_name),
                None => None,
            },
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

    /// Java: Location getLocationForModule(Location location, Path file).
    pub fn get_location_for_module_path(
        &mut self,
        location: &dyn Location,
        _file: &FsPath,
    ) -> Result<StdOption<Rc<dyn Location>>, Error> {
        // TODO(S3c): the by-path lookup (ModuleTable.get(Path) with parent walking) is used by
        // the file manager together with the module path scan.
        let _ = self.handler_index(location);
        Ok(None)
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
            HandlerKind::ModulePath(_) => {
                unimplemented!(
                    "TODO(S3c): ModulePathLocationHandler module scan needs java.util.jar Manifest and the zip/jmod containers"
                )
            }
            HandlerKind::ModuleSourcePath(state) => match &state.module_table {
                None => Vec::new(),
                Some(table) => vec![table.locations()],
            },
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
    ) {
        if !self.fs_info.is_directory(prefix) {
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
        };
        let separator = if cfg!(windows) { ';' } else { ':' };
        let value = format!("a{separator}b");
        let entries = ctx.get_path_entries(&value, None);
        assert_eq!(entries.len(), 2);
        let default = FsPath::from_path_buf(PathBuf::from("."));
        let entries = ctx.get_path_entries(&format!("a{separator}{separator}b"), Some(&default));
        assert_eq!(entries.len(), 3);
    }
}
