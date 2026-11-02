//! Provides interfaces for tools which can be invoked from a program,
//! for example, compilers.
//!
//! <p>These interfaces and classes are required as part of the
//! Java Platform, Standard Edition (Java SE),
//! but there is no requirement to provide any tools implementing them.
//!
//! <p>Unless explicitly allowed, all methods in this package might
//! throw a {@link java.lang.NullPointerException} if given a
//! {@code null} argument or if given a
//! {@linkplain java.lang.Iterable list or collection} containing
//! {@code null} elements.  Similarly, no method may return
//! {@code null} unless explicitly allowed.
//!
//! <p>This package is the home of the Java programming language compiler framework.
//! This framework allows clients of the framework to locate and run
//! compilers from programs.  The framework also provides Service
//! Provider Interfaces (SPI) for structured access to diagnostics
//! ({@link javax.tools.DiagnosticListener}) as well as a file
//! abstraction for overriding file access ({@link
//! javax.tools.JavaFileManager} and {@link
//! javax.tools.JavaFileObject}).  See {@link
//! javax.tools.JavaCompiler} for more details on using the SPI.
//!
//! <p>There is no requirement for a compiler at runtime.  However, if
//! a default compiler is provided, it can be located using the
//! {@link javax.tools.ToolProvider}, for example:
//!
//! <p>{@code JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();}
//!
//! <p>It is possible to provide alternative compilers or tools
//! through the {@linkplain java.util.ServiceLoader service provider
//! mechanism}.
//!
//! <p>For example, if {@code com.vendor.VendorJavaCompiler} is a
//! provider of the {@code JavaCompiler} tool then its jar file
//! would contain the file {@code
//! META-INF/services/javax.tools.JavaCompiler}.  This file would
//! contain the single line:
//!
//! <p>{@code com.vendor.VendorJavaCompiler}
//!
//! <p>If the jar file is on the class path, VendorJavaCompiler can be
//! located using code like this:
//!
//! <p>{@code JavaCompiler compiler = ServiceLoader.load(JavaCompiler.class).iterator().next();}
//!
//! @author Peter von der Ah&eacute;
//! @author Jonathan Gibbons
//! @since 1.6
//!
//! this module holds one Rust file per Java file of the
//! package; the package documentation above is the `package-info.java` port.
//! Method signature mappings are documented per file.

pub mod diagnostic;
pub mod diagnostic_collector;
pub mod diagnostic_listener;
pub mod documentation_tool;
pub mod file_object;
pub mod forwarding_file_object;
pub mod forwarding_java_file_manager;
pub mod forwarding_java_file_object;
pub mod java_compiler;
pub mod java_file_manager;
pub mod java_file_object;
pub mod option_checker;
pub mod simple_java_file_object;
pub mod standard_java_file_manager;
pub mod standard_location;
pub mod tool;
pub mod tool_provider;
