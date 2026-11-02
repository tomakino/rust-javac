//
// contains the Annotate support types that the -3.5b Annotate
// body and the -3.5 pre-requisite Check.validateAnnotation family need:
// the AnnotationTypeCompleter interface and the AnnotationTypeMetadata class
// (Annotate.java:1310-1460). The Annotate class body itself follows in 3.5b.
//
// Rust:
// - the Java AnnotationTypeMetadata holds a ClassSymbol and a completer; the
// port stores the SymbolId and Rc<RefCell<dyn AnnotationTypeCompleter>>
// (arena identity + ADR 0001 explicit context).
// - the AnnotationTypeMetadata object itself is shared through the ClassSymbol
// (Rc<AnnotationTypeMetadata>, upstream single-object semantics); the
// getters are shared-object entry points that take the completer under a short
// borrow (registered adapter).
// - the Java NOT_AN_ANNOTATION_TYPE anonymous singleton maps to the
// Default/none state (is_metadata_for_annotation_type == false, all getters
// empty), which is behaviourally identical.
// - the Java isCompleted() loop uses the COMPLETE symbol flag, which is not
// ported; the port uses the completed-completer state
// (CompilerContext::complete_symbol takes the completer), registered adapter.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use crate::code::attribute::{Attribute, AttributeKind};
use crate::code::deferred_lint_handler::DeferredLintHandler;
use crate::code::flags;
use crate::code::kinds::Kind;
use crate::code::lint::Lint;
use crate::code::source::{Feature, Source};
use crate::code::symtab::Symtab;
use crate::code::scope::LookupKind;
use crate::code::symbol::{CompletionFailure, SymbolKind};
use crate::code::type_tag::TypeTag;
use crate::comp::attr::Attr;
use crate::comp::attr_context::AttrContext;
use crate::comp::check::{Check, CheckContext};
use crate::comp::const_fold::ConstFold;
use crate::comp::enter::Enter;
use crate::comp::env::Env;
use crate::comp::resolve::Resolve;
use crate::comp::type_envs::TypeEnvs;
use crate::compiler_context::CompilerContext;
use crate::ids::{AttributeId, SymbolId, TreeId, TypeId};
use crate::resources::compiler_properties::errors;
use crate::resources::diagnostic_contract::{DiagnosticArg, KindNameArg, SymbolArg, TypeArg};
use crate::tree::jc_tree::{Tag, TreeKind};
use crate::tree::tree_info;
use crate::tree::tree_maker::{TreeFactory, TreeMaker};
use crate::tree::tree_scanner::{tree_scanner_visitor_delegate, TreeScannerVisitor};
use crate::util::abstract_log::AbstractLog;
use crate::util::jc_diagnostic::{JCDiagnostic, SimpleDiagnosticPosition};
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::names::Names;
use crate::util::pair::Pair;

/// Java: public static interface Annotate.AnnotationTypeCompleter
pub trait AnnotationTypeCompleter {
    /// Java: void complete(ClassSymbol sym) throws CompletionFailure
    ///
    /// Port adapter (A3k): the caller's Attr is forwarded through the
    /// annotation-type completion chain. Upstream completion runs on the shared
    /// Java Annotate instance, which can always reach the enclosing Attr; the
    /// ported SourceCompleter needs the same mutable Attr the caller holds (the
    /// RefCell<Rc<Attr>> service must not be re-borrowed while it is lent out).
    /// None is the Attr-less entry point, which falls back to that service.
    fn complete(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        sym: SymbolId,
        attr: Option<&mut crate::comp::attr::Attr>,
    ) -> Result<(), CompletionFailure>;
}

