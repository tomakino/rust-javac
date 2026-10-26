// PORTING ADAPTER (not a 1:1 port): test fixtures for the javax.lang.model.util
// tests.
// Why not 1:1: javac tests use anonymous subclasses of the visitor classes; the
// Rust tests need concrete Element/TypeMirror/AnnotationValue implementations to
// exercise the visitor defaults.  This module is compiled only under `#[cfg(test)]`
// and is not part of the ported API.
#![allow(dead_code)]

use crate::lang::model::element::{
    AnnotationMirror, AnnotationValue, ConstantValue, Directive, Element, ElementKind,
    ElementVisitor, ExecutableElement, Modifier, ModuleElement, Name, PackageElement,
    Parameterizable, QualifiedNameable, RecordComponentElement, TypeElement,
    TypeParameterElement, Value, ValueRef, VariableElement,
};
use crate::lang::model::AnnotatedConstruct;
use crate::lang::model::type_::{
    ArrayType, DeclaredType, ErrorType, ExecutableType, IntersectionType, NoType, NullType,
    PrimitiveType, ReferenceType, TypeKind, TypeMirror, TypeVariable, UnionType, WildcardType,
};
use crate::jdk::{Annotation, Class};
use javac_strings::JavaString;
use std::fmt;
use std::rc::Rc;

pub struct FixtureName {
    pub text: JavaString,
}

impl Name for FixtureName {
    fn length(&self) -> usize {
        self.text.len()
    }
    fn char_at(&self, index: usize) -> u16 {
        self.text.char_at(index)
    }
    fn sub_sequence(&self, start: usize, end: usize) -> JavaString {
        self.text.substring(start, end)
    }
    fn to_java_string(&self) -> JavaString {
        self.text.clone()
    }
    fn content_equals(&self, cs: &JavaString) -> bool {
        self.text == *cs
    }
}

/// An element of a chosen kind with a name and enclosed elements.
pub struct Fixture {
    pub kind: ElementKind,
    pub name: JavaString,
    pub enclosed: Vec<Rc<dyn Element>>,
}

impl Fixture {
    pub fn new(kind: ElementKind, name: &str) -> Fixture {
        Fixture { kind, name: JavaString::from(name), enclosed: Vec::new() }
    }

    pub fn with_enclosed(kind: ElementKind, name: &str, enclosed: Vec<Rc<dyn Element>>) -> Fixture {
        Fixture { kind, name: JavaString::from(name), enclosed }
    }
}

impl fmt::Display for Fixture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name.to_string_lossy())
    }
}

impl AnnotatedConstruct for Fixture {
    fn get_annotation_mirrors(&self) -> Vec<Rc<dyn AnnotationMirror>> {
        Vec::new()
    }
    fn get_annotation<A: Annotation>(&self, _annotation_type: Class<A>) -> Option<A> {
        None
    }
    fn get_annotations_by_type<A: Annotation>(&self, _annotation_type: Class<A>) -> Vec<A> {
        Vec::new()
    }
}

