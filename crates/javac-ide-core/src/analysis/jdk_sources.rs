//! JDK source lookup for go-to-definition.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use javac_zip::ZipArchive;

use super::positions::{byte_to_utf16, offset_to_line_col};
use super::source_files::{simple_class_name, source_relative_path};

use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::parser::parser::Parser;
use crate::parser::parser_factory::ParserFactory;
use crate::tree::jc_tree::TreeKind;
use crate::util::java_string::JavaString;

/// Parsed declaration metadata for one JDK method/constructor.
///
/// The source is parsed once per class; overload selection and the returned
/// parameter names are computed from these AST-derived fields, never from a
/// second text pass.
#[derive(Debug, Clone)]
struct JdkMethod {
    name: String,
    parameter_names: Vec<String>,
    parameter_types: Vec<String>,
}

static JDK_METHOD_CACHE: OnceLock<Mutex<HashMap<String, Vec<JdkMethod>>>> = OnceLock::new();

#[cfg(test)]
static JDK_PARSE_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn jdk_method_cache() -> &'static Mutex<HashMap<String, Vec<JdkMethod>>> {
    JDK_METHOD_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Parameter names for a JDK method/constructor, read from the JDK src.zip and
/// parsed into an AST with the project's `JavacParser`.
///
/// The declaration is selected by method name, arity and the AST parameter
/// types. The parsed class metadata is cached by class name; a cache hit never
/// re-opens src.zip or re-parses the source.
pub(crate) fn jdk_method_parameter_names(
    parser_factory: &ParserFactory,
    class_flat: &str,
    method_name: &str,
    parameter_count: usize,
    parameter_types: &[String],
) -> Option<Vec<String>> {
    let methods = jdk_methods(parser_factory, class_flat)?;
    let source_name = if method_name == "<init>" {
        simple_class_name(class_flat).to_string()
    } else {
        method_name.to_string()
    };
    let mut candidates: Vec<&JdkMethod> = methods
        .iter()
        .filter(|method| {
            (method.name == source_name || method.name == method_name)
                && method.parameter_names.len() == parameter_count
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    if candidates.len() == 1 {
        return Some(candidates.pop()?.parameter_names.clone());
    }

    let expected: Vec<String> = parameter_types.iter().map(|ty| normalize_type(ty)).collect();
    let mut scored: Vec<(usize, &JdkMethod)> = candidates
        .iter()
        .map(|method| {
            let score = method
                .parameter_types
                .iter()
                .zip(expected.iter())
                .filter(|(actual, expected)| normalize_type(actual) == **expected)
                .count();
            (score, *method)
        })
        .collect();
    let best = scored.iter().map(|(score, _)| *score).max().unwrap_or(0);
    if best > 0 {
        let winners: Vec<&(usize, &JdkMethod)> =
            scored.iter().filter(|(score, _)| *score == best).collect();
        if winners.len() == 1 {
            return Some(winners[0].1.parameter_names.clone());
        }
        return None;
    }
    // Generic type variables do not have a useful descriptor match. If all
    // same-arity declarations expose the same names, the answer is unambiguous.
    let first = &scored[0].1.parameter_names;
    if scored.iter().all(|(_, method)| method.parameter_names == *first) {
        return Some(first.clone());
    }
    None
}

fn jdk_methods(parser_factory: &ParserFactory, class_flat: &str) -> Option<Vec<JdkMethod>> {
    if let Ok(cache) = jdk_method_cache().lock()
        && let Some(methods) = cache.get(class_flat)
    {
        return Some(methods.clone());
    }
    let jdk_src = default_jdk_src()?;
    let text = read_class_source(&jdk_src, class_flat)?;
    #[cfg(test)]
    JDK_PARSE_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let methods = parse_methods(parser_factory, class_flat, &text);
    if let Ok(mut cache) = jdk_method_cache().lock() {
        cache.insert(class_flat.to_string(), methods.clone());
    }
    Some(methods)
}

fn parse_methods(parser_factory: &ParserFactory, class_flat: &str, text: &str) -> Vec<JdkMethod> {
    let mut ctx = CompilerContext::new();
    let input = JavaString::from(text);
    let mut parser = parser_factory.new_parser(&mut ctx, &input, false, true, true);
    let _ = parser.parse_compilation_unit(&mut ctx);
    let class_name = simple_class_name(class_flat);
    let mut methods = Vec::new();
    for index in 0..ctx.trees.len() {
        let tree = TreeId::new(index as u32);
        let TreeKind::MethodDecl { name, params, .. } = &ctx.tree(tree).kind else {
            continue;
        };
        let method_name = name.to_java_string().to_string_lossy().to_string();
        let mut parameter_names = Vec::new();
        let mut parameter_types = Vec::new();
        for param in params.iter() {
            let TreeKind::VariableDecl {
                name: parameter_name,
                vartype,
                ..
            } = &ctx.tree(param).kind
            else {
                parameter_names.push(String::new());
                parameter_types.push(String::new());
                continue;
            };
            parameter_names.push(
                parameter_name
                    .to_java_string()
                    .to_string_lossy()
                    .to_string(),
            );
            parameter_types.push(
                vartype
                    .map(|vartype| ast_type_name(&ctx, vartype))
                    .unwrap_or_default(),
            );
        }
        // Constructors may be parsed under `<init>` or the class's simple name.
        let name = if method_name == "<init>" {
            class_name.to_string()
        } else {
            method_name
        };
        methods.push(JdkMethod {
            name,
            parameter_names,
            parameter_types,
        });
    }
    methods
}

/// Type name from the AST node (Ident/TypeApply/ArrayTypeTree/PrimitiveTypeTree),
/// so overload matching never parses source text.
fn ast_type_name(ctx: &CompilerContext, tree: TreeId) -> String {
    match &ctx.tree(tree).kind {
        TreeKind::Ident { name, .. } => {
            name.to_java_string().to_string_lossy().to_string()
        }
        TreeKind::TypeApply { clazz, .. } => clazz
            .map(|clazz| ast_type_name(ctx, clazz))
            .unwrap_or_default(),
        TreeKind::ArrayTypeTree { elemtype } => elemtype
            .map(|elemtype| format!("{}[]", ast_type_name(ctx, elemtype)))
            .unwrap_or_default(),
        TreeKind::PrimitiveTypeTree { typetag } => {
            format!("{typetag:?}").to_ascii_lowercase()
        }
        TreeKind::AnnotatedType { underlying_type, .. } => underlying_type
            .map(|underlying_type| ast_type_name(ctx, underlying_type))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

fn normalize_type(text: &str) -> String {
    let text = text.trim();
    let text = text
        .strip_suffix("...")
        .or_else(|| text.strip_suffix("[]"))
        .unwrap_or(text)
        .trim();
    let mut depth = 0usize;
    let mut base = String::new();
    for ch in text.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            ch if depth == 0 => base.push(ch),
            _ => {}
        }
    }
    let base = base.trim();
    base.rsplit('.').next().unwrap_or(base).to_string()
}

fn read_class_source(jdk_src: &Path, class_flat: &str) -> Option<String> {
    if !jdk_src.exists() {
        return None;
    }
    let relative = source_relative_path(class_flat)?;
    let archive = ZipArchive::open(jdk_src).ok()?;
    let suffix = format!("/{relative}");
    let entry_name = archive
        .entries()
        .iter()
        .map(|entry| entry.name().to_string())
        .find(|name| name == &relative || name.ends_with(&suffix))?;
    let bytes = archive.get_entry(&entry_name)?;
    Some(String::from_utf8_lossy(&bytes).to_string())
}

/// Find the JDK source declaration of a simple name in the JDK src.zip.
///
/// The matching entry is extracted under the session output directory so the
/// editor can open it as a normal file.
pub(crate) fn jdk_source_location(
    jdk_src: &Path,
    word: &str,
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    if !jdk_src.exists() {
        return None;
    }
    let archive = ZipArchive::open(jdk_src).ok()?;
    let suffix = format!("/{word}.java");
    let entry_name = archive
        .entries()
        .iter()
        .map(|entry| entry.name().to_string())
        .find(|name| name.ends_with(&suffix) || name == &format!("{word}.java"))?;
    let bytes = archive.get_entry(&entry_name)?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let markers = [
        format!("class {word}"),
        format!("interface {word}"),
        format!("enum {word}"),
        format!("record {word}"),
    ];
    let offset = text
        .lines()
        .enumerate()
        .find(|(_, line)| markers.iter().any(|marker| line.contains(marker.as_str())))
        .map(|(index, _)| {
            text.lines()
                .take(index)
                .map(|line| line.len() + 1)
                .sum::<usize>()
        })?;
    let extracted = super::source_files::extract(&bytes, "jdk-src", &entry_name, output_dir)?;
    Some(location_at_offset(&text, &extracted, offset))
}

/// Find a member declaration in the JDK src.zip for `class_flat`.
///
/// JDK 9+ src.zip entries are prefixed with the module name
/// (`java.base/java/lang/Object.java`), so match the class path as a suffix.
pub(crate) fn jdk_member_location(
    jdk_src: &Path,
    class_flat: &str,
    member: &str,
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    let (text, extracted) = class_source_text(jdk_src, class_flat, output_dir)?;
    let offset = super::external_sources::locate_member(&text, member)?;
    Some(location_at_offset(&text, &extracted, offset))
}

/// Find a constructor declaration in the JDK src.zip for `class_flat`.
///
/// `arity` selects the overload by declared parameter count; the caller
/// degrades to [`jdk_class_location`] when the source text cannot pick a
/// unique declaration.
pub(crate) fn jdk_constructor_location(
    jdk_src: &Path,
    class_flat: &str,
    arity: usize,
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    let (text, extracted) = class_source_text(jdk_src, class_flat, output_dir)?;
    let simple = simple_class_name(class_flat);
    let offset = super::external_sources::locate_constructor(&text, simple, Some(arity))?;
    Some(location_at_offset(&text, &extracted, offset))
}

/// Find the type declaration of `class_flat` in the JDK src.zip.
pub(crate) fn jdk_class_location(
    jdk_src: &Path,
    class_flat: &str,
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    let (text, extracted) = class_source_text(jdk_src, class_flat, output_dir)?;
    let simple = simple_class_name(class_flat);
    // `locate_member` prefers the type declaration (`class X`) over members.
    let offset = super::external_sources::locate_member(&text, simple)?;
    Some(location_at_offset(&text, &extracted, offset))
}

/// Read `class_flat`'s source from the JDK src.zip and extract it under the
/// session output directory. Returns the source text and the extracted path.
fn class_source_text(
    jdk_src: &Path,
    class_flat: &str,
    output_dir: &Path,
) -> Option<(String, PathBuf)> {
    if !jdk_src.exists() {
        return None;
    }
    let relative = source_relative_path(class_flat)?;
    let archive = ZipArchive::open(jdk_src).ok()?;
    let suffix = format!("/{relative}");
    let entry_name = archive
        .entries()
        .iter()
        .map(|entry| entry.name().to_string())
        .find(|name| name == &relative || name.ends_with(&suffix))?;
    let bytes = archive.get_entry(&entry_name)?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let extracted = super::source_files::extract(&bytes, "jdk-src", &entry_name, output_dir)?;
    Some((text, extracted))
}

/// LSP location of a byte offset in an extracted JDK source file.
///
/// The byte offset becomes a UTF-16 offset before the line/column conversion,
/// so non-ASCII source lines keep the editor cursor on the identifier.
fn location_at_offset(
    text: &str,
    file: &Path,
    offset: usize,
) -> javac_analysis_api::Location {
    let offset = byte_to_utf16(text, offset);
    let (line, column) = offset_to_line_col(text, offset);
    javac_analysis_api::Location {
        file: file.to_path_buf(),
        range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(line, column)),
    }
}

/// The default JDK src.zip location. Delegates to the single locator in
/// [`super::source_files`] (union of RAVAC_PLATFORM_DIR / RAVAC_JDK_SRC /
/// JAVA_HOME); no second locator is kept here.
pub(crate) fn default_jdk_src() -> Option<PathBuf> {
    super::source_files::jdk_src_zip()
}

/// Major JDK version from the platform/`JAVA_HOME` `release` file, when
/// available. `None` means the source annotation omits the version rather
/// than guessing it from a path.
pub(crate) fn jdk_release_version() -> Option<String> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(platform) = std::env::var_os("RAVAC_PLATFORM_DIR") {
        roots.push(PathBuf::from(platform));
    }
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        roots.push(PathBuf::from(home));
    }
    if let Some(src) = default_jdk_src() {
        if let Some(parent) = src.parent() {
            roots.push(parent.to_path_buf());
            if parent.file_name().is_some_and(|name| name == "lib")
                && let Some(home) = parent.parent()
            {
                roots.push(home.to_path_buf());
            }
        }
    }
    for root in roots {
        let Ok(text) = std::fs::read_to_string(root.join("release")) else {
            continue;
        };
        if let Some(value) = text
            .lines()
            .find_map(|line| line.strip_prefix("JAVA_VERSION="))
        {
            let value = value.trim().trim_matches('"');
            let major = value.split('.').next().unwrap_or(value);
            if !major.is_empty() {
                return Some(major.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod parameter_tests {
    use super::*;
    use crate::code::source::Source;
    use crate::parser::scanner_factory::ScannerFactory;
    use crate::util::context::Context;
    use crate::util::names::Names;
    use std::rc::Rc;
    use std::sync::atomic::Ordering;

    fn parser_factory() -> ParserFactory {
        let mut context = Context::new();
        let names = Rc::new(Names::new(false));
        let source = Source::JDK17;
        let scanner_factory =
            ScannerFactory::new(&mut context, Rc::clone(&names), source);
        ParserFactory::new(&mut context, names, source, scanner_factory, None)
    }

    #[test]
    fn jdk_parameter_names_cache_hit_does_not_reparse() {
        if default_jdk_src().is_none() {
            return;
        }
        if let Ok(mut cache) = jdk_method_cache().lock() {
            cache.clear();
        }
        let factory = parser_factory();
        let before = JDK_PARSE_COUNT.load(Ordering::SeqCst);
        let first = jdk_method_parameter_names(
            &factory,
            "java.util.List",
            "add",
            1,
            &["E".to_string()],
        )
        .expect("List.add parameter names");
        assert_eq!(first, vec!["e".to_string()]);
        let after_first = JDK_PARSE_COUNT.load(Ordering::SeqCst);
        assert_eq!(after_first, before + 1, "first lookup must parse once");

        let second = jdk_method_parameter_names(
            &factory,
            "java.util.List",
            "add",
            1,
            &["E".to_string()],
        )
        .expect("cached List.add parameter names");
        assert_eq!(second, first);
        assert_eq!(
            JDK_PARSE_COUNT.load(Ordering::SeqCst),
            after_first,
            "cache hit must not parse src.zip again"
        );
    }
}