/// Java: public static class Annotate.AnnotationTypeMetadata
#[derive(Clone)]
pub struct AnnotationTypeMetadata {
    /// Java: final ClassSymbol metaDataFor
    meta_data_for: Option<SymbolId>,
    /// Java: private Compound target
    target: RefCell<Option<AttributeId>>,
    /// Java: private Compound repeatable
    repeatable: RefCell<Option<AttributeId>>,
    /// Java: private AnnotationTypeCompleter annotationTypeCompleter
    annotation_type_completer: RefCell<Option<Rc<RefCell<dyn AnnotationTypeCompleter>>>>,
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
            target: RefCell::new(None),
            repeatable: RefCell::new(None),
            annotation_type_completer: RefCell::new(annotation_type_completer),
            is_metadata_for_annotation_type: true,
        }
    }

    /// Java: public static AnnotationTypeMetadata notAnAnnotationType()
    pub fn not_an_annotation_type() -> AnnotationTypeMetadata {
        AnnotationTypeMetadata {
            meta_data_for: None,
            target: RefCell::new(None),
            repeatable: RefCell::new(None),
            annotation_type_completer: RefCell::new(None),
            is_metadata_for_annotation_type: false,
        }
    }

    /// Java: private void init() — make sure metaDataFor is member entered,
    /// then run the deferred completer (Java: while (!isCompleted()) complete()).
    ///
    /// The upstream AnnotationTypeMetadata object is shared through the
    /// ClassSymbol; the port stores the shared Rc and takes the completer under a
    /// short borrow so the completer's own setters can mutate the same object
    /// without re-entering the RefCell borrow (registered adapter for the shared
    /// object model).
    ///
    /// # Arguments
    ///   - metadata: the shared metadata object;
    ///   - ctx/syms: the compiler services.
    ///
    /// # Returns
    ///   - Ok(()) when the initialization completed (no-op when the completer
    ///     was already taken or the type is not an annotation type).
    ///
    /// # Panics
    ///   - none (completion failures are returned).
    ///
    /// Upstream: Annotate.java:1288-1302.
    fn complete_shared(
        metadata: &Rc<AnnotationTypeMetadata>,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        mut attr: Option<&mut crate::comp::attr::Attr>,
    ) -> Result<(), CompletionFailure> {
        let Some(meta_data_for) = metadata.meta_data_for else {
            return Ok(());
        };
        while ctx.sym(meta_data_for).completer.is_some() {
            // Java: metaDataFor.complete() runs while the caller's Attr is on
            // the stack; keep passing that Attr so a source annotation type
            // completed here does not look the service up re-entrantly.
            match attr.as_deref_mut() {
                Some(attr) => {
                    ctx.complete_symbol_with_attr(syms, meta_data_for, Some(attr))?
                }
                None => ctx.complete_symbol(syms, meta_data_for)?,
            }
        }
        let completer = {
            let mut completer = metadata.annotation_type_completer.borrow_mut();
            if completer.is_none() {
                return Ok(());
            }
            completer.take()
        };
        if let Some(completer) = completer {
            completer.borrow_mut().complete(ctx, syms, meta_data_for, attr)?;
        }
        Ok(())
    }

    /// Java: public Compound getRepeatable() (Annotate.java:1304-1307).
    ///
    /// # Arguments
    ///   - metadata: the shared metadata object;
    ///   - ctx/syms: the compiler services.
    ///
    /// # Returns
    ///   - the @Repeatable compound, if any.
    ///
    /// # Panics
    ///   - none (completion failures are returned).
    ///
    /// Upstream: Annotate.java:1304-1307.
    pub fn get_repeatable_shared(
        metadata: &Rc<AnnotationTypeMetadata>,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        attr: Option<&mut crate::comp::attr::Attr>,
    ) -> Result<Option<AttributeId>, CompletionFailure> {
        Self::complete_shared(metadata, ctx, syms, attr)?;
        Ok(*metadata.repeatable.borrow())
    }

    /// Java: public Compound getTarget() (Annotate.java:1314-1317).
    ///
    /// # Arguments
    ///   - metadata: the shared metadata object;
    ///   - ctx/syms: the compiler services.
    ///
    /// # Returns
    ///   - the @Target compound, if any.
    ///
    /// # Panics
    ///   - none (completion failures are returned).
    ///
    /// Upstream: Annotate.java:1314-1317.
    pub fn get_target_shared(
        metadata: &Rc<AnnotationTypeMetadata>,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        attr: Option<&mut crate::comp::attr::Attr>,
    ) -> Result<Option<AttributeId>, CompletionFailure> {
        Self::complete_shared(metadata, ctx, syms, attr)?;
        Ok(*metadata.target.borrow())
    }

    /// Java: public Set<MethodSymbol> getAnnotationElements()
    /// (Annotate.java:1324-1335).
    ///
    /// # Arguments
    ///   - metadata: the shared metadata object;
    ///   - ctx/syms: the compiler services.
    ///
    /// # Returns
    ///   - the non-synthetic annotation elements.
    ///
    /// # Panics
    ///   - none (completion failures are returned).
    ///
    /// Upstream: Annotate.java:1324-1335.
    pub fn get_annotation_elements_shared(
        metadata: &Rc<AnnotationTypeMetadata>,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        attr: Option<&mut crate::comp::attr::Attr>,
    ) -> Result<Vec<SymbolId>, CompletionFailure> {
        Self::complete_shared(metadata, ctx, syms, attr)?;
        let Some(meta_data_for) = metadata.meta_data_for else {
            return Ok(Vec::new());
        };
        let members = match &ctx.sym(meta_data_for).kind_data {
            SymbolKind::ClassSymbol { members_field, .. } => *members_field,
            _ => None,
        };
        let mut result: Vec<SymbolId> = Vec::new();
        if let Some(scope) = members {
            let clinit = syms.names().clinit.clone();
            let symbols = ctx.scope_get_symbols(scope, LookupKind::NON_RECURSIVE, &|_| true);
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
    /// (Annotate.java:1337-1348).
    ///
    /// # Arguments
    ///   - metadata: the shared metadata object;
    ///   - ctx/syms: the compiler services.
    ///
    /// # Returns
    ///   - the annotation elements that declare a default value.
    ///
    /// # Panics
    ///   - none (completion failures are returned).
    ///
    /// Upstream: Annotate.java:1337-1348.
    pub fn get_annotation_elements_with_default_shared(
        metadata: &Rc<AnnotationTypeMetadata>,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        attr: Option<&mut crate::comp::attr::Attr>,
    ) -> Result<Vec<SymbolId>, CompletionFailure> {
        let members = Self::get_annotation_elements_shared(metadata, ctx, syms, attr)?;
        let mut result: Vec<SymbolId> = Vec::new();
        for member in members {
            let has_default = match &ctx.sym(member).kind_data {
                SymbolKind::MethodSymbol { default_value, .. } => default_value.is_some(),
                _ => false,
            };
            if has_default {
                result.push(member);
            }
        }
        Ok(result)
    }

    /// Java: public void setRepeatable(Compound repeatable) (Annotate.java:1309-1312).
    ///
    /// # Arguments
    ///   - repeatable: the @Repeatable compound (may be null).
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - the upstream Assert.checkNull: the value must not already be set.
    ///
    /// Upstream: Annotate.java:1309-1312.
    pub fn set_repeatable(&self, repeatable: Option<AttributeId>) {
        assert!(
            self.repeatable.borrow().is_none(),
            "AssertionError: repeatable already set"
        );
        *self.repeatable.borrow_mut() = repeatable;
    }

    /// Java: public void setTarget(Compound target) (Annotate.java:1319-1322).
    ///
    /// # Arguments
    ///   - target: the @Target compound (may be null).
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - the upstream Assert.checkNull: the value must not already be set.
    ///
    /// Upstream: Annotate.java:1319-1322.
    pub fn set_target(&self, target: Option<AttributeId>) {
        assert!(
            self.target.borrow().is_none(),
            "AssertionError: target already set"
        );
        *self.target.borrow_mut() = target;
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

// ---------------------------------------------------------------------------
// Java: public class Annotate (Annotate.java:78-1426) —.
//
// (ADR 0001): the upstream Context-injected services are held in
// an Rc<AnnotateServices> so the queued closures (the Java Runnable queues) can
// capture the service set without borrowing the Annotate object; the queue
// state itself is Rc<RefCell<AnnotateQueues>> because the Java Queues object is
// replaced/exported by setQueues and is mutated from queued closures.
// The blockCount/flushCount scalars use Cell (the Java fields are only ever
// read/incremented from the owning Annotate instance).
// ---------------------------------------------------------------------------
/// Java: java.lang.Runnable queued by the annotation scheduler. The ported
/// closure receives the CompilerContext explicitly (ADR 0001) and propagates
/// CompletionFailure like the Java call sites.
pub type AnnotateRunnable = Box<
    dyn FnMut(
        &mut CompilerContext,
        &crate::code::symtab::Symtab,
        &mut crate::comp::attr::Attr,
    ) -> Result<(), CompletionFailure>,
>;

/// Java: static class Annotate.Queues (Annotate.java:1409-1426).
pub struct AnnotateQueues {
    /// Java: private final ListBuffer<Runnable> q
    pub q: VecDeque<AnnotateRunnable>,
    /// Java: private final ListBuffer<Runnable> validateQ
    pub validate_q: VecDeque<AnnotateRunnable>,
    /// Java: private final ListBuffer<Runnable> typesQ
    pub types_q: VecDeque<AnnotateRunnable>,
    /// Java: private final ListBuffer<Runnable> afterTypesQ
    pub after_types_q: VecDeque<AnnotateRunnable>,
}

impl AnnotateQueues {
    /// Java: public Queues()
    pub fn new() -> AnnotateQueues {
        AnnotateQueues {
            q: VecDeque::new(),
            validate_q: VecDeque::new(),
            types_q: VecDeque::new(),
            after_types_q: VecDeque::new(),
        }
    }
}

impl Default for AnnotateQueues {
    fn default() -> Self {
        AnnotateQueues::new()
    }
}

/// Java: class Annotate.AnnotationValueContext extends Check.NestedCheckContext
/// (Annotate.java:691-701) — the annotation-value compatibility override.
pub struct AnnotationValueContext {
    pub enclosing_context: Rc<dyn CheckContext>,
}

impl CheckContext for AnnotationValueContext {
    fn compatible(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &crate::code::types::Types,
        found: TypeId,
        req: TypeId,
        warn: &crate::util::warner::Warner,
    ) -> bool {
        // Java: return found.hasTag(TypeTag.NONE) || super.compatible(found, req, warn);
        ctx.typ(found).has_tag(TypeTag::NONE)
            || self
                .enclosing_context
                .compatible(ctx, syms, types, found, req, warn)
    }

    fn report(&self, ctx: &mut CompilerContext, pos: i32, details: &JCDiagnostic) {
        self.enclosing_context.report(ctx, pos, details);
    }

    fn check_warner(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &crate::code::types::Types,
        pos: i32,
        found: TypeId,
        req: TypeId,
    ) -> crate::util::warner::Warner {
        self.enclosing_context
            .check_warner(ctx, syms, types, pos, found, req)
    }

    fn inference_context(&self) -> &Rc<crate::comp::infer::InferenceContext> {
        self.enclosing_context.inference_context()
    }

    fn deferred_attr_context(&self) -> &Rc<crate::comp::deferred_attr::DeferredAttrContext> {
        self.enclosing_context.deferred_attr_context()
    }
}

/// The service bundle captured by the queued closures (the Java Annotate
/// instance fields).
pub struct AnnotateServices {
    pub names: Rc<Names>,
    pub log: Rc<Log>,
    pub syms: Rc<Symtab>,
    pub types: Rc<crate::code::types::Types>,
    pub chk: Rc<Check>,
    pub attr: Rc<std::cell::RefCell<Attr>>,
    pub cfolder: Rc<ConstFold>,
    pub deferred_lint_handler: Rc<DeferredLintHandler>,
    pub enter: Rc<Enter>,
    pub lint: Rc<Lint>,
    pub resolve: Rc<Resolve>,
    pub make: Rc<TreeMaker>,
    pub type_envs: Rc<TypeEnvs>,
    pub infer: Rc<crate::comp::infer::Infer>,
    pub queues: Rc<std::cell::RefCell<AnnotateQueues>>,
    /// Java: private final Attribute theUnfinishedDefaultValue
    pub the_unfinished_default_value: AttributeId,
    /// Java: private final boolean allowRepeatedAnnos
    pub allow_repeated_annos: bool,
    /// Java: private final String sourceName
    pub source_name: String,
    /// Annotation types whose declaration-level @Repeatable validation has been
    /// queued once (the use-site processing reports declaration errors after
    /// the repeated uses).
    pub validated_repeatable_decls: Rc<std::cell::RefCell<std::collections::HashSet<SymbolId>>>,
    /// Weak self reference used by the queued closures (the Java inner-class
    /// instances capture the enclosing Annotate); set right after construction.
    pub self_ref: std::cell::RefCell<std::rc::Weak<AnnotateServices>>,
}

/// Java: public class Annotate (Annotate.java:78).
pub struct Annotate {
    pub services: Rc<AnnotateServices>,
    /// Java: private int blockCount = 0 (the constructor sets it to 1).
    pub block_count: Cell<i32>,
    /// Java: private int flushCount = 0
    pub flush_count: Cell<i32>,
}

impl Annotate {
    /// Java: protected Annotate(Context context) — explicit services (ADR 0001).
#[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: &mut CompilerContext,
        names: Rc<Names>,
        log: Rc<Log>,
        syms: Rc<Symtab>,
        types: Rc<crate::code::types::Types>,
        chk: Rc<Check>,
        attr: Rc<std::cell::RefCell<Attr>>,
        cfolder: Rc<ConstFold>,
        deferred_lint_handler: Rc<DeferredLintHandler>,
        enter: Rc<Enter>,
        lint: Rc<Lint>,
        resolve: Rc<Resolve>,
        make: Rc<TreeMaker>,
        type_envs: Rc<TypeEnvs>,
        diags: Rc<crate::util::jc_diagnostic::Factory>,
        source: Source,
    ) -> Annotate {
        let error_ty = ctx.typ(syms.err_type).clone();
        let unfinished = ctx.alloc_attribute(Attribute::new(
            Some(syms.err_type),
            AttributeKind::Error,
        ));
        let _ = error_ty;
        let infer = crate::comp::infer::Infer::new(
            ctx,
            Feature::GRAPH_INFERENCE.allowed_in_source(source),
            Rc::clone(&diags),
            Rc::clone(&names),
        );
        let queues = Rc::new(std::cell::RefCell::new(AnnotateQueues::new()));
        let services = Rc::new(AnnotateServices {
            names,
            log,
            syms,
            types,
            chk,
            attr,
            cfolder,
            deferred_lint_handler,
            enter,
            lint,
            resolve,
            make,
            type_envs,
            infer,
            queues,
            the_unfinished_default_value: unfinished,
            allow_repeated_annos: Feature::REPEATED_ANNOTATIONS.allowed_in_source(source),
            source_name: source.name().to_string(),
            validated_repeatable_decls: Rc::new(std::cell::RefCell::new(
                std::collections::HashSet::new(),
            )),
            self_ref: std::cell::RefCell::new(std::rc::Weak::new()),
        });
        *services.self_ref.borrow_mut() = Rc::downgrade(&services);
        Annotate {
            services,
            block_count: Cell::new(1),
            flush_count: Cell::new(0),
        }
    }

    /// Java: public static Annotate instance(Context context) (Annotate.java:100-120).
    ///
    /// the ported Annotate is created by the service wiring with
    /// its explicit dependencies; the Context registration makes the instance
    /// reachable like the Java Context singleton (registered adapter: the
    /// instance must be registered by the compiler entry point).
    pub fn instance(
        context: &mut crate::util::context::Context,
    ) -> Rc<Annotate> {
        match context.get::<Annotate>() {
            Some(instance) => instance,
            None => panic!("AssertionError: Annotate is not registered in the Context"),
        }
    }

    /// Java: public void blockAnnotations() (Annotate.java:121-123)
    pub fn block_annotations(&self) {
        self.block_count.set(self.block_count.get() + 1);
    }

    /// Java: public void unblockAnnotations() (Annotate.java:126-131)
    pub fn unblock_annotations(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
    ) -> Result<(), CompletionFailure> {
        self.block_count.set(self.block_count.get() - 1);
        if self.block_count.get() == 0 {
            // Port adapter: the flush needs the Attr service; borrow it only
            // here (the Attr-inside callers use unblock_annotations_with_attr
            // because they already hold it).
            let attr_rc = std::rc::Rc::clone(&self.services.attr);
            let mut attr = attr_rc.borrow_mut();
            self.flush_with_attr(ctx, syms, &mut attr)?;
        }
        Ok(())
    }

    pub fn unblock_annotations_with_attr(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut crate::comp::attr::Attr,
    ) -> Result<(), CompletionFailure> {
        self.block_count.set(self.block_count.get() - 1);
        if self.block_count.get() == 0 {
            self.flush_with_attr(ctx, syms, attr)?;
        }
        Ok(())
    }

    /// Java: public void unblockAnnotationsNoFlush() (Annotate.java:135-137)
    pub fn unblock_annotations_no_flush(&self) {
        self.block_count.set(self.block_count.get() - 1);
    }

    /// Java: public boolean annotationsBlocked() (Annotate.java:140)
    pub fn annotations_blocked(&self) -> bool {
        self.block_count.get() > 0
    }

    /// Java: public void enterDone() (Annotate.java:142-144)
    pub fn enter_done(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
    ) -> Result<(), CompletionFailure> {
        self.unblock_annotations(ctx, syms)
    }

    /// Java: public void normal(Runnable r) (Annotate.java:180-182)
    pub fn normal(&self, task: AnnotateRunnable) {
        self.services.queues.borrow_mut().q.push_back(task);
    }

    /// Java: public void validate(Runnable a) (Annotate.java:184-186)
    pub fn validate(&self, task: AnnotateRunnable) {
        self.services.queues.borrow_mut().validate_q.push_back(task);
    }

    /// Java: public void typeAnnotation(Runnable a) (Annotate.java:211-213)
    pub fn type_annotation(&self, task: AnnotateRunnable) {
        self.services.queues.borrow_mut().types_q.push_back(task);
    }

    /// Java: public void afterTypes(Runnable a) (Annotate.java:215-217)
    pub fn after_types(&self, task: AnnotateRunnable) {
        self.services.queues.borrow_mut().after_types_q.push_back(task);
    }

    /// Java: private boolean isFlushing() (Annotate.java:203-205)
    pub fn is_flushing(&self) -> bool {
        self.flush_count.get() > 0
    }

    /// Java: private void startFlushing() (Annotate.java:206-208)
    fn start_flushing(&self) {
        self.flush_count.set(self.flush_count.get() + 1);
    }

    /// Java: private void doneFlushing() (Annotate.java:209-211)
    fn done_flushing(&self) {
        self.flush_count.set(self.flush_count.get() - 1);
    }

    /// Java: public void flush() (Annotate.java:188-207)
    pub fn flush(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
    ) -> Result<(), CompletionFailure> {
        // Port adapter: the Attr-inside callers pass their &mut Attr to
        // flush_with_attr; the normal path borrows the Attr service once.
        let attr_rc = std::rc::Rc::clone(&self.services.attr);
        // Completion can re-enter Annotate while Attr is already mutably
        // borrowed (e.g. while resolving imports); upstream shares the same
        // instance, so skip this flush instead of panicking. Annotations are
        // retried by the normal flush path.
        let Ok(mut attr) = attr_rc.try_borrow_mut() else {
            return Ok(());
        };
        self.flush_with_attr(ctx, syms, &mut attr)
    }

    pub fn flush_with_attr(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attr: &mut crate::comp::attr::Attr,
    ) -> Result<(), CompletionFailure> {
        if self.annotations_blocked() {
            return Ok(());
        }
        if self.is_flushing() {
            return Ok(());
        }
        self.start_flushing();
        let result = (|| -> Result<(), CompletionFailure> {
            loop {
                let task = self.services.queues.borrow_mut().q.pop_front();
                match task {
                    Some(mut task) => task(ctx, syms, attr)?,
                    None => break,
                }
            }
            loop {
                let task = self.services.queues.borrow_mut().types_q.pop_front();
                match task {
                    Some(mut task) => task(ctx, syms, attr)?,
                    None => break,
                }
            }
            loop {
                let task = self
                    .services
                    .queues
                    .borrow_mut()
                    .after_types_q
                    .pop_front();
                match task {
                    Some(mut task) => task(ctx, syms, attr)?,
                    None => break,
                }
            }
            loop {
                let task = self.services.queues.borrow_mut().validate_q.pop_front();
                match task {
                    Some(mut task) => task(ctx, syms, attr)?,
                    None => break,
                }
            }
            Ok(())
        })();
        self.done_flushing();
        result
    }

    /// Java: public Queues setQueues(Queues nue) (Annotate.java:1401-1413)
    pub fn set_queues(&self, nue: AnnotateQueues) -> AnnotateQueues {
        std::mem::replace(&mut *self.services.queues.borrow_mut(), nue)
    }

    /// Java: public void newRound() (Annotate.java:1397-1399)
    pub fn new_round(&self) {
        self.block_count.set(1);
    }

    /// Java: public Attribute unfinishedDefaultValue() (Annotate.java:1314-1316)
    pub fn unfinished_default_value(&self) -> AttributeId {
        self.services.the_unfinished_default_value
    }
}

// Annotate : scheduling/entry of annotations.
//
// Rust: the queued closures capture the Rc<AnnotateServices> and the
// tree/symbol ids; the CompilerContext is passed to flush and forwarded to each
// closure (ADR 0001). The Java methods that used the implicit Context keep the
// explicit ctx parameter.

impl Annotate {
    /// Java: public List<TypeCompound> fromAnnotations(List<JCAnnotation>
    /// annotations) (Annotate.java:146-158)
    pub fn from_annotations(&self, ctx: &CompilerContext, annotations: &List<TreeId>) -> List<AttributeId> {
        self.services.from_annotations(ctx, annotations)
    }

    /// Java: public void annotateLater(List<JCAnnotation> annotations,
    /// Env<AttrContext> localEnv, Symbol s, DiagnosticPosition deferPos)
    /// (Annotate.java:246-310)
    pub fn annotate_later(
        &self,
        ctx: &mut CompilerContext,
        annotations: List<TreeId>,
        local_env: Rc<Env<AttrContext>>,
        s: SymbolId,
        defer_pos: Option<i32>,
    ) {
        if annotations.is_empty() {
            return;
        }
        // mark Annotations as incomplete for now
        ctx.sym_mut(s).reset_annotations();
        let services = Rc::clone(&self.services);
        let services_validate = Rc::clone(&self.services);
        let env = Rc::clone(&local_env);
        let env_validate = Rc::clone(&local_env);
        let annotations_normal = annotations.clone();
        let annotations_validate = annotations.clone();
        self.normal(Box::new(move |ctx: &mut CompilerContext, syms: &crate::code::symtab::Symtab, _attr: &mut crate::comp::attr::Attr| {
            services.annotate_later_normal(ctx, syms, &annotations_normal, &env, s, defer_pos, _attr)
        }));
        self.validate(Box::new(move |ctx: &mut CompilerContext, syms: &crate::code::symtab::Symtab, _attr: &mut crate::comp::attr::Attr| {
            let services = &services_validate;
            let source = env_source_file(ctx, &env_validate);
            let prev = services.log.use_source(source);
            let declaration_tree = {
                let env_tree = env_validate.tree;
                match env_tree {
                    Some(env_tree) => tree_info::declaration_for(ctx, s, env_tree),
                    None => None,
                }
            };
            let result = services.chk.validate_annotations(
                ctx,
                syms,
                &annotations_validate,
                declaration_tree,
                s,
                Some(_attr),
            );
            services.log.use_source(prev);
            result
        }));
    }

    /// Java: public void annotateDefaultValueLater(JCExpression defaultValue,
    /// Env<AttrContext> localEnv, MethodSymbol m, DiagnosticPosition deferPos)
    /// (Annotate.java:312-341)
    pub fn annotate_default_value_later(
        &self,
        ctx: &mut CompilerContext,
        default_value: TreeId,
        local_env: Rc<Env<AttrContext>>,
        m: SymbolId,
        defer_pos: Option<i32>,
    ) {
        let _ = ctx;
        let services = Rc::clone(&self.services);
        let services_validate = Rc::clone(&self.services);
        let env = Rc::clone(&local_env);
        let env_validate = Rc::clone(&local_env);
        self.normal(Box::new(move |ctx: &mut CompilerContext, syms: &crate::code::symtab::Symtab, _attr: &mut crate::comp::attr::Attr| {
            let source = env_source_file(ctx, &env);
            let prev = services.log.use_source(source);
            let prev_pos = services.deferred_lint_handler.immediate();
            let result = if let Some(defer_pos) = defer_pos {
                let saved = services.deferred_lint_handler.set_pos(Rc::new(
                    SimpleDiagnosticPosition::new(defer_pos),
                ));
                let r = services.enter_default_value(ctx, syms, default_value, &env, m, _attr);
                services.deferred_lint_handler.set_pos_state(saved);
                r
            } else {
                services.enter_default_value(ctx, syms, default_value, &env, m, _attr)
            };
            services.deferred_lint_handler.set_pos_state(prev_pos);
            services.log.use_source(prev);
            result
        }));
        self.validate(Box::new(move |ctx: &mut CompilerContext, syms: &crate::code::symtab::Symtab, _attr: &mut crate::comp::attr::Attr| {
            let source = env_source_file(ctx, &env_validate);
            let prev = services_validate.log.use_source(source);
            services_validate
                .chk
                .validate_annotation_tree(ctx, syms, default_value, Some(_attr));
            services_validate.log.use_source(prev);
            Ok(())
        }));
    }
}

impl AnnotateServices {
    /// Java: public void annotateLater normal clause (Annotate.java:251-303)
    #[allow(clippy::redundant_closure_call)]
    #[allow(clippy::too_many_arguments)]
    fn annotate_later_normal(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        annotations: &List<TreeId>,
        local_env: &Rc<Env<AttrContext>>,
        s: SymbolId,
        defer_pos: Option<i32>,
    
        attr: &mut crate::comp::attr::Attr,) -> Result<(), CompletionFailure> {
        assert!(
            ctx.sym(s).kind == Kind::PCK || ctx.sym(s).annotations_pending_completion(),
            "AssertionError: annotations pending completion"
        );
        let source = env_source_file(ctx, local_env);
        let prev = self.log.use_source(source);
        let prev_pos = match defer_pos {
            Some(defer_pos) => self
                .deferred_lint_handler.set_pos(Rc::new(SimpleDiagnosticPosition::new(defer_pos))),
            None => self.deferred_lint_handler.immediate(),
        };
        let prev_lint = if defer_pos.is_some() {
            None
        } else {
            Some(self.chk.set_lint(Rc::clone(&self.lint)))
        };
        let result = (|| -> Result<(), CompletionFailure> {
            if ctx.sym(s).has_annotations() && annotations.non_empty() {
                let head = *annotations.head();
                self.log.error_at(
                    ctx.tree(head).pos,
                    crate::resources::compiler_properties::errors::already_annotated(
                        KindNameArg::placeholder(
                            format!("{:?}", ctx.sym(s).kind.kind_name()),
                        ),
                        SymbolArg::placeholder(crate::comp::resolve::symbol_display_name(
                            ctx, s,
                        )),
                    )
                    .into(),
                );
            }
            self.annotate_now(ctx, syms, s, annotations, local_env, false, false, attr)
        })();
        if let Some(prev_lint) = prev_lint {
            self.chk.set_lint(prev_lint);
        }
        self.deferred_lint_handler.set_pos_state(prev_pos);
        self.log.use_source(prev);
        result
    }

    /// Java: private void enterDefaultValue(JCExpression defaultValue,
    /// Env<AttrContext> localEnv, MethodSymbol m) (Annotate.java:343-348)
    fn enter_default_value(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        default_value: TreeId,
        local_env: &Rc<Env<AttrContext>>,
        m: SymbolId,
    
        attr: &mut crate::comp::attr::Attr,) -> Result<(), CompletionFailure> {
        let m_type = ctx.sym(m).ty.expect("method type");
        let return_type = return_type_of(ctx, m_type);
        let value = self.attribute_annotation_value(ctx, syms, return_type, default_value, local_env, attr)?;
        if let SymbolKind::MethodSymbol { default_value: slot, .. } = &mut ctx.sym_mut(m).kind_data {
            *slot = Some(value);
        }
        Ok(())
    }

    /// Java: private boolean isAttributeTrue(Attribute attr) (Annotate.java:452-456)
    fn is_attribute_true(&self, ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab, attr: AttributeId) -> bool {
        let (attr_ty, constant_value) = match &ctx.attr(attr).kind {
            AttributeKind::Constant { value } => (ctx.attr(attr).ty, Some(value.clone())),
            _ => (None, None),
        };
        let Some(constant_value) = constant_value else {
            return false;
        };
        let is_boolean = match attr_ty {
            Some(ty) => self.types.is_same_type(ctx, syms, ty, syms.boolean_type),
            None => false,
        };
        let truthy = match constant_value {
            crate::util::constants::ConstantValue::Boolean(value) => value,
            crate::util::constants::ConstantValue::Integer(value) => value != 0,
            _ => false,
        };
        is_boolean && truthy
    }
}

/// Annotate : attribute/annotation value conversion + repeated
/// annotations + TypeAnnotate + annotation-type completer.
impl AnnotateServices {
    /// Java: private void annotateNow(Symbol toAnnotate, List<JCAnnotation>
    /// withAnnotations, Env<AttrContext> env, boolean typeAnnotations,
    /// boolean isTypeParam) (Annotate.java:325-450)
    #[allow(clippy::too_many_arguments)]
    fn annotate_now(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        to_annotate: SymbolId,
        with_annotations: &List<TreeId>,
        env: &Rc<Env<AttrContext>>,
        type_annotations: bool,
        is_type_param: bool,
    
        attr: &mut crate::comp::attr::Attr,) -> Result<(), CompletionFailure> {
        let mut annotated: Vec<(SymbolId, Vec<AttributeId>)> = Vec::new();
        let mut pos: std::collections::HashMap<AttributeId, i32> = std::collections::HashMap::new();
        let mut current = with_annotations.clone();
        while current.non_empty() {
            let a = *current.head();
            let annotation_type = syms.annotation_type;
            let c = if type_annotations {
                self.attribute_type_annotation(ctx, syms, a, annotation_type, env, attr)?
            } else {
                self.attribute_annotation(ctx, syms, a, annotation_type, env, attr)?
            };
            let annotation_ty = ctx.tree(a).ty.expect("annotation tree type");
            let annotation_tsym = ctx.typ(annotation_ty).tsym.expect("annotation tsym");
            let is_annotation_type =
                (ctx.sym(annotation_tsym).flags_field & flags::ANNOTATION as i64) != 0;
            if is_annotation_type {
                if let Some(entry) = annotated
                    .iter_mut()
                    .find(|(tsym, _)| *tsym == annotation_tsym)
                {
                    if !self.allow_repeated_annos {
                        self.log.error_at(
                            ctx.tree(a).pos,
                            Feature::REPEATED_ANNOTATIONS.error(&self.source_name),
                        );
                    }
                    entry.1.push(c);
                } else {
                    annotated.push((annotation_tsym, vec![c]));
                }
                pos.insert(c, ctx.tree(a).pos);
            }
            // Note: @Deprecated has no effect on local variables and parameters
            let deprecated_type = syms.deprecated_type;
            let is_erroneous = ctx.typ(annotation_ty).has_tag(TypeTag::ERROR);
            let owner_kind = ctx.sym(to_annotate).owner.map(|owner| ctx.sym(owner).kind);
            if !is_erroneous
                && (ctx.sym(to_annotate).kind == Kind::MDL || owner_kind != Some(Kind::MTH))
                && {
                    self.types
                        .is_same_type(ctx, syms, annotation_ty, deprecated_type)
                }
            {
                ctx.sym_mut(to_annotate).flags_field |= flags::DEPRECATED as i64;
                ctx.sym_mut(to_annotate).flags_field |= flags::DEPRECATED_ANNOTATION;
                let for_removal = self.names.from_string(&javac_strings::JavaString::from("forRemoval"));
                if self.is_attribute_true(ctx, syms, ctx.attr(c).member(ctx, &for_removal).unwrap_or(c))
                {
                    ctx.sym_mut(to_annotate).flags_field |= flags::DEPRECATED_REMOVAL;
                }
            }
            let preview_type = syms.preview_feature_type;
            if !is_erroneous && {
                self.types
                    .is_same_type(ctx, syms, annotation_ty, preview_type)
            } {
                ctx.sym_mut(to_annotate).flags_field |= flags::PREVIEW_API;
                let reflective = self.names.reflective.clone();
                if self.is_attribute_true(ctx, syms, ctx.attr(c).member(ctx, &reflective).unwrap_or(c))
                {
                    ctx.sym_mut(to_annotate).flags_field |= flags::PREVIEW_REFLECTIVE;
                }
            }
            let value_based_type = syms.value_based_type;
            if !is_erroneous
                && ctx.sym(to_annotate).kind == Kind::TYP
                && {
                    self.types
                        .is_same_type(ctx, syms, annotation_ty, value_based_type)
                }
            {
                ctx.sym_mut(to_annotate).flags_field |= flags::VALUE_BASED;
            }
            current = current.tail();
        }
        let mut buf: List<AttributeId> = List::nil();
        for (_, lb) in annotated.iter() {
            if lb.len() == 1 {
                buf = buf.prepend(lb[0]);
            } else {
                let context = AnnotatedContext {
                    env: Rc::clone(env),
                    annotated: annotated.clone(),
                    pos: pos.clone(),
                    is_type_compound: type_annotations,
                };
                if let Some(res) =
                    self.make_container_annotation(ctx, syms, lb, &context, to_annotate, is_type_param, attr)?
                {
                    buf = buf.prepend(res);
                }
            }
        }
        let mut reversed: List<AttributeId> = List::nil();
        let mut current_buf = buf;
        while current_buf.non_empty() {
            reversed = reversed.prepend(*current_buf.head());
            current_buf = current_buf.tail();
        }
        if type_annotations {
            ctx.sym_mut(to_annotate).append_unique_type_attributes(reversed);
        } else {
            ctx.sym_mut(to_annotate).reset_annotations();
            ctx.sym_mut(to_annotate).set_declaration_attributes(reversed);
        }
        Ok(())
    }

    /// Java: public Attribute.Compound attributeAnnotation(JCAnnotation tree,
    /// Type expectedAnnotationType, Env<AttrContext> env) (Annotate.java:462-476)
    #[allow(clippy::unnecessary_unwrap)]
    fn attribute_annotation(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        tree: TreeId,
        expected_annotation_type: TypeId,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr) -> Result<AttributeId, CompletionFailure> {
        let existing = match &ctx.tree(tree).kind {
            TreeKind::Annotation { attribute, .. } => *attribute,
            _ => None,
        };
        if existing.is_some() && ctx.tree(tree).ty.is_some() {
            return Ok(existing.expect("annotation attribute"));
        }
        let elems = self.attribute_annotation_values(ctx, syms, tree, expected_annotation_type, env, attr)?;
        let ty = ctx.tree(tree).ty.expect("annotation tree type");
        let attribute = ctx.alloc_attribute(Attribute::new_compound(Some(ty), elems));
        if let TreeKind::Annotation { attribute: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(attribute);
        }
        Ok(attribute)
    }

    /// Java: public Attribute.TypeCompound attributeTypeAnnotation(JCAnnotation a,
    /// Type expectedAnnotationType, Env<AttrContext> env) (Annotate.java:484-507)
    #[allow(clippy::unnecessary_unwrap)]
    fn attribute_type_annotation(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        a: TreeId,
        expected_annotation_type: TypeId,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr) -> Result<AttributeId, CompletionFailure> {
        let existing = match &ctx.tree(a).kind {
            TreeKind::Annotation { attribute, .. } => *attribute,
            _ => None,
        };
        let is_type_compound = existing
            .map(|existing| matches!(&ctx.attr(existing).kind, AttributeKind::TypeCompound { .. }))
            .unwrap_or(false);
        if !(existing.is_none() || ctx.tree(a).ty.is_none() || !is_type_compound) {
            return Ok(existing.expect("type compound"));
        }
        let elems = self.attribute_annotation_values(ctx, syms, a, expected_annotation_type, env, attr)?;
        let ty = ctx.tree(a).ty.expect("annotation tree type");
        let position = crate::code::type_annotation_position::TypeAnnotationPosition::unknown();
        let attribute = ctx.alloc_attribute(Attribute::new_type_compound(Some(ty), elems, Some(position)));
        if let TreeKind::Annotation { attribute: slot, .. } = &mut ctx.tree_mut(a).kind {
            *slot = Some(attribute);
        }
        Ok(attribute)
    }
}

/// Annotate : annotation element values.
impl AnnotateServices {
    /// Attributes the name/value pairs of an annotation, including the elided
    /// single `value` element.
    ///
    /// Port adapter: the synthesized `value=` assignment is written back into
    /// the annotation tree, matching the upstream in-place replacement of the
    /// single argument.
    ///
    /// # Arguments
    ///   - ctx/syms: the compiler services;
    ///   - a: the annotation tree;
    ///   - expected: the annotation type;
    ///   - env: the attribution environment;
    ///   - attr: the enclosing Attr.
    ///
    /// # Returns
    ///   - the name/value pairs.
    ///
    /// # Panics
    ///   - completion failures are returned.
    fn attribute_annotation_values(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        a: TreeId,
        expected: TypeId,
        env: &Rc<Env<AttrContext>>,
    
        attr: &mut crate::comp::attr::Attr,) -> Result<List<Pair<SymbolId, AttributeId>>, CompletionFailure> {
        let annotation_type_tree = match &ctx.tree(a).kind {
            TreeKind::Annotation { annotation_type, .. } => annotation_type.expect("annotation type"),
            _ => panic!("expected an annotation"),
        };
        let at = match ctx.tree(annotation_type_tree).ty {
            Some(at) => at,
            None => {
                    attr.attrib_type(ctx, syms, annotation_type_tree, env)
            }
        };
        let checked = {
            let chk = Rc::clone(&self.chk);
            chk.check_type_basic(ctx, syms, ctx.tree(annotation_type_tree).pos, at, expected)
        };
        ctx.tree_mut(a).ty = Some(checked);
        let is_error = ctx.typ(checked).has_tag(TypeTag::ERROR);
        let tsym = ctx.typ(checked).tsym.expect("annotation type symbol");
        let is_annotation_type = (ctx.sym(tsym).flags_field & flags::ANNOTATION as i64) != 0;
        let mut is_error = is_error;
        if !is_annotation_type && !is_error {
            self.log.error_at(
                ctx.tree(annotation_type_tree).pos,
                errors::not_annotation_type(
                    TypeArg::placeholder(crate::comp::resolve::symbol_display_name(ctx, tsym)),
                )
                .into(),
            );
            is_error = true;
        }
        let mut args = match &ctx.tree(a).kind {
            TreeKind::Annotation { args, .. } => args.clone(),
            _ => panic!("expected an annotation"),
        };
        let mut elided_value = false;
        if args.length() == 1 {
            let head = *args.head();
            if !ctx.tree(head).has_tag(Tag::ASSIGN) {
                let value_name = self.names.value.clone();
                let ident = {
                    let make = &*self.make;
                    make.ident(ctx, value_name)
                };
                let assign = {
                    let make = &*self.make;
                    make.at(ctx.tree(head).pos);
                    make.assign(ctx, ident, head)
                };
                args = List::of(assign);
                // The upstream replaces the single argument with the
                // synthesized "value=" assignment in the tree itself, so the
                // later Check.validateAnnotation inventory sees the elided
                // value as provided.
                if let TreeKind::Annotation {
                    args: tree_args, ..
                } = &mut ctx.tree_mut(a).kind
                {
                    *tree_args = args.clone();
                }
                elided_value = true;
            }
        }
        let mut buf: List<Pair<SymbolId, AttributeId>> = List::nil();
        let mut current = args;
        while current.non_empty() {
            let arg = *current.head();
            if let Some(pair) = self.attribute_annotation_name_value_pair(ctx, syms,
                arg,
                checked,
                is_error,
                env,
                elided_value,
                annotation_type_tree,
            attr)? {
                let method_erroneous = ctx
                    .sym(pair.fst)
                    .ty
                    .map(|ty| ctx.typ(ty).has_tag(TypeTag::ERROR))
                    .unwrap_or(false);
                if !method_erroneous {
                    buf = buf.append(pair);
                }
            }
            current = current.tail();
        }
        Ok(buf)
    }

    /// Java: private Pair<MethodSymbol, Attribute> attributeAnnotationNameValuePair(
    /// JCExpression nameValuePair, Type thisAnnotationType, boolean badAnnotation,
    /// Env<AttrContext> env, boolean elidedValue) (Annotate.java:552-589)
    #[allow(clippy::too_many_arguments)]
    fn attribute_annotation_name_value_pair(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        name_value_pair: TreeId,
        this_annotation_type: TypeId,
        bad_annotation: bool,
        env: &Rc<Env<AttrContext>>,
        elided_value: bool,
        _annotation_type_tree: TreeId,
    
        attr: &mut crate::comp::attr::Attr,) -> Result<Option<Pair<SymbolId, AttributeId>>, CompletionFailure> {
        if !ctx.tree(name_value_pair).has_tag(Tag::ASSIGN) {
            self.log.error_at(
                ctx.tree(name_value_pair).pos,
                errors::annotation_value_must_be_name_value().into(),
            );
            // Java: the malformed name/value pair is still attributed as an
            // expression, so an identifier such as `name` is reported as an
            // unresolved variable. Resolve that identifier directly without
            // the enclosing-class location used for ordinary expressions.
            if let TreeKind::Binary { lhs: Some(lhs), .. } = &ctx.tree(name_value_pair).kind
                && let TreeKind::Ident { name, .. } = &ctx.tree(*lhs).kind
            {
                let info = crate::resources::compiler_properties::errors::cant_resolve(
                    crate::resources::diagnostic_contract::KindNameArg::placeholder(
                        "kindname.variable",
                    ),
                    name.clone(),
                    (),
                    (),
                );
                self.log
                    .error_at(ctx.tree(*lhs).pos, info.into());
            }
            let err_type = syms.err_type;
            ctx.tree_mut(name_value_pair).ty = Some(err_type);
            return Ok(None);
        }
        let (lhs, rhs) = match &ctx.tree(name_value_pair).kind {
            TreeKind::Assign { lhs, rhs } => (lhs.expect("assign lhs"), rhs.expect("assign rhs")),
            _ => panic!("expected an assignment"),
        };
        if !ctx.tree(lhs).has_tag(Tag::IDENT) {
            self.log.error_at(
                ctx.tree(name_value_pair).pos,
                errors::annotation_value_must_be_name_value().into(),
            );
            let err_type = syms.err_type;
            ctx.tree_mut(name_value_pair).ty = Some(err_type);
            let _ = self.attribute_annotation_value(ctx, syms, err_type, name_value_pair, env, attr)?;
            return Ok(None);
        }
        let left_pos = if elided_value { ctx.tree(rhs).pos } else { ctx.tree(lhs).pos };
        let left_name = match &ctx.tree(lhs).kind {
            TreeKind::Ident { name, .. } => name.clone(),
            _ => panic!("expected an identifier"),
        };
        let method = {
            let resolve = &*self.resolve;
            let chk = Rc::clone(&self.chk);
            let infer = Rc::clone(&self.infer);
            let location = env
                .encl_class
                .and_then(|encl_class| match &ctx.tree(encl_class).kind {
                    TreeKind::ClassDecl { sym, .. } => *sym,
                    _ => None,
                })
                .expect("enclosing class symbol");
            resolve.resolve_qualified_method(
                ctx,
                syms,
                &self.types,
                attr,
                &chk,
                &infer,
                Some(Rc::new(SimpleDiagnosticPosition::new(left_pos))),
                env,
                location,
                this_annotation_type,
                left_name.clone(),
                List::nil(),
                None,
            )
        };
        if let TreeKind::Ident { sym: slot, .. } = &mut ctx.tree_mut(lhs).kind {
            *slot = Some(method);
        }
        let method_ty = ctx.sym(method).ty.expect("annotation element method type");
        ctx.tree_mut(lhs).ty = Some(method_ty);
        let owner = ctx.sym(method).owner;
        let this_tsym = ctx.typ(this_annotation_type).tsym;
        if owner != this_tsym && !bad_annotation {
            self.log.error_at(
                ctx.tree(lhs).pos,
                errors::no_annotation_member(
                    left_name,
                    TypeArg::placeholder(crate::comp::resolve::symbol_display_name(
                        ctx,
                        this_tsym.expect("annotation type symbol"),
                    )),
                )
                .into(),
            );
        }
        let result_type = return_type_of(ctx, method_ty);
        let value = self.attribute_annotation_value(ctx, syms, result_type, rhs, env, attr)?;
        ctx.tree_mut(name_value_pair).ty = Some(result_type);
        let method_erroneous = ctx
            .sym(method)
            .ty
            .map(|ty| ctx.typ(ty).has_tag(TypeTag::ERROR))
            .unwrap_or(false);
        if method_erroneous {
            return Ok(None);
        }
        Ok(Some(Pair::new(method, value)))
    }
}

/// Annotate : annotation element values + helpers.
impl AnnotateServices {
    /// Java: private Attribute attributeAnnotationValue(Type expectedElementType,
    /// JCExpression tree, Env<AttrContext> env) (Annotate.java:591-660)
    fn attribute_annotation_value(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        expected_element_type: TypeId,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    
        attr: &mut crate::comp::attr::Attr,) -> Result<AttributeId, CompletionFailure> {
        let mut expected_element_type = expected_element_type;
        let expected_sym = ctx
            .typ(expected_element_type)
            .tsym;
        let completion_error = match expected_sym {
            Some(sym) if ctx.sym(sym).completer.is_some() => {
                ctx.complete_symbol_with_attr(syms, sym, Some(attr)).err()
            }
            _ => None,
        };
        if let Some(failure) = completion_error {
            self.log.error_at(
                ctx.tree(tree).pos,
                errors::cant_access(
                    SymbolArg::placeholder(
                        failure
                            .sym
                            .map(|sym| crate::comp::resolve::symbol_display_name(ctx, sym))
                            .unwrap_or_default(),
                    ),
                    DiagnosticArg::from_display("completion failure"),
                )
                .into(),
            );
            expected_element_type = syms.err_type;
        }
        if ctx
            .typ(expected_element_type)
            .has_tag(TypeTag::ARRAY)
        {
            return self.get_annotation_array_value(ctx, syms, expected_element_type, tree, env, attr);
        }
        if ctx.tree(tree).has_tag(Tag::NEWARRAY) {
            if !ctx.typ(expected_element_type).has_tag(TypeTag::ERROR) {
                self.log.error_at(
                    ctx.tree(tree).pos,
                    errors::annotation_value_not_allowable_type().into(),
                );
            }
            let elems = match &ctx.tree(tree).kind {
                TreeKind::NewArray { elems, .. } => (*elems).clone(),
                _ => None,
            };
            let elemtype = match &ctx.tree(tree).kind {
                TreeKind::NewArray { elemtype, .. } => *elemtype,
                _ => None,
            };
            if let Some(elemtype) = elemtype {
                self.log.error_at(
                    ctx.tree(elemtype).pos,
                    errors::new_not_allowed_in_annotation().into(),
                );
            }
            if let Some(elems) = elems {
                let err_type = syms.err_type;
                for elem in elems.iter() {
                    let _ = self.attribute_annotation_value(ctx, syms, err_type, elem, env, attr)?;
                }
            }
            let err_type = syms.err_type;
            return Ok(ctx.alloc_attribute(Attribute::new_error(Some(err_type))));
        }
        let expected_tsym = ctx.typ(expected_element_type).tsym;
        let expected_is_annotation_type = expected_tsym
            .map(|tsym| (ctx.sym(tsym).flags_field & flags::ANNOTATION as i64) != 0)
            .unwrap_or(false);
        if expected_is_annotation_type {
            if ctx.tree(tree).has_tag(Tag::ANNOTATION) {
                return self.attribute_annotation(ctx, syms, tree, expected_element_type, env, attr);
            } else {
                self.log.error_at(
                    ctx.tree(tree).pos,
                    errors::annotation_value_must_be_annotation().into(),
                );
                expected_element_type = syms.err_type;
            }
        }
        if ctx.tree(tree).has_tag(Tag::ANNOTATION) {
            if !ctx.typ(expected_element_type).has_tag(TypeTag::ERROR) {
                self.log.error_at(
                    ctx.tree(tree).pos,
                    errors::annotation_not_valid_for_type(TypeArg::placeholder(
                        crate::comp::resolve::symbol_display_name(
                            ctx,
                            ctx.typ(expected_element_type)
                                .tsym
                                .expect("expected element type symbol"),
                        ),
                    ))
                    .into(),
                );
            }
            let err_type = syms.err_type;
            let _ = self.attribute_annotation(ctx, syms, tree, err_type, env, attr)?;
            let annotation_type_tree = match &ctx.tree(tree).kind {
                TreeKind::Annotation { annotation_type, .. } => *annotation_type,
                _ => None,
            };
            let orig = annotation_type_tree.and_then(|t| ctx.tree(t).ty);
            return Ok(ctx.alloc_attribute(Attribute::new_error(orig)));
        }
        // Java: MemberEnter.InitTreeVisitor(annotation value) — the ported visitor');
        // entry point is member_enter::init_tree_visitor (the same JLS 15.28 rules).
        let result = crate::comp::member_enter::init_tree_visitor(ctx, tree);
        if !result {
            self.log.error_at(
                ctx.tree(tree).pos,
                errors::expression_not_allowable_as_annotation_value().into(),
            );
            let err_type = syms.err_type;
            return Ok(ctx.alloc_attribute(Attribute::new_error(Some(err_type))));
        }
        let string_type = syms.string_type;
        let is_primitive = ctx.typ(expected_element_type).is_primitive();
        let is_string = {
            self.types
                .is_same_type(ctx, syms, expected_element_type, string_type)
        } && !ctx.typ(expected_element_type).has_tag(TypeTag::ERROR);
        if is_primitive || is_string {
            return self.get_annotation_primitive_value(ctx, syms, expected_element_type, tree, env, attr);
        }
        let class_type = syms.class_type;
        let class_tsym = ctx.typ(class_type).tsym;
        if ctx.typ(expected_element_type).tsym == class_tsym {
            return self.get_annotation_class_value(ctx, syms, expected_element_type, tree, env, attr);
        }
        let is_enum = ctx.typ(expected_element_type).has_tag(TypeTag::CLASS)
            && ctx
                .typ(expected_element_type)
                .tsym
                .map(|tsym| (ctx.sym(tsym).flags_field & flags::ENUM as i64) != 0)
                .unwrap_or(false);
        if is_enum {
            return self.get_annotation_enum_value(ctx, syms, expected_element_type, tree, env, attr);
        }
        if !ctx.typ(expected_element_type).has_tag(TypeTag::ERROR) {
            self.log.error_at(
                ctx.tree(tree).pos,
                errors::annotation_value_not_allowable_type().into(),
            );
        }
        let info = self.annotation_value_info(ctx, expected_element_type, attr);
        let attributed = {
            attr.attrib_tree(ctx, syms, tree, env, info)?
        };
        Ok(ctx.alloc_attribute(Attribute::new_error(Some(attributed))))
    }
}

/// Annotate : class/enum/primitive/array annotation values +
/// repeating-annotation containers.
impl AnnotateServices {
    /// Java: private Attribute getAnnotationEnumValue(Type expectedElementType,
    /// JCExpression tree, Env<AttrContext> env) (Annotate.java:662-677)
    #[allow(clippy::single_match)]
    fn get_annotation_enum_value(&self, ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab, expected_element_type: TypeId, tree: TreeId, env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,) -> Result<AttributeId, CompletionFailure> {
        let info = self.annotation_value_info(ctx, expected_element_type, attr);
        let result = {
            attr.attrib_tree(ctx, syms, tree, env, info)?
        };
        let sym = tree_info::symbol(ctx, tree);
        let nonstatic_select = tree_info::nonstatic_select(ctx, tree);
        let is_enum_constant = sym
            .map(|sym| ctx.sym(sym).kind == Kind::VAR && (ctx.sym(sym).flags_field & flags::ENUM as i64) != 0)
            .unwrap_or(false);
        if sym.is_none() || nonstatic_select || !is_enum_constant {
            self.log.error_at(
                ctx.tree(tree).pos,
                errors::enum_annotation_must_be_enum_constant().into(),
            );
            return Ok(ctx.alloc_attribute(Attribute::new_error(Some(original_type_of(ctx, result)))));
        }
        Ok(ctx.alloc_attribute(Attribute::new_enum(Some(expected_element_type), sym.expect("enum constant"))))
    }

    /// Java: private Attribute getAnnotationClassValue(Type expectedElementType,
    /// JCExpression tree, Env<AttrContext> env) (Annotate.java:679-703)
    fn get_annotation_class_value(&self, ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab, expected_element_type: TypeId, tree: TreeId, env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,) -> Result<AttributeId, CompletionFailure> {
        let info = self.annotation_value_info(ctx, expected_element_type, attr);
        let result = {
            attr.attrib_tree(ctx, syms, tree, env, info)?
        };
        if ctx.typ(result).has_tag(TypeTag::ERROR) {
            let class_name = self.names._class.clone();
            let tree_name = tree_info::name(ctx, tree);
            if tree_name.as_ref().map(|name| name.to_java_string() == class_name.to_java_string()).unwrap_or(false) {
                let selected = match &ctx.tree(tree).kind {
                    TreeKind::FieldAccess { selected, .. } => *selected,
                    _ => None,
                };
                if let Some(selected) = selected {
                    let selected_ty = ctx.tree(selected).ty;
                    let selected_erroneous = selected_ty.map(|ty| ctx.typ(ty).has_tag(TypeTag::ERROR)).unwrap_or(false);
                    if selected_erroneous {
                        let name = ctx.typ(selected_ty.expect("selected type")).tsym.map(|tsym| ctx.sym(tsym).name.clone()).unwrap_or_else(|| self.names.empty.clone());
                        let unknown_symbol = syms.unknown_symbol;
                        let class_type = syms.class_type;
                        let err_type =
                            self.types.create_error_type(ctx, name, unknown_symbol, Some(class_type));
                        return Ok(ctx.alloc_attribute(Attribute::new_unresolved_class(Some(expected_element_type), Some(err_type))));
                    }
                }
            }
            return Ok(ctx.alloc_attribute(Attribute::new_error(Some(original_type_of(ctx, result)))));
        }
        let class_name = self.names._class.clone();
        let matches_class = tree_info::name(ctx, tree).as_ref().map(|name| name.to_java_string() == class_name.to_java_string()).unwrap_or(false);
        if !matches_class {
            self.log.error_at(ctx.tree(tree).pos, errors::annotation_value_must_be_class_literal().into());
            let err_type = syms.err_type;
            return Ok(ctx.alloc_attribute(Attribute::new_error(Some(err_type))));
        }
        let selected = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { selected, .. } => *selected,
            _ => None,
        };
        let selected_ty = selected.and_then(|selected| ctx.tree(selected).ty);
        let attribute = {
            Attribute::new_class(ctx, syms, &self.types, selected_ty.expect("class literal type"))
        };
        Ok(ctx.alloc_attribute(attribute))
    }

    /// Java: private Attribute getAnnotationPrimitiveValue(Type
    /// expectedElementType, JCExpression tree, Env<AttrContext> env)
    /// (Annotate.java:705-722)
    fn get_annotation_primitive_value(&self, ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab, expected_element_type: TypeId, tree: TreeId, env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,) -> Result<AttributeId, CompletionFailure> {
        let info = self.annotation_value_info(ctx, expected_element_type, attr);
        let mut result = {
            attr.attrib_tree(ctx, syms, tree, env, info)?
        };
        if ctx.typ(result).has_tag(TypeTag::ERROR) {
            return Ok(ctx.alloc_attribute(Attribute::new_error(Some(original_type_of(ctx, result)))));
        }
        if ctx.typ(result).const_value().is_none() {
            self.log.error_at(ctx.tree(tree).pos, errors::attribute_value_must_be_constant().into());
            return Ok(ctx.alloc_attribute(Attribute::new_error(Some(expected_element_type))));
        }
        let tree_type = ctx.tree(tree).ty;
        if let Some(tree_type) = tree_type {
            let tree_tsym = ctx.typ(tree_type).tsym;
            if let Some(tree_tsym) = tree_tsym {
                self.queue_scan_tree_and_type_annotate(tree, env, tree_tsym, ctx.tree(tree).pos);
            }
        }
        result = {
            self.cfolder.coerce(ctx, syms, result, expected_element_type)
        };
        let value = ctx.typ(result).const_value().expect("constant value").clone();
        Ok(ctx.alloc_attribute(Attribute::new_constant(Some(expected_element_type), value)))
    }

    /// Java: private Attr.ResultInfo annotationValueInfo(Type pt) (Annotate.java:686-689)
    fn annotation_value_info(&self, ctx: &mut CompilerContext, pt: TypeId,
        attr: &mut crate::comp::attr::Attr,) -> crate::comp::attr::ResultInfo {
        let _ = ctx;
        let base = attr.unknown_expr_info.clone();
        let mut result = base.dup(Some(pt));
        result.check_context = Rc::new(AnnotationValueContext {
            enclosing_context: Rc::clone(&base.check_context),
        });
        result
    }


    /// Java: private Attribute getAnnotationArrayValue(Type expectedElementType,
    /// JCExpression tree, Env<AttrContext> env) (Annotate.java:742-770)
    fn get_annotation_array_value(&self, ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab, expected_element_type: TypeId, tree: TreeId, env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,) -> Result<AttributeId, CompletionFailure> {
        let mut tree = tree;
        if !ctx.tree(tree).has_tag(Tag::NEWARRAY) {
            let make = &*self.make;
            make.at(ctx.tree(tree).pos);
            let array = make.new_array(ctx, None, List::nil(), Some(List::of(tree)));
            tree = array;
        }
        let (elemtype, elems) = match &ctx.tree(tree).kind {
            TreeKind::NewArray { elemtype, elems, .. } => (*elemtype, (*elems).clone()),
            _ => panic!("expected a new array"),
        };
        if let Some(elemtype) = elemtype {
            self.log.error_at(ctx.tree(elemtype).pos, errors::new_not_allowed_in_annotation().into());
        }
        let elem_type = {
            self.types.elemtype(ctx, syms, expected_element_type)
        };
        let mut buf: Vec<AttributeId> = Vec::new();
        if let Some(elems) = elems {
            for elem in elems.iter() {
                buf.push(self.attribute_annotation_value(ctx, syms, elem_type, elem, env, attr)?);
            }
        }
        ctx.tree_mut(tree).ty = Some(expected_element_type);
        Ok(ctx.alloc_attribute(Attribute::new_array(Some(expected_element_type), buf)))
    }
}

/// Annotate : repeating annotations.
#[derive(Clone)]
pub struct AnnotatedContext {
    pub env: Rc<Env<AttrContext>>,
    pub annotated: Vec<(SymbolId, Vec<AttributeId>)>,
    pub pos: std::collections::HashMap<AttributeId, i32>,
    pub is_type_compound: bool,
}

impl AnnotateServices {
    /// Java: private T processRepeatedAnnotations(List<T> annotations,
    /// AnnotationContext<T> ctx, Symbol on, boolean isTypeParam)
    /// (Annotate.java:800-935)
    #[allow(clippy::collapsible_if, clippy::manual_map)]
    #[allow(clippy::too_many_arguments)]
    fn process_repeated_annotations(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        annotations: &[AttributeId],
        context: &AnnotatedContext,
        on: SymbolId,
        is_type_param: bool,
        attr: &mut crate::comp::attr::Attr,
    ) -> Result<Option<AttributeId>, CompletionFailure> {
        let first_occurrence = annotations[0];
        let mut repeated: Vec<AttributeId> = Vec::new();
        let mut orig_anno_type: Option<TypeId> = None;
        let mut array_of_orig_anno_type: Option<TypeId> = None;
        let mut target_container_type: Option<TypeId> = None;
        let mut container_value_symbol: Option<SymbolId> = None;
        let mut count = 0;
        for current_anno in annotations.iter() {
            count += 1;
            let anno_type = ctx.attr(*current_anno).ty.expect("annotation attribute type");
            orig_anno_type = Some(anno_type);
            if array_of_orig_anno_type.is_none() {
                array_of_orig_anno_type = Some(self.types.make_array_type(ctx, syms, anno_type));
            }
            let report_error = count > 1;
            let current_pos = context.pos.get(current_anno).copied().unwrap_or(crate::util::position::NOPOS);
            let current_container_type = self.get_containing_type(ctx, syms, *current_anno, current_pos, report_error, &mut *attr)?;
            let Some(current_container_type) = current_container_type else {
                continue;
            };
            assert!(
                target_container_type.is_none() || target_container_type == Some(current_container_type),
                "AssertionError: target container type"
            );
            target_container_type = Some(current_container_type);
            container_value_symbol = self.validate_container(ctx, syms, current_container_type, anno_type, current_pos)?;
            if container_value_symbol.is_none() {
                continue;
            }
            repeated.push(*current_anno);
        }
        let orig_anno_type = orig_anno_type.expect("annotation type");
        if !repeated.is_empty() && target_container_type.is_none() {
            self.log.error_at(
                context.pos.get(&first_occurrence).copied().unwrap_or(crate::util::position::NOPOS),
                errors::duplicate_annotation_invalid_repeated(TypeArg::placeholder(
                    crate::comp::resolve::symbol_display_name(
                        ctx,
                        ctx.typ(orig_anno_type).tsym.expect("annotation tsym"),
                    ),
                ))
                .into(),
            );
            return Ok(None);
        }
        if repeated.is_empty() {
            return Ok(None);
        }
        // The port keeps sym.raw_attributes in source order (javac keeps them
        // reversed and reverses here), so the repeated annotations are already
        // in source order and must not be reversed again.
        let pos = context.pos.get(&first_occurrence).copied().unwrap_or(crate::util::position::NOPOS);
        let pair = Pair::new(container_value_symbol.expect("container value symbol"), {
            let values: Vec<AttributeId> = repeated.clone();
            ctx.alloc_attribute(Attribute::new_array(array_of_orig_anno_type, values))
        });
        if context.is_type_compound {
            let container_type = target_container_type.expect("container type");
            let first_position = match &ctx.attr(first_occurrence).kind {
                AttributeKind::TypeCompound { position, .. } => position.clone(),
                _ => None,
            };
            let at = ctx.alloc_attribute(Attribute::new_type_compound(
                Some(container_type),
                List::of(pair),
                first_position,
            ));
            let anno_tree = {
                let make = &*self.make;
                make.at(pos);
                make.translate_attribute(ctx, syms, at)
            };
            let defer_ok = {
                let chk = Rc::clone(&self.chk);
                chk.validate_annotation_defer_errors(ctx, syms, anno_tree, Some(&mut *attr))?
            };
            if !defer_ok {
                self.log.error_at(
                    ctx.tree(anno_tree).pos,
                    errors::duplicate_annotation_invalid_repeated(TypeArg::placeholder(
                        crate::comp::resolve::symbol_display_name(
                            ctx,
                            ctx.typ(orig_anno_type).tsym.expect("annotation tsym"),
                        ),
                    ))
                    .into(),
                );
            }
            // Java: if (!chk.isTypeAnnotation(annoTree, isTypeParam)) {
            //     log.error(pos, isTypeParam ?
            //         Errors.InvalidRepeatableAnnotationNotApplicable(targetContainerType, on) :
            //         Errors.InvalidRepeatableAnnotationNotApplicableInContext(targetContainerType));
            // } (Annotate.java:832-835).
            if !self
                .chk
                .is_type_annotation_tree(
                    ctx,
                    syms,
                    anno_tree,
                    is_type_param,
                    Some(&mut *attr),
                )
            {
                let container_name = crate::comp::resolve::symbol_display_name(
                    ctx,
                    ctx.typ(container_type).tsym.expect("container tsym"),
                );
                let info = if is_type_param {
                    errors::invalid_repeatable_annotation_not_applicable(
                        TypeArg::placeholder(container_name),
                        SymbolArg::placeholder(crate::comp::resolve::symbol_display_name(ctx, on)),
                    )
                } else {
                    errors::invalid_repeatable_annotation_not_applicable_in_context(
                        TypeArg::placeholder(container_name),
                    )
                };
                self.log.error_at(pos, info.into());
            }
            if let AttributeKind::TypeCompound { synthesized, .. } = &mut ctx.attr_mut(at).kind {
                *synthesized = true;
            }
            return Ok(Some(at));
        }
        let container_type = target_container_type.expect("container type");
        let c = ctx.alloc_attribute(Attribute::new_compound(Some(container_type), List::of(pair)));
        let anno_tree = {
            let make = &*self.make;
            make.at(pos);
            make.translate_attribute(ctx, syms, c)
        };
        let is_record_member = (ctx.sym(on).flags_field & flags::RECORD) != 0
            || ctx
                .sym(on)
                .owner
                .and_then(|owner| crate::code::symbol::symbol_encl_class(ctx, owner))
                .map(|encl| ctx.sym(encl).is_record())
                .unwrap_or(false);
        let applicable = {
            let chk = Rc::clone(&self.chk);
            chk.annotation_applicable(ctx, syms, anno_tree, on, Some(&mut *attr))?
        };
        let suppress = is_record_member && (ctx.sym(on).flags_field & flags::GENERATED_MEMBER as i64) != 0;
        if !applicable && !suppress {
            self.log.error_at(
                ctx.tree(anno_tree).pos,
                errors::invalid_repeatable_annotation_not_applicable(
                    TypeArg::placeholder(crate::comp::resolve::symbol_display_name(
                        ctx,
                        ctx.typ(container_type).tsym.expect("container tsym"),
                    )),
                    SymbolArg::placeholder(crate::comp::resolve::symbol_display_name(ctx, on)),
                )
                .into(),
            );
        }
        let defer_ok = {
            let chk = Rc::clone(&self.chk);
            chk.validate_annotation_defer_errors(ctx, syms, anno_tree, Some(&mut *attr))?
        };
        if !defer_ok {
            self.log.error_at(
                ctx.tree(anno_tree).pos,
                errors::duplicate_annotation_invalid_repeated(TypeArg::placeholder(
                    crate::comp::resolve::symbol_display_name(
                        ctx,
                        ctx.typ(orig_anno_type).tsym.expect("annotation tsym"),
                    ),
                ))
                .into(),
            );
        }
        let attributed = self.attribute_annotation(ctx, syms, anno_tree, container_type, &context.env, attr)?;
        if let AttributeKind::Compound { synthesized, .. } = &mut ctx.attr_mut(c).kind {
            *synthesized = true;
        }
        let _ = attributed;
        Ok(Some(c))
    }
}

/// Annotate : container lookup + TypeAnnotate + annotation-type
/// completer + shared helpers.
impl AnnotateServices {
    /// Java: private Type getContainingType(Attribute.Compound currentAnno,
    /// DiagnosticPosition pos, boolean reportError) (Annotate.java:940-960)
    fn get_containing_type(&self, ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab, current_anno: AttributeId, pos: i32, report_error: bool,
        attr: &mut crate::comp::attr::Attr) -> Result<Option<TypeId>, CompletionFailure> {
        let orig_anno_type = ctx.attr(current_anno).ty.expect("annotation type");
        let orig_anno_decl = ctx.typ(orig_anno_type).tsym.expect("annotation symbol");
        let Some(metadata) = ctx.sym(orig_anno_decl).get_annotation_type_metadata().cloned() else {
            return Ok(None);
        };
        let ca = AnnotationTypeMetadata::get_repeatable_shared(&metadata, ctx, syms, Some(&mut *attr))?;
        let Some(ca) = ca else {
            if report_error {
                self.log.error_at(pos, errors::duplicate_annotation_missing_container(TypeArg::placeholder(crate::comp::resolve::symbol_display_name(ctx, orig_anno_decl))).into());
            }
            return Ok(None);
        };
        let extracted = self.extract_containing_type(ctx, ca, pos, orig_anno_decl)?;
        Ok(self.filter_same(ctx, syms, extracted, Some(orig_anno_type)))
    }

    /// Java: private Type filterSame(Type t, Type s) (Annotate.java:962-968)
    fn filter_same(&self, ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab, t: Option<TypeId>, s: Option<TypeId>) -> Option<TypeId> {
        let Some(t) = t else { return t };
        let Some(s) = s else { return Some(t) };
        if self.types.is_same_type(ctx, syms, t, s) { None } else { Some(t) }
    }

    /// Java: private Type extractContainingType(Attribute.Compound ca,
    /// DiagnosticPosition pos, TypeSymbol annoDecl) (Annotate.java:971-993)
    fn extract_containing_type(&self, ctx: &mut CompilerContext, ca: AttributeId, pos: i32, anno_decl: SymbolId) -> Result<Option<TypeId>, CompletionFailure> {
        let values = match &ctx.attr(ca).kind {
            AttributeKind::Compound { values, .. } => values.clone(),
            _ => List::nil(),
        };
        if values.is_empty() {
            self.log.error_at(pos, errors::invalid_repeatable_annotation(SymbolArg::placeholder(crate::comp::resolve::symbol_display_name(ctx, anno_decl))).into());
            return Ok(None);
        }
        let pair = values.head().clone();
        let name = ctx.sym(pair.fst).name.clone();
        if name.to_java_string() != self.names.value.to_java_string() {
            self.log.error_at(pos, errors::invalid_repeatable_annotation(SymbolArg::placeholder(crate::comp::resolve::symbol_display_name(ctx, anno_decl))).into());
            return Ok(None);
        }
        let class_type = match &ctx.attr(pair.snd).kind {
            AttributeKind::Class { class_type } => *class_type,
            _ => {
                self.log.error_at(pos, errors::invalid_repeatable_annotation(SymbolArg::placeholder(crate::comp::resolve::symbol_display_name(ctx, anno_decl))).into());
                return Ok(None);
            }
        };
        Ok(class_type)
    }

    /// Java: private MethodSymbol validateContainer(Type targetContainerType,
    /// Type originalAnnoType, DiagnosticPosition pos) (Annotate.java:1000-1057)
    fn validate_container(&self, ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab, target_container_type: TypeId, original_anno_type: TypeId, pos: i32) -> Result<Option<SymbolId>, CompletionFailure> {
        let mut fatal_error = false;
        let container_tsym = ctx.typ(target_container_type).tsym.expect("container symbol");
        let members = match &ctx.sym(container_tsym).kind_data {
            SymbolKind::ClassSymbol { members_field, .. } => *members_field,
            _ => None,
        };
        let value_name = self.names.value.clone();
        let mut nr_value_elems = 0;
        let mut container_value_symbol: Option<SymbolId> = None;
        if let Some(scope) = members {
            let symbols = ctx.scope_get_symbols_by_name(scope, &value_name, LookupKind::NON_RECURSIVE, &|_| true);
            for elm in symbols {
                nr_value_elems += 1;
                if nr_value_elems == 1 && ctx.sym(elm).kind == Kind::MTH {
                    container_value_symbol = Some(elm);
                } else {
                    let _ = fatal_error;
                    fatal_error = true;
                }
            }
        }
        if fatal_error {
            self.log.error_at(pos, errors::invalid_repeatable_annotation_multiple_values(TypeArg::placeholder(crate::comp::resolve::symbol_display_name(ctx, container_tsym)), nr_value_elems).into());
            return Ok(None);
        } else if nr_value_elems == 0 {
            self.log.error_at(pos, errors::invalid_repeatable_annotation_no_value(DiagnosticArg::from_display(crate::comp::resolve::symbol_display_name(ctx, container_tsym))).into());
            return Ok(None);
        }
        let Some(container_value_symbol) = container_value_symbol else { return Ok(None) };
        if ctx.sym(container_value_symbol).kind != Kind::MTH {
            self.log.error_at(pos, errors::invalid_repeatable_annotation_invalid_value(TypeArg::placeholder(crate::comp::resolve::symbol_display_name(ctx, container_tsym))).into());
            fatal_error = true;
        }
        let method_type = ctx.sym(container_value_symbol).ty.expect("container value method type");
        let value_ret_type = return_type_of(ctx, method_type);
        let expected_type = {
            self.types.make_array_type(ctx, syms, original_anno_type)
        };
        let is_array = ctx.typ(value_ret_type).has_tag(TypeTag::ARRAY);
        let same = {
            self.types.is_same_type(ctx, syms, expected_type, value_ret_type)
        };
        if !(is_array && same) {
            self.log.error_at(pos, errors::invalid_repeatable_annotation_value_return(DiagnosticArg::from_display(crate::comp::resolve::symbol_display_name(ctx, container_tsym)), TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, value_ret_type)), TypeArg::placeholder(crate::comp::attr::type_display_of(ctx, expected_type))).into());
            fatal_error = true;
        }
        Ok(if fatal_error { None } else { Some(container_value_symbol) })
    }

    /// Java: private <T> T makeContainerAnnotation(List<T> toBeReplaced,
    /// AnnotationContext<T> ctx, Symbol sym, boolean isTypeParam)
    /// (Annotate.java:1062-1080)
    /// Run the declaration-level @Repeatable validation once, after the
    /// use-position diagnostics of process_repeated_annotations. The @Repeatable
    /// diagnostic position is the annotation on the annotation type declaration.
    fn validate_repeatable_declaration(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        orig_anno_type: TypeId,
        attr: &mut crate::comp::attr::Attr,
    ) -> Result<(), CompletionFailure> {
        let Some(decl) = ctx.typ(orig_anno_type).tsym else {
            return Ok(());
        };
        if !self.validated_repeatable_decls.borrow_mut().insert(decl) {
            return Ok(());
        }
        let Some(metadata) = ctx.sym(decl).get_annotation_type_metadata().cloned() else {
            return Ok(());
        };
        let repeatable = AnnotationTypeMetadata::get_repeatable_shared(
            &metadata,
            ctx,
            syms,
            Some(&mut *attr),
        )?;
        let Some(repeatable) = repeatable else {
            return Ok(());
        };
        let pos = self
            .repeatable_annotation_position(ctx, syms, decl)
            .unwrap_or(crate::util::position::NOPOS);
        self.chk
            .validate_repeatable(ctx, syms, pos, decl, repeatable, Some(&mut *attr));
        Ok(())
    }

    fn repeatable_annotation_position(
        &self,
        ctx: &CompilerContext,
        syms: &Symtab,
        decl: SymbolId,
    ) -> Option<i32> {
        let env = self.type_envs.get(decl)?;
        let tree = env.tree?;
        let TreeKind::ClassDecl { mods, .. } = &ctx.tree(tree).kind else {
            return None;
        };
        let Some(mods) = *mods else {
            return None;
        };
        let TreeKind::Modifiers { annotations, .. } = &ctx.tree(mods).kind else {
            return None;
        };
        let repeatable_tsym = ctx.typ(syms.repeatable_type).tsym?;
        for annotation in annotations.iter() {
            if ctx
                .tree(annotation)
                .ty
                .and_then(|ty| ctx.typ(ty).tsym)
                == Some(repeatable_tsym)
            {
                return Some(ctx.tree(annotation).pos);
            }
        }
        None
    }

    #[allow(clippy::collapsible_if)]
    #[allow(clippy::too_many_arguments)]
    fn make_container_annotation(&self, ctx: &mut CompilerContext, syms: &crate::code::symtab::Symtab, to_be_replaced: &[AttributeId], context: &AnnotatedContext, sym: SymbolId, is_type_param: bool, attr: &mut crate::comp::attr::Attr) -> Result<Option<AttributeId>, CompletionFailure> {
        let valid_repeated = self.process_repeated_annotations(ctx, syms, to_be_replaced, context, sym, is_type_param, attr)?;
        let orig_anno_type = ctx
            .attr(to_be_replaced[0])
            .ty
            .expect("repeated annotation type");
        // Java: Check.validateRepeatable runs for the annotation type
        // declaration after the repeated uses have been processed.
        self.validate_repeatable_declaration(ctx, syms, orig_anno_type, attr)?;
        let Some(valid_repeated) = valid_repeated else { return Ok(None) };
        let repeated_type = ctx.attr(valid_repeated).ty.expect("repeated annotation type");
        let repeated_tsym = ctx.typ(repeated_type).tsym.expect("repeated annotation symbol");
        if let Some((_, manual)) = context.annotated.iter().find(|(tsym, _)| *tsym == repeated_tsym) {
            if let Some(first) = manual.first() {
                let first_pos = context.pos.get(first).copied().unwrap_or(crate::util::position::NOPOS);
                let first_ty = ctx.attr(*first).ty.expect("manual container type");
                let first_tsym = ctx.typ(first_ty).tsym.expect("manual container symbol");
                self.log.error_at(first_pos, errors::invalid_repeatable_annotation_repeated_and_container_present(SymbolArg::placeholder(crate::comp::resolve::symbol_display_name(ctx, first_tsym))).into());
            }
        }
        Ok(Some(valid_repeated))
    }
}

