// Origin: com/sun/tools/javac/comp/Enter.java @ 982d53c (jdk-17.0.19-ga)
//
// PORTING_RULES §19 registration:
// Ported in this file: the Enter structure, classEnv/topLevelEnv/getTopLevelEnv/
// enterScope/moduleEnv, classEnter, visitTopLevel/visitClassDef/
// visitTypeParameter/visitModuleDef/visitTree, classNameMatchesFileName/
// duplicateClass, main/complete/newRound (see the batch report).
//
// NOT PORTED IN THIS STEP (registered; no substitute):
// - Annotate instance: annotate.blockAnnotations/unblockAnnotations(%NoFlush)
//   calls are TODO in the corresponding methods (Annotate batch);
// - TypeEnter instance: enterType/completionEnabled/uncompleted processing and
//   ensureImportsChecked are TODO (TypeEnter/MemberEnter batch); visitClassDef
//   therefore does not install the TypeEnter completer;
// - Todo instance: todo.append of package/module envs is TODO (Todo batch);
// - Modules instance: module/package visibility checks, module-info handling,
//   usesProvidesCompleter and isInModuleGraph are TODO (Modules batch);
// - JCTree.Visitor dispatch is expressed as a TreeKind match (registered
//   adapter: the local tree arena has no accept()/Visitor);
// - predefClassDef construction needs TreeMaker.ClassDef/Modifiers factories,
//   which are not ported yet: the field stays None (registered);
// - Scope.NamedImportScope/StarImportScope are not ported (Scope batch): the
//   compilation unit import scopes stay None (registered);
// - UnenterScanner needs the JCTree.Visitor callback scanner (tree scanner
//   batch): unenter() is unimplemented below.

use std::cell::RefCell;
use std::rc::Rc;

use javac_tools::tools::java_file_manager::JavaFileManager;
use javac_tools::tools::java_file_object::{JavaFileObject, Kind};

use crate::code::flags;
use crate::code::kinds::KindName;
use crate::code::symbol::{CompletionFailure, SymbolKind};
use crate::code::symtab::Symtab;
use crate::comp::attr_context::AttrContext;
use crate::comp::check::Check;
use crate::comp::env::Env;
use crate::comp::type_envs::TypeEnvs;
use crate::compiler_context::CompilerContext;
use crate::ids::{ScopeId, SymbolId, TreeId, TypeId};
use crate::resources::compiler_properties::{errors, warnings};
use crate::resources::diagnostic_contract::{KindNameArg, SymbolArg};
use crate::tree::jc_tree::{Tag, TreeKind};
use crate::tree::tree_info;
use crate::util::abstract_log::AbstractLog;
use crate::util::jc_diagnostic::{Error as JCDiagnosticError, Factory, Warning as JCDiagnosticWarning};
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::name::Name;
use crate::util::names::Names;

/// Java: public class Enter (see the file header for the ported subset).
pub struct Enter {
    pub log: Rc<RefCell<Log>>,
    pub names: Rc<Names>,
    pub diags: Rc<Factory>,
    pub chk: Rc<RefCell<Check>>,
    pub types: Rc<crate::code::types::Types>,
    pub lint: Option<Rc<crate::code::lint::Lint>>,
    pub file_manager: Rc<dyn JavaFileManager>,
    pub pkginfo_opt: crate::main::option::PkgInfo,
    pub type_envs: Rc<RefCell<TypeEnvs>>,
    /// Java: private JCClassDecl predefClassDef;
    /// TODO(TreeMaker batch): TreeMaker.ClassDef/Modifiers factories are not
    /// ported; the field stays None (registered).
    pub predef_class_def: Option<TreeId>,
    /// Java: ListBuffer<ClassSymbol> uncompleted;
    pub uncompleted: Option<Vec<SymbolId>>,
    /// Java: ListBuffer<JCCompilationUnit> unfinishedModules
    pub unfinished_modules: Vec<TreeId>,
    /// Java: protected Env<AttrContext> env;
    pub env: Option<Rc<Env<AttrContext>>>,
    /// Java: Type result;
    pub result: Option<TypeId>,
}

