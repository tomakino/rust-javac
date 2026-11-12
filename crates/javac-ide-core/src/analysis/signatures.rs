//! IDEA-style declaration signatures for hover, completion, signature help
//! and the outline.
//!
//! Method signatures are rendered from compiler symbols in one place:
//! `[<T>] [ReturnType] name(params)`. Parameter names are used when the symbol
//! carries real names; callers choose how synthetic `argN`/blank names are
//! treated.

use crate::code::flags;
use crate::code::symbol::SymbolKind;
use crate::code::type_::TypeKindData;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId};
use crate::tree::jc_tree::TreeKind;
use crate::tree::tree_info;

use super::type_names::type_name;
use javac_analysis_api::ParameterInformation;

/// How parameter names are rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParameterNames {
    /// Real names only; synthetic `argN`/blank names are omitted (`(Reader)`).
    Real,
    /// Real names; synthetic parameters become `p0`, `p1`, ... (completion).
    Positional,
    /// Whatever the symbol carries (class files yield `arg0`, ...; signature help).
    Verbatim,
}

/// Components of a method or constructor signature.
#[derive(Debug, Clone)]
pub(crate) struct MethodSignature {
    /// `<T, U>`, when the method is generic.
    pub(crate) type_parameters: Option<String>,
    /// Return type; `None` for constructors.
    pub(crate) return_type: Option<String>,
    /// Method name, or the owning class name for constructors.
    pub(crate) name: String,
    /// Rendered `type name` parameters.
    pub(crate) parameters: Vec<String>,
    /// Structured parameters for signature help.
    pub(crate) parameter_info: Vec<ParameterInformation>,
}

impl MethodSignature {
    /// `[<T>] [ReturnType] name(params)`.
    pub(crate) fn label(&self) -> String {
        let mut text = String::new();
        if let Some(type_parameters) = &self.type_parameters {
            text.push_str(type_parameters);
            text.push(' ');
        }
        if let Some(return_type) = &self.return_type {
            text.push_str(return_type);
            text.push(' ');
        }
        text.push_str(&self.name);
        text.push_str(&self.parameter_list());
        text
    }

    /// `(type name, ...)`.
    pub(crate) fn parameter_list(&self) -> String {
        format!("({})", self.parameters.join(", "))
    }
}

