#![allow(dead_code)] // TODO(Attr/TreeMaker/Check batches): helpers waiting for their batches (registered in the file header).
// Origin: com/sun/tools/javac/comp/TypeEnter.java @ 982d53c (jdk-17.0.19-ga)
//
// Registration (this file):
// - Ported below: the TypeEnter structure/dependencies injection, instance
// equivalent (new), allowTypeAnnos/allowDeprecationOnImport/completionEnabled,
// ensureImportsChecked, complete(), finishImports, the Phase machinery
// (Phase.completeEnvs/doCompleteEnvs; the Java inner phase subclasses are
// expressed as TypeEnter phase methods, registered adapter), ImportsPhase
// (runPhase/resolveImports/checkClassPackageClash/doImport/attribImportType/
// importAll/importStaticAll/importNamedStatic/importNamed), HierarchyPhase
// (runPhase/complete/superTypes), PermitsPhase, AbstractHeaderPhase
// (baseEnv/enumBase/modelMissingTypes/Synthesizer structure), HeaderPhase
// (runPhase/attribSuperTypes/clearTypeParams/default constructor/this&super
// field/finishClass), RecordPhase, MembersPhase, DefaultConstructorHelper
// hierarchy, defaultConstructor, finishClass/addAccessor/addEnumMembers/
// getCanonicalConstructorDecl/addRecordMembersIfNeeded/lookupMethod/
// getDefaultConstructorHelper, markDeprecated/handleDeprecatedAnnotations/
// setFlagIfAttributeTrue.
//
// NOT PORTED IN THIS FILE (registered; dependencies not ported yet, no
// substitute):
// - Attr: attr.attribImportQualifier/attribType/attribBase/
// attribTypeVariables -> explicit unimplemented helpers (Attr);
// - Annotate/TypeAnnotations/Todo/Preview: annotate.blockAnnotations/
// unblockAnnotations/annotateLater/organizeTypeAnnotationsSignatures/
// typeAnnotations.validate*, todo.append, preview checks -> TODO comments
// (Annotate/TypeAnnotations/Todo batches);
// - TreeMaker factories (Import/QualIdent/ClassDef/MethodDef/Modifiers/
// VarDef/Return/Block/Type/Select/Ident/Assign...) are not ported (registered
// in TreeMaker): make.* calls are explicit unimplemented helpers;
// - Check: checkImportsUnique/checkImportsResolvable/
// checkImportedPackagesObservable/importAccessible/checkCanonical/
// checkDefaultConstructor/checkNonCyclic -> explicit unimplemented helpers
// (Check);
// - Lint: Lint.suppress is not ported (Lint) -> unimplemented helper;
// - ClassSymbol.isCompleted/setAnnotationTypeMetadata/getRecordComponent,
// TreeInfo.recordFields/hasConstructors/isConstructor, RecordComponent ->
// registered TODOs where used (symbol/record batches);
// - Modules: syms.enterPackage/lookupPackage/getClass are ported; the
// checkClassPackageClash body follows upstream with the local adapters.
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use crate::code::deferred_lint_handler::DeferredLintHandler;
use crate::code::flags;
use crate::code::kinds::Kind;
use crate::code::scope::LookupKind;
use crate::code::symbol::{CompletionFailure, SymbolKind};
use crate::code::symtab::Symtab;
use crate::code::type_tag::TypeTag;
use crate::code::type_::TypeKindData;
use crate::code::types::Types;
use crate::comp::annotate::Annotate;
use crate::comp::attr_context::AttrContext;
use crate::comp::check::Check;
use crate::comp::enter::Enter;
use crate::comp::env::Env;
use crate::comp::member_enter::MemberEnter;
use crate::comp::type_envs::TypeEnvs;
use crate::compiler_context::CompilerContext;
use crate::ids::{ScopeId, SymbolId, TreeId, TypeId};
use crate::tree::jc_tree::{Tag, TreeKind};
use crate::tree::tree_maker::TreeFactory;
use crate::tree::tree_info;
use crate::util::abstract_log::AbstractLog;
use crate::code::dependencies::{CompletionCause, Dependencies};
use crate::util::jc_diagnostic::SimpleDiagnosticPosition;
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::names::Names;

/// Java: public class TypeEnter implements Completer
pub struct TypeEnter {
    pub names: Rc<Names>,
    pub enter: Rc<RefCell<Enter>>,
    pub member_enter: Rc<RefCell<MemberEnter>>,
    pub log: Rc<RefCell<Log>>,
    pub chk: Rc<RefCell<Check>>,
    pub types: Rc<Types>,
    /// The shared symbol table (needed by the import-scope queries;
    /// registered adapter: the Java import scopes read the Context services).
    pub syms: Rc<std::cell::RefCell<crate::code::symtab::Symtab>>,
    pub make: Rc<RefCell<crate::tree::tree_maker::TreeMaker>>,
    pub deferred_lint_handler: Rc<RefCell<DeferredLintHandler>>,
    pub lint: Option<Rc<crate::code::lint::Lint>>,
    pub type_envs: Rc<RefCell<TypeEnvs>>,
    pub dependencies: Rc<RefCell<dyn Dependencies>>,
    /// Java: boolean allowTypeAnnos
    pub allow_type_annos: bool,
    /// Java: boolean allowDeprecationOnImport
    pub allow_deprecation_on_import: bool,
    /// Java: boolean completionEnabled = true
    pub completion_enabled: std::cell::Cell<bool>,
    /// Java: private final ImportsPhase completeClass = new ImportsPhase()
    /// (expressed as phase state; registered adapter for Java inner classes).
    pub top_level_phase: std::cell::Cell<Option<Phase>>,
    /// ImportsPhase state (Java: inner class fields).
    pub imports_env: std::cell::RefCell<Option<Rc<Env<AttrContext>>>>,
    pub static_import_filter: std::cell::RefCell<Option<crate::code::scope::FilterImportFn>>,
    pub type_import_filter: std::cell::RefCell<Option<crate::code::scope::FilterImportFn>>,
    /// Java: private final Annotate annotate = Annotate.instance(context) —
    /// explicit injection adapter (ADR 0001; Rc cycle broken by a cell, see
    /// MemberEnter.annotate).
    pub annotate: Rc<RefCell<Option<Rc<Annotate>>>>,
    /// Java: private final Todo todo = Todo.instance(context)
    pub todo: Rc<RefCell<crate::comp::todo::Todo>>,
    /// Java: private final Attr attr = Attr.instance(context) — injected at
    /// wiring time to avoid the Attr/TypeEnter construction cycle.
    pub attr: RefCell<Option<Rc<RefCell<crate::comp::attr::Attr>>>>,
    /// Java: private final TypeAnnotations typeAnnotations — injected at wiring
    /// time (only the MembersPhase uses it).
    pub type_annotations:
        RefCell<Option<Rc<crate::code::type_annotations::TypeAnnotations>>>,
    /// Java: private final boolean checkClash (from the Options).
    pub check_clash: bool,
}

/// Java: TypeEnter.Phase (the inner subclasses are expressed as TypeEnter phase
/// methods; registered adapter).
pub struct Phase {
    pub queue: Vec<Rc<Env<AttrContext>>>,
    pub next: Option<Rc<Phase>>,
    pub phase_name: CompletionCause,
}

