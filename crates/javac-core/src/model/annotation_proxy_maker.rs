//
// PORTING ADAPTER (registered): the upstream generator produces JVM dynamic
// proxies of java.lang.annotation.Annotation through
// sun.reflect.annotation.AnnotationParser/AnnotationInvocationHandler. The port
// has no JVM/reflection runtime, so the same "dynamic proxy return form" is
// represented explicitly by javac_tools::jdk::AnnotationProxy values: the
// annotation type binary name plus the ordered element values (primitive
// wrappers, String, Class names, enum constants, nested annotations, arrays and
// the ExceptionProxy alternatives). Every branch of the upstream generator is
// mapped below; the only unobservable difference is the JVM proxy object
// identity (upstream proxies are compared through Annotation.equals).

use std::rc::Rc;

use javac_strings::JavaString;
use javac_tools::jdk::{
    AnnotationProxy, AnnotationProxyException, AnnotationValueProxy,
};

use crate::code::attribute::AttributeKind;
use crate::code::flags;
use crate::code::scope::LookupKind;
use crate::code::symbol::SymbolKind;
use crate::code::type_::TypeKindData;
use crate::code::type_tag::TypeTag;
use crate::compiler_context::CompilerContext;
use crate::ids::{AttributeId, SymbolId, TypeId};
use crate::util::constants::ConstantValue;
use crate::util::pair::Pair;

/// The ported dynamic annotation proxy (AnnotationProxyMaker.JavacAnnotationProxy).
pub struct JavacAnnotationProxy {
    /// The annotation type's binary (flat) name.
    type_name: JavaString,
    /// The element values in upstream LinkedHashMap order.
    values: Vec<(JavaString, AnnotationValueProxy)>,
}

impl JavacAnnotationProxy {
    fn new(type_name: JavaString, values: Vec<(JavaString, AnnotationValueProxy)>) -> JavacAnnotationProxy {
        JavacAnnotationProxy { type_name, values }
    }
}

impl AnnotationProxy for JavacAnnotationProxy {
    fn annotation_type_name(&self) -> &JavaString {
        &self.type_name
    }

    fn member_names(&self) -> Vec<JavaString> {
        self.values.iter().map(|(name, _)| name.clone()).collect()
    }

    fn member_value(&self, name: &JavaString) -> Option<AnnotationValueProxy> {
        self.values
            .iter()
            .find(|(element, _)| element == name)
            .map(|(_, value)| value.clone())
    }
}

impl std::fmt::Display for JavacAnnotationProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}", self.type_name)?;
        if self.values.is_empty() {
            return Ok(());
        }
        write!(f, "(")?;
        for (index, (name, value)) in self.values.iter().enumerate() {
            if index != 0 {
                write!(f, ", ")?;
            }
            write!(f, "{name}={value}")?;
        }
        write!(f, ")")
    }
}

/// Java: public static <A extends Annotation> A generateAnnotation(
/// Attribute.Compound anno, Class<A> annoType) (AnnotationProxyMaker.java:96-99).
///
/// PORTING ADAPTER: the Class parameter only supplies the annotation type of
/// the compound (upstream also uses it for the proxy interface); the port reads
/// that type from the Attribute.Compound itself, exactly like the upstream
/// getAllValues implementation does (anno.type.tsym).
///
/// # Arguments
///   - ctx: the compiler context (attribute/type/symbol arenas);
///   - anno: the compound annotation attribute.
///
/// # Returns
///   - the generated annotation proxy.
///
/// # Panics
///   - when the annotation attribute is not a compound or its type/members are
///     missing (class-file invariant, as in the upstream NPE/ClassCast paths).
pub fn generate_annotation(ctx: &mut CompilerContext, anno: AttributeId) -> Rc<dyn AnnotationProxy> {
    Rc::new(generate_annotation_value(ctx, anno))
}

