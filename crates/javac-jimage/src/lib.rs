// PORTING ADAPTER: JDK 17 jimage (lib/modules) reader.
//!
//! javac reads system classes through the jrt:// file system backed by the
//! lib/modules jimage archive. rust-javac is pure Rust and must implement that
//! container format itself (no JVM filesystem, no external tools at runtime).
//!
//! The format is documented by the JDK implementation:
//!   java.base/jdk/internal/jimage/{ImageReader,ImageHeader,ImageStringsReader,
//!   BasicImageReader,ImageLocation,ImageStream}.java
//! which were extracted from the baseline JDK src.zip for reference. This adapter follows
//! ImageHeader / ImageStringsReader / ImageLocation / BasicImageReader one-for-one (including
//! the PerfectHashBuilder redirect-table lookup) and loads the index into memory once.
//!
//! PORTING_RULES: registered PORTING_RULES section 9 adapter: it is not a javac class port but
//! the pure-Rust replacement for the JVM jrt filesystem used by com.sun.tools.javac.file (P2).
//! The observable behaviour (resource names, resource bytes, error cases) mirrors
//! BasicImageReader / ImageReader of the same JDK 17 baseline.
//!
//! Known limitations (reported to the Lead):
//! - the jimage 1.0 format is little-endian here; the baseline images are produced by the local
//!   JDK and are little-endian, and a big-endian image is rejected by the header magic check;
//! - resources compressed by "jlink --compress" are reported as unavailable (the baseline
//!   lib/modules has none: BasicImageReader reports compressedSize == 0 for every entry);
//! - packages() is not implemented yet; the synthetic /packages tree can be added when
//!   com.sun.tools.javac.file needs it (P2).

pub mod image_header;
pub mod image_location;
pub mod image_reader;
pub mod image_strings;

use std::cell::OnceCell;
use std::fmt;
use std::io;
use std::path::Path;

use image_location::ImageLocation;
use image_reader::ImageIndex;

