//! Type annotations helper: separates declaration annotations from type
//! annotations, organizes the type annotation positions of a class tree and
//! validates the type annotation applicability rules.
//!
//!
//! * Java class -> Rust struct; instance(Context) is the Java static factory.
//! * the Java TypeAnnotationPositions/TypeAnnotationValidator inner TreeScanners
//!   map to explicit visitor structs in this module (registered section 9).
//! * the annotation metadata access uses the ported Symbol/Type metadata API
//!   (get_annotation_type_metadata/get_annotation_mirrors and the TypeMetadata
//!   entries).

use std::rc::Rc;

use crate::code::attribute::AttributeKind;
use crate::code::symtab::Symtab;
use crate::code::type_annotation_position::TypePathEntry;
use crate::compiler_context::CompilerContext;
use crate::tree::tree_scanner::TreeScannerVisitor;
use crate::ids::{AttributeId, SymbolId};
use crate::ids::TreeId;
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
    pub chk: Rc<crate::comp::check::Check>,
    /// Java: final Attr attr (used by validateTypeAnnotationsSignatures).
    pub attr: Rc<std::cell::RefCell<crate::comp::attr::Attr>>,
    /// Java: final Annotate annotate (afterTypes/validate hooks).
    pub annotate: Rc<crate::comp::annotate::Annotate>,
    /// Java: final Types types
    pub types: Rc<crate::code::types::Types>,
    /// Adapter: the shared symbol table cell (the signatures hook runs later).
    pub syms: Rc<std::cell::RefCell<Symtab>>,
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
        chk: Rc<crate::comp::check::Check>,
        attr: Rc<std::cell::RefCell<crate::comp::attr::Attr>>,
        annotate: Rc<crate::comp::annotate::Annotate>,
        types: Rc<crate::code::types::Types>,
        syms: Rc<std::cell::RefCell<Symtab>>,
        diags: Rc<Factory>,
    ) -> TypeAnnotations {
        TypeAnnotations {
            names,
            log,
            chk,
            attr,
            annotate,
            types,
            syms,
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
        syms: &Symtab,
        tsym: SymbolId,
        attr: Option<&mut crate::comp::attr::Attr>,
    ) -> Option<List<AttributeId>> {
        let metadata = ctx.sym(tsym).get_annotation_type_metadata().cloned()?;
        let at_target =
            crate::comp::annotate::AnnotationTypeMetadata::get_target_shared(&metadata, ctx, syms, attr)
                .ok()??;
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
        syms: &Symtab,
        a: AttributeId,
        s: SymbolId,
        attr: Option<&mut crate::comp::attr::Attr>,
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
        let targets = self.annotation_targets(ctx, syms, tsym, attr);
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
impl TypeAnnotations {
    /// Java: public void organizeTypeAnnotationsBodies(JCClassDecl tree)
    /// (TypeAnnotations.java:146-150).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the class declaration.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: TypeAnnotations.java:146-150.
    pub fn organize_type_annotations_bodies(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &Symtab,
        tree: crate::ids::TreeId,
    ) {
        let mut positions = TypeAnnotationPositions::new(self, syms, false);
        crate::tree::tree_scanner::TreeScannerVisitor::scan(&mut positions, ctx, Some(tree));
    }
    /// Java: public void organizeTypeAnnotationsSignatures(final Env<AttrContext> env,
    /// final JCClassDecl tree) (TypeAnnotations.java:120-129).
    ///
    /// # Arguments
    ///   - self: the shared TypeAnnotations handle;
    ///   - ctx: the compiler context;
    ///   - env: the attribution environment;
    ///   - tree: the class declaration.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - the env toplevel must be a compilation unit.
    ///
    /// Upstream: TypeAnnotations.java:120-129.
    pub fn organize_type_annotations_signatures(
        self: &Rc<TypeAnnotations>,
        env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        tree: crate::ids::TreeId,
    ) {
        let ta = Rc::clone(self);
        let env = Rc::clone(env);
        self.annotate.after_types(Box::new(
            move |ctx: &mut crate::compiler_context::CompilerContext,
                  syms: &crate::code::symtab::Symtab,
                  attr: &mut crate::comp::attr::Attr| {
                let toplevel = env.toplevel.expect("env toplevel");
                let sourcefile = match &ctx.tree(toplevel).kind {
                    crate::tree::jc_tree::TreeKind::CompilationUnit { sourcefile, .. } => {
                        sourcefile.clone()
                    }
                    _ => None,
                };
                let prev = crate::util::abstract_log::AbstractLog::use_source(
                    &mut *ta.log.borrow_mut(),
                    sourcefile,
                );
                let mut positions = TypeAnnotationPositions::new_with_attr(&ta, syms, true, attr);
                crate::tree::tree_scanner::TreeScannerVisitor::scan(
                    &mut positions,
                    ctx,
                    Some(tree),
                );
                drop(syms);
                let result: Result<(), crate::code::symbol::CompletionFailure> = Ok(());
                crate::util::abstract_log::AbstractLog::use_source(
                    &mut *ta.log.borrow_mut(),
                    prev,
                );
                result
            },
        ));
    }

    /// Java: public void validateTypeAnnotationsSignatures(final Env<AttrContext> env,
    /// final JCClassDecl tree) (TypeAnnotations.java:131-143).
    ///
    /// # Arguments
    ///   - self: the shared TypeAnnotations handle;
    ///   - env: the attribution environment;
    ///   - tree: the class declaration.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - the env toplevel must be a compilation unit.
    ///
    /// Upstream: TypeAnnotations.java:131-143.
    pub fn validate_type_annotations_signatures(
        self: &Rc<TypeAnnotations>,
        env: &Rc<crate::comp::env::Env<crate::comp::attr_context::AttrContext>>,
        tree: crate::ids::TreeId,
    ) {
        let ta = Rc::clone(self);
        let env = Rc::clone(env);
        self.annotate.validate(Box::new(
            move |ctx: &mut crate::compiler_context::CompilerContext,
                  syms: &crate::code::symtab::Symtab,
                  attr: &mut crate::comp::attr::Attr| {
                let toplevel = env.toplevel.expect("env toplevel");
                let sourcefile = match &ctx.tree(toplevel).kind {
                    crate::tree::jc_tree::TreeKind::CompilationUnit { sourcefile, .. } => {
                        sourcefile.clone()
                    }
                    _ => None,
                };
                let prev = crate::util::abstract_log::AbstractLog::use_source(
                    &mut *ta.log.borrow_mut(),
                    sourcefile,
                );
                {
                    attr.validate_type_annotations(ctx, syms, tree, true);
                }
                crate::util::abstract_log::AbstractLog::use_source(
                    &mut *ta.log.borrow_mut(),
                    prev,
                );
                Ok(())
            },
        ));
    }
    }

/// Java: private class TypeAnnotationPositions extends TreeScanner
/// (TypeAnnotations.java:257-1450).
pub struct TypeAnnotationPositions<'a, 'b> {
    /// Java: the enclosing TypeAnnotations
    pub type_annotations: &'a TypeAnnotations,
    /// Adapter: the explicit syms service (ADR 0001)
    pub syms: &'b Symtab,
    /// Java: private final boolean sigOnly
    pub sig_only: bool,
    /// Java: private List<JCTree> frames = List.nil() (head is the last)
    pub frames: Vec<crate::ids::TreeId>,
    /// Java: private JCLambda currentLambda = null
    pub current_lambda: Option<crate::ids::TreeId>,
    /// Java: private boolean isInClass = false
    pub is_in_class: bool,
    /// Port adapter: the caller's Attr while running inside Annotate.flush, so
    /// annotation-target completion does not re-borrow the shared Attr service.
    pub attr: Option<&'a mut crate::comp::attr::Attr>,
}

impl<'a, 'b> TypeAnnotationPositions<'a, 'b> {
    /// Java: TypeAnnotationPositions(boolean sigOnly) (TypeAnnotations.java:262-264).
    pub fn new(
        type_annotations: &'a TypeAnnotations,
        syms: &'b Symtab,
        sig_only: bool,
    ) -> TypeAnnotationPositions<'a, 'b> {
        TypeAnnotationPositions {
            type_annotations,
            syms,
            sig_only,
            frames: Vec::new(),
            current_lambda: None,
            is_in_class: false,
            attr: None,
        }
    }

    /// Construct with the caller's Attr (Annotate.flush path).
    pub fn new_with_attr(
        type_annotations: &'a TypeAnnotations,
        syms: &'b Symtab,
        sig_only: bool,
        attr: &'a mut crate::comp::attr::Attr,
    ) -> TypeAnnotationPositions<'a, 'b> {
        TypeAnnotationPositions {
            type_annotations,
            syms,
            sig_only,
            frames: Vec::new(),
            current_lambda: None,
            is_in_class: false,
            attr: Some(attr),
        }
    }

    /// Java: protected void push(JCTree t)
    fn push(&mut self, tree: crate::ids::TreeId) {
        self.frames.push(tree);
    }

    /// Java: protected JCTree pop()
    fn pop(&mut self) -> crate::ids::TreeId {
        self.frames.pop().expect("AssertionError: empty frame stack")
    }

    /// Java: private JCTree peek2()
    fn peek2(&self) -> crate::ids::TreeId {
        let len = self.frames.len();
        self.frames[len - 2]
    }
}

impl<'a, 'b> crate::tree::tree_scanner::TreeScannerVisitor for TypeAnnotationPositions<'a, 'b> {
    /// Java: public void scan(JCTree tree) (TypeAnnotations.java:287-295): push/pop
    /// the frame stack around the TreeScanner navigation.
    fn scan(&mut self, ctx: &mut crate::compiler_context::CompilerContext, tree: Option<crate::ids::TreeId>) {
        let Some(tree) = tree else {
            return;
        };
        self.push(tree);
        crate::tree::jc_tree::accept(ctx, tree, self);
        self.pop();
    }

    /// Java: public void visitClassDef(JCClassDecl tree) (TypeAnnotations.java:1120-1136).
    #[allow(clippy::collapsible_if)]
    fn visit_class_def(&mut self, ctx: &mut crate::compiler_context::CompilerContext, tree: crate::ids::TreeId) {
        if self.is_in_class {
            return;
        }
        self.is_in_class = true;
        let (mods, typarams, extending, implementing, defs, sym) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::ClassDecl {
                mods,
                typarams,
                extending,
                implementing,
                defs,
                sym,
                ..
            } => (*mods, typarams.clone(), *extending, implementing.clone(), defs.clone(), *sym),
            _ => return,
        };
        if self.sig_only {
            self.scan(ctx, mods);
            self.scan_list(ctx, &typarams);
            self.scan(ctx, extending);
            self.scan_list(ctx, &implementing);
        }
        self.scan_list(ctx, &defs);
        if let Some(sym) = sym {
            if ctx.sym(sym).is_record() {
                let components = match &ctx.sym(sym).kind_data {
                    crate::code::symbol::SymbolKind::ClassSymbol { record_components, .. } => {
                        record_components.clone()
                    }
                    _ => List::nil(),
                };
                let mut current = components;
                while current.non_empty() {
                    let component = *current.head();
                    current = current.tail();
                    let accessor = match &ctx.sym(component).kind_data {
                        crate::code::symbol::SymbolKind::RecordComponent { accessor_meth, .. } => {
                            *accessor_meth
                        }
                        _ => None,
                    };
                    if let Some(accessor) = accessor {
                        self.scan_list(ctx, &List::of(accessor));
                    }
                }
            }
        }
    }

    /// Java: public void visitMethodDef(JCMethodDecl tree) (TypeAnnotations.java:1142-1205).
    fn visit_method_def(&mut self, ctx: &mut crate::compiler_context::CompilerContext, tree: crate::ids::TreeId) {
        let (mods, restype, typarams, recvparam, params, thrown, body, default_value, sym, _) =
            match &ctx.tree(tree).kind {
                crate::tree::jc_tree::TreeKind::MethodDecl {
                    mods,
                    restype,
                    typarams,
                    recvparam,
                    params,
                    thrown,
                    body,
                    default_value,
                    sym,
                    completes_normally,
                    ..
                } => (
                    *mods,
                    *restype,
                    typarams.clone(),
                    *recvparam,
                    params.clone(),
                    thrown.clone(),
                    *body,
                    *default_value,
                    *sym,
                    *completes_normally,
                ),
                _ => return,
            };
        let sym = match sym {
            Some(sym) => sym,
            None => panic!("AssertionError: Visiting tree node before memberEnter"),
        };
        if self.sig_only {
            let mods_annotations = match mods.map(|mods| &ctx.tree(mods).kind) {
                Some(crate::tree::jc_tree::TreeKind::Modifiers { annotations, .. }) => annotations.clone(),
                _ => List::nil(),
            };
            if !mods_annotations.is_empty() {
                let is_constructor = ctx.sym(sym).is_constructor(&self.type_annotations.names);
                if is_constructor {
                    let pos = crate::code::type_annotation_position::TypeAnnotationPosition::method_return(
                        List::nil(),
                        ctx.tree(tree).pos,
                        None,
                    );
                    self.separate_annotations_kinds(ctx, tree, None, sym, pos);
                } else if let Some(restype) = restype {
                    let pos = crate::code::type_annotation_position::TypeAnnotationPosition::method_return(
                        List::nil(),
                        ctx.tree(restype).pos,
                        None,
                    );
                    let ret = self
                        .type_annotations
                        .types
                        .get_return_type_public(ctx, ctx.sym(sym).ty.expect("method type"));
                    self.separate_annotations_kinds(ctx, restype, Some(ret), sym, pos);
                }
            }
            if let Some(recvparam) = recvparam {
                let recv_sym = match &ctx.tree(recvparam).kind {
                    crate::tree::jc_tree::TreeKind::VariableDecl { sym, .. } => *sym,
                    _ => None,
                };
                if let Some(recv_sym) = recv_sym {
                    let recv_mods_annotations = match &ctx.tree(recvparam).kind {
                        crate::tree::jc_tree::TreeKind::VariableDecl { mods, .. } => {
                            self.modifiers_annotations(ctx, *mods)
                        }
                        _ => List::nil(),
                    };
                    if !recv_mods_annotations.is_empty() {
                        let vartype = match &ctx.tree(recvparam).kind {
                            crate::tree::jc_tree::TreeKind::VariableDecl { vartype: Some(vartype), .. } => *vartype,
                            _ => return,
                        };
                        let pos = crate::code::type_annotation_position::TypeAnnotationPosition::method_receiver(
                            List::nil(),
                            ctx.tree(vartype).pos,
                            None,
                        );
                        self.push(recvparam);
                        let recv_type = ctx.sym(recv_sym).ty.expect("receiver type");
                        self.separate_annotations_kinds(ctx, vartype, Some(recv_type), recv_sym, pos);
                        self.pop();
                    }
                }
            }
            let mut index = 0i32;
            let mut params_current = params.clone();
            while params_current.non_empty() {
                let param = *params_current.head();
                params_current = params_current.tail();
                let param_mods_annotations = match &ctx.tree(param).kind {
                    crate::tree::jc_tree::TreeKind::VariableDecl { mods, .. } => self.modifiers_annotations(ctx, *mods),
                    _ => List::nil(),
                };
                if !param_mods_annotations.is_empty() {
                    let param_sym = match &ctx.tree(param).kind {
                        crate::tree::jc_tree::TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
                        _ => return,
                    };
                    let vartype = match &ctx.tree(param).kind {
                        crate::tree::jc_tree::TreeKind::VariableDecl { vartype: Some(vartype), .. } => *vartype,
                        _ => return,
                    };
                    let pos = crate::code::type_annotation_position::TypeAnnotationPosition::method_parameter(
                        List::nil(),
                        index,
                        ctx.tree(vartype).pos,
                        None,
                    );
                    self.push(param);
                    let param_type = ctx.sym(param_sym).ty.expect("parameter type");
                    self.separate_annotations_kinds(ctx, vartype, Some(param_type), param_sym, pos);
                    self.pop();
                }
                index += 1;
            }
        }
        if self.sig_only {
            self.scan(ctx, mods);
            self.scan(ctx, restype);
            self.scan_list(ctx, &typarams);
            self.scan(ctx, recvparam);
            self.scan_list(ctx, &params);
            self.scan_list(ctx, &thrown);
        } else {
            self.scan(ctx, default_value);
            self.scan(ctx, body);
        }
    }


    /// Java: public void visitLambda(JCLambda tree) (TypeAnnotations.java:1210-1250).
    #[allow(clippy::collapsible_if)]
    fn visit_lambda(&mut self, ctx: &mut crate::compiler_context::CompilerContext, tree: crate::ids::TreeId) {
        let prev_lambda = self.current_lambda;
        self.current_lambda = Some(tree);
        let params = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::Lambda { params, .. } => params.clone(),
            _ => List::nil(),
        };
        let mut index = 0i32;
        let mut current = params.clone();
        while current.non_empty() {
            let param = *current.head();
            current = current.tail();
            let annotations = match &ctx.tree(param).kind {
                crate::tree::jc_tree::TreeKind::VariableDecl { mods, .. } => self.modifiers_annotations(ctx, *mods),
                _ => List::nil(),
            };
            if !annotations.is_empty() {
                let (param_sym, vartype, declared_using_var) = match &ctx.tree(param).kind {
                    crate::tree::jc_tree::TreeKind::VariableDecl { sym: Some(sym), vartype: Some(vartype), declared_using_var, .. } => (*sym, *vartype, *declared_using_var),
                    _ => {
                        index += 1;
                        continue;
                    }
                };
                let pos = crate::code::type_annotation_position::TypeAnnotationPosition::method_parameter(
                    List::nil(),
                    index,
                    ctx.tree(vartype).pos,
                    Some(tree),
                );
                self.push(param);
                if !declared_using_var {
                    let param_type = ctx.sym(param_sym).ty.expect("lambda parameter type");
                    self.separate_annotations_kinds(ctx, vartype, Some(param_type), param_sym, pos);
                }
                self.pop();
            }
            index += 1;
        }
        let body = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::Lambda { body, .. } => *body,
            _ => None,
        };
        self.scan(ctx, body);
        self.scan_list(ctx, &params);
        self.current_lambda = prev_lambda;
    }

    /// Java: public void visitVarDef(JCVariableDecl tree) (TypeAnnotations.java:1255-1330).
    #[allow(clippy::collapsible_if)]
    fn visit_var_def(&mut self, ctx: &mut crate::compiler_context::CompilerContext, tree: crate::ids::TreeId) {
        use crate::code::symbol::ElementKind;
        let (mods, vartype, init, sym, declared_using_var) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::VariableDecl {
                mods,
                vartype,
                init,
                sym,
                declared_using_var,
                ..
            } => (*mods, *vartype, *init, *sym, *declared_using_var),
            _ => return,
        };
        let annotations = self.modifiers_annotations(ctx, mods);
        let sym = match (annotations.is_empty(), sym) {
            (true, _) => None,
            (false, Some(sym)) => Some(sym),
            (false, None) => panic!("AssertionError: Visiting tree node before memberEnter"),
        };
        if let Some(sym) = sym {
            let element_kind = ctx.sym(sym).get_kind(ctx);
            if element_kind == ElementKind::PARAMETER {
                // Parameters are handled in visitMethodDef or visitLambda.
            } else if element_kind == ElementKind::FIELD {
                if self.sig_only {
                    let pos = crate::code::type_annotation_position::TypeAnnotationPosition::field(
                        List::nil(),
                        ctx.tree(tree).pos,
                        None,
                    );
                    if let Some(vartype) = vartype {
                        let field_type = ctx.sym(sym).ty.expect("field type");
                        self.separate_annotations_kinds(ctx, vartype, Some(field_type), sym, pos);
                    }
                }
            } else if element_kind == ElementKind::LOCAL_VARIABLE {
                let pos = crate::code::type_annotation_position::TypeAnnotationPosition::local_variable(
                    List::nil(),
                    ctx.tree(tree).pos,
                    self.current_lambda,
                );
                if !declared_using_var {
                    if let Some(vartype) = vartype {
                        let local_type = ctx.sym(sym).ty.expect("local variable type");
                        self.separate_annotations_kinds(ctx, vartype, Some(local_type), sym, pos);
                    }
                }
            } else if element_kind == ElementKind::BINDING_VARIABLE {
                let pos = crate::code::type_annotation_position::TypeAnnotationPosition::local_variable(
                    List::nil(),
                    ctx.tree(tree).pos,
                    self.current_lambda,
                );
                if let Some(vartype) = vartype {
                    let binding_type = ctx.sym(sym).ty.expect("binding variable type");
                    self.separate_annotations_kinds(ctx, vartype, Some(binding_type), sym, pos);
                }
            } else if element_kind == ElementKind::EXCEPTION_PARAMETER {
                let pos = crate::code::type_annotation_position::TypeAnnotationPosition::exception_parameter(
                    List::nil(),
                    ctx.tree(tree).pos,
                    self.current_lambda,
                );
                if let Some(vartype) = vartype {
                    let exception_type = ctx.sym(sym).ty.expect("exception parameter type");
                    self.separate_annotations_kinds(ctx, vartype, Some(exception_type), sym, pos);
                }
            } else if element_kind == ElementKind::RESOURCE_VARIABLE {
                let pos = crate::code::type_annotation_position::TypeAnnotationPosition::resource_variable(
                    List::nil(),
                    ctx.tree(tree).pos,
                    self.current_lambda,
                );
                if let Some(vartype) = vartype {
                    let resource_type = ctx.sym(sym).ty.expect("resource variable type");
                    self.separate_annotations_kinds(ctx, vartype, Some(resource_type), sym, pos);
                }
            } else if element_kind == ElementKind::ENUM_CONSTANT {
                // No type annotations can occur here.
            } else {
                panic!("AssertionError: Unhandled variable kind");
            }
        }
        self.scan(ctx, mods);
        self.scan(ctx, vartype);
        if !self.sig_only {
            self.scan(ctx, init);
        }
        if self.sig_only {
            if let Some(sym) = sym {
                let flags_value = ctx.sym(sym).flags_field;
                if ctx.sym(sym).get_kind(ctx) == ElementKind::FIELD
                    && (flags_value & crate::code::flags::RECORD) != 0
                {
                    let owner = ctx.sym(sym).owner.expect("record component owner");
                    let components = match &ctx.sym(owner).kind_data {
                        crate::code::symbol::SymbolKind::ClassSymbol { record_components, .. } => {
                            record_components.clone()
                        }
                        _ => List::nil(),
                    };
                    let mut current = components;
                    while current.non_empty() {
                        let component = *current.head();
                        current = current.tail();
                        let matches = match &ctx.sym(component).kind_data {
                            crate::code::symbol::SymbolKind::RecordComponent { pos, .. } => *pos == ctx.tree(tree).pos,
                            _ => false,
                        };
                        if matches {
                            let raw_type_attributes = ctx.sym(sym).get_raw_type_attributes();
                            ctx.sym_mut(component).set_type_attributes(raw_type_attributes);
                            ctx.sym_mut(component).ty = ctx.sym(sym).ty;
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Java: public void visitBlock(JCBlock tree) (TypeAnnotations.java:1332-1338).
    fn visit_block(&mut self, ctx: &mut crate::compiler_context::CompilerContext, tree: crate::ids::TreeId) {
        if !self.sig_only {
            let stats = match &ctx.tree(tree).kind {
                crate::tree::jc_tree::TreeKind::Block { stats, .. } => stats.clone(),
                _ => List::nil(),
            };
            self.scan_list(ctx, &stats);
        }
    }

    /// Java: public void visitAnnotatedType(JCAnnotatedType tree) (TypeAnnotations.java:1340-1348).
    fn visit_annotated_type(&mut self, ctx: &mut crate::compiler_context::CompilerContext, tree: crate::ids::TreeId) {
        self.push(tree);
        let (annotations, underlying) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::AnnotatedType { annotations, underlying_type } => {
                (annotations.clone(), *underlying_type)
            }
            _ => (List::nil(), None),
        };
        self.find_position(ctx, tree, tree, &annotations);
        self.pop();
        self.scan(ctx, underlying);
    }

    /// Java: public void visitTypeParameter(JCTypeParameter tree) (TypeAnnotations.java:1350-1354).
    fn visit_type_parameter(&mut self, ctx: &mut crate::compiler_context::CompilerContext, tree: crate::ids::TreeId) {
        let (annotations, bounds) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::TypeParameter { annotations, bounds, .. } => {
                (annotations.clone(), bounds.clone())
            }
            _ => (List::nil(), List::nil()),
        };
        let frame = self.peek2();
        self.find_position(ctx, tree, frame, &annotations);
        self.scan_list(ctx, &bounds);
    }


    /// Java: public void visitNewClass(JCNewClass tree) (TypeAnnotations.java:1387-1403).
    fn visit_new_class(&mut self, ctx: &mut crate::compiler_context::CompilerContext, tree: crate::ids::TreeId) {
        let (encl, typeargs, clazz, args, def) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::NewClass { encl, typeargs, clazz, args, def, .. } => {
                (*encl, typeargs.clone(), *clazz, args.clone(), *def)
            }
            _ => return,
        };
        if let Some(def_tree) = def {
            let def_sym = match &ctx.tree(def_tree).kind {
                crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. } => Some(*sym),
                _ => None,
            };
            if def_sym.is_some() {
                self.propagate_new_class_annotations_to_owner(ctx, tree);
            }
        }
        self.scan(ctx, encl);
        self.scan_list(ctx, &typeargs);
        if def.is_none() {
            self.scan(ctx, clazz);
        }
        self.scan_list(ctx, &args);
    }

    /// Java: public void visitNewArray(JCNewArray tree) (TypeAnnotations.java:1405-1460).
    #[allow(clippy::collapsible_if)]
    fn visit_new_array(&mut self, ctx: &mut crate::compiler_context::CompilerContext, tree: crate::ids::TreeId) {
        let (elemtype, annotations, dim_annotations, elems) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::NewArray { elemtype, annotations, dim_annotations, elems, .. } => {
                (*elemtype, annotations.clone(), dim_annotations.clone(), elems.clone())
            }
            _ => return,
        };
        self.find_position(ctx, tree, tree, &annotations);
        let mut depth: List<TypePathEntry> = List::nil();
        let mut dim_current = dim_annotations.clone();
        let mut dim_index = 0i32;
        while dim_current.non_empty() {
            let dim_annos = dim_current.head().clone();
            dim_current = dim_current.tail();
            let mut location: List<TypePathEntry> = List::nil();
            if dim_index != 0 {
                depth = depth.append(TypePathEntry::ARRAY);
                location = depth.clone();
            }
            let pos = crate::code::type_annotation_position::TypeAnnotationPosition::new_obj(
                location,
                ctx.tree(tree).pos,
                self.current_lambda,
            );
            self.set_type_annotation_pos(ctx, &dim_annos, pos);
            dim_index += 1;
        }
        let mut elem = elemtype;
        depth = depth.append(TypePathEntry::ARRAY);
        while let Some(elem_tree) = elem {
            let elem_kind = ctx.tree(elem_tree).kind.clone();
            match &elem_kind {
                crate::tree::jc_tree::TreeKind::AnnotatedType { annotations, underlying_type } => {
                    let location_v = self.locate_nested_types(ctx, ctx.tree(elem_tree).ty.unwrap_or_else(|| panic!("annotated element type")));
                    let mut location = depth.clone();
                    let mut location_current = location_v;
                    while location_current.non_empty() {
                        location = location.append(*location_current.head());
                        location_current = location_current.tail();
                    }
                    let pos = crate::code::type_annotation_position::TypeAnnotationPosition::new_obj(
                        location,
                        ctx.tree(tree).pos,
                        self.current_lambda,
                    );
                    self.set_type_annotation_pos(ctx, annotations, pos);
                    elem = *underlying_type;
                }
                crate::tree::jc_tree::TreeKind::ArrayTypeTree { elemtype: Some(inner) } => {
                    depth = depth.append(TypePathEntry::ARRAY);
                    elem = Some(*inner);
                }
                crate::tree::jc_tree::TreeKind::FieldAccess { selected: Some(selected), .. } => {
                    elem = Some(*selected);
                }
                _ => break,
            }
        }
        if let Some(elems) = elems {
            self.scan_list(ctx, &elems);
        }
    }
}

