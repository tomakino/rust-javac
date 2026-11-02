//! Provides methods for locating tool providers, for example,
//! providers of compilers.

use super::documentation_tool::DocumentationTool;
use super::java_compiler::JavaCompiler;
use crate::jdk::ClassLoader;
use javac_strings::JavaString;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

/// Provides methods for locating tool providers, for example,
/// providers of compilers.  This class complements the
/// functionality of {@link java.util.ServiceLoader}.
///
/// @author Peter von der Ah&eacute;
/// @since 1.6
///
/// — class mapping:
/// * Java "public class ToolProvider" with a private constructor and static
///   methods -> Rust struct with a private field (so it cannot be
///   instantiated) and associated functions.
/// * the private static final Java Strings of the upstream lookup are exposed
///   as functions returning JavaString (a Rust const cannot hold a JavaString).
/// * Java ServiceLoader based discovery is not available in Rust.  The
///   getSystemTool/matches private static helpers are therefore not ported;
///   getSystemJavaCompiler is unimplemented until the API phase provides the
///   compiler entry point (REGISTERED P6).
pub struct ToolProvider(());

fn system_java_compiler_module() -> JavaString {
    JavaString::from("jdk.compiler")
}

fn system_java_compiler_name() -> JavaString {
    JavaString::from("com.sun.tools.javac.api.JavacTool")
}

fn system_documentation_tool_module() -> JavaString {
    JavaString::from("jdk.javadoc")
}

fn system_documentation_tool_name() -> JavaString {
    JavaString::from("jdk.javadoc.internal.api.JavadocTool")
}

/// The factory installed for the platform compiler (registered adapter for the
/// ServiceLoader lookup of javax.tools.ToolProvider; see get_system_java_compiler).
pub type SystemJavaCompilerFactory = Arc<dyn Fn() -> Option<Rc<dyn JavaCompiler>> + Send + Sync>;

static SYSTEM_JAVA_COMPILER: OnceLock<SystemJavaCompilerFactory> = OnceLock::new();

/// Installs the platform compiler factory (the ported replacement for the
/// ServiceLoader discovery; registered adapter, §9). Returns the factory back
/// when a factory was already installed.
pub fn install_system_java_compiler(
    factory: SystemJavaCompilerFactory,
) -> Result<(), SystemJavaCompilerFactory> {
    SYSTEM_JAVA_COMPILER.set(factory)
}

impl ToolProvider {
    /// Returns the Java programming language compiler provided
    /// with this platform.
    /// <p>The file manager returned by calling
    /// {@link JavaCompiler#getStandardFileManager getStandardFileManager}
    /// on this compiler supports paths provided by any
    /// {@linkplain java.nio.file.FileSystem filesystem}.</p>
    /// @return the compiler provided with this platform or
    /// null if no compiler is provided
    /// @implNote This implementation returns the compiler provided
    /// by the jdk.compiler module if that module is available,
    /// and null otherwise.
    ///
    /// REGISTERED P6: the Rust compiler entry point is created by the API phase
    /// (javac-core); javax.tools cannot depend on javac-core (dependency
    /// direction), so no compiler instance can be built here yet.
    /// Java constants used by the upstream lookup:
    ///   systemJavaCompilerModule = "jdk.compiler"
    ///   systemJavaCompilerName   = "com.sun.tools.javac.api.JavacTool"
    pub fn get_system_java_compiler() -> Option<Rc<dyn JavaCompiler>> {
        let _ = (system_java_compiler_module(), system_java_compiler_name());
        // Java: ServiceLoader discovery of com.sun.tools.javac.api.JavacTool in
        // the jdk.compiler module; the port reads the factory installed by the
        // compiler entry point (registered adapter). None means that the
        // platform compiler is not available (upstream null).
        SYSTEM_JAVA_COMPILER.get().and_then(|factory| factory())
    }

    /// Returns the Java programming language documentation tool provided
    /// with this platform.
    /// <p>The file manager returned by calling
    /// {@link DocumentationTool#getStandardFileManager getStandardFileManager}
    /// on this tool supports paths provided by any
    /// {@linkplain java.nio.file.FileSystem filesystem}.</p>
    /// @return the documentation tool provided with this platform or
    /// null if no documentation tool is provided
    /// @implNote This implementation returns the tool provided
    /// by the jdk.javadoc module if that module is available,
    /// and null otherwise.
    ///
    /// the javadoc tool is out of scope for rust-javac, so the
    /// platform provides no documentation tool: null (None) is the faithful
    /// result, exactly like upstream on a platform without jdk.javadoc.
    /// Java constants of the upstream lookup:
    ///   systemDocumentationToolModule = "jdk.javadoc"
    ///   systemDocumentationToolName   = "jdk.javadoc.internal.api.JavadocTool"
    pub fn get_system_documentation_tool() -> Option<Rc<dyn DocumentationTool>> {
        let _ = (system_documentation_tool_module(), system_documentation_tool_name());
        None
    }

    /// Returns a class loader that may be used to load system tools,
    /// or null if no such special loader is provided.
    /// @implSpec This implementation always returns null.
    /// @deprecated This method is subject to removal in a future version of
    /// Java SE.
    /// Use the {@link java.util.spi.ToolProvider system tool provider} or
    /// {@link java.util.ServiceLoader service loader} mechanisms to
    /// locate system tools as well as user-installed tools.
    /// @return a class loader, or null
    ///
    /// upstream always returns null, so None is exact.
    pub fn get_system_tool_class_loader() -> Option<Rc<dyn ClassLoader>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::ToolProvider;
    use javac_strings::JavaString;

    #[test]
    fn system_compiler_factory_registration_round_trip() {
        // This test owns the process-wide registration: run the unregistered
        // assertion first, then install a factory that reports an unavailable
        // platform compiler (the JavacTool implementation is provided by the
        // compiler entry point and is not registered in this crate test).
        assert!(super::ToolProvider::get_system_java_compiler().is_none());
        let factory: super::SystemJavaCompilerFactory = std::sync::Arc::new(|| None);
        assert!(super::install_system_java_compiler(factory).is_ok());
        assert!(super::ToolProvider::get_system_java_compiler().is_none());
        let duplicate: super::SystemJavaCompilerFactory = std::sync::Arc::new(|| None);
        assert!(super::install_system_java_compiler(duplicate).is_err());
    }

    #[test]
    fn system_documentation_tool_is_absent() {
        // javadoc is not ported: no documentation tool is provided.
        assert!(ToolProvider::get_system_documentation_tool().is_none());
    }

    #[test]
    fn system_tool_class_loader_is_always_null() {
        // Java: getSystemToolClassLoader() always returns null.
        assert!(ToolProvider::get_system_tool_class_loader().is_none());
    }

    #[test]
    fn constants_match_upstream() {
        // The lookup constants are part of the upstream implementation; they
        // are kept for the P6 API wiring.
        assert_eq!(super::system_java_compiler_module(), JavaString::from("jdk.compiler"));
        assert_eq!(
            super::system_java_compiler_name(),
            JavaString::from("com.sun.tools.javac.api.JavacTool")
        );
        assert_eq!(
            super::system_documentation_tool_module(),
            JavaString::from("jdk.javadoc")
        );
        assert_eq!(
            super::system_documentation_tool_name(),
            JavaString::from("jdk.javadoc.internal.api.JavadocTool")
        );
    }
}
