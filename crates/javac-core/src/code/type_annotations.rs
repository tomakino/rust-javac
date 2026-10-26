// Origin: com/sun/tools/javac/code/TypeAnnotations.java @ 982d53c (jdk-17.0.19-ga)

//! Type annotations helper: separates declaration annotations from type
//! annotations, organizes the type annotation positions of a class tree and
//! validates the type annotation applicability rules.
//!
//! PORTING_RULES:
//! * Java class -> Rust struct; instance(Context) is the Java static factory.
//! * the Java TypeAnnotationPositions/TypeAnnotationValidator inner TreeScanners
//!   map to explicit visitor structs in this module (registered section 9).
//! * the annotation metadata access uses the ported Symbol/Type metadata API
//!   (get_annotation_type_metadata/get_annotation_mirrors and the TypeMetadata
//!   entries).

use std::rc::Rc;

use crate::code::attribute::AttributeKind;
use crate::code::symtab::Symtab;
use crate::ids::{AttributeId, SymbolId};
use crate::util::jc_diagnostic::Factory;
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::names::Names;

/// Java: TypeAnnotations.AnnotationType (the annotation target kind).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationType {
    DECLARATION,
    TYPE,
    NONE,
    BOTH,
}

/// Helper class for type annotations (Java: TypeAnnotations).
pub struct TypeAnnotations {
    /// Java: final Names names
    pub names: Rc<Names>,
    /// Java: final Log log
    pub log: Rc<std::cell::RefCell<Log>>,
    /// Java: final Check chk
    pub chk: Rc<std::cell::RefCell<crate::comp::check::Check>>,
    /// Java: final Attr attr (used by validateTypeAnnotationsSignatures).
    pub attr: Rc<std::cell::RefCell<crate::comp::attr::Attr>>,
    /// Java: final Annotate annotate (afterTypes/validate hooks).
    pub annotate: Rc<crate::comp::annotate::Annotate>,
    /// Java: final Types types
    pub types: Rc<crate::code::types::Types>,
    /// Java: final Factory diags
    pub diags: Rc<Factory>,
}