// ---------------------------------------------------------------------------
// Shared helpers for comp/annotate.rs (env/type utilities).
// ---------------------------------------------------------------------------

/// The source file of an Env (Java: env.toplevel.sourcefile).
fn env_source_file(
    ctx: &CompilerContext,
    env: &Rc<Env<AttrContext>>,
) -> Option<Rc<dyn javac_tools::tools::java_file_object::JavaFileObject>> {
    let toplevel = env.toplevel?;
    match &ctx.tree(toplevel).kind {
        TreeKind::CompilationUnit { sourcefile, .. } => sourcefile.clone(),
        _ => None,
    }
}

/// Java: MethodSymbol.getReturnType() for a method type.
fn return_type_of(ctx: &CompilerContext, method_type: TypeId) -> TypeId {
    match &ctx.typ(method_type).kind {
        crate::code::type_::TypeKindData::MethodType { restype, .. }
        | crate::code::type_::TypeKindData::PartiallyInferredMethodType { restype, .. } => {
            restype.expect("method return type")
        }
        crate::code::type_::TypeKindData::ForAll { qtype, .. } => {
            return_type_of(ctx, qtype.expect("for-all qtype"))
        }
        // Java: ErrorType.getReturnType() returns this.
        crate::code::type_::TypeKindData::ErrorType { .. } => method_type,
        _ => panic!("ClassCastException: MethodType expected"),
    }
}

