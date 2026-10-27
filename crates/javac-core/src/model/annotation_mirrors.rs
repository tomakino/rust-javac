// Origin: javax/lang/model (AnnotatedConstruct.getAnnotationMirrors) and the
// javax.lang.model mirror objects of com.sun.tools.javac.model.JavacElements /
// JavacTypes @ 982d53c (jdk-17.0.19-ga)
//
// PORTING ADAPTER (registered): the upstream mirror objects are the compiler's
// own Symbol/Type/Attribute objects implementing the javax.lang.model
// interfaces. The port has no JVM object graph shared with the compiler, so the
// mirror views are immutable snapshots built from the arena data of
// CompilerContext. They implement every method of the mirror traits; the
// annotation lists of the snapshot elements themselves are empty because the
// annotation API target is the annotated Symbol (AnnotatedConstruct semantics
// for Symbol are implemented by model::symbol_annotations); this limitation is
// registered.

use std::rc::Rc;

use javac_strings::JavaString;
use javac_tools::lang::model::annotated_construct::AnnotatedConstruct;
use javac_tools::lang::model::element::annotation_mirror::AnnotationMirror;
use javac_tools::lang::model::element::annotation_value::{AnnotationValue, Value};
use javac_tools::lang::model::element::annotation_value_visitor::AnnotationValueVisitor;
use javac_tools::lang::model::element::element::Element;
use javac_tools::lang::model::element::element_kind::ElementKind;
use javac_tools::lang::model::element::element_visitor::ElementVisitor;
use javac_tools::lang::model::element::executable_element::ExecutableElement;
use javac_tools::lang::model::element::modifier::Modifier;
use javac_tools::lang::model::element::name::Name;
use javac_tools::lang::model::element::nesting_kind::NestingKind;
use javac_tools::lang::model::element::package_element::PackageElement;
use javac_tools::lang::model::element::qualified_nameable::QualifiedNameable;
use javac_tools::lang::model::element::type_element::TypeElement;
use javac_tools::lang::model::element::variable_element::{ConstantValue, VariableElement};
use javac_tools::lang::model::type_::array_type::ArrayType;
use javac_tools::lang::model::type_::declared_type::DeclaredType;
use javac_tools::lang::model::type_::no_type::NoType;
use javac_tools::lang::model::type_::primitive_type::PrimitiveType;
use javac_tools::lang::model::type_::reference_type::ReferenceType;
use javac_tools::lang::model::type_::type_kind::TypeKind;
use javac_tools::lang::model::type_::type_mirror::TypeMirror;
use javac_tools::lang::model::type_::type_visitor::TypeVisitor;

use crate::code::attribute::AttributeKind;
use crate::code::flags;
use crate::code::kinds::Kind;
use crate::code::symbol::SymbolKind;
use crate::code::symtab::Symtab;
use crate::code::type_tag::TypeTag;
use crate::compiler_context::CompilerContext;
use crate::ids::{AttributeId, SymbolId, TypeId};

/// Java: javax.lang.model.element.Name snapshot.
#[derive(Clone)]
pub struct JavacName {
    text: JavaString,
}

impl Name for JavacName {
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

impl ElementData {
    fn shallow(&self) -> ElementData {
        ElementData {
            kind: self.kind,
            nesting_kind: self.nesting_kind,
            simple_name: self.simple_name.clone(),
            qualified_name: self.qualified_name.clone(),
            modifiers: self.modifiers.clone(),
            enclosing: None,
            enclosed: Vec::new(),
            display: self.display.clone(),
            type_proxy: None,
            executable: None,
            variable: None,
        }
    }
}

impl std::fmt::Display for JavacName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
}

#[derive(Clone)]
struct ElementData {
    kind: ElementKind,
    nesting_kind: NestingKind,
    simple_name: JavaString,
    qualified_name: JavaString,
    modifiers: Vec<Modifier>,
    enclosing: Option<Rc<dyn Element>>,
    enclosed: Vec<Rc<dyn Element>>,
    display: String,
    type_proxy: Option<Rc<dyn TypeMirror>>,
    executable: Option<ExecutableData>,
    variable: Option<VariableData>,
}

#[derive(Clone)]
struct ExecutableData {
    parameters: Vec<Rc<dyn VariableElement>>,
    return_type: Rc<dyn TypeMirror>,
    receiver_type: Rc<dyn TypeMirror>,
    varargs: bool,
    is_default: bool,
    thrown: Vec<Rc<dyn TypeMirror>>,
    default_value: Option<Rc<dyn AnnotationValue>>,
}

