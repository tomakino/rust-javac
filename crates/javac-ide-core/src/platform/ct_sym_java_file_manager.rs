//! The platform file manager of JDKPlatformProvider.PlatformDescriptionImpl
//! (JDKPlatformProvider.java:154-359): the Java anonymous JavacFileManager
//! subclass and its private SigJavaFileObject.
//!
//!
//! * the Java anonymous subclass -> CtSymJavaFileManager, which forwards to the
//!   inner JavacFileManager and overrides the ct.sym-backed input paths
//!   (hasLocation/getJavaFileForInput/list/inferBinaryName);
//! * a file inside the ct.sym java.nio FileSystem -> CtSymFileObject, read from the
//!   registered javac-ctsym adapter (CtSym; the archive is opened once);
//! * SigJavaFileObject -> a forwarding file object with Kind.CLASS
//!   (ForwardingJavaFileObject<JavaFileObject>; Java subclassing -> explicit
//!   delegation, the javac-tools forwarding pattern).

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::rc::Rc;

use javac_ctsym::CtSym;
use javac_strings::JavaString;
use javac_tools::error::Error;
use javac_tools::java_io::{Reader, StringReader, Writer};
use javac_tools::jdk::ClassLoader;
use javac_tools::tools::file_object::FileObject;
use javac_tools::tools::java_file_manager::{
    FileObjectRef, JavaFileManager, JavaFileObjectRef, Location, LocationRef,
};
use javac_tools::tools::java_file_object::{JavaFileObject, Kind};
use javac_tools::uri::URI;

use crate::file::javac_file_manager::JavacFileManager;
use javac_tools::lang::model::element::modifier::Modifier;
use javac_tools::lang::model::element::nesting_kind::NestingKind;

/// The StandardLocation names this manager overrides (Java: the anonymous
/// subclass creates explicit locations for PLATFORM_CLASS_PATH/SYSTEM_MODULES).
const PLATFORM_CLASS_PATH: &str = "PLATFORM_CLASS_PATH";
const SYSTEM_MODULES: &str = "SYSTEM_MODULES";

/// What the manager knows about a file object it created (Java: the subclass
/// override can inspect the JavaFileObject identity; Rc<dyn JavaFileObject>
/// cannot be downcast, so the manager keeps the mapping — registered adapter).
struct KnownCtSymFile {
    /// The binary class name of the signature file.
    class_name: JavaString,
    /// The ct.sym entry name (section/module/class.sig).
    entry_name: String,
    /// The wrapped file object for a SigJavaFileObject, if this is one.
    delegate: Option<JavaFileObjectRef>,
}

/// The package-oriented location of one ct.sym module (Java: the locations
/// installed by `fm.setLocationForModule(SYSTEM_MODULES, module, paths)` in
/// JDKPlatformProvider.java:296-319).
///
/// The ct.sym archive is read through javac-ctsym, not through a java.nio
/// FileSystem, so a module has no path: the port identifies it by the
/// `SYSTEM_MODULES[module]` name and reads the .sig entries from the archive.
struct CtSymModuleLocation {
    /// The module name (the directory name inside the release section).
    module: JavaString,
}

impl Location for CtSymModuleLocation {
    fn get_name(&self) -> JavaString {
        JavaString::from(
            format!("SYSTEM_MODULES[{}]", self.module.to_string_lossy()).as_str(),
        )
    }

    fn is_output_location(&self) -> bool {
        false
    }

    fn is_module_oriented_location(&self) -> bool {
        // Java: the per-module locations are package-oriented.
        false
    }
}

/// Java: the anonymous JavacFileManager subclass in
/// PlatformDescriptionImpl.getFileManager() (JDKPlatformProvider.java:159-248).
///
/// It forwards every operation to the inner JavacFileManager and overrides the
/// paths that the platform description installs.
pub(crate) struct CtSymJavaFileManager {
    /// The inner JavacFileManager (Java: super).
    inner: Rc<JavacFileManager>,
    /// The ct.sym archive (Java: the ct.sym java.nio FileSystem).
    archive: Rc<CtSym>,
    /// The source release (Java: the ctSymVersion section; the local adapter keys
    /// by the numeric release).
    release: i32,
    /// Java: Feature.MODULES.allowedInSource(Source.lookup(sourceVersion)).
    has_modules: bool,
    /// The file objects created by this manager.
    known: RefCell<HashMap<usize, KnownCtSymFile>>,
}

