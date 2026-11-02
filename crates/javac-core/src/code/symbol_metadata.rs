//
// Adapters (registered):
// - the upstream sentinel values DECL_NOT_STARTED/DECL_IN_PROGRESS are
// List.of(null) instances compared by identity; the port models the three
// states with the DeclState enum;
// - the Symbol back-reference is the SymbolId arena id;
// - Attribute.Compound/TypeCompound lists are List<AttributeId> (the Attribute
// is an arena value with identity semantics; ADR 0001).
use crate::code::attribute::{Attribute, AttributeKind};
use crate::code::flags;
use crate::code::kinds::Kind;
use crate::ids::{AttributeId, SymbolId};
use crate::util::list::List;

/// The state of the declaration attributes list: the upstream sentinels
/// (DECL_NOT_STARTED / DECL_IN_PROGRESS) plus the real list.
#[derive(Clone)]
pub enum DeclState {
    /// Java: private static final List<Attribute.Compound> DECL_NOT_STARTED
    NotStarted,
    /// Java: private static final List<Attribute.Compound> DECL_IN_PROGRESS
    InProgress,
    /// A real attribute list.
    Attributes(List<AttributeId>),
}

impl DeclState {
    fn list(&self) -> List<AttributeId> {
        match self {
            DeclState::NotStarted | DeclState::InProgress => List::nil(),
            DeclState::Attributes(list) => list.clone(),
        }
    }
}

/// Java: public class SymbolMetadata
#[derive(Clone)]
pub struct SymbolMetadata {
    /// Java: private final Symbol sym
    sym: SymbolId,
    /// Java: private List<Attribute.Compound> attributes = DECL_NOT_STARTED
    attributes: DeclState,
    /// Java: private List<Attribute.TypeCompound> type_attributes = List.nil()
    type_attributes: List<AttributeId>,
    /// Java: private List<Attribute.TypeCompound> init_type_attributes = List.nil()
    init_type_attributes: List<AttributeId>,
    /// Java: private List<Attribute.TypeCompound> clinit_type_attributes = List.nil()
    clinit_type_attributes: List<AttributeId>,
}

impl SymbolMetadata {
    /// Java: public SymbolMetadata(Symbol sym)
    pub fn new(sym: SymbolId) -> SymbolMetadata {
        SymbolMetadata {
            sym,
            attributes: DeclState::NotStarted,
            type_attributes: List::nil(),
            init_type_attributes: List::nil(),
            clinit_type_attributes: List::nil(),
        }
    }

    /// Returns the arena id of the owning symbol (test accessor for the
    /// rebinding adapter; not part of the upstream API).
    #[cfg(test)]
    pub fn owner(&self) -> SymbolId {
        self.sym
    }

    /// Rebinds the metadata to the arena id of its owning symbol. The ported
    /// SymbolMetadata carries the SymbolId (the Java back-reference); when a
    /// Symbol clone enters the symbol arena through the unique allocation entry
    /// point, CompilerContext::alloc_symbol updates the id and rebinds the
    /// copied metadata (registered adapter: Java has no clone path here).
    pub fn rebind(&mut self, sym: SymbolId) {
        self.sym = sym;
    }

    /// Java: public List<Attribute.Compound> getDeclarationAttributes()
    pub fn get_declaration_attributes(&self) -> List<AttributeId> {
        self.filter_decl_sentinels()
    }

    /// Java: public List<Attribute.TypeCompound> getTypeAttributes()
    pub fn get_type_attributes(&self) -> List<AttributeId> {
        self.type_attributes.clone()
    }

    /// Java: public List<Attribute.TypeCompound> getInitTypeAttributes()
    pub fn get_init_type_attributes(&self) -> List<AttributeId> {
        self.init_type_attributes.clone()
    }

    /// Java: public List<Attribute.TypeCompound> getClassInitTypeAttributes()
    pub fn get_class_init_type_attributes(&self) -> List<AttributeId> {
        self.clinit_type_attributes.clone()
    }

    /// Java: public void setDeclarationAttributes(List<Attribute.Compound> a)
    pub fn set_declaration_attributes(&mut self, attributes: List<AttributeId>) {
        // Java: Assert.check(pendingCompletion() || !isStarted()); null check.
        assert!(self.pending_completion() || !self.is_started());
        self.attributes = DeclState::Attributes(attributes);
    }

    /// Java: public void setTypeAttributes(List<Attribute.TypeCompound> a)
    pub fn set_type_attributes(&mut self, attributes: List<AttributeId>) {
        self.type_attributes = attributes;
    }

