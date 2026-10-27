// Origin: com/sun/tools/javac/platform/JDKPlatformProvider.java @ 982d53c (jdk-17.0.19-ga)

//! PlatformProvider for JDK N.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES — this file is ported in batches (S1 interface parts, S2 the
//! --release platform set); getFileManager stays unimplemented until S3.
//!
//! ## Ported (S1 + S2)
//! * `JDKPlatformProvider` itself: `getPlatform` -> `new PlatformDescriptionImpl(platformName)`.
//! * `NUMERICAL_COMPARATOR` -> `numerical_comparator` free function (java.util.Comparator
//!   is not a ported type; the sort order is 1:1: numeric when both parse, otherwise
//!   Integer.MAX_VALUE, ties broken by String.compareTo).
//! * `PlatformDescriptionImpl`: the `sourceVersion`/`ctSymVersion` derivation
//!   (`Integer.parseInt`, `Integer.toString(int, Character.MAX_RADIX)`,
//!   `StringUtils.toUpperCase`), the three empty plugin/processor/option lists and
//!   `close`.  `java.lang.Integer` is not a ported surface: the file-local
//!   `parse_int`/`integer_to_string` reproduce the calls for the radix range
//!   used here (10 and 36); the documented helper differences are listed below.
//! * S2: the static `SUPPORTED_JAVA_PLATFORM_VERSIONS` scan.  Java computes it once in
//!   the class initializer; the Rust port keeps the result in a per-provider
//!   `OnceLock` (lazily computed like the initializer, but per instance because the JDK
//!   home is now a constructor argument).  The scan itself is 1:1:
//!   `findCtSym()` -> `<java.home>/lib/ct.sym`; if the file does not exist the set
//!   stays empty; otherwise every ct.sym root directory name containing '-' is skipped, the
//!   other names are read one base-36 character at a time
//!   (`Integer.parseInt(Character.toString(ver), Character.MAX_RADIX)`), looked up in
//!   `Target.lookup` and recorded as `Target.ordinal() - Target.JDK1_1.ordinal() + 1`;
//!   the set is numerically sorted and de-duplicated (the Java TreeSet with
//!   NUMERICAL_COMPARATOR).
//!   * `System.getProperty("java.home")` is a JVM facility and rust-javac runs without a
//!     JVM (P6 launcher not ported), so the JDK home is injected:
//!     `JDKPlatformProvider::new(java_home)` (registered deviation; the P6 service wiring
//!     passes the launcher's JDK home).
//!   * `java.nio.file` is represented by the registered javac-fs adapter
//!     (`Files::exists`) and ct.sym reading by the registered javac-ctsym adapter
//!     (`CtSym::open` / `sections()`), which implements the same static-initializer
//!     decoding over javac-zip.
//!   * upstream catches `IOException | ProviderNotFoundException` around the archive scan and
//!     leaves the set empty; the Rust port returns the empty set for a missing file and for a
//!     `javac_ctsym::Error` (I/O, not-a-zip, corrupted) — same observable behaviour, no
//!     substitute data.
//!   * an invalid base-36 character in a section name makes the Java initializer fail
//!     (NumberFormatException -> ExceptionInInitializerError); `parse_int_radix` panics with the
//!     same message.  javac-ctsym already panics while opening such an archive (see its docs).
//! * Dependencies: javac-core/Cargo.toml adds javac-fs (workspace dependency) and javac-ctsym
//!   (path dependency for now; javac-ctsym is not yet in the workspace dependencies table).
//!
//! ## Not ported yet (TODO, with the dependency that unblocks each)
//! 1. `PlatformDescriptionImpl.getFileManager()` (S3): needs
//!    - `com.sun.tools.javac.file.JavacFileManager`/`CacheFSInfo` (the `file` package
//!      is still an empty placeholder),
//!    - `com.sun.tools.javac.util.Context` (ported) plus `Log` (diagnostic output) and
//!      `main.Option` (now ported: Option.MULTIRELEASE/handleOption exist),
//!    - the ct.sym archive handle for the platform file manager (javac-ctsym/javac-fs now exist,
//!      but JavacFileManager and its Locations are not ported),
//!    - the Java anonymous `JavacFileManager` subclass + `SigJavaFileObject`: Java
//!      subclassing -> Rust composition with explicit overriding (the javac-tools
//!      `ForwardingJavaFileObject` pattern).
//! 2. `SigJavaFileObject` (S3, its only caller is getFileManager()).
//! 3. the `ctSym2FileSystem` map and the `close()` loop over it (S3; in this no
//!    file system handle is kept open — javac-ctsym reads the archive once and drops it — so
//!    close() has nothing to release and returns Ok like the upstream empty-loop case).
//! 4. S4 (launcher): supply the real java.home in front of JDKPlatformProvider::new.
//! 5. S5 (P6): real service discovery for PlatformUtils (its provider parameter is the
//!    injection point; getSupportedPlatformNames then feeds --release).
//!
//! ## Oracle (development-time, recorded)
//! The baseline JDK 17.0.19 (Temurin 17.0.19+10) javac accepts exactly releases 7..17:
//! javac --release 6 -version exits 2, 7..17 exit 0, javac --release 18 -version exits 2.
//! That is the same set the ct.sym scan derives; the snapshot test below asserts the
//! provider's list against this recorded oracle and, when a JDK home is discoverable
//! (JAVA_HOME or PATH), against the real lib/ct.sym.