/// Java: Type.getOriginalType().
fn original_type_of(ctx: &CompilerContext, ty: TypeId) -> TypeId {
    match &ctx.typ(ty).kind {
        crate::code::type_::TypeKindData::ErrorType { original_type, .. } => original_type.unwrap_or(ty),
        _ => ty,
    }
}

// ---------------------------------------------------------------------------
// Type annotation second-stage processing (Annotate.java:1084-1250)
// ---------------------------------------------------------------------------

/// Java: private class Annotate.TypeAnnotate extends TreeScanner
/// (Annotate.java:1074-1180).
pub struct TypeAnnotate<'a> {
    services: Rc<AnnotateServices>,
    env: Rc<Env<AttrContext>>,
    sym: SymbolId,
    defer_pos: Option<i32>,
    syms: &'a crate::code::symtab::Symtab,
    attr: &'a mut crate::comp::attr::Attr,
}

tree_scanner_visitor_delegate!(TypeAnnotate<'a>, 'a);

impl TreeScannerVisitor for TypeAnnotate<'_> {
    fn visit_annotated_type(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, underlying_type) = match &ctx.tree(tree).kind {
            TreeKind::AnnotatedType { annotations, underlying_type } => (annotations.clone(), *underlying_type),
            _ => panic!("expected an annotated type"),
        };
        self.services.enter_type_annotations(
            ctx,
            self.syms,
            &annotations,
            &self.env,
            self.sym,
            self.defer_pos,
            false,
            &mut *self.attr,
        );
        self.scan(ctx, underlying_type);
    }

    fn visit_type_parameter(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, bounds) = match &ctx.tree(tree).kind {
            TreeKind::TypeParameter { annotations, bounds, .. } => (annotations.clone(), bounds.clone()),
            _ => panic!("expected a type parameter"),
        };
        self.services.enter_type_annotations(ctx, self.syms, &annotations, &self.env, self.sym, self.defer_pos, true, &mut *self.attr);
        self.scan_list(ctx, &bounds);
    }

    fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, dim_annotations, elemtype, elems) = match &ctx.tree(tree).kind {
            TreeKind::NewArray { annotations, dim_annotations, elemtype, elems, .. } => {
                (annotations.clone(), dim_annotations.clone(), *elemtype, (*elems).clone())
            }
            _ => panic!("expected a new array"),
        };
        self.services.enter_type_annotations(ctx, self.syms, &annotations, &self.env, self.sym, self.defer_pos, false, &mut *self.attr);
        for dim_annos in dim_annotations.iter() {
            self.services.enter_type_annotations(ctx, self.syms, &dim_annos, &self.env, self.sym, self.defer_pos, false, &mut *self.attr);
        }
        self.scan(ctx, elemtype);
        if let Some(elems) = elems {
            self.scan_list(ctx, &elems);
        }
    }

    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, restype, typarams, recvparam, params, thrown, default_value) = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl { mods, restype, typarams, recvparam, params, thrown, default_value, .. } => {
                (*mods, *restype, typarams.clone(), *recvparam, params.clone(), thrown.clone(), *default_value)
            }
            _ => panic!("expected a method declaration"),
        };
        self.scan(ctx, mods);
        self.scan(ctx, restype);
        self.scan_list(ctx, &typarams);
        self.scan(ctx, recvparam);
        self.scan_list(ctx, &params);
        self.scan_list(ctx, &thrown);
        self.scan(ctx, default_value);
        // Do not annotate the body, just the signature.
    }

    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, vartype, init) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl { mods, vartype, init, .. } => (*mods, *vartype, *init),
            _ => panic!("expected a variable declaration"),
        };
        let prev_pos = self.defer_pos;
        self.defer_pos = Some(ctx.tree(tree).pos);
        if ctx.sym(self.sym).kind == Kind::VAR {
            self.scan(ctx, mods);
            self.scan(ctx, vartype);
        }
        self.scan(ctx, init);
        self.defer_pos = prev_pos;
    }

    fn visit_binding_pattern(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {
        // type binding pattern types are annotated separately (Annotate.java:1145-1149)
    }

    fn visit_class_def(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {
        // a classdef declared within a method is visited separately later
    }

    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (encl, typeargs, clazz, args, def) = match &ctx.tree(tree).kind {
            TreeKind::NewClass { encl, typeargs, clazz, args, def, .. } => {
                (*encl, typeargs.clone(), *clazz, args.clone(), *def)
            }
            _ => panic!("expected a new class"),
        };
        self.scan(ctx, encl);
        self.scan_list(ctx, &typeargs);
        if def.is_none() {
            self.scan(ctx, clazz);
        }
        self.scan_list(ctx, &args);
    }
}

