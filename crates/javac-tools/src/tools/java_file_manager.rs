// NOTE: the four "Failure modes: undocumented upstream." comments below are the
// upstream JavaFileManager.java "describe failure modes" note, rewritten
// per project rules (semantics unchanged).

//! File manager for tools operating on Java programming language
//! source and class files.

use super::file_object::FileObject;
use super::java_file_object::{JavaFileObject, Kind};
use super::option_checker::OptionChecker;
use crate::error::Error;
use crate::jdk::{Class, ClassLoader, ServiceLoader};
use javac_strings::JavaString;
use std::rc::Rc;

// PORTING_RULES: shared-reference aliases for the Java interfaces, so the
// signatures below read like the upstream ones.
/// FileObject reference (Java: a FileObject instance).
pub type FileObjectRef = Rc<dyn FileObject>;
/// JavaFileObject reference (Java: a JavaFileObject instance).
pub type JavaFileObjectRef = Rc<dyn JavaFileObject>;
/// Location reference (Java: a Location instance).
pub type LocationRef = Rc<dyn Location>;

/// Interface for locations of file objects.  Used by file managers
/// to determine where to place or search for file objects.
///
/// <p>Informally, a Location corresponds to a "search path", such as a class
/// path or module path, as used by command-line tools that use the default file system.
///
/// <p>Some locations are typically used to identify a place in which
/// a tool can find files to be read; others are typically used to identify
/// a place where a tool can write files. If a location is used to identify
/// a place for reading files, those files may be organized in a simple
/// <em>package/class</em> hierarchy: such locations are described as
/// <strong>package-oriented</strong>.
/// Alternatively, the files may be organized in a <em>module/package/class</em>
/// hierarchy: such locations are described as <strong>module-oriented</strong>.
/// If a location is typically used to identify a place where a tool can write files,
/// it is up to the tool that writes the files to specify how those files will be
/// organized.
///
/// <p>You can access the classes in a package-oriented location using methods like
/// {@link JavaFileManager#getJavaFileForInput} or {@link JavaFileManager#list}.
/// It is not possible to directly list the classes in a module-oriented
/// location. Instead, you can get a package-oriented location for any specific module
/// using methods like {@link JavaFileManager#getLocationForModule} or
/// {@link JavaFileManager#listLocationsForModules}.
///
/// PORTING_RULES: Java nested interface JavaFileManager.Location -> Rust trait
/// Location in the module of JavaFileManager (Rust traits cannot be nested
/// inside traits).  get_name returns a Java String (javac_strings::JavaString).
pub trait Location {
    /// Returns the name of this location.
    ///
    /// @return a name
    fn get_name(&self) -> JavaString;

    /// Determines if this is an output location.
    /// An output location is a location that is conventionally used for
    /// output.
    ///
    /// @apiNote An output location may be used to write files in either
    /// a package-oriented organization or in a module-oriented organization.
    ///
    /// @return true if this is an output location, false otherwise
    fn is_output_location(&self) -> bool;

    /// Indicates if this location is module-oriented location, and therefore
    /// expected to contain classes in a <em>module/package/class</em>
    /// hierarchy, as compared to a package-oriented location, which
    /// is expected to contain classes in a <em>package/class</em> hierarchy.
    /// The result of this method is undefined if this is an output
    /// location.
    ///
    /// @implNote This implementation returns true if the name includes
    /// the word "MODULE".
    ///
    /// @return true if this location is expected to contain modules
    /// @since 9
    fn is_module_oriented_location(&self) -> bool {
        // Java: return getName().matches("\\bMODULE\\b");
        // PORTING_RULES: java.util.regex.Matcher.matches requires the entire
        // input sequence to match the pattern.  A word boundary (backslash-b)
        // matches between a word character and a non-word character (or the
        // ends of the input); therefore the pattern matches an input only if
        // the whole input is exactly "MODULE".  The upstream expression is
        // reproduced by this equality check (itself a suspicious construct,
        // but javac behavior is copied as-is, docs/PLAN.md §2.3).
        name_matches_module_word(&self.get_name())
    }
}

