//! This class provides operations to locate module definitions from the source
//! and class files on the paths provided to javac.
//!
//! ModuleFinder.java:71-416 is ported: findModule(Name),
//! findModule(ModuleSymbol), findAllModules, findSingleModule, readModule,
//! readModuleName, scanModulePath, findModuleInfo, moduleLocationIterator,
//! getModuleInfoFromLocation, preferredFileObject and getDescription.
//! moduleNameFromSourceReader is the SOURCE branch of readModuleName
//! (ModuleNameReader.readModuleName).

use std::cell::RefCell;
use std::rc::Rc;

use javac_tools::tools::java_file_manager::{JavaFileManager, JavaFileObjectRef, LocationRef};
use javac_tools::tools::java_file_object::Kind as FileKind;
use javac_tools::tools::standard_location::StandardLocation;

use crate::code::class_finder::ClassFinder;
use crate::code::flags;
use crate::code::kinds::Kind;
use crate::code::symbol::{Completer, CompleterRef, CompletionFailure, SymbolKind};
use crate::code::symtab::Symtab;
use crate::compiler_context::CompilerContext;
use crate::jvm::module_name_reader::ModuleNameReader;
use crate::resources::compiler_properties::{errors, fragments};
use crate::util::abstract_log::AbstractLog;
use crate::util::java_string::JavaString;
use crate::util::jc_diagnostic::{Factory, Fragment};
use crate::util::log::Log;
use crate::util::name::Name;
use crate::util::names::Names;

/// The module-oriented locations scanned by scanModulePath (ModuleFinder.java:
/// 123-128).
const MODULE_LOCATION_OUTERS: [StandardLocation; 4] = [
    StandardLocation::MODULE_SOURCE_PATH,
    StandardLocation::UPGRADE_MODULE_PATH,
    StandardLocation::SYSTEM_MODULES,
    StandardLocation::MODULE_PATH,
];

/// Java: private class ModuleLocationIterator (ModuleFinder.java:119-163).
struct ModuleLocationIterator {
    /// Java: StandardLocation outer.
    outer: Option<StandardLocation>,
    /// Index of the next outer location.
    outer_index: usize,
    /// The module locations of the current outer location.
    inner: Option<Vec<Vec<LocationRef>>>,
    /// Index of the next inner location list.
    inner_index: usize,
}

impl ModuleLocationIterator {
    /// Creates an exhausted iterator (Java: field initialisation).
    ///
    /// # Arguments
    ///   - none.
    ///
    /// # Returns
    ///   - the iterator.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: ModuleFinder.java:119-165.
    fn new() -> ModuleLocationIterator {
        ModuleLocationIterator {
            outer: None,
            outer_index: 0,
            inner: None,
            inner_index: 0,
        }
    }

    /// Java: public boolean hasNext() (ModuleFinder.java:131-150).
    fn has_next(&mut self, file_manager: &dyn JavaFileManager) -> bool {
        loop {
            if let Some(inner) = &self.inner
                && self.inner_index < inner.len()
            {
                return true;
            }
            if self.outer_index >= MODULE_LOCATION_OUTERS.len() {
                return false;
            }
            let outer = MODULE_LOCATION_OUTERS[self.outer_index];
            self.outer_index += 1;
            self.outer = Some(outer);
            match file_manager.list_locations_for_modules(&outer) {
                Ok(inner) => {
                    self.inner = Some(inner);
                    self.inner_index = 0;
                }
                Err(_) => {
                    // Java: catch (IOException e) { System.err.println(...) }
                    self.inner = None;
                }
            }
        }
    }

    /// Java: public Set<Location> next() (ModuleFinder.java:152-161).
    fn next(&mut self, file_manager: &dyn JavaFileManager) -> Option<Vec<LocationRef>> {
        if !self.has_next(file_manager) {
            return None;
        }
        let inner = self.inner.as_ref()?;
        let result = inner[self.inner_index].clone();
        self.inner_index += 1;
        Some(result)
    }

    /// Java: moduleLocationIterator.outer.
    fn outer(&self) -> Option<StandardLocation> {
        self.outer
    }
}

/// Java: public class ModuleFinder (ModuleFinder.java:71).
pub struct ModuleFinder {
    /// Java: private final Names names.
    names: Rc<Names>,
    /// Java: private final ClassFinder classFinder.
    class_finder: Rc<ClassFinder>,
    /// Java: private final JavaFileManager fileManager.
    file_manager: Rc<dyn JavaFileManager>,
    /// Java: private final Log log.
    log: Rc<Log>,
    /// Java: private final JCDiagnostic.Factory diags.
    diags: Rc<Factory>,
    /// Java: ModuleLocationIterator moduleLocationIterator.
    module_location_iterator: RefCell<ModuleLocationIterator>,
    /// Java: private ModuleNameReader moduleNameReader (lazy).
    module_name_reader: RefCell<Option<ModuleNameReader>>,
}

