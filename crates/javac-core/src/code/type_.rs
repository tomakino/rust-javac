// Origin: com/sun/tools/javac/code/Type.java @ 982d53c (jdk-17.0.19-ga)

//! This class represents Java types. The class itself defines the behavior of
//! the following types:
//! <pre>
//! base types (tags: BYTE, CHAR, SHORT, INT, LONG, FLOAT, DOUBLE, BOOLEAN),
//! type void (tag: VOID),
//! the bottom type (tag: BOT),
//! the missing type (tag: NONE).
//! </pre>
//! <p>The behavior of the following types is defined in subclasses, which are
//! all static inner classes of this class:
//! <pre>
//! class types (tag: CLASS, class: ClassType),
//! array types (tag: ARRAY, class: ArrayType),
//! method types (tag: METHOD, class: MethodType),
//! package types (tag: PACKAGE, class: PackageType),
//! type variables (tag: TYPEVAR, class: TypeVar),
//! type arguments (tag: WILDCARD, class: WildcardType),
//! generic method types (tag: FORALL, class: ForAll),
//! the error type (tag: ERROR, class: ErrorType).
//! </pre>
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: ADR 0001 (arena + IDs) defines the representation:
//! - a Type is a value in the type arena (TypeId is the arena index);
//! - nested types are Option<TypeId> / List<TypeId>; the type symbol is
//!   Option<SymbolId> (Symbol is not ported yet, so only its arena ID is used);
//! - the upstream Java subclasses map 1:1 to TypeKindData variants; the
//!   abstract subclass DelegatedType has no variant: its fields (qtype) are
//!   merged into the concrete variant (ForAll);
//! - the per-node tag (JCPrimitiveType.tag / DelegatedType.tag / getTag) is the
//!   shared Type.tag field;
//! - the Java constructors take a TypeMetadata argument; TypeMetadata (and the
//!   annotation-based metadata machinery) is not ported yet, so the adapter
//!   constructor Type::new takes the already mapped fields; this is registered
//!   in PORTING_RULES section 9.
//!
//! NOT PORTED IN THIS STEP (see docs/STATUS.md; do not add placeholder logic):
//! - TypeMetadata/Attribute annotations: the metadata field, cloneWithMetadata,
//!   stripMetadata, annotatedType, isAnnotated, getAnnotationMirrors,
//!   getAnnotation, needsStripping, typeNoMetadata;
//! - Symbol-dependent behavior: flags (isInterface/isFinal), complete(),
//!   apiComplete, tsym.type comparisons (ClassType.isErroneous/isRaw,
//!   getTypeArguments' lazy complete), Symbol.kind (ClassType/ErrorType.getKind);
//! - Types/Infer/Printer-dependent behavior: getKind (needs Symbol.kind for
//!   ClassType/ErrorType), constValue (needs the Java constant value mapping),
//!   isFalse/isTrue, constType, baseType, getModelType, getModelTypes,
//!   getOriginalType (the "this" result needs the arena ID), map/TypeMapping,
//!   StructuralTypeMapping, contains/containsAny/filter, toString, stringValue,
//!   equals/hashCode, poolTag/poolKey, accept(Type.Visitor)/accept(TypeVisitor),
//!   UndetVar (needs InferenceContext), the static noType/recoveryType/stuckType
//!   instances and moreInfo;
//! - recursive accessors that need the type arena (getTypeArguments,
//!   getEnclosingType, getParameterTypes, getReturnType, getReceiverType,
//!   getThrownTypes, allparams) are not ported yet: they need the arena/ID
//!   parameter; getComponents/getExplicitComponents are direct field reads and
//!   ARE ported;
//! - ArrayType.makeVarargs/isVarargs (the upstream anonymous subclass override
//!   has no direct Rust mapping; needs a design decision);
//! - Type.Visitor interface.

/// Java: Type.UndetVar.Kind (NORMAL/CAPTURED/THROWS).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UndetVarKind {
    NORMAL,
    CAPTURED,
    THROWS,
}

/// Java: Type.UndetVar.InferenceBound (inference variable bound kinds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InferenceBound {
    /// lower bounds
    LOWER,
    /// equality constraints
    EQ,
    /// upper bounds
    UPPER,
}

impl InferenceBound {
    /// Java: public InferenceBound complement()
    pub fn complement(self) -> InferenceBound {
        match self {
            InferenceBound::LOWER => InferenceBound::UPPER,
            InferenceBound::EQ => InferenceBound::EQ,
            InferenceBound::UPPER => InferenceBound::LOWER,
        }
    }

    /// Java: public boolean lessThan(InferenceBound that)
    pub fn less_than(self, that: InferenceBound) -> bool {
        if that == self {
            return false;
        }
        match that {
            InferenceBound::UPPER => true,
            InferenceBound::LOWER => false,
            InferenceBound::EQ => self != InferenceBound::UPPER,
        }
    }

    /// Java: EnumMap ordinal order (the declaration order).
    pub fn ordinal(self) -> usize {
        match self {
            InferenceBound::LOWER => 0,
            InferenceBound::EQ => 1,
            InferenceBound::UPPER => 2,
        }
    }

    /// Java: InferenceBound.values()
    pub fn values() -> [InferenceBound; 3] {
        [
            InferenceBound::LOWER,
            InferenceBound::EQ,
            InferenceBound::UPPER,
        ]
    }
}

use crate::code::bound_kind::BoundKind;
use crate::code::type_tag::TypeTag;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::util::constants::ConstantValue;
use crate::util::list::List;

/// This class represents Java types (upstream abstract class Type).
///
/// PORTING_RULES: the shared fields of the upstream Type class are the tag, the
/// type symbol and the node-specific fields (TypeKindData).
#[derive(Clone)]
pub struct Type {
    /// The type tag (upstream: abstract TypeTag getTag(); subclasses store the
    /// tag either in a field or return a constant).
    pub tag: TypeTag,
    /// The defining class / interface / package / type variable.
    pub tsym: Option<SymbolId>,
    /// The node-specific fields (one variant per upstream concrete subclass).
    pub kind: TypeKindData,
    /// Java: the constValue() override of the constant type subclasses
    /// (JCPrimitiveType/ClassType anonymous subclasses created by constType).
    /// PORTING_RULES adapter: a const type is the same Type with a constant
    /// value attached (the upstream anonymous subclasses only override
    /// constValue and baseType; registered in section 9).
    pub const_value: Option<ConstantValue>,
    /// Java: protected final TypeMetadata metadata (TypeMetadata.java:89;
    /// Java null -> None for the default value).
    pub metadata: Option<crate::code::type_metadata::TypeMetadata>,
}

impl Type {
    /// Adapter constructor: the upstream constructors also take TypeMetadata,
    /// which is not ported yet (see the module doc).
    pub fn new(tag: TypeTag, tsym: Option<SymbolId>, kind: TypeKindData) -> Type {
        Type {
            tag,
            tsym,
            kind,
            const_value: None,
            metadata: None,
        }
    }

    /// Java: public TypeMetadata getMetadata() (Type.java:91-93)
    pub fn get_metadata(&self) -> Option<&crate::code::type_metadata::TypeMetadata> {
        self.metadata.as_ref()
    }

    /// Java: public Entry getMetadataOfKind(final Entry.Kind kind)
    /// (Type.java:95-97)
    pub fn get_metadata_of_kind(
        &self,
        kind: crate::code::type_metadata::EntryKind,
    ) -> Option<&crate::code::type_metadata::Entry> {
        self.metadata.as_ref().and_then(|metadata| metadata.get(kind))
    }

    /// Java: protected Type typeNoMetadata() (Type.java:348-350).
    ///
    /// PORTING_RULES Porting rules adapter: the upstream baseType() default returns this
    /// for most subclasses; the port returns a clone without metadata.
    pub fn type_no_metadata(&self) -> Type {
        if self.metadata.is_none() {
            self.clone()
        } else {
            let mut stripped = self.clone();
            stripped.metadata = None;
            stripped
        }
    }

    /// Java: public abstract Type cloneWithMetadata(TypeMetadata metadata)
    /// (Type.java:354-356).
    pub fn clone_with_metadata(
        &self,
        metadata: crate::code::type_metadata::TypeMetadata,
    ) -> Type {
        let mut cloned = self.clone();
        cloned.metadata = Some(metadata);
        cloned
    }