/// Java: getName().matches(pattern) with the module word-boundary pattern
/// documented on the default method above.
fn name_matches_module_word(name: &JavaString) -> bool {
    *name == JavaString::from("MODULE")
}

/// File manager for tools operating on Java programming language
/// source and class files.  In this context, <em>file</em> means an
/// abstraction of regular files and other sources of data.
///
/// <p>When constructing new JavaFileObjects, the file manager must
/// determine where to create them.  For example, if a file manager
/// manages regular files on a file system, it would most likely have a
/// current/working directory to use as default location when creating
/// or finding files.  A number of hints can be provided to a file
/// manager as to where to create files.  Any file manager might choose
/// to ignore these hints.
///
/// <p>Some methods in this interface use class names.  Such class
/// names must be given in the Java Virtual Machine internal form of
/// fully qualified class and interface names.  For convenience '.' and '/'
/// are interchangeable.  The internal form is defined in chapter four of
/// <cite>The Java Virtual Machine Specification</cite>.
///
/// <blockquote><p>
///   <i>Discussion:</i> this means that the names
///   "java/lang.package-info", "java/lang/package-info",
///   "java.lang.package-info", are valid and equivalent.  Compare to
///   binary name as defined in
///   <cite>The Java Language Specification</cite>,
///   section 13.1 "The Form of a Binary".
/// </p></blockquote>
///
/// <p>The case of names is significant.  All names should be treated
/// as case-sensitive.  For example, some file systems have
/// case-insensitive, case-aware file names.  File objects representing
/// such files should take care to preserve case by using {@link
/// java.io.File#getCanonicalFile} or similar means.  If the system is
/// not case-aware, file objects must use other means to preserve case.
///
/// <p><em><a id="relative_name">Relative names</a>:</em> some
/// methods in this interface use relative names.  A relative name is a
/// non-null, non-empty sequence of path segments separated by '/'.
/// '.' or '..'  are invalid path segments.  A valid relative name must
/// match the "path-rootless" rule of RFC 3986, section 3.3.  Informally,
/// this should be true:
///
/// <pre>  URI.create(relativeName).normalize().getPath().equals(relativeName)</pre>
///
/// <p>All methods in this interface might throw a SecurityException.
///
/// <p>An object of this interface is not required to support
/// multi-threaded access, that is, be synchronized.  However, it must
/// support concurrent access to different file objects created by this
/// object.
///
/// <p><em>Implementation note:</em> a consequence of this requirement
/// is that a trivial implementation of output to a {@linkplain
/// java.util.jar.JarOutputStream} is not a sufficient implementation.
/// That is, rather than creating a JavaFileObject that returns the
/// JarOutputStream directly, the contents must be cached until closed
/// and then written to the JarOutputStream.
///
/// <p>Unless explicitly allowed, all methods in this interface might
/// throw a NullPointerException if given a null argument.
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @see JavaFileObject
/// @see FileObject
/// @since 1.6
///
/// PORTING_RULES — signature mapping:
/// * Closeable/Flushable -> flush()/close() methods returning Result of
///   Error (close is not Drop: Java close is explicit and observable).
/// * Iterable of JavaFileObject -> Vec of JavaFileObjectRef (the ported file
///   managers build a list, exactly like the upstream JavacFileManager).
/// * Set of Kind kinds -> slice of Kind (the upstream argument is an EnumSet;
///   the port keeps the element order given by the caller).
/// * Iterator of String remaining -> &mut dyn Iterator of JavaString
///   (the Java method consumes elements from the iterator).
/// * Java String parameters -> &javac_strings::JavaString; nullable
///   FileObject sibling -> Option of FileObjectRef.
/// * ClassLoader, Class and ServiceLoader -> the minimal placeholders of
///   crate::jdk.
/// * getLocationForModule has two overloads; the JavaFileObject one becomes
///   get_location_for_module_of_file (docs/PORTING_RULES.md §3).
/// * getServiceLoader is generic: it keeps "where Self: Sized" so the trait
///   stays usable as dyn JavaFileManager.  Annotation processing is out of
///   scope, so no upstream code calls it through dyn.
/// * dyn JavaFileManager must be usable: P6 JavaCompiler accepts a
///   user-supplied manager through Rc of dyn JavaFileManager.
pub trait JavaFileManager: OptionChecker {
    /// Returns a class loader for loading plug-ins from the given
    /// package-oriented location.
    /// For example, to load annotation processors,
    /// a compiler will request a class loader for the {@link
    /// StandardLocation#ANNOTATION_PROCESSOR_PATH
    /// ANNOTATION_PROCESSOR_PATH} location.
    ///
    /// @param location a location
    /// @return a class loader for the given location; or null
    /// if loading plug-ins from the given location is disabled or if
    /// the location is not known
    /// @throws SecurityException if a class loader can not be created
    /// in the current security context
    /// @throws IllegalStateException if {@link #close} has been called
    /// and this file manager cannot be reopened
    /// @throws IllegalArgumentException if the location is a module-oriented location
    fn get_class_loader(&self, location: &dyn Location) -> Option<Rc<dyn ClassLoader>>;

