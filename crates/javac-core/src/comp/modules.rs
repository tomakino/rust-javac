// Origin: com/sun/tools/javac/comp/Modules.java @ 982d53c (jdk-17.0.19-ga)

//! This class provides the module system: module declarations are entered,
//! resolved and completed; the module graph (requires/transitive closure),
//! visible packages and --add-exports/--add-reads/--add-modules handling are
//! ported 1:1 from Modules.java.
//!
//! PORTING_RULES:
//! - the service fields are injected explicitly (ADR 0001); the class is
//!   shared as Rc<Modules> and its mutable Java fields are RefCell/Cell values,
//!   so the completed Symbol.completer callbacks can re-enter the module graph
//!   without a RefCell borrow of the whole service (registered adapter);
//! - JNIWriter.multiModuleMode is not set (JNIWriter is not ported yet;
//!   registered NOT PORTED in this file);
//! - getUsesProvidesCompleter/UsesProvidesVisitor are ported (Modules.java:
//!   967-1211) together with the Check deprecated/preview/module-requires/
//!   opens checks they use.
//!
//! NOT PORTED IN THIS FILE:
//! - JNIWriter.multiModuleMode assignment (Modules.java:206-207): JNIWriter is
//!   not ported (registered dependency);
//! - (none: getUsesProvidesCompleter/UsesProvidesVisitor are ported).

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

use javac_tools::tools::java_file_manager::{JavaFileManager, JavaFileObjectRef, Location, LocationRef};
use javac_tools::tools::java_file_object::{JavaFileObject, Kind as FileKind};
use javac_tools::tools::standard_location::StandardLocation;

use crate::code::class_finder::ClassFinder;
use crate::code::deferred_completion_failure_handler::DeferredCompletionFailureHandler;
use crate::code::directive::{Directive, ExportsFlag, OpensFlag, RequiresFlag};
use crate::code::flags;
use crate::code::symbol::ModuleFlags;
use crate::code::lint::LintCategory;
use crate::code::kinds::Kind;
use crate::code::module_finder::ModuleFinder;
use crate::code::source::{Feature, Source};
use crate::code::symbol::{
    Completer, CompleterRef, CompletionFailure, ModuleResolutionFlags, SymbolKind,
};
use crate::code::symtab::Symtab;
use crate::code::type_tag::TypeTag;
use crate::comp::attr::Attr;
use crate::comp::attr_context::AttrContext;
use crate::comp::check::Check;
use crate::code::deferred_lint_handler::DeferredLintHandler;
use crate::comp::env::Env;
use crate::comp::type_envs::TypeEnvs;
use crate::compiler_context::CompilerContext;
use crate::ids::{DirectiveId, SymbolId, TreeId, TypeId};
use crate::jvm::target::Target;
use crate::resources::compiler_properties::{errors, warnings};
use crate::resources::diagnostic_contract::SymbolArg;
use crate::tree::jc_tree::TreeKind;
use javac_source::tree::module_tree::ModuleKind;
use crate::tree::tree_info;
use crate::util::list::List;
use crate::util::abstract_log::AbstractLog;
use crate::util::java_string::JavaString;
use crate::util::jc_diagnostic::{Error as JCDiagnosticError, SimpleDiagnosticPosition, Warning as JCDiagnosticWarning, DiagnosticPosition};
use crate::util::log::Log;
use crate::util::name::Name;
use crate::util::names::Names;
use crate::util::options::Options;

/// Java: interface Modules.PackageNameFinder (Modules.java:1816-1818).
pub trait PackageNameFinder {
    /// Java: Name findPackageNameOf(JavaFileObject jfo)
    fn find_package_name_of(&self, jfo: &dyn JavaFileObject) -> Option<Name>;
}

const ALL_SYSTEM: &str = "ALL-SYSTEM";
const ALL_MODULE_PATH: &str = "ALL-MODULE-PATH";

/// Java: public class Modules extends JCTree.Visitor (Modules.java:133).
pub struct Modules {
    log: Rc<RefCell<Log>>,
    names: Rc<Names>,
    attr: Rc<RefCell<Attr>>,
    chk: Rc<RefCell<Check>>,
    deferred_lint_handler: Rc<RefCell<DeferredLintHandler>>,
    type_envs: Rc<RefCell<TypeEnvs>>,
    types: Rc<crate::code::types::Types>,
    file_manager: Rc<dyn JavaFileManager>,
    module_finder: Rc<RefCell<ModuleFinder>>,
    source: Source,
    target: Target,
    allow_modules: bool,
    allow_access_into_system: bool,
    /// Java: public final boolean multiModuleMode.
    pub multi_module_mode: bool,
    java_se: Name,
    java_: Name,
    /// Java: ModuleSymbol defaultModule (null -> None).
    default_module: RefCell<Option<SymbolId>>,
    add_exports_opt: Option<String>,
    add_exports: RefCell<Option<HashMap<SymbolId, Vec<DirectiveId>>>>,
    add_reads_opt: Option<String>,
    add_reads: RefCell<Option<HashMap<SymbolId, Vec<DirectiveId>>>>,
    add_mods_opt: Option<String>,
    extra_add_mods: RefCell<HashSet<String>>,
    limit_mods_opt: Option<String>,
    extra_limit_mods: RefCell<HashSet<String>>,
    module_version_opt: Option<String>,
    lint_options: bool,
    root_modules: RefCell<Option<Vec<SymbolId>>>,
    warned_missing: RefCell<HashSet<SymbolId>>,
    /// Java: public PackageNameFinder findPackageInFile.
    pub find_package_in_file: RefCell<Option<Rc<dyn PackageNameFinder>>>,
    /// Java: int depth = -1.
    depth: Cell<i32>,
    /// Java: boolean inInitModules.
    in_init_modules: Cell<bool>,
    /// Java: private Set<ModuleSymbol> allModules.
    all_modules: RefCell<Option<Vec<SymbolId>>>,
    requires_transitive_cache: RefCell<HashMap<SymbolId, Vec<SymbolId>>>,
    self_ref: Weak<Modules>,
}

impl Modules {
    /// Java: protected Modules(Context context) (Modules.java:183-217).
    ///
    /// # Arguments
    ///   - the injected javac services and options;
    ///   - self_ref: the cyclic handle used by the completer closures.
    ///
    /// # Returns
    ///   - the module system.
    ///
    /// # Panics
    ///   - none.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        log: Rc<RefCell<Log>>,
        names: Rc<Names>,
        attr: Rc<RefCell<Attr>>,
        chk: Rc<RefCell<Check>>,
        deferred_lint_handler: Rc<RefCell<DeferredLintHandler>>,
        type_envs: Rc<RefCell<TypeEnvs>>,
        types: Rc<crate::code::types::Types>,
        file_manager: Rc<dyn JavaFileManager>,
        module_finder: Rc<RefCell<ModuleFinder>>,
        source: Source,
        target: Target,
        options: &Options,
        self_ref: Weak<Modules>,
    ) -> Modules {
        let allow_modules = Feature::MODULES.allowed_in_source(source);
        let allow_access_into_system = options.is_unset_option(crate::main::option::Option::RELEASE);
        let lint_options = options.is_unset_choice(
            crate::main::option::Option::XLINT_CUSTOM,
            &format!("-{}", crate::code::lint::LintCategory::OPTIONS.option()),
        );
        let multi_module_mode = file_manager.has_location(&StandardLocation::MODULE_SOURCE_PATH);
        let add_exports_opt = options
            .get_option(crate::main::option::Option::ADD_EXPORTS)
            .map(|s| s.to_string());
        let add_reads_opt = options
            .get_option(crate::main::option::Option::ADD_READS)
            .map(|s| s.to_string());
        let add_mods_opt = options
            .get_option(crate::main::option::Option::ADD_MODULES)
            .map(|s| s.to_string());
        let limit_mods_opt = options
            .get_option(crate::main::option::Option::LIMIT_MODULES)
            .map(|s| s.to_string());
        let module_version_opt = options
            .get_option(crate::main::option::Option::MODULE_VERSION)
            .map(|s| s.to_string());
        Modules {
            log,
            names: Rc::clone(&names),
            attr,
            chk,
            deferred_lint_handler,
            type_envs,
            types,
            file_manager,
            module_finder,
            source,
            target,
            allow_modules,
            allow_access_into_system,
            multi_module_mode,
            java_se: names.from_string(&JavaString::from("java.se")),
            java_: names.from_string(&JavaString::from("java.")),
            default_module: RefCell::new(None),
            add_exports_opt,
            add_exports: RefCell::new(None),
            add_reads_opt,
            add_reads: RefCell::new(None),
            add_mods_opt,
            extra_add_mods: RefCell::new(HashSet::new()),
            limit_mods_opt,
            extra_limit_mods: RefCell::new(HashSet::new()),
            module_version_opt,
            lint_options,
            root_modules: RefCell::new(None),
            warned_missing: RefCell::new(HashSet::new()),
            find_package_in_file: RefCell::new(None),
            depth: Cell::new(-1),
            in_init_modules: Cell::new(false),
            all_modules: RefCell::new(None),
            requires_transitive_cache: RefCell::new(HashMap::new()),
            self_ref,
        }
    }

    fn log(&self) -> std::cell::RefMut<'_, Log> {
        self.log.borrow_mut()
    }

    /// Java: public void addExtraAddModules(String... extras) (Modules.java:221-223).
    pub fn add_extra_add_modules(&self, extras: &[&str]) {
        let mut set = self.extra_add_mods.borrow_mut();
        for extra in extras {
            set.insert((*extra).to_string());
        }
    }

}

impl Modules {
    /// Java: public void initModules(List<JCCompilationUnit> trees)
    /// (Modules.java:226-242).
    pub fn init_modules(&self, ctx: &mut CompilerContext, syms: &mut Symtab, trees: &[TreeId]) {
        assert!(!self.in_init_modules.get(), "AssertionError: inInitModules");
        self.in_init_modules.set(true);
        assert!(self.root_modules.borrow().is_none(), "AssertionError: rootModules");
        let result = self.enter_with(ctx, syms, trees, None, &mut |modules, ctx, syms| {
            assert!(self.root_modules.borrow().is_none());
            assert!(self.all_modules.borrow().is_none());
            *self.root_modules.borrow_mut() = Some(modules.clone());
            self.setup_all_modules(ctx, syms);
            assert!(self.all_modules.borrow().is_some());
            self.in_init_modules.set(false);
        });
        self.in_init_modules.set(false);
        let _ = result;
    }

    /// Java: public boolean enter(List<JCCompilationUnit> trees, ClassSymbol c)
    /// (Modules.java:244-247).
    pub fn enter(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        trees: &[TreeId],
        c: Option<SymbolId>,
    ) -> bool {
        assert!(
            self.root_modules.borrow().is_some()
                || self.in_init_modules.get()
                || !self.allow_modules,
            "AssertionError: enter precondition"
        );
        self.enter_with(ctx, syms, trees, c, &mut |_modules, _ctx, _syms| {})
    }

    /// Java: private boolean enter(List<JCCompilationUnit> trees,
    /// Consumer<Set<ModuleSymbol>> init, ClassSymbol c) (Modules.java:249-279).
    fn enter_with(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        trees: &[TreeId],
        c: Option<SymbolId>,
        init: &mut dyn FnMut(&Vec<SymbolId>, &mut CompilerContext, &mut Symtab),
    ) -> bool {
        if !self.allow_modules {
            for tree in trees {
                set_compilation_unit_module(ctx, *tree, syms.no_module);
            }
            *self.default_module.borrow_mut() = Some(syms.no_module);
            return true;
        }
        let start_errors = self.log.borrow().nerrors;
        self.depth.set(self.depth.get() + 1);
        let result: Result<(), CompletionFailure> = (|| {
            let mut roots = self.enter_modules(ctx, syms, trees, c);
            self.set_compilation_unit_modules(ctx, syms, trees, &mut roots, c)?;
            init(&roots, ctx, syms);
            for msym in &roots {
                ctx.complete_symbol(syms, *msym)?;
            }
            Ok(())
        })();
        self.depth.set(self.depth.get() - 1);
        if let Err(failure) = result {
            let chk = self.chk.borrow();
            chk.completion_error(ctx, crate::util::position::NOPOS, &failure, syms);
        }
        self.log.borrow().nerrors == start_errors
    }

    /// Java: public Completer getCompleter() (Modules.java:281-283).
    pub fn get_completer(self: &Rc<Modules>) -> CompleterRef {
        Rc::new(RefCell::new(MainCompleter {
            modules: Rc::clone(self),
        }))
    }

    /// Java: public ModuleSymbol getDefaultModule() (Modules.java:285-287).
    pub fn get_default_module(&self) -> Option<SymbolId> {
        *self.default_module.borrow()
    }

    /// Java: public boolean modulesInitialized() (Modules.java:289-291).
    pub fn modules_initialized(&self) -> bool {
        self.all_modules.borrow().is_some()
    }

    /// Java: private Set<ModuleSymbol> enterModules(List<JCCompilationUnit> trees,
    /// ClassSymbol c) (Modules.java:293-304).
    fn enter_modules(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        trees: &[TreeId],
        c: Option<SymbolId>,
    ) -> Vec<SymbolId> {
        let mut modules: Vec<SymbolId> = Vec::new();
        for tree in trees {
            let prev = match compilation_unit_sourcefile(ctx, *tree) {
                Some(source) => AbstractLog::use_source(&mut *self.log.borrow_mut(), Some(source)),
                None => None,
            };
            self.enter_module(ctx, syms, *tree, c, &mut modules);
            AbstractLog::use_source(&mut *self.log.borrow_mut(), prev);
        }
        modules
    }

