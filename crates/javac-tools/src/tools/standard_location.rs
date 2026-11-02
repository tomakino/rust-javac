//! Standard locations of file objects.

use super::java_file_manager::Location;
use javac_strings::JavaString;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Standard locations of file objects.
///
/// @author Peter von der Ah&eacute;
/// @since 1.6
///
///
/// * implements JavaFileManager.Location -> impl Location for StandardLocation;
///   the Location trait is defined in java_file_manager (the Java nested
///   interface JavaFileManager.Location).
/// * locationFor returns Rc of dyn Location and caches the instances exactly
///   like the upstream static ConcurrentMap.  The cache is a
///   thread_local!{ RefCell<HashMap<..>> } because Rc is not Send; the JDK
///   cache is shared between threads.  The ported compiler is single-threaded,
///   the observable locationFor(x) == locationFor(y) identity contract is
///   preserved per thread (registered difference).
/// * Java enum constants keep their Java names (UPPER_SNAKE_CASE).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardLocation {
    /// Location of new class files.
    CLASS_OUTPUT,

    /// Location of new source files.
    SOURCE_OUTPUT,

    /// Location to search for user class files.
    CLASS_PATH,

    /// Location to search for existing source files.
    SOURCE_PATH,

    /// Location to search for annotation processors.
    ANNOTATION_PROCESSOR_PATH,

    /// Location to search for modules containing annotation processors.
    /// @since 9
    ANNOTATION_PROCESSOR_MODULE_PATH,

    /// Location to search for platform classes.  Sometimes called
    /// the boot class path.
    PLATFORM_CLASS_PATH,

    /// Location of new native header files.
    /// @since 1.8
    NATIVE_HEADER_OUTPUT,

    /// Location to search for the source code of modules.
    /// @since 9
    MODULE_SOURCE_PATH,

    /// Location to search for upgradeable system modules.
    /// @since 9
    UPGRADE_MODULE_PATH,

    /// Location to search for system modules.
    /// @since 9
    SYSTEM_MODULES,

    /// Location to search for precompiled user modules.
    /// @since 9
    MODULE_PATH,

    /// Location to search for module patches.
    /// @since 9
    PATCH_MODULE_PATH,
}

/// The anonymous Location implementation created by
/// StandardLocation.locationFor for a name that is not one of the enum
/// constants (upstream: new Location() { ... }).
pub struct AnonymousLocation {
    name: JavaString,
}

impl AnonymousLocation {
    fn new(name: JavaString) -> AnonymousLocation {
        AnonymousLocation { name }
    }
}

impl Location for AnonymousLocation {
    // Java: public String getName() { return name; }
    fn get_name(&self) -> JavaString {
        self.name.clone()
    }

    // Java: public boolean isOutputLocation() {
    //           return name.endsWith("_OUTPUT");
    //       }
    fn is_output_location(&self) -> bool {
        self.name.ends_with(&JavaString::from("_OUTPUT"))
    }

    // Java: the anonymous class does not override isModuleOrientedLocation,
    // so the interface default method is used.
}

thread_local! {
    // Java: private static final ConcurrentMap<String,Location> locations
    //           = new ConcurrentHashMap<>();
    static LOCATIONS: RefCell<HashMap<JavaString, Rc<dyn Location>>> =
        RefCell::new(HashMap::new());
}

impl StandardLocation {
    /// Returns a location object with the given name.  The following
    /// property must hold: locationFor(x) == locationFor(y) if and only if
    /// x.equals(y).  The returned location will be an output location if and
    /// only if name ends with "_OUTPUT". It will be considered to be a
    /// module-oriented location if the name contains the word "MODULE".
    ///
    /// @param name a name
    /// @return a location
    ///
    /// @revised 9
    pub fn location_for(name: &JavaString) -> Rc<dyn Location> {
        LOCATIONS.with(|locations| {
            let mut locations = locations.borrow_mut();
            if locations.is_empty() {
                // can't use valueOf which throws IllegalArgumentException
                for location in StandardLocation::values() {
                    locations
                        .entry(location.get_name())
                        .or_insert_with(|| Rc::new(location));
                }
            }
            // Java: name.getClass(); /* null-check */
            // Rust: name is a JavaString, which cannot be null, so
            // the null check has no Rust form.
            locations
                .entry(name.clone())
                .or_insert_with(|| Rc::new(AnonymousLocation::new(name.clone())))
                .clone()
        })
    }