    /// Lists all file objects matching the given criteria in the given
    /// package-oriented location.
    /// List file objects in "subpackages" if recurse is true.
    ///
    /// <p>Note: even if the given location is unknown to this file
    /// manager, it may not return null.  Also, an unknown
    /// location may not cause an exception.
    ///
    /// @param location     a location
    /// @param packageName  a package name
    /// @param kinds        return objects only of these kinds
    /// @param recurse      if true include "subpackages"
    /// @return an Iterable of file objects matching the given criteria
    /// @throws IOException if an I/O error occurred, or if {@link
    /// #close} has been called and this file manager cannot be
    /// reopened
    /// @throws IllegalArgumentException if the location is a module-oriented location
    /// @throws IllegalStateException if {@link #close} has been called
    /// and this file manager cannot be reopened
    fn list(
        &self,
        location: &dyn Location,
        package_name: &JavaString,
        kinds: &[Kind],
        recurse: bool,
    ) -> Result<Vec<JavaFileObjectRef>, Error>;

    /// Infers a binary name of a file object based on a package-oriented location.
    /// The binary name returned might not be a valid binary name according to
    /// <cite>The Java Language Specification</cite>.
    ///
    /// @param location a location
    /// @param file a file object
    /// @return a binary name or null the file object is not
    /// found in the given location
    /// @throws IllegalArgumentException if the location is a module-oriented location
    /// @throws IllegalStateException if {@link #close} has been called
    /// and this file manager cannot be reopened
    fn infer_binary_name(
        &self,
        location: &dyn Location,
        file: &JavaFileObjectRef,
    ) -> Option<JavaString>;

    /// Compares two file objects and return true if they represent the
    /// same underlying object.
    ///
    /// @param a a file object
    /// @param b a file object
    /// @return true if the given file objects represent the same
    /// underlying object
    ///
    /// @throws IllegalArgumentException if either of the arguments
    /// were created with another file manager and this file manager
    /// does not support foreign file objects
    fn is_same_file(&self, a: &FileObjectRef, b: &FileObjectRef) -> bool;

    /// Handles one option.  If current is an option to this
    /// file manager it will consume any arguments to that option from
    /// remaining and return true, otherwise return false.
    ///
    /// @param current current option
    /// @param remaining remaining options
    /// @return true if this option was handled by this file manager,
    /// false otherwise
    /// @throws IllegalArgumentException if this option to this file
    /// manager is used incorrectly
    /// @throws IllegalStateException if {@link #close} has been called
    /// and this file manager cannot be reopened
    fn handle_option(
        &self,
        current: &JavaString,
        remaining: &mut dyn Iterator<Item = JavaString>,
    ) -> bool;

    /// Determines if a location is known to this file manager.
    ///
    /// @param location a location
    /// @return true if the location is known
    fn has_location(&self, location: &dyn Location) -> bool;