    /// Java: private void enterModule(JCCompilationUnit toplevel, ClassSymbol c,
    /// Set<ModuleSymbol> modules) (Modules.java:307-351).
    fn enter_module(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        toplevel: TreeId,
        c: Option<SymbolId>,
        modules: &mut Vec<SymbolId>,
    ) {
        let sourcefile = compilation_unit_sourcefile(ctx, toplevel)
            .expect("CompilationUnit.sourcefile (upstream non-null)");
        let is_module_info = sourcefile.is_name_compatible(
            &JavaString::from("module-info"),
            FileKind::SOURCE,
        );
        let module_decl = tree_info::get_module(ctx, toplevel);
        if let Some(decl) = module_decl {
            if !is_module_info {
                let pos = ctx.tree(decl).pos;
                self.log.borrow_mut().error_at(
                    pos,
                    JCDiagnosticError::from(errors::module_decl_sb_in_module_info_java()),
                );
            }
            let qual_id = match &ctx.tree(decl).kind {
                TreeKind::ModuleDecl { qual_id, .. } => *qual_id,
                _ => None,
            }
            .expect("module decl qualId");
            let name = tree_info::full_name(ctx, qual_id).expect("module name");
            let sym = match c {
                Some(c) => {
                    let owner = ctx.sym(c).owner.expect("module owner of package-info");
                    let tree_name = tree_info::full_name(ctx, qual_id).expect("module name");
                    if ctx.sym(owner).name.to_java_string()
                        != tree_name.to_java_string()
                    {
                        let pos = ctx.tree(decl).pos;
                        self.log.borrow_mut().error_at(
                            pos,
                            JCDiagnosticError::from(errors::module_name_mismatch(tree_name.clone(), ctx.sym(owner).name.clone())),
                        );
                    }
                    owner
                }
                None => {
                    let sym = syms.enter_module(ctx, name);
                    let info = module_info_symbol(ctx, sym);
                    if module_info_sourcefile(ctx, info).is_some()
                        && !same_file(module_info_sourcefile(ctx, info), &sourcefile)
                    {
                        self.log.borrow_mut().error_at(
                            ctx.tree(decl).pos,
                            JCDiagnosticError::from(errors::duplicate_module(
                                SymbolArg::placeholder(ctx.sym(sym).name.to_java_string().to_string_lossy()),
                            )),
                        );
                        return;
                    }
                    sym
                }
            };
            let source_completer = self.get_source_completer(toplevel, sourcefile.clone());
            ctx.sym_mut(sym).completer = Some(source_completer);
            set_module_info_sourcefile(ctx, sym, Some(sourcefile));
            if let TreeKind::ModuleDecl { sym: slot, .. } = &mut ctx.tree_mut(decl).kind {
                *slot = Some(sym);
            }
            if self.multi_module_mode || modules.is_empty() {
                modules.push(sym);
            } else {
                self.log.borrow_mut().error_at(
                    ctx.tree(toplevel).pos,
                    JCDiagnosticError::from(errors::too_many_modules()),
                );
            }
            let env = Rc::new(Env {
                next: None,
                outer: None,
                tree: Some(decl),
                toplevel: Some(toplevel),
                encl_class: None,
                encl_method: None,
                info: Rc::new(RefCell::new(AttrContext::new())),
                base_clause: false,
            });
            self.type_envs.borrow_mut().put(sym, env);
        } else if is_module_info && self.multi_module_mode {
            let tree = match &ctx.tree(toplevel).kind {
                TreeKind::CompilationUnit { defs, .. } if !defs.is_empty() => {
                    Some(*defs.head())
                }
                _ => Some(toplevel),
            };
            if let Some(tree) = tree {
                self.log.borrow_mut().error_at(
                    ctx.tree(tree).pos,
                    JCDiagnosticError::from(errors::expected_module()),
                );
            }
        }
    }
}

impl Modules {
    /// Java: private void setCompilationUnitModules(...) (Modules.java:353-534).
    fn set_compilation_unit_modules(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        trees: &[TreeId],
        root_modules: &mut Vec<SymbolId>,
        c: Option<SymbolId>,
    ) -> Result<(), CompletionFailure> {
        if self.multi_module_mode {
            let mut patches_automatic_modules = false;
            for tree in trees {
                let defs_empty = match &ctx.tree(*tree).kind {
                    TreeKind::CompilationUnit { defs, .. } => defs.is_empty(),
                    _ => false,
                };
                if defs_empty {
                    set_compilation_unit_module(ctx, *tree, syms.unnamed_module);
                    continue;
                }
                let sourcefile = compilation_unit_sourcefile(ctx, *tree)
                    .expect("CompilationUnit.sourcefile");
                let prev = AbstractLog::use_source(&mut *self.log.borrow_mut(), Some(sourcefile.clone()));
                let msp_locn = self.get_module_location(ctx, *tree);
                let patch_locn = if self
                    .file_manager
                    .has_location(&StandardLocation::PATCH_MODULE_PATH)
                {
                    self.file_manager
                        .get_location_for_module_of_file(
                            &StandardLocation::PATCH_MODULE_PATH,
                            &sourcefile,
                        )
                        .ok()
                } else {
                    None
                };
                if let Some(plocn) = patch_locn {
                    let name = self
                        .names
                        .from_string(&self.module_name(ctx, &plocn));
                    let msym = self
                        .module_finder
                        .borrow_mut()
                        .find_module_by_name(ctx, syms, &name);
                    set_compilation_unit_module(ctx, *tree, msym);
                    root_modules.push(msym);
                    patches_automatic_modules |=
                        (ctx.sym(msym).flags_field & flags::AUTOMATIC_MODULE) != 0;
                    if let Some(msplocn) = &msp_locn {
                        let mspname = self
                            .names
                            .from_string(&self.module_name(ctx, msplocn));
                        if ctx.sym(msym).name.to_java_string()
                            != mspname.to_java_string()
                        {
                            self.log.borrow_mut().error_at(
                                ctx.tree(*tree).pos,
                                JCDiagnosticError::from(errors::file_patched_and_msp(ctx.sym(msym).name.clone(), mspname.clone())),
                            );
                        }
                    }
                } else if let Some(msplocn) = &msp_locn {
                    let decl = tree_info::get_module(ctx, *tree);
                    if let Some(decl) = decl {
                        let canonical = self
                            .file_manager
                            .get_java_file_for_input(
                                msplocn.as_ref(),
                                &self.names.module_info.to_java_string(),
                                FileKind::SOURCE,
                            )
                            .ok()
                            .flatten();
                        let same = canonical
                            .map(|canonical| {
                                !self
                                    .file_manager
                                    .is_same_file(&(canonical as javac_tools::tools::java_file_manager::FileObjectRef), &(sourcefile.clone() as javac_tools::tools::java_file_manager::FileObjectRef))
                            })
                            .unwrap_or(true);
                        if same {
                            self.log.borrow_mut().error_at(
                                ctx.tree(*tree).pos,
                                JCDiagnosticError::from(
                                    errors::module_not_found_on_module_source_path(),
                                ),
                            );
                        }
                    }
                    let name = self
                        .names
                        .from_string(&self.module_name(ctx, msplocn));
                    let msym = match decl {
                        Some(decl) => {
                            let sym = match &ctx.tree(decl).kind {
                                TreeKind::ModuleDecl { sym: Some(sym), .. } => *sym,
                                _ => panic!("AssertionError: module decl sym"),
                            };
                            if ctx.sym(sym).name.to_java_string()
                                != name.to_java_string()
                            {
                                let pos = match &ctx.tree(decl).kind {
                                    TreeKind::ModuleDecl { qual_id, .. } => {
                                        qual_id.map(|q| ctx.tree(q).pos).unwrap_or(0)
                                    }
                                    _ => 0,
                                };
                                self.log.borrow_mut().error_at(
                                    pos,
                                    JCDiagnosticError::from(errors::module_name_mismatch(ctx.sym(sym).name.clone(), name.clone())),
                                );
                            }
                            sym
                        }
                        None => {
                            let pkg = compilation_unit_package(ctx, *tree);
                            if pkg.is_none() {
                                self.log.borrow_mut().error_at(
                                    ctx.tree(*tree).pos,
                                    JCDiagnosticError::from(
                                        errors::unnamed_pkg_not_allowed_named_modules(),
                                    ),
                                );
                            }
                            syms.enter_module(ctx, name.clone())
                        }
                    };
                    if module_source_location(ctx, msym).is_none() {
                        set_module_source_location(ctx, msym, Some(msplocn.clone()));
                        if self
                            .file_manager
                            .has_location(&StandardLocation::PATCH_MODULE_PATH)
                        {
                            let patch = self
                                .file_manager
                                .get_location_for_module(
                                    &StandardLocation::PATCH_MODULE_PATH,
                                    &ctx.sym(msym).name.to_java_string(),
                                )
                                .ok();
                            set_module_patch_location(ctx, msym, patch);
                        }
                        if self
                            .file_manager
                            .has_location(&StandardLocation::CLASS_OUTPUT)
                        {
                            let output = self
                                .file_manager
                                .get_location_for_module(
                                    &StandardLocation::CLASS_OUTPUT,
                                    &ctx.sym(msym).name.to_java_string(),
                                )
                                .ok();
                            if module_patch_location(ctx, msym).is_none() {
                                set_module_class_location(ctx, msym, output);
                            } else {
                                set_module_patch_output_location(ctx, msym, output);
                            }
                        }
                    }
                    set_compilation_unit_module(ctx, *tree, msym);
                    root_modules.push(msym);
                } else if let Some(c) = c {
                    let packge = crate::code::symbol::symbol_packge(ctx, c);
                    let in_unnamed = packge
                        .and_then(|p| module_of_package(ctx, p))
                        .map(|m| m == syms.unnamed_module)
                        .unwrap_or(false);
                    if in_unnamed {
                        set_compilation_unit_module(ctx, *tree, syms.unnamed_module);
                    } else {
                        if tree_info::get_module(ctx, *tree).is_some() {
                            self.log.borrow_mut().error_at(
                                ctx.tree(*tree).pos,
                                JCDiagnosticError::from(
                                    errors::module_not_found_on_module_source_path(),
                                ),
                            );
                        } else {
                            self.log.borrow_mut().error_at(
                                ctx.tree(*tree).pos,
                                JCDiagnosticError::from(
                                    errors::not_in_module_on_module_source_path(),
                                ),
                            );
                        }
                        set_compilation_unit_module(ctx, *tree, syms.err_module);
                    }
                } else {
                    if tree_info::get_module(ctx, *tree).is_some() {
                        self.log.borrow_mut().error_at(
                            ctx.tree(*tree).pos,
                            JCDiagnosticError::from(
                                errors::module_not_found_on_module_source_path(),
                            ),
                        );
                    } else {
                        self.log.borrow_mut().error_at(
                            ctx.tree(*tree).pos,
                            JCDiagnosticError::from(
                                errors::not_in_module_on_module_source_path(),
                            ),
                        );
                    }
                    set_compilation_unit_module(ctx, *tree, syms.err_module);
                }
                AbstractLog::use_source(&mut *self.log.borrow_mut(), prev);
            }
            if !patches_automatic_modules {
                self.check_no_all_module_path();
            }
            let unnamed = syms.unnamed_module;
            if module_source_location(ctx, unnamed).is_none() {
                ctx.sym_mut(unnamed).completer = Some(Rc::new(RefCell::new(
                    UnnamedModuleCompleter {
                        modules: self.self_ref.upgrade().expect("Modules self"),
                    },
                )));
                set_module_source_location(
                    ctx,
                    unnamed,
                    Some(Rc::new(StandardLocation::SOURCE_PATH)),
                );
                set_module_class_location(
                    ctx,
                    unnamed,
                    Some(Rc::new(StandardLocation::CLASS_PATH)),
                );
            }
            *self.default_module.borrow_mut() = Some(unnamed);
            return Ok(());
        }
        let mut module: Option<SymbolId> = None;
        if self.default_module.borrow().is_none() {
            let module_override = self.single_module_override(ctx, trees);
            match root_modules.len() {
                0 => {
                    let default = match self
                        .module_finder
                        .borrow_mut()
                        .find_single_module(ctx, syms)
                    {
                        Ok(default) => default,
                        Err(failure) => {
                            let chk = self.chk.borrow();
                            chk.completion_error(
                                ctx,
                                crate::util::position::NOPOS,
                                &failure,
                                syms,
                            );
                            syms.unnamed_module
                        }
                    };
                    *self.default_module.borrow_mut() = Some(default);
                    if default == syms.unnamed_module {
                        if let Some(module_override) = module_override {
                            let default = self.module_finder.borrow_mut().find_module_by_name(
                                ctx,
                                syms,
                                &self.names.from_string(&JavaString::from(module_override.as_str())),
                            );
                            set_module_patch_output_location(
                                ctx,
                                default,
                                Some(Rc::new(StandardLocation::CLASS_OUTPUT)),
                            );
                            *self.default_module.borrow_mut() = Some(default);
                            if (ctx.sym(default).flags_field & flags::AUTOMATIC_MODULE) == 0 {
                                self.check_no_all_module_path();
                            }
                        } else {
                            set_module_completer(
                                ctx,
                                default,
                                Some(Rc::new(RefCell::new(UnnamedModuleCompleter {
                                    modules: self.self_ref.upgrade().expect("Modules self"),
                                }))),
                            );
                            set_module_source_location(
                                ctx,
                                default,
                                Some(Rc::new(StandardLocation::SOURCE_PATH)),
                            );
                            set_module_class_location(
                                ctx,
                                default,
                                Some(Rc::new(StandardLocation::CLASS_PATH)),
                            );
                        }
                    } else {
                        self.check_no_all_module_path();
                        ctx.complete_symbol(syms, default)?;
                        set_module_completer(
                            ctx,
                            default,
                            Some(Rc::new(RefCell::new(MainCompleter {
                                modules: self.self_ref.upgrade().expect("Modules self"),
                            }))),
                        );
                        set_module_source_location(
                            ctx,
                            default,
                            Some(Rc::new(StandardLocation::SOURCE_PATH)),
                        );
                    }
                    root_modules.push(self.default_module.borrow().expect("default module"));
                }
                1 => {
                    self.check_no_all_module_path();
                    let default = root_modules[0];
                    *self.default_module.borrow_mut() = Some(default);
                    set_module_source_location(
                        ctx,
                        default,
                        Some(Rc::new(StandardLocation::SOURCE_PATH)),
                    );
                    if self
                        .file_manager
                        .has_location(&StandardLocation::PATCH_MODULE_PATH)
                    {
                        let patch = self
                            .file_manager
                            .get_location_for_module(
                                &StandardLocation::PATCH_MODULE_PATH,
                                &ctx.sym(default).name.to_java_string(),
                            )
                            .ok();
                        set_module_patch_location(ctx, default, patch);
                    }
                    if module_patch_location(ctx, default).is_none() {
                        set_module_class_location(
                            ctx,
                            default,
                            Some(Rc::new(StandardLocation::CLASS_OUTPUT)),
                        );
                    } else {
                        set_module_patch_output_location(
                            ctx,
                            default,
                            Some(Rc::new(StandardLocation::CLASS_OUTPUT)),
                        );
                    }
                }
                _ => crate::util::assert::error(),
            }
        } else if root_modules.len() == 1 {
            module = Some(root_modules[0]);
            let module = module.unwrap();
            ctx.complete_symbol(syms, module)?;
            set_module_completer(
                ctx,
                module,
                Some(Rc::new(RefCell::new(MainCompleter {
                    modules: self.self_ref.upgrade().expect("Modules self"),
                }))),
            );
        } else {
            assert!(root_modules.is_empty(), "AssertionError: rootModules");
            let c = c.expect("AssertionError: class symbol");
            let module_sym = crate::code::symbol::symbol_packge(ctx, c)
                .and_then(|p| module_of_package(ctx, p))
                .expect("package module");
            root_modules.push(module_sym);
            module = Some(module_sym);
        }
        let default = *self.default_module.borrow();
        if let Some(default) = default
            && default != syms.unnamed_module
        {
            ctx.sym_mut(syms.unnamed_module).completer = Some(Rc::new(RefCell::new(
                UnnamedModuleCompleter {
                    modules: self.self_ref.upgrade().expect("Modules self"),
                },
            )));
            set_module_class_location(
                ctx,
                syms.unnamed_module,
                Some(Rc::new(StandardLocation::CLASS_PATH)),
            );
        }
        let module = match module {
            Some(module) => module,
            None => default.expect("default module"),
        };
        for tree in trees {
            if let Some(default) = default
                && default != syms.unnamed_module
                && module_source_location_is_source_path(ctx, default)
                && self
                    .file_manager
                    .has_location(&StandardLocation::SOURCE_PATH)
            {
                self.check_source_location(ctx, *tree, module);
            }
            set_compilation_unit_module(ctx, *tree, module);
        }
        Ok(())
    }