impl AnnotateServices {
    /// Java: public void enterTypeAnnotations(List<JCAnnotation> annotations,
    /// Env<AttrContext> env, Symbol s, DiagnosticPosition deferPos,
    /// boolean isTypeParam) (Annotate.java:1020-1036)
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::manual_map)]
    pub fn enter_type_annotations(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        annotations: &List<TreeId>,
        env: &Rc<Env<AttrContext>>,
        s: SymbolId,
        defer_pos: Option<i32>,
        is_type_param: bool,
        attr: &mut crate::comp::attr::Attr) {
        let source = env_source_file(ctx, env);
        let prev = self.log.use_source(source);
        let prev_lint_pos = match defer_pos {
            Some(defer_pos) => Some(self.deferred_lint_handler.set_pos(Rc::new(
                SimpleDiagnosticPosition::new(defer_pos),
            ))),
            None => None,
        };
        let _ = self.annotate_now(ctx, syms, s, annotations, env, true, is_type_param, attr);
        if let Some(prev_lint_pos) = prev_lint_pos {
            self.deferred_lint_handler.set_pos_state(prev_lint_pos);
        }
        self.log.use_source(prev);
    }

}


impl AnnotateServices {
    /// Java: public List<TypeCompound> fromAnnotations(List<JCAnnotation> annotations)
    /// (Annotate.java:146-158)
    pub fn from_annotations(&self, ctx: &CompilerContext, annotations: &List<TreeId>) -> List<AttributeId> {
        if annotations.is_empty() {
            return List::nil();
        }
        let mut buf: List<AttributeId> = List::nil();
        for anno in annotations.iter() {
            let attribute = match &ctx.tree(anno).kind {
                TreeKind::Annotation { attribute, .. } => *attribute,
                _ => panic!("expected an annotation"),
            };
            let attribute = attribute.expect("AssertionError: anno.attribute is null");
            buf = buf.append(attribute);
        }
        buf
    }

