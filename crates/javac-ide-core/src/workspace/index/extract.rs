//! Single-file extraction for incremental index updates.
//!
//! Compiles exactly one file with a throw-away compiler and returns the
//! declarations, occurrences, supertypes, and Javadoc that belong to it. The
//! result is plain data: it carries no compiler symbols or trees.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::code::scope::LookupKind;
use crate::code::symbol::SymbolKind;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId as CompilerSymbolId, TreeId};
use crate::main::mini_compiler::MiniCompiler;
use crate::tree::jc_tree::TreeKind;
use crate::tree::tree_scanner::TreeScannerVisitor;
use crate::workspace::analysis::FileAnalysis;

use super::build::{
    canonical, class_supertypes, collect_javadocs, kind_name, name_start_of, source_file,
    symbol_kind, OccurrenceCollector,
};
use super::key::{FileId, IndexSymbolKind};
use super::model::{ByteRange, OccurrenceRole, TypeRef};

/// Index identity of one declaration, independent of compiler internals.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolIdentity {
    /// Fully qualified display name (members use `owner.name`; overloads share
    /// this string and are separated by `signature`).
    pub flat_name: String,
    pub kind: IndexSymbolKind,
    /// Parameter fingerprint for methods/constructors, `file:pos:index` for
    /// locals/parameters, empty otherwise.
    pub signature: String,
}

impl SymbolIdentity {
    pub fn is_local(&self) -> bool {
        matches!(
            self.kind,
            IndexSymbolKind::Local
                | IndexSymbolKind::Parameter
                | IndexSymbolKind::TypeParameter
        )
    }
}

