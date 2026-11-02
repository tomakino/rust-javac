// PORTING ADAPTER (not a 1:1 port)
//
// Adapter: a unified "system class library byte source" over the JDK 17
// containers.  Why not 1:1: javac reads the system class library through
// com.sun.tools.javac.file.ClassPath, which is built on jrt-fs and the
// javac file manager (both outside this crate's scope); this crate provides the
// equivalent capability for the P2 file/ClassReader layer: locate a JDK 17
// installation and read class bytes from lib/modules (jimage) or jmods, with
// the same container readers the other crates already ported.
//
// Reference behavior: javac's ClassPath/system image handling
// (com.sun.tools.javac.file.ClassPath, JmodFile, ImageReader): the system
// image lives in lib/modules, falls back to the jmods directory, and reports an
// error when neither is present.
//
//! System class library byte source for a JDK 17 installation.
//!
//! The path search order follows javac: `<jdk_home>/lib/modules` (jimage) is
//! preferred; when there is no jimage, the `<jdk_home>/jmods` directory is used;
//! otherwise construction fails with [`Error::NoSystemLibraries`].
//!
//!
//! * this is a new adapter crate, not a 1:1 Java file; the class bytes read are
//!   exactly the container bytes (the jimage/jmod readers are the ported ones).
//! * caches: repeated reads are O(1) lookups ([HashMap]);
//!   only class files that were actually read are held (no whole-image or
//!   whole-file buffer), and missing classes are cached as None.
//! * `ClassPath` is single-threaded (it uses RefCell for the lazy module state
//!   and the cache); create one per compilation task.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use javac_jimage::JImage;
use javac_jmod::JMod;

/// Errors of the system class path layer.
#[derive(Debug)]
pub enum Error {
    /// The JDK home has neither lib/modules nor a non-empty jmods directory.
    NoSystemLibraries { home: PathBuf },
    /// The lib/modules jimage could not be opened.
    Image(javac_jimage::Error),
    /// A jmods entry could not be opened.
    Jmod(javac_jmod::Error),
    /// An I/O error while scanning the JDK home.
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoSystemLibraries { home } => write!(
                f,
                "no system class library found in {} (expected lib/modules or jmods)",
                home.display()
            ),
            Error::Image(err) => write!(f, "invalid lib/modules jimage: {}", err),
            Error::Jmod(err) => write!(f, "invalid jmods entry: {}", err),
            Error::Io(err) => write!(f, "I/O error: {}", err),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Image(err) => Some(err),
            Error::Jmod(err) => Some(err),
            Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<javac_jimage::Error> for Error {
    fn from(err: javac_jimage::Error) -> Error {
        Error::Image(err)
    }
}

impl From<javac_jmod::Error> for Error {
    fn from(err: javac_jmod::Error) -> Error {
        Error::Jmod(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

/// The container the system class library was found in.
enum Source {
    /// lib/modules jimage.
    Image(JImage),
    /// jmods/*.jmod, lazily mutable because reading class bytes advances the
    /// per-archive state of the ported jmod reader.
    JMods(RefCell<Vec<JMod>>),
}

/// Key of the class-byte cache: (module, normalized binary name).
type ClassCacheKey = (String, String);

/// Cached class bytes (None caches a missing class).
type ClassCache = HashMap<ClassCacheKey, Option<Rc<Vec<u8>>>>;

/// A system class library byte source for one JDK installation.
///
/// single-threaded (RefCell for the jmod state and the caches).
pub struct ClassPath {
    home: PathBuf,
    source: Source,
    modules: Vec<String>,
    /// Class bytes already read: (module, normalized binary name) -> bytes.
    cache: RefCell<ClassCache>,
    /// Module resolution cache: normalized binary name -> module (or None).
    module_cache: RefCell<HashMap<String, Option<String>>>,
}

impl fmt::Debug for ClassPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClassPath")
            .field("home", &self.home)
            .field("modules", &self.modules.len())
            .field(
                "source",
                &match self.source {
                    Source::Image(_) => "lib/modules",
                    Source::JMods(_) => "jmods",
                },
            )
            .finish()
    }
}

/// Normalizes a class name to the slash-separated form without a ".class"
/// suffix.  Dot-separated binary names and slash-separated resource names are
/// both accepted (javac accepts both when converting names to paths).
fn normalize_class_name(binary_name: &str) -> String {
    let name = binary_name.strip_suffix(".class").unwrap_or(binary_name);
    if name.contains('/') {
        name.to_string()
    } else {
        name.replace('.', "/")
    }
}

impl ClassPath {
    /// Opens the system class library of the given JDK home.
    ///
    /// Search order (javac's system image handling): `lib/modules` (jimage),
    /// then the `jmods` directory; otherwise [`Error::NoSystemLibraries`].
    pub fn system(jdk_home: &Path) -> Result<ClassPath, Error> {
        let image_path = jdk_home.join("lib").join("modules");
        if image_path.is_file() {
            let image = JImage::open(&image_path)?;
            let modules = image.modules().to_vec();
            return Ok(ClassPath {
                home: jdk_home.to_path_buf(),
                source: Source::Image(image),
                modules,
                cache: RefCell::new(HashMap::new()),
                module_cache: RefCell::new(HashMap::new()),
            });
        }

        let jmods_dir = jdk_home.join("jmods");
        if jmods_dir.is_dir() {
            let mut files: Vec<PathBuf> = Vec::new();
            for entry in std::fs::read_dir(&jmods_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) == Some("jmod") {
                    files.push(path);
                }
            }
            files.sort();
            if !files.is_empty() {
                let mut jmods = Vec::with_capacity(files.len());
                let mut modules = Vec::with_capacity(files.len());
                for path in &files {
                    let jmod = JMod::open(path)?;
                    modules.push(jmod.module_name().to_string());
                    jmods.push(jmod);
                }
                return Ok(ClassPath {
                    home: jdk_home.to_path_buf(),
                    source: Source::JMods(RefCell::new(jmods)),
                    modules,
                    cache: RefCell::new(HashMap::new()),
                    module_cache: RefCell::new(HashMap::new()),
                });
            }
        }