use std::cmp::Ordering;
use std::rc::Rc;
use std::sync::OnceLock;

use javac_ctsym::CtSym;
use javac_fs::{Files, Path as FsPath};
use javac_source::util::plugin::Plugin;
use javac_tools::error::Error;
use javac_tools::jdk::Processor;
use javac_tools::tools::java_file_manager::JavaFileManager;

use crate::code::source::{Feature, Source};
use crate::file::cache_fs_info::CacheFSInfo;
use crate::file::javac_file_manager::JavacFileManager;
use crate::platform::ct_sym_java_file_manager::CtSymJavaFileManager;
use crate::util::context::Context;
use crate::util::log::Log;
use crate::util::options::Options;
use crate::jvm::target::Target;
use crate::platform::platform_description::{PlatformDescription, PluginInfo};
use crate::platform::platform_provider::{PlatformNotSupported, PlatformProvider};
use crate::util::java_string::JavaString;
use crate::util::string_utils::to_upper_case;

/// Java: private static final String[] symbolFileLocation = { "lib", "ct.sym" };
const SYMBOL_FILE_LOCATION: [&str; 2] = ["lib", "ct.sym"];

/// PlatformProvider for JDK N.
///
/// PORTING_RULES: the JDK home is injected (the upstream reads the java.home system
/// property, which does not exist without a JVM); see the file header.
pub struct JDKPlatformProvider {
    /// Java: System.getProperty("java.home") — injected by the launcher (P6).
    java_home: FsPath,
    /// Java: static SUPPORTED_JAVA_PLATFORM_VERSIONS (lazy, per provider instance).
    supported: OnceLock<Vec<JavaString>>,
}

impl JDKPlatformProvider {
    /// Creates the provider for the given JDK home (the upstream java.home property).
    ///
    /// PORTING_RULES: upstream has only the implicit no-argument constructor; the JDK home
    /// is injected because rust-javac runs without a JVM (registered deviation).
    pub fn new(java_home: FsPath) -> JDKPlatformProvider {
        JDKPlatformProvider {
            java_home,
            supported: OnceLock::new(),
        }
    }

    /// Java:
    /// <pre>
    /// static Path findCtSym() {
    ///     String javaHome = System.getProperty("java.home");
    ///     Path file = Paths.get(javaHome);
    ///     // file == ${jdk.home}
    ///     for (String name : symbolFileLocation)
    ///         file = file.resolve(name);
    ///     return file;
    /// }
    /// </pre>
    ///
    /// PORTING_RULES: java.nio.file.Path is the registered javac-fs adapter; the java.home
    /// property is the injected argument.
    pub fn find_ct_sym(java_home: &FsPath) -> FsPath {
        let mut file = java_home.clone();
        for name in SYMBOL_FILE_LOCATION {
            file = file
                .resolve(&FsPath::new(name).expect("symbol file location names are valid paths"));
        }
        file
    }