    /// The Rc<Self> used by the queued closures (set at construction).
    fn self_rc(&self) -> Rc<AnnotateServices> {
        self.self_ref
            .borrow()
            .upgrade()
            .expect("AnnotateServices self reference")
    }

    /// Java: public void queueScanTreeAndTypeAnnotate(JCTree tree,
    /// Env<AttrContext> env, Symbol sym, DiagnosticPosition deferPos)
    /// (Annotate.java:1041-1049)
    pub fn queue_scan_tree_and_type_annotate(&self, tree: TreeId, env: &Rc<Env<AttrContext>>, sym: SymbolId, defer_pos: i32) {
        let services = self.self_rc();
        let env = Rc::clone(env);
        self.queues.borrow_mut().q.push_back(Box::new(move |ctx: &mut CompilerContext, syms: &crate::code::symtab::Symtab, attr: &mut crate::comp::attr::Attr| {
            let mut annotator = TypeAnnotate {
                services: Rc::clone(&services),
                env: Rc::clone(&env),
                sym,
                defer_pos: Some(defer_pos),
                syms,
                attr,
            };
            annotator.scan(ctx, Some(tree));
            Ok(())
        }));
    }

    /// Java: public void annotateTypeSecondStage(JCTree tree,
    /// List<JCAnnotation> annotations, Type storeAt) (Annotate.java:1052-1062)
    pub fn annotate_type_second_stage(&self, tree: TreeId, annotations: &List<TreeId>, store_at: TypeId) {
        let services = self.self_rc();
        let annotations = annotations.clone();
        let _ = tree;
        self.queues.borrow_mut().types_q.push_back(Box::new(move |ctx: &mut CompilerContext, _syms: &crate::code::symtab::Symtab, _attr: &mut crate::comp::attr::Attr| {
            let compounds = services.from_annotations(ctx, &annotations);
            assert!(
                annotations.length() == compounds.length(),
                "AssertionError: annotations size mismatch"
            );
            let current = ctx.typ(store_at).metadata.clone().unwrap_or_else(
                crate::code::type_metadata::TypeMetadata::empty,
            );
            let entry = crate::code::type_metadata::Entry::Annotations(
                crate::code::type_metadata::Annotations::new(compounds),
            );
            ctx.typ_mut(store_at).metadata = Some(current.combine(entry));
            Ok(())
        }));
    }

