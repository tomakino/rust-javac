// Origin: com/sun/tools/javac/code/Attribute.java @ 982d53c (jdk-17.0.19-ga)
// PORTING_RULES §19 gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P3 semantic services (Enter/Attr/Resolve/Check or ClassReader; see docs/PLAN.md).
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: getFirstEmbeddedTC (Attribute.java:197)
// - NOT PORTED: getAnnotationType (Attribute.java:286)
// End of §19 gap registration.

//! An annotation value.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: the abstract Attribute class is represented by
//! Attribute { ty: Option<TypeId>, kind: AttributeKind } (one variant per
//! subclass); nested Attribute references use AttributeId (Attribute is an
//! arena value with identity semantics, ADR 0001); the Java Object constant
//! value is mapped to the ConstValue enum (same value domain as
//! tree::LiteralValue; the unification is registered in PORTING_RULES
//! section 9); Symbol references are Option<SymbolId>;
//! List<Pair<MethodSymbol, Attribute>> keeps the upstream Pair.
//!
//! NOT PORTED IN THIS STEP (see docs/STATUS.md; no placeholder logic):
//! - TypeAnnotationPosition is not ported yet: Compound.position and all the
//!   position methods (getPosition/isContainerTypeCompound/tryFixPosition/
//!   hasUnknownPosition) and Array.getPosition are skipped;
//! - the Types-dependent Class.makeClassType constructor (types.boxedClass/
//!   types.erasure) is skipped;
//! - toString()/getValue()/accept(AnnotationValueVisitor) and
//!   Compound.member/getElemPair/getElementValues: they need the Symbol name
//!   printing/arena access or the javax.lang.model API layer;
//! - Constants.format/Constants.decode (Constants is not ported yet).

use crate::ids::{AttributeId, SymbolId, TypeId};
use crate::util::java_string::JavaString;
use crate::util::list::List;
use crate::util::pair::Pair;

/// The constant value of an annotation element (Java: Java Object value).
///
/// PORTING_RULES: Java Object constant values map to this enum (registered
/// section 9; the same value domain as tree/jc_tree.rs LiteralValue).
#[derive(Clone, Debug, PartialEq)]
pub enum ConstValue {
    Integer(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    /// Java Character (UTF-16 code unit).
    Character(u16),
    Boolean(bool),
    String(JavaString),
}

/// Java: public abstract class Attribute implements AnnotationValue
pub struct Attribute {
    /// The type of the annotation element.
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
            AttributeKind::Compound { .. } => visitor.visit_compound(self),
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
            AttributeKind::Compound { synthesized, .. } => *synthesized,
            _ => false,
        }
    }

    /// Java: Compound.setSynthesized(boolean synthesized)
    pub fn set_synthesized(&mut self, synthesized: bool) {
        match &mut self.kind {
            AttributeKind::Compound { synthesized: slot, .. } => *slot = synthesized,
            _ => panic!("ClassCastException: not a compound attribute"),
        }
    }
}

/// The node-specific fields of the Attribute subclasses.
///
/// PORTING_RULES: one variant per upstream concrete class; TypeCompound is
/// represented by the Compound variant (the upstream TypeCompound only changes
/// the position, which is not ported yet).
#[allow(clippy::large_enum_variant)]
pub enum AttributeKind {
    /// Java: Attribute.Constant (the value for an annotation element of
    /// primitive type or String).
    Constant {
        /// The constant value (Java: Object value, see ConstValue).
        value: ConstValue,
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
        /// The attributes values, as pairs (each pair contains a reference to
        /// the accessing method in the attribute interface and the value).
        values: List<Pair<SymbolId, AttributeId>>,
        /// Whether this compound attribute is synthesized.
        synthesized: bool,
    },
    /// Java: Attribute.Array (the value for an annotation element of an array
    /// type).
    Array {
        /// The array values (Java: public final Attribute[] values; the
        /// attributes are arena values, see AttributeId).
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

    #[test]
    fn constant_and_array_attributes() {
        let constant = Attribute::new(
            Some(TypeId::new(1)),
            AttributeKind::Constant {
                value: ConstValue::Integer(42),
            },
        );
        assert_eq!(constant.ty, Some(TypeId::new(1)));
        match &constant.kind {
            AttributeKind::Constant { value } => assert_eq!(*value, ConstValue::Integer(42)),
            _ => panic!("expected a constant attribute"),
        }
        assert_eq!(visit(&constant), vec!["constant"]);

        let array = Attribute::new(
            Some(TypeId::new(2)),
            AttributeKind::Array {
                values: vec![AttributeId::new(0)],
            },
        );
        match &array.kind {
            AttributeKind::Array { values } => assert_eq!(values.len(), 1),
            _ => panic!("expected an array attribute"),
        }
        assert_eq!(visit(&array), vec!["array"]);
    }

    #[test]
    fn compound_attribute_and_synthesized_flag() {
        let mut compound = Attribute::new(
            Some(TypeId::new(1)),
            AttributeKind::Compound {
                values: List::nil(),
                synthesized: false,
            },
        );
        assert!(!compound.is_synthesized());
        compound.set_synthesized(true);
        assert!(compound.is_synthesized());
        assert_eq!(visit(&compound), vec!["compound"]);
    }

    #[test]
    fn class_enum_and_error_attributes() {
        let class = Attribute::new(
            Some(TypeId::new(1)),
            AttributeKind::Class {
                class_type: Some(TypeId::new(2)),
            },
        );
        assert_eq!(visit(&class), vec!["class"]);
        assert!(!class.is_synthesized());

        let enum_attribute = Attribute::new(
            Some(TypeId::new(1)),
            AttributeKind::Enum {
                value: Some(SymbolId::new(3)),
            },
        );
        assert_eq!(visit(&enum_attribute), vec!["enum"]);

        let error = Attribute::new(Some(TypeId::new(1)), AttributeKind::Error);
        assert_eq!(visit(&error), vec!["error"]);

        let unresolved = Attribute::new(
            Some(TypeId::new(1)),
            AttributeKind::UnresolvedClass {
                class_type: Some(TypeId::new(4)),
            },
        );
        assert_eq!(visit(&unresolved), vec!["error"]);
    }

    #[test]
    fn const_value_variants_and_retention_policy() {
        let values = [
            ConstValue::Integer(1),
            ConstValue::Long(2_i64),
            ConstValue::Float(1.5),
            ConstValue::Double(2.5),
            ConstValue::Character(0x41),
            ConstValue::Boolean(true),
            ConstValue::String(JavaString::from("s")),
        ];
        assert_eq!(values.len(), 7);
        assert_eq!(RetentionPolicy::SOURCE, RetentionPolicy::SOURCE);
        assert_ne!(RetentionPolicy::SOURCE, RetentionPolicy::RUNTIME);
        assert_eq!(
            [RetentionPolicy::SOURCE, RetentionPolicy::CLASS, RetentionPolicy::RUNTIME].len(),
            3
        );
    }

    #[test]
    #[should_panic(expected = "not a compound attribute")]
    fn set_synthesized_panics_for_non_compound() {
        let mut error = Attribute::new(Some(TypeId::new(1)), AttributeKind::Error);
        error.set_synthesized(true);
    }
}
