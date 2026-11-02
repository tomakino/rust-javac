//! A visitor of the values of annotation interface elements, using a variant of
//! the visitor design pattern.
//!
//! — signature mapping:
//! * visit methods take `&mut self`; the annotation value is `&dyn
//!   AnnotationValue`; Java `char` -> `u16`; Java `String` ->
//!   `&javac_strings::JavaString`; Java `int`/`long`/`short`/`byte`/
//!   `float`/`double`/`boolean` keep the Java primitive types.
//! * `List<? extends AnnotationValue>` -> `&[Rc<dyn AnnotationValue>]`.
//! * the single-argument convenience overload `visit(AnnotationValue)` only
//!   forwards `null` upstream and is not ported (registered difference).

use super::annotation_mirror::AnnotationMirror;
use super::annotation_value::AnnotationValue;
use super::variable_element::VariableElement;
use crate::lang::model::type_::type_mirror::TypeMirror;
use javac_strings::JavaString;
use std::rc::Rc;

/// A visitor of the values of annotation interface elements, using a
/// variant of the visitor design pattern.  Unlike a standard visitor
/// which dispatches based on the concrete type of a member of a type
/// hierarchy, this visitor dispatches based on the type of data
/// stored; there are no distinct subclasses for storing, for example,
/// boolean values versus int values.  Classes
/// implementing this interface are used to operate on a value when the
/// type of that value is unknown at compile time.  When a visitor is
/// passed to a value's {@link AnnotationValue#accept accept} method,
/// the visitXyz method applicable to that value is invoked.
///
/// <p> Classes implementing this interface may or may not throw a
/// NullPointerException if the additional parameter p
/// is null; see documentation of the implementing class for
/// details.
///
/// @param <R> the return type of this visitor's methods
/// @param <P> the type of the additional parameter to this visitor's methods.
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
/// @since 1.6
pub trait AnnotationValueVisitor<R, P> {
    /// Visits an annotation value.
    /// @param av the value to visit
    /// @param p a visitor-specified parameter
    /// @return  a visitor-specified result
    fn visit(&mut self, _av: &dyn AnnotationValue, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit: Rust: visitor interface method not overridden")
    }

    /// Visits a boolean value in an annotation.
    /// @param b the value being visited
    /// @param p a visitor-specified parameter
    /// @return the result of the visit
    fn visit_boolean(&mut self, _b: bool, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_boolean: Rust: visitor interface method not overridden")
    }

    /// Visits a byte value in an annotation.
    /// @param  b the value being visited
    /// @param  p a visitor-specified parameter
    /// @return the result of the visit
    fn visit_byte(&mut self, _b: i8, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_byte: Rust: visitor interface method not overridden")
    }

    /// Visits a char value in an annotation.
    /// @param  c the value being visited
    /// @param  p a visitor-specified parameter
    /// @return the result of the visit
    fn visit_char(&mut self, _c: u16, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_char: Rust: visitor interface method not overridden")
    }

    /// Visits a double value in an annotation.
    /// @param  d the value being visited
    /// @param  p a visitor-specified parameter
    /// @return the result of the visit
    fn visit_double(&mut self, _d: f64, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_double: Rust: visitor interface method not overridden")
    }

    /// Visits a float value in an annotation.
    /// @param  f the value being visited
    /// @param  p a visitor-specified parameter
    /// @return the result of the visit
    fn visit_float(&mut self, _f: f32, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_float: Rust: visitor interface method not overridden")
    }

    /// Visits an int value in an annotation.
    /// @param  i the value being visited
    /// @param  p a visitor-specified parameter
    /// @return the result of the visit
    fn visit_int(&mut self, _i: i32, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_int: Rust: visitor interface method not overridden")
    }

    /// Visits a long value in an annotation.
    /// @param  i the value being visited
    /// @param  p a visitor-specified parameter
    /// @return the result of the visit
    fn visit_long(&mut self, _i: i64, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_long: Rust: visitor interface method not overridden")
    }

    /// Visits a short value in an annotation.
    /// @param  s the value being visited
    /// @param  p a visitor-specified parameter
    /// @return the result of the visit
    fn visit_short(&mut self, _s: i16, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_short: Rust: visitor interface method not overridden")
    }

    /// Visits a string value in an annotation.
    /// @param  s the value being visited
    /// @param  p a visitor-specified parameter
    /// @return the result of the visit
    fn visit_string(&mut self, _s: &JavaString, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_string: Rust: visitor interface method not overridden")
    }

    /// Visits a type value in an annotation.
    /// @param  t the value being visited
    /// @param  p a visitor-specified parameter
    /// @return the result of the visit
    fn visit_type(&mut self, _t: &dyn TypeMirror, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_type: Rust: visitor interface method not overridden")
    }

    /// Visits an enum value in an annotation.
    /// @param  c the value being visited
    /// @param  p a visitor-specified parameter
    /// @return the result of the visit
    fn visit_enum_constant(&mut self, _c: &dyn VariableElement, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_enum_constant: Rust: visitor interface method not overridden")
    }

    /// Visits an annotation value in an annotation.
    /// @param  a the value being visited
    /// @param  p a visitor-specified parameter
    /// @return the result of the visit
    fn visit_annotation(&mut self, _a: &dyn AnnotationMirror, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_annotation: Rust: visitor interface method not overridden")
    }

    /// Visits an array value in an annotation.
    /// @param  vals the value being visited
    /// @param  p a visitor-specified parameter
    /// @return the result of the visit
    fn visit_array(&mut self, _vals: &[Rc<dyn AnnotationValue>], _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_array: Rust: visitor interface method not overridden")
    }

    /// Visits an unknown kind of annotation value.
    /// This can occur if the language evolves and new kinds
    /// of value can be stored in an annotation.
    /// @param  av the unknown value being visited
    /// @param  p a visitor-specified parameter
    /// @return the result of the visit
    /// @throws UnknownAnnotationValueException
    ///  a visitor implementation may optionally throw this exception
    fn visit_unknown(&mut self, _av: &dyn AnnotationValue, _p: P) -> R {
        // Rust: the Java interface method is abstract; the Rust default
        // panics so that the visitor-class layers can supply the real default
        // implementations (registered language-shape difference).
        panic!("AbstractMethodError: visit_unknown: Rust: visitor interface method not overridden")
    }
}