    /// Java: public void annotateTypeParameterSecondStage(JCTree tree,
    /// List<JCAnnotation> annotations) (Annotate.java:1064-1070)
    pub fn annotate_type_parameter_second_stage(&self, tree: TreeId, annotations: &List<TreeId>) {
        let services = self.self_rc();
        let annotations = annotations.clone();
        let _ = tree;
        self.queues.borrow_mut().types_q.push_back(Box::new(move |ctx: &mut CompilerContext, _syms: &crate::code::symtab::Symtab, _attr: &mut crate::comp::attr::Attr| {
            let compounds = services.from_annotations(ctx, &annotations);
            assert!(
                annotations.length() == compounds.length(),
                "AssertionError: annotations size mismatch"
            );
            Ok(())
        }));
    }
}

// ---------------------------------------------------------------------------
// Annotation type completer (Annotate.java:1186-1312)
// ---------------------------------------------------------------------------

/// Java: public class Annotate.AnnotationTypeVisitor extends TreeScanner
/// (Annotate.java:1240-1295).
pub struct AnnotationTypeVisitor<'a> {
    services: Rc<AnnotateServices>,
    env: Option<Rc<Env<AttrContext>>>,
    target: Option<AttributeId>,
    repeatable: Option<AttributeId>,
    syms: &'a crate::code::symtab::Symtab,
    /// Port adapter (A3k): the Attr of the caller that triggered the source
    /// annotation completion. Upstream Annotate reuses the single Java Attr
    /// instance; the ported service object can only be re-borrowed when no
    /// caller Attr is in flight, so the visitor carries the caller's Attr.
    attr: Option<&'a mut crate::comp::attr::Attr>,
}

tree_scanner_visitor_delegate!(AnnotationTypeVisitor<'a>, 'a);

impl TreeScannerVisitor for AnnotationTypeVisitor<'_> {
    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sym = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { sym, .. } => *sym,
            _ => None,
        };
        let prev_env = self.env.clone();
        self.env = sym.and_then(|sym| self.services.type_envs.get(sym));
        let mods = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { mods, .. } => *mods,
            _ => None,
        };
        self.scan(ctx, mods);
        self.env = prev_env;
    }

    fn visit_annotation(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let annotation_type_tree = match &ctx.tree(tree).kind {
            TreeKind::Annotation { annotation_type, .. } => annotation_type.expect("annotation type"),
            _ => panic!("expected an annotation"),
        };
        let services = Rc::clone(&self.services);
        let syms = self.syms;
        let current_env = self.env.clone();
        let t = match ctx.tree(annotation_type_tree).ty {
            Some(t) => t,
            None => {
                let env = current_env.clone().expect("annotation type visitor env");
                // attribType: the caller's Attr when the completion was triggered
                // from Attr, the Annotate service's Attr otherwise.
                let attributed = match self.attr.as_deref_mut() {
                    Some(attr) => attr.attrib_type(ctx, syms, annotation_type_tree, &env),
                    None => {
                        let mut attr = services.attr.borrow_mut();
                        attr.attrib_type(ctx, syms, annotation_type_tree, &env)
                    }
                };
                let annotation_type = syms.annotation_type;
                let checked = {
                    let chk = Rc::clone(&services.chk);
                    chk.check_type_basic(ctx, syms, ctx.tree(annotation_type_tree).pos, attributed, annotation_type)
                };
                ctx.tree_mut(annotation_type_tree).ty = Some(checked);
                checked
            }
        };
        let annotation_target_type = syms.annotation_target_type;
        let repeatable_type = syms.repeatable_type;
        if t == annotation_target_type {
            let env = current_env.expect("env");
            let result = match self.attr.as_deref_mut() {
                Some(attr) => services
                    .attribute_annotation(ctx, syms, tree, annotation_target_type, &env, attr)
                    .ok(),
                None => {
                    let attr_rc = Rc::clone(&services.attr);
                    // Java: Annotate reuses the single Attr instance while an
                    // enclosing Attr call is running. The Rust port makes that
                    // Attr explicit: every Attr-inside metadata caller passes
                    // it through SourceCompleter::complete, so self.attr is
                    // None only for an Attr-free completion. The local RefMut
                    // is then the sole borrow of the shared Attr.
                    let mut attr = attr_rc.borrow_mut();
                    services
                        .attribute_annotation(ctx, syms, tree, annotation_target_type, &env, &mut attr)
                        .ok()
                }
            };
            self.target = result;
        } else if t == repeatable_type {
            let env = current_env.expect("env");
            let result = match self.attr.as_deref_mut() {
                Some(attr) => services
                    .attribute_annotation(ctx, syms, tree, repeatable_type, &env, attr)
                    .ok(),
                None => {
                    let attr_rc = Rc::clone(&services.attr);
                    // Same contract as the annotation_target_type branch:
                    // self.attr is None only outside an Attr call, so this
                    // local RefMut is the sole borrow.
                    let mut attr = attr_rc.borrow_mut();
                    services
                        .attribute_annotation(ctx, syms, tree, repeatable_type, &env, &mut attr)
                        .ok()
                }
            };
            self.repeatable = result;
        }
    }
}