impl CtSymJavaFileManager {
    /// Creates the platform file manager view over the given state.
    ///
    /// # Arguments
    ///   - inner: the wired JavacFileManager;
    ///   - archive: the opened ct.sym archive;
    ///   - release: the numeric source release;
    ///   - has_modules: whether the source level supports modules.
    ///
    /// # Returns
    ///   - the shared platform file manager.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: JDKPlatformProvider.java:154-248.
    pub(crate) fn new(
        inner: Rc<JavacFileManager>,
        archive: Rc<CtSym>,
        release: i32,
        has_modules: bool,
    ) -> Rc<CtSymJavaFileManager> {
        Rc::new(CtSymJavaFileManager {
            inner,
            archive,
            release,
            has_modules,
            known: RefCell::new(HashMap::new()),
        })
    }

    /// The location name of a Location (the ported locations are identified by
    /// name; Java compares the Location constants).
    fn location_name(location: &dyn Location) -> String {
        location.get_name().to_string_lossy()
    }

    /// Reads the signature file of the given class from ct.sym (Java: the .sig
    /// then .class getFileForInput sequence, JDKPlatformProvider.java:168-187).
    fn ct_sym_class(&self, class_name: &JavaString) -> Option<JavaFileObjectRef> {
        if self.has_modules {
            return None;
        }
        let name = class_name.to_string_lossy();
        for module in self.archive.modules(self.release) {
            if self
                .archive
                .get_sig_in_module(self.release, module, &name)
                .is_some()
            {
                let entry = self.archive.find_sig(self.release, &name)?;
                return self.make_sig_file(
                    module.to_string(),
                    class_name.clone(),
                    entry.entry_name(),
                );
            }
        }
        None
    }

    /// Lists the signature files of a package (Java: the overridden list method,
    /// JDKPlatformProvider.java:192-238).
    fn list_sig_files(&self, package_name: &JavaString, recurse: bool) -> Vec<JavaFileObjectRef> {
        let package = package_name.to_string_lossy();
        let mut result = Vec::new();
        for class_name in self.archive.classes(self.release) {
            let class_package = match class_name.rfind('.') {
                Some(index) => &class_name[..index],
                None => "",
            };
            let included = if package.is_empty() {
                recurse || class_package.is_empty()
            } else if recurse {
                class_package == package || class_package.starts_with(&format!("{package}."))
            } else {
                class_package == package
            };
            if !included {
                continue;
            }
            if let Some(file) = self.ct_sym_class(&JavaString::from(class_name.as_str())) {
                result.push(file);
            }
        }
        result
    }

    /// Whether the release is described by the running JDK image (the
    /// `system-modules` list) instead of ct.sym signatures
    /// (JDKPlatformProvider.java:284-295).
    fn image_release(&self) -> bool {
        self.has_modules && self.archive.has_system_modules(self.release)
    }

    /// The module name of a per-module location created by this manager.
    fn module_of_location(location: &dyn Location) -> Option<JavaString> {
        let name = location.get_name().to_string_lossy();
        name.strip_prefix("SYSTEM_MODULES[")
            .and_then(|rest| rest.strip_suffix(']'))
            .map(JavaString::from)
    }

    /// Reads one signature of a specific module (Java: getFileForInput on the
    /// package-oriented location installed for that module,
    /// JDKPlatformProvider.java:168-187).
    fn ct_sym_class_in_module(
        &self,
        module: &str,
        class_name: &JavaString,
    ) -> Option<JavaFileObjectRef> {
        let name = class_name.to_string_lossy();
        let entry = self
            .archive
            .sig_entries(self.release)
            .into_iter()
            .find(|entry| entry.module() == module && entry.class_name() == name)?;
        self.make_sig_file(module.to_string(), class_name.clone(), entry.entry_name())
    }

