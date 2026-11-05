//! Internal utilities to work with PlatformDescriptions.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//!
//! * Java static method -> free function; `String` -> `&JavaString`;
//!   `String.indexOf(":")` uses `JavaString::index_of_char` (Porting rules UTF-16 code units)
//!   and `String.substring` maps to `JavaString::substring`.
//! * DEPENDENCY DEVIATION (P6): the Java implementation discovers the providers with
//!   `ServiceLoader.load(PlatformProvider.class, Arguments.class.getClassLoader())`.
//!   Neither `java.util.ServiceLoader` (the ported javac-tools type is an opaque
//!   placeholder: service loading / annotation processing is out of scope) nor
//!   `com.sun.tools.javac.main.Arguments` (the main package is not ported) exists, so
//!   the provider set is an explicit parameter.  P6 supplies it from the ported service
//!   registry; the selection algorithm below is 1:1 with the Java stream code.
//! * `StreamSupport.stream(...).filter(...).findFirst().flatMap(...).orElse(null)` ->
//!   Rust iterator `find` + `and_then` (java.util.stream is not a ported surface);
//!   `Optional<PlatformDescription>` -> `Option`; the null return -> `None`.

use crate::platform::platform_description::PlatformDescription;
use crate::platform::platform_provider::PlatformProvider;
use crate::util::java_string::JavaString;
use std::rc::Rc;

/// Java: {@code public static PlatformDescription lookupPlatformDescription(String platformString)}.
///
/// the provider set is an explicit parameter because the Java
/// `ServiceLoader` discovery is not ported (see the file header); P6 passes the
/// providers of the ported service registry.
pub fn lookup_platform_description(
    providers: &[Rc<dyn PlatformProvider>],
    platform_string: &JavaString,
) -> Option<Rc<dyn PlatformDescription>> {
    // Java: int separator = platformString.indexOf(":");
    let separator = platform_string.index_of_char(':' as u16, 0);
    // Java:
    // String platformProviderName =
    // separator != (-1) ? platformString.substring(0, separator) : platformString;
    let platform_provider_name = if separator != -1 {
        platform_string.substring(0, separator as usize)
    } else {
        platform_string.clone()
    };
    // Java:
    // String platformOptions =
    // separator != (-1) ? platformString.substring(separator + 1) : "";
    let platform_options = if separator != -1 {
        platform_string.substring(separator as usize + 1, platform_string.len())
    } else {
        JavaString::new()
    };

    // Java:
    // Iterable<PlatformProvider> providers =
    // ServiceLoader.load(PlatformProvider.class, Arguments.class.getClassLoader());
    // return StreamSupport.stream(providers.spliterator(), false)
    //.filter(provider -> StreamSupport.stream(provider.getSupportedPlatformNames()
    //.spliterator(), false)
    //.anyMatch(platformProviderName::equals))
    //.findFirst()
    //.flatMap(provider -> {
    // try {
    // return Optional.of(provider.getPlatform(platformProviderName, platformOptions));
    // } catch (PlatformNotSupported pns) {
    // return Optional.empty();
    // }
    // })
    //.orElse(null);
    providers
        .iter()
        .find(|provider| {
            provider
                .get_supported_platform_names()
                .iter()
                .any(|name| platform_provider_name.eq(name))
        })
        .and_then(|provider| {
            provider
                .get_platform(&platform_provider_name, &platform_options)
                .ok()
        })
}

#[cfg(test)]
mod tests {
    use super::lookup_platform_description;
    use crate::platform::platform_description::{PlatformDescription, PluginInfo};
    use crate::platform::platform_provider::{PlatformNotSupported, PlatformProvider};
    use crate::util::java_string::JavaString;
    use javac_source::util::plugin::Plugin;
    use javac_tools::error::Error;
    use javac_tools::jdk::Processor;
    use javac_tools::tools::java_file_manager::JavaFileManager;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn js(s: &str) -> JavaString {
        JavaString::from(s)
    }

    struct TestDescription {
        source_version: JavaString,
    }

    impl PlatformDescription for TestDescription {
        fn get_file_manager(&self) -> Rc<dyn JavaFileManager> {
            panic!("UnsupportedOperationException: get_file_manager (test fixture capacity)")
        }
        fn get_source_version(&self) -> JavaString {
            self.source_version.clone()
        }
        fn get_target_version(&self) -> JavaString {
            self.source_version.clone()
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
        names: Vec<JavaString>,
        supported: bool,
        calls: RefCell<Vec<(JavaString, JavaString)>>,
    }

