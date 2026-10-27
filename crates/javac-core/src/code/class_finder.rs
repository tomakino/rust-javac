// Origin: com/sun/tools/javac/code/ClassFinder.java @ 982d53c (jdk-17.0.19-ga)
// Gap registration (method-level audit of ClassFinder.java):
// Dependency/phase: P2/S3 (ClassFinder body; the file layer JavaFileManager/
// Locations/JavacFileManager/JRTIndex is ported in this crate and injected).
//
// Ported in this file:
// - ClassFinder structure (fields for reader/log/names/file manager/dependencies/
// dcfh/source completer/current file+owner/profile/jrtIndex/flags);
// - getCompleter (via a Weak self reference, registered adapter), new
// (constructor; useCtProps/jrtIndex injection registered adapter),
// getSupplementaryFlags (jrtIndex==null path; Some path registered TODO),
// complete, completeOwners, completeEnclosing, fillIn(ClassSymbol),
// classFileNotFound, newCompletionFailure, loadClass, includeClassFile,
// preferredFileObject, getPackageFileKinds, extraFileActions,
// fillIn(PackageSymbol), scanModulePaths, scanUserPaths, scanPlatformPath,
// fillIn(package,location,files) and list();
// - BadClassFile/BadEnclosingMethodAttr (previously ported).
//
// Still not ported (no substitutes):
// - NOT PORTED: instance(Context) — the port injects dependencies in new();
// - the annotate.blockAnnotations/unblockAnnotationsNoFlush/flush calls are
// wired through the injected Annotate service (registered adapter);
// - NOT PORTED: getSupplementaryFlags jrtIndex!=null path (JRTIndex.isInJRT
// needs the PathFileObject downcast; JRTIndex integration);
// - NOT PORTED: verbose path printing (StandardJavaFileManager.getLocationAsPaths
// is not part of the ported JavaFileManager trait);
// - NOT PORTED: newCompletionFailure cached CompletionFailure reuse (the port
// allocates a fresh error value; the upstream reuse is a pure allocation
// optimization, registered adapter);
// - the module completer is installed by Symtab.set_module_completer
//   (Modules.getCompleter; batch 1b wiring).
//
// Registered adapters:
// - getCompleter/thisCompleter -> Weak<RefCell<ClassFinder>> self reference
// (no Context singletons, ADR 0001);
// - constructor useCtProps/jrtIndex computation -> jrt_index parameter supplied
// by the bootstrap;
// - Iterator<JavaFileObject> (list) -> Vec of filtered file objects (same
// filtering and HAS_RESOURCE marking);
// - standard location comparisons -> Location.get_name() equality (StandardLocation
// values are singletons upstream).

use std::cell::RefCell;
use std::rc::Rc;

use javac_tools::error::Error;
use javac_tools::tools::java_file_manager::{
    JavaFileManager, JavaFileObjectRef, Location, LocationRef,
};
use javac_tools::tools::java_file_object::{JavaFileObject, Kind};
use javac_tools::tools::standard_location::StandardLocation;

use crate::code::dependencies::{CompletionCause, Dependencies};
use crate::code::deferred_completion_failure_handler::DeferredCompletionFailureHandler;
use crate::comp::annotate::Annotate;
use crate::code::flags;
use crate::code::kinds::Kind as SymKind;
use crate::code::symbol::{Completer, CompleterRef, CompletionFailure};
use crate::code::symbol::SymbolKind as SymData;
use crate::code::symtab::Symtab;
use crate::compiler_context::CompilerContext;
use crate::file::jrt_index::JRTIndex;
use crate::ids::SymbolId;
use crate::jvm::class_reader::ClassReader;
use crate::jvm::profile::Profile;
use crate::resources::compiler_properties::fragments;
use crate::resources::diagnostic_contract::DiagnosticArg;
use crate::util::convert;
use crate::util::jc_diagnostic::{Factory, Fragment, JCDiagnostic};
use crate::util::log::Log;
use crate::util::name::Name;
use crate::util::names::Names;
use crate::util::options::Options;

/// Java: public class ClassFinder
pub struct ClassFinder {
    /// Java: ClassReader reader;
    pub reader: Rc<RefCell<ClassReader>>,
    /// Java: boolean verbose;
    pub verbose: bool,
    /// Java: private boolean cacheCompletionFailure;
    pub cache_completion_failure: bool,
    /// Java: protected boolean preferSource;
    pub prefer_source: bool,
    /// Java: protected boolean userPathsFirst;
    pub user_paths_first: bool,
    /// Java: final Log log;
    pub log: Rc<RefCell<Log>>,
    /// Java: final Names names;
    pub names: Rc<Names>,
    /// Java: final Name completionFailureName;
    pub completion_failure_name: Option<Name>,
    /// Java: private final JavaFileManager fileManager;
    pub file_manager: Rc<dyn JavaFileManager>,
    /// Java: private final Dependencies dependencies;
    pub dependencies: Rc<RefCell<dyn Dependencies>>,
    /// Java: JCDiagnostic.Factory diagFactory;
    pub diag_factory: Rc<Factory>,
    /// Java: final DeferredCompletionFailureHandler dcfh;
    pub dcfh: Rc<DeferredCompletionFailureHandler>,
    /// Java: public Completer sourceCompleter = Completer.NULL_COMPLETER;
    pub source_completer: RefCell<Option<CompleterRef>>,
    /// Java: protected JavaFileObject currentClassFile = null;
    pub current_class_file: RefCell<Option<Rc<dyn JavaFileObject>>>,
    /// Java: protected Symbol currentOwner = null;
    pub current_owner: std::cell::Cell<Option<SymbolId>>,
    /// Java: private final Profile profile;
    pub profile: Rc<Profile>,
    /// Java: private final JRTIndex jrtIndex;
    pub jrt_index: Option<Rc<JRTIndex>>,
    /// Java: private Map<PackageSymbol, Long> supplementaryFlags;
    pub supplementary_flags: RefCell<std::collections::HashMap<SymbolId, i64>>,
    /// Java: protected Location currentLoc; // FIXME
    pub current_loc: RefCell<Option<LocationRef>>,
    /// Java: private boolean verbosePath = true;
    pub verbose_path: std::cell::Cell<bool>,
    /// Java: private boolean preferCurrent;
    pub prefer_current: std::cell::Cell<bool>,
    /// Java: final Annotate annotate (ClassFinder.java:116)
    ///
    /// PORTING_RULES adapter: the Annotate service is created after the
    /// ClassFinder by the bootstrap; the shared injection cell mirrors the
    /// ClassReader/Enter/MemberEnter adapter (registered section 9).
    pub annotate: Rc<RefCell<Option<Rc<Annotate>>>>,
    /// Java: private final Completer thisCompleter = this::complete;
    /// PORTING_RULES adapter: a Weak self reference breaks the Rc cycle.
    pub self_ref: std::rc::Weak<ClassFinder>,
}

