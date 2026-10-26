// Origin: javax/tools/JavaCompiler.java @ 982d53c (jdk-17.0.19-ga)

//! Interface to invoke Java programming language compilers from programs.

use super::diagnostic_listener::DiagnosticListener;
use super::java_file_manager::JavaFileManager;
use super::java_file_manager::JavaFileObjectRef;
use super::option_checker::OptionChecker;
use super::standard_java_file_manager::StandardJavaFileManager;
use super::tool::Tool;
use crate::jdk::Processor;
use crate::java_io::Writer;
use javac_strings::JavaString;
use std::cell::RefCell;
use std::rc::Rc;

/// Interface to invoke Java programming language compilers from
/// programs.
///
/// <p>The compiler might generate diagnostics during compilation (for
/// example, error messages).  If a diagnostic listener is provided,
/// the diagnostics will be supplied to the listener.  If no listener
/// is provided, the diagnostics will be formatted in an unspecified
/// format and written to the default output, which is System.err unless
/// otherwise specified.  Even if a diagnostic listener is supplied, some
/// diagnostics might not fit in a Diagnostic and will be written to the
/// default output.
///
/// <p>A compiler tool has an associated standard file manager, which
/// is the file manager that is native to the tool (or built-in).  The
/// standard file manager can be obtained by calling {@linkplain
/// #getStandardFileManager getStandardFileManager}.
///
/// <p>A compiler tool must function with any file manager as long as
/// any additional requirements as detailed in the methods below are
/// met.  If no file manager is provided, the compiler tool will use a
/// standard file manager such as the one returned by {@linkplain
/// #getStandardFileManager getStandardFileManager}.
///
/// <p>An instance implementing this interface must conform to
/// <cite>The Java Language Specification</cite>
/// and generate class files conforming to
/// <cite>The Java Virtual Machine Specification</cite>.
/// The versions of these specifications are defined in the {@linkplain Tool}
/// interface.
///
/// Additionally, an instance of this interface supporting {@link
/// javax.lang.model.SourceVersion#RELEASE_6 SourceVersion.RELEASE_6}
/// or higher must also support annotation processing.
///
/// <p>The compiler relies on two services: {@linkplain
/// DiagnosticListener diagnostic listener} and {@linkplain
/// JavaFileManager file manager}.  Although most classes and
/// interfaces in this package defines an API for compilers (and
/// tools in general) the interfaces {@linkplain DiagnosticListener},
/// {@linkplain JavaFileManager}, {@linkplain FileObject}, and
/// {@linkplain JavaFileObject} are not intended to be used in
/// applications.  Instead these interfaces are intended to be
/// implemented and used to provide customized services for a
/// compiler and thus defines an SPI for compilers.
///
/// <p>There are a number of classes and interfaces in this package
/// which are designed to ease the implementation of the SPI to
/// customize the behavior of a compiler:
///
/// <dl>
///   <dt>{@link StandardJavaFileManager}</dt>
///   <dd>
///
///     Every compiler which implements this interface provides a
///     standard file manager for operating on regular files.  The
///     StandardJavaFileManager interface defines additional methods for
///     creating file objects from regular files.
///
///     <p>The standard file manager serves two purposes:
///
///     <ul>
///       <li>basic building block for customizing how a compiler reads
///       and writes files</li>
///       <li>sharing between multiple compilation tasks</li>
///     </ul>
///
///     <p>Reusing a file manager can potentially reduce overhead of
///     scanning the file system and reading jar files.  Although there
///     might be no reduction in overhead, a standard file manager must
///     work with multiple sequential compilations making the following
///     example a recommended coding pattern:
///
///     <pre>
///       File[] files1 = ... ; // input for first compilation task
///       File[] files2 = ... ; // input for second compilation task
///
///       JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
///       StandardJavaFileManager fileManager = compiler.getStandardFileManager(null, null, null);
///
///       Iterable&lt;? extends JavaFileObject&gt; compilationUnits1 =
///           fileManager.getJavaFileObjectsFromFiles(Arrays.asList(files1));
///       compiler.getTask(null, fileManager, null, null, null, compilationUnits1).call();
///
///       Iterable&lt;? extends JavaFileObject&gt; compilationUnits2 =
///           fileManager.getJavaFileObjects(files2); // use alternative method
///       // reuse the same file manager to allow caching of jar files
///       compiler.getTask(null, fileManager, null, null, null, compilationUnits2).call();
///
///       fileManager.close();</pre>
///
///   </dd>
///
///   <dt>{@link DiagnosticCollector}</dt>
///   <dd>
///     Used to collect diagnostics in a list, for example:
///     <pre>
///       Iterable&lt;? extends JavaFileObject&gt; compilationUnits = ...;
///       JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
///       DiagnosticCollector&lt;JavaFileObject&gt; diagnostics = new DiagnosticCollector&lt;JavaFileObject&gt;();
///       StandardJavaFileManager fileManager = compiler.getStandardFileManager(diagnostics, null, null);
///       compiler.getTask(null, fileManager, diagnostics, null, null, compilationUnits).call();
///
///       for (Diagnostic&lt;? extends JavaFileObject&gt; diagnostic : diagnostics.getDiagnostics())
///           System.out.format("Error on line %d in %s%n",
///                             diagnostic.getLineNumber(),
///                             diagnostic.getSource().toUri());
///
///       fileManager.close();</pre>
///   </dd>
///
///   <dt>
///     {@link ForwardingJavaFileManager}, {@link ForwardingFileObject}, and
///     {@link ForwardingJavaFileObject}
///   </dt>
///   <dd>
///
///     Subclassing is not available for overriding the behavior of a
///     standard file manager as it is created by calling a method on a
///     compiler, not by invoking a constructor.  Instead forwarding
///     (or delegation) should be used.  These classes makes it easy to
///     forward most calls to a given file manager or file object while
///     allowing customizing behavior.  For example, consider how to
///     log all calls to {@linkplain JavaFileManager#flush}:
///
///     <pre>
///       final  Logger logger = ...;
///       Iterable&lt;? extends JavaFileObject&gt; compilationUnits = ...;
///       JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
///       StandardJavaFileManager stdFileManager = compiler.getStandardFileManager(null, null, null);
///       JavaFileManager fileManager = new ForwardingJavaFileManager(stdFileManager) {
///           public void flush() throws IOException {
///               logger.entering(StandardJavaFileManager.class.getName(), "flush");
///               super.flush();
///               logger.exiting(StandardJavaFileManager.class.getName(), "flush");
///           }
///       };
///       compiler.getTask(null, fileManager, null, null, null, compilationUnits).call();</pre>
///   </dd>
///
///   <dt>{@link SimpleJavaFileObject}</dt>
///   <dd>
///
///     This class provides a basic file object implementation which
///     can be used as building block for creating file objects.  For
///     example, here is how to define a file object which represent
///     source code stored in a string: see the SimpleJavaFileObject
///     port for the Rust form (the upstream JavaSourceFromString example
///     overrides getCharContent; a Rust implementation re-implements the
///     two virtual-method call sites, documented there).
///   </dd>
/// </dl>
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @see DiagnosticListener
/// @see Diagnostic
/// @see JavaFileManager
/// @since 1.6
///
/// PORTING_RULES — signature mapping:
/// * out Writer can be null ("use the compiler's default") -> Option of
///   Rc of RefCell of dyn Writer.  A Java Writer is a shared mutable
///   reference; Rc<RefCell<..>> is the only Rust representation (the crate
///   never uses Rc<RefCell<..>> for object graphs).
/// * fileManager can be null ("use the compiler's standard file manager") ->
///   Option of Rc of dyn JavaFileManager.
/// * DiagnosticListener of "? super JavaFileObject" ->
///   Rc of dyn DiagnosticListener of JavaFileObjectRef.  The wildcard
///   (? super) cannot be expressed in Rust; the compiler only uses
///   JavaFileObject as the source type.
/// * Iterable of String (null = none) -> slice of JavaString.
/// * Iterable of JavaFileObject (null = none) -> slice of JavaFileObjectRef.
/// * Locale / Charset names -> Option of &javac_strings::JavaString.
/// * the returned CompilationTask is a Box (the Rust caller owns it; a Java
///   caller holds a reference to a GC object).
pub trait JavaCompiler: Tool + OptionChecker {
    /// Creates a future for a compilation task with the given
    /// components and arguments.  The compilation might not have
    /// completed as described in the CompilationTask interface.
    ///
    /// <p>If a file manager is provided, it must be able to handle all
    /// locations defined in {@link StandardLocation}.
    ///
    /// <p>Note that annotation processing can process both the
    /// compilation units of source code to be compiled, passed with
    /// the compilationUnits parameter, as well as class
    /// files, whose names are passed with the classes
    /// parameter.
    ///
    /// @param out a Writer for additional output from the compiler;
    /// use System.err if null
    /// @param fileManager a file manager; if null use the
    /// compiler's standard file manager
    /// @param diagnosticListener a diagnostic listener; if
    /// null use the compiler's default method for reporting
    /// diagnostics
    /// @param options compiler options, null means no options
    /// @param classes names of classes to be processed by annotation
    /// processing, null means no class names
    /// @param compilationUnits the compilation units to compile, null
    /// means no compilation units
    /// @return an object representing the compilation
    /// @throws RuntimeException if an unrecoverable error
    /// occurred in a user supplied component.  The
    /// {@linkplain Throwable#getCause() cause} will be the error in
    /// user code.
    /// @throws IllegalArgumentException if any of the options are invalid,
    /// or if any of the given compilation units are of other kind than
    /// {@linkplain JavaFileObject.Kind#SOURCE source}
    fn get_task(
        &self,
        out: Option<Rc<RefCell<dyn Writer>>>,
        file_manager: Option<Rc<dyn JavaFileManager>>,
        diagnostic_listener: Option<Rc<dyn DiagnosticListener<JavaFileObjectRef>>>,
        options: &[JavaString],
        classes: &[JavaString],
        compilation_units: &[JavaFileObjectRef],
    ) -> Box<dyn CompilationTask>;

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
    /// @param locale the locale to apply when formatting diagnostics;
    /// null means no locale
    /// @param charset the character set used for decoding bytes; if
    /// null use the platform default
    /// @return the standard file manager
    fn get_standard_file_manager(
        &self,
        diagnostic_listener: Option<Rc<dyn DiagnosticListener<JavaFileObjectRef>>>,
        locale: Option<&JavaString>,
        charset: Option<&JavaString>,
    ) -> Rc<dyn StandardJavaFileManager>;
}