fn synthetic(name: &str) -> bool {
    name.is_empty()
        || name
            .strip_prefix("arg")
            .map(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or(false)
}

fn parameter_name(raw: &str, index: usize, names: ParameterNames) -> Option<String> {
    if !synthetic(raw) {
        return Some(raw.to_string());
    }
    match names {
        ParameterNames::Real => None,
        ParameterNames::Positional => Some(format!("p{index}")),
        ParameterNames::Verbatim => Some(if raw.is_empty() {
            format!("arg{index}")
        } else {
            raw.to_string()
        }),
    }
}

fn varargs_type(ctx: &CompilerContext, ty: crate::ids::TypeId) -> String {
    match &ctx.typ(ty).kind {
        TypeKindData::ArrayType {
            elemtype: Some(elemtype),
            ..
        } if ctx.typ(ty).tag == crate::code::type_tag::TypeTag::ARRAY => {
            format!("{}...", type_name(ctx, *elemtype))
        }
        _ => type_name(ctx, ty),
    }
}

pub(crate) fn varargs_element_type(ctx: &CompilerContext, ty: crate::ids::TypeId) -> String {
    match &ctx.typ(ty).kind {
        TypeKindData::ArrayType {
            elemtype: Some(elemtype),
            ..
        } if ctx.typ(ty).tag == crate::code::type_tag::TypeTag::ARRAY => {
            type_name(ctx, *elemtype)
        }
        _ => type_name(ctx, ty),
    }
}

fn owner_simple_name(ctx: &CompilerContext, symbol: SymbolId) -> Option<String> {
    let owner = ctx.sym(symbol).owner?;
    let SymbolKind::ClassSymbol { flatname, .. } = &ctx.sym(owner).kind_data else {
        return None;
    };
    flatname
        .to_string()
        .rsplit(['.', '$'])
        .next()
        .map(str::to_string)
}

/// Signature of a method or constructor from its compiler symbol. This is the
/// single renderer used by hover, completion, signature help and the outline.
pub(crate) fn signature_parts(
    ctx: &CompilerContext,
    symbol: SymbolId,
    names: ParameterNames,
) -> Option<MethodSignature> {
    let record = ctx.sym(symbol);
    let SymbolKind::MethodSymbol { params, .. } = &record.kind_data else {
        return None;
    };
    let raw_name = record.name.to_java_string().to_string_lossy().to_string();
    let constructor = raw_name == "<init>";
    let name = if constructor {
        owner_simple_name(ctx, symbol).unwrap_or_else(|| raw_name.clone())
    } else {
        raw_name.clone()
    };

    // Generic methods carry a ForAll wrapper; unwrap it for the return and
    // parameter types and render the type variables as the prefix.
    let mut type_parameters = None;
    let mut method_type = record.ty;
    if let Some(ty) = method_type {
        if let TypeKindData::ForAll { tvars, qtype } = &ctx.typ(ty).kind {
            let names: Vec<String> = tvars.iter().map(|tvar| type_name(ctx, tvar)).collect();
            if !names.is_empty() {
                type_parameters = Some(format!("<{}>", names.join(", ")));
            }
            method_type = *qtype;
        }
    }
    let (argtypes, restype) = match method_type.map(|ty| &ctx.typ(ty).kind) {
        Some(TypeKindData::MethodType {
            argtypes, restype, ..
        })
        | Some(TypeKindData::PartiallyInferredMethodType {
            argtypes, restype, ..
        }) => (argtypes.clone(), *restype),
        _ => (crate::util::list::List::nil(), None),
    };

    let varargs = record.flags_field & (flags::VARARGS as i64) != 0;
    let mut parameters = Vec::new();
    let mut parameter_info = Vec::new();
    let parameter_symbols: Vec<SymbolId> = params.iter().collect();
    if !parameter_symbols.is_empty() {
        let count = parameter_symbols.len();
        for (index, param) in parameter_symbols.into_iter().enumerate() {
            let param_record = ctx.sym(param);
            let ty_id = param_record.ty.or_else(|| {
                (index < argtypes.length() as usize)
                    .then(|| argtypes.get(index as i32))
            });
            let mut ty = ty_id
                .map(|ty| type_name(ctx, ty))
                .unwrap_or_else(|| "?".to_string());
            let is_varargs = varargs && index + 1 == count;
            if is_varargs {
                if let Some(ty_id) = ty_id {
                    ty = varargs_type(ctx, ty_id);
                }
            }
            let raw = param_record.name.to_java_string().to_string_lossy();
            let param_name = parameter_name(&raw, index, names);
            parameter_info.push(ParameterInformation {
                type_name: ty_id
                    .map(|ty_id| {
                        if is_varargs {
                            varargs_element_type(ctx, ty_id)
                        } else {
                            type_name(ctx, ty_id)
                        }
                    })
                    .unwrap_or_else(|| "?".to_string()),
                name: param_name.clone(),
                varargs: is_varargs,
            });
            parameters.push(match param_name {
                Some(param_name) => format!("{ty} {param_name}"),
                None => ty,
            });
        }
    } else {
        let count = argtypes.length() as usize;
        for (index, ty) in argtypes.iter().enumerate() {
            let is_varargs = varargs && index + 1 == count;
            let mut text = type_name(ctx, ty);
            if is_varargs {
                text = varargs_type(ctx, ty);
            }
            let param_name = match names {
                ParameterNames::Real => None,
                ParameterNames::Positional => Some(format!("p{index}")),
                ParameterNames::Verbatim => Some(format!("arg{index}")),
            };
            parameter_info.push(ParameterInformation {
                type_name: if is_varargs {
                    varargs_element_type(ctx, ty)
                } else {
                    type_name(ctx, ty)
                },
                name: param_name.clone(),
                varargs: is_varargs,
            });
            parameters.push(match param_name {
                Some(param_name) => format!("{text} {param_name}"),
                None => text,
            });
        }
    }

    let return_type = if constructor {
        None
    } else {
        Some(
            restype
                .map(|restype| type_name(ctx, restype))
                .unwrap_or_else(|| "void".to_string()),
        )
    };
    Some(MethodSignature {
        type_parameters,
        return_type,
        name,
        parameters,
        parameter_info,
    })
}

/// Modifiers of a symbol, for outline entries.
pub(crate) fn modifiers_for_symbol(ctx: &CompilerContext, symbol: SymbolId) -> String {
    tree_info::flag_names(ctx.sym(symbol).flags_field)
        .to_string_lossy()
        .trim()
        .to_string()
}

fn modifiers(ctx: &CompilerContext, mods: Option<TreeId>) -> String {
    let flags_value = mods
        .map(|mods| match &ctx.tree(mods).kind {
            TreeKind::Modifiers { flags, .. } => *flags,
            _ => 0,
        })
        .unwrap_or(0);
    tree_info::flag_names(flags_value)
        .to_string_lossy()
        .trim()
        .to_string()
}

fn type_of(ctx: &CompilerContext, tree: Option<TreeId>) -> Option<String> {
    tree.and_then(|tree| ctx.tree(tree).ty)
        .map(|ty| type_name(ctx, ty))
        .filter(|text| text != "?")
}

fn type_parameter_names(ctx: &CompilerContext, typarams: &crate::util::list::List<TreeId>) -> String {
    let names: Vec<String> = typarams
        .iter()
        .filter_map(|param| match &ctx.tree(param).kind {
            TreeKind::TypeParameter { name, .. } => {
                Some(name.to_java_string().to_string_lossy())
            }
            _ => None,
        })
        .collect();
    if names.is_empty() {
        String::new()
    } else {
        format!("<{}>", names.join(", "))
    }
}

fn join_modifiers(mods: &str, rest: String) -> String {
    if mods.is_empty() {
        rest
    } else {
        format!("{mods} {rest}")
    }
}

/// Signature of a class, interface, enum or record declaration.
pub(crate) fn class_signature(ctx: &CompilerContext, tree: TreeId) -> Option<String> {
    let TreeKind::ClassDecl {
        mods,
        name,
        typarams,
        extending,
        implementing,
        ..
    } = &ctx.tree(tree).kind
    else {
        return None;
    };
    let flags_value = mods
        .map(|mods| match &ctx.tree(mods).kind {
            TreeKind::Modifiers { flags, .. } => *flags,
            _ => 0,
        })
        .unwrap_or(0);
    let keyword = if (flags_value & crate::code::flags::INTERFACE as i64) != 0 {
        "interface"
    } else if (flags_value & crate::code::flags::ENUM as i64) != 0 {
        "enum"
    } else if (flags_value & crate::code::flags::RECORD as i64) != 0 {
        "record"
    } else {
        "class"
    };
    let mut text = format!(
        "{keyword} {name}{params}",
        name = name.to_java_string().to_string_lossy(),
        params = type_parameter_names(ctx, typarams)
    );
    if extending.is_some() {
        if let Some(extending_text) = type_of(ctx, *extending) {
            text.push_str(&format!(" extends {extending_text}"));
        }
    }
    let interfaces: Vec<String> = implementing
        .iter()
        .filter_map(|tree| type_of(ctx, Some(tree)))
        .collect();
    if !interfaces.is_empty() {
        text.push_str(&format!(" implements {}", interfaces.join(", ")));
    }
    Some(join_modifiers(&modifiers(ctx, *mods), text))
}

/// Signature of a field or local variable declaration.
pub(crate) fn field_signature(ctx: &CompilerContext, tree: TreeId) -> Option<String> {
    let TreeKind::VariableDecl {
        mods,
        name,
        vartype,
        ..
    } = &ctx.tree(tree).kind
    else {
        return None;
    };
    let ty = type_of(ctx, *vartype).unwrap_or_else(|| "?".to_string());
    let text = format!("{ty} {}", name.to_java_string().to_string_lossy());
    Some(join_modifiers(&modifiers(ctx, *mods), text))
}