impl ClassFinder {
    /// Java: protected ClassFinder(Context context)
    ///
    /// PORTING_RULES adapter: dependencies are injected; the upstream
    /// useCtProps/jrtIndex computation (JavacFileManager instanceof checks) is
    /// passed as jrt_index by the bootstrap (registered).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        reader: Rc<RefCell<ClassReader>>,
        file_manager: Rc<dyn JavaFileManager>,
        names: Rc<Names>,
        log: Rc<RefCell<Log>>,
        diag_factory: Rc<Factory>,
        dcfh: Rc<DeferredCompletionFailureHandler>,
        profile: Rc<Profile>,
        options: &Options,
        jrt_index: Option<Rc<JRTIndex>>,
        annotate: Rc<RefCell<Option<Rc<Annotate>>>>,
    ) -> Rc<ClassFinder> {
        let verbose = options.is_set_option(crate::main::option::Option::VERBOSE);
        let cache_completion_failure = options.is_unset("dev");
        let prefer_source = options
            .get("-Xprefer")
            .map(|value| value.as_str() == "source")
            .unwrap_or(false);
        let user_paths_first = options.is_set("-XXuserPathsFirst");
        let completion_failure_name = if options.is_set("failcomplete") {
            options.get("failcomplete").map(|value| {
                names.from_string(&javac_strings::JavaString::from(value.as_str()))
            })
        } else {
            None
        };
        // Java: the inner annotation/proxy classes capture ClassReader.this
        // (ClassReader.java:3199/3159); the port stores the owning Rc.
        reader.borrow_mut().reader_handle = Some(Rc::clone(&reader));
        Rc::new_cyclic(|weak| ClassFinder {
            reader,
            verbose,
            cache_completion_failure,
            prefer_source,
            user_paths_first,
            log,
            names,
            completion_failure_name,
            file_manager,
            dependencies: Rc::new(RefCell::new(crate::code::dependencies::DummyDependencies)),
            diag_factory,
            dcfh,
            source_completer: RefCell::new(None),
            current_class_file: RefCell::new(None),
            current_owner: std::cell::Cell::new(None),
            profile,
            jrt_index,
            annotate,
            supplementary_flags: RefCell::new(std::collections::HashMap::new()),
            current_loc: RefCell::new(None),
            verbose_path: std::cell::Cell::new(true),
            prefer_current: std::cell::Cell::new(false),
            self_ref: weak.clone(),
        })
    }

    /// Java: public Completer getCompleter()
    pub fn get_completer(&self) -> CompleterRef {
        Rc::new(RefCell::new(ClassFinderCompleter {
            finder: self.self_ref.clone(),
        }))
    }

    /// Completer factory adapter: a fresh wrapper is created for every symbol
    /// (nested completion must not borrow one shared RefCell).
    pub fn get_completer_factory(&self) -> crate::code::symbol::CompleterFactory {
        let finder = self.self_ref.clone();
        Rc::new(move || {
            Rc::new(RefCell::new(ClassFinderCompleter {
                finder: finder.clone(),
            }))
        })
    }

    /// Java: long getSupplementaryFlags(ClassSymbol c)
    pub fn get_supplementary_flags(
        &self,
        ctx: &mut CompilerContext,
        c: SymbolId,
    ) -> i64 {
        let module_info = self.names.module_info.clone();
        let is_module_info = ctx.sym(c).name.to_java_string() == module_info.to_java_string();
        if self.jrt_index.is_none() || is_module_info {
            return 0;
        }
        // Java: ClassFinder.java:244-271.
        let Some(classfile) = class_classfile(ctx, c) else {
            return 0;
        };
        if !classfile.is_in_jrt() {
            return 0;
        }
        let Some(packge) = crate::code::symbol::symbol_packge(ctx, c) else {
            return 0;
        };
        if let Some(flags) = self.supplementary_flags.borrow().get(&packge) {
            return *flags;
        }
        let mut new_flags = 0i64;
        let jrt_index = self.jrt_index.as_ref().expect("jrtIndex");
        let package_name = ctx.sym(packge).flat_name().to_java_string();
        if let Ok(ct_sym) = jrt_index.get_ct_sym(&package_name) {
            // Java: Profile minProfile = Profile.DEFAULT; if (ctSym.proprietary)
            // newFlags |= PROPRIETARY; if (ctSym.minProfile != null)
            // minProfile = Profile.lookup(ctSym.minProfile);
            let mut min_profile = crate::jvm::profile::Profile::DEFAULT;
            if ct_sym.proprietary {
                new_flags |= crate::code::flags::PROPRIETARY;
            }
            if let Some(min) = &ct_sym.min_profile {
                min_profile = crate::jvm::profile::Profile::lookup(&min.to_string_lossy())
                    .expect("profile");
            }
            if *self.profile != crate::jvm::profile::Profile::DEFAULT
                && min_profile.value() > self.profile.value()
            {
                new_flags |= crate::code::flags::NOT_IN_PROFILE;
            }
        }
        self.supplementary_flags
            .borrow_mut()
            .insert(packge, new_flags);
        new_flags
    }

    /// Java: private void complete(Symbol sym) throws CompletionFailure
    pub fn complete(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        sym: SymbolId,
    ) -> Result<(), CompletionFailure> {
        if ctx.sym(sym).kind == SymKind::TYP {
            let c = sym;
            self.dependencies
                .borrow_mut()
                .push(ctx, c, CompletionCause::CLASS_READER);
            if let Some(annotate) = self.annotate.borrow().clone() {
                annotate.block_annotations();
            }
            let error_scope = ctx.scope_create_error(Some(c));
            if let SymData::ClassSymbol { members_field, .. } = &mut ctx.sym_mut(c).kind_data {
                *members_field = Some(error_scope);
            }
            let result = self.complete_class_body(ctx, syms, c);
            if let Some(annotate) = self.annotate.borrow().clone() {
                annotate.unblock_annotations_no_flush();
            }
            self.dependencies.borrow_mut().pop();
            result?;
        } else if ctx.sym(sym).kind == SymKind::PCK {
            let p = sym;
            self.fill_in_package(ctx, syms, p)?;
        }
        if !self.reader.borrow().filling {
            if let Some(annotate) = self.annotate.borrow().clone() {
                annotate.flush(ctx)?;
            }
        }
        Ok(())
    }

    fn complete_class_body(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        c: SymbolId,
    ) -> Result<(), CompletionFailure> {
        if let Some(owner) = ctx.sym(c).owner {
            self.complete_owners(ctx, syms, owner)?;
        }
        self.complete_enclosing(ctx, syms, c)?;
        self.fill_in_class(ctx, syms, c)
    }

    /// Java: private void completeOwners(Symbol o)
    fn complete_owners(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        o: SymbolId,
    ) -> Result<(), CompletionFailure> {
        if ctx.sym(o).kind != SymKind::PCK
            && let Some(owner) = ctx.sym(o).owner {
                self.complete_owners(ctx, syms, owner)?;
            }
        ctx.complete_symbol(syms, o)
    }

    /// Java: private void completeEnclosing(ClassSymbol c)
    fn complete_enclosing(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        c: SymbolId,
    ) -> Result<(), CompletionFailure> {
        let owner = ctx.sym(c).owner.expect("class owner");
        if ctx.sym(owner).kind != SymKind::PCK {
            return Ok(());
        }
        let short_name = convert::short_name(&ctx.sym(c).name.clone());
        let candidates = convert::enclosing_candidates(&short_name);
        let mut names = candidates;
        while names.non_empty() {
            let name = names.head().clone();
            let mut encl: Option<SymbolId> = None;
            if let Some(members) = ctx.sym(owner).members() {
                encl = ctx.scope_find_first(members, &name, &|_| true);
            }
            if encl.is_none() {
                let package = package_of(ctx, c);
                let module = match &ctx.sym(package).kind_data {
                    SymData::PackageSymbol { modle, .. } => modle.expect("package module"),
                    _ => panic!("package expected"),
                };
                let flat = crate::code::symtab::form_flat_name(ctx, &name, Some(owner));
                encl = syms.get_class(module, &flat);
            }
            if let Some(encl) = encl {
                ctx.complete_class_symbol(syms, encl)?;
            }
            names = names.tail();
        }
        Ok(())
    }

    /// Java: void fillIn(ClassSymbol c) (ClassFinder.java:341-392) — the
    /// ModuleFinder.readModule path needs the same completion entry.
    ///
    /// # Arguments
    ///   - ctx/syms: the compiler services;
    ///   - c: the class symbol.
    ///
    /// # Returns
    ///   - Ok(()) when the class body was filled in.
    ///
    /// # Panics
    ///   - none.
    pub(crate) fn fill_in(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        c: SymbolId,
    ) -> Result<(), CompletionFailure> {
        self.fill_in_class(ctx, syms, c)
    }

    /// Java: void fillIn(ClassSymbol c) (ClassFinder.java:341-392)
    fn fill_in_class(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        c: SymbolId,
    ) -> Result<(), CompletionFailure> {
        let fullname = class_fullname(ctx, c);
        let completion_failure_name = self.completion_failure_name.clone();
        if let Some(name) = completion_failure_name
            && name.to_java_string() == fullname.to_java_string() {
                let diag = self.diag_factory.fragment(Fragment::from(
                    fragments::user_selected_completion_failure(),
                ));
                let dcfh = Rc::clone(&self.dcfh);
                return Err(CompletionFailure::new(
                    Some(c),
                    Box::new(move || diag.clone()),
                    Some(dcfh),
                ));
            }
        self.current_owner.set(Some(c));
        let classfile = class_classfile(ctx, c);
        let Some(classfile) = classfile else {
            return Err(self.class_file_not_found(ctx, c));
        };
        let previous_class_file = self.current_class_file.borrow().clone();
        let prev_owner = ctx.sym(c).owner;
        let prev_name = fullname.clone();
        *self.current_class_file.borrow_mut() = Some(Rc::clone(&classfile));
        if self.verbose {
            self.log.borrow().print_verbose(
                "loading",
                vec![crate::util::jc_diagnostic::diagnostic_arg(
                    classfile.get_name().to_string_lossy(),
                )],
            );
        }
        let result = if classfile.get_kind() == Kind::CLASS {
            let read = self
                .reader
                .borrow_mut()
                .read_class_file(ctx, syms, c);
            match read {
                Ok(()) => {
                    let extra = self.get_supplementary_flags(ctx, c);
                    ctx.sym_mut(c).flags_field |= extra;
                    Ok(())
                }
                Err(failure) => Err(failure),
            }
        } else {
            // Java: if (!sourceCompleter.isTerminal()) sourceCompleter.complete(c);
            // else throw new IllegalStateException(...)
            let source_completer = self.source_completer.borrow().clone();
            match source_completer {
                Some(completer) if !completer.borrow().is_terminal() => {
                    completer.borrow_mut().complete(ctx, syms, c)
                }
                _ => panic!(
                    "IllegalStateException: Source completer required to read {}",
                    classfile.to_uri()
                ),
            }
        };
        *self.current_class_file.borrow_mut() = previous_class_file;
        if let Err(failure) = result {
            if failure.bad_class_file {
                self.purge_bad_class(ctx, c, prev_owner, &prev_name);
            }
            return Err(failure);
        }
        Ok(())
    }

    /// Java: where of fillIn — purge the partially initialized symbol after a
    /// BadClassFile (ClassFinder.java:365-388)
    fn purge_bad_class(
        &self,
        ctx: &mut CompilerContext,
        c: SymbolId,
        prev_owner: Option<SymbolId>,
        prev_name: &Name,
    ) {
        ctx.sym_mut(c).owner = prev_owner;
        let members = class_members_field(ctx, c);
        if let Some(members) = members {
            let scope_snapshot = ctx.scope(members).clone();
            let candidates = scope_snapshot.get_symbols(
                members,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
                ctx,
            );
            let symbols: Vec<SymbolId> = candidates
                .into_iter()
                .filter(|sym| ctx.sym(*sym).kind == SymKind::TYP)
                .collect();
            for sym in symbols {
                let package = package_of(ctx, sym);
                ctx.sym_mut(sym).owner = Some(package);
                let package_members = match ensure_package_members(ctx, package) {
                    Some(scope) => scope,
                    None => continue,
                };
                ctx.scope_enter(package_members, sym);
                let flat = symbol_flat_name(ctx, sym);
                if let SymData::ClassSymbol { fullname, .. } = &mut ctx.sym_mut(sym).kind_data {
                    *fullname = flat.clone();
                }
                ctx.sym_mut(sym).name = convert::short_name(&flat);
                ctx.reset_symbol(sym);
            }
        }
        if let SymData::ClassSymbol { fullname, .. } = &mut ctx.sym_mut(c).kind_data {
            *fullname = prev_name.clone();
        }
        ctx.sym_mut(c).name = convert::short_name(prev_name);
        ctx.reset_symbol(c);
    }

    /// Java: private CompletionFailure classFileNotFound(ClassSymbol c)
    fn class_file_not_found(&self, ctx: &mut CompilerContext, c: SymbolId) -> CompletionFailure {
        let flat = symbol_flat_name(ctx, c);
        self.new_completion_failure(c, fragments::class_file_not_found(flat))
    }

    /// Java: private CompletionFailure newCompletionFailure(TypeSymbol c,
    /// Supplier<JCDiagnostic> diag)
    fn new_completion_failure(
        &self,
        c: SymbolId,
        fragment: crate::resources::diagnostic_contract::FragmentInfo,
    ) -> CompletionFailure {
        // PORTING_RULES adapter: the upstream reuses one cached CompletionFailure;
        // the port allocates a fresh value (the reuse is a pure allocation
        // optimization, registered).
        let diag = self.diag_factory.fragment(Fragment::from(fragment));
        let dcfh = Rc::clone(&self.dcfh);
        CompletionFailure::new(Some(c), Box::new(move || diag.clone()), Some(dcfh))
    }

    /// Java: public ClassSymbol loadClass(ModuleSymbol msym, Name flatname)
    pub fn load_class(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        msym: SymbolId,
        flatname: Name,
    ) -> Result<SymbolId, CompletionFailure> {
        let package_name = convert::package_part(&flatname);
        let ps = syms.lookup_package(ctx, msym, &package_name);
        let package_module = match &ctx.sym(ps).kind_data {
            SymData::PackageSymbol { modle, .. } => modle.expect("msym"),
            _ => panic!("package expected"),
        };
        let absent = syms.get_class(package_module, &flatname).is_none();
        let c = syms.enter_class_flat(ctx, package_module, flatname.clone());
        if class_members_field(ctx, c).is_none() {
            match ctx.complete_class_symbol(syms, c) {
                Ok(()) => {}
                Err(failure) => {
                    if absent {
                        syms.remove_class(package_module, &flatname);
                        if let Some(dcfh) = failure.dcfh.clone() {
                            dcfh.class_symbol_removed(ctx, c);
                        }
                    }
                    return Err(failure);
                }
            }
        }
        Ok(c)
    }

    /// Java: protected void includeClassFile(PackageSymbol p, JavaFileObject file)
    fn include_class_file(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        p: SymbolId,
        file: &Rc<dyn JavaFileObject>,
    ) {
        if (ctx.sym(p).flags_field & flags::EXISTS as i64) == 0 {
            let mut q = Some(p);
            while let Some(current) = q {
                if ctx.sym(current).kind != SymKind::PCK {
                    break;
                }
                ctx.sym_mut(current).flags_field |= flags::EXISTS as i64;
                q = ctx.sym(current).owner;
            }
        }
        let kind = file.get_kind();
        let seen = if kind == Kind::CLASS {
            flags::CLASS_SEEN as i64
        } else {
            flags::SOURCE_SEEN as i64
        };
        let binary_name = {
            let loc = self.current_loc.borrow().clone().expect("currentLoc");
            self.file_manager
                .infer_binary_name(&*loc, file)
                .expect("inferBinaryName")
                .to_string()
        };
        let last_dot = binary_name.rfind('.').map(|index| index as i32).unwrap_or(-1);
        let class_name_text = &binary_name[(last_dot + 1) as usize..];
        let classname = self
            .names
            .from_string(&javac_strings::JavaString::from(class_name_text));
        let is_pkg_info =
            classname.to_java_string() == self.names.package_info.to_java_string();
        let mut c = if is_pkg_info {
            match &ctx.sym(p).kind_data {
                SymData::PackageSymbol { package_info, .. } => *package_info,
                _ => panic!("package expected"),
            }
        } else {
            let members = class_members_field(ctx, p);
            members.and_then(|scope| ctx.scope_find_first(scope, &classname, &|_| true))
        };
        if c.is_none() {
            // Java: syms.enterClass(p.modle, classname, p)
            let package_module = match &ctx.sym(p).kind_data {
                SymData::PackageSymbol { modle, .. } => modle.expect("package module"),
                _ => panic!("package expected"),
            };
            let class = syms.enter_class(ctx, package_module, classname, Some(p));
            if class_classfile(ctx, class).is_none()
                && let SymData::ClassSymbol { classfile, .. } = &mut ctx.sym_mut(class).kind_data {
                    *classfile = Some(Rc::clone(file));
                }
            if is_pkg_info {
                if let SymData::PackageSymbol { package_info, .. } = &mut ctx.sym_mut(p).kind_data {
                    *package_info = Some(class);
                }
            } else if ctx.sym(class).owner == Some(p)
                && let Some(members) = class_members_field(ctx, p) {
                    ctx.scope_enter(members, class);
                }
            c = Some(class);
        } else if !self.prefer_current.get() {
            let Some(existing) = c else { crate::util::assert::error() };
            let existing_classfile = class_classfile(ctx, existing);
            if let Some(existing_classfile) = existing_classfile
                && (ctx.sym(existing).flags_field & seen) == 0
                    && (ctx.sym(existing).flags_field
                        & (flags::CLASS_SEEN as i64 | flags::SOURCE_SEEN as i64))
                        != 0
                    {
                        let preferred =
                            self.preferred_file_object(file, &existing_classfile);
                        if let SymData::ClassSymbol { classfile, .. } =
                            &mut ctx.sym_mut(existing).kind_data
                        {
                            *classfile = Some(preferred);
                        }
                    }
        }
        if let Some(c) = c {
            ctx.sym_mut(c).flags_field |= seen;
        }
    }

    /// Java: protected JavaFileObject preferredFileObject(JavaFileObject a,
    /// JavaFileObject b)
    pub(crate) fn preferred_file_object(
        &self,
        a: &Rc<dyn JavaFileObject>,
        b: &Rc<dyn JavaFileObject>,
    ) -> Rc<dyn JavaFileObject> {
        if self.prefer_source {
            if a.get_kind() == Kind::SOURCE {
                Rc::clone(a)
            } else {
                Rc::clone(b)
            }
        } else {
            let a_date = a.get_last_modified();
            let b_date = b.get_last_modified();
            if a_date > b_date {
                Rc::clone(a)
            } else {
                Rc::clone(b)
            }
        }
    }

    /// Java: protected EnumSet<JavaFileObject.Kind> getPackageFileKinds()
    fn get_package_file_kinds(&self) -> Vec<Kind> {
        vec![Kind::CLASS, Kind::SOURCE]
    }

    /// Java: protected void extraFileActions(PackageSymbol pack, JavaFileObject fe)
    fn extra_file_actions(&self, _pack: SymbolId, _file: &Rc<dyn JavaFileObject>) {}

    /// Java: private void fillIn(PackageSymbol p) throws IOException
    fn fill_in_package(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        p: SymbolId,
    ) -> Result<(), CompletionFailure> {
        if class_members_field(ctx, p).is_none() {
            let members = ctx.scope_create(Some(p));
            if let SymData::PackageSymbol { members_field, .. } = &mut ctx.sym_mut(p).kind_data {
                *members_field = Some(members);
            }
        }
        let module = match &ctx.sym(p).kind_data {
            SymData::PackageSymbol { modle, .. }
            | SymData::RootPackageSymbol { modle, .. } => modle.expect("package module"),
            _ => panic!(
                "package expected: {} kind {:?}",
                ctx.sym(p).name.to_java_string().to_string_lossy(),
                ctx.sym(p).kind
            ),
        };
        // Java: msym.complete(); (ClassFinder.java:352).
        ctx.complete_symbol(syms, module)?;
        if module == syms.no_module {
            self.prefer_current.set(false);
            if self.user_paths_first {
                self.scan_user_paths(ctx, syms, p, true)?;
                self.prefer_current.set(true);
                self.scan_platform_path(ctx, syms, p)?;
            } else {
                self.scan_platform_path(ctx, syms, p)?;
                self.scan_user_paths(ctx, syms, p, true)?;
            }
        } else if location_is(&module_class_location(ctx, module), StandardLocation::CLASS_PATH) {
            let include_source_path =
                location_is(&module_source_location(ctx, module), StandardLocation::SOURCE_PATH);
            self.scan_user_paths(ctx, syms, p, include_source_path)?;
        } else {
            self.scan_module_paths(ctx, syms, p, module)?;
        }
        Ok(())
    }

    /// Java: private void scanModulePaths(PackageSymbol p, ModuleSymbol msym)
    fn scan_module_paths(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        p: SymbolId,
        msym: SymbolId,
    ) -> Result<(), CompletionFailure> {
        let kinds = self.get_package_file_kinds();
        let class_kinds: Vec<Kind> = kinds.iter().copied().filter(|k| *k != Kind::SOURCE).collect();
        let want_class_files = !class_kinds.is_empty();
        let source_kinds: Vec<Kind> = kinds.iter().copied().filter(|k| *k != Kind::CLASS).collect();
        let want_source_files = !source_kinds.is_empty();
        let package_name = package_fullname(ctx, p);
        let class_locn = module_class_location(ctx, msym);

        let source_locn = module_source_location(ctx, msym);
        let patch_locn = module_patch_location(ctx, msym);
        let patch_out_locn = module_patch_output_location(ctx, msym);
        let prev_prefer_current = self.prefer_current.get();
        let result = (|| -> Result<(), CompletionFailure> {
            self.prefer_current.set(false);
            if want_class_files
                && let Some(location) = patch_out_locn.clone() {
                    let files = self.list(ctx, p, &location, &package_name, &class_kinds)?;
                    self.fill_in_files(ctx, syms, p, &location, files)?;
                }
            if (want_class_files || want_source_files)
                && let Some(location) = patch_locn.clone() {
                    let mut combined = class_kinds.clone();
                    combined.extend(source_kinds.iter().copied());
                    let files = self.list(ctx, p, &location, &package_name, &combined)?;
                    self.fill_in_files(ctx, syms, p, &location, files)?;
                }
            self.prefer_current.set(true);
            if want_class_files
                && let Some(location) = class_locn.clone() {
                    let files = self.list(ctx, p, &location, &package_name, &class_kinds)?;
                    self.fill_in_files(ctx, syms, p, &location, files)?;
                }
            if want_source_files
                && let Some(location) = source_locn.clone() {
                    let files = self.list(ctx, p, &location, &package_name, &source_kinds)?;
                    self.fill_in_files(ctx, syms, p, &location, files)?;
                }
            Ok(())
        })();
        self.prefer_current.set(prev_prefer_current);
        result
    }

    /// Java: private void scanUserPaths(PackageSymbol p, boolean includeSourcePath)
    fn scan_user_paths(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        p: SymbolId,
        include_source_path: bool,
    ) -> Result<(), CompletionFailure> {
        let kinds = self.get_package_file_kinds();
        let class_kinds: Vec<Kind> = kinds.iter().copied().filter(|k| *k != Kind::SOURCE).collect();
        let want_class_files = !class_kinds.is_empty();
        let source_kinds: Vec<Kind> = kinds.iter().copied().filter(|k| *k != Kind::CLASS).collect();
        let want_source_files = !source_kinds.is_empty();
        let source_path_location = standard_location(StandardLocation::SOURCE_PATH);
        let have_source_path =
            include_source_path && self.file_manager.has_location(&*source_path_location);
        if self.verbose && self.verbose_path.get() {
            self.verbose_path.set(false); // print once per compile
            // TODO(S3/verbose): the upstream prints the source/class paths through
            // StandardJavaFileManager.getLocationAsPaths; that method is not part of
            // the ported JavaFileManager trait (registered).
        }
        let package_name = package_fullname(ctx, p);
        if want_source_files && !have_source_path {
            let location = standard_location(StandardLocation::CLASS_PATH);
            let files = self.list(ctx, p, &location, &package_name, &kinds)?;
            self.fill_in_files(ctx, syms, p, &location, files)
        } else {
            if want_class_files {
                let location = standard_location(StandardLocation::CLASS_PATH);
                let files = self.list(ctx, p, &location, &package_name, &class_kinds)?;
                self.fill_in_files(ctx, syms, p, &location, files)?;
            }
            if want_source_files {
                let location = standard_location(StandardLocation::SOURCE_PATH);
                let files = self.list(ctx, p, &location, &package_name, &source_kinds)?;
                self.fill_in_files(ctx, syms, p, &location, files)?;
            }
            Ok(())
        }
    }

    /// Java: private void scanPlatformPath(PackageSymbol p)
    fn scan_platform_path(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        p: SymbolId,
    ) -> Result<(), CompletionFailure> {
        let location = standard_location(StandardLocation::PLATFORM_CLASS_PATH);
        let package_name = package_fullname(ctx, p);
        let files = self.list(ctx, p, &location, &package_name, &[Kind::CLASS])?;
        self.fill_in_files(ctx, syms, p, &location, files)
    }

    /// Java: private void fillIn(PackageSymbol p, Location location,
    /// Iterable<JavaFileObject> files)
    fn fill_in_files(
        &self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        p: SymbolId,
        location: &LocationRef,
        files: Vec<JavaFileObjectRef>,
    ) -> Result<(), CompletionFailure> {
        *self.current_loc.borrow_mut() = Some(Rc::clone(location));
        for file in files {
            match file.get_kind() {
                Kind::OTHER => self.extra_file_actions(p, &file),
                Kind::CLASS | Kind::SOURCE => {
                    let binary_name = self
                        .file_manager
                        .infer_binary_name(&**location, &file)
                        .expect("inferBinaryName")
                        .to_string();
                    let simple_name = binary_name
                        .rsplit('.')
                        .next()
                        .unwrap_or(&binary_name)
                        .to_string();
                    let is_identifier =
                        javac_tools::lang::model::source_version::SourceVersion::is_identifier(
                            &javac_strings::JavaString::from(simple_name.as_str()),
                        );
                    if is_identifier || simple_name == "package-info" {
                        self.include_class_file(ctx, syms, p, &file);
                    }
                }
                _ => self.extra_file_actions(p, &file),
            }
        }
        Ok(())
    }

    /// Java: Iterable<JavaFileObject> list(Location location, PackageSymbol p,
    /// String packageName, Set<Kind> kinds)
    ///
    /// PORTING_RULES adapter: the upstream lazy Iterator is expressed as a Vec
    /// with the same filtering and the HAS_RESOURCE marking.
    fn list(
        &self,
        ctx: &mut CompilerContext,
        p: SymbolId,
        location: &LocationRef,
        package_name: &str,
        kinds: &[Kind],
    ) -> Result<Vec<JavaFileObjectRef>, CompletionFailure> {
        let all_kinds = [Kind::SOURCE, Kind::CLASS, Kind::HTML, Kind::OTHER];
        let package_name = javac_strings::JavaString::from(package_name);
        let listed = self
            .file_manager
            .list(&**location, &package_name, &all_kinds, false);
        let listed = match listed {
            Ok(listed) => {
                listed
            }
            Err(error) => return Err(self.io_failure(ctx, p, error)),
        };
        let mut result = Vec::new();
        for file in listed {
            let kind = file.get_kind();
            if kind != Kind::CLASS && kind != Kind::SOURCE {
                ctx.sym_mut(p).flags_field |= flags::HAS_RESOURCE;
            }
            if kinds.contains(&kind) {
                result.push(file);
            }
        }
        Ok(result)
    }

    /// Java: the IOException -> CompletionFailure(ExceptionMessage) mapping of
    /// complete(PackageSymbol) (ClassFinder.java:296-305)
    fn io_failure(
        &self,
        _ctx: &mut CompilerContext,
        p: SymbolId,
        error: Error,
    ) -> CompletionFailure {
        // Java: Fragments.ExceptionMessage(ex.getLocalizedMessage()) — for
        // java.io.IOException getLocalizedMessage() == getMessage(), i.e. the
        // Java String message of the exception.
        let message = io_failure_message(&error);
        let diag = self.diag_factory.fragment(Fragment::from(
            fragments::exception_message(message),
        ));
        let dcfh = Rc::clone(&self.dcfh);
        CompletionFailure::new(Some(p), Box::new(move || diag.clone()), Some(dcfh))
    }
}

