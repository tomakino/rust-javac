// Origin: javax/lang/model/util/SimpleAnnotationValueVisitor6.java @ 982d53c (jdk-17.0.19-ga)

//! A simple visitor for annotation values with default behavior appropriate for
//! the RELEASE_6 source version.
//!
//! PORTING_RULES — class mapping:
//! * Java concrete class -> Rust trait with the upstream default behavior.
//! * the Java `protected R defaultAction(Object o, P p)` parameter is an
//!   `Object` holding the unboxed value; it is represented by the borrowing
//!   `ValueRef` enum (registered representation choice), which carries exactly
//!   the documented value shapes.
//! * the `DEFAULT_VALUE` field (null default from the no-arg constructor) ->
//!   `default_value() -> Option<&R>`; `default_action` panics if it would have
//!   returned null (Java returns null).
//! * visit methods take `&mut self` and call `default_action`.

use super::abstract_annotation_value_visitor6::AbstractAnnotationValueVisitor6;
use crate::lang::model::element::{
    AnnotationMirror, AnnotationValue, ValueRef, VariableElement,
};
use crate::lang::model::type_::TypeMirror;
use javac_strings::JavaString;
use std::rc::Rc;

/// A simple visitor for annotation values with default behavior
/// appropriate for the RELEASE_6 source version.  Visit methods call
/// {@link #defaultAction} passing their arguments to defaultAction's
/// corresponding parameters.
///
/// <p id=note_for_subclasses><strong>WARNING:</strong> The AnnotationValueVisitor
/// interface implemented by this class may have methods added to it in the
/// future to accommodate new, currently unknown, language structures added to
/// future versions of the Java programming language.
///
/// @param <R> the return type of this visitor's methods
/// @param <P> the type of the additional parameter to this visitor's methods.
///
/// @author Joseph D. Darcy
/// @author Scott Seligman
/// @author Peter von der Ah&eacute;
///
/// @see SimpleAnnotationValueVisitor7
/// @see SimpleAnnotationValueVisitor8
/// @see SimpleAnnotationValueVisitor9
/// @see SimpleAnnotationValueVisitor14
/// @since 1.6
pub trait SimpleAnnotationValueVisitor6<R: Clone, P>: AbstractAnnotationValueVisitor6<R, P> {
    /// Default value to be returned; {@link #defaultAction
    /// defaultAction} returns this value unless the method is
    /// overridden.
    ///
    /// Java: protected final R DEFAULT_VALUE; (null for the no-arg
    /// constructor, which has no Rust form; the accessor returns None for it).
    fn default_value(&self) -> Option<&R>;

    /// The default action for visit methods.
    ///
    /// @implSpec The implementation in this class just returns {@link
    /// #DEFAULT_VALUE}; subclasses will commonly override this method.
    ///
    /// @param o the value of the annotation
    /// @param p a visitor-specified parameter
    /// @return DEFAULT_VALUE unless overridden
    fn default_action(&mut self, _o: ValueRef<'_>, _p: P) -> R {
        match self.default_value() {
            Some(value) => value.clone(),
            None => panic!(
                "SimpleAnnotationValueVisitor6.defaultAction: DEFAULT_VALUE is null (Java); \
                 the Rust implementation must return Some(default value) from default_value()"
            ),
        }
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_boolean(&mut self, b: bool, p: P) -> R {
        self.default_action(ValueRef::Boolean(b), p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_byte(&mut self, b: i8, p: P) -> R {
        self.default_action(ValueRef::Byte(b), p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_char(&mut self, c: u16, p: P) -> R {
        self.default_action(ValueRef::Character(c), p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_double(&mut self, d: f64, p: P) -> R {
        self.default_action(ValueRef::Double(d), p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_float(&mut self, f: f32, p: P) -> R {
        self.default_action(ValueRef::Float(f), p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_int(&mut self, i: i32, p: P) -> R {
        self.default_action(ValueRef::Integer(i), p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_long(&mut self, i: i64, p: P) -> R {
        self.default_action(ValueRef::Long(i), p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_short(&mut self, s: i16, p: P) -> R {
        self.default_action(ValueRef::Short(s), p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_string(&mut self, s: &JavaString, p: P) -> R {
        self.default_action(ValueRef::String(s), p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_type(&mut self, t: &dyn TypeMirror, p: P) -> R {
        self.default_action(ValueRef::Type(t), p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_enum_constant(&mut self, c: &dyn VariableElement, p: P) -> R {
        self.default_action(ValueRef::EnumConstant(c), p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_annotation(&mut self, a: &dyn AnnotationMirror, p: P) -> R {
        self.default_action(ValueRef::Annotation(a), p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls defaultAction.
    fn visit_array(&mut self, vals: &[Rc<dyn AnnotationValue>], p: P) -> R {
        self.default_action(ValueRef::Array(vals), p)
    }
}

#[cfg(test)]
mod tests {
    use super::SimpleAnnotationValueVisitor6;
    use crate::lang::model::element::{AnnotationValueVisitor, Value, ValueRef};
    use crate::lang::model::util::test_support::ValueFixture;
    use javac_strings::JavaString;

    struct DefaultVisitor {
        value: usize,
        last: Option<()>,
    }

    impl AnnotationValueVisitor<usize, ()> for DefaultVisitor {}
    impl crate::lang::model::util::abstract_annotation_value_visitor6::AbstractAnnotationValueVisitor6<
        usize,
        (),
    > for DefaultVisitor
    {
    }
    impl SimpleAnnotationValueVisitor6<usize, ()> for DefaultVisitor {
        fn default_value(&self) -> Option<&usize> {
            Some(&self.value)
        }
        fn default_action(&mut self, _o: ValueRef<'_>, _p: ()) -> usize {
            self.last = Some(());
            self.value
        }
    }

    #[test]
    fn visit_methods_call_default_action() {
        let mut v = DefaultVisitor { value: 5, last: None };
        let int = ValueFixture { value: Value::Integer(1) };
        assert_eq!(SimpleAnnotationValueVisitor6::visit_int(&mut v, 1, ()), 5);
        let string = ValueFixture { value: Value::String(JavaString::from("s")) };
        assert_eq!(
            SimpleAnnotationValueVisitor6::visit_string(&mut v, &JavaString::from("s"), ()),
            5
        );
        assert!(v.last.is_some());
        let _ = int;
        let _ = string;
    }
}