impl Enter {
    /// Java: protected Enter(Context context) — injected dependencies; Annotate/
    /// TypeEnter/Modules/Todo are not ported (TODO in the methods, registered).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        log: Rc<RefCell<Log>>,
        names: Rc<Names>,
        diags: Rc<Factory>,
        chk: Rc<RefCell<Check>>,
        types: Rc<crate::code::types::Types>,
        lint: Option<Rc<crate::code::lint::Lint>>,
        file_manager: Rc<dyn JavaFileManager>,
        options: &crate::util::options::Options,
    ) -> Enter {
        Enter {
            log,
            names,
            diags,
            chk,
            types,
            lint,
            file_manager,
            pkginfo_opt: crate::main::option::PkgInfo::get(options),
            type_envs: Rc::new(RefCell::new(TypeEnvs::new())),
            predef_class_def: None,
            uncompleted: None,
            unfinished_modules: Vec::new(),
            env: None,
            result: None,
        }
    }

    /// Java: public Env<AttrContext> getEnv(TypeSymbol sym)
    pub fn get_env(&self, sym: SymbolId) -> Option<Rc<Env<AttrContext>>> {
        self.type_envs.borrow().get(sym)
    }

    /// Java: public Iterable<Env<AttrContext>> getEnvs()
    pub fn get_envs(&self) -> Vec<Rc<Env<AttrContext>>> {
        self.type_envs.borrow().values()
    }

    /// Java: public Env<AttrContext> getClassEnv(TypeSymbol sym)
    pub fn get_class_env(&self, sym: SymbolId) -> Option<Rc<Env<AttrContext>>> {
        let _local_env = self.get_env(sym)?;
        // TODO(Lint batch): lint.augment(sym) is not ported (Lint::augment).
        unimplemented!("TODO(Lint batch): Enter.getClassEnv lintEnv.augment")
    }

    /// Java: public Env<AttrContext> classEnv(JCClassDecl tree,
    /// Env<AttrContext> env)
    pub fn class_env(
        &self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) -> Rc<Env<AttrContext>> {
        let class = class_tree_symbol(ctx, tree).expect("class symbol");
        let scope = ctx.scope_create(Some(class));
        let mut info = env.info.borrow().dup(Some(scope));
        info.is_self_call = false;
        info.lint = None;
        info.is_anonymous_diamond = env
            .tree
            .map(|env_tree| tree_info::is_diamond(ctx, env_tree))
            .unwrap_or(false);
        Rc::new(Env {
            next: None,
            outer: Some(Rc::clone(env)),
            tree: Some(tree),
            toplevel: env.toplevel,
            encl_class: Some(tree),
            encl_method: env.encl_method,
            info: Rc::new(RefCell::new(info)),
            base_clause: env.base_clause,
        })
    }

    /// Java: Env<AttrContext> topLevelEnv(JCCompilationUnit tree)
    fn top_level_env(&self, ctx: &mut CompilerContext, tree: TreeId) -> Rc<Env<AttrContext>> {
        let packge = compilation_unit_package(ctx, tree).expect("tree.packge");
        let toplevel_scope = ctx.scope_create(Some(packge));
        // Java: tree.namedImportScope = new NamedImportScope(tree.packge);
        //       tree.starImportScope = new StarImportScope(tree.packge);
        // TODO(Scope batch): NamedImportScope/StarImportScope are not ported;
        // the fields stay None (registered).
        set_compilation_unit_scopes(ctx, tree, toplevel_scope);
        let mut info = AttrContext::new();
        info.scope = Some(toplevel_scope);
        info.lint = self.lint.clone();
        Rc::new(Env {
            next: None,
            outer: None,
            tree: Some(tree),
            toplevel: Some(tree),
            encl_class: self.predef_class_def,
            encl_method: None,
            info: Rc::new(RefCell::new(info)),
            base_clause: false,
        })
    }

    /// Java: public Env<AttrContext> getTopLevelEnv(JCCompilationUnit tree)
    pub fn get_top_level_env(&self, ctx: &mut CompilerContext, tree: TreeId) -> Rc<Env<AttrContext>> {
        let toplevel_scope = compilation_unit_toplevel_scope(ctx, tree).expect("tree.toplevelScope");
        let mut info = AttrContext::new();
        info.scope = Some(toplevel_scope);
        info.lint = self.lint.clone();
        Rc::new(Env {
            next: None,
            outer: None,
            tree: Some(tree),
            toplevel: Some(tree),
            encl_class: self.predef_class_def,
            encl_method: None,
            info: Rc::new(RefCell::new(info)),
            base_clause: false,
        })
    }

    /// Java: WriteableScope enterScope(Env<AttrContext> env)
    pub(crate) fn enter_scope(&self, ctx: &mut CompilerContext, env: &Rc<Env<AttrContext>>) -> ScopeId {
        let tree = env.tree.expect("env.tree");
        if ctx.tree(tree).has_tag(Tag::CLASSDEF) {
            let class = class_tree_symbol(ctx, tree).expect("class symbol");
            class_members_scope(ctx, class).expect("class members_field")
        } else {
            env.info.borrow().scope.expect("env.info.scope")
        }
    }

    /// Java: public Env<AttrContext> moduleEnv(JCModuleDecl tree,
    /// Env<AttrContext> env)
    pub fn module_env(
        &self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        env: &Rc<Env<AttrContext>>,
    ) -> Rc<Env<AttrContext>> {
        let module = module_tree_symbol(ctx, tree).expect("module symbol");
        let scope = ctx.scope_create(Some(module));
        let mut info = env.info.borrow().dup(Some(scope));
        info.is_self_call = false;
        info.lint = None;
        Rc::new(Env {
            next: None,
            outer: Some(Rc::clone(env)),
            tree: Some(tree),
            toplevel: env.toplevel,
            encl_class: self.predef_class_def,
            encl_method: None,
            info: Rc::new(RefCell::new(info)),
            base_clause: env.base_clause,
        })
    }

    /// Java: Type classEnter(JCTree tree, Env<AttrContext> env)
    pub(crate) fn class_enter(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
        env: Option<Rc<Env<AttrContext>>>,
    ) -> Option<TypeId> {
        let prev_env = self.env.clone();
        // TODO(Annotate batch): annotate.blockAnnotations();
        self.env = env;
        let previous_result = self.result;
        self.result = None;
        let visit_result = self.visit(ctx, syms, tree);
        let result = match visit_result {
            Ok(()) => self.result,
            Err(failure) => {
                // Java: return chk.completionError(tree.pos(), ex);
                Some(self.chk.borrow().completion_error(
                    ctx,
                    ctx.tree(tree).pos,
                    &failure,
                    syms,
                ))
            }
        };
        // TODO(Annotate batch): annotate.unblockAnnotations();
        self.env = prev_env;
        self.result = previous_result;
        result
    }

    /// Java: <T extends JCTree> List<Type> classEnter(List<T> trees,
    /// Env<AttrContext> env)
    pub(crate) fn class_enter_list(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        trees: List<TreeId>,
        env: Option<Rc<Env<AttrContext>>>,
    ) -> List<TypeId> {
        let mut ts = List::nil();
        let mut list = trees;
        while list.non_empty() {
            let tree = *list.head();
            if let Some(ty) = self.class_enter(ctx, syms, tree, env.clone()) {
                ts = ts.append(ty);
            }
            list = list.tail();
        }
        ts
    }

    fn visit(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<(), CompletionFailure> {
        match ctx.tree(tree).tag {
            Tag::TOPLEVEL => self.visit_top_level(ctx, syms, tree),
            Tag::CLASSDEF => self.visit_class_def(ctx, syms, tree),
            Tag::TYPEPARAMETER => self.visit_type_parameter(ctx, syms, tree),
            Tag::MODULEDEF => self.visit_module_def(ctx, syms, tree),
            _ => self.visit_tree(ctx),
        }
    }

    /// Java: public void visitTopLevel(JCCompilationUnit tree)
    fn visit_top_level(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<(), CompletionFailure> {
        let sourcefile = compilation_unit_sourcefile(ctx, tree)
            .expect("CompilationUnit.sourcefile (upstream non-null)");
        let prev_source = self.log.borrow_mut().use_source(Some(Rc::clone(&sourcefile)));
        let mut add_env = false;
        let is_pkg_info = sourcefile
            .is_name_compatible(&javac_strings::JavaString::from("package-info"), Kind::SOURCE);
        let module = compilation_unit_module(ctx, tree).expect("tree.modle");
        if tree_info::is_module_info(ctx, tree) {
            let pd = compilation_unit_package_decl(ctx, tree);
            if let Some(pd) = pd {
                self.log.borrow_mut().error_at(
                    ctx.tree(pd).pos,
                    JCDiagnosticError::from(errors::no_pkg_in_module_info_java()),
                );
            }
            set_compilation_unit_package(ctx, tree, syms.root_package);
            let top_env = self.top_level_env(ctx, tree);
            self.class_enter_list(ctx, syms, compilation_unit_defs(ctx, tree), Some(top_env));
            // TODO(Modules batch): tree.modle.usesProvidesCompleter =
            //     modules.getUsesProvidesCompleter();
        } else {
            let pd = compilation_unit_package_decl(ctx, tree);
            if let Some(pd) = pd {
                let pid = package_decl_pid(ctx, pd).expect("PackageDecl.pid");
                let package_name = tree_info::full_name(ctx, pid).expect("package name");
                let packge = syms.enter_package(ctx, module, package_name);
                set_compilation_unit_package(ctx, tree, packge);
                set_package_decl_package(ctx, pd, packge);
                self.set_package_symbols(ctx, pd, packge);
                let annotations = package_decl_annotations(ctx, pd);
                if !annotations.is_empty()
                    || self.pkginfo_opt == crate::main::option::PkgInfo::ALWAYS
                    || compilation_unit_doc_comments(ctx, tree)
                {
                    if is_pkg_info {
                        add_env = true;
                    } else if !annotations.is_empty() {
                        let first = *annotations.head();
                        self.log.borrow_mut().error_at(
                            ctx.tree(first).pos,
                            JCDiagnosticError::from(
                                errors::pkg_annotations_sb_in_package_info_java(),
                            ),
                        );
                    }
                }
            } else {
                let unnamed = module_unnamed_package(ctx, module).expect("unnamed package");
                set_compilation_unit_package(ctx, tree, unnamed);
            }
            // TODO(Modules batch): dependencyWithPackage / PackageInOtherModule
            // check (syms.listPackageModules and module.visiblePackages).
            let packge = compilation_unit_package(ctx, tree).expect("tree.packge");
            // Java: tree.packge.complete(); // Find all classes in package.
            ctx.complete_symbol(syms, packge)?;
            let top_env = self.top_level_env(ctx, tree);
            let package_env = if is_pkg_info {
                // Java: topEnv.dup(pd)
                Some(top_env.dup_shared(pd))
            } else {
                None
            };
            if is_pkg_info {
                let package_env = package_env.clone().expect("package env");
                let existing = self.type_envs.borrow().get(packge);
                if let Some(env0) = existing {
                    let tree0 = env0.toplevel.expect("toplevel");
                    let file0 = compilation_unit_sourcefile(ctx, tree0).expect("sourcefile");
                    if !self.file_manager.is_same_file(
                        &(Rc::clone(&sourcefile) as javac_tools::tools::java_file_manager::FileObjectRef),
                        &(Rc::clone(&file0) as javac_tools::tools::java_file_manager::FileObjectRef),
                    ) {
                        let pd = compilation_unit_package_decl(ctx, tree);
                        let pos = pd
                            .and_then(|pd| package_decl_pid(ctx, pd))
                            .map(|pid| ctx.tree(pid).pos)
                            .unwrap_or(ctx.tree(tree).pos);
                        let warn = JCDiagnosticWarning::from(warnings::pkg_info_already_seen(
                            SymbolArg::placeholder(ctx.sym(packge).name.to_java_string().to_string_lossy()),
                        ));
                        self.log.borrow_mut().warning_at(pos, warn);
                    }
                }
                self.type_envs.borrow_mut().put(packge, package_env);
                // Java: for (Symbol q = tree.packge; q != null && q.kind == PCK;
                //             q = q.owner) q.flags_field |= EXISTS;
                let mut q = Some(packge);
                while let Some(current) = q {
                    if ctx.sym(current).kind != crate::code::kinds::Kind::PCK {
                        break;
                    }
                    ctx.sym_mut(current).flags_field |= flags::EXISTS as i64;
                    q = ctx.sym(current).owner;
                }
                let name = self.names.package_info.clone();
                let c = syms.enter_class(ctx, module, name.clone(), Some(packge));
                let flatname = self.names.from_string(&javac_strings::JavaString::from(
                    format!("{}.{}", package_fullname(ctx, packge), name.to_java_string()).as_str(),
                ));
                if let SymbolKind::ClassSymbol { flatname: slot, .. } = &mut ctx.sym_mut(c).kind_data
                {
                    *slot = flatname;
                }
                if let SymbolKind::ClassSymbol { sourcefile: slot, .. } =
                    &mut ctx.sym_mut(c).kind_data
                {
                    *slot = Some(Rc::clone(&sourcefile));
                }
                ctx.sym_mut(c).completer = None;
                let scope = ctx.scope_create(Some(c));
                if let SymbolKind::ClassSymbol { members_field, .. } = &mut ctx.sym_mut(c).kind_data {
                    *members_field = Some(scope);
                }
                if let SymbolKind::PackageSymbol { package_info, .. } =
                    &mut ctx.sym_mut(packge).kind_data
                {
                    *package_info = Some(c);
                }
                if let SymbolKind::PackageSymbol { sourcefile: slot, .. } =
                    &mut ctx.sym_mut(packge).kind_data
                {
                    *slot = Some(Rc::clone(&sourcefile));
                }
            }
            self.class_enter_list(ctx, syms, compilation_unit_defs(ctx, tree), Some(top_env.clone()));
            if add_env {
                // TODO(Todo batch): todo.append(packageEnv);
            }
        }
        self.log.borrow_mut().use_source(prev_source);
        self.result = None;
        Ok(())
    }

    /// Java: private final TreeScanner setPackageSymbols (visitPackageDef/
    /// visitSelect/visitIdent); expressed as a recursion (registered adapter).
    fn set_package_symbols(&self, ctx: &mut CompilerContext, pd: TreeId, packge: SymbolId) {
        if let Some(pid) = package_decl_pid(ctx, pd) {
            self.set_package_symbols_for(ctx, pid, packge);
        }
    }

    fn set_package_symbols_for(&self, ctx: &mut CompilerContext, tree: TreeId, current: SymbolId) {
        let selected = match &ctx.tree(tree).kind {
            TreeKind::Ident { .. } => {
                set_tree_symbol_and_type(ctx, tree, current);
                return;
            }
            TreeKind::FieldAccess { selected, .. } => *selected,
            _ => return,
        };
        set_tree_symbol_and_type(ctx, tree, current);
        let owner = ctx.sym(current).owner.expect("package owner");
        if let Some(selected) = selected {
            self.set_package_symbols_for(ctx, selected, owner);
        }
    }

    /// Java: public void visitClassDef(JCClassDecl tree)
    fn visit_class_def(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<(), CompletionFailure> {
        let env = self.env.clone().expect("env");
        let scope = self.enter_scope(ctx, &env);
        let owner = ctx.scope(scope).owner.expect("scope owner");
        let tree_name = class_tree_name(ctx, tree);
        let tree_mods = class_tree_mods(ctx, tree);
        let mut tree_flags = tree_mods
            .map(|mods| modifiers_flags(ctx, mods))
            .unwrap_or(0);
        
        let class: SymbolId = if ctx.sym(owner).kind == crate::code::kinds::Kind::PCK {
            // We are seeing a toplevel class.
            let packge = owner;
            let mut q = Some(packge);
            while let Some(current) = q {
                if ctx.sym(current).kind != crate::code::kinds::Kind::PCK {
                    break;
                }
                ctx.sym_mut(current).flags_field |= flags::EXISTS as i64;
                q = ctx.sym(current).owner;
            }
            let module = env.toplevel.and_then(|top| compilation_unit_module(ctx, top));
            let module = module.expect("toplevel module");
            let c = syms.enter_class(ctx, module, tree_name.clone(), Some(packge));
            // Java: packge.members().enterIfAbsent(c);
            if let Some(members) = class_members_scope(ctx, packge) {
                ctx.scope_enter_if_absent(members, c);
            }
            if (tree_flags & flags::PUBLIC as i64) != 0 && !self.class_name_matches_file_name(ctx, c, &env)
            {
                let top_element = if (tree_flags & flags::ENUM as i64) != 0 {
                    KindName::ENUM
                } else if (tree_flags & flags::INTERFACE as i64) != 0 {
                    KindName::INTERFACE
                } else {
                    KindName::CLASS
                };
                self.log.borrow_mut().error_at(
                    ctx.tree(tree).pos,
                    JCDiagnosticError::from(errors::class_public_should_be_in_file(
                        KindNameArg::from(top_element.name()),
                        tree_name.clone(),
                    )),
                );
            }
            c
        } else if !tree_name.is_empty()
            && !self
                .chk
                .borrow()
                .check_unique_class_name(ctx, ctx.tree(tree).pos, &tree_name, scope)
        {
            self.result = None;
            return Ok(());
        } else if ctx.sym(owner).kind == crate::code::kinds::Kind::TYP {
            // We are seeing a member class.
            let module = env.toplevel.and_then(|top| compilation_unit_module(ctx, top));
            let module = module.expect("toplevel module");
            let c = syms.enter_class(ctx, module, tree_name.clone(), Some(owner));
            if ctx.sym(c).owner != Some(owner) {
                if ctx.sym(c).name.to_java_string() != tree_name.to_java_string() {
                    self.log.borrow_mut().error_at(
                        ctx.tree(tree).pos,
                        JCDiagnosticError::from(errors::same_binary_name(
                            ctx.sym(c).name.clone(),
                            tree_name.clone(),
                        )),
                    );
                    let err_type = self.types.create_error_type(
                        ctx,
                        tree_name.clone(),
                        owner,
                        None,
                    );
                    let tsym = ctx.typ(err_type).tsym.expect("error type symbol");
                    set_class_tree_symbol(ctx, tree, tsym);
                    self.result = Some(err_type);
                    return Ok(());
                }
                // anonymous class loaded from a classfile may be recreated from
                // source: fix the owner.
                let c_owner = ctx.sym(c).owner.expect("class owner");
                if let Some(cowner_members) = class_members_scope(ctx, c_owner) {
                    ctx.scope_remove(cowner_members, c);
                }
                ctx.sym_mut(c).owner = Some(owner);
            }
            if (ctx.sym(owner).flags_field & flags::INTERFACE as i64) != 0 {
                tree_flags |= (flags::PUBLIC | flags::STATIC) as i64;
                set_modifiers_flags(ctx, tree_mods, tree_flags);
            }
            c
        } else {
            // We are seeing a local class.
            let c = syms.define_class(ctx, tree_name.clone(), Some(owner));
            let flatname = self.chk.borrow_mut().local_class_name(ctx, c);
            if let SymbolKind::ClassSymbol { flatname: slot, .. } = &mut ctx.sym_mut(c).kind_data {
                *slot = flatname;
            }
            if !ctx.sym(c).name.is_empty() {
                self.chk.borrow().check_transparent_class(ctx, ctx.tree(tree).pos, c, scope);
            }
            c
        };
        set_class_tree_symbol(ctx, tree, class);
        // Enter class into 'compiled' table and enclosing scope.
        if self.chk.borrow().get_compiled(ctx, class).is_some() {
            self.duplicate_class(ctx, tree, class);
            let err_type = self.types.create_error_type(
                ctx,
                class_tree_name(ctx, tree),
                owner,
                None,
            );
            let tsym = ctx.typ(err_type).tsym.expect("error type symbol");
            set_class_tree_symbol(ctx, tree, tsym);
            self.result = Some(err_type);
            return Ok(());
        }
        self.chk.borrow_mut().put_compiled(ctx, class);
        ctx.scope_enter(scope, class);
        // Set up an environment for class block.
        let local_env = self.class_env(ctx, tree, &env);
        self.type_envs.borrow_mut().put(class, Rc::clone(&local_env));
        // Fill out class fields.
        ctx.sym_mut(class).completer = None;
        let checked_flags = self.chk.borrow().check_flags(
            ctx,
            ctx.tree(tree).pos,
            tree_flags,
            class,
            tree,
        );
        ctx.sym_mut(class).flags_field = checked_flags;
        let class_sourcefile =
            compilation_unit_sourcefile(ctx, env.toplevel.expect("toplevel"));
        if let SymbolKind::ClassSymbol { sourcefile: slot, .. } =
            &mut ctx.sym_mut(class).kind_data
        {
            *slot = class_sourcefile;
        }
        let members_scope = ctx.scope_create(Some(class));
        if let SymbolKind::ClassSymbol { members_field, .. } = &mut ctx.sym_mut(class).kind_data {
            *members_field = Some(members_scope);
        }
        // TODO(Annotate batch): c.clearAnnotationMetadata();
        let class_ty = ctx.sym(class).ty.expect("class type");
        if ctx.sym(owner).kind != crate::code::kinds::Kind::PCK
            && (ctx.sym(class).flags_field & flags::STATIC as i64) == 0
        {
            // local or inner class: set the enclosing instance class.
            let mut owner1 = Some(owner);
            while let Some(current) = owner1 {
                let current_kind = ctx.sym(current).kind;
                let is_val_mth = matches!(
                    current_kind,
                    crate::code::kinds::Kind::VAR | crate::code::kinds::Kind::MTH
                );
                if !is_val_mth || (ctx.sym(current).flags_field & flags::STATIC as i64) != 0 {
                    break;
                }
                owner1 = ctx.sym(current).owner;
            }
            if let Some(owner1) = owner1
                && ctx.sym(owner1).kind == crate::code::kinds::Kind::TYP {
                    let owner_type = ctx.sym(owner1).ty.expect("owner class type");
                    ctx.typ_mut(class_ty).set_enclosing_type(Some(owner_type));
                }
        }
        // Enter type parameters.
        let typarams = self.class_enter_list(
            ctx,
            syms,
            class_tree_typarams(ctx, tree),
            Some(Rc::clone(&local_env)),
        );
        if let crate::code::type_::TypeKindData::ClassType { typarams_field, allparams_field, .. } =
            &mut ctx.typ_mut(class_ty).kind
        {
            *typarams_field = Some(typarams.clone());
            *allparams_field = None;
        }
        // TODO(TypeEnter/MemberEnter batch): c.completer = typeEnter;
        // The two conditions are kept nested exactly like the upstream
        // (if (!c.isDirectlyOrIndirectlyLocal() && uncompleted != null) ...);
        // Rust let-chains would require a newer edition (registered).
        #[allow(clippy::collapsible_if)]
        if !crate::code::symbol::symbol_is_directly_or_indirectly_local(ctx, class) {
            if let Some(uncompleted) = self.uncompleted.as_mut() {
                uncompleted.push(class);
            }
        }
        // Recursively enter all member classes.
        self.class_enter_list(ctx, syms, class_tree_defs(ctx, tree), Some(local_env));
        self.result = ctx.sym(class).ty;
        Ok(())
    }

    /// Java: private static boolean classNameMatchesFileName(ClassSymbol c,
    /// Env<AttrContext> env)
    fn class_name_matches_file_name(
        &self,
        ctx: &mut CompilerContext,
        c: SymbolId,
        env: &Rc<Env<AttrContext>>,
    ) -> bool {
        let toplevel = env.toplevel.expect("toplevel");
        let sourcefile = compilation_unit_sourcefile(ctx, toplevel).expect("sourcefile");
        sourcefile.is_name_compatible(&ctx.sym(c).name.to_java_string(), Kind::SOURCE)
    }

    /// Java: protected void duplicateClass(DiagnosticPosition pos, ClassSymbol c)
    fn duplicate_class(&self, ctx: &mut CompilerContext, tree: TreeId, c: SymbolId) {
        let _ = tree;
        self.log.borrow_mut().error_at(
            ctx.tree(tree).pos,
            JCDiagnosticError::from(errors::duplicate_class(class_fullname(ctx, c))),
        );
    }

    /// Java: public void visitTypeParameter(JCTypeParameter tree)
    fn visit_type_parameter(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<(), CompletionFailure> {
        let env = self.env.clone().expect("env");
        let scope = env.info.borrow().scope.expect("env.info.scope");
        let name = type_parameter_name(ctx, tree);
        let existing_type = type_parameter_type(ctx, tree);
        let tvar = match existing_type {
            Some(existing) => existing,
            None => {
                let owner = ctx.scope(scope).owner.expect("scope owner");
                let bot = syms.bot_type;
                ctx.new_type_var_with_owner(&self.names, name.clone(), owner, bot)
            }
        };
        set_type_parameter_type(ctx, tree, tvar);
        let tsym = ctx.typ(tvar).tsym.expect("type var symbol");
        let unique = self.chk.borrow().check_unique(ctx, ctx.tree(tree).pos, tsym, scope);
        if unique {
            ctx.scope_enter(scope, tsym);
        }
        self.result = Some(tvar);
        Ok(())
    }

    /// Java: public void visitModuleDef(JCModuleDecl tree)
    fn visit_module_def(
        &mut self,
        ctx: &mut CompilerContext,
        _syms: &mut Symtab,
        tree: TreeId,
    ) -> Result<(), CompletionFailure> {
        let env = self.env.clone().expect("env");
        let module_env = self.module_env(ctx, tree, &env);
        let module = module_tree_symbol(ctx, tree).expect("tree.sym");
        self.type_envs.borrow_mut().put(module, module_env);
        // TODO(Modules batch): if (modules.isInModuleGraph(tree.sym))
        //     todo.append(moduleEnv);
        Ok(())
    }

    /// Java: public void visitTree(JCTree tree) { result = null; }
    fn visit_tree(&mut self, _ctx: &mut CompilerContext) -> Result<(), CompletionFailure> {
        self.result = None;
        Ok(())
    }

    /// Java: public void main(List<JCCompilationUnit> trees)
    pub fn main(&mut self, ctx: &mut CompilerContext, syms: &mut Symtab, trees: List<TreeId>) {
        self.complete(ctx, syms, trees, None);
    }

    /// Java: public void complete(List<JCCompilationUnit> trees, ClassSymbol c)
    pub fn complete(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        trees: List<TreeId>,
        c: Option<SymbolId>,
    ) {
        // TODO(Annotate batch): annotate.blockAnnotations();
        let prev_uncompleted = self.uncompleted.take();
        // TODO(TypeEnter batch): if (typeEnter.completionEnabled)
        //     uncompleted = new ListBuffer<>();
        let result = self.class_enter_list(ctx, syms, trees, None);
        let _ = result;
        // TODO(TypeEnter batch): complete the uncompleted classes in memberEnter
        // and run ensureImportsChecked; not ported yet (registered).
        let _ = c;
        self.uncompleted = prev_uncompleted;
        // TODO(Annotate batch): annotate.unblockAnnotations();
    }

    /// Java: public void newRound() { typeEnvs.clear(); }
    pub fn new_round(&mut self) {
        self.type_envs.borrow_mut().clear();
    }

    /// Java: public void unenter(JCCompilationUnit topLevel, JCTree tree)
    pub fn unenter(&mut self, _ctx: &mut CompilerContext, _top_level: TreeId, _tree: TreeId) {
        // TODO(Enter batch): UnenterScanner needs the JCTree.Visitor callback
        // scanner (tree scanner batch); registered.
        unimplemented!("TODO(Enter batch): Enter.unenter (UnenterScanner)");
    }
}

// ---------------------------------------------------------------------------
// Local tree-field accessors (registered adapter: JCTree fields are arena
// entries; the upstream code mutates the tree object directly).
// ---------------------------------------------------------------------------

fn compilation_unit_sourcefile(
    ctx: &CompilerContext,
    tree: TreeId,
) -> Option<Rc<dyn JavaFileObject>> {
    match &ctx.tree(tree).kind {
        TreeKind::CompilationUnit { sourcefile, .. } => sourcefile.clone(),
        _ => None,
    }
}

fn compilation_unit_defs(ctx: &CompilerContext, tree: TreeId) -> List<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::CompilationUnit { defs, .. } => defs.clone(),
        _ => List::nil(),
    }
}

fn compilation_unit_module(ctx: &CompilerContext, tree: TreeId) -> Option<SymbolId> {
    match &ctx.tree(tree).kind {
        TreeKind::CompilationUnit { modle, .. } => *modle,
        _ => None,
    }
}

fn compilation_unit_package(ctx: &CompilerContext, tree: TreeId) -> Option<SymbolId> {
    match &ctx.tree(tree).kind {
        TreeKind::CompilationUnit { packge, .. } => *packge,
        _ => None,
    }
}

fn set_compilation_unit_package(ctx: &mut CompilerContext, tree: TreeId, packge: SymbolId) {
    if let TreeKind::CompilationUnit { packge: slot, .. } = &mut ctx.tree_mut(tree).kind {
        *slot = Some(packge);
    }
}

fn compilation_unit_toplevel_scope(ctx: &CompilerContext, tree: TreeId) -> Option<ScopeId> {
    match &ctx.tree(tree).kind {
        TreeKind::CompilationUnit { toplevel_scope, .. } => *toplevel_scope,
        _ => None,
    }
}

fn set_compilation_unit_scopes(ctx: &mut CompilerContext, tree: TreeId, toplevel_scope: ScopeId) {
    if let TreeKind::CompilationUnit {
        toplevel_scope: slot,
        named_import_scope,
        star_import_scope,
        ..
    } = &mut ctx.tree_mut(tree).kind
    {
        *slot = Some(toplevel_scope);
        // TODO(Scope batch): NamedImportScope/StarImportScope (registered).
        *named_import_scope = None;
        *star_import_scope = None;
    }
}

/// Java: JCCompilationUnit.getPackage() — the local tree keeps the package
/// declaration in defs (upstream has the dedicated pid field; registered
/// adapter: scan defs for the PACKAGEDEF entry).
fn compilation_unit_package_decl(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    let mut defs = compilation_unit_defs(ctx, tree);
    while defs.non_empty() {
        let def = *defs.head();
        if ctx.tree(def).has_tag(Tag::PACKAGEDEF) {
            return Some(def);
        }
        defs = defs.tail();
    }
    None
}

fn compilation_unit_doc_comments(ctx: &CompilerContext, tree: TreeId) -> bool {
    match &ctx.tree(tree).kind {
        TreeKind::CompilationUnit { doc_comments, .. } => doc_comments.is_some(),
        _ => false,
    }
}

fn package_decl_pid(ctx: &CompilerContext, pd: TreeId) -> Option<TreeId> {
    match &ctx.tree(pd).kind {
        TreeKind::PackageDecl { pid, .. } => *pid,
        _ => None,
    }
}

fn package_decl_annotations(ctx: &CompilerContext, pd: TreeId) -> List<TreeId> {
    match &ctx.tree(pd).kind {
        TreeKind::PackageDecl { annotations, .. } => annotations.clone(),
        _ => List::nil(),
    }
}

fn set_package_decl_package(ctx: &mut CompilerContext, pd: TreeId, packge: SymbolId) {
    if let TreeKind::PackageDecl { packge: slot, .. } = &mut ctx.tree_mut(pd).kind {
        *slot = Some(packge);
    }
}

fn package_fullname(ctx: &CompilerContext, packge: SymbolId) -> String {
    match &ctx.sym(packge).kind_data {
        SymbolKind::PackageSymbol { fullname, .. }
        | SymbolKind::RootPackageSymbol { fullname, .. } => {
            fullname.to_java_string().to_string()
        }
        _ => ctx.sym(packge).name.to_java_string().to_string(),
    }
}

fn class_tree_symbol(ctx: &CompilerContext, tree: TreeId) -> Option<SymbolId> {
    match &ctx.tree(tree).kind {
        TreeKind::ClassDecl { sym, .. } => *sym,
        _ => None,
    }
}

fn class_tree_name(ctx: &CompilerContext, tree: TreeId) -> Name {
    match &ctx.tree(tree).kind {
        TreeKind::ClassDecl { name, .. } => name.clone(),
        _ => panic!("ClassDecl expected"),
    }
}

fn class_tree_mods(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::ClassDecl { mods, .. } => *mods,
        _ => None,
    }
}

