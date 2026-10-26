// Origin: com/sun/tools/javac/code/Attribute.java @ 982d53c (jdk-17.0.19-ga)
// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P3 semantic services (Attr/Annotatees).
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: getValue() (Attribute.java:57 and the overrides) — Java Object
// return type (the ported value domain is util::constants::ConstantValue, see
// below); the javax.lang.model AnnotationValue API layer.
// - NOT PORTED: accept(AnnotationValueVisitor, P) (Attribute.java:62 and the
// overrides) — javax.lang.model visitor API layer.
// - NOT PORTED: Compound.toString's type part / Enum.toString (Attribute.java:258/
// 366) call Type.toString / Symbol.toString, which are not ported yet; the
// ported toString uses explicit unimplemented helpers (registered: Types/
// Symbol printing).
// End of Gap registration.

//! An annotation value.
//!
//! <p><b>This is NOT part of any supported API.</b>
//!
//! PORTING_RULES: the abstract Attribute class is represented by
//! Attribute { ty: Option<TypeId>, kind: AttributeKind } (one variant per
//! subclass); nested Attribute references use AttributeId (Attribute is an
//! arena value with identity semantics, ADR 0001); the Java Object constant
//! value is the ported util::constants::ConstantValue enum (the same domain as
//! the Java primitive wrapper types + String); Symbol references are
//! Option<SymbolId>; List<Pair<MethodSymbol, Attribute>> keeps the upstream Pair;
//! TypeAnnotationPosition is the ported code::type_annotation_position type and
//! the position methods take the CompilerContext explicitly (the Java methods
//! read this.values through the arena; ADR 0001).

use crate::code::type_annotation_position::TypeAnnotationPosition;
use crate::compiler_context::CompilerContext;
use crate::ids::{AttributeId, SymbolId, TypeId};
use crate::util::constants::ConstantValue;

/// PORTING_RULES adapter: the previous ConstValue enum is unified into
/// util::constants::ConstantValue (the same Java Object value domain;
/// registered in section 9).
pub type ConstValue = ConstantValue;
use crate::util::list::List;
use crate::util::pair::Pair;

/// Java: public abstract class Attribute implements AnnotationValue
pub struct Attribute {
    /// The type of the annotation element (Java: public Type type).
    pub ty: Option<TypeId>,
    /// The node-specific fields (one variant per upstream concrete subclass).
    pub kind: AttributeKind,
}

impl Attribute {
    pub fn new(ty: Option<TypeId>, kind: AttributeKind) -> Attribute {
        Attribute { ty, kind }
    }

    /// Java: public abstract void accept(Visitor v)
    pub fn accept(&self, visitor: &mut dyn Visitor) {
        match &self.kind {
            AttributeKind::Constant { .. } => visitor.visit_constant(self),
            AttributeKind::Class { .. } => visitor.visit_class(self),
            AttributeKind::Compound { .. } | AttributeKind::TypeCompound { .. } => {
                visitor.visit_compound(self)
            }
            AttributeKind::Array { .. } => visitor.visit_array(self),
            AttributeKind::Enum { .. } => visitor.visit_enum(self),
            AttributeKind::Error | AttributeKind::UnresolvedClass { .. } => {
                visitor.visit_error(self)
            }
        }
    }

    /// Java: public boolean isSynthesized() (overridden by Compound)
    pub fn is_synthesized(&self) -> bool {
        match &self.kind {
            AttributeKind::Compound { synthesized, .. }
            | AttributeKind::TypeCompound { synthesized, .. } => *synthesized,
            _ => false,
        }
    }

    /// Java: Compound.setSynthesized(boolean synthesized)
    pub fn set_synthesized(&mut self, synthesized: bool) {
        match &mut self.kind {
            AttributeKind::Compound {
                synthesized: slot, ..
            }
            | AttributeKind::TypeCompound {
                synthesized: slot, ..
            } => *slot = synthesized,
            _ => panic!("ClassCastException: not a compound attribute"),
        }
    }