impl TypeEnter {
    /// Java: protected TypeEnter(Context context) — injected dependencies.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        names: Rc<Names>,
        enter: Rc<RefCell<Enter>>,
        member_enter: Rc<RefCell<MemberEnter>>,
        log: Rc<RefCell<Log>>,
        chk: Rc<RefCell<Check>>,
        types: Rc<Types>,
        syms: Rc<std::cell::RefCell<crate::code::symtab::Symtab>>,
        make: Rc<RefCell<crate::tree::tree_maker::TreeMaker>>,
        deferred_lint_handler: Rc<RefCell<DeferredLintHandler>>,
        lint: Option<Rc<crate::code::lint::Lint>>,
        type_envs: Rc<RefCell<TypeEnvs>>,
        dependencies: Rc<RefCell<dyn Dependencies>>,
        annotate: Rc<RefCell<Option<Rc<Annotate>>>>,
        todo: Rc<RefCell<crate::comp::todo::Todo>>,
    ) -> TypeEnter {
        TypeEnter {
            names,
            enter,
            member_enter,
            log,
            chk,
            types,
            syms,
            make,
            deferred_lint_handler,
            lint,
            type_envs,
            dependencies,
            allow_type_annos: true,
            allow_deprecation_on_import: true,
            completion_enabled: std::cell::Cell::new(true),
            top_level_phase: std::cell::Cell::new(None),
            imports_env: std::cell::RefCell::new(None),
            static_import_filter: std::cell::RefCell::new(None),
            type_import_filter: std::cell::RefCell::new(None),
            annotate,
            todo,
            attr: RefCell::new(None),
            type_annotations: RefCell::new(None),
            check_clash: true,
        }
    }

    /// Java: protected void ensureImportsChecked(List<JCCompilationUnit> trees)
    pub fn ensure_imports_checked(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        trees: List<TreeId>,
    ) {
        let mut current = trees;
        while current.non_empty() {
            let tree = *current.head();
            let star_scope = compilation_unit_star_import_scope(ctx, tree);
            let filled = star_scope
                .map(|scope| ctx.scope_is_filled(scope))
                .unwrap_or(false);
            if !filled {
                let top_env = self.enter.borrow().get_top_level_env(ctx, tree);
                {
                    let attr_rc = self
                        .attr
                        .borrow()
                        .clone()
                        .expect("TypeEnter.attr must be injected");
                    let mut attr = attr_rc.borrow_mut();
                    self.finish_imports(ctx, syms, tree, Some(&top_env), &mut attr);
                }
            }
            current = current.tail();
        }
    }

    /// Java: void finishImports(JCCompilationUnit toplevel, Runnable resolve)
    pub fn finish_imports(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        toplevel: TreeId,
        resolve: Option<&Rc<Env<AttrContext>>>,
        attr: &mut crate::comp::attr::Attr,
    ) {
        let sourcefile = compilation_unit_sourcefile(ctx, toplevel);
        let prev = self.log.borrow_mut().use_source(sourcefile);
        // Java: resolve.run() (the completeClass.resolveImports closure)
        if let Some(env) = resolve {
            self.resolve_imports(ctx, syms, toplevel, env, attr);
        }
        self.check_imports_unique(ctx, syms, toplevel, Some(&mut *attr));
        self.check_imports_resolvable(ctx, syms, toplevel);
        self.check_imported_packages_observable(ctx, toplevel);
        let types = Rc::clone(&self.types);
        if let Some(scope) = compilation_unit_named_import_scope(ctx, toplevel) {
            ctx.scope_finalize(scope, &types, syms, Some(&mut *attr));
        }
        if let Some(scope) = compilation_unit_star_import_scope(ctx, toplevel) {
            ctx.scope_finalize(scope, &types, syms, Some(&mut *attr));
        }
        self.log.borrow_mut().use_source(prev);
    }

    /// Java: public void complete(Symbol sym) throws CompletionFailure — the
    /// normal entry obtains the Attr service and delegates to the explicit-attr
    /// variant (the Attr-inside call site passes its own reference instead).
    pub fn complete(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        sym: SymbolId,
    ) -> Result<(), CompletionFailure> {
        let attr_rc = self
            .attr
            .borrow()
            .clone()
            .expect("TypeEnter.attr must be injected");
        let mut attr = attr_rc.borrow_mut();
        self.complete_with_attr(ctx, syms, sym, &mut attr)
    }

    /// Java: public void complete(Symbol sym) with the enclosing &mut Attr
    /// injected (upstream TypeEnter.attr field is called directly).
    pub fn complete_with_attr(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        sym: SymbolId,
        attr: &mut crate::comp::attr::Attr,
    ) -> Result<(), CompletionFailure> {
        // Suppress some (recursive) MemberEnter invocations
        if !self.completion_enabled.get() {
            // Re-install same completer for next time around and return.
            // Java: Assert.check((sym.flags() & Flags.COMPOUND) == 0);
            // sym.completer = this;
            // TODO(TypeEnter/Completer): the completer registration is an
            // adapter concern (the local completion is driven by the caller's
            // complete_symbol); registered.
            return Ok(());
        }
        let annotate = self
            .annotate
            .borrow()
            .clone()
            .expect("TypeEnter.annotate must be injected");
        annotate.block_annotations();
        ctx.sym_mut(sym).flags_field |= flags::UNATTRIBUTED as i64;
        self.dependencies
            .borrow_mut()
            .push(ctx, sym, CompletionCause::MEMBER_ENTER);
        let env = {
            let type_envs = self.type_envs.borrow();
            type_envs.get(sym)
        };
        let queue = match env {
            Some(env) => {
                let queue = self.phase_imports_complete_envs(ctx, syms, vec![env], attr);
                for queued_env in &queue {
                    self.hierarchy_phase_run(ctx, syms, queued_env, attr);
                    self.permits_phase_run(ctx, syms, queued_env);
                    self.header_phase_run(ctx, syms, queued_env, attr);
                    self.record_phase_run(ctx, syms, queued_env, attr);
                    self.members_phase_run(ctx, syms, queued_env, attr);
                }
                queue
            }
            None => Vec::new(),
        };
        self.dependencies.borrow_mut().pop();
        if !queue.is_empty() {
            let mut seen: HashSet<TreeId> = HashSet::new();
            for env in queue {
                let toplevel = toplevel(&env);
                let encl_class = env.encl_class;
                let in_defs = encl_class
                    .map(|class| compilation_unit_defs(ctx, toplevel).iter().any(|d| d == class))
                    .unwrap_or(false);
                if in_defs && seen.insert(toplevel) {
                    self.finish_imports(ctx, syms, toplevel, None, attr);
                }
            }
        }
        annotate
            .unblock_annotations_with_attr(ctx, syms, attr)
            .expect("unblockAnnotations flush");
        Ok(())
    }

    // ------------------------------------------------------------------
    // Java: TypeEnter.Phase machinery (the inner Phase subclasses are the
    // phase_* methods below; registered adapter).
    // ------------------------------------------------------------------

    fn phase_imports_complete_envs(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        envs: Vec<Rc<Env<AttrContext>>>,
        attr: &mut crate::comp::attr::Attr,
    ) -> Vec<Rc<Env<AttrContext>>> {
        let mut queue = Vec::new();
        for env in envs {
            let tree = env.encl_class.expect("class tree");
            queue.push(Rc::clone(&env));
            let sourcefile = compilation_unit_sourcefile(ctx, toplevel(&env));
            let prev_source = self.log.borrow_mut().use_source(sourcefile);
            let prev_lint = self
                .deferred_lint_handler
                .borrow_mut()
                .set_pos(Rc::new(SimpleDiagnosticPosition::new(ctx.tree(tree).pos)));
            let sym = class_tree_symbol(ctx, tree).expect("class symbol");
            self.dependencies
                .borrow_mut()
                .push(ctx, sym, CompletionCause::IMPORTS_PHASE);
            match self.run_imports_phase(ctx, syms, &env, attr) {
                Ok(()) => {}
                Err(failure) => {
                    self.chk
                        .borrow()
                        .completion_error(ctx, ctx.tree(tree).pos, &failure, syms);
                }
            }
            self.dependencies.borrow_mut().pop();
            self.deferred_lint_handler.borrow_mut().set_pos_state(prev_lint);
            self.log.borrow_mut().use_source(prev_source);
        }
        queue
    }

    /// Java: ImportsPhase.runPhase(Env<AttrContext> env)
    fn run_imports_phase(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) -> Result<(), CompletionFailure> {
        let tree = env.encl_class.expect("class tree");
        let sym = class_tree_symbol(ctx, tree).expect("class symbol");
        let owner = ctx.sym(sym).owner.expect("class owner");
        // If sym is a toplevel-class, make sure any import clauses in its
        // source file have been seen.
        if ctx.sym(owner).kind == Kind::PCK {
            let top = self.enter.borrow().get_top_level_env(ctx, toplevel(env));
            self.resolve_imports(ctx, syms, toplevel(env), &top, attr);
            self.todo.borrow_mut().append(ctx, Rc::clone(env));
        }
        if ctx.sym(owner).kind == Kind::TYP {
            // Java: sym.owner.complete();
            ctx.complete_symbol_with_attr(syms, owner, Some(attr))?;
        }
        Ok(())
    }

    /// Java: ImportsPhase.resolveImports(JCCompilationUnit tree,
    /// Env<AttrContext> env)
    fn resolve_imports(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) {
        if compilation_unit_star_import_scope(ctx, tree)
            .map(|scope| ctx.scope_is_filled(scope))
            .unwrap_or(false)
        {
            // we must have already processed this toplevel
            return;
        }
        let prev_static = self.static_import_filter.borrow().clone();
        let prev_type = self.type_import_filter.borrow().clone();
        let prev_lint_pos = self
            .deferred_lint_handler
            .borrow_mut()
            .immediate();
        // TODO(Lint): Lint prevLint = chk.setLint(lint);
        let prev_env = self.imports_env.borrow().clone();
        *self.imports_env.borrow_mut() = Some(Rc::clone(env));
        let packge = compilation_unit_package(ctx, tree);
        let chk = Rc::clone(&self.chk);
        let types = Rc::clone(&self.types);
        let packge_symbol = packge.expect("package symbol");
        let chk_for_static = Rc::clone(&chk);
        let types_for_static = Rc::clone(&types);
        *self.static_import_filter.borrow_mut() = Some(Rc::new(move |ctx, syms, origin, sym| {
            let origin_owner = ctx.scope(origin).owner;
            ctx.sym(sym).is_static(ctx)
                && chk_for_static
                    .borrow()
                    .import_accessible(ctx, syms, sym, packge_symbol)
                && origin_owner
                    .map(|owner| {
                        let syms_ref: &crate::code::symtab::Symtab = syms;
                        crate::code::symbol::is_member_of(
                            ctx,
                            syms_ref,
                            &types_for_static,
                            sym,
                            owner,
                        )
                    })
                    .unwrap_or(false)
        }));
        let chk_for_type = Rc::clone(&chk);
        *self.type_import_filter.borrow_mut() = Some(Rc::new(move |ctx, syms, _origin, sym| {
            ctx.sym(sym).kind == crate::code::kinds::Kind::TYP
                && chk_for_type
                    .borrow()
                    .import_accessible(ctx, syms, sym, packge_symbol)
        }));
        // Import-on-demand java.lang.
        let java_lang = syms.enter_package(ctx, syms.java_base, self.names.java_lang.clone());
        // Java: javaLang.members() completes the package before listing it.
        if let Err(failure) = ctx.complete_symbol_with_attr(syms, java_lang, Some(attr)) {
            let chk_rc = Rc::clone(&self.chk);
            let chk = chk_rc.borrow();
            chk.completion_error(ctx, ctx.tree(tree).pos, &failure, syms);
            return;
        }
        let java_lang_members_empty = match class_members_scope(ctx, java_lang) {
            Some(scope) => ctx
                .scope_get_symbols(scope, LookupKind::NON_RECURSIVE, &|_| true)
                .is_empty(),
            None => true,
        };
        if java_lang_members_empty && !symbol_exists(ctx, java_lang) {
            // Java: throw new FatalError(diags.fragment(Fragments.FatalErrNoJavaLang));
            let fragment =
                crate::resources::compiler_properties::fragments::fatal_err_no_java_lang();
            crate::util::fatal_error::fatal_error(format!("{:?}", fragment.info().key()));
        }
        // Java: importAll(make.at(tree.pos()).Import(make.QualIdent(javaLang), false), javaLang, env);
        self.import_all_java_lang(ctx, syms, tree, java_lang, env, attr);
        let has_module_decl = compilation_unit_module_decl(ctx, tree).is_some();
        // Process the package def and all import clauses.
        if compilation_unit_package_decl(ctx, tree).is_some() && !has_module_decl {
            self.check_class_package_clash(ctx, syms, env, attr);
        }
        let mut imports = compilation_unit_imports(ctx, tree);
        while imports.non_empty() {
            let imp = *imports.head();
            self.do_import(ctx, syms, imp, attr);
            imports = imports.tail();
        }
        if let Some(decl) = compilation_unit_module_decl(ctx, tree) {
            // check @Deprecated:
            let annotations = module_decl_annotations(ctx, decl);
            let sym = module_decl_symbol(ctx, decl);
            if let Some(sym) = sym {
                self.mark_deprecated(ctx, syms, sym, annotations, env, attr);
            }
            // process module annotations
            let annotate = self
                .annotate
                .borrow()
                .clone()
                .expect("TypeEnter.annotate must be injected");
            let modle = compilation_unit_module(ctx, toplevel(env));
            if let Some(modle) = modle {
                annotate.annotate_later(
                    ctx,
                    module_decl_annotations(ctx, decl),
                    Rc::clone(env),
                    modle,
                    None,
                );
            }
        }
        *self.imports_env.borrow_mut() = prev_env;
        // TODO(Lint): chk.setLint(prevLint);
        self.deferred_lint_handler.borrow_mut().set_pos_state(prev_lint_pos);
        *self.static_import_filter.borrow_mut() = prev_static;
        *self.type_import_filter.borrow_mut() = prev_type;
    }

    /// Java: ImportsPhase.checkClassPackageClash(JCPackageDecl tree)
    fn check_class_package_clash(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) {
        // check that no class exists with same fully qualified name as toplevel
        // package
        if let Some(package_decl) = compilation_unit_package_decl(ctx, toplevel(env))
            && package_decl_is_non_null(ctx, package_decl) {
                let mut p = compilation_unit_package(ctx, toplevel(env)).expect("packge");
                let root = Some(syms.root_package);
                loop {
                    let owner = ctx.sym(p).owner;
                    if owner == root || owner.is_none() {
                        break;
                    }
                    if let Some(owner) = owner {
                        // Java: p.owner.complete(); enter all class members of p
                        if let Err(failure) = ctx.complete_symbol_with_attr(syms, owner, Some(attr)) {
                            self.chk.borrow().completion_error(
                                ctx,
                                ctx.tree(toplevel(env)).pos,
                                &failure,
                                syms,
                            );
                        }
                        // Java: PackageSymbol pack =
                        // syms.lookupPackage(env.toplevel.modle,
                        //                    p.owner.getQualifiedName());
                        // if (syms.getClass(pack.modle, p.getQualifiedName()) != null)
                        //     log.error(tree.pos, Errors.PkgClashesWithClassOfSameName(p));
                        // (TypeEnter.java:396-400).
                        let modle = compilation_unit_module(ctx, toplevel(env)).expect("tree.modle");
                        let owner_fullname = package_fullname_of_type_enter(ctx, owner);
                        let pack = syms.lookup_package(ctx, modle, &owner_fullname);
                        let p_fullname = package_fullname_of_type_enter(ctx, p);
                        let pack_modle = match &ctx.sym(pack).kind_data {
                            crate::code::symbol::SymbolKind::PackageSymbol { modle, .. }
                            | crate::code::symbol::SymbolKind::RootPackageSymbol { modle, .. } => {
                                *modle
                            }
                            _ => None,
                        };
                        if let Some(pack_modle) = pack_modle
                            && syms.get_class(pack_modle, &p_fullname).is_some()
                        {
                            let name = p_fullname.to_java_string().to_string_lossy();
                            self.log.borrow_mut().error_at(
                                ctx.tree(package_decl).pos,
                                crate::util::jc_diagnostic::Error::from(
                                    crate::resources::compiler_properties::errors::
                                        pkg_clashes_with_class_of_same_name(
                                            crate::resources::diagnostic_contract::SymbolArg::
                                                placeholder(name),
                                        ),
                                ),
                            );
                        }
                    }
                    p = owner.unwrap();
                }
            }
        // process package annotations (TypeEnter.java:405)
        let annotate = self
            .annotate
            .borrow()
            .clone()
            .expect("TypeEnter.annotate must be injected");
        if let Some(package_decl) = compilation_unit_package_decl(ctx, toplevel(env))
            && package_decl_is_non_null(ctx, package_decl)
        {
            let package_annotations = match &ctx.tree(package_decl).kind {
                TreeKind::PackageDecl { annotations, .. } => annotations.clone(),
                _ => List::nil(),
            };
            if package_annotations.non_empty() {
                let packge = compilation_unit_package(ctx, toplevel(env)).expect("packge");
                annotate.annotate_later(ctx, package_annotations, Rc::clone(env), packge, None);
            }
        }
    }

    /// Java: ImportsPhase.doImport(JCImport tree)
    fn do_import(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        tree: TreeId,
        attr: &mut crate::comp::attr::Attr,
    ) {
        let qualid = import_qualid(ctx, tree).expect("import qualid");
        let field_access = match &ctx.tree(qualid).kind {
            TreeKind::FieldAccess { selected, name, .. } => (*selected, name.clone()),
            _ => panic!("ClassCastException: JCFieldAccess expected"),
        };
        let (selected, name) = field_access;
        let env = self.imports_env.borrow().clone().expect("imports env");
        // Create a local environment pointing to this tree to disable effects of
        // other imports in Resolve.findGlobalType
        let local_env = Env::dup_shared(&env, Some(tree));
        // Java: TypeSymbol p = attr.attribImportQualifier(tree, localEnv).tsym;
        let p = self.attrib_import_qualifier(ctx, syms, tree, &local_env, attr);
        if name.to_java_string() == self.names.asterisk.to_java_string() {
            // Import on demand.
            if let Some(selected) = selected {
                self.chk.borrow().check_canonical(ctx, selected);
            }
            if import_static_import(ctx, tree) {
                self.import_static_all(ctx, syms, tree, p, &env, attr);
            } else {
                self.import_all(ctx, syms, tree, p, &env, attr);
            }
        } else if import_static_import(ctx, tree) {
            // Named static import.
            self.import_named_static(ctx, syms, tree, p, name, &local_env, attr);
            if let Some(selected) = selected {
                self.chk.borrow().check_canonical(ctx, selected);
            }
        } else {
            // Named type import.
            let imported_type = self.attrib_import_type(ctx, syms, qualid, &local_env, attr);
            let original_type = original_type(ctx, imported_type);
            let c = if ctx.typ(original_type).has_tag(TypeTag::CLASS) {
                ctx.typ(original_type).tsym
            } else {
                ctx.typ(imported_type).tsym
            };
            // TODO(Check): chk.checkCanonical(imp);
            if let Some(c) = c {
                self.import_named(ctx, syms, ctx.tree(tree).pos, c, &env, tree, attr);
            }
        }
    }

    /// Java: ImportsPhase.attribImportType(JCTree tree, Env<AttrContext> env)
    fn attrib_import_type(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) -> crate::ids::TypeId {
        use crate::code::lint::LintCategory;
        // Java: Assert.check(completionEnabled);
        let base_lint = match &self.lint {
            Some(lint) => Rc::clone(lint),
            None => self.chk.borrow().lint.clone(),
        };
        let lint = if self.allow_deprecation_on_import {
            crate::code::lint::Lint::new_from(&base_lint)
        } else {
            base_lint.suppress(&[
                LintCategory::DEPRECATION,
                LintCategory::REMOVAL,
                LintCategory::PREVIEW,
            ])
        };
        let prev_lint = self.chk.borrow_mut().set_lint(Rc::new(lint));
        let prev_completion = self.completion_enabled.get();
        self.completion_enabled.set(false);
        let result = { attr.attrib_type(ctx, syms, tree, env) };
        self.completion_enabled.set(prev_completion);
        self.chk.borrow_mut().set_lint(prev_lint);
        result
    }

    fn attrib_import_qualifier(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        tree: TreeId,
        local_env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) -> SymbolId {
        let imported = attr.attrib_import_qualifier(ctx, syms, tree, local_env);
        ctx.typ(imported).tsym.expect("import qualifier symbol")
    }

    /// Java: ImportsPhase.importAll(JCImport imp, TypeSymbol tsym,
    /// Env<AttrContext> env)
    fn import_all(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        _tree: TreeId,
        tsym: SymbolId,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) {
        // Java: Symbol.members() completes the symbol (Symbol.java:870-873);
        // the port performs the completion explicitly.
        if let Err(failure) = ctx.complete_symbol_with_attr(syms, tsym, Some(attr)) {
            self.chk.borrow().completion_error(
                ctx,
                ctx.tree(_tree).pos,
                &failure,
                syms,
            );
        }
        let star_scope = compilation_unit_star_import_scope(ctx, toplevel(env)).expect("star scope");
        // Java: tsym.members() (works for packages and classes).
        let origin = ctx.sym(tsym).members().expect("type members");
        // Java: toplevel(&env).starImportScope.importAll(types, tsym.members(),
        // typeImportFilter, imp, cfHandler);
        let type_filter = self.type_import_filter.borrow().clone();
        ctx.scope_import_all(
            star_scope,
            origin,
            type_filter,
            Some(_tree),
            None,
            false,
            Rc::clone(&self.types),
        );
    }

    /// Java: ImportsPhase.importStaticAll(JCImport imp, TypeSymbol tsym,
    /// Env<AttrContext> env)
    #[allow(clippy::too_many_arguments)]
    fn import_static_all(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        tree: TreeId,
        tsym: SymbolId,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) {
        // Java: TypeSymbol.members() completes the symbol (Symbol.java:870-873).
        if let Err(failure) = ctx.complete_symbol_with_attr(syms, tsym, Some(attr)) {
            self.chk.borrow().completion_error(
                ctx,
                ctx.tree(tree).pos,
                &failure,
                syms,
            );
        }
        let star_scope = compilation_unit_star_import_scope(ctx, toplevel(env)).expect("star scope");
        let origin = class_members_scope(ctx, tsym).expect("type members");
        let filter = self.static_import_filter.borrow().clone();
        ctx.scope_import_all(
            star_scope,
            origin,
            filter,
            Some(tree),
            None,
            true,
            Rc::clone(&self.types),
        );
    }

    /// Java: ImportsPhase.importNamedStatic(JCImport imp, TypeSymbol tsym,
    /// Name name, Env<AttrContext> env)
    #[allow(clippy::too_many_arguments)]
    fn import_named_static(
        &self,
        ctx: &mut CompilerContext,
        _syms: &Symtab,
        tree: TreeId,
        tsym: SymbolId,
        name: crate::util::name::Name,
        env: &Rc<Env<AttrContext>>,
        _attr: &mut crate::comp::attr::Attr,
    ) {
        if ctx.sym(tsym).kind != Kind::TYP {
            // Java: log.error(DiagnosticFlag.RECOVERABLE, imp.pos(),
            // Errors.StaticImpOnlyClassesAndInterfaces);
            self.log.borrow_mut().error_at(
                ctx.tree(tree).pos,
                crate::util::jc_diagnostic::Error::from(
                    crate::resources::compiler_properties::errors::static_imp_only_classes_and_interfaces(),
                ),
            );
            return;
        }
        let named_scope =
            compilation_unit_named_import_scope(ctx, toplevel(env)).expect("named scope");
        let origin = class_members_scope(ctx, tsym).expect("type members");
        let filter = self.static_import_filter.borrow().clone();
        let new_scope = ctx.scope_import_by_name(
            named_scope,
            origin,
            name,
            filter,
            Some(tree),
            None,
            true,
            Rc::clone(&self.types),
        );
        set_import_scope(ctx, tree, new_scope);
    }

    /// Java: ImportsPhase.importNamed(DiagnosticPosition pos, Symbol tsym,
    /// Env<AttrContext> env, JCImport imp)
    #[allow(clippy::too_many_arguments)]
    fn import_named(
        &self,
        ctx: &mut CompilerContext,
        _syms: &Symtab,
        _pos: i32,
        tsym: SymbolId,
        env: &Rc<Env<AttrContext>>,
        tree: TreeId,
        _attr: &mut crate::comp::attr::Attr,
    ) {
        if ctx.sym(tsym).kind == Kind::TYP {
            let named_scope =
                compilation_unit_named_import_scope(ctx, toplevel(env)).expect("named scope");
            // Java: tsym.owner.members() — the owner is a package for
            // top-level imports and a class for nested-class imports; both
            // symbol kinds expose their members scope through Symbol.members().
            let owner_members = ctx
                .sym(tsym)
                .owner
                .and_then(|owner| ctx.sym(owner).members())
                .expect("owner members");
            let new_scope = ctx.scope_import_type(named_scope, owner_members, owner_members, tsym);
            set_import_scope(ctx, tree, new_scope);
        }
    }

    /// Java: importAll(make.at(tree.pos()).Import(make.QualIdent(javaLang), false),
    /// javaLang, env) — the java.lang import-on-demand.
    fn import_all_java_lang(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        tree: TreeId,
        java_lang: SymbolId,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) {
        let pos = ctx.tree(tree).pos;
        let imp = {
            let mut make = self.make.borrow_mut();
            make.at(pos);
            let qual = make.qual_ident(ctx, syms, java_lang);
            make.import(ctx, qual, false)
        };
        self.import_all(ctx, syms, imp, java_lang, env, attr);
    }
    /// Delegates the unique-import check to the Check service, forwarding the
    /// enclosing Attr for the import-scope completion.
    fn check_imports_unique(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        toplevel: TreeId,
        attr: Option<&mut crate::comp::attr::Attr>,
    ) {
        self.chk
            .borrow()
            .check_imports_unique(ctx, syms, toplevel, attr);
    }

    /// Java: Check.checkImportsResolvable(toplevel) (Check.java:4002-4024).
    fn check_imports_resolvable(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        toplevel: TreeId,
    ) {
        self.chk.borrow().check_imports_resolvable(ctx, syms, toplevel);
    }

    /// Java: Check.checkImportedPackagesObservable(toplevel) (Check.java:4026-4039).
    fn check_imported_packages_observable(&self, ctx: &mut CompilerContext, toplevel: TreeId) {
        self.chk.borrow().check_imported_packages_observable(ctx, toplevel);
    }

    /// Java: public void markDeprecated(Symbol sym, List<JCAnnotation>
    /// annotations, Env<AttrContext> env)
    pub fn mark_deprecated(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        sym: SymbolId,
        annotations: List<TreeId>,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) {
        // In general, we cannot fully process annotations yet, but we can
        // attribute the annotation types and then check to see if the
        // @Deprecated annotation is present.
        self.attrib_annotation_types(ctx, syms, annotations.clone(), env, attr);
        self.handle_deprecated_annotations(ctx, syms, annotations, sym);
    }

    /// Attribute the annotation type trees of a list of annotations (the
    /// markDeprecated helper).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - annotations: the annotation trees;
    ///   - env: the attribution environment.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - the Attr service is not injected.
    fn attrib_annotation_types(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        annotations: List<TreeId>,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) {
        let mut current = annotations;
        while current.non_empty() {
            let annotation = *current.head();
            current = current.tail();
            if let Some(annotation_type) = annotation_type(ctx, annotation) {
                attr.attrib_type(ctx, syms, annotation_type, env);
            }
        }
    }

    /// Java: private void handleDeprecatedAnnotations(List<JCAnnotation>
    /// annotations, Symbol sym)
    fn handle_deprecated_annotations(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        annotations: List<TreeId>,
        sym: SymbolId,
    ) {
        let mut al = annotations;
        while !al.is_empty() {
            let annotation = *al.head();
            let annotation_type = annotation_type(ctx, annotation).and_then(|tree| ctx.tree(tree).ty);
            if annotation_type == Some(syms.deprecated_type) {
                ctx.sym_mut(sym).flags_field |=
                    (flags::DEPRECATED | flags::DEPRECATED_ANNOTATION as i32) as i64;
                self.set_flag_if_attribute_true(
                    ctx,
                    annotation,
                    sym,
                    self.names.forRemoval.clone(),
                    flags::DEPRECATED_REMOVAL,
                );
            } else if annotation_type == Some(syms.preview_feature_type) {
                ctx.sym_mut(sym).flags_field |= flags::PREVIEW_API;
                self.set_flag_if_attribute_true(
                    ctx,
                    annotation,
                    sym,
                    self.names.reflective.clone(),
                    flags::PREVIEW_REFLECTIVE,
                );
            }
            al = al.tail();
        }
    }

    /// Java: private void setFlagIfAttributeTrue(JCAnnotation a, Symbol sym,
    /// Name attribute, long flag)
    fn set_flag_if_attribute_true(
        &self,
        ctx: &mut CompilerContext,
        annotation: TreeId,
        sym: SymbolId,
        attribute: crate::util::name::Name,
        flag: i64,
    ) {
        let args = annotation_args(ctx, annotation);
        let mut current = args;
        while current.non_empty() {
            let arg = *current.head();
            if ctx.tree(arg).has_tag(Tag::ASSIGN) {
                let (lhs, rhs) = assign_parts(ctx, arg);
                if let Some(lhs) = lhs {
                    if tree_info::name(ctx, lhs) == Some(attribute.clone())
                        && let Some(rhs) = rhs {
                            let rhs = tree_info::skip_parens(ctx, rhs);
                            if ctx.tree(rhs).has_tag(Tag::LITERAL)
                                && literal_is_true(ctx, rhs) {
                                    ctx.sym_mut(sym).flags_field |= flag;
                                }
                        }
                    // Java: findFirst().ifPresent(...) — the first matching
                    // assign is handled and the loop ends.
                    return;
                }
            }
            current = current.tail();
        }
    }


    // ------------------------------------------------------------------
    // Java: AbstractHeaderPhase (TypeEnter.java:500-676)
    // ------------------------------------------------------------------

    /// Java: protected Env<AttrContext> baseEnv(JCClassDecl tree,
    /// Env<AttrContext> env)
    fn base_env(
        &self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) -> Rc<Env<AttrContext>> {
        let sym = class_tree_symbol(ctx, tree).expect("class sym");
        let base_scope = ctx.scope_create(Some(sym));
        // import already entered local classes into base scope
        let outer = env.outer.clone().expect("env.outer");
        let outer_scope = outer.info.borrow().scope.expect("outer.info.scope");
        let symbols = ctx.scope_get_symbols(
            outer_scope,
            LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        for symbol in symbols {
            if crate::code::symbol::symbol_is_directly_or_indirectly_local(ctx, symbol) {
                ctx.scope_enter(base_scope, symbol);
            }
        }
        // import current type-parameters into base scope
        if let Some(typarams) = class_tree_typarams(ctx, tree) {
            let mut current = typarams;
            while current.non_empty() {
                let typaram = *current.head();
                if let Some(ty) = ctx.tree(typaram).ty
                    && let Some(tsym) = ctx.typ(ty).tsym {
                        ctx.scope_enter(base_scope, tsym);
                    }
                current = current.tail();
            }
        }
        // the base clause can't see members of this class
        let mut info = outer.info.borrow().dup(Some(base_scope));
        info.is_self_call = false;
        let base_env = Env {
            next: None,
            outer: Some(Rc::clone(&outer)),
            tree: Some(tree),
            toplevel: outer.toplevel,
            encl_class: outer.encl_class,
            encl_method: outer.encl_method,
            info: Rc::new(RefCell::new(info)),
            base_clause: true,
        };
        Rc::new(base_env)
    }

    /// Java: protected JCExpression enumBase(int pos, ClassSymbol c)
    fn enum_base(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        pos: i32,
        symbol: SymbolId,
    ) -> TreeId {
        let class_type = ctx.sym(symbol).ty.expect("enum class type");
        let mut make = self.make.borrow_mut();
        make.at(pos);
        let qual = make.qual_ident(ctx, syms, syms.enum_sym);
        let ty_tree = make.type_expr(ctx, syms, class_type);
        make.type_apply(ctx, qual, List::of(ty_tree))
    }

    /// Java: protected Type modelMissingTypes(Env<AttrContext> env, Type t,
    /// JCExpression tree, boolean interfaceExpected)
    fn model_missing_types(
        &self,
        _ctx: &mut CompilerContext,
        _env: &Rc<Env<AttrContext>>,
        t: crate::ids::TypeId,
        _tree: TreeId,
        _interface_expected: bool,
    ) -> crate::ids::TypeId {
        // Java: if (!t.hasTag(ERROR)) return t; else new ErrorType(...)
        // getModelType() -> new Synthesizer(...).visit(tree)
        // TODO(Synthesizer): the local ErrorType modelType hook and the
        // Synthesizer visitor are not ported; registered.
        t
    }

    // ------------------------------------------------------------------
    // Java: HierarchyPhase (TypeEnter.java:700-846)
    // ------------------------------------------------------------------

    /// Java: HierarchyPhase.runPhase(Env<AttrContext> env)
    fn hierarchy_phase_run(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) {
        let tree = env.encl_class.expect("class tree");
        let sym = class_tree_symbol(ctx, tree).expect("class symbol");
        let sym_ty = ctx.sym(sym).ty.expect("class type");
        let base_env = self.base_env(ctx, tree, env);
        self.attrib_super_types(ctx, syms, env, attr, &base_env);
        if class_fullname(ctx, sym) == Some(self.names.java_lang_Object.clone()) {
            let (extending, implementing) = match &ctx.tree(tree).kind {
                TreeKind::ClassDecl {
                    extending,
                    implementing,
                    ..
                } => (*extending, implementing.clone()),
                _ => (None, List::nil()),
            };
            if let Some(extending) = extending {
                let supertype_field = match &ctx.typ(sym_ty).kind {
                    crate::code::type_::TypeKindData::ClassType {
                        supertype_field, ..
                    } => *supertype_field,
                    _ => None,
                };
                if let Some(supertype_field) = supertype_field {
                    let chk_rc = Rc::clone(&self.chk);
                    let chk = chk_rc.borrow();
                    chk.check_non_cyclic(ctx, syms, Some(ctx.tree(extending).pos), supertype_field);
                    if let crate::code::type_::TypeKindData::ClassType {
                        supertype_field, ..
                    } = &mut ctx.typ_mut(sym_ty).kind
                    {
                        *supertype_field = None;
                    }
                }
            } else if implementing.non_empty() {
                let first = *implementing.head();
                let interfaces = match &ctx.typ(sym_ty).kind {
                    crate::code::type_::TypeKindData::ClassType {
                        interfaces_field, ..
                    } => interfaces_field.clone(),
                    _ => None,
                };
                if let Some(interfaces) = interfaces
                    && interfaces.non_empty()
                {
                    let chk_rc = Rc::clone(&self.chk);
                    let chk = chk_rc.borrow();
                    chk.check_non_cyclic(ctx, syms, Some(ctx.tree(first).pos), *interfaces.head());
                }
                if let crate::code::type_::TypeKindData::ClassType {
                    interfaces_field, ..
                } = &mut ctx.typ_mut(sym_ty).kind
                {
                    *interfaces_field = Some(List::nil());
                }
            }
        }
        let annotations = class_tree_annotations(ctx, tree);
        self.mark_deprecated(ctx, syms, sym, annotations, &base_env, attr);
        {
            let chk_rc = Rc::clone(&self.chk);
            let chk = chk_rc.borrow();
            chk.check_non_cyclic_decl(ctx, syms, tree);
        }
    }

    /// Attribute the supertypes, interfaces and permitted subclasses of a
    /// class declaration.
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - env: the class environment;
    ///   - base_env: the environment for base clauses.
    ///
    /// # Returns
    ///   - none (the class type fields and symbol flags are updated).
    ///
    /// # Panics
    ///   - a class symbol invariant is violated.
    fn attrib_super_types(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
        base_env: &Rc<Env<AttrContext>>,
    ) {
        use crate::code::flags;
        let tree = env.encl_class.expect("env.enclClass");
        let sym = class_tree_symbol(ctx, tree).expect("class symbol");
        let sym_ty = ctx.sym(sym).ty.expect("class type");
        let (extending_tree, implementing, permitting) = class_decl_clauses(ctx, tree);
        let mut extending: Option<TreeId> = None;
        let supertype = if let Some(extending_tree) = extending_tree {
            let cleared = self.clear_type_params_base(ctx, extending_tree);
            extending = Some(cleared);
            let supertype = attr.attrib_base(
                ctx,
                syms,
                cleared,
                base_env,
                true,
                false,
                true,
            );
            if supertype == syms.record_type {
                let info = crate::resources::compiler_properties::errors::invalid_supertype_record(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(String::from(
                        "Record",
                    )),
                );
                self.log.borrow_mut().error_at(
                    ctx.tree(tree).pos,
                    crate::util::jc_diagnostic::Error::from(info),
                );
            }
            supertype
        } else if (class_tree_mods_flags(ctx, tree) & flags::ENUM as i64) != 0 {
            let pos = ctx.tree(tree).pos;
            let enum_tree = self.enum_base(ctx, syms, pos, sym);
            attr.attrib_base(
                ctx,
                syms,
                enum_tree,
                base_env,
                true,
                false,
                false,
            )
        } else if class_fullname(ctx, sym) == Some(self.names.java_lang_Object.clone()) {
            self.types.no_type(ctx)
        } else if symbol_is_record(ctx, sym) {
            syms.record_type
        } else {
            syms.object_type
        };
        let supertype =
            self.model_missing_types(ctx, base_env, supertype, extending.unwrap_or(tree), false);
        // Determine interfaces.
        let mut interfaces: List<TypeId> = List::nil();
        let mut all_interfaces: Option<List<TypeId>> = None;
        let mut iface_trees = implementing;
        while iface_trees.non_empty() {
            let iface = *iface_trees.head();
            iface_trees = iface_trees.tail();
            let cleared = self.clear_type_params_base(ctx, iface);
            let it = attr.attrib_base(
                ctx,
                syms,
                cleared,
                base_env,
                false,
                true,
                true,
            );
            if ctx.typ(it).has_tag(crate::code::type_tag::TypeTag::CLASS) {
                interfaces = interfaces.append(it);
                if let Some(all) = all_interfaces.as_mut() {
                    *all = all.append(it);
                }
            } else {
                if all_interfaces.is_none() {
                    all_interfaces = Some(interfaces.clone());
                }
                let modeled =
                    self.model_missing_types(ctx, base_env, it, cleared, true);
                let all = all_interfaces.as_mut().expect("all interfaces");
                *all = all.append(modeled);
            }
        }
        // Determine permits.
        let mut permitted_symbols: List<SymbolId> = List::nil();
        let mut permitted_trees = permitting;
        while permitted_trees.non_empty() {
            let permitted = *permitted_trees.head();
            permitted_trees = permitted_trees.tail();
            let pt = attr.attrib_base(
                ctx,
                syms,
                permitted,
                base_env,
                false,
                false,
                false,
            );
            if let Some(tsym) = ctx.typ(pt).tsym {
                permitted_symbols = permitted_symbols.append(tsym);
            }
        }
        let (interface_list, all_interface_list) = if (ctx.sym(sym).flags_field
            & flags::ANNOTATION as i64)
            != 0
        {
            let annotation_ty = syms.annotation_type;
            (List::of(annotation_ty), List::of(annotation_ty))
        } else {
            (
                interface_list_source(&interfaces),
                all_interfaces.unwrap_or_else(|| interface_list_source(&interfaces)),
            )
        };
        let supertype_field = supertype;
        if let crate::code::type_::TypeKindData::ClassType {
                supertype_field: slot,
                interfaces_field,
                all_interfaces_field: all_slot,
                ..
            } = &mut ctx.typ_mut(sym_ty).kind {
            *slot = Some(supertype_field);
            *interfaces_field = Some(interface_list);
            *all_slot = Some(all_interface_list);
        }
        if permitted_symbols.non_empty() {
            if let crate::code::symbol::SymbolKind::ClassSymbol {
                    permitted,
                    is_permitted_explicit,
                    ..
                } = &mut ctx.sym_mut(sym).kind_data {
                *permitted = permitted_symbols;
                *is_permitted_explicit = true;
            }
        } else {
            if let crate::code::symbol::SymbolKind::ClassSymbol {
                    is_permitted_explicit,
                    ..
                } = &mut ctx.sym_mut(sym).kind_data {
                *is_permitted_explicit = false;
            }
        }
    }

    /// The base clearTypeParams (returns the tree unchanged).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - super_type: the supertype tree.
    ///
    /// # Returns
    ///   - the supertype tree (unchanged).
    ///
    /// # Panics
    ///   - none.
    fn clear_type_params_base(
        &self,
        _ctx: &mut CompilerContext,
        super_type: TreeId,
    ) -> TreeId {
        super_type
    }



    /// Java: HeaderPhase.clearTypeParams (HierarchyPhase override).
    fn clear_type_params(&self, _ctx: &mut CompilerContext, super_type: TreeId) -> TreeId {
        // Java: case TYPEAPPLY: return ((JCTypeApply) superType).clazz; else
        // return superType.
        if ctx_tree_has_tag(_ctx, super_type, Tag::TYPEAPPLY) {
            type_apply_clazz(_ctx, super_type).unwrap_or(super_type)
        } else {
            super_type
        }
    }

    // ------------------------------------------------------------------
    // Java: PermitsPhase (TypeEnter.java:853-878)
    // ------------------------------------------------------------------

    /// Java: PermitsPhase.runPhase(Env<AttrContext> env)
    fn permits_phase_run(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
    ) {
        let tree = env.encl_class.expect("class tree");
        let sym = class_tree_symbol(ctx, tree).expect("class symbol");
        let sym_ty = ctx.sym(sym).ty.expect("class type");
        let is_anonymous = ctx.sym(sym).is_anonymous();
        let is_enum = (ctx.sym(sym).flags_field & crate::code::flags::ENUM as i64) != 0;
        if !is_anonymous || is_enum {
            let supertypes = {
                let types = Rc::clone(&self.types);
                let syms_borrow = syms;
                types.direct_supertypes(ctx, syms_borrow, sym_ty)
            };
            let mut current = supertypes;
            while current.non_empty() {
                let supertype = *current.head();
                current = current.tail();
                let Some(sup_tsym) = ctx.typ(supertype).tsym else {
                    continue;
                };
                let sup_class = sup_tsym;
                if ctx.sym(sup_class).kind != crate::code::kinds::Kind::TYP {
                    continue;
                }
                let sup_env = self.enter.borrow().get_env(sup_class);
                if ctx.sym(sup_class).is_sealed()
                    && !class_is_permitted_explicit(ctx, sup_class)
                    && sup_env
                        .as_ref()
                        .map(|sup_env| sup_env.toplevel == env.toplevel)
                        .unwrap_or(false)
                {
                    if let crate::code::symbol::SymbolKind::ClassSymbol {
                            permitted, ..
                        } = &mut ctx.sym_mut(sup_class).kind_data {
                        *permitted = permitted.append(sym);
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Java: HeaderPhase (TypeEnter.java:880-935)
    // ------------------------------------------------------------------

    /// Java: HeaderPhase.runPhase(Env<AttrContext> env)
    fn header_phase_run(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) {
        use crate::code::flags;
        let tree = env.encl_class.expect("class tree");
        let sym = class_tree_symbol(ctx, tree).expect("class symbol");
        let base_env = self.base_env(ctx, tree, env);
        let annotate = self
            .annotate
            .borrow()
            .clone()
            .expect("TypeEnter.annotate must be injected");
        let (extending, implementing) = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl {
                extending,
                implementing,
                ..
            } => (*extending, implementing.clone()),
            _ => (None, List::nil()),
        };
        if let Some(extending) = extending {
            annotate.services.queue_scan_tree_and_type_annotate(
                extending,
                &base_env,
                sym,
                ctx.tree(tree).pos,
            );
        }
        let mut impls = implementing.clone();
        while impls.non_empty() {
            let interface = *impls.head();
            impls = impls.tail();
            annotate.services.queue_scan_tree_and_type_annotate(
                interface,
                &base_env,
                sym,
                ctx.tree(tree).pos,
            );
        }
        annotate.flush_with_attr(ctx, syms, attr).expect("annotation flush");
        self.attrib_super_types(ctx, syms, env, attr, &base_env);
        let mut interfaces_seen: std::collections::HashSet<TypeId> =
            std::collections::HashSet::new();
        let mut impls = implementing.clone();
        while impls.non_empty() {
            let interface = *impls.head();
            impls = impls.tail();
            if let Some(it) = ctx.tree(interface).ty
                && ctx.typ(it).has_tag(crate::code::type_tag::TypeTag::CLASS)
            {
                let erased = {
                    let types = Rc::clone(&self.types);
                    let syms_borrow = syms;
                    types.erasure(ctx, syms_borrow, it)
                };
                let chk_rc = Rc::clone(&self.chk);
                let chk = chk_rc.borrow();
                chk.check_not_repeated(ctx.tree(interface).pos, erased, &mut interfaces_seen);
            }
        }
        let annotations = class_tree_annotations(ctx, tree);
        annotate.annotate_later(ctx, annotations.clone(), Rc::clone(&base_env), sym, Some(ctx.tree(tree).pos));
        let typarams = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { typarams, .. } => typarams.clone(),
            _ => List::nil(),
        };
        {
            attr.attrib_type_variables(ctx, syms, typarams.clone(), &base_env, false);
        }
        let mut params = typarams;
        while params.non_empty() {
            let param = *params.head();
            params = params.tail();
            annotate.services.queue_scan_tree_and_type_annotate(
                param,
                &base_env,
                sym,
                ctx.tree(tree).pos,
            );
        }
        // check that no package exists with the same fully qualified name.
        let owner = ctx.sym(sym).owner.expect("class owner");
        if self.check_clash && ctx.sym(owner).kind == crate::code::kinds::Kind::PCK {
            let (modle, unnamed_package) = match &ctx.tree(toplevel(env)).kind {
                TreeKind::CompilationUnit { modle, packge, .. } => {
                    (*modle, *packge)
                }
                _ => (None, None),
            };
            let fullname = class_fullname(ctx, sym)
                .unwrap_or_else(|| self.names.empty.clone());
            if owner != unnamed_package.unwrap_or(owner)
                && modle
                    .map(|modle| syms.package_exists(ctx, modle, &fullname))
                    .unwrap_or(false)
            {
                let info = crate::resources::compiler_properties::errors::clash_with_pkg_of_same_name(
                    crate::resources::diagnostic_contract::KindNameArg::placeholder(String::from(
                        "class",
                    )),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        class_fullname(ctx, sym)
                            .map(|name| name.to_java_string().to_string_lossy().to_string())
                            .unwrap_or_default(),
                    ),
                );
                self.log.borrow_mut().error_at(
                    ctx.tree(tree).pos,
                    crate::util::jc_diagnostic::Error::from(info),
                );
            }
        }
        // auxiliary source file flag (non-public top-level class whose name does
        // not match the source file name).
        if ctx.sym(owner).kind == crate::code::kinds::Kind::PCK
            && (ctx.sym(sym).flags_field & flags::PUBLIC as i64) == 0
        {
            let sourcefile = compilation_unit_sourcefile(ctx, toplevel(env));
            if let Some(sourcefile) = sourcefile {
                let name = ctx.sym(sym).name.to_java_string();
                if !sourcefile.is_name_compatible(
                    &name,
                    javac_tools::tools::java_file_object::Kind::SOURCE,
                ) {
                    ctx.sym_mut(sym).flags_field |= flags::AUXILIARY;
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Java: AbstractMembersPhase/RecordPhase/MembersPhase
    // (TypeEnter.java:937-1090)
    // ------------------------------------------------------------------

    /// Java: AbstractMembersPhase.enterThisAndSuper(ClassSymbol sym,
    /// Env<AttrContext> env)
    fn enter_this_and_super(
        &self,
        ctx: &mut CompilerContext,
        sym: SymbolId,
        env: &Rc<Env<AttrContext>>,
    ) {
        // enter symbols for 'this' into current scope.
        let ty = ctx.sym(sym).ty.expect("class type");
        let this_sym = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
            ((flags::FINAL as i64 | flags::HASINIT as i64)),
            self.names._this.clone(),
            ty,
            sym,
        ));
        set_var_symbol_pos(ctx, this_sym, 0);
        let scope = env.info.borrow().scope.expect("env.info.scope");
        ctx.scope_enter(scope, this_sym);
        // if this is a class, enter symbol for 'super' into current scope.
        if (ctx.sym(sym).flags_field & flags::INTERFACE as i64) == 0 {
            let supertype = class_supertype(ctx, ty);
            if let Some(supertype) = supertype
                && ctx.typ(supertype).has_tag(TypeTag::CLASS) {
                    let super_sym = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
                        ((flags::FINAL as i64 | flags::HASINIT as i64)),
                        self.names._super.clone(),
                        supertype,
                        sym,
                    ));
                    set_var_symbol_pos(ctx, super_sym, 0);
                    ctx.scope_enter(scope, super_sym);
                }
        }
    }

    /// The RecordPhase: enter the record component fields and the record
    /// components themselves, and enter the constructors.
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - env: the record class environment.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - a record invariant is violated.
    fn record_phase_run(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) {
        use crate::code::flags;
        let tree = env.encl_class.expect("class tree");
        let sym = class_tree_symbol(ctx, tree).expect("class symbol");
        if (ctx.sym(sym).flags_field & flags::RECORD) == 0 {
            return;
        }
        let fields = tree_info::record_fields(ctx, tree);
        let mut current = fields;
        while current.non_empty() {
            let field = *current.head();
            current = current.tail();
            let existing = ctx.sym(sym).find_record_component_to_remove(ctx, field);
            if let Some(existing) = existing {
                let original_annos = record_component_original_annos(ctx, existing);
                let field_annos_len = match &ctx.tree(field).kind {
                    TreeKind::VariableDecl { mods: Some(mods), .. } => {
                        match &ctx.tree(*mods).kind {
                            TreeKind::Modifiers { annotations, .. } => annotations.length(),
                            _ => 0,
                        }
                    }
                    _ => 0,
                };
                if original_annos.length() != field_annos_len {
                    let copier = crate::tree::tree_copier::TreeCopier {
                        make: Rc::clone(&self.make),
                        speculative: false,
                        hook: None,
                    };
                    let mut copied: List<TreeId> = List::nil();
                    let mut a = original_annos;
                    while a.non_empty() {
                        let anno = *a.head();
                        a = a.tail();
                        if let Some(copy) = copier.copy(ctx, Some(anno)) {
                            copied = copied.append(copy);
                        }
                    }
                    if let TreeKind::VariableDecl { mods: Some(mods), .. } = &ctx.tree(field).kind
                        && let TreeKind::Modifiers { annotations, .. } =
                            &mut ctx.tree_mut(*mods).kind
                    {
                        *annotations = copied;
                    }
                }
            }
            {
                let member_enter = Rc::clone(&self.member_enter);
                member_enter.borrow()
                    .member_enter(ctx, syms, field, env, attr);
            }
            let field_sym = match &ctx.tree(field).kind {
                TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
                _ => continue,
            };
            let annos = match &ctx.tree(field).kind {
                TreeKind::VariableDecl { mods: Some(mods), .. } => {
                    match &ctx.tree(*mods).kind {
                        TreeKind::Modifiers { annotations, .. } => {
                            let copier = crate::tree::tree_copier::TreeCopier {
                                make: Rc::clone(&self.make),
                                speculative: false,
                                hook: None,
                            };
                            let mut copied: List<TreeId> = List::nil();
                            let mut a = annotations.clone();
                            while a.non_empty() {
                                let anno = *a.head();
                                a = a.tail();
                                if let Some(copy) = copier.copy(ctx, Some(anno)) {
                                    copied = copied.append(copy);
                                }
                            }
                            copied
                        }
                        _ => List::nil(),
                    }
                }
                _ => List::nil(),
            };
            ctx.create_record_component(sym, existing, field_sym, annos);
        }
        self.enter_this_and_super(ctx, sym, env);
        let defs = class_tree_defs(ctx, tree);
        let mut current = defs;
        while current.non_empty() {
            let def = *current.head();
            current = current.tail();
            if tree_info::is_constructor(ctx, def) {
                let member_enter = Rc::clone(&self.member_enter);
                member_enter.borrow()
                    .member_enter(ctx, syms, def, env, attr);
            }
        }
    }

    /// Java: MembersPhase.runPhase(Env<AttrContext> env)
    fn members_phase_run(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) {
        let tree = env.encl_class.expect("class tree");
        let sym = class_tree_symbol(ctx, tree).expect("class symbol");
        let mut default_constructor: Option<TreeId> = None;
        let helper = self.get_default_constructor_helper(ctx, syms, env);
        if let Some(mut helper) = helper {
            {
                let chk_rc = Rc::clone(&self.chk);
                let chk = chk_rc.borrow();
                chk.check_default_constructor(ctx, ctx.tree(tree).pos, sym);
            }
            let dc = self.default_constructor(ctx, syms, tree, helper.as_mut());
            if let TreeKind::ClassDecl { defs, .. } = &mut ctx.tree_mut(tree).kind {
                *defs = defs.clone().prepend(dc);
            }
            default_constructor = Some(dc);
        }
        let is_record = symbol_is_record(ctx, sym);
        if !is_record {
            self.enter_this_and_super(ctx, sym, env);
        }
        let typarams = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { typarams, .. } => typarams.clone(),
            _ => List::nil(),
        };
        let mut params = typarams;
        while params.non_empty() {
            let param = *params.head();
            params = params.tail();
            if let Some(tvar_ty) = ctx.tree(param).ty {
                let chk_rc = Rc::clone(&self.chk);
                let chk = chk_rc.borrow();
                chk.check_non_cyclic(ctx, syms, Some(ctx.tree(param).pos), tvar_ty);
            }
        }
        self.finish_class(ctx, syms, tree, default_constructor, env, attr);
        if self.allow_type_annos {
            let type_annotations = self.type_annotations.borrow().clone();
            if let Some(type_annotations) = type_annotations {
                type_annotations.organize_type_annotations_signatures(env, tree);
                type_annotations.validate_type_annotations_signatures(env, tree);
            }
        }
    }

    // ------------------------------------------------------------------
    // Java: finishClass / addAccessor / addEnumMembers /
    // getCanonicalConstructorDecl / addRecordMembersIfNeeded / lookupMethod
    // (TypeEnter.java:1090-1180)
    // ------------------------------------------------------------------

    /// Enter the members of a class (the MembersPhase finishClass).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the class declaration;
    ///   - default_constructor: the generated default constructor (if any);
    ///   - env: the class environment.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - a class invariant is violated.
    fn finish_class(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        tree: TreeId,
        default_constructor: Option<TreeId>,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) {
        use crate::code::flags;
        let sym = class_tree_symbol(ctx, tree).expect("class symbol");
        let sym_ty = ctx.sym(sym).ty.expect("class type");
        if (class_tree_mods_flags(ctx, tree) & flags::ENUM as i64) != 0
            && !ctx.typ(sym_ty).has_tag(crate::code::type_tag::TypeTag::ERROR)
        {
            let supertype = {
                let syms_borrow = syms;
                let types = Rc::clone(&self.types);
                types.supertype(ctx, syms_borrow, sym_ty, Some(attr))
            };
            if let Some(super_tsym) = ctx.typ(supertype).tsym
                && (ctx.sym(super_tsym).flags_field & flags::ENUM as i64) == 0
            {
                self.add_enum_members(ctx, syms, tree, env, attr);
            }
        }
        let is_record = (ctx.sym(sym).flags_field & flags::RECORD) != 0;
        let defs = class_tree_defs(ctx, tree);
        let defs_to_enter = if is_record {
            let mut already_entered: List<TreeId> = List::nil();
            let record_fields = tree_info::record_fields(ctx, tree);
            let mut fields = record_fields;
            while fields.non_empty() {
                let field = *fields.head();
                fields = fields.tail();
                already_entered = already_entered.prepend(field);
            }
            let mut current = defs.clone();
            while current.non_empty() {
                let def = *current.head();
                current = current.tail();
                if tree_info::is_constructor(ctx, def) && Some(def) != default_constructor {
                    already_entered = already_entered.prepend(def);
                }
            }
            let already: List<TreeId> = already_entered;
            let mut diff: List<TreeId> = List::nil();
            let mut current = defs.clone();
            while current.non_empty() {
                let def = *current.head();
                current = current.tail();
                let mut contained = false;
                let mut a = already.clone();
                while a.non_empty() {
                    if *a.head() == def {
                        contained = true;
                        break;
                    }
                    a = a.tail();
                }
                if !contained {
                    diff = diff.append(def);
                }
            }
            diff
        } else {
            defs
        };
        {
            let member_enter = Rc::clone(&self.member_enter);
            member_enter.borrow()
                .member_enter_list(ctx, syms, defs_to_enter, env, attr);
        }
        if is_record {
            self.add_record_members_if_needed(ctx, syms, tree, env, attr);
        }
        if ctx.sym(sym).is_annotation_type() {
            let annotate = self
                .annotate
                .borrow()
                .clone()
                .expect("TypeEnter.annotate must be injected");
            let completer = annotate.services.annotation_type_source_completer();
            let metadata = crate::comp::annotate::AnnotationTypeMetadata::new(
                sym,
                Some(completer),
            );
            ctx.sym_mut(sym).set_annotation_type_metadata(std::rc::Rc::new(
                std::cell::RefCell::new(metadata),
            ));
        }
    }


    /// Add an accessor for a record component field.
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the record component field declaration;
    ///   - env: the record class environment.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - a record invariant is violated.
    fn add_accessor(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) {
        use crate::code::flags;
        let (name, field_sym) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl { name, sym, .. } => (name.clone(), *sym),
            _ => return,
        };
        let field_sym = field_sym.expect("record field symbol");
        let owner = ctx.sym(field_sym).owner.expect("record owner");
        let impl_sym = self.lookup_method(ctx, syms, owner, name.clone(), List::nil());
        let Some(rec) = ctx.sym(owner).get_record_component(ctx, field_sym) else {
            return;
        };
        let generated = match impl_sym {
            None => true,
            Some(impl_sym) => (ctx.sym(impl_sym).flags_field & flags::GENERATED_MEMBER as i64) != 0,
        };
        if generated {
            let original_annos = record_component_original_annos(ctx, rec);
            let annos = if original_annos.is_empty() {
                original_annos
            } else {
                let copier = crate::tree::tree_copier::TreeCopier {
                    make: Rc::clone(&self.make),
                    speculative: false,
                    hook: None,
                };
                let mut copied: List<TreeId> = List::nil();
                let mut current = original_annos;
                while current.non_empty() {
                    let anno = *current.head();
                    current = current.tail();
                    if let Some(copy) = copier.copy(ctx, Some(anno)) {
                        copied = copied.append(copy);
                    }
                }
                copied
            };
            let record_fields = tree_info::record_fields(ctx, env.encl_class.expect("record tree"));
            let record_field = {
                let mut found: Option<TreeId> = None;
                let mut current = record_fields;
                while current.non_empty() {
                    let rf = *current.head();
                    current = current.tail();
                    if let TreeKind::VariableDecl {
                        name: rf_name, ..
                    } = &ctx.tree(rf).kind
                        && *rf_name == name
                    {
                        found = Some(rf);
                        break;
                    }
                }
                found.expect("record field")
            };
            let vartype = match &ctx.tree(record_field).kind {
                TreeKind::VariableDecl {
                    vartype: Some(vartype),
                    ..
                } => {
                    let copier = crate::tree::tree_copier::TreeCopier {
                        make: Rc::clone(&self.make),
                        speculative: false,
                        hook: None,
                    };
                    copier.copy_tree(ctx, *vartype)
                }
                _ => {
                    let mut make = self.make.borrow_mut();
                    make.at(ctx.tree(record_field).pos);
                    make.erroneous_empty(ctx)
                }
            };
            let getter = {
                let mut make = self.make.borrow_mut();
                make.at(ctx.tree(tree).pos);
                let mods = make.modifiers_with_annotations(
                    ctx,
                    (flags::PUBLIC as i64) | (flags::GENERATED_MEMBER as i64),
                    annos,
                );
                make.method_def(
                    ctx,
                    mods,
                    name.clone(),
                    Some(vartype),
                    List::nil(),
                    None,
                    List::nil(),
                    List::nil(),
                    None,
                    None,
                )
            };
            {
                let member_enter = Rc::clone(&self.member_enter);
                member_enter.borrow()
                    .member_enter(ctx, syms, getter, env, attr);
            }
            let getter_sym = match &ctx.tree(getter).kind {
                TreeKind::MethodDecl {
                    sym: Some(sym), ..
                } => *sym,
                _ => panic!("generated accessor symbol"),
            };
            if let crate::code::symbol::SymbolKind::RecordComponent {
                    accessor,
                    accessor_meth,
                    ..
                } = &mut ctx.sym_mut(rec).kind_data {
                *accessor = Some(getter_sym);
                *accessor_meth = Some(getter);
            }
        } else if let Some(impl_sym) = impl_sym {
            if let crate::code::symbol::SymbolKind::RecordComponent {
                    accessor,
                    ..
                } = &mut ctx.sym_mut(rec).kind_data {
                *accessor = Some(impl_sym);
            }
        }
    }


    /// Add the implicit members (values/valueOf) of an enum type.
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the enum declaration;
    ///   - env: the enum class environment.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - an enum invariant is violated.
    fn add_enum_members(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) {
        use crate::code::flags;
        let sym = class_tree_symbol(ctx, tree).expect("enum symbol");
        let sym_ty = ctx.sym(sym).ty.expect("enum type");
        let values = {
            let mut make = self.make.borrow_mut();
            make.at(ctx.tree(tree).pos);
            let elem_tree = make.type_expr(ctx, syms, sym_ty);
            let values_type = make.type_array(ctx, elem_tree);
            let mods = make.modifiers(ctx, (flags::PUBLIC as i64) | (flags::STATIC as i64));
            make.method_def(
                ctx,
                mods,
                self.names.values.clone(),
                Some(values_type),
                List::nil(),
                None,
                List::nil(),
                List::nil(),
                None,
                None,
            )
        };
        {
            let member_enter = Rc::clone(&self.member_enter);
            member_enter.borrow()
                .member_enter(ctx, syms, values, env, attr);
        }
        let value_of = {
            let mut make = self.make.borrow_mut();
            make.at(ctx.tree(tree).pos);
            let ret = make.type_expr(ctx, syms, sym_ty);
            let param_type = make.type_expr(ctx, syms, syms.string_type);
            let param_mods = make.modifiers(
                ctx,
                (flags::PARAMETER) | (flags::MANDATED as i64),
            );
            let param = make.var_def(
                ctx,
                param_mods,
                make_name(self.names.as_ref(), "name"),
                Some(param_type),
                None,
            );
            let mods = make.modifiers(ctx, (flags::PUBLIC as i64) | (flags::STATIC as i64));
            make.method_def(
                ctx,
                mods,
                self.names.valueOf.clone(),
                Some(ret),
                List::nil(),
                None,
                List::of(param),
                List::nil(),
                None,
                None,
            )
        };
        {
            let member_enter = Rc::clone(&self.member_enter);
            member_enter.borrow()
                .member_enter(ctx, syms, value_of, env, attr);
        }
    }


    /// Add the implicit members of a record type.
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the record declaration;
    ///   - env: the record class environment.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - a record invariant is violated.
    fn add_record_members_if_needed(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
        attr: &mut crate::comp::attr::Attr,
    ) {
        use crate::code::flags;
        let sym = class_tree_symbol(ctx, tree).expect("record symbol");
        let make_no_arg = |this: &TypeEnter, ctx: &mut CompilerContext, syms: &Symtab, name: crate::util::name::Name, ret: TypeId| {
            let mut make = this.make.borrow_mut();
            make.at(ctx.tree(tree).pos);
            let ret_tree = make.type_expr(ctx, syms, ret);
            let mods = make.modifiers(
                ctx,
                (flags::PUBLIC as i64)
                    | (flags::RECORD)
                    | (flags::FINAL as i64)
                    | (flags::GENERATED_MEMBER as i64),
            );
            make.method_def(
                ctx,
                mods,
                name,
                Some(ret_tree),
                List::nil(),
                None,
                List::nil(),
                List::nil(),
                None,
                None,
            )
        };
        if self
            .lookup_method(ctx, syms, sym, self.names.toString.clone(), List::nil())
            .is_none()
        {
            let to_string = make_no_arg(
                self,
                ctx,
                syms,
                self.names.toString.clone(),
                syms.string_type,
            );
            let member_enter = Rc::clone(&self.member_enter);
            member_enter.borrow()
                .member_enter(ctx, syms, to_string, env, attr);
        }
        if self
            .lookup_method(ctx, syms, sym, self.names.hashCode.clone(), List::nil())
            .is_none()
        {
            let hash_code = make_no_arg(
                self,
                ctx,
                syms,
                self.names.hashCode.clone(),
                syms.int_type,
            );
            let member_enter = Rc::clone(&self.member_enter);
            member_enter.borrow()
                .member_enter(ctx, syms, hash_code, env, attr);
        }
        if self
            .lookup_method(ctx, syms, sym, self.names.equals.clone(), List::of(syms.object_type))
            .is_none()
        {
            let equals = {
                let mut make = self.make.borrow_mut();
                make.at(ctx.tree(tree).pos);
                let ret = make.type_expr(ctx, syms, syms.boolean_type);
                let object_ty = make.type_expr(ctx, syms, syms.object_type);
                let param_mods = make.modifiers(ctx, flags::PARAMETER);
                let param = make.var_def(
                    ctx,
                    param_mods,
                    make_name(self.names.as_ref(), "o"),
                    Some(object_ty),
                    None,
                );
                let mods = make.modifiers(
                    ctx,
                    (flags::PUBLIC as i64)
                        | (flags::RECORD)
                        | (flags::FINAL as i64)
                        | (flags::GENERATED_MEMBER as i64),
                );
                make.method_def(
                    ctx,
                    mods,
                    self.names.equals.clone(),
                    Some(ret),
                    List::nil(),
                    None,
                    List::of(param),
                    List::nil(),
                    None,
                    None,
                )
            };
            let member_enter = Rc::clone(&self.member_enter);
            member_enter.borrow()
                .member_enter(ctx, syms, equals, env, attr);
        }
        // fields can't be varargs: clear the flag.
        let record_fields = tree_info::record_fields(ctx, tree);
        let mut current = record_fields.clone();
        while current.non_empty() {
            let field = *current.head();
            current = current.tail();
            let field_sym = match &ctx.tree(field).kind {
                TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
                _ => continue,
            };
            let mods = match &ctx.tree(field).kind {
                TreeKind::VariableDecl { mods, .. } => *mods,
                _ => None,
            };
            if let Some(mods) = mods
                && let TreeKind::Modifiers { flags, .. } = &mut ctx.tree_mut(mods).kind
            {
                *flags &= !(flags::VARARGS);
            }
            ctx.sym_mut(field_sym).flags_field &= !(flags::VARARGS);
        }
        // add the accessors.
        let object_sym = ctx.typ(syms.object_type).tsym.expect("object symbol");
        let mut current = record_fields;
        while current.non_empty() {
            let field = *current.head();
            current = current.tail();
            let name = match &ctx.tree(field).kind {
                TreeKind::VariableDecl { name, .. } => name.clone(),
                _ => continue,
            };
            if self
                .lookup_method(ctx, syms, object_sym, name, List::nil())
                .is_none()
            {
                self.add_accessor(ctx, syms, field, env, attr);
            }
        }
    }


    /// Return the canonical constructor of a record, if the record declares
    /// one with exactly the component types.
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the record class declaration.
    ///
    /// # Returns
    ///   - the canonical constructor tree, if any.
    ///
    /// # Panics
    ///   - a record invariant is violated.
    fn get_canonical_constructor_decl(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        tree: TreeId,
    ) -> Option<TreeId> {
        let sym = class_tree_symbol(ctx, tree).expect("class symbol");
        if !symbol_is_record(ctx, sym) {
            return None;
        }
        let record_fields = tree_info::record_fields(ctx, tree);
        let mut record_field_types: List<TypeId> = List::nil();
        let mut current = record_fields;
        while current.non_empty() {
            let field = *current.head();
            current = current.tail();
            if let TreeKind::VariableDecl {
                sym: Some(field_sym),
                ..
            } = &ctx.tree(field).kind
                && let Some(ty) = ctx.sym(*field_sym).ty
            {
                record_field_types = record_field_types.append(ty);
            }
        }
        let record_component_erased_types = {
            let syms_borrow = syms;
            let types = Rc::clone(&self.types);
            let mut erased: List<TypeId> = List::nil();
            let mut current = record_field_types;
            while current.non_empty() {
                let ty = *current.head();
                current = current.tail();
                erased = erased.append(types.erasure(ctx, syms_borrow, ty));
            }
            erased
        };
        let defs = class_tree_defs(ctx, tree);
        let mut current = defs;
        while current.non_empty() {
            let def = *current.head();
            current = current.tail();
            if tree_info::is_constructor(ctx, def) {
                let params = match &ctx.tree(def).kind {
                    TreeKind::MethodDecl { params, .. } => params.clone(),
                    _ => List::nil(),
                };
                let mut param_types: List<TypeId> = List::nil();
                let mut p = params;
                while p.non_empty() {
                    let param = *p.head();
                    p = p.tail();
                    if let TreeKind::VariableDecl {
                        sym: Some(param_sym),
                        ..
                    } = &ctx.tree(param).kind
                        && let Some(ty) = ctx.sym(*param_sym).ty
                    {
                        param_types = param_types.append(ty);
                    }
                }
                let mut erased_params: List<TypeId> = List::nil();
                let syms_borrow = syms;
                let types = Rc::clone(&self.types);
                let mut current2 = param_types;
                while current2.non_empty() {
                    let ty = *current2.head();
                    current2 = current2.tail();
                    erased_params = erased_params.append(types.erasure(ctx, syms_borrow, ty));
                }
                let same = {
                    let types = Rc::clone(&self.types);
                    let syms_borrow = syms;
                    types.is_same_types(
                        ctx,
                        syms_borrow,
                        &erased_params,
                        &record_component_erased_types,
                    )
                };
                if same {
                    return Some(def);
                }
            }
        }
        None
    }

    /// Look up a method with the given name and parameter types in a class.
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - c: the class symbol;
    ///   - name: the method name;
    ///   - argtypes: the parameter types.
    ///
    /// # Returns
    ///   - the matching method symbol, if any.
    ///
    /// # Panics
    ///   - none.
    fn lookup_method(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        c: SymbolId,
        name: crate::util::name::Name,
        argtypes: List<TypeId>,
    ) -> Option<SymbolId> {
        let scope = class_members_scope(ctx, c)?;
        let symbols = ctx.scope_get_symbols_by_name(
            scope,
            &name,
            LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        for sym in symbols {
            if ctx.sym(sym).kind != crate::code::kinds::Kind::MTH {
                continue;
            }
            let Some(ty) = ctx.sym(sym).ty else {
                continue;
            };
            let parameter_types = ctx.typ(ty).get_parameter_types(ctx);
            if self
                .types
                .is_same_types(ctx, syms, &parameter_types, &argtypes)
            {
                return Some(sym);
            }
        }
        None
    }

    /// Generate the default constructor for a class.
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - tree: the class declaration;
    ///   - helper: the default constructor helper.
    ///
    /// # Returns
    ///   - the generated constructor tree.
    ///
    /// # Panics
    ///   - the helper services are not initialized.
    fn default_constructor(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        tree: TreeId,
        helper: &mut dyn DefaultConstructorHelper,
    ) -> TreeId {
        let names = Rc::clone(&self.names);
        let init_type = helper
            .constructor_type(ctx, syms, &self.types, &names)
            .expect("constructor type");
        let init_sym = helper
            .constructor_symbol(ctx, syms, &self.types, &names)
            .expect("constructor symbol");
        let mut stats: List<TreeId> = List::nil();
        let owner = helper.owner();
        let owner_ty = ctx.sym(owner).ty.expect("owner type");
        if owner_ty != syms.object_type {
            let enclosing = helper.enclosing_type(ctx, syms, &self.types);
            let mut make = self.make.borrow_mut();
            make.at(ctx.tree(tree).pos);
            let meth = if enclosing.is_some() {
                let params = match &ctx.sym(init_sym).kind_data {
                    crate::code::symbol::SymbolKind::MethodSymbol { params, .. } => {
                        params.clone()
                    }
                    _ => List::nil(),
                };
                let first = *params.head();
                let first_tree = make.ident(ctx, ctx.sym(first).name.clone());
                make.select(ctx, first_tree, names._super.clone())
            } else {
                make.ident(ctx, names._super.clone())
            };
            let typeargs = {
                let type_args = ctx.typ(init_type).get_type_arguments();
                if type_args.non_empty() {
                    make.types(ctx, syms, type_args)
                } else {
                    List::nil()
                }
            };
            let super_names = helper.super_args(ctx, syms, &self.types, &names, &mut make);
            let mut super_args: List<TreeId> = List::nil();
            let mut current = super_names;
            while current.non_empty() {
                let name = current.head().clone();
                current = current.tail();
                super_args = super_args.append(make.ident(ctx, name));
            }
            let apply = make.apply(ctx, typeargs, meth, super_args);
            let exec = make.exec(ctx, apply);
            stats = stats.append(exec);
        }
        let body = {
            let mut make = self.make.borrow_mut();
            make.block(ctx, 0, stats)
        };
        let result = {
            let mut make = self.make.borrow_mut();
            make.method_def_symbol(ctx, syms, init_sym, Some(body))
        };
        helper.final_adjustment(ctx, syms, &names, result)
    }

    /// Return the default constructor helper of a class declaration.
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - env: the class environment.
    ///
    /// # Returns
    ///   - the helper, or None when the class declares a constructor.
    ///
    /// # Panics
    ///   - a class invariant is violated.
    fn get_default_constructor_helper(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        env: &Rc<Env<AttrContext>>,
    ) -> Option<Box<dyn DefaultConstructorHelper>> {
        use crate::code::flags;
        let tree = env.encl_class.expect("class tree");
        let sym = class_tree_symbol(ctx, tree).expect("class symbol");
        let defs = class_tree_defs(ctx, tree);
        let is_class_without_init = (ctx.sym(sym).flags_field & flags::INTERFACE as i64) == 0
            && !tree_info::has_constructors(ctx, &defs);
        let is_record = symbol_is_record(ctx, sym);
        let mut helper: Option<Box<dyn DefaultConstructorHelper>> = None;
        if is_class_without_init && !is_record {
            helper = Some(Box::new(BasicConstructorHelper::new(sym)));
            if ctx.sym(sym).name.is_empty() {
                let next_tree = env.next.as_ref().and_then(|next| next.tree);
                let mut enclosing_constructor: Option<SymbolId> = None;
                let mut encl: Option<TypeId> = None;
                if let Some(next_tree) = next_tree
                    && let TreeKind::NewClass {
                        constructor: Some(constructor),
                        encl: encl_tree,
                        ..
                    } = &ctx.tree(next_tree).kind
                {
                    if ctx.sym(*constructor).kind != crate::code::kinds::Kind::ERR {
                        enclosing_constructor = Some(*constructor);
                        encl = encl_tree.and_then(|tree| ctx.tree(tree).ty);
                        helper = Some(Box::new(AnonClassConstructorHelper::new(
                            sym,
                            *constructor,
                            encl,
                        )));
                    } else {
                        helper = None;
                    }
                }
            }
        }
        if is_record {
            let canonical_init = self.get_canonical_constructor_decl(ctx, syms, tree);
            if is_class_without_init || canonical_init.is_none() {
                let record_fields = tree_info::record_fields(ctx, tree);
                helper = Some(Box::new(RecordConstructorHelper::new(sym, record_fields)));
            }
            if let Some(canonical_init) = canonical_init {
                let canonical_sym = match &ctx.tree(canonical_init).kind {
                    TreeKind::MethodDecl {
                        sym: Some(sym), ..
                    } => *sym,
                    _ => panic!("record canonical constructor symbol"),
                };
                ctx.sym_mut(canonical_sym).flags_field |= flags::RECORD;
            }
        }
        helper
    }

}

// ---------------------------------------------------------------------------
// Local tree/symbol accessors (arena adapters; registered).
// ---------------------------------------------------------------------------

fn compilation_unit_defs(ctx: &CompilerContext, unit: TreeId) -> List<TreeId> {
    match &ctx.tree(unit).kind {
        TreeKind::CompilationUnit { defs, .. } => defs.clone(),
        _ => List::nil(),
    }
}

fn compilation_unit_sourcefile(
    ctx: &CompilerContext,
    unit: TreeId,
) -> Option<javac_tools::tools::java_file_manager::JavaFileObjectRef> {
    match &ctx.tree(unit).kind {
        TreeKind::CompilationUnit { sourcefile, .. } => sourcefile.clone(),
        _ => None,
    }
}

fn compilation_unit_module(ctx: &CompilerContext, unit: TreeId) -> Option<SymbolId> {
    match &ctx.tree(unit).kind {
        TreeKind::CompilationUnit { modle, .. } => *modle,
        _ => None,
    }
}

fn compilation_unit_package(ctx: &CompilerContext, unit: TreeId) -> Option<SymbolId> {
    match &ctx.tree(unit).kind {
        TreeKind::CompilationUnit { packge, .. } => *packge,
        _ => None,
    }
}

fn compilation_unit_package_decl(ctx: &CompilerContext, unit: TreeId) -> Option<TreeId> {
    match &ctx.tree(unit).kind {
        TreeKind::CompilationUnit { defs, .. } => defs
            .iter()
            .find(|def| ctx.tree(*def).has_tag(Tag::PACKAGEDEF)),
        _ => None,
    }
}

fn compilation_unit_module_decl(ctx: &CompilerContext, unit: TreeId) -> Option<TreeId> {
    match &ctx.tree(unit).kind {
        TreeKind::CompilationUnit { defs, .. } => defs
            .iter()
            .find(|def| ctx.tree(*def).has_tag(Tag::MODULEDEF)),
        _ => None,
    }
}

fn compilation_unit_imports(ctx: &CompilerContext, unit: TreeId) -> List<TreeId> {
    match &ctx.tree(unit).kind {
        TreeKind::CompilationUnit { defs, .. } => {
            let imports: Vec<TreeId> = defs
                .iter()
                .filter(|def| ctx.tree(*def).has_tag(Tag::IMPORT))
                .collect();
            List::from_iterable(imports)
        }
        _ => List::nil(),
    }
}

fn compilation_unit_named_import_scope(ctx: &CompilerContext, unit: TreeId) -> Option<ScopeId> {
    match &ctx.tree(unit).kind {
        TreeKind::CompilationUnit {
            named_import_scope, ..
        } => *named_import_scope,
        _ => None,
    }
}

fn compilation_unit_star_import_scope(ctx: &CompilerContext, unit: TreeId) -> Option<ScopeId> {
    match &ctx.tree(unit).kind {
        TreeKind::CompilationUnit {
            star_import_scope, ..
        } => *star_import_scope,
        _ => None,
    }
}

fn class_tree_symbol(ctx: &CompilerContext, tree: TreeId) -> Option<SymbolId> {
    match &ctx.tree(tree).kind {
        TreeKind::ClassDecl { sym, .. } => *sym,
        _ => None,
    }
}

fn class_members_scope(ctx: &CompilerContext, class: SymbolId) -> Option<ScopeId> {
    match &ctx.sym(class).kind_data {
        SymbolKind::ClassSymbol { members_field, .. }
        | SymbolKind::PackageSymbol { members_field, .. } => *members_field,
        _ => None,
    }
}

fn symbol_exists(ctx: &CompilerContext, symbol: SymbolId) -> bool {
    (ctx.sym(symbol).flags_field & flags::EXISTS as i64) != 0
}

fn package_decl_is_non_null(_ctx: &CompilerContext, _tree: TreeId) -> bool {
    // Java: tree.pid != null — the local PackageDecl keeps the pid tree.
    true
}

fn import_qualid(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::Import { qualid, .. } => *qualid,
        _ => None,
    }
}