/// Java: ex.getLocalizedMessage() for the IOException caught by ClassFinder
/// (ClassFinder.java:300-306). The Rust Error enum models the checked
/// IOException and UnsupportedOperationException; the latter is a runtime
/// exception that upstream does not catch here (PORTING_RULES section 6: a
/// runtime exception maps to panic).
pub(crate) fn io_failure_message(error: &Error) -> javac_strings::JavaString {
    match error {
        Error::IOException(message) => message.clone(),
        Error::UnsupportedOperationException => {
            panic!("UnsupportedOperationException")
        }
    }
}

/// Java: the anonymous Completer thisCompleter = this::complete (registered
/// adapter: Weak self reference instead of the Java implicit this).
pub struct ClassFinderCompleter {
    pub finder: std::rc::Weak<ClassFinder>,
}

impl Completer for ClassFinderCompleter {
    fn complete(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut Symtab,
        sym: SymbolId,
    ) -> Result<(), CompletionFailure> {
        let finder = self.finder.upgrade().expect("ClassFinder is alive");
        finder.complete(ctx, syms, sym)
    }
}

/// Java: Location comparisons against StandardLocation singletons (the port
/// compares Location.get_name(); registered adapter).
fn standard_location(location: StandardLocation) -> LocationRef {
    Rc::new(location)
}