    /// Java: public TypeAnnotationPosition getPosition() — the base class
    /// returns null; Compound returns its position; Array returns the position
    /// of its first value.
    pub fn get_position(&self, ctx: &CompilerContext) -> Option<TypeAnnotationPosition> {
        match &self.kind {
            AttributeKind::Compound { position, .. }
            | AttributeKind::TypeCompound { position, .. } => position.clone(),
            AttributeKind::Array { values } => values
                .first()
                .and_then(|value| ctx.attr(*value).get_position(ctx)),
            _ => None,
        }
    }

    /// Java: Compound.isContainerTypeCompound()
    pub fn is_container_type_compound(&self, ctx: &CompilerContext) -> bool {
        if self.is_synthesized() && self.values_length() == 1 {
            return self.get_first_embedded_tc(ctx).is_some();
        }
        false
    }

    fn values_length(&self) -> usize {
        match &self.kind {
            AttributeKind::Compound { values, .. }
            | AttributeKind::TypeCompound { values, .. } => values.length() as usize,
            _ => 0,
        }
    }

    fn values(&self) -> List<Pair<SymbolId, AttributeId>> {
        match &self.kind {
            AttributeKind::Compound { values, .. }
            | AttributeKind::TypeCompound { values, .. } => values.clone(),
            _ => List::nil(),
        }
    }