fn import_static_import(ctx: &CompilerContext, tree: TreeId) -> bool {
    match &ctx.tree(tree).kind {
        TreeKind::Import { static_import, .. } => *static_import,
        _ => false,
    }
}

fn set_import_scope(ctx: &mut CompilerContext, tree: TreeId, scope: ScopeId) {
    if let TreeKind::Import { import_scope, .. } = &mut ctx.tree_mut(tree).kind {
        *import_scope = Some(scope);
    }
}

fn module_decl_annotations(ctx: &CompilerContext, decl: TreeId) -> List<TreeId> {
    match &ctx.tree(decl).kind {
        TreeKind::ModuleDecl { mods, .. } => mods
            .and_then(|mods| match &ctx.tree(mods).kind {
                TreeKind::Modifiers { annotations, .. } => Some(annotations.clone()),
                _ => None,
            })
            .unwrap_or_else(List::nil),
        _ => List::nil(),
    }
}

fn module_decl_symbol(ctx: &CompilerContext, decl: TreeId) -> Option<SymbolId> {
    match &ctx.tree(decl).kind {
        TreeKind::ModuleDecl { sym, .. } => *sym,
        _ => None,
    }
}

/// Java: Env.toplevel (the compilation unit; the port models Java null -> None).
fn toplevel(env: &Rc<Env<AttrContext>>) -> TreeId {
    env.toplevel.expect("env.toplevel")
}

