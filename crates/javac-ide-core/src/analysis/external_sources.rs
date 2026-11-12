//! Classpath source lookup for go-to-definition.
//!
//! A definition may point at a class from a jar. When the matching
//! `-sources.jar` is available next to the binary jar, the source file is
//! extracted under the session output directory so the editor can open it.
//! Declarations are located by parsing the source with `JavacParser`, exactly
//! like project sources.

use std::path::{Path, PathBuf};

use javac_zip::ZipArchive;

use super::positions::{byte_to_utf16, offset_to_line_col};
use super::source_files::{simple_class_name, source_relative_path, zip_entry};
use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::parser::parser::Parser;
use crate::parser::parser_factory::ParserFactory;
use crate::tree::jc_tree::TreeKind;
use crate::util::java_string::JavaString;

/// Locate `member` in the sources jar for `class_flat` on the class path.
pub(crate) fn external_source_location(
    parser_factory: &ParserFactory,
    class_flat: &str,
    member: &str,
    class_path: &[PathBuf],
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    sources_location(
        parser_factory,
        class_flat,
        &|factory, text, class_name| locate_member(factory, text, class_name, member),
        class_path,
        output_dir,
    )
}

/// Locate a constructor declaration of `class_flat` in its sources jar.
pub(crate) fn external_constructor_location(
    parser_factory: &ParserFactory,
    class_flat: &str,
    arity: usize,
    class_path: &[PathBuf],
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    sources_location(
        parser_factory,
        class_flat,
        &|factory, text, class_name| locate_constructor(factory, text, class_name, arity),
        class_path,
        output_dir,
    )
}

fn sources_location(
    parser_factory: &ParserFactory,
    class_flat: &str,
    locate: &dyn Fn(&ParserFactory, &str, &str) -> Option<usize>,
    class_path: &[PathBuf],
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    let relative = source_relative_path(class_flat)?;
    let simple = simple_class_name(class_flat);
    for entry in class_path {
        let Some(sources) = super::source_files::sources_jar(entry) else {
            continue;
        };
        let Ok(archive) = ZipArchive::open(&sources) else {
            continue;
        };
        let Some(entry_name) = zip_entry(&archive, &relative) else {
            continue;
        };
        let Some(bytes) = archive.get_entry(&entry_name) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes).to_string();
        let Some(offset) = locate(parser_factory, &text, simple) else {
            continue;
        };
        let stem = sources.file_stem()?.to_string_lossy();
        let extracted = super::source_files::extract(
            &bytes,
            &format!("external-src/{stem}"),
            &relative,
            output_dir,
        )?;
        let offset = byte_to_utf16(&text, offset);
        let (line, column) = offset_to_line_col(&text, offset);
        return Some(javac_analysis_api::Location {
            file: PathBuf::from(extracted),
            range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(
                line, column,
            )),
        });
    }
    None
}

fn parse_source(factory: &ParserFactory, text: &str) -> Option<(CompilerContext, TreeId)> {
    let mut ctx = CompilerContext::new();
    let input = JavaString::from(text);
    let mut parser = factory.new_parser(&mut ctx, &input, false, true, true);
    let root = parser.parse_compilation_unit(&mut ctx);
    Some((ctx, root))
}

/// Byte offset of a type/member declaration, from AST nodes only.
pub(crate) fn locate_member(
    factory: &ParserFactory,
    text: &str,
    class_name: &str,
    member: &str,
) -> Option<usize> {
    if member.is_empty() {
        return None;
    }
    let (ctx, _root) = parse_source(factory, text)?;
    let mut type_match = None;
    let mut member_match = None;
    for index in 0..ctx.trees.len() {
        let definition = TreeId::new(index as u32);
        match &ctx.tree(definition).kind {
            TreeKind::ClassDecl { name, .. }
                if name.to_java_string().to_string_lossy() == member =>
            {
                type_match.get_or_insert(ctx.tree(definition).pos);
            }
            TreeKind::MethodDecl { name, .. }
            | TreeKind::VariableDecl { name, .. }
                if name.to_java_string().to_string_lossy() == member =>
            {
                member_match.get_or_insert(ctx.tree(definition).pos);
            }
            _ => {}
        }
    }
    if let Some(position) = type_match {
        return (position >= 0).then_some(position as usize);
    }
    member_match.and_then(|position| (position >= 0).then_some(position as usize))
}

/// Byte offset of a constructor declaration with the requested arity.
pub(crate) fn locate_constructor(
    factory: &ParserFactory,
    text: &str,
    class_name: &str,
    arity: usize,
) -> Option<usize> {
    let (ctx, _root) = parse_source(factory, text)?;
    let mut matches = Vec::new();
    for index in 0..ctx.trees.len() {
        let definition = TreeId::new(index as u32);
        let TreeKind::MethodDecl { name, params, .. } = &ctx.tree(definition).kind else {
            continue;
        };
        let name = name.to_java_string().to_string_lossy();
        if (name == class_name || name == "<init>") && params.size() as usize == arity {
            let position = ctx.tree(definition).pos;
            if position >= 0 {
                matches.push(position as usize);
            }
        }
    }
    (matches.len() == 1).then(|| matches[0])
}