/// Interface representing a future for a compilation task.  The
/// compilation task has not yet started.  To start the task, call
/// the {@linkplain #call call} method.
///
/// <p>Before calling the call method, additional aspects of the
/// task can be configured, for example, by calling the
/// {@linkplain #setProcessors setProcessors} method.
///
/// PORTING_RULES: Java nested interface JavaCompiler.CompilationTask ->
/// Rust trait CompilationTask.  Callable of Boolean -> call() returning bool.
/// The configuring methods and call() take &mut self: a Java task mutates
/// itself and call() may only be invoked once (IllegalStateException
/// otherwise), which Rust expresses through exclusive access.  The ported
/// task keeps that once-only check and panics on a second call, matching the
/// upstream runtime exception (crate::error).
pub trait CompilationTask {
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

    /// Sets processors (for annotation processing).  This will
    /// bypass the normal discovery mechanism.
    ///
    /// @param processors processors (for annotation processing)
    /// @throws IllegalStateException if the task has started
    fn set_processors(&mut self, processors: &[Rc<dyn Processor>]);

    /// Sets the locale to be applied when formatting diagnostics and
    /// other localized data.
    ///
    /// @param locale the locale to apply; null means apply no
    /// locale
    /// @throws IllegalStateException if the task has started
    fn set_locale(&mut self, locale: Option<&JavaString>);

