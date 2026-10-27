// PORTING ADAPTER (not a 1:1 port)
// Adapter for: java.lang.Class, java.lang.ClassLoader, java.util.ServiceLoader,
//              javax.annotation.processing.Processor.
// Why not 1:1: these types are JVM / reflection / annotation-processing
//              facilities.  rust-javac runs without a JVM and does not port
//              annotation processing (docs/PLAN.md §3.1); the opaque
//              placeholders keep the javax.tools signatures complete while no
//              Rust implementation can exist.
//! Minimal placeholders for the `java.lang` / `java.util` /
//! `javax.annotation.processing` types that appear in `javax.tools`
//! signatures but have no Rust counterpart.
//!
//! PORTING_RULES — JDK type mappings used by `javax.tools`:
//!
//! * `java.lang.Class<S>`       -> opaque `Class<S>` (only carried through the
//!   API; the compiler never inspects it)
//! * `java.lang.ClassLoader`    -> opaque trait `ClassLoader`
//! * `java.util.ServiceLoader<S>` -> opaque `ServiceLoader<S>`
//! * `javax.annotation.processing.Processor` -> opaque trait `Processor`
//!   (annotation processing is out of scope, docs/PLAN.md §3.1)
//! * `java.lang.annotation.Annotation` -> opaque trait `Annotation`
//! * `java.nio.charset.Charset` -> the charset name
//!   (`javac_strings::JavaString`); the name-to-decoder mapping is done by the
//!   `file` port
//! * `java.util.Locale`         -> the locale tag (`javac_strings::JavaString`)
//!
//! TODO(P6): these placeholders are finalized by the API phase; annotation
//! processing is not ported, so class loading is never observed by the
//! compiler itself.

use std::marker::PhantomData;

/// Mapping of `java.lang.Class<S>`.  `Class<()>` represents `Class<?>`.
pub struct Class<S = ()> {
    marker: PhantomData<S>,
}

impl<S> Class<S> {
    pub fn new() -> Class<S> {
        Class { marker: PhantomData }
    }
}

impl<S> Default for Class<S> {
    fn default() -> Class<S> {
        Class::new()
    }
}

/// Mapping of `java.lang.ClassLoader`: an opaque class loader handle.
///
/// The Rust compiler does not run annotation processors (docs/PLAN.md §3.1), so
/// no implementation is provided; the trait only keeps the API signature.
pub trait ClassLoader {
    /// Finds the class with the given binary name (Java:
    /// URLClassLoader.findClass). The byte content of the class file is
    /// returned instead of a java.lang.Class (the port does not run a JVM;
    /// registered adapter, see the javax.tools class loader contract).
    fn find_class(&self, name: &javac_strings::JavaString) -> Option<Vec<u8>>;

    /// Finds the resource with the given name (Java:
    /// URLClassLoader.findResource).
    fn find_resource(&self, name: &javac_strings::JavaString) -> Option<Vec<u8>>;
}

/// Mapping of `java.lang.annotation.Annotation`.
///
/// PORTING_RULES: annotation instances are not modeled by the compiler port
/// (annotation processing is out of scope, docs/PLAN.md §3.1); the trait is the
/// type bound used by `AnnotatedConstruct.getAnnotation/getAnnotationsByType`.
pub trait Annotation {}

/// Mapping of `java.util.ServiceLoader<S>`: an opaque service loader handle.
pub struct ServiceLoader<S> {
    marker: PhantomData<S>,
}

impl<S> ServiceLoader<S> {
    pub fn new() -> ServiceLoader<S> {
        ServiceLoader { marker: PhantomData }
    }
}

impl<S> Default for ServiceLoader<S> {
    fn default() -> ServiceLoader<S> {
        ServiceLoader::new()
    }
}

/// Mapping of `javax.annotation.processing.Processor`.
///
/// Annotation processing is out of scope (docs/PLAN.md §3.1); the trait keeps
/// the type name used by `JavaCompiler.CompilationTask.setProcessors`.
pub trait Processor {}

#[cfg(test)]
mod tests {
    use super::{Class, ClassLoader, Processor, ServiceLoader};
    use std::rc::Rc;

    struct Loader;
    impl ClassLoader for Loader {
        fn find_class(&self, _name: &javac_strings::JavaString) -> Option<Vec<u8>> {
            None
        }

        fn find_resource(&self, _name: &javac_strings::JavaString) -> Option<Vec<u8>> {
            None
        }
    }

    struct P;
    impl Processor for P {}

    #[test]
    fn placeholders_are_constructible() {
        let _c: Class<()> = Class::new();
        let _s: ServiceLoader<P> = ServiceLoader::new();
        let loader: Rc<dyn ClassLoader> = Rc::new(Loader);
        let _: &dyn ClassLoader = &*loader;
        let p: Rc<dyn Processor> = Rc::new(P);
        let _: &dyn Processor = &*p;
    }
}