impl<'a, 'b> TypeAnnotationPositions<'a, 'b> {
    /// Java: private void findPosition(JCTree tree, JCTree frame,
    /// List<JCAnnotation> annotations) (TypeAnnotations.java:1462-1472).
    fn find_position(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: crate::ids::TreeId,
        frame: crate::ids::TreeId,
        annotations: &List<crate::ids::TreeId>,
    ) {
        if !annotations.is_empty() {
            // The ported frame stack is a Vec pushed root-first; ResolveFrame
            // walks the path from the innermost annotated tree outwards.
            let mut frames = self.frames.clone();
            frames.reverse();
            let p = self.resolve_frame(ctx, tree, frame, &frames, self.current_lambda, 0, List::nil());
            self.set_type_annotation_pos(ctx, annotations, p);
        }
    }

    /// Java: private void setTypeAnnotationPos(List<JCAnnotation> annotations,
    /// TypeAnnotationPosition position) (TypeAnnotations.java:1474-1482).
    #[allow(clippy::collapsible_if)]
    fn set_type_annotation_pos(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        annotations: &List<crate::ids::TreeId>,
        position: crate::code::type_annotation_position::TypeAnnotationPosition,
    ) {
        let mut current = annotations.clone();
        while current.non_empty() {
            let anno = *current.head();
            current = current.tail();
            let attribute = match &ctx.tree(anno).kind {
                crate::tree::jc_tree::TreeKind::Annotation { attribute, .. } => *attribute,
                _ => None,
            };
            if let Some(attribute) = attribute {
                if let crate::code::attribute::AttributeKind::TypeCompound { position: slot, .. } =
                    &mut ctx.attr_mut(attribute).kind
                {
                    *slot = Some(position.clone());
                }
            }
        }
    }