    /// Java: private Compound getFirstEmbeddedTC()
    fn get_first_embedded_tc(&self, ctx: &CompilerContext) -> Option<AttributeId> {
        let values = self.values();
        if values.length() != 1 {
            return None;
        }
        let value = values.head();
        let (method, attribute) = (value.fst, value.snd);
        let method_name = ctx.sym(method).name.clone();
        if !method_name.content_equals(&javac_strings::JavaString::from("value")) {
            return None;
        }
        match &ctx.attr(attribute).kind {
            AttributeKind::Array { values: array } => {
                if array.is_empty() {
                    return None;
                }
                match &ctx.attr(array[0]).kind {
                    AttributeKind::TypeCompound { .. } => Some(array[0]),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Java: Compound.hasUnknownPosition()
    pub fn has_unknown_position(&self) -> bool {
        let position = match &self.kind {
            AttributeKind::Compound { position, .. }
            | AttributeKind::TypeCompound { position, .. } => {
                position.as_ref().expect("compound position")
            }
            _ => panic!("ClassCastException: not a compound attribute"),
        };
        position.type_ == crate::code::target_type::TargetType::UNKNOWN
    }

    /// Java: Compound.tryFixPosition()
    pub fn try_fix_position(&mut self, ctx: &CompilerContext) -> bool {
        if !self.is_container_type_compound(ctx) {
            return false;
        }
        let from = self.get_first_embedded_tc(ctx);
        let Some(from) = from else {
            return false;
        };
        let from_position = match &ctx.attr(from).kind {
            AttributeKind::TypeCompound {
                position: Some(position),
                ..
            } => position.clone(),
            _ => return false,
        };
        if from_position.type_ != crate::code::target_type::TargetType::UNKNOWN {
            if let AttributeKind::Compound {
                position: slot, ..
            }
            | AttributeKind::TypeCompound {
                position: slot, ..
            } = &mut self.kind
            {
                *slot = Some(from_position);
            }
            return true;
        }
        false
    }

    /// Java: Compound.member(Name member)
    pub fn member(&self, ctx: &CompilerContext, member: &crate::util::name::Name) -> Option<AttributeId> {
        self.get_elem_pair(ctx, member).map(|pair| pair.1)
    }

    /// Java: private Compound.getElemPair(Name member)
    pub fn get_elem_pair(
        &self,
        ctx: &CompilerContext,
        member: &crate::util::name::Name,
    ) -> Option<(SymbolId, AttributeId)> {
        let values = self.values();
        let mut current = values;
        while current.non_empty() {
            let pair = current.head();
            let name = ctx.sym(pair.fst).name.clone();
            if name.content_equals(&member.to_java_string()) {
                return Some((pair.fst, pair.snd));
            }
            current = current.tail();
        }
        None
    }

    /// Java: Compound.getElementValues() — a LinkedHashMap<MethodSymbol,
    /// Attribute> in the upstream (insertion order); the port returns the
    /// ordered pairs (registered adapter).
    pub fn get_element_values(
        &self,
        _ctx: &CompilerContext,
    ) -> Vec<(SymbolId, AttributeId)> {
        let values = self.values();
        let mut result = Vec::new();
        let mut current = values;
        while current.non_empty() {
            let pair = current.head();
            result.push((pair.fst, pair.snd));
            current = current.tail();
        }
        result
    }

    /// Java: Compound.getAnnotationType() — (DeclaredType) type.
    pub fn get_annotation_type(&self) -> Option<TypeId> {
        match &self.kind {
            AttributeKind::Compound { .. } | AttributeKind::TypeCompound { .. } => self.ty,
            _ => panic!("ClassCastException: not a compound attribute"),
        }
    }

    /// Java: public String toString() — Constant/Array/Compound/Enum/Error.
    /// The Type/Symbol display parts are not ported yet (registered helpers).
    pub fn to_string(&self, ctx: &CompilerContext) -> String {
        match &self.kind {
            AttributeKind::Constant { value } => {
                let ty = self.ty.expect("constant attribute type");
                crate::util::constants::format(value.clone(), ctx.typ(ty))
            }
            AttributeKind::Class { class_type } => {
                let class_type = class_type.expect("class attribute type");
                format!("{}.class", type_display(ctx, class_type))
            }
            AttributeKind::Compound { .. } | AttributeKind::TypeCompound { .. } => {
                let mut result = String::from("@");
                let ty = self.ty.expect("compound attribute type");
                result.push_str(&type_display(ctx, ty));
                let values = self.values();
                let length = values.length();
                if length > 0 {
                    result.push('(');
                    let mut first = true;
                    let mut current = values;
                    while current.non_empty() {
                        let pair = current.head();
                        if !first {
                            result.push_str(", ");
                        }
                        first = false;
                        let name = ctx.sym(pair.fst).name.clone();
                        if length > 1
                            || !name.content_equals(&javac_strings::JavaString::from("value"))
                        {
                            result.push_str(&name.to_java_string().to_string_lossy());
                            result.push('=');
                        }
                        result.push_str(&ctx.attr(pair.snd).to_string(ctx));
                        current = current.tail();
                    }
                    result.push(')');
                }
                result
            }
            AttributeKind::Array { values } => {
                let mut result = String::from("{");
                let mut first = true;
                for value in values {
                    if !first {
                        result.push_str(", ");
                    }
                    first = false;
                    result.push_str(&ctx.attr(*value).to_string(ctx));
                }
                result.push('}');
                result
            }
            AttributeKind::Enum { value } => {
                let value = value.expect("enum attribute value");
                symbol_display(ctx, value)
            }
            AttributeKind::Error | AttributeKind::UnresolvedClass { .. } => String::from("<error>"),
        }
    }

    /// Java: Attribute.Class(Types types, Type type) — the Types-dependent
    /// constructor (makeClassType).
    pub fn new_class(
        ctx: &mut CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        types: &crate::code::types::Types,
        class_type: TypeId,
    ) -> Attribute {
        let arg = if ctx.typ(class_type).is_primitive() {
            let boxed = types.boxed_class(ctx, syms, class_type);
            ctx.sym(boxed).ty.expect("boxed class type")
        } else {
            types.erasure(ctx, syms, class_type)
        };
        let class_ty = syms.class_type;
        let enclosing = ctx.typ(class_ty).get_enclosing_type().expect("classType enclosing");
        let tsym = ctx.typ(class_ty).tsym.expect("classType tsym");
        let ty = ctx.alloc_type(crate::code::type_::Type::new(
            crate::code::type_tag::TypeTag::CLASS,
            Some(tsym),
            crate::code::type_::TypeKindData::ClassType {
                outer_field: Some(enclosing),
                typarams_field: Some(List::of(arg)),
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
            },
        ));
        Attribute::new(
            Some(ty),
            AttributeKind::Class {
                class_type: Some(class_type),
            },
        )
    }
}

/// Java: Type.toString()/Symbol.toString() — the display layers are not ported
/// yet (Types/Symbol printing); explicit unimplemented helpers.
fn type_display(_ctx: &CompilerContext, _ty: TypeId) -> String {
    unimplemented!("TODO(Types/Symbol printing batch): Type.toString")
}

fn symbol_display(_ctx: &CompilerContext, _symbol: SymbolId) -> String {
    unimplemented!("TODO(Symbol printing batch): Symbol.toString")
}

/// The node-specific fields of the Attribute subclasses.
///
/// PORTING_RULES: one variant per upstream concrete class (TypeCompound is a
/// distinct upstream class, hence a distinct variant).
#[allow(clippy::large_enum_variant)]
pub enum AttributeKind {
    /// Java: Attribute.Constant (the value for an annotation element of
    /// primitive type or String).
    Constant {
        /// The constant value (Java: Object value).
        value: ConstantValue,
    },
    /// Java: Attribute.Class (the value for an annotation element of type
    /// java.lang.Class, represented as a ClassSymbol).
    Class {
        /// The class type (Java: public final Type classType).
        class_type: Option<TypeId>,
    },
    /// Java: Attribute.Compound (a compound annotation element value, the type
    /// of which is an attribute interface).
    Compound {
        /// The attributes values, as pairs.
        values: List<Pair<SymbolId, AttributeId>>,
        /// Java: public TypeAnnotationPosition position
        position: Option<TypeAnnotationPosition>,
        /// Java: private boolean synthesized (Compound only).
        synthesized: bool,
    },
    /// Java: public static class TypeCompound extends Compound
    TypeCompound {
        /// The attributes values, as pairs.
        values: List<Pair<SymbolId, AttributeId>>,
        /// Java: public TypeAnnotationPosition position
        position: Option<TypeAnnotationPosition>,
        /// Java: private boolean synthesized (inherited from Compound).
        synthesized: bool,
    },
    /// Java: Attribute.Array (the value for an annotation element of an array
    /// type).
    Array {
        /// The array values (Java: public final Attribute[] values).
        values: Vec<AttributeId>,
    },
    /// Java: Attribute.Enum (the value for an annotation element of an enum
    /// type).
    Enum {
        /// The enum constant symbol (Java: public VarSymbol value).
        value: Option<SymbolId>,
    },
    /// Java: Attribute.Error
    Error,
    /// Java: Attribute.UnresolvedClass
    UnresolvedClass {
        /// The unresolved class type.
        class_type: Option<TypeId>,
    },
}

impl AttributeKind {
    /// Java: Attribute.TypeCompound(Compound compound,
    /// TypeAnnotationPosition position)
    pub fn type_compound_from(
        compound: &Attribute,
        position: TypeAnnotationPosition,
    ) -> AttributeKind {
        match &compound.kind {
            AttributeKind::Compound {
                values,
                synthesized,
                ..
            } => AttributeKind::TypeCompound {
                values: values.clone(),
                position: Some(position),
                synthesized: *synthesized,
            },
            _ => panic!("ClassCastException: not a compound attribute"),
        }
    }
}

/// Java: public static interface Visitor (dynamic dispatch on the attribute
/// kind).
pub trait Visitor {
    fn visit_constant(&mut self, value: &Attribute);
    fn visit_class(&mut self, clazz: &Attribute);
    fn visit_compound(&mut self, compound: &Attribute);
    fn visit_array(&mut self, array: &Attribute);
    fn visit_enum(&mut self, e: &Attribute);
    fn visit_error(&mut self, e: &Attribute);
}

/// Java: public static enum RetentionPolicy (a mirror of
/// java.lang.annotation.RetentionPolicy).
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RetentionPolicy {
    SOURCE,
    CLASS,
    RUNTIME,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::source::Source;
    use crate::code::symtab::Symtab;
    use crate::code::target_type::TargetType;
    use crate::code::type_annotation_position::TypeAnnotationPosition;
    use crate::code::types::Types;
    use crate::jvm::target::Target;
    use crate::util::name::Name;
    use crate::util::names::Names;
    use std::rc::Rc;

    struct RecordingVisitor {
        visited: Vec<&'static str>,
    }

    impl Visitor for RecordingVisitor {
        fn visit_constant(&mut self, _value: &Attribute) {
            self.visited.push("constant");
        }
        fn visit_class(&mut self, _clazz: &Attribute) {
            self.visited.push("class");
        }
        fn visit_compound(&mut self, _compound: &Attribute) {
            self.visited.push("compound");
        }
        fn visit_array(&mut self, _array: &Attribute) {
            self.visited.push("array");
        }
        fn visit_enum(&mut self, _e: &Attribute) {
            self.visited.push("enum");
        }
        fn visit_error(&mut self, _e: &Attribute) {
            self.visited.push("error");
        }
    }

    fn visit(attribute: &Attribute) -> Vec<&'static str> {
        let mut visitor = RecordingVisitor { visited: Vec::new() };
        attribute.accept(&mut visitor);
        visitor.visited
    }

    fn harness() -> (CompilerContext, Symtab, Types, Rc<Names>) {
        let names = Rc::new(Names::new(false));
        let mut ctx = CompilerContext::new();
        let syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, Source::JDK17);
        let types = Types::with_names(Rc::clone(&names));
        (ctx, syms, types, names)
    }

    fn name(names: &Names, text: &str) -> Name {
        names.from_string(&javac_strings::JavaString::from(text))
    }

    #[test]
    fn constant_attribute_to_string_and_value_domain() {
        let (mut ctx, syms, _types, _names) = harness();
        let constant = Attribute::new(
            Some(syms.int_type),
            AttributeKind::Constant {
                value: ConstantValue::Integer(42),
            },
        );
        assert_eq!(constant.to_string(&ctx), "42");
        assert_eq!(ctx.type_const_value(syms.int_type), None);
        let constant_type = ctx.type_const_type(syms.int_type, ConstantValue::Integer(7));
        assert_eq!(
            ctx.type_const_value(constant_type),
            Some(&ConstantValue::Integer(7))
        );
        assert_eq!(ctx.type_base_type(constant_type), syms.int_type);
        assert_eq!(ctx.typ(constant_type).get_tag(), crate::code::type_tag::TypeTag::INT);
    }

    #[test]
    fn compound_member_and_element_values() {
        let (mut ctx, syms, _types, names) = harness();
        let method = ctx.alloc_symbol(crate::code::symbol::Symbol::new_method_symbol(
            0,
            name(&names, "m"),
            syms.int_type,
            syms.java_base,
        ));
        let constant = ctx.alloc_attribute(Attribute::new(
            Some(syms.int_type),
            AttributeKind::Constant {
                value: ConstantValue::Integer(1),
            },
        ));
        let compound = Attribute::new(
            Some(syms.int_type),
            AttributeKind::Compound {
                values: List::of(Pair::of(method, constant)),
                position: None,
                synthesized: false,
            },
        );
        let member = compound.member(&ctx, &name(&names, "m"));
        assert_eq!(member, Some(constant));
        assert_eq!(compound.get_elem_pair(&ctx, &name(&names, "m")), Some((method, constant)));
        assert_eq!(compound.get_element_values(&ctx), vec![(method, constant)]);
        assert_eq!(compound.get_annotation_type(), Some(syms.int_type));
        assert!(!compound.is_synthesized());
    }

    #[test]
    fn type_compound_position_and_container_fixing() {
        let (mut ctx, syms, _types, names) = harness();
        let method = ctx.alloc_symbol(crate::code::symbol::Symbol::new_method_symbol(
            0,
            name(&names, "value"),
            syms.int_type,
            syms.java_base,
        ));
        let position = TypeAnnotationPosition::field(
            crate::code::type_annotation_position::TypeAnnotationPosition::empty_path(),
            5,
        );
        let type_compound_attr = ctx.alloc_attribute(Attribute::new(
            Some(syms.object_type),
            AttributeKind::TypeCompound {
                values: List::nil(),
                position: Some(position.clone()),
                synthesized: false,
            },
        ));
        let array = ctx.alloc_attribute(Attribute::new(
            Some(syms.object_type),
            AttributeKind::Array {
                values: vec![type_compound_attr],
            },
        ));
        let mut container = Attribute::new(
            Some(syms.object_type),
            AttributeKind::Compound {
                values: List::of(Pair::of(method, array)),
                position: Some(TypeAnnotationPosition::unknown()),
                synthesized: true,
            },
        );
        assert!(container.is_container_type_compound(&ctx));
        assert!(container.has_unknown_position());
        assert!(container.try_fix_position(&ctx));
        assert!(!container.has_unknown_position());
        let fixed = container.get_position(&ctx).expect("fixed position");
        assert_eq!(fixed.type_, TargetType::FIELD);
        assert_eq!(fixed.pos, 5);
    }

    #[test]
    fn class_attribute_make_class_type_boxes_primitives() {
        let (mut ctx, mut syms, types, _names) = harness();
        let int_type = syms.int_type;
        let attribute = Attribute::new_class(&mut ctx, &mut syms, &types, int_type);
        let class_type = match &attribute.kind {
            AttributeKind::Class { class_type } => class_type.expect("class type"),
            _ => panic!("class attribute expected"),
        };
        assert_eq!(class_type, syms.int_type);
        let attribute_type = attribute.ty.expect("attribute type");
        let arguments = ctx.typ(attribute_type).get_type_arguments();
        assert_eq!(arguments.length(), 1);
        let boxed = types.boxed_class(&mut ctx, &mut syms, int_type);
        assert_eq!(
            ctx.typ(*arguments.head()).tsym,
            ctx.sym(boxed).ty.and_then(|ty| ctx.typ(ty).tsym)
        );
    }

    #[test]
    fn visitor_dispatch_and_error_attributes() {
        let error = Attribute::new(Some(TypeId::new(1)), AttributeKind::Error);
        assert_eq!(visit(&error), vec!["error"]);
        let unresolved = Attribute::new(
            Some(TypeId::new(1)),
            AttributeKind::UnresolvedClass {
                class_type: Some(TypeId::new(2)),
            },
        );
        assert_eq!(visit(&unresolved), vec!["error"]);
        assert_eq!(error.to_string(&ctx_placeholder()), "<error>");
    }

    fn ctx_placeholder() -> CompilerContext {
        CompilerContext::new()
    }
}

/// Java: the Object returned by Attribute.getValue() (the AnnotationValue
/// domain). PORTING_RULES Porting rules adapter: the union of the upstream return types
/// (Type / List<Attribute> / VarSymbol / String / this) is represented as an
/// enum so callers can dispatch without any/object downcasts.
#[derive(Clone, Debug)]
pub enum AttributeValue {
    /// Java: Attribute.Constant.getValue() -> Object (the constant value)
    Constant(ConstantValue),
    /// Java: Attribute.Class.getValue() -> Type
    Class(TypeId),
    /// Java: Attribute.Array.getValue() -> List<Attribute> (arena ids, Porting rules)
    Array(Vec<AttributeId>),
    /// Java: Attribute.Enum.getValue() -> VarSymbol
    Enum(SymbolId),
    /// Java: Attribute.Error.getValue() -> String ("<error>")
    Error(String),
    /// Java: Attribute.Compound.getValue() -> this (the compound attribute id)
    Compound(AttributeId),
}

impl Attribute {
    /// Java: public Constant(Type type, Object value) (Attribute.java:76)
    pub fn new_constant(ty: Option<TypeId>, value: ConstantValue) -> Attribute {
        Attribute::new(ty, AttributeKind::Constant { value })
    }

    /// Java: public Compound(Type type, List<Pair<MethodSymbol,Attribute>>
    /// values, TypeAnnotationPosition position) (Attribute.java:166)
    pub fn new_compound_at(
        ty: Option<TypeId>,
        values: List<crate::util::pair::Pair<SymbolId, AttributeId>>,
        position: Option<TypeAnnotationPosition>,
    ) -> Attribute {
        Attribute::new(
            ty,
            AttributeKind::Compound {
                values,
                position,
                synthesized: false,
            },
        )
    }

    /// Java: public Compound(Type type, List<Pair<MethodSymbol,Attribute>>
    /// values) (Attribute.java:174) — the position is null.
    pub fn new_compound(
        ty: Option<TypeId>,
        values: List<crate::util::pair::Pair<SymbolId, AttributeId>>,
    ) -> Attribute {
        Attribute::new_compound_at(ty, values, None)
    }

    /// Java: public TypeCompound(Type type, List<Pair<MethodSymbol,Attribute>>
    /// values, TypeAnnotationPosition position) (Attribute.java:322-326)
    pub fn new_type_compound(
        ty: Option<TypeId>,
        values: List<crate::util::pair::Pair<SymbolId, AttributeId>>,
        position: Option<TypeAnnotationPosition>,
    ) -> Attribute {
        Attribute::new(
            ty,
            AttributeKind::TypeCompound {
                values,
                position,
                synthesized: false,
            },
        )
    }

    /// Java: public Array(Type type, Attribute[] values) (Attribute.java:316)
    pub fn new_array(ty: Option<TypeId>, values: Vec<AttributeId>) -> Attribute {
        Attribute::new(ty, AttributeKind::Array { values })
    }

    /// Java: public Enum(Type type, VarSymbol value) (Attribute.java:363)
    pub fn new_enum(ty: Option<TypeId>, value: SymbolId) -> Attribute {
        Attribute::new(
            ty,
            AttributeKind::Enum {
                value: Some(value),
            },
        )
    }

    /// Java: public Error(Type type) (Attribute.java:383)
    pub fn new_error(ty: Option<TypeId>) -> Attribute {
        Attribute::new(ty, AttributeKind::Error)
    }

    /// Java: public UnresolvedClass(Type type, Type classType)
    /// (Attribute.java:403-406)
    pub fn new_unresolved_class(
        ty: Option<TypeId>,
        class_type: Option<TypeId>,
    ) -> Attribute {
        Attribute::new(ty, AttributeKind::UnresolvedClass { class_type })
    }

    /// Java: Attribute.getValue() — the per-subclass Object result.
    /// AttributeId is passed because Compound.getValue() returns this.
    pub fn get_value(&self, self_id: AttributeId) -> AttributeValue {
        match &self.kind {
            AttributeKind::Constant { value } => AttributeValue::Constant(value.clone()),
            AttributeKind::Class { class_type } => AttributeValue::Class(
                class_type.expect("Attribute.Class.getValue: classType"),
            ),
            AttributeKind::Compound { values, .. }
            | AttributeKind::TypeCompound { values, .. } => {
                let _ = values;
                AttributeValue::Compound(self_id)
            }
            AttributeKind::Array { values } => AttributeValue::Array(values.clone()),
            AttributeKind::Enum { value } => AttributeValue::Enum(
                value.expect("Attribute.Enum.getValue: VarSymbol"),
            ),
            AttributeKind::Error => AttributeValue::Error(String::from("<error>")),
            AttributeKind::UnresolvedClass { .. } => {
                AttributeValue::Error(String::from("<error>"))
            }
        }
    }
}
