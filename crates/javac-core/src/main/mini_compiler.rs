// Source: com/sun/tools/javac/main/JavaCompiler.java (minimal wiring subset)

//! Minimal compiler wiring for the compilation pipeline.
//!
//! PORTING ADAPTER (registered): the 1:1 JavaCompiler.java port is deferred to
//! the batch that ports Modules/TransPatterns/LambdaToMethod/
//! JavacProcessingEnvironment/TaskEvent+TaskListener/MultiTaskListener/
//! DeferredCompletionFailureHandler/JNIWriter. This file wires the same phase
//! sequence (parse -> enter -> attribute -> flow -> transTypes -> lower ->
//! generate -> class writer) over the ported services for sources that use no
//! pattern matching, lambdas, modules or annotation processing; every phase
//! call below is the upstream JavaCompiler call of that phase.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use javac_fs::Path as FsPath;
use javac_tools::tools::java_file_manager::{JavaFileManager, JavaFileObjectRef};
use javac_tools::tools::simple_java_file_object::SimpleJavaFileObject;
use javac_tools::uri::URI;
use javac_tools::tools::standard_java_file_manager::StandardJavaFileManager;
use javac_tools::tools::standard_location::StandardLocation;

use crate::code::deferred_completion_failure_handler::DeferredCompletionFailureHandler;
use crate::code::deferred_lint_handler::DeferredLintHandler;
use crate::code::dependencies::DummyDependencies;
use crate::code::lint::Lint;
use crate::code::preview::Preview;
use crate::code::source::Source;
use crate::code::symtab::Symtab;
use crate::code::types::Types;
use crate::comp::annotate::Annotate;
use crate::comp::attr::Attr;
use crate::comp::check::Check;
use crate::comp::const_fold::ConstFold;
use crate::comp::enter::Enter;
use crate::comp::flow::Flow;
use crate::comp::infer::Infer;
use crate::comp::lower::Lower;
use crate::comp::member_enter::MemberEnter;
use crate::comp::operators::Operators;
use crate::comp::resolve::Resolve;
use crate::comp::todo::Todo;
use crate::comp::trans_types::TransTypes;
use crate::code::type_annotations::TypeAnnotations;
use crate::comp::type_enter::TypeEnter;
use crate::comp::attr_context::AttrContext;
use crate::comp::env::Env;
use crate::comp::type_envs::TypeEnvs;
use crate::compiler_context::CompilerContext;
use crate::file::cache_fs_info::CacheFSInfo;
use crate::file::javac_file_manager::JavacFileManager;
use crate::jvm::class_reader::ClassReader;
use crate::jvm::class_writer::ClassWriter;
use crate::jvm::codegen::Gen;
use crate::jvm::pool_writer::PoolWriter;
use crate::jvm::profile::Profile;
use crate::jvm::target::Target;
use crate::ids::TreeId;
use crate::parser::parser::Parser;
use crate::tree::jc_tree::TreeKind;
use crate::util::abstract_log::AbstractLog;
use crate::util::list::List;
use crate::tree::tree_maker::TreeMaker;
use crate::util::context::Context as UtilContext;
use crate::util::javac_messages::JavacMessages;
use crate::util::jc_diagnostic::Factory;
use crate::util::log::Log;
use crate::util::names::Names;
use crate::util::options::Options;

/// One class file produced by a compilation.
pub struct CompiledClass {
    /// The binary class name.
    pub name: String,
    /// The generated class file bytes.
    pub bytes: Vec<u8>,
}