        Err(Error::NoSystemLibraries {
            home: jdk_home.to_path_buf(),
        })
    }

    /// The JDK home this class path was created from.
    pub fn jdk_home(&self) -> &Path {
        &self.home
    }

    /// The module names of the system class library, in container order.
    pub fn list_modules(&self) -> &[String] {
        &self.modules
    }

    /// Reads the class file bytes of a binary name from any system module.
    ///
    /// Accepts dot-separated (`java.lang.Object`) and slash-separated
    /// (`java/lang/Object`) names, with or without a trailing `.class`.
    pub fn read_class(&self, binary_name: &str) -> Option<Vec<u8>> {
        let name = normalize_class_name(binary_name);
        let module = self.module_of_class(&name)?;
        self.read_class_in_module_cached(&module, &name)
    }

    /// Reads the class file bytes of a binary name from the given module.
    pub fn read_class_in_module(&self, module: &str, binary_name: &str) -> Option<Vec<u8>> {
        let name = normalize_class_name(binary_name);
        self.read_class_in_module_cached(module, &name)
    }

    /// Returns the module containing the given class, if any.
    ///
    /// The result is cached, so repeated queries are O(1).
    pub fn module_of_class(&self, binary_name: &str) -> Option<String> {
        let name = normalize_class_name(binary_name);
        if let Some(cached) = self.module_cache.borrow().get(&name) {
            return cached.clone();
        }
        let module = match &self.source {
            Source::Image(image) => {
                let mut found = None;
                for module in &self.modules {
                    if image.get_class_bytes(module, &name).is_some() {
                        found = Some(module.clone());
                        break;
                    }
                }
                found
            }
            Source::JMods(jmods) => {
                let jmods = jmods.borrow();
                let mut found = None;
                for jmod in jmods.iter() {
                    // The ported jmod reader exposes its class entries; a cheap
                    // membership check avoids reading bytes during resolution.
                    let entry = format!("classes/{}.class", name);
                    if jmod.list_classes().contains(&entry) {
                        found = Some(jmod.module_name().to_string());
                        break;
                    }
                }
                found
            }
        };
        self.module_cache.borrow_mut().insert(name, module.clone());
        module
    }

    /// Reads (and caches) the bytes of one class in one module.
    fn read_class_in_module_cached(&self, module: &str, name: &str) -> Option<Vec<u8>> {
        let key = (module.to_string(), name.to_string());
        if let Some(cached) = self.cache.borrow().get(&key) {
            return cached.as_ref().map(|bytes| bytes.as_ref().clone());
        }
        let loaded = match &self.source {
            Source::Image(image) => image.get_class_bytes(module, name),
            Source::JMods(jmods) => {
                let mut jmods = jmods.borrow_mut();
                jmods
                    .iter_mut()
                    .find(|jmod| jmod.module_name() == module)
                    .and_then(|jmod| jmod.get_class_bytes(name))
            }
        };
        self.cache
            .borrow_mut()
            .insert(key, loaded.clone().map(Rc::new));
        loaded
    }
}

#[cfg(test)]
mod tests {
    use super::{ClassPath, Error};
    use javac_jimage::JImage;
    use javac_jmod::JMod;
    use std::path::PathBuf;

    const CAFEBABE: [u8; 4] = [0xCA, 0xFE, 0xBA, 0xBE];