#[derive(Clone)]
struct VariableData {
    constant_value: Option<ConstantValue>,
}

fn empty_annotations() -> Vec<Rc<dyn AnnotationMirror>> {
    Vec::new()
}

macro_rules! element_impl {
    ($ty:ty, $visit:ident, $asfn:ident, $astrait:path) => {
        impl AnnotatedConstruct for $ty {
            fn get_annotation_mirrors(&self) -> Vec<Rc<dyn AnnotationMirror>> {
                empty_annotations()
            }

            fn get_annotation<A: javac_tools::jdk::Annotation>(
                &self,
                _annotation_type: javac_tools::jdk::Class<A>,
            ) -> Option<A>
            where
                Self: Sized,
            {
                None
            }

            fn get_annotations_by_type<A: javac_tools::jdk::Annotation>(
                &self,
                _annotation_type: javac_tools::jdk::Class<A>,
            ) -> Vec<A>
            where
                Self: Sized,
            {
                Vec::new()
            }
        }

        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.data.display)
            }
        }

        impl Element for $ty {
            fn as_type(&self) -> Rc<dyn TypeMirror> {
                if let Some(proxy) = &self.data.type_proxy {
                    return proxy.clone();
                }
                if matches!(
                    self.data.kind,
                    ElementKind::CLASS
                        | ElementKind::INTERFACE
                        | ElementKind::ENUM
                        | ElementKind::ANNOTATION_TYPE
                        | ElementKind::RECORD
                ) {
                    let shallow = self.data.shallow();
                    return Rc::new(JavacDeclaredType {
                        element: Rc::new(JavacTypeElement {
                            data: Rc::new(shallow),
                        }),
                        display: self.data.qualified_name.to_string(),
                    });
                }
                Rc::new(JavacNoType)
            }

            fn get_kind(&self) -> ElementKind {
                self.data.kind
            }

            fn get_modifiers(&self) -> Vec<Modifier> {
                self.data.modifiers.clone()
            }

            fn get_simple_name(&self) -> Rc<dyn Name> {
                Rc::new(JavacName {
                    text: self.data.simple_name.clone(),
                })
            }

            fn get_enclosing_element(&self) -> Option<Rc<dyn Element>> {
                self.data.enclosing.clone()
            }

            fn get_enclosed_elements(&self) -> Vec<Rc<dyn Element>> {
                self.data.enclosed.clone()
            }

            fn $asfn(&self) -> Option<&dyn $astrait> {
                Some(self)
            }

            fn accept<R, P>(&self, v: &mut dyn ElementVisitor<R, P>, p: P) -> R {
                v.$visit(self, p)
            }
        }
    };
}

/// Java: TypeElement snapshot (a class/interface/annotation/enum element).
pub struct JavacTypeElement {
    data: Rc<ElementData>,
}

element_impl!(JavacTypeElement, visit_type, as_type_element, TypeElement);

impl QualifiedNameable for JavacTypeElement {
    fn get_qualified_name(&self) -> Rc<dyn Name> {
        Rc::new(JavacName {
            text: self.data.qualified_name.clone(),
        })
    }
}

impl javac_tools::lang::model::element::parameterizable::Parameterizable for JavacTypeElement {
    fn get_type_parameters(
        &self,
    ) -> Vec<Rc<dyn javac_tools::lang::model::element::type_parameter_element::TypeParameterElement>> {
        Vec::new()
    }
}

impl TypeElement for JavacTypeElement {
    fn get_nesting_kind(&self) -> NestingKind {
        self.data.nesting_kind
    }

    fn get_superclass(&self) -> Rc<dyn TypeMirror> {
        Rc::new(JavacNoType)
    }

    fn get_interfaces(&self) -> Vec<Rc<dyn TypeMirror>> {
        Vec::new()
    }

    fn get_record_components(
        &self,
    ) -> Vec<Rc<dyn javac_tools::lang::model::element::record_component_element::RecordComponentElement>> {
        Vec::new()
    }

    fn get_permitted_subclasses(&self) -> Vec<Rc<dyn TypeMirror>> {
        Vec::new()
    }
}

/// Java: ExecutableElement snapshot (an annotation element method).
pub struct JavacExecutableElement {
    data: Rc<ElementData>,
}

element_impl!(JavacExecutableElement, visit_executable, as_executable_element, ExecutableElement);

impl javac_tools::lang::model::element::parameterizable::Parameterizable
    for JavacExecutableElement
{
    fn get_type_parameters(
        &self,
    ) -> Vec<Rc<dyn javac_tools::lang::model::element::type_parameter_element::TypeParameterElement>> {
        Vec::new()
    }
}