fn generate_annotation_value(ctx: &mut CompilerContext, anno: AttributeId) -> JavacAnnotationProxy {
    let annotation_type = ctx
        .attr(anno)
        .ty
        .and_then(|ty| ctx.typ(ty).tsym)
        .expect("annotation type symbol");
    let type_name = ctx.sym(annotation_type).flat_name().to_java_string();
    let members = ctx
        .sym(annotation_type)
        .members()
        .expect("annotation type members");
    let mut values: Vec<(SymbolId, AttributeId)> = Vec::new();
    // First find the default values (AnnotationProxyMaker.java:122-135).
    for member in ctx.scope_get_symbols(members, LookupKind::NON_RECURSIVE, &|_| true) {
        if ctx.sym(member).kind != crate::code::kinds::Kind::MTH {
            continue;
        }
        if let SymbolKind::MethodSymbol {
            default_value: Some(default),
            ..
        } = &ctx.sym(member).kind_data
        {
            values.push((member, *default));
        }
    }
    // Next find the explicit values, possibly overriding defaults
    // (AnnotationProxyMaker.java:127-130: LinkedHashMap.put keeps the position
    // of an existing key).
    let explicit = match &ctx.attr(anno).kind {
        AttributeKind::Compound { values, .. } | AttributeKind::TypeCompound { values, .. } => {
            values.clone()
        }
        _ => panic!("AssertionError: not a compound annotation"),
    };
    let explicit: Vec<Pair<SymbolId, AttributeId>> = explicit.iter().collect();
    for pair in explicit {
        match values.iter_mut().find(|(member, _)| *member == pair.fst) {
            Some(slot) => slot.1 = pair.snd,
            None => values.push((pair.fst, pair.snd)),
        }
    }
    let mut reflected: Vec<(JavaString, AnnotationValueProxy)> = Vec::new();
    for (member, value) in values {
        if let Some(generated) = generate_value(ctx, member, value, None) {
            reflected.push((ctx.sym(member).name.to_java_string(), generated));
        }
    }
    JavacAnnotationProxy::new(type_name, reflected)
}

/// Java: Object generateValue(MethodSymbol meth, Attribute attr)
/// (AnnotationProxyMaker.java:145-148 + ValueVisitor 151-268).
fn generate_value(
    ctx: &mut CompilerContext,
    meth: SymbolId,
    attr: AttributeId,
    expected: Option<TypeId>,
) -> Option<AnnotationValueProxy> {
    let return_type = match expected {
        Some(return_type) => return_type,
        None => {
            let method_ty = ctx.sym(meth).ty?;
            ctx.typ(method_ty).get_return_type(ctx)?
        }
    };
    let annotation_name = ctx
        .attr(attr)
        .ty
        .and_then(|ty| ctx.typ(ty).tsym)
        .map(|tsym| ctx.sym(tsym).flat_name().to_java_string())
        .unwrap_or_else(|| JavaString::from("<error>"));
    let element_name = ctx.sym(meth).name.to_java_string();
    let attr_kind = ctx.attr(attr).kind.clone();
    let value = match &attr_kind {
        AttributeKind::Constant { value } => {
            Some(annotation_constant_value(value))
        }
        AttributeKind::Class { class_type } => {
            let type_name = class_type
                .map(|ty| type_name_of(ctx, ty))
                .unwrap_or_else(|| JavaString::from("<error>"));
            Some(AnnotationValueProxy::Exception(
                AnnotationProxyException::MirroredType {
                    type_name,
                    annotation: annotation_name.clone(),
                    element: element_name.clone(),
                },
            ))
        }
        AttributeKind::Array { values } => {
            generate_array_value(ctx, meth, attr, values, return_type, &element_name)
        }
        AttributeKind::Enum { value } => {
            generate_enum_value(ctx, attr, *value)
        }
        AttributeKind::Compound { .. } | AttributeKind::TypeCompound { .. } => {
            let return_tsym = ctx.typ(return_type).tsym?;
            if (ctx.sym(return_tsym).flags_field & flags::ANNOTATION as i64) == 0 {
                None
            } else {
                Some(AnnotationValueProxy::Annotation(Rc::new(
                    generate_annotation_value(ctx, attr),
                )))
            }
        }
        AttributeKind::Error => None,
        AttributeKind::UnresolvedClass { class_type } => {
            let type_name = class_type
                .map(|ty| type_name_of(ctx, ty))
                .unwrap_or_else(|| JavaString::from("<error>"));
            Some(AnnotationValueProxy::Exception(
                AnnotationProxyException::MirroredType {
                    type_name,
                    annotation: annotation_name.clone(),
                    element: element_name.clone(),
                },
            ))
        }
    }?;
    // The upstream check: an exception proxy passes; otherwise
    // AnnotationType.invocationHandlerReturnType(returnClass).isInstance(value).
    if matches!(value, AnnotationValueProxy::Exception(_)) {
        return Some(value);
    }
    if value_matches(ctx, &value, return_type) {
        Some(value)
    } else {
        let found_type = ctx
            .attr(attr)
            .ty
            .map(|ty| crate::code::type_::type_to_string(ctx, ty))
            .unwrap_or_else(|| "<error>".to_string());
        Some(AnnotationValueProxy::Exception(
            AnnotationProxyException::AnnotationTypeMismatch {
                annotation: annotation_name,
                element: element_name,
                found_type: JavaString::from(found_type.as_str()),
            },
        ))
    }
}