    /// Java: private void checkSourceLocation(JCCompilationUnit tree,
    /// ModuleSymbol msym) (Modules.java:536-564).
    fn check_source_location(&self, ctx: &mut CompilerContext, tree: TreeId, msym: SymbolId) {
        let source = compilation_unit_sourcefile(ctx, tree).expect("sourcefile");
        let contains = |loc: Option<LocationRef>| -> bool {
            let Some(loc) = loc else { return false };
            self.file_manager.contains(&*loc, &(Rc::clone(&source) as javac_tools::tools::java_file_manager::FileObjectRef)).unwrap_or(false)
        };
        if contains(module_source_location(ctx, msym)) {
            return;
        }
        if contains(module_patch_location(ctx, msym)) {
            return;
        }
        if self
            .file_manager
            .has_location(&StandardLocation::SOURCE_OUTPUT)
        {
            if contains(Some(Rc::new(StandardLocation::SOURCE_OUTPUT))) {
                return;
            }
        } else if contains(Some(Rc::new(StandardLocation::CLASS_OUTPUT))) {
            return;
        }
        let prev = AbstractLog::use_source(&mut *self.log.borrow_mut(), Some(source));
        self.log.borrow_mut().error_at(
            ctx.tree(tree).pos,
            JCDiagnosticError::from(errors::file_sb_on_source_or_patch_path_for_module()),
        );
        AbstractLog::use_source(&mut *self.log.borrow_mut(), prev);
    }

    /// Java: private String singleModuleOverride(List<JCCompilationUnit> trees)
    /// (Modules.java:566-594).
    fn single_module_override(&self, ctx: &mut CompilerContext, trees: &[TreeId]) -> Option<String> {
        if !self
            .file_manager
            .has_location(&StandardLocation::PATCH_MODULE_PATH)
        {
            return None;
        }
        let mut overrides: Vec<String> = Vec::new();
        for tree in trees {
            let source = compilation_unit_sourcefile(ctx, *tree).expect("sourcefile");
            if let Ok(loc) = self.file_manager.get_location_for_module_of_file(
                &StandardLocation::PATCH_MODULE_PATH,
                &source,
            ) {
                let name = self.module_name(ctx, &loc).to_string_lossy();
                if !overrides.iter().any(|o| o == &name) {
                    overrides.push(name);
                }
            }
        }
        match overrides.len() {
            0 => None,
            1 => Some(overrides[0].clone()),
            _ => {
                self.log.borrow_mut().error_key(JCDiagnosticError::from(
                    errors::too_many_patched_modules(
                        overrides
                            .iter()
                            .map(|s| JavaString::from(s.as_str()))
                            .collect(),
                    ),
                ));
                None
            }
        }
    }

    /// Java: private Location getModuleLocation(JCCompilationUnit tree)
    /// (Modules.java:606-618).
    fn get_module_location(&self, ctx: &mut CompilerContext, tree: TreeId) -> Option<LocationRef> {
        let source = compilation_unit_sourcefile(ctx, tree).expect("sourcefile");
        let loc = self
            .file_manager
            .get_location_for_module_of_file(&StandardLocation::MODULE_SOURCE_PATH, &source)
            .ok();
        if loc.is_some() {
            return loc;
        }
        let source_output = if self
            .file_manager
            .has_location(&StandardLocation::SOURCE_OUTPUT)
        {
            StandardLocation::SOURCE_OUTPUT
        } else {
            StandardLocation::CLASS_OUTPUT
        };
        self.file_manager
            .get_location_for_module_of_file(&source_output, &source)
            .ok()
    }

    /// Java: private void checkNoAllModulePath() (Modules.java:620-624).
    fn check_no_all_module_path(&self) {
        if let Some(add_mods) = &self.add_mods_opt
            && add_mods.split(',').any(|s| s == ALL_MODULE_PATH)
        {
            self.log
                .borrow_mut()
                .error_key(JCDiagnosticError::from(errors::addmods_all_module_path_invalid()));
        }
    }

    /// Java: private void setupAllModules() (Modules.java:1220-1360).
    fn setup_all_modules(&self, ctx: &mut CompilerContext, syms: &mut Symtab) {
        assert!(self.root_modules.borrow().is_some(), "AssertionError: rootModules");
        assert!(self.all_modules.borrow().is_none(), "AssertionError: allModules");
        let roots = self.root_modules.borrow().clone().expect("rootModules");
        let mut observable: Option<Vec<SymbolId>> = None;
        let limit_mods_opt = &self.limit_mods_opt;
        let extra_limit_mods = self.extra_limit_mods.borrow().clone();
        if limit_mods_opt.is_some() || !extra_limit_mods.is_empty() {
            let mut limit_mods: Vec<SymbolId> = Vec::new();
            if let Some(limit_mods_opt) = limit_mods_opt {
                for limit in limit_mods_opt.split(',') {
                    if !self.is_valid_name(limit) {
                        continue;
                    }
                    let name = self.names.from_string(&JavaString::from(limit));
                    let module = syms.enter_module(ctx, name);
                    if !limit_mods.contains(&module) {
                        limit_mods.push(module);
                    }
                }
            }
            for limit in &extra_limit_mods {
                let name = self.names.from_string(&JavaString::from(limit.as_str()));
                let module = syms.enter_module(ctx, name);
                if !limit_mods.contains(&module) {
                    limit_mods.push(module);
                }
            }
            let mut closure = self.compute_transitive_closure(ctx, syms, &limit_mods, &roots, None);
            for root in &roots {
                if !closure.contains(root) {
                    closure.push(*root);
                }
            }
            observable = Some(closure);
            if self.lint_options {
                for msym in &limit_mods {
                    if !observable.as_ref().unwrap().contains(msym) {
                        let warning = JCDiagnosticWarning::from(warnings::module_for_option_not_found(
                            crate::resources::diagnostic_contract::OptionArg::placeholder("--limit-modules"),
                            SymbolArg::placeholder(
                                ctx.sym(*msym).name.to_java_string().to_string_lossy(),
                            ),
                        ));
                        self.log
                            .borrow_mut()
                            .warning_lint_key(LintCategory::OPTIONS, warning);
                    }
                }
            }
        }
        let system_module_pred = |ctx: &CompilerContext, sym: SymbolId| {
            (ctx.sym(sym).flags_field & flags::SYSTEM_MODULE) != 0
        };
        let mut enabled_root: Vec<SymbolId> = Vec::new();
        let unnamed = syms.unnamed_module;
        if roots.contains(&unnamed) {
            let java_se = syms.get_module(&self.java_se);
            let jdk_module_pred: Box<
                dyn Fn(&mut CompilerContext, &mut Symtab, SymbolId)
                    -> Result<bool, CompletionFailure>,
            > = if self.target.all_api_modules_are_roots() {
                Box::new(move |ctx, syms, sym| {
                    ctx.complete_symbol(syms, sym)?;
                    let exports = module_exports(ctx, sym);
                    Ok(exports
                        .iter()
                        .any(|d| ctx.directive(d).get_target_modules().is_none()))
                })
            } else if let Some(java_se) = java_se
                && observable
                    .as_ref()
                    .map(|o| o.contains(&java_se))
                    .unwrap_or(true)
            {
                enabled_root.push(java_se);
                Box::new(move |ctx, syms, sym| {
                    ctx.complete_symbol(syms, sym)?;
                    Ok(
                        !ctx.sym(sym)
                            .name
                            .to_java_string()
                            .to_string_lossy()
                            .starts_with("java.")
                            && module_exports(ctx, sym)
                                .iter()
                                .any(|d| ctx.directive(d).get_target_modules().is_none()),
                    )
                })
            } else {
                Box::new(|_ctx, _syms, _sym| Ok(true))
            };
            let no_incubator_pred = |ctx: &mut CompilerContext,
                                     syms: &mut Symtab,
                                     sym: SymbolId|
             -> Result<bool, CompletionFailure> {
                ctx.complete_symbol(syms, sym)?;
                Ok(!module_resolution_flags(ctx, sym)
                    .contains(&ModuleResolutionFlags::DO_NOT_RESOLVE_BY_DEFAULT))
            };
            let all: Vec<SymbolId> = syms.get_all_modules();
            for sym in all {
                let attempt: Result<bool, CompletionFailure> = (|| {
                    if observable.is_none() {
                        let found = self
                            .module_finder
                            .borrow_mut()
                            .find_module(ctx, syms, sym);
                        if ctx.sym(found).kind == Kind::ERR {
                            return Ok(false);
                        }
                    }
                    let observable_ok = observable
                        .as_ref()
                        .map(|o| o.contains(&sym))
                        .unwrap_or(true);
                    let jdk_ok = jdk_module_pred(ctx, syms, sym)?;
                    let incubator_ok = no_incubator_pred(ctx, syms, sym)?;
                    Ok(system_module_pred(ctx, sym)
                        && observable_ok
                        && jdk_ok
                        && incubator_ok
                        && !enabled_root.contains(&sym))
                })();
                match attempt {
                    Ok(true) => enabled_root.push(sym),
                    Ok(false) => {}
                    Err(failure) => {
                        let chk = self.chk.borrow();
                        chk.completion_error(
                            ctx,
                            crate::util::position::NOPOS,
                            &failure,
                            syms,
                        );
                    }
                }
            }
        }
        for root in &roots {
            if !enabled_root.contains(root) {
                enabled_root.push(*root);
            }
        }
        let extra_add_mods = self.extra_add_mods.borrow().clone();
        if self.add_mods_opt.is_some() || !extra_add_mods.is_empty() {
            let mut full: Vec<String> = extra_add_mods.into_iter().collect();
            if let Some(add_mods) = &self.add_mods_opt {
                for m in add_mods.split(',') {
                    if !full.iter().any(|x| x == m) {
                        full.push(m.to_string());
                    }
                }
            }
            for added in &full {
                let mut modules: Vec<SymbolId> = Vec::new();
                if added == ALL_SYSTEM || added == ALL_MODULE_PATH {
                    let all: Vec<SymbolId> = syms.get_all_modules();
                    for sym in all {
                        let system = system_module_pred(ctx, sym);
                        let observable_ok = observable
                            .as_ref()
                            .map(|o| o.contains(&sym))
                            .unwrap_or(true);
                        if (added == ALL_SYSTEM) == system && observable_ok {
                            modules.push(sym);
                        }
                    }
                } else {
                    if !self.is_valid_name(added) {
                        continue;
                    }
                    let name = self.names.from_string(&JavaString::from(added.as_str()));
                    modules.push(syms.enter_module(ctx, name));
                }
                for sym in modules {
                    if !enabled_root.contains(&sym) {
                        enabled_root.push(sym);
                    }
                    if let Some(observable) = &mut observable
                        && !observable.contains(&sym)
                    {
                        observable.push(sym);
                    }
                }
            }
        }
        let mut result = self.compute_transitive_closure(ctx, syms, &enabled_root, &roots, observable);
        if !result.contains(&unnamed) {
            result.push(unnamed);
        }
        let has_automatic = result
            .iter()
            .any(|m| (ctx.sym(*m).flags_field & flags::AUTOMATIC_MODULE) != 0);
        if has_automatic {
            let all: Vec<SymbolId> = syms.get_all_modules();
            for sym in all {
                if (ctx.sym(sym).flags_field & flags::AUTOMATIC_MODULE) != 0
                    && !result.contains(&sym)
                {
                    result.push(sym);
                }
            }
        }
        let incubating: Vec<String> = result
            .iter()
            .filter(|m| {
                module_resolution_flags(ctx, **m)
                    .contains(&ModuleResolutionFlags::WARN_INCUBATING)
            })
            .map(|m| ctx.sym(*m).name.to_java_string().to_string_lossy())
            .collect();
        if !incubating.is_empty() {
            let warning = JCDiagnosticWarning::from(warnings::incubating_modules(
                JavaString::from(incubating.join(",").as_str()),
            ));
            self.log.borrow_mut().warning_key(warning);
        }
        if let Some(version) = &self.module_version_opt {
            let version_name = self.names.from_string(&JavaString::from(version.as_str()));
            for root in &roots {
                if let SymbolKind::ModuleSymbol { version, .. } =
                    &mut ctx.sym_mut(*root).kind_data
                {
                    *version = Some(version_name.clone());
                }
            }
        }
        *self.all_modules.borrow_mut() = Some(result);
    }

    /// Java: private Set<ModuleSymbol> computeTransitiveClosure(...)
    /// (Modules.java:1369-1421).
    fn compute_transitive_closure(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        base: &[SymbolId],
        root_modules: &[SymbolId],
        observable: Option<Vec<SymbolId>>,
    ) -> Vec<SymbolId> {
        let mut primary_todo: Vec<SymbolId> = Vec::new();
        let mut secondary_todo: Vec<SymbolId> = Vec::new();
        for ms in base {
            if root_modules.contains(ms) {
                primary_todo.push(*ms);
            } else {
                secondary_todo.push(*ms);
            }
        }
        let mut result: Vec<SymbolId> = Vec::new();
        result.push(syms.java_base);
        while !primary_todo.is_empty() || !secondary_todo.is_empty() {
            let (current, is_primary) = if let Some(current) = primary_todo.pop() {
                (current, true)
            } else {
                (secondary_todo.pop().expect("secondary todo"), false)
            };
            if let Some(observable) = &observable
                && !observable.contains(&current)
            {
                continue;
            }
            if result.contains(&current)
                || current == syms.unnamed_module
                || (ctx.sym(current).flags_field & flags::AUTOMATIC_MODULE) != 0
            {
                continue;
            }
            result.push(current);
            if let Err(failure) = ctx.complete_symbol(syms, current) {
                let chk = self.chk.borrow();
                chk.completion_error(ctx, crate::util::position::NOPOS, &failure, syms);
                continue;
            }
            if ctx.sym(current).kind == Kind::ERR
                && (is_primary || base.contains(&current))
                && self.warned_missing.borrow_mut().insert(current)
            {
                self.log.borrow_mut().error_key(JCDiagnosticError::from(
                    errors::module_not_found(SymbolArg::placeholder(
                        ctx.sym(current).name.to_java_string().to_string_lossy(),
                    )),
                ));
            }
            for rd in module_requires(ctx, current).iter() {
                let module = match ctx.directive(rd).get_dependency() {
                    Some(module) => module,
                    None => continue,
                };
                if module == syms.java_base {
                    continue;
                }
                if (ctx.directive(rd).is_transitive() && is_primary)
                    || root_modules.contains(&current)
                {
                    primary_todo.push(module);
                } else {
                    secondary_todo.push(module);
                }
            }
        }
        result
    }

    /// Java: public boolean isInModuleGraph(ModuleSymbol msym)
    /// (Modules.java:1365-1367).
    pub fn is_in_module_graph(&self, msym: SymbolId) -> bool {
        self.all_modules
            .borrow()
            .as_ref()
            .map(|all| all.contains(&msym))
            .unwrap_or(true)
    }

    /// Java: public ModuleSymbol getObservableModule(Name name)
    /// (Modules.java:1423-1431).
    pub fn get_observable_module(&self, ctx: &mut CompilerContext, syms: &mut Symtab, name: &Name) -> Option<SymbolId> {
        let module = syms.get_module(name)?;
        if self.all_modules().contains(&module) {
            Some(module)
        } else {
            None
        }
    }

