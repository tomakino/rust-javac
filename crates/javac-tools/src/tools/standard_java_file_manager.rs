//! File manager based on java.io.File and java.nio.file.Path.

use super::java_file_manager::{FileObjectRef, JavaFileManager, JavaFileObjectRef, Location};
use crate::error::Error;
use javac_strings::JavaString;
use std::path::PathBuf;
use std::rc::Rc;

/// A file manager based on java.io.File and java.nio.file.Path.
///
/// A common way to obtain an instance of this class is using
/// {@linkplain JavaCompiler#getStandardFileManager getStandardFileManager},
/// for example:
///
/// <pre>
///   JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
///   DiagnosticCollector&lt;JavaFileObject&gt; diagnostics =
///       new DiagnosticCollector&lt;JavaFileObject&gt;();
///   StandardJavaFileManager fm = compiler.getStandardFileManager(diagnostics, null, null);
/// </pre>
///
/// This file manager creates file objects representing regular
/// {@linkplain java.io.File files},
/// {@linkplain java.util.zip.ZipEntry zip file entries}, or entries in
/// similar file system based containers.  Any file object returned
/// from a file manager implementing this interface must observe the
/// following behavior:
///
/// <ul>
///   <li>File names need not be canonical.</li>
///   <li>
///     For file objects representing regular files
///     <ul>
///       <li>the method FileObject.delete() is equivalent to
///           java.io.File.delete(),</li>
///       <li>the method FileObject.getLastModified() is equivalent to
///           java.io.File.lastModified(),</li>
///       <li>the methods FileObject.getCharContent(boolean),
///           FileObject.openInputStream(), and
///           FileObject.openReader(boolean) must succeed if the following
///           would succeed (ignoring encoding issues):
///           <blockquote><pre>new FileInputStream(new File(fileObject.toUri()))</pre></blockquote>
///       </li>
///       <li>and the methods FileObject.openOutputStream(), and
///           FileObject.openWriter() must succeed if the following would
///           succeed (ignoring encoding issues):
///           <blockquote><pre>new FileOutputStream(new File(fileObject.toUri()))</pre></blockquote>
///       </li>
///     </ul>
///   </li>
///   <li>
///     The java.net.URI URI returned from FileObject.toUri()
///     <ul>
///       <li>must be absolute (have a schema), and</li>
///       <li>must have a normalized path component which can be resolved
///           without any process-specific context such as the current
///           directory (file names must be absolute).</li>
///     </ul>
///   </li>
/// </ul>
///
/// According to these rules, the following URIs, for example, are
/// allowed:
/// <ul>
///   <li>file:///C:/Documents%20and%20Settings/UncleBob/BobsApp/Test.java</li>
///   <li>jar:///C:/Documents%20and%20Settings/UncleBob/lib/vendorA.jar!/com/vendora/LibraryClass.class</li>
/// </ul>
/// Whereas these are not (reason in parentheses):
/// <ul>
///   <li>file:BobsApp/Test.java (the file name is relative and depend on the
///       current directory)</li>
///   <li>jar:lib/vendorA.jar!/com/vendora/LibraryClass.class (the first half
///       of the path depends on the current directory, whereas the component
///       after ! is legal)</li>
///   <li>Test.java (this URI depends on the current directory and does not
///       have a schema)</li>
///   <li>jar:///C:/Documents%20and%20Settings/UncleBob/BobsApp/../lib/vendorA.jar!com/vendora/LibraryClass.class
///       (the path is not normalized)</li>
/// </ul>
///
/// <p>All implementations of this interface must support Path objects representing
/// files in the default file system.  It is recommended that implementations
/// should support Path objects from any filesystem.</p>
///
/// @apiNote Some methods on this interface take a Collection of Path instead of
/// an Iterable of Path.  This is to prevent the possibility of accidentally
/// calling the method with a single Path as such an argument, because although
/// Path implements Iterable, it would almost never be correct to call these
/// methods with a single Path and have it be treated as an Iterable of its
/// components.
///
/// @author Peter von der Ah&eacute;
/// @since 1.6
///
/// — signature mapping:
/// * java.io.File and java.nio.file.Path both map to std::path::PathBuf (Rust
///   has a single OS path type).  The File and Path variants of the upstream
///   methods are kept as separate methods with distinct names so that the
///   upstream API surface stays visible, but their parameter types coincide.
/// * Iterable and Collection of File/Path -> slice of PathBuf.
/// * Iterable of JavaFileObject return -> Vec of JavaFileObjectRef.
/// * nullable search path (null = default search path) -> Option of slice.
/// * String parameters (file names) -> &javac_strings::JavaString.
/// * the private static helpers asPaths/asFiles/asCollection of the upstream
///   interface are identity conversions under the PathBuf mapping (a File is a
///   Path and vice versa); they are therefore not ported as functions.
/// * isSameFile is re-declared by upstream only to specialize the
///   documentation; it is inherited from JavaFileManager and not repeated here.
pub trait StandardJavaFileManager: JavaFileManager {
    /// Returns file objects representing the given files.
    ///
    /// @param files a list of files
    /// @return a list of file objects
    /// @throws IllegalArgumentException if the list of files includes
    /// a directory
    ///
    /// Java: getJavaFileObjectsFromFiles(Iterable of File).
    fn get_java_file_objects_from_files(&self, files: &[PathBuf]) -> Vec<JavaFileObjectRef>;