    /// Java: public void setInitTypeAttributes(List<Attribute.TypeCompound> a)
    pub fn set_init_type_attributes(&mut self, attributes: List<AttributeId>) {
        self.init_type_attributes = attributes;
    }

    /// Java: public void setClassInitTypeAttributes(List<Attribute.TypeCompound> a)
    pub fn set_class_init_type_attributes(&mut self, attributes: List<AttributeId>) {
        self.clinit_type_attributes = attributes;
    }

    /// Java: public void setAttributes(SymbolMetadata other)
    pub fn set_attributes(
        &mut self,
        ctx: &crate::compiler_context::CompilerContext,
        other: &SymbolMetadata,
    ) {
        self.set_declaration_attributes(other.get_declaration_attributes());
        let sym = self.sym;
        if (ctx.sym(sym).flags_field & flags::BRIDGE) != 0 {
            assert_eq!(ctx.sym(other.sym).kind, Kind::MTH);
            let mut type_attributes: List<AttributeId> = List::nil();
            let mut current = other.get_type_attributes();
            while current.non_empty() {
                let type_compound = *current.head();
                // Carry over only contractual type annotations: i.e nothing
                // interior to method body.
                if !type_compound_position_is_local(ctx, type_compound) {
                    type_attributes = type_attributes.append(type_compound);
                }
                current = current.tail();
            }
            self.set_type_attributes(type_attributes);
        } else {
            self.set_type_attributes(other.get_type_attributes());
        }
        if ctx.sym(sym).kind == Kind::TYP {
            self.set_init_type_attributes(other.get_init_type_attributes());
            self.set_class_init_type_attributes(other.get_class_init_type_attributes());
        }
    }

    /// Java: public SymbolMetadata reset()
    pub fn reset(&mut self) -> &mut SymbolMetadata {
        self.attributes = DeclState::InProgress;
        self
    }

    /// Java: public boolean isEmpty()
    pub fn is_empty(&self) -> bool {
        !self.is_started() || self.pending_completion() || self.filter_decl_sentinels().is_empty()
    }

    /// Java: public boolean isTypesEmpty()
    pub fn is_types_empty(&self) -> bool {
        self.type_attributes.is_empty()
    }

    /// Java: public boolean pendingCompletion()
    pub fn pending_completion(&self) -> bool {
        matches!(self.attributes, DeclState::InProgress)
    }

    /// Java: public SymbolMetadata append(List<Attribute.Compound> l)
    pub fn append(&mut self, list: List<AttributeId>) -> &mut SymbolMetadata {
        let mut attributes = self.filter_decl_sentinels();
        if !list.is_empty() {
            if attributes.is_empty() {
                attributes = list;
            } else {
                attributes = attributes.append_list(&list);
            }
        }
        self.attributes = DeclState::Attributes(attributes);
        self
    }

    /// Java: public SymbolMetadata appendUniqueTypes(List<Attribute.TypeCompound> l)
    pub fn append_unique_types(&mut self, list: List<AttributeId>) -> &mut SymbolMetadata {
        if !list.is_empty() {
            if self.type_attributes.is_empty() {
                self.type_attributes = list;
            } else {
                let mut current = list;
                while current.non_empty() {
                    let type_compound = *current.head();
                    if !self
                        .type_attributes
                        .iter()
                        .any(|existing| existing == type_compound)
                    {
                        self.type_attributes = self.type_attributes.append(type_compound);
                    }
                    current = current.tail();
                }
            }
        }
        self
    }

    /// Java: public SymbolMetadata appendInitTypeAttributes(List<Attribute.TypeCompound> l)
    pub fn append_init_type_attributes(&mut self, list: List<AttributeId>) -> &mut SymbolMetadata {
        if !list.is_empty() {
            if self.init_type_attributes.is_empty() {
                self.init_type_attributes = list;
            } else {
                self.init_type_attributes = self.init_type_attributes.append_list(&list);
            }
        }
        self
    }

    /// Java: public SymbolMetadata appendClassInitTypeAttributes(List<Attribute.TypeCompound> l)
    pub fn append_class_init_type_attributes(
        &mut self,
        list: List<AttributeId>,
    ) -> &mut SymbolMetadata {
        if !list.is_empty() {
            if self.clinit_type_attributes.is_empty() {
                self.clinit_type_attributes = list;
            } else {
                self.clinit_type_attributes = self.clinit_type_attributes.append_list(&list);
            }
        }
        self
    }