    /// Java: public boolean isRootModule(ModuleSymbol module)
    /// (Modules.java:756-759).
    pub fn is_root_module(&self, module: SymbolId) -> bool {
        self.root_modules
            .borrow()
            .as_ref()
            .expect("AssertionError: rootModules")
            .contains(&module)
    }

    /// Java: public Set<ModuleSymbol> getRootModules() (Modules.java:761-764).
    pub fn get_root_modules(&self) -> Vec<SymbolId> {
        self.root_modules
            .borrow()
            .as_ref()
            .expect("AssertionError: rootModules")
            .clone()
    }

    /// Java: public Set<ModuleSymbol> allModules() (Modules.java:1215-1218).
    pub fn all_modules(&self) -> Vec<SymbolId> {
        self.all_modules
            .borrow()
            .as_ref()
            .expect("AssertionError: allModules")
            .clone()
    }

    /// Java: public void newRound() (Modules.java:1809-1814).
    pub fn new_round(&self) {
        *self.all_modules.borrow_mut() = None;
        *self.root_modules.borrow_mut() = None;
        *self.default_module.borrow_mut() = None;
        self.warned_missing.borrow_mut().clear();
    }
}


impl Modules {
    /// Java: private void initAddExports() (Modules.java:1614-1678).
    fn init_add_exports(&self, ctx: &mut CompilerContext, syms: &mut Symtab) {
        if self.add_exports.borrow().is_some() {
            return;
        }
        let mut add_exports: HashMap<SymbolId, Vec<DirectiveId>> = HashMap::new();
        let mut unknown_modules: HashSet<SymbolId> = HashSet::new();
        *self.add_exports.borrow_mut() = Some(HashMap::new());
        let Some(opt) = &self.add_exports_opt else {
            return;
        };
        for s in opt.split('\0') {
            if s.is_empty() {
                continue;
            }
            let Some((rest, targets)) = s.split_once('=') else { continue };
            let Some((module_name, package_name)) = rest.split_once('/') else { continue };
            if !self.is_valid_name(module_name) {
                continue;
            }
            let msym = syms.enter_module(
                ctx,
                self.names
                    .from_string(&JavaString::from(module_name)),
            );
            if !self.is_known_module(ctx, syms, msym, &mut unknown_modules) {
                continue;
            }
            if !self.is_valid_name(package_name) {
                continue;
            }
            if !self.allow_access_into_system
                && (ctx.sym(msym).flags_field & flags::SYSTEM_MODULE) != 0
            {
                let error = JCDiagnosticError::from(errors::add_exports_with_release(
                    SymbolArg::placeholder(ctx.sym(msym).name.to_java_string().to_string_lossy()),
                ));
                self.log.borrow_mut().error_key(error);
                continue;
            }
            let package = syms.enter_package(
                ctx,
                msym,
                self.names
                    .from_string(&JavaString::from(package_name)),
            );
            set_package_module(ctx, package, msym);
            let mut target_modules: Vec<SymbolId> = Vec::new();
            for to_module in targets.split([',', ' ']) {
                if to_module.is_empty() {
                    continue;
                }
                let target = if to_module == "ALL-UNNAMED" {
                    syms.unnamed_module
                } else {
                    if !self.is_valid_name(to_module) {
                        continue;
                    }
                    let target = syms.enter_module(
                        ctx,
                        self.names
                            .from_string(&JavaString::from(to_module)),
                    );
                    if !self.is_known_module(ctx, syms, target, &mut unknown_modules) {
                        continue;
                    }
                    target
                };
                target_modules.push(target);
            }
            let directive = ctx.alloc_directive(Directive::Exports {
                packge: Some(package),
                modules: Some(List::from_iterable(target_modules)),
                flags: Vec::new(),
            });
            add_exports.entry(msym).or_default().push(directive);
        }
        *self.add_exports.borrow_mut() = Some(add_exports);
    }

    /// Java: private boolean isKnownModule(...) (Modules.java:1680-1693).
    fn is_known_module(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        msym: SymbolId,
        unknown_modules: &mut HashSet<SymbolId>,
    ) -> bool {
        if self.all_modules().contains(&msym) {
            return true;
        }
        if !unknown_modules.contains(&msym) {
            if self.lint_options {
                let warning = JCDiagnosticWarning::from(warnings::module_for_option_not_found(
                    crate::resources::diagnostic_contract::OptionArg::placeholder("--add-exports"),
                    SymbolArg::placeholder(
                        ctx.sym(msym).name.to_java_string().to_string_lossy(),
                    ),
                ));
                self.log
                    .borrow_mut()
                    .warning_lint_key(LintCategory::OPTIONS, warning);
            }
            unknown_modules.insert(msym);
        }
        let _ = syms;
        false
    }

    /// Java: private void initAddReads() (Modules.java:1695-1755).
    fn init_add_reads(&self, ctx: &mut CompilerContext, syms: &mut Symtab) {
        if self.add_reads.borrow().is_some() {
            return;
        }
        let mut add_reads: HashMap<SymbolId, Vec<DirectiveId>> = HashMap::new();
        *self.add_reads.borrow_mut() = Some(HashMap::new());
        let Some(opt) = &self.add_reads_opt else {
            return;
        };
        for s in opt.split('\0') {
            if s.is_empty() {
                continue;
            }
            let Some((source_name, targets)) = s.split_once('=') else { continue };
            if !self.is_valid_name(source_name) {
                continue;
            }
            let msym = syms.enter_module(
                ctx,
                self.names
                    .from_string(&JavaString::from(source_name)),
            );
            if !self.all_modules().contains(&msym) {
                if self.lint_options {
                    let warning = JCDiagnosticWarning::from(warnings::module_for_option_not_found(
                        crate::resources::diagnostic_contract::OptionArg::placeholder("--add-reads"),
                        SymbolArg::placeholder(
                            ctx.sym(msym).name.to_java_string().to_string_lossy(),
                        ),
                    ));
                    self.log
                        .borrow_mut()
                        .warning_lint_key(LintCategory::OPTIONS, warning);
                }
                continue;
            }
            if !self.allow_access_into_system
                && (ctx.sym(msym).flags_field & flags::SYSTEM_MODULE) != 0
            {
                let error = JCDiagnosticError::from(errors::add_reads_with_release(
                    SymbolArg::placeholder(ctx.sym(msym).name.to_java_string().to_string_lossy()),
                ));
                self.log.borrow_mut().error_key(error);
                continue;
            }
            for target_name in targets.split([',', ' ']) {
                if target_name.is_empty() {
                    continue;
                }
                let target = if target_name == "ALL-UNNAMED" {
                    syms.unnamed_module
                } else {
                    if !self.is_valid_name(target_name) {
                        continue;
                    }
                    let target = syms.enter_module(
                        ctx,
                        self.names
                            .from_string(&JavaString::from(target_name)),
                    );
                    if !self.all_modules().contains(&target) {
                        if self.lint_options {
                            let warning = JCDiagnosticWarning::from(
                                warnings::module_for_option_not_found(
                                    crate::resources::diagnostic_contract::OptionArg::placeholder("--add-reads"),
                                    SymbolArg::placeholder(
                                        ctx.sym(target)
                                            .name
                                            .to_java_string()
                                            .to_string_lossy(),
                                    ),
                                ),
                            );
                            self.log
                                .borrow_mut()
                                .warning_lint_key(LintCategory::OPTIONS, warning);
                        }
                        continue;
                    }
                    target
                };
                let directive = ctx.alloc_directive(Directive::Requires {
                    module: Some(target),
                    flags: vec![RequiresFlag::EXTRA],
                });
                add_reads.entry(msym).or_default().push(directive);
            }
        }
        *self.add_reads.borrow_mut() = Some(add_reads);
    }

    /// Java: private void initVisiblePackages(ModuleSymbol msym,
    /// Collection<ModuleSymbol> readable) (Modules.java:1558-1577).
    fn init_visible_packages(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        msym: SymbolId,
        readable: &[SymbolId],
    ) {
        self.init_add_exports(ctx, syms);
        set_module_visible_packages(ctx, msym, HashMap::new());
        set_module_read_modules(ctx, msym, readable.to_vec());
        let mut seen: HashMap<Name, SymbolId> = HashMap::new();
        for rm in readable {
            if *rm == syms.unnamed_module {
                continue;
            }
            let exports = module_exports(ctx, *rm);
            self.add_visible_packages(ctx, msym, &mut seen, *rm, &exports.iter().collect::<Vec<_>>());
        }
        let add_exports = self.add_exports.borrow().clone().unwrap_or_default();
        let read_modules = module_read_modules(ctx, msym);
        for (exports_from, exports) in add_exports {
            if read_modules.contains(&exports_from) {
                self.add_visible_packages(ctx, msym, &mut seen, exports_from, &exports);
            }
        }
    }

    /// Java: private void addVisiblePackages(...) (Modules.java:1579-1612).
    fn add_visible_packages(
        &self,
        ctx: &mut CompilerContext,
        msym: SymbolId,
        seen_packages: &mut HashMap<Name, SymbolId>,
        exports_from: SymbolId,
        exports: &[DirectiveId],
    ) {
        for d in exports {
            let (package, target_modules) = match ctx.directive(*d) {
                Directive::Exports { packge, modules, .. } => (*packge, modules.clone()),
                _ => continue,
            };
            let Some(package) = package else { continue };
            let exported = target_modules
                .map(|modules| modules.iter().any(|m| m == msym))
                .unwrap_or(true);
            if !exported {
                continue;
            }
            let package_name = ctx.sym(package).name.clone();
            let previous = seen_packages.get(&package_name).copied();
            if let Some(previous) = previous
                && previous != exports_from
            {
                let env = self.type_envs.borrow().get(msym);
                let orig_source = env
                    .as_ref()
                    .and_then(|env| env.toplevel)
                    .and_then(|t| compilation_unit_sourcefile(ctx, t));
                let prev = orig_source
                    .as_ref()
                    .map(|s| AbstractLog::use_source(&mut *self.log.borrow_mut(), Some(s.clone())));
                let pos = env
                    .as_ref()
                    .and_then(|env| env.tree)
                    .map(|t| ctx.tree(t).pos)
                    .unwrap_or(crate::util::position::NOPOS);
                if is_unnamed_module(ctx, msym) {
                    let error = JCDiagnosticError::from(errors::package_clash_from_requires_in_unnamed(
                        package_name.clone(),
                        SymbolArg::placeholder(
                            ctx.sym(previous).name.to_java_string().to_string_lossy(),
                        ),
                        SymbolArg::placeholder(
                            ctx.sym(exports_from).name.to_java_string().to_string_lossy(),
                        ),
                    ));
                    self.log.borrow_mut().error_at(pos, error);
                } else {
                    let error = JCDiagnosticError::from(errors::package_clash_from_requires(
                        SymbolArg::placeholder(
                            ctx.sym(msym).name.to_java_string().to_string_lossy(),
                        ),
                        package_name.clone(),
                        SymbolArg::placeholder(
                            ctx.sym(previous).name.to_java_string().to_string_lossy(),
                        ),
                        SymbolArg::placeholder(
                            ctx.sym(exports_from).name.to_java_string().to_string_lossy(),
                        ),
                    ));
                    self.log.borrow_mut().error_at(pos, error);
                }
                if let Some(prev) = prev {
                    AbstractLog::use_source(&mut *self.log.borrow_mut(), prev);
                }
                continue;
            }
            seen_packages.insert(package_name.clone(), exports_from);
            let mut visible = module_visible_packages(ctx, msym);
            visible.insert(package_name, package);
            set_module_visible_packages(ctx, msym, visible);
        }
    }

    /// Java: private Set<ModuleSymbol> retrieveRequiresTransitive(ModuleSymbol msym)
    /// (Modules.java:1520-1556).
    fn retrieve_requires_transitive(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        msym: SymbolId,
    ) -> Vec<SymbolId> {
        if let Some(cached) = self.requires_transitive_cache.borrow().get(&msym) {
            return cached.clone();
        }
        let mut requires_transitive: Vec<SymbolId> = Vec::new();
        let mut seen: HashSet<SymbolId> = HashSet::new();
        let mut todo: Vec<SymbolId> = vec![msym];
        while let Some(current) = todo.pop() {
            if !seen.insert(current) {
                continue;
            }
            if !requires_transitive.contains(&current) {
                requires_transitive.push(current);
            }
            let _ = ctx.complete_symbol(syms, current);
            if current != syms.unnamed_module {
                for rd in module_requires(ctx, current).iter() {
                    if ctx.directive(rd).is_transitive()
                        && let Some(module) = ctx.directive(rd).get_dependency()
                    {
                        todo.push(module);
                    }
                }
            } else {
                for module in self.all_modules() {
                    todo.push(module);
                }
            }
        }
        requires_transitive.retain(|m| *m != msym);
        self.requires_transitive_cache
            .borrow_mut()
            .insert(msym, requires_transitive.clone());
        requires_transitive
    }

    /// Java: private void completeModule(ModuleSymbol msym)
    /// (Modules.java:1460-1518).
    fn complete_module(&self, ctx: &mut CompilerContext, syms: &mut Symtab, msym: SymbolId) {
        if self.in_init_modules.get() {
            set_module_completer(
                ctx,
                msym,
                Some(Rc::new(RefCell::new(MainCompleter {
                    modules: self.self_ref.upgrade().expect("Modules self"),
                }))),
            );
            return;
        }
        if (ctx.sym(msym).flags_field & flags::AUTOMATIC_MODULE) != 0 {
            self.complete_automatic_module(ctx, syms, msym);
        }
        self.init_add_reads(ctx, syms);
        let mut requires = module_requires(ctx, msym);
        if let Some(add_reads) = self.add_reads.borrow().clone()
            && let Some(extra) = add_reads.get(&msym)
        {
            requires = List::from_iterable(
                requires.iter().chain(extra.iter().copied()).collect::<Vec<_>>(),
            );
        }
        let mut requires_kept: Vec<DirectiveId> = Vec::new();
        for rd in requires.iter() {
            let module = ctx.directive(rd).get_dependency().expect("requires module");
            if !self.all_modules().contains(&module) {
                let env = self.type_envs.borrow().get(msym);
                if let Some(env) = env {
                    let source = env
                        .toplevel
                        .and_then(|t| compilation_unit_sourcefile(ctx, t));
                    let prev = source
                        .as_ref()
                        .map(|s| AbstractLog::use_source(&mut *self.log.borrow_mut(), Some(s.clone())));
                    let pos = env.tree.map(|t| ctx.tree(t).pos).unwrap_or(0);
                    let error = JCDiagnosticError::from(errors::module_not_found(
                        SymbolArg::placeholder(
                            ctx.sym(module).name.to_java_string().to_string_lossy(),
                        ),
                    ));
                    self.log.borrow_mut().error_at(pos, error);
                    if let Some(prev) = prev {
                        AbstractLog::use_source(&mut *self.log.borrow_mut(), prev);
                    }
                }
            } else {
                requires_kept.push(rd);
            }
        }
        set_module_requires(ctx, msym, List::from_iterable(requires_kept.clone()));
        let mut readable: Vec<SymbolId> = Vec::new();
        let mut transitive: Vec<SymbolId> = Vec::new();
        for rd in &requires_kept {
            let module = ctx.directive(*rd).get_dependency().expect("requires module");
            let _ = ctx.complete_symbol(syms, module);
            if !readable.contains(&module) {
                readable.push(module);
            }
            let s = self.retrieve_requires_transitive(ctx, syms, module);
            for m in &s {
                if !readable.contains(m) {
                    readable.push(*m);
                }
            }
            if ctx.directive(*rd).is_transitive() {
                if !transitive.contains(&module) {
                    transitive.push(module);
                }
                for m in &s {
                    if !transitive.contains(m) {
                        transitive.push(*m);
                    }
                }
            }
        }
        self.requires_transitive_cache
            .borrow_mut()
            .insert(msym, transitive);
        self.init_visible_packages(ctx, syms, msym, &readable);
        for d in module_exports(ctx, msym).iter() {
            if let Directive::Exports { packge: Some(packge), .. } = ctx.directive(d) {
                set_package_module(ctx, *packge, msym);
            }
        }
    }