    /// Java: public Type annotatedType(final List<Attribute.TypeCompound> annos)
    /// (Type.java:400-404).
    pub fn annotated_type(&self, annos: &List<crate::ids::AttributeId>) -> Type {
        let base = self
            .metadata
            .clone()
            .unwrap_or_else(crate::code::type_metadata::TypeMetadata::empty);
        let entry = crate::code::type_metadata::Entry::Annotations(
            crate::code::type_metadata::Annotations::new(annos.clone()),
        );
        self.clone_with_metadata(base.combine(entry))
    }

    /// Java: public boolean isAnnotated() (Type.java:406-412)
    pub fn is_annotated(&self) -> bool {
        match self.get_metadata_of_kind(crate::code::type_metadata::EntryKind::ANNOTATIONS) {
            Some(crate::code::type_metadata::Entry::Annotations(annotations)) => {
                !annotations.get_annotations().is_empty()
            }
            None => false,
        }
    }

    /// Java: public List<Attribute.TypeCompound> getAnnotationMirrors()
    /// (Type.java:414-419)
    pub fn get_annotation_mirrors(&self) -> List<crate::ids::AttributeId> {
        match self.get_metadata_of_kind(crate::code::type_metadata::EntryKind::ANNOTATIONS) {
            Some(crate::code::type_metadata::Entry::Annotations(annotations)) => {
                annotations.get_annotations()
            }
            None => List::nil(),
        }
    }

    /// Java: public <A extends Annotation> A getAnnotation(Class<A>
    /// annotationType) (Type.java:423-425): the upstream body returns null.
    pub fn get_annotation(&self) -> Option<crate::ids::AttributeId> {
        None
    }

    /// Java: public <A extends Annotation> A[] getAnnotationsByType(
    /// Class<A> annotationType) (Type.java:428-433): the upstream body
    /// returns an empty array.
    pub fn get_annotations_by_type(&self) -> Vec<crate::ids::AttributeId> {
        Vec::new()
    }

    /// Java: public Type constType(Object constValue) — the base Type rejects
    /// it (AssertionError); JCPrimitiveType/ClassType create constant types,
    /// NullType/ErrorType return this (NullType/ErrorType overrides);
    /// ArrayType/TypeVar/WildcardType/... keep the base behavior
    /// (AssertionError).
    pub fn const_type(&self, value: ConstantValue) -> Type {
        match self.tag {
            TypeTag::BOOLEAN
            | TypeTag::CHAR
            | TypeTag::BYTE
            | TypeTag::SHORT
            | TypeTag::INT
            | TypeTag::LONG
            | TypeTag::FLOAT
            | TypeTag::DOUBLE => {
                let mut constant = self.clone();
                constant.const_value = Some(value);
                constant
            }
            TypeTag::CLASS => {
                let mut constant = self.clone();
                constant.const_value = Some(value);
                constant
            }
            TypeTag::BOT | TypeTag::ERROR => self.clone(),
            _ => panic!("AssertionError: constType on {}", self.tag),
        }
    }

    /// Java: public Object constValue() (default null; the constant type
    /// subclasses override). PORTING_RULES: Java null -> Option::None.
    pub fn const_value(&self) -> Option<&ConstantValue> {
        self.const_value.as_ref()
    }

    /// Java: public abstract TypeTag getTag()
    pub fn get_tag(&self) -> TypeTag {
        self.tag
    }

    /// Checks if the current type tag is equal to the given tag.
    pub fn has_tag(&self, tag: TypeTag) -> bool {
        tag == self.get_tag()
    }

    /// Java: public boolean isFalse() (Type.java:780-786)
    pub fn is_false(&self) -> bool {
        self.tag == TypeTag::BOOLEAN
            && match self.const_value.as_ref() {
                Some(crate::util::constants::ConstantValue::Boolean(value)) => !*value,
                Some(crate::util::constants::ConstantValue::Integer(value)) => *value == 0,
                _ => false,
            }
    }

    /// Java: public boolean isTrue() (Type.java:790-795)
    pub fn is_true(&self) -> bool {
        self.tag == TypeTag::BOOLEAN
            && match self.const_value.as_ref() {
                Some(crate::util::constants::ConstantValue::Boolean(value)) => *value,
                Some(crate::util::constants::ConstantValue::Integer(value)) => *value != 0,
                _ => false,
            }
    }

    /// Java: public boolean isNumeric()
    pub fn is_numeric(&self) -> bool {
        match &self.kind {
            TypeKindData::JCPrimitiveType => self.tag != TypeTag::BOOLEAN,
            _ => false,
        }
    }

    /// Java: public boolean isIntegral()
    pub fn is_integral(&self) -> bool {
        match &self.kind {
            TypeKindData::JCPrimitiveType => matches!(
                self.tag,
                TypeTag::CHAR | TypeTag::BYTE | TypeTag::SHORT | TypeTag::INT | TypeTag::LONG
            ),
            _ => false,
        }
    }

    /// Java: public boolean isPrimitive()
    pub fn is_primitive(&self) -> bool {
        matches!(&self.kind, TypeKindData::JCPrimitiveType)
    }

    /// Java: public boolean isPrimitiveOrVoid()
    pub fn is_primitive_or_void(&self) -> bool {
        matches!(
            &self.kind,
            TypeKindData::JCPrimitiveType | TypeKindData::JCVoidType
        )
    }

    /// Java: public boolean isReference()
    pub fn is_reference(&self) -> bool {
        matches!(
            &self.kind,
            TypeKindData::WildcardType { .. }
                | TypeKindData::ClassType { .. }
                | TypeKindData::ErasedClassType { .. }
                | TypeKindData::UnionClassType { .. }
                | TypeKindData::IntersectionClassType { .. }
                | TypeKindData::ArrayType { .. }
                | TypeKindData::TypeVar { .. }
                | TypeKindData::CapturedType { .. }
                | TypeKindData::ErrorType { .. }
        )
    }

    /// Java: public boolean isNullOrReference()
    pub fn is_null_or_reference(&self) -> bool {
        self.is_reference() || matches!(&self.kind, TypeKindData::BottomType)
    }

    /// Java: public boolean isPartial()
    pub fn is_partial(&self) -> bool {
        matches!(
            &self.kind,
            TypeKindData::ErrorType { .. }
                | TypeKindData::UnknownType
                | TypeKindData::UndetVar { .. }
                | TypeKindData::PartiallyInferredMethodType { .. }
        )
    }

    /// Java: public boolean isIntersection()
    pub fn is_intersection(&self) -> bool {
        matches!(&self.kind, TypeKindData::IntersectionClassType { .. })
    }

    /// Java: public boolean isUnion()
    pub fn is_union(&self) -> bool {
        matches!(&self.kind, TypeKindData::UnionClassType { .. })
    }

    /// Java: WildcardType.isSuperBound()
    pub fn is_super_bound(&self) -> bool {
        match &self.kind {
            TypeKindData::WildcardType { kind, .. } => {
                *kind == BoundKind::SUPER || *kind == BoundKind::UNBOUND
            }
            _ => false,
        }
    }

    /// Java: WildcardType.isExtendsBound()
    pub fn is_extends_bound(&self) -> bool {
        match &self.kind {
            TypeKindData::WildcardType { kind, .. } => {
                *kind == BoundKind::EXTENDS || *kind == BoundKind::UNBOUND
            }
            _ => false,
        }
    }

    /// Java: WildcardType.isUnbound()
    pub fn is_unbound(&self) -> bool {
        match &self.kind {
            TypeKindData::WildcardType { kind, .. } => *kind == BoundKind::UNBOUND,
            _ => false,
        }
    }

    /// Java: public TypeSymbol asElement() (MethodType overrides it to null).
    pub fn as_element(&self) -> Option<SymbolId> {
        match &self.kind {
            TypeKindData::MethodType { .. } => None,
            _ => self.tsym,
        }
    }

    /// Java: TypeVar.isCaptured() (overridden by CapturedType/UndetVar).
    pub fn is_captured(&self) -> bool {
        matches!(&self.kind, TypeKindData::CapturedType { .. })
    }

    /// Java: WildcardType.getExtendsBound()
    pub fn get_extends_bound(&self) -> Option<TypeId> {
        match &self.kind {
            TypeKindData::WildcardType { type_, kind, .. } if *kind == BoundKind::EXTENDS => *type_,
            _ => None,
        }
    }

    /// Java: WildcardType.getSuperBound()
    pub fn get_super_bound(&self) -> Option<TypeId> {
        match &self.kind {
            TypeKindData::WildcardType { type_, kind, .. } if *kind == BoundKind::SUPER => *type_,
            _ => None,
        }
    }