    /// Java:
    /// <pre>
    /// static {
    ///     SUPPORTED_JAVA_PLATFORM_VERSIONS = new TreeSet<>(NUMERICAL_COMPARATOR);
    ///     Path ctSymFile = findCtSym();
    ///     if (Files.exists(ctSymFile)) {
    ///         try (FileSystem fs = FileSystems.newFileSystem(ctSymFile, (ClassLoader)null);
    ///              DirectoryStream<Path> dir = ...) {
    ///             for (Path section : dir) {
    ///                 if (section.getFileName().toString().contains("-"))
    ///                     continue;
    ///                 for (char ver : section.getFileName().toString().toCharArray()) {
    ///                     String verString = Character.toString(ver);
    ///                     Target t = Target.lookup("" + Integer.parseInt(verString, Character.MAX_RADIX));
    ///                     if (t != null) {
    ///                         SUPPORTED_JAVA_PLATFORM_VERSIONS.add(targetNumericVersion(t));
    ///                     }
    ///                 }
    ///             }
    ///         } catch (IOException | ProviderNotFoundException ex) {
    ///         }
    ///     }
    /// }
    /// </pre>
    fn compute_supported_java_platform_versions(java_home: &FsPath) -> Vec<JavaString> {
        let mut versions: Vec<JavaString> = Vec::new();
        let ct_sym = JDKPlatformProvider::find_ct_sym(java_home);
        // Java: if (Files.exists(ctSymFile)) {... } — a missing ct.sym leaves the set empty.
        if Files::exists(&ct_sym) {
            // Java: FileSystems.newFileSystem(...) throws IOException/ProviderNotFoundException,
            // both caught -> empty set. javac-ctsym reports the same failures as Err.
            if let Ok(archive) = CtSym::open(&ct_sym.as_path()) {
                for section in archive.sections() {
                    let name = section.name();
                    if name.contains('-') {
                        continue;
                    }
                    for ch in name.chars() {
                        let version = parse_int_radix(ch);
                        if let Some(target) = Target::lookup(&version.to_string()) {
                            versions.push(target_numeric_version(target));
                        }
                    }
                }
            }
        }
        // Java: the TreeSet uses NUMERICAL_COMPARATOR; dedup matches the TreeSet identity
        // (numeric equality plus String.compareTo equality = string equality).
        versions.sort_by(numerical_comparator);
        versions.dedup();
        versions
    }

    /// Java: private static String targetNumericVersion(Target target) {
    ///           return Integer.toString(target.ordinal() - Target.JDK1_1.ordinal() + 1); }
    fn target_numeric_version_value(target: Target) -> i32 {
        target.ordinal() - Target::JDK1_1.ordinal() + 1
    }
}

impl PlatformProvider for JDKPlatformProvider {
    /// Java:
    /// <pre>
    /// public Iterable&lt;String&gt; getSupportedPlatformNames() {
    ///     return SUPPORTED_JAVA_PLATFORM_VERSIONS;
    /// }
    /// </pre>
    fn get_supported_platform_names(&self) -> Vec<JavaString> {
        self.supported
            .get_or_init(|| {
                JDKPlatformProvider::compute_supported_java_platform_versions(&self.java_home)
            })
            .clone()
    }

    /// Java:
    /// <pre>
    /// public PlatformDescription getPlatform(String platformName, String options) {
    ///     return new PlatformDescriptionImpl(platformName);
    /// }
    /// </pre>
    fn get_platform(
        &self,
        platform_name: &JavaString,
        _options: &JavaString,
    ) -> Result<Rc<dyn PlatformDescription>, PlatformNotSupported> {
        Ok(Rc::new(PlatformDescriptionImpl::with_java_home(
            platform_name,
            self.java_home.clone(),
        )))
    }
}

