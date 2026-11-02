//! The interface for a javac plug-in.
//!
//! (§14): Java interface -> Rust trait; `String` -> JavaString;
//! `String... args` -> `&[JavaString]`; the Java `default boolean autoStart()`
//! is a Rust default method returning false.  The JavacTask parameter is held as
//! `Rc<RefCell<dyn JavacTask>>` because init() registers task listeners and the
//! task is a shared mutable object upstream.

use super::javac_task::JavacTask;
use javac_strings::JavaString;
use std::cell::RefCell;
use std::rc::Rc;

/// The interface for a javac plug-in.
///
/// <p>The javac plug-in mechanism allows a user to specify one or more plug-ins
/// on the javac command line, to be started soon after the compilation
/// has begun. Plug-ins are identified by a user-friendly name. Each plug-in that
/// is started will be passed an array of strings, which may be used to
/// provide the plug-in with values for any desired options or other arguments.
///
/// <p>Plug-ins are located via a {@link java.util.ServiceLoader},
/// using the same class path as annotation processors.
///
/// @since 1.8
pub trait Plugin {
    /// Returns the user-friendly name of this plug-in.
    /// @return the user-friendly name of the plug-in
    fn get_name(&self) -> JavaString;

    /// Initializes the plug-in for a given compilation task.
    /// @param task The compilation task that has just been started
    /// @param args Arguments, if any, for the plug-in
    fn init(&mut self, task: Rc<RefCell<dyn JavacTask>>, args: &[JavaString]);

    /// Returns whether or not this plugin should be automatically started,
    /// even if not explicitly specified in the command-line options.
    ///
    /// @return whether or not this plugin should be automatically started
    fn auto_start(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::Plugin;
    use crate::util::javac_task::JavacTask;
    use crate::util::task_listener::TaskListener;
    use javac_strings::JavaString;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct NoopTask;

    impl javac_tools::tools::java_compiler::CompilationTask for NoopTask {
        fn add_modules(&mut self, _module_names: &[JavaString]) {}
        fn set_processors(&mut self, _processors: &[Rc<dyn javac_tools::jdk::Processor>]) {}
        fn set_locale(&mut self, _locale: Option<&JavaString>) {}
        fn call(&mut self) -> bool {
            true
        }
    }

    impl JavacTask for NoopTask {
        fn parse(&mut self) -> Result<Vec<Rc<dyn crate::tree::compilation_unit_tree::CompilationUnitTree>>, javac_tools::error::Error> {
            Ok(Vec::new())
        }
        fn analyze(&mut self) -> Result<Vec<Rc<dyn javac_tools::lang::model::element::Element>>, javac_tools::error::Error> {
            Ok(Vec::new())
        }
        fn generate(&mut self) -> Result<Vec<Rc<dyn javac_tools::tools::java_file_object::JavaFileObject>>, javac_tools::error::Error> {
            Ok(Vec::new())
        }
        fn set_task_listener(&mut self, _listener: Rc<RefCell<dyn TaskListener>>) {}
        fn add_task_listener(&mut self, _listener: Rc<RefCell<dyn TaskListener>>) {}
        fn remove_task_listener(&mut self, _listener: Rc<RefCell<dyn TaskListener>>) {}
        fn get_type_mirror(&mut self, _path: &[Rc<dyn crate::tree::tree::Tree>]) -> Option<Rc<dyn javac_tools::lang::model::type_::TypeMirror>> {
            None
        }
    }

    struct TestPlugin;

    impl Plugin for TestPlugin {
        fn get_name(&self) -> JavaString {
            JavaString::from("test-plugin")
        }
        fn init(&mut self, _task: Rc<RefCell<dyn JavacTask>>, _args: &[JavaString]) {}
    }

    #[test]
    fn plugin_defaults() {
        let plugin = TestPlugin;
        assert_eq!(plugin.get_name().to_string_lossy(), "test-plugin");
        assert!(!plugin.auto_start());
        let task: Rc<RefCell<dyn JavacTask>> = Rc::new(RefCell::new(NoopTask));
        let mut plugin = TestPlugin;
        plugin.init(task, &[]);
    }
}