fn location_is(location: &Option<LocationRef>, expected: StandardLocation) -> bool {
    match location {
        Some(location) => {
            location.get_name().to_string() == expected.get_name().to_string()
        }
        None => false,
    }
}

fn class_fullname(ctx: &CompilerContext, c: SymbolId) -> Name {
    match &ctx.sym(c).kind_data {
        SymData::ClassSymbol { fullname, .. } => fullname.clone(),
        _ => panic!("class symbol expected"),
    }
}

fn class_classfile(
    ctx: &CompilerContext,
    c: SymbolId,
) -> Option<Rc<dyn JavaFileObject>> {
    match &ctx.sym(c).kind_data {
        SymData::ClassSymbol { classfile, .. } => classfile.clone(),
        _ => None,
    }
}

fn class_members_field(
    ctx: &CompilerContext,
    c: SymbolId,
) -> Option<crate::ids::ScopeId> {
    match &ctx.sym(c).kind_data {
        SymData::ClassSymbol { members_field, .. }
        | SymData::PackageSymbol { members_field, .. }
        | SymData::RootPackageSymbol { members_field, .. } => *members_field,
        _ => None,
    }
}

fn package_fullname(ctx: &CompilerContext, p: SymbolId) -> String {
    match &ctx.sym(p).kind_data {
        SymData::PackageSymbol { fullname, .. }
        | SymData::RootPackageSymbol { fullname, .. } => fullname.to_java_string().to_string(),
        _ => ctx.sym(p).name.to_java_string().to_string(),
    }
}