    /// Java: private void setupAutomaticModule(ModuleSymbol msym)
    /// (Modules.java:674-699).
    fn setup_automatic_module(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        msym: SymbolId,
    ) -> Result<(), CompletionFailure> {
        let mut directives: Vec<DirectiveId> = Vec::new();
        let mut exports: Vec<DirectiveId> = Vec::new();
        let mut seen_packages: HashSet<String> = HashSet::new();
        let class_location = module_class_location(ctx, msym).expect("automatic module class location");
        let files = self
            .file_manager
            .list(&*class_location, &JavaString::from(""), &[FileKind::CLASS], true)
            .unwrap_or_default();
        for clazz in files {
            let Some(bin_name) = self.file_manager.infer_binary_name(&*class_location, &clazz) else {
                continue;
            };
            let bin_name = bin_name.to_string_lossy();
            let pack = match bin_name.rfind('.') {
                Some(index) => &bin_name[..index],
                None => "",
            };
            if seen_packages.insert(pack.to_string()) {
                let package = syms.enter_package(
                    ctx,
                    msym,
                    self.names.from_string(&JavaString::from(pack)),
                );
                let d = ctx.alloc_directive(Directive::Exports {
                    packge: Some(package),
                    modules: None,
                    flags: Vec::new(),
                });
                directives.push(d);
                exports.push(d);
            }
        }
        set_module_exports(ctx, msym, List::from_iterable(exports));
        set_module_provides(ctx, msym, List::nil());
        set_module_requires(ctx, msym, List::nil());
        set_module_uses(ctx, msym, List::nil());
        set_module_directives(ctx, msym, List::from_iterable(directives));
        Ok(())
    }

    /// Java: private void completeAutomaticModule(ModuleSymbol msym)
    /// (Modules.java:701-724).
    fn complete_automatic_module(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        msym: SymbolId,
    ) {
        let mut directives: Vec<DirectiveId> = module_directives(ctx, msym).iter().collect();
        let mut requires: Vec<DirectiveId> = Vec::new();
        for ms in self.all_modules() {
            if ms == syms.unnamed_module || ms == msym {
                continue;
            }
            let flags = if (ctx.sym(ms).flags_field & flags::AUTOMATIC_MODULE) != 0 {
                vec![RequiresFlag::TRANSITIVE]
            } else {
                Vec::new()
            };
            let d = ctx.alloc_directive(Directive::Requires {
                module: Some(ms),
                flags,
            });
            directives.push(d);
            requires.push(d);
        }
        let requires_unnamed = ctx.alloc_directive(Directive::Requires {
            module: Some(syms.unnamed_module),
            flags: Vec::new(),
        });
        directives.push(requires_unnamed);
        requires.push(requires_unnamed);
        set_module_requires(ctx, msym, List::from_iterable(requires));
        set_module_directives(ctx, msym, List::from_iterable(directives));
    }

    /// Java: private void checkCyclicDependencies(JCModuleDecl mod)
    /// (Modules.java:1757-1782).
    fn check_cyclic_dependencies(&self, ctx: &mut CompilerContext, syms: &mut Symtab, mod_tree: TreeId) {
        let directives = match &ctx.tree(mod_tree).kind {
            TreeKind::ModuleDecl { directives, .. } => directives.clone().unwrap_or_else(List::nil),
            _ => return,
        };
        for d in directives.iter() {
            let requires = match &ctx.tree(d).kind {
                TreeKind::Requires { directive: Some(rd), module_name, .. } => {
                    Some((*rd, *module_name))
                }
                _ => None,
            };
            let Some((rd, module_name)) = requires else { continue };
            let mut non_synthetic: Vec<SymbolId> = Vec::new();
            let start = ctx.directive(rd).get_dependency().expect("requires module");
            let mut queue: Vec<SymbolId> = vec![start];
            while let Some(current) = queue.pop() {
                if non_synthetic.contains(&current) {
                    continue;
                }
                non_synthetic.push(current);
                let _ = ctx.complete_symbol(syms, current);
                if (ctx.sym(current).flags_field & flags::AUTOMATIC_MODULE) != 0 {
                    continue;
                }
                for dep in module_requires(ctx, current).iter() {
                    let is_extra = match ctx.directive(dep) {
                        Directive::Requires { flags, .. } => flags.contains(&RequiresFlag::EXTRA),
                        _ => false,
                    };
                    if !is_extra
                        && let Some(module) = ctx.directive(dep).get_dependency()
                    {
                        queue.push(module);
                    }
                }
            }
            let mod_sym = match &ctx.tree(mod_tree).kind {
                TreeKind::ModuleDecl { sym: Some(sym), .. } => *sym,
                _ => continue,
            };
            if non_synthetic.contains(&mod_sym) {
                let pos = module_name.map(|n| ctx.tree(n).pos).unwrap_or(0);
                let error = JCDiagnosticError::from(errors::cyclic_requires(
                    SymbolArg::placeholder(
                        ctx.sym(start).name.to_java_string().to_string_lossy(),
                    ),
                ));
                self.log.borrow_mut().error_at(pos, error);
            }
        }
    }

    /// Java: private boolean isValidName(CharSequence name)
    /// (Modules.java:1784-1786).
    fn is_valid_name(&self, name: &str) -> bool {
        let Some(version) = Source::to_source_version(self.source) else {
            return false;
        };
        javac_tools::lang::model::source_version::SourceVersion::is_name_version(
            &JavaString::from(name),
            version,
        )
    }

    /// Module name of the module-oriented location (fileManager.inferModuleName).
    fn module_name(&self, ctx: &mut CompilerContext, location: &LocationRef) -> JavaString {
        let _ = ctx;
        self.file_manager
            .infer_module_name(location.as_ref())
            .unwrap_or_else(|_| JavaString::from(""))
    }
}


/// Java: the anonymous mainCompleter (Modules.java:626-672).
struct MainCompleter {
    modules: Rc<Modules>,
}

impl Completer for MainCompleter {
    fn complete(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        sym: SymbolId,
    ) -> Result<(), CompletionFailure> {
        self.modules.main_complete(ctx, syms, sym)
    }
}

/// Java: the anonymous source completer (Modules.java:726-754).
struct SourceCompleter {
    modules: Rc<Modules>,
    tree: TreeId,
    sourcefile: JavaFileObjectRef,
}

impl Completer for SourceCompleter {
    fn complete(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        sym: SymbolId,
    ) -> Result<(), CompletionFailure> {
        let modules = Rc::clone(&self.modules);
        let decl = tree_info::get_module(ctx, self.tree).expect("module decl");
        let prev_source = AbstractLog::use_source(
            &mut *modules.log.borrow_mut(),
            Some(Rc::clone(&self.sourcefile)),
        );
        let prev_lint = modules
            .deferred_lint_handler
            .borrow_mut()
            .set_pos(Rc::new(SimpleDiagnosticPosition::new(ctx.tree(decl).pos)));
        ctx.sym_mut(sym).flags_field |= flags::UNATTRIBUTED as i64;
        let result = (|| -> Result<(), CompletionFailure> {
            let mut visitor = ModuleVisitor::new(&modules, ctx, syms, sym);
            visitor.visit_module_def(decl);
            modules.complete_module(ctx, syms, sym);
            modules.check_cyclic_dependencies(ctx, syms, decl);
            Ok(())
        })();
        ctx.sym_mut(sym).flags_field &= !(flags::UNATTRIBUTED as i64);
        modules
            .deferred_lint_handler
            .borrow_mut()
            .set_pos_state(prev_lint);
        AbstractLog::use_source(&mut *modules.log.borrow_mut(), prev_source);
        result
    }
}

/// Java: the anonymous unnamedModuleCompleter (Modules.java:1433-1456).
struct UnnamedModuleCompleter {
    modules: Rc<Modules>,
}

impl Completer for UnnamedModuleCompleter {
    fn complete(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        sym: SymbolId,
    ) -> Result<(), CompletionFailure> {
        let modules = Rc::clone(&self.modules);
        if modules.in_init_modules.get() {
            set_module_completer(
                ctx,
                sym,
                Some(Rc::new(RefCell::new(UnnamedModuleCompleter {
                    modules: Rc::clone(&modules),
                }))),
            );
            return Ok(());
        }
        let mut all = modules.all_modules();
        all.retain(|m| *m != syms.unnamed_module);
        for module in all {
            let _ = ctx.complete_symbol(syms, module);
        }
        modules.init_visible_packages(ctx, syms, sym, &modules.all_modules());
        Ok(())
    }
}

impl Modules {
    /// Java: mainCompleter.complete(Symbol sym) (Modules.java:626-658).
    fn main_complete(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        sym: SymbolId,
    ) -> Result<(), CompletionFailure> {
        let msym = self.module_finder.borrow_mut().find_module(ctx, syms, sym);
        if ctx.sym(msym).kind == Kind::ERR {
            self.init_err_module(ctx, msym);
        } else if (ctx.sym(msym).flags_field & flags::AUTOMATIC_MODULE) != 0 {
            self.setup_automatic_module(ctx, syms, msym)?;
        } else {
            let module_info = module_info_symbol(ctx, msym);
            if let Err(failure) = ctx.complete_class_symbol(syms, module_info) {
                set_module_kind(ctx, msym, Kind::ERR);
                self.init_err_module(ctx, msym);
                self.complete_module(ctx, syms, msym);
                return Err(failure);
            }
        }
        let module_info = module_info_symbol(ctx, msym);
        let classfile = module_info_classfile(ctx, module_info);
        let is_class = classfile
            .as_ref()
            .map(|fo| fo.get_kind() == FileKind::CLASS)
            .unwrap_or(false);
        if classfile.is_none() || is_class {
            self.complete_module(ctx, syms, msym);
        }
        Ok(())
    }

    /// Java: mainCompleter.initErrModule(ModuleSymbol) (Modules.java:660-666).
    fn init_err_module(&self, ctx: &mut CompilerContext, msym: SymbolId) {
        set_module_directives(ctx, msym, List::nil());
        set_module_exports(ctx, msym, List::nil());
        set_module_provides(ctx, msym, List::nil());
        set_module_requires(ctx, msym, List::nil());
        set_module_uses(ctx, msym, List::nil());
    }

    /// Java: private Completer getSourceCompleter(JCCompilationUnit tree)
    /// (Modules.java:726-754).
    fn get_source_completer(&self, tree: TreeId, sourcefile: JavaFileObjectRef) -> CompleterRef {
        Rc::new(RefCell::new(SourceCompleter {
            modules: self.self_ref.upgrade().expect("Modules self"),
            tree,
            sourcefile,
        }))
    }

    /// Java: public Completer getUsesProvidesCompleter() (Modules.java:967-986).
    pub fn get_uses_provides_completer(&self) -> CompleterRef {
        Rc::new(RefCell::new(UsesProvidesCompleter {
            modules: self.self_ref.upgrade().expect("Modules self"),
        }))
    }}

/// Java: class ModuleVisitor extends JCTree.Visitor (Modules.java:766-965).
struct ModuleVisitor<'a> {
    modules: &'a Modules,
    ctx: &'a mut CompilerContext,
    syms: &'a mut Symtab,
    sym: Option<SymbolId>,
    all_requires: HashSet<SymbolId>,
    all_exports: HashMap<SymbolId, Vec<DirectiveId>>,
    all_opens: HashMap<SymbolId, Vec<DirectiveId>>,
}

