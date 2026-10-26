// Origin: com/sun/tools/javac/platform/PlatformDescription.java @ 982d53c (jdk-17.0.19-ga)

//! A description of settings needed for a particular {@code --release name} option.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES (Porting rules/Porting rules):
//! * Java interface -> Rust trait (used as `dyn PlatformDescription`).
//!   The Java `PlatformDescription extends Closeable`: java.io.Closeable is not a
//!   ported type, so the interface's own `close()` re-declaration is kept and the
//!   checked `IOException` maps to `Result<(), javac_tools::error::Error>` (Porting rules).
//! * `JavaFileManager` -> `Rc<dyn JavaFileManager>`; `String` -> `JavaString`.
//! * `PluginInfo<T>` is a Java nested interface; Rust traits cannot contain
//!   types, so it is the top-level trait `PluginInfo<T>` of this module.
//!   `List<PluginInfo<Processor>>` / `List<PluginInfo<Plugin>>` ->
//!   `Vec<Rc<dyn PluginInfo<Rc<dyn Processor>>>>` /
//!   `Vec<Rc<dyn PluginInfo<Rc<dyn Plugin>>>>`: the Java generic `T` (the
//!   plugin instance) is the shared-reference form of the ported trait object.
//! * `Map<String, String> getOptions()` -> `Vec<(JavaString, JavaString)>`
//!   (Porting rules Map mapping; the iteration order is the implementation's, as with the
//!   upstream Map).

use crate::util::java_string::JavaString;
use javac_source::util::plugin::Plugin;
use javac_tools::error::Error;
use javac_tools::jdk::Processor;
use javac_tools::tools::java_file_manager::JavaFileManager;
use std::rc::Rc;

/// A description of settings needed for a particular {@code --release name} option.
///
/// <p><b>This is NOT part of any supported API.
/// If you write code that depends on this, you do so at your own risk.
/// This code and its internal interfaces are subject to change or
/// deletion without notice.</b>
pub trait PlatformDescription {
    /// Java: JavaFileManager getFileManager();
    fn get_file_manager(&self) -> Rc<dyn JavaFileManager>;

    /// Returns the source version that should be selected.
    /// Equivalent to {@code -source N} on the command line.
    ///
    /// @return the required source version
    fn get_source_version(&self) -> JavaString;

    /// Returns the target version that should be selected.
    /// Equivalent to {@code -target N} on the command line.
    ///
    /// @return the required target version
    fn get_target_version(&self) -> JavaString;

    /// Returns the annotation Processors that should be enabled in the current javac run.
    ///
    /// @return annotation Processors
    fn get_annotation_processors(&self) -> Vec<Rc<dyn PluginInfo<Rc<dyn Processor>>>>;

    /// Returns the javac Plugins that should be enabled in the current javac run.
    ///
    /// @return javac Plugins
    fn get_plugins(&self) -> Vec<Rc<dyn PluginInfo<Rc<dyn Plugin>>>>;

    /// Returns the additional command line options that should be passed to javac.
    ///
    /// @return additional command line options
    fn get_additional_options(&self) -> Vec<JavaString>;

    /// Perform cleanup for this PlatformProvider - this instance won't be used after this method
    /// is called.
    ///
    /// @throws IOException if unexpected problems occur during closing this provider
    fn close(&self) -> Result<(), Error>;
}

/// Returns the description of annotation Processor or javac Plugin that should be enabled
/// in the current javac run.
///
/// (Java: the nested interface {@code PlatformDescription.PluginInfo<T>}; Rust traits
/// cannot hold nested types, so the interface is declared at module level.)
///
/// @param <T> either Processor or Plugin
pub trait PluginInfo<T> {
    /// Returns the name of the Processor or Plugin.
    ///
    /// @return name of the Processor or Plugin.
    fn get_name(&self) -> JavaString;

    /// Returns the options for the Processor or Plugin. For plugin, the map will be converted
    /// to a collection by concatenating the map's keys, the '=' sign and the map's values.
    ///
    /// PORTING_RULES: Java {@code Map<String, String>} -> {@code Vec<(JavaString, JavaString)>}
    /// (Porting rules); the iteration order is the one of the implementation.
    ///
    /// @return the options
    fn get_options(&self) -> Vec<(JavaString, JavaString)>;

    /// Returns the Processor or Plugin instance.
    ///
    /// @return the Processor or Plugin instance.
    fn get_plugin(&self) -> T;
}

#[cfg(test)]
mod tests {
    use super::{PlatformDescription, PluginInfo};
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

    struct ProcessorInfo;

    impl PluginInfo<Rc<dyn Processor>> for ProcessorInfo {
        fn get_name(&self) -> JavaString {
            JavaString::from("annotation-processor")
        }
        fn get_options(&self) -> Vec<(JavaString, JavaString)> {
            vec![(JavaString::from("key"), JavaString::from("value"))]
        }
        fn get_plugin(&self) -> Rc<dyn Processor> {
            panic!("UnsupportedOperationException: get_plugin (test fixture capacity)")
        }
    }

    struct JavacPluginInfo;

    impl PluginInfo<Rc<dyn Plugin>> for JavacPluginInfo {
        fn get_name(&self) -> JavaString {
            JavaString::from("javac-plugin")
        }
        fn get_options(&self) -> Vec<(JavaString, JavaString)> {
            Vec::new()
        }
        fn get_plugin(&self) -> Rc<dyn Plugin> {
            panic!("UnsupportedOperationException: get_plugin (test fixture capacity)")
        }
    }

    #[test]
    fn platform_description_is_object_safe() {
        let description: Rc<dyn PlatformDescription> = Rc::new(NoopDescription);
        assert_eq!(description.get_source_version().to_string_lossy(), "17");
        assert_eq!(description.get_target_version().to_string_lossy(), "17");
        assert!(description.get_annotation_processors().is_empty());
        assert!(description.get_plugins().is_empty());
        assert!(description.get_additional_options().is_empty());
        assert_eq!(description.close(), Ok(()));
    }

    #[test]
    fn plugin_info_is_generic_over_processor_and_plugin() {
        let processor: Rc<dyn PluginInfo<Rc<dyn Processor>>> = Rc::new(ProcessorInfo);
        assert_eq!(
            processor.get_name().to_string_lossy(),
            "annotation-processor"
        );
        assert_eq!(
            processor.get_options(),
            vec![(JavaString::from("key"), JavaString::from("value"))]
        );
        let plugin: Rc<dyn PluginInfo<Rc<dyn Plugin>>> = Rc::new(JavacPluginInfo);
        assert_eq!(plugin.get_name().to_string_lossy(), "javac-plugin");
    }
}