    /// Returns a {@linkplain JavaFileObject file object} for input
    /// representing the specified class of the specified kind in the
    /// given package-oriented location.
    ///
    /// @param location a location
    /// @param className the name of a class
    /// @param kind the kind of file, must be one of {@link
    /// JavaFileObject.Kind#SOURCE SOURCE} or {@link
    /// JavaFileObject.Kind#CLASS CLASS}
    /// @return a file object, might return null if the
    /// file does not exist
    /// @throws IllegalArgumentException if the location is not known
    /// to this file manager and the file manager does not support
    /// unknown locations, or if the kind is not valid, or if the
    /// location is a module-oriented location
    /// @throws IOException if an I/O error occurred, or if {@link
    /// #close} has been called and this file manager cannot be
    /// reopened
    /// @throws IllegalStateException if {@link #close} has been called
    /// and this file manager cannot be reopened
    fn get_java_file_for_input(
        &self,
        location: &dyn Location,
        class_name: &JavaString,
        kind: Kind,
    ) -> Result<Option<JavaFileObjectRef>, Error>;

    /// Returns a {@linkplain JavaFileObject file object} for output
    /// representing the specified class of the specified kind in the
    /// given package-oriented location.
    ///
    /// <p>Optionally, this file manager might consider the sibling as
    /// a hint for where to place the output.  The exact semantics of
    /// this hint is unspecified.  The JDK compiler, javac, for
    /// example, will place class files in the same directories as
    /// originating source files unless a class file output directory
    /// is provided.  To facilitate this behavior, javac might provide
    /// the originating source file as sibling when calling this
    /// method.
    ///
    /// @param location a package-oriented location
    /// @param className the name of a class
    /// @param kind the kind of file, must be one of {@link
    /// JavaFileObject.Kind#SOURCE SOURCE} or {@link
    /// JavaFileObject.Kind#CLASS CLASS}
    /// @param sibling a file object to be used as hint for placement;
    /// might be null
    /// @return a file object for output
    /// @throws IllegalArgumentException if sibling is not known to
    /// this file manager, or if the location is not known to this file
    /// manager and the file manager does not support unknown
    /// locations, or if the kind is not valid, or if the location is
    /// not an output location
    /// @throws IOException if an I/O error occurred, or if {@link
    /// #close} has been called and this file manager cannot be
    /// reopened
    /// @throws IllegalStateException {@link #close} has been called
    /// and this file manager cannot be reopened
    fn get_java_file_for_output(
        &self,
        location: &dyn Location,
        class_name: &JavaString,
        kind: Kind,
        sibling: Option<&FileObjectRef>,
    ) -> Result<JavaFileObjectRef, Error>;

    /// Returns a {@linkplain FileObject file object} for input
    /// representing the specified <a href="JavaFileManager.html#relative_name">relative
    /// name</a> in the specified package in the given package-oriented location.
    ///
    /// <p>If the returned object represents a {@linkplain
    /// JavaFileObject.Kind#SOURCE source} or {@linkplain
    /// JavaFileObject.Kind#CLASS class} file, it must be an instance
    /// of {@link JavaFileObject}.
    ///
    /// <p>Informally, the file object returned by this method is
    /// located in the concatenation of the location, package name, and
    /// relative name.  For example, to locate the properties file
    /// "resources/compiler.properties" in the package
    /// "com.sun.tools.javac" in the {@linkplain
    /// StandardLocation#SOURCE_PATH SOURCE_PATH} location, this method
    /// might be called like so:
    ///
    /// <pre>getFileForInput(SOURCE_PATH, "com.sun.tools.javac", "resources/compiler.properties");</pre>
    ///
    /// @param location a package-oriented location
    /// @param packageName a package name
    /// @param relativeName a relative name
    /// @return a file object, might return null if the file
    /// does not exist
    /// @throws IllegalArgumentException if the location is not known
    /// to this file manager and the file manager does not support
    /// unknown locations, or if relativeName is not valid,
    /// or if the location is a module-oriented location
    /// @throws IOException if an I/O error occurred, or if {@link
    /// #close} has been called and this file manager cannot be
    /// reopened
    /// @throws IllegalStateException if {@link #close} has been called
    /// and this file manager cannot be reopened
    fn get_file_for_input(
        &self,
        location: &dyn Location,
        package_name: &JavaString,
        relative_name: &JavaString,
    ) -> Result<Option<FileObjectRef>, Error>;