impl ExecutableElement for JavacExecutableElement {
    fn get_parameters(&self) -> Vec<Rc<dyn VariableElement>> {
        self.data
            .executable
            .as_ref()
            .map(|executable| executable.parameters.clone())
            .unwrap_or_default()
    }

    fn get_return_type(&self) -> Rc<dyn TypeMirror> {
        self.data
            .executable
            .as_ref()
            .map(|executable| executable.return_type.clone())
            .unwrap_or_else(|| Rc::new(JavacNoType))
    }

    fn get_receiver_type(&self) -> Rc<dyn TypeMirror> {
        self.data
            .executable
            .as_ref()
            .map(|executable| executable.receiver_type.clone())
            .unwrap_or_else(|| Rc::new(JavacNoType))
    }

    fn is_var_args(&self) -> bool {
        self.data
            .executable
            .as_ref()
            .map(|executable| executable.varargs)
            .unwrap_or(false)
    }

    fn is_default(&self) -> bool {
        self.data
            .executable
            .as_ref()
            .map(|executable| executable.is_default)
            .unwrap_or(false)
    }

    fn get_thrown_types(&self) -> Vec<Rc<dyn TypeMirror>> {
        self.data
            .executable
            .as_ref()
            .map(|executable| executable.thrown.clone())
            .unwrap_or_default()
    }

    fn get_default_value(&self) -> Option<Rc<dyn AnnotationValue>> {
        self.data
            .executable
            .as_ref()
            .and_then(|executable| executable.default_value.clone())
    }
}

/// Java: VariableElement snapshot (an enum constant).
pub struct JavacVariableElement {
    data: Rc<ElementData>,
}

element_impl!(JavacVariableElement, visit_variable, as_variable_element, VariableElement);

impl VariableElement for JavacVariableElement {
    fn get_constant_value(&self) -> Option<ConstantValue> {
        self.data
            .variable
            .as_ref()
            .and_then(|variable| variable.constant_value.clone())
    }
}

/// Java: PackageElement snapshot.
pub struct JavacPackageElement {
    data: Rc<ElementData>,
    unnamed: bool,
}

element_impl!(JavacPackageElement, visit_package, as_package_element, PackageElement);

impl PackageElement for JavacPackageElement {
    fn is_unnamed(&self) -> bool {
        self.unnamed
    }
}

impl QualifiedNameable for JavacPackageElement {
    fn get_qualified_name(&self) -> Rc<dyn Name> {
        Rc::new(JavacName {
            text: self.data.qualified_name.clone(),
        })
    }
}

/// Java: DeclaredType snapshot.
pub struct JavacDeclaredType {
    element: Rc<dyn Element>,
    display: String,
}

impl std::fmt::Display for JavacDeclaredType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display)
    }
}

impl AnnotatedConstruct for JavacDeclaredType {
    fn get_annotation_mirrors(&self) -> Vec<Rc<dyn AnnotationMirror>> {
        empty_annotations()
    }

    fn get_annotation<A: javac_tools::jdk::Annotation>(
        &self,
        _annotation_type: javac_tools::jdk::Class<A>,
    ) -> Option<A>
    where
        Self: Sized,
    {
        None
    }

    fn get_annotations_by_type<A: javac_tools::jdk::Annotation>(
        &self,
        _annotation_type: javac_tools::jdk::Class<A>,
    ) -> Vec<A>
    where
        Self: Sized,
    {
        Vec::new()
    }
}

impl TypeMirror for JavacDeclaredType {
    fn get_kind(&self) -> TypeKind {
        TypeKind::DECLARED
    }

    fn accept<R, P>(&self, v: &mut dyn TypeVisitor<R, P>, p: P) -> R {
        v.visit_declared(self, p)
    }

    fn as_declared_type(&self) -> Option<&dyn DeclaredType> {
        Some(self)
    }
}

impl ReferenceType for JavacDeclaredType {}

impl DeclaredType for JavacDeclaredType {
    fn as_element(&self) -> Rc<dyn Element> {
        self.element.clone()
    }

    fn get_enclosing_type(&self) -> Rc<dyn TypeMirror> {
        Rc::new(JavacNoType)
    }

    fn get_type_arguments(&self) -> Vec<Rc<dyn TypeMirror>> {
        Vec::new()
    }
}

/// Java: NoType snapshot (TypeKind.NONE).
pub struct JavacNoType;

impl std::fmt::Display for JavacNoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "none")
    }
}