fn class_tree_typarams(ctx: &CompilerContext, tree: TreeId) -> List<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::ClassDecl { typarams, .. } => typarams.clone(),
        _ => List::nil(),
    }
}

fn class_tree_defs(ctx: &CompilerContext, tree: TreeId) -> List<TreeId> {
    match &ctx.tree(tree).kind {
        TreeKind::ClassDecl { defs, .. } => defs.clone(),
        _ => List::nil(),
    }
}

fn set_class_tree_symbol(ctx: &mut CompilerContext, tree: TreeId, symbol: SymbolId) {
    if let TreeKind::ClassDecl { sym, .. } = &mut ctx.tree_mut(tree).kind {
        *sym = Some(symbol);
    }
}

fn class_fullname(ctx: &CompilerContext, class: SymbolId) -> Name {
    match &ctx.sym(class).kind_data {
        SymbolKind::ClassSymbol { fullname, .. } => fullname.clone(),
        _ => ctx.sym(class).name.clone(),
    }
}

fn class_members_scope(ctx: &CompilerContext, symbol: SymbolId) -> Option<ScopeId> {
    match &ctx.sym(symbol).kind_data {
        SymbolKind::ClassSymbol { members_field, .. }
        | SymbolKind::PackageSymbol { members_field, .. }
        | SymbolKind::RootPackageSymbol { members_field, .. } => *members_field,
        _ => None,
    }
}

