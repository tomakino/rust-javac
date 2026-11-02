//! Interface to invoke Java programming language documentation tools from
//! programs.
//!
//! PORTING_RULES: the javadoc tool itself is out of scope for rust-javac
//! (docs/PLAN.md §1: the project ports javac).  This file ports the
//! javax.tools API interface only, so that the javax.tools surface is complete
//! and the JavaFileManager.Location enum can be used by callers.  The
//! documentation task implementation is not provided; see ToolProvider.

use super::diagnostic_listener::DiagnosticListener;
use super::java_file_manager::JavaFileManager;
use super::java_file_manager::{JavaFileObjectRef, Location as JavaFileManagerLocation};
use super::option_checker::OptionChecker;
use super::standard_java_file_manager::StandardJavaFileManager;
use super::tool::Tool;
use crate::jdk::Class;
use crate::java_io::Writer;
use javac_strings::JavaString;
use std::cell::RefCell;
use std::rc::Rc;

/// Interface to invoke Java programming language documentation tools from
/// programs.
///
/// @since 1.8
///
/// PORTING_RULES — signature mapping (identical to JavaCompiler):
/// * out Writer -> Option of Rc of RefCell of dyn Writer.
/// * fileManager -> Option of Rc of dyn JavaFileManager.
/// * DiagnosticListener of "? super JavaFileObject" ->
///   Rc of dyn DiagnosticListener of JavaFileObjectRef.
/// * the doclet Class object -> Option of Class (opaque placeholder,
///   crate::jdk); None means "use the standard doclet", as upstream null does.
/// * Iterable of String / JavaFileObject -> slices (Java String ->
///   javac_strings::JavaString).
/// * Locale / Charset names -> Option of &JavaString.
pub trait DocumentationTool: Tool + OptionChecker {
    /// Creates a future for a documentation task with the given
    /// components and arguments.  The task might not have
    /// completed as described in the DocumentationTask interface.
    ///
    /// <p>If a file manager is provided, it must be able to handle all
    /// locations defined in {@link DocumentationTool.Location},
    /// as well as {@link StandardLocation#SOURCE_PATH},
    /// {@link StandardLocation#CLASS_PATH}, and
    /// {@link StandardLocation#PLATFORM_CLASS_PATH}.
    ///
    /// @param out a Writer for additional output from the tool;
    /// use System.err if null
    ///
    /// @param fileManager a file manager; if null use the
    /// tool's standard file manager
    ///
    /// @param diagnosticListener a diagnostic listener; if null
    /// use the tool's default method for reporting diagnostics
    ///
    /// @param docletClass a class providing the necessary methods required
    /// of a doclet; a value of null means to use the standard doclet.
    ///
    /// @param options documentation tool options and doclet options,
    /// null means no options
    ///
    /// @param compilationUnits the compilation units to compile, null
    /// means no compilation units
    ///
    /// @return an object representing the compilation
    ///
    /// @throws RuntimeException if an unrecoverable error
    /// occurred in a user supplied component.
    /// The {@linkplain Throwable#getCause() cause} will be the error in
    /// user code.
    ///
    /// @throws IllegalArgumentException if any of the given
    /// compilation units are of other kind than
    /// {@linkplain JavaFileObject.Kind#SOURCE source}
    fn get_task(
        &self,
        out: Option<Rc<RefCell<dyn Writer>>>,
        file_manager: Option<Rc<dyn JavaFileManager>>,
        diagnostic_listener: Option<Rc<dyn DiagnosticListener<JavaFileObjectRef>>>,
        doclet_class: Option<Class<()>>,
        options: &[JavaString],
        compilation_units: &[JavaFileObjectRef],
    ) -> Box<dyn DocumentationTask>;

    /// Returns a new instance of the standard file manager implementation
    /// for this tool.  The file manager will use the given diagnostic
    /// listener for producing any non-fatal diagnostics.  Fatal errors
    /// will be signaled with the appropriate exceptions.
    ///
    /// <p>The standard file manager will be automatically reopened if
    /// it is accessed after calls to flush or close.
    /// The standard file manager must be usable with other tools.
    ///
    /// @param diagnosticListener a diagnostic listener for non-fatal
    /// diagnostics; if null use the compiler's default method
    /// for reporting diagnostics
    ///
    /// @param locale the locale to apply when formatting diagnostics;
    /// null means the default locale.
    ///
    /// @param charset the character set used for decoding bytes; if
    /// null use the platform default
    ///
    /// @return the standard file manager
    fn get_standard_file_manager(
        &self,
        diagnostic_listener: Option<Rc<dyn DiagnosticListener<JavaFileObjectRef>>>,
        locale: Option<&JavaString>,
        charset: Option<&JavaString>,
    ) -> Rc<dyn StandardJavaFileManager>;
}