    /// Returns file objects representing the given paths.
    ///
    /// @implSpec
    /// The default implementation lazily converts each path to a file and calls
    /// {@link #getJavaFileObjectsFromFiles(Iterable) getJavaFileObjectsFromFiles}.
    /// IllegalArgumentException will be thrown
    /// if any of the paths cannot be converted to a file at the point the conversion happens.
    ///
    /// @param paths a list of paths
    /// @return a list of file objects
    /// @throws IllegalArgumentException if the list of paths includes
    /// a directory or if this file manager does not support any of the
    /// given paths
    ///
    /// @since 13
    ///
    /// overload getJavaFileObjectsFromPaths(Collection of Path)
    /// -> get_java_file_objects_from_paths.
    fn get_java_file_objects_from_paths(&self, paths: &[PathBuf]) -> Vec<JavaFileObjectRef> {
        // Java: return getJavaFileObjectsFromFiles(asFiles(paths));
        self.get_java_file_objects_from_files(paths)
    }

    /// Returns file objects representing the given paths.
    ///
    /// @implSpec
    /// The default implementation lazily converts each path to a file and calls
    /// {@link #getJavaFileObjectsFromPaths(Collection) getJavaFileObjectsFromPaths}.
    /// IllegalArgumentException will be thrown
    /// if any of the paths cannot be converted to a file at the point the conversion happens.
    ///
    /// @param paths a list of paths
    /// @return a list of file objects
    /// @throws IllegalArgumentException if the list of paths includes
    /// a directory or if this file manager does not support any of the
    /// given paths.
    ///
    /// @since 9
    /// @deprecated use {@link #getJavaFileObjectsFromPaths(Collection)} instead,
    /// to prevent the possibility of accidentally calling the method with a
    /// single Path as such an argument. Although Path implements
    /// Iterable, it would almost never be correct to pass a single
    /// Path and have it be treated as an Iterable of its
    /// components.
    ///
    /// overload getJavaFileObjectsFromPaths(Iterable of Path)
    /// -> get_java_file_objects_from_paths_iterable.
    #[deprecated(
        note = "Java: deprecated since 13; use get_java_file_objects_from_paths(Collection)"
    )]
    fn get_java_file_objects_from_paths_iterable(
        &self,
        paths: &[PathBuf],
    ) -> Vec<JavaFileObjectRef> {
        // Java: return getJavaFileObjectsFromPaths(asCollection(paths));
        self.get_java_file_objects_from_paths(paths)
    }

    /// Returns file objects representing the given files.
    /// Convenience method equivalent to:
    ///
    /// <pre>
    ///     getJavaFileObjectsFromFiles(Arrays.asList(files))
    /// </pre>
    ///
    /// @param files an array of files
    /// @return a list of file objects
    /// @throws IllegalArgumentException if the array of files includes
    /// a directory or if this file manager does not support any of the
    /// given paths
    /// @throws NullPointerException if the given array contains null
    /// elements
    ///
    /// Java: getJavaFileObjects(File...).
    fn get_java_file_objects(&self, files: &[PathBuf]) -> Vec<JavaFileObjectRef>;

    /// Returns file objects representing the given paths.
    /// Convenience method equivalent to:
    ///
    /// <pre>
    ///     getJavaFileObjectsFromPaths(Arrays.asList(paths))
    /// </pre>
    ///
    /// @implSpec
    /// The default implementation will only throw NullPointerException
    /// if {@linkplain #getJavaFileObjectsFromPaths(Collection)} throws it.
    ///
    /// @param paths an array of paths
    /// @return a list of file objects
    /// @throws IllegalArgumentException if the array of files includes
    /// a directory or if this file manager does not support any of the
    /// given paths
    /// @throws NullPointerException if the given array contains null
    /// elements
    ///
    /// @since 9
    ///
    /// overload getJavaFileObjects(Path...)
    /// -> get_java_file_objects_paths.
    fn get_java_file_objects_paths(&self, paths: &[PathBuf]) -> Vec<JavaFileObjectRef> {
        // Java: return getJavaFileObjectsFromPaths(Arrays.asList(paths));
        self.get_java_file_objects_from_paths(paths)
    }

    /// Returns file objects representing the given file names.
    ///
    /// @param names a list of file names
    /// @return a list of file objects
    /// @throws IllegalArgumentException if the list of file names
    /// includes a directory
    ///
    /// Java: getJavaFileObjectsFromStrings(Iterable of String).
    fn get_java_file_objects_from_strings(&self, names: &[JavaString]) -> Vec<JavaFileObjectRef>;

    /// Returns file objects representing the given file names.
    /// Convenience method equivalent to:
    ///
    /// <pre>
    ///     getJavaFileObjectsFromStrings(Arrays.asList(names))
    /// </pre>
    ///
    /// @param names a list of file names
    /// @return a list of file objects
    /// @throws IllegalArgumentException if the array of file names
    /// includes a directory
    /// @throws NullPointerException if the given array contains null
    /// elements
    ///
    /// Java: getJavaFileObjects(String...).
    ///
    /// overload getJavaFileObjects(String...)
    /// -> get_java_file_objects_strings.
    fn get_java_file_objects_strings(&self, names: &[JavaString]) -> Vec<JavaFileObjectRef>;

    /// Associates the given search path with the given location.  Any
    /// previous value will be discarded.
    ///
    /// If the location is a module-oriented or output location, any module-specific
    /// associations set up by {@linkplain #setLocationForModule setLocationForModule}
    /// will be cancelled.
    ///
    /// @param location a location
    /// @param files a list of files, if null use the default
    /// search path for this location
    /// @see #getLocation
    /// @throws IllegalArgumentException if location is an output
    /// location and files does not contain exactly one element
    /// @throws IOException if location is an output location and
    /// does not represent an existing directory
    fn set_location(&self, location: &dyn Location, files: Option<&[PathBuf]>)
    -> Result<(), Error>;

    /// Associates the given search path with the given location.
    /// Any previous value will be discarded.
    ///
    /// If the location is a module-oriented or output location, any module-specific
    /// associations set up by {@linkplain #setLocationForModule setLocationForModule}
    /// will be cancelled.
    ///
    /// @implSpec
    /// The default implementation lazily converts each path to a file and calls
    /// {@link #setLocation setLocation}.
    /// IllegalArgumentException will be thrown if any of the paths cannot
    /// be converted to a file at the point the conversion happens.
    ///
    /// @param location a location
    /// @param paths a list of paths, if null use the default
    /// search path for this location
    /// @see #getLocation
    /// @throws IllegalArgumentException if location is an output
    /// location and paths does not contain exactly one element
    /// or if this file manager does not support any of the given paths
    /// @throws IOException if location is an output location and
    /// paths does not represent an existing directory
    ///
    /// @since 9
    fn set_location_from_paths(
        &self,
        location: &dyn Location,
        paths: Option<&[PathBuf]>,
    ) -> Result<(), Error> {
        // Java: setLocation(location, asFiles(paths));
        self.set_location(location, paths)
    }

    /// Associates the given search path with the given module and location,
    /// which must be a module-oriented or output location.
    /// Any previous value will be discarded.
    /// This overrides any default association derived from the search path
    /// associated with the location itself.
    ///
    /// All such module-specific associations will be cancelled if a
    /// new search path is associated with the location by calling
    /// {@linkplain #setLocation setLocation} or
    /// {@linkplain #setLocationFromPaths setLocationFromPaths}.
    ///
    /// @throws IllegalStateException if the location is not a module-oriented
    ///  or output location.
    /// @throws UnsupportedOperationException if this operation is not supported by
    ///  this file manager.
    /// @throws IOException if location is an output location and
    /// paths does not represent an existing directory
    ///
    /// @param location the location
    /// @param moduleName the name of the module
    /// @param paths the search path to associate with the location and module.
    ///
    /// @see #setLocation
    /// @see #setLocationFromPaths
    ///
    /// @since 9
    fn set_location_for_module(
        &self,
        location: &dyn Location,
        module_name: &JavaString,
        paths: &[PathBuf],
    ) -> Result<(), Error> {
        let _ = (location, module_name, paths);
        Err(Error::UnsupportedOperationException)
    }

    /// Returns the search path associated with the given location.
    ///
    /// @param location a location
    /// @return a list of files or null if this location has no
    /// associated search path
    /// @throws IllegalStateException if any element of the search path
    /// cannot be converted to a java.io.File, or if the search path
    /// cannot be represented as a simple series of files.
    ///
    /// @see #setLocation
    fn get_location(&self, location: &dyn Location) -> Option<Vec<PathBuf>>;

    /// Returns the search path associated with the given location.
    ///
    /// @implSpec
    /// The default implementation calls {@link #getLocation getLocation}
    /// and then returns an Iterable formed by calling toPath()
    /// on each File returned from getLocation.
    ///
    /// @param location a location
    /// @return a list of paths or null if this location has no
    /// associated search path
    /// @throws IllegalStateException if the search path cannot be represented
    /// as a simple series of paths.
    ///
    /// @see #setLocationFromPaths
    /// @since 9
    fn get_location_as_paths(&self, location: &dyn Location) -> Option<Vec<PathBuf>> {
        // Java: return asPaths(getLocation(location));
        self.get_location(location)
    }

    /// Returns the path, if any, underlying this file object (optional operation).
    /// File objects derived from a java.nio.file.FileSystem,
    /// including the default file system, typically have a corresponding underlying
    /// java.nio.file.Path object. In such cases, this method may be
    /// used to access that object.
    ///
    /// @implSpec
    /// The default implementation throws UnsupportedOperationException
    /// for all files.
    ///
    /// @param file a file object
    /// @return a path representing the same underlying file system artifact
    /// @throws IllegalArgumentException if the file object does not have an underlying path
    /// @throws UnsupportedOperationException if the operation is not supported by this file manager
    ///
    /// @since 9
    fn as_path(&self, file: &FileObjectRef) -> Result<PathBuf, Error> {
        let _ = file;
        Err(Error::UnsupportedOperationException)
    }

    /// Specify a factory that can be used to generate a path from a string, or series of strings.
    ///
    /// If this method is not called, a factory whose getPath method is
    /// equivalent to calling
    /// {@link java.nio.file.Paths#get(String, String...) java.nio.file.Paths.get(first, more)}
    /// will be used.
    ///
    /// @implSpec
    /// The default implementation of this method ignores the factory that is provided.
    ///
    /// @param f  the factory
    ///
    /// @since 9
    fn set_path_factory(&self, f: Rc<dyn PathFactory>) {
        let _ = f;
    }
}