fn module_tree_symbol(ctx: &CompilerContext, tree: TreeId) -> Option<SymbolId> {
    match &ctx.tree(tree).kind {
        TreeKind::ModuleDecl { sym, .. } => *sym,
        _ => None,
    }
}

fn module_unnamed_package(ctx: &CompilerContext, module: SymbolId) -> Option<SymbolId> {
    match &ctx.sym(module).kind_data {
        SymbolKind::ModuleSymbol { unnamed_package, .. } => *unnamed_package,
        _ => None,
    }
}

fn type_parameter_name(ctx: &CompilerContext, tree: TreeId) -> Name {
    match &ctx.tree(tree).kind {
        TreeKind::TypeParameter { name, .. } => name.clone(),
        _ => panic!("TypeParameter expected"),
    }
}

fn type_parameter_type(ctx: &CompilerContext, tree: TreeId) -> Option<TypeId> {
    // JCTree.type is the shared tree-type field (Java: JCTree.type).
    ctx.tree(tree).ty
}

fn set_type_parameter_type(ctx: &mut CompilerContext, tree: TreeId, ty: TypeId) {
    ctx.tree_mut(tree).ty = Some(ty);
}

fn modifiers_flags(ctx: &CompilerContext, mods: TreeId) -> i64 {
    match &ctx.tree(mods).kind {
        TreeKind::Modifiers { flags, .. } => *flags,
        _ => 0,
    }
}