/// Errors produced while opening or reading a jimage file.
#[derive(Debug)]
pub enum Error {
    /// The file could not be read.
    Io(io::Error),
    /// The file does not start with the jimage magic (or is too short to hold a header).
    NotAnImage(String),
    /// The image version is not the supported 1.0.
    WrongVersion {
        name: String,
        major: i32,
        minor: i32,
    },
    /// The image index is truncated or holds nonsensical offsets.
    Corrupted(String),
    /// A location uses an attribute kind the format does not define.
    InvalidAttributeKind(usize),
    /// A location attribute runs past the locations table.
    MissingAttributeData,
    /// An offset is outside the referenced image table.
    OffsetOutOfBounds(&'static str, usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "I/O error: {}", err),
            Error::NotAnImage(name) => write!(f, "\"{}\" is not an image file", name),
            Error::WrongVersion {
                name,
                major,
                minor,
            } => write!(
                f,
                "the image file \"{}\" is not the correct version. Major: {}. Minor: {}",
                name, major, minor
            ),
            Error::Corrupted(message) => write!(f, "corrupted image: {}", message),
            Error::InvalidAttributeKind(kind) => {
                write!(f, "invalid jimage attribute kind: {}", kind)
            }
            Error::MissingAttributeData => write!(f, "missing jimage attribute data"),
            Error::OffsetOutOfBounds(table, offset) => {
                write!(f, "offset {} out of bounds in the {} table", offset, table)
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

/// A read-only view of a JDK lib/modules jimage archive.
#[derive(Debug)]
pub struct JImage {
    index: ImageIndex,
    modules: Vec<String>,
    /// Built lazily: only callers of resources() need the full listing.
    resources: OnceCell<Vec<String>>,
}

impl JImage {
    /// Opens the jimage at the given path and builds its module/resource directory once.
    ///
    /// Java: ImageReader.open(Path) (BasicImageReader.open plus getModuleNames and the
    /// /modules subtree walk that jimage list also performs).
    pub fn open(path: &Path) -> Result<JImage, Error> {
        let index = ImageIndex::open(path)?;
        let modules = index.modules()?;
        Ok(JImage {
            index,
            modules,
            resources: OnceCell::new(),
        })
    }

    /// The module names in image order (Java: ImageReader.getModuleNames()).
    pub fn modules(&self) -> &[String] {
        &self.modules
    }

    /// All module resources, sorted, named "<module>/<path>" — the names jimage list prints.
    ///
    /// Built lazily, on first use.
    pub fn resources(&self) -> &[String] {
        self.resources.get_or_init(|| {
            self.index
                .resource_names(&self.modules)
                .unwrap_or_default()
        })
    }

    /// Lists the children of a jimage tree node, e.g. "/modules/java.base/java/lang".
    ///
    /// Java: Files.newDirectoryStream(dir) on the jrt: file system (the directory's tree
    /// node holds the offsets of its children).
    pub fn list_directory(&self, node_path: &str) -> Option<Vec<String>> {
        let location = self.index.find_location(node_path)?;
        let content = self.index.get_resource(&location)?;
        let mut children = Vec::new();
        for chunk in content.as_chunks::<4>().0 {
            let attribute_offset = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            if attribute_offset <= 0 {
                continue;
            }
            if let Ok(child) = self.index.get_location(attribute_offset as usize) {
                children.push(child.full_name());
            }
        }
        Some(children)
    }

    /// Returns the bytes of the named resource, or None when the resource does not exist.
    ///
    /// The name is the jrt resource name without the "/modules/" prefix, for example
    /// "java.base/java/lang/Object.class" (a leading slash is also accepted).
    ///
    /// Java: BasicImageReader.getResource(String name) with ImageReader's /modules-relative name.
    pub fn find_resource(&self, name: &str) -> Option<Vec<u8>> {
        let full_name = if name.starts_with('/') {
            name.to_string()
        } else {
            format!("/{}", name)
        };
        let location: ImageLocation = self.index.find_location(&full_name)?;
        self.index.get_resource(&location)
    }

    /// Returns the class file bytes of a binary class name in a module.
    ///
    /// The binary name is dot-separated ("java.lang.Object"); an already slash-separated name
    /// works too. Java: the equivalent of converting a BinaryName to the jimage resource path.
    pub fn get_class_bytes(&self, module: &str, binary_name: &str) -> Option<Vec<u8>> {
        let path = binary_name.replace('.', "/");
        self.find_resource(&format!("{}/{}.class", module, path))
    }

    /// The image header (Java: BasicImageReader.getHeader()).
    pub fn header(&self) -> &image_header::ImageHeader {
        self.index.header()
    }

    /// The package names of the synthetic /packages tree, in image order.
    ///
    /// Java: ImageReader.handlePackages("/packages") / visitPackageLocation (the /packages
    /// subtree is built from the image's packages tree nodes).
    pub fn packages(&self) -> Result<Vec<String>, Error> {
        self.index.packages()
    }

    /// The modules that contain the package, in the image's package resource order.
    ///
    /// Java: ImageReader.visitPackageLocation; an unknown package yields an empty list.
    pub fn modules_of_package(&self, package: &str) -> Result<Vec<String>, Error> {
        self.index.modules_of_package(package)
    }

    /// The packages of a module, in /packages order (the reverse of the /packages tree).
    pub fn packages_of_module(&self, module: &str) -> Result<Vec<String>, Error> {
        self.index.packages_of_module(module)
    }

    /// The full entry list including the synthetic /modules and /packages tree nodes
    /// (Java: BasicImageReader.getEntryNames()).
    pub fn get_entry_names(&self) -> Vec<String> {
        self.index.get_entry_names()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::image_header;
    use super::image_location;
    use super::image_strings;
    use super::JImage;

    // Recorded from the baseline JDK 17.0.19 (Temurin 17.0.19+10), the JDK that also produced
    // this machine's lib/modules:
    // - EXPECTED_MODULES: module names and resource counts in /modules child order, which is
    //   what modules() returns (Java ImageReader.getModuleNames());
    // - EXPECTED_SAMPLE_RESOURCES: the first resource of every module in "jimage list" order
    //   (jimage list sorts full names) — checked for membership, not compared in order;
    // - EXPECTED_RESOURCE_FNV / EXPECTED_ENTRY_FNV: FNV-1a 64 over the complete sorted name
    //   lists (UTF-8 bytes plus '\n' after each name), computed from BasicImageReader data so
    //   that every resource name is compared, not only a sample.
    const EXPECTED_FLAGS: i32 = 0;
    const EXPECTED_RESOURCE_COUNT: i32 = 29580;
    const EXPECTED_TABLE_LENGTH: i32 = 29580;
    const EXPECTED_LOCATIONS_SIZE: i32 = 610665;
    const EXPECTED_STRINGS_SIZE: i32 = 649411;
    const EXPECTED_INDEX_SIZE: u64 = 1496744;
    const EXPECTED_ENTRY_COUNT: usize = 29580;
    const EXPECTED_SYNTHETIC_ENTRIES: usize = 2;
    const EXPECTED_TOTAL_RESOURCES: usize = 27316;
    const EXPECTED_OBJECT_CLASS_LEN: usize = 1895;
    const EXPECTED_MODULE_INFO_LEN: usize = 11454;
    const EXPECTED_MAIN_CLASS_LEN: usize = 1294;

    const EXPECTED_MODULES: &[(&str, usize)] = &[
        ("java.base", 6430),
        ("java.compiler", 147),
        ("java.datatransfer", 26),
        ("java.desktop", 5665),
        ("java.instrument", 11),
        ("java.logging", 67),
        ("java.management", 479),
        ("java.management.rmi", 36),
        ("java.naming", 249),
        ("java.net.http", 357),
        ("java.prefs", 22),
        ("java.rmi", 178),
        ("java.scripting", 20),
        ("java.se", 1),
        ("java.security.jgss", 269),
        ("java.security.sasl", 36),
        ("java.smartcardio", 32),
        ("java.sql", 79),
        ("java.sql.rowset", 72),
        ("java.transaction.xa", 4),
        ("java.xml", 2346),
        ("java.xml.crypto", 405),
        ("jdk.accessibility", 224),
        ("jdk.attach", 15),
        ("jdk.charsets", 298),
        ("jdk.compiler", 1655),
        ("jdk.crypto.cryptoki", 137),
        ("jdk.crypto.ec", 88),
        ("jdk.crypto.mscapi", 41),
        ("jdk.dynalink", 96),
        ("jdk.editpad", 5),
        ("jdk.hotspot.agent", 1739),
        ("jdk.httpserver", 71),
        ("jdk.incubator.foreign", 163),
        ("jdk.incubator.vector", 178),
        ("jdk.internal.ed", 4),
        ("jdk.internal.jvmstat", 68),
        ("jdk.internal.le", 214),
        ("jdk.internal.opt", 62),
        ("jdk.internal.vm.ci", 284),
        ("jdk.internal.vm.compiler", 1),
        ("jdk.internal.vm.compiler.management", 1),
        ("jdk.jartool", 71),
        ("jdk.javadoc", 566),
        ("jdk.jcmd", 53),
        ("jdk.jconsole", 236),
        ("jdk.jdeps", 422),
        ("jdk.jdi", 552),
        ("jdk.jdwp.agent", 1),
        ("jdk.jfr", 322),
        ("jdk.jlink", 188),
        ("jdk.jpackage", 121),
        ("jdk.jshell", 289),
        ("jdk.jsobject", 3),
        ("jdk.jstatd", 14),
        ("jdk.localedata", 1809),
        ("jdk.management", 35),
        ("jdk.management.agent", 44),
        ("jdk.management.jfr", 27),
        ("jdk.naming.dns", 26),
        ("jdk.naming.rmi", 13),
        ("jdk.net", 15),
        ("jdk.nio.mapmode", 2),
        ("jdk.random", 11),
        ("jdk.sctp", 27),
        ("jdk.security.auth", 26),
        ("jdk.security.jgss", 17),
        ("jdk.unsupported", 12),
        ("jdk.unsupported.desktop", 12),
        ("jdk.xml.dom", 90),
        ("jdk.zipfs", 37),
    ];

    const EXPECTED_SAMPLE_RESOURCES: &[&str] = &[
        "java.base/META-INF/services/java.nio.file.spi.FileSystemProvider",
        "java.compiler/javax/annotation/processing/AbstractProcessor.class",
        "java.datatransfer/java/awt/datatransfer/Clipboard.class",
        "java.desktop/com/sun/accessibility/internal/resources/accessibility.class",
        "java.instrument/java/lang/instrument/ClassDefinition.class",
        "java.logging/java/util/logging/ConsoleHandler.class",
        "java.management/com/sun/jmx/defaults/JmxProperties.class",
        "java.management.rmi/com/sun/jmx/remote/internal/rmi/ProxyRef.class",
        "java.naming/com/sun/jndi/ldap/AbstractLdapNamingEnumeration.class",
        "java.net.http/java/net/http/HttpClient$Builder.class",
        "java.prefs/java/util/prefs/AbstractPreferences$1.class",
        "java.rmi/java/rmi/AccessException.class",
        "java.scripting/com/sun/tools/script/shell/Main$1.class",
        "java.se/module-info.class",
        "java.security.jgss/javax/security/auth/kerberos/DelegationPermission.class",
        "java.security.sasl/com/sun/security/sasl/ClientFactoryImpl.class",
        "java.smartcardio/javax/smartcardio/ATR.class",
        "java.sql/java/sql/Array.class",
        "java.sql.rowset/com/sun/rowset/CachedRowSetImpl$1.class",
        "java.transaction.xa/javax/transaction/xa/XAException.class",
        "java.xml/com/sun/java_cup/internal/runtime/Scanner.class",
        "java.xml.crypto/com/sun/org/apache/xml/internal/security/Init$1.class",
        "jdk.accessibility/com/sun/java/accessibility/internal/AccessBridge$1.class",
        "jdk.attach/com/sun/tools/attach/AgentInitializationException.class",
        "jdk.charsets/module-info.class",
        "jdk.compiler/com/sun/source/doctree/AttributeTree$ValueKind.class",
        "jdk.crypto.cryptoki/module-info.class",
        "jdk.crypto.ec/module-info.class",
        "jdk.crypto.mscapi/module-info.class",
        "jdk.dynalink/jdk/dynalink/BiClassValue$1.class",
        "jdk.editpad/jdk/editpad/EditPad$1.class",
        "jdk.hotspot.agent/com/sun/java/swing/action/AboutAction.class",
        "jdk.httpserver/com/sun/net/httpserver/Authenticator$Failure.class",
        "jdk.incubator.foreign/jdk/incubator/foreign/AbstractLayout.class",
        "jdk.incubator.vector/jdk/incubator/vector/AbstractMask$MBinOp.class",
        "jdk.internal.ed/jdk/internal/editor/external/ExternalEditor$1.class",
        "jdk.internal.jvmstat/module-info.class",
        "jdk.internal.le/jdk/internal/org/jline/keymap/BindingReader.class",
        "jdk.internal.opt/jdk/internal/joptsimple/AbstractOptionSpec.class",
        "jdk.internal.vm.ci/jdk/vm/ci/aarch64/AArch64$1.class",
        "jdk.internal.vm.compiler/module-info.class",
        "jdk.internal.vm.compiler.management/module-info.class",
        "jdk.jartool/com/sun/jarsigner/ContentSigner.class",
        "jdk.javadoc/jdk/javadoc/doclet/Doclet$Option$Kind.class",
        "jdk.jcmd/module-info.class",
        "jdk.jconsole/com/sun/tools/jconsole/JConsoleContext$ConnectionState.class",
        "jdk.jdeps/com/sun/tools/classfile/AccessFlags$Kind.class",
        "jdk.jdi/com/sun/jdi/AbsentInformationException.class",
        "jdk.jdwp.agent/module-info.class",
        "jdk.jfr/jdk/jfr/AnnotationElement.class",
        "jdk.jlink/jdk/tools/jimage/JImageTask$1.class",
        "jdk.jpackage/jdk/jpackage/internal/AbstractAppImageBuilder$IconType.class",
        "jdk.jshell/jdk/internal/jshell/debug/InternalDebugControl.class",
        "jdk.jsobject/module-info.class",
        "jdk.jstatd/module-info.class",
        "jdk.localedata/module-info.class",
        "jdk.management/com/sun/management/DiagnosticCommandMBean.class",
        "jdk.management.agent/jdk/internal/agent/Agent$StatusCollector.class",
        "jdk.management.jfr/jdk/management/jfr/ConfigurationInfo.class",
        "jdk.naming.dns/com/sun/jndi/dns/BaseNameClassPairEnumeration.class",
        "jdk.naming.rmi/com/sun/jndi/rmi/registry/AtomicNameParser.class",
        "jdk.net/jdk/net/ExtendedSocketOptions$1.class",
        "jdk.nio.mapmode/jdk/nio/mapmode/ExtendedMapMode.class",
        "jdk.random/jdk/random/L128X1024MixRandom.class",
        "jdk.sctp/com/sun/nio/sctp/AbstractNotificationHandler.class",
        "jdk.security.auth/com/sun/security/auth/LdapPrincipal.class",
        "jdk.security.jgss/com/sun/security/jgss/AuthorizationDataEntry.class",
        "jdk.unsupported/com/sun/nio/file/ExtendedCopyOption.class",
        "jdk.unsupported.desktop/jdk/swing/interop/DispatcherWrapper$DispatcherProxy.class",
        "jdk.xml.dom/module-info.class",
        "jdk.zipfs/jdk/nio/zipfs/ByteArrayChannel.class",
    ];

    /// FNV-1a 64 over the sorted resource names (UTF-8 bytes plus one newline each).
    const EXPECTED_RESOURCE_FNV: u64 = 11244746063798739379;
    /// FNV-1a 64 over getEntryNames() (sorted, including the synthetic tree nodes).
    const EXPECTED_ENTRY_FNV: u64 = 15034021959058779748;

    /// The lib/modules file of the baseline JDK, if this machine has one.
    fn baseline_image() -> Option<PathBuf> {
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            let candidate = PathBuf::from(home).join("lib").join("modules");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        let fallback = PathBuf::from(r"C:\Program Files\Eclipse Adoptium\jdk-17.0.19.10-hotspot\lib\modules");
        if fallback.is_file() {
            return Some(fallback);
        }
        None
    }

    fn fnv1a64_names(names: &[String]) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for name in names {
            for byte in name.as_bytes() {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
            hash ^= b'\n' as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    fn open_baseline() -> Option<JImage> {
        let path = baseline_image()?;
        match JImage::open(&path) {
            Ok(image) => Some(image),
            Err(err) => panic!("cannot open the baseline image {}: {}", path.display(), err),
        }
    }

    fn is_recorded_baseline(image: &JImage) -> bool {
        let header = image.header();
        header.resource_count == EXPECTED_RESOURCE_COUNT
            && header.table_length == EXPECTED_TABLE_LENGTH
            && header.locations_size == EXPECTED_LOCATIONS_SIZE
            && header.strings_size == EXPECTED_STRINGS_SIZE
    }

    #[test]
    fn opens_image_and_reads_known_resources() {
        let Some(image) = open_baseline() else {
            eprintln!("skipping opens_image_and_reads_known_resources: no JDK lib/modules found");
            return;
        };

        let header = image.header();
        assert_eq!(header.magic, image_header::MAGIC);
        assert_eq!(header.major_version, 1);
        assert_eq!(header.minor_version, 0);

        let modules: Vec<&str> = image.modules().iter().map(String::as_str).collect();
        assert!(!modules.is_empty());
        assert!(modules.contains(&"java.base"));
        assert!(modules.contains(&"java.compiler"));
        assert!(modules.contains(&"jdk.compiler"));

        // java.base/java/lang/Object.class is the class javac starts from for every compile.
        let object = image
            .find_resource("java.base/java/lang/Object.class")
            .expect("java.base/java/lang/Object.class must exist");
        assert!(!object.is_empty());
        assert_eq!(&object[..4], &[0xCA, 0xFE, 0xBA, 0xBE]);

        // The leading slash is accepted as well (the internal name form).
        assert_eq!(
            image.find_resource("/java.base/java/lang/Object.class").as_deref(),
            Some(object.as_slice())
        );
        assert_eq!(
            image.get_class_bytes("java.base", "java.lang.Object").as_deref(),
            Some(object.as_slice())
        );
        assert_eq!(
            image.get_class_bytes("java.base", "java/lang/Object").as_deref(),
            Some(object.as_slice())
        );

        assert!(image
            .get_class_bytes("java.base", "java.lang.NoSuchClassXyz")
            .is_none());
        assert!(image
            .find_resource("java.base/java/lang/NoSuchClassXyz.class")
            .is_none());
        assert!(image.find_resource("no.such.module/Foo.class").is_none());
        assert!(image.find_resource("java.base").is_none());
    }

    #[test]
    fn matches_the_recorded_jimage_list() {
        let Some(image) = open_baseline() else {
            eprintln!("skipping matches_the_recorded_jimage_list: no JDK lib/modules found");
            return;
        };
        if !is_recorded_baseline(&image) {
            eprintln!(
                "skipping the recorded jimage list comparison: the image is not the recorded JDK 17 baseline"
            );
            return;
        }

        let header = image.header();
        assert_eq!(header.flags, EXPECTED_FLAGS);
        assert_eq!(header.resource_count, EXPECTED_RESOURCE_COUNT);
        assert_eq!(header.table_length, EXPECTED_TABLE_LENGTH);
        assert_eq!(header.locations_size, EXPECTED_LOCATIONS_SIZE);
        assert_eq!(header.strings_size, EXPECTED_STRINGS_SIZE);
        assert_eq!(header.get_index_size(), EXPECTED_INDEX_SIZE);

        // Module names and their order must equal "jimage list" output.
        let modules = image.modules();
        assert_eq!(modules.len(), EXPECTED_MODULES.len());
        for (index, (expected_name, _)) in EXPECTED_MODULES.iter().enumerate() {
            assert_eq!(modules[index], *expected_name, "module #{}", index);
        }

        // The flat resource list must equal the full "jimage list" resource list.
        let resources = image.resources();
        assert_eq!(resources.len(), EXPECTED_TOTAL_RESOURCES);
        assert!(resources.windows(2).all(|pair| pair[0] <= pair[1]));

        let mut counts: HashMap<&str, usize> = HashMap::new();
        for resource in resources {
            let module = resource.split('/').next().expect("module prefix");
            *counts.entry(module).or_insert(0) += 1;
        }
        assert_eq!(counts.len(), EXPECTED_MODULES.len());
        for (module, expected_count) in EXPECTED_MODULES {
            assert_eq!(
                counts.get(module).copied().unwrap_or(0),
                *expected_count,
                "resource count of {}",
                module
            );
        }

        for sample in EXPECTED_SAMPLE_RESOURCES {
            assert!(
                resources.binary_search(&sample.to_string()).is_ok(),
                "missing resource {}",
                sample
            );
        }

        // The complete sorted resource name list must equal the recorded jimage list.
        assert_eq!(
            fnv1a64_names(image.resources()),
            EXPECTED_RESOURCE_FNV,
            "resource name list does not match the recorded jimage list"
        );

        // Resource lengths recorded through BasicImageReader.
        assert_eq!(
            image
                .find_resource("java.base/java/lang/Object.class")
                .expect("Object.class")
                .len(),
            EXPECTED_OBJECT_CLASS_LEN
        );
        assert_eq!(
            image
                .find_resource("java.base/module-info.class")
                .expect("module-info.class")
                .len(),
            EXPECTED_MODULE_INFO_LEN
        );
        assert_eq!(
            image
                .find_resource("jdk.compiler/com/sun/tools/javac/Main.class")
                .expect("javac Main.class")
                .len(),
            EXPECTED_MAIN_CLASS_LEN
        );

        // getEntryNames also contains the synthetic /modules and /packages tree nodes.
        let entries = image.get_entry_names();
        assert_eq!(entries.len(), EXPECTED_ENTRY_COUNT);
        assert_eq!(
            fnv1a64_names(&entries),
            EXPECTED_ENTRY_FNV,
            "entry name list does not match the recorded image"
        );
        assert_eq!(
            entries.len(),
            EXPECTED_TOTAL_RESOURCES + EXPECTED_SYNTHETIC_ENTRIES + EXPECTED_TREE_NODES
        );
        assert!(entries.contains(&"/modules".to_string()));
        assert!(entries.contains(&"/packages".to_string()));
        assert!(entries.contains(&"/java.base/java/lang/Object.class".to_string()));
        assert!(!image
            .resources()
            .iter()
            .any(|name| name.starts_with("modules/") || name.starts_with("packages/")));
    }

    /// /modules and /packages tree nodes that carry the synthetic "modules"/"packages" module
    /// marker: 29580 entries = 27316 resources + 2262 tree nodes + 2 roots.
    const EXPECTED_TREE_NODES: usize = 2262;

    fn temp_file(tag: &str, bytes: &[u8]) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "rust-javac-jimage-test-{}-{}.bin",
            std::process::id(),
            tag
        ));
        std::fs::write(&path, bytes).expect("write temp file");
        path
    }

    #[test]
    fn rejects_missing_and_malformed_files() {
        let missing = {
            let mut path = std::env::temp_dir();
            path.push("rust-javac-jimage-no-such-image");
            path
        };
        assert!(matches!(
            JImage::open(&missing).unwrap_err(),
            super::Error::Io(_)
        ));

        // Shorter than the 28-byte header.
        let short = temp_file("short", &[0u8; 10]);
        assert!(matches!(
            JImage::open(&short).unwrap_err(),
            super::Error::NotAnImage(_)
        ));
        std::fs::remove_file(&short).ok();

        // Right length, wrong magic.
        let mut bad_magic = [0u8; 28];
        bad_magic[..4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        let bad_magic_path = temp_file("bad-magic", &bad_magic);
        assert!(matches!(
            JImage::open(&bad_magic_path).unwrap_err(),
            super::Error::NotAnImage(_)
        ));
        std::fs::remove_file(&bad_magic_path).ok();

        // Right magic, unsupported version (2.0).
        let mut wrong_version = [0u8; 28];
        wrong_version[..4].copy_from_slice(&image_header::MAGIC.to_le_bytes());
        wrong_version[4..8].copy_from_slice(&(2i32 << 16).to_le_bytes());
        let wrong_version_path = temp_file("wrong-version", &wrong_version);
        assert!(matches!(
            JImage::open(&wrong_version_path).unwrap_err(),
            super::Error::WrongVersion { .. }
        ));
        std::fs::remove_file(&wrong_version_path).ok();

        // Right magic and version, but the file is shorter than the declared index.
        let mut truncated = [0u8; 28];
        truncated[..4].copy_from_slice(&image_header::MAGIC.to_le_bytes());
        truncated[4..8].copy_from_slice(&(1i32 << 16).to_le_bytes());
        truncated[16..20].copy_from_slice(&10i32.to_le_bytes()); // tableLength
        truncated[20..24].copy_from_slice(&100i32.to_le_bytes()); // locationsSize
        truncated[24..28].copy_from_slice(&100i32.to_le_bytes()); // stringsSize
        let truncated_path = temp_file("truncated-index", &truncated);
        assert!(matches!(
            JImage::open(&truncated_path).unwrap_err(),
            super::Error::Corrupted(_)
        ));
        std::fs::remove_file(&truncated_path).ok();
    }

    #[test]
    fn image_strings_hash_matches_jdk() {
        // Produced by jdk.internal.jimage.ImageStringsReader.hashCode on the baseline JDK.
        assert_eq!(image_strings::hash_code(&units("/java.base/java/lang/Object.class")), 2066871583);
        assert_eq!(image_strings::hash_code(&units("/java.base/module-info.class")), 2084612757);
        assert_eq!(image_strings::hash_code(&units("/modules")), 271141917);
        assert_eq!(image_strings::hash_code(&units("/packages")), 192025657);
        assert_eq!(
            image_strings::hash_code(&units(
                "/java.base/META-INF/services/java.nio.file.spi.FileSystemProvider"
            )),
            1326423269
        );
        assert_eq!(image_strings::hash_code(&units("")), image_strings::HASH_MULTIPLIER);
        assert_eq!(
            image_strings::hash_code_seeded(&units("/java.base/java/lang/Object.class"), 12345),
            1812041453
        );
        assert_eq!(
            image_strings::hash_code_seeded(&units("/java.base/java/lang/Object.class"), 1),
            1076931637
        );
        // hash(module, name) is hash("/module/name"), which is what the image index uses.
        assert_eq!(
            image_strings::hash_code_module_name("java.base", "java/lang/Object.class"),
            image_strings::hash_code(&units("/java.base/java/lang/Object.class"))
        );
        assert_eq!(
            image_strings::hash_code_module_name("jdk.compiler", "com/sun/tools/javac/Main.class"),
            1301913827
        );
    }

    fn units(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn image_strings_decode_modified_utf8() {
        // Build a minimal string table: every entry is terminated by a zero byte.
        let mut table: Vec<u8> = Vec::new();
        // offset 0: "abc"
        table.extend_from_slice(b"abc");
        table.push(0);
        // offset 4: U+00E9
        let offset_e_acute = table.len();
        table.extend_from_slice(&[0xC3, 0xA9]);
        table.push(0);
        // offset 7: U+4E2D U+6587
        let offset_cjk = table.len();
        table.extend_from_slice(&[0xE4, 0xB8, 0xAD, 0xE6, 0x96, 0x87]);
        table.push(0);
        // offset 14: U+1F600 as the surrogate pair D83D DE00
        let offset_emoji = table.len();
        table.extend_from_slice(&[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
        table.push(0);
        // offset 21: "a" NUL "b" (modified UTF-8 encodes NUL as C0 80)
        let offset_nul = table.len();
        table.extend_from_slice(&[0x61, 0xC0, 0x80, 0x62]);
        table.push(0);

        assert_eq!(image_strings::string_from_bytes(&table, 0).unwrap(), "abc");
        assert_eq!(image_strings::string_from_bytes(&table, offset_e_acute).unwrap(), "é");
        assert_eq!(image_strings::string_from_bytes(&table, offset_cjk).unwrap(), "中文");
        assert_eq!(image_strings::string_from_bytes(&table, offset_emoji).unwrap(), "😀");
        assert_eq!(
            image_strings::string_from_bytes(&table, offset_nul).unwrap(),
            "a\0b"
        );

        // match_string returns the number of matched UTF-16 code units (or None).
        let name = units("abc");
        assert_eq!(image_strings::match_string(&table, 0, &name, 0), Some(3));
        assert_eq!(image_strings::match_string(&table, 0, &name, 1), None);
        let longer = units("abcd");
        assert_eq!(image_strings::match_string(&table, 0, &longer, 0), Some(3));
        let e_acute = units("é");
        assert_eq!(
            image_strings::match_string(&table, offset_e_acute, &e_acute, 0),
            Some(1)
        );
        let emoji = units("😀");
        assert_eq!(
            image_strings::match_string(&table, offset_emoji, &emoji, 0),
            Some(2)
        );
    }

    #[test]
    fn image_location_decompress_reads_attribute_record() {
        let mut bytes: Vec<u8> = Vec::new();
        // kind = 1 (MODULE), value 5 (1 byte)
        bytes.extend_from_slice(&[(1 << 3), 0x05]);
        // kind = 3 (BASE), value 7 (1 byte)
        bytes.extend_from_slice(&[(3 << 3), 0x07]);
        // kind = 5 (OFFSET), value 100 (1 byte)
        bytes.extend_from_slice(&[(5 << 3), 0x64]);
        // kind = 7 (UNCOMPRESSED), value 1895 = 0x0767 (2 bytes)
        bytes.extend_from_slice(&[(7 << 3) | 1, 0x07, 0x67]);
        // ATTRIBUTE_END
        bytes.push(0);

        let attributes = image_location::decompress(&bytes, 0).expect("decompress");
        assert_eq!(attributes[image_location::ATTRIBUTE_MODULE], 5);
        assert_eq!(attributes[image_location::ATTRIBUTE_BASE], 7);
        assert_eq!(attributes[image_location::ATTRIBUTE_OFFSET], 100);
        assert_eq!(attributes[image_location::ATTRIBUTE_UNCOMPRESSED], 1895);
        assert_eq!(attributes[image_location::ATTRIBUTE_PARENT], 0);
        assert_eq!(attributes[image_location::ATTRIBUTE_EXTENSION], 0);
    }

    /// Recorded from the baseline JDK 17.0.19 through jdk.internal.jimage.ImageReader (the
    /// /packages tree) and cross-checked with jrt-fs /packages (identical, 984 packages);
    /// jimage list skips the synthetic tree resources, so ImageReader is the reference.
    const EXPECTED_PACKAGE_COUNT: usize = 984;
    const EXPECTED_PACKAGE_FNV: u64 = 17146790916066939826; // -1299953157642611790 as u64
    const EXPECTED_PACKAGE_PAIR_COUNT: usize = 1205;
    const EXPECTED_PACKAGE_PAIR_FNV: u64 = 17686967839628854155; // -759776234080697461
    const EXPECTED_MODULE_COUNT: usize = 67;
    const EXPECTED_MODULE_PACKAGE_FNV: u64 = 14470025612618747865; // -3976718461090803751

    #[test]
    fn packages_match_the_packages_tree() {
        let Some(image) = open_baseline() else {
            eprintln!("skipping: no baseline JDK image found");
            return;
        };
        let mut packages = image.packages().expect("packages");
        assert_eq!(packages.len(), EXPECTED_PACKAGE_COUNT);
        packages.sort();
        assert_eq!(fnv1a64_names(&packages), EXPECTED_PACKAGE_FNV);
        // The /packages entries of getEntryNames() are exactly the package tree leaves.
        let mut entry_packages: Vec<String> = image
            .get_entry_names()
            .iter()
            .filter_map(|name| name.strip_prefix("/packages/"))
            .map(str::to_string)
            .collect();
        entry_packages.sort();
        assert_eq!(entry_packages, packages);
    }

    #[test]
    fn modules_of_package_samples_match_the_tree() {
        let Some(image) = open_baseline() else {
            eprintln!("skipping: no baseline JDK image found");
            return;
        };
        let cases: &[(&str, &[&str])] = &[
            ("java.lang", &["java.base", "java.instrument", "java.management"]),
            ("java.util", &["java.base", "java.logging", "java.prefs"]),
            ("java.io", &["java.base"]),
            ("java.nio", &["java.base"]),
            ("javax.tools", &["java.compiler"]),
            ("com.sun.beans", &["java.desktop"]),
        ];
        for (package, expected) in cases {
            let mut modules = image.modules_of_package(package).expect("modules_of_package");
            modules.sort();
            let expected: Vec<String> = expected.iter().map(|module| module.to_string()).collect();
            assert_eq!(modules, expected, "package {}", package);
        }
        assert!(image.modules_of_package("no.such.package").unwrap().is_empty());
    }

    #[test]
    fn package_module_pairs_match_the_tree() {
        let Some(image) = open_baseline() else {
            eprintln!("skipping: no baseline JDK image found");
            return;
        };
        let packages = image.packages().expect("packages");
        let mut lines = Vec::new();
        let mut pair_count = 0usize;
        for package in &packages {
            let mut modules = image.modules_of_package(package).expect("modules_of_package");
            modules.sort();
            pair_count += modules.len();
            lines.push(format!("{}={}", package, modules.join(",")));
        }
        assert_eq!(pair_count, EXPECTED_PACKAGE_PAIR_COUNT);
        assert_eq!(fnv1a64_names(&lines), EXPECTED_PACKAGE_PAIR_FNV);
    }

    #[test]
    fn packages_of_module_matches_the_tree() {
        let Some(image) = open_baseline() else {
            eprintln!("skipping: no baseline JDK image found");
            return;
        };
        let mut modules = image.modules().to_vec();
        modules.sort();
        let mut lines = Vec::new();
        for module in &modules {
            let mut packages = image.packages_of_module(module).expect("packages_of_module");
            packages.sort();
            if !packages.is_empty() {
                lines.push(format!("{}={}", module, packages.join(",")));
            }
        }
        assert_eq!(lines.len(), EXPECTED_MODULE_COUNT);
        assert_eq!(fnv1a64_names(&lines), EXPECTED_MODULE_PACKAGE_FNV);
        for (module, count) in [("java.base", 189), ("java.compiler", 9), ("jdk.compiler", 40)] {
            assert_eq!(
                image.packages_of_module(module).expect("packages_of_module").len(),
                count,
                "module {}",
                module
            );
        }
        assert!(image.packages_of_module("no.such.module").unwrap().is_empty());
    }

    #[test]
    fn packages_cover_the_resource_derived_packages() {
        let Some(image) = open_baseline() else {
            eprintln!("skipping: no baseline JDK image found");
            return;
        };
        let packages: std::collections::HashSet<String> =
            image.packages().expect("packages").into_iter().collect();
        let mut derived = std::collections::HashSet::new();
        for resource in image.resources() {
            let Some((_, path)) = resource.split_once('/') else {
                continue;
            };
            let Some((dir, _file)) = path.rsplit_once('/') else {
                continue;
            };
            let package = dir.replace('/', ".");
            if package.split('.').all(is_package_identifier) {
                derived.insert(package);
            }
        }
        for package in &derived {
            assert!(
                packages.contains(package),
                "resource-derived package {} is missing from /packages",
                package
            );
        }
        // Recorded on the baseline image: the resources derive 874 distinct package-like
        // directory names, all of which are present in the /packages tree.
        assert_eq!(derived.len(), 874);
    }

    fn is_package_identifier(segment: &str) -> bool {
        let mut chars = segment.chars();
        match chars.next() {
            Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
            _ => return false,
        }
        chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
    }
}
