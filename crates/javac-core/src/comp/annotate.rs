// Origin: com/sun/tools/javac/comp/Annotate.java @ 982d53c (jdk-17.0.19-ga)
//
// Batch 3.5a contains the Annotate support types that the batch-3.5b Annotate
// body and the batch-3.5 pre-requisite Check.validateAnnotation family need:
// the AnnotationTypeCompleter interface and the AnnotationTypeMetadata class
// (Annotate.java:1310-1460). The Annotate class body itself follows in 3.5b.
//
// PORTING_RULES:
// - the Java AnnotationTypeMetadata holds a ClassSymbol and a completer; the
//   port stores the SymbolId and Rc<RefCell<dyn AnnotationTypeCompleter>>
//   (arena identity + ADR 0001 explicit context).
// - the Java NOT_AN_ANNOTATION_TYPE anonymous singleton maps to the
//   Default/none state (is_metadata_for_annotation_type == false, all getters
//   empty), which is behaviourally identical.
// - the Java isCompleted() loop uses the COMPLETE symbol flag, which is not
//   ported; the port uses the completed-completer state
//   (CompilerContext::complete_symbol takes the completer), registered §9.

use std::cell::RefCell;
use std::rc::Rc;

use crate::code::flags;
use crate::code::kinds::Kind;
use crate::code::scope::LookupKind;
use crate::code::symtab::Symtab;
use crate::code::symbol::CompletionFailure;
use crate::compiler_context::CompilerContext;
use crate::ids::{AttributeId, SymbolId};

/// Java: public static interface Annotate.AnnotationTypeCompleter
pub trait AnnotationTypeCompleter {
    /// Java: void complete(ClassSymbol sym) throws CompletionFailure
    fn complete(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        sym: SymbolId,
    ) -> Result<(), CompletionFailure>;
}

/// Java: public static class Annotate.AnnotationTypeMetadata
#[derive(Clone)]
pub struct AnnotationTypeMetadata {
    /// Java: final ClassSymbol metaDataFor
    meta_data_for: Option<SymbolId>,
    /// Java: private Compound target
    target: Option<AttributeId>,
    /// Java: private Compound repeatable
    repeatable: Option<AttributeId>,
    /// Java: private AnnotationTypeCompleter annotationTypeCompleter
    annotation_type_completer: Option<Rc<RefCell<dyn AnnotationTypeCompleter>>>,
    /// Java: NOT_AN_ANNOTATION_TYPE.isMetadataForAnnotationType() == false
    is_metadata_for_annotation_type: bool,
}

impl Default for AnnotationTypeMetadata {
    fn default() -> Self {
        AnnotationTypeMetadata::not_an_annotation_type()
    }
}

impl AnnotationTypeMetadata {
    /// Java: public AnnotationTypeMetadata(ClassSymbol metaDataFor,
    /// AnnotationTypeCompleter annotationTypeCompleter)
    pub fn new(
        meta_data_for: SymbolId,
        annotation_type_completer: Option<Rc<RefCell<dyn AnnotationTypeCompleter>>>,
    ) -> AnnotationTypeMetadata {
        AnnotationTypeMetadata {
            meta_data_for: Some(meta_data_for),
            target: None,
            repeatable: None,
            annotation_type_completer,
            is_metadata_for_annotation_type: true,
        }
    }

    /// Java: public static AnnotationTypeMetadata notAnAnnotationType()
    pub fn not_an_annotation_type() -> AnnotationTypeMetadata {
        AnnotationTypeMetadata {
            meta_data_for: None,
            target: None,
            repeatable: None,
            annotation_type_completer: None,
            is_metadata_for_annotation_type: false,
        }
    }

    /// Java: private void init() — Make sure metaDataFor is member entered;
    /// then run the deferred completer (Java: while (!isCompleted()) complete()).
    fn init(&mut self, ctx: &mut CompilerContext, syms: &mut Symtab) -> Result<(), CompletionFailure> {
        let Some(meta_data_for) = self.meta_data_for else {
            return Ok(());
        };
        while ctx.sym(meta_data_for).completer.is_some() {
            ctx.complete_symbol(syms, meta_data_for)?;
        }
        if let Some(completer) = self.annotation_type_completer.take() {
            completer.borrow_mut().complete(ctx, syms, meta_data_for)?;
        }
        Ok(())
    }

    /// Java: public void complete()
    pub fn complete(&mut self, ctx: &mut CompilerContext, syms: &mut Symtab) -> Result<(), CompletionFailure> {
        self.init(ctx, syms)
    }

    /// Java: public Compound getRepeatable()
    pub fn get_repeatable(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
    ) -> Result<Option<AttributeId>, CompletionFailure> {
        self.init(ctx, syms)?;
        Ok(self.repeatable)
    }

    /// Java: public void setRepeatable(Compound repeatable)
    pub fn set_repeatable(&mut self, repeatable: AttributeId) {
        assert!(self.repeatable.is_none(), "AssertionError: repeatable already set");
        self.repeatable = Some(repeatable);
    }

    /// Java: public Compound getTarget()
    pub fn get_target(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
    ) -> Result<Option<AttributeId>, CompletionFailure> {
        self.init(ctx, syms)?;
        Ok(self.target)
    }

    /// Java: public void setTarget(Compound target)
    pub fn set_target(&mut self, target: AttributeId) {
        assert!(self.target.is_none(), "AssertionError: target already set");
        self.target = Some(target);
    }

    /// Java: public Set<MethodSymbol> getAnnotationElements()
    pub fn get_annotation_elements(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
    ) -> Result<Vec<SymbolId>, CompletionFailure> {
        self.init(ctx, syms)?;
        let Some(meta_data_for) = self.meta_data_for else {
            return Ok(Vec::new());
        };
        let members = match &ctx.sym(meta_data_for).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
            _ => None,
        };
        let mut result: Vec<SymbolId> = Vec::new();
        if let Some(scope) = members {
            let clinit = syms.names().clinit.clone();
            let symbols = ctx.scope_get_symbols(
                scope,
                LookupKind::NON_RECURSIVE,
                &|_| true,
            );
            for sym in symbols {
                let candidate = ctx.sym(sym);
                if candidate.kind == Kind::MTH
                    && candidate.name.to_java_string() != clinit.to_java_string()
                    && (candidate.flags_field & flags::SYNTHETIC as i64) == 0
                {
                    result.push(sym);
                }
            }
        }
        Ok(result)
    }

    /// Java: public Set<MethodSymbol> getAnnotationElementsWithDefault()
    pub fn get_annotation_elements_with_default(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
    ) -> Result<Vec<SymbolId>, CompletionFailure> {
        let members = self.get_annotation_elements(ctx, syms)?;
        let mut result: Vec<SymbolId> = Vec::new();
        for member in members {
            let has_default = match &ctx.sym(member).kind_data {
                crate::code::symbol::SymbolKind::MethodSymbol { default_value, .. } => {
                    default_value.is_some()
                }
                _ => false,
            };
            if has_default {
                result.push(member);
            }
        }
        Ok(result)
    }

    /// Java: public boolean isMetadataForAnnotationType()
    pub fn is_metadata_for_annotation_type(&self) -> bool {
        self.is_metadata_for_annotation_type
    }

    /// Java: final ClassSymbol metaDataFor accessor (the port keeps Option for
    /// the NOT_AN_ANNOTATION_TYPE state).
    pub fn meta_data_for(&self) -> Option<SymbolId> {
        self.meta_data_for
    }
}