impl<'a> ModuleVisitor<'a> {
    fn new(
        modules: &'a Modules,
        ctx: &'a mut CompilerContext,
        syms: &'a mut Symtab,
        _msym: SymbolId,
    ) -> ModuleVisitor<'a> {
        ModuleVisitor {
            modules,
            ctx,
            syms,
            sym: None,
            all_requires: HashSet::new(),
            all_exports: HashMap::new(),
            all_opens: HashMap::new(),
        }
    }

    fn visit_module_def(&mut self, tree: TreeId) {
        let (mods, kind, qual_id, directives) = match &self.ctx.tree(tree).kind {
            TreeKind::ModuleDecl {
                mods,
                kind,
                qual_id,
                directives,
                ..
            } => (*mods, *kind, *qual_id, directives.clone()),
            _ => panic!("AssertionError: JCModuleDecl expected"),
        };
        let sym = match &self.ctx.tree(tree).kind {
            TreeKind::ModuleDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("AssertionError: module decl symbol"),
        };
        self.sym = Some(sym);
        if kind == ModuleKind::OPEN {
            let mut module_flags = module_flags_read(self.ctx, sym);
            if !module_flags.contains(&ModuleFlags::OPEN) {
                module_flags.push(ModuleFlags::OPEN);
            }
            set_module_flags(self.ctx, sym, module_flags);
        }
        if let Some(mods) = mods
            && let TreeKind::Modifiers { flags: mod_flags, .. } = &self.ctx.tree(mods).kind
        {
            self.ctx.sym_mut(sym).flags_field |= mod_flags & (flags::DEPRECATED as i64);
        }
        let _ = qual_id;
        set_module_requires(self.ctx, sym, List::nil());
        set_module_exports(self.ctx, sym, List::nil());
        set_module_opens(self.ctx, sym, List::nil());
        if let Some(directives) = directives {
            for directive in directives.iter() {
                self.visit_directive(directive);
            }
        }
        let requires = module_requires(self.ctx, sym);
        let exports = module_exports(self.ctx, sym);
        let opens = module_opens(self.ctx, sym);
        set_module_requires(self.ctx, sym, reverse_list(&requires));
        set_module_exports(self.ctx, sym, reverse_list(&exports));
        set_module_opens(self.ctx, sym, reverse_list(&opens));
        self.ensure_java_base();
    }

    fn visit_directive(&mut self, directive: TreeId) {
        match &self.ctx.tree(directive).kind {
            TreeKind::Requires { .. } => self.visit_requires(directive),
            TreeKind::Exports { .. } => self.visit_exports(directive),
            TreeKind::Opens { .. } => self.visit_opens(directive),
            TreeKind::Provides { .. } => {}
            TreeKind::Uses { .. } => {}
            _ => {}
        }
    }

    fn visit_requires(&mut self, tree: TreeId) {
        let (module_name, is_transitive, is_static_phase) = match &self.ctx.tree(tree).kind {
            TreeKind::Requires {
                module_name,
                is_transitive,
                is_static_phase,
                ..
            } => (*module_name, *is_transitive, *is_static_phase),
            _ => return,
        };
        let sym = self.sym.expect("module sym");
        let module_name = module_name.expect("requires module name");
        let msym = self.lookup_module(module_name);
        let module_kind = self.ctx.sym(msym).kind;
        if module_kind != Kind::MDL {
            let pos = self.ctx.tree(module_name).pos;
            let error = JCDiagnosticError::from(errors::module_not_found(
                SymbolArg::placeholder(self.ctx.sym(msym).name.to_java_string().to_string_lossy()),
            ));
            self.modules.log.borrow_mut().error_at(pos, error);
            self.modules.warned_missing.borrow_mut().insert(msym);
        } else if self.all_requires.contains(&msym) {
            let pos = self.ctx.tree(module_name).pos;
            let error = JCDiagnosticError::from(errors::duplicate_requires(
                SymbolArg::placeholder(self.ctx.sym(msym).name.to_java_string().to_string_lossy()),
            ));
            self.modules.log.borrow_mut().error_at(pos, error);
        } else {
            self.all_requires.insert(msym);
            let mut directive_flags: Vec<RequiresFlag> = Vec::new();
            if is_transitive {
                if msym == self.syms.java_base && self.source_at_least_jdk10() {
                    let error = JCDiagnosticError::from(errors::modifier_not_allowed_here(self.modules.names.transitive.clone()));
                    let pos = self.ctx.tree(tree).pos;
                    self.modules.log.borrow_mut().error_at(pos, error);
                } else {
                    directive_flags.push(RequiresFlag::TRANSITIVE);
                }
            }
            if is_static_phase {
                if msym == self.syms.java_base && self.source_at_least_jdk10() {
                    let error = JCDiagnosticError::from(errors::mod_not_allowed_here(vec![crate::resources::diagnostic_contract::FlagArg::placeholder("static")]));
                    let pos = self.ctx.tree(tree).pos;
                    self.modules.log.borrow_mut().error_at(pos, error);
                } else {
                    directive_flags.push(RequiresFlag::STATIC_PHASE);
                }
            }
            let d = self.ctx.alloc_directive(Directive::Requires {
                module: Some(msym),
                flags: directive_flags,
            });
            if let TreeKind::Requires { directive, .. } = &mut self.ctx.tree_mut(tree).kind {
                *directive = Some(d);
            }
            let requires = module_requires(self.ctx, sym);
            set_module_requires(self.ctx, sym, requires.prepend(d));
        }
    }

    fn visit_exports(&mut self, tree: TreeId) {
        let (qualid, module_names) = match &self.ctx.tree(tree).kind {
            TreeKind::Exports {
                qualid,
                module_names,
                ..
            } => (*qualid, module_names.clone()),
            _ => return,
        };
        let qualid = qualid.expect("exports qualid");
        let name = tree_info::full_name(self.ctx, qualid).expect("exports package name");
        let sym = self.sym.expect("module sym");
        let package = self.syms.enter_package(self.ctx, sym, name);
        {
            let attr = self.modules.attr.borrow();
            let _ = &attr;
        }
        self.modules
            .attr
            .borrow_mut()
            .set_package_symbols(self.ctx, qualid, package);
        let exports_for_package = self
            .all_exports
            .get(&package)
            .cloned()
            .unwrap_or_default();
        for _d in &exports_for_package {
            self.report_exports_conflict(qualid, package);
        }
        let mut to_modules: Option<Vec<SymbolId>> = None;
        if let Some(module_names) = module_names {
            let mut to: Vec<SymbolId> = Vec::new();
            for n in module_names.iter() {
                let msym = self.lookup_module(n);
                {
                    let chk = self.modules.chk.borrow();
                    chk.check_module_exists(self.ctx, self.ctx.tree(n).pos, msym);
                }
                for d in &exports_for_package {
                    self.check_duplicate_exports_to_module(n, msym, *d);
                }
                if to.contains(&msym) {
                    self.report_exports_conflict_to_module(n, msym);
                } else {
                    to.push(msym);
                }
            }
            to_modules = Some(to);
        }
        if to_modules.as_ref().map(|to| !to.is_empty()).unwrap_or(true) {
            let d = self.ctx.alloc_directive(Directive::Exports {
                packge: Some(package),
                modules: to_modules.map(List::from_iterable),
                flags: Vec::new(),
            });
            let exports = module_exports(self.ctx, sym);
            set_module_exports(self.ctx, sym, exports.prepend(d));
            if let TreeKind::Exports { directive, .. } = &mut self.ctx.tree_mut(tree).kind {
                *directive = Some(d);
            }
            self.all_exports.entry(package).or_default().push(d);
        }
    }

    fn visit_opens(&mut self, tree: TreeId) {
        let (qualid, module_names) = match &self.ctx.tree(tree).kind {
            TreeKind::Opens {
                qualid,
                module_names,
                ..
            } => (*qualid, module_names.clone()),
            _ => return,
        };
        let qualid = qualid.expect("opens qualid");
        let name = tree_info::full_name(self.ctx, qualid).expect("opens package name");
        let sym = self.sym.expect("module sym");
        let package = self.syms.enter_package(self.ctx, sym, name);
        self.modules
            .attr
            .borrow_mut()
            .set_package_symbols(self.ctx, qualid, package);
        if module_flags_read(self.ctx, sym).contains(&ModuleFlags::OPEN) {
            let pos = self.ctx.tree(tree).pos;
            let error = JCDiagnosticError::from(errors::no_opens_unless_strong());
            self.modules.log.borrow_mut().error_at(pos, error);
        }
        let opens_for_package = self.all_opens.get(&package).cloned().unwrap_or_default();
        for _d in &opens_for_package {
            self.report_opens_conflict(qualid, package);
        }
        let mut to_modules: Option<Vec<SymbolId>> = None;
        if let Some(module_names) = module_names {
            let mut to: Vec<SymbolId> = Vec::new();
            for n in module_names.iter() {
                let msym = self.lookup_module(n);
                {
                    let chk = self.modules.chk.borrow();
                    chk.check_module_exists(self.ctx, self.ctx.tree(n).pos, msym);
                }
                for d in &opens_for_package {
                    self.check_duplicate_opens_to_module(n, msym, *d);
                }
                if to.contains(&msym) {
                    self.report_opens_conflict_to_module(n, msym);
                } else {
                    to.push(msym);
                }
            }
            to_modules = Some(to);
        }
        if to_modules.as_ref().map(|to| !to.is_empty()).unwrap_or(true) {
            let d = self.ctx.alloc_directive(Directive::Opens {
                packge: Some(package),
                modules: to_modules.map(List::from_iterable),
                flags: Vec::new(),
            });
            let opens = module_opens(self.ctx, sym);
            set_module_opens(self.ctx, sym, opens.prepend(d));
            if let TreeKind::Opens { directive, .. } = &mut self.ctx.tree_mut(tree).kind {
                *directive = Some(d);
            }
            self.all_opens.entry(package).or_default().push(d);
        }
    }

    fn report_exports_conflict(&mut self, qualid: TreeId, package: SymbolId) {
        let pos = self.ctx.tree(qualid).pos;
        let error = JCDiagnosticError::from(errors::conflicting_exports(
            SymbolArg::placeholder(self.ctx.sym(package).name.to_java_string().to_string_lossy()),
        ));
        self.modules.log.borrow_mut().error_at(pos, error);
    }

    fn check_duplicate_exports_to_module(&mut self, name: TreeId, msym: SymbolId, d: DirectiveId) {
        if let Directive::Exports { modules: Some(modules), .. } = self.ctx.directive(d) {
            for other in modules.iter() {
                if other == msym {
                    self.report_exports_conflict_to_module(name, msym);
                }
            }
        }
    }

    fn report_exports_conflict_to_module(&mut self, name: TreeId, msym: SymbolId) {
        let pos = self.ctx.tree(name).pos;
        let error = JCDiagnosticError::from(errors::conflicting_exports_to_module(
            SymbolArg::placeholder(self.ctx.sym(msym).name.to_java_string().to_string_lossy()),
        ));
        self.modules.log.borrow_mut().error_at(pos, error);
    }

    fn report_opens_conflict(&mut self, qualid: TreeId, package: SymbolId) {
        let pos = self.ctx.tree(qualid).pos;
        let error = JCDiagnosticError::from(errors::conflicting_opens(
            SymbolArg::placeholder(self.ctx.sym(package).name.to_java_string().to_string_lossy()),
        ));
        self.modules.log.borrow_mut().error_at(pos, error);
    }

    fn check_duplicate_opens_to_module(&mut self, name: TreeId, msym: SymbolId, d: DirectiveId) {
        if let Directive::Opens { modules: Some(modules), .. } = self.ctx.directive(d) {
            for other in modules.iter() {
                if other == msym {
                    self.report_opens_conflict_to_module(name, msym);
                }
            }
        }
    }

    fn report_opens_conflict_to_module(&mut self, name: TreeId, msym: SymbolId) {
        let pos = self.ctx.tree(name).pos;
        let error = JCDiagnosticError::from(errors::conflicting_opens_to_module(
            SymbolArg::placeholder(self.ctx.sym(msym).name.to_java_string().to_string_lossy()),
        ));
        self.modules.log.borrow_mut().error_at(pos, error);
    }

    fn ensure_java_base(&mut self) {
        let sym = self.sym.expect("module sym");
        if self.ctx.sym(sym).name.to_java_string()
            == self.modules.names.java_base.to_java_string()
        {
            return;
        }
        let requires = module_requires(self.ctx, sym);
        for d in requires.iter() {
            if let Some(module) = self.ctx.directive(d).get_dependency()
                && self.ctx.sym(module).name.to_java_string()
                    == self.modules.names.java_base.to_java_string()
            {
                return;
            }
        }
        let java_base = self
            .syms
            .enter_module(self.ctx, self.modules.names.java_base.clone());
        let d = self.ctx.alloc_directive(Directive::Requires {
            module: Some(java_base),
            flags: vec![RequiresFlag::MANDATED],
        });
        let requires = module_requires(self.ctx, sym);
        set_module_requires(self.ctx, sym, requires.prepend(d));
    }

    fn lookup_module(&mut self, module_name: TreeId) -> SymbolId {
        let name = tree_info::full_name(self.ctx, module_name).expect("module name");
        let msym = self
            .modules
            .module_finder
            .borrow_mut()
            .find_module_by_name(self.ctx, self.syms, &name);
        tree_info::set_symbol(self.ctx, module_name, msym);
        msym
    }

    fn source_at_least_jdk10(&self) -> bool {
        (self.modules.source as i32) >= (Source::JDK10 as i32)
    }
}

// ---------------------------------------------------------------------------
// ModuleSymbol field helpers (Java public fields).
// ---------------------------------------------------------------------------

fn module_info_symbol(ctx: &CompilerContext, msym: SymbolId) -> SymbolId {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { module_info, .. } => module_info.expect("module-info symbol"),
        _ => panic!("AssertionError: module expected"),
    }
}

fn module_info_sourcefile(ctx: &CompilerContext, module_info: SymbolId) -> Option<JavaFileObjectRef> {
    match &ctx.sym(module_info).kind_data {
        SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile.clone(),
        _ => None,
    }
}

fn module_info_classfile(ctx: &CompilerContext, module_info: SymbolId) -> Option<JavaFileObjectRef> {
    match &ctx.sym(module_info).kind_data {
        SymbolKind::ClassSymbol { classfile, .. } => classfile.clone(),
        _ => None,
    }
}

fn set_module_info_sourcefile(
    ctx: &mut CompilerContext,
    msym: SymbolId,
    sourcefile: Option<JavaFileObjectRef>,
) {
    let module_info = module_info_symbol(ctx, msym);
    if let SymbolKind::ClassSymbol { sourcefile: slot, .. } = &mut ctx.sym_mut(module_info).kind_data {
        *slot = sourcefile;
    }
}

fn same_file(a: Option<JavaFileObjectRef>, b: &JavaFileObjectRef) -> bool {
    match a {
        Some(a) => Rc::ptr_eq(&a, b) || a.get_name().to_string_lossy() == b.get_name().to_string_lossy(),
        None => false,
    }
}

fn compilation_unit_sourcefile(ctx: &CompilerContext, tree: TreeId) -> Option<JavaFileObjectRef> {
    match &ctx.tree(tree).kind {
        TreeKind::CompilationUnit { sourcefile, .. } => sourcefile.clone(),
        _ => None,
    }
}

fn compilation_unit_package(ctx: &CompilerContext, tree: TreeId) -> Option<SymbolId> {
    match &ctx.tree(tree).kind {
        TreeKind::CompilationUnit { packge, .. } => *packge,
        _ => None,
    }
}

fn set_compilation_unit_module(ctx: &mut CompilerContext, tree: TreeId, module: SymbolId) {
    if let TreeKind::CompilationUnit { modle, .. } = &mut ctx.tree_mut(tree).kind {
        *modle = Some(module);
    }
}

fn module_of_package(ctx: &CompilerContext, package: SymbolId) -> Option<SymbolId> {
    match &ctx.sym(package).kind_data {
        SymbolKind::PackageSymbol { modle, .. } => *modle,
        _ => None,
    }
}

fn set_package_module(ctx: &mut CompilerContext, package: SymbolId, module: SymbolId) {
    if let SymbolKind::PackageSymbol { modle, .. } = &mut ctx.sym_mut(package).kind_data {
        *modle = Some(module);
    }
}

fn module_kind_data<F, T>(ctx: &CompilerContext, msym: SymbolId, f: F) -> T
where
    F: FnOnce(&SymbolKind) -> T,
{
    f(&ctx.sym(msym).kind_data)
}

fn module_source_location(ctx: &CompilerContext, msym: SymbolId) -> Option<LocationRef> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { source_location, .. } => source_location.clone(),
        _ => None,
    }
}

fn module_class_location(ctx: &CompilerContext, msym: SymbolId) -> Option<LocationRef> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { class_location, .. } => class_location.clone(),
        _ => None,
    }
}

fn module_patch_location(ctx: &CompilerContext, msym: SymbolId) -> Option<LocationRef> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { patch_location, .. } => patch_location.clone(),
        _ => None,
    }
}

fn module_patch_output_location(ctx: &CompilerContext, msym: SymbolId) -> Option<LocationRef> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { patch_output_location, .. } => patch_output_location.clone(),
        _ => None,
    }
}

fn set_module_source_location(ctx: &mut CompilerContext, msym: SymbolId, location: Option<LocationRef>) {
    if let SymbolKind::ModuleSymbol { source_location, .. } = &mut ctx.sym_mut(msym).kind_data {
        *source_location = location;
    }
}

fn set_module_class_location(ctx: &mut CompilerContext, msym: SymbolId, location: Option<LocationRef>) {
    if let SymbolKind::ModuleSymbol { class_location, .. } = &mut ctx.sym_mut(msym).kind_data {
        *class_location = location;
    }
}