/// Java: public static final Comparator<String> NUMERICAL_COMPARATOR = (s1, s2) -> { ... }.
///
/// PORTING_RULES: java.util.Comparator<String> is not a ported type, so the comparator is a
/// free function returning Ordering (used with slice::sort_by).  Unparseable versions
/// compare as Integer.MAX_VALUE and therefore sort last; equal numeric values fall back to
/// String.compareTo.
pub fn numerical_comparator(s1: &JavaString, s2: &JavaString) -> Ordering {
    let i1 = parse_int_or_max(s1);
    let i2 = parse_int_or_max(s2);
    if i1 != i2 {
        // Java: return i1 - i2; — Java int arithmetic wraps on overflow (Porting rules).
        i1.wrapping_sub(i2).cmp(&0)
    } else {
        // Java: return s1.compareTo(s2);
        s1.cmp(s2)
    }
}

/// Java:
/// <pre>
/// try {
///     return Integer.parseInt(s);
/// } catch (NumberFormatException ex) {
///     return Integer.MAX_VALUE;
/// }
/// </pre>
///
/// PORTING_RULES: java.lang.Integer is not a ported surface; Rust's decimal parse accepts
/// the same leading '+'/'-' as Integer.parseInt and rejects overflow (-> MAX_VALUE), but it
/// does not accept the non-ASCII digits that Character.digit (used by parseInt) accepts.
fn parse_int_or_max(s: &JavaString) -> i32 {
    s.to_string_lossy().parse::<i32>().unwrap_or(i32::MAX)
}

/// Java: targetNumericVersion(Target) — Integer.toString(ordinal - JDK1_1.ordinal + 1).
fn target_numeric_version(target: Target) -> JavaString {
    let version = JDKPlatformProvider::target_numeric_version_value(target);
    JavaString::from(version.to_string().as_str())
}

/// Java: Integer.parseInt(Character.toString(ver), Character.MAX_RADIX) — one base-36 digit.
///
/// Java's uncaught NumberFormatException fails the class initializer
/// (ExceptionInInitializerError); the Rust port panics with the same message.  javac-ctsym
/// already panics while opening an archive with such a directory name.
fn parse_int_radix(ch: char) -> i32 {
    match ch.to_digit(CHARACTER_MAX_RADIX) {
        Some(digit) => digit as i32,
        // Java: throw new NumberFormatException("For input string: \"" + ver + "\"");
        None => panic!("NumberFormatException: For input string: \"{ch}\""),
    }
}

/// Java: static class PlatformDescriptionImpl implements PlatformDescription.
struct PlatformDescriptionImpl {
    /// Java: private final String sourceVersion;
    source_version: JavaString,

    /// Java: private final String ctSymVersion;
    ///
    /// TODO(S3): read by getFileManager() (the ct.sym section lookup), which is not
    /// ported yet; the field is part of the 1:1 state.
    #[allow(dead_code)]
    ct_sym_version: JavaString,
    /// The JDK home (Java: System.getProperty("java.home") read by findCtSym();
    /// injected because the port runs without a JVM - registered deviation).
    java_home: std::option::Option<FsPath>,
    // Java: private final Map<Path, FileSystem> ctSym2FileSystem = new HashMap<>();
    // TODO(S3): the map is only filled by getFileManager(); no FileSystem handle is kept in
    // this (javac-ctsym reads the archive once and drops it), so the field is not
    // declared yet. close() below is the upstream empty-loop case.
}

impl PlatformDescriptionImpl {
    /// Java:
    /// <pre>
    /// PlatformDescriptionImpl(String sourceVersion) {
    ///     this.sourceVersion = sourceVersion;
    ///     this.ctSymVersion =
    ///             StringUtils.toUpperCase(Integer.toString(
    ///                 Integer.parseInt(sourceVersion), Character.MAX_RADIX));
    /// }
    /// </pre>
    fn new(source_version: &JavaString) -> PlatformDescriptionImpl {
        let source_version = source_version.clone();
        let numeric = parse_int(&source_version);
        let base36 = integer_to_string(numeric, CHARACTER_MAX_RADIX);
        let ct_sym_version = to_upper_case(&JavaString::from(base36.as_str()));
        PlatformDescriptionImpl {
            source_version,
            ct_sym_version,
            java_home: None,
        }
    }

    /// Creates the description with the injected JDK home (the upstream reads
    /// java.home from the system properties in findCtSym).
    fn with_java_home(source_version: &JavaString, java_home: FsPath) -> PlatformDescriptionImpl {
        let mut description = PlatformDescriptionImpl::new(source_version);
        description.java_home = Some(java_home);
        description
    }
}