#[derive(Debug, Clone)]
pub struct ExtractedDeclaration {
    pub identity: SymbolIdentity,
    pub name: String,
    pub owner: Option<SymbolIdentity>,
    pub range: ByteRange,
    /// AST name position (parser name table / `decl_pos`), not a text search.
    pub name_start: u32,
    pub detail: String,
    pub modifiers: u32,
    /// AST-first deprecation: the declaring tree carries a `@Deprecated`
    /// annotation (`mods.annotations`), independent of `flags_field`.
    pub deprecated: bool,
    pub supertypes: Vec<String>,
    pub return_type: Option<TypeRef>,
    pub javadoc: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExtractedOccurrence {
    pub identity: SymbolIdentity,
    pub range: ByteRange,
    pub role: OccurrenceRole,
}

#[derive(Debug, Default)]
pub struct ExtractedFile {
    pub declarations: Vec<ExtractedDeclaration>,
    pub occurrences: Vec<ExtractedOccurrence>,
    pub failed: bool,
}

/// Analyze one file and extract its index facts.
pub fn extract_file(
    config: &crate::analysis::AnalysisConfig,
    path: &Path,
    file: FileId,
    text: &str,
) -> ExtractedFile {
    let mut analysis = FileAnalysis::new(config.clone());
    let key = PathBuf::from(canonical(path));
    analysis.texts.insert(key.clone(), text.to_string());
    analysis.compiler.overlay_text = Some((key, text.to_string()));
    let compiled = analysis.compiler.compile_files(&[path.to_path_buf()]);
    let failed = compiled.is_err() || analysis.compiler.log.nerrors.get() > 0;
    if failed {
        return ExtractedFile {
            failed: true,
            ..Default::default()
        };
    }
    let javadocs = collect_javadocs(&mut analysis.compiler);
    // AST-first deprecation: declarations whose declaring tree carries a
    // `@Deprecated` annotation (independent of the symbol flag).
    let deprecated_annotations = deprecated_annotation_symbols(&analysis.compiler.ctx);
    let mut out = ExtractedFile::default();
    let target = PathBuf::from(canonical(path));
    let mut identities: HashMap<CompilerSymbolId, SymbolIdentity> = HashMap::new();
    let mut seen = HashSet::new();
    // Declaration TreeIds let `name_start` come from the parser's name table /
    // `decl_pos` instead of a text search.
    let symbol_trees = symbol_tree_map(&analysis.compiler.ctx);
    let name_positions = compilation_unit(&analysis.compiler.ctx, &target).and_then(|unit| {
        match &analysis.compiler.ctx.tree(unit).kind {
            TreeKind::CompilationUnit { name_positions, .. } => name_positions.clone(),
            _ => None,
        }
    });
    let classes = analysis.compiler.syms.get_all_classes();
    for class in classes {
        collect_class(
            &mut analysis.compiler,
            class,
            &target,
            file,
            &javadocs,
            &deprecated_annotations,
            &symbol_trees,
            name_positions.as_deref(),
            text,
            &mut out.declarations,
            &mut identities,
            &mut seen,
        );
    }

    for index in 0..analysis.compiler.ctx.trees.len() {
        let unit = TreeId::new(index as u32);
        let source = match &analysis.compiler.ctx.tree(unit).kind {
            TreeKind::CompilationUnit {
                sourcefile: Some(source),
                ..
            } => source.get_name().to_string_lossy().to_string(),
            _ => continue,
        };
        if PathBuf::from(canonical(Path::new(&source))) != target {
            continue;
        }
        let mut collector = OccurrenceCollector::new(file, text.to_string());
        collector.scan(&mut analysis.compiler.ctx, Some(unit));
        for raw in &collector.raw {
            if raw.position < 0 {
                continue;
            }
            let identity = match identity_of(&mut analysis.compiler.ctx, raw.symbol, file) {
                Some(identity) => identity,
                None if raw.role == OccurrenceRole::Import => {
                    // Package imports (`import java.util.*;`) target a package,
                    // which has no class flat name: the index identity is the
                    // package path itself.
                    let SymbolKind::PackageSymbol { fullname, .. } =
                        &analysis.compiler.ctx.sym(raw.symbol).kind_data
                    else {
                        continue;
                    };
                    SymbolIdentity {
                        flat_name: fullname.to_java_string().to_string_lossy().to_string(),
                        kind: IndexSymbolKind::Other,
                        signature: String::new(),
                    }
                }
                None => continue,
            };
            let start = raw.position as u32;
            let name_len = simple_name(&identity.flat_name).encode_utf16().count() as u32;
            // Import occurrences may carry an explicit span (package imports
            // cover the whole `java.util` path).
            let end = raw.end.unwrap_or(start + name_len.max(1));
            out.occurrences.push(ExtractedOccurrence {
                identity,
                range: ByteRange {
                    start,
                    end: end.max(start + 1),
                },
                role: raw.role,
            });
        }
    }

    // Declaration ranges already start at the declared name (AST name start);
    // no text post-processing is needed.

    // Type/method/constructor declarations are not emitted by the tree
    // scanner; the builder adds their declaration occurrence itself. Mirror
    // that here so a local FileIndex replacement keeps them.
    let declaration_occurrences: Vec<ExtractedOccurrence> = out
        .declarations
        .iter()
        .filter(|declaration| {
            matches!(
                declaration.identity.kind,
                IndexSymbolKind::Class
                    | IndexSymbolKind::Interface
                    | IndexSymbolKind::Enum
                    | IndexSymbolKind::Annotation
                    | IndexSymbolKind::Record
                    | IndexSymbolKind::Method
                    | IndexSymbolKind::Constructor
            )
        })
        .map(|declaration| ExtractedOccurrence {
            identity: declaration.identity.clone(),
            range: declaration.range,
            role: OccurrenceRole::Declaration,
        })
        .collect();
    out.occurrences.extend(declaration_occurrences);
    out
}

fn collect_class(
    compiler: &mut MiniCompiler,
    class: CompilerSymbolId,
    target: &Path,
    file: FileId,
    javadocs: &HashMap<CompilerSymbolId, String>,
    deprecated_annotations: &HashMap<CompilerSymbolId, bool>,
    symbol_trees: &HashMap<CompilerSymbolId, TreeId>,
    name_positions: Option<&dyn crate::tree::end_pos_table::EndPosTable>,
    text: &str,
    out: &mut Vec<ExtractedDeclaration>,
    identities: &mut HashMap<CompilerSymbolId, SymbolIdentity>,
    seen: &mut HashSet<CompilerSymbolId>,
) {
    if !seen.insert(class) {
        return;
    }
    if let Some(declaration) = declaration_of(
        &mut compiler.ctx,
        class,
        target,
        file,
        javadocs,
        deprecated_annotations,
        symbol_trees,
        name_positions,
        text,
    ) {
        identities.insert(class, declaration.identity.clone());
        out.push(declaration);
    }
    let Some(scope) = compiler.ctx.sym(class).members() else {
        return;
    };
    let members = compiler
        .ctx
        .scope_get_symbols(scope, LookupKind::NON_RECURSIVE, &|_| true);
    for member in members {
        if is_synthesised_member(&compiler.ctx, class, member) {
            continue;
        }
        let is_class = matches!(
            compiler.ctx.sym(member).kind_data,
            SymbolKind::ClassSymbol { .. }
        );
        if is_class {
            collect_class(
                compiler,
                member,
                target,
                file,
                javadocs,
                deprecated_annotations,
                symbol_trees,
                name_positions,
                text,
                out,
                identities,
                seen,
            );
        } else if let Some(declaration) = declaration_of(
            &mut compiler.ctx,
            member,
            target,
            file,
            javadocs,
            deprecated_annotations,
            symbol_trees,
            name_positions,
            text,
        ) {
            identities.insert(member, declaration.identity.clone());
            out.push(declaration);
        }
    }
}

/// Compiler-synthesised members (default constructors, record object methods,
/// enum `values`/`valueOf`) are not outline entries. Shared by extraction and
/// the batch index builder. Do not filter on "no declaration tree": fields do
/// not always carry their symbol on the `VariableDecl`, so that test removes
/// real members.
pub(crate) fn is_synthesised_member(
    ctx: &CompilerContext,
    class: CompilerSymbolId,
    member: CompilerSymbolId,
) -> bool {
    // Compiler-generated constructors are not part of the outline.
    if (ctx.sym(member).flags_field & crate::code::flags::GENERATEDCONSTR) != 0 {
        return true;
    }
    let member_name = ctx.sym(member).name.to_java_string().to_string_lossy();
    let generated = matches!(ctx.sym(member).kind_data, SymbolKind::MethodSymbol { .. })
        && (ctx.sym(member).flags_field & 0x2000_0000_0000_0000_u64 as i64) != 0;
    let enum_synthetic = (ctx.sym(class).flags_field & 0x4000) != 0
        && matches!(member_name.as_ref(), "values" | "valueOf");
    generated || enum_synthetic
}

/// AST-first: symbols whose declaring tree carries a `@Deprecated` annotation.
fn deprecated_annotation_symbols(ctx: &CompilerContext) -> HashMap<CompilerSymbolId, bool> {
    let mut deprecated = HashMap::new();
    for index in 0..ctx.trees.len() {
        let tree = TreeId::new(index as u32);
        let (mods, symbol) = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl {
                mods: Some(mods),
                sym: Some(symbol),
                ..
            }
            | TreeKind::MethodDecl {
                mods: Some(mods),
                sym: Some(symbol),
                ..
            }
            | TreeKind::VariableDecl {
                mods: Some(mods),
                sym: Some(symbol),
                ..
            } => (*mods, *symbol),
            _ => continue,
        };
        if has_deprecated_annotation(ctx, mods) {
            deprecated.insert(symbol, true);
        }
    }
    deprecated
}