fn package_of(ctx: &CompilerContext, symbol: SymbolId) -> SymbolId {
    let mut owner = Some(symbol);
    while let Some(current) = owner {
        if ctx.sym(current).kind == SymKind::PCK {
            return current;
        }
        owner = ctx.sym(current).owner;
    }
    panic!("package owner missing")
}

fn symbol_flat_name(ctx: &CompilerContext, symbol: SymbolId) -> Name {
    match &ctx.sym(symbol).kind_data {
        SymData::ClassSymbol { flatname, .. } => flatname.clone(),
        SymData::PackageSymbol { fullname, .. }
        | SymData::RootPackageSymbol { fullname, .. } => fullname.clone(),
        _ => ctx.sym(symbol).name.clone(),
    }
}

fn ensure_package_members(
    ctx: &mut CompilerContext,
    package: SymbolId,
) -> Option<crate::ids::ScopeId> {
    if let Some(members) = class_members_field(ctx, package) {
        return Some(members);
    }
    let members = ctx.scope_create(Some(package));
    if let SymData::PackageSymbol { members_field, .. } = &mut ctx.sym_mut(package).kind_data {
        *members_field = Some(members);
    }
    Some(members)
}

fn module_class_location(ctx: &CompilerContext, msym: SymbolId) -> Option<LocationRef> {
    match &ctx.sym(msym).kind_data {
        SymData::ModuleSymbol { class_location, .. } => class_location.clone(),
        _ => None,
    }
}