/// The minimal compiler wiring over the ported compilation phases.
pub struct MiniCompiler {
    /// The compiler context (arena).
    pub ctx: CompilerContext,
    /// The compiler service table.
    pub util_context: UtilContext,
    /// The diagnostic log.
    pub log: Rc<RefCell<Log>>,
    /// The name table.
    pub names: Rc<Names>,
    /// The compiler options.
    pub options: Rc<Options>,
    /// The diagnostic factory.
    pub diags: Rc<Factory>,
    /// The symbol table.
    pub syms: Rc<RefCell<Symtab>>,
    /// The type relations.
    pub types: Rc<Types>,
    /// The lint settings.
    pub lint: Rc<Lint>,
    /// The preview feature service.
    pub preview: Rc<Preview>,
    /// The source language level.
    pub source: Source,
    /// The code generation target.
    pub target: Target,
    /// The target profile.
    pub profile: Rc<Profile>,
    /// The tree factory.
    pub make: Rc<RefCell<TreeMaker>>,
    /// The parser factory.
    pub parser_factory: crate::parser::parser_factory::ParserFactory,
    /// The file manager.
    pub file_manager: Rc<JavacFileManager>,
    /// The deferred lint handler.
    pub deferred_lint_handler: Rc<RefCell<DeferredLintHandler>>,
    /// The deferred completion failure handler.
    pub dcfh: Rc<DeferredCompletionFailureHandler>,
    /// The class reader.
    pub reader: Rc<RefCell<ClassReader>>,
    /// The class finder.
    pub finder: Rc<crate::code::class_finder::ClassFinder>,
    /// The check service.
    pub chk: Rc<RefCell<Check>>,
    /// The enter pass.
    pub enter: Rc<RefCell<Enter>>,
    /// The member enter pass.
    pub member_enter: Rc<RefCell<MemberEnter>>,
    /// The type enter pass.
    pub type_enter: Rc<RefCell<TypeEnter>>,
    /// The type environments.
    pub type_envs: Rc<RefCell<TypeEnvs>>,
    /// The annotation service.
    pub annotate: Rc<Annotate>,
    /// The attribution pass.
    pub attr: Rc<RefCell<Attr>>,
    /// The resolver.
    pub resolve: Rc<RefCell<Resolve>>,
    /// The flow pass.
    pub flow: Rc<Flow>,
    /// The inference engine.
    pub infer: Rc<Infer>,
    /// The constant folder.
    pub cfolder: Rc<ConstFold>,
    /// The operators.
    pub operators: Rc<RefCell<Option<Rc<Operators>>>>,
    /// The trans types pass.
    pub trans_types: Rc<RefCell<TransTypes>>,
    /// The lower pass.
    pub lower: Rc<RefCell<Lower>>,
    /// The code generator.
    pub generator: Rc<RefCell<Gen>>,
    /// The class writer.
    pub writer: Rc<RefCell<ClassWriter>>,
    /// The constant pool writer.
    pub pool_writer: Rc<RefCell<PoolWriter>>,
    /// The todo list.
    pub todo: Rc<RefCell<Todo>>,
    /// The output directory of the class files.
    pub out_dir: PathBuf,
    /// The JDK home directory.
    pub java_home: PathBuf,
    /// The class files generated by the last phase run.
    pub generated_classes: Vec<CompiledClass>,
}