    /// Returns a {@linkplain FileObject file object} for output
    /// representing the specified <a href="JavaFileManager.html#relative_name">relative
    /// name</a> in the specified package in the given location.
    ///
    /// <p>Optionally, this file manager might consider the sibling as
    /// a hint for where to place the output.  The exact semantics of
    /// this hint is unspecified.  The JDK compiler, javac, for
    /// example, will place class files in the same directories as
    /// originating source files unless a class file output directory
    /// is provided.  To facilitate this behavior, javac might provide
    /// the originating source file as sibling when calling this
    /// method.
    ///
    /// <p>If the returned object represents a {@linkplain
    /// JavaFileObject.Kind#SOURCE source} or {@linkplain
    /// JavaFileObject.Kind#CLASS class} file, it must be an instance
    /// of {@link JavaFileObject}.
    ///
    /// <p>Informally, the file object returned by this method is
    /// located in the concatenation of the location, package name, and
    /// relative name or next to the sibling argument.  See {@link
    /// #getFileForInput getFileForInput} for an example.
    ///
    /// @param location an output location
    /// @param packageName a package name
    /// @param relativeName a relative name
    /// @param sibling a file object to be used as hint for placement;
    /// might be null
    /// @return a file object
    /// @throws IllegalArgumentException if sibling is not known to
    /// this file manager, or if the location is not known to this file
    /// manager and the file manager does not support unknown
    /// locations, or if relativeName is not valid,
    /// or if the location is not an output location
    /// @throws IOException if an I/O error occurred, or if {@link
    /// #close} has been called and this file manager cannot be
    /// reopened
    /// @throws IllegalStateException if {@link #close} has been called
    /// and this file manager cannot be reopened
    fn get_file_for_output(
        &self,
        location: &dyn Location,
        package_name: &JavaString,
        relative_name: &JavaString,
        sibling: Option<&FileObjectRef>,
    ) -> Result<FileObjectRef, Error>;

    /// Flushes any resources opened for output by this file manager
    /// directly or indirectly.  Flushing a closed file manager has no
    /// effect.
    ///
    /// @throws IOException if an I/O error occurred
    /// @see #close
    fn flush(&self) -> Result<(), Error>;

    /// Releases any resources opened by this file manager directly or
    /// indirectly.  This might render this file manager useless and
    /// the effect of subsequent calls to methods on this object or any
    /// objects obtained through this object is undefined unless
    /// explicitly allowed.  However, closing a file manager which has
    /// already been closed has no effect.
    ///
    /// @throws IOException if an I/O error occurred
    /// @see #flush
    fn close(&self) -> Result<(), Error>;

    /// Gets a location for a named module within a location, which may be either
    /// a module-oriented location or an output location.
    /// The result will be an output location if the given location is
    /// an output location, or it will be a package-oriented location.
    ///
    /// @implSpec This implementation throws UnsupportedOperationException.
    ///
    /// @param location the module-oriented location
    /// @param moduleName the name of the module to be found
    /// @return the location for the named module
    ///
    /// @throws IOException if an I/O error occurred
    /// @throws UnsupportedOperationException if this operation if not supported by this file manager
    /// @throws IllegalArgumentException if the location is neither an output location nor a
    /// module-oriented location
    /// @since 9
    fn get_location_for_module(
        &self,
        location: &dyn Location,
        module_name: &JavaString,
    ) -> Result<LocationRef, Error> {
        // Failure modes: undocumented upstream.
        let _ = (location, module_name);
        Err(Error::UnsupportedOperationException)
    }