fn set_modifiers_flags(ctx: &mut CompilerContext, mods: Option<TreeId>, new_flags: i64) {
    if let Some(mods) = mods
        && let TreeKind::Modifiers { flags, .. } = &mut ctx.tree_mut(mods).kind {
            *flags = new_flags;
        }
}

fn set_tree_symbol_and_type(ctx: &mut CompilerContext, tree: TreeId, symbol: SymbolId) {
    let ty = ctx.sym(symbol).ty;
    match &mut ctx.tree_mut(tree).kind {
        TreeKind::Ident { sym, .. } => {
            *sym = Some(symbol);
        }
        TreeKind::FieldAccess { sym, .. } => {
            *sym = Some(symbol);
        }
        _ => {}
    }
    ctx.tree_mut(tree).ty = ty;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    use javac_tools::error::Error as ToolsError;
    use javac_tools::tools::file_object::FileObject;
    use javac_tools::tools::java_file_manager::{
        FileObjectRef, JavaFileObjectRef, Location, LocationRef,
    };
    use javac_tools::tools::java_file_object::{JavaFileObject, Kind};
    use javac_tools::tools::option_checker::OptionChecker;
    use javac_tools::uri::URI;

    use crate::code::source::Source;
    use crate::jvm::target::Target;
    use crate::ids::SymbolId;
    use crate::parser::parser_factory::ParserFactory;
    use crate::parser::parser::Parser;
    use crate::parser::scanner_factory::ScannerFactory;
    use crate::util::context::Context;
    use crate::util::javac_messages::JavacMessages;
    use crate::util::jc_diagnostic::Factory;

    /// The development-time ENTER oracle (tools/gen-enter-oracle).
    const ENTER_ORACLE: &str =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/enter-oracle.txt"));

    /// The same sources as tools/gen-enter-oracle/GenEnterOracle.java.
    fn sample_source(name: &str) -> &'static str {
        match name {
            "Sample.java" => {
                "package p;\nclass A {\n    class B {\n        class C {}\n    }\n    interface I {\n        class Nested {}\n    }\n    int f;\n    void m() {}\n}\ninterface Top {}\n"
            }
            "Unnamed.java" => "class U {\n    enum E { X, Y }\n}\n",
            "PublicMismatch.java" => "public class Foo {\n}\n",
            "Duplicate.java" => "package d;\nclass D {}\nclass D {}\n",
            other => panic!("unknown sample {other}"),
        }
    }

    /// A source-file JavaFileObject carrying only the name (Enter does not read
    /// the content; the parser input comes from the string).
    struct TestSourceFile {
        name: javac_strings::JavaString,
        base: String,
        content: String,
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
            Ok(Some(javac_strings::JavaString::from(self.content.as_str())))
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
            simple_name: &javac_strings::JavaString,
            kind: Kind,
        ) -> bool {
            kind == Kind::SOURCE && simple_name.to_string_lossy() == self.base
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

    /// A file manager that is never used by the Enter tests (no package-info /
    /// no class file lookups); every method is explicitly unimplemented.
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
        unit: TreeId,
        log: Rc<RefCell<Log>>,
        names: Rc<Names>,
        options: Rc<crate::util::options::Options>,
        diags: Rc<Factory>,
    }

    fn harness(sample: &str) -> Harness {
        let source = sample_source(sample);
        let base = sample.trim_end_matches(".java").to_string();
        let sourcefile = Rc::new(TestSourceFile {
            name: javac_strings::JavaString::from(sample),
            base,
            content: source.to_string(),
        });
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = JavacMessages::instance(&mut context);
        let diags = Rc::new(Factory::new(messages, "compiler"));
        let options = crate::util::options::Options::instance(&mut context);
        let scanner_factory =
            ScannerFactory::new(&mut context, Rc::clone(&names), Source::DEFAULT);
        let factory = ParserFactory::new(
            &mut context,
            Rc::clone(&names),
            Source::DEFAULT,
            scanner_factory,
            None,
        );
        // Java: the compiler sets the log source before parsing (Log.useSource).
        log.borrow_mut()
            .use_source(Some(sourcefile as Rc<dyn JavaFileObject>));
        let mut ctx = CompilerContext::new();
        let mut parser = factory.new_parser(
            &mut ctx,
            &javac_strings::JavaString::from(source),
            false,
            true,
            false,
        );
        let unit = parser.parse_compilation_unit(&mut ctx);
        Harness {
            ctx,
            unit,
            log,
            names,
            options,
            diags,
        }
    }

    fn run_enter(harness: &mut Harness, sample: &str) {
        let base = sample.trim_end_matches(".java").to_string();
        let sourcefile = Rc::new(TestSourceFile {
            name: javac_strings::JavaString::from(sample),
            base,
            content: sample_source(sample).to_string(),
        });
        let mut ctx = std::mem::take(&mut harness.ctx);
        let mut syms = Symtab::new(
            &mut ctx,
            Rc::clone(&harness.names),
            Target::JDK1_17,
            Source::JDK17,
        );
        let java_base = syms.java_base;
        if let TreeKind::CompilationUnit { modle, sourcefile: slot, .. } =
            &mut ctx.tree_mut(harness.unit).kind
        {
            *modle = Some(java_base);
            *slot = Some(sourcefile as Rc<dyn JavaFileObject>);
        }
        let types = Rc::new(crate::code::types::Types::new());
        let chk = Rc::new(RefCell::new(Check::new(
            Rc::clone(&harness.names),
            Rc::clone(&harness.log),
            Rc::clone(&harness.diags),
            Source::JDK17,
            '$',
            Rc::clone(&types),
        )));
        let manager: Rc<dyn JavaFileManager> = Rc::new(UnusedFileManager);
        let mut enter = Enter::new(
            Rc::clone(&harness.log),
            Rc::clone(&harness.names),
            Rc::clone(&harness.diags),
            chk,
            types,
            None,
            manager,
            &harness.options,
        );
        enter.main(&mut ctx, &mut syms, List::of(harness.unit));
        harness.ctx = ctx;
    }

    /// Java: javax.lang.model.element.Modifier name for the flags (sorted).
    fn modifier_names(flags_value: i64) -> String {
        let mut names: Vec<&str> = Vec::new();
        let pairs: [(i64, &str); 14] = [
            (crate::code::flags::PUBLIC as i64, "public"),
            (crate::code::flags::PROTECTED as i64, "protected"),
            (crate::code::flags::PRIVATE as i64, "private"),
            (crate::code::flags::ABSTRACT as i64, "abstract"),
            (crate::code::flags::STATIC as i64, "static"),
            (crate::code::flags::FINAL as i64, "final"),
            (crate::code::flags::TRANSIENT as i64, "transient"),
            (crate::code::flags::VOLATILE as i64, "volatile"),
            (crate::code::flags::SYNCHRONIZED as i64, "synchronized"),
            (crate::code::flags::NATIVE as i64, "native"),
            (crate::code::flags::STRICTFP as i64, "strictfp"),
            (crate::code::flags::DEFAULT, "default"),
            (crate::code::flags::SEALED, "sealed"),
            (crate::code::flags::NON_SEALED, "non-sealed"),
        ];
        for (bit, name) in pairs {
            if (flags_value & bit) != 0 {
                names.push(name);
            }
        }
        names.sort_unstable();
        names.join(",")
    }

    fn type_kind(flags_value: i64) -> &'static str {
        if (flags_value & crate::code::flags::ANNOTATION as i64) != 0 {
            "ANNOTATION_TYPE"
        } else if (flags_value & crate::code::flags::ENUM as i64) != 0 {
            "ENUM"
        } else if (flags_value & crate::code::flags::INTERFACE as i64) != 0 {
            "INTERFACE"
        } else if (flags_value & crate::code::flags::RECORD) != 0 {
            "RECORD"
        } else {
            "CLASS"
        }
    }

    fn print_type(ctx: &CompilerContext, class: SymbolId, depth: usize, out: &mut String) {
        let indent = "  ".repeat(depth);
        // Oracle comparison: the javac oracle prints TypeElement.getQualifiedName()
        // (dots), i.e. the ClassSymbol fullname (not the binary flatname).
        let (flatname, name) = match &ctx.sym(class).kind_data {
            SymbolKind::ClassSymbol { fullname, .. } => (
                fullname.to_java_string().to_string(),
                ctx.sym(class).name.to_java_string().to_string(),
            ),
            _ => return,
        };
        let owner = ctx.sym(class).owner.expect("class owner");
        let owner_text = match ctx.sym(owner).kind {
            crate::code::kinds::Kind::PCK => {
                let fullname = match &ctx.sym(owner).kind_data {
                    SymbolKind::PackageSymbol { fullname, .. }
                    | SymbolKind::RootPackageSymbol { fullname, .. } => {
                        fullname.to_java_string().to_string()
                    }
                    _ => String::new(),
                };
                if fullname.is_empty() {
                    "PACKAGE:unnamed package".to_string()
                } else {
                    format!("PACKAGE:{fullname}")
                }
            }
            crate::code::kinds::Kind::TYP => {
                let owner_flat = match &ctx.sym(owner).kind_data {
                    SymbolKind::ClassSymbol { fullname, .. } => {
                        fullname.to_java_string().to_string()
                    }
                    _ => ctx.sym(owner).name.to_java_string().to_string(),
                };
                format!("{}:{owner_flat}", type_kind(ctx.sym(owner).flags_field))
            }
            _ => format!("{:?}:{}", ctx.sym(owner).kind, ctx.sym(owner).name.to_java_string()),
        };
        let nesting = match ctx.sym(owner).kind {
            crate::code::kinds::Kind::PCK => "TOP_LEVEL",
            crate::code::kinds::Kind::TYP => "MEMBER",
            crate::code::kinds::Kind::MTH | crate::code::kinds::Kind::VAR => "LOCAL",
            _ => "ANONYMOUS",
        };
        let kind = type_kind(ctx.sym(class).flags_field);
        let modifiers = modifier_names(ctx.sym(class).flags_field);
        out.push_str(&format!(
            "{indent}TYPE flatname={flatname} name={name} kind={kind} nesting={nesting} owner={owner_text} modifiers=[{modifiers}]\n"
        ));
        if let Some(members) = class_members_scope(ctx, class) {
            // The scope iteration order is an implementation detail (hash table);
            // the oracle comparison normalizes it by symbol name.
            let mut symbols = ctx.scope_get_symbols(
                members,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|symbol| ctx.sym(symbol).kind == crate::code::kinds::Kind::TYP,
            );
            symbols.sort_by_key(|symbol| ctx.sym(*symbol).name.to_java_string().to_string());
            for symbol in symbols {
                print_type(ctx, symbol, depth + 1, out);
            }
        }
    }

    fn oracle_sample(name: &str) -> (i32, String) {
        let mut errors = -1;
        let mut types = String::new();
        let mut in_sample = false;
        for line in ENTER_ORACLE.lines() {
            if let Some(rest) = line.strip_prefix("SAMPLE ") {
                in_sample = rest.starts_with(name);
                if in_sample {
                    let errors_text = rest.split("errors=").nth(1).expect("errors");
                    errors = errors_text.trim().parse().expect("errors count");
                }
                continue;
            }
            if !in_sample {
                continue;
            }
            if line.starts_with("UNIT ") || line.starts_with("ERROR ") {
                continue;
            }
            if line.starts_with("TYPE ") || line.trim_start().starts_with("TYPE ") {
                types.push_str(line);
                types.push('\n');
            }
        }
        (errors, types)
    }

    #[test]
    fn enter_matches_the_javac_oracle() {
        let mut sample_names: Vec<&str> = Vec::new();
        for line in ENTER_ORACLE.lines() {
            if let Some(rest) = line.strip_prefix("SAMPLE ") {
                sample_names.push(rest.split(' ').next().expect("sample name"));
            }
        }
        assert!(!sample_names.is_empty(), "enter oracle is empty");
        for sample in sample_names {
            let (expected_errors, expected_types) = oracle_sample(sample);
            let mut harness = harness(sample);
            run_enter(&mut harness, sample);
            let actual_errors = harness.log.borrow().nerrors;
            let actual_types = {
                let mut out = String::new();
                for def in compilation_unit_defs(&harness.ctx, harness.unit).iter() {
                    if harness.ctx.tree(def).has_tag(Tag::CLASSDEF)
                        && let Some(class) = class_tree_symbol(&harness.ctx, def) {
                            print_type(&harness.ctx, class, 0, &mut out);
                        }
                }
                out
            };
            assert_eq!(actual_errors, expected_errors, "{sample}: error count");
            assert_eq!(actual_types, expected_types, "{sample}: type structure");
        }
    }

    #[test]
    fn enter_enters_nested_classes_into_scopes() {
        let mut harness = harness("Sample.java");
        run_enter(&mut harness, "Sample.java");
        let ctx = &harness.ctx;
        let packge = compilation_unit_package(ctx, harness.unit).expect("tree.packge");
        let toplevel_scope = compilation_unit_toplevel_scope(ctx, harness.unit).expect("toplevelScope");
        let names: Vec<String> = ctx
            .scope_get_symbols(
                toplevel_scope,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            )
            .into_iter()
            .map(|symbol| ctx.sym(symbol).name.to_java_string().to_string())
            .collect();
        assert!(names.contains(&"A".to_string()), "toplevel scope: {names:?}");
        assert!(names.contains(&"Top".to_string()), "toplevel scope: {names:?}");
        assert_eq!(
            package_fullname(ctx, packge),
            "p",
            "package of the compilation unit"
        );
    }
}