    /// Java: private ListBuffer<TypePathEntry> locateNestedTypes(Type type,
    /// ListBuffer<TypePathEntry> depth) (TypeAnnotations.java:1085-1098).
    fn locate_nested_types(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        ty: crate::ids::TypeId,
    ) -> List<TypePathEntry> {
        let mut depth: List<TypePathEntry> = List::nil();
        let mut encl = ctx.typ(ty).get_enclosing_type();
        while let Some(encl_ty) = encl {
            if ctx.typ(encl_ty).has_tag(crate::code::type_tag::TypeTag::NONE)
                || ctx.typ(encl_ty).has_tag(crate::code::type_tag::TypeTag::ERROR)
            {
                break;
            }
            depth = depth.prepend(TypePathEntry::INNER_TYPE);
            encl = ctx.typ(encl_ty).get_enclosing_type();
        }
        depth
    }

    /// Java: private int methodParamIndex(List<JCTree> path, JCTree param)
    /// (TypeAnnotations.java:1100-1118).
    fn method_param_index(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        path: &[crate::ids::TreeId],
        param: crate::ids::TreeId,
    ) -> i32 {
        let mut index = path.len() as isize - 1;
        while index >= 0 {
            let node = path[index as usize];
            match &ctx.tree(node).kind {
                crate::tree::jc_tree::TreeKind::MethodDecl { params, .. } => {
                    return param_index_in_list(params, param);
                }
                crate::tree::jc_tree::TreeKind::Lambda { params, .. } => {
                    return param_index_in_list(params, param);
                }
                _ => {}
            }
            index -= 1;
        }
        panic!("AssertionError: methodParamIndex expected to find method or lambda for param");
    }
}