/// Java: Type.getOriginalType() — the port exposes the error type's
/// original_type field (registered adapter).
fn original_type(ctx: &CompilerContext, t: crate::ids::TypeId) -> crate::ids::TypeId {
    match &ctx.typ(t).kind {
        TypeKindData::ErrorType {
            original_type: Some(original),
            ..
        } => *original,
        _ => t,
    }
}

/// Java: JCAnnotation.annotationType (the port keeps the arena tree id).
fn annotation_type(ctx: &CompilerContext, annotation: TreeId) -> Option<TreeId> {
    match &ctx.tree(annotation).kind {
        TreeKind::Annotation { annotation_type, .. } => *annotation_type,
        _ => None,
    }
}

/// Java: JCAnnotation.args
fn annotation_args(ctx: &CompilerContext, annotation: TreeId) -> List<TreeId> {
    match &ctx.tree(annotation).kind {
        TreeKind::Annotation { args, .. } => args.clone(),
        _ => List::nil(),
    }
}

/// Java: JCAssign.lhs/rhs
fn assign_parts(ctx: &CompilerContext, assign: TreeId) -> (Option<TreeId>, Option<TreeId>) {
    match &ctx.tree(assign).kind {
        TreeKind::Assign { lhs, rhs } => (*lhs, *rhs),
        _ => (None, None),
    }
}