fn module_source_location(ctx: &CompilerContext, msym: SymbolId) -> Option<LocationRef> {
    match &ctx.sym(msym).kind_data {
        SymData::ModuleSymbol { source_location, .. } => source_location.clone(),
        _ => None,
    }
}

fn module_patch_location(ctx: &CompilerContext, msym: SymbolId) -> Option<LocationRef> {
    match &ctx.sym(msym).kind_data {
        SymData::ModuleSymbol { patch_location, .. } => patch_location.clone(),
        _ => None,
    }
}

fn module_patch_output_location(ctx: &CompilerContext, msym: SymbolId) -> Option<LocationRef> {
    match &ctx.sym(msym).kind_data {
        SymData::ModuleSymbol { patch_output_location, .. } => patch_output_location.clone(),
        _ => None,
    }
}

/// Used for bad class definition files, such as bad .class files or
/// for .java files with unexpected package or class names.
///
/// Java: public static class ClassFinder.BadClassFile extends CompletionFailure
pub struct BadClassFile {
    /// Java: the inherited CompletionFailure part.
    pub failure: CompletionFailure,
}

impl BadClassFile {
    /// Java: BadClassFile(TypeSymbol sym, JavaFileObject file, JCDiagnostic diag,
    /// JCDiagnostic.Factory diagFactory, DeferredCompletionFailureHandler dcfh)
    pub fn new(
        sym: Option<SymbolId>,
        file: Rc<dyn JavaFileObject>,
        diag: JCDiagnostic,
        diag_factory: Rc<Factory>,
        dcfh: Option<Rc<DeferredCompletionFailureHandler>>,
    ) -> BadClassFile {
        let supplier_file = Rc::clone(&file);
        let supplier_diag = diag.clone();
        let supplier_factory = Rc::clone(&diag_factory);
        let mut failure = CompletionFailure::new(
            sym,
            Box::new(move || {
                create_bad_class_file_diagnostic(
                    &supplier_file,
                    &supplier_diag,
                    &supplier_factory,
                )
            }),
            dcfh,
        );
        // ClassFinder.fillIn only purges partially initialized symbols for
        // BadClassFile; the marker records the Rust error subtype (registered
        // adapter).
        failure.bad_class_file = true;
        BadClassFile { failure }
    }
}

/// Java: private static JCDiagnostic createBadClassFileDiagnostic(
/// JavaFileObject file, JCDiagnostic diag, JCDiagnostic.Factory diagFactory)
pub fn create_bad_class_file_diagnostic(
    file: &Rc<dyn JavaFileObject>,
    diag: &JCDiagnostic,
    diag_factory: &Rc<Factory>,
) -> JCDiagnostic {
    let is_source = file.get_kind() == Kind::SOURCE;
    // PORTING_RULES: P4 placeholder arguments (ADR 0002); the file and the
    // nested diagnostic are carried as their text until the typed diagnostic
    // arguments land.
    let file_arg = DiagnosticArg::from_display(file.to_uri());
    let diag_arg = DiagnosticArg::from_display(diag.get_message(None));
    let fragment = if is_source {
        Fragment::from(fragments::bad_source_file_header(file_arg, diag_arg))
    } else {
        Fragment::from(fragments::bad_class_file_header(file_arg, diag_arg))
    };
    diag_factory.fragment(fragment)
}