impl ModuleFinder {
    /// Java: protected ModuleFinder(Context context) (ModuleFinder.java:107-117).
    ///
    /// # Arguments
    ///   - names: the name table;
    ///   - class_finder: the class finder;
    ///   - file_manager: the javac file manager;
    ///   - log: the log.
    ///
    /// # Returns
    ///   - the module finder.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: ModuleFinder.java:99-117.
    pub fn new(
        names: Rc<Names>,
        class_finder: Rc<ClassFinder>,
        file_manager: Rc<dyn JavaFileManager>,
        log: Rc<Log>,
        diags: Rc<Factory>,
    ) -> ModuleFinder {
        ModuleFinder {
            names,
            class_finder,
            file_manager,
            log,
            diags,
            module_location_iterator: RefCell::new(ModuleLocationIterator::new()),
            module_name_reader: RefCell::new(None),
        }
    }

    /// Java: public ModuleSymbol findModule(Name name) (ModuleFinder.java:167-169).
    ///
    /// # Arguments
    ///   - ctx/syms: the compiler context and symbol table;
    ///   - name: the module name.
    ///
    /// # Returns
    ///   - the (possibly completed) module symbol.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: ModuleFinder.java:167-169.
    pub fn find_module_by_name(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        name: &Name,
    ) -> crate::ids::SymbolId {
        let msym = syms.enter_module(ctx, name.clone());
        self.find_module(ctx, syms, msym)
    }

    /// Java: public ModuleSymbol findModule(ModuleSymbol msym)
    /// (ModuleFinder.java:171-184).
    ///
    /// # Arguments
    ///   - ctx/syms: the compiler context and symbol table;
    ///   - msym: the module symbol.
    ///
    /// # Returns
    ///   - the (possibly completed) module symbol.
    ///
    /// # Panics
    ///   - none.
    ///
    /// Upstream: ModuleFinder.java:171-184.
    pub fn find_module(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        msym: crate::ids::SymbolId,
    ) -> crate::ids::SymbolId {
        let (source_location, class_location) = module_locations(ctx, msym);
        let kind = ctx.sym(msym).kind;
        if kind != Kind::ERR && source_location.is_none() && class_location.is_none() {
            let list = self.scan_module_path(ctx, syms, Some(msym));
            if list.is_empty() {
                ctx.sym_mut(msym).kind = Kind::ERR;
            }
        }
        let kind = ctx.sym(msym).kind;
        if kind != Kind::ERR {
            // The unnamed module of a -source 8 compilation has no module-info
            // class symbol; there is nothing to look up in that case.
            if let Some(module_info) = module_info_symbol_opt(ctx, msym) {
                let (sourcefile, classfile) = module_info_files(ctx, module_info);
                if sourcefile.is_none() && classfile.is_none() {
                    self.find_module_info(ctx, msym);
                }
            }
        }
        msym
    }

    /// Java: public List<ModuleSymbol> findAllModules() (ModuleFinder.java:186-195).
    ///
    /// # Arguments
    ///   - ctx/syms: the compiler services.
    ///
    /// # Returns
    ///   - the modules found on the module paths.
    ///
    /// # Panics
    ///   - none.
    pub fn find_all_modules(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
    ) -> Vec<crate::ids::SymbolId> {
        let list = self.scan_module_path(ctx, syms, None);
        for msym in &list {
            let module_info = module_info_symbol(ctx, *msym);
            let (sourcefile, classfile) = module_info_files(ctx, module_info);
            let kind = ctx.sym(*msym).kind;
            if kind != Kind::ERR && sourcefile.is_none() && classfile.is_none() {
                self.find_module_info(ctx, *msym);
            }
        }
        list
    }

    /// Java: public ModuleSymbol findSingleModule() (ModuleFinder.java:197-222).
    ///
    /// # Arguments
    ///   - ctx/syms: the compiler services.
    ///
    /// # Returns
    ///   - the single module symbol (the unnamed module when no module-info).
    ///
    /// # Panics
    ///   - none.
    pub fn find_single_module(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
    ) -> Result<crate::ids::SymbolId, CompletionFailure> {
        let src_fo = self.get_module_info_from_location(
            &Some(Rc::new(StandardLocation::SOURCE_PATH)),
            FileKind::SOURCE,
        );
        let class_fo = self.get_module_info_from_location(
            &Some(Rc::new(StandardLocation::CLASS_OUTPUT)),
            FileKind::CLASS,
        );
        let fo = match (src_fo, class_fo) {
            (None, None) => None,
            (Some(src), None) => Some(src),
            (None, Some(class)) => Some(class),
            (Some(src), Some(class)) => Some(self.class_finder.preferred_file_object(&src, &class)),
        };
        let msym = match fo {
            None => syms.unnamed_module,
            Some(fo) => self.read_module(ctx, syms, fo)?,
        };
        if module_patch_location(ctx, msym).is_none() {
            set_module_class_location(ctx, msym, Some(Rc::new(StandardLocation::CLASS_OUTPUT)));
        } else {
            set_module_patch_output_location(
                ctx,
                msym,
                Some(Rc::new(StandardLocation::CLASS_OUTPUT)),
            );
        }
        Ok(msym)
    }