    /// Java: Type.getUpperBound() (TypeVar/CapturedType; the DelegatedType
    /// override is not ported yet because it needs the arena).
    pub fn get_upper_bound(&self) -> Option<TypeId> {
        match &self.kind {
            TypeKindData::TypeVar { bound, .. } | TypeKindData::CapturedType { bound, .. } => *bound,
            _ => None,
        }
    }

    /// Java: Type.getLowerBound() (TypeVar/CapturedType override).
    pub fn get_lower_bound(&self) -> Option<TypeId> {
        match &self.kind {
            TypeKindData::TypeVar { lower, .. } | TypeKindData::CapturedType { lower, .. } => {
                *lower
            }
            _ => None,
        }
    }

    /// Java: TypeVar.setUpperBound(Type bound)
    pub fn set_upper_bound(&mut self, bound: Option<TypeId>) {
        match &mut self.kind {
            TypeKindData::TypeVar { bound: slot, .. }
            | TypeKindData::CapturedType { bound: slot, .. } => *slot = bound,
            _ => panic!("ClassCastException: not a type variable"),
        }
    }

    /// Java: IntersectionClassType.getComponents()
    pub fn get_components(&self) -> List<TypeId> {
        match &self.kind {
            TypeKindData::IntersectionClassType {
                supertype_field,
                interfaces_field,
                ..
            } => {
                let supertype = supertype_field.expect("intersection type has no supertype");
                let interfaces = interfaces_field
                    .clone()
                    .expect("intersection type has no interfaces");
                interfaces.prepend(supertype)
            }
            _ => panic!("ClassCastException: not an intersection type"),
        }
    }

    /// Java: IntersectionClassType.getExplicitComponents()
    pub fn get_explicit_components(&self) -> List<TypeId> {
        match &self.kind {
            TypeKindData::IntersectionClassType {
                all_interfaces,
                interfaces_field,
                ..
            } => {
                if *all_interfaces {
                    interfaces_field
                        .clone()
                        .expect("intersection type has no interfaces")
                } else {
                    self.get_components()
                }
            }
            _ => panic!("ClassCastException: not an intersection type"),
        }
    }

    /// Java: IntersectionClassType.getBounds()
    pub fn get_bounds(&self) -> List<TypeId> {
        self.get_explicit_components()
    }

    /// Java: UnionClassType.getAlternatives()
    pub fn get_alternatives(&self) -> List<TypeId> {
        match &self.kind {
            TypeKindData::UnionClassType {
                alternatives_field, ..
            } => alternatives_field.clone(),
            _ => panic!("ClassCastException: not a union type"),
        }
    }

    /// Java: UnionClassType.getAlternativeTypes()
    pub fn get_alternative_types(&self) -> List<TypeId> {
        self.get_alternatives()
    }

    /// Java: ClassType.setEnclosingType(Type outer)
    pub fn set_enclosing_type(&mut self, outer: Option<TypeId>) {
        match &mut self.kind {
            TypeKindData::ClassType { outer_field, .. }
            | TypeKindData::ErasedClassType { outer_field, .. }
            | TypeKindData::UnionClassType { outer_field, .. }
            | TypeKindData::IntersectionClassType { outer_field, .. }
            | TypeKindData::ErrorType { outer_field, .. } => *outer_field = outer,
            _ => panic!("ClassCastException: not a class type"),
        }
    }
}

/// The node-specific fields of the Type subclasses.
///
/// PORTING_RULES: one variant per upstream concrete class; inherited fields of
/// ClassType (outer/typarams/allparams/supertype/interfaces/all_interfaces) are
/// repeated in the subclasses that inherit from it (as in the Java class
/// hierarchy). Field names keep the upstream names (snake_case).
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub enum TypeKindData {
    /// Java: Type.JCPrimitiveType (the primitive tag is Type.tag).
    JCPrimitiveType,
    /// Java: Type.WildcardType
    WildcardType {
        /// The bound type (upstream field "type"; mapped to type_ because it is
        /// a Rust keyword).
        type_: Option<TypeId>,
        /// The wildcard bound kind.
        kind: BoundKind,
        /// The bound type variable (bound).
        bound: Option<TypeId>,
        /// Java: WildcardType.isPrintingBound (recursion guard for moreInfo).
        is_printing_bound: std::cell::Cell<bool>,
    },
    /// Java: Type.ClassType
    ClassType {
        /// The enclosing type of this type (outer_field).
        outer_field: Option<TypeId>,
        /// The type parameters of this type (to be set once class is loaded).
        typarams_field: Option<List<TypeId>>,
        /// A cache variable for the type parameters of this type.
        allparams_field: Option<List<TypeId>>,
        /// The supertype of this class (to be set once class is loaded).
        supertype_field: Option<TypeId>,
        /// The interfaces of this class (to be set once class is loaded).
        interfaces_field: Option<List<TypeId>>,
        /// All the interfaces of this class, including missing ones.
        all_interfaces_field: Option<List<TypeId>>,
    },
    /// Java: Type.ErasedClassType
    ErasedClassType {
        /// The enclosing type of this type (outer_field).
        outer_field: Option<TypeId>,
        /// The type parameters of this type.
        typarams_field: Option<List<TypeId>>,
        /// A cache variable for the type parameters of this type.
        allparams_field: Option<List<TypeId>>,
        /// The supertype of this class.
        supertype_field: Option<TypeId>,
        /// The interfaces of this class.
        interfaces_field: Option<List<TypeId>>,
        /// All the interfaces of this class, including missing ones.
        all_interfaces_field: Option<List<TypeId>>,
    },
    /// Java: Type.UnionClassType (a clone of a ClassType that knows about the
    /// alternatives of a union type).
    UnionClassType {
        /// The enclosing type of this type.
        outer_field: Option<TypeId>,
        /// The type parameters of this type.
        typarams_field: Option<List<TypeId>>,
        /// A cache variable for the type parameters of this type.
        allparams_field: Option<List<TypeId>>,
        /// The supertype of this class.
        supertype_field: Option<TypeId>,
        /// The interfaces of this class.
        interfaces_field: Option<List<TypeId>>,
        /// All the interfaces of this class, including missing ones.
        all_interfaces_field: Option<List<TypeId>>,
        /// The union alternatives (alternatives_field).
        alternatives_field: List<TypeId>,
    },
    /// Java: Type.IntersectionClassType (a clone of a ClassType that knows about
    /// the bounds of an intersection type).
    IntersectionClassType {
        /// The enclosing type of this type.
        outer_field: Option<TypeId>,
        /// The type parameters of this type.
        typarams_field: Option<List<TypeId>>,
        /// A cache variable for the type parameters of this type.
        allparams_field: Option<List<TypeId>>,
        /// The supertype of this class.
        supertype_field: Option<TypeId>,
        /// The interfaces of this class.
        interfaces_field: Option<List<TypeId>>,
        /// All the interfaces of this class, including missing ones.
        all_interfaces_field: Option<List<TypeId>>,
        /// The intersection components are all interfaces.
        all_interfaces: bool,
    },
    /// Java: Type.DelegatedType subtype used by the JVM verifier tables.
    ///
    /// The wrapped qualified type and the allocation offset identify an object
    /// that has been allocated but not yet constructed; the tag is
    /// UNINITIALIZED_THIS or UNINITIALIZED_OBJECT.
    UninitializedType {
        /// The qualified type of the constructed object.
        qtype: Option<TypeId>,
        /// The bytecode offset of the allocation site (-1 for this).
        offset: i32,
    },
    /// Java: Type.ArrayType
    ArrayType {
        /// The element type.
        elemtype: Option<TypeId>,
        /// Java: ArrayType.isVarargs() — the upstream override lives in the
        /// anonymous subclass returned by makeVarargs (Porting rules: the flag is a field
        /// here).
        varargs: bool,
    },
    /// Java: Type.MethodType
    MethodType {
        /// The argument types.
        argtypes: List<TypeId>,
        /// The return type.
        restype: Option<TypeId>,
        /// The thrown types.
        thrown: List<TypeId>,
        /// The type of the method receiver.
        recvtype: Option<TypeId>,
    },
    /// Java: Type.PackageType
    PackageType,
    /// Java: Type.ModuleType
    ModuleType,
    /// Java: Type.TypeVar
    TypeVar {
        /// The upper bound (_bound); must be nonempty once it is set.
        bound: Option<TypeId>,
        /// The lower bound (normally syms.botType).
        lower: Option<TypeId>,
    },
    /// Java: Type.CapturedType (a captured type variable from a wildcard)
    CapturedType {
        /// The upper bound.
        bound: Option<TypeId>,
        /// The lower bound.
        lower: Option<TypeId>,
        /// The wildcard from which this type was captured.
        wildcard: Option<TypeId>,
    },
    /// Java: Type.ForAll (generic method type; merges DelegatedType.qtype)
    ForAll {
        /// The method type variables.
        tvars: List<TypeId>,
        /// The underlying method type (DelegatedType.qtype).
        qtype: Option<TypeId>,
    },
    /// Java: Type.JCNoType (the missing type, tag NONE). The upstream has one
    /// base class plus three anonymous subclasses with their own toString.
    JCNoType {
        /// The upstream display identity (Type.java:99-121 / 2212-2242).
        kind: JCNoTypeKind,
    },
    /// Java: Type.JCVoidType (tag VOID)
    JCVoidType,
    /// Java: Type.BottomType (the bottom type, tag BOT; package-private class)
    BottomType,
    /// Java: Type.ErrorType
    ErrorType {
        /// The enclosing type of this type.
        outer_field: Option<TypeId>,
        /// The type parameters of this type.
        typarams_field: Option<List<TypeId>>,
        /// A cache variable for the type parameters of this type.
        allparams_field: Option<List<TypeId>>,
        /// The supertype of this class.
        supertype_field: Option<TypeId>,
        /// The interfaces of this class.
        interfaces_field: Option<List<TypeId>>,
        /// All the interfaces of this class, including missing ones.
        all_interfaces_field: Option<List<TypeId>>,
        /// The original type (originalType).
        original_type: Option<TypeId>,
        /// Java: AttrRecover.RecoveryErrorType.candidateSymbol (Porting rules: the upstream
        /// ErrorType subclass is expressed by this field; None for plain error
        /// types).
        candidate_symbol: Option<SymbolId>,
    },
    /// Java: Infer.PartiallyInferredMethodType (a MethodType that can be
    /// checked against several targets; tag METHOD, isPartial=true)
    PartiallyInferredMethodType {
        /// The parameter types (same as the underlying MethodType)
        argtypes: List<TypeId>,
        /// The return type
        restype: Option<TypeId>,
        /// The thrown types
        thrown: List<TypeId>,
        /// The receiver type
        recvtype: Option<TypeId>,
        /// Java: final InferenceContext inferenceContext
        inference_context: std::rc::Rc<crate::comp::infer::InferenceContext>,
        /// Java: Env<AttrContext> env
        env: std::rc::Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        /// Java: final Warner warn (shared mutable state, registered)
        warn: std::rc::Rc<std::cell::RefCell<crate::util::warner::Warner>>,
    },
    /// Java: Type.UnknownType
    UnknownType,
    /// Java: Type.UndetVar (an inference variable; DelegatedType with qtype)
    UndetVar {
        /// The original type variable (DelegatedType.qtype).
        qtype: TypeId,
        /// The inference variable's inferred type (inst; null while uninstantiated).
        inst: Option<TypeId>,
        /// The inference variable's bounds, indexed by InferenceBound ordinal
        /// (upstream EnumMap<InferenceBound, List<Type>>: LOWER/EQ/UPPER).
        bounds: [List<TypeId>; 3],
        /// The number of declared (upper) bounds.
        declared_count: usize,
        /// The inference variable kind (upstream UndetVar.Kind).
        kind: UndetVarKind,
        /// The inference variable's change listener.
        listener: Option<std::rc::Rc<dyn crate::comp::infer::UndetVarListener>>,
        /// The list of incorporation actions (used by the incorporation engine).
        incorporation_actions: std::collections::VecDeque<crate::comp::infer::IncorporationAction>,
    },
    /// Java: DeferredAttr.DeferredType (tag DEFERRED; the speculative
    /// attribution state of the upstream DeferredType fields).
    Deferred {
        /// Java: JCExpression tree
        tree: TreeId,
        /// Java: Env<AttrContext> env
        env: std::rc::Rc<
            crate::comp::env::Env<crate::comp::attr_context::AttrContext>,
        >,
        /// Java: AttrMode mode
        mode: Option<crate::comp::deferred_attr::AttrMode>,
        /// Java: Set<Symbol> notPertinentToApplicability
        not_pertinent: Vec<SymbolId>,
        /// Java: SpeculativeCache speculativeCache
        speculative_cache: std::rc::Rc<
            std::cell::RefCell<crate::comp::deferred_attr::SpeculativeCache>,
        >,
        /// Java: the ArgumentType state when this deferred type is an
        /// ArgumentAttr.ArgumentType (a Java subclass of DeferredType): the
        /// kind, the speculative tree and the ResultInfo-keyed speculative
        /// types. Porting rules registered: Java subclassing is expressed by this optional
        /// payload plus kind dispatch in argument_attr.rs.
        argument: Option<
            std::rc::Rc<
                std::cell::RefCell<crate::comp::argument_attr::ArgumentTypeData>,
            >,
        >,
    },
    /// Java: ClassReader.ProxyType (ClassReader.java:3199-3227): a placeholder
    /// type for module-info annotation values (tag NONE); resolve decodes the
    /// classfile signature name on deproxy.
    ProxyType {
        /// The classfile signature name (constant pool UTF8).
        name: crate::util::name::Name,
    },
}