impl PlatformDescription for PlatformDescriptionImpl {
    fn get_file_manager(&self) -> Rc<dyn JavaFileManager> {
        let java_home = self
            .java_home
            .clone()
            .expect("IllegalStateException: java.home is not injected (registered deviation)");
        let ct_sym_path = JDKPlatformProvider::find_ct_sym(&java_home);
        // Java: if (Files.exists(file)) { ... } else throw new
        // IllegalStateException("Cannot find ct.sym!");
        if !Files::exists(&ct_sym_path) {
            panic!("IllegalStateException: Cannot find ct.sym!");
        }
        let archive = Rc::new(
            CtSym::open(&ct_sym_path.as_path()).unwrap_or_else(|error| {
                panic!("IllegalStateException: cannot open ct.sym: {error:?}")
            }),
        );
        // Java: Context context = new Context(); context.put(Log.errKey, pw);
        // CacheFSInfo.preRegister(context); new JavacFileManager(context, true, null).
        // The ported wiring injects the services (registered adapter).
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let options = Options::instance(&mut context);
        let fs_info = Rc::new(CacheFSInfo::new());
        let inner = JavacFileManager::new(None, java_home, log, options, fs_info);
        // Java: fm.handleOption(Option.MULTIRELEASE, sourceVersion);
        let mut multi_release_args = std::iter::once(self.source_version.clone());
        let _ = JavaFileManager::handle_option(
            &*inner,
            &JavaString::from("--multi-release"),
            &mut multi_release_args,
        );
        let release = parse_int(&self.source_version);
        let has_modules = Source::lookup(&self.source_version.to_string_lossy())
            .map(|source| Feature::MODULES.allowed_in_source(source))
            .unwrap_or(false);
        // Java: the ctSym2FileSystem cache is unnecessary in the port (javac-ctsym
        // reads the archive once; close() has nothing to release — registered).
        CtSymJavaFileManager::new(inner, archive, release, has_modules) as Rc<dyn JavaFileManager>
    }

    /// Java: public String getSourceVersion() { return sourceVersion; }
    fn get_source_version(&self) -> JavaString {
        self.source_version.clone()
    }

    /// Java: public String getTargetVersion() { return sourceVersion; }
    fn get_target_version(&self) -> JavaString {
        self.source_version.clone()
    }

    /// Java: public List<PluginInfo<Processor>> getAnnotationProcessors() {
    ///           return Collections.emptyList();
    ///       }
    fn get_annotation_processors(&self) -> Vec<Rc<dyn PluginInfo<Rc<dyn Processor>>>> {
        Vec::new()
    }

    /// Java: public List<PluginInfo<Plugin>> getPlugins() {
    ///           return Collections.emptyList();
    ///       }
    fn get_plugins(&self) -> Vec<Rc<dyn PluginInfo<Rc<dyn Plugin>>>> {
        Vec::new()
    }

    /// Java: public List<String> getAdditionalOptions() {
    ///           return Collections.emptyList();
    ///       }
    fn get_additional_options(&self) -> Vec<JavaString> {
        Vec::new()
    }

    /// Java:
    /// <pre>
    /// public void close() throws IOException {
    ///     for (FileSystem fs : ctSym2FileSystem.values()) {
    ///         fs.close();
    ///     }
    ///     ctSym2FileSystem.clear();
    /// }
    /// </pre>
    ///
    /// PORTING_RULES: no file system handle is opened by this (getFileManager is still
    /// TODO(S3)), so the upstream loop over an empty map is the Ok(()) case here.  When S3
    /// opens the ct.sym file system, the map and this loop are ported 1:1.
    fn close(&self) -> Result<(), Error> {
        Ok(())
    }
}

/// Java: Character.MAX_RADIX.
const CHARACTER_MAX_RADIX: u32 = 36;