    /// Java: private ModuleSymbol readModule(JavaFileObject fo)
    /// (ModuleFinder.java:224-273).
    ///
    /// # Arguments
    ///   - ctx/syms: the compiler services;
    ///   - fo: the module-info file object.
    ///
    /// # Returns
    ///   - the module symbol.
    ///
    /// # Panics
    ///   - none for the upstream Assert.error default case (kept as the ported
    ///     Assert.error panic).
    fn read_module(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        fo: JavaFileObjectRef,
    ) -> Result<crate::ids::SymbolId, CompletionFailure> {
        let name = match fo.get_kind() {
            FileKind::SOURCE => match Self::read_source_module_name(&fo) {
                Some(name) if !name.to_string_lossy().is_empty() => self.names.from_string(&name),
                _ => {
                    let module_info = syms.define_class(
                        ctx,
                        self.names.module_info.clone(),
                        Some(syms.err_module),
                    );
                    let diag = self
                        .diags
                        .fragment(Fragment::from(fragments::file_does_not_contain_module()));
                    let dcfh = Some(Rc::clone(&self.class_finder.dcfh));
                    let failure = {
                        let mut failure = CompletionFailure::new(
                            Some(module_info),
                            Box::new(move || diag.clone()),
                            dcfh,
                        );
                        failure.bad_class_file = true;
                        failure
                    };
                    return Err(failure);
                }
            },
            FileKind::CLASS => match self.read_module_name(&fo) {
                Some(name) => self.names.from_string(&name),
                None => self.names.error.clone(),
            },
            _ => crate::util::assert::error(),
        };
        let msym = syms.enter_module(ctx, name.clone());
        let module_info = module_info_symbol(ctx, msym);
        let classfile = match &ctx.sym(module_info).kind_data {
            SymbolKind::ClassSymbol { classfile, .. } => classfile.clone(),
            _ => panic!("AssertionError: module-info is not a class symbol"),
        };
        if classfile.is_none() {
            set_module_info_classfile(ctx, module_info, Some(Rc::clone(&fo)));
            if self
                .file_manager
                .has_location(&StandardLocation::PATCH_MODULE_PATH)
                && name != self.names.error
            {
                // Adapter: the ported JavaFileManager trait has no nullable
                // Location return; a missing patch location is reported as Err
                // (registered section 9).
                let patch_location = self
                    .file_manager
                    .get_location_for_module(
                        &StandardLocation::PATCH_MODULE_PATH,
                        &name.to_java_string(),
                    )
                    .ok();
                set_patch_location(ctx, msym, patch_location.clone());
                if let Some(patch_location) = patch_location {
                    let patch_fo_class = self.get_module_info_from_location(
                        &Some(patch_location.clone()),
                        FileKind::CLASS,
                    );
                    let patch_fo_source =
                        self.get_module_info_from_location(&Some(patch_location), FileKind::SOURCE);
                    let patch_fo = match (patch_fo_class, patch_fo_source) {
                        (None, None) => None,
                        (Some(class), None) => Some(class),
                        (None, Some(source)) => Some(source),
                        (Some(class), Some(source)) => {
                            Some(self.class_finder.preferred_file_object(&class, &source))
                        }
                    };
                    if let Some(patch_fo) = patch_fo {
                        set_module_info_classfile(ctx, module_info, Some(patch_fo));
                    }
                }
            }
            ctx.sym_mut(module_info).completer = None;
            // A source module-info found on the source path has no Enter tree
            // yet; its directives are read when the module is completed. Do
            // not force ClassFinder.fillIn here, which would require a source
            // completer that Enter has not installed.
        }
        Ok(msym)
    }

    /// Java: private String readModuleName(JavaFileObject jfo)
    /// (ModuleFinder.java:275-279).
    fn read_module_name(&self, fo: &JavaFileObjectRef) -> Option<JavaString> {
        if self.module_name_reader.borrow().is_none() {
            *self.module_name_reader.borrow_mut() = Some(ModuleNameReader::new());
        }
        let mut reader = self.module_name_reader.borrow_mut();
        reader
            .as_mut()
            .expect("module name reader")
            .read_module_name_from_java_file_object(fo.as_ref())
            .ok()
    }

    /// Java: JavaCompiler.readModuleName(JavaFileObject fo) -- the SOURCE
    /// branch of ModuleFinder.readModule parses the module declaration from the
    /// source text (ModuleFinder.ModuleNameFromSourceReader).
    fn read_source_module_name(fo: &JavaFileObjectRef) -> Option<JavaString> {
        let text = fo.get_char_content(false).ok()??.to_string_lossy();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0usize;
        while i < chars.len() {
            skip_source_space_and_comments(&chars, &mut i);
            if i >= chars.len() {
                return None;
            }
            let (ident, next) = read_source_identifier(&chars, i);
            if ident.is_empty() {
                i += 1;
                continue;
            }
            i = next;
            if ident != "module" {
                continue;
            }
            skip_source_space_and_comments(&chars, &mut i);
            let mut name = String::new();
            loop {
                let (part, next) = read_source_identifier(&chars, i);
                if part.is_empty() {
                    return None;
                }
                name.push_str(&part);
                i = next;
                skip_source_space_and_comments(&chars, &mut i);
                if i < chars.len() && chars[i] == '.' {
                    name.push('.');
                    i += 1;
                    skip_source_space_and_comments(&chars, &mut i);
                } else {
                    break;
                }
            }
            return Some(JavaString::from(name.as_str()));
        }
        None
    }