    /// Java: enum#name() (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            StandardLocation::CLASS_OUTPUT => "CLASS_OUTPUT",
            StandardLocation::SOURCE_OUTPUT => "SOURCE_OUTPUT",
            StandardLocation::CLASS_PATH => "CLASS_PATH",
            StandardLocation::SOURCE_PATH => "SOURCE_PATH",
            StandardLocation::ANNOTATION_PROCESSOR_PATH => "ANNOTATION_PROCESSOR_PATH",
            StandardLocation::ANNOTATION_PROCESSOR_MODULE_PATH => {
                "ANNOTATION_PROCESSOR_MODULE_PATH"
            }
            StandardLocation::PLATFORM_CLASS_PATH => "PLATFORM_CLASS_PATH",
            StandardLocation::NATIVE_HEADER_OUTPUT => "NATIVE_HEADER_OUTPUT",
            StandardLocation::MODULE_SOURCE_PATH => "MODULE_SOURCE_PATH",
            StandardLocation::UPGRADE_MODULE_PATH => "UPGRADE_MODULE_PATH",
            StandardLocation::SYSTEM_MODULES => "SYSTEM_MODULES",
            StandardLocation::MODULE_PATH => "MODULE_PATH",
            StandardLocation::PATCH_MODULE_PATH => "PATCH_MODULE_PATH",
        })
    }

    /// Java: enum#values().
    pub fn values() -> [StandardLocation; 13] {
        [
            StandardLocation::CLASS_OUTPUT,
            StandardLocation::SOURCE_OUTPUT,
            StandardLocation::CLASS_PATH,
            StandardLocation::SOURCE_PATH,
            StandardLocation::ANNOTATION_PROCESSOR_PATH,
            StandardLocation::ANNOTATION_PROCESSOR_MODULE_PATH,
            StandardLocation::PLATFORM_CLASS_PATH,
            StandardLocation::NATIVE_HEADER_OUTPUT,
            StandardLocation::MODULE_SOURCE_PATH,
            StandardLocation::UPGRADE_MODULE_PATH,
            StandardLocation::SYSTEM_MODULES,
            StandardLocation::MODULE_PATH,
            StandardLocation::PATCH_MODULE_PATH,
        ]
    }
}

impl Location for StandardLocation {
    // Java: public String getName() { return name(); }
    fn get_name(&self) -> JavaString {
        self.name()
    }

    // Java: switch (this) { case CLASS_OUTPUT: case SOURCE_OUTPUT:
    //       case NATIVE_HEADER_OUTPUT: return true; default: return false; }
    fn is_output_location(&self) -> bool {
        matches!(
            self,
            StandardLocation::CLASS_OUTPUT
                | StandardLocation::SOURCE_OUTPUT
                | StandardLocation::NATIVE_HEADER_OUTPUT
        )
    }

    /// {@inheritDoc}
    /// @since 9
    ///
    /// Java: switch (this) { case MODULE_SOURCE_PATH:
    ///       case ANNOTATION_PROCESSOR_MODULE_PATH: case UPGRADE_MODULE_PATH:
    ///       case SYSTEM_MODULES: case MODULE_PATH: case PATCH_MODULE_PATH:
    ///       return true; default: return false; }
    fn is_module_oriented_location(&self) -> bool {
        matches!(
            self,
            StandardLocation::MODULE_SOURCE_PATH
                | StandardLocation::ANNOTATION_PROCESSOR_MODULE_PATH
                | StandardLocation::UPGRADE_MODULE_PATH
                | StandardLocation::SYSTEM_MODULES
                | StandardLocation::MODULE_PATH
                | StandardLocation::PATCH_MODULE_PATH
        )
    }
}

impl std::fmt::Display for StandardLocation {
    /// Java: Enum.toString() = name().
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::StandardLocation;
    use crate::tools::java_file_manager::Location;
    use javac_strings::JavaString;
    use std::rc::Rc;

    fn js(s: &str) -> JavaString {
        JavaString::from(s)
    }

    #[test]
    fn output_locations() {
        assert!(StandardLocation::CLASS_OUTPUT.is_output_location());
        assert!(StandardLocation::SOURCE_OUTPUT.is_output_location());
        assert!(StandardLocation::NATIVE_HEADER_OUTPUT.is_output_location());
        assert!(!StandardLocation::CLASS_PATH.is_output_location());
    }

    #[test]
    fn module_oriented_locations() {
        for location in [
            StandardLocation::MODULE_SOURCE_PATH,
            StandardLocation::ANNOTATION_PROCESSOR_MODULE_PATH,
            StandardLocation::UPGRADE_MODULE_PATH,
            StandardLocation::SYSTEM_MODULES,
            StandardLocation::MODULE_PATH,
            StandardLocation::PATCH_MODULE_PATH,
        ] {
            assert!(location.is_module_oriented_location(), "{location}");
        }
        assert!(!StandardLocation::CLASS_PATH.is_module_oriented_location());
    }

    #[test]
    fn location_for_returns_cached_identity() {
        // Java contract: locationFor(x) == locationFor(y) iff x.equals(y).
        let a = StandardLocation::location_for(&js("CLASS_PATH"));
        let b = StandardLocation::location_for(&js("CLASS_PATH"));
        assert!(Rc::ptr_eq(&a, &b));
        assert_eq!(a.get_name(), js("CLASS_PATH"));
        assert!(!a.is_output_location());
        assert_eq!(
            a.is_module_oriented_location(),
            StandardLocation::CLASS_PATH.is_module_oriented_location()
        );
    }

    #[test]
    fn location_for_unknown_name() {
        let out = StandardLocation::location_for(&js("MY_OUTPUT"));
        assert!(out.is_output_location());
        assert_eq!(out.get_name(), js("MY_OUTPUT"));
        // Default isModuleOrientedLocation: getName().matches(module pattern)
        // requires the whole name to be "MODULE".
        assert!(!out.is_module_oriented_location());
        let module = StandardLocation::location_for(&js("MODULE"));
        assert!(module.is_module_oriented_location());
        assert!(Rc::ptr_eq(
            &module,
            &StandardLocation::location_for(&js("MODULE"))
        ));
    }

    #[test]
    fn standard_names_are_cached() {
        let a = StandardLocation::location_for(&js("CLASS_OUTPUT"));
        assert!(a.is_output_location());
        assert_eq!(a.get_name(), StandardLocation::CLASS_OUTPUT.name());
        assert!(Rc::ptr_eq(
            &a,
            &StandardLocation::location_for(&js("CLASS_OUTPUT"))
        ));
    }
}