impl AnnotatedConstruct for JavacNoType {
    fn get_annotation_mirrors(&self) -> Vec<Rc<dyn AnnotationMirror>> {
        empty_annotations()
    }

    fn get_annotation<A: javac_tools::jdk::Annotation>(
        &self,
        _annotation_type: javac_tools::jdk::Class<A>,
    ) -> Option<A>
    where
        Self: Sized,
    {
        None
    }

    fn get_annotations_by_type<A: javac_tools::jdk::Annotation>(
        &self,
        _annotation_type: javac_tools::jdk::Class<A>,
    ) -> Vec<A>
    where
        Self: Sized,
    {
        Vec::new()
    }
}

impl TypeMirror for JavacNoType {
    fn get_kind(&self) -> TypeKind {
        TypeKind::NONE
    }

    fn accept<R, P>(&self, v: &mut dyn TypeVisitor<R, P>, p: P) -> R {
        v.visit_no_type(self, p)
    }

    fn as_no_type(&self) -> Option<&dyn NoType> {
        Some(self)
    }
}

impl NoType for JavacNoType {}

/// Java: ArrayType snapshot.
pub struct JavacArrayType {
    component: Rc<dyn TypeMirror>,
    display: String,
}

impl std::fmt::Display for JavacArrayType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display)
    }
}

impl AnnotatedConstruct for JavacArrayType {
    fn get_annotation_mirrors(&self) -> Vec<Rc<dyn AnnotationMirror>> {
        empty_annotations()
    }

    fn get_annotation<A: javac_tools::jdk::Annotation>(
        &self,
        _annotation_type: javac_tools::jdk::Class<A>,
    ) -> Option<A>
    where
        Self: Sized,
    {
        None
    }

    fn get_annotations_by_type<A: javac_tools::jdk::Annotation>(
        &self,
        _annotation_type: javac_tools::jdk::Class<A>,
    ) -> Vec<A>
    where
        Self: Sized,
    {
        Vec::new()
    }
}

impl TypeMirror for JavacArrayType {
    fn get_kind(&self) -> TypeKind {
        TypeKind::ARRAY
    }

    fn accept<R, P>(&self, v: &mut dyn TypeVisitor<R, P>, p: P) -> R {
        v.visit_array(self, p)
    }

    fn as_array_type(&self) -> Option<&dyn ArrayType> {
        Some(self)
    }
}

impl ReferenceType for JavacArrayType {}

impl ArrayType for JavacArrayType {
    fn get_component_type(&self) -> Rc<dyn TypeMirror> {
        self.component.clone()
    }
}

/// Java: PrimitiveType snapshot (a primitive Class value).
pub struct JavacPrimitiveType {
    kind: TypeKind,
    display: String,
}

impl std::fmt::Display for JavacPrimitiveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display)
    }
}

impl AnnotatedConstruct for JavacPrimitiveType {
    fn get_annotation_mirrors(&self) -> Vec<Rc<dyn AnnotationMirror>> {
        empty_annotations()
    }

    fn get_annotation<A: javac_tools::jdk::Annotation>(
        &self,
        _annotation_type: javac_tools::jdk::Class<A>,
    ) -> Option<A>
    where
        Self: Sized,
    {
        None
    }

    fn get_annotations_by_type<A: javac_tools::jdk::Annotation>(
        &self,
        _annotation_type: javac_tools::jdk::Class<A>,
    ) -> Vec<A>
    where
        Self: Sized,
    {
        Vec::new()
    }
}

impl TypeMirror for JavacPrimitiveType {
    fn get_kind(&self) -> TypeKind {
        self.kind
    }

    fn accept<R, P>(&self, v: &mut dyn TypeVisitor<R, P>, p: P) -> R {
        v.visit_primitive(self, p)
    }

    fn as_primitive_type(&self) -> Option<&dyn PrimitiveType> {
        Some(self)
    }
}

impl PrimitiveType for JavacPrimitiveType {}

/// Java: AnnotationValue snapshot.
pub struct JavacAnnotationValue {
    value: Value,
    display: String,
}

impl std::fmt::Display for JavacAnnotationValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display)
    }
}

impl AnnotationValue for JavacAnnotationValue {
    fn get_value(&self) -> Value {
        self.value.clone()
    }

