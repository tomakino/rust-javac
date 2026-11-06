
//! IDE analysis engine: a resident session that parses and attributes
//! individual source files without running the full compiler pipeline.
//!
//! This crate is the new architecture. It does not modify the compiler: the
//! build pipeline in javac-core stays untouched. The session keeps the
//! compiler services alive across requests and reuses the module graph; only
//! the per-file state (parsed trees, user symbols, diagnostics) is reset.
//!
//! The first analyze call runs the normal compiler once (cold module/symbol
//! setup). Later calls parse, enter and attribute the file directly, clean up
//! the previous round's user classes, and skip code generation.

mod diagnostics;
mod doc_render;
mod features;
mod host;
mod index;
mod inspections;
mod jdk_docs;
mod jdk_sources;
mod positions;
mod symbols;
mod signatures;
mod type_names;

pub use host::*;
pub use index::*;
pub use positions::*;
pub use symbols::*;

use symbols::TracedClass;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::code::scope::LookupKind;
use crate::code::symbol::{symbol_packge, SymbolKind};
use crate::ids::{SymbolId, TreeId};
use crate::main::mini_compiler::MiniCompiler;
use crate::parser::parser::Parser;
use crate::tree::jc_tree::TreeKind;
use crate::util::list::List;
use crate::util::name::Name;
use javac_strings::JavaString;
use javac_tools::tools::java_file_object::Kind;
use javac_tools::tools::standard_java_file_manager::StandardJavaFileManager;

/// Configuration for an analysis session.
#[derive(Debug, Clone)]
pub struct AnalysisConfig {
    /// Class path entries (project output, jars, module output).
    pub class_path: Vec<PathBuf>,
    /// Directory that receives the (optional) class files.
    pub output_dir: PathBuf,
    /// Source language level, e.g. "17".
    pub source_level: String,
    /// Source roots used to resolve project classes that are not yet compiled.
    pub source_path: Vec<PathBuf>,
}

/// Result of analyzing one source file.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub errors: i32,
    pub warnings: i32,
    pub ok: bool,
}

/// Result cached for an unchanged file (mtime + length identify the version).
struct CachedFile {
    path: PathBuf,
    mtime: Option<std::time::SystemTime>,
    len: u64,
    result: AnalysisResult,
}

/// A resident analysis session.
pub struct AnalysisSession {
    pub(crate) config: AnalysisConfig,
    pub(crate) compiler: MiniCompiler,
    pub(crate) initialized: bool,
    pub(crate) traced: Vec<TracedClass>,
    pub(crate) cache: Option<CachedFile>,
    pub(crate) texts: HashMap<PathBuf, String>,
    pub(crate) index: HashMap<String, IndexEntry>,
    pub(crate) index_built: bool,
    /// Lazily built Javadoc cache over the JDK src.zip.
    pub(crate) jdk_docs: Option<jdk_docs::JdkDocCache>,
    /// Last successfully computed inlay hints per file, used to keep hints
    /// stable while the buffer is temporarily unparsable.
    pub(crate) inlay_hint_cache:
        HashMap<PathBuf, Vec<(javac_analysis_api::InlayHint, String)>>,
    /// Lazily built short-name catalog for "new" class completion.
    pub(crate) class_short_names:
        Option<std::collections::BTreeMap<String, Vec<crate::ids::SymbolId>>>,
    /// Whether "new" completion expands public constructors. IDEA exposes this
    /// as a completion setting; the default keeps the previous behaviour.
    pub(crate) completion_show_constructors: bool,

    /// True when receiver completion consumed the typed prefix as part of a
    /// fully qualified class name ("java.util.Base64"), so the prefix must
    /// not be used for filtering or replacement.
    pub(crate) receiver_prefix_consumed: bool,
}

impl AnalysisSession {
    /// Read-only access to the configuration this session was created with.
pub fn config(&self) -> &AnalysisConfig {
    &self.config
}