    /// Wraps one .sig entry into a SigJavaFileObject and records it.
    fn make_sig_file(
        &self,
        module: String,
        class_name: JavaString,
        entry_name: String,
    ) -> Option<JavaFileObjectRef> {
        let underlying: JavaFileObjectRef = Rc::new(CtSymFileObject {
            archive: Rc::clone(&self.archive),
            release: self.release,
            module,
            class_name: class_name.clone(),
            entry_name: entry_name.clone(),
        });
        self.known.borrow_mut().insert(
            Rc::as_ptr(&underlying) as *const () as usize,
            KnownCtSymFile {
                class_name: class_name.clone(),
                entry_name: entry_name.clone(),
                delegate: None,
            },
        );
        let sig: JavaFileObjectRef = Rc::new(SigJavaFileObject {
            delegate: Rc::clone(&underlying),
        });
        self.known.borrow_mut().insert(
            Rc::as_ptr(&sig) as *const () as usize,
            KnownCtSymFile {
                class_name,
                entry_name,
                delegate: Some(underlying),
            },
        );
        Some(sig)
    }

    /// Lists the signature files of one module's package
    /// (Java: list on the per-module location, JDKPlatformProvider.java:192-238).
    fn list_module_sig_files(
        &self,
        module: &JavaString,
        package_name: &JavaString,
        recurse: bool,
    ) -> Vec<JavaFileObjectRef> {
        let package = package_name.to_string_lossy();
        let module = module.to_string_lossy();
        let mut result = Vec::new();
        for entry in self.archive.sig_entries(self.release) {
            if entry.module() != module {
                continue;
            }
            let class_name = entry.class_name();
            let class_package = match class_name.rfind('.') {
                Some(index) => &class_name[..index],
                None => "",
            };
            let included = if package.is_empty() {
                recurse || class_package.is_empty()
            } else if recurse {
                class_package == package || class_package.starts_with(&format!("{package}."))
            } else {
                class_package == package
            };
            if !included {
                continue;
            }
            if let Some(file) = self.ct_sym_class_in_module(&module, &JavaString::from(class_name))
            {
                result.push(file);
            }
        }
        result
    }

    /// The module of a recorded file object (from the ct.sym entry name
    /// `section/module/path.sig`).
    fn known_module_for_ptr(&self, ptr: usize) -> Option<String> {
        self.known
            .borrow()
            .get(&ptr)
            .and_then(|known| known.entry_name.split('/').nth(1).map(str::to_string))
    }

    /// The class name recorded for a created file object.
    fn known_class_name(&self, file: &JavaFileObjectRef) -> Option<JavaString> {
        self.known
            .borrow()
            .get(&(Rc::as_ptr(file) as *const () as usize))
            .map(|known| known.class_name.clone())
    }

    /// The ct.sym entry name of a file object created by this manager (either a
    /// SigJavaFileObject or its underlying ct.sym file object).
    fn known_entry_for_ptr(&self, ptr: usize) -> Option<String> {
        self.known
            .borrow()
            .get(&ptr)
            .map(|known| known.entry_name.clone())
    }
}

impl javac_tools::tools::option_checker::OptionChecker for CtSymJavaFileManager {
    fn is_supported_option(&self, option: &JavaString) -> i32 {
        self.inner.is_supported_option(option)
    }
}

impl JavaFileManager for CtSymJavaFileManager {
    fn get_class_loader(&self, location: &dyn Location) -> Option<Rc<dyn ClassLoader>> {
        self.inner.get_class_loader(location)
    }