    impl TestProvider {
        fn new(names: &[&str], supported: bool) -> TestProvider {
            TestProvider {
                names: names.iter().map(|n| JavaString::from(*n)).collect(),
                supported,
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl PlatformProvider for TestProvider {
        fn get_supported_platform_names(&self) -> Vec<JavaString> {
            self.names.clone()
        }
        fn get_platform(
            &self,
            platform_name: &JavaString,
            options: &JavaString,
        ) -> Result<Rc<dyn PlatformDescription>, PlatformNotSupported> {
            self.calls
                .borrow_mut()
                .push((platform_name.clone(), options.clone()));
            if self.supported {
                Ok(Rc::new(TestDescription {
                    source_version: platform_name.clone(),
                }))
            } else {
                Err(PlatformNotSupported)
            }
        }
    }

    #[test]
    fn lookup_returns_none_without_providers() {
        assert!(lookup_platform_description(&[], &js("17")).is_none());
    }

    #[test]
    fn lookup_selects_the_matching_provider_and_splits_options() {
        let provider = Rc::new(TestProvider::new(&["7", "17"], true));
        let providers: Vec<Rc<dyn PlatformProvider>> = vec![provider.clone()];
        let description =
            lookup_platform_description(&providers, &js("17:inline")).expect("description");
        assert_eq!(description.get_source_version().to_string_lossy(), "17");
        assert_eq!(
            provider.calls.borrow().clone(),
            vec![(js("17"), js("inline"))]
        );
    }

    #[test]
    fn lookup_passes_an_empty_options_string_without_a_separator() {
        let provider = Rc::new(TestProvider::new(&["17"], true));
        let providers: Vec<Rc<dyn PlatformProvider>> = vec![provider.clone()];
        let description = lookup_platform_description(&providers, &js("17")).expect("description");
        assert_eq!(description.get_source_version().to_string_lossy(), "17");
        assert_eq!(provider.calls.borrow().clone(), vec![(js("17"), js(""))]);
    }

    #[test]
    fn lookup_splits_at_the_first_colon_only() {
        let provider = Rc::new(TestProvider::new(&["17"], true));
        let providers: Vec<Rc<dyn PlatformProvider>> = vec![provider.clone()];
        let _ = lookup_platform_description(&providers, &js("17:a:b"));
        assert_eq!(provider.calls.borrow().clone(), vec![(js("17"), js("a:b"))]);
    }

    #[test]
    fn lookup_returns_none_when_no_provider_lists_the_name() {
        let provider = Rc::new(TestProvider::new(&["17"], true));
        let providers: Vec<Rc<dyn PlatformProvider>> = vec![provider.clone()];
        assert!(lookup_platform_description(&providers, &js("8")).is_none());
        assert!(provider.calls.borrow().is_empty());
    }

    #[test]
    fn lookup_skips_providers_that_do_not_list_the_name() {
        let other = Rc::new(TestProvider::new(&["7"], false));
        let matching = Rc::new(TestProvider::new(&["17"], true));
        let providers: Vec<Rc<dyn PlatformProvider>> = vec![other.clone(), matching.clone()];
        let description = lookup_platform_description(&providers, &js("17")).expect("description");
        assert_eq!(description.get_source_version().to_string_lossy(), "17");
        assert!(other.calls.borrow().is_empty());
        assert_eq!(matching.calls.borrow().len(), 1);
    }

    #[test]
    fn lookup_stops_at_the_first_matching_provider() {
        // Java: findFirst() selects one provider; a PlatformNotSupported from it
        // makes the whole lookup return null, the later providers are not asked.
        let rejecting = Rc::new(TestProvider::new(&["17"], false));
        let accepting = Rc::new(TestProvider::new(&["17"], true));
        let providers: Vec<Rc<dyn PlatformProvider>> = vec![rejecting.clone(), accepting.clone()];
        assert!(lookup_platform_description(&providers, &js("17")).is_none());
        assert_eq!(rejecting.calls.borrow().len(), 1);
        assert!(accepting.calls.borrow().is_empty());
    }

    #[test]
    fn lookup_matches_an_empty_platform_name_before_the_colon() {
        // Java: ":options" -> platformProviderName "" (no provider lists "" here).
        let provider = Rc::new(TestProvider::new(&["17"], true));
        let providers: Vec<Rc<dyn PlatformProvider>> = vec![provider.clone()];
        assert!(lookup_platform_description(&providers, &js(":options")).is_none());
        assert!(provider.calls.borrow().is_empty());
    }
}