/// Java: Boolean.TRUE.equals(((JCLiteral) rhs).getValue())
fn literal_is_true(ctx: &CompilerContext, literal: TreeId) -> bool {
    match &ctx.tree(literal).kind {
        TreeKind::Literal {
            value: Some(crate::tree::jc_tree::LiteralValue::Boolean(value)),
            ..
        } => *value,
        _ => false,
    }
}

/// Java: TypeEnter.DefaultConstructorHelper (TypeEnter.java:1180-1220)
///
/// Adapter: the Java interface is implemented by the three concrete helper
/// classes below; the methods take the compiler services explicitly because the
/// Rust helpers cannot capture the TypeEnter instance (constructor cycle).
pub trait DefaultConstructorHelper {
    /// Java: Type constructorType()
    fn constructor_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &crate::code::types::Types,
        names: &Names,
    ) -> Option<TypeId>;
    /// Java: MethodSymbol constructorSymbol()
    fn constructor_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &crate::code::types::Types,
        names: &Names,
    ) -> Option<SymbolId>;
    /// Java: Type enclosingType()
    fn enclosing_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &crate::code::types::Types,
    ) -> Option<TypeId>;
    /// Java: TypeSymbol owner()
    fn owner(&self) -> SymbolId;
    /// Java: List<Name> superArgs()
    fn super_args(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &crate::code::types::Types,
        names: &Names,
        make: &mut crate::tree::tree_maker::TreeMaker,
    ) -> List<crate::util::name::Name>;
    /// Java: default JCMethodDecl finalAdjustment(JCMethodDecl md) { return md; }
    fn final_adjustment(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &Symtab,
        _names: &Names,
        method: TreeId,
    ) -> TreeId {
        method
    }
}