/// Java: public static class ClassFinder.BadEnclosingMethodAttr extends BadClassFile
pub struct BadEnclosingMethodAttr {
    pub bad_class_file: BadClassFile,
}

impl BadEnclosingMethodAttr {
    /// Java: BadEnclosingMethodAttr(TypeSymbol sym, JavaFileObject file,
    /// JCDiagnostic diag, JCDiagnostic.Factory diagFactory, DeferredCompletionFailureHandler dcfh)
    pub fn new(
        sym: Option<SymbolId>,
        file: Rc<dyn JavaFileObject>,
        diag: JCDiagnostic,
        diag_factory: Rc<Factory>,
        dcfh: Option<Rc<DeferredCompletionFailureHandler>>,
    ) -> BadEnclosingMethodAttr {
        BadEnclosingMethodAttr {
            bad_class_file: BadClassFile::new(sym, file, diag, diag_factory, dcfh),
        }
    }
}

impl std::ops::Deref for BadEnclosingMethodAttr {
    type Target = BadClassFile;

    fn deref(&self) -> &BadClassFile {
        &self.bad_class_file
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    use javac_tools::error::Error as ToolsError;
    use javac_tools::tools::file_object::FileObject;
    use javac_tools::tools::java_file_object::{Kind, JavaFileObject};
    use javac_tools::tools::option_checker::OptionChecker;
    use javac_tools::tools::java_file_manager::{FileObjectRef, JavaFileManager, JavaFileObjectRef};
    use javac_tools::uri::URI;

    use crate::compiler_context::CompilerContext;
    use crate::code::deferred_completion_failure_handler::DeferredCompletionFailureHandler;
    use crate::code::preview::Preview;
    use crate::code::source::Source;
    use crate::code::symbol::SymbolKind;
    use crate::jvm::target::Target;
    use crate::jvm::class_reader::ClassReader;
    use crate::util::context::Context;
    use crate::util::javac_messages::JavacMessages;
    use crate::util::jc_diagnostic::Factory;
    use crate::util::log::Log;
    use crate::util::names::Names;
    use crate::util::list::List;
    use crate::util::options::Options;

    fn baseline_java_home() -> Option<std::path::PathBuf> {
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            let home = std::path::PathBuf::from(home);
            if home.join("lib").join("modules").is_file() || home.join("jmods").is_dir() {
                return Some(home);
            }
        }
        let fallback =
            std::path::PathBuf::from(r"C:\Program Files\Eclipse Adoptium\jdk-17.0.19.10-hotspot");
        if fallback.join("lib").join("modules").is_file() || fallback.join("jmods").is_dir() {
            return Some(fallback);
        }
        None
    }

    struct TestClassFile {
        name: javac_strings::JavaString,
        bytes: Vec<u8>,
    }

    impl FileObject for TestClassFile {
        fn to_uri(&self) -> URI {
            URI::create(&javac_strings::JavaString::from(
                format!("memory:/{}", self.name.to_string_lossy()).as_str(),
            ))
        }

        fn get_name(&self) -> javac_strings::JavaString {
            self.name.clone()
        }