/// Java: private AnnotationTypeCompleter theSourceCompleter
/// (Annotate.java:1189-1196).
pub struct SourceCompleter {
    services: Rc<AnnotateServices>,
}

impl SourceCompleter {
    pub fn new(services: Rc<AnnotateServices>) -> SourceCompleter {
        SourceCompleter { services }
    }
}

impl AnnotationTypeCompleter for SourceCompleter {
    fn complete(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        sym: SymbolId,
        attr: Option<&mut crate::comp::attr::Attr>,
    ) -> Result<(), CompletionFailure> {
        let env = self.services.type_envs.get(sym);
        if let Some(env) = env {
            self.services.attribute_annotation_type(ctx, syms, &env, attr)?;
        }
        Ok(())
    }
}

impl AnnotateServices {
    /// Java: public AnnotationTypeCompleter annotationTypeSourceCompleter()
    /// (Annotate.java:1199-1201)
    pub fn annotation_type_source_completer(self: &Rc<AnnotateServices>) -> Rc<std::cell::RefCell<dyn AnnotationTypeCompleter>> {
        Rc::new(std::cell::RefCell::new(SourceCompleter::new(Rc::clone(self))))
    }

    /// Java: private void attributeAnnotationType(Env<AttrContext> env)
    /// (Annotate.java:1203-1220)
    fn attribute_annotation_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        env: &Rc<Env<AttrContext>>,
        attr: Option<&mut crate::comp::attr::Attr>,
    ) -> Result<(), CompletionFailure> {
        let tree = env.tree.expect("annotation type env tree");
        let sym = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { sym, .. } => sym.expect("annotation type symbol"),
            _ => panic!("expected a class declaration"),
        };
        let is_annotation_type = (ctx.sym(sym).flags_field & flags::ANNOTATION as i64) != 0;
        assert!(is_annotation_type, "trying to annotation type complete a non-annotation type");
        let source = env_source_file(ctx, env);
        let prev = self.log.use_source(source);
        let mut visitor = AnnotationTypeVisitor {
            services: self.self_rc(),
            env: Some(Rc::clone(env)),
            target: None,
            repeatable: None,
            syms,
            attr,
        };
        visitor.scan(ctx, Some(tree));
        let target = visitor.target;
        let repeatable = visitor.repeatable;
        if let Some(metadata) = ctx.sym(sym).get_annotation_type_metadata().cloned() {
            metadata.set_repeatable(repeatable);
            metadata.set_target(target);
        }
        self.log.use_source(prev);
        Ok(())
    }
}

// Java: public static interface Annotate.AnnotationTypeCompleter is defined in the
// -3.5a section above (AnnotationTypeCompleter); this file now contains the
// full Annotate body.

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::code::types::Types;
    use crate::comp::attr::Attr;
    use crate::comp::check::Check;
    use crate::comp::const_fold::ConstFold;
    use javac_tools::error::Error as ToolsError;
    use javac_tools::tools::java_file_manager::{
        FileObjectRef, JavaFileManager, JavaFileObjectRef, Location, LocationRef,
    };
    use javac_tools::tools::java_file_object::Kind;
    use javac_tools::tools::option_checker::OptionChecker;
    use crate::code::dependencies::{Dependencies, DummyDependencies};
    use crate::comp::enter::Enter;
    use crate::comp::member_enter::MemberEnter;
    use crate::comp::type_enter::TypeEnter;
    use crate::comp::type_envs::TypeEnvs;
    use crate::jvm::target::Target;
    use crate::util::context::Context;
    use crate::util::javac_messages::JavacMessages;
    use crate::util::options::Options;

struct UnusedFileManager;

    impl OptionChecker for UnusedFileManager {
        fn is_supported_option(&self, _option: &javac_strings::JavaString) -> i32 {
            0
        }
    }

    impl JavaFileManager for UnusedFileManager {
        fn get_class_loader(
            &self,
            _location: &dyn Location,
        ) -> Option<Rc<dyn javac_tools::jdk::ClassLoader>> {
            None
        }
        fn list(
            &self,
            _location: &dyn Location,
            _package_name: &javac_strings::JavaString,
            _kinds: &[Kind],
            _recurse: bool,
        ) -> Result<Vec<JavaFileObjectRef>, ToolsError> {
            Ok(Vec::new())
        }
        fn infer_binary_name(
            &self,
            _location: &dyn Location,
            _file: &JavaFileObjectRef,
        ) -> Option<javac_strings::JavaString> {
            None
        }
        fn is_same_file(&self, a: &FileObjectRef, b: &FileObjectRef) -> bool {
            a.get_name().to_string_lossy() == b.get_name().to_string_lossy()
        }
        fn handle_option(
            &self,
            _current: &javac_strings::JavaString,
            _remaining: &mut dyn Iterator<Item = javac_strings::JavaString>,
        ) -> bool {
            false
        }
        fn has_location(&self, _location: &dyn Location) -> bool {
            false
        }
        fn get_java_file_for_input(
            &self,
            _location: &dyn Location,
            _class_name: &javac_strings::JavaString,
            _kind: Kind,
        ) -> Result<Option<JavaFileObjectRef>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn get_java_file_for_output(
            &self,
            _location: &dyn Location,
            _class_name: &javac_strings::JavaString,
            _kind: Kind,
            _sibling: Option<&FileObjectRef>,
        ) -> Result<JavaFileObjectRef, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn get_file_for_input(
            &self,
            _location: &dyn Location,
            _package_name: &javac_strings::JavaString,
            _relative_name: &javac_strings::JavaString,
        ) -> Result<Option<FileObjectRef>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn get_file_for_output(
            &self,
            _location: &dyn Location,
            _package_name: &javac_strings::JavaString,
            _relative_name: &javac_strings::JavaString,
            _sibling: Option<&FileObjectRef>,
        ) -> Result<FileObjectRef, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn flush(&self) -> Result<(), ToolsError> {
            Ok(())
        }
        fn close(&self) -> Result<(), ToolsError> {
            Ok(())
        }
        fn list_locations_for_modules(
            &self,
            _location: &dyn Location,
        ) -> Result<Vec<Vec<LocationRef>>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
    }

    #[allow(dead_code)]
    struct AnnotateHarness {
        ctx: CompilerContext,
        annotate: Rc<Annotate>,
        make: Rc<TreeMaker>,
        syms: Rc<Symtab>,
        types: Rc<Types>,
        log: Rc<Log>,
    }

    fn harness() -> AnnotateHarness {

    #[test]
    fn instance_returns_the_registered_service() {
        let harness = harness();
        let mut context = Context::new();
        context.put::<Annotate>(Rc::clone(&harness.annotate));
        let instance = Annotate::instance(&mut context);
        assert!(Rc::ptr_eq(&instance, &harness.annotate));
    }

    #[test]
    #[should_panic(expected = "Annotate is not registered in the Context")]
    fn instance_requires_registration() {
        let mut context = Context::new();
        let _ = Annotate::instance(&mut context);
    }
        let mut ctx = CompilerContext::new();
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = JavacMessages::instance(&mut context);
        let diags = Rc::new(crate::util::jc_diagnostic::Factory::new(messages, "compiler"));
        let options = Options::instance(&mut context);
        let lint = Lint::instance(&mut context);
        let syms = Rc::new(Symtab::new(
            &mut ctx,
            Rc::clone(&names),
            Target::DEFAULT,
            Source::JDK17,
        ));
        let types = Rc::new(Types::with_names(Rc::clone(&names)));
        let deferred = Rc::new(DeferredLintHandler::new());
        let check = Rc::new(Check::new(
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&diags),
            Source::JDK17,
            '$',
            Rc::clone(&types),
            Rc::clone(&lint),
            Rc::clone(&deferred),
            Rc::clone(&syms),
        ));
        crate::comp::check::attach_self(&check);
        let make = Rc::new(TreeMaker::new(Some(Rc::clone(&names))));
        let type_envs = Rc::new(TypeEnvs::new());
        let dependencies: Rc<dyn Dependencies> = Rc::new(DummyDependencies);
        let manager: Rc<dyn JavaFileManager> = Rc::new(UnusedFileManager);
        let todo = crate::comp::todo::Todo::instance(&mut context);
        let enter = Rc::new(Enter::new(
            Rc::clone(&log),
            Rc::clone(&names),
            Rc::clone(&diags),
            Rc::clone(&check),
            Rc::clone(&types),
            None,
            manager,
            &options,
            Rc::clone(&todo),
            Rc::clone(&type_envs)
        ));
        let annotate_cell: Rc<RefCell<Option<Rc<Annotate>>>> = Rc::new(RefCell::new(None));
        let member_enter = Rc::new(MemberEnter::new(
            Rc::clone(&enter),
            Rc::clone(&log),
            Rc::clone(&check),
            Rc::clone(&types),
            Rc::clone(&deferred),
            Rc::clone(&annotate_cell)
        ));
        let type_enter = Rc::new(RefCell::new(TypeEnter::new(
            Rc::clone(&names),
            Rc::clone(&enter),
            Rc::clone(&member_enter),
            Rc::clone(&log),
            Rc::clone(&check),
            Rc::clone(&types),
            Rc::clone(&syms),
            Rc::clone(&make),
            Rc::clone(&deferred),
            None,
            Rc::clone(&type_envs),
            Rc::clone(&dependencies),
            Rc::clone(&annotate_cell),
            Rc::clone(&todo)
        )));
        type_enter.borrow().set_self_ref(Rc::downgrade(&type_enter));
        let basic_handler = Rc::clone(&check.basic_handler);
        let infer = crate::comp::infer::Infer::new(
            &mut ctx,
            Feature::GRAPH_INFERENCE.allowed_in_source(Source::JDK17),
            Rc::clone(&diags),
            Rc::clone(&names),
        );
        let attr = Rc::new(RefCell::new(Attr::new(
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&check),
            Rc::clone(&member_enter),
            Rc::clone(&type_enter),
            Rc::clone(&make),
            Rc::clone(&enter),
            Rc::clone(&types),
            Rc::clone(&diags),
            Rc::clone(&deferred),
            Rc::clone(&type_envs),
            Rc::clone(&dependencies),
            basic_handler,
            Rc::clone(&syms),
            Rc::clone(&infer),
            Source::JDK17,
            &options,
            crate::code::preview::Preview::instance(&mut context),
        )));
        let resolve = Rc::new(crate::comp::resolve::Resolve::new(
            &mut ctx,
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&types),
            Rc::clone(&diags),
            Source::JDK17,
            Rc::clone(&options),
            None,
        ));
        let cfolder = Rc::new(ConstFold::new());
        let annotate = Rc::new(Annotate::new(
            &mut ctx,
            names,
            Rc::clone(&log),
            Rc::clone(&syms),
            Rc::clone(&types),
            check,
            attr,
            cfolder,
            deferred,
            enter,
            lint,
            resolve,
            Rc::clone(&make),
            Rc::clone(&type_envs),
            Rc::clone(&diags),
            Source::JDK17,
        ));
        *annotate_cell.borrow_mut() = Some(Rc::clone(&annotate));
        AnnotateHarness {
            ctx,
            annotate,
            make,
            syms,
            types,
            log,
        }
    }

    #[test]
    fn flush_respects_the_annotation_block_and_runs_queues_in_upstream_order() {
        let mut h = harness();
        let order: Rc<RefCell<Vec<&str>>> = Rc::new(RefCell::new(Vec::new()));
        let push = |name: &'static str| {
            let order = Rc::clone(&order);
            Box::new(
                move |_ctx: &mut CompilerContext, _syms: &crate::code::symtab::Symtab, _attr: &mut crate::comp::attr::Attr| {
                    order.borrow_mut().push(name);
                    Ok(())
                },
            ) as AnnotateRunnable
        };
        h.annotate.normal(push("normal"));
        h.annotate.type_annotation(push("types"));
        h.annotate.after_types(push("afterTypes"));
        h.annotate.validate(push("validate"));
        h.annotate.flush(&mut h.ctx, &&*h.syms).expect("flush while blocked");
        assert!(order.borrow().is_empty(), "blocked annotations must not flush");
        h.annotate.unblock_annotations(&mut h.ctx, &&*h.syms).expect("unblock");
        assert_eq!(
            order.borrow().clone(),
            vec!["normal", "types", "afterTypes", "validate"]
        );
        assert!(!h.annotate.annotations_blocked());
    }

    #[test]
    fn translate_attribute_builds_annotation_and_array_trees() {
        let mut h = harness();
        let bool_type = h.syms.boolean_type;
        let string_type = h.syms.string_type;
        let constant = h.ctx.alloc_attribute(Attribute::new_constant(
            Some(string_type),
            crate::util::constants::ConstantValue::String(javac_strings::JavaString::from("value")),
        ));
        let compound = h.ctx.alloc_attribute(Attribute::new_compound(Some(bool_type), List::nil()));
        let compound_tree = {
            let make = &*h.make;
            make.at(0);
            make.translate_attribute(&mut h.ctx, &&*h.syms, compound)
        };
        assert!(h.ctx.tree(compound_tree).has_tag(Tag::ANNOTATION));
        let array = h
            .ctx
            .alloc_attribute(Attribute::new_array(Some(string_type), vec![constant]));
        let array_tree = {
            let make = &*h.make;
            make.at(0);
            make.translate_attribute(&mut h.ctx, &&*h.syms, array)
        };
        assert!(h.ctx.tree(array_tree).has_tag(Tag::NEWARRAY));
        assert_eq!(h.log.nerrors.get(), 0);
    }
}