    /// Gets a location for the module containing a specific file
    /// to be found within a location, which may be either
    /// a module-oriented location or an output location.
    /// The result will be an output location if the given location is
    /// an output location, or it will be a package-oriented location.
    ///
    /// @implSpec This implementation throws UnsupportedOperationException.
    ///
    /// @param location the module-oriented location
    /// @param fo the file
    /// @return the module containing the file
    ///
    /// @throws IOException if an I/O error occurred
    /// @throws UnsupportedOperationException if this operation if not supported by this file manager
    /// @throws IllegalArgumentException if the location is neither an output location nor a
    /// module-oriented location
    /// @since 9
    ///
    /// PORTING_RULES: overload getLocationForModule(Location, JavaFileObject)
    /// -> get_location_for_module_of_file.
    fn get_location_for_module_of_file(
        &self,
        location: &dyn Location,
        fo: &JavaFileObjectRef,
    ) -> Result<LocationRef, Error> {
        let _ = (location, fo);
        Err(Error::UnsupportedOperationException)
    }

    /// Get a service loader for a specific service class from a given location.
    ///
    /// If the location is a module-oriented location, the service loader will use the
    /// service declarations in the modules found in that location. Otherwise, a service loader
    /// is created using the package-oriented location, in which case, the services are
    /// determined using the provider-configuration files in META-INF/services.
    ///
    /// @implSpec This implementation throws UnsupportedOperationException.
    ///
    /// @param location the module-oriented location
    /// @param service  the Class object of the service class
    /// @param <S> the service class
    /// @return a service loader for the given service class
    ///
    /// @throws IOException if an I/O error occurred
    /// @throws UnsupportedOperationException if this operation if not supported by this file manager
    /// @since 9
    fn get_service_loader<S>(
        &self,
        location: &dyn Location,
        service: Class<S>,
    ) -> Result<ServiceLoader<S>, Error>
    where
        Self: Sized,
    {
        // Failure modes: undocumented upstream.
        let _ = (location, service);
        Err(Error::UnsupportedOperationException)
    }

    /// Infer the name of the module from its location, as returned by
    /// getLocationForModule or listModuleLocations.
    ///
    /// @implSpec This implementation throws UnsupportedOperationException.
    ///
    /// @param location a package-oriented location representing a module
    /// @return the name of the module
    ///
    /// @throws IOException if an I/O error occurred
    /// @throws UnsupportedOperationException if this operation if not supported by this file manager
    /// @throws IllegalArgumentException if the location is not one known to this file manager
    /// @since 9
    fn infer_module_name(&self, location: &dyn Location) -> Result<JavaString, Error> {
        // Failure modes: undocumented upstream.
        let _ = location;
        Err(Error::UnsupportedOperationException)
    }

    /// Lists the locations for all the modules in a module-oriented location or an output location.
    /// The locations that are returned will be output locations if the given location is an output,
    /// or it will be a package-oriented locations.
    ///
    /// @implSpec This implementation throws UnsupportedOperationException.
    ///
    /// @param location  the module-oriented location for which to list the modules
    /// @return  a series of sets of locations containing modules
    ///
    /// @throws IOException if an I/O error occurred
    /// @throws UnsupportedOperationException if this operation if not supported by this file manager
    /// @throws IllegalArgumentException if the location is not a module-oriented location
    /// @since 9
    ///
    /// PORTING_RULES: Iterable of Set of Location -> Vec of Vec of LocationRef.
    fn list_locations_for_modules(
        &self,
        location: &dyn Location,
    ) -> Result<Vec<Vec<LocationRef>>, Error> {
        // Failure modes: undocumented upstream.
        let _ = location;
        Err(Error::UnsupportedOperationException)
    }

