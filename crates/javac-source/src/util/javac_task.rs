//! Provides access to functionality specific to the JDK Java Compiler, javac.
//!
//! PORTING_RULES (§14): the Java abstract class becomes a Rust trait whose
//! implementation is provided by the P6 API layer (javac-core).  Notes:
//! * the static `instance(ProcessingEnvironment)` factory depends on
//!   javax.annotation.processing (not ported) and on the internal
//!   BasicJavacTask/Context; it is deferred to P6 (REGISTERED(P6)).
//! * `getElements()/getTypes()` return the javax.lang.model.util utility types
//!   that are deferred to P6 in javac-tools; they are left out of this trait
//!   until that port lands (documented P6 dependency).
//! * `Iterable<? extends X>` -> Vec<Rc<dyn X>>; declared IOException ->
//!   Result<..., javac_tools::error::Error>.
//! * listeners are stored by reference identity in Java; the Rust signatures
//!   use Rc<RefCell<dyn TaskListener>> so add/remove can compare pointers.
//! * `setParameterNameProvider` has a no-op Java default implementation and is a
//!   Rust default method.

use super::parameter_name_provider::ParameterNameProvider;
use super::task_listener::TaskListener;
use crate::tree::compilation_unit_tree::CompilationUnitTree;
use crate::tree::tree::Tree;
use javac_tools::error::Error;
use javac_tools::lang::model::element::Element;
use javac_tools::lang::model::type_::TypeMirror;
use javac_tools::tools::java_compiler::CompilationTask;
use javac_tools::tools::java_file_object::JavaFileObject;
use std::cell::RefCell;
use std::rc::Rc;

/// Provides access to functionality specific to the JDK Java Compiler, javac.
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
pub trait JavacTask: CompilationTask {
    /// Parses the specified files returning a list of abstract syntax trees.
    ///
    /// @return a list of abstract syntax trees
    /// @throws IOException if an unhandled I/O error occurred in the compiler.
    /// @throws IllegalStateException if the operation cannot be performed at this time.
    fn parse(&mut self) -> Result<Vec<Rc<dyn CompilationUnitTree>>, Error>;

    /// Completes all analysis.
    ///
    /// @return a list of elements that were analyzed
    /// @throws IOException if an unhandled I/O error occurred in the compiler.
    /// @throws IllegalStateException if the operation cannot be performed at this time.
    fn analyze(&mut self) -> Result<Vec<Rc<dyn Element>>, Error>;

    /// Generates code.
    ///
    /// @return a list of files that were generated
    /// @throws IOException if an unhandled I/O error occurred in the compiler.
    /// @throws IllegalStateException if the operation cannot be performed at this time.
    fn generate(&mut self) -> Result<Vec<Rc<dyn JavaFileObject>>, Error>;

    /// Sets a specified listener to receive notification of events
    /// describing the progress of this compilation task.
    ///
    /// If another listener is receiving notifications as a result of a prior
    /// call of this method, then that listener will no longer receive notifications.
    ///
    /// Informally, this method is equivalent to calling {@code removeTaskListener} for
    /// any listener that has been previously set, followed by {@code addTaskListener}
    /// for the new listener.
    ///
    /// @param task_listener the task listener
    /// @throws IllegalStateException if the specified listener has already been added.
    fn set_task_listener(&mut self, task_listener: Rc<RefCell<dyn TaskListener>>);

    /// Adds a specified listener so that it receives notification of events
    /// describing the progress of this compilation task.
    ///
    /// This method may be called at any time before or during the compilation.
    ///
    /// @param task_listener the task listener
    /// @throws IllegalStateException if the specified listener has already been added.
    /// @since 1.8
    fn add_task_listener(&mut self, task_listener: Rc<RefCell<dyn TaskListener>>);

    /// Removes the specified listener so that it no longer receives
    /// notification of events describing the progress of this
    /// compilation task.
    ///
    /// This method may be called at any time before or during the compilation.
    ///
    /// @param task_listener the task listener
    /// @since 1.8
    fn remove_task_listener(&mut self, task_listener: Rc<RefCell<dyn TaskListener>>);