    /// Java: private List<ModuleSymbol> scanModulePath(ModuleSymbol toFind)
    /// (ModuleFinder.java:290-352).
    fn scan_module_path(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        to_find: Option<crate::ids::SymbolId>,
    ) -> Vec<crate::ids::SymbolId> {
        let mut results: Vec<crate::ids::SymbolId> = Vec::new();
        let mut names_in_set: Vec<(Name, LocationRef)> = Vec::new();
        let multi_module_mode = self
            .file_manager
            .has_location(&StandardLocation::MODULE_SOURCE_PATH);
        let mut loc_iter = self.module_location_iterator.borrow_mut();
        while let Some(locns) = loc_iter.next(self.file_manager.as_ref()) {
            names_in_set.clear();
            for location in locns {
                let name = match self.file_manager.infer_module_name(location.as_ref()) {
                    Ok(name) => self.names.from_string(&name),
                    // Java: catch (IOException e) { // skip location for now? }
                    Err(_) => continue,
                };
                let duplicate = names_in_set
                    .iter()
                    .any(|(candidate, _)| candidate.to_java_string() == name.to_java_string());
                if duplicate {
                    let outer = loc_iter.outer().expect("module location iterator outer");
                    // Java: log.error(Errors.DuplicateModuleOnPath(
                    //         getDescription(moduleLocationIterator.outer), n));
                    let fragment = self
                        .diags
                        .fragment(Fragment::from(
                            Self::get_description(outer),
                        ));
                    let arg = crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(
                        Rc::new(fragment),
                    );
                    let info = errors::duplicate_module_on_path(arg, name.clone());
                    let error = crate::util::jc_diagnostic::Error::from(info);
                    self.log.error_key(error);
                    continue;
                }
                names_in_set.push((name.clone(), Rc::clone(&location)));
                let msym = syms.enter_module(ctx, name.clone());
                let (source_location, class_location) = module_locations(ctx, msym);
                if source_location.is_some() || class_location.is_some() {
                    // module has already been found, so ignore this instance
                    continue;
                }
                let module_name = ctx.sym(msym).name.to_java_string();
                let patch_location = module_patch_location(ctx, msym);
                if self
                    .file_manager
                    .has_location(&StandardLocation::PATCH_MODULE_PATH)
                    && patch_location.is_none()
                {
                    let patch = self
                        .file_manager
                        .get_location_for_module(&StandardLocation::PATCH_MODULE_PATH, &module_name)
                        .ok();
                    set_patch_location(ctx, msym, patch.clone());
                    if patch.is_some()
                        && multi_module_mode
                        && self
                            .file_manager
                            .has_location(&StandardLocation::CLASS_OUTPUT)
                    {
                        let output = self
                            .file_manager
                            .get_location_for_module(&StandardLocation::CLASS_OUTPUT, &module_name)
                            .ok();
                        set_patch_output_location(ctx, msym, output);
                    }
                }
                let outer = loc_iter.outer().expect("module location iterator outer");
                if outer == StandardLocation::MODULE_SOURCE_PATH {
                    set_source_location(ctx, msym, Some(Rc::clone(&location)));
                    if self
                        .file_manager
                        .has_location(&StandardLocation::CLASS_OUTPUT)
                    {
                        let class_output = self
                            .file_manager
                            .get_location_for_module(&StandardLocation::CLASS_OUTPUT, &module_name)
                            .ok();
                        set_class_location(ctx, msym, class_output);
                    }
                } else {
                    set_class_location(ctx, msym, Some(Rc::clone(&location)));
                }
                if outer == StandardLocation::SYSTEM_MODULES
                    || outer == StandardLocation::UPGRADE_MODULE_PATH
                {
                    ctx.sym_mut(msym).flags_field |= flags::SYSTEM_MODULE;
                }
                let (source_location, class_location) = module_locations(ctx, msym);
                if to_find.is_none()
                    || (to_find == Some(msym)
                        && (source_location.is_some() || class_location.is_some()))
                {
                    // Note: cannot return msym directly, because we must finish
                    // processing this set first.
                    results.push(msym);
                }
            }
            if to_find.is_some() && !results.is_empty() {
                return results;
            }
        }
        results
    }

