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
    canonical, class_supertypes, collect_javadocs, kind_name, source_file, symbol_kind,
    OccurrenceCollector,
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
    pub detail: String,
    pub modifiers: u32,
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
    let mut out = ExtractedFile::default();
    let target = PathBuf::from(canonical(path));
    let mut identities: HashMap<CompilerSymbolId, SymbolIdentity> = HashMap::new();
    let mut seen = HashSet::new();
    let classes = analysis.compiler.syms.get_all_classes();
    for class in classes {
        collect_class(
            &mut analysis.compiler,
            class,
            &target,
            file,
            &javadocs,
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
            let Some(identity) = identity_of(&mut analysis.compiler.ctx, raw.symbol, file) else {
                continue;
            };
            let name_len = simple_name(&identity.flat_name).encode_utf16().count() as u32;
            out.occurrences.push(ExtractedOccurrence {
                identity,
                range: ByteRange {
                    start: raw.position as u32,
                    end: raw.position as u32 + name_len.max(1),
                },
                role: raw.role,
            });
        }
    }

    // Declaration positions point at the declaration tree start (`class`,
    // modifiers, return type); move them onto the declared name.
    for declaration in &mut out.declarations {
        let start =
            crate::analysis::positions::declared_name_start(text, declaration.range.start, &declaration.name);
        declaration.range = ByteRange {
            start,
            end: start.saturating_add(declaration.name.encode_utf16().count() as u32),
        };
    }
    for occurrence in &mut out.occurrences {
        if occurrence.role == OccurrenceRole::Declaration {
            let name = simple_name(&occurrence.identity.flat_name);
            let start = crate::analysis::positions::declared_name_start(text, occurrence.range.start, name);
            occurrence.range = ByteRange {
                start,
                end: start.saturating_add(name.encode_utf16().count() as u32),
            };
        }
    }

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
    out: &mut Vec<ExtractedDeclaration>,
    identities: &mut HashMap<CompilerSymbolId, SymbolIdentity>,
    seen: &mut HashSet<CompilerSymbolId>,
) {
    if !seen.insert(class) {
        return;
    }
    if let Some(declaration) = declaration_of(&mut compiler.ctx, class, target, file, javadocs) {
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
        if (compiler.ctx.sym(member).flags_field & crate::code::flags::GENERATEDCONSTR) != 0 {
            continue;
        }
        let is_class = matches!(
            compiler.ctx.sym(member).kind_data,
            SymbolKind::ClassSymbol { .. }
        );
        if is_class {
            collect_class(compiler, member, target, file, javadocs, out, identities, seen);
        } else if let Some(declaration) =
            declaration_of(&mut compiler.ctx, member, target, file, javadocs)
        {
            identities.insert(member, declaration.identity.clone());
            out.push(declaration);
        }
    }
}

fn declaration_of(
    ctx: &mut CompilerContext,
    symbol: CompilerSymbolId,
    target: &Path,
    file: FileId,
    javadocs: &HashMap<CompilerSymbolId, String>,
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
    let pos = ctx.sym(symbol).pos.get().max(0) as u32;
    let range = ByteRange {
        start: pos,
        end: pos.saturating_add(name.encode_utf16().count() as u32),
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
        detail,
        modifiers: ctx.sym(symbol).flags_field as u32,
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