/// Java: the JCNoType display identity: the base class ("<none>") and the three
/// anonymous subclasses in Type.java:99-121.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JCNoTypeKind {
    /// Java: new JCNoType() (base class; Type.toString prints "<none>").
    Base,
    /// Java: Type.noType (toString "none").
    NoType,
    /// Java: Type.recoveryType (toString "recovery").
    Recovery,
    /// Java: Type.stuckType (toString "stuck").
    Stuck,
}

// ----------------------------------------------------------------------
// Java: Type subclass constructors (P2 ClassReader). TypeMetadata is not
// ported yet (file header); the adapter constructors take mapped fields.
// ----------------------------------------------------------------------
impl Type {
    /// Java: public ClassType(Type outer, List<Type> typarams, TypeSymbol tsym)
    pub fn new_class_type(
        outer: Option<TypeId>,
        typarams: Option<List<TypeId>>,
        tsym: SymbolId,
    ) -> Type {
        Type::new(
            TypeTag::CLASS,
            Some(tsym),
            TypeKindData::ClassType {
                outer_field: outer,
                typarams_field: typarams,
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
            },
        )
    }

    /// Java: public MethodType(List<Type> argtypes, Type restype, List<Type> thrown, TypeSymbol methodClass)
    pub fn new_method_type(
        argtypes: List<TypeId>,
        restype: TypeId,
        thrown: List<TypeId>,
        method_class: SymbolId,
    ) -> Type {
        Type::new(
            TypeTag::METHOD,
            Some(method_class),
            TypeKindData::MethodType {
                argtypes,
                restype: Some(restype),
                thrown,
                recvtype: None,
            },
        )
    }

    /// Java: ArrayType.isVarargs()
    pub fn is_varargs(&self) -> bool {
        match &self.kind {
            TypeKindData::ArrayType { varargs, .. } => *varargs,
            _ => false,
        }
    }

    /// Java: public ArrayType(Type elemtype, TypeSymbol arrayClass)
    pub fn new_array_type(elemtype: TypeId, array_class: SymbolId) -> Type {
        Type::new(
            TypeTag::ARRAY,
            Some(array_class),
            TypeKindData::ArrayType {
                elemtype: Some(elemtype),
                varargs: false,
            },
        )
    }

    /// Java: public TypeVar(TypeSymbol tsym, Type bound, Type lower, TypeMetadata)
    pub fn new_type_var(tsym: SymbolId, bound: Option<TypeId>, lower: TypeId) -> Type {
        Type::new(
            TypeTag::TYPEVAR,
            Some(tsym),
            TypeKindData::TypeVar { bound, lower: Some(lower) },
        )
    }

    /// Java: public ForAll(List<Type> tvars, Type qtype)
    pub fn new_for_all(tvars: List<TypeId>, qtype: TypeId) -> Type {
        Type::new(
            TypeTag::FORALL,
            None,
            TypeKindData::ForAll { tvars, qtype: Some(qtype) },
        )
    }

    /// Java: public WildcardType(Type type, BoundKind kind, TypeSymbol tsym, TypeVar bound)
    pub fn new_wildcard_type(
        type_: TypeId,
        kind: BoundKind,
        tsym: SymbolId,
        bound: Option<TypeId>,
    ) -> Type {
        Type::new(
            TypeTag::WILDCARD,
            Some(tsym),
            TypeKindData::WildcardType { type_: Some(type_), kind, bound, is_printing_bound: std::cell::Cell::new(false), },
        )
    }