/// Adapter: the index of param inside a list.
fn param_index_in_list(params: &List<crate::ids::TreeId>, param: crate::ids::TreeId) -> i32 {
    let mut index = 0i32;
    let mut current = params.clone();
    while current.non_empty() {
        if *current.head() == param {
            return index;
        }
        index += 1;
        current = current.tail();
    }
    -1
}

crate::tree::tree_scanner::tree_scanner_visitor_delegate!(TypeAnnotationPositions<'a, 'b>, 'a, 'b);
impl<'a, 'b> TypeAnnotationPositions<'a, 'b> {
    /// Java: private void separateAnnotationsKinds(JCTree typetree, Type type,
    /// Symbol sym, TypeAnnotationPosition pos) (TypeAnnotations.java:305-420).
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::collapsible_if)]
    fn separate_annotations_kinds(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        typetree: crate::ids::TreeId,
        ty: Option<crate::ids::TypeId>,
        sym: crate::ids::SymbolId,
        pos: crate::code::type_annotation_position::TypeAnnotationPosition,
    ) {
        let all_annotations = ctx.sym(sym).get_raw_attributes();
        let mut decl_annos: List<crate::ids::AttributeId> = List::nil();
        let mut type_annos: List<crate::ids::AttributeId> = List::nil();
        let mut only_type_annos: List<crate::ids::AttributeId> = List::nil();
        let mut current = all_annotations;
        while current.non_empty() {
            let a = *current.head();
            current = current.tail();
            let target = self
                .type_annotations
                .annotation_target_type(ctx, self.syms, a, sym, self.attr.as_deref_mut());
            match target {
                AnnotationType::DECLARATION => {
                    decl_annos = decl_annos.append(a);
                }
                AnnotationType::BOTH => {
                    decl_annos = decl_annos.append(a);
                    let ta = self.to_type_compound(ctx, a, pos.clone());
                    type_annos = type_annos.append(ta);
                }
                AnnotationType::TYPE => {
                    let ta = self.to_type_compound(ctx, a, pos.clone());
                    type_annos = type_annos.append(ta);
                    only_type_annos = only_type_annos.append(ta);
                }
                AnnotationType::NONE => {}
            }
        }
        if type_annos.is_empty() {
            return;
        }
        ctx.sym_mut(sym).reset_annotations();
        ctx.sym_mut(sym).set_declaration_attributes(decl_annos);
        let type_annotations = type_annos.clone();
        let ty = match ty {
            Some(ty) => ty,
            None => {
                let enclosing = ctx.sym(sym).owner;
                let enclosing_ty = enclosing.and_then(|enclosing| ctx.sym(enclosing).ty);
                let enclosing_ty = match enclosing_ty {
                    Some(ty) => ty,
                    None => return,
                };
                let _ = self.type_with_annotations(
                    ctx,
                    typetree,
                    enclosing_ty,
                    &type_annotations,
                    &type_annotations,
                    pos.clone(),
                );
                ctx.sym_mut(sym).append_unique_type_attributes(type_annotations);
                return;
            }
        };
        let annotated = self.type_with_annotations(
            ctx,
            typetree,
            ty,
            &type_annotations,
            &only_type_annos,
            pos.clone(),
        );
        let element_kind = ctx.sym(sym).get_kind(ctx);
        if element_kind == crate::code::symbol::ElementKind::METHOD {
            if let Some(method_ty) = ctx.sym(sym).ty {
                if let crate::code::type_::TypeKindData::MethodType { restype, .. } =
                    &mut ctx.typ_mut(method_ty).kind
                {
                    *restype = Some(annotated);
                }
            }
        } else if element_kind == crate::code::symbol::ElementKind::PARAMETER
            && self.current_lambda.is_none()
        {
            ctx.sym_mut(sym).ty = Some(annotated);
            if ctx.sym(sym).name == self.type_annotations.names._this {
                let owner = ctx.sym(sym).owner.expect("receiver owner");
                if let Some(owner_ty) = ctx.sym(owner).ty {
                    if let crate::code::type_::TypeKindData::MethodType { recvtype, .. } =
                        &mut ctx.typ_mut(owner_ty).kind
                    {
                        *recvtype = Some(annotated);
                    }
                }
            } else {
                let owner = ctx.sym(sym).owner.expect("method owner");
                let owner_ty = ctx.sym(owner).ty.expect("method owner type");
                let method_params = match &ctx.sym(owner).kind_data {
                    crate::code::symbol::SymbolKind::MethodSymbol { params, .. } => params.clone(),
                    _ => List::nil(),
                };
                let old_args = match &ctx.typ(owner_ty).kind {
                    crate::code::type_::TypeKindData::MethodType { argtypes, .. } => argtypes.clone(),
                    _ => List::nil(),
                };
                let mut new_args: List<crate::ids::TypeId> = List::nil();
                let mut params_current = method_params;
                let mut old_args_current = old_args;
                // Java: for (Symbol p : methodSymbol.params) {
                //           Type t = p == sym ? annotated : oldArgs.head;
                //           oldArgs = oldArgs.tail;
                //           newArgs.append(t);
                //       }
                // Every formal consumes one old argument, including the one that
                // is replaced by the annotated type.
                while params_current.non_empty() && old_args_current.non_empty() {
                    let param = *params_current.head();
                    params_current = params_current.tail();
                    let old_arg = *old_args_current.head();
                    old_args_current = old_args_current.tail();
                    new_args = new_args.append(if param == sym { annotated } else { old_arg });
                }
                // Java: methType.argtypes = newArgs.toList();
                while old_args_current.non_empty() {
                    new_args = new_args.append(*old_args_current.head());
                    old_args_current = old_args_current.tail();
                }
                if let crate::code::type_::TypeKindData::MethodType { argtypes, .. } =
                    &mut ctx.typ_mut(owner_ty).kind
                {
                    *argtypes = new_args;
                }
            }
        } else {
            ctx.sym_mut(sym).ty = Some(annotated);
        }
        ctx.sym_mut(sym).append_unique_type_attributes(type_annotations.clone());
        let element_kind = ctx.sym(sym).get_kind(ctx);
        if element_kind == crate::code::symbol::ElementKind::PARAMETER
            || element_kind == crate::code::symbol::ElementKind::LOCAL_VARIABLE
            || element_kind == crate::code::symbol::ElementKind::RESOURCE_VARIABLE
            || element_kind == crate::code::symbol::ElementKind::EXCEPTION_PARAMETER
            || element_kind == crate::code::symbol::ElementKind::BINDING_VARIABLE
        {
            self.append_type_annotations_to_owner(ctx, sym, &type_annotations);
        }
    }

    /// Java: private void appendTypeAnnotationsToOwner(Symbol sym,
    /// List<Attribute.TypeCompound> typeAnnotations) (TypeAnnotations.java:422-445).
    fn append_type_annotations_to_owner(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        sym: crate::ids::SymbolId,
        type_annotations: &List<crate::ids::AttributeId>,
    ) {
        let owner = match ctx.sym(sym).owner {
            Some(owner) => owner,
            None => return,
        };
        let owner_flags = ctx.sym(owner).flags_field;
        if (owner_flags & crate::code::flags::BLOCK as i64) != 0 {
            let class_sym = match ctx.sym(owner).owner {
                Some(class_sym) => class_sym,
                None => return,
            };
            if (owner_flags & crate::code::flags::STATIC as i64) != 0 {
                ctx.sym_mut(class_sym).append_class_init_type_attributes(type_annotations.clone());
            } else {
                ctx.sym_mut(class_sym).append_init_type_attributes(type_annotations.clone());
            }
        } else {
            ctx.sym_mut(owner).append_unique_type_attributes(type_annotations.clone());
        }
    }

    /// Java: private Attribute.TypeCompound toTypeCompound(Attribute.Compound a,
    /// TypeAnnotationPosition p) (TypeAnnotations.java:1010-1015).
    fn to_type_compound(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        a: crate::ids::AttributeId,
        position: crate::code::type_annotation_position::TypeAnnotationPosition,
    ) -> crate::ids::AttributeId {
        let (ty, values, synthesized) = match &ctx.attr(a).kind {
            crate::code::attribute::AttributeKind::Compound { values, synthesized, .. } => {
                (ctx.attr(a).ty, values.clone(), *synthesized)
            }
            _ => (ctx.attr(a).ty, List::nil(), false),
        };
        ctx.alloc_attribute(crate::code::attribute::Attribute::new(
            ty,
            crate::code::attribute::AttributeKind::TypeCompound {
                values,
                position: Some(position),
                synthesized,
            },
        ))
    }
}
impl<'a, 'b> TypeAnnotationPositions<'a, 'b> {
    /// Java: private Type typeWithAnnotations(JCTree typetree, Type type,
    /// List<Attribute.TypeCompound> annotations, List<Attribute.TypeCompound>
    /// onlyTypeAnnotations, TypeAnnotationPosition pos) (TypeAnnotations.java:447-560).
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    #[allow(clippy::collapsible_if)]
    fn type_with_annotations(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        typetree: crate::ids::TreeId,
        ty: crate::ids::TypeId,
        annotations: &List<crate::ids::AttributeId>,
        only_type_annotations: &List<crate::ids::AttributeId>,
        mut pos: crate::code::type_annotation_position::TypeAnnotationPosition,
    ) -> crate::ids::TypeId {
        use crate::code::type_::TypeKindData;
        if annotations.is_empty() {
            return ty;
        }
        for tc in annotations.iter() {
            let matches = match &ctx.attr(tc).kind {
                crate::code::attribute::AttributeKind::TypeCompound { position, .. } => {
                    position.as_ref().map(|p| p.pos == pos.pos).unwrap_or(false)
                }
                _ => false,
            };
            assert!(matches, "AssertionError: all annotations share the same position");
        }
        if ctx.typ(ty).has_tag(crate::code::type_tag::TypeTag::ARRAY) {
            return self.rewrite_array_type(ctx, ty, annotations, pos);
        }
        if ctx.typ(ty).has_tag(crate::code::type_tag::TypeTag::TYPEVAR) {
            // Java: type.annotatedType(onlyTypeAnnotations) — cloneWithMetadata
            // installs the baseType() link used by equalsIgnoreMetadata.
            return ctx.annotated_type_id(ty, &only_type_annotations);
        }
        if matches!(&ctx.typ(ty).kind, TypeKindData::UnionClassType { .. }) {
            let first = match &ctx.tree(typetree).kind {
                crate::tree::jc_tree::TreeKind::TypeUnion { alternatives } => {
                    if alternatives.non_empty() { Some(*alternatives.head()) } else { None }
                }
                _ => None,
            };
            if let Some(first) = first {
                let first_ty = ctx.tree(first).ty.expect("union alternative type");
                let res = self.type_with_annotations(
                    ctx,
                    first,
                    first_ty,
                    annotations,
                    only_type_annotations,
                    pos.clone(),
                );
                ctx.tree_mut(first).ty = Some(res);
            }
            return ty;
        }
        // Walk from the outermost type tree down to the actual top-level class.
        let mut encl_ty = Some(ty);
        let mut encl_el = ctx.typ(ty).tsym;
        let mut encl_tr = Some(typetree);
        while let (Some(encl_ty_value), Some(encl_el_value), Some(encl_tr_value)) =
            (encl_ty, encl_el, encl_tr)
        {
            let encl_kind = ctx.sym(encl_el_value).get_kind(ctx);
            if encl_kind == crate::code::symbol::ElementKind::PACKAGE
                || ctx.typ(encl_ty_value).has_tag(crate::code::type_tag::TypeTag::NONE)
                || ctx.typ(encl_ty_value).has_tag(crate::code::type_tag::TypeTag::ERROR)
            {
                break;
            }
            let tr_kind = &ctx.tree(encl_tr_value).kind;
            if let crate::tree::jc_tree::TreeKind::FieldAccess { selected, .. } = tr_kind {
                let next_tr = *selected;
                let next_ty = ctx.typ(encl_ty_value).get_enclosing_type();
                let next_el = ctx.sym(encl_el_value).owner;
                encl_ty = next_ty;
                encl_el = next_el;
                encl_tr = next_tr;
            } else if let crate::tree::jc_tree::TreeKind::TypeApply { clazz, .. } = tr_kind {
                encl_tr = *clazz;
            } else if let crate::tree::jc_tree::TreeKind::AnnotatedType { underlying_type, .. } = tr_kind {
                encl_tr = *underlying_type;
            } else {
                break;
            }
        }
        if let Some(encl_ty_value) = encl_ty {
            if ctx.typ(encl_ty_value).has_tag(crate::code::type_tag::TypeTag::NONE) {
                if !only_type_annotations.is_empty() {
                    let info = if only_type_annotations.length() == 1 {
                        let annotation = *only_type_annotations.head();
                        let display = ctx
                            .attr(annotation)
                            .ty
                            .map(|t| crate::comp::attr::annotation_type_display(ctx, t))
                            .unwrap_or_default();
                        crate::resources::compiler_properties::errors::cant_type_annotate_scoping1(
                            crate::resources::diagnostic_contract::AnnotationArg::placeholder(display),
                        )
                    } else {
                        let mut args: Vec<crate::resources::diagnostic_contract::AnnotationArg> = Vec::new();
                        for annotation in only_type_annotations.iter() {
                            args.push(crate::resources::diagnostic_contract::AnnotationArg::placeholder(
                                ctx.attr(annotation).ty.map(|t| crate::comp::attr::annotation_type_display(ctx, t)).unwrap_or_default(),
                            ));
                        }
                        crate::resources::compiler_properties::errors::cant_type_annotate_scoping(args)
                    };
                    crate::util::abstract_log::AbstractLog::error_at(
                        &mut *self.type_annotations.log.borrow_mut(),
                        ctx.tree(typetree).pos,
                        crate::util::jc_diagnostic::Error::from(info),
                    );
                }
                return ty;
            }
            let mut depth: List<crate::code::type_annotation_position::TypePathEntry> = List::nil();
            let mut top_ty = Some(encl_ty_value);
            let mut top_el = encl_el;
            while let Some(top_ty_value) = top_ty {
                if ctx.typ(top_ty_value).has_tag(crate::code::type_tag::TypeTag::NONE)
                    || ctx.typ(top_ty_value).has_tag(crate::code::type_tag::TypeTag::ERROR)
                {
                    break;
                }
                let encl = ctx.typ(top_ty_value).get_enclosing_type();
                let el = top_el.and_then(|el| ctx.sym(el).owner);
                top_ty = encl;
                top_el = el;
                if let Some(encl_value) = encl {
                    if !ctx.typ(encl_value).has_tag(crate::code::type_tag::TypeTag::NONE) {
                        depth = depth.append(crate::code::type_annotation_position::TypePathEntry::INNER_TYPE);
                    }
                }
            }
            if !depth.is_empty() {
                let mut location = pos.location.clone();
                let mut depth_current = depth;
                while depth_current.non_empty() {
                    location = location.append(*depth_current.head());
                    depth_current = depth_current.tail();
                }
                pos.location = location;
            }
            let ret = self.type_with_annotations_upto(ctx, ty, encl_ty_value, annotations);
            ctx.tree_mut(typetree).ty = Some(ret);
            return ret;
        }
        ty
    }