/// Interface representing a future for a documentation task.  The
/// task has not yet started.  To start the task, call
/// the {@linkplain #call call} method.
///
/// <p>Before calling the call method, additional aspects of the
/// task can be configured, for example, by calling the
/// {@linkplain #setLocale setLocale} method.
///
/// PORTING_RULES: Java nested interface DocumentationTool.DocumentationTask ->
/// Rust trait DocumentationTask; Callable of Boolean -> call() returning bool.
pub trait DocumentationTask {
    /// Adds root modules to be taken into account during module
    /// resolution.
    /// Invalid module names may cause either
    /// IllegalArgumentException to be thrown,
    /// or diagnostics to be reported when the task is started.
    /// @param moduleNames the names of the root modules
    /// @throws IllegalArgumentException may be thrown for some
    ///      invalid module names
    /// @throws IllegalStateException if the task has started
    /// @since 9
    fn add_modules(&mut self, module_names: &[JavaString]);

    /// Sets the locale to be applied when formatting diagnostics and
    /// other localized data.
    ///
    /// @param locale the locale to apply; null means apply no
    /// locale
    /// @throws IllegalStateException if the task has started
    fn set_locale(&mut self, locale: Option<&JavaString>);

    /// Performs this documentation task.  The task may only
    /// be performed once.  Subsequent calls to this method throw
    /// IllegalStateException.
    ///
    /// @return true if and only all the files were processed without errors;
    /// false otherwise
    ///
    /// @throws RuntimeException if an unrecoverable error occurred
    /// in a user-supplied component.  The
    /// {@linkplain Throwable#getCause() cause} will be the error
    /// in user code.
    ///
    /// @throws IllegalStateException if called more than once
    fn call(&mut self) -> bool;
}

/// Locations specific to {@link DocumentationTool}.
///
/// @see StandardLocation
///
/// PORTING_RULES: Java nested enum DocumentationTool.Location implements
/// JavaFileManager.Location.  In Rust the enum lives in this module and
/// implements the Location trait through its import alias.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Location {
    /// Location of new documentation files.
    DOCUMENTATION_OUTPUT,

    /// Location to search for doclets.
    DOCLET_PATH,

    /// Location to search for taglets.
    TAGLET_PATH,
}

impl Location {
    /// Java: enum#name() (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            Location::DOCUMENTATION_OUTPUT => "DOCUMENTATION_OUTPUT",
            Location::DOCLET_PATH => "DOCLET_PATH",
            Location::TAGLET_PATH => "TAGLET_PATH",
        })
    }

    /// Java: enum#values().
    pub fn values() -> [Location; 3] {
        [Location::DOCUMENTATION_OUTPUT, Location::DOCLET_PATH, Location::TAGLET_PATH]
    }
}

impl JavaFileManagerLocation for Location {
    // Java: public String getName() { return name(); }
    fn get_name(&self) -> JavaString {
        self.name()
    }

    // Java: switch (this) { case DOCUMENTATION_OUTPUT: return true;
    //       default: return false; }
    fn is_output_location(&self) -> bool {
        matches!(self, Location::DOCUMENTATION_OUTPUT)
    }

    // Java: the enum does not override isModuleOrientedLocation, so the
    // interface default method applies.
}

impl std::fmt::Display for Location {
    /// Java: Enum.toString() = name().
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::Location;
    use crate::tools::java_file_manager::Location as JavaFileManagerLocation;
    use javac_strings::JavaString;

    #[test]
    fn documentation_locations() {
        assert!(Location::DOCUMENTATION_OUTPUT.is_output_location());
        assert!(!Location::DOCLET_PATH.is_output_location());
        assert!(!Location::TAGLET_PATH.is_output_location());
        assert_eq!(
            Location::DOCUMENTATION_OUTPUT.get_name(),
            JavaString::from("DOCUMENTATION_OUTPUT")
        );
        // Default interface method applies: only the exact name "MODULE"
        // matches the upstream regular expression.
        assert!(!Location::DOCUMENTATION_OUTPUT.is_module_oriented_location());
        assert_eq!(Location::values()[2], Location::TAGLET_PATH);
    }
}