    /// Sets the specified {@link ParameterNameProvider}. It may be used when
    /// {@link javax.lang.model.element.VariableElement#getSimpleName()} is called
    /// for a method parameter for which an authoritative name is not found.
    ///
    /// @implSpec The default implementation of this method does nothing.
    ///
    /// @param provider the provider
    /// @since 13
    fn set_parameter_name_provider(&mut self, provider: Option<Rc<dyn ParameterNameProvider>>) {
        let _ = provider;
    }

    /// Returns a type mirror of the tree node determined by the specified path.
    /// This method has been superseded by methods on {@link Trees}.
    ///
    /// @param path the path
    /// @return the type mirror
    /// @see Trees#getTypeMirror
    fn get_type_mirror(&mut self, path: &[Rc<dyn Tree>]) -> Option<Rc<dyn TypeMirror>>;
}

#[cfg(test)]
mod tests {
    use super::JavacTask;
    use crate::tree::compilation_unit_tree::CompilationUnitTree;
    use crate::util::task_listener::TaskListener;
    use javac_tools::error::Error;
    use javac_tools::lang::model::element::Element;
    use javac_tools::lang::model::type_::TypeMirror;
    use javac_tools::tools::java_compiler::CompilationTask;
    use javac_tools::tools::java_file_object::JavaFileObject;
    use javac_tools::jdk::Processor;
    use javac_strings::JavaString;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct FakeTask {
        listeners: Vec<Rc<RefCell<dyn TaskListener>>>,
    }

    impl CompilationTask for FakeTask {
        fn add_modules(&mut self, _module_names: &[JavaString]) {}
        fn set_processors(&mut self, _processors: &[Rc<dyn Processor>]) {}
        fn set_locale(&mut self, _locale: Option<&JavaString>) {}
        fn call(&mut self) -> bool {
            true
        }
    }

    impl JavacTask for FakeTask {
        fn parse(&mut self) -> Result<Vec<Rc<dyn CompilationUnitTree>>, Error> {
            Ok(Vec::new())
        }
        fn analyze(&mut self) -> Result<Vec<Rc<dyn Element>>, Error> {
            Ok(Vec::new())
        }
        fn generate(&mut self) -> Result<Vec<Rc<dyn JavaFileObject>>, Error> {
            Ok(Vec::new())
        }
        fn set_task_listener(&mut self, task_listener: Rc<RefCell<dyn TaskListener>>) {
            self.listeners = vec![task_listener];
        }
        fn add_task_listener(&mut self, task_listener: Rc<RefCell<dyn TaskListener>>) {
            self.listeners.push(task_listener);
        }
        fn remove_task_listener(&mut self, task_listener: Rc<RefCell<dyn TaskListener>>) {
            // Java removes one registration, not every equal listener.
            if let Some(pos) = self
                .listeners
                .iter()
                .position(|l| Rc::ptr_eq(l, &task_listener))
            {
                self.listeners.remove(pos);
            }
        }
        fn get_type_mirror(&mut self, _path: &[Rc<dyn crate::tree::tree::Tree>]) -> Option<Rc<dyn TypeMirror>> {
            None
        }
    }

    struct NoopListener;

    impl TaskListener for NoopListener {}

    #[test]
    fn javac_task_is_object_safe() {
        let mut task = FakeTask { listeners: Vec::new() };
        {
            let dyn_task: &mut dyn JavacTask = &mut task;
            assert!(dyn_task.parse().unwrap().is_empty());
            assert!(dyn_task.analyze().unwrap().is_empty());
            assert!(dyn_task.generate().unwrap().is_empty());
            assert!(dyn_task.get_type_mirror(&[]).is_none());
            let listener: Rc<RefCell<dyn TaskListener>> = Rc::new(RefCell::new(NoopListener));
            dyn_task.set_task_listener(listener.clone());
            dyn_task.add_task_listener(listener.clone());
            dyn_task.remove_task_listener(listener);
            // the inherited CompilationTask method stays callable
            assert!(dyn_task.call());
        }
        assert_eq!(task.listeners.len(), 1);
    }
}