/// Java: class BasicConstructorHelper implements DefaultConstructorHelper
pub struct BasicConstructorHelper {
    owner: SymbolId,
    constructor_type: Option<TypeId>,
    constructor_symbol: Option<SymbolId>,
}

impl BasicConstructorHelper {
    /// Java: BasicConstructorHelper(TypeSymbol owner)
    pub fn new(owner: SymbolId) -> BasicConstructorHelper {
        BasicConstructorHelper {
            owner,
            constructor_type: None,
            constructor_symbol: None,
        }
    }
}

impl DefaultConstructorHelper for BasicConstructorHelper {
    /// Java: public Type constructorType()
    fn constructor_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        _types: &crate::code::types::Types,
        _names: &Names,
    ) -> Option<TypeId> {
        if self.constructor_type.is_none() {
            let method_type = crate::code::types::new_method_type_2(
                ctx,
                List::nil(),
                Some(syms.void_type),
                List::nil(),
                Some(syms.method_class),
            );
            self.constructor_type = Some(method_type);
        }
        self.constructor_type
    }

    /// Java: public MethodSymbol constructorSymbol()
    fn constructor_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &crate::code::types::Types,
        names: &Names,
    ) -> Option<SymbolId> {
        if self.constructor_symbol.is_none() {
            use crate::code::flags;
            let owner_flags = ctx.sym(self.owner).flags_field;
            let owner_ty = ctx.sym(self.owner).ty.expect("owner type");
            let flags_value = if (owner_flags & flags::ENUM as i64) != 0
                && {
                    let supertype = types.supertype(ctx, syms, owner_ty, None);
                    ctx.typ(supertype).tsym == Some(syms.enum_sym)
                }
            {
                (flags::PRIVATE as i64) | (flags::GENERATEDCONSTR)
            } else {
                (owner_flags & flags::AccessFlags as i64)
                    | (flags::GENERATEDCONSTR)
            };
            let ctor_type = self
                .constructor_type(ctx, syms, types, names)
                .expect("constructor type");
            let symbol = crate::code::symbol::Symbol::new_method_symbol(
                flags_value,
                names.init.clone(),
                ctor_type,
                self.owner,
            );
            self.constructor_symbol = Some(ctx.alloc_symbol(symbol));
        }
        self.constructor_symbol
    }

    /// Java: public Type enclosingType()
    fn enclosing_type(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &Symtab,
        _types: &crate::code::types::Types,
    ) -> Option<TypeId> {
        None
    }

    /// Java: public TypeSymbol owner()
    fn owner(&self) -> SymbolId {
        self.owner
    }

    /// Java: public List<Name> superArgs()
    fn super_args(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &Symtab,
        _types: &crate::code::types::Types,
        _names: &Names,
        _make: &mut crate::tree::tree_maker::TreeMaker,
    ) -> List<crate::util::name::Name> {
        List::nil()
    }
}

/// Java: class AnonClassConstructorHelper extends BasicConstructorHelper
pub struct AnonClassConstructorHelper {
    base: BasicConstructorHelper,
    constr: SymbolId,
    encl: Option<TypeId>,
    based: bool,
}