    /// The baseline JDK 17 used by the other container-reader tests: JAVA_HOME
    /// first, then the recorded Temurin 17.0.19 installation path.
    fn baseline_java_home() -> Option<PathBuf> {
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            let home = PathBuf::from(home);
            if home.join("lib").join("modules").is_file() || home.join("jmods").is_dir() {
                return Some(home);
            }
        }
        let fallback = PathBuf::from(r"C:\Program Files\Eclipse Adoptium\jdk-17.0.19.10-hotspot");
        if fallback.join("lib").join("modules").is_file() || fallback.join("jmods").is_dir() {
            return Some(fallback);
        }
        None
    }

    #[test]
    fn system_classes_match_direct_container_reads() {
        let Some(home) = baseline_java_home() else {
            eprintln!("skipping: no baseline JDK 17 installation found");
            return;
        };
        let class_path = ClassPath::system(&home).expect("system class path");

        let image_path = home.join("lib").join("modules");
        let image = if image_path.is_file() {
            Some(JImage::open(&image_path).expect("direct jimage"))
        } else {
            None
        };

        let cases = [
            ("java.base", "java.lang.Object"),
            ("java.base", "java.util.List"),
            ("java.compiler", "javax.tools.JavaCompiler"),
            ("jdk.compiler", "com.sun.tools.javac.Main"),
        ];
        for (module, name) in cases {
            let bytes = class_path
                .read_class(name)
                .unwrap_or_else(|| panic!("read_class({}) should find the class", name));
            assert_eq!(&bytes[..4], &CAFEBABE, "class magic for {}", name);
            assert_eq!(
                class_path.module_of_class(name).as_deref(),
                Some(module),
                "module_of_class({})",
                name
            );
            let in_module = class_path
                .read_class_in_module(module, name)
                .unwrap_or_else(|| panic!("read_class_in_module({}, {})", module, name));
            assert_eq!(in_module, bytes, "module-qualified read for {}", name);

            // Byte-for-byte identical with the direct container readers.
            if let Some(image) = &image {
                let direct = image
                    .get_class_bytes(module, name)
                    .unwrap_or_else(|| panic!("direct jimage read of {}", name));
                assert_eq!(direct, bytes, "direct jimage comparison for {}", name);
            } else {
                let jmod_path = home.join("jmods").join(format!("{}.jmod", module));
                let mut jmod = JMod::open(&jmod_path).expect("direct jmod");
                let direct = jmod
                    .get_class_bytes(name)
                    .unwrap_or_else(|| panic!("direct jmod read of {}", name));
                assert_eq!(direct, bytes, "direct jmod comparison for {}", name);
            }
        }

        // Slash-separated and ".class"-suffixed names are accepted too.
        let slash = class_path
            .read_class("java/lang/Object")
            .expect("slash name");
        assert_eq!(slash, class_path.read_class("java.lang.Object").unwrap());
        let suffixed = class_path
            .read_class("java.lang.Object.class")
            .expect("suffixed name");
        assert_eq!(suffixed, class_path.read_class("java.lang.Object").unwrap());
    }

    #[test]
    fn missing_classes_and_modules_return_none() {
        let Some(home) = baseline_java_home() else {
            eprintln!("skipping: no baseline JDK 17 installation found");
            return;
        };
        let class_path = ClassPath::system(&home).expect("system class path");
        assert!(class_path.read_class("no.such.Class").is_none());
        assert!(class_path.module_of_class("no.such.Class").is_none());
        assert!(
            class_path
                .read_class_in_module("java.base", "no.such.Class")
                .is_none()
        );
        assert!(
            class_path
                .read_class_in_module("no.such.module", "java.lang.Object")
                .is_none()
        );
    }

    #[test]
    fn module_listing_is_consistent() {
        let Some(home) = baseline_java_home() else {
            eprintln!("skipping: no baseline JDK 17 installation found");
            return;
        };
        let class_path = ClassPath::system(&home).expect("system class path");
        let modules = class_path.list_modules();
        assert!(!modules.is_empty());
        assert!(modules.iter().any(|module| module == "java.base"));
        assert_eq!(
            class_path.module_of_class("java.lang.Object").as_deref(),
            Some("java.base")
        );
        // Repeated resolution is served by the module cache and is consistent.
        assert_eq!(
            class_path.module_of_class("java.lang.Object").as_deref(),
            Some("java.base")
        );
        // Repeated reads are identical (cache hits).
        let first = class_path.read_class("java.lang.Object").unwrap();
        let second = class_path.read_class("java.lang.Object").unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn missing_system_library_is_reported() {
        let dir = std::env::temp_dir().join(format!(
            "javac-classpath-no-system-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let result = ClassPath::system(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        match result {
            Err(Error::NoSystemLibraries { home }) => assert_eq!(home, dir),
            other => panic!("expected NoSystemLibraries, got {:?}", other),
        }
    }
}
