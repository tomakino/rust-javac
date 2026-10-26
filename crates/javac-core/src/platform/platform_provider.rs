// Origin: com/sun/tools/javac/platform/PlatformProvider.java @ 982d53c (jdk-17.0.19-ga)

//! A collection of platform descriptions that can be selected using {@code --release name}
//! command line option.
//! Register in {@code META-INF/services/com.sun.tools.javac.platform.PlatformProvider}.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES (Porting rules/Porting rules):
//! * Java interface -> Rust trait (used as `dyn PlatformProvider`; the providers are
//!   shared references, `Rc<dyn PlatformProvider>`, because the Java API hands out
//!   the same provider instance from the service loader).
//! * `Iterable<String>` -> `Vec<JavaString>` (Porting rules Iterable mapping; the upstream
//!   consumer only tests membership, the order is kept as returned).
//! * the nested Java class `PlatformProvider.PlatformNotSupported extends Exception`
//!   -> struct `PlatformNotSupported` with Display and std::error::Error; a Rust trait
//!   cannot hold nested types, so the class is declared at module level.
//! * `getPlatform(...) throws PlatformNotSupported` -> `Result<_, PlatformNotSupported>`
//!   (checked-exception mapping, Porting rules): PlatformUtils catches it and returns null exactly as the
//!   Java stream code does.  There is no message in the upstream exception.

use crate::platform::platform_description::PlatformDescription;
use crate::util::java_string::JavaString;
use std::fmt;
use std::rc::Rc;

/// A collection of platform descriptions that can be selected using {@code --release name}
/// command line option.
/// Register in {@code META-INF/services/com.sun.tools.javac.platform.PlatformProvider}.
///
/// <p><b>This is NOT part of any supported API.
/// If you write code that depends on this, you do so at your own risk.
/// This code and its internal interfaces are subject to change or
/// deletion without notice.</b>
pub trait PlatformProvider {
    /// Names of platforms supported by this provider. Each returned name can be used as the key for --release.
    ///
    /// @return the platform keys
    fn get_supported_platform_names(&self) -> Vec<JavaString>;

    /// Create a description of a selected platform.
    ///
    /// @param platform_name the name of the selected platform
    /// @param options additional parameter, which can be specified after ':' on the command line
    /// @return a PlatformDescription
    /// @throws PlatformNotSupported if the given platform is not supported
    fn get_platform(
        &self,
        platform_name: &JavaString,
        options: &JavaString,
    ) -> Result<Rc<dyn PlatformDescription>, PlatformNotSupported>;
}

/// Throw if the given platform is not supported.
///
/// (Java: the nested class {@code PlatformProvider.PlatformNotSupported extends Exception};
/// Rust traits cannot hold nested types, so it is declared at module level.  The upstream
/// exception carries no message and no cause.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformNotSupported;

impl fmt::Display for PlatformNotSupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PlatformNotSupported")
    }
}

impl std::error::Error for PlatformNotSupported {}

#[cfg(test)]
mod tests {
    use super::{PlatformNotSupported, PlatformProvider};
    use crate::platform::platform_description::{PlatformDescription, PluginInfo};
    use crate::util::java_string::JavaString;
    use javac_source::util::plugin::Plugin;
    use javac_tools::error::Error;
    use javac_tools::jdk::Processor;
    use javac_tools::tools::java_file_manager::JavaFileManager;
    use std::rc::Rc;

    struct NoopDescription;

    impl PlatformDescription for NoopDescription {
        fn get_file_manager(&self) -> Rc<dyn JavaFileManager> {
            panic!("UnsupportedOperationException: get_file_manager (test fixture capacity)")
        }
        fn get_source_version(&self) -> JavaString {
            JavaString::from("17")
        }
        fn get_target_version(&self) -> JavaString {
            JavaString::from("17")
        }
        fn get_annotation_processors(&self) -> Vec<Rc<dyn PluginInfo<Rc<dyn Processor>>>> {
            Vec::new()
        }
        fn get_plugins(&self) -> Vec<Rc<dyn PluginInfo<Rc<dyn Plugin>>>> {
            Vec::new()
        }
        fn get_additional_options(&self) -> Vec<JavaString> {
            Vec::new()
        }
        fn close(&self) -> Result<(), Error> {
            Ok(())
        }
    }

    struct TestProvider {
        supported: bool,
    }

    impl PlatformProvider for TestProvider {
        fn get_supported_platform_names(&self) -> Vec<JavaString> {
            vec![JavaString::from("17")]
        }
        fn get_platform(
            &self,
            _platform_name: &JavaString,
            _options: &JavaString,
        ) -> Result<Rc<dyn PlatformDescription>, PlatformNotSupported> {
            if self.supported {
                Ok(Rc::new(NoopDescription))
            } else {
                Err(PlatformNotSupported)
            }
        }
    }

    #[test]
    fn platform_provider_is_object_safe() {
        let provider: Rc<dyn PlatformProvider> = Rc::new(TestProvider { supported: true });
        assert_eq!(
            provider.get_supported_platform_names(),
            vec![JavaString::from("17")]
        );
        let description = provider
            .get_platform(&JavaString::from("17"), &JavaString::new())
            .unwrap_or_else(|_| panic!("unexpected PlatformNotSupported"));
        assert_eq!(description.get_source_version().to_string_lossy(), "17");
    }

    #[test]
    fn platform_not_supported_is_the_declared_failure() {
        let provider: Rc<dyn PlatformProvider> = Rc::new(TestProvider { supported: false });
        match provider.get_platform(&JavaString::from("17"), &JavaString::new()) {
            Ok(_) => panic!("unexpected PlatformDescription"),
            Err(error) => assert_eq!(error, PlatformNotSupported),
        }
        assert_eq!(PlatformNotSupported.to_string(), "PlatformNotSupported");
    }
}