fn set_module_patch_location(ctx: &mut CompilerContext, msym: SymbolId, location: Option<LocationRef>) {
    if let SymbolKind::ModuleSymbol { patch_location, .. } = &mut ctx.sym_mut(msym).kind_data {
        *patch_location = location;
    }
}

fn set_module_patch_output_location(
    ctx: &mut CompilerContext,
    msym: SymbolId,
    location: Option<LocationRef>,
) {
    if let SymbolKind::ModuleSymbol { patch_output_location, .. } = &mut ctx.sym_mut(msym).kind_data {
        *patch_output_location = location;
    }
}

fn module_source_location_is_source_path(ctx: &CompilerContext, msym: SymbolId) -> bool {
    module_source_location(ctx, msym)
        .map(|loc| loc.get_name().to_string_lossy() == StandardLocation::SOURCE_PATH.get_name().to_string_lossy())
        .unwrap_or(false)
}

fn module_requires(ctx: &CompilerContext, msym: SymbolId) -> List<DirectiveId> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { requires, .. } => requires.clone(),
        _ => List::nil(),
    }
}

fn set_module_requires(ctx: &mut CompilerContext, msym: SymbolId, list: List<DirectiveId>) {
    if let SymbolKind::ModuleSymbol { requires, .. } = &mut ctx.sym_mut(msym).kind_data {
        *requires = list;
    }
}

fn module_exports(ctx: &CompilerContext, msym: SymbolId) -> List<DirectiveId> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { exports, .. } => exports.clone(),
        _ => List::nil(),
    }
}

fn set_module_exports(ctx: &mut CompilerContext, msym: SymbolId, list: List<DirectiveId>) {
    if let SymbolKind::ModuleSymbol { exports, .. } = &mut ctx.sym_mut(msym).kind_data {
        *exports = list;
    }
}

fn module_opens(ctx: &CompilerContext, msym: SymbolId) -> List<DirectiveId> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { opens, .. } => opens.clone(),
        _ => List::nil(),
    }
}

fn set_module_opens(ctx: &mut CompilerContext, msym: SymbolId, list: List<DirectiveId>) {
    if let SymbolKind::ModuleSymbol { opens, .. } = &mut ctx.sym_mut(msym).kind_data {
        *opens = list;
    }
}

fn module_provides(ctx: &CompilerContext, msym: SymbolId) -> List<DirectiveId> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { provides, .. } => provides.clone(),
        _ => List::nil(),
    }
}

fn set_module_provides(ctx: &mut CompilerContext, msym: SymbolId, list: List<DirectiveId>) {
    if let SymbolKind::ModuleSymbol { provides, .. } = &mut ctx.sym_mut(msym).kind_data {
        *provides = list;
    }
}

fn module_uses(ctx: &CompilerContext, msym: SymbolId) -> List<DirectiveId> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { uses, .. } => uses.clone(),
        _ => List::nil(),
    }
}

fn set_module_uses(ctx: &mut CompilerContext, msym: SymbolId, list: List<DirectiveId>) {
    if let SymbolKind::ModuleSymbol { uses, .. } = &mut ctx.sym_mut(msym).kind_data {
        *uses = list;
    }
}

fn module_directives(ctx: &CompilerContext, msym: SymbolId) -> List<DirectiveId> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { directives, .. } => directives.clone(),
        _ => List::nil(),
    }
}

fn set_module_directives(ctx: &mut CompilerContext, msym: SymbolId, list: List<DirectiveId>) {
    if let SymbolKind::ModuleSymbol { directives, .. } = &mut ctx.sym_mut(msym).kind_data {
        *directives = list;
    }
}

fn module_flags_read(ctx: &CompilerContext, msym: SymbolId) -> Vec<ModuleFlags> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { flags, .. } => flags.clone(),
        _ => Vec::new(),
    }
}

fn set_module_flags(ctx: &mut CompilerContext, msym: SymbolId, flags: Vec<ModuleFlags>) {
    if let SymbolKind::ModuleSymbol { flags: slot, .. } = &mut ctx.sym_mut(msym).kind_data {
        *slot = flags;
    }
}

fn module_visible_packages(ctx: &CompilerContext, msym: SymbolId) -> HashMap<Name, SymbolId> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { visible_packages, .. } => visible_packages.clone(),
        _ => HashMap::new(),
    }
}

fn set_module_visible_packages(
    ctx: &mut CompilerContext,
    msym: SymbolId,
    packages: HashMap<Name, SymbolId>,
) {
    if let SymbolKind::ModuleSymbol { visible_packages, .. } = &mut ctx.sym_mut(msym).kind_data {
        *visible_packages = packages;
    }
}

fn module_read_modules(ctx: &CompilerContext, msym: SymbolId) -> Vec<SymbolId> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { read_modules, .. } => read_modules.clone(),
        _ => Vec::new(),
    }
}

fn set_module_read_modules(ctx: &mut CompilerContext, msym: SymbolId, modules: Vec<SymbolId>) {
    if let SymbolKind::ModuleSymbol { read_modules, .. } = &mut ctx.sym_mut(msym).kind_data {
        *read_modules = modules;
    }
}

fn module_resolution_flags(ctx: &CompilerContext, msym: SymbolId) -> Vec<ModuleResolutionFlags> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { resolution_flags, .. } => resolution_flags.clone(),
        _ => Vec::new(),
    }
}

fn set_module_kind(ctx: &mut CompilerContext, msym: SymbolId, kind: Kind) {
    ctx.sym_mut(msym).kind = kind;
}

fn set_module_completer(ctx: &mut CompilerContext, msym: SymbolId, completer: Option<CompleterRef>) {
    ctx.sym_mut(msym).completer = completer;
}

fn is_unnamed_module(ctx: &CompilerContext, msym: SymbolId) -> bool {
    ctx.sym(msym).name.is_empty()
}

fn reverse_list(list: &List<DirectiveId>) -> List<DirectiveId> {
    let mut result = List::nil();
    for item in list.iter() {
        result = result.prepend(item);
    }
    result
}


/// Java: the anonymous usesProvidesCompleter (Modules.java:967-986).
struct UsesProvidesCompleter {
    modules: Rc<Modules>,
}

impl Completer for UsesProvidesCompleter {
    fn complete(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        sym: SymbolId,
    ) -> Result<(), CompletionFailure> {
        let modules = Rc::clone(&self.modules);
        ctx.complete_symbol(syms, sym)?;
        let env = modules
            .type_envs
            .borrow()
            .get(sym)
            .expect("AssertionError: typeEnvs entry for module");
        let toplevel = env.toplevel.expect("AssertionError: module env toplevel");
        let decl = tree_info::get_module(ctx, toplevel).expect("AssertionError: moduleDecl");
        let source = compilation_unit_sourcefile(ctx, toplevel);
        let prev_source = match source {
            Some(source) => {
                AbstractLog::use_source(&mut *modules.log.borrow_mut(), Some(source))
            }
            None => None,
        };
        let prev_lint = modules
            .deferred_lint_handler
            .borrow_mut()
            .set_pos(Rc::new(SimpleDiagnosticPosition::new(ctx.tree(decl).pos)));
        let mut visitor = UsesProvidesVisitor::new(&modules, ctx, syms, sym, env.clone());
        visitor.visit_module_def(decl);
        modules
            .deferred_lint_handler
            .borrow_mut()
            .set_pos_state(prev_lint);
        AbstractLog::use_source(&mut *modules.log.borrow_mut(), prev_source);
        Ok(())
    }
}

/// Java: class UsesProvidesVisitor extends JCTree.Visitor
/// (Modules.java:988-1211).
struct UsesProvidesVisitor<'a> {
    modules: &'a Modules,
    ctx: &'a mut CompilerContext,
    syms: &'a mut Symtab,
    msym: SymbolId,
    env: Rc<Env<AttrContext>>,
    all_uses: HashSet<SymbolId>,
    all_provides: HashMap<SymbolId, HashSet<SymbolId>>,
    directive_to_tree: Vec<(DirectiveId, TreeId)>,
}