    // ------------------------------------------------------------------
    // Java: Type accessor methods needed by ClassReader (P2).
    // ForAll/DelegatedType delegation needs the type arena, so those
    // accessors take CompilerContext (ADR 0001 registered adapter).
    // ------------------------------------------------------------------

    /// Java: public List<Type> getParameterTypes() (base returns List.nil();
    /// MethodType overrides; ForAll delegates to qtype).
    pub fn get_parameter_types(&self, ctx: &crate::compiler_context::CompilerContext) -> List<TypeId> {
        match &self.kind {
            TypeKindData::MethodType { argtypes, .. } => argtypes.clone(),
            TypeKindData::ForAll { qtype: Some(qtype), .. } => {
                ctx.typ(*qtype).get_parameter_types(ctx)
            }
            _ => List::nil(),
        }
    }

    /// Java: public Type getReturnType() (base returns Type.noType -> None;
    /// MethodType overrides; ForAll delegates to qtype).
    pub fn get_return_type(&self, ctx: &crate::compiler_context::CompilerContext) -> Option<TypeId> {
        match &self.kind {
            TypeKindData::MethodType { restype, .. } => *restype,
            TypeKindData::ForAll { qtype: Some(qtype), .. } => ctx.typ(*qtype).get_return_type(ctx),
            _ => None,
        }
    }

    /// Java: public List<Type> getThrownTypes() (base returns List.nil();
    /// MethodType overrides; ForAll delegates to qtype).
    pub fn get_thrown_types(&self, ctx: &crate::compiler_context::CompilerContext) -> List<TypeId> {
        match &self.kind {
            TypeKindData::MethodType { thrown, .. } => thrown.clone(),
            TypeKindData::ForAll { qtype: Some(qtype), .. } => ctx.typ(*qtype).get_thrown_types(ctx),
            _ => List::nil(),
        }
    }

    /// Java: public Type getEnclosingType() (base returns Type.noType -> None;
    /// ClassType & friends return outer_field).
    pub fn get_enclosing_type(&self) -> Option<TypeId> {
        match &self.kind {
            TypeKindData::ClassType { outer_field, .. }
            | TypeKindData::ErasedClassType { outer_field, .. }
            | TypeKindData::UnionClassType { outer_field, .. }
            | TypeKindData::IntersectionClassType { outer_field, .. }
            | TypeKindData::ErrorType { outer_field, .. } => *outer_field,
            _ => None,
        }
    }

    /// Java: public List<Type> getTypeArguments() (ClassType view).
    pub fn get_type_arguments(&self) -> List<TypeId> {
        match &self.kind {
            TypeKindData::ClassType { typarams_field, .. }
            | TypeKindData::ErasedClassType { typarams_field, .. }
            | TypeKindData::UnionClassType { typarams_field, .. }
            | TypeKindData::IntersectionClassType { typarams_field, .. }
            | TypeKindData::ErrorType { typarams_field, .. } => {
                typarams_field.clone().unwrap_or_else(List::nil)
            }
            _ => List::nil(),
        }
    }
}


// Type constructors needed by the Signature/attribute.
impl Type {
    /// Java: public IntersectionClassType(List<Type> bounds, ClassSymbol csym, boolean allInterfaces)
    pub fn new_intersection_class_type(
        bounds: List<TypeId>,
        csym: SymbolId,
        all_interfaces: bool,
    ) -> Type {
        let head = *bounds.head();
        let tail = bounds.tail();
        Type::new(
            TypeTag::CLASS,
            Some(csym),
            TypeKindData::IntersectionClassType {
                outer_field: None,
                typarams_field: Some(List::nil()),
                allparams_field: None,
                supertype_field: Some(head),
                interfaces_field: Some(tail),
                all_interfaces_field: None,
                all_interfaces,
            },
        )
    }
}

// ----------------------------------------------------------------------
// Java: Type.toString()/subclass overrides (the display layer).
// Porting rules: the upstream identity-hash based parts (CapturedType ids,
// anonymous-class moreInfo) use the stable arena index (registered adapter).
// ----------------------------------------------------------------------

use std::sync::atomic::{AtomicBool, Ordering};

use crate::compiler_context::CompilerContext;

/// Java: Type.moreInfo (Type.java:126) — a global display switch.
pub static MORE_INFO: AtomicBool = AtomicBool::new(false);

/// Set the global moreInfo display switch (the upstream field is a public
/// static boolean).
///
/// # Arguments
///   - value: the new switch value.
///
/// # Returns
///   - none.
///
/// # Panics
///   - none.
pub fn set_more_info(value: bool) {
    MORE_INFO.store(value, Ordering::Relaxed);
}

fn more_info() -> bool {
    MORE_INFO.load(Ordering::Relaxed)
}

/// Java: Printer.PRIME (com/sun/tools/javac/code/Printer.java:55).
const PRINTER_PRIME: i64 = 997;

fn stable_type_id(ty: TypeId) -> i64 {
    i64::from(ty.index())
}

fn push_name(sb: &mut String, name: &crate::util::name::Name) {
    sb.push_str(&name.to_java_string().to_string_lossy());
}

/// Java: Type.appendAnnotationsString(StringBuilder[, boolean prefix])
/// (Type.java:447-460).
fn push_annotations(
    ctx: &CompilerContext,
    ty: TypeId,
    sb: &mut String,
    prefix: bool,
) {
    if ctx.typ(ty).is_annotated() {
        if prefix {
            sb.push(' ');
        }
        let mut annotations = ctx.typ(ty).get_annotation_mirrors();
        let mut first = true;
        while annotations.non_empty() {
            if !first {
                sb.push(',');
            }
            first = false;
            let attribute = *annotations.head();
            sb.push_str(&ctx.attr(attribute).to_string(ctx));
            annotations = annotations.tail();
        }
        sb.push(' ');
    }
}

/// Java: Type.toString(List<Type>) (Type.java:484-494).
pub fn type_list_to_string(ctx: &CompilerContext, list: &List<TypeId>) -> String {
    let mut sb = String::new();
    let mut current = list.clone();
    let mut first = true;
    while current.non_empty() {
        if !first {
            sb.push(',');
        }
        first = false;
        let ty = current.head().clone();
        type_to_string_into(ctx, ty, &mut sb);
        current = current.tail();
    }
    sb
}

fn class_type_parts(
    ctx: &CompilerContext,
    ty: TypeId,
) -> (Option<TypeId>, Option<List<TypeId>>) {
    match &ctx.typ(ty).kind {
        TypeKindData::ClassType { outer_field, typarams_field, .. }
        | TypeKindData::ErasedClassType { outer_field, typarams_field, .. }
        | TypeKindData::UnionClassType { outer_field, typarams_field, .. }
        | TypeKindData::IntersectionClassType { outer_field, typarams_field, .. }
        | TypeKindData::ErrorType { outer_field, typarams_field, .. } => {
            (outer_field.clone(), typarams_field.clone())
        }
        _ => panic!("ClassCastException: ClassType expected"),
    }
}

fn class_type_fields(
    ctx: &CompilerContext,
    ty: TypeId,
) -> (Option<TypeId>, List<TypeId>) {
    match &ctx.typ(ty).kind {
        TypeKindData::ClassType { supertype_field, interfaces_field, .. }
        | TypeKindData::ErasedClassType { supertype_field, interfaces_field, .. }
        | TypeKindData::UnionClassType { supertype_field, interfaces_field, .. }
        | TypeKindData::IntersectionClassType { supertype_field, interfaces_field, .. }
        | TypeKindData::ErrorType { supertype_field, interfaces_field, .. } => {
            (supertype_field.clone(), interfaces_field.clone().unwrap_or_else(List::nil))
        }
        _ => panic!("ClassCastException: ClassType expected"),
    }
}