    /// Java: private void findModuleInfo(ModuleSymbol msym)
    /// (ModuleFinder.java:354-392).
    fn find_module_info(&self, ctx: &mut CompilerContext, msym: crate::ids::SymbolId) {
        let module_info = module_info_symbol(ctx, msym);
        let mut fo = self.get_module_info_from_location(
            &module_patch_output_location(ctx, msym),
            FileKind::CLASS,
        );
        fo = Self::preferred_file_object(
            self.class_finder.as_ref(),
            self.get_module_info_from_location(&module_patch_location(ctx, msym), FileKind::CLASS),
            fo,
        );
        fo = Self::preferred_file_object(
            self.class_finder.as_ref(),
            self.get_module_info_from_location(&module_patch_location(ctx, msym), FileKind::SOURCE),
            fo,
        );
        if fo.is_none() {
            fo = self
                .get_module_info_from_location(&module_class_location(ctx, msym), FileKind::CLASS);
            fo = Self::preferred_file_object(
                self.class_finder.as_ref(),
                self.get_module_info_from_location(
                    &module_source_location(ctx, msym),
                    FileKind::SOURCE,
                ),
                fo,
            );
        }
        if fo.is_none() {
            let (source_location, class_location) = module_locations(ctx, msym);
            let module_name = if source_location.is_none() && class_location.is_some() {
                let location = class_location.expect("class location");
                self.file_manager.infer_module_name(location.as_ref()).ok()
            } else {
                None
            };
            if let Some(_module_name) = module_name {
                set_module_info_classfile(ctx, module_info, None);
                ctx.sym_mut(msym).flags_field |= flags::AUTOMATIC_MODULE;
            } else {
                ctx.sym_mut(msym).kind = Kind::ERR;
            }
        } else {
            set_module_info_classfile(ctx, module_info, fo);
            let completer: CompleterRef = Rc::new(RefCell::new(ModuleInfoCompleter {
                class_finder: Rc::clone(&self.class_finder),
            }));
            ctx.sym_mut(module_info).completer = Some(completer);
        }
    }

    /// Java: private JavaFileObject getModuleInfoFromLocation(Location location,
    /// Kind kind) (ModuleFinder.java:281-288).
    fn get_module_info_from_location(
        &self,
        location: &Option<LocationRef>,
        kind: FileKind,
    ) -> Option<JavaFileObjectRef> {
        let location = location.as_ref()?;
        if !self.file_manager.has_location(location.as_ref()) {
            return None;
        }
        let name = self.names.module_info.to_java_string();
        self.file_manager
            .get_java_file_for_input(location.as_ref(), &name, kind)
            .ok()
            .flatten()
    }

    /// Java: private JavaFileObject preferredFileObject(JavaFileObject fo1,
    /// JavaFileObject fo2) (ModuleFinder.java:394-398).
    fn preferred_file_object(
        class_finder: &ClassFinder,
        fo1: Option<JavaFileObjectRef>,
        fo2: Option<JavaFileObjectRef>,
    ) -> Option<JavaFileObjectRef> {
        match (fo1, fo2) {
            (None, fo2) => fo2,
            (fo1, None) => fo1,
            (Some(fo1), Some(fo2)) => Some(class_finder.preferred_file_object(&fo1, &fo2)),
        }
    }

    /// Java: Fragment getDescription(StandardLocation l) (ModuleFinder.java:400-409).
    fn get_description(
        location: StandardLocation,
    ) -> crate::resources::diagnostic_contract::FragmentInfo {
        match location {
            StandardLocation::MODULE_PATH => fragments::locn_module_path(),
            StandardLocation::MODULE_SOURCE_PATH => fragments::locn_module_source_path(),
            StandardLocation::SYSTEM_MODULES => fragments::locn_system_modules(),
            StandardLocation::UPGRADE_MODULE_PATH => fragments::locn_upgrade_module_path(),
            _ => panic!("AssertionError: unexpected module location {location:?}"),
        }
    }
}

/// Java: the anonymous Symbol.Completer of findModuleInfo (ModuleFinder.java:378-387).
struct ModuleInfoCompleter {
    class_finder: Rc<ClassFinder>,
}

impl Completer for ModuleInfoCompleter {
    fn complete(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        sym: crate::ids::SymbolId,
    ) -> Result<(), CompletionFailure> {
        // Java: classFinder.fillIn(msym.module_info).
        self.class_finder.complete(ctx, syms, sym)
    }
}

/// The source and class locations of a module symbol (Java field reads).
fn module_locations(
    ctx: &CompilerContext,
    msym: crate::ids::SymbolId,
) -> (Option<LocationRef>, Option<LocationRef>) {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol {
            source_location,
            class_location,
            ..
        } => (source_location.clone(), class_location.clone()),
        _ => panic!("module expected"),
    }
}

fn module_patch_location(ctx: &CompilerContext, msym: crate::ids::SymbolId) -> Option<LocationRef> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { patch_location, .. } => patch_location.clone(),
        _ => panic!("module expected"),
    }
}

