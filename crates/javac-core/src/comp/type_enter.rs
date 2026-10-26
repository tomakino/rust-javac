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
use crate::ids::{ScopeId, SymbolId, TreeId};
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
    pub completion_enabled: bool,
    /// Java: private final ImportsPhase completeClass = new ImportsPhase()
    /// (expressed as phase state; registered adapter for Java inner classes).
    pub top_level_phase: Option<Phase>,
    /// ImportsPhase state (Java: inner class fields).
    pub imports_env: Option<Rc<Env<AttrContext>>>,
    pub static_import_filter: Option<Rc<dyn Fn(ScopeId, SymbolId) -> bool>>,
    pub type_import_filter: Option<Rc<dyn Fn(ScopeId, SymbolId) -> bool>>,
    /// Java: private final Annotate annotate = Annotate.instance(context) —
    /// explicit injection adapter (ADR 0001; Rc cycle broken by a cell, see
    /// MemberEnter.annotate).
    pub annotate: Rc<RefCell<Option<Rc<Annotate>>>>,
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
        make: Rc<RefCell<crate::tree::tree_maker::TreeMaker>>,
        deferred_lint_handler: Rc<RefCell<DeferredLintHandler>>,
        lint: Option<Rc<crate::code::lint::Lint>>,
        type_envs: Rc<RefCell<TypeEnvs>>,
        dependencies: Rc<RefCell<dyn Dependencies>>,
        annotate: Rc<RefCell<Option<Rc<Annotate>>>>,
    ) -> TypeEnter {
        TypeEnter {
            names,
            enter,
            member_enter,
            log,
            chk,
            types,
            make,
            deferred_lint_handler,
            lint,
            type_envs,
            dependencies,
            allow_type_annos: true,
            allow_deprecation_on_import: true,
            completion_enabled: true,
            top_level_phase: None,
            imports_env: None,
            static_import_filter: None,
            type_import_filter: None,
            annotate,
        }
    }

    /// Java: protected void ensureImportsChecked(List<JCCompilationUnit> trees)
    pub fn ensure_imports_checked(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
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
                self.finish_imports(ctx, syms, tree, Some(&top_env));
            }
            current = current.tail();
        }
    }

    /// Java: void finishImports(JCCompilationUnit toplevel, Runnable resolve)
    pub fn finish_imports(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        toplevel: TreeId,
        resolve: Option<&Rc<Env<AttrContext>>>,
    ) {
        let sourcefile = compilation_unit_sourcefile(ctx, toplevel);
        let prev = self.log.borrow_mut().use_source(sourcefile);
        // Java: resolve.run() (the completeClass.resolveImports closure)
        if let Some(env) = resolve {
            self.resolve_imports(ctx, syms, toplevel, env);
        }
        self.check_imports_unique(ctx, toplevel);
        self.check_imports_resolvable(ctx, toplevel);
        self.check_imported_packages_observable(ctx, toplevel);
        if let Some(scope) = compilation_unit_named_import_scope(ctx, toplevel) {
            ctx.scope_finalize(scope);
        }
        if let Some(scope) = compilation_unit_star_import_scope(ctx, toplevel) {
            ctx.scope_finalize(scope);
        }
        self.log.borrow_mut().use_source(prev);
    }

    /// Java: public void complete(Symbol sym) throws CompletionFailure
    pub fn complete(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        sym: SymbolId,
    ) {
        // Suppress some (recursive) MemberEnter invocations
        if !self.completion_enabled {
            // Re-install same completer for next time around and return.
            // Java: Assert.check((sym.flags() & Flags.COMPOUND) == 0);
            // sym.completer = this;
            // TODO(TypeEnter/Completer): the completer registration is an
            // adapter concern (the local completion is driven by the caller's
            // complete_symbol); registered.
            return;
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
            Some(env) => self.phase_imports_complete_envs(ctx, syms, vec![env]),
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
                    self.finish_imports(ctx, syms, toplevel, None);
                }
            }
        }
        annotate
            .unblock_annotations(ctx)
            .expect("unblockAnnotations flush");
    }

    // ------------------------------------------------------------------
    // Java: TypeEnter.Phase machinery (the inner Phase subclasses are the
    // phase_* methods below; registered adapter).
    // ------------------------------------------------------------------

    fn phase_imports_complete_envs(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        envs: Vec<Rc<Env<AttrContext>>>,
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
            match self.run_imports_phase(ctx, syms, &env) {
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
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
    ) -> Result<(), CompletionFailure> {
        let tree = env.encl_class.expect("class tree");
        let sym = class_tree_symbol(ctx, tree).expect("class symbol");
        let owner = ctx.sym(sym).owner.expect("class owner");
        // If sym is a toplevel-class, make sure any import clauses in its
        // source file have been seen.
        if ctx.sym(owner).kind == Kind::PCK {
            let top = self.enter.borrow().get_top_level_env(ctx, toplevel(env));
            self.resolve_imports(ctx, syms, toplevel(env), &top);
            // TODO(Todo): todo.append(env);
        }
        if ctx.sym(owner).kind == Kind::TYP {
            // Java: sym.owner.complete();
            ctx.complete_symbol(syms, owner)?;
        }
        Ok(())
    }

    /// Java: ImportsPhase.resolveImports(JCCompilationUnit tree,
    /// Env<AttrContext> env)
    fn resolve_imports(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) {
        if compilation_unit_star_import_scope(ctx, tree)
            .map(|scope| ctx.scope_is_filled(scope))
            .unwrap_or(false)
        {
            // we must have already processed this toplevel
            return;
        }
        let prev_static = self.static_import_filter.clone();
        let prev_type = self.type_import_filter.clone();
        let prev_lint_pos = self
            .deferred_lint_handler
            .borrow_mut()
            .immediate();
        // TODO(Lint): Lint prevLint = chk.setLint(lint);
        let prev_env = self.imports_env.clone();
        self.imports_env = Some(Rc::clone(env));
        let packge = compilation_unit_package(ctx, tree);
        let names = Rc::clone(&self.names);
        let chk = Rc::clone(&self.chk);
        let types = Rc::clone(&self.types);
        let chk_for_static = Rc::clone(&chk);
        let types_for_static = Rc::clone(&types);
        self.static_import_filter = Some(Rc::new(move |origin, sym| {
            let mut ctx = CompilerContext::new();
            let _ = (&mut ctx, origin, sym, &chk_for_static, &types_for_static);
            unimplemented!("TODO(Attr/Check batch): ImportsPhase.importAccessible/static filter")
        }));
        let chk_for_type = Rc::clone(&chk);
        self.type_import_filter = Some(Rc::new(move |_origin, sym| {
            let _ = (sym, &chk_for_type);
            unimplemented!("TODO(Check batch): ImportsPhase.importAccessible/type filter")
        }));
        // Import-on-demand java.lang.
        let java_lang = syms.enter_package(ctx, syms.java_base, self.names.java_lang.clone());
        let java_lang_members_empty = ctx
            .scope_get_symbols(
                class_members_scope(ctx, java_lang).expect("java.lang members"),
                LookupKind::NON_RECURSIVE,
                &|_| true,
            )
            .is_empty();
        if java_lang_members_empty && !symbol_exists(ctx, java_lang) {
            // Java: throw new FatalError(diags.fragment(Fragments.FatalErrNoJavaLang));
            unimplemented!("TODO(File/Query batch): FatalError(Fragments.FatalErrNoJavaLang)")
        }
        // Java: importAll(make.at(tree.pos()).Import(make.QualIdent(javaLang), false), javaLang, env);
        self.import_all_java_lang(ctx, syms, tree, java_lang, env);
        let has_module_decl = compilation_unit_module_decl(ctx, tree).is_some();
        // Process the package def and all import clauses.
        if compilation_unit_package_decl(ctx, tree).is_some() && !has_module_decl {
            self.check_class_package_clash(ctx, syms, env);
        }
        let mut imports = compilation_unit_imports(ctx, tree);
        while imports.non_empty() {
            let imp = *imports.head();
            self.do_import(ctx, syms, imp);
            imports = imports.tail();
        }
        if let Some(decl) = compilation_unit_module_decl(ctx, tree) {
            // check @Deprecated:
            let annotations = module_decl_annotations(ctx, decl);
            let sym = module_decl_symbol(ctx, decl);
            if let Some(sym) = sym {
                self.mark_deprecated(ctx, syms, sym, annotations);
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
        self.imports_env = prev_env;
        // TODO(Lint): chk.setLint(prevLint);
        self.deferred_lint_handler.borrow_mut().set_pos_state(prev_lint_pos);
        self.static_import_filter = prev_static;
        self.type_import_filter = prev_type;
        let _ = (packge, names, chk, types);
    }

    /// Java: ImportsPhase.checkClassPackageClash(JCPackageDecl tree)
    fn check_class_package_clash(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        env: &Rc<Env<AttrContext>>,
    ) {
        // check that no class exists with same fully qualified name as toplevel
        // package
        if let Some(package_decl) = compilation_unit_package_decl(ctx, toplevel(env))
            && package_decl_is_non_null(ctx, package_decl) {
                let mut p = compilation_unit_package(ctx, toplevel(env)).expect("packge");
                let root = ctx.sym(syms.root_package).owner;
                loop {
                    let owner = ctx.sym(p).owner;
                    if owner == root || owner.is_none() {
                        break;
                    }
                    if let Some(owner) = owner {
                        // Java: p.owner.complete(); enter all class members of p
                        let _ = ctx.complete_symbol(syms, owner);
                        // TODO(Modules): syms.lookupPackage(toplevel(&env).modle,
                        // p.owner.getQualifiedName()) + syms.getClass(...) +
                        // Errors.PkgClashesWithClassOfSameName(p);
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
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) {
        let qualid = import_qualid(ctx, tree).expect("import qualid");
        let field_access = match &ctx.tree(qualid).kind {
            TreeKind::FieldAccess { selected, name, .. } => (*selected, name.clone()),
            _ => panic!("ClassCastException: JCFieldAccess expected"),
        };
        let (selected, name) = field_access;
        let env = self.imports_env.clone().expect("imports env");
        // Create a local environment pointing to this tree to disable effects of
        // other imports in Resolve.findGlobalType
        let local_env = env.dup_shared(Some(tree));
        // Java: TypeSymbol p = attr.attribImportQualifier(tree, localEnv).tsym;
        let p = self.attrib_import_qualifier(ctx, syms, tree, &local_env);
        if name.to_java_string() == self.names.asterisk.to_java_string() {
            // Import on demand.
            // TODO(Check): chk.checkCanonical(imp.selected);
            if import_static_import(ctx, tree) {
                self.import_static_all(ctx, tree, p, &env);
            } else {
                self.import_all(ctx, tree, p, &env);
            }
        } else if import_static_import(ctx, tree) {
            // Named static import.
            self.import_named_static(ctx, syms, tree, p, name, &local_env);
            // TODO(Check): chk.checkCanonical(imp.selected);
        } else {
            // Named type import.
            let imported_type = self.attrib_import_type(ctx, syms, qualid, &local_env);
            let original_type = original_type(ctx, imported_type);
            let c = if ctx.typ(original_type).has_tag(TypeTag::CLASS) {
                ctx.typ(original_type).tsym
            } else {
                ctx.typ(imported_type).tsym
            };
            // TODO(Check): chk.checkCanonical(imp);
            if let Some(c) = c {
                self.import_named(ctx, syms, ctx.tree(tree).pos, c, &env, tree);
            }
        }
        let _ = (selected, syms);
    }

    /// Java: ImportsPhase.attribImportType(JCTree tree, Env<AttrContext> env)
    fn attrib_import_type(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _tree: TreeId,
        _env: &Rc<Env<AttrContext>>,
    ) -> crate::ids::TypeId {
        // Java: Assert.check(completionEnabled);
        // Lint prevLint = chk.setLint(...); completionEnabled = false;
        // return attr.attribType(tree, env);
        unimplemented!("TODO(Attr batch): ImportsPhase.attribImportType attr.attribType")
    }

    fn attrib_import_qualifier(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _tree: TreeId,
        _local_env: &Rc<Env<AttrContext>>,
    ) -> SymbolId {
        unimplemented!("TODO(Attr batch): ImportsPhase.doImport attr.attribImportQualifier")
    }

    /// Java: ImportsPhase.importAll(JCImport imp, TypeSymbol tsym,
    /// Env<AttrContext> env)
    fn import_all(
        &mut self,
        ctx: &mut CompilerContext,
        _tree: TreeId,
        tsym: SymbolId,
        env: &Rc<Env<AttrContext>>,
    ) {
        let star_scope = compilation_unit_star_import_scope(ctx, toplevel(env)).expect("star scope");
        let origin = class_members_scope(ctx, tsym).expect("type members");
        let filter = self.static_import_filter.clone();
        let _ = filter;
        // Java: toplevel(&env).starImportScope.importAll(types, tsym.members(),
        // typeImportFilter, imp, cfHandler);
        let type_filter = self.type_import_filter.clone();
        ctx.scope_import_all(
            star_scope,
            origin,
            type_filter,
            Some(_tree),
            None,
            false,
        );
    }

    /// Java: ImportsPhase.importStaticAll(JCImport imp, TypeSymbol tsym,
    /// Env<AttrContext> env)
    fn import_static_all(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        tsym: SymbolId,
        env: &Rc<Env<AttrContext>>,
    ) {
        let star_scope = compilation_unit_star_import_scope(ctx, toplevel(env)).expect("star scope");
        let origin = class_members_scope(ctx, tsym).expect("type members");
        let filter = self.static_import_filter.clone();
        ctx.scope_import_all(
            star_scope,
            origin,
            filter,
            Some(tree),
            None,
            true,
        );
    }

    /// Java: ImportsPhase.importNamedStatic(JCImport imp, TypeSymbol tsym,
    /// Name name, Env<AttrContext> env)
    fn import_named_static(
        &mut self,
        ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        tree: TreeId,
        tsym: SymbolId,
        name: crate::util::name::Name,
        env: &Rc<Env<AttrContext>>,
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
        let filter = self.static_import_filter.clone();
        let new_scope = ctx.scope_import_by_name(
            named_scope,
            origin,
            name,
            filter,
            Some(tree),
            None,
            true,
        );
        set_import_scope(ctx, tree, new_scope);
    }

    /// Java: ImportsPhase.importNamed(DiagnosticPosition pos, Symbol tsym,
    /// Env<AttrContext> env, JCImport imp)
    fn import_named(
        &mut self,
        ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _pos: i32,
        tsym: SymbolId,
        env: &Rc<Env<AttrContext>>,
        tree: TreeId,
    ) {
        if ctx.sym(tsym).kind == Kind::TYP {
            let named_scope =
                compilation_unit_named_import_scope(ctx, toplevel(env)).expect("named scope");
            let owner_members = ctx
                .sym(tsym)
                .owner
                .and_then(|owner| class_members_scope(ctx, owner))
                .expect("owner members");
            let new_scope = ctx.scope_import_type(named_scope, owner_members, owner_members, tsym);
            set_import_scope(ctx, tree, new_scope);
        }
    }

    /// Java: importAll(make.at(tree.pos()).Import(make.QualIdent(javaLang), false),
    /// javaLang, env) — the java.lang import-on-demand.
    fn import_all_java_lang(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        java_lang: SymbolId,
        env: &Rc<Env<AttrContext>>,
    ) {
        let pos = ctx.tree(tree).pos;
        let imp = {
            let mut make = self.make.borrow_mut();
            make.at(pos);
            let qual = make.qual_ident(ctx, syms, java_lang);
            make.import(ctx, qual, false)
        };
        self.import_all(ctx, imp, java_lang, env);
    }
    /// Java: Check.checkImportsUnique(toplevel) — TODO(Check).
    fn check_imports_unique(&mut self, _ctx: &mut CompilerContext, _toplevel: TreeId) {
        unimplemented!("TODO(Check batch): Check.checkImportsUnique")
    }

    /// Java: Check.checkImportsResolvable(toplevel) — TODO(Check).
    fn check_imports_resolvable(&mut self, _ctx: &mut CompilerContext, _toplevel: TreeId) {
        unimplemented!("TODO(Check batch): Check.checkImportsResolvable")
    }

    /// Java: Check.checkImportedPackagesObservable(toplevel) — TODO(Check).
    fn check_imported_packages_observable(&mut self, _ctx: &mut CompilerContext, _toplevel: TreeId) {
        unimplemented!("TODO(Check batch): Check.checkImportedPackagesObservable")
    }

    /// Java: public void markDeprecated(Symbol sym, List<JCAnnotation>
    /// annotations, Env<AttrContext> env)
    pub fn mark_deprecated(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        sym: SymbolId,
        annotations: List<TreeId>,
    ) {
        // In general, we cannot fully process annotations yet, but we can
        // attribute the annotation types and then check to see if the
        // @Deprecated annotation is present.
        self.attrib_annotation_types(ctx, syms, annotations.clone());
        self.handle_deprecated_annotations(ctx, syms, annotations, sym);
    }

    /// Java: Attr.attribAnnotationTypes(annotations, env) — TODO(Attr).
    fn attrib_annotation_types(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _annotations: List<TreeId>,
    ) {
        unimplemented!("TODO(Attr batch): TypeEnter.markDeprecated Attr.attribAnnotationTypes")
    }

    /// Java: private void handleDeprecatedAnnotations(List<JCAnnotation>
    /// annotations, Symbol sym)
    fn handle_deprecated_annotations(
        &mut self,
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
        &mut self,
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
        &mut self,
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
        &mut self,
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
        &mut self,
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
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _env: &Rc<Env<AttrContext>>,
    ) {
        // TODO(Attr): supertypes processing (attribSuperTypes /
        // chk.checkNonCyclic / markDeprecated / chk.checkNonCyclicDecl);
        // registered.
        unimplemented!("TODO(Attr/Check batch): TypeEnter.hierarchyPhase.runPhase")
    }

    /// Java: HierarchyPhase.attribSuperTypes(Env<AttrContext> env,
    /// Env<AttrContext> baseEnv) (TypeEnter.java)
    fn attrib_super_types(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _env: &Rc<Env<AttrContext>>,
        _base_env: &Rc<Env<AttrContext>>,
    ) {
        unimplemented!("TODO(Attr batch): TypeEnter.attribSuperTypes (attr.attribBase/modelMissingTypes)")
    }

    /// Java: HeaderPhase.clearTypeParams (HierarchyPhase override).
    fn clear_type_params(&mut self, _ctx: &mut CompilerContext, super_type: TreeId) -> TreeId {
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
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _env: &Rc<Env<AttrContext>>,
    ) {
        unimplemented!("TODO(Types/Symbol batch): TypeEnter.permitsPhase.runPhase (directSupertypes/isSealed/getEnv)")
    }

    // ------------------------------------------------------------------
    // Java: HeaderPhase (TypeEnter.java:880-935)
    // ------------------------------------------------------------------

    /// Java: HeaderPhase.runPhase(Env<AttrContext> env)
    fn header_phase_run(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _env: &Rc<Env<AttrContext>>,
    ) {
        // TODO(Attr/Check/Annotate): annotate.queueScanTreeAndTypeAnnotate/
        // annotate.flush/attribSuperTypes/chk.checkNotRepeated/annotateLater/
        // attr.attribTypeVariables/syms.packageExists/Errors.ClashWithPkgOfSameName/
        // JavaFileObject.isNameCompatible/AUXILIARY; registered.
        unimplemented!("TODO(Attr/Check/Annotate batch): TypeEnter.headerPhase.runPhase")
    }

    // ------------------------------------------------------------------
    // Java: AbstractMembersPhase/RecordPhase/MembersPhase
    // (TypeEnter.java:937-1090)
    // ------------------------------------------------------------------

    /// Java: AbstractMembersPhase.enterThisAndSuper(ClassSymbol sym,
    /// Env<AttrContext> env)
    fn enter_this_and_super(
        &mut self,
        ctx: &mut CompilerContext,
        sym: SymbolId,
        env: &Rc<Env<AttrContext>>,
    ) {
        // enter symbols for 'this' into current scope.
        let ty = ctx.sym(sym).ty.expect("class type");
        let this_sym = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
            (flags::FINAL | flags::HASINIT) as i64,
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
                        (flags::FINAL | flags::HASINIT) as i64,
                        self.names._super.clone(),
                        supertype,
                        sym,
                    ));
                    set_var_symbol_pos(ctx, super_sym, 0);
                    ctx.scope_enter(scope, super_sym);
                }
        }
    }

    /// Java: RecordPhase.runPhase(Env<AttrContext> env)
    fn record_phase_run(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _env: &Rc<Env<AttrContext>>,
    ) {
        // TODO(Record): TreeInfo.recordFields/findRecordComponentToRemove/
        // TreeCopier/createRecordComponent/memberEnter; registered.
        unimplemented!("TODO(Record batch): TypeEnter.recordPhase.runPhase")
    }

    /// Java: MembersPhase.runPhase(Env<AttrContext> env)
    fn members_phase_run(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _env: &Rc<Env<AttrContext>>,
    ) {
        // TODO(Attr/Check/Annotate): checkDefaultConstructor/
        // defaultConstructor/enterThisAndSuper/checkNonCyclic/finishClass/
        // typeAnnotations.organizeTypeAnnotationsSignatures; registered.
        unimplemented!("TODO(Attr/Check batch): TypeEnter.membersPhase.runPhase")
    }

    // ------------------------------------------------------------------
    // Java: finishClass / addAccessor / addEnumMembers /
    // getCanonicalConstructorDecl / addRecordMembersIfNeeded / lookupMethod
    // (TypeEnter.java:1090-1180)
    // ------------------------------------------------------------------

    /// Java: void finishClass(JCClassDecl tree, JCTree defaultConstructor,
    /// Env<AttrContext> env)
    fn finish_class(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _tree: TreeId,
        _default_constructor: Option<TreeId>,
        _env: &Rc<Env<AttrContext>>,
    ) {
        // TODO(Enum/Record/Annotate): addEnumMembers/addRecordMembersIfNeeded/
        // setAnnotationTypeMetadata; memberEnter.memberEnter(defs) is ported.
        // The defs member enter part is:
        // memberEnter.memberEnter(defsToEnter, env);
        unimplemented!("TODO(Enum/Record/Annotate batch): TypeEnter.finishClass")
    }

    /// Java: private void addAccessor(JCVariableDecl tree, Env<AttrContext> env)
    fn add_accessor(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _tree: TreeId,
        _env: &Rc<Env<AttrContext>>,
    ) {
        unimplemented!("TODO(Record batch): TypeEnter.addAccessor (TreeMaker/TreeCopier/RecordComponent)")
    }

    /// Java: private void addEnumMembers(JCClassDecl tree, Env<AttrContext> env)
    fn add_enum_members(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _tree: TreeId,
        _env: &Rc<Env<AttrContext>>,
    ) {
        unimplemented!("TODO(Enum batch): TypeEnter.addEnumMembers (make.VarDef/Assign/flags)")
    }

    /// Java: private void addRecordMembersIfNeeded(JCClassDecl tree,
    /// Env<AttrContext> env)
    fn add_record_members_if_needed(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        _tree: TreeId,
        _env: &Rc<Env<AttrContext>>,
    ) {
        unimplemented!("TODO(Record batch): TypeEnter.addRecordMembersIfNeeded")
    }

    /// Java: private JCMethodDecl getCanonicalConstructorDecl(JCClassDecl tree)
    fn get_canonical_constructor_decl(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
    ) -> Option<TreeId> {
        let symbol = class_tree_symbol(ctx, tree).expect("class sym");
        // Java: sym.isRecord() && tree.defs.stream().filter(t ->
        // TreeInfo.isConstructor(t) && t.sym.params.length ==
        // TreeInfo.recordFields(tree).length()).findFirst()
        if !symbol_is_record(ctx, symbol) {
            return None;
        }
        let defs = class_tree_defs(ctx, tree);
        let mut current = defs;
        while current.non_empty() {
            let def = *current.head();
            if tree_info::is_constructor(ctx, def) {
                // TODO(Record): compare the parameter count with the
                // record components (TreeInfo.recordFields).
                return Some(def);
            }
            current = current.tail();
        }
        None
    }

    /// Java: private MethodSymbol lookupMethod(TypeSymbol c, Name name,
    /// List<Type> argtypes)
    fn lookup_method(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        c: SymbolId,
        name: crate::util::name::Name,
        _argtypes: List<crate::ids::TypeId>,
    ) -> Option<SymbolId> {
        let scope = class_members_scope(ctx, c)?;
        let symbols = ctx.scope_get_symbols_by_name(scope, &name, LookupKind::NON_RECURSIVE, &|_| true);
        for sym in symbols {
            // Java: if (types.hasSameArgs(m.type, new MethodType(argtypes,
            // syms.voidType, List.nil(), syms.methodClass), true)) return m;
            if let Some(ty) = ctx.sym(sym).ty
                && ctx.typ(ty).has_tag(TypeTag::METHOD) {
                    let same = self.types.has_same_args(ctx, syms, ty, ty);
                    if same {
                        // TODO(Types): compare with the synthetic
                        // MethodType(argtypes, voidType, nil, methodClass);
                        // the full hasSameArgs comparison is registered.
                        return Some(sym);
                    }
                }
        }
        None
    }

    /// Java: JCTree defaultConstructor(TreeMaker make,
    /// DefaultConstructorHelper helper)
    fn default_constructor(
        &mut self,
        _ctx: &mut CompilerContext,
        _helper: &dyn DefaultConstructorHelper,
    ) -> TreeId {
        unimplemented!("TODO(TreeMaker/Attr batch): TypeEnter.defaultConstructor")
    }

    /// Java: DefaultConstructorHelper getDefaultConstructorHelper(
    /// Env<AttrContext> env)
    fn get_default_constructor_helper(
        &mut self,
        _ctx: &mut CompilerContext,
        _env: &Rc<Env<AttrContext>>,
    ) -> Option<Box<dyn DefaultConstructorHelper>> {
        // TODO(Record/TreeMaker): BasicConstructorHelper/
        // AnonClassConstructorHelper/RecordConstructorHelper need TreeMaker and
        // RecordComponent; registered.
        unimplemented!("TODO(Record/TreeMaker batch): TypeEnter.getDefaultConstructorHelper")
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
        SymbolKind::ClassSymbol { members_field, .. } => *members_field,
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

/// Java: TypeEnter.DefaultConstructorHelper (TypeEnter.java:1180-1190)
///
/// TODO(TreeMaker/Attr): the helper method bodies need TreeMaker/Attr/
/// Types.memberType; the trait and the concrete helper types are defined here so
/// that the mapping is explicit (registered; the implementations follow in the
/// Attr/TreeMaker batches).
pub trait DefaultConstructorHelper {
    /// Java: Type constructorType()
    fn constructor_type(&mut self) -> Option<crate::ids::TypeId>;
    /// Java: MethodSymbol constructorSymbol()
    fn constructor_symbol(&mut self) -> Option<SymbolId>;
    /// Java: Type enclosingType()
    fn enclosing_type(&mut self) -> Option<crate::ids::TypeId>;
    /// Java: TypeSymbol owner()
    fn owner(&self) -> SymbolId;
    /// Java: List<Name> superArgs()
    fn super_args(&mut self) -> List<crate::util::name::Name>;
    /// Java: default JCMethodDecl finalAdjustment(JCMethodDecl md) { return md; }
    fn final_adjustment(&mut self, method: TreeId) -> TreeId {
        method
    }
}

/// Java: class BasicConstructorHelper implements DefaultConstructorHelper
pub struct BasicConstructorHelper {
    owner: SymbolId,
    constructor_type: Option<crate::ids::TypeId>,
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
    fn constructor_type(&mut self) -> Option<crate::ids::TypeId> {
        unimplemented!("TODO(Types batch): BasicConstructorHelper.constructorType")
    }
    fn constructor_symbol(&mut self) -> Option<SymbolId> {
        unimplemented!("TODO(Types batch): BasicConstructorHelper.constructorSymbol")
    }
    fn enclosing_type(&mut self) -> Option<crate::ids::TypeId> {
        unimplemented!("TODO(Types batch): BasicConstructorHelper.enclosingType")
    }
    fn owner(&self) -> SymbolId {
        self.owner
    }
    fn super_args(&mut self) -> List<crate::util::name::Name> {
        unimplemented!("TODO(Types batch): BasicConstructorHelper.superArgs")
    }
}

/// Java: class AnonClassConstructorHelper extends BasicConstructorHelper
pub struct AnonClassConstructorHelper {
    base: BasicConstructorHelper,
    constr: SymbolId,
    encl: Option<crate::ids::TypeId>,
    based: bool,
}

impl AnonClassConstructorHelper {
    /// Java: AnonClassConstructorHelper(TypeSymbol owner, MethodSymbol constr,
    /// JCExpression encl)
    pub fn new(
        owner: SymbolId,
        constr: SymbolId,
        encl: Option<crate::ids::TypeId>,
    ) -> AnonClassConstructorHelper {
        AnonClassConstructorHelper {
            base: BasicConstructorHelper::new(owner),
            constr,
            encl,
            based: false,
        }
    }
}

impl DefaultConstructorHelper for AnonClassConstructorHelper {
    fn constructor_type(&mut self) -> Option<crate::ids::TypeId> {
        unimplemented!("TODO(Types batch): AnonClassConstructorHelper.constructorType")
    }
    fn constructor_symbol(&mut self) -> Option<SymbolId> {
        unimplemented!("TODO(Types batch): AnonClassConstructorHelper.constructorSymbol")
    }
    fn enclosing_type(&mut self) -> Option<crate::ids::TypeId> {
        unimplemented!("TODO(Types batch): AnonClassConstructorHelper.enclosingType")
    }
    fn owner(&self) -> SymbolId {
        self.base.owner
    }
    fn super_args(&mut self) -> List<crate::util::name::Name> {
        unimplemented!("TODO(Types batch): AnonClassConstructorHelper.superArgs")
    }
    fn final_adjustment(&mut self, method: TreeId) -> TreeId {
        method
    }
}

/// Java: class RecordConstructorHelper extends BasicConstructorHelper
pub struct RecordConstructorHelper {
    base: BasicConstructorHelper,
    record_field_decls: List<TreeId>,
}

impl RecordConstructorHelper {
    /// Java: RecordConstructorHelper(ClassSymbol owner, List<JCVariableDecl>
    /// recordFieldDecls)
    pub fn new(owner: SymbolId, record_field_decls: List<TreeId>) -> RecordConstructorHelper {
        RecordConstructorHelper {
            base: BasicConstructorHelper::new(owner),
            record_field_decls,
        }
    }
}

impl DefaultConstructorHelper for RecordConstructorHelper {
    fn constructor_type(&mut self) -> Option<crate::ids::TypeId> {
        unimplemented!("TODO(Record batch): RecordConstructorHelper.constructorType")
    }
    fn constructor_symbol(&mut self) -> Option<SymbolId> {
        unimplemented!("TODO(Record batch): RecordConstructorHelper.constructorSymbol")
    }
    fn enclosing_type(&mut self) -> Option<crate::ids::TypeId> {
        unimplemented!("TODO(Record batch): RecordConstructorHelper.enclosingType")
    }
    fn owner(&self) -> SymbolId {
        self.base.owner
    }
    fn super_args(&mut self) -> List<crate::util::name::Name> {
        unimplemented!("TODO(Record batch): RecordConstructorHelper.superArgs")
    }
    fn final_adjustment(&mut self, _method: TreeId) -> TreeId {
        unimplemented!("TODO(Record batch): RecordConstructorHelper.finalAdjustment")
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
        let manager: Rc<dyn JavaFileManager> = Rc::new(UnusedFileManager);
        let enter = Rc::new(RefCell::new(Enter::new(
            Rc::clone(&log),
            Rc::clone(&names),
            Rc::clone(&diags),
            Rc::clone(&chk),
            Rc::clone(&types),
            None,
            manager,
            &options,
        )));
        let annotate_cell: Rc<RefCell<Option<Rc<Annotate>>>> = Rc::new(RefCell::new(None));
        let member_enter = Rc::new(RefCell::new(MemberEnter::new(
            Rc::clone(&enter),
            Rc::clone(&log),
            Rc::clone(&chk),
            Rc::clone(&types),
            Rc::clone(&deferred),
            Rc::clone(&annotate_cell),
        )));
        let type_envs = Rc::new(RefCell::new(TypeEnvs::new()));
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
            make,
            Rc::clone(&deferred),
            None,
            type_envs,
            dependencies,
            annotate_cell,
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
        ctx.scope_import_all(star, origin, None, None, None, false);
        assert!(ctx.scope_is_filled(star));
        assert_eq!(ctx.scope_get_mark(star), 1);
        ctx.scope_import_all(star, origin, None, None, None, false);
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