impl AnonClassConstructorHelper {
    /// Java: AnonClassConstructorHelper(TypeSymbol owner, MethodSymbol constr,
    /// JCExpression encl)
    pub fn new(owner: SymbolId, constr: SymbolId, encl: Option<TypeId>) -> AnonClassConstructorHelper {
        AnonClassConstructorHelper {
            base: BasicConstructorHelper::new(owner),
            constr,
            encl,
            based: false,
        }
    }

    fn enclosing_type_value(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &Symtab,
        _types: &crate::code::types::Types,
    ) -> Option<TypeId> {
        self.encl
    }
}

impl DefaultConstructorHelper for AnonClassConstructorHelper {
    /// Java: public Type constructorType()
    fn constructor_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &crate::code::types::Types,
        _names: &Names,
    ) -> Option<TypeId> {
        if self.base.constructor_type.is_none() {
            let owner_ty = ctx.sym(self.base.owner).ty.expect("owner type");
            let mut ctype = types.member_type(ctx, syms, owner_ty, self.constr);
            let enclosing = self.encl;
            if let Some(enclosing) = enclosing {
                let params = ctx
                    .typ(ctype)
                    .get_parameter_types(ctx);
                ctype = types.create_method_type_with_parameters(
                    ctx,
                    ctype,
                    params.prepend(enclosing),
                );
                self.based = true;
            }
            self.base.constructor_type = Some(ctype);
        }
        self.base.constructor_type
    }

    /// Java: public MethodSymbol constructorSymbol()
    fn constructor_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &crate::code::types::Types,
        names: &Names,
    ) -> Option<SymbolId> {
        if self.base.constructor_symbol.is_some() {
            return self.base.constructor_symbol;
        }
        use crate::code::flags;
        let csym = self
            .base
            .constructor_symbol(ctx, syms, types, names)
            .expect("basic constructor symbol");
        let constr_flags = ctx.sym(self.constr).flags_field;
        ctx.sym_mut(csym).flags_field |= (flags::ANONCONSTR as i64)
            | (constr_flags & flags::VARARGS);
        if self.based {
            ctx.sym_mut(csym).flags_field |= flags::ANONCONSTR_BASED;
        }
        let ctor_type = self
            .constructor_type(ctx, syms, types, names)
            .expect("constructor type");
        let mut argtypes = ctx.typ(ctor_type).get_parameter_types(ctx);
        let mut params: List<SymbolId> = List::nil();
        if self.encl.is_some() {
            let enclosing = self.encl.expect("enclosing type");
            let name = names.from_utf(b"x0");
            let symbol = crate::code::symbol::Symbol::new_var_symbol(
                flags::PARAMETER,
                name,
                enclosing,
                csym,
            );
            params = params.prepend(ctx.alloc_symbol(symbol));
            argtypes = argtypes.tail();
        }
        let constr_params = match &ctx.sym(self.constr).kind_data {
            crate::code::symbol::SymbolKind::MethodSymbol { params, .. } => params.clone(),
            _ => List::nil(),
        };
        let mut current = constr_params;
        while current.non_empty() {
            let p = *current.head();
            current = current.tail();
            let p_flags = ctx.sym(p).flags_field;
            let head = argtypes.head();
            let symbol = crate::code::symbol::Symbol::new_var_symbol(
                (flags::PARAMETER) | p_flags,
                ctx.sym(p).name.clone(),
                *head,
                csym,
            );
            params = params.append(ctx.alloc_symbol(symbol));
            argtypes = argtypes.tail();
        }
        ctx.sym_mut(csym).set_params(params);
        self.base.constructor_symbol = Some(csym);
        Some(csym)
    }

    /// Java: public Type enclosingType()
    fn enclosing_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &crate::code::types::Types,
    ) -> Option<TypeId> {
        self.enclosing_type_value(ctx, syms, types)
    }

    /// Java: public TypeSymbol owner()
    fn owner(&self) -> SymbolId {
        self.base.owner
    }

    /// Java: public List<Name> superArgs()
    fn super_args(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &crate::code::types::Types,
        names: &Names,
        make: &mut crate::tree::tree_maker::TreeMaker,
    ) -> List<crate::util::name::Name> {
        let ctor_symbol = self
            .constructor_symbol(ctx, syms, types, names)
            .expect("constructor symbol");
        let ctor_type = self
            .constructor_type(ctx, syms, types, names)
            .expect("constructor type");
        let argtypes = ctx.typ(ctor_type).get_parameter_types(ctx);
        let make_params = make.params(ctx, syms, argtypes, ctor_symbol);
        let mut current = make_params;
        if self.encl.is_some() {
            current = current.tail();
        }
        let mut result: List<crate::util::name::Name> = List::nil();
        while current.non_empty() {
            let param = *current.head();
            let name = match &ctx.tree(param).kind {
                TreeKind::VariableDecl { name, .. } => name.clone(),
                _ => names.empty.clone(),
            };
            result = result.append(name);
            current = current.tail();
        }
        result
    }
}

/// Java: class RecordConstructorHelper extends BasicConstructorHelper
pub struct RecordConstructorHelper {
    base: BasicConstructorHelper,
    record_field_decls: List<TreeId>,
    last_is_varargs: bool,
}

/// Return the record component symbols of a class.
///
/// # Arguments
///   - ctx: the compiler context;
///   - class: the record class symbol.
///
/// # Returns
///   - the record component list.
///
/// # Panics
///   - none.
fn record_components(ctx: &CompilerContext, class: SymbolId) -> List<SymbolId> {
    match &ctx.sym(class).kind_data {
        crate::code::symbol::SymbolKind::ClassSymbol {
            record_components, ..
        } => record_components.clone(),
        _ => List::nil(),
    }
}

impl RecordConstructorHelper {
    /// Java: RecordConstructorHelper(ClassSymbol owner, List<JCVariableDecl>
    /// recordFieldDecls)
    pub fn new(owner: SymbolId, record_field_decls: List<TreeId>) -> RecordConstructorHelper {
        RecordConstructorHelper {
            base: BasicConstructorHelper::new(owner),
            record_field_decls,
            last_is_varargs: false,
        }
    }
}

impl DefaultConstructorHelper for RecordConstructorHelper {
    /// Java: public Type constructorType()
    fn constructor_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &crate::code::types::Types,
        _names: &Names,
    ) -> Option<TypeId> {
        if self.base.constructor_type.is_none() {
            let mut argtypes: List<TypeId> = List::nil();
            let mut current = self.record_field_decls.clone();
            while current.non_empty() {
                let field = *current.head();
                current = current.tail();
                if let TreeKind::VariableDecl {
                    sym: Some(field_sym),
                    ..
                } = &ctx.tree(field).kind
                {
                    let field_ty = ctx.sym(*field_sym).ty.expect("record field type");
                    let is_last = current.is_empty();
                    let ty = if is_last && self.last_is_varargs {
                        types.elemtype(ctx, syms, field_ty)
                    } else {
                        field_ty
                    };
                    argtypes = argtypes.append(ty);
                }
            }
            let method_type = crate::code::types::new_method_type_2(
                ctx,
                argtypes,
                Some(syms.void_type),
                List::nil(),
                Some(syms.method_class),
            );
            self.base.constructor_type = Some(method_type);
        }
        self.base.constructor_type
    }

    /// Java: public MethodSymbol constructorSymbol()
    fn constructor_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &crate::code::types::Types,
        names: &Names,
    ) -> Option<SymbolId> {
        if self.base.constructor_symbol.is_some() {
            return self.base.constructor_symbol;
        }
        use crate::code::flags;
        let csym = self
            .base
            .constructor_symbol(ctx, syms, types, names)
            .expect("basic constructor symbol");
        ctx.sym_mut(csym).flags_field |= (flags::COMPACT_RECORD_CONSTRUCTOR)
            | (flags::GENERATEDCONSTR);
        let mut params: List<SymbolId> = List::nil();
        let mut current = self.record_field_decls.clone();
        while current.non_empty() {
            let field = *current.head();
            current = current.tail();
            let is_last = current.is_empty();
            if let TreeKind::VariableDecl {
                sym: Some(field_sym),
                ..
            } = &ctx.tree(field).kind
            {
                let field_ty = ctx.sym(*field_sym).ty.expect("record field type");
                let mut parameter_flags = (flags::GENERATED_MEMBER as i64)
                    | (flags::PARAMETER)
                    | (flags::RECORD);
                if is_last && self.last_is_varargs {
                    parameter_flags |= flags::VARARGS;
                }
                let symbol = crate::code::symbol::Symbol::new_var_symbol(
                    parameter_flags,
                    ctx.sym(*field_sym).name.clone(),
                    field_ty,
                    csym,
                );
                params = params.append(ctx.alloc_symbol(symbol));
            }
        }
        ctx.sym_mut(csym).set_params(params);
        ctx.sym_mut(csym).flags_field |= flags::RECORD;
        self.base.constructor_symbol = Some(csym);
        Some(csym)
    }

    /// Java: public Type enclosingType()
    fn enclosing_type(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &Symtab,
        _types: &crate::code::types::Types,
    ) -> Option<TypeId> {
        None
    }

    /// Java: public TypeSymbol owner()
    fn owner(&self) -> SymbolId {
        self.base.owner
    }

    /// Java: public List<Name> superArgs()
    fn super_args(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &Symtab,
        _types: &crate::code::types::Types,
        _names: &Names,
        _make: &mut crate::tree::tree_maker::TreeMaker,
    ) -> List<crate::util::name::Name> {
        List::nil()
    }

    /// Java: public JCMethodDecl finalAdjustment(JCMethodDecl md)
    fn final_adjustment(
        &mut self,
        ctx: &mut CompilerContext,
        _syms: &Symtab,
        _names: &Names,
        method: TreeId,
    ) -> TreeId {
        let owner = self.base.owner;
        let mut field_decls = self.record_field_decls.clone();
        let params = match &ctx.tree(method).kind {
            TreeKind::MethodDecl { params, .. } => params.clone(),
            _ => List::nil(),
        };
        let mut current = params;
        while current.non_empty() {
            let arg = *current.head();
            current = current.tail();
            let arg_sym = match &ctx.tree(arg).kind {
                TreeKind::VariableDecl {
                    sym: Some(sym), ..
                } => *sym,
                _ => continue,
            };
            let Some(rc) = ctx.sym(owner).get_record_component(ctx, arg_sym) else {
                continue;
            };
            let original_annos = record_component_original_annos(ctx, rc);
            let copier = crate::tree::tree_copier::TreeCopier {
                make: Rc::new(std::cell::RefCell::new(
                    crate::tree::tree_maker::TreeMaker::new(None),
                )),
                speculative: false,
                hook: None,
            };
            let annos = if original_annos.is_empty() {
                List::nil()
            } else {
                let mut copied: List<TreeId> = List::nil();
                let mut a = original_annos;
                while a.non_empty() {
                    let anno = *a.head();
                    a = a.tail();
                    if let Some(copy) = copier.copy(ctx, Some(anno)) {
                        copied = copied.append(copy);
                    }
                }
                copied
            };
            if let TreeKind::VariableDecl { mods, .. } = &mut ctx.tree_mut(arg).kind
                && let Some(mods) = *mods
                && let TreeKind::Modifiers {
                    annotations, ..
                } = &mut ctx.tree_mut(mods).kind
            {
                *annotations = annos;
            }
            if !field_decls.is_empty() {
                let field = *field_decls.head();
                field_decls = field_decls.tail();
                if let TreeKind::VariableDecl {
                    vartype: Some(vartype),
                    ..
                } = &ctx.tree(field).kind
                {
                    let copied = copier.copy_tree(ctx, *vartype);
                    if let TreeKind::VariableDecl { vartype, .. } =
                        &mut ctx.tree_mut(arg).kind
                    {
                        *vartype = Some(copied);
                    }
                }
            }
        }
        method
    }
}

// ---------------------------------------------------------------------------
// Local accessors for the TypeEnter phase code.
// ---------------------------------------------------------------------------

fn class_tree_typarams(ctx: &CompilerContext, tree: TreeId) -> Option<List<TreeId>> {
    match &ctx.tree(tree).kind {
        TreeKind::ClassDecl { typarams, .. } => Some(typarams.clone()),
        _ => None,
    }
}

fn class_tree_defs(ctx: &CompilerContext, tree: TreeId) -> List<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::ClassDecl { defs, .. } => defs.clone(),
        _ => List::nil(),
    }
}

fn ctx_tree_has_tag(ctx: &CompilerContext, tree: TreeId, tag: Tag) -> bool {
    ctx.tree(tree).has_tag(tag)
}

fn type_apply_clazz(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::TypeApply { clazz, .. } => *clazz,
        _ => None,
    }
}

fn class_supertype(ctx: &CompilerContext, ty: crate::ids::TypeId) -> Option<crate::ids::TypeId> {
    match &ctx.typ(ty).kind {
        TypeKindData::ClassType {
            supertype_field, ..
        } => *supertype_field,
        _ => None,
    }
}

fn symbol_is_record(ctx: &CompilerContext, symbol: SymbolId) -> bool {
    (ctx.sym(symbol).flags_field & flags::RECORD) != 0
}

fn set_var_symbol_pos(ctx: &mut CompilerContext, symbol: SymbolId, pos: i32) {
    if let SymbolKind::VarSymbol { pos: slot, .. } = &mut ctx.sym_mut(symbol).kind_data {
        *slot = pos;
    }
}

impl TypeEnter {
    /// Inject the Attr service (the upstream constructor receives it; the
    /// local construction order requires a setter).
    ///
    /// # Arguments
    ///   - attr: the Attr service.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    pub fn set_attr(&self, attr: Rc<RefCell<crate::comp::attr::Attr>>) {
        *self.attr.borrow_mut() = Some(attr);
    }

    /// Inject the TypeAnnotations service.
    ///
    /// # Arguments
    ///   - type_annotations: the TypeAnnotations service.
    ///
    /// # Returns
    ///   - none.
    ///
    /// # Panics
    ///   - none.
    pub fn set_type_annotations(
        &self,
        type_annotations: Rc<crate::code::type_annotations::TypeAnnotations>,
    ) {
        *self.type_annotations.borrow_mut() = Some(type_annotations);
    }
}

/// Return the (extending, implementing, permitting) clauses of a class decl.
///
/// # Arguments
///   - ctx: the compiler context;
///   - tree: the class declaration.
///
/// # Returns
///   - the clause trees.
///
/// # Panics
///   - the tree is not a class declaration.
fn class_decl_clauses(
    ctx: &CompilerContext,
    tree: TreeId,
) -> (Option<TreeId>, List<TreeId>, List<TreeId>) {
    match &ctx.tree(tree).kind {
        TreeKind::ClassDecl {
            extending,
            implementing,
            permitting,
            ..
        } => (*extending, implementing.clone(), permitting.clone()),
        _ => (None, List::nil(), List::nil()),
    }
}

/// Return the flags of a class declaration's modifiers.
///
/// # Arguments
///   - ctx: the compiler context;
///   - tree: the class declaration.
///
/// # Returns
///   - the modifier flags (0 when absent).
///
/// # Panics
///   - none.
fn class_tree_mods_flags(ctx: &CompilerContext, tree: TreeId) -> i64 {
    let mods = match &ctx.tree(tree).kind {
        TreeKind::ClassDecl { mods, .. } => *mods,
        _ => None,
    };
    match mods.map(|mods| &ctx.tree(mods).kind) {
        Some(TreeKind::Modifiers { flags, .. }) => *flags,
        _ => 0,
    }
}

/// Return the full name of a class symbol.
///
/// # Arguments
///   - ctx: the compiler context;
///   - sym: the class symbol.
///
/// # Returns
///   - the full name.
///
/// # Panics
///   - none.
fn class_fullname(ctx: &CompilerContext, sym: SymbolId) -> Option<crate::util::name::Name> {
    match &ctx.sym(sym).kind_data {
        crate::code::symbol::SymbolKind::ClassSymbol {
            fullname: name, ..
        } => Some(name.clone()),
        _ => None,
    }
}