    fn accept<R, P>(&self, v: &mut dyn AnnotationValueVisitor<R, P>, p: P) -> R {
        match &self.value {
            Value::Boolean(value) => v.visit_boolean(*value, p),
            Value::Byte(value) => v.visit_byte(*value, p),
            Value::Character(value) => v.visit_char(*value, p),
            Value::Double(value) => v.visit_double(*value, p),
            Value::Float(value) => v.visit_float(*value, p),
            Value::Integer(value) => v.visit_int(*value, p),
            Value::Long(value) => v.visit_long(*value, p),
            Value::Short(value) => v.visit_short(*value, p),
            Value::String(value) => v.visit_string(value, p),
            Value::Type(value) => v.visit_type(&**value, p),
            Value::EnumConstant(value) => v.visit_enum_constant(&**value, p),
            Value::Annotation(value) => v.visit_annotation(&**value, p),
            Value::Array(values) => v.visit_array(values, p),
        }
    }
}

/// Java: AnnotationMirror snapshot.
pub struct JavacAnnotationMirror {
    annotation_type: Rc<dyn DeclaredType>,
    element_values: Vec<(Rc<dyn ExecutableElement>, Rc<dyn AnnotationValue>)>,
    display: String,
}

impl std::fmt::Display for JavacAnnotationMirror {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display)
    }
}

impl AnnotationMirror for JavacAnnotationMirror {
    fn get_annotation_type(&self) -> Rc<dyn DeclaredType> {
        self.annotation_type.clone()
    }

    fn get_element_values(&self) -> Vec<(Rc<dyn ExecutableElement>, Rc<dyn AnnotationValue>)> {
        self.element_values.clone()
    }
}

/// Java: Symbol.getAnnotationMirrors() (Symbol.java:731-733): the raw compound
/// attributes as mirror views.
///
/// # Arguments
///   - ctx: the compiler context;
///   - syms: the symbol table;
///   - symbol: the annotated symbol.
///
/// # Returns
///   - the declaration-annotation mirrors in declaration order.
///
/// # Panics
///   - none.
pub fn annotation_mirrors(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    symbol: SymbolId,
) -> Vec<Rc<dyn AnnotationMirror>> {
    let attributes = ctx.sym(symbol).get_raw_attributes();
    let mut mirrors = Vec::new();
    for attribute in attributes.iter() {
        if matches!(
            ctx.attr(attribute).kind,
            AttributeKind::Compound { .. } | AttributeKind::TypeCompound { .. }
        ) {
            mirrors.push(build_annotation_mirror(ctx, syms, attribute));
        }
    }
    mirrors
}

/// Build one AnnotationMirror snapshot from an Attribute.Compound.
///
/// # Arguments
///   - ctx: the compiler context;
///   - syms: the symbol table;
///   - attribute: the compound annotation attribute.
///
/// # Returns
///   - the mirror view.
///
/// # Panics
///   - when the attribute has no annotation type symbol.
pub fn build_annotation_mirror(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    attribute: AttributeId,
) -> Rc<dyn AnnotationMirror> {
    build_annotation_mirror_parts(ctx, syms, attribute).1
}

fn build_annotation_mirror_parts(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    attribute: AttributeId,
) -> (String, Rc<dyn AnnotationMirror>) {
    let annotation_type = ctx
        .attr(attribute)
        .ty
        .and_then(|ty| ctx.typ(ty).tsym)
        .expect("annotation type symbol");
    let type_element = type_element(ctx, syms, annotation_type, 0);
    let annotation_type_proxy: Rc<dyn DeclaredType> = Rc::new(JavacDeclaredType {
        element: type_element.clone() as Rc<dyn Element>,
        display: qualified_name(ctx, annotation_type),
    });
    let mut element_values = Vec::new();
    let mut value_display = Vec::new();
    if let AttributeKind::Compound { values, .. } | AttributeKind::TypeCompound { values, .. } =
        &ctx.attr(attribute).kind
    {
        let values = values.clone();
        for pair in values.iter() {
            let method = executable_element(ctx, syms, pair.fst, 1);
            let (value, display) = annotation_value(ctx, syms, pair.snd, 1);
            value_display.push(format!(
                "{}={}",
                ctx.sym(pair.fst).name.to_java_string(),
                display
            ));
            element_values.push((method as Rc<dyn ExecutableElement>, value));
        }
    }
    let display = if value_display.is_empty() {
        format!("@{}", qualified_name(ctx, annotation_type))
    } else {
        format!(
            "@{}({})",
            qualified_name(ctx, annotation_type),
            value_display.join(",")
        )
    };
    (
        display.clone(),
        Rc::new(JavacAnnotationMirror {
            annotation_type: annotation_type_proxy,
            element_values,
            display,
        }),
    )
}