/// Factory to create Path objects from strings.
///
/// Java nested interface StandardJavaFileManager.PathFactory ->
/// Rust trait PathFactory (traits cannot be nested inside traits).  The path
/// strings are Java Strings; converting them to an OS path is the platform
/// boundary handled by the file layer (REGISTERED P2: platform path encoding).
///
/// @since 9
pub trait PathFactory {
    /// Converts a path string, or a sequence of strings that when joined form a path string, to a Path.
    ///
    /// @param first  the path string or initial part of the path string
    /// @param more   additional strings to be joined to form the path string
    /// @return       the resulting Path
    fn get_path(&self, first: &JavaString, more: &[JavaString]) -> PathBuf;
}

#[cfg(test)]
mod tests {
    use super::{PathFactory, StandardJavaFileManager};
    use crate::error::Error;
    use crate::jdk::ClassLoader;
    use crate::tools::file_object::FileObject;
    use crate::tools::forwarding_java_file_manager::ForwardingJavaFileManager;
    use crate::tools::java_file_manager::{
        FileObjectRef, JavaFileManager, JavaFileObjectRef, Location,
    };
    use crate::tools::java_file_object::Kind;
    use crate::tools::option_checker::OptionChecker;
    use crate::uri::URI;
    use javac_strings::JavaString;
    use std::path::PathBuf;
    use std::rc::Rc;

