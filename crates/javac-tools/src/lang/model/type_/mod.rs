//! Interfaces used to model Java programming language types.
//!
//! <p> Unless otherwise specified in a particular implementation, the
//! collections returned by methods in this package should be expected
//! to be unmodifiable by the caller and unsafe for concurrent access.
//!
//! <p> Unless otherwise specified, methods in this package will throw
//! a {@code NullPointerException} if given a {@code null} argument.
//!
//! @author Joseph D. Darcy
//! @author Scott Seligman
//! @author Peter von der Ah&eacute;
//! @see javax.lang.model.util.Types
//! @jls 4.1 The Kinds of Types and Values
//! @jls 4.2 Primitive Types and Values
//! @jls 4.3 Reference Types and Values
//! @jls 4.4 Type Variables
//! @jls 4.5 Parameterized Types
//! @jls 4.8 Raw Types
//! @jls 4.9 Intersection Types
//! @jls 10.1 Array Types
//! @since 1.6
//!
//! PORTING_RULES: the Java package name `type` is a Rust keyword, so the
//! module is named `type_` (docs/PORTING_RULES.md §3); the file names mirror
//! the Java class names in snake_case.

pub mod array_type;
pub mod declared_type;
pub mod error_type;
pub mod executable_type;
pub mod intersection_type;
pub mod mirrored_type_exception;
pub mod mirrored_types_exception;
pub mod no_type;
pub mod null_type;
pub mod primitive_type;
pub mod reference_type;
pub mod type_kind;
pub mod type_mirror;
pub mod type_variable;
pub mod type_visitor;
pub mod union_type;
pub mod unknown_type_exception;
pub mod wildcard_type;

// PORTING_RULES: the source files are one-per-Java-file modules; these
// re-exports expose the Java package-level simple names
// (javax.lang.model.type.X) at the module root.
pub use array_type::ArrayType;
pub use declared_type::DeclaredType;
pub use error_type::ErrorType;
pub use executable_type::ExecutableType;
pub use intersection_type::IntersectionType;
pub use mirrored_type_exception::MirroredTypeException;
pub use mirrored_types_exception::MirroredTypesException;
pub use no_type::NoType;
pub use null_type::NullType;
pub use primitive_type::PrimitiveType;
pub use reference_type::ReferenceType;
pub use type_kind::TypeKind;
pub use type_mirror::TypeMirror;
pub use type_variable::TypeVariable;
pub use type_visitor::TypeVisitor;
pub use union_type::UnionType;
pub use unknown_type_exception::UnknownTypeException;
pub use wildcard_type::WildcardType;