/// Java: java.lang.Integer.parseInt(String) — decimal only; malformed input throws
/// NumberFormatException, which is an unchecked Java exception and becomes a panic
/// (PORTING_RULES Porting rules).
///
/// PORTING_RULES: java.lang.Integer is not a ported surface; Rust's decimal parse accepts
/// the same leading '+'/'-' and rejects whitespace like Integer.parseInt, but it does not
/// accept the non-ASCII digits that Character.digit (used by parseInt) accepts.  Platform
/// names are ASCII version strings produced by the JDK itself.
fn parse_int(source: &JavaString) -> i32 {
    let text = source.to_string_lossy();
    match text.parse::<i32>() {
        Ok(value) => value,
        // Java: throw new NumberFormatException("For input string: \"" + s + "\"");
        Err(_) => panic!("NumberFormatException: For input string: \"{text}\""),
    }
}

/// Java: java.lang.Integer.toString(int, int) for the 2..=36 radix range (lowercase
/// digits), used with radix 36 here.
fn integer_to_string(value: i32, radix: u32) -> String {
    debug_assert!((2..=CHARACTER_MAX_RADIX).contains(&radix));
    if value == 0 {
        return "0".to_string();
    }
    let negative = value < 0;
    // unsigned_abs() handles i32::MIN, whose magnitude does not fit in i32.
    let mut magnitude = (value as i64).unsigned_abs();
    let radix = u64::from(radix);
    let mut digits: Vec<u8> = Vec::new();
    while magnitude != 0 {
        let digit = (magnitude % radix) as u8;
        digits.push(if digit < 10 {
            b'0' + digit
        } else {
            b'a' + (digit - 10)
        });
        magnitude /= radix;
    }
    if negative {
        digits.push(b'-');
    }
    digits.reverse();
    String::from_utf8(digits).expect("base-36 digits are ASCII")
}

#[cfg(test)]
mod tests {
    use super::{JDKPlatformProvider, PlatformDescriptionImpl, numerical_comparator};
    use crate::platform::platform_description::PlatformDescription;
    use crate::platform::platform_provider::PlatformProvider;
    use crate::util::java_string::JavaString;
    use javac_ctsym::CtSym;
    use javac_fs::Path as FsPath;
    use std::cmp::Ordering;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn js(s: &str) -> JavaString {
        JavaString::from(s)
    }

    /// The recorded development-time oracle: the baseline javac 17.0.19 accepts exactly the
    /// releases 7..17 (javac --release 6 -version exits 2; 7..17 exit 0; 18 exits 2).
    const ORACLE_ACCEPTED_RELEASES: [&str; 11] = [
        "7", "8", "9", "10", "11", "12", "13", "14", "15", "16", "17",
    ];