/// `@Deprecated` in `mods.annotations` (simple or qualified name).
fn has_deprecated_annotation(ctx: &CompilerContext, mods: TreeId) -> bool {
    let annotations = match &ctx.tree(mods).kind {
        TreeKind::Modifiers { annotations, .. } => annotations.clone(),
        _ => return false,
    };
    for annotation in annotations.iter() {
        let annotation_type = match &ctx.tree(annotation).kind {
            TreeKind::Annotation {
                annotation_type: Some(annotation_type),
                ..
            } => *annotation_type,
            _ => continue,
        };
        let name = match &ctx.tree(annotation_type).kind {
            TreeKind::Ident { name, .. } | TreeKind::FieldAccess { name, .. } => {
                name.to_java_string().to_string_lossy().to_string()
            }
            _ => continue,
        };
        if name == "Deprecated" || name == "java.lang.Deprecated" {
            return true;
        }
    }
    false
}

/// The compilation unit parsed for the target source file.
fn compilation_unit(ctx: &CompilerContext, target: &Path) -> Option<TreeId> {
    for index in 0..ctx.trees.len() {
        let unit = TreeId::new(index as u32);
        if let TreeKind::CompilationUnit {
            sourcefile: Some(source),
            ..
        } = &ctx.tree(unit).kind
        {
            let path = PathBuf::from(canonical(Path::new(&source.get_name().to_string_lossy())));
            if path == *target {
                return Some(unit);
            }
        }
    }
    None
}

