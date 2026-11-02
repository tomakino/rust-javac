//! A visitor of types, in the style of the visitor design pattern.
//!
//! — signature mapping:
//! * visit methods take `&mut self`, the visited type as `&dyn X`, and the
//!   parameter `P` by value.
//! * the single-argument convenience overload `visit(TypeMirror)` only
//!   forwards `null` upstream and is not ported (registered difference).

use super::array_type::ArrayType;
use super::declared_type::DeclaredType;
use super::error_type::ErrorType;
use super::executable_type::ExecutableType;
use super::intersection_type::IntersectionType;
use super::no_type::NoType;
use super::null_type::NullType;
use super::primitive_type::PrimitiveType;
use super::type_mirror::TypeMirror;
use super::type_variable::TypeVariable;
use super::union_type::UnionType;
use super::wildcard_type::WildcardType;

/// A visitor of types, in the style of the
/// visitor design pattern.  Classes implementing this
/// interface are used to operate on a type when the kind of
/// type is unknown at compile time.  When a visitor is passed to a
/// type's {@link TypeMirror#accept accept} method, the visitXyz
/// method most applicable to that type is invoked.
///
/// <p> Classes implementing this interface may or may not throw a
/// NullPointerException if the additional parameter p
/// is null; see documentation of the implementing class for
/// details.
///
/// @param <R> the return type of this visitor's methods.
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @since 1.6
pub trait TypeVisitor<R, P> {
    /// Visits a type.
    /// @param t the type to visit
    /// @param p a visitor-specified parameter
    /// @return  a visitor-specified result
    fn visit(&mut self, _t: &dyn TypeMirror, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a primitive type.
    /// @param t the type to visit
    /// @param p a visitor-specified parameter
    /// @return  a visitor-specified result
    fn visit_primitive(&mut self, _t: &dyn PrimitiveType, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_primitive: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits the null type.
    /// @param t the type to visit
    /// @param p a visitor-specified parameter
    /// @return  a visitor-specified result
    fn visit_null(&mut self, _t: &dyn NullType, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_null: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an array type.
    /// @param t the type to visit
    /// @param p a visitor-specified parameter
    /// @return  a visitor-specified result
    fn visit_array(&mut self, _t: &dyn ArrayType, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_array: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a declared type.
    /// @param t the type to visit
    /// @param p a visitor-specified parameter
    /// @return  a visitor-specified result
    fn visit_declared(&mut self, _t: &dyn DeclaredType, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_declared: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an error type.
    /// @param t the type to visit
    /// @param p a visitor-specified parameter
    /// @return  a visitor-specified result
    fn visit_error(&mut self, _t: &dyn ErrorType, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_error: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a type variable.
    /// @param t the type to visit
    /// @param p a visitor-specified parameter
    /// @return  a visitor-specified result
    fn visit_type_variable(&mut self, _t: &dyn TypeVariable, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_type_variable: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a wildcard type.
    /// @param t the type to visit
    /// @param p a visitor-specified parameter
    /// @return  a visitor-specified result
    fn visit_wildcard(&mut self, _t: &dyn WildcardType, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_wildcard: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an executable type.
    /// @param t the type to visit
    /// @param p a visitor-specified parameter
    /// @return  a visitor-specified result
    fn visit_executable(&mut self, _t: &dyn ExecutableType, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_executable: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a {@link NoType} instance.
    /// @param t the type to visit
    /// @param p a visitor-specified parameter
    /// @return  a visitor-specified result
    fn visit_no_type(&mut self, _t: &dyn NoType, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_no_type: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an unknown kind of type.
    /// This can occur if the language evolves and new kinds
    /// of types are added to the TypeMirror hierarchy.
    /// @param t the type to visit
    /// @param p a visitor-specified parameter
    /// @return  a visitor-specified result
    /// @throws UnknownTypeException
    ///  a visitor implementation may optionally throw this exception
    fn visit_unknown(&mut self, _t: &dyn TypeMirror, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_unknown: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a union type.
    ///
    /// @param t the type to visit
    /// @param p a visitor-specified parameter
    /// @return  a visitor-specified result
    /// @since 1.7
    fn visit_union(&mut self, _t: &dyn UnionType, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_union: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an intersection type.
    ///
    /// @param t the type to visit
    /// @param p a visitor-specified parameter
    /// @return  a visitor-specified result
    /// @since 1.8
    fn visit_intersection(&mut self, _t: &dyn IntersectionType, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_intersection: PORTING_RULES: visitor interface method not overridden")
    }
}