    /// Toggle IDEA-style constructor expansion for `new` completion.
    pub fn set_completion_show_constructors(&mut self, enabled: bool) {
        self.completion_show_constructors = enabled;
    }

pub fn new(config: AnalysisConfig) -> Self {
        std::fs::create_dir_all(&config.output_dir)
            .expect("create analysis output directory");
        let mut compiler = build_compiler(&config);
        // The IDE reports diagnostics through AnalysisHost; keep the
        // compiler's default writer from polluting the server stderr.
        compiler.log.set_writers(std::rc::Rc::new(std::cell::RefCell::new(
            Box::new(NullWriter) as Box<dyn std::io::Write>,
        )));
        Self {
            config,
            compiler,
            initialized: false,
            traced: Vec::new(),
            cache: None,
            texts: HashMap::new(),
            index: HashMap::new(),
            index_built: false,
            jdk_docs: None,
            inlay_hint_cache: HashMap::new(),
            class_short_names: None,
            completion_show_constructors: false,
            receiver_prefix_consumed: false,
        }
    }

    fn rebuild(&mut self) {
        self.compiler = build_compiler(&self.config);
        self.initialized = false;
        self.traced.clear();
        self.jdk_docs = None;
        self.inlay_hint_cache.clear();
        self.class_short_names = None;
    }

    /// Analyze one source file with the project class path.
    ///
    /// The first call initializes the module graph through the compiler
    /// driver; later calls reuse it. Code generation is skipped.
    pub fn analyze(&mut self, source: &Path) -> AnalysisResult {
        self.compiler.log.take_diagnostics();
        // Unchanged file: answer from the cache (an edit invalidates it by
        // mtime/length, so a resident IDE session does not re-analyze on every
        // keystroke).
        if let Some(cached) = &self.cache
            && cached.path == source
            && cached.mtime == file_mtime(source)
            && cached.len == file_len(source)
        {
            return cached.result.clone();
        }
        // Edited file: reuse the compiler and drop the context-dependent type
        // caches, so the next round sees the new symbols.
        self.class_short_names = None;
        if self.initialized {
            self.compiler.types.reset_caches();
        }
        if !self.initialized {
            let _ = self.compiler.compile_files(&[source.to_path_buf()]);
            self.initialized = true;
            self.trace_classes_for(source);
            if !self.index_built {
                self.build_index();
                self.index_built = true;
            }
            self.trace_classes_for(source);
            let result = self.result();
            self.cache = Some(CachedFile {
                path: source.to_path_buf(),
                mtime: file_mtime(source),
                len: file_len(source),
                result: result.clone(),
            });
            return result;
        }

        // New analysis round: clear the compiled-class table and the
        // per-request diagnostic counters, then drop the previous user classes.
        self.compiler.chk.new_round();
        self.compiler.log.nerrors.set(0);
        self.compiler.log.nwarnings.set(0);
        self.remove_traced_classes();
        let Some(tree) = self.parse_source(source) else {
            return self.result();
        };
        // Attach the tree to the class-path default module. The module graph
        // itself was built by the first compilation.
        {
            let module = self.compiler.syms.unnamed_module;
            if let TreeKind::CompilationUnit { modle, .. } =
                &mut self.compiler.ctx.tree_mut(tree).kind
            {
                *modle = Some(module);
            }
        }
        let roots = List::from_iterable([tree].into_iter());
        {
            let syms = Rc::clone(&self.compiler.syms);
            let syms_ref = &*syms;
            let _ = self
                .compiler
                .enter
                .main(&mut self.compiler.ctx, syms_ref, roots);
        }
        loop {
            let env = self.compiler.todo.poll(&self.compiler.ctx);
            let Some(env) = env else { break };
            let syms = Rc::clone(&self.compiler.syms);
            let syms_ref = &*syms;
            self.compiler
                .attr
                .borrow_mut()
                .attrib(&mut self.compiler.ctx, syms_ref, &env);
        }
        self.trace_classes_for(source);
        self.update_index_for(source);
        let result = self.result();
        self.cache = Some(CachedFile {
            path: source.to_path_buf(),
            mtime: file_mtime(source),
            len: file_len(source),
            result: result.clone(),
        });
        result
    }