    /// Java: private Type rewriteArrayType(ArrayType type, List<TypeCompound>
    /// annotations, TypeAnnotationPosition pos) (TypeAnnotations.java:568-620).
    fn rewrite_array_type(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        ty: crate::ids::TypeId,
        annotations: &List<crate::ids::AttributeId>,
        mut pos: crate::code::type_annotation_position::TypeAnnotationPosition,
    ) -> crate::ids::TypeId {
        use crate::code::type_::TypeKindData;
        let (base_elem, base_varargs) = match &ctx.typ(ty).kind {
            TypeKindData::ArrayType { elemtype, varargs } => {
                (elemtype.expect("array element type"), *varargs)
            }
            _ => return ty,
        };
        let mut loc: List<crate::code::type_annotation_position::TypePathEntry> = List::nil();
        let mut tmp_ty = base_elem;
        loc = loc.prepend(crate::code::type_annotation_position::TypePathEntry::ARRAY);
        // The top-level copy of the array type.
        let tsym = ctx.typ(ty).tsym.expect("array type symbol");
        let new_top = ctx.alloc_type(crate::code::type_::Type::new(
            crate::code::type_tag::TypeTag::ARRAY,
            Some(tsym),
            TypeKindData::ArrayType { elemtype: Some(tmp_ty), varargs: base_varargs },
        ));
        let mut tomodify = new_top;
        // Peel the nested array levels, reusing the original element chain.
        while ctx.typ(tmp_ty).has_tag(crate::code::type_tag::TypeTag::ARRAY) {
            let (inner_elem, inner_varargs) = match &ctx.typ(tmp_ty).kind {
                TypeKindData::ArrayType { elemtype, varargs } => {
                    (elemtype.expect("nested array element type"), *varargs)
                }
                _ => panic!("AssertionError: array expected"),
            };
            let arr_sym = ctx.typ(tmp_ty).tsym.expect("nested array symbol");
            let new_arr = ctx.alloc_type(crate::code::type_::Type::new(
                crate::code::type_tag::TypeTag::ARRAY,
                Some(arr_sym),
                TypeKindData::ArrayType { elemtype: Some(inner_elem), varargs: inner_varargs },
            ));
            if let TypeKindData::ArrayType { elemtype, .. } = &mut ctx.typ_mut(tomodify).kind {
                *elemtype = Some(new_arr);
            }
            tomodify = new_arr;
            tmp_ty = inner_elem;
            loc = loc.prepend(crate::code::type_annotation_position::TypePathEntry::ARRAY);
        }
        // Fix the innermost element type: insert the annotations into its metadata.
        let annotations_list = annotations.clone();
        let existing = ctx.typ(tmp_ty).get_annotation_mirrors();
        let combined = if existing.is_empty() {
            annotations_list
        } else {
            let mut result = annotations_list;
            let mut current = existing;
            while current.non_empty() {
                result = result.append(*current.head());
                current = current.tail();
            }
            result
        };
        let base_metadata = ctx
            .typ(tmp_ty)
            .get_metadata()
            .cloned()
            .unwrap_or_else(crate::code::type_metadata::TypeMetadata::empty);
        let without = base_metadata.without(crate::code::type_metadata::EntryKind::ANNOTATIONS);
        let metadata = without.combine(crate::code::type_metadata::Entry::Annotations(
            crate::code::type_metadata::Annotations::new(combined),
        ));
        let new_elem = ctx.clone_type_with_metadata(tmp_ty, metadata);
        if let TypeKindData::ArrayType { elemtype, .. } = &mut ctx.typ_mut(tomodify).kind {
            *elemtype = Some(new_elem);
        }
        pos.location = loc;
        for annotation in annotations.iter() {
            if let crate::code::attribute::AttributeKind::TypeCompound { position, .. } =
                &mut ctx.attr_mut(annotation).kind
            {
                *position = Some(pos.clone());
            }
        }
        new_top
    }

