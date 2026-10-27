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
//! * `java.lang.Class<S>`       -> `Class<S>` handle; opaque for the
//!   javax.tools signatures, and carrying the annotation type's binary name
//!   for the ported annotation-proxy API (AnnoConstruct.getName matching).
//! * `java.lang.ClassLoader`    -> opaque trait `ClassLoader`
//! * `java.util.ServiceLoader<S>` -> opaque `ServiceLoader<S>`
//! * `javax.annotation.processing.Processor` -> opaque trait `Processor`
//!   (annotation processing is out of scope, docs/PLAN.md §3.1)
//! * `java.lang.annotation.Annotation` -> `AnnotationProxy` (the explicit
//!   dynamic-proxy return form: annotation type binary name + ordered element
//!   values; see the type below).  The empty `Annotation` marker trait is kept
//!   for the untouched javax.lang.model signatures.
//! * `java.nio.charset.Charset` -> the charset name
//!   (`javac_strings::JavaString`); the name-to-decoder mapping is done by the
//!   `file` port
//! * `java.util.Locale`         -> the locale tag (`javac_strings::JavaString`)
//!
//! TODO(P6): these placeholders are finalized by the API phase; annotation
//! processing is not ported, so class loading is never observed by the
//! compiler itself.

use std::marker::PhantomData;
use std::rc::Rc;

use javac_strings::JavaString;

/// Mapping of `java.lang.Class<S>`.  `Class<()>` represents `Class<?>`.
///
/// PORTING ADAPTER: the JVM Class object carries the runtime type identity that
/// javac reads through reflection (getName/isAnnotation/getMethod).  The port
/// keeps the annotation type's binary (flat) name in the handle so the ported
/// annotation-proxy API can perform the same name-based matching; handles
/// created without a name stay opaque for the javax.tools signatures.
#[derive(Clone)]
pub struct Class<S = ()> {
    marker: PhantomData<S>,
    flat_name: Option<JavaString>,
}

impl<S> Class<S> {
    /// Create an opaque class handle (no runtime type identity).
    pub fn new() -> Class<S> {
        Class {
            marker: PhantomData,
            flat_name: None,
        }
    }

    /// Create a class handle that carries the annotation type's binary name.
    pub fn with_flat_name(flat_name: JavaString) -> Class<S> {
        Class {
            marker: PhantomData,
            flat_name: Some(flat_name),
        }
    }