impl TypeAnnotations {
    /// Java: TypeAnnotations(Context context) — the explicit-service constructor.
    ///
    /// # Arguments
    ///   - names/log/chk/attr/annotate/diags: the injected services.
    ///
    /// # Returns
    ///   - the shared TypeAnnotations handle.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: TypeAnnotations.java:105-118.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        names: Rc<Names>,
        log: Rc<std::cell::RefCell<Log>>,
        chk: Rc<std::cell::RefCell<crate::comp::check::Check>>,
        attr: Rc<std::cell::RefCell<crate::comp::attr::Attr>>,
        annotate: Rc<crate::comp::annotate::Annotate>,
        types: Rc<crate::code::types::Types>,
        diags: Rc<Factory>,
    ) -> TypeAnnotations {
        TypeAnnotations {
            names,
            log,
            chk,
            attr,
            annotate,
            types,
            diags,
        }
    }

    /// Java: public List<Attribute> annotationTargets(TypeSymbol tsym)
    /// (TypeAnnotations.java:152-168).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tsym: the annotation type symbol.
    ///
    /// # Returns
    ///   - the @Target target list (None when the metadata has no target or a
    ///     non-array target value).
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: TypeAnnotations.java:152-168.
    pub fn annotation_targets(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        tsym: SymbolId,
    ) -> Option<List<AttributeId>> {
        let mut metadata = ctx.sym(tsym).get_annotation_type_metadata().cloned()?;
        let at_target = metadata.get_target(ctx, syms).ok()??;
        let value = self.compound_member(ctx, at_target, &self.names.value)?;
        let values = match &ctx.attr(value).kind {
            AttributeKind::Array { values } => values.clone(),
            _ => return None,
        };
        let mut result: List<AttributeId> = List::nil();
        for value in values {
            if !matches!(&ctx.attr(value).kind, AttributeKind::Enum { .. }) {
                return None;
            }
            result = result.append(value);
        }
        Some(result)
    }

    /// Adapter: Attribute.Compound.member(Name) (Attribute.java:490-510).
    fn compound_member(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        compound: AttributeId,
        name: &crate::util::name::Name,
    ) -> Option<AttributeId> {
        let values = match &ctx.attr(compound).kind {
            AttributeKind::Compound { values, .. } | AttributeKind::TypeCompound { values, .. } => {
                values.clone()
            }
            _ => return None,
        };
        let mut current = values;
        while current.non_empty() {
            let pair = current.head().clone();
            current = current.tail();
            if &ctx.sym(pair.fst).name == name {
                return Some(pair.snd);
            }
        }
        None
    }

    /// Java: public AnnotationType annotationTargetType(Attribute.Compound a,
    /// Symbol s) (TypeAnnotations.java:175-185).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - a: the annotation attribute;
    ///   - s: the annotated symbol.
    ///
    /// # Returns
    ///   - the annotation target kind.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: TypeAnnotations.java:175-185.
    pub fn annotation_target_type(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &mut Symtab,
        a: AttributeId,
        s: SymbolId,
    ) -> AnnotationType {
        let Some(anno_type) = ctx.attr(a).ty else {
            return AnnotationType::NONE;
        };
        let Some(tsym) = ctx.typ(anno_type).tsym else {
            return AnnotationType::NONE;
        };
        if !ctx.sym(tsym).is_annotation_type() {
            return AnnotationType::NONE;
        }
        let targets = self.annotation_targets(ctx, syms, tsym);
        match targets {
            None => AnnotationType::DECLARATION,
            Some(targets) => {
                let mut acc = AnnotationType::NONE;
                let mut current = targets;
                while current.non_empty() {
                    let target = *current.head();
                    current = current.tail();
                    let at = self.target_to_annotation_type(ctx, target, s);
                    acc = self.combine_annotation_type(acc, at);
                }
                acc
            }
        }
    }

    /// Java: private AnnotationType combineAnnotationType(AnnotationType at1,
    /// AnnotationType at2) (TypeAnnotations.java:187-197).
    fn combine_annotation_type(
        &self,
        at1: AnnotationType,
        at2: AnnotationType,
    ) -> AnnotationType {
        if at1 == AnnotationType::NONE {
            at2
        } else if at2 == AnnotationType::NONE {
            at1
        } else if at1 != at2 {
            AnnotationType::BOTH
        } else {
            at1
        }
    }

    /// Java: private AnnotationType targetToAnnotationType(Attribute a, Symbol s)
    /// (TypeAnnotations.java:199-255).
    fn target_to_annotation_type(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        a: AttributeId,
        s: SymbolId,
    ) -> AnnotationType {
        use crate::code::kinds::Kind;
        let enum_sym = match &ctx.attr(a).kind {
            AttributeKind::Enum { value: Some(value) } => *value,
            _ => return AnnotationType::NONE,
        };
        let target = ctx.sym(enum_sym).name.clone();
        let sym_kind = ctx.sym(s).kind;
        let flags_value = ctx.sym(s).flags_field;
        if target == self.names.TYPE {
            if sym_kind == Kind::TYP {
                return AnnotationType::DECLARATION;
            }
        } else if target == self.names.FIELD || target == self.names.RECORD_COMPONENT {
            let owner_is_mth = ctx
                .sym(s)
                .owner
                .map(|owner| ctx.sym(owner).kind == Kind::MTH)
                .unwrap_or(false);
            if sym_kind == Kind::VAR && !owner_is_mth {
                return AnnotationType::DECLARATION;
            }
        } else if target == self.names.METHOD {
            if sym_kind == Kind::MTH && !ctx.sym(s).is_constructor(&self.names) {
                return AnnotationType::DECLARATION;
            }
        } else if target == self.names.PARAMETER {
            let owner_is_mth = ctx
                .sym(s)
                .owner
                .map(|owner| ctx.sym(owner).kind == Kind::MTH)
                .unwrap_or(false);
            if sym_kind == Kind::VAR
                && owner_is_mth
                && (flags_value & crate::code::flags::PARAMETER) != 0
            {
                return AnnotationType::DECLARATION;
            }
        } else if target == self.names.CONSTRUCTOR {
            if sym_kind == Kind::MTH && ctx.sym(s).is_constructor(&self.names) {
                return AnnotationType::DECLARATION;
            }
        } else if target == self.names.LOCAL_VARIABLE {
            let owner_is_mth = ctx
                .sym(s)
                .owner
                .map(|owner| ctx.sym(owner).kind == Kind::MTH)
                .unwrap_or(false);
            if sym_kind == Kind::VAR
                && owner_is_mth
                && (flags_value & crate::code::flags::PARAMETER) == 0
            {
                return AnnotationType::DECLARATION;
            }
        } else if target == self.names.ANNOTATION_TYPE {
            if sym_kind == Kind::TYP
                && (flags_value & crate::code::flags::ANNOTATION as i64) != 0
            {
                return AnnotationType::DECLARATION;
            }
        } else if target == self.names.PACKAGE {
            if sym_kind == Kind::PCK {
                return AnnotationType::DECLARATION;
            }
        } else if target == self.names.TYPE_USE {
            let is_constructor = sym_kind == Kind::MTH && ctx.sym(s).is_constructor(&self.names);
            let returns_non_void = sym_kind == Kind::MTH
                && !is_constructor
                && ctx
                    .sym(s)
                    .ty
                    .map(|ty| {
                        let ret = self.types.get_return_type_public(ctx, ty);
                        !ctx.typ(ret).has_tag(crate::code::type_tag::TypeTag::VOID)
                    })
                    .unwrap_or(false);
            if sym_kind == Kind::TYP || sym_kind == Kind::VAR || returns_non_void || is_constructor {
                return AnnotationType::TYPE;
            }
        } else if target == self.names.TYPE_PARAMETER {
            // TYPE_PARAMETER does not distinguish typing here.
        } else if target == self.names.MODULE {
            if sym_kind == Kind::MDL {
                return AnnotationType::DECLARATION;
            }
        } else {
            panic!("AssertionError: annotationTargetType(): unrecognized Attribute name");
        }
        AnnotationType::NONE
    }
}

