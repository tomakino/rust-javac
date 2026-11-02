//! Diagnostic display helpers for types.

use crate::compiler_context::CompilerContext;
use crate::ids::TypeId;

/// Diagnostic display of a type: javac's default formatter renders Type
/// arguments with the rich Printer (simple class names when unambiguous).
pub(crate) fn type_display(ctx: &CompilerContext, ty: TypeId) -> String {
    crate::code::printer::visit_type(ctx, ty)
}


/// helper: diagnostic display text of a type for the Infer/Attr
/// adapter argument placeholders (adapter, the type printer is pending).
pub(crate) fn type_display_of(ctx: &CompilerContext, t: TypeId) -> String {
    type_display(ctx, t)
}

/// Java: Printer.visitAnnotationType -- an annotation type is printed as
/// @Name, retaining the enclosing class names but not the package.
pub(crate) fn annotation_type_display(ctx: &CompilerContext, t: TypeId) -> String {
    let Some(tsym) = ctx.typ(t).tsym else {
        return format!("@{}", type_display(ctx, t));
    };
    let mut name = ctx.sym(tsym).name.to_java_string().to_string_lossy();
    let mut owner = ctx.sym(tsym).owner;
    while let Some(owner_sym) = owner {
        if ctx.sym(owner_sym).kind != crate::code::kinds::Kind::TYP {
            break;
        }
        name = format!(
            "{}.{}",
            ctx.sym(owner_sym).name.to_java_string().to_string_lossy(),
            name
        );
        owner = ctx.sym(owner_sym).owner;
    }
    format!("@{name}")
}