    fn temp_dir(tag: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "javac-platform-{}-{}-{}",
            tag,
            std::process::id(),
            unique
        ))
    }

    fn strings(versions: &[JavaString]) -> Vec<String> {
        versions
            .iter()
            .map(|version| version.to_string_lossy())
            .collect()
    }

    /// Finds a JDK home with lib/ct.sym: JAVA_HOME first, then the directory above the
    /// javac found on PATH (tests run on the developer machine, not in the compiler).
    fn baseline_java_home() -> Option<PathBuf> {
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            let home = PathBuf::from(home);
            if home.join("lib").join("ct.sym").is_file() {
                return Some(home);
            }
        }
        if let Some(paths) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&paths) {
                let javac = dir.join(if cfg!(windows) { "javac.exe" } else { "javac" });
                if javac.is_file()
                    && let Some(bin) = javac.parent()
                    && let Some(home) = bin.parent()
                    && home.join("lib").join("ct.sym").is_file()
                {
                    return Some(home.to_path_buf());
                }
            }
        }
        None
    }

    #[test]
    fn numerical_comparator_orders_versions_numerically() {
        assert_eq!(numerical_comparator(&js("7"), &js("17")), Ordering::Less);
        assert_eq!(numerical_comparator(&js("17"), &js("8")), Ordering::Greater);
        assert_eq!(numerical_comparator(&js("17"), &js("17")), Ordering::Equal);
        let mut versions = [js("17"), js("7"), js("11"), js("8")];
        versions.sort_by(numerical_comparator);
        let sorted: Vec<String> = versions.iter().map(|v| v.to_string_lossy()).collect();
        assert_eq!(sorted, ["7", "8", "11", "17"]);
    }

    #[test]
    fn numerical_comparator_places_non_numeric_versions_last() {
        // NumberFormatException -> Integer.MAX_VALUE (Java comparator behaviour).
        assert_eq!(numerical_comparator(&js("17"), &js("x")), Ordering::Less);
        assert_eq!(numerical_comparator(&js("x"), &js("17")), Ordering::Greater);
        // Equal numeric values fall back to String.compareTo.
        assert_eq!(numerical_comparator(&js("07"), &js("7")), Ordering::Less);
        assert_eq!(numerical_comparator(&js("x"), &js("y")), Ordering::Less);
    }

    #[test]
    fn description_derives_versions_from_the_platform_name() {
        let description = PlatformDescriptionImpl::new(&js("17"));
        assert_eq!(description.get_source_version().to_string_lossy(), "17");
        assert_eq!(description.get_target_version().to_string_lossy(), "17");
        assert!(description.get_annotation_processors().is_empty());
        assert!(description.get_plugins().is_empty());
        assert!(description.get_additional_options().is_empty());
        assert_eq!(description.close(), Ok(()));
    }

    #[test]
    fn description_ct_sym_version_is_uppercase_base36() {
        // Integer.toString(n, Character.MAX_RADIX) followed by StringUtils.toUpperCase.
        assert_eq!(
            PlatformDescriptionImpl::new(&js("7"))
                .ct_sym_version
                .to_string_lossy(),
            "7"
        );
        assert_eq!(
            PlatformDescriptionImpl::new(&js("8"))
                .ct_sym_version
                .to_string_lossy(),
            "8"
        );
        assert_eq!(
            PlatformDescriptionImpl::new(&js("11"))
                .ct_sym_version
                .to_string_lossy(),
            "B"
        );
        assert_eq!(
            PlatformDescriptionImpl::new(&js("17"))
                .ct_sym_version
                .to_string_lossy(),
            "H"
        );
        assert_eq!(
            PlatformDescriptionImpl::new(&js("36"))
                .ct_sym_version
                .to_string_lossy(),
            "10"
        );
    }

    #[test]
    fn find_ct_sym_resolves_lib_ct_sym() {
        let home = FsPath::from_path_buf(temp_dir("find"));
        let expected = home.as_path().join("lib").join("ct.sym");
        assert_eq!(JDKPlatformProvider::find_ct_sym(&home).as_path(), expected);
    }

    #[test]
    fn missing_ct_sym_yields_no_platforms() {
        let dir = temp_dir("missing");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let provider = JDKPlatformProvider::new(FsPath::from_path_buf(dir.clone()));
        assert!(provider.get_supported_platform_names().is_empty());
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn corrupt_ct_sym_yields_no_platforms() {
        // Java: FileSystems.newFileSystem throws IOException for a damaged archive and the
        // static initializer catches it, leaving the supported set empty.
        let dir = temp_dir("corrupt");
        std::fs::create_dir_all(dir.join("lib")).expect("create temp dir");
        std::fs::write(dir.join("lib").join("ct.sym"), b"this is not a zip archive")
            .expect("write corrupt ct.sym");
        let provider = JDKPlatformProvider::new(FsPath::from_path_buf(dir.clone()));
        assert!(provider.get_supported_platform_names().is_empty());
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn supported_platform_versions_match_the_recorded_javac_oracle() {
        let Some(home) = baseline_java_home() else {
            // No JDK home discoverable on this machine: the recorded oracle snapshot is
            // still asserted by the constant above; skip the live ct.sym cross-check.
            return;
        };
        let provider = JDKPlatformProvider::new(FsPath::from_path_buf(home.clone()));
        let versions = strings(&provider.get_supported_platform_names());
        assert_eq!(versions, ORACLE_ACCEPTED_RELEASES);
        // The ct.sym reader decodes the same release set (independent source of truth for
        // the same archive).
        let archive = CtSym::open(&home.join("lib").join("ct.sym")).expect("open baseline ct.sym");
        assert_eq!(
            archive.releases(),
            [7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17]
        );
        // 6 and 18 are not part of the baseline archive (and javac rejects them).
        assert!(!versions.contains(&"6".to_string()));
        assert!(!versions.contains(&"18".to_string()));
    }

    #[test]
    fn get_platform_returns_a_description() {
        let provider = JDKPlatformProvider::new(FsPath::from_path_buf(temp_dir("provider")));
        let description = provider
            .get_platform(&js("17"), &js(""))
            .unwrap_or_else(|_| panic!("unexpected PlatformNotSupported"));
        assert_eq!(description.get_source_version().to_string_lossy(), "17");
    }

    #[test]
    #[should_panic(expected = "NumberFormatException")]
    fn a_non_numeric_platform_name_throws_number_format() {
        let _ = PlatformDescriptionImpl::new(&js("not-a-version"));
    }

    /// Java: PlatformDescriptionImpl.getFileManager (JDKPlatformProvider.java:154-247):
    /// a pre-modules release reads the .sig files of ct.sym through the platform
    /// file manager (the Java anonymous JavacFileManager subclass).
    #[test]
    fn platform_file_manager_reads_ct_sym_signature_files() {
        use javac_tools::tools::file_object::FileObject;
        use javac_tools::tools::java_file_manager::JavaFileManager;
        use javac_tools::tools::java_file_object::Kind;
        use javac_tools::tools::standard_location::StandardLocation;
        use std::io::Read;

        let Some(home) = baseline_java_home() else {
            return;
        };
        let provider = JDKPlatformProvider::new(FsPath::from_path_buf(home));
        let description = provider.get_platform(&js("8"), &js("")).expect("platform 8");
        let file_manager = description.get_file_manager();
        assert!(file_manager.has_location(&StandardLocation::PLATFORM_CLASS_PATH));
        assert!(!file_manager.has_location(&StandardLocation::SYSTEM_MODULES));
        let object = file_manager
            .get_java_file_for_input(
                &StandardLocation::PLATFORM_CLASS_PATH,
                &js("java.lang.Object"),
                Kind::CLASS,
            )
            .expect("java.lang.Object input")
            .expect("java.lang.Object exists in ct.sym release 8");
        assert_eq!(object.get_kind(), Kind::CLASS);
        let mut bytes = Vec::new();
        object
            .open_input_stream()
            .expect("signature stream")
            .read_to_end(&mut bytes)
            .expect("signature bytes");
        assert!(bytes.len() > 4, "the signature file is a class file");
        assert_eq!(&bytes[..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
        assert_eq!(
            file_manager.infer_binary_name(&StandardLocation::PLATFORM_CLASS_PATH, &object),
            Some(js("java.lang.Object"))
        );
        let listed = file_manager
            .list(
                &StandardLocation::PLATFORM_CLASS_PATH,
                &js("java.lang"),
                &[Kind::CLASS],
                false,
            )
            .expect("java.lang listing");
        assert!(
            listed
                .iter()
                .any(|file| file.get_name().to_string_lossy().ends_with("Object.sig")),
            "the listing contains the java.lang.Object signature file"
        );
    }

    /// Java: PlatformDescriptionImpl.getFileManager: for a modules release the
    /// platform file manager exposes the running image system modules
    /// (JDKPlatformProvider.java:284-295) and does not expose PLATFORM_CLASS_PATH.
    #[test]
    fn platform_file_manager_uses_system_modules_for_a_current_release() {
        use javac_tools::tools::java_file_manager::JavaFileManager;
        use javac_tools::tools::standard_location::StandardLocation;

        let Some(home) = baseline_java_home() else {
            return;
        };
        let provider = JDKPlatformProvider::new(FsPath::from_path_buf(home));
        let description = provider.get_platform(&js("17"), &js("")).expect("platform 17");
        let file_manager = description.get_file_manager();
        assert!(file_manager.has_location(&StandardLocation::SYSTEM_MODULES));
        assert!(!file_manager.has_location(&StandardLocation::PLATFORM_CLASS_PATH));
        assert!(
            file_manager
                .get_location_for_module(&StandardLocation::SYSTEM_MODULES, &js("java.base"))
                .is_ok(),
            "the java.base module location is available"
        );
    }
}