        fn open_input_stream(&self) -> Result<Box<dyn Read>, ToolsError> {
            Ok(Box::new(std::io::Cursor::new(self.bytes.clone())))
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
            Ok(None)
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

    impl JavaFileObject for TestClassFile {
        fn get_kind(&self) -> Kind {
            Kind::CLASS
        }

        fn is_name_compatible(
            &self,
            _simple_name: &javac_strings::JavaString,
            _kind: Kind,
        ) -> bool {
            true
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

    /// A platform class path file manager over the given class files (test
    /// infrastructure only: list/infer_binary_name/has_location are backed by a
    /// class list; the class BYTES come from java-classpath/jimage).
    struct TestFileManager {
        files: Vec<Rc<TestClassFile>>,
    }

    impl OptionChecker for TestFileManager {
        fn is_supported_option(&self, _option: &javac_strings::JavaString) -> i32 {
            0
        }
    }

    impl JavaFileManager for TestFileManager {
        fn get_class_loader(
            &self,
            _location: &dyn Location,
        ) -> Option<Rc<dyn javac_tools::jdk::ClassLoader>> {
            None
        }

        fn list(
            &self,
            location: &dyn Location,
            package_name: &javac_strings::JavaString,
            kinds: &[Kind],
            _recurse: bool,
        ) -> Result<Vec<JavaFileObjectRef>, ToolsError> {
            if location.get_name().to_string_lossy() != "PLATFORM_CLASS_PATH"
                || !kinds.contains(&Kind::CLASS)
            {
                return Ok(Vec::new());
            }
            // Java: Names are dot-separated (dots are the Name separator); the
            // test file names are slash-separated paths.
            let package_path = package_name.to_string_lossy().replace('.', "/");
            let mut result = Vec::new();
            for file in &self.files {
                let name = file.name.to_string_lossy();
                let Some(binary_name) = name.strip_suffix(".class") else {
                    continue;
                };
                let package = match binary_name.rfind('/') {
                    Some(index) => &binary_name[..index],
                    None => "",
                };
                if package == package_path {
                    result.push(Rc::clone(file) as JavaFileObjectRef);
                }
            }
            Ok(result)
        }

        fn infer_binary_name(
            &self,
            _location: &dyn Location,
            file: &JavaFileObjectRef,
        ) -> Option<javac_strings::JavaString> {
            // Java binary names are dot-separated (the upstream inferBinaryName
            // result); the test file names are slash-separated.
            let name = file.get_name().to_string_lossy();
            let binary_name = name.strip_suffix(".class").unwrap_or(&name);
            Some(javac_strings::JavaString::from(
                binary_name.replace('/', ".").as_str(),
            ))
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

        fn has_location(&self, location: &dyn Location) -> bool {
            location.get_name().to_string_lossy() == "PLATFORM_CLASS_PATH"
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

    struct FinderFixture {
        ctx: CompilerContext,
        syms: Symtab,
        finder: Rc<ClassFinder>,
        names: Rc<Names>,
        java_base: SymbolId,
    }

    fn finder_fixture(extra: Vec<(&str, Vec<u8>)>) -> Option<FinderFixture> {
        let home = baseline_java_home()?;
        let class_path = javac_classpath::ClassPath::system(&home).ok()?;
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        for binary in ["java.lang.Object", "java.lang.String", "java.util.List"] {
            if let Some(bytes) = class_path.read_class(binary) {
                files.push((format!("{}.class", binary.replace('.', "/")), bytes));
            }
        }
        for (binary, bytes) in extra {
            files.push((format!("{}.class", binary.replace('.', "/")), bytes));
        }
        let files: Vec<Rc<TestClassFile>> = files
            .into_iter()
            .map(|(name, bytes)| {
                Rc::new(TestClassFile {
                    name: javac_strings::JavaString::from(name.as_str()),
                    bytes,
                })
            })
            .collect();
        let manager = Rc::new(TestFileManager { files });

        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = JavacMessages::instance(&mut context);
        let factory = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Profile::instance(&mut context);
        let options = Options::instance(&mut context);
        let source = Source::JDK17;
        let reader = Rc::new(RefCell::new(ClassReader::new(
            Rc::clone(&log),
            Rc::clone(&names),
            Rc::clone(&factory),
            Rc::clone(&dcfh),
            Rc::clone(&preview),
            Rc::clone(&profile),
            &options,
            source,
            Rc::new(RefCell::new(None)),
        )));
        let mut ctx = CompilerContext::new();
        let mut syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let java_base = syms.java_base;
        // Test bootstrap: the upstream Modules sets the system module locations;
        // The platform class location is installed by the bootstrap; the
// The mini compiler calls Modules.initModules (JavaCompiler.initModules
// equivalent) at the start of every compile, so the
        // here (registered adapter, see the registration).
        if let SymbolKind::ModuleSymbol { class_location, .. } =
            &mut ctx.sym_mut(java_base).kind_data
        {
            *class_location = Some(Rc::new(StandardLocation::PLATFORM_CLASS_PATH));
        }
        let finder = ClassFinder::new(
            Rc::clone(&reader),
            manager as Rc<dyn JavaFileManager>,
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&factory),
            Rc::clone(&dcfh),
            Rc::clone(&profile),
            &options,
            None,
            std::rc::Rc::new(std::cell::RefCell::new(None)),
        );
        let factory = finder.get_completer_factory();
        syms.set_initial_completer(&mut ctx, factory);
        Some(FinderFixture {
            ctx,
            syms,
            finder,
            names,
            java_base,
        })
    }

    fn member_counts(ctx: &mut CompilerContext, _syms: &Symtab, c: SymbolId) -> (usize, usize, usize) {
        let mut fields = 0;
        let mut methods = 0;
        let mut classes = 0;
        let members = match &ctx.sym(c).kind_data {
            SymbolKind::ClassSymbol { members_field, .. } => *members_field,
            _ => None,
        };
        if let Some(members) = members {
            let symbols = ctx.scope_get_symbols(
                members,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            );
            for symbol in symbols {
                match ctx.sym(symbol).kind {
                    SymKind::VAR => fields += 1,
                    SymKind::MTH => methods += 1,
                    SymKind::TYP => classes += 1,
                    _ => {}
                }
            }
        }
        (fields, methods, classes)
    }

    fn load(fixture: &mut FinderFixture, binary_name: &str) -> Result<SymbolId, CompletionFailure> {
        let flatname = fixture
            .names
            .from_string(&javac_strings::JavaString::from(binary_name));
        fixture
            .finder
            .load_class(&mut fixture.ctx, &mut fixture.syms, fixture.java_base, flatname)
    }

    #[test]
    fn loads_platform_classes_like_javap() {
        let Some(mut fixture) = finder_fixture(Vec::new()) else {
            return;
        };
        let object = load(&mut fixture, "java.lang.Object").expect("load java.lang.Object");
        let (fields, methods, _) = member_counts(&mut fixture.ctx, &fixture.syms, object);
        assert_eq!(fields + methods, 12, "Object members (javap -private: 12)");
        let object_super = match &fixture.ctx.typ(fixture.ctx.sym(object).ty.expect("type")).kind {
            crate::code::type_::TypeKindData::ClassType { supertype_field, .. } => *supertype_field,
            _ => panic!("class type"),
        };
        assert!(
            object_super
                .map(|t| fixture.ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::NONE))
                .unwrap_or(true),
            "Object has no superclass"
        );

        let string = load(&mut fixture, "java.lang.String").expect("load java.lang.String");
        let (fields, methods, classes) = member_counts(&mut fixture.ctx, &fixture.syms, string);
        assert_eq!(fields + methods, 171, "String fields+methods (javap -private: 171)");
        assert_eq!(classes, 1, "String inner class member entered by InnerClasses");
        let string_super = match &fixture.ctx.typ(fixture.ctx.sym(string).ty.expect("type")).kind {
            crate::code::type_::TypeKindData::ClassType { supertype_field, .. } => *supertype_field,
            _ => panic!("class type"),
        };
        assert_eq!(
            string_super.map(|t| {
                fixture
                    .ctx
                    .typ(t)
                    .tsym
                    .map(|s| fixture.ctx.sym(s).flat_name().to_java_string().to_string())
            }),
            Some(Some("java.lang.Object".to_string())),
            "String superclass"
        );

        let list = load(&mut fixture, "java.util.List").expect("load java.util.List");
        let (fields, methods, _) = member_counts(&mut fixture.ctx, &fixture.syms, list);
        assert_eq!(fields + methods, 41, "List fields+methods (javap -private: 41)");
        let interfaces = match &fixture.ctx.typ(fixture.ctx.sym(list).ty.expect("type")).kind {
            crate::code::type_::TypeKindData::ClassType { interfaces_field, .. } => {
                interfaces_field.clone().unwrap_or_else(List::nil)
            }
            _ => panic!("class type"),
        };
        let mut names = Vec::new();
        let mut current = interfaces;
        while current.non_empty() {
            if let Some(tsym) = fixture.ctx.typ(*current.head()).tsym {
                names.push(
                    fixture
                        .ctx
                        .sym(tsym)
                        .flat_name()
                        .to_java_string()
                        .to_string(),
                );
            }
            current = current.tail();
        }
        assert!(
            names.contains(&"java.util.Collection".to_string()),
            "List interfaces: {names:?}"
        );
    }

    #[test]
    fn missing_class_reports_class_file_not_found() {
        let Some(mut fixture) = finder_fixture(Vec::new()) else {
            return;
        };
        let error = load(&mut fixture, "java.lang.NoSuchClass").expect_err("missing class");
        let message = error.get_message().to_string_lossy();
        assert!(
            message.contains("class file for java.lang.NoSuchClass not found"),
            "message: {message}"
        );
    }

    #[test]
    fn bad_class_file_reports_bad_class_file_header() {
        let mut bad_bytes = vec![0x00, 0x01, 0x02, 0x03];
        bad_bytes.extend_from_slice(&[0u8; 8]);
        let Some(mut fixture) = finder_fixture(vec![("java.lang.Bad", bad_bytes)]) else {
            return;
        };
        let error = load(&mut fixture, "java.lang.Bad").expect_err("bad class file");
        let message = error.get_message().to_string_lossy();
        assert!(message.contains("bad class file"), "message: {message}");
    }
}

#[cfg(test)]
mod io_failure_tests {
    use super::*;

    #[test]
    fn io_failure_message_uses_the_ioexception_message() {
        let error = Error::io(javac_strings::JavaString::from("disk read failed"));
        assert_eq!(
            io_failure_message(&error).to_string_lossy(),
            "disk read failed"
        );
        // The Debug representation must not leak into the diagnostic.
        assert_ne!(io_failure_message(&error).to_string_lossy(), format!("{error:?}"));
    }

    #[test]
    #[should_panic(expected = "UnsupportedOperationException")]
    fn io_failure_message_panics_for_unsupported_operation() {
        let error = Error::UnsupportedOperationException;
        let _ = io_failure_message(&error);
    }
}