/// Return an interface list clone (the interface list source for the class type).
///
/// # Arguments
///   - interfaces: the built interface list.
///
/// # Returns
///   - a clone of the list.
///
/// # Panics
///   - none.
fn interface_list_source(interfaces: &List<TypeId>) -> List<TypeId> {
    interfaces.clone()
}

/// Return the annotation trees of a class declaration's modifiers.
///
/// # Arguments
///   - ctx: the compiler context;
///   - tree: the class declaration.
///
/// # Returns
///   - the annotations list.
///
/// # Panics
///   - none.
fn class_tree_annotations(ctx: &CompilerContext, tree: TreeId) -> List<TreeId> {
    let mods = match &ctx.tree(tree).kind {
        TreeKind::ClassDecl { mods, .. } => *mods,
        _ => None,
    };
    match mods.map(|mods| &ctx.tree(mods).kind) {
        Some(TreeKind::Modifiers { annotations, .. }) => annotations.clone(),
        _ => List::nil(),
    }
}

/// Return whether a class has an explicit permitted list.
///
/// # Arguments
///   - ctx: the compiler context;
///   - sym: the class symbol.
///
/// # Returns
///   - the isPermittedExplicit value.
///
/// # Panics
///   - none.
fn class_is_permitted_explicit(ctx: &CompilerContext, sym: SymbolId) -> bool {
    match &ctx.sym(sym).kind_data {
        crate::code::symbol::SymbolKind::ClassSymbol {
            is_permitted_explicit,
            ..
        } => *is_permitted_explicit,
        _ => false,
    }
}

/// Create a Name from a string (Names.fromString adapter).
///
/// # Arguments
///   - ctx: the compiler context;
///   - names: the standard names;
///   - text: the name text.
///
/// # Returns
///   - the interned name.
///
/// # Panics
///   - none.
fn make_name(
    names: &Names,
    text: &str,
) -> crate::util::name::Name {
    names.from_string(&javac_strings::JavaString::from(text))
}

/// Return the original annotations of a record component.
///
/// # Arguments
///   - ctx: the compiler context;
///   - rec: the record component symbol.
///
/// # Returns
///   - the original annotation trees.
///
/// # Panics
///   - none.
fn record_component_original_annos(
    ctx: &CompilerContext,
    rec: SymbolId,
) -> List<TreeId> {
    match &ctx.sym(rec).kind_data {
        crate::code::symbol::SymbolKind::RecordComponent {
            original_annos, ..
        } => original_annos.clone(),
        _ => List::nil(),
    }
}

/// The qualified name of a package symbol (Java: PackageSymbol.fullname).
fn package_fullname_of_type_enter(
    ctx: &CompilerContext,
    sym: SymbolId,
) -> crate::util::name::Name {
    match &ctx.sym(sym).kind_data {
        crate::code::symbol::SymbolKind::PackageSymbol { fullname, .. }
        | crate::code::symbol::SymbolKind::RootPackageSymbol { fullname, .. } => fullname.clone(),
        _ => ctx.sym(sym).name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    use javac_tools::error::Error as ToolsError;
    use javac_tools::tools::file_object::FileObject;
    use javac_tools::tools::java_file_manager::{
        FileObjectRef, JavaFileManager, JavaFileObjectRef, Location, LocationRef,
    };
    use javac_tools::tools::java_file_object::{JavaFileObject, Kind};
    use javac_tools::tools::option_checker::OptionChecker;
    use javac_tools::uri::URI;

    use crate::code::source::Source;
    use crate::jvm::target::Target;
    use crate::tree::jc_tree::{JCTree, LiteralValue};
    use crate::util::context::Context;
    use crate::util::javac_messages::JavacMessages;
    use crate::util::jc_diagnostic::Factory;

    struct TestSourceFile {
        name: javac_strings::JavaString,
    }

    impl FileObject for TestSourceFile {
        fn to_uri(&self) -> URI {
            URI::create(&javac_strings::JavaString::from(
                format!("string:///{}", self.name.to_string_lossy()).as_str(),
            ))
        }
        fn get_name(&self) -> javac_strings::JavaString {
            self.name.clone()
        }
        fn open_input_stream(&self) -> Result<Box<dyn Read>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn open_output_stream(&self) -> Result<Box<dyn std::io::Write>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn open_reader(
            &self,
            _ignore_encoding_errors: bool,
        ) -> Result<Box<dyn javac_tools::java_io::Reader>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn get_char_content(
            &self,
            _ignore_encoding_errors: bool,
        ) -> Result<Option<javac_strings::JavaString>, ToolsError> {
            Ok(Some(javac_strings::JavaString::from("")))
        }
        fn open_writer(&self) -> Result<Box<dyn javac_tools::java_io::Writer>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn get_last_modified(&self) -> i64 {
            0
        }
        fn delete(&self) -> bool {
            false
        }
    }

    impl JavaFileObject for TestSourceFile {
        fn get_kind(&self) -> Kind {
            Kind::SOURCE
        }
        fn is_name_compatible(
            &self,
            _simple_name: &javac_strings::JavaString,
            kind: Kind,
        ) -> bool {
            kind == Kind::SOURCE
        }
        fn get_nesting_kind(
            &self,
        ) -> Option<javac_tools::lang::model::element::nesting_kind::NestingKind> {
            None
        }
        fn get_access_level(
            &self,
        ) -> Option<javac_tools::lang::model::element::modifier::Modifier> {
            None
        }
    }

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

    struct Harness {
        ctx: CompilerContext,
        syms: Rc<RefCell<Symtab>>,
        type_enter: TypeEnter,
        names: Rc<Names>,
    }

    fn harness() -> Harness {
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = JavacMessages::instance(&mut context);
        let diags = Rc::new(Factory::new(messages, "compiler"));
        let options = crate::util::options::Options::instance(&mut context);
        let mut ctx = CompilerContext::new();
        let syms = Rc::new(RefCell::new(Symtab::new(
            &mut ctx,
            Rc::clone(&names),
            Target::JDK1_17,
            Source::JDK17,
        )));
        let types = Rc::new(Types::with_names(Rc::clone(&names)));
        let deferred = Rc::new(RefCell::new(DeferredLintHandler::new()));
        let lint = crate::code::lint::Lint::instance(&mut context);
        let chk = Rc::new(RefCell::new(crate::comp::check::Check::new(
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&diags),
            Source::JDK17,
            '$',
            Rc::clone(&types),
            lint,
            Rc::clone(&deferred),
            Rc::clone(&syms),
        )));
        crate::comp::check::attach_self(&chk);
        let manager: Rc<dyn JavaFileManager> = Rc::new(UnusedFileManager);
        let todo = crate::comp::todo::Todo::instance(&mut context);
        let type_envs = Rc::new(RefCell::new(TypeEnvs::new()));
        let enter = Rc::new(RefCell::new(Enter::new(
            Rc::clone(&log),
            Rc::clone(&names),
            Rc::clone(&diags),
            Rc::clone(&chk),
            Rc::clone(&types),
            None,
            manager,
            &options,
            Rc::clone(&todo),
            Rc::clone(&type_envs)
        )));
        let annotate_cell: Rc<RefCell<Option<Rc<Annotate>>>> = Rc::new(RefCell::new(None));
        let member_enter = Rc::new(RefCell::new(MemberEnter::new(
            Rc::clone(&enter),
            Rc::clone(&log),
            Rc::clone(&chk),
            Rc::clone(&types),
            Rc::clone(&deferred),
            Rc::clone(&annotate_cell)
        )));
        let dependencies: Rc<RefCell<dyn Dependencies>> =
            Rc::new(RefCell::new(crate::code::dependencies::DummyDependencies));
        let make = Rc::new(RefCell::new(crate::tree::tree_maker::TreeMaker::new(Some(
            Rc::clone(&names),
        ))));
        let type_enter = TypeEnter::new(
            Rc::clone(&names),
            enter,
            member_enter,
            Rc::clone(&log),
            Rc::clone(&chk),
            Rc::clone(&types),
            Rc::clone(&syms),
            make,
            Rc::clone(&deferred),
            None,
            type_envs,
            dependencies,
            annotate_cell,
            Rc::clone(&todo)
        );
        Harness {
            ctx,
            syms,
            type_enter,
            names,
        }
    }

    fn alloc(ctx: &mut CompilerContext, tag: Tag, kind: TreeKind) -> TreeId {
        ctx.alloc_tree(JCTree {
            pos: 0,
            ty: None,
            tag,
            kind,
        })
    }

    fn annotation_with_arg(
        harness: &mut Harness,
        annotation_type: crate::ids::TypeId,
        attribute: crate::util::name::Name,
        value: bool,
    ) -> TreeId {
        let ctx = &mut harness.ctx;
        let ident = alloc(
            ctx,
            Tag::IDENT,
            TreeKind::Ident {
                name: harness.names.empty.clone(),
                sym: None,
            },
        );
        ctx.tree_mut(ident).ty = Some(annotation_type);
        let lhs = alloc(
            ctx,
            Tag::IDENT,
            TreeKind::Ident {
                name: attribute,
                sym: None,
            },
        );
        let rhs = alloc(
            ctx,
            Tag::LITERAL,
            TreeKind::Literal {
                typetag: crate::code::type_tag::TypeTag::BOOLEAN,
                value: Some(LiteralValue::Boolean(value)),
            },
        );
        let assign = alloc(
            ctx,
            Tag::ASSIGN,
            TreeKind::Assign {
                lhs: Some(lhs),
                rhs: Some(rhs),
            },
        );
        alloc(
            ctx,
            Tag::ANNOTATION,
            TreeKind::Annotation {
                annotation_type: Some(ident),
                args: List::of(assign),
                attribute: None,
            },
        )
    }

    fn new_symbol(harness: &mut Harness) -> SymbolId {
        let ctx = &mut harness.ctx;
        let owner = harness.syms.borrow().java_base;
        ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
            0,
            harness.names.empty.clone(),
            ctx.sym(owner).ty.unwrap_or(harness.syms.borrow().err_type),
            owner,
        ))
    }

    /// Java: TypeEnter.handleDeprecatedAnnotations (the flags decision).
    #[test]
    fn handle_deprecated_annotations_sets_the_deprecated_flags() {
        let mut harness = harness();
        let deprecated_type = harness.syms.borrow().deprecated_type;
        let sym = new_symbol(&mut harness);
        let for_removal = harness.names.forRemoval.clone();
        let annotation = annotation_with_arg(
            &mut harness,
            deprecated_type,
            for_removal.clone(),
            true,
        );
        harness.type_enter.handle_deprecated_annotations(
            &mut harness.ctx,
            &harness.syms.borrow(),
            List::of(annotation),
            sym,
        );
        let flags_value = harness.ctx.sym(sym).flags_field;
        assert_ne!(flags_value & (flags::DEPRECATED | flags::DEPRECATED_ANNOTATION as i32) as i64, 0);
        assert_ne!(flags_value & flags::DEPRECATED_REMOVAL, 0);
    }

    /// Java: TypeEnter.handleDeprecatedAnnotations (PreviewFeature branch).
    #[test]
    fn handle_deprecated_annotations_sets_the_preview_flags() {
        let mut harness = harness();
        let preview_type = harness.syms.borrow().preview_feature_type;
        let sym = new_symbol(&mut harness);
        let reflective = harness.names.reflective.clone();
        let annotation = annotation_with_arg(
            &mut harness,
            preview_type,
            reflective.clone(),
            true,
        );
        harness.type_enter.handle_deprecated_annotations(
            &mut harness.ctx,
            &harness.syms.borrow(),
            List::of(annotation),
            sym,
        );
        let flags_value = harness.ctx.sym(sym).flags_field;
        assert_ne!(flags_value & flags::PREVIEW_API, 0);
        assert_ne!(flags_value & flags::PREVIEW_REFLECTIVE, 0);
    }

    /// Java: TypeEnter.handleDeprecatedAnnotations — unrelated annotations do
    /// not change the flags.
    #[test]
    fn handle_deprecated_annotations_ignores_other_annotations() {
        let mut harness = harness();
        let other_type = harness.syms.borrow().object_type;
        let sym = new_symbol(&mut harness);
        let reflective = harness.names.reflective.clone();
        let annotation = annotation_with_arg(
            &mut harness,
            other_type,
            reflective.clone(),
            true,
        );
        harness.type_enter.handle_deprecated_annotations(
            &mut harness.ctx,
            &harness.syms.borrow(),
            List::of(annotation),
            sym,
        );
        assert_eq!(harness.ctx.sym(sym).flags_field, 0);
    }

    /// Java: TypeEnter.setFlagIfAttributeTrue — the attribute value decides the
    /// flag (false does not set it).
    #[test]
    fn set_flag_if_attribute_true_requires_the_literal_true_value() {
        let mut harness = harness();
        let sym = new_symbol(&mut harness);
        let deprecated_type = harness.syms.borrow().deprecated_type;
        let for_removal = harness.names.forRemoval.clone();
        let annotation = annotation_with_arg(
            &mut harness,
            deprecated_type,
            for_removal.clone(),
            false,
        );
        harness.type_enter.set_flag_if_attribute_true(
            &mut harness.ctx,
            annotation,
            sym,
            for_removal,
            flags::DEPRECATED_REMOVAL,
        );
        assert_eq!(harness.ctx.sym(sym).flags_field, 0);
    }

    /// Java: Scope.NamedImportScope.importType/CompoundScope structure —
    /// the import structure (named imports / sub-scopes / mark) is verifiable
    /// without Attr.
    #[test]
    fn named_import_scope_structure_records_the_imported_type() {
        let mut harness = harness();
        let ctx = &mut harness.ctx;
        let owner = harness.syms.borrow().java_base;
        let named = ctx.alloc_scope(crate::code::scope::Scope::new_named_import_scope(Some(owner)));
        let origin = ctx.scope_create(Some(owner));
        let symbol = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
            0,
            harness.names.empty.clone(),
            harness.syms.borrow().err_type,
            owner,
        ));
        let imported = ctx.scope_import_type(named, origin, origin, symbol);
        assert!(ctx.scope_is_filled(named));
        assert_eq!(ctx.scope_get_mark(named), 1);
        let data_ok = match &ctx.scope(named).data {
            crate::code::scope::ScopeData::NamedImportScope {
                sub_scopes,
                name2_scopes,
                ..
            } => {
                !sub_scopes.is_empty()
                    && *sub_scopes.head() == imported
                    && name2_scopes
                        .get(&ctx.sym(symbol).name)
                        .map(|scopes| scopes == &vec![imported])
                        .unwrap_or(false)
            }
            _ => false,
        };
        assert!(data_ok);
    }

    /// Java: Scope.StarImportScope.importAll deduplication — the same
    /// origin/filter/static import is entered once.
    #[test]
    fn star_import_scope_structure_deduplicates_import_all() {
        let mut harness = harness();
        let ctx = &mut harness.ctx;
        let owner = harness.syms.borrow().java_base;
        let star = ctx.alloc_scope(crate::code::scope::Scope::new_star_import_scope(Some(owner)));
        let origin = ctx.scope_create(Some(owner));
        assert!(!ctx.scope_is_filled(star));
        ctx.scope_import_all(
            star,
            origin,
            None,
            None,
            None,
            false,
            Rc::clone(&harness.type_enter.types),
        );
        assert!(ctx.scope_is_filled(star));
        assert_eq!(ctx.scope_get_mark(star), 1);
        ctx.scope_import_all(
            star,
            origin,
            None,
            None,
            None,
            false,
            Rc::clone(&harness.type_enter.types),
        );
        assert_eq!(ctx.scope_get_mark(star), 1);
        let count = match &ctx.scope(star).data {
            crate::code::scope::ScopeData::StarImportScope { sub_scopes, .. } => {
                sub_scopes.iter().count()
            }
            _ => 0,
        };
        assert_eq!(count, 1);
    }

}