    /// Determines whether or not a given file object is "contained in" a specified location.
    ///
    /// <p>For a package-oriented location, a file object is contained in the location if there exist
    /// values for <i>packageName</i> and <i>relativeName</i> such that either of the following
    /// calls would return the {@link #isSameFile same} file object:
    /// <pre>
    ///     getFileForInput(location, <i>packageName</i>, <i>relativeName</i>)
    ///     getFileForOutput(location, <i>packageName</i>, <i>relativeName</i>, null)
    /// </pre>
    ///
    /// <p>For a module-oriented location, a file object is contained in the location if there exists
    /// a module that may be obtained by the call:
    /// <pre>
    ///     getLocationForModule(location, <i>moduleName</i>)
    /// </pre>
    /// such that the file object is contained in the (package-oriented) location for that module.
    ///
    /// @implSpec This implementation throws UnsupportedOperationException.
    ///
    /// @param location the location
    /// @param fo the file object
    /// @return whether or not the file is contained in the location
    ///
    /// @throws IOException if there is a problem determining the result
    /// @throws UnsupportedOperationException if the method is not supported
    ///
    /// @since 9
    fn contains(&self, location: &dyn Location, fo: &FileObjectRef) -> Result<bool, Error> {
        let _ = (location, fo);
        Err(Error::UnsupportedOperationException)
    }
}

#[cfg(test)]
mod tests {
    use super::{FileObjectRef, JavaFileManager, JavaFileObjectRef, Location, LocationRef};
    use crate::error::Error;
    use crate::java_io::{Reader, Writer};
    use crate::jdk::{Class, ClassLoader, ServiceLoader};
    use crate::lang::model::element::modifier::Modifier;
    use crate::lang::model::element::nesting_kind::NestingKind;
    use crate::tools::file_object::FileObject;
    use crate::tools::java_file_object::{JavaFileObject, Kind};
    use crate::tools::option_checker::OptionChecker;
    use crate::uri::URI;
    use javac_strings::JavaString;
    use std::io::{Read, Write};
    use std::rc::Rc;

    fn js(s: &str) -> JavaString {
        JavaString::from(s)
    }