    fn result(&self) -> AnalysisResult {
        let errors = self.compiler.log.nerrors.get();
        let warnings = self.compiler.log.nwarnings.get();
        AnalysisResult {
            errors,
            warnings,
            ok: errors == 0,
        }
    }

    fn parse_source(&mut self, path: &Path) -> Option<TreeId> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().ok()?.join(path)
        };
        let overlay = self
            .compiler
            .overlay_text
            .as_ref()
            .filter(|(path, _)| {
                if path == &absolute {
                    return true;
                }
                match std::fs::canonicalize(&absolute) {
                    Ok(canonical) => path == &canonical,
                    Err(_) => false,
                }
            })
            .map(|(_, text)| text.clone());
        let (sourcefile, content): (
            Rc<dyn javac_tools::tools::java_file_object::JavaFileObject>,
            JavaString,
        ) = match overlay {
            Some(text) => {
                let name = JavaString::from(absolute.to_string_lossy().as_ref());
                let object: Rc<dyn javac_tools::tools::java_file_object::JavaFileObject> =
                    Rc::new(crate::file::overlay_file_object::OverlayFileObject::new(
                        name,
                        JavaString::from(text.as_str()),
                    ));
                (object, JavaString::from(text.as_str()))
            }
            None => {
                let sourcefile = self
                    .compiler
                    .file_manager
                    .get_java_file_objects_from_files(&[absolute.clone()])
                    .into_iter()
                    .next()?;
                let content = sourcefile
                    .get_char_content(false)
                    .ok()
                    .and_then(|content| content)?;
                (sourcefile, content)
            }
        };
        let module_info =
            sourcefile.is_name_compatible(&JavaString::from("module-info"), Kind::SOURCE);
        let mut parser = self.compiler.parser_factory.new_parser_full(
            &mut self.compiler.ctx,
            &content,
            true,
            true,
            false,
            module_info,
        );
        let tree = parser.parse_compilation_unit(&mut self.compiler.ctx);
        if let TreeKind::CompilationUnit {
            sourcefile: slot, ..
        } = &mut self.compiler.ctx.tree_mut(tree).kind
        {
            *slot = Some(Rc::clone(&sourcefile));
        }
        Some(tree)
    }

}

fn build_compiler(config: &AnalysisConfig) -> MiniCompiler {
    let class_path = config
        .class_path
        .iter()
        .map(|entry| entry.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(";");
    let source_level = config.source_level.clone();
    let source_path = config
        .source_path
        .iter()
        .map(|entry| entry.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(";");
    MiniCompiler::new_with_options(&config.output_dir, |options| {
        if !class_path.is_empty() {
            options.put("--class-path", class_path.clone());
        }
        options.put("--source", source_level.clone());
        if !source_path.is_empty() {
            options.put("--source-path", source_path);
        }
        options.put("should-stop.at", "ATTR");
        // Read parameter names from LocalVariableTable / MethodParameters so
        // completion can show real names instead of synthetic arg0/arg1.
        options.put("-parameters", "");
    })
}

fn file_mtime(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).ok().and_then(|meta| meta.modified().ok())
}

fn file_len(path: &Path) -> u64 {
    std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

fn canonical(path: &Path) -> String {
    if let Ok(resolved) = std::fs::canonicalize(path) {
        return resolved.to_string_lossy().into_owned();
    }
    // Unsaved buffers do not exist on disk: make the path absolute so it
    // matches the compiler's SimpleFileObject name.
    if path.is_absolute() {
        return path.to_string_lossy().into_owned();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path).to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}




/// Discards the compiler's formatted diagnostics (the IDE uses the API).
struct NullWriter;

impl std::io::Write for NullWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