/// Java: ValueVisitor.visitArray (AnnotationProxyMaker.java:179-207).
fn generate_array_value(
    ctx: &mut CompilerContext,
    meth: SymbolId,
    attr: AttributeId,
    values: &[AttributeId],
    return_type: TypeId,
    element_name: &JavaString,
) -> Option<AnnotationValueProxy> {
    let array_ty = ctx.attr(attr).ty?;
    let elem_type = {
        match &ctx.typ(array_ty).kind {
            TypeKindData::ArrayType { elemtype, .. } => (*elemtype)?,
            _ => return None,
        }
    };
    let elem_name = ctx
        .typ(elem_type)
        .tsym
        .map(|tsym| ctx.sym(tsym).get_qualified_name().to_java_string())
        .unwrap_or_default();
    if elem_name == JavaString::from("java.lang.Class") {
        // Construct a proxy for a MirroredTypesException.
        let mut type_names = Vec::new();
        for value in values {
            let class_type = match &ctx.attr(*value).kind {
                AttributeKind::Class { class_type } => (*class_type)?,
                _ => return None,
            };
            type_names.push(type_name_of(ctx, class_type));
        }
        let annotation_name = ctx
            .attr(attr)
            .ty
            .and_then(|ty| ctx.typ(ty).tsym)
            .map(|tsym| ctx.sym(tsym).flat_name().to_java_string())
            .unwrap_or_else(|| JavaString::from("<error>"));
        return Some(AnnotationValueProxy::Exception(
            AnnotationProxyException::MirroredTypes {
                type_names,
                annotation: annotation_name,
                element: element_name.clone(),
            },
        ));
    }
    let return_elem = {
        match &ctx.typ(return_type).kind {
            TypeKindData::ArrayType { elemtype, .. } => (*elemtype)?,
            _ => return None,
        }
    };
    let mut result = Vec::new();
    for value in values {
        match generate_value(ctx, meth, *value, Some(return_elem)) {
            None => return None,
            Some(AnnotationValueProxy::Exception(exception)) => {
                return Some(AnnotationValueProxy::Exception(exception));
            }
            Some(item) => {
                if !value_matches(ctx, &item, return_elem) {
                    return None;
                }
                result.push(item);
            }
        }
    }
    Some(AnnotationValueProxy::Array(result))
}

/// Java: ValueVisitor.visitEnum (AnnotationProxyMaker.java:224-238).
fn generate_enum_value(
    ctx: &mut CompilerContext,
    attr: AttributeId,
    value: Option<SymbolId>,
) -> Option<AnnotationValueProxy> {
    let enum_type = ctx.attr(attr).ty?;
    let enum_tsym = ctx.typ(enum_type).tsym?;
    let enum_name = ctx.sym(enum_tsym).flat_name().to_java_string();
    if (ctx.sym(enum_tsym).flags_field & flags::ENUM as i64) == 0 {
        return None;
    }
    let value = value?;
    let constant_name = ctx.sym(value).name.to_java_string();
    let members = ctx.sym(enum_tsym).members()?;
    let exists = ctx
        .scope_get_symbols(members, LookupKind::NON_RECURSIVE, &|_| true)
        .into_iter()
        .any(|member| {
            ctx.sym(member).kind == crate::code::kinds::Kind::VAR
                && ctx.sym(member).name.to_java_string() == constant_name
        });
    if exists {
        Some(AnnotationValueProxy::Enum {
            enum_type: enum_name,
            constant: constant_name,
        })
    } else {
        Some(AnnotationValueProxy::Exception(
            AnnotationProxyException::EnumConstantNotPresent {
                enum_type: enum_name,
                constant: constant_name,
            },
        ))
    }
}

