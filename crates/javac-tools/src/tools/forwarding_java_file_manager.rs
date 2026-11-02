//! Forwards calls to a given file manager.

use super::java_file_manager::{FileObjectRef, JavaFileManager, JavaFileObjectRef, Location, LocationRef};
use super::java_file_object::Kind;
use super::option_checker::OptionChecker;
use crate::error::Error;
use crate::jdk::{Class, ClassLoader, ServiceLoader};
use javac_strings::JavaString;
use std::rc::Rc;

/// Forwards calls to a given file manager.  Subclasses of this class
/// might override some of these methods and might also provide
/// additional fields and methods.
///
/// @param <M> the kind of file manager forwarded to by this object
/// @author Peter von der Ah&eacute;
/// @since 1.6
///
/// PORTING_RULES — class mapping:
/// * Java inheritance is not available in Rust; the class is expressed as a
///   struct with the upstream protected field and explicit forwarding (the
///   same shape as the single upstream field plus its overrides).
/// * the protected field fileManager -> pub field file_manager.
/// * the protected constructor -> pub constructor.
/// * the overridden default methods of JavaFileManager (getLocationForModule,
///   getServiceLoader, inferModuleName, listLocationsForModules, contains) are
///   forwarded as well, exactly as upstream.
pub struct ForwardingJavaFileManager<M: JavaFileManager> {
    /// The file manager which all methods are delegated to.
    ///
    /// Java: protected final M fileManager;
    pub file_manager: M,
}

impl<M: JavaFileManager> ForwardingJavaFileManager<M> {
    /// Creates a new instance of ForwardingJavaFileManager.
    /// @param fileManager delegate to this file manager
    ///
    /// Java: protected ForwardingJavaFileManager(M fileManager) {
    ///           this.fileManager = Objects.requireNonNull(fileManager); }
    pub fn new(file_manager: M) -> ForwardingJavaFileManager<M> {
        ForwardingJavaFileManager { file_manager }
    }
}

impl<M: JavaFileManager> OptionChecker for ForwardingJavaFileManager<M> {
    fn is_supported_option(&self, option: &JavaString) -> i32 {
        self.file_manager.is_supported_option(option)
    }
}

impl<M: JavaFileManager> JavaFileManager for ForwardingJavaFileManager<M> {
    /// @throws SecurityException {@inheritDoc}
    /// @throws IllegalStateException {@inheritDoc}
    fn get_class_loader(&self, location: &dyn Location) -> Option<Rc<dyn ClassLoader>> {
        self.file_manager.get_class_loader(location)
    }

    /// @throws IOException {@inheritDoc}
    /// @throws IllegalStateException {@inheritDoc}
    fn list(
        &self,
        location: &dyn Location,
        package_name: &JavaString,
        kinds: &[Kind],
        recurse: bool,
    ) -> Result<Vec<JavaFileObjectRef>, Error> {
        self.file_manager.list(location, package_name, kinds, recurse)
    }

    /// @throws IllegalStateException {@inheritDoc}
    fn infer_binary_name(
        &self,
        location: &dyn Location,
        file: &JavaFileObjectRef,
    ) -> Option<JavaString> {
        self.file_manager.infer_binary_name(location, file)
    }

    /// @throws IllegalArgumentException {@inheritDoc}
    fn is_same_file(&self, a: &FileObjectRef, b: &FileObjectRef) -> bool {
        self.file_manager.is_same_file(a, b)
    }

    /// @throws IllegalArgumentException {@inheritDoc}
    /// @throws IllegalStateException {@inheritDoc}
    fn handle_option(
        &self,
        current: &JavaString,
        remaining: &mut dyn Iterator<Item = JavaString>,
    ) -> bool {
        self.file_manager.handle_option(current, remaining)
    }

    fn has_location(&self, location: &dyn Location) -> bool {
        self.file_manager.has_location(location)
    }

    /// @throws IllegalArgumentException {@inheritDoc}
    /// @throws IllegalStateException {@inheritDoc}
    fn get_java_file_for_input(
        &self,
        location: &dyn Location,
        class_name: &JavaString,
        kind: Kind,
    ) -> Result<Option<JavaFileObjectRef>, Error> {
        self.file_manager.get_java_file_for_input(location, class_name, kind)
    }

    /// @throws IllegalArgumentException {@inheritDoc}
    /// @throws IllegalStateException {@inheritDoc}
    fn get_java_file_for_output(
        &self,
        location: &dyn Location,
        class_name: &JavaString,
        kind: Kind,
        sibling: Option<&FileObjectRef>,
    ) -> Result<JavaFileObjectRef, Error> {
        self.file_manager.get_java_file_for_output(location, class_name, kind, sibling)
    }

    /// @throws IllegalArgumentException {@inheritDoc}
    /// @throws IllegalStateException {@inheritDoc}
    fn get_file_for_input(
        &self,
        location: &dyn Location,
        package_name: &JavaString,
        relative_name: &JavaString,
    ) -> Result<Option<FileObjectRef>, Error> {
        self.file_manager.get_file_for_input(location, package_name, relative_name)
    }

    /// @throws IllegalArgumentException {@inheritDoc}
    /// @throws IllegalStateException {@inheritDoc}
    fn get_file_for_output(
        &self,
        location: &dyn Location,
        package_name: &JavaString,
        relative_name: &JavaString,
        sibling: Option<&FileObjectRef>,
    ) -> Result<FileObjectRef, Error> {
        self.file_manager.get_file_for_output(location, package_name, relative_name, sibling)
    }