fn nesting_kind(ctx: &CompilerContext, symbol: SymbolId) -> NestingKind {
    match ctx.sym(symbol).owner {
        Some(owner) if ctx.sym(owner).kind == Kind::PCK => NestingKind::TOP_LEVEL,
        Some(owner) if ctx.sym(owner).kind == Kind::MTH => NestingKind::LOCAL,
        Some(_) => NestingKind::MEMBER,
        None => NestingKind::TOP_LEVEL,
    }
}

fn qualified_name(ctx: &CompilerContext, symbol: SymbolId) -> String {
    ctx.sym(symbol).get_qualified_name().to_java_string().to_string()
}

fn modifiers(ctx: &CompilerContext, symbol: SymbolId) -> Vec<Modifier> {
    let flags_field = ctx.sym(symbol).flags_field;
    let mut result = Vec::new();
    if (flags_field & flags::PUBLIC as i64) != 0 {
        result.push(Modifier::PUBLIC);
    }
    if (flags_field & flags::PROTECTED as i64) != 0 {
        result.push(Modifier::PROTECTED);
    }
    if (flags_field & flags::PRIVATE as i64) != 0 {
        result.push(Modifier::PRIVATE);
    }
    if (flags_field & flags::ABSTRACT as i64) != 0 {
        result.push(Modifier::ABSTRACT);
    }
    if (flags_field & flags::STATIC as i64) != 0 {
        result.push(Modifier::STATIC);
    }
    if (flags_field & flags::FINAL as i64) != 0 {
        result.push(Modifier::FINAL);
    }
    result
}

fn type_element(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    symbol: SymbolId,
    depth: usize,
) -> Rc<JavacTypeElement> {
    let flags_field = ctx.sym(symbol).flags_field;
    let kind = if (flags_field & flags::ANNOTATION as i64) != 0 {
        ElementKind::ANNOTATION_TYPE
    } else if (flags_field & flags::INTERFACE as i64) != 0 {
        ElementKind::INTERFACE
    } else if (flags_field & flags::ENUM as i64) != 0 {
        ElementKind::ENUM
    } else if (flags_field & flags::RECORD) != 0 {
        ElementKind::RECORD
    } else {
        ElementKind::CLASS
    };
    let mut enclosed = Vec::new();
    if depth == 0
        && let Some(members) = ctx.sym(symbol).members()
    {
        for member in ctx.scope_get_symbols(
            members,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        ) {
            if ctx.sym(member).kind == Kind::MTH {
                enclosed.push(
                    executable_element(ctx, syms, member, depth + 1) as Rc<dyn Element>
                );
            } else if ctx.sym(member).kind == Kind::VAR
                && (ctx.sym(member).flags_field & flags::ENUM as i64) != 0
            {
                enclosed.push(
                    variable_element(ctx, syms, member, depth + 1) as Rc<dyn Element>
                );
            }
        }
    }
    let enclosing = ctx.sym(symbol).owner.and_then(|owner| {
        if ctx.sym(owner).kind == Kind::PCK {
            Some(package_element(ctx, owner) as Rc<dyn Element>)
        } else {
            None
        }
    });
    let display = qualified_name(ctx, symbol);
    Rc::new(JavacTypeElement {
        data: Rc::new(ElementData {
            kind,
            nesting_kind: nesting_kind(ctx, symbol),
            simple_name: ctx.sym(symbol).name.to_java_string(),
            qualified_name: ctx.sym(symbol).get_qualified_name().to_java_string(),
            modifiers: modifiers(ctx, symbol),
            enclosing,
            enclosed,
            display,
            type_proxy: None,
            executable: None,
            variable: None,
        }),
    })
}

fn package_element(
    ctx: &mut CompilerContext,
    symbol: SymbolId,
) -> Rc<JavacPackageElement> {
    let fullname = ctx.sym(symbol).get_qualified_name().to_java_string();
    let simple_name = ctx.sym(symbol).name.to_java_string();
    Rc::new(JavacPackageElement {
        unnamed: ctx.sym(symbol).name.is_empty(),
        data: Rc::new(ElementData {
            kind: ElementKind::PACKAGE,
            nesting_kind: NestingKind::TOP_LEVEL,
            simple_name,
            qualified_name: fullname.clone(),
            modifiers: Vec::new(),
            enclosing: None,
            enclosed: Vec::new(),
            display: fullname.to_string(),
            type_proxy: None,
            executable: None,
            variable: None,
        }),
    })
}