    /// Java: private Type typeWithAnnotations(Type type, Type stopAt,
    /// List<Attribute.TypeCompound> annotations) (TypeAnnotations.java:625-695).
    fn type_with_annotations_upto(
        &self,
        ctx: &mut crate::compiler_context::CompilerContext,
        ty: crate::ids::TypeId,
        stop_at: crate::ids::TypeId,
        annotations: &List<crate::ids::AttributeId>,
    ) -> crate::ids::TypeId {
        use crate::code::type_::TypeKindData;
        if ty == stop_at || ctx.typ(ty).get_enclosing_type().is_none() {
            return ctx.annotated_type_id(ty, annotations);
        }
        let ty_kind = ctx.typ(ty).kind.clone();
        match &ty_kind {
            TypeKindData::ClassType {
                outer_field,
                typarams_field,
                supertype_field,
                interfaces_field,
                all_interfaces_field,
                ..
            } => {
                let outer = outer_field
                    .map(|outer| self.type_with_annotations_upto(ctx, outer, stop_at, annotations));
                let tsym = ctx.typ(ty).tsym.expect("class type symbol");
                let metadata = ctx.typ(ty).get_metadata().cloned();
                let new_ty = ctx.alloc_type(crate::code::type_::Type::new(
                    crate::code::type_tag::TypeTag::CLASS,
                    Some(tsym),
                    TypeKindData::ClassType {
                        outer_field: outer,
                        typarams_field: typarams_field.clone(),
                        allparams_field: None,
                        supertype_field: *supertype_field,
                        interfaces_field: interfaces_field.clone(),
                        all_interfaces_field: all_interfaces_field.clone(),
                    },
                ));
                if let Some(metadata) = metadata {
                    ctx.typ_mut(new_ty).metadata = Some(metadata);
                }
                new_ty
            }
            TypeKindData::ArrayType { elemtype, varargs } => {
                let elem = elemtype.expect("array element type");
                let new_elem = self.type_with_annotations_upto(ctx, elem, stop_at, annotations);
                let tsym = ctx.typ(ty).tsym.expect("array type symbol");
                ctx.alloc_type(crate::code::type_::Type::new(
                    crate::code::type_tag::TypeTag::ARRAY,
                    Some(tsym),
                    TypeKindData::ArrayType { elemtype: Some(new_elem), varargs: *varargs },
                ))
            }
            TypeKindData::WildcardType { .. }
            | TypeKindData::TypeVar { .. }
            | TypeKindData::ErrorType { .. }
            | _ => ctx.annotated_type_id(ty, annotations),
        }
    }
}
impl<'a, 'b> TypeAnnotationPositions<'a, 'b> {
    /// Java: private TypeAnnotationPosition resolveFrame(JCTree tree, JCTree frame,
    /// List<JCTree> path, JCLambda currentLambda, int outer_type_index,
    /// ListBuffer<TypePathEntry> location) (TypeAnnotations.java:700-1120).
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn resolve_frame(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: crate::ids::TreeId,
        frame: crate::ids::TreeId,
        path: &[crate::ids::TreeId],
        current_lambda: Option<crate::ids::TreeId>,
        outer_type_index: i32,
        mut location: List<crate::code::type_annotation_position::TypePathEntry>,
    ) -> crate::code::type_annotation_position::TypeAnnotationPosition {
        use crate::code::type_annotation_position::TypeAnnotationPosition;
        use crate::code::type_annotation_position::TypePathEntry;
        use crate::code::type_annotation_position::TypePathEntryKind;
        use crate::tree::jc_tree::TreeKind;
        let frame_pos = ctx.tree(frame).pos;
        match &ctx.tree(frame).kind {
            TreeKind::TypeCast { .. } => TypeAnnotationPosition::type_cast(
                location,
                outer_type_index,
                frame_pos,
                current_lambda,
            ),
            TreeKind::InstanceOf { .. } => {
                TypeAnnotationPosition::instance_of(location, frame_pos, current_lambda)
            }
            TreeKind::NewClass { typeargs, def, .. } => {
                let def = *def;
                let typeargs = typeargs.clone();
                if let Some(def) = def {
                    let implementing = match &ctx.tree(def).kind {
                        TreeKind::ClassDecl { implementing, .. } => implementing.clone(),
                        _ => List::nil(),
                    };
                    let mut type_index: Option<i32> = None;
                    let mut index = 0i32;
                    let mut current = implementing;
                    while current.non_empty() {
                        if *current.head() == tree {
                            type_index = Some(index);
                        }
                        index += 1;
                        current = current.tail();
                    }
                    match type_index {
                        Some(type_index) => TypeAnnotationPosition::class_extends(
                            location,
                            type_index,
                            frame_pos,
                            current_lambda,
                        ),
                        None => TypeAnnotationPosition::class_extends(
                            location,
                            -1,
                            frame_pos,
                            current_lambda,
                        ),
                    }
                } else {
                    let mut type_index: Option<i32> = None;
                    let mut index = 0i32;
                    let mut current = typeargs;
                    while current.non_empty() {
                        if *current.head() == tree {
                            type_index = Some(index);
                        }
                        index += 1;
                        current = current.tail();
                    }
                    match type_index {
                        Some(type_index) => TypeAnnotationPosition::constructor_invocation_type_arg(
                            location,
                            type_index,
                            frame_pos,
                            current_lambda,
                        ),
                        None => TypeAnnotationPosition::new_obj(location, frame_pos, current_lambda),
                    }
                }
            }
            TreeKind::NewArray { .. } => {
                TypeAnnotationPosition::new_obj(location, frame_pos, current_lambda)
            }
            TreeKind::ClassDecl { extending, implementing, typarams, .. } => {
                let extending = *extending;
                let implementing = implementing.clone();
                let typarams = typarams.clone();
                if extending == Some(tree) {
                    TypeAnnotationPosition::class_extends(location, -1, frame_pos, current_lambda)
                } else {
                    let mut found: Option<(i32, bool)> = None;
                    let mut implementing_index = 0i32;
                    let mut current = implementing;
                    while current.non_empty() {
                        if *current.head() == tree {
                            found = Some((implementing_index, true));
                        }
                        implementing_index += 1;
                        current = current.tail();
                    }
                    if found.is_none() {
                        let mut parameter_index = 0i32;
                        let mut current = typarams;
                        while current.non_empty() {
                            if *current.head() == tree {
                                found = Some((parameter_index, false));
                            }
                            parameter_index += 1;
                            current = current.tail();
                        }
                    }
                    match found {
                        Some((type_index, true)) => TypeAnnotationPosition::class_extends(
                            location,
                            type_index,
                            frame_pos,
                            current_lambda,
                        ),
                        Some((parameter_index, false)) => TypeAnnotationPosition::type_parameter(
                            location,
                            parameter_index,
                            current_lambda,
                        ),
                        None => panic!("AssertionError: Could not determine position of tree within frame"),
                    }
                }
            }
            TreeKind::MethodDecl { thrown, restype, typarams, .. } => {
                let thrown = thrown.clone();
                let restype = *restype;
                let typarams = typarams.clone();
                let mut thrown_index: Option<i32> = None;
                let mut index = 0i32;
                let mut current = thrown;
                while current.non_empty() {
                    if *current.head() == tree {
                        thrown_index = Some(index);
                    }
                    index += 1;
                    current = current.tail();
                }
                if let Some(type_index) = thrown_index {
                    TypeAnnotationPosition::method_throws(location, type_index, frame_pos, current_lambda)
                } else if restype == Some(tree) {
                    TypeAnnotationPosition::method_return(location, frame_pos, current_lambda)
                } else {
                    let mut parameter_index: Option<i32> = None;
                    let mut index = 0i32;
                    let mut current = typarams;
                    while current.non_empty() {
                        if *current.head() == tree {
                            parameter_index = Some(index);
                        }
                        index += 1;
                        current = current.tail();
                    }
                    match parameter_index {
                        Some(parameter_index) => TypeAnnotationPosition::method_type_parameter(
                            location,
                            parameter_index,
                            current_lambda,
                        ),
                        None => panic!("AssertionError: Could not determine position of tree within frame"),
                    }
                }
            }
            TreeKind::TypeApply { clazz, arguments } => {
                let clazz = *clazz;
                let arguments = arguments.clone();
                let new_path: Vec<crate::ids::TreeId> = path[1..].to_vec();
                if new_path.len() < 2 {
                    panic!("AssertionError: too shallow frame path");
                }
                if clazz != Some(tree) {
                    let mut arg: Option<i32> = None;
                    let mut index = 0i32;
                    let mut current = arguments;
                    while current.non_empty() {
                        if *current.head() == tree {
                            arg = Some(index);
                        }
                        index += 1;
                        current = current.tail();
                    }
                    let arg = match arg {
                        Some(arg) => arg,
                        None => panic!("AssertionError: Could not determine type argument position of tree within frame"),
                    };
                    location = location.prepend(TypePathEntry {
                        tag: TypePathEntryKind::TYPE_ARGUMENT,
                        arg,
                    });
                    let frame_ty = ctx.tree(frame).ty.expect("type apply type");
                    let type_to_use = if ctx.tree(new_path[1]).has_tag(crate::tree::jc_tree::Tag::NEWCLASS) {
                        ctx.tree(new_path[1]).ty.expect("new class type")
                    } else {
                        frame_ty
                    };
                    let nested = self.locate_nested_types(ctx, type_to_use);
                    location = location.append_list(&nested);
                }
                self.resolve_frame(
                    ctx,
                    new_path[0],
                    new_path[1],
                    &new_path,
                    current_lambda,
                    outer_type_index,
                    location,
                )
            }
            TreeKind::MemberReference { expr, typeargs, mode, .. } => {
                let expr = *expr;
                let typeargs = typeargs.clone();
                let mode = *mode;
                if expr == Some(tree) {
                    match mode {
                        crate::tree::jc_tree::ReferenceMode::INVOKE => {
                            TypeAnnotationPosition::method_ref(location, frame_pos, current_lambda)
                        }
                        crate::tree::jc_tree::ReferenceMode::NEW => {
                            TypeAnnotationPosition::constructor_ref(location, frame_pos, current_lambda)
                        }
                    }
                } else {
                    let mut type_index: Option<i32> = None;
                    let mut index = 0i32;
                    let mut current = typeargs.unwrap_or_else(List::nil);
                    while current.non_empty() {
                        if *current.head() == tree {
                            type_index = Some(index);
                        }
                        index += 1;
                        current = current.tail();
                    }
                    match type_index {
                        Some(type_index) => match mode {
                            crate::tree::jc_tree::ReferenceMode::INVOKE => {
                                TypeAnnotationPosition::method_ref_type_arg(
                                    location,
                                    type_index,
                                    frame_pos,
                                    current_lambda,
                                )
                            }
                            crate::tree::jc_tree::ReferenceMode::NEW => {
                                TypeAnnotationPosition::constructor_ref_type_arg(
                                    location,
                                    type_index,
                                    frame_pos,
                                    current_lambda,
                                )
                            }
                        },
                        None => panic!("AssertionError: Could not determine type argument position of tree within frame"),
                    }
                }
            }
            TreeKind::ArrayTypeTree { .. } => {
                location = location.prepend(TypePathEntry::ARRAY);
                let mut new_path: Vec<crate::ids::TreeId> = path[1..].to_vec();
                loop {
                    if new_path.len() < 2 {
                        panic!("AssertionError: too shallow array frame path");
                    }
                    let np_head = new_path[1];
                    if ctx.tree(np_head).has_tag(crate::tree::jc_tree::Tag::TYPEARRAY) {
                        new_path.remove(0);
                        location = location.prepend(TypePathEntry::ARRAY);
                    } else if ctx.tree(np_head).has_tag(crate::tree::jc_tree::Tag::ANNOTATED_TYPE) {
                        new_path.remove(0);
                    } else {
                        break;
                    }
                }
                self.resolve_frame(
                    ctx,
                    new_path[0],
                    new_path[1],
                    &new_path,
                    current_lambda,
                    outer_type_index,
                    location,
                )
            }
            TreeKind::TypeParameter { bounds, .. } => {
                let bounds = bounds.clone();
                if path.len() < 3 {
                    panic!("AssertionError: too shallow type parameter frame path");
                }
                let outer = path[2];
                let first_bound = if bounds.non_empty() { Some(*bounds.head()) } else { None };
                let bounds_is_interface = first_bound
                    .and_then(|first_bound| ctx.tree(first_bound).ty)
                    .map(|first_ty| {
                        ctx.typ(first_ty)
                            .tsym
                            .map(|tsym| (ctx.sym(tsym).flags_field & crate::code::flags::INTERFACE as i64) != 0)
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                let mut bound_index = 0i32;
                let mut i = 0i32;
                let mut current = bounds;
                while current.non_empty() {
                    if *current.head() == tree {
                        bound_index = i;
                    }
                    i += 1;
                    current = current.tail();
                }
                if bounds_is_interface {
                    bound_index += 1;
                }
                let parameter_tree = path[1];
                let outer_kind = &ctx.tree(outer).kind;
                if let TreeKind::ClassDecl { typarams, .. } = outer_kind {
                    let mut parameter_index = 0i32;
                    let mut i = 0i32;
                    let mut current = typarams.clone();
                    while current.non_empty() {
                        if *current.head() == parameter_tree {
                            parameter_index = i;
                        }
                        i += 1;
                        current = current.tail();
                    }
                    TypeAnnotationPosition::type_parameter_bound(
                        location,
                        parameter_index,
                        bound_index,
                        frame_pos,
                        current_lambda,
                    )
                } else if let TreeKind::MethodDecl { typarams, .. } = outer_kind {
                    let mut parameter_index = 0i32;
                    let mut i = 0i32;
                    let mut current = typarams.clone();
                    while current.non_empty() {
                        if *current.head() == parameter_tree {
                            parameter_index = i;
                        }
                        i += 1;
                        current = current.tail();
                    }
                    TypeAnnotationPosition::method_type_parameter_bound(
                        location,
                        parameter_index,
                        bound_index,
                        frame_pos,
                        current_lambda,
                    )
                } else {
                    panic!("AssertionError: Could not determine position of tree within frame");
                }
            }
            TreeKind::VariableDecl { sym: Some(v), .. } => {
                let v = *v;
                if ctx.sym(v).get_kind(ctx) != crate::code::symbol::ElementKind::FIELD {
                    let raw_type_attributes = ctx.sym(v).get_raw_type_attributes();
                    self.append_type_annotations_to_owner(ctx, v, &raw_type_attributes);
                }
                match ctx.sym(v).get_kind(ctx) {
                    crate::code::symbol::ElementKind::BINDING_VARIABLE
                    | crate::code::symbol::ElementKind::LOCAL_VARIABLE => {
                        TypeAnnotationPosition::local_variable(location, frame_pos, current_lambda)
                    }
                    crate::code::symbol::ElementKind::FIELD => {
                        TypeAnnotationPosition::field(location, frame_pos, current_lambda)
                    }
                    crate::code::symbol::ElementKind::PARAMETER => {
                        if ctx.sym(v).name == self.type_annotations.names._this {
                            TypeAnnotationPosition::method_receiver(location, frame_pos, current_lambda)
                        } else {
                            let parameter_index = self.method_param_index(ctx, path, frame);
                            TypeAnnotationPosition::method_parameter(
                                location,
                                parameter_index,
                                frame_pos,
                                current_lambda,
                            )
                        }
                    }
                    crate::code::symbol::ElementKind::EXCEPTION_PARAMETER => {
                        TypeAnnotationPosition::exception_parameter(location, frame_pos, current_lambda)
                    }
                    crate::code::symbol::ElementKind::RESOURCE_VARIABLE => {
                        TypeAnnotationPosition::resource_variable(location, frame_pos, current_lambda)
                    }
                    _ => panic!("AssertionError: Found unexpected type annotation for variable"),
                }
            }
            TreeKind::AnnotatedType { underlying_type, .. } => {
                if frame == tree {
                    let underlying = underlying_type.expect("annotated type underlying");
                    let utype = ctx.tree(underlying).ty.expect("underlying type");
                    let tsym_kind = ctx.typ(utype).tsym.map(|tsym| ctx.sym(tsym).get_kind(ctx));
                    let is_wildcard = ctx.typ(utype).has_tag(crate::code::type_tag::TypeTag::WILDCARD);
                    let is_array = ctx.typ(utype).has_tag(crate::code::type_tag::TypeTag::ARRAY);
                    let is_type_parameter = tsym_kind
                        .map(|kind| kind == crate::code::symbol::ElementKind::TYPE_PARAMETER)
                        .unwrap_or(false);
                    if !is_type_parameter && !is_wildcard && !is_array {
                        let nested = self.locate_nested_types(ctx, utype);
                        location = location.append_list(&nested);
                    }
                }
                let new_path: Vec<crate::ids::TreeId> = path[1..].to_vec();
                if new_path.len() < 2 {
                    panic!("AssertionError: too shallow annotated type frame path");
                }
                self.resolve_frame(
                    ctx,
                    new_path[0],
                    new_path[1],
                    &new_path,
                    current_lambda,
                    outer_type_index,
                    location,
                )
            }
            TreeKind::TypeUnion { .. } => {
                let new_path: Vec<crate::ids::TreeId> = path[1..].to_vec();
                if new_path.len() < 2 {
                    panic!("AssertionError: too shallow union frame path");
                }
                self.resolve_frame(
                    ctx,
                    new_path[0],
                    new_path[1],
                    &new_path,
                    current_lambda,
                    outer_type_index,
                    location,
                )
            }
            TreeKind::TypeIntersection { bounds } => {
                let mut bound_index = 0i32;
                let mut i = 0i32;
                let mut current = bounds.clone();
                while current.non_empty() {
                    if *current.head() == tree {
                        bound_index = i;
                    }
                    i += 1;
                    current = current.tail();
                }
                let new_path: Vec<crate::ids::TreeId> = path[1..].to_vec();
                if new_path.len() < 2 {
                    panic!("AssertionError: too shallow intersection frame path");
                }
                self.resolve_frame(
                    ctx,
                    new_path[0],
                    new_path[1],
                    &new_path,
                    current_lambda,
                    bound_index,
                    location,
                )
            }
            TreeKind::MethodInvocation { meth, typeargs, .. } => {
                let mut type_index: Option<i32> = None;
                let mut index = 0i32;
                let mut current = typeargs.clone();
                while current.non_empty() {
                    if *current.head() == tree {
                        type_index = Some(index);
                    }
                    index += 1;
                    current = current.tail();
                }
                let type_index = match type_index {
                    Some(type_index) => type_index,
                    None => return crate::code::type_annotation_position::TypeAnnotationPosition::unknown(),
                };
                let exsym = meth.and_then(|meth| crate::tree::tree_info::symbol(ctx, meth));
                let exsym = match exsym {
                    Some(exsym) => exsym,
                    None => panic!("AssertionError: could not determine symbol for method invocation"),
                };
                if ctx.sym(exsym).is_constructor(&self.type_annotations.names) {
                    TypeAnnotationPosition::constructor_invocation_type_arg(
                        location,
                        type_index,
                        ctx.tree(frame).pos,
                        current_lambda,
                    )
                } else {
                    TypeAnnotationPosition::method_invocation_type_arg(
                        location,
                        type_index,
                        ctx.tree(frame).pos,
                        current_lambda,
                    )
                }
            }
            TreeKind::Wildcard { kind, .. } => {
                let bound_kind = kind.and_then(|kind| match &ctx.tree(kind).kind {
                    TreeKind::TypeBoundKind { kind } => Some(*kind),
                    _ => None,
                });
                match bound_kind {
                    Some(crate::code::bound_kind::BoundKind::EXTENDS)
                    | Some(crate::code::bound_kind::BoundKind::SUPER) => {
                        location = location.prepend(TypePathEntry::WILDCARD);
                        let new_path: Vec<crate::ids::TreeId> = path[1..].to_vec();
                        if new_path.len() < 2 {
                            panic!("AssertionError: too shallow wildcard frame path");
                        }
                        self.resolve_frame(
                            ctx,
                            new_path[0],
                            new_path[1],
                            &new_path,
                            current_lambda,
                            outer_type_index,
                            location,
                        )
                    }
                    _ => panic!("AssertionError: Unknown wildcard bound"),
                }
            }
            TreeKind::FieldAccess { .. } => {
                let new_path: Vec<crate::ids::TreeId> = path[1..].to_vec();
                if new_path.len() < 2 {
                    panic!("AssertionError: too shallow member select frame path");
                }
                self.resolve_frame(
                    ctx,
                    new_path[0],
                    new_path[1],
                    &new_path,
                    current_lambda,
                    outer_type_index,
                    location,
                )
            }
            _ => panic!("AssertionError: Unresolved frame"),
        }
    }
}

impl<'a, 'b> TypeAnnotationPositions<'a, 'b> {
    /// Adapter: the modifier annotations of a VariableDecl/JCMethodDecl.
    fn modifiers_annotations(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        mods: Option<crate::ids::TreeId>,
    ) -> List<crate::ids::TreeId> {
        match mods.map(|mods| &ctx.tree(mods).kind) {
            Some(crate::tree::jc_tree::TreeKind::Modifiers { annotations, .. }) => annotations.clone(),
            _ => List::nil(),
        }
    }

    /// Java: private void propagateNewClassAnnotationsToOwner(JCNewClass tree)
    /// (TypeAnnotations.java:1356-1385).
    fn propagate_new_class_annotations_to_owner(
        &mut self,
        ctx: &mut crate::compiler_context::CompilerContext,
        tree: crate::ids::TreeId,
    ) {
        let (clazz, def) = match &ctx.tree(tree).kind {
            crate::tree::jc_tree::TreeKind::NewClass { clazz, def, .. } => (clazz.expect("new class clazz"), *def),
            _ => return,
        };
        let def = match def {
            Some(def) => def,
            None => return,
        };
        let sym = match &ctx.tree(def).kind {
            crate::tree::jc_tree::TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => return,
        };
        let owner = ctx.sym(sym).owner.expect("anonymous class owner");
        let owner_encl_class = match crate::code::symbol::symbol_encl_class(ctx, owner) {
            Some(encl) => encl,
            None => return,
        };
        let owner_encl_ty = match ctx.sym(owner_encl_class).ty {
            Some(ty) => ty,
            None => return,
        };
        let mut depth = self.locate_nested_types(ctx, owner_encl_ty);
        depth = depth.prepend(TypePathEntry::INNER_TYPE);
        let pos = crate::code::type_annotation_position::TypeAnnotationPosition::new_obj(
            depth,
            ctx.tree(tree).pos,
            None,
        );
        let clazz_ty = ctx.tree(clazz).ty.expect("new class clazz type");
        let expected_location = self.locate_nested_types(ctx, clazz_ty);
        let old_attributes = ctx.sym(sym).get_raw_type_attributes();
        let mut new_attributes: List<crate::ids::AttributeId> = List::nil();
        let mut current = old_attributes;
        while current.non_empty() {
            let old = *current.head();
            current = current.tail();
            let (old_ty, old_values, old_position, synthesized) = match &ctx.attr(old).kind {
                crate::code::attribute::AttributeKind::TypeCompound { values, position, synthesized } => {
                    (ctx.attr(old).ty, values.clone(), position.clone(), *synthesized)
                }
                _ => continue,
            };
            let old_location = old_position.map(|p| p.location).unwrap_or_else(List::nil);
            if old_location == expected_location {
                let new_attr = ctx.alloc_attribute(crate::code::attribute::Attribute::new(
                    old_ty,
                    crate::code::attribute::AttributeKind::TypeCompound {
                        values: old_values,
                        position: Some(pos.clone()),
                        synthesized,
                    },
                ));
                new_attributes = new_attributes.append(new_attr);
            }
        }
        ctx.sym_mut(owner).append_unique_type_attributes(new_attributes);
    }
}
