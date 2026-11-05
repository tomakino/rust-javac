//! The `--release` option handling of `Arguments.handleReleaseOptions`
//! (Arguments.java:286-330).
//!
//! Java:
//! <pre>
//! public boolean handleReleaseOptions(Predicate&lt;Iterable&lt;String&gt;&gt; additionalOptions) {
//!     String platformString = options.get(Option.RELEASE);
//!     checkOptionAllowed(platformString == null,
//!             option -&gt; reportDiag(Errors.ReleaseBootclasspathConflict(option)),
//!             Option.BOOT_CLASS_PATH, Option.XBOOTCLASSPATH, Option.XBOOTCLASSPATH_APPEND,
//!             Option.XBOOTCLASSPATH_PREPEND, Option.ENDORSEDDIRS, Option.DJAVA_ENDORSED_DIRS,
//!             Option.EXTDIRS, Option.DJAVA_EXT_DIRS, Option.SOURCE, Option.TARGET,
//!             Option.SYSTEM, Option.UPGRADE_MODULE_PATH);
//!     if (platformString != null) {
//!         PlatformDescription platformDescription =
//!                 PlatformUtils.lookupPlatformDescription(platformString);
//!         if (platformDescription == null) {
//!             reportDiag(Errors.UnsupportedReleaseVersion(platformString));
//!             return false;
//!         }
//!         options.put(Option.SOURCE, platformDescription.getSourceVersion());
//!         options.put(Option.TARGET, platformDescription.getTargetVersion());
//!         context.put(PlatformDescription.class, platformDescription);
//!         if (!additionalOptions.test(platformDescription.getAdditionalOptions()))
//!             return false;
//!         JavaFileManager platformFM = platformDescription.getFileManager();
//!         DelegatingJavaFileManager.installReleaseFileManager(context, platformFM, getFileManager());
//!     }
//!     return true;
//! }
//! </pre>
//!
//! The provider set is passed explicitly instead of using ServiceLoader (see
//! platform_utils.rs); the JDK provider is the ported JDKPlatformProvider, whose
//! supported platform names are the ct.sym release sections.
//!
//! © the adapter: the file manager installation is performed by
//! `JavacFileManager::install_release_file_manager`; this module only resolves
//! the description and the source/target versions.

use std::rc::Rc;

use javac_fs::Path as FsPath;
use javac_strings::JavaString;

use crate::platform::jdk_platform_provider::JDKPlatformProvider;
use crate::platform::platform_description::PlatformDescription;
use crate::platform::platform_provider::PlatformProvider;
use crate::platform::platform_utils::lookup_platform_description;

/// One resolved `--release` platform (Java: the PlatformDescription stored in
/// the Context plus the SOURCE/TARGET values put into the Options table).
pub struct ReleasePlatform {
    /// Java: platformDescription.getSourceVersion().
    pub source_version: JavaString,
    /// Java: platformDescription.getTargetVersion().
    pub target_version: JavaString,
    /// Java: context.put(PlatformDescription.class, platformDescription).
    pub description: Rc<dyn PlatformDescription>,
}

/// The failures reported by Arguments.handleReleaseOptions.
#[derive(Debug)]
pub enum ReleaseError {
    /// Java: Errors.ReleaseBootclasspathConflict(option) — the option name.
    Conflict(String),
    /// Java: Errors.UnsupportedReleaseVersion(value) — the option value.
    Unsupported(String),
}

/// Java: the option list of `checkOptionAllowed` in handleReleaseOptions
/// (Arguments.java:297-304), in the upstream order.  The port matches the
/// names as written on the command line because the trimmed Option constants
/// (boot class path, endorsed dirs, ext dirs) are not part of the ported
/// Option enum (see main/option.rs header).
/// The entries are (accepted spellings on the command line, the option name
/// javac reports).  javac reports Option.getName(), the primary name, so the
/// removed bootclasspath spellings keep their trailing colon
/// ("-Xbootclasspath/a:") and the boot class path is reported as
/// "--boot-class-path" even when the "-bootclasspath" alias was used.
const CONFLICTING_OPTIONS: &[(&[&str], &str)] = &[
    (&["--boot-class-path", "-bootclasspath"], "--boot-class-path"),
    (&["-Xbootclasspath"], "-Xbootclasspath:"),
    (&["-Xbootclasspath/a"], "-Xbootclasspath/a:"),
    (&["-Xbootclasspath/p"], "-Xbootclasspath/p:"),
    (&["-endorseddirs"], "-endorseddirs"),
    (&["-Djava.endorsed.dirs"], "-Djava.endorsed.dirs"),
    (&["-extdirs"], "-extdirs"),
    (&["-Djava.ext.dirs"], "-Djava.ext.dirs"),
    (&["--source", "-source"], "--source"),
    (&["--target", "-target"], "--target"),
    (&["--system"], "--system"),
    (&["--upgrade-module-path"], "--upgrade-module-path"),
];

/// Java: checkOptionAllowed(platformString == null, ..., options) — returns the
/// conflicting option names in the upstream order (empty when there is no
/// conflict).
///
/// The caller passes the option table as the (name, value) pairs collected from
/// the command line.
pub fn conflicting_options(options: &[(String, String)]) -> Vec<String> {
    if !options.iter().any(|(name, _)| name == "--release") {
        return Vec::new();
    }
    let mut conflicts = Vec::new();
    for (names, reported) in CONFLICTING_OPTIONS {
        if options
            .iter()
            .any(|(name, _)| names.iter().any(|candidate| candidate == name))
        {
            conflicts.push((*reported).to_string());
        }
    }
    conflicts
}

/// The JDK home used for the platform lookup (Java: System.getProperty("java.home")).
pub fn java_home() -> FsPath {
    super::mini_compiler::java_home()
}

/// Java: PlatformUtils.lookupPlatformDescription(platformString) with the JDK
/// provider, followed by the UnsupportedReleaseVersion check.
///
/// # Arguments
///   - value: the `--release` argument;
///   - java_home: the JDK home (System.getProperty("java.home") upstream).
///
/// # Returns
///   - the resolved platform description and its source/target versions.
///
/// # Panics
///   - none.
pub fn lookup_platform(value: &str, java_home: FsPath) -> Result<ReleasePlatform, ReleaseError> {
    let providers: Vec<Rc<dyn PlatformProvider>> = vec![Rc::new(JDKPlatformProvider::new(java_home))];
    let platform_string = JavaString::from(value);
    let description = lookup_platform_description(&providers, &platform_string)
        .ok_or_else(|| ReleaseError::Unsupported(value.to_string()))?;
    Ok(ReleasePlatform {
        source_version: description.get_source_version(),
        target_version: description.get_target_version(),
        description,
    })
}