impl Element for Fixture {
    fn as_type(&self) -> Rc<dyn TypeMirror> {
        panic!("UnsupportedOperationException: as_type: fixture has no type mirror")
    }
    fn get_kind(&self) -> ElementKind {
        self.kind
    }
    fn get_modifiers(&self) -> Vec<Modifier> {
        Vec::new()
    }
    fn get_simple_name(&self) -> Rc<dyn Name> {
        Rc::new(FixtureName { text: self.name.clone() })
    }
    fn get_enclosing_element(&self) -> Option<Rc<dyn Element>> {
        None
    }
    fn get_enclosed_elements(&self) -> Vec<Rc<dyn Element>> {
        self.enclosed.clone()
    }
    fn accept<R, P>(&self, _v: &mut dyn ElementVisitor<R, P>, _p: P) -> R {
        panic!("UnsupportedOperationException: accept: fixture is not visited through accept")
    }
    fn as_package_element(&self) -> Option<&dyn PackageElement> {
        (self.kind == ElementKind::PACKAGE).then_some(self as &dyn PackageElement)
    }
    fn as_module_element(&self) -> Option<&dyn ModuleElement> {
        (self.kind == ElementKind::MODULE).then_some(self as &dyn ModuleElement)
    }
    fn as_type_element(&self) -> Option<&dyn TypeElement> {
        (self.kind.is_class() || self.kind.is_interface()).then_some(self as &dyn TypeElement)
    }
    fn as_variable_element(&self) -> Option<&dyn VariableElement> {
        matches!(
            self.kind,
            ElementKind::FIELD
                | ElementKind::ENUM_CONSTANT
                | ElementKind::PARAMETER
                | ElementKind::LOCAL_VARIABLE
                | ElementKind::EXCEPTION_PARAMETER
                | ElementKind::RESOURCE_VARIABLE
                | ElementKind::BINDING_VARIABLE
        )
        .then_some(self as &dyn VariableElement)
    }
    fn as_executable_element(&self) -> Option<&dyn ExecutableElement> {
        matches!(
            self.kind,
            ElementKind::METHOD
                | ElementKind::CONSTRUCTOR
                | ElementKind::STATIC_INIT
                | ElementKind::INSTANCE_INIT
        )
        .then_some(self as &dyn ExecutableElement)
    }
    fn as_type_parameter_element(&self) -> Option<&dyn TypeParameterElement> {
        (self.kind == ElementKind::TYPE_PARAMETER).then_some(self as &dyn TypeParameterElement)
    }
    fn as_record_component_element(&self) -> Option<&dyn RecordComponentElement> {
        (self.kind == ElementKind::RECORD_COMPONENT).then_some(self as &dyn RecordComponentElement)
    }
}

impl Parameterizable for Fixture {
    fn get_type_parameters(&self) -> Vec<Rc<dyn TypeParameterElement>> {
        Vec::new()
    }
}

impl QualifiedNameable for Fixture {
    fn get_qualified_name(&self) -> Rc<dyn Name> {
        Rc::new(FixtureName { text: self.name.clone() })
    }
}

impl VariableElement for Fixture {
    fn get_constant_value(&self) -> Option<ConstantValue> {
        None
    }
}

impl ExecutableElement for Fixture {
    fn get_parameters(&self) -> Vec<Rc<dyn VariableElement>> {
        Vec::new()
    }
    fn get_return_type(&self) -> Rc<dyn TypeMirror> {
        panic!("UnsupportedOperationException: get_return_type: fixture has no type mirror")
    }
    fn get_receiver_type(&self) -> Rc<dyn TypeMirror> {
        panic!("UnsupportedOperationException: get_receiver_type: fixture has no type mirror")
    }
    fn is_var_args(&self) -> bool {
        false
    }
    fn is_default(&self) -> bool {
        false
    }
    fn get_thrown_types(&self) -> Vec<Rc<dyn TypeMirror>> {
        Vec::new()
    }
    fn get_default_value(&self) -> Option<Rc<dyn AnnotationValue>> {
        None
    }
}

impl TypeElement for Fixture {
    fn get_nesting_kind(&self) -> crate::lang::model::element::NestingKind {
        crate::lang::model::element::NestingKind::TOP_LEVEL
    }
    fn get_superclass(&self) -> Rc<dyn TypeMirror> {
        panic!("UnsupportedOperationException: get_superclass: fixture has no type mirror")
    }
    fn get_interfaces(&self) -> Vec<Rc<dyn TypeMirror>> {
        Vec::new()
    }
}

impl PackageElement for Fixture {
    fn is_unnamed(&self) -> bool {
        false
    }
}

impl ModuleElement for Fixture {
    fn is_open(&self) -> bool {
        false
    }
    fn is_unnamed(&self) -> bool {
        false
    }
    fn get_directives(&self) -> Vec<Rc<dyn Directive>> {
        Vec::new()
    }
}