/// Java: ClassType.className(Symbol sym, boolean longform) (Type.java:1051-1079).
fn class_name(ctx: &CompilerContext, ty: TypeId, sym: SymbolId, longform: bool) -> String {
    let symbol = ctx.sym(sym);
    if symbol.name.is_empty() && (symbol.flags_field & crate::code::flags::COMPOUND as i64) != 0 {
        let (supertype, interfaces) = class_type_fields(ctx, ty);
        let mut sb = String::new();
        if let Some(supertype) = supertype {
            sb.push_str(&type_to_string(ctx, supertype));
        }
        let mut current = interfaces;
        while current.non_empty() {
            sb.push('&');
            sb.push_str(&type_to_string(ctx, current.head().clone()));
            current = current.tail();
        }
        sb
    } else if symbol.name.is_empty() {
        let norm_ty = symbol.ty.expect("anonymous class type");
        let (supertype, interfaces) = class_type_fields(ctx, norm_ty);
        let arg = if interfaces.non_empty() {
            Some(*interfaces.head())
        } else {
            supertype
        };
        let arg_text = arg.map(|t| type_to_string(ctx, t)).unwrap_or_default();
        let messages = crate::util::javac_messages::JavacMessages::new(
            crate::util::javac_messages::DEFAULT_BUNDLE_NAME,
        );
        let mut sb = messages
            .get_localized_string("compiler.misc.anonymous.class", &[arg_text]);
        if more_info() {
            sb.push_str(&stable_type_id(ty).to_string());
        }
        sb
    } else if longform {
        ctx.sym(sym)
            .get_qualified_name()
            .to_java_string()
            .to_string_lossy()
            .to_string()
    } else {
        symbol.name.to_java_string().to_string_lossy().to_string()
    }
}

/// Java: Type base toString and the subclass overrides.
fn type_to_string_into(ctx: &CompilerContext, ty: TypeId, sb: &mut String) {
    let t = ctx.typ(ty);
    match &t.kind {
        TypeKindData::JCPrimitiveType
        | TypeKindData::JCVoidType
        | TypeKindData::BottomType
        | TypeKindData::UnknownType
        | TypeKindData::TypeVar { .. } => {
            // Java: Type.toString() (Type.java:465-477).
            push_annotations(ctx, ty, sb, false);
            match t.tsym {
                Some(sym) => push_name(sb, &ctx.sym(sym).name),
                None => sb.push_str("<none>"),
            }
            if more_info() && t.has_tag(crate::code::type_tag::TypeTag::TYPEVAR) {
                sb.push_str(&stable_type_id(ty).to_string());
            }
        }
        TypeKindData::JCNoType { kind: jc_kind } => match jc_kind {
            JCNoTypeKind::Base => {
                push_annotations(ctx, ty, sb, false);
                match t.tsym {
                    Some(sym) => push_name(sb, &ctx.sym(sym).name),
                    None => sb.push_str("<none>"),
                }
            }
            JCNoTypeKind::NoType => sb.push_str("none"),
            JCNoTypeKind::Recovery => sb.push_str("recovery"),
            JCNoTypeKind::Stuck => sb.push_str("stuck"),
        },
        TypeKindData::WildcardType { type_, kind, bound, is_printing_bound } => {
            // Java: WildcardType.toString() (Type.java:904-917).
            push_annotations(ctx, ty, sb, false);
            sb.push_str(&kind.to_string());
            if !matches!(kind, crate::code::bound_kind::BoundKind::UNBOUND)
                && let Some(type_) = type_
            {
                type_to_string_into(ctx, type_.clone(), sb);
            }
            if more_info() && bound.is_some() && !is_printing_bound.get() {
                is_printing_bound.set(true);
                sb.push_str("{:");
                if let Some(bound) = bound
                    && let Some(upper) = ctx.typ(bound.clone()).get_upper_bound()
                {
                    type_to_string_into(ctx, upper, sb);
                }
                sb.push_str(":}");
                is_printing_bound.set(false);
            }
        }
        TypeKindData::ClassType { .. }
        | TypeKindData::ErasedClassType { .. }
        | TypeKindData::UnionClassType { .. }
        | TypeKindData::IntersectionClassType { .. }
        | TypeKindData::ErrorType { .. } => {
            // Java: ClassType.toString() (Type.java:1031-1048).
            let tsym = t.tsym.expect("ClassType.tsym");
            let (outer, typarams) = class_type_parts(ctx, ty);
            let enclosing_is_class = outer
                .map(|outer| ctx.typ(outer).has_tag(crate::code::type_tag::TypeTag::CLASS))
                .unwrap_or(false)
                && ctx
                    .sym(tsym)
                    .owner
                    .map(|owner| ctx.sym(owner).kind == crate::code::kinds::Kind::TYP)
                    .unwrap_or(false);
            if enclosing_is_class {
                type_to_string_into(ctx, outer.expect("enclosing type"), sb);
                sb.push('.');
                push_annotations(ctx, ty, sb, false);
                sb.push_str(&class_name(ctx, ty, tsym, false));
            } else {
                push_annotations(ctx, ty, sb, false);
                sb.push_str(&class_name(ctx, ty, tsym, true));
            }
            let arguments = typarams.unwrap_or_else(List::nil);
            if !arguments.is_empty() {
                sb.push('<');
                sb.push_str(&type_list_to_string(ctx, &arguments));
                sb.push('>');
            }
        }
        TypeKindData::ArrayType { elemtype, .. } => {
            // Java: ArrayType.toString() (Type.java:1340-1357).
            let Some(elemtype) = elemtype else {
                sb.push_str("<none>");
                return;
            };
            let mut root = elemtype.clone();
            while let TypeKindData::ArrayType { elemtype: Some(inner), .. } = &ctx.typ(root).kind {
                root = inner.clone();
            }
            type_to_string_into(ctx, root, sb);
            let mut current = ty;
            loop {
                push_annotations(ctx, current, sb, true);
                sb.push_str("[]");
                match &ctx.typ(current).kind {
                    TypeKindData::ArrayType { elemtype: Some(elemtype), .. }
                        if ctx
                            .typ(elemtype.clone())
                            .has_tag(crate::code::type_tag::TypeTag::ARRAY) => {
                        current = elemtype.clone();
                    }
                    _ => break,
                }
            }
        }
        TypeKindData::MethodType { argtypes, restype, .. } => {
            // Java: MethodType.toString() (Type.java:1474-1482).
            push_annotations(ctx, ty, sb, false);
            sb.push('(');
            sb.push_str(&{ let args = argtypes.clone(); type_list_to_string(ctx, &args) });
            sb.push(')');
            if let Some(restype) = restype {
                type_to_string_into(ctx, restype.clone(), sb);
            }
        }
        TypeKindData::PartiallyInferredMethodType { argtypes, restype, .. } => {
            // The upstream JDK 17 has no such class (registered legacy port);
            // printed like the underlying MethodType.
            sb.push('(');
            sb.push_str(&{ let args = argtypes.clone(); type_list_to_string(ctx, &args) });
            sb.push(')');
            if let Some(restype) = restype {
                type_to_string_into(ctx, restype.clone(), sb);
            }
        }
        TypeKindData::PackageType | TypeKindData::ModuleType => {
            // Java: PackageType/ModuleType.toString() (Type.java:1562/1605).
            push_annotations(ctx, ty, sb, false);
            if let Some(tsym) = t.tsym {
                push_name(sb, &ctx.sym(tsym).get_qualified_name());
            } else {
                sb.push_str("<none>");
            }
        }
        TypeKindData::ForAll { tvars, qtype } => {
            // Java: ForAll.toString() (Type.java:1838-1845).
            push_annotations(ctx, ty, sb, false);
            sb.push('<');
            sb.push_str(&{ let args = tvars.clone(); type_list_to_string(ctx, &args) });
            sb.push('>');
            if let Some(qtype) = qtype {
                type_to_string_into(ctx, qtype.clone(), sb);
            }
        }
        TypeKindData::UndetVar { qtype, inst, .. } => {
            // Java: UndetVar.toString() (Type.java:1991-2000).
            push_annotations(ctx, ty, sb, false);
            if let Some(inst) = inst {
                type_to_string_into(ctx, inst.clone(), sb);
            } else {
                type_to_string_into(ctx, qtype.clone(), sb);
                sb.push('?');
            }
        }
        TypeKindData::CapturedType { wildcard, .. } => {
            // Java: CapturedType.toString() (Type.java:1774-1782); the upstream
            // identity hash is replaced by the stable arena identity.
            push_annotations(ctx, ty, sb, false);
            sb.push_str("capture#");
            let id = (stable_type_id(ty) & 0xFFFF_FFFF) % PRINTER_PRIME;
            sb.push_str(&id.to_string());
            sb.push_str(" of ");
            if let Some(wildcard) = wildcard {
                type_to_string_into(ctx, wildcard.clone(), sb);
            }
        }
        TypeKindData::UninitializedType { qtype, .. } => {
            // Java: UninitializedType extends DelegatedType; DelegatedType.toString
            // (Type.java:1802) prints qtype.
            if let Some(qtype) = qtype {
                type_to_string_into(ctx, qtype.clone(), sb);
            }
        }
        TypeKindData::Deferred { .. } => {
            // Java: DeferredAttr.DeferredType.toString() (DeferredAttr.java:240-243).
            sb.push_str("DeferredType");
        }
        TypeKindData::ProxyType { .. } => {
            // Java: ClassReader.ProxyType.toString() (ClassReader.java:3222-3225).
            sb.push_str("<ProxyType>");
        }
    }
}