impl MiniCompiler {
    /// Create the compiler services.
    ///
    /// # Arguments
    ///   - out_dir: the directory the class files are written to.
    ///
    /// # Returns
    ///   - the wired compiler.
    ///
    /// # Panics
    ///   - a service invariant is violated.
    pub fn new(out_dir: &Path) -> MiniCompiler {
        let mut util_context = UtilContext::new();
        let messages = JavacMessages::instance(&mut util_context);
        let diags = Rc::new(Factory::new(messages, "compiler"));
        let log = Log::instance(&mut util_context);
        let names = Rc::new(Names::new(false));
        util_context.put(Rc::clone(&names));
        let options = Options::instance(&mut util_context);
        let lint = Lint::instance(&mut util_context);
        let preview = Preview::instance(&mut util_context);
        let source = Source::JDK17;
        let target = Target::JDK1_17;
        let profile = Profile::instance(&mut util_context);
        let mut ctx = CompilerContext::new();
        let syms_value = Symtab::new(&mut ctx, Rc::clone(&names), target, source);
        let syms = Rc::new(RefCell::new(syms_value));
        let types = Rc::new(Types::with_names(Rc::clone(&names)));
        let make = TreeMaker::instance(&mut util_context);
        let deferred_lint_handler = Rc::new(RefCell::new(DeferredLintHandler::new()));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let fs_info = Rc::new(CacheFSInfo::new());
        let file_manager = JavacFileManager::new(
            None,
            java_home(),
            Rc::clone(&log),
            Rc::clone(&options),
            fs_info,
        );
        let class_out = StandardLocation::location_for(&js("CLASS_OUTPUT"));
        file_manager
            .set_location(&*class_out, Some(&[out_dir.to_path_buf()]))
            .expect("cannot set CLASS_OUTPUT");
        let annotate_cell: Rc<RefCell<Option<Rc<Annotate>>>> = Rc::new(RefCell::new(None));
        let reader = Rc::new(RefCell::new(ClassReader::new(
            Rc::clone(&log),
            Rc::clone(&names),
            Rc::clone(&diags),
            Rc::clone(&dcfh),
            Rc::clone(&preview),
            Rc::clone(&profile),
            &options,
            source,
            Rc::clone(&annotate_cell),
        )));
        let finder = crate::code::class_finder::ClassFinder::new(
            Rc::clone(&reader),
            Rc::clone(&file_manager) as Rc<dyn JavaFileManager>,
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&diags),
            Rc::clone(&dcfh),
            Rc::clone(&profile),
            &options,
            None,
        );
        let java_home_path = java_home().as_path().to_path_buf();
        let todo = Todo::instance(&mut util_context);
        self_set_initial_completer(&mut ctx, &syms, &finder);
        let chk = Rc::new(RefCell::new(Check::new(
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&diags),
            source,
            char::from_u32(target.synthetic_name_char() as u32).expect("synthetic name char"),
            Rc::clone(&types),
            Rc::clone(&lint),
            Rc::clone(&deferred_lint_handler),
            Rc::clone(&syms),
        )));
        crate::comp::check::attach_self(&chk);
        let type_envs = Rc::new(RefCell::new(TypeEnvs::new()));
        let enter = Rc::new(RefCell::new(Enter::new(
            Rc::clone(&log),
            Rc::clone(&names),
            Rc::clone(&diags),
            Rc::clone(&chk),
            Rc::clone(&types),
            Some(Rc::clone(&lint)),
            Rc::clone(&file_manager) as Rc<dyn JavaFileManager>,
            &options,
            Rc::clone(&todo),
            Rc::clone(&type_envs)
        )));
        let member_enter = Rc::new(RefCell::new(MemberEnter::new(
            Rc::clone(&enter),
            Rc::clone(&log),
            Rc::clone(&chk),
            Rc::clone(&types),
            Rc::clone(&deferred_lint_handler),
            Rc::clone(&annotate_cell),
        )));
        let dependencies: Rc<RefCell<dyn crate::code::dependencies::Dependencies>> =
            Rc::new(RefCell::new(DummyDependencies));
        let type_enter = Rc::new(RefCell::new(TypeEnter::new(
            Rc::clone(&names),
            Rc::clone(&enter),
            Rc::clone(&member_enter),
            Rc::clone(&log),
            Rc::clone(&chk),
            Rc::clone(&types),
            Rc::clone(&syms),
            Rc::clone(&make),
            Rc::clone(&deferred_lint_handler),
            Some(Rc::clone(&lint)),
            Rc::clone(&type_envs),
            Rc::clone(&dependencies),
            Rc::clone(&annotate_cell),
            Rc::clone(&todo),
        )));
        *enter.borrow().type_enter.borrow_mut() = Some(Rc::clone(&type_enter));
        let infer = Infer::new(
            &mut ctx,
            crate::code::source::Feature::GRAPH_INFERENCE.allowed_in_source(source),
            Rc::clone(&diags),
            Rc::clone(&names),
        );
        let cfolder = Rc::new(ConstFold::new());
        let basic_handler = Rc::clone(&chk.borrow().basic_handler);
        let attr = Rc::new(RefCell::new(Attr::new(
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&chk),
            Rc::clone(&member_enter),
            Rc::clone(&type_enter),
            Rc::clone(&make),
            Rc::clone(&enter),
            Rc::clone(&types),
            Rc::clone(&diags),
            Rc::clone(&deferred_lint_handler),
            Rc::clone(&type_envs),
            Rc::clone(&dependencies),
            basic_handler,
            Rc::clone(&syms),
            Rc::clone(&infer),
            source,
            &options,
            &preview,
        )));
        crate::comp::attr::wire_attr_services(
            &mut ctx,
            Rc::clone(&attr),
            Rc::clone(&options),
            source,
            Some(Rc::clone(&finder)),
        );
        let resolve = attr
            .borrow()
            .rs
            .borrow()
            .clone()
            .expect("resolve service installed");
        util_context.put::<RefCell<Resolve>>(Rc::clone(&resolve));
        let operators = Rc::clone(&attr.borrow().operators);
        let flow = attr
            .borrow()
            .flow
            .borrow()
            .clone()
            .expect("flow service installed");
        let annotate = attr
            .borrow()
            .annotate
            .borrow()
            .clone()
            .expect("annotate service installed");
        *enter.borrow().annotate.borrow_mut() = Some(Rc::clone(&annotate));
        util_context.put::<Annotate>(Rc::clone(&annotate));
        let type_annotations = Rc::new(TypeAnnotations::new(
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&chk),
            Rc::clone(&attr),
            Rc::clone(&annotate),
            Rc::clone(&types),
            Rc::clone(&syms),
            Rc::clone(&diags),
        ));
        type_enter.borrow().set_attr(Rc::clone(&attr));
        type_enter
            .borrow()
            .set_type_annotations(Rc::clone(&type_annotations));
        let trans_types = Rc::new(RefCell::new(TransTypes::new(
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&syms),
            Rc::clone(&make),
            Rc::clone(&enter),
            Rc::clone(&types),
            Rc::clone(&annotate),
            Rc::clone(&attr),
            Rc::clone(&resolve),
            Rc::new(RefCell::new(crate::comp::compile_states::CompileStates::new())),
            Rc::clone(&diags),
            Rc::clone(&chk),
            crate::code::source::Feature::GRAPH_INFERENCE.allowed_in_source(source),
            true,
        )));
        let lower = Rc::new(RefCell::new(Lower::new(
            Rc::clone(&names),
            Rc::clone(&log),
            Rc::clone(&syms),
            Rc::clone(&resolve),
            Rc::clone(&operators),
            Rc::clone(&chk),
            Rc::clone(&attr),
            Rc::clone(&make),
            Rc::clone(&cfolder),
            target,
            Rc::clone(&type_envs),
            Rc::clone(&types),
            Rc::clone(&infer),
            crate::main::option::PkgInfo::LEGACY,
            options.is_set("debug.lower"),
            options.is_set("disableProtectedAccessors"),
        )));
        let pool_writer = Rc::new(RefCell::new(PoolWriter::new(
            Rc::clone(&types),
            Rc::clone(&names),
        )));
        let generator = Rc::new(RefCell::new(Gen::new(
            &mut ctx,
            Rc::clone(&log),
            Rc::clone(&syms),
            Rc::clone(&chk),
            Rc::clone(&resolve),
            Rc::clone(&make),
            Rc::clone(&names),
            target,
            Rc::clone(&types),
            Rc::clone(&annotate),
            Rc::clone(&attr),
            Rc::clone(&infer),
            Rc::clone(&pool_writer),
            &options,
        )));
        let method_codes = Rc::clone(&generator.borrow().method_codes);
        let writer = Rc::new(RefCell::new(ClassWriter::new(
            Rc::clone(&log),
            Rc::clone(&names),
            Rc::clone(&syms),
            Rc::clone(&types),
            Rc::clone(&chk),
            Rc::clone(&options),
            target,
            Some(source),
            Rc::clone(&preview),
            Rc::clone(&pool_writer),
            method_codes,
        )));
        let mut parser_context = UtilContext::new();
        let scanner_factory = crate::parser::scanner_factory::ScannerFactory::new(
            &mut parser_context,
            Rc::clone(&names),
            source,
        );
        let parser_factory = crate::parser::parser_factory::ParserFactory::new(
            &mut parser_context,
            Rc::clone(&names),
            source,
            scanner_factory,
            None,
        );
        let mut compiler = MiniCompiler {
            ctx,
            util_context,
            log,
            names,
            options,
            diags,
            syms,
            types,
            lint,
            preview,
            source,
            target,
            profile,
            make,
            parser_factory,
            file_manager,
            deferred_lint_handler,
            dcfh,
            reader,
            finder,
            chk,
            enter,
            member_enter,
            type_enter,
            type_envs,
            annotate,
            attr,
            resolve,
            flow,
            infer,
            cfolder,
            operators,
            trans_types,
            lower,
            generator,
            writer,
            pool_writer,
            todo,
            out_dir: out_dir.to_path_buf(),
            java_home: java_home_path,
            generated_classes: Vec::new(),
        };
        compiler.bootstrap_modules();
        compiler
    }

    /// Install the module bootstrap that Modules.java performs for a
    /// non-modular compilation: the predefined classes live in the java.base
    /// module, whose class location is the SYSTEM_MODULES[java.base] location;
    /// the unnamed module reads every java.base package and its class location
    /// is CLASS_PATH (Modules.java:440-500; Modules is not ported yet, so this
    /// is the registered adapter of that bootstrap).
    fn bootstrap_modules(&mut self) {
        let java_base = self.syms.borrow().java_base;
        let unnamed = self.syms.borrow().unnamed_module;
        let system_modules = StandardLocation::location_for(&js("SYSTEM_MODULES"));
        let module_location_result = self
            .file_manager
            .get_location_for_module(&*system_modules, &js("java.base"));
        if let Ok(module_location) = module_location_result
            && let crate::code::symbol::SymbolKind::ModuleSymbol {
                class_location, ..
            } = &mut self.ctx.sym_mut(java_base).kind_data
        {
            *class_location = Some(module_location);
        }
        let source_path = StandardLocation::location_for(&js("SOURCE_PATH"));
        let class_path = StandardLocation::location_for(&js("CLASS_PATH"));
        if let crate::code::symbol::SymbolKind::ModuleSymbol {
            source_location,
            class_location,
            ..
        } = &mut self.ctx.sym_mut(unnamed).kind_data
        {
            *source_location = Some(source_path);
            *class_location = Some(class_path);
        }
        // Java: the unnamed module's visible packages are all the packages of
        // the system modules it reads; for the minimal non-modular compilation
        // the java.base packages are entered and mapped (Modules.java:440-500).
        let modules_file = self.java_home.join("lib").join("modules");
        if let Ok(image) = javac_jimage::JImage::open(&modules_file) {
            let packages_result = image.packages_of_module("java.base");            if let Ok(packages) = packages_result {
                for package in packages {
                    let name = self
                        .names
                        .from_string(&javac_strings::JavaString::from(package.as_str()));
                    let package_symbol = self
                        .syms
                        .borrow_mut()
                        .enter_package(&mut self.ctx, java_base, name);
                    // Java: Modules bootstrap records every module package in the
                    // module's visiblePackages (the module reads its own packages).
                    self.syms.borrow_mut().add_visible_package(
                        &mut self.ctx,
                        java_base,
                        package_symbol,
                    );
                    self.syms.borrow_mut().add_visible_package(
                        &mut self.ctx,
                        unnamed,
                        package_symbol,
                    );
                }
            }
        }
    }
}