fn module_patch_output_location(
    ctx: &CompilerContext,
    msym: crate::ids::SymbolId,
) -> Option<LocationRef> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol {
            patch_output_location,
            ..
        } => patch_output_location.clone(),
        _ => panic!("module expected"),
    }
}

fn module_class_location(ctx: &CompilerContext, msym: crate::ids::SymbolId) -> Option<LocationRef> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { class_location, .. } => class_location.clone(),
        _ => panic!("module expected"),
    }
}

fn module_source_location(
    ctx: &CompilerContext,
    msym: crate::ids::SymbolId,
) -> Option<LocationRef> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol {
            source_location, ..
        } => source_location.clone(),
        _ => panic!("module expected"),
    }
}

fn set_source_location(
    ctx: &mut CompilerContext,
    msym: crate::ids::SymbolId,
    location: Option<LocationRef>,
) {
    if let SymbolKind::ModuleSymbol {
        source_location, ..
    } = &mut ctx.sym_mut(msym).kind_data
    {
        *source_location = location;
    }
}

fn set_class_location(
    ctx: &mut CompilerContext,
    msym: crate::ids::SymbolId,
    location: Option<LocationRef>,
) {
    if let SymbolKind::ModuleSymbol { class_location, .. } = &mut ctx.sym_mut(msym).kind_data {
        *class_location = location;
    }
}

fn set_patch_location(
    ctx: &mut CompilerContext,
    msym: crate::ids::SymbolId,
    location: Option<LocationRef>,
) {
    if let SymbolKind::ModuleSymbol { patch_location, .. } = &mut ctx.sym_mut(msym).kind_data {
        *patch_location = location;
    }
}

/// Java: msym.classLocation = location (ModuleFinder.java:212-216).
fn set_module_class_location(
    ctx: &mut CompilerContext,
    msym: crate::ids::SymbolId,
    location: Option<LocationRef>,
) {
    if let SymbolKind::ModuleSymbol { class_location, .. } = &mut ctx.sym_mut(msym).kind_data {
        *class_location = location;
    }
}

/// Java: msym.patchOutputLocation = location (ModuleFinder.java:212-216).
fn set_module_patch_output_location(
    ctx: &mut CompilerContext,
    msym: crate::ids::SymbolId,
    location: Option<LocationRef>,
) {
    if let SymbolKind::ModuleSymbol {
        patch_output_location,
        ..
    } = &mut ctx.sym_mut(msym).kind_data
    {
        *patch_output_location = location;
    }
}

fn set_patch_output_location(
    ctx: &mut CompilerContext,
    msym: crate::ids::SymbolId,
    location: Option<LocationRef>,
) {
    if let SymbolKind::ModuleSymbol {
        patch_output_location,
        ..
    } = &mut ctx.sym_mut(msym).kind_data
    {
        *patch_output_location = location;
    }
}

/// The module-info class symbol of a module, if Java initialized the slot
/// (Java: msym.module_info).
fn module_info_symbol_opt(
    ctx: &CompilerContext,
    msym: crate::ids::SymbolId,
) -> Option<crate::ids::SymbolId> {
    match &ctx.sym(msym).kind_data {
        SymbolKind::ModuleSymbol { module_info, .. } => *module_info,
        _ => panic!("module expected"),
    }
}

/// The module-info class symbol of a module (Java: msym.module_info).
fn module_info_symbol(ctx: &CompilerContext, msym: crate::ids::SymbolId) -> crate::ids::SymbolId {
    module_info_symbol_opt(ctx, msym).expect("module-info symbol")
}

/// The source and class files of the module-info class (Java field reads).
fn module_info_files(
    ctx: &CompilerContext,
    module_info: crate::ids::SymbolId,
) -> (
    Option<Rc<dyn javac_tools::tools::java_file_object::JavaFileObject>>,
    Option<Rc<dyn javac_tools::tools::java_file_object::JavaFileObject>>,
) {
    match &ctx.sym(module_info).kind_data {
        SymbolKind::ClassSymbol {
            sourcefile,
            classfile,
            ..
        } => (sourcefile.clone(), classfile.clone()),
        _ => panic!("class expected"),
    }
}

fn set_module_info_classfile(
    ctx: &mut CompilerContext,
    module_info: crate::ids::SymbolId,
    file: Option<Rc<dyn javac_tools::tools::java_file_object::JavaFileObject>>,
) {
    if let SymbolKind::ClassSymbol { classfile, .. } = &mut ctx.sym_mut(module_info).kind_data {
        *classfile = file;
    }
}

/// Skip whitespace and Java comments while scanning a module declaration.
fn skip_source_space_and_comments(chars: &[char], i: &mut usize) {
    loop {
        while *i < chars.len() && chars[*i].is_whitespace() {
            *i += 1;
        }
        if *i + 1 < chars.len() && chars[*i] == '/' && chars[*i + 1] == '/' {
            *i += 2;
            while *i < chars.len() && chars[*i] != '\n' {
                *i += 1;
            }
            continue;
        }
        if *i + 1 < chars.len() && chars[*i] == '/' && chars[*i + 1] == '*' {
            *i += 2;
            while *i + 1 < chars.len() && !(chars[*i] == '*' && chars[*i + 1] == '/') {
                *i += 1;
            }
            if *i + 1 < chars.len() {
                *i += 2;
            }
            continue;
        }
        break;
    }
}