    /// Return the binary (flat) name carried by this handle, if any.
    pub fn get_flat_name(&self) -> Option<&JavaString> {
        self.flat_name.as_ref()
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

/// Java: the dynamic-proxy return form of a `java.lang.annotation.Annotation`
/// instance (registered adapter for JVM dynamic proxies).
///
/// The upstream proxy is generated by AnnotationProxyMaker and answers the
/// annotation interface methods through AnnotationInvocationHandler.  The port
/// represents the same result explicitly: the annotation type's binary name
/// plus the ordered element values in dynamic-proxy return form.
pub trait AnnotationProxy: std::fmt::Display {
    /// Java: the annotation type of this instance (`Annotation.annotationType()`).
    fn annotation_type_name(&self) -> &JavaString;

    /// The element names in the upstream LinkedHashMap order.
    fn member_names(&self) -> Vec<JavaString>;

    /// The dynamic-proxy return form of one element value; None when absent.
    fn member_value(&self, name: &JavaString) -> Option<AnnotationValueProxy>;
}

/// Java: the "dynamic proxy return form" of one annotation element value
/// (the values accepted by sun.reflect.annotation.AnnotationInvocationHandler).
#[derive(Clone)]
pub enum AnnotationValueProxy {
    /// Java: a Boolean wrapper value.
    Boolean(bool),
    /// Java: a Byte wrapper value.
    Byte(i8),
    /// Java: a Character wrapper value (UTF-16 code unit).
    Character(u16),
    /// Java: a Short wrapper value.
    Short(i16),
    /// Java: an Integer wrapper value.
    Integer(i32),
    /// Java: a Long wrapper value.
    Long(i64),
    /// Java: a Float wrapper value.
    Float(f32),
    /// Java: a Double wrapper value.
    Double(f64),
    /// Java: a String value.
    String(JavaString),
    /// Java: a Class value (upstream: MirroredTypeException proxy, rendered as
    /// the type name).
    Class(JavaString),
    /// Java: an enum constant value.
    Enum {
        /// The enum type's binary name.
        enum_type: JavaString,
        /// The constant name.
        constant: JavaString,
    },
    /// Java: a nested annotation value.
    Annotation(Rc<dyn AnnotationProxy>),
    /// Java: an array value.
    Array(Vec<AnnotationValueProxy>),
    /// Java: an ExceptionProxy value (thrown on access by the upstream
    /// AnnotationInvocationHandler).
    Exception(AnnotationProxyException),
}

/// Java: the ExceptionProxy alternatives of AnnotationProxyMaker.
#[derive(Clone)]
pub enum AnnotationProxyException {
    /// Java: MirroredTypeExceptionProxy (a Class element value).
    MirroredType {
        /// The referenced type's name.
        type_name: JavaString,
        /// The annotation type containing the element.
        annotation: JavaString,
        /// The element name.
        element: JavaString,
    },
    /// Java: MirroredTypesExceptionProxy (a Class[] element value).
    MirroredTypes {
        /// The referenced types' names.
        type_names: Vec<JavaString>,
        /// The annotation type containing the element.
        annotation: JavaString,
        /// The element name.
        element: JavaString,
    },
    /// Java: EnumConstantNotPresentExceptionProxy.
    EnumConstantNotPresent {
        /// The enum type's binary name.
        enum_type: JavaString,
        /// The missing constant name.
        constant: JavaString,
    },
    /// Java: AnnotationTypeMismatchExceptionProxy.
    AnnotationTypeMismatch {
        /// The annotation type containing the element.
        annotation: JavaString,
        /// The element name.
        element: JavaString,
        /// The attribute type found in the class file (upstream attr.type).
        found_type: JavaString,
    },
}

impl std::fmt::Display for AnnotationValueProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnnotationValueProxy::Boolean(value) => write!(f, "{value}"),
            AnnotationValueProxy::Byte(value) => write!(f, "{value}"),
            AnnotationValueProxy::Character(value) => {
                write!(f, "'{}'", char::from_u32(*value as u32).unwrap_or('?'))
            }
            AnnotationValueProxy::Short(value) => write!(f, "{value}"),
            AnnotationValueProxy::Integer(value) => write!(f, "{value}"),
            AnnotationValueProxy::Long(value) => write!(f, "{value}"),
            AnnotationValueProxy::Float(value) => write!(f, "{value}"),
            AnnotationValueProxy::Double(value) => write!(f, "{value}"),
            AnnotationValueProxy::String(value) => write!(f, "\"{value}\""),
            AnnotationValueProxy::Class(type_name) => write!(f, "{type_name}.class"),
            AnnotationValueProxy::Enum {
                enum_type,
                constant,
            } => write!(f, "{enum_type}.{constant}"),
            AnnotationValueProxy::Annotation(annotation) => write!(f, "{annotation}"),
            AnnotationValueProxy::Array(values) => {
                write!(f, "[")?;
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{value}")?;
                }
                write!(f, "]")
            }
            AnnotationValueProxy::Exception(exception) => match exception {
                AnnotationProxyException::MirroredType { type_name, .. } => {
                    write!(f, "{type_name}.class")
                }
                AnnotationProxyException::MirroredTypes { type_names, .. } => {
                    write!(f, "[")?;
                    for (index, type_name) in type_names.iter().enumerate() {
                        if index != 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{type_name}.class")?;
                    }
                    write!(f, "]")
                }
                AnnotationProxyException::EnumConstantNotPresent { .. }
                | AnnotationProxyException::AnnotationTypeMismatch { .. } => write!(f, "<error>"),
            },
        }
    }
}

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

    struct Proxy {
        name: javac_strings::JavaString,
    }

    impl super::AnnotationProxy for Proxy {
        fn annotation_type_name(&self) -> &javac_strings::JavaString {
            &self.name
        }

        fn member_names(&self) -> Vec<javac_strings::JavaString> {
            vec![javac_strings::JavaString::from("value")]
        }

        fn member_value(
            &self,
            name: &javac_strings::JavaString,
        ) -> Option<super::AnnotationValueProxy> {
            if name == &javac_strings::JavaString::from("value") {
                Some(super::AnnotationValueProxy::Integer(1))
            } else {
                None
            }
        }
    }

    impl std::fmt::Display for Proxy {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "@p.A(value=1)")
        }
    }

    #[test]
    fn placeholders_are_constructible() {
        let _c: Class<()> = Class::new();
        let named = Class::<()>::with_flat_name(javac_strings::JavaString::from("p.A"));
        assert_eq!(
            named.get_flat_name().cloned(),
            Some(javac_strings::JavaString::from("p.A"))
        );
        let _s: ServiceLoader<P> = ServiceLoader::new();
        let loader: Rc<dyn ClassLoader> = Rc::new(Loader);
        let _: &dyn ClassLoader = &*loader;
        let p: Rc<dyn Processor> = Rc::new(P);
        let _: &dyn Processor = &*p;
        let proxy: Rc<dyn super::AnnotationProxy> = Rc::new(Proxy {
            name: javac_strings::JavaString::from("p.A"),
        });
        assert!(matches!(
            proxy.member_value(&javac_strings::JavaString::from("value")),
            Some(super::AnnotationValueProxy::Integer(1))
        ));
        assert_eq!(format!("{proxy}"), "@p.A(value=1)");
    }
}