/// Java: Type.toString() dispatch: the Java source which the type represents.
///
/// # Arguments
///   - ctx: the compiler context;
///   - ty: the type to display.
///
/// # Returns
///   - the type display string (upstream Type.toString and subclass overrides).
///
/// # Panics
///   - a ClassType has no tsym (upstream invariant).
pub fn type_to_string(ctx: &CompilerContext, ty: TypeId) -> String {
    let mut sb = String::new();
    type_to_string_into(ctx, ty, &mut sb);
    sb
}
#[cfg(test)]
mod tests {

    #[test]
    fn display_layer_matches_upstream_rules() {
        use super::{type_to_string, Type, TypeKindData, JCNoTypeKind};
        use crate::code::bound_kind::BoundKind;
        use crate::code::type_tag::TypeTag;
        use crate::util::list::List;

        let mut ctx = crate::compiler_context::CompilerContext::new();
        let names = std::rc::Rc::new(crate::util::names::Names::new(false));
        let syms = crate::code::symtab::Symtab::new(
            &mut ctx,
            std::rc::Rc::clone(&names),
            crate::jvm::target::Target::JDK1_17,
            crate::code::source::Source::JDK17,
        );
        // Java: ClassType.toString -> getQualifiedName().
        assert_eq!(type_to_string(&ctx, syms.string_type), "java.lang.String");
        // Java: ArrayType.toString -> root component + [].
        let array = ctx.alloc_type(Type::new(
            TypeTag::ARRAY,
            None,
            TypeKindData::ArrayType {
                elemtype: Some(syms.int_type),
                varargs: false,
            },
        ));
        assert_eq!(type_to_string(&ctx, array), "int[]");
        // Java: MethodType.toString -> (argtypes)restype.
        let method = ctx.alloc_type(Type::new(
            TypeTag::METHOD,
            Some(syms.method_class),
            TypeKindData::MethodType {
                argtypes: List::of(syms.int_type),
                restype: Some(syms.void_type),
                thrown: List::nil(),
                recvtype: None,
            },
        ));
        assert_eq!(type_to_string(&ctx, method), "(int)void");
        // Java: Type.noType / recoveryType / stuckType toString.
        let no_type = ctx.alloc_type(Type::new(
            TypeTag::NONE,
            None,
            TypeKindData::JCNoType { kind: JCNoTypeKind::NoType },
        ));
        let recovery = ctx.alloc_type(Type::new(
            TypeTag::NONE,
            None,
            TypeKindData::JCNoType { kind: JCNoTypeKind::Recovery },
        ));
        let stuck = ctx.alloc_type(Type::new(
            TypeTag::NONE,
            None,
            TypeKindData::JCNoType { kind: JCNoTypeKind::Stuck },
        ));
        assert_eq!(type_to_string(&ctx, no_type), "none");
        assert_eq!(type_to_string(&ctx, recovery), "recovery");
        assert_eq!(type_to_string(&ctx, stuck), "stuck");
        // Java: WildcardType.toString (UNBOUND -> "?").
        let wildcard = ctx.alloc_type(Type::new(
            TypeTag::WILDCARD,
            None,
            TypeKindData::WildcardType {
                type_: None,
                kind: BoundKind::UNBOUND,
                bound: None,
                is_printing_bound: std::cell::Cell::new(false),
            },
        ));
        assert_eq!(type_to_string(&ctx, wildcard), "?");
        // Java: Type.appendAnnotationsString + ClassType.toString (the
        // annotated-class path: "<annotations> <class name>" with the
        // annotations printed by Attribute.toString and "," separated).
        let annotation = ctx.alloc_attribute(
            crate::code::attribute::Attribute::new_type_compound(
                Some(syms.string_type),
                List::nil(),
                None,
            ),
        );
        let annotated = Type::new(
            TypeTag::CLASS,
            ctx.typ(syms.string_type).tsym,
            TypeKindData::ClassType {
                outer_field: None,
                typarams_field: None,
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
            },
        )
        .annotated_type(&List::of(annotation));
        let annotated = ctx.alloc_type(annotated);
        assert_eq!(
            type_to_string(&ctx, annotated),
            "@java.lang.String java.lang.String"
        );
        // Java: Symbol.toString (ClassSymbol -> fullname, ModuleSymbol -> name).
        let object_tsym = ctx.typ(syms.object_type).tsym.expect("Object symbol");
        assert_eq!(
            crate::code::symbol::symbol_to_string(&ctx, object_tsym),
            "java.lang.Object"
        );
        assert_eq!(
            crate::code::symbol::symbol_to_string(&ctx, syms.unnamed_module),
            "<unnamed>"
        );
    }

    use super::*;

    fn primitive(tag: TypeTag) -> Type {
        Type::new(tag, None, TypeKindData::JCPrimitiveType)
    }

    fn class_type() -> Type {
        Type::new(
            TypeTag::CLASS,
            None,
            TypeKindData::ClassType {
                outer_field: None,
                typarams_field: None,
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
            },
        )
    }

    fn array_type(elem: TypeId) -> Type {
        Type::new(
            TypeTag::ARRAY,
            None,
            TypeKindData::ArrayType {
                elemtype: Some(elem),
                varargs: false,
            },
        )
    }

    fn type_var(bound: Option<TypeId>, lower: Option<TypeId>) -> Type {
        Type::new(TypeTag::TYPEVAR, None, TypeKindData::TypeVar { bound, lower })
    }

    #[test]
    fn primitive_type_flags_and_tags() {
        let int = primitive(TypeTag::INT);
        assert_eq!(int.get_tag(), TypeTag::INT);
        assert!(int.has_tag(TypeTag::INT));
        assert!(!int.has_tag(TypeTag::LONG));
        assert!(int.is_primitive());
        assert!(int.is_primitive_or_void());
        assert!(int.is_numeric());
        assert!(int.is_integral());
        assert!(!int.is_reference());
        assert!(!int.is_null_or_reference());
        assert!(!int.is_partial());

        let boolean = primitive(TypeTag::BOOLEAN);
        assert!(boolean.is_primitive());
        assert!(!boolean.is_numeric());
        assert!(!boolean.is_integral());

        let double = primitive(TypeTag::DOUBLE);
        assert!(double.is_numeric());
        assert!(!double.is_integral());
    }

    #[test]
    fn reference_types() {
        assert!(class_type().is_reference());
        assert!(class_type().is_null_or_reference());
        assert!(array_type(TypeId::new(1)).is_reference());
        assert!(type_var(None, None).is_reference());
        assert!(
            Type::new(
                TypeTag::TYPEVAR,
                None,
                TypeKindData::CapturedType {
                    bound: None,
                    lower: None,
                    wildcard: None,
                },
            )
            .is_reference()
        );
        assert!(
            Type::new(
                TypeTag::WILDCARD,
                None,
                TypeKindData::WildcardType {
                    type_: None,
                    kind: BoundKind::UNBOUND,
                    bound: None, is_printing_bound: std::cell::Cell::new(false), },
            )
            .is_reference()
        );
        assert!(
            Type::new(
                TypeTag::ERROR,
                None,
                TypeKindData::ErrorType {
                    outer_field: None,
                    typarams_field: None,
                    allparams_field: None,
                    supertype_field: None,
                    interfaces_field: None,
                    all_interfaces_field: None,
                    original_type: None,
                    candidate_symbol: None,
                },
            )
            .is_reference()
        );
    }

    #[test]
    fn void_bottom_and_missing_types() {
        let void = Type::new(TypeTag::VOID, None, TypeKindData::JCVoidType);
        assert!(void.is_primitive_or_void());
        assert!(!void.is_primitive());
        assert!(!void.is_reference());

        let bottom = Type::new(TypeTag::BOT, None, TypeKindData::BottomType);
        assert!(bottom.is_null_or_reference());
        assert!(!bottom.is_reference());

        let no_type = Type::new(
            TypeTag::NONE,
            None,
            TypeKindData::JCNoType { kind: JCNoTypeKind::Base },
        );
        assert!(!no_type.is_reference());
        assert!(!no_type.is_primitive_or_void());
    }