    fn flush(&self) -> Result<(), Error> {
        self.file_manager.flush()
    }

    fn close(&self) -> Result<(), Error> {
        self.file_manager.close()
    }

    /// @since 9
    fn get_location_for_module(
        &self,
        location: &dyn Location,
        module_name: &JavaString,
    ) -> Result<LocationRef, Error> {
        self.file_manager.get_location_for_module(location, module_name)
    }

    /// @since 9
    fn get_location_for_module_of_file(
        &self,
        location: &dyn Location,
        fo: &JavaFileObjectRef,
    ) -> Result<LocationRef, Error> {
        self.file_manager.get_location_for_module_of_file(location, fo)
    }

    /// @since 9
    fn get_service_loader<S>(
        &self,
        location: &dyn Location,
        service: Class<S>,
    ) -> Result<ServiceLoader<S>, Error>
    where
        Self: Sized,
    {
        self.file_manager.get_service_loader(location, service)
    }

    /// @since 9
    fn infer_module_name(&self, location: &dyn Location) -> Result<JavaString, Error> {
        self.file_manager.infer_module_name(location)
    }

    /// @since 9
    fn list_locations_for_modules(
        &self,
        location: &dyn Location,
    ) -> Result<Vec<Vec<LocationRef>>, Error> {
        self.file_manager.list_locations_for_modules(location)
    }

    /// @since 9
    fn contains(&self, location: &dyn Location, fo: &FileObjectRef) -> Result<bool, Error> {
        self.file_manager.contains(location, fo)
    }
}

#[cfg(test)]
mod tests {
    use super::ForwardingJavaFileManager;
    use crate::error::Error;
    use crate::jdk::{Class, ClassLoader, ServiceLoader};
    use crate::tools::file_object::FileObject;
    use crate::tools::java_file_manager::{FileObjectRef, JavaFileManager, JavaFileObjectRef, Location};
    use crate::tools::java_file_object::{JavaFileObject, Kind};
    use crate::tools::option_checker::OptionChecker;
    use crate::uri::URI;
    use javac_strings::JavaString;
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

    struct InnerFile;

    impl FileObject for InnerFile {
        fn to_uri(&self) -> URI {
            URI::create(&js("mem:///Inner.java"))
        }
        fn get_name(&self) -> JavaString {
            js("/Inner.java")
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
            5
        }
        fn delete(&self) -> bool {
            false
        }
    }

    impl JavaFileObject for InnerFile {
        fn get_kind(&self) -> Kind {
            Kind::SOURCE
        }
        fn is_name_compatible(&self, simple_name: &JavaString, kind: Kind) -> bool {
            kind == Kind::SOURCE && *simple_name == js("Inner")
        }
        fn get_nesting_kind(&self) -> Option<crate::lang::model::element::nesting_kind::NestingKind> {
            None
        }
        fn get_access_level(&self) -> Option<crate::lang::model::element::modifier::Modifier> {
            None
        }
    }

    impl OptionChecker for Inner {
        fn is_supported_option(&self, option: &JavaString) -> i32 {
            if *option == js("-d") { 1 } else { -1 }
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
            Some(js("Inner"))
        }
        fn is_same_file(&self, a: &FileObjectRef, b: &FileObjectRef) -> bool {
            Rc::ptr_eq(a, b)
        }
        fn handle_option(
            &self,
            current: &JavaString,
            remaining: &mut dyn Iterator<Item = JavaString>,
        ) -> bool {
            if *current == js("-d") {
                let _ = remaining.next();
                true
            } else {
                false
            }
        }
        fn has_location(&self, _location: &dyn Location) -> bool {
            true
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
    fn forwards_calls() {
        let forwarding = ForwardingJavaFileManager::new(Inner);
        let location = InnerLocation;
        assert_eq!(forwarding.is_supported_option(&js("-d")), 1);
        assert!(forwarding.has_location(&location));
        assert!(forwarding.list(&location, &js("p"), &[Kind::SOURCE], false).unwrap().is_empty());
        assert!(forwarding.flush().is_ok());
        assert!(forwarding.close().is_ok());
        let fo: JavaFileObjectRef = Rc::new(InnerFile);
        assert_eq!(
            forwarding.infer_binary_name(&location, &fo),
            Some(js("Inner"))
        );
        // overridden default method is forwarded, so it uses the inner
        // manager's default implementation (UnsupportedOperationException).
        assert_eq!(
            forwarding.get_location_for_module(&location, &js("m")).err(),
            Some(Error::UnsupportedOperationException)
        );
    }

    #[test]
    fn handle_option_consumes_argument() {
        let forwarding = ForwardingJavaFileManager::new(Inner);
        let mut args = vec![js("out")].into_iter();
        assert!(forwarding.handle_option(&js("-d"), &mut args));
        assert!(args.next().is_none());
    }

    #[test]
    fn forwarding_manager_is_object_safe() {
        // ForwardingJavaFileManager itself can be used through dyn.
        let forwarding: Rc<dyn JavaFileManager> = Rc::new(ForwardingJavaFileManager::new(Inner));
        assert_eq!(forwarding.is_supported_option(&js("-d")), 1);
    }

    #[test]
    fn sized_generic_forwarding() {
        let forwarding = ForwardingJavaFileManager::new(Inner);
        let location = InnerLocation;
        assert_eq!(
            forwarding.get_service_loader(&location, Class::<()>::new()).err(),
            Some(Error::UnsupportedOperationException)
        );
        let _ = ServiceLoader::<()>::new();
    }
}