impl TypeParameterElement for Fixture {
    fn get_generic_element(&self) -> Rc<dyn Element> {
        panic!("UnsupportedOperationException: get_generic_element: fixture")
    }
    fn get_bounds(&self) -> Vec<Rc<dyn TypeMirror>> {
        Vec::new()
    }
}

impl RecordComponentElement for Fixture {
    fn get_accessor(&self) -> Rc<dyn ExecutableElement> {
        panic!("UnsupportedOperationException: get_accessor: fixture")
    }
}

/// A type mirror of a chosen kind; implements every TypeMirror subinterface so
/// the object-conversion accessors can be exercised.
pub struct TypeFixture {
    pub kind: TypeKind,
}

impl TypeFixture {
    pub fn new(kind: TypeKind) -> TypeFixture {
        TypeFixture { kind }
    }
}

impl fmt::Display for TypeFixture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl AnnotatedConstruct for TypeFixture {
    fn get_annotation_mirrors(&self) -> Vec<Rc<dyn AnnotationMirror>> {
        Vec::new()
    }
    fn get_annotation<A: Annotation>(&self, _annotation_type: Class<A>) -> Option<A> {
        None
    }
    fn get_annotations_by_type<A: Annotation>(&self, _annotation_type: Class<A>) -> Vec<A> {
        Vec::new()
    }
}

impl TypeMirror for TypeFixture {
    fn get_kind(&self) -> TypeKind {
        self.kind
    }
    fn accept<R, P>(&self, _v: &mut dyn crate::lang::model::type_::TypeVisitor<R, P>, _p: P) -> R {
        panic!("UnsupportedOperationException: accept: fixture is not visited through accept")
    }
    fn as_primitive_type(&self) -> Option<&dyn PrimitiveType> {
        self.kind.is_primitive().then_some(self as &dyn PrimitiveType)
    }
    fn as_null_type(&self) -> Option<&dyn NullType> {
        (self.kind == TypeKind::NULL).then_some(self as &dyn NullType)
    }
    fn as_array_type(&self) -> Option<&dyn ArrayType> {
        (self.kind == TypeKind::ARRAY).then_some(self as &dyn ArrayType)
    }
    fn as_declared_type(&self) -> Option<&dyn DeclaredType> {
        (self.kind == TypeKind::DECLARED).then_some(self as &dyn DeclaredType)
    }
    fn as_error_type(&self) -> Option<&dyn ErrorType> {
        (self.kind == TypeKind::ERROR).then_some(self as &dyn ErrorType)
    }
    fn as_type_variable(&self) -> Option<&dyn TypeVariable> {
        (self.kind == TypeKind::TYPEVAR).then_some(self as &dyn TypeVariable)
    }
    fn as_wildcard_type(&self) -> Option<&dyn WildcardType> {
        (self.kind == TypeKind::WILDCARD).then_some(self as &dyn WildcardType)
    }
    fn as_executable_type(&self) -> Option<&dyn ExecutableType> {
        (self.kind == TypeKind::EXECUTABLE).then_some(self as &dyn ExecutableType)
    }
    fn as_no_type(&self) -> Option<&dyn NoType> {
        matches!(
            self.kind,
            TypeKind::VOID | TypeKind::NONE | TypeKind::PACKAGE | TypeKind::MODULE
        )
        .then_some(self as &dyn NoType)
    }
    fn as_union_type(&self) -> Option<&dyn UnionType> {
        (self.kind == TypeKind::UNION).then_some(self as &dyn UnionType)
    }
    fn as_intersection_type(&self) -> Option<&dyn IntersectionType> {
        (self.kind == TypeKind::INTERSECTION).then_some(self as &dyn IntersectionType)
    }
}