    #[test]
    fn wildcard_bounds() {
        let extends = Type::new(
            TypeTag::WILDCARD,
            None,
            TypeKindData::WildcardType {
                type_: Some(TypeId::new(7)),
                kind: BoundKind::EXTENDS,
                bound: None, is_printing_bound: std::cell::Cell::new(false), },
        );
        assert!(extends.is_extends_bound());
        assert!(!extends.is_super_bound());
        assert!(!extends.is_unbound());
        assert_eq!(extends.get_extends_bound(), Some(TypeId::new(7)));
        assert_eq!(extends.get_super_bound(), None);

        let super_ = Type::new(
            TypeTag::WILDCARD,
            None,
            TypeKindData::WildcardType {
                type_: Some(TypeId::new(8)),
                kind: BoundKind::SUPER,
                bound: None, is_printing_bound: std::cell::Cell::new(false), },
        );
        assert!(super_.is_super_bound());
        // Java: WildcardType.isExtendsBound() is true only for EXTENDS/UNBOUND.
        assert!(!super_.is_extends_bound());
        assert_eq!(super_.get_super_bound(), Some(TypeId::new(8)));
        assert_eq!(super_.get_extends_bound(), None);

        let unbound = Type::new(
            TypeTag::WILDCARD,
            None,
            TypeKindData::WildcardType {
                type_: Some(TypeId::new(9)),
                kind: BoundKind::UNBOUND,
                bound: None, is_printing_bound: std::cell::Cell::new(false), },
        );
        assert!(unbound.is_unbound());
        assert!(unbound.is_super_bound());
        assert!(unbound.is_extends_bound());
    }

    #[test]
    fn type_variable_bounds_and_setters() {
        let mut var = type_var(Some(TypeId::new(1)), Some(TypeId::new(2)));
        assert_eq!(var.get_upper_bound(), Some(TypeId::new(1)));
        assert_eq!(var.get_lower_bound(), Some(TypeId::new(2)));
        assert!(!var.is_captured());
        var.set_upper_bound(Some(TypeId::new(3)));
        assert_eq!(var.get_upper_bound(), Some(TypeId::new(3)));
        var.set_upper_bound(None);
        assert_eq!(var.get_upper_bound(), None);
    }

    #[test]
    fn captured_type() {
        let captured = Type::new(
            TypeTag::TYPEVAR,
            None,
            TypeKindData::CapturedType {
                bound: Some(TypeId::new(1)),
                lower: Some(TypeId::new(2)),
                wildcard: Some(TypeId::new(3)),
            },
        );
        assert!(captured.is_captured());
        assert_eq!(captured.get_upper_bound(), Some(TypeId::new(1)));
        assert_eq!(captured.get_lower_bound(), Some(TypeId::new(2)));
    }

    #[test]
    fn intersection_components() {
        let mut intersection = Type::new(
            TypeTag::CLASS,
            None,
            TypeKindData::IntersectionClassType {
                outer_field: None,
                typarams_field: None,
                allparams_field: None,
                supertype_field: Some(TypeId::new(1)),
                interfaces_field: Some(List::of2(TypeId::new(2), TypeId::new(3))),
                all_interfaces_field: None,
                all_interfaces: false,
            },
        );
        assert!(intersection.is_intersection());
        assert!(!intersection.is_union());
        let components = intersection.get_components();
        assert_eq!(components.length(), 3);
        assert_eq!(*components.head(), TypeId::new(1));
        let explicit = intersection.get_explicit_components();
        assert_eq!(explicit.length(), 3);
        assert_eq!(intersection.get_bounds().length(), 3);

        if let TypeKindData::IntersectionClassType { all_interfaces, .. } =
            &mut intersection.kind
        {
            *all_interfaces = true;
        }
        let explicit = intersection.get_explicit_components();
        assert_eq!(explicit.length(), 2);
        assert_eq!(*explicit.head(), TypeId::new(2));
    }

    #[test]
    fn union_alternatives() {
        let union = Type::new(
            TypeTag::CLASS,
            None,
            TypeKindData::UnionClassType {
                outer_field: None,
                typarams_field: None,
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
                alternatives_field: List::of2(TypeId::new(1), TypeId::new(2)),
            },
        );
        assert!(union.is_union());
        assert!(!union.is_intersection());
        assert_eq!(union.get_alternatives().length(), 2);
        assert_eq!(union.get_alternative_types().length(), 2);
        assert_eq!(*union.get_alternatives().head(), TypeId::new(1));
    }

    #[test]
    fn class_type_fields_and_setters() {
        let mut class = class_type();
        assert_eq!(class.get_tag(), TypeTag::CLASS);
        class.set_enclosing_type(Some(TypeId::new(5)));
        match &class.kind {
            TypeKindData::ClassType { outer_field, .. } => {
                assert_eq!(outer_field.clone(), Some(TypeId::new(5)));
            }
            _ => panic!("expected a class type"),
        }
    }

    #[test]
    fn method_type_fields() {
        let method = Type::new(
            TypeTag::METHOD,
            Some(SymbolId::new(1)),
            TypeKindData::MethodType {
                argtypes: List::of2(TypeId::new(1), TypeId::new(2)),
                restype: Some(TypeId::new(3)),
                thrown: List::of(TypeId::new(4)),
                recvtype: Some(TypeId::new(5)),
            },
        );
        match &method.kind {
            TypeKindData::MethodType {
                argtypes,
                restype,
                thrown,
                recvtype,
            } => {
                assert_eq!(argtypes.length(), 2);
                assert_eq!(restype.clone(), Some(TypeId::new(3)));
                assert_eq!(thrown.length(), 1);
                assert_eq!(*recvtype, Some(TypeId::new(5)));
            }
            _ => panic!("expected a method type"),
        }
        // Java: MethodType.asElement() returns null.
        assert_eq!(method.as_element(), None);
    }

    #[test]
    fn forall_fields() {
        let forall = Type::new(
            TypeTag::FORALL,
            None,
            TypeKindData::ForAll {
                tvars: List::of(TypeId::new(1)),
                qtype: Some(TypeId::new(2)),
            },
        );
        match &forall.kind {
            TypeKindData::ForAll { tvars, qtype } => {
                assert_eq!(tvars.length(), 1);
                assert_eq!(qtype.clone(), Some(TypeId::new(2)));
            }
            _ => panic!("expected a forall type"),
        }
    }

    #[test]
    fn partial_types() {
        let error = Type::new(
            TypeTag::ERROR,
            None,
            TypeKindData::ErrorType {
                outer_field: None,
                typarams_field: None,
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
                original_type: Some(TypeId::new(1)),
                candidate_symbol: None,
            },
        );
        assert!(error.is_partial());
        match &error.kind {
            TypeKindData::ErrorType { original_type, .. } => {
                assert_eq!(*original_type, Some(TypeId::new(1)));
            }
            _ => panic!("expected an error type"),
        }
        let unknown = Type::new(TypeTag::UNKNOWN, None, TypeKindData::UnknownType);
        assert!(unknown.is_partial());
        assert!(!primitive(TypeTag::INT).is_partial());
        assert!(!class_type().is_partial());
    }

    #[test]
    fn as_element_and_tags() {
        let class = Type::new(
            TypeTag::CLASS,
            Some(SymbolId::new(9)),
            TypeKindData::ClassType {
                outer_field: None,
                typarams_field: None,
                allparams_field: None,
                supertype_field: None,
                interfaces_field: None,
                all_interfaces_field: None,
            },
        );
        assert_eq!(class.as_element(), Some(SymbolId::new(9)));
        let array = Type::new(TypeTag::ARRAY, None, TypeKindData::ArrayType {
            elemtype: None,
            varargs: false,
        });
        assert_eq!(array.as_element(), None);
    }

    #[test]
    #[should_panic(expected = "not an intersection type")]
    fn components_panics_for_non_intersection() {
        let _ = class_type().get_components();
    }

    #[test]
    #[should_panic(expected = "not a union type")]
    fn alternatives_panics_for_non_union() {
        let _ = class_type().get_alternatives();
    }

    #[test]
    #[should_panic(expected = "not a type variable")]
    fn set_upper_bound_panics_for_non_type_variable() {
        let mut class = class_type();
        class.set_upper_bound(None);
    }

    #[test]
    #[should_panic(expected = "not a class type")]
    fn set_enclosing_type_panics_for_non_class_type() {
        let mut primitive = primitive(TypeTag::INT);
        primitive.set_enclosing_type(None);
    }
}
