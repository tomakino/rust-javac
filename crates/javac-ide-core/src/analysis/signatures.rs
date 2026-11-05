//! IDEA-style declaration signatures for hover and completion.

use crate::code::flags;
use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::tree::jc_tree::TreeKind;
use crate::tree::tree_info;

use super::type_names::type_name;

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

/// Signature of a method or constructor declaration.
pub(crate) fn method_signature(ctx: &CompilerContext, tree: TreeId) -> Option<String> {
    let TreeKind::MethodDecl {
        mods,
        name,
        restype,
        params,
        thrown,
        ..
    } = &ctx.tree(tree).kind
    else {
        return None;
    };
    let method_name = name.to_java_string().to_string_lossy();
    let parameters: Vec<String> = params
        .iter()
        .filter_map(|param| match &ctx.tree(param).kind {
            TreeKind::VariableDecl {
                vartype, name, ..
            } => {
                let ty = type_of(ctx, *vartype).unwrap_or_else(|| "?".to_string());
                Some(format!(
                    "{ty} {}",
                    name.to_java_string().to_string_lossy()
                ))
            }
            _ => None,
        })
        .collect();
    let signature = match type_of(ctx, *restype) {
        Some(ret) => format!("{ret} {method_name}({})", parameters.join(", ")),
        None => format!("{method_name}({})", parameters.join(", ")),
    };
    let thrown_types: Vec<String> = thrown
        .iter()
        .filter_map(|tree| type_of(ctx, Some(tree)))
        .collect();
    let mut text = signature;
    if !thrown_types.is_empty() {
        text.push_str(&format!(" throws {}", thrown_types.join(", ")));
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