fn executable_element(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    symbol: SymbolId,
    depth: usize,
) -> Rc<JavacExecutableElement> {
    let display = format!(
        "{}.{}",
        qualified_name_of_owner(ctx, symbol),
        ctx.sym(symbol).name.to_java_string()
    );
    let return_type = ctx
        .sym(symbol)
        .ty
        .and_then(|ty| ctx.typ(ty).get_return_type(ctx))
        .map(|ty| type_proxy(ctx, syms, ty, depth + 1))
        .unwrap_or_else(|| Rc::new(JavacNoType));
    let default_value = match &ctx.sym(symbol).kind_data {
        SymbolKind::MethodSymbol {
            default_value: Some(default),
            ..
        } => {
            let (value, _display) = annotation_value(ctx, syms, *default, depth + 1);
            Some(value)
        }
        _ => None,
    };
    Rc::new(JavacExecutableElement {
        data: Rc::new(ElementData {
            kind: ElementKind::METHOD,
            nesting_kind: NestingKind::TOP_LEVEL,
            simple_name: ctx.sym(symbol).name.to_java_string(),
            qualified_name: ctx.sym(symbol).name.to_java_string(),
            modifiers: modifiers(ctx, symbol),
            enclosing: None,
            enclosed: Vec::new(),
            display,
            type_proxy: None,
            executable: Some(ExecutableData {
                parameters: Vec::new(),
                return_type,
                receiver_type: Rc::new(JavacNoType),
                varargs: false,
                is_default: default_value.is_some(),
                thrown: Vec::new(),
                default_value,
            }),
            variable: None,
        }),
    })
}

fn qualified_name_of_owner(ctx: &CompilerContext, symbol: SymbolId) -> String {
    ctx.sym(symbol)
        .owner
        .map(|owner| {
            ctx.sym(owner)
                .get_qualified_name()
                .to_java_string()
                .to_string()
        })
        .unwrap_or_default()
}

fn variable_element(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    symbol: SymbolId,
    depth: usize,
) -> Rc<JavacVariableElement> {
    let display = format!(
        "{}.{}",
        qualified_name_of_owner(ctx, symbol),
        ctx.sym(symbol).name.to_java_string()
    );
    let type_proxy = ctx
        .sym(symbol)
        .ty
        .map(|ty| type_proxy(ctx, syms, ty, depth + 1));
    Rc::new(JavacVariableElement {
        data: Rc::new(ElementData {
            kind: ElementKind::ENUM_CONSTANT,
            nesting_kind: NestingKind::TOP_LEVEL,
            simple_name: ctx.sym(symbol).name.to_java_string(),
            qualified_name: ctx.sym(symbol).name.to_java_string(),
            modifiers: modifiers(ctx, symbol),
            enclosing: None,
            enclosed: Vec::new(),
            display,
            type_proxy,
            executable: None,
            variable: Some(VariableData {
                constant_value: None,
            }),
        }),
    })
}

fn type_proxy(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    ty: TypeId,
    depth: usize,
) -> Rc<dyn TypeMirror> {
    let display = crate::code::type_::type_to_string(ctx, ty);
    match ctx.typ(ty).get_tag() {
        TypeTag::CLASS => {
            let symbol = ctx.typ(ty).tsym;
            match symbol {
                Some(symbol) => Rc::new(JavacDeclaredType {
                    element: type_element(ctx, syms, symbol, depth + 1) as Rc<dyn Element>,
                    display,
                }),
                None => Rc::new(JavacNoType),
            }
        }
        TypeTag::ARRAY => {
            let component = match &ctx.typ(ty).kind {
                crate::code::type_::TypeKindData::ArrayType { elemtype, .. } => elemtype
                    .map(|elemtype| type_proxy(ctx, syms, elemtype, depth + 1))
                    .unwrap_or_else(|| Rc::new(JavacNoType)),
                _ => Rc::new(JavacNoType),
            };
            Rc::new(JavacArrayType { component, display })
        }
        tag => {
            let kind = match tag {
                TypeTag::BOOLEAN => TypeKind::BOOLEAN,
                TypeTag::BYTE => TypeKind::BYTE,
                TypeTag::CHAR => TypeKind::CHAR,
                TypeTag::SHORT => TypeKind::SHORT,
                TypeTag::INT => TypeKind::INT,
                TypeTag::LONG => TypeKind::LONG,
                TypeTag::FLOAT => TypeKind::FLOAT,
                TypeTag::DOUBLE => TypeKind::DOUBLE,
                TypeTag::VOID => TypeKind::VOID,
                _ => TypeKind::OTHER,
            };
            if matches!(
                tag,
                TypeTag::BOOLEAN
                    | TypeTag::BYTE
                    | TypeTag::CHAR
                    | TypeTag::SHORT
                    | TypeTag::INT
                    | TypeTag::LONG
                    | TypeTag::FLOAT
                    | TypeTag::DOUBLE
            ) {
                Rc::new(JavacPrimitiveType { kind, display })
            } else {
                Rc::new(JavacNoType)
            }
        }
    }
}