/// Java: AnnotationProxyMaker.ValueVisitor.getValue's
/// AnnotationType.invocationHandlerReturnType(returnClass).isInstance(value)
/// check (AnnotationProxyMaker.java:161-170), expressed against the element
/// method's declared return type.
fn value_matches(ctx: &CompilerContext, value: &AnnotationValueProxy, expected: TypeId) -> bool {
    match value {
        AnnotationValueProxy::Boolean(_) => ctx.typ(expected).has_tag(TypeTag::BOOLEAN),
        AnnotationValueProxy::Byte(_) => ctx.typ(expected).has_tag(TypeTag::BYTE),
        AnnotationValueProxy::Character(_) => ctx.typ(expected).has_tag(TypeTag::CHAR),
        AnnotationValueProxy::Short(_) => ctx.typ(expected).has_tag(TypeTag::SHORT),
        AnnotationValueProxy::Integer(_) => ctx.typ(expected).has_tag(TypeTag::INT),
        AnnotationValueProxy::Long(_) => ctx.typ(expected).has_tag(TypeTag::LONG),
        AnnotationValueProxy::Float(_) => ctx.typ(expected).has_tag(TypeTag::FLOAT),
        AnnotationValueProxy::Double(_) => ctx.typ(expected).has_tag(TypeTag::DOUBLE),
        AnnotationValueProxy::String(_) => ctx
            .typ(expected)
            .has_tag(TypeTag::CLASS)
            && ctx
                .typ(expected)
                .tsym
                .map(|tsym| ctx.sym(tsym).flat_name().to_java_string() == JavaString::from("java.lang.String"))
                .unwrap_or(false),
        AnnotationValueProxy::Class(_) => ctx.typ(expected).has_tag(TypeTag::CLASS),
        AnnotationValueProxy::Enum { .. } => {
            ctx.typ(expected).has_tag(TypeTag::CLASS)
                && ctx
                    .typ(expected)
                    .tsym
                    .map(|tsym| (ctx.sym(tsym).flags_field & flags::ENUM as i64) != 0)
                    .unwrap_or(false)
        }
        AnnotationValueProxy::Annotation(_) => {
            ctx.typ(expected).has_tag(TypeTag::CLASS)
                && ctx
                    .typ(expected)
                    .tsym
                    .map(|tsym| (ctx.sym(tsym).flags_field & flags::ANNOTATION as i64) != 0)
                    .unwrap_or(false)
        }
        AnnotationValueProxy::Array(values) => match &ctx.typ(expected).kind {
            TypeKindData::ArrayType {
                elemtype: Some(elemtype),
                ..
            } => values
                .iter()
                .all(|value| value_matches(ctx, value, *elemtype)),
            _ => false,
        },
        AnnotationValueProxy::Exception(_) => true,
    }
}

/// Convert an Attribute.Constant value to the dynamic proxy return form.
fn annotation_constant_value(value: &ConstantValue) -> AnnotationValueProxy {
    match value {
        ConstantValue::Boolean(value) => AnnotationValueProxy::Boolean(*value),
        ConstantValue::Byte(value) => AnnotationValueProxy::Byte(*value),
        ConstantValue::Character(value) => AnnotationValueProxy::Character(*value),
        ConstantValue::Short(value) => AnnotationValueProxy::Short(*value),
        ConstantValue::Integer(value) => AnnotationValueProxy::Integer(*value),
        ConstantValue::Long(value) => AnnotationValueProxy::Long(*value),
        ConstantValue::Float(value) => AnnotationValueProxy::Float(*value),
        ConstantValue::Double(value) => AnnotationValueProxy::Double(*value),
        ConstantValue::String(value) => AnnotationValueProxy::String(value.clone()),
    }
}

/// The binary (flat) name of a class type, as rendered by the upstream
/// MirroredTypeException proxies (TypeMirror.toString()).
fn type_name_of(ctx: &CompilerContext, ty: TypeId) -> JavaString {
    // Java: MirroredTypeExceptionProxy renders TypeMirror.toString() (the
    // qualified type spelling, including primitive arrays).
    JavaString::from(crate::code::type_::type_to_string(ctx, ty).as_str())
}