    // A minimal location implementation ("package-oriented, not output").
    struct TestLocation(&'static str);

    impl Location for TestLocation {
        fn get_name(&self) -> JavaString {
            js(self.0)
        }

        fn is_output_location(&self) -> bool {
            false
        }
    }

    // A minimal file object implementation, only used to exercise the trait.
    struct TestFile;

    impl FileObject for TestFile {
        fn to_uri(&self) -> URI {
            URI::create(&js("mem:///Test.java"))
        }
        fn get_name(&self) -> JavaString {
            js("/Test.java")
        }
        fn open_input_stream(&self) -> Result<Box<dyn Read>, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn open_output_stream(&self) -> Result<Box<dyn Write>, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn open_reader(&self, _ignore: bool) -> Result<Box<dyn Reader>, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn get_char_content(&self, _ignore: bool) -> Result<Option<JavaString>, Error> {
            Ok(None)
        }
        fn open_writer(&self) -> Result<Box<dyn Writer>, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn get_last_modified(&self) -> i64 {
            0
        }
        fn delete(&self) -> bool {
            false
        }
    }

    impl JavaFileObject for TestFile {
        fn get_kind(&self) -> Kind {
            Kind::SOURCE
        }
        fn is_name_compatible(&self, simple_name: &JavaString, kind: Kind) -> bool {
            kind == Kind::SOURCE && *simple_name == js("Test")
        }
        fn get_nesting_kind(&self) -> Option<NestingKind> {
            None
        }
        fn get_access_level(&self) -> Option<Modifier> {
            None
        }
    }

    struct TestManager;

    impl OptionChecker for TestManager {
        fn is_supported_option(&self, _option: &JavaString) -> i32 {
            -1
        }
    }

    impl JavaFileManager for TestManager {
        fn get_class_loader(&self, _location: &dyn Location) -> Option<Rc<dyn ClassLoader>> {
            None
        }
        fn list(
            &self,
            _location: &dyn Location,
            _package_name: &JavaString,
            _kinds: &[Kind],
            _recurse: bool,
        ) -> Result<Vec<JavaFileObjectRef>, Error> {
            Ok(vec![Rc::new(TestFile)])
        }
        fn infer_binary_name(
            &self,
            _location: &dyn Location,
            _file: &JavaFileObjectRef,
        ) -> Option<JavaString> {
            None
        }
        fn is_same_file(&self, a: &FileObjectRef, b: &FileObjectRef) -> bool {
            Rc::ptr_eq(a, b)
        }
        fn handle_option(
            &self,
            _current: &JavaString,
            _remaining: &mut dyn Iterator<Item = JavaString>,
        ) -> bool {
            false
        }
        fn has_location(&self, _location: &dyn Location) -> bool {
            false
        }
        fn get_java_file_for_input(
            &self,
            _location: &dyn Location,
            _class_name: &JavaString,
            _kind: Kind,
        ) -> Result<Option<JavaFileObjectRef>, Error> {
            Ok(None)
        }
        fn get_java_file_for_output(
            &self,
            _location: &dyn Location,
            _class_name: &JavaString,
            _kind: Kind,
            _sibling: Option<&FileObjectRef>,
        ) -> Result<JavaFileObjectRef, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn get_file_for_input(
            &self,
            _location: &dyn Location,
            _package_name: &JavaString,
            _relative_name: &JavaString,
        ) -> Result<Option<FileObjectRef>, Error> {
            Ok(None)
        }
        fn get_file_for_output(
            &self,
            _location: &dyn Location,
            _package_name: &JavaString,
            _relative_name: &JavaString,
            _sibling: Option<&FileObjectRef>,
        ) -> Result<FileObjectRef, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn flush(&self) -> Result<(), Error> {
            Ok(())
        }
        fn close(&self) -> Result<(), Error> {
            Ok(())
        }
    }

    #[test]
    fn location_default_is_module_oriented() {
        // Java: getName().matches(module word-boundary pattern) — only the
        // exact name "MODULE" matches the whole input.
        let location = TestLocation("MODULE");
        assert!(location.is_module_oriented_location());
        assert!(!TestLocation("MODULE_PATH").is_module_oriented_location());
        assert!(!TestLocation("CLASS_PATH").is_module_oriented_location());
    }

    #[test]
    fn java_file_manager_is_object_safe() {
        // Hard requirement: P6 JavaCompiler accepts a user supplied
        // Rc<dyn JavaFileManager>.
        let manager: Rc<dyn JavaFileManager> = Rc::new(TestManager);
        let location = TestLocation("CLASS_PATH");
        let a: FileObjectRef = Rc::new(TestFile);
        let b: FileObjectRef = Rc::new(TestFile);
        assert!(!manager.is_same_file(&a, &b));
        assert!(!manager.has_location(&location));
        let listed = manager.list(&location, &js("p"), &[Kind::SOURCE], false).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].get_kind(), Kind::SOURCE);
        assert_eq!(manager.is_supported_option(&js("-d")), -1);
    }

    #[test]
    fn default_methods_are_unsupported_operation() {
        let manager: Rc<dyn JavaFileManager> = Rc::new(TestManager);
        let location = TestLocation("CLASS_PATH");
        assert_eq!(
            manager.get_location_for_module(&location, &js("m")).err(),
            Some(Error::UnsupportedOperationException)
        );
        let fo: JavaFileObjectRef = Rc::new(TestFile);
        assert_eq!(
            manager.get_location_for_module_of_file(&location, &fo).err(),
            Some(Error::UnsupportedOperationException)
        );
        assert_eq!(
            manager.infer_module_name(&location).err(),
            Some(Error::UnsupportedOperationException)
        );
        assert_eq!(
            manager.list_locations_for_modules(&location).err(),
            Some(Error::UnsupportedOperationException)
        );
        let fo_file: FileObjectRef = fo.clone() as FileObjectRef;
        assert_eq!(
            manager.contains(&location, &fo_file).err(),
            Some(Error::UnsupportedOperationException)
        );
    }

    #[test]
    fn sized_generic_default_method() {
        // getServiceLoader keeps "where Self: Sized"; it is callable on a
        // concrete manager (and intentionally not through dyn).
        let manager = TestManager;
        let location = TestLocation("CLASS_PATH");
        let result = manager.get_service_loader(&location, Class::<()>::new());
        assert_eq!(result.err(), Some(Error::UnsupportedOperationException));
        let _ = ServiceLoader::<()>::new();
        let _: LocationRef = Rc::new(TestLocation("MODULE"));
    }
}
