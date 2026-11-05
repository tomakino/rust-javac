
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

use std::cell::RefCell;
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
}

/// One symbol of the analyzed file (classes and their direct members).
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: &'static str,
    pub flat_name: String,
    /// Declaration offset in the source text (NOPOS when unknown).
    pub pos: i32,
}

/// Result of analyzing one source file.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub errors: i32,
    pub warnings: i32,
    pub ok: bool,
}

struct TracedClass {
    sym: SymbolId,
    flat_name: Name,
    package: Option<SymbolId>,
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
    config: AnalysisConfig,
    compiler: MiniCompiler,
    initialized: bool,
    traced: Vec<TracedClass>,
    cache: Option<CachedFile>,
    last_text: Option<String>,
}

impl AnalysisSession {
    pub fn new(config: AnalysisConfig) -> Self {
        std::fs::create_dir_all(&config.output_dir)
            .expect("create analysis output directory");
        let compiler = build_compiler(&config);
        Self {
            config,
            compiler,
            initialized: false,
            traced: Vec::new(),
            cache: None,
            last_text: None,
        }
    }

    fn rebuild(&mut self) {
        self.compiler = build_compiler(&self.config);
        self.initialized = false;
        self.traced.clear();
    }

    /// Analyze one source file with the project class path.
    ///
    /// The first call initializes the module graph through the compiler
    /// driver; later calls reuse it. Code generation is skipped.
    pub fn analyze(&mut self, source: &Path) -> AnalysisResult {
        self.last_text = self
            .compiler
            .overlay_text
            .as_ref()
            .map(|(_, text)| text.clone())
            .or_else(|| std::fs::read_to_string(source).ok());
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
        // Edited file: the cross-compilation caches are not isolated yet, so a
        // fresh compiler keeps semantics correct. Unchanged files never pay it.
        if self.initialized {
            self.rebuild();
        }
        if !self.initialized {
            let _ = self.compiler.compile_files(&[source.to_path_buf()]);
            self.initialized = true;
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
        let module_info =
            sourcefile.is_name_compatible(&JavaString::from("module-info"), Kind::SOURCE);
        let mut parser = self.compiler.parser_factory.new_parser_full(
            &mut self.compiler.ctx,
            &content,
            false,
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

    fn trace_classes_for(&mut self, source: &Path) {
        self.traced.clear();
        let source_name = canonical(source);
        let classes = self.compiler.syms.get_all_classes();
        for sym in classes {
            let matching = match &self.compiler.ctx.sym(sym).kind_data {
                SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
                    .as_ref()
                    .map(|file| canonical(Path::new(&file.get_name().to_string_lossy())))
                    .map(|name| name == source_name)
                    .unwrap_or(false),
                _ => false,
            };
            if matching {
                let flat_name = self.compiler.ctx.sym(sym).flat_name();
                let package = symbol_packge(&self.compiler.ctx, sym);
                self.traced.push(TracedClass {
                    sym,
                    flat_name,
                    package,
                });
            }
        }
    }

    /// The symbol at a source position (1-based line and column).
    ///
    /// The word under the cursor is read from the file text and matched
    /// against the analyzed symbols; this is the base for go-to-definition and
    /// hover.
    pub fn symbol_at(&mut self, source: &Path, line: usize, column: usize) -> Option<SymbolInfo> {
        let text = self
            .last_text
            .clone()
            .or_else(|| std::fs::read_to_string(source).ok())?;
        let line_text = text.lines().nth(line.saturating_sub(1))?;
        let chars: Vec<char> = line_text.chars().collect();
        if chars.is_empty() {
            return None;
        }
        let mut col = column.saturating_sub(1).min(chars.len() - 1);
        let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
        if !is_ident(chars[col]) && col > 0 && is_ident(chars[col - 1]) {
            col -= 1;
        }
        if !is_ident(chars[col]) {
            return None;
        }
        let mut start = col;
        while start > 0 && is_ident(chars[start - 1]) {
            start -= 1;
        }
        let mut end = col;
        while end + 1 < chars.len() && is_ident(chars[end + 1]) {
            end += 1;
        }
        let word: String = chars[start..=end].iter().collect();
        if word.is_empty() {
            return None;
        }
        let cursor = line_offset(&text, line, start + 1) as i64;
        self.symbols()
            .into_iter()
            .filter(|symbol| symbol.name == word && symbol.pos >= 0)
            .min_by_key(|symbol| (symbol.pos as i64 - cursor).abs())
            .or_else(|| self.symbols().into_iter().find(|symbol| symbol.name == word))
    }

    /// Symbols of the analyzed file: its classes and their direct members.
    /// This is the base for completion and go-to-definition.
    pub fn symbols(&mut self) -> Vec<SymbolInfo> {
        let mut out = Vec::new();
        for traced in &self.traced {
            let class = &self.compiler.ctx.sym(traced.sym);
            let class_pos = class.pos.get();
            out.push(SymbolInfo {
                name: class.name.to_java_string().to_string_lossy(),
                kind: "class",
                flat_name: traced.flat_name.to_string(),
                pos: class_pos,
            });
            let scope = class.members();
            if let Some(scope) = scope {
                let members = self.compiler.ctx.scope_get_symbols(
                    scope,
                    LookupKind::NON_RECURSIVE,
                    &|_| true,
                );
                for member in members {
                    let sym = &self.compiler.ctx.sym(member);
                    let kind = match sym.kind_data {
                        SymbolKind::MethodSymbol { .. } => "method",
                        SymbolKind::VarSymbol { .. } => "field",
                        SymbolKind::ClassSymbol { .. } => "nested",
                        _ => "member",
                    };
                    let member_pos = match &sym.kind_data {
                        SymbolKind::VarSymbol { pos, .. } => *pos,
                        _ => sym.pos.get(),
                    };
                    out.push(SymbolInfo {
                        name: sym.name.to_java_string().to_string_lossy(),
                        kind,
                        flat_name: traced.flat_name.to_string(),
                        pos: member_pos,
                    });
                }
            }
        }
        out
    }

    fn remove_traced_classes(&mut self) {
        for traced in std::mem::take(&mut self.traced) {
            self.compiler
                .chk
                .remove_compiled(&self.compiler.ctx, traced.sym);
            if let Some(package) = traced.package {
                if let Some(scope) = self.compiler.ctx.sym(package).members() {
                    self.compiler.ctx.scope_remove(scope, traced.sym);
                }
                self.compiler
                    .syms
                    .remove_class(traced.sym, &traced.flat_name);
            }
        }
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
    MiniCompiler::new_with_options(&config.output_dir, |options| {
        if !class_path.is_empty() {
            options.put("--class-path", class_path.clone());
        }
        options.put("--source", source_level.clone());
        options.put("should-stop.at", "ATTR");
    })
}

fn file_mtime(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).ok().and_then(|meta| meta.modified().ok())
}

fn file_len(path: &Path) -> u64 {
    std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

fn canonical(path: &Path) -> String {
    std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}


impl javac_analysis_api::AnalysisHost for AnalysisSession {
    fn analyze(
        &mut self,
        file: &Path,
        text: Option<&str>,
    ) -> Vec<javac_analysis_api::Diagnostic> {
        self.compiler.overlay_text = text.map(|text| (PathBuf::from(canonical(file)), text.to_string()));
        if text.is_some() {
            self.cache = None;
        }
        let _ = self.analyze(file);
        self.compiler.overlay_text = None;
        let text = self.last_text.clone().unwrap_or_default();
        let diagnostics = self.compiler.log.take_diagnostics();
        diagnostics
            .iter()
            .map(|diag| convert_diagnostic(diag, &text))
            .collect()
    }

    fn symbols(&mut self, _file: &Path) -> Vec<javac_analysis_api::Symbol> {
        self.symbols()
            .into_iter()
            .map(|symbol| javac_analysis_api::Symbol {
                name: symbol.name,
                kind: map_kind(symbol.kind),
                range: None,
                qualified_name: Some(symbol.flat_name),
            })
            .collect()
    }

    fn symbol_at(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::Symbol> {
        self.symbol_at(file, position.line as usize + 1, position.column as usize + 1)
            .map(|symbol| javac_analysis_api::Symbol {
                name: symbol.name,
                kind: map_kind(symbol.kind),
                range: None,
                qualified_name: Some(symbol.flat_name),
            })
    }

    fn completion(
        &mut self,
        _file: &Path,
        _position: javac_analysis_api::Position,
    ) -> Vec<javac_analysis_api::CompletionItem> {
        // Baseline completion: every symbol of the analyzed file. The LSP
        // client filters by the typed prefix.
        self.symbols()
            .into_iter()
            .map(|symbol| javac_analysis_api::CompletionItem {
                label: symbol.name,
                kind: map_kind(symbol.kind),
                detail: Some(symbol.flat_name),
            })
            .collect()
    }

    fn definition(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::Location> {
        let symbol = AnalysisSession::symbol_at(
            self,
            file,
            position.line as usize + 1,
            position.column as usize + 1,
        )?;
        if symbol.pos < 0 {
            return None;
        }
        let text = self.last_text.clone()?;
        let (line, column) = offset_to_line_col(&text, symbol.pos as usize);
        Some(javac_analysis_api::Location {
            file: file.to_path_buf(),
            range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(
                line, column,
            )),
        })
    }

    fn references(
        &mut self,
        _file: &Path,
        _position: javac_analysis_api::Position,
    ) -> Vec<javac_analysis_api::Location> {
        Vec::new()
    }

    fn hover(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<String> {
        let symbol = AnalysisSession::symbol_at(self, file, position.line as usize + 1, position.column as usize + 1)?;
        Some(format!("{:?} {}", symbol.kind, symbol.name))
    }
}

fn map_kind(kind: &'static str) -> javac_analysis_api::SymbolKind {
    match kind {
        "class" => javac_analysis_api::SymbolKind::Class,
        "method" => javac_analysis_api::SymbolKind::Method,
        "field" => javac_analysis_api::SymbolKind::Field,
        "nested" => javac_analysis_api::SymbolKind::Class,
        _ => javac_analysis_api::SymbolKind::Field,
    }
}




/// Convert one compiler diagnostic to the API type.
fn convert_diagnostic(
    diagnostic: &crate::util::jc_diagnostic::JCDiagnostic,
    text: &str,
) -> javac_analysis_api::Diagnostic {
    use crate::util::jc_diagnostic::DiagnosticType;
    let offset = diagnostic.get_position().max(0) as usize;
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let line = before.matches('\n').count() as u32;
    let column = before
        .rfind('\n')
        .map(|index| (offset - index - 1) as u32)
        .unwrap_or(offset as u32);
    let severity = match diagnostic.get_type() {
        DiagnosticType::ERROR => javac_analysis_api::Severity::Error,
        DiagnosticType::WARNING => javac_analysis_api::Severity::Warning,
        _ => javac_analysis_api::Severity::Information,
    };
    javac_analysis_api::Diagnostic {
        range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(line, column)),
        severity,
        message: diagnostic.get_message(None).to_string_lossy(),
        code: None,
    }
}


/// Character offset of a 1-based line/column in the given text.
fn line_offset(text: &str, line: usize, column: usize) -> usize {
    let mut current_line = 1;
    let mut current_column = 1;
    for (byte_index, current) in text.char_indices() {
        if current_line == line && current_column >= column {
            return byte_index;
        }
        if current == '\n' {
            current_line += 1;
            current_column = 1;
        } else {
            current_column += 1;
        }
    }
    text.len()
}


/// Zero-based line/column of a character offset.
fn offset_to_line_col(text: &str, offset: usize) -> (u32, u32) {
    let mut line = 0u32;
    let mut column = 0u32;
    for (byte_index, current) in text.char_indices() {
        if byte_index >= offset {
            break;
        }
        if current == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    (line, column)
}