    fn js(s: &str) -> JavaString {
        JavaString::from(s)
    }

    struct Inner;

    struct InnerLocation;

    impl Location for InnerLocation {
        fn get_name(&self) -> JavaString {
            js("CLASS_PATH")
        }
        fn is_output_location(&self) -> bool {
            false
        }
    }

    impl OptionChecker for Inner {
        fn is_supported_option(&self, _option: &JavaString) -> i32 {
            -1
        }
    }

    impl JavaFileManager for Inner {
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
            Ok(vec![])
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

    // A test implementation: forwarding base + the StandardJavaFileManager
    // methods that have no default implementation.
    struct TestStandardManager {
        base: ForwardingJavaFileManager<Inner>,
        location: Option<Vec<PathBuf>>,
        files: Vec<JavaFileObjectRef>,
    }

    impl TestStandardManager {
        fn new() -> TestStandardManager {
            TestStandardManager {
                base: ForwardingJavaFileManager::new(Inner),
                location: None,
                files: Vec::new(),
            }
        }
    }

    impl OptionChecker for TestStandardManager {
        fn is_supported_option(&self, option: &JavaString) -> i32 {
            self.base.is_supported_option(option)
        }
    }

    impl JavaFileManager for TestStandardManager {
        fn get_class_loader(&self, location: &dyn Location) -> Option<Rc<dyn ClassLoader>> {
            self.base.get_class_loader(location)
        }
        fn list(
            &self,
            location: &dyn Location,
            package_name: &JavaString,
            kinds: &[Kind],
            recurse: bool,
        ) -> Result<Vec<JavaFileObjectRef>, Error> {
            self.base.list(location, package_name, kinds, recurse)
        }
        fn infer_binary_name(
            &self,
            location: &dyn Location,
            file: &JavaFileObjectRef,
        ) -> Option<JavaString> {
            self.base.infer_binary_name(location, file)
        }
        fn is_same_file(&self, a: &FileObjectRef, b: &FileObjectRef) -> bool {
            self.base.is_same_file(a, b)
        }
        fn handle_option(
            &self,
            current: &JavaString,
            remaining: &mut dyn Iterator<Item = JavaString>,
        ) -> bool {
            self.base.handle_option(current, remaining)
        }
        fn has_location(&self, location: &dyn Location) -> bool {
            self.base.has_location(location)
        }
        fn get_java_file_for_input(
            &self,
            location: &dyn Location,
            class_name: &JavaString,
            kind: Kind,
        ) -> Result<Option<JavaFileObjectRef>, Error> {
            self.base
                .get_java_file_for_input(location, class_name, kind)
        }
        fn get_java_file_for_output(
            &self,
            location: &dyn Location,
            class_name: &JavaString,
            kind: Kind,
            sibling: Option<&FileObjectRef>,
        ) -> Result<JavaFileObjectRef, Error> {
            self.base
                .get_java_file_for_output(location, class_name, kind, sibling)
        }
        fn get_file_for_input(
            &self,
            location: &dyn Location,
            package_name: &JavaString,
            relative_name: &JavaString,
        ) -> Result<Option<FileObjectRef>, Error> {
            self.base
                .get_file_for_input(location, package_name, relative_name)
        }
        fn get_file_for_output(
            &self,
            location: &dyn Location,
            package_name: &JavaString,
            relative_name: &JavaString,
            sibling: Option<&FileObjectRef>,
        ) -> Result<FileObjectRef, Error> {
            self.base
                .get_file_for_output(location, package_name, relative_name, sibling)
        }
        fn flush(&self) -> Result<(), Error> {
            self.base.flush()
        }
        fn close(&self) -> Result<(), Error> {
            self.base.close()
        }
    }