/// `symbol -> declaring tree` for the declarations in the arena.
fn symbol_tree_map(ctx: &CompilerContext) -> HashMap<CompilerSymbolId, TreeId> {
    let mut trees = HashMap::new();
    // Two passes: class declarations must win their symbol mapping; an enum
    // constant's `VariableDecl` can carry the enum's own symbol as `sym`.
    for pass in 0..2 {
        for index in 0..ctx.trees.len() {
            let tree = TreeId::new(index as u32);
            let symbol = match &ctx.tree(tree).kind {
                TreeKind::ClassDecl {
                    sym: Some(symbol), ..
                } if pass == 0 => *symbol,
                TreeKind::MethodDecl {
                    sym: Some(symbol), ..
                }
                | TreeKind::VariableDecl {
                    sym: Some(symbol), ..
                } if pass == 1 => *symbol,
                _ => continue,
            };
            trees.entry(symbol).or_insert(tree);
        }
    }
    trees
}

/// AST name position: parser name table (classes/interfaces/enums/records/
/// methods), then `VariableDecl.decl_pos` (fields/locals/parameters/enum
/// constants), then the declaring tree position (constructors/type parameters).

fn declaration_of(
    ctx: &mut CompilerContext,
    symbol: CompilerSymbolId,
    target: &Path,
    file: FileId,
    javadocs: &HashMap<CompilerSymbolId, String>,
    deprecated_annotations: &HashMap<CompilerSymbolId, bool>,
    symbol_trees: &HashMap<CompilerSymbolId, TreeId>,
    name_positions: Option<&dyn crate::tree::end_pos_table::EndPosTable>,
    text: &str,
) -> Option<ExtractedDeclaration> {
    let source = source_file(ctx, symbol)?;
    if PathBuf::from(canonical(&source)) != target {
        return None;
    }
    let kind = symbol_kind(ctx, symbol)?;
    let name = ctx
        .sym(symbol)
        .name
        .to_java_string()
        .to_string_lossy()
        .to_string();
    let flat_name = flat_of(ctx, symbol)?;
    let signature = signature_of(ctx, symbol, kind, file);
    let owner = ctx
        .sym(symbol)
        .owner
        .and_then(|owner| identity_of(ctx, owner, file));
    let name_start = name_start_of(ctx, symbol, symbol_trees, name_positions, Some(text));
    let range = ByteRange {
        start: name_start,
        end: name_start.saturating_add(name.encode_utf16().count() as u32),
    };
    let detail = match &ctx.sym(symbol).kind_data {
        SymbolKind::VarSymbol { .. } => {
            let ty = ctx
                .sym(symbol)
                .ty
                .map(|ty| crate::analysis::type_names::type_name(ctx, ty));
            match ty {
                Some(ty) => format!("{ty} {name}"),
                None => format!("{} {}", kind_name(kind), name),
            }
        }
        _ if matches!(
            kind,
            IndexSymbolKind::Method | IndexSymbolKind::Constructor
        ) =>
        {
            format!("{name}{signature}")
        }
        _ => format!("{} {}", kind_name(kind), name),
    };
    let return_type = if matches!(
        kind,
        IndexSymbolKind::Method | IndexSymbolKind::Constructor
    ) {
        ctx.sym(symbol)
            .ty
            .and_then(|ty| match &ctx.typ(ty).kind {
                crate::code::type_::TypeKindData::MethodType { restype, .. } => *restype,
                _ => None,
            })
            .map(|restype| {
                TypeRef::External(crate::analysis::type_names::type_name(ctx, restype))
            })
    } else {
        None
    };
    let supertypes = if kind.is_type() {
        class_supertypes(ctx, symbol)
    } else {
        Vec::new()
    };
    Some(ExtractedDeclaration {
        identity: SymbolIdentity {
            flat_name,
            kind,
            signature,
        },
        name,
        owner,
        range,
        name_start,
        detail,
        modifiers: ctx.sym(symbol).flags_field as u32,
        deprecated: deprecated_annotations
            .get(&symbol)
            .copied()
            .unwrap_or(false),
        supertypes,
        return_type,
        javadoc: javadocs.get(&symbol).cloned(),
    })
}

