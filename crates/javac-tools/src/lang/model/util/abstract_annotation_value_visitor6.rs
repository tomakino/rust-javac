// Origin: javax/lang/model/util/AbstractAnnotationValueVisitor6.java @ 982d53c (jdk-17.0.19-ga)

//! A skeletal visitor for annotation values with default behavior appropriate
//! for the RELEASE_6 source version.
//!
//! PORTING_RULES — class mapping:
//! * Java abstract class -> Rust trait with the upstream default behavior.
//! * the Java `final R visit(AnnotationValue, P)` forwards to `av.accept(this, p)`;
//!   Rust cannot call the generic `AnnotationValue::accept` on
//!   `&dyn AnnotationValue`, so the dispatch is reproduced from
//!   `AnnotationValue::get_value()` and the closed `Value` representation (the
//!   same information the Java dispatch uses).  Registered adapter contract.
//! * the single-argument convenience overload `visit(AnnotationValue)` forwards
//!   null upstream and is not ported.
//! * `visitUnknown` throws `UnknownAnnotationValueException` upstream; the Rust
//!   default panics with the same exception message.

use crate::lang::model::element::{
    AnnotationValue, AnnotationValueVisitor, Value,
};

/// A skeletal visitor for annotation values with default behavior
/// appropriate for the RELEASE_6 source version.
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
/// @see AbstractAnnotationValueVisitor7
/// @see AbstractAnnotationValueVisitor8
/// @see AbstractAnnotationValueVisitor9
/// @see AbstractAnnotationValueVisitor14
/// @since 1.6
pub trait AbstractAnnotationValueVisitor6<R, P>: AnnotationValueVisitor<R, P> {
    /// Visits any annotation value as if by passing itself to that
    /// value's accept.  The invocation
    /// v.visit(av, p) is equivalent to av.accept(v, p).
    /// @param av {@inheritDoc}
    /// @param p  {@inheritDoc}
    /// @return   {@inheritDoc}
    fn visit(&mut self, av: &dyn AnnotationValue, p: P) -> R {
        dispatch_annotation_value(self, av, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec The default implementation of this method in
    /// AbstractAnnotationValueVisitor6 will always throw
    /// new UnknownAnnotationValueException(av, p).  This behavior is not
    /// required of a subclass.
    ///
    /// @param av {@inheritDoc}
    /// @param p  {@inheritDoc}
    /// @return   {@inheritDoc}
    fn visit_unknown(&mut self, av: &dyn AnnotationValue, p: P) -> R {
        let _ = p;
        // Java: throw new UnknownAnnotationValueException(av, p);
        panic!("Unknown annotation value: \"{}\"", av)
    }
}

/// Java: the value's own `accept(visitor, p)` performs the dispatch.
///
/// PORTING_RULES: `AnnotationValue::accept` is generic and therefore not callable
/// on `&dyn AnnotationValue`; the dispatch is reproduced from the closed
/// `Value` representation of `get_value()`, which is exactly what the Java
/// accept implementations dispatch on (registered adapter contract).
pub(crate) fn dispatch_annotation_value<R, P, V: AnnotationValueVisitor<R, P> + ?Sized>(
    v: &mut V,
    av: &dyn AnnotationValue,
    p: P,
) -> R {
    match av.get_value() {
        Value::Boolean(b) => v.visit_boolean(b, p),
        Value::Byte(b) => v.visit_byte(b, p),
        Value::Character(c) => v.visit_char(c, p),
        Value::Double(d) => v.visit_double(d, p),
        Value::Float(f) => v.visit_float(f, p),
        Value::Integer(i) => v.visit_int(i, p),
        Value::Long(l) => v.visit_long(l, p),
        Value::Short(s) => v.visit_short(s, p),
        Value::String(s) => v.visit_string(&s, p),
        Value::Type(t) => v.visit_type(t.as_ref(), p),
        Value::EnumConstant(c) => v.visit_enum_constant(c.as_ref(), p),
        Value::Annotation(a) => v.visit_annotation(a.as_ref(), p),
        Value::Array(values) => v.visit_array(&values, p),
    }
}

#[cfg(test)]
mod tests {
    use super::AbstractAnnotationValueVisitor6;
    use crate::lang::model::element::{AnnotationValue, AnnotationValueVisitor, Value};
    use crate::lang::model::util::test_support::ValueFixture;

    struct TestVisitor {
        visited: Vec<i32>,
    }

    impl TestVisitor {
        fn new() -> TestVisitor {
            TestVisitor { visited: Vec::new() }
        }
    }

    impl AbstractAnnotationValueVisitor6<(), ()> for TestVisitor {}

    impl AnnotationValueVisitor<(), ()> for TestVisitor {
        fn visit_int(&mut self, i: i32, _p: ()) {
            self.visited.push(i);
        }
        fn visit_string(&mut self, s: &javac_strings::JavaString, _p: ()) {
            self.visited.push(-2 - s.len() as i32);
        }
        fn visit_unknown(&mut self, _av: &dyn AnnotationValue, _p: ()) {
            self.visited.push(-1);
        }
    }

    #[test]
    fn dispatch_uses_the_value_shape() {
        let mut v = TestVisitor::new();
        let int = ValueFixture { value: Value::Integer(42) };
        AbstractAnnotationValueVisitor6::visit(&mut v, &int, ());
        let string = ValueFixture { value: Value::String(javac_strings::JavaString::from("s")) };
        AbstractAnnotationValueVisitor6::visit(&mut v, &string, ());
        assert_eq!(v.visited, vec![42, -3]);
    }

    #[test]
    fn visitor_is_object_safe() {
        let v: &mut dyn AbstractAnnotationValueVisitor6<(), ()> = &mut TestVisitor::new();
        let int = ValueFixture { value: Value::Integer(1) };
        AbstractAnnotationValueVisitor6::visit(v, &int, ());
    }
}