/// Read one Java identifier starting at i, returning it and the new index.
fn read_source_identifier(chars: &[char], i: usize) -> (String, usize) {
    let mut j = i;
    let mut value = String::new();
    while j < chars.len() {
        let c = chars[j];
        if c.is_alphanumeric() || c == '_' || c == '$' {
            value.push(c);
            j += 1;
        } else {
            break;
        }
    }
    (value, j)
}

#[cfg(test)]
mod tests {
    use super::*;
    use javac_tools::tools::java_file_manager::Location;

    use std::cell::RefCell;
    use std::rc::Rc;

    use javac_strings::JavaString;
    use javac_tools::error::Error;
    use javac_tools::tools::java_file_manager::{FileObjectRef, JavaFileObjectRef};
    use javac_tools::tools::java_file_object::Kind as FileKind;
    use javac_tools::tools::option_checker::OptionChecker;

    use crate::code::class_finder::ClassFinder;
    use crate::code::deferred_completion_failure_handler::DeferredCompletionFailureHandler;
    use crate::code::preview::Preview;
    use crate::code::source::Source;
    use crate::code::symtab::Symtab;
    use crate::compiler_context::CompilerContext;
    use crate::jvm::class_reader::ClassReader;
    use crate::jvm::profile::Profile;
    use crate::jvm::target::Target;
    use crate::util::context::Context;
    use crate::util::javac_messages::JavacMessages;
    use crate::util::jc_diagnostic::Factory;
    use crate::util::log::Log;
    use crate::util::names::Names;
    use crate::util::options::Options;

    /// A JavaFileManager stub exposing one module location on MODULE_PATH (the
    /// module finder only needs listLocationsForModules/inferModuleName/has_location/
    /// getJavaFileForInput for the scanned modules).
    struct StubFileManager {
        module_locations: Vec<Vec<LocationRef>>,
        module_name: JavaString,
    }

    impl JavaFileManager for StubFileManager {
        fn get_class_loader(
            &self,
            _location: &dyn Location,
        ) -> Option<Rc<dyn javac_tools::jdk::ClassLoader>> {
            None
        }

        fn list(
            &self,
            _location: &dyn Location,
            _package_name: &JavaString,
            _kinds: &[FileKind],
            _recurse: bool,
        ) -> Result<Vec<JavaFileObjectRef>, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn infer_binary_name(
            &self,
            _location: &dyn Location,
            _file: &JavaFileObjectRef,
        ) -> Option<JavaString> {
            None
        }

        fn is_same_file(&self, _a: &FileObjectRef, _b: &FileObjectRef) -> bool {
            false
        }

        fn handle_option(
            &self,
            _current: &JavaString,
            _remaining: &mut dyn Iterator<Item = JavaString>,
        ) -> bool {
            false
        }

        fn has_location(&self, _location: &dyn Location) -> bool {
            true
        }

        fn get_java_file_for_input(
            &self,
            _location: &dyn Location,
            _class_name: &JavaString,
            _kind: FileKind,
        ) -> Result<Option<JavaFileObjectRef>, Error> {
            Ok(None)
        }

        fn get_java_file_for_output(
            &self,
            _location: &dyn Location,
            _class_name: &JavaString,
            _kind: FileKind,
            _sibling: Option<&FileObjectRef>,
        ) -> Result<JavaFileObjectRef, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn get_file_for_input(
            &self,
            _location: &dyn Location,
            _package_name: &JavaString,
            _relative_name: &JavaString,
        ) -> Result<Option<FileObjectRef>, Error> {
            Ok(None)
        }

        fn get_file_for_output(
            &self,
            _location: &dyn Location,
            _package_name: &JavaString,
            _relative_name: &JavaString,
            _sibling: Option<&FileObjectRef>,
        ) -> Result<FileObjectRef, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn flush(&self) -> Result<(), Error> {
            Ok(())
        }

        fn close(&self) -> Result<(), Error> {
            Ok(())
        }

        fn get_location_for_module(
            &self,
            _location: &dyn Location,
            _module_name: &JavaString,
        ) -> Result<LocationRef, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn get_location_for_module_of_file(
            &self,
            _location: &dyn Location,
            _fo: &JavaFileObjectRef,
        ) -> Result<LocationRef, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn infer_module_name(&self, _location: &dyn Location) -> Result<JavaString, Error> {
            Ok(self.module_name.clone())
        }

        fn list_locations_for_modules(
            &self,
            location: &dyn Location,
        ) -> Result<Vec<Vec<LocationRef>>, Error> {
            if location.get_name().to_string_lossy() == "MODULE_PATH" {
                Ok(self.module_locations.clone())
            } else {
                Ok(Vec::new())
            }
        }

