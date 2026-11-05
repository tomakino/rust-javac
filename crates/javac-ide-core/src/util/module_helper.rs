//! Module export helper.
//!
//!
//! - the upstream class only uses java.lang.Module (a JDK platform class that
//!   exists only in a running VM and is not part of the javac reference tree);
//!   addExports(Module, Module) is therefore NOT PORTED (REGISTERED, no substitute
//!   API is introduced; registered in );
//! - the private javacInternalPackages constant array is fully ported and
//!   exposed as javac_internal_packages() for the module-layer integration
//!   (it is the only data in the class).

/// Java: private static final String[] javacInternalPackages
pub const JAVAC_INTERNAL_PACKAGES: [&str; 13] = [
    "com.sun.tools.javac.api",
    "com.sun.tools.javac.code",
    "com.sun.tools.javac.comp",
    "com.sun.tools.javac.file",
    "com.sun.tools.javac.jvm",
    "com.sun.tools.javac.main",
    "com.sun.tools.javac.model",
    "com.sun.tools.javac.parser",
    "com.sun.tools.javac.platform",
    "com.sun.tools.javac.processing",
    "com.sun.tools.javac.tree",
    "com.sun.tools.javac.util",
    "com.sun.tools.doclint",
];

/// The package names exported by javac's ModuleHelper.addExports
/// (Java: private static final String[] javacInternalPackages).
pub fn javac_internal_packages() -> &'static [&'static str] {
    &JAVAC_INTERNAL_PACKAGES
}

// Java: public static void addExports(Module from, Module to)
//
// NOT PORTED: java.lang.Module is a JDK platform type, not part of the javac
// reference sources and not available outside a running VM; the module-layer
// ports it together with the java.lang.Module interop (registered
// section 9). No substitute helper is introduced.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_packages_match_the_upstream_array() {
        assert_eq!(javac_internal_packages().len(), 13);
        assert_eq!(javac_internal_packages()[0], "com.sun.tools.javac.api");
        assert_eq!(javac_internal_packages()[11], "com.sun.tools.javac.util");
        assert_eq!(javac_internal_packages()[12], "com.sun.tools.doclint");
        // The array is ordered and contains no duplicates.
        let mut sorted = javac_internal_packages().to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), javac_internal_packages().len());
    }
}