impl ReferenceType for TypeFixture {}
impl PrimitiveType for TypeFixture {}
impl NullType for TypeFixture {}
impl NoType for TypeFixture {}
impl ArrayType for TypeFixture {
    fn get_component_type(&self) -> Rc<dyn TypeMirror> {
        panic!("UnsupportedOperationException: get_component_type: fixture")
    }
}
impl DeclaredType for TypeFixture {
    fn as_element(&self) -> Rc<dyn Element> {
        panic!("UnsupportedOperationException: as_element: fixture")
    }
    fn get_enclosing_type(&self) -> Rc<dyn TypeMirror> {
        panic!("UnsupportedOperationException: get_enclosing_type: fixture")
    }
    fn get_type_arguments(&self) -> Vec<Rc<dyn TypeMirror>> {
        Vec::new()
    }
}
impl ErrorType for TypeFixture {}
impl TypeVariable for TypeFixture {
    fn as_element(&self) -> Rc<dyn Element> {
        panic!("UnsupportedOperationException: as_element: fixture")
    }
    fn get_upper_bound(&self) -> Rc<dyn TypeMirror> {
        panic!("UnsupportedOperationException: get_upper_bound: fixture")
    }
    fn get_lower_bound(&self) -> Rc<dyn TypeMirror> {
        panic!("UnsupportedOperationException: get_lower_bound: fixture")
    }
}
impl WildcardType for TypeFixture {
    fn get_extends_bound(&self) -> Option<Rc<dyn TypeMirror>> {
        None
    }
    fn get_super_bound(&self) -> Option<Rc<dyn TypeMirror>> {
        None
    }
}
impl ExecutableType for TypeFixture {
    fn get_type_variables(&self) -> Vec<Rc<dyn TypeVariable>> {
        Vec::new()
    }
    fn get_return_type(&self) -> Rc<dyn TypeMirror> {
        panic!("UnsupportedOperationException: get_return_type: fixture")
    }
    fn get_parameter_types(&self) -> Vec<Rc<dyn TypeMirror>> {
        Vec::new()
    }
    fn get_receiver_type(&self) -> Rc<dyn TypeMirror> {
        panic!("UnsupportedOperationException: get_receiver_type: fixture")
    }
    fn get_thrown_types(&self) -> Vec<Rc<dyn TypeMirror>> {
        Vec::new()
    }
}
impl UnionType for TypeFixture {
    fn get_alternatives(&self) -> Vec<Rc<dyn TypeMirror>> {
        Vec::new()
    }
}
impl IntersectionType for TypeFixture {
    fn get_bounds(&self) -> Vec<Rc<dyn TypeMirror>> {
        Vec::new()
    }
}

/// An annotation value of a chosen value shape.
pub struct ValueFixture {
    pub value: Value,
}

impl fmt::Display for ValueFixture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("value")
    }
}

impl AnnotationValue for ValueFixture {
    fn get_value(&self) -> Value {
        self.value.clone()
    }
    fn accept<R, P>(
        &self,
        _v: &mut dyn crate::lang::model::element::AnnotationValueVisitor<R, P>,
        _p: P,
    ) -> R {
        panic!("UnsupportedOperationException: accept: fixture is not visited through accept")
    }
}

/// A borrowing view of the fixture value (helper for tests that need ValueRef).
pub fn value_ref(value: &Value) -> ValueRef<'_> {
    match value {
        Value::Boolean(b) => ValueRef::Boolean(*b),
        Value::Byte(b) => ValueRef::Byte(*b),
        Value::Character(c) => ValueRef::Character(*c),
        Value::Double(d) => ValueRef::Double(*d),
        Value::Float(f) => ValueRef::Float(*f),
        Value::Integer(i) => ValueRef::Integer(*i),
        Value::Long(l) => ValueRef::Long(*l),
        Value::Short(s) => ValueRef::Short(*s),
        Value::String(s) => ValueRef::String(s),
        Value::Type(t) => ValueRef::Type(t.as_ref()),
        Value::EnumConstant(c) => ValueRef::EnumConstant(c.as_ref()),
        Value::Annotation(a) => ValueRef::Annotation(a.as_ref()),
        Value::Array(v) => ValueRef::Array(v),
    }
}