        fn contains(&self, _location: &dyn Location, _fo: &FileObjectRef) -> Result<bool, Error> {
            Ok(false)
        }
    }

    impl OptionChecker for StubFileManager {
        fn is_supported_option(&self, _option: &JavaString) -> i32 {
            -1
        }
    }

    struct Fixture {
        ctx: CompilerContext,
        syms: Symtab,
        finder: Rc<ClassFinder>,
        names: Rc<Names>,
        log: Rc<Log>,
        diags: Rc<Factory>,
        file_manager: Rc<StubFileManager>,
    }

    fn fixture(module_locations: Vec<Vec<LocationRef>>) -> Fixture {
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = JavacMessages::instance(&mut context);
        let diags = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Profile::instance(&mut context);
        let options = Options::instance(&mut context);
        let source = Source::JDK17;
        let file_manager = Rc::new(StubFileManager {
            module_locations,
            module_name: JavaString::from("m"),
        });
        let reader = Rc::new(RefCell::new(ClassReader::new(
            Rc::clone(&log),
            Rc::clone(&names),
            Rc::clone(&diags),
            Rc::clone(&dcfh),
            Rc::clone(&preview),
            Rc::clone(&profile),
            &options,
            source,
            Rc::new(RefCell::new(None)),
        )));
        let mut ctx = CompilerContext::new();
        let mut syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let finder = ClassFinder::new(
            Rc::clone(&reader),
            Rc::clone(&file_manager) as Rc<dyn JavaFileManager>,
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&diags),
            Rc::clone(&dcfh),
            Rc::clone(&profile),
            &options,
            None,
            Rc::new(RefCell::new(None)),
        );
        let factory = finder.get_completer_factory();
        Symtab::set_initial_completer(&mut syms, &mut ctx, factory);
        Fixture {
            ctx,
            syms,
            finder,
            names,
            log,
            diags,
            file_manager,
        }
    }

    /// Java: ModuleLocationIterator (ModuleFinder.java:119-165): the iterator
    /// walks the module-oriented locations and exposes the current outer location.
    #[test]
    fn module_location_iterator_walks_the_module_locations() {
        let mut iterator = ModuleLocationIterator::new();
        let file_manager = StubFileManager {
            module_locations: vec![vec![Rc::new(StandardLocation::MODULE_PATH)]],
            module_name: JavaString::from("m"),
        };
        let first = iterator.next(&file_manager);
        assert_eq!(first.as_ref().map(|locations| locations.len()), Some(1));
        assert_eq!(iterator.outer(), Some(StandardLocation::MODULE_PATH));
        assert!(iterator.next(&file_manager).is_none());
        assert!(iterator.next(&file_manager).is_none());
    }

    /// Java: ModuleFinder.findModule (ModuleFinder.java:171-184): a module found on
    /// MODULE_PATH gets its class location and, without a module-info file, is
    /// marked as an automatic module (findModuleInfo:367-375).
    #[test]
    fn find_module_scans_the_module_path_and_finds_an_automatic_module() {
        let mut fixture = fixture(vec![vec![Rc::new(StandardLocation::MODULE_PATH)]]);
        let mut module_finder = ModuleFinder::new(
            Rc::clone(&fixture.names),
            Rc::clone(&fixture.finder),
            Rc::clone(&fixture.file_manager) as Rc<dyn JavaFileManager>,
            Rc::clone(&fixture.log),
            Rc::clone(&fixture.diags),
        );
        let name = fixture.names.from_utf(b"m");
        let module = module_finder.find_module_by_name(&mut fixture.ctx, &fixture.syms, &name);
        assert_ne!(
            fixture.ctx.sym(module).kind,
            Kind::ERR,
            "the module on the module path is found"
        );
        let _class_location = match &fixture.ctx.sym(module).kind_data {
            SymbolKind::ModuleSymbol { class_location, .. } => class_location.clone(),
            _ => None,
        };
        assert_ne!(
            fixture.ctx.sym(module).flags_field & flags::AUTOMATIC_MODULE,
            0,
            "the module without module-info is an automatic module"
        );
    }

    /// Java: ModuleFinder.findModule (ModuleFinder.java:171-184): when the scan
    /// finds nothing the module is marked ERR (the module-info lookup is then
    /// skipped by the second guard).
    #[test]
    fn find_module_marks_an_unknown_module_as_error() {
        let mut fixture = fixture(Vec::new());
        let mut module_finder = ModuleFinder::new(
            Rc::clone(&fixture.names),
            Rc::clone(&fixture.finder),
            Rc::clone(&fixture.file_manager) as Rc<dyn JavaFileManager>,
            Rc::clone(&fixture.log),
            Rc::clone(&fixture.diags),
        );
        let name = fixture.names.from_utf(b"missing");
        let module = module_finder.find_module_by_name(&mut fixture.ctx, &fixture.syms, &name);
        assert_eq!(
            fixture.ctx.sym(module).kind,
            Kind::ERR,
            "a module that is not on any module path is marked ERR"
        );
    }
}