    impl StandardJavaFileManager for TestStandardManager {
        fn get_java_file_objects_from_files(&self, _files: &[PathBuf]) -> Vec<JavaFileObjectRef> {
            self.files.clone()
        }
        fn get_java_file_objects(&self, files: &[PathBuf]) -> Vec<JavaFileObjectRef> {
            self.get_java_file_objects_from_files(files)
        }
        fn get_java_file_objects_from_strings(
            &self,
            _names: &[JavaString],
        ) -> Vec<JavaFileObjectRef> {
            self.files.clone()
        }
        fn get_java_file_objects_strings(&self, names: &[JavaString]) -> Vec<JavaFileObjectRef> {
            self.get_java_file_objects_from_strings(names)
        }
        fn set_location(
            &self,
            _location: &dyn Location,
            _files: Option<&[PathBuf]>,
        ) -> Result<(), Error> {
            Ok(())
        }
        fn get_location(&self, _location: &dyn Location) -> Option<Vec<PathBuf>> {
            self.location.clone()
        }
    }

    // A minimal file object used to exercise the as_path default method.
    struct TestFile;

    impl FileObject for TestFile {
        fn to_uri(&self) -> URI {
            URI::create(&js("mem:///T.java"))
        }
        fn get_name(&self) -> JavaString {
            js("/T.java")
        }
        fn open_input_stream(&self) -> Result<Box<dyn std::io::Read>, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn open_output_stream(&self) -> Result<Box<dyn std::io::Write>, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn open_reader(&self, _ignore: bool) -> Result<Box<dyn crate::java_io::Reader>, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn get_char_content(&self, _ignore: bool) -> Result<Option<JavaString>, Error> {
            Ok(None)
        }
        fn open_writer(&self) -> Result<Box<dyn crate::java_io::Writer>, Error> {
            Err(Error::UnsupportedOperationException)
        }
        fn get_last_modified(&self) -> i64 {
            0
        }
        fn delete(&self) -> bool {
            false
        }
    }