    fn list(
        &self,
        location: &dyn Location,
        package_name: &JavaString,
        kinds: &[Kind],
        recurse: bool,
    ) -> Result<Vec<JavaFileObjectRef>, Error> {
        if Self::location_name(location) == PLATFORM_CLASS_PATH && !self.has_modules {
            return Ok(self.list_sig_files(package_name, recurse));
        }
        if Self::module_of_location(location).is_some() {
            if self.image_release() {
                // JDKPlatformProvider.java:284-295: a release with a
                // system-modules list resolves its modules through the running
                // jrt image; the location came from the inner manager.
                return self.inner.list(location, package_name, kinds, recurse);
            }
            let Some(module) = Self::module_of_location(location) else {
                unreachable!("checked above")
            };
            // The per-module locations installed by getFileManager list the .sig
            // entries of that module (JDKPlatformProvider.java:296-319).  The
            // platform manager reports them as CLASS file objects (SigJavaFileObject),
            // so a request that does not ask for classes lists nothing.
            if !kinds.iter().any(|kind| *kind == Kind::CLASS) {
                return Ok(Vec::new());
            }
            return Ok(self.list_module_sig_files(&module, package_name, recurse));
        }
        self.inner.list(location, package_name, kinds, recurse)
    }

    fn infer_binary_name(
        &self,
        location: &dyn Location,
        file: &JavaFileObjectRef,
    ) -> Option<JavaString> {
        if self
            .known
            .borrow()
            .contains_key(&(Rc::as_ptr(file) as *const () as usize))
        {
            return self.known_class_name(file);
        }
        self.inner.infer_binary_name(location, file)
    }

    fn is_same_file(&self, a: &FileObjectRef, b: &FileObjectRef) -> bool {
        // Java: super.isSameFile compares the path file objects; the ct.sym file
        // objects of this manager are identified by their entry name (adapter).
        let entry_a = self.known_entry_for_ptr(Rc::as_ptr(a) as *const () as usize);
        let entry_b = self.known_entry_for_ptr(Rc::as_ptr(b) as *const () as usize);
        if let (Some(entry_a), Some(entry_b)) = (entry_a, entry_b) {
            return entry_a == entry_b;
        }
        self.inner.is_same_file(a, b)
    }

    fn handle_option(
        &self,
        current: &JavaString,
        remaining: &mut dyn Iterator<Item = JavaString>,
    ) -> bool {
        self.inner.handle_option(current, remaining)
    }

    fn has_location(&self, location: &dyn Location) -> bool {
        match Self::location_name(location).as_str() {
            PLATFORM_CLASS_PATH => !self.has_modules,
            SYSTEM_MODULES => self.has_modules,
            _ => {
                // The package-oriented locations of the managed modules: the
                // ct.sym per-module locations before the image list, and the
                // inner manager's jrt module locations for a modules release
                // described by the running JDK image.  Both are named
                // `SYSTEM_MODULES[<module>]` (Locations.ModuleLocation).
                self.has_modules && Self::module_of_location(location).is_some()
            }
        }
    }

    fn get_java_file_for_input(
        &self,
        location: &dyn Location,
        class_name: &JavaString,
        kind: Kind,
    ) -> Result<Option<JavaFileObjectRef>, Error> {
        if Self::location_name(location) == PLATFORM_CLASS_PATH {
            if self.has_modules || kind != Kind::CLASS {
                return Ok(None);
            }
            return Ok(self.ct_sym_class(class_name));
        }
        if let Some(module) = Self::module_of_location(location) {
            if self.image_release() {
                return self
                    .inner
                    .get_java_file_for_input(location, class_name, kind);
            }
            if kind != Kind::CLASS {
                return Ok(None);
            }
            return Ok(self.ct_sym_class_in_module(&module.to_string_lossy(), class_name));
        }
        self.inner
            .get_java_file_for_input(location, class_name, kind)
    }

    fn get_java_file_for_output(
        &self,
        location: &dyn Location,
        class_name: &JavaString,
        kind: Kind,
        sibling: Option<&FileObjectRef>,
    ) -> Result<JavaFileObjectRef, Error> {
        self.inner
            .get_java_file_for_output(location, class_name, kind, sibling)
    }