impl<'a> UsesProvidesVisitor<'a> {
    fn new(
        modules: &'a Modules,
        ctx: &'a mut CompilerContext,
        syms: &'a mut Symtab,
        msym: SymbolId,
        env: Rc<Env<AttrContext>>,
    ) -> UsesProvidesVisitor<'a> {
        UsesProvidesVisitor {
            modules,
            ctx,
            syms,
            msym,
            env,
            all_uses: HashSet::new(),
            all_provides: HashMap::new(),
            directive_to_tree: Vec::new(),
        }
    }

    fn visit_module_def(&mut self, tree: TreeId) {
        set_module_directives(self.ctx, self.msym, List::nil());
        set_module_provides(self.ctx, self.msym, List::nil());
        set_module_uses(self.ctx, self.msym, List::nil());
        let directives = match &self.ctx.tree(tree).kind {
            TreeKind::ModuleDecl { directives, .. } => directives.clone().unwrap_or_else(List::nil),
            _ => List::nil(),
        };
        for directive in directives.iter() {
            match &self.ctx.tree(directive).kind {
                TreeKind::Exports { .. } => self.visit_exports(directive),
                TreeKind::Opens { .. } => self.visit_opens(directive),
                TreeKind::Provides { .. } => self.visit_provides(directive),
                TreeKind::Requires { .. } => self.visit_requires(directive),
                TreeKind::Uses { .. } => self.visit_uses(directive),
                _ => {}
            }
        }
        let directives = module_directives(self.ctx, self.msym);
        set_module_directives(self.ctx, self.msym, reverse_list(&directives));
        let provides = module_provides(self.ctx, self.msym);
        set_module_provides(self.ctx, self.msym, reverse_list(&provides));
        let uses = module_uses(self.ctx, self.msym);
        set_module_uses(self.ctx, self.msym, reverse_list(&uses));
        let requires = module_requires(self.ctx, self.msym);
        if requires.non_empty() {
            let head = *requires.head();
            if directive_requires_flags(self.ctx, head).contains(&RequiresFlag::MANDATED) {
                let directives = module_directives(self.ctx, self.msym);
                set_module_directives(self.ctx, self.msym, directives.prepend(head));
            }
        }
        let add_reads = self
            .modules
            .add_reads
            .borrow()
            .clone()
            .unwrap_or_default()
            .get(&self.msym)
            .cloned()
            .unwrap_or_default();
        let directives = module_directives(self.ctx, self.msym);
        let mut appended: Vec<DirectiveId> = directives.iter().collect();
        appended.extend(add_reads);
        set_module_directives(self.ctx, self.msym, List::from_iterable(appended));
        self.check_for_correctness();
    }

    fn visit_exports(&mut self, tree: TreeId) {
        let (qualid, directive) = match &self.ctx.tree(tree).kind {
            TreeKind::Exports { qualid, directive, .. } => (*qualid, *directive),
            _ => return,
        };
        let (Some(qualid), Some(directive)) = (qualid, directive) else {
            return;
        };
        let packge = match self.ctx.directive(directive) {
            Directive::Exports { packge: Some(packge), .. } => *packge,
            _ => return,
        };
        let members = package_members_scope(self.ctx, packge);
        let mut files_to_check: Vec<JavaFileObjectRef> = Vec::new();
        let mut package_not_empty = false;
        if let Some(members) = members {
            let symbols = self.ctx.scope_get_symbols(
                members,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            );
            for csym in symbols {
                if self.ctx.sym(csym).kind != Kind::TYP {
                    continue;
                }
                let classfile = classfile_of(self.ctx, csym);
                let terminal = completer_terminal(self.ctx, csym);
                let kind = classfile.as_ref().map(|f| f.get_kind());
                if terminal || kind == Some(FileKind::CLASS) {
                    package_not_empty = true;
                    files_to_check.clear();
                    break;
                }
                if kind == Some(FileKind::SOURCE)
                    && let Some(classfile) = classfile
                {
                    files_to_check.push(classfile);
                }
            }
        }
        for jfo in files_to_check {
            let finder = self
                .modules
                .find_package_in_file
                .borrow()
                .clone()
                .expect("AssertionError: findPackageInFile");
            if finder.find_package_name_of(jfo.as_ref()) == Some(package_fullname(self.ctx, packge)) {
                package_not_empty = true;
                break;
            }
        }
        if !package_not_empty {
            let pos = self.ctx.tree(qualid).pos;
            let name = self
                .ctx
                .sym(packge)
                .name
                .to_java_string()
                .to_string_lossy();
            let error = JCDiagnosticError::from(
                crate::resources::compiler_properties::errors::package_empty_or_not_found(
                    SymbolArg::placeholder(name),
                ),
            );
            self.modules.log.borrow_mut().error_at(pos, error);
        }
        let directives = module_directives(self.ctx, self.msym);
        set_module_directives(self.ctx, self.msym, directives.prepend(directive));
    }

    fn visit_opens(&mut self, tree: TreeId) {
        let (qualid, directive) = match &self.ctx.tree(tree).kind {
            TreeKind::Opens { qualid, directive, .. } => (*qualid, *directive),
            _ => return,
        };
        let (Some(qualid), Some(directive)) = (qualid, directive) else {
            return;
        };
        let packge = match self.ctx.directive(directive) {
            Directive::Opens { packge: Some(packge), .. } => *packge,
            _ => return,
        };
        let pos = self.ctx.tree(qualid).pos;
        {
            let chk = self.modules.chk.borrow();
            chk.check_package_exists_for_opens(self.ctx, pos, packge);
        }
        let directives = module_directives(self.ctx, self.msym);
        set_module_directives(self.ctx, self.msym, directives.prepend(directive));
    }

    fn no_args_constructor(&mut self, tsym: SymbolId) -> Option<SymbolId> {
        let members = class_members_scope(self.ctx, tsym)?;
        let name = self.modules.names.init.clone();
        let candidates = self.ctx.scope_get_symbols_by_name(
            members,
            &name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        for sym in candidates {
            if self.ctx.sym(sym).kind == Kind::MTH && method_params_of(self.ctx, sym).is_empty() {
                return Some(sym);
            }
        }
        None
    }

    fn factory_method(&mut self, tsym: SymbolId) -> Option<SymbolId> {
        let members = class_members_scope(self.ctx, tsym)?;
        let name = self.modules.names.provider.clone();
        let candidates = self.ctx.scope_get_symbols_by_name(
            members,
            &name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        for sym in candidates {
            if self.ctx.sym(sym).kind == Kind::MTH
                && ctx_symbol_is_static(self.ctx, sym)
                && (self.ctx.sym(sym).flags_field & (crate::code::flags::PUBLIC as i64)) != 0
                && method_params_of(self.ctx, sym).is_empty()
            {
                return Some(sym);
            }
        }
        None
    }

    fn visit_provides(&mut self, tree: TreeId) {
        let (service_name, impl_names) = match &self.ctx.tree(tree).kind {
            TreeKind::Provides { service_name, impl_names } => (*service_name, impl_names.clone()),
            _ => return,
        };
        let Some(service_name) = service_name else { return };
        let st = {
            let syms = &mut *self.syms;
            self.modules
                .attr
                .borrow_mut()
                .attrib_type_with(self.ctx, syms, service_name, &self.env, Some(syms.object_type))
        };
        let service = self.ctx.typ(st).tsym.expect("AssertionError: service tsym");
        if self.all_provides.contains_key(&service) {
            let pos = self.ctx.tree(service_name).pos;
            let name = self.ctx.sym(service).name.to_java_string().to_string_lossy();
            let error = JCDiagnosticError::from(
                crate::resources::compiler_properties::errors::repeated_provides_for_service(
                    SymbolArg::placeholder(name),
                ),
            );
            self.modules.log.borrow_mut().error_at(pos, error);
        }
        let mut impls: Vec<SymbolId> = Vec::new();
        for impl_name in impl_names.iter() {
            let it = {
                let prev = {
                    let mut info = self.env.info.borrow_mut();
                    let prev = info.visiting_service_implementation;
                    info.visiting_service_implementation = true;
                    prev
                };
                let result = {
                    let syms = &mut *self.syms;
                    self.modules.attr.borrow_mut().attrib_type_with(
                        self.ctx,
                        syms,
                        impl_name,
                        &self.env,
                        Some(syms.object_type),
                    )
                };
                self.env.info.borrow_mut().visiting_service_implementation = prev;
                result
            };
            let impl_sym = self.ctx.typ(it).tsym.expect("AssertionError: impl tsym");
            if (self.ctx.sym(impl_sym).flags_field & (crate::code::flags::PUBLIC as i64)) == 0 {
                let pos = self.ctx.tree(impl_name).pos;
                let name = self
                    .ctx
                    .sym(impl_sym)
                    .name
                    .to_java_string()
                    .to_string_lossy();
                let location = self.ctx.sym(impl_sym).location().map(|loc| {
                    self.ctx.sym(loc).name.to_java_string().to_string_lossy()
                });
                let error = JCDiagnosticError::from(
                    crate::resources::compiler_properties::errors::not_def_public(
                        SymbolArg::placeholder(name),
                        SymbolArg::placeholder(location.unwrap_or_default()),
                    ),
                );
                self.modules.log.borrow_mut().error_at(pos, error);
            }
            let factory = self.factory_method(impl_sym);
            if let Some(factory) = factory {
                let factory_ty = self.ctx.sym(factory).ty.expect("factory type");
                let return_type = self.ctx.typ(factory_ty).get_return_type(self.ctx);
                if let Some(return_type) = return_type
                    && !self.modules.types.is_subtype(self.ctx, self.syms, return_type, st)
                {
                    let pos = self.ctx.tree(impl_name).pos;
                    let error = JCDiagnosticError::from(
                        crate::resources::compiler_properties::errors::
                            service_implementation_provider_return_must_be_subtype_of_service_interface(
                            ),
                    );
                    self.modules.log.borrow_mut().error_at(pos, error);
                }
            } else if !self.modules.types.is_subtype(self.ctx, self.syms, it, st) {
                let pos = self.ctx.tree(impl_name).pos;
                let error = JCDiagnosticError::from(
                    crate::resources::compiler_properties::errors::
                        service_implementation_must_be_subtype_of_service_interface(),
                );
                self.modules.log.borrow_mut().error_at(pos, error);
            } else if (self.ctx.sym(impl_sym).flags_field & (crate::code::flags::ABSTRACT as i64)) != 0 {
                let pos = self.ctx.tree(impl_name).pos;
                let name = self
                    .ctx
                    .sym(impl_sym)
                    .name
                    .to_java_string()
                    .to_string_lossy();
                let error = JCDiagnosticError::from(
                    crate::resources::compiler_properties::errors::service_implementation_is_abstract(
                        SymbolArg::placeholder(name),
                    ),
                );
                self.modules.log.borrow_mut().error_at(pos, error);
            } else if is_inner_symbol(self.ctx, impl_sym) {
                let pos = self.ctx.tree(impl_name).pos;
                let name = self
                    .ctx
                    .sym(impl_sym)
                    .name
                    .to_java_string()
                    .to_string_lossy();
                let error = JCDiagnosticError::from(
                    crate::resources::compiler_properties::errors::service_implementation_is_inner(
                        SymbolArg::placeholder(name),
                    ),
                );
                self.modules.log.borrow_mut().error_at(pos, error);
            } else {
                let constructor = self.no_args_constructor(impl_sym);
                match constructor {
                    None => {
                        let pos = self.ctx.tree(impl_name).pos;
                        let name = self
                            .ctx
                            .sym(impl_sym)
                            .name
                            .to_java_string()
                            .to_string_lossy();
                        let error = JCDiagnosticError::from(
                            crate::resources::compiler_properties::errors::
                                service_implementation_doesnt_have_a_no_args_constructor(
                                    SymbolArg::placeholder(name),
                                ),
                        );
                        self.modules.log.borrow_mut().error_at(pos, error);
                    }
                    Some(constructor) => {
                        if (self.ctx.sym(constructor).flags_field
                            & (crate::code::flags::PUBLIC as i64))
                            == 0
                        {
                            let pos = self.ctx.tree(impl_name).pos;
                            let name = self
                                .ctx
                                .sym(impl_sym)
                                .name
                                .to_java_string()
                                .to_string_lossy();
                            let error = JCDiagnosticError::from(
                                crate::resources::compiler_properties::errors::
                                    service_implementation_no_args_constructor_not_public(
                                        SymbolArg::placeholder(name),
                                    ),
                            );
                            self.modules.log.borrow_mut().error_at(pos, error);
                        }
                    }
                }
            }
            if self.ctx.typ(it).has_tag(TypeTag::CLASS) {
                let set = self
                    .all_provides
                    .entry(service)
                    .or_default();
                if set.insert(impl_sym) {
                    impls.push(impl_sym);
                } else {
                    let pos = self.ctx.tree(impl_name).pos;
                    let service_name = self
                        .ctx
                        .sym(service)
                        .name
                        .to_java_string()
                        .to_string_lossy();
                    let impl_name = self
                        .ctx
                        .sym(impl_sym)
                        .name
                        .to_java_string()
                        .to_string_lossy();
                    let error = JCDiagnosticError::from(
                        crate::resources::compiler_properties::errors::duplicate_provides(
                            SymbolArg::placeholder(service_name),
                            SymbolArg::placeholder(impl_name),
                        ),
                    );
                    self.modules.log.borrow_mut().error_at(pos, error);
                }
            }
        }
        if self.ctx.typ(st).has_tag(TypeTag::CLASS) && !impls.is_empty() {
            let directive = self.ctx.alloc_directive(Directive::Provides {
                service: Some(service),
                impls: List::from_iterable(impls),
            });
            let provides = module_provides(self.ctx, self.msym);
            set_module_provides(self.ctx, self.msym, provides.prepend(directive));
            let directives = module_directives(self.ctx, self.msym);
            set_module_directives(self.ctx, self.msym, directives.prepend(directive));
            self.directive_to_tree.push((directive, tree));
        }
    }

    fn visit_requires(&mut self, tree: TreeId) {
        let directive = match &self.ctx.tree(tree).kind {
            TreeKind::Requires {
                directive: Some(directive),
                module_name,
                ..
            } => (*directive, *module_name),
            _ => return,
        };
        let (directive, module_name) = directive;
        let Some(module) = directive_requires_module(self.ctx, directive) else {
            return;
        };
        if self.modules.all_modules().contains(&module) {
            let pos = module_name.map(|n| self.ctx.tree(n).pos).unwrap_or(0);
            {
                let chk = self.modules.chk.borrow();
                chk.check_deprecated(self.ctx, pos, self.msym, module);
                chk.check_preview(self.ctx, pos, self.msym, module);
                chk.check_module_requires(self.ctx, pos, directive);
            }
            let directives = module_directives(self.ctx, self.msym);
            set_module_directives(self.ctx, self.msym, directives.prepend(directive));
        }
    }

    fn visit_uses(&mut self, tree: TreeId) {
        let qualid = match &self.ctx.tree(tree).kind {
            TreeKind::Uses { qualid } => *qualid,
            _ => return,
        };
        let Some(qualid) = qualid else { return };
        let st = {
            let syms = &mut *self.syms;
            self.modules
                .attr
                .borrow_mut()
                .attrib_type_with(self.ctx, syms, qualid, &self.env, Some(syms.object_type))
        };
        let sym = tree_info::symbol(self.ctx, qualid).expect("AssertionError: uses symbol");
        if (self.ctx.sym(sym).flags_field & (crate::code::flags::ENUM as i64)) != 0 {
            let pos = self.ctx.tree(qualid).pos;
            let name = self.ctx.sym(sym).name.to_java_string().to_string_lossy();
            let error = JCDiagnosticError::from(
                crate::resources::compiler_properties::errors::service_definition_is_enum(
                    SymbolArg::placeholder(name),
                ),
            );
            self.modules.log.borrow_mut().error_at(pos, error);
        } else if self.ctx.typ(st).has_tag(TypeTag::CLASS) {
            let service = self.ctx.typ(st).tsym.expect("AssertionError: uses tsym");
            if self.all_uses.insert(service) {
                let directive = self.ctx.alloc_directive(Directive::Uses {
                    service: Some(service),
                });
                let uses = module_uses(self.ctx, self.msym);
                set_module_uses(self.ctx, self.msym, uses.prepend(directive));
                let directives = module_directives(self.ctx, self.msym);
                set_module_directives(self.ctx, self.msym, directives.prepend(directive));
            } else {
                let pos = self.ctx.tree(tree).pos;
                let name = self
                    .ctx
                    .sym(service)
                    .name
                    .to_java_string()
                    .to_string_lossy();
                let error = JCDiagnosticError::from(
                    crate::resources::compiler_properties::errors::duplicate_uses(
                        SymbolArg::placeholder(name),
                    ),
                );
                self.modules.log.borrow_mut().error_at(pos, error);
            }
        }
    }

    fn check_for_correctness(&mut self) {
        let visible_packages = module_visible_packages(self.ctx, self.msym);
        let provides = module_provides(self.ctx, self.msym);
        for directive in provides.iter() {
            let (service, impls) = match self.ctx.directive(directive) {
                Directive::Provides {
                    service: Some(service),
                    impls,
                } => (*service, impls.clone()),
                _ => continue,
            };
            let Some(tree) = self
                .directive_to_tree
                .iter()
                .find(|(d, _)| *d == directive)
                .map(|(_, t)| *t)
            else {
                continue;
            };
            for impl_sym in impls.iter() {
                let impl_package = crate::code::symbol::symbol_packge(self.ctx, impl_sym)
                    .expect("AssertionError: impl package");
                let impl_module = module_of_package(self.ctx, impl_package);
                if impl_module != Some(self.msym) {
                    let pos = self.ctx.tree(tree).pos;
                    let name = impl_module
                        .map(|m| {
                            self.ctx
                                .sym(m)
                                .name
                                .to_java_string()
                                .to_string_lossy()
                        })
                        .unwrap_or_default();
                    let error = JCDiagnosticError::from(
                        crate::resources::compiler_properties::errors::
                            service_implementation_not_in_right_module(
                                SymbolArg::placeholder(name),
                            ),
                    );
                    self.modules.log.borrow_mut().error_at(pos, error);
                }
                let interface_package = crate::code::symbol::symbol_packge(self.ctx, service)
                    .expect("AssertionError: service package");
                let is_interface_declared_in_current_module =
                    module_of_package(self.ctx, interface_package) == Some(self.msym);
                let interface_fullname = package_fullname(self.ctx, interface_package);
                let is_interface_exported_from_readable_module = visible_packages
                    .get(&interface_fullname)
                    .map(|p| *p == interface_package)
                    .unwrap_or(false);
                if !is_interface_declared_in_current_module
                    || is_interface_exported_from_readable_module
                {
                    continue;
                }
                let mut warn = true;
                for export in module_exports(self.ctx, self.msym).iter() {
                    if let Directive::Exports { packge: Some(packge), .. } =
                        self.ctx.directive(export)
                        && *packge == interface_package
                    {
                        warn = false;
                        break;
                    }
                }
                if warn {
                    for uses in module_uses(self.ctx, self.msym).iter() {
                        if let Directive::Uses { service: Some(used), .. } =
                            self.ctx.directive(uses)
                            && *used == service
                        {
                            warn = false;
                            break;
                        }
                    }
                }
                if warn {
                    let pos = self.ctx.tree(tree).pos;
                    let name = self
                        .ctx
                        .sym(service)
                        .name
                        .to_java_string()
                        .to_string_lossy();
                    let warning = JCDiagnosticWarning::from(
                        crate::resources::compiler_properties::warnings::
                            service_provided_but_not_exported_or_used(
                                SymbolArg::placeholder(name),
                            ),
                    );
                    self.modules.log.borrow_mut().warning_pos(
                        Some(Rc::new(SimpleDiagnosticPosition::new(pos))),
                        warning,
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// UsesProvidesVisitor helpers.
// ---------------------------------------------------------------------------

fn package_members_scope(ctx: &CompilerContext, packge: SymbolId) -> Option<crate::ids::ScopeId> {
    match &ctx.sym(packge).kind_data {
        SymbolKind::PackageSymbol { members_field, .. } => *members_field,
        _ => None,
    }
}

fn class_members_scope(ctx: &CompilerContext, class: SymbolId) -> Option<crate::ids::ScopeId> {
    match &ctx.sym(class).kind_data {
        SymbolKind::ClassSymbol { members_field, .. } => *members_field,
        _ => None,
    }
}

fn classfile_of(ctx: &CompilerContext, class: SymbolId) -> Option<JavaFileObjectRef> {
    match &ctx.sym(class).kind_data {
        SymbolKind::ClassSymbol { classfile, .. } => classfile.clone(),
        _ => None,
    }
}

fn completer_terminal(ctx: &CompilerContext, sym: SymbolId) -> bool {
    match &ctx.sym(sym).completer {
        None => true,
        Some(completer) => completer.borrow().is_terminal(),
    }
}

fn package_fullname(ctx: &CompilerContext, packge: SymbolId) -> Name {
    match &ctx.sym(packge).kind_data {
        SymbolKind::PackageSymbol { fullname, .. } => fullname.clone(),
        _ => ctx.sym(packge).name.clone(),
    }
}

fn method_params_of(ctx: &CompilerContext, method: SymbolId) -> List<SymbolId> {
    match &ctx.sym(method).kind_data {
        SymbolKind::MethodSymbol { params, .. } => params.clone(),
        _ => List::nil(),
    }
}

fn ctx_symbol_is_static(ctx: &CompilerContext, sym: SymbolId) -> bool {
    ctx.sym(sym).is_static(ctx)
}

fn is_inner_symbol(ctx: &CompilerContext, sym: SymbolId) -> bool {
    if ctx.sym(sym).kind != Kind::TYP {
        return false;
    }
    ctx.sym(sym)
        .ty
        .and_then(|ty| ctx.typ(ty).get_enclosing_type())
        .map(|outer| ctx.typ(outer).has_tag(TypeTag::CLASS))
        .unwrap_or(false)
}

fn directive_requires_module(ctx: &CompilerContext, directive: DirectiveId) -> Option<SymbolId> {
    match ctx.directive(directive) {
        Directive::Requires { module, .. } => *module,
        _ => None,
    }
}

fn directive_requires_flags(ctx: &CompilerContext, directive: DirectiveId) -> Vec<RequiresFlag> {
    match ctx.directive(directive) {
        Directive::Requires { flags, .. } => flags.clone(),
        _ => Vec::new(),
    }
}