    #[test]
    fn default_methods_delegate() {
        let manager = TestStandardManager::new();
        let location = InnerLocation;
        assert!(
            manager
                .get_java_file_objects_from_paths(&[PathBuf::from("a")])
                .is_empty()
        );
        assert!(
            manager
                .get_java_file_objects_paths(&[PathBuf::from("a")])
                .is_empty()
        );
        #[allow(deprecated)]
        {
            assert!(
                manager
                    .get_java_file_objects_from_paths_iterable(&[PathBuf::from("a")])
                    .is_empty()
            );
        }
        assert!(manager.get_location_as_paths(&location).is_none());
        assert!(manager.set_location_from_paths(&location, None).is_ok());
    }

    #[test]
    fn default_methods_are_unsupported_operation() {
        let manager = TestStandardManager::new();
        let location = InnerLocation;
        assert_eq!(
            manager
                .set_location_for_module(&location, &js("m"), &[PathBuf::from("a")])
                .err(),
            Some(Error::UnsupportedOperationException)
        );
        let file: FileObjectRef = Rc::new(TestFile);
        assert_eq!(
            manager.as_path(&file).err(),
            Some(Error::UnsupportedOperationException)
        );
    }

    struct TestPathFactory;

    impl PathFactory for TestPathFactory {
        fn get_path(&self, first: &JavaString, more: &[JavaString]) -> PathBuf {
            // Rust: converting a Java String to an OS path is the
            // platform-encoding boundary of the file layer (REGISTERED P2); the
            // lossy view is only used for this test factory.
            let mut path = PathBuf::from(first.to_string_lossy());
            for part in more {
                path.push(part.to_string_lossy());
            }
            path
        }
    }

    #[test]
    fn path_factory() {
        let factory = TestPathFactory;
        assert_eq!(
            factory.get_path(&js("a"), &[js("b"), js("c")]),
            PathBuf::from("a/b/c")
        );
        let manager = TestStandardManager::new();
        manager.set_path_factory(Rc::new(TestPathFactory));
    }

    #[test]
    fn standard_manager_is_object_safe() {
        let manager: Rc<dyn StandardJavaFileManager> = Rc::new(TestStandardManager::new());
        assert!(
            manager
                .get_java_file_objects_from_paths(&[PathBuf::from("x")])
                .is_empty()
        );
        let names = [js("A.java")];
        assert!(manager.get_java_file_objects_strings(&names).is_empty());
    }
}