/// Return the JDK home directory used by the file manager.
fn java_home() -> FsPath {
    match std::env::var_os("JAVA_HOME") {
        Some(home) => FsPath::from_path_buf(PathBuf::from(home)),
        None => FsPath::from_path_buf(std::env::temp_dir()),
    }
}

/// Convert a string literal to a Java string.
fn js(value: &str) -> javac_strings::JavaString {
    javac_strings::JavaString::from(value)
}

impl MiniCompiler {
    /// Compile a list of source files through the ported phases.
    ///
    /// # Arguments
    ///   - source_paths: the Java source files to compile.
    ///
    /// # Returns
    ///   - the compiled classes, or an error message when the compilation
    ///     reported errors.
    ///
    /// # Panics
    ///   - a phase invariant is violated.
    pub fn compile_files(
        &mut self,
        source_paths: &[PathBuf],
    ) -> Result<Vec<CompiledClass>, String> {
        // JavaCompiler.parseFiles
        let mut roots: Vec<TreeId> = Vec::new();
        for path in source_paths {
            roots.push(self.parse_file(path)?);
        }
        // JavaCompiler.enterTrees -> InitModules is not ported (single module).
        let root_list = List::from_iterable(roots.iter().copied());
        let queued = {
            let syms = Rc::clone(&self.syms);
            let mut syms = syms.borrow_mut();
            self.enter
                .borrow_mut()
                .main(&mut self.ctx, &mut syms, root_list.clone())
        };
        for clazz in queued {
            let syms = Rc::clone(&self.syms);
            let mut syms = syms.borrow_mut();
            self.type_enter
                .borrow_mut()
                .complete(&mut self.ctx, &mut syms, clazz);
        }
        {
            let syms = Rc::clone(&self.syms);
            let mut syms = syms.borrow_mut();
            self.type_enter
                .borrow_mut()
                .ensure_imports_checked(&mut self.ctx, &mut syms, root_list);
        }
        // JavaCompiler.attribute(todo)
        let mut envs: Vec<Rc<Env<AttrContext>>> = Vec::new();
        loop {
            let env = self.todo.borrow_mut().poll(&self.ctx);
            match env {
                Some(env) => envs.push(env),
                None => break,
            }
        }
        for env in &envs {
            self.attribute(env)?;
        }
        if self.error_count() > 0 {
            return Err(self.error_message());
        }
        // JavaCompiler.flow(envs)
        for env in &envs {
            let make = Rc::clone(&self.make);
            self.flow.analyze_tree(&mut self.ctx, env, &make);
        }
        if self.error_count() > 0 {
            return Err(self.error_message());
        }
        // JavaCompiler.desugar(envs): TransTypes then Lower.
        let mut classes: Vec<CompiledClass> = Vec::new();
        for env in envs {
            let cdef = env.encl_class.expect("env.enclClass");
            let local_make = Rc::new(RefCell::new(
                self.make.borrow().for_toplevel(env.toplevel),
            ));
            let translated = self
                .trans_types
                .borrow_mut()
                .translate_top_level_class(&mut self.ctx, cdef, Rc::clone(&local_make))
                .expect("transTypes result");
            let translated = match &mut self.ctx.tree_mut(cdef).kind {
                TreeKind::ClassDecl { .. } => translated,
                _ => translated,
            };
            let defs = self.lower.borrow_mut().translate_top_level_class(
                &mut self.ctx,
                Rc::clone(&env),
                translated,
                Rc::clone(&local_make),
            );
            if self.error_count() > 0 {
                return Err(self.error_message());
            }
            // JavaCompiler.generate(queue)
            let mut current = defs;
            while current.non_empty() {
                let classdef = *current.head();
                current = current.tail();
                self.generate_class(&env, classdef)?;
            }
            let mut generated = self.take_generated_classes()?;
            classes.append(&mut generated);
        }
        if self.error_count() > 0 {
            return Err(self.error_message());
        }
        Ok(classes)
    }