fn annotation_value(
    ctx: &mut CompilerContext,
    syms: &Symtab,
    attribute: AttributeId,
    depth: usize,
) -> (Rc<dyn AnnotationValue>, String) {
    match &ctx.attr(attribute).kind {
        AttributeKind::Constant { value } => {
            let value = constant_value(value);
            let display = value_display(&value);
            (Rc::new(JavacAnnotationValue { value, display: display.clone() }), display)
        }
        AttributeKind::Class { class_type } => {
            let value = Value::Type(
                class_type
                    .map(|ty| type_proxy(ctx, syms, ty, depth + 1))
                    .unwrap_or_else(|| Rc::new(JavacNoType)),
            );
            let display = value_display(&value);
            (Rc::new(JavacAnnotationValue { value, display: display.clone() }), display)
        }
        AttributeKind::Array { values } => {
            let values = values.clone();
            let mut proxies = Vec::new();
            for value in values {
                let (proxy, _display) = annotation_value(ctx, syms, value, depth + 1);
                proxies.push(proxy);
            }
            let value = Value::Array(proxies);
            let display = value_display(&value);
            (Rc::new(JavacAnnotationValue { value, display: display.clone() }), display)
        }
        AttributeKind::Enum { value } => {
            let constant = value
                .map(|value| variable_element(ctx, syms, value, depth + 1) as Rc<dyn VariableElement>)
                .expect("Attribute.Enum.getValue: VarSymbol");
            let value = Value::EnumConstant(constant);
            let display = value_display(&value);
            (Rc::new(JavacAnnotationValue { value, display: display.clone() }), display)
        }
        AttributeKind::Compound { .. } | AttributeKind::TypeCompound { .. } => {
            let (display, mirror) = build_annotation_mirror_parts(ctx, syms, attribute);
            let value = Value::Annotation(mirror);
            (
                Rc::new(JavacAnnotationValue { value, display: display.clone() }),
                display,
            )
        }
        AttributeKind::Error => {
            let value = Value::String(JavaString::from("<error>"));
            let display = value_display(&value);
            (Rc::new(JavacAnnotationValue { value, display: display.clone() }), display)
        }
        AttributeKind::UnresolvedClass { .. } => {
            let value = Value::String(JavaString::from("<error>"));
            let display = value_display(&value);
            (Rc::new(JavacAnnotationValue { value, display: display.clone() }), display)
        }
    }
}

fn constant_value(value: &crate::util::constants::ConstantValue) -> Value {
    match value {
        crate::util::constants::ConstantValue::Boolean(value) => Value::Boolean(*value),
        crate::util::constants::ConstantValue::Byte(value) => Value::Byte(*value),
        crate::util::constants::ConstantValue::Character(value) => Value::Character(*value),
        crate::util::constants::ConstantValue::Short(value) => Value::Short(*value),
        crate::util::constants::ConstantValue::Integer(value) => Value::Integer(*value),
        crate::util::constants::ConstantValue::Long(value) => Value::Long(*value),
        crate::util::constants::ConstantValue::Float(value) => Value::Float(*value),
        crate::util::constants::ConstantValue::Double(value) => Value::Double(*value),
        crate::util::constants::ConstantValue::String(value) => Value::String(value.clone()),
    }
}

fn value_display(value: &Value) -> String {
    match value {
        Value::Boolean(value) => value.to_string(),
        Value::Byte(value) => value.to_string(),
        Value::Character(value) => char::from_u32(*value as u32)
            .map(|value| format!("'{}'", value))
            .unwrap_or_default(),
        Value::Double(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::Integer(value) => value.to_string(),
        Value::Long(value) => value.to_string(),
        Value::Short(value) => value.to_string(),
        Value::String(value) => format!("\"{value}\""),
        Value::Type(value) => format!("{}.class", value),
        Value::EnumConstant(value) => format!("{}", value),
        Value::Annotation(_) => String::from("<annotation>"),
        Value::Array(values) => {
            let values: Vec<String> = values.iter().map(|value| format!("{}", value)).collect();
            format!("[{}]", values.join(","))
        }
    }
}