    fn get_file_for_input(
        &self,
        location: &dyn Location,
        package_name: &JavaString,
        relative_name: &JavaString,
    ) -> Result<Option<FileObjectRef>, Error> {
        if let Some(module) = Self::module_of_location(location) {
            if self.image_release() {
                return self
                    .inner
                    .get_file_for_input(location, package_name, relative_name);
            }
            let relative = relative_name.to_string_lossy();
            let Some(stem) = relative
                .strip_suffix(".sig")
                .or_else(|| relative.strip_suffix(".class"))
            else {
                return Ok(None);
            };
            let class_name = if package_name.is_empty() {
                stem.replace('/', ".")
            } else {
                format!(
                    "{}.{}",
                    package_name.to_string_lossy(),
                    stem.replace('/', ".")
                )
            };
            return Ok(self
                .ct_sym_class_in_module(
                    &module.to_string_lossy(),
                    &JavaString::from(class_name.as_str()),
                )
                .map(|file| file as FileObjectRef));
        }
        if Self::location_name(location) == PLATFORM_CLASS_PATH && !self.has_modules {
            let relative = relative_name.to_string_lossy();
            let suffix = match relative.strip_suffix(".sig") {
                Some(stem) => Some((stem, Kind::CLASS)),
                None => relative
                    .strip_suffix(".class")
                    .map(|stem| (stem, Kind::CLASS)),
            };
            if let Some((stem, _kind)) = suffix {
                let class_name = if package_name.is_empty() {
                    stem.replace('/', ".")
                } else {
                    format!(
                        "{}.{}",
                        package_name.to_string_lossy(),
                        stem.replace('/', ".")
                    )
                };
                return Ok(self
                    .ct_sym_class(&JavaString::from(class_name.as_str()))
                    .map(|file| file as FileObjectRef));
            }
            return Ok(None);
        }
        self.inner
            .get_file_for_input(location, package_name, relative_name)
    }

    fn get_file_for_output(
        &self,
        location: &dyn Location,
        package_name: &JavaString,
        relative_name: &JavaString,
        sibling: Option<&FileObjectRef>,
    ) -> Result<FileObjectRef, Error> {
        self.inner
            .get_file_for_output(location, package_name, relative_name, sibling)
    }

    fn flush(&self) -> Result<(), Error> {
        self.inner.flush()
    }

    fn close(&self) -> Result<(), Error> {
        self.inner.close()
    }

    fn get_location_for_module(
        &self,
        location: &dyn Location,
        module_name: &JavaString,
    ) -> Result<LocationRef, Error> {
        // Java: PlatformDescriptionImpl installs one location per ct.sym module
        // when the release has no system-modules image list
        // (JDKPlatformProvider.java:296-319); a modules release described by the
        // running image forwards to the inner manager's jrt modules.
        if Self::location_name(location) == SYSTEM_MODULES && self.has_modules && !self.image_release()
        {
            let module = module_name.to_string_lossy();
            if self
                .archive
                .modules(self.release)
                .iter()
                .any(|name| *name == module)
            {
                return Ok(
                    Rc::new(CtSymModuleLocation {
                        module: module_name.clone(),
                    }) as LocationRef,
                );
            }
            return Err(Error::io(JavaString::from(
                format!("module not found: {module}").as_str(),
            )));
        }
        self.inner.get_location_for_module(location, module_name)
    }

    fn get_location_for_module_of_file(
        &self,
        location: &dyn Location,
        fo: &JavaFileObjectRef,
    ) -> Result<LocationRef, Error> {
        if Self::location_name(location) == SYSTEM_MODULES && self.has_modules {
            if let Some(module) = self.known_module_for_ptr(Rc::as_ptr(fo) as *const () as usize) {
                return Ok(
                    Rc::new(CtSymModuleLocation {
                        module: JavaString::from(module.as_str()),
                    }) as LocationRef,
                );
            }
        }
        self.inner.get_location_for_module_of_file(location, fo)
    }