/// Index identity of a compiler symbol, usable for declarations and for
/// references to symbols declared in other files.
pub fn identity_of(
    ctx: &mut CompilerContext,
    symbol: CompilerSymbolId,
    file: FileId,
) -> Option<SymbolIdentity> {
    let kind = symbol_kind(ctx, symbol)?;
    let flat_name = flat_of(ctx, symbol)?;
    let signature = signature_of(ctx, symbol, kind, file);
    Some(SymbolIdentity {
        flat_name,
        kind,
        signature,
    })
}

fn flat_of(ctx: &CompilerContext, symbol: CompilerSymbolId) -> Option<String> {
    match &ctx.sym(symbol).kind_data {
        SymbolKind::ClassSymbol { flatname, .. } => Some(flatname.to_string()),
        _ => {
            let owner = ctx.sym(symbol).owner?;
            let owner_flat = flat_of(ctx, owner)?;
            let name = ctx.sym(symbol).name.to_java_string().to_string_lossy();
            Some(format!("{owner_flat}.{name}"))
        }
    }
}

fn signature_of(
    ctx: &mut CompilerContext,
    symbol: CompilerSymbolId,
    kind: IndexSymbolKind,
    file: FileId,
) -> String {
    match kind {
        IndexSymbolKind::Method | IndexSymbolKind::Constructor => {
            let mut signature = String::from("(");
            if let SymbolKind::MethodSymbol { params, .. } = &ctx.sym(symbol).kind_data {
                for (index, param) in params.iter().enumerate() {
                    if index > 0 {
                        signature.push(',');
                    }
                    if let Some(ty) = ctx.sym(param).ty {
                        signature.push_str(&crate::analysis::type_name(ctx, ty));
                    } else {
                        signature.push('?');
                    }
                }
            }
            signature.push(')');
            signature
        }
        IndexSymbolKind::Local | IndexSymbolKind::Parameter => {
            let position = ctx.sym(symbol).pos.get().max(0);
            format!("{}:{}:{}", file, position, symbol.index())
        }
        _ => String::new(),
    }
}

pub fn simple_name(flat_name: &str) -> &str {
    flat_name
        .rsplit(['.', '$'])
        .next()
        .unwrap_or(flat_name)
}