    /// Performs this compilation task.  The compilation may only
    /// be performed once.  Subsequent calls to this method throw
    /// IllegalStateException.
    ///
    /// @return true if and only all the files compiled without errors;
    /// false otherwise
    ///
    /// @throws RuntimeException if an unrecoverable error occurred
    /// in a user-supplied component.  The
    /// {@linkplain Throwable#getCause() cause} will be the error
    /// in user code.
    /// @throws IllegalStateException if called more than once
    fn call(&mut self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::{CompilationTask, JavaCompiler};
    use crate::jdk::Processor;
    use crate::java_io::Writer;
    use crate::lang::model::source_version::SourceVersion;
    use crate::tools::diagnostic_listener::DiagnosticListener;
    use crate::tools::java_file_manager::JavaFileManager;
    use crate::tools::java_file_manager::JavaFileObjectRef;
    use crate::tools::option_checker::OptionChecker;
    use crate::tools::standard_java_file_manager::StandardJavaFileManager;
    use crate::tools::tool::Tool;
    use javac_strings::JavaString;
    use std::cell::RefCell;
    use std::io::{Read, Write};
    use std::rc::Rc;

    fn js(s: &str) -> JavaString {
        JavaString::from(s)
    }

    struct Task {
        started: bool,
        modules: Vec<JavaString>,
        locale: Option<JavaString>,
        processors: usize,
    }

    impl CompilationTask for Task {
        fn add_modules(&mut self, module_names: &[JavaString]) {
            assert!(!self.started, "IllegalStateException: task has started");
            self.modules.extend(module_names.iter().cloned());
        }
        fn set_processors(&mut self, processors: &[Rc<dyn Processor>]) {
            assert!(!self.started, "IllegalStateException: task has started");
            self.processors = processors.len();
        }
        fn set_locale(&mut self, locale: Option<&JavaString>) {
            assert!(!self.started, "IllegalStateException: task has started");
            self.locale = locale.cloned();
        }
        fn call(&mut self) -> bool {
            assert!(!self.started, "IllegalStateException: task called twice");
            self.started = true;
            true
        }
    }

    struct Compiler;

    impl Tool for Compiler {
        fn run(
            &self,
            _in: Option<&mut dyn Read>,
            _out: Option<&mut dyn Write>,
            _err: Option<&mut dyn Write>,
            _arguments: &[JavaString],
        ) -> i32 {
            0
        }
        fn get_source_versions(&self) -> Vec<SourceVersion> {
            SourceVersion::values().to_vec()
        }
    }

    impl OptionChecker for Compiler {
        fn is_supported_option(&self, _option: &JavaString) -> i32 {
            -1
        }
    }

    impl JavaCompiler for Compiler {
        fn get_task(
            &self,
            _out: Option<Rc<RefCell<dyn Writer>>>,
            _file_manager: Option<Rc<dyn JavaFileManager>>,
            _diagnostic_listener: Option<Rc<dyn DiagnosticListener<JavaFileObjectRef>>>,
            _options: &[JavaString],
            _classes: &[JavaString],
            _compilation_units: &[JavaFileObjectRef],
        ) -> Box<dyn CompilationTask> {
            Box::new(Task {
                started: false,
                modules: Vec::new(),
                locale: None,
                processors: 0,
            })
        }

        fn get_standard_file_manager(
            &self,
            _diagnostic_listener: Option<Rc<dyn DiagnosticListener<JavaFileObjectRef>>>,
            _locale: Option<&JavaString>,
            _charset: Option<&JavaString>,
        ) -> Rc<dyn StandardJavaFileManager> {
            panic!("AbstractMethodError: get_standard_file_manager: test compiler does not provide a standard file manager")
        }
    }

    #[test]
    fn compiler_is_object_safe() {
        let compiler: Rc<dyn JavaCompiler> = Rc::new(Compiler);
        // Tool + OptionChecker supertraits stay callable through dyn.
        assert_eq!(compiler.is_supported_option(&js("-d")), -1);
        assert_eq!(compiler.get_source_versions().len(), 18);
        let options = [js("-g")];
        let classes = [js("java.lang.String")];
        let mut task = compiler.get_task(None, None, None, &options, &classes, &[]);
        task.add_modules(&[js("java.base")]);
        task.set_locale(None);
        task.set_processors(&[]);
        assert!(task.call());
    }
}