    fn infer_module_name(&self, location: &dyn Location) -> Result<JavaString, Error> {
        if let Some(module) = Self::module_of_location(location) {
            return Ok(module);
        }
        self.inner.infer_module_name(location)
    }

    fn list_locations_for_modules(
        &self,
        location: &dyn Location,
    ) -> Result<Vec<Vec<LocationRef>>, Error> {
        if Self::location_name(location) == SYSTEM_MODULES && self.has_modules && !self.image_release()
        {
            let mut result = Vec::new();
            for module in self.archive.modules(self.release) {
                result.push(vec![
                    Rc::new(CtSymModuleLocation {
                        module: JavaString::from(module),
                    }) as LocationRef,
                ]);
            }
            return Ok(result);
        }
        self.inner.list_locations_for_modules(location)
    }

    fn contains(&self, location: &dyn Location, fo: &FileObjectRef) -> Result<bool, Error> {
        if let Some(module) = Self::module_of_location(location) {
            if self.image_release() {
                return self.inner.contains(location, fo);
            }
            let ptr = Rc::as_ptr(fo) as *const () as usize;
            return Ok(self
                .known_module_for_ptr(ptr)
                .is_some_and(|known| known == module.to_string_lossy()));
        }
        self.inner.contains(location, fo)
    }
}

/// Java: JDKPlatformProvider.PlatformDescriptionImpl.SigJavaFileObject extends
/// ForwardingJavaFileObject<JavaFileObject> (JDKPlatformProvider.java:340-359).
///
/// The forwarding is explicit (Rust has no subclassing): every operation delegates
/// to the wrapped file object; getKind reports CLASS and isNameCompatible appends
/// the ".sig" suffix (the upstream overrides).
struct SigJavaFileObject {
    /// Java: ForwardingJavaFileObject.fileObject.
    delegate: JavaFileObjectRef,
}

impl SigJavaFileObject {
    /// Java: public JavaFileObject getDelegate().
    fn get_delegate(&self) -> JavaFileObjectRef {
        Rc::clone(&self.delegate)
    }
}

impl FileObject for SigJavaFileObject {
    fn to_uri(&self) -> URI {
        self.delegate.to_uri()
    }

    fn get_name(&self) -> JavaString {
        self.delegate.get_name()
    }

    fn open_input_stream(&self) -> Result<Box<dyn Read>, Error> {
        self.delegate.open_input_stream()
    }

    fn open_output_stream(&self) -> Result<Box<dyn Write>, Error> {
        self.delegate.open_output_stream()
    }

    fn open_reader(&self, ignore_encoding_errors: bool) -> Result<Box<dyn Reader>, Error> {
        self.delegate.open_reader(ignore_encoding_errors)
    }

    fn get_char_content(&self, ignore_encoding_errors: bool) -> Result<Option<JavaString>, Error> {
        self.delegate.get_char_content(ignore_encoding_errors)
    }

    fn open_writer(&self) -> Result<Box<dyn Writer>, Error> {
        self.delegate.open_writer()
    }

    fn get_last_modified(&self) -> i64 {
        self.delegate.get_last_modified()
    }

    fn delete(&self) -> bool {
        self.delegate.delete()
    }

    fn is_in_jrt(&self) -> bool {
        self.delegate.is_in_jrt()
    }
}

impl JavaFileObject for SigJavaFileObject {
    fn get_kind(&self) -> Kind {
        Kind::CLASS
    }

    fn is_name_compatible(&self, simple_name: &JavaString, _kind: Kind) -> bool {
        // Java: super.isNameCompatible(simpleName + ".sig", Kind.OTHER)
        // (JDKPlatformProvider.java:351-354).
        let simple = JavaString::from(format!("{}.sig", simple_name.to_string_lossy()).as_str());
        self.delegate.is_name_compatible(&simple, Kind::OTHER)
    }

    fn get_nesting_kind(&self) -> Option<NestingKind> {
        self.delegate.get_nesting_kind()
    }

    fn get_access_level(&self) -> Option<Modifier> {
        self.delegate.get_access_level()
    }
}