    /// Parse one source file into a compilation unit.
    fn parse_file(&mut self, path: &Path) -> Result<TreeId, String> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|error| format!("cannot resolve the current directory: {error}"))?
                .join(path)
        };
        let content = std::fs::read_to_string(&absolute)
            .map_err(|error| format!("cannot read {}: {error}", absolute.display()))?;
        let uri_text = format!("file:///{}", absolute.to_string_lossy().replace('\\', "/"));
        let sourcefile: JavaFileObjectRef = Rc::new(SimpleJavaFileObject::new(
            URI::create(&javac_strings::JavaString::from(uri_text.as_str())),
            javac_tools::tools::java_file_object::Kind::SOURCE,
        ));
        let prev_source = self.log.borrow_mut().use_source(Some(Rc::clone(&sourcefile)));
        let result = {
            let mut parser = self.parser_factory.new_parser_full(
                &mut self.ctx,
                &javac_strings::JavaString::from(content.as_str()),
                false,
                false,
                false,
                false,
            );
            parser.parse_compilation_unit(&mut self.ctx)
        };
        self.log.borrow_mut().use_source(prev_source);
        // JavaCompiler.parse: tree.sourcefile = filename
        if let TreeKind::CompilationUnit { sourcefile: slot, .. } =
            &mut self.ctx.tree_mut(result).kind
        {
            *slot = Some(Rc::clone(&sourcefile));
        }
        // JavaCompiler.initModules -> Modules sets tree.modle to the default
        // module; for a non-modular compilation that is the unnamed module
        // (Modules.java:440-500; registered adapter).
        let unnamed = self.syms.borrow().unnamed_module;
        if let TreeKind::CompilationUnit { modle: slot, .. } = &mut self.ctx.tree_mut(result).kind {
            *slot = Some(unnamed);
        }
        Ok(result)
    }

    /// JavaCompiler.attribute(Env<AttrContext> env).
    fn attribute(&mut self, env: &Rc<Env<AttrContext>>) -> Result<(), String> {
        let sourcefile = env
            .encl_class
            .and_then(|encl| match &self.ctx.tree(encl).kind {
                TreeKind::ClassDecl { sym: Some(sym), .. } => {
                    match &self.ctx.sym(*sym).kind_data {
                        crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => {
                            sourcefile.clone()
                        }
                        _ => None,
                    }
                }
                _ => None,
            })
            .or_else(|| {
                env.toplevel.and_then(|toplevel| match &self.ctx.tree(toplevel).kind {
                    TreeKind::CompilationUnit { sourcefile, .. } => sourcefile.clone(),
                    _ => None,
                })
            });
        let prev_source = self.log.borrow_mut().use_source(sourcefile);
        let make = Rc::clone(&self.make);
        make.borrow_mut().at(crate::util::position::FIRSTPOS);
        let local_make = Rc::new(RefCell::new(
            self.make.borrow().for_toplevel(env.toplevel),
        ));
        let _ = local_make;
        {
            let syms = Rc::clone(&self.syms);
            let mut syms = syms.borrow_mut();
            self.attr
                .borrow_mut()
                .attrib(&mut self.ctx, &mut syms, env);
        }
        self.log.borrow_mut().use_source(prev_source);
        Ok(())
    }

    /// JavaCompiler.generate: emit the class file of one class definition.
    fn generate_class(
        &mut self,
        env: &Rc<Env<AttrContext>>,
        classdef: TreeId,
    ) -> Result<(), String> {
        let c = match &self.ctx.tree(classdef).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => return Err("class definition without a symbol".to_string()),
        };
        self.generator
            .borrow_mut()
            .gen_class(&mut self.ctx, env, classdef);
        if self.error_count() > 0 {
            return Err(self.error_message());
        }
        let (name, bytes) = self
            .writer
            .borrow_mut()
            .write_class(&mut self.ctx, c)
            .map_err(|error| format!("class writer error: {error:?}"))?;
        self.write_class_file(&name, &bytes)?;
        self.generated_classes.push(CompiledClass { name, bytes });
        Ok(())
    }

    /// Write one class file through the file manager output API.
    fn write_class_file(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let class_out = StandardLocation::location_for(&js("CLASS_OUTPUT"));
        let file = self
            .file_manager
            .get_java_file_for_output(
                &*class_out,
                &js(name),
                javac_tools::tools::java_file_object::Kind::CLASS,
                None,
            )
            .map_err(|error| format!("cannot get the output file for {name}: {error:?}"))?;
        let mut stream = file
            .open_output_stream()
            .map_err(|error| format!("cannot open the output file for {name}: {error:?}"))?;
        std::io::Write::write_all(&mut stream, bytes)
            .map_err(|error| format!("cannot write the class file {name}: {error}"))?;
        Ok(())
    }

    /// Return the number of errors reported so far.
    pub fn error_count(&self) -> i32 {
        self.log.borrow().nerrors
    }

    /// Return the diagnostic summary of the current compilation.
    fn error_message(&self) -> String {
        let log = self.log.borrow();
        format!("compilation failed with {} error(s)", log.nerrors)
    }

    /// Take the classes generated since the previous call.
    fn take_generated_classes(&mut self) -> Result<Vec<CompiledClass>, String> {
        let generated = std::mem::take(&mut self.generated_classes);
        Ok(generated)
    }
}

/// Install the ClassFinder completer on every symbol created by Symtab::new
/// (upstream Symtab binds initialCompleter from ClassFinder at construction;
/// the Rust bootstrap creates the ClassFinder after Symtab, registered adapter).
fn self_set_initial_completer(
    ctx: &mut CompilerContext,
    syms: &Rc<RefCell<Symtab>>,
    finder: &Rc<crate::code::class_finder::ClassFinder>,
) {
    syms.borrow_mut()
        .set_initial_completer(ctx, finder.get_completer_factory());
}