    /// Java: public SymbolMetadata prepend(List<Attribute.Compound> l)
    pub fn prepend(&mut self, list: List<AttributeId>) -> &mut SymbolMetadata {
        let mut attributes = self.filter_decl_sentinels();
        if !list.is_empty() {
            if attributes.is_empty() {
                attributes = list;
            } else {
                attributes = attributes.prepend_list(&list);
            }
        }
        self.attributes = DeclState::Attributes(attributes);
        self
    }

    /// Java: private List<Attribute.Compound> filterDeclSentinels(List<Attribute.Compound> a)
    fn filter_decl_sentinels(&self) -> List<AttributeId> {
        self.attributes.list()
    }

    /// Java: private boolean isStarted()
    fn is_started(&self) -> bool {
        !matches!(self.attributes, DeclState::NotStarted)
    }

    /// Java: public void removeDeclarationMetadata(Attribute.Compound compound)
    pub fn remove_declaration_metadata(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        compound: AttributeId,
    ) {
        let attributes = self.filter_decl_sentinels();
        if attributes.iter().any(|existing| existing == compound) {
            self.attributes =
                DeclState::Attributes(remove_from_compound_list(ctx, attributes.clone(), compound));
        } else {
            // slow path: the attributes list might contain annotation containers
            let mut current = attributes.clone();
            while current.non_empty() {
                let attr_compound = *current.head();
                let is_container = ctx.attr(attr_compound).is_synthesized()
                    && !attribute_values(ctx, attr_compound).is_empty();
                if !is_container {
                    current = current.tail();
                    continue;
                }
                let values = attribute_values(ctx, attr_compound);
                let value = values.head();
                let method_name = ctx.sym(value.fst).name.clone();
                if !method_name.content_equals(&javac_strings::JavaString::from("value")) {
                    current = current.tail();
                    continue;
                }
                let AttributeKind::Array { values: array } = &ctx.attr(value.snd).kind else {
                    current = current.tail();
                    continue;
                };
                if array.is_empty() {
                    current = current.tail();
                    continue;
                }
                let is_compound = matches!(
                    &ctx.attr(array[0]).kind,
                    AttributeKind::Compound { .. } | AttributeKind::TypeCompound { .. }
                );
                if is_compound && ctx.attr(array[0]).ty == ctx.attr(compound).ty {
                    self.attributes = DeclState::Attributes(remove_from_compound_list(
                        ctx,
                        attributes.clone(),
                        attr_compound,
                    ));
                    return;
                }
                current = current.tail();
            }
        }
    }
}

fn attribute_values(
    ctx: &crate::compiler_context::CompilerContext,
    attribute: AttributeId,
) -> List<crate::util::pair::Pair<SymbolId, AttributeId>> {
    match &ctx.attr(attribute).kind {
        AttributeKind::Compound { values, .. } | AttributeKind::TypeCompound { values, .. } => {
            values.clone()
        }
        _ => List::nil(),
    }
}

fn type_compound_position_is_local(
    ctx: &crate::compiler_context::CompilerContext,
    type_compound: AttributeId,
) -> bool {
    match &ctx.attr(type_compound).kind {
        AttributeKind::TypeCompound {
            position: Some(position),
            ..
        } => target_type_is_local(position.type_),
        _ => false,
    }
}

fn target_type_is_local(target: crate::code::target_type::TargetType) -> bool {
    // Java: TypeAnnotationPosition.isLocal() delegates to TargetType.isLocal(),
    // which marks every target that lives inside a method body (CAST, NEW,
    // INSTANCEOF, the reference/type-argument forms, ...) as local. Listing
    // only a subset left CAST annotations on bridge methods, adding a stray
    // CAST entry to the bridge's code attribute
    // (SymbolMetadata.java:144-160 filters them out for BRIDGE methods).
    target.is_local()
}

fn remove_from_compound_list(
    _ctx: &mut crate::compiler_context::CompilerContext,
    list: List<AttributeId>,
    compound: AttributeId,
) -> List<AttributeId> {
    let mut result: List<AttributeId> = List::nil();
    let mut current = list;
    while current.non_empty() {
        let candidate = *current.head();
        if candidate != compound {
            result = result.append(candidate);
        }
        current = current.tail();
    }
    result
}

/// Java: SymbolMetadata — the helper used by the Attribute test layer.
pub fn attribute_type(attribute: &Attribute) -> Option<crate::ids::TypeId> {
    attribute.ty
}