/// A read-only FileObject over a signature entry of the ct.sym archive (Java: the
/// PathFileObject seen through the ct.sym java.nio FileSystem; the ported adapter
/// reads the entry bytes from javac-ctsym).
struct CtSymFileObject {
    /// The ct.sym archive.
    archive: Rc<CtSym>,
    /// The release (section) that provides the entry.
    release: i32,
    /// The module directory of the entry.
    module: String,
    /// The binary class name.
    class_name: JavaString,
    /// The entry name (section/module/class.sig).
    entry_name: String,
}

impl CtSymFileObject {
    /// The signature bytes of the entry (Java: Files.newInputStream of the entry).
    fn read_signature(&self) -> Result<Vec<u8>, Error> {
        self.archive
            .get_sig_in_module(
                self.release,
                &self.module,
                &self.class_name.to_string_lossy(),
            )
            .ok_or_else(|| {
                Error::io(JavaString::from(
                    format!("ct.sym entry not found: {}", self.entry_name).as_str(),
                ))
            })
    }
}

impl FileObject for CtSymFileObject {
    fn to_uri(&self) -> URI {
        // Java: the ct.sym FileSystem path URI (jar:file:<ct.sym>!/<entry>).
        URI::create(&JavaString::from(
            format!(
                "jar:file:{}!/{}",
                self.archive.path().display(),
                self.entry_name
            )
            .as_str(),
        ))
    }

    fn get_name(&self) -> JavaString {
        JavaString::from(self.entry_name.as_str())
    }

    fn open_input_stream(&self) -> Result<Box<dyn Read>, Error> {
        Ok(Box::new(Cursor::new(self.read_signature()?)))
    }

    fn open_output_stream(&self) -> Result<Box<dyn Write>, Error> {
        Err(Error::io(JavaString::from(
            "ReadOnlyFileSystemException: ct.sym",
        )))
    }

    fn open_reader(&self, _ignore_encoding_errors: bool) -> Result<Box<dyn Reader>, Error> {
        // Java: the PathFileObject reader decodes the entry with the file manager
        // decoder; the signature text is not consumed by the compiler, the ported
        // adapter exposes the UTF-8 view (registered adapter).
        let bytes = self.read_signature()?;
        Ok(Box::new(StringReader::new(JavaString::from(
            String::from_utf8_lossy(&bytes).as_ref(),
        ))))
    }

    fn get_char_content(&self, _ignore_encoding_errors: bool) -> Result<Option<JavaString>, Error> {
        let bytes = self.read_signature()?;
        Ok(Some(JavaString::from(
            String::from_utf8_lossy(&bytes).as_ref(),
        )))
    }

    fn open_writer(&self) -> Result<Box<dyn Writer>, Error> {
        Err(Error::io(JavaString::from(
            "ReadOnlyFileSystemException: ct.sym",
        )))
    }

    fn get_last_modified(&self) -> i64 {
        // Java: Files.getLastModifiedTime on the ct.sym FileSystem; javac-ctsym
        // does not expose entry timestamps (registered adapter: 0, the unknown
        // value allowed by the FileObject contract).
        0
    }

    fn delete(&self) -> bool {
        false
    }

    fn is_in_jrt(&self) -> bool {
        false
    }
}

impl JavaFileObject for CtSymFileObject {
    fn get_kind(&self) -> Kind {
        // Java: the .sig file object's PathFileObject kind is OTHER.
        Kind::OTHER
    }

    fn is_name_compatible(&self, simple_name: &JavaString, kind: Kind) -> bool {
        let base = format!(
            "{}{}",
            simple_name.to_string_lossy(),
            kind.extension().to_string_lossy()
        );
        let file_name = self
            .entry_name
            .rsplit('/')
            .next()
            .unwrap_or(self.entry_name.as_str());
        file_name == base
    }

    fn get_nesting_kind(&self) -> Option<NestingKind> {
        None
    }

    fn get_access_level(&self) -> Option<Modifier> {
        None
    }
}
