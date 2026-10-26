// Origin: com/sun/tools/javac/tree/TreeInfo.java @ 982d53c (jdk-17.0.19-ga)
// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: tree (tree nodes/scanners for the corresponding upstream feature).
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: getTree (TreeInfo.java:663)
// - NOT PORTED: getStartPosition (TreeInfo.java:664)
// - NOT PORTED: getPreferredPosition (TreeInfo.java:665)
// - NOT PORTED: getEndPosition (TreeInfo.java:666)
// - NOT PORTED: visitIdent (TreeInfo.java:721)
// - NOT PORTED: visitSelect (TreeInfo.java:725)
// - NOT PORTED: scan (TreeInfo.java:749)
// - NOT PORTED: visitTopLevel (TreeInfo.java:753)
// - NOT PORTED: visitModuleDef (TreeInfo.java:757)
// - NOT PORTED: visitPackageDef (TreeInfo.java:761)
// - NOT PORTED: visitClassDef (TreeInfo.java:765)
// - NOT PORTED: visitMethodDef (TreeInfo.java:769)
// - NOT PORTED: visitVarDef (TreeInfo.java:773)
// - NOT PORTED: visitTypeParameter (TreeInfo.java:777)
// - NOT PORTED: scan (TreeInfo.java:810)
// - NOT PORTED: scan (TreeInfo.java:1308)
// - NOT PORTED: visitAnnotation (TreeInfo.java:1314)
// End of Gap registration.

//! Utility class containing inspector methods for trees.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES: ADR 0001 (arena + IDs):
//! - the upstream static methods become free functions; functions that inspect
//!   child/symbol/type fields take (ctx: &CompilerContext, tree: TreeId) (the
//!   Java static methods had the object graph at hand);
//! - JCTree parameters/returns -> TreeId / Option<TreeId> (Java null ->
//!   Option); List<JCTree> -> List<TreeId>; Symbol -> Option<SymbolId>;
//!   Type -> Option<TypeId>; Name -> Option<Name> where the upstream returns
//!   null;
//! - Name comparisons use the content (Name::to_java_string) because the same
//!   name may live in different name tables in the adapter tests; the upstream
//!   compares table-interned Name identities (registered section 9).
//!
//! NOT PORTED IN THIS STEP (dependencies are not ported yet; no placeholder
//! logic is written, tracked in docs/STATUS.md):
//! - Env/AttrContext-dependent: isInAnnotation, getCommentText,
//!   getCommentTree, scopeFor, pathFor, declarationFor, positionFor,
//!   diagnosticPositionFor (they need Env/AttrContext, TreeScanner,
//!   DiagnosticPosition and DCTree);
//! - diagEndPos (returns a DiagnosticPosition anonymous class; the
//!   JCDiagnostic/DiagnosticPosition layer is not ported yet);
//! - containsTypeAnnotation (needs the internal TreeScanner);
//! - flagNames (Flags.toString is not ported yet).

// The upstream algorithms are copied with their exact if/match nesting; the
// clippy style suggestions to collapse them are not applied (registered in
// PORTING_RULES section 9: control flow is kept 1:1 with the upstream source).
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::question_mark)]

use javac_source::tree::tree::Kind;
use javac_tools::tools::java_file_object::Kind as JavaFileObjectKind;

use crate::code::flags;
use crate::code::kinds::Kind as KindsKind;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::tree::end_pos_table::EndPosTable;
use crate::tree::jc_tree::{PolyKind, Tag, TreeKind};
use crate::util::list::List;
use crate::util::name::Name;
use crate::util::position::NOPOS;

/// PORTING_RULES adapter: the upstream List API is accessed through the ported
/// util::list::List; this helper trait adds the nullable-head/tail accessors
/// used by the copied algorithms (Java: List.head/list.last with empty checks).
trait ListExt<A> {
    fn head_opt(&self) -> Option<A>;
    fn last_opt(&self) -> Option<A>;
}

impl<A: Copy> ListExt<A> for List<A> {
    fn head_opt(&self) -> Option<A> {
        if self.non_empty() {
            Some(*self.head())
        } else {
            None
        }
    }

    fn last_opt(&self) -> Option<A> {
        self.last()
    }
}

/// Java: public static List<JCExpression> args(JCTree t)
pub fn args(ctx: &CompilerContext, tree: TreeId) -> Option<List<TreeId>> {
    match &ctx.tree(tree).kind {
        TreeKind::MethodInvocation { args, .. } => Some(args.clone()),
        TreeKind::NewClass { args, .. } => Some(args.clone()),
        _ => None,
    }
}

/// Is tree a constructor declaration?
pub fn is_constructor(ctx: &CompilerContext, tree: TreeId) -> bool {
    let node = ctx.tree(tree);
    if node.has_tag(Tag::METHODDEF) {
        if let TreeKind::MethodDecl { name, .. } = &node.kind {
            let names = name.table.names();
            return name.to_java_string() == names.init.to_java_string();
        }
    }
    false
}

/// Java: public static boolean isCanonicalConstructor(JCTree tree)
pub fn is_canonical_constructor(ctx: &CompilerContext, tree: TreeId) -> bool {
    if !is_constructor(ctx, tree) {
        return false;
    }
    match &ctx.tree(tree).kind {
        TreeKind::MethodDecl { sym, .. } => sym
            .map(|sym| (ctx.sym(sym).flags() & flags::RECORD) != 0)
            .unwrap_or(false),
        _ => false,
    }
}

/// Java: public static boolean isCompactConstructor(JCTree tree)
pub fn is_compact_constructor(ctx: &CompilerContext, tree: TreeId) -> bool {
    if !is_canonical_constructor(ctx, tree) {
        return false;
    }
    match &ctx.tree(tree).kind {
        TreeKind::MethodDecl { sym, .. } => sym
            .map(|sym| (ctx.sym(sym).flags() & flags::COMPACT_RECORD_CONSTRUCTOR) != 0)
            .unwrap_or(false),
        _ => false,
    }
}

/// Java: public static boolean isReceiverParam(JCTree tree)
pub fn is_receiver_param(ctx: &CompilerContext, tree: TreeId) -> bool {
    match &ctx.tree(tree).kind {
        TreeKind::VariableDecl { nameexpr, .. } => nameexpr.is_some(),
        _ => false,
    }
}

/// Is there a constructor declaration in the given list of trees?
pub fn has_constructors(ctx: &CompilerContext, trees: &List<TreeId>) -> bool {
    for tree in trees.iter() {
        if is_constructor(ctx, tree) {
            return true;
        }
    }
    false
}

/// Is there a constructor invocation in the given list of trees?
///
/// PORTING_RULES: the upstream returns names.empty when no call is found; the
/// adapter returns None for that case.
pub fn get_constructor_invocation_name(
    ctx: &CompilerContext,
    trees: &List<TreeId>,
) -> Option<Name> {
    for tree in trees.iter() {
        let node = ctx.tree(tree);
        if !node.has_tag(Tag::EXEC) {
            continue;
        }
        let TreeKind::ExpressionStatement { expr } = &node.kind else {
            continue;
        };
        let Some(expr) = *expr else {
            continue;
        };
        let expr_node = ctx.tree(expr);
        if !expr_node.has_tag(Tag::APPLY) {
            continue;
        }
        let TreeKind::MethodInvocation { meth, .. } = &expr_node.kind else {
            continue;
        };
        if let Some(meth_name) = meth.and_then(|meth| name(ctx, meth)) {
            let names = meth_name.table.names();
            let text = meth_name.to_java_string();
            if text == names._this.to_java_string() || text == names._super.to_java_string() {
                return Some(meth_name);
            }
        }
    }
    None
}

/// Java: public static boolean isMultiCatch(JCCatch catchClause)
pub fn is_multi_catch(ctx: &CompilerContext, catch_clause: TreeId) -> bool {
    match &ctx.tree(catch_clause).kind {
        TreeKind::Catch { param, .. } => match param.and_then(|param| match &ctx.tree(param).kind {
            TreeKind::VariableDecl { vartype, .. } => *vartype,
            _ => None,
        }) {
            Some(vartype) => ctx.tree(vartype).has_tag(Tag::TYPEUNION),
            None => false,
        },
        _ => false,
    }
}

/// Is statement an initializer for a synthetic field?
pub fn is_synthetic_init(ctx: &CompilerContext, stat: TreeId) -> bool {
    let node = ctx.tree(stat);
    if !node.has_tag(Tag::EXEC) {
        return false;
    }
    let TreeKind::ExpressionStatement { expr } = &node.kind else {
        return false;
    };
    let Some(expr) = *expr else {
        return false;
    };
    let expr_node = ctx.tree(expr);
    if !expr_node.has_tag(Tag::ASSIGN) {
        return false;
    }
    let TreeKind::Assign { lhs, .. } = &expr_node.kind else {
        return false;
    };
    let Some(lhs) = *lhs else {
        return false;
    };
    let lhs_node = ctx.tree(lhs);
    if !lhs_node.has_tag(Tag::SELECT) {
        return false;
    }
    let TreeKind::FieldAccess { selected, sym, .. } = &lhs_node.kind else {
        return false;
    };
    let Some(sym) = *sym else {
        return false;
    };
    if (ctx.sym(sym).flags() & flags::SYNTHETIC as i64) == 0 {
        return false;
    }
    match selected.and_then(|selected| name(ctx, selected)) {
        Some(selected_name) => {
            selected_name.to_java_string() == selected_name.table.names()._this.to_java_string()
        }
        None => false,
    }
}

/// If the expression is a method call, return the method name, null otherwise.
pub fn called_method_name(ctx: &CompilerContext, tree: TreeId) -> Option<Name> {
    let node = ctx.tree(tree);
    if !node.has_tag(Tag::EXEC) {
        return None;
    }
    let TreeKind::ExpressionStatement { expr } = &node.kind else {
        return None;
    };
    let expr = expr.as_ref()?;
    let expr_node = ctx.tree(*expr);
    if !expr_node.has_tag(Tag::APPLY) {
        return None;
    }
    let TreeKind::MethodInvocation { meth, .. } = &expr_node.kind else {
        return None;
    };
    meth.and_then(|meth| name(ctx, meth))
}

/// Is this a call to this or super?
pub fn is_self_call(ctx: &CompilerContext, tree: TreeId) -> bool {
    match called_method_name(ctx, tree) {
        Some(name) => {
            let names = name.table.names();
            let text = name.to_java_string();
            text == names._this.to_java_string() || text == names._super.to_java_string()
        }
        None => false,
    }
}

/// Is this tree a 'this' identifier?
pub fn is_this_qualifier(ctx: &CompilerContext, tree: TreeId) -> bool {
    match &ctx.tree(tree).kind {
        TreeKind::Parens { expr } => match expr {
            Some(expr) => is_this_qualifier(ctx, skip_parens(ctx, *expr)),
            None => false,
        },
        TreeKind::Ident { name, .. } => {
            name.to_java_string() == name.table.names()._this.to_java_string()
        }
        _ => false,
    }
}

/// Is this tree an identifier, possibly qualified by 'this'?
pub fn is_ident_or_this_dot_ident(ctx: &CompilerContext, tree: TreeId) -> bool {
    match &ctx.tree(tree).kind {
        TreeKind::Parens { expr } => match expr {
            Some(expr) => is_ident_or_this_dot_ident(ctx, skip_parens(ctx, *expr)),
            None => false,
        },
        TreeKind::Ident { .. } => true,
        TreeKind::FieldAccess { selected, .. } => match selected {
            Some(selected) => is_this_qualifier(ctx, *selected),
            None => false,
        },
        _ => false,
    }
}

/// Is this a call to super?
pub fn is_super_call(ctx: &CompilerContext, tree: TreeId) -> bool {
    match called_method_name(ctx, tree) {
        Some(name) => name.to_java_string() == name.table.names()._super.to_java_string(),
        None => false,
    }
}

/// Java: public static List<JCVariableDecl> recordFields(JCClassDecl tree)
pub fn record_fields(ctx: &CompilerContext, tree: TreeId) -> List<TreeId> {
    let mut result = List::nil();
    let TreeKind::ClassDecl { defs, .. } = &ctx.tree(tree).kind else {
        return result;
    };
    let defs = defs.clone();
    for def in defs.iter() {
        let node = ctx.tree(def);
        if !node.has_tag(Tag::VARDEF) {
            continue;
        }
        let TreeKind::VariableDecl { mods, .. } = &node.kind else {
            continue;
        };
        let Some(mods) = *mods else {
            continue;
        };
        let TreeKind::Modifiers {
            flags: mod_flags, ..
        } = &ctx.tree(mods).kind
        else {
            continue;
        };
        if (mod_flags & flags::RECORD) == flags::RECORD {
            result = result.append(def);
        }
    }
    result
}

/// Java: public static List<Type> recordFieldTypes(JCClassDecl tree)
///
/// PORTING_RULES: JCVariableDecl.type may be null before Attr, so the result is
/// List<Option<TypeId>> (the upstream List<Type> may contain null).
pub fn record_field_types(ctx: &CompilerContext, tree: TreeId) -> List<Option<TypeId>> {
    let mut result = List::nil();
    for field in record_fields(ctx, tree).iter() {
        let node = ctx.tree(field);
        let TreeKind::VariableDecl { .. } = &node.kind else {
            continue;
        };
        result = result.append(node.ty);
    }
    result
}

/// Return the first call in a constructor definition.
pub fn first_constructor_call(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    let node = ctx.tree(tree);
    if !node.has_tag(Tag::METHODDEF) {
        return None;
    }
    let TreeKind::MethodDecl { name, body, .. } = &node.kind else {
        return None;
    };
    let names = name.table.names();
    if name.to_java_string() != names.init.to_java_string() {
        return None;
    }
    let body = (*body)?;
    let TreeKind::Block { stats, .. } = &ctx.tree(body).kind else {
        return None;
    };
    let mut stats = stats.clone();
    while let Some(head) = stats.head_opt() {
        if is_synthetic_init(ctx, head) {
            stats = stats.tail();
        } else {
            break;
        }
    }
    let Some(head) = stats.head_opt() else {
        return None;
    };
    let head_node = ctx.tree(head);
    if !head_node.has_tag(Tag::EXEC) {
        return None;
    }
    let TreeKind::ExpressionStatement { expr } = &head_node.kind else {
        return None;
    };
    let expr = (*expr)?;
    if !ctx.tree(expr).has_tag(Tag::APPLY) {
        return None;
    }
    Some(expr)
}

/// Is this a constructor whose first (non-synthetic) statement is not of the
/// form this(...)?
pub fn is_initial_constructor(ctx: &CompilerContext, tree: TreeId) -> bool {
    let Some(app) = first_constructor_call(ctx, tree) else {
        return false;
    };
    let TreeKind::MethodInvocation { meth, .. } = &ctx.tree(app).kind else {
        return false;
    };
    match meth.and_then(|meth| name(ctx, meth)) {
        None => true,
        Some(meth) => meth.to_java_string() != meth.table.names()._this.to_java_string(),
    }
}

/// Return true if a tree represents a diamond new expr.
pub fn is_diamond(ctx: &CompilerContext, tree: TreeId) -> bool {
    match &ctx.tree(tree).kind {
        TreeKind::TypeApply { arguments, .. } => arguments.is_empty(),
        TreeKind::NewClass { clazz, .. } => match clazz {
            Some(clazz) => is_diamond(ctx, *clazz),
            None => false,
        },
        TreeKind::AnnotatedType {
            underlying_type, ..
        } => match underlying_type {
            Some(underlying_type) => is_diamond(ctx, *underlying_type),
            None => false,
        },
        _ => false,
    }
}

/// Java: public static boolean isEnumInit(JCTree tree)
pub fn is_enum_init(ctx: &CompilerContext, tree: TreeId) -> bool {
    let node = ctx.tree(tree);
    if !node.has_tag(Tag::VARDEF) {
        return false;
    }
    let TreeKind::VariableDecl { mods, .. } = &node.kind else {
        return false;
    };
    let Some(mods) = *mods else {
        return false;
    };
    match &ctx.tree(mods).kind {
        TreeKind::Modifiers {
            flags: mod_flags, ..
        } => (mod_flags & flags::ENUM as i64) != 0,
        _ => false,
    }
}

/// Set 'polyKind' on given tree.
pub fn set_poly_kind(ctx: &mut CompilerContext, tree: TreeId, pkind: PolyKind) {
    match &mut ctx.tree_mut(tree).kind {
        TreeKind::MethodInvocation { poly_kind, .. } => *poly_kind = pkind,
        TreeKind::NewClass { poly_kind, .. } => *poly_kind = pkind,
        TreeKind::MemberReference { ref_poly_kind, .. } => *ref_poly_kind = pkind,
        _ => panic!("Unexpected tree: {:?}", tree),
    }
}

/// Set 'varargsElement' on given tree.
pub fn set_varargs_element(
    ctx: &mut CompilerContext,
    tree: TreeId,
    varargs_element: Option<TypeId>,
) {
    match &mut ctx.tree_mut(tree).kind {
        TreeKind::MethodInvocation {
            varargs_element: slot,
            ..
        } => *slot = varargs_element,
        TreeKind::NewClass {
            varargs_element: slot,
            ..
        } => *slot = varargs_element,
        TreeKind::MemberReference {
            varargs_element: slot,
            ..
        } => *slot = varargs_element,
        _ => panic!("Unexpected tree: {:?}", tree),
    }
}

/// Return true if the tree corresponds to an expression statement.
pub fn is_expression_statement(ctx: &CompilerContext, tree: TreeId) -> bool {
    matches!(
        ctx.tree(tree).tag,
        Tag::PREINC
            | Tag::PREDEC
            | Tag::POSTINC
            | Tag::POSTDEC
            | Tag::ASSIGN
            | Tag::BITOR_ASG
            | Tag::BITXOR_ASG
            | Tag::BITAND_ASG
            | Tag::SL_ASG
            | Tag::SR_ASG
            | Tag::USR_ASG
            | Tag::PLUS_ASG
            | Tag::MINUS_ASG
            | Tag::MUL_ASG
            | Tag::DIV_ASG
            | Tag::MOD_ASG
            | Tag::APPLY
            | Tag::NEWCLASS
            | Tag::ERRONEOUS
    )
}

/// Return true if the tree corresponds to a statement.
pub fn is_statement(ctx: &CompilerContext, tree: TreeId) -> bool {
    let node = ctx.tree(tree);
    let is_statement_node = matches!(
        node.kind,
        TreeKind::ClassDecl { .. }
            | TreeKind::MethodDecl { .. }
            | TreeKind::VariableDecl { .. }
            | TreeKind::Skip {}
            | TreeKind::Block { .. }
            | TreeKind::DoWhileLoop { .. }
            | TreeKind::WhileLoop { .. }
            | TreeKind::ForLoop { .. }
            | TreeKind::EnhancedForLoop { .. }
            | TreeKind::LabeledStatement { .. }
            | TreeKind::Switch { .. }
            | TreeKind::SwitchExpression { .. }
            | TreeKind::Synchronized { .. }
            | TreeKind::Try { .. }
            | TreeKind::Catch { .. }
            | TreeKind::If { .. }
            | TreeKind::ExpressionStatement { .. }
            | TreeKind::Break { .. }
            | TreeKind::Yield { .. }
            | TreeKind::Continue { .. }
            | TreeKind::Return { .. }
            | TreeKind::Throw { .. }
            | TreeKind::Assert { .. }
    );
    is_statement_node
        && !node.has_tag(Tag::CLASSDEF)
        && !node.has_tag(Tag::BLOCK)
        && !node.has_tag(Tag::METHODDEF)
}

/// Return true if the AST corresponds to a static select of the kind A.B.
pub fn is_static_selector(
    ctx: &CompilerContext,
    base: Option<TreeId>,
    names: &crate::util::names::Names,
) -> bool {
    let Some(base) = base else {
        return false;
    };
    match &ctx.tree(base).kind {
        TreeKind::Ident { name, .. } => {
            name.to_java_string() != names._this.to_java_string()
                && name.to_java_string() != names._super.to_java_string()
                && is_static_sym(ctx, base)
        }
        TreeKind::FieldAccess { selected, .. } => {
            is_static_sym(ctx, base) && is_static_selector(ctx, *selected, names)
        }
        TreeKind::TypeApply { .. } | TreeKind::ArrayTypeTree { .. } => true,
        TreeKind::AnnotatedType {
            underlying_type, ..
        } => is_static_selector(ctx, *underlying_type, names),
        _ => false,
    }
}

fn is_static_sym(ctx: &CompilerContext, tree: TreeId) -> bool {
    match symbol(ctx, tree) {
        Some(sym) => {
            let kind = ctx.sym(sym).kind;
            kind == KindsKind::TYP || kind == KindsKind::PCK
        }
        None => false,
    }
}

/// Return true if a tree represents the null literal.
pub fn is_null(ctx: &CompilerContext, tree: TreeId) -> bool {
    match &ctx.tree(tree).kind {
        TreeKind::Literal { typetag, .. } => *typetag == crate::code::type_tag::TypeTag::BOT,
        _ => false,
    }
}

/// The position of the first statement in a block, or the position of the
/// block itself if it is empty.
pub fn first_stat_pos(ctx: &CompilerContext, tree: TreeId) -> i32 {
    let node = ctx.tree(tree);
    if node.has_tag(Tag::BLOCK) {
        if let TreeKind::Block { stats, .. } = &node.kind {
            if let Some(head) = stats.head_opt() {
                return ctx.tree(head).pos;
            }
        }
    }
    node.pos
}

/// The end position of given tree, if it is a block with defined endpos.
pub fn end_pos(ctx: &CompilerContext, tree: TreeId) -> i32 {
    let node = ctx.tree(tree);
    if node.has_tag(Tag::BLOCK) {
        if let TreeKind::Block { endpos, .. } = &node.kind {
            if *endpos != NOPOS {
                return *endpos;
            }
        }
    } else if node.has_tag(Tag::SYNCHRONIZED) {
        if let TreeKind::Synchronized { body, .. } = &node.kind {
            if let Some(body) = *body {
                return end_pos(ctx, body);
            }
        }
    } else if node.has_tag(Tag::TRY) {
        if let TreeKind::Try {
            body,
            catchers,
            finalizer,
            ..
        } = &node.kind
        {
            if let Some(finalizer) = *finalizer {
                return end_pos(ctx, finalizer);
            }
            if let Some(last) = catchers.last_opt() {
                if let TreeKind::Catch {
                    body: catch_body, ..
                } = &ctx.tree(last).kind
                {
                    if let Some(catch_body) = *catch_body {
                        return end_pos(ctx, catch_body);
                    }
                }
            }
            if let Some(body) = *body {
                return end_pos(ctx, body);
            }
        }
    } else if node.has_tag(Tag::SWITCH) {
        if let TreeKind::Switch { endpos, .. } = &node.kind {
            if *endpos != NOPOS {
                return *endpos;
            }
        }
    } else if node.has_tag(Tag::SWITCH_EXPRESSION) {
        if let TreeKind::SwitchExpression { endpos, .. } = &node.kind {
            if *endpos != NOPOS {
                return *endpos;
            }
        }
    }
    node.pos
}

/// Get the start position for a tree node.
pub fn get_start_pos(ctx: &CompilerContext, tree: TreeId) -> i32 {
    get_start_pos_opt(ctx, Some(tree))
}

fn get_start_pos_opt(ctx: &CompilerContext, tree: Option<TreeId>) -> i32 {
    let Some(tree) = tree else {
        return NOPOS;
    };
    let node = ctx.tree(tree);
    match &node.kind {
        TreeKind::ModuleDecl { mods, .. } => {
            let Some(mods) = *mods else {
                return node.pos;
            };
            match &ctx.tree(mods).kind {
                TreeKind::Modifiers { annotations, .. } => {
                    if annotations.is_empty() {
                        node.pos
                    } else {
                        get_start_pos(ctx, *annotations.head())
                    }
                }
                _ => node.pos,
            }
        }
        TreeKind::PackageDecl { annotations, .. } => {
            if annotations.is_empty() {
                node.pos
            } else {
                get_start_pos(ctx, *annotations.head())
            }
        }
        TreeKind::MethodInvocation { meth, .. } => get_start_pos_opt(ctx, *meth),
        TreeKind::Assign { lhs, .. } => get_start_pos_opt(ctx, *lhs),
        TreeKind::AssignOp { lhs, .. } => get_start_pos_opt(ctx, *lhs),
        TreeKind::Binary { lhs, .. } => get_start_pos_opt(ctx, *lhs),
        TreeKind::Unary { arg, .. } => get_start_pos_opt(ctx, *arg),
        TreeKind::ClassDecl { mods, .. } => {
            if let Some(mods) = *mods {
                if ctx.tree(mods).pos != NOPOS {
                    return ctx.tree(mods).pos;
                }
            }
            node.pos
        }
        TreeKind::Conditional { cond, .. } => get_start_pos_opt(ctx, *cond),
        TreeKind::ExpressionStatement { expr } => get_start_pos_opt(ctx, *expr),
        TreeKind::ArrayAccess { indexed, .. } => get_start_pos_opt(ctx, *indexed),
        TreeKind::MethodDecl {
            mods,
            typarams,
            restype,
            ..
        } => {
            if let Some(mods) = *mods {
                if ctx.tree(mods).pos != NOPOS {
                    return ctx.tree(mods).pos;
                }
            }
            if let Some(head) = typarams.head_opt() {
                return get_start_pos(ctx, head);
            }
            match restype {
                Some(restype) => get_start_pos(ctx, *restype),
                None => node.pos,
            }
        }
        TreeKind::FieldAccess { selected, .. } => get_start_pos_opt(ctx, *selected),
        TreeKind::TypeApply { clazz, .. } => get_start_pos_opt(ctx, *clazz),
        TreeKind::ArrayTypeTree { elemtype, .. } => get_start_pos_opt(ctx, *elemtype),
        TreeKind::InstanceOf { expr, .. } => get_start_pos_opt(ctx, *expr),
        TreeKind::AnnotatedType {
            annotations,
            underlying_type,
        } => {
            if let Some(head) = annotations.head_opt() {
                let underlying = underlying_type.expect("annotated type has no underlying type");
                if ctx.tree(underlying).has_tag(Tag::TYPEARRAY)
                    || ctx.tree(underlying).has_tag(Tag::SELECT)
                {
                    get_start_pos(ctx, underlying)
                } else {
                    get_start_pos(ctx, head)
                }
            } else {
                get_start_pos_opt(ctx, *underlying_type)
            }
        }
        TreeKind::NewClass { encl, .. } => match encl {
            Some(encl) => get_start_pos(ctx, *encl),
            None => node.pos,
        },
        TreeKind::VariableDecl {
            mods,
            vartype,
            start_pos,
            ..
        } => {
            if *start_pos != NOPOS {
                *start_pos
            } else if let Some(mods) = *mods {
                if ctx.tree(mods).pos != NOPOS {
                    ctx.tree(mods).pos
                } else {
                    vartype_start(ctx, node.pos, *vartype)
                }
            } else {
                vartype_start(ctx, node.pos, *vartype)
            }
        }
        TreeKind::BindingPattern { var } => get_start_pos_opt(ctx, *var),
        TreeKind::GuardPattern { patt, .. } => get_start_pos_opt(ctx, *patt),
        TreeKind::Erroneous { errs } => match errs.head_opt() {
            Some(head) => get_start_pos(ctx, head),
            None => node.pos,
        },
        _ => node.pos,
    }
}

fn vartype_start(ctx: &CompilerContext, node_pos: i32, vartype: Option<TreeId>) -> i32 {
    match vartype {
        Some(vartype) => {
            if ctx.tree(vartype).pos == NOPOS {
                node_pos
            } else {
                get_start_pos(ctx, vartype)
            }
        }
        None => node_pos,
    }
}

/// The end position of given tree, given a table of end positions generated by
/// the parser.
pub fn get_end_pos(
    ctx: &CompilerContext,
    tree: Option<TreeId>,
    end_pos_table: Option<&dyn EndPosTable>,
) -> i32 {
    let Some(tree) = tree else {
        return NOPOS;
    };
    let Some(end_pos_table) = end_pos_table else {
        return end_pos(ctx, tree);
    };
    let map_pos = end_pos_table.get_end_pos(tree);
    if map_pos != NOPOS {
        return map_pos;
    }
    let node = ctx.tree(tree);
    match &node.kind {
        TreeKind::AssignOp { rhs, .. } => get_end_pos(ctx, *rhs, Some(end_pos_table)),
        TreeKind::Binary { rhs, .. } => get_end_pos(ctx, *rhs, Some(end_pos_table)),
        TreeKind::Unary { arg, .. } => get_end_pos(ctx, *arg, Some(end_pos_table)),
        TreeKind::Case { stats, .. } => match stats.last_opt() {
            Some(last) => get_end_pos(ctx, Some(last), Some(end_pos_table)),
            None => NOPOS,
        },
        TreeKind::Catch { body, .. } => get_end_pos(ctx, *body, Some(end_pos_table)),
        TreeKind::Conditional { falsepart, .. } => {
            get_end_pos(ctx, *falsepart, Some(end_pos_table))
        }
        TreeKind::ForLoop { body, .. } => get_end_pos(ctx, *body, Some(end_pos_table)),
        TreeKind::EnhancedForLoop { body, .. } => get_end_pos(ctx, *body, Some(end_pos_table)),
        TreeKind::If {
            thenpart, elsepart, ..
        } => match elsepart {
            Some(elsepart) => get_end_pos(ctx, Some(*elsepart), Some(end_pos_table)),
            None => get_end_pos(ctx, *thenpart, Some(end_pos_table)),
        },
        TreeKind::LabeledStatement { body, .. } => get_end_pos(ctx, *body, Some(end_pos_table)),
        TreeKind::Modifiers { annotations, .. } => match annotations.last_opt() {
            Some(last) => get_end_pos(ctx, Some(last), Some(end_pos_table)),
            None => NOPOS,
        },
        TreeKind::Synchronized { body, .. } => get_end_pos(ctx, *body, Some(end_pos_table)),
        TreeKind::CompilationUnit { defs, .. } => match defs.last_opt() {
            Some(last) => get_end_pos(ctx, Some(last), Some(end_pos_table)),
            None => NOPOS,
        },
        TreeKind::Try {
            body,
            catchers,
            finalizer,
            ..
        } => {
            if let Some(finalizer) = *finalizer {
                get_end_pos(ctx, Some(finalizer), Some(end_pos_table))
            } else if let Some(last) = catchers.last_opt() {
                get_end_pos(ctx, Some(last), Some(end_pos_table))
            } else {
                get_end_pos(ctx, *body, Some(end_pos_table))
            }
        }
        TreeKind::Wildcard { inner, .. } => get_end_pos(ctx, *inner, Some(end_pos_table)),
        TreeKind::TypeCast { expr, .. } => get_end_pos(ctx, *expr, Some(end_pos_table)),
        TreeKind::InstanceOf { pattern, .. } => get_end_pos(ctx, *pattern, Some(end_pos_table)),
        TreeKind::WhileLoop { body, .. } => get_end_pos(ctx, *body, Some(end_pos_table)),
        TreeKind::AnnotatedType {
            underlying_type, ..
        } => get_end_pos(ctx, *underlying_type, Some(end_pos_table)),
        TreeKind::ParenthesizedPattern { pattern, .. } => {
            get_end_pos(ctx, *pattern, Some(end_pos_table))
        }
        TreeKind::GuardPattern { expr, .. } => get_end_pos(ctx, *expr, Some(end_pos_table)),
        TreeKind::Erroneous { errs } => match errs.last_opt() {
            Some(last) => get_end_pos(ctx, Some(last), Some(end_pos_table)),
            None => NOPOS,
        },
        _ => NOPOS,
    }
}

/// The position of the finalizer of given try/synchronized statement.
pub fn finalizer_pos(ctx: &CompilerContext, tree: TreeId, pos_kind: PosKind) -> i32 {
    let node = ctx.tree(tree);
    if node.has_tag(Tag::TRY) {
        if let TreeKind::Try { finalizer, .. } = &node.kind {
            let finalizer = finalizer.expect("try statement has no finalizer");
            return pos_kind.to_pos(ctx, finalizer);
        }
    } else if node.has_tag(Tag::SYNCHRONIZED) {
        if let TreeKind::Synchronized { body, .. } = &node.kind {
            if let Some(body) = *body {
                return end_pos(ctx, body);
            }
        }
    }
    panic!("Unexpected tree: {:?}", tree);
}

/// Java: public enum PosKind (the position functions used by finalizerPos).
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PosKind {
    START_POS,
    FIRST_STAT_POS,
    END_POS,
}

impl PosKind {
    fn to_pos(self, ctx: &CompilerContext, tree: TreeId) -> i32 {
        match self {
            PosKind::START_POS => get_start_pos(ctx, tree),
            PosKind::FIRST_STAT_POS => first_stat_pos(ctx, tree),
            PosKind::END_POS => end_pos(ctx, tree),
        }
    }
}

/// Return the statement referenced by a label.
pub fn referenced_statement(ctx: &CompilerContext, tree: TreeId) -> TreeId {
    let mut t = tree;
    while ctx.tree(t).has_tag(Tag::LABELLED) {
        let body = match &ctx.tree(t).kind {
            TreeKind::LabeledStatement { body, .. } => *body,
            _ => None,
        };
        match body {
            Some(body) => t = body,
            None => break,
        }
    }
    match ctx.tree(t).tag {
        Tag::DOLOOP | Tag::WHILELOOP | Tag::FORLOOP | Tag::FOREACHLOOP | Tag::SWITCH => t,
        _ => tree,
    }
}

/// Skip parens and return the enclosed expression.
pub fn skip_parens(ctx: &CompilerContext, tree: TreeId) -> TreeId {
    let mut tree = tree;
    while ctx.tree(tree).has_tag(Tag::PARENS) {
        match &ctx.tree(tree).kind {
            TreeKind::Parens { expr } => match expr {
                Some(expr) => tree = *expr,
                None => break,
            },
            _ => break,
        }
    }
    tree
}

/// Return the types of a list of trees.
pub fn types(ctx: &CompilerContext, trees: &List<TreeId>) -> List<Option<TypeId>> {
    let mut result = List::nil();
    for tree in trees.iter() {
        result = result.append(ctx.tree(tree).ty);
    }
    result
}

/// If this tree is an identifier or a field or a parameterized type, return its
/// name, otherwise return null.
pub fn name(ctx: &CompilerContext, tree: TreeId) -> Option<Name> {
    match &ctx.tree(tree).kind {
        TreeKind::Ident { name, .. } => Some(name.clone()),
        TreeKind::FieldAccess { name, .. } => Some(name.clone()),
        TreeKind::TypeApply { clazz, .. } => clazz.and_then(|clazz| name(ctx, clazz)),
        _ => None,
    }
}

/// If this tree is a qualified identifier, return its fully qualified name,
/// otherwise return null.
pub fn full_name(ctx: &CompilerContext, tree: TreeId) -> Option<Name> {
    let tree = skip_parens(ctx, tree);
    match &ctx.tree(tree).kind {
        TreeKind::Ident { name, .. } => Some(name.clone()),
        TreeKind::FieldAccess { selected, .. } => {
            let selected_name = full_name(ctx, (*selected)?)?;
            let name = name(ctx, tree)?;
            Some(selected_name.append(&name))
        }
        _ => None,
    }
}

/// Java: public static Symbol symbolFor(JCTree node)
pub fn symbol_for(ctx: &CompilerContext, node: TreeId) -> Option<SymbolId> {
    let sym = symbol_for_impl(ctx, node)?;
    // The upstream calls sym.baseSymbol(); baseSymbol returns the symbol itself
    // for every ported subclass except the not-yet-ported delegated symbols.
    Some(sym)
}

fn symbol_for_impl(ctx: &CompilerContext, node: TreeId) -> Option<SymbolId> {
    let node = skip_parens(ctx, node);
    match &ctx.tree(node).kind {
        TreeKind::CompilationUnit { packge, defs, .. } => {
            if is_module_info(ctx, node) {
                if let Some(module_decl) = get_module(ctx, node) {
                    return symbol_for(ctx, module_decl);
                }
            }
            let _ = defs;
            *packge
        }
        TreeKind::ModuleDecl { sym, .. } => *sym,
        TreeKind::PackageDecl { packge, .. } => *packge,
        TreeKind::ClassDecl { sym, .. } => *sym,
        TreeKind::MethodDecl { sym, .. } => *sym,
        TreeKind::VariableDecl { sym, .. } => *sym,
        TreeKind::Ident { sym, .. } => *sym,
        TreeKind::FieldAccess { sym, .. } => *sym,
        TreeKind::MemberReference { sym, .. } => *sym,
        TreeKind::NewClass { constructor, .. } => *constructor,
        TreeKind::MethodInvocation { meth, .. } => match meth {
            Some(meth) => symbol_for(ctx, *meth),
            None => None,
        },
        TreeKind::TypeApply { clazz, .. } => match clazz {
            Some(clazz) => symbol_for(ctx, *clazz),
            None => None,
        },
        TreeKind::Annotation { .. } | TreeKind::TypeParameter { .. } => {
            let node = ctx.tree(node);
            node.ty.map(|ty| match ctx.typ(ty).tsym {
                Some(tsym) => tsym,
                None => panic!("type has no symbol"),
            })
        }
        _ => None,
    }
}

/// Java: public static DiagnosticPosition diagEndPos(JCTree tree)
/// (TreeInfo.java:660-673): the ported diagnostic-position surface is the
/// preferred start position (i32); see the module note (Porting rules adapter).
pub fn diag_end_pos(ctx: &CompilerContext, tree: TreeId) -> i32 {
    end_pos(ctx, tree)
}

/// Java: public static long flags(JCTree tree) (TreeInfo.java:1028-1041)
pub fn flags(ctx: &CompilerContext, tree: TreeId) -> i64 {
    match &ctx.tree(tree).kind {
        TreeKind::VariableDecl { mods, .. } => mods
            .and_then(|mods| match &ctx.tree(mods).kind {
                TreeKind::Modifiers { flags, .. } => Some(*flags),
                _ => None,
            })
            .unwrap_or(0),
        TreeKind::MethodDecl { mods, .. } => mods
            .and_then(|mods| match &ctx.tree(mods).kind {
                TreeKind::Modifiers { flags, .. } => Some(*flags),
                _ => None,
            })
            .unwrap_or(0),
        TreeKind::ClassDecl { mods, .. } => mods
            .and_then(|mods| match &ctx.tree(mods).kind {
                TreeKind::Modifiers { flags, .. } => Some(*flags),
                _ => None,
            })
            .unwrap_or(0),
        TreeKind::Block { flags, .. } => *flags,
        _ => 0,
    }
}

/// Java: public static DiagnosticPosition diagnosticPositionFor(final Symbol
/// sym, final JCTree tree) (TreeInfo.java:711-733): the ported surface is the
/// tree start position (Porting rules adapter).
pub fn diagnostic_position_for(
    ctx: &CompilerContext,
    _sym: crate::ids::SymbolId,
    tree: TreeId,
) -> i32 {
    get_start_pos(ctx, tree)
}
/// Java: public static boolean isDeclaration(JCTree node)
pub fn is_declaration(ctx: &CompilerContext, node: TreeId) -> bool {
    let node = skip_parens(ctx, node);
    matches!(
        ctx.tree(node).tag,
        Tag::PACKAGEDEF | Tag::CLASSDEF | Tag::METHODDEF | Tag::VARDEF
    )
}

/// If this tree is an identifier or a field, return its symbol, otherwise
/// return null.
pub fn symbol(ctx: &CompilerContext, tree: TreeId) -> Option<SymbolId> {
    let tree = skip_parens(ctx, tree);
    match &ctx.tree(tree).kind {
        TreeKind::Ident { sym, .. } => *sym,
        TreeKind::FieldAccess { sym, .. } => *sym,
        TreeKind::TypeApply { clazz, .. } => match clazz {
            Some(clazz) => symbol(ctx, *clazz),
            None => None,
        },
        TreeKind::AnnotatedType {
            underlying_type, ..
        } => match underlying_type {
            Some(underlying_type) => symbol(ctx, *underlying_type),
            None => None,
        },
        TreeKind::MemberReference { sym, .. } => *sym,
        _ => None,
    }
}

/// If this tree has a modifiers field, return it, otherwise return null.
pub fn get_modifiers(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    let tree = skip_parens(ctx, tree);
    match &ctx.tree(tree).kind {
        TreeKind::VariableDecl { mods, .. } => *mods,
        TreeKind::MethodDecl { mods, .. } => *mods,
        TreeKind::ClassDecl { mods, .. } => *mods,
        TreeKind::ModuleDecl { mods, .. } => *mods,
        _ => None,
    }
}

/// Return true if this is a nonstatic selection.
pub fn nonstatic_select(ctx: &CompilerContext, tree: TreeId) -> bool {
    let tree = skip_parens(ctx, tree);
    if !ctx.tree(tree).has_tag(Tag::SELECT) {
        return false;
    }
    let TreeKind::FieldAccess { selected, .. } = &ctx.tree(tree).kind else {
        return false;
    };
    match selected.and_then(|selected| symbol(ctx, selected)) {
        None => true,
        Some(sym) => {
            let kind = ctx.sym(sym).kind;
            kind != KindsKind::PCK && kind != KindsKind::TYP
        }
    }
}

/// If this tree is an identifier or a field, set its symbol, otherwise skip.
pub fn set_symbol(ctx: &mut CompilerContext, tree: TreeId, sym: SymbolId) {
    let tree = skip_parens(ctx, tree);
    match &mut ctx.tree_mut(tree).kind {
        TreeKind::Ident { sym: slot, .. } => *slot = Some(sym),
        TreeKind::FieldAccess { sym: slot, .. } => *slot = Some(sym),
        _ => {}
    }
}

/// If this tree is a declaration or a block, return its flags field, otherwise
/// return 0.
pub fn flags_of(ctx: &CompilerContext, tree: TreeId) -> i64 {
    match &ctx.tree(tree).kind {
        TreeKind::VariableDecl { mods, .. } => mods
            .map(|mods| match &ctx.tree(mods).kind {
                TreeKind::Modifiers {
                    flags: mod_flags, ..
                } => *mod_flags,
                _ => 0,
            })
            .unwrap_or(0),
        TreeKind::MethodDecl { mods, .. } => mods
            .map(|mods| match &ctx.tree(mods).kind {
                TreeKind::Modifiers {
                    flags: mod_flags, ..
                } => *mod_flags,
                _ => 0,
            })
            .unwrap_or(0),
        TreeKind::ClassDecl { mods, .. } => mods
            .map(|mods| match &ctx.tree(mods).kind {
                TreeKind::Modifiers {
                    flags: mod_flags, ..
                } => *mod_flags,
                _ => 0,
            })
            .unwrap_or(0),
        TreeKind::Block {
            flags: block_flags, ..
        } => *block_flags,
        _ => 0,
    }
}

/// Return first (smallest) flag in flags (pre: flags != 0).
pub fn first_flag(flags: i64) -> i64 {
    let mut flag: i64 = 1;
    while (flag & flags) == 0 {
        flag <<= 1;
    }
    flag
}

// Operator precedences values.
pub const NOT_EXPRESSION: i32 = -1; // not an expression
pub const NO_PREC: i32 = 0; // no enclosing expression
pub const ASSIGN_PREC: i32 = 1;
pub const ASSIGNOP_PREC: i32 = 2;
pub const COND_PREC: i32 = 3;
pub const OR_PREC: i32 = 4;
pub const AND_PREC: i32 = 5;
pub const BITOR_PREC: i32 = 6;
pub const BITXOR_PREC: i32 = 7;
pub const BITAND_PREC: i32 = 8;
pub const EQ_PREC: i32 = 9;
pub const ORD_PREC: i32 = 10;
pub const SHIFT_PREC: i32 = 11;
pub const ADD_PREC: i32 = 12;
pub const MUL_PREC: i32 = 13;
pub const PREFIX_PREC: i32 = 14;
pub const POSTFIX_PREC: i32 = 15;
pub const PREC_COUNT: i32 = 16;

/// Map operators to their precedence levels.
pub fn op_prec(op: Tag) -> i32 {
    match op {
        Tag::POS | Tag::NEG | Tag::NOT | Tag::COMPL | Tag::PREINC | Tag::PREDEC => PREFIX_PREC,
        Tag::POSTINC | Tag::POSTDEC | Tag::NULLCHK => POSTFIX_PREC,
        Tag::ASSIGN => ASSIGN_PREC,
        Tag::BITOR_ASG
        | Tag::BITXOR_ASG
        | Tag::BITAND_ASG
        | Tag::SL_ASG
        | Tag::SR_ASG
        | Tag::USR_ASG
        | Tag::PLUS_ASG
        | Tag::MINUS_ASG
        | Tag::MUL_ASG
        | Tag::DIV_ASG
        | Tag::MOD_ASG => ASSIGNOP_PREC,
        Tag::OR => OR_PREC,
        Tag::AND => AND_PREC,
        Tag::EQ | Tag::NE => EQ_PREC,
        Tag::LT | Tag::GT | Tag::LE | Tag::GE => ORD_PREC,
        Tag::BITOR => BITOR_PREC,
        Tag::BITXOR => BITXOR_PREC,
        Tag::BITAND => BITAND_PREC,
        Tag::SL | Tag::SR | Tag::USR => SHIFT_PREC,
        Tag::PLUS | Tag::MINUS => ADD_PREC,
        Tag::MUL | Tag::DIV | Tag::MOD => MUL_PREC,
        Tag::TYPETEST => ORD_PREC,
        _ => panic!("Unexpected operator: {op:?}"),
    }
}

/// Java: static Tree.Kind tagToKind(JCTree.Tag tag)
pub fn tag_to_kind(tag: Tag) -> Option<Kind> {
    Some(match tag {
        Tag::POSTINC => Kind::POSTFIX_INCREMENT,
        Tag::POSTDEC => Kind::POSTFIX_DECREMENT,
        Tag::PREINC => Kind::PREFIX_INCREMENT,
        Tag::PREDEC => Kind::PREFIX_DECREMENT,
        Tag::POS => Kind::UNARY_PLUS,
        Tag::NEG => Kind::UNARY_MINUS,
        Tag::COMPL => Kind::BITWISE_COMPLEMENT,
        Tag::NOT => Kind::LOGICAL_COMPLEMENT,
        Tag::MUL => Kind::MULTIPLY,
        Tag::DIV => Kind::DIVIDE,
        Tag::MOD => Kind::REMAINDER,
        Tag::PLUS => Kind::PLUS,
        Tag::MINUS => Kind::MINUS,
        Tag::SL => Kind::LEFT_SHIFT,
        Tag::SR => Kind::RIGHT_SHIFT,
        Tag::USR => Kind::UNSIGNED_RIGHT_SHIFT,
        Tag::LT => Kind::LESS_THAN,
        Tag::GT => Kind::GREATER_THAN,
        Tag::LE => Kind::LESS_THAN_EQUAL,
        Tag::GE => Kind::GREATER_THAN_EQUAL,
        Tag::EQ => Kind::EQUAL_TO,
        Tag::NE => Kind::NOT_EQUAL_TO,
        Tag::BITAND => Kind::AND,
        Tag::BITXOR => Kind::XOR,
        Tag::BITOR => Kind::OR,
        Tag::AND => Kind::CONDITIONAL_AND,
        Tag::OR => Kind::CONDITIONAL_OR,
        Tag::MUL_ASG => Kind::MULTIPLY_ASSIGNMENT,
        Tag::DIV_ASG => Kind::DIVIDE_ASSIGNMENT,
        Tag::MOD_ASG => Kind::REMAINDER_ASSIGNMENT,
        Tag::PLUS_ASG => Kind::PLUS_ASSIGNMENT,
        Tag::MINUS_ASG => Kind::MINUS_ASSIGNMENT,
        Tag::SL_ASG => Kind::LEFT_SHIFT_ASSIGNMENT,
        Tag::SR_ASG => Kind::RIGHT_SHIFT_ASSIGNMENT,
        Tag::USR_ASG => Kind::UNSIGNED_RIGHT_SHIFT_ASSIGNMENT,
        Tag::BITAND_ASG => Kind::AND_ASSIGNMENT,
        Tag::BITXOR_ASG => Kind::XOR_ASSIGNMENT,
        Tag::BITOR_ASG => Kind::OR_ASSIGNMENT,
        Tag::NULLCHK => Kind::OTHER,
        Tag::ANNOTATION => Kind::ANNOTATION,
        Tag::TYPE_ANNOTATION => Kind::TYPE_ANNOTATION,
        Tag::EXPORTS => Kind::EXPORTS,
        Tag::OPENS => Kind::OPENS,
        _ => return None,
    })
}

/// Returns the underlying type of the tree if it is an annotated type, or the
/// tree itself otherwise.
pub fn type_in(ctx: &CompilerContext, tree: TreeId) -> TreeId {
    match &ctx.tree(tree).kind {
        TreeKind::AnnotatedType {
            underlying_type, ..
        } => underlying_type.expect("annotated type has no underlying type"),
        TreeKind::Ident { .. }
        | TreeKind::PrimitiveTypeTree { .. }
        | TreeKind::FieldAccess { .. }
        | TreeKind::ArrayTypeTree { .. }
        | TreeKind::Wildcard { .. }
        | TreeKind::TypeParameter { .. }
        | TreeKind::TypeApply { .. }
        | TreeKind::Erroneous { .. } => tree,
        _ => panic!("Unexpected type tree: {:?}", tree),
    }
}

/// Return the inner-most type of a type tree.
pub fn innermost_type(ctx: &CompilerContext, ty: TreeId, skip_annos: bool) -> Option<TreeId> {
    let mut last_annotated_type: Option<TreeId> = None;
    let mut cur = Some(ty);
    while let Some(current) = cur {
        match &ctx.tree(current).kind {
            TreeKind::ArrayTypeTree { elemtype } => {
                last_annotated_type = None;
                cur = *elemtype;
            }
            TreeKind::Wildcard { inner, .. } => {
                last_annotated_type = None;
                cur = *inner;
            }
            TreeKind::AnnotatedType {
                underlying_type, ..
            } => {
                last_annotated_type = Some(current);
                cur = *underlying_type;
            }
            _ => break,
        }
    }
    if !skip_annos && last_annotated_type.is_some() {
        last_annotated_type
    } else {
        cur
    }
}

/// Java: TreeInfo.flagNames(long flags) (Flags.toString; the upstream Pretty
/// printFlags helper uses it).
pub fn flag_names(flags: i64) -> crate::util::java_string::JavaString {
    // Java: return Flags.toString(flags & ExtendedStandardFlags).trim();
    let text = flags::to_java_string(flags & flags::ExtendedStandardFlags).to_string_lossy();
    crate::util::java_string::JavaString::from(text.trim())
}

/// Java: public static boolean isModuleInfo(JCCompilationUnit tree)
pub fn is_module_info(ctx: &CompilerContext, tree: TreeId) -> bool {
    let TreeKind::CompilationUnit { sourcefile, .. } = &ctx.tree(tree).kind else {
        return false;
    };
    let Some(sourcefile) = sourcefile else {
        return false;
    };
    sourcefile.is_name_compatible(
        &crate::util::java_string::JavaString::from("module-info"),
        JavaFileObjectKind::SOURCE,
    ) && get_module(ctx, tree).is_some()
}

/// Java: public static JCModuleDecl getModule(JCCompilationUnit t)
pub fn get_module(ctx: &CompilerContext, tree: TreeId) -> Option<TreeId> {
    let TreeKind::CompilationUnit { defs, .. } = &ctx.tree(tree).kind else {
        return None;
    };
    match defs.head_opt() {
        Some(head) => {
            let node = ctx.tree(head);
            if node.has_tag(Tag::MODULEDEF) {
                Some(head)
            } else {
                None
            }
        }
        None => None,
    }
}

/// Java: public static boolean isPackageInfo(JCCompilationUnit tree)
pub fn is_package_info(ctx: &CompilerContext, tree: TreeId) -> bool {
    let TreeKind::CompilationUnit { sourcefile, .. } = &ctx.tree(tree).kind else {
        return false;
    };
    let Some(sourcefile) = sourcefile else {
        return false;
    };
    sourcefile.is_name_compatible(
        &crate::util::java_string::JavaString::from("package-info"),
        JavaFileObjectKind::SOURCE,
    )
}

/// Java: public static boolean isErrorEnumSwitch(JCExpression selector, List<JCCase> cases)
pub fn is_error_enum_switch(ctx: &CompilerContext, selector: TreeId, cases: &List<TreeId>) -> bool {
    let selector_ty = ctx.tree(selector).ty;
    let is_err = match selector_ty {
        Some(ty) => match ctx.typ(ty).tsym {
            Some(tsym) => ctx.sym(tsym).kind == KindsKind::ERR,
            None => false,
        },
        None => false,
    };
    if !is_err {
        return false;
    }
    for case in cases.iter() {
        let TreeKind::Case { labels, .. } = &ctx.tree(case).kind else {
            return false;
        };
        for label in labels.iter() {
            if !ctx.tree(label).has_tag(Tag::IDENT) {
                return false;
            }
        }
    }
    true
}

/// Java: public record PatternPrimaryType(Type type, boolean unconditional)
pub struct PatternPrimaryType {
    pub ty: Option<TypeId>,
    pub unconditional: bool,
}

impl PatternPrimaryType {
    pub fn new(ty: Option<TypeId>, unconditional: bool) -> PatternPrimaryType {
        PatternPrimaryType { ty, unconditional }
    }

    /// Java: Type type()
    pub fn ty(&self) -> Option<TypeId> {
        self.ty
    }

    /// Java: boolean unconditional()
    pub fn unconditional(&self) -> bool {
        self.unconditional
    }
}

/// Java: public static PatternPrimaryType primaryPatternType(JCPattern pat)
pub fn primary_pattern_type(ctx: &CompilerContext, pat: TreeId) -> PatternPrimaryType {
    match &ctx.tree(pat).kind {
        TreeKind::BindingPattern { var } => {
            let ty = var.and_then(|var| ctx.tree(var).ty);
            PatternPrimaryType::new(ty, true)
        }
        TreeKind::GuardPattern { patt, expr } => {
            let nested = primary_pattern_type(ctx, (*patt).expect("guard pattern has no pattern"));
            let mut unconditional = nested.unconditional;
            if let Some(expr) = *expr {
                let expr_ty = ctx.tree(expr).ty;
                if let Some(expr_ty) = expr_ty {
                    let expr_ty_node = ctx.typ(expr_ty);
                    if expr_ty_node.has_tag(crate::code::type_tag::TypeTag::BOOLEAN)
                        && unconditional
                    {
                        unconditional = false;
                        // TODO: the constant value (Type.constValue) is not
                        // ported yet; see docs/STATUS.md.
                    }
                }
            }
            PatternPrimaryType::new(nested.ty, unconditional)
        }
        TreeKind::ParenthesizedPattern { pattern } => primary_pattern_type(
            ctx,
            (*pattern).expect("parenthesized pattern has no pattern"),
        ),
        _ => panic!("Unexpected pattern tree: {:?}", pat),
    }
}

/// Java: public static JCBindingPattern primaryPatternTree(JCPattern pat)
pub fn primary_pattern_tree(ctx: &CompilerContext, pat: TreeId) -> TreeId {
    match &ctx.tree(pat).kind {
        TreeKind::BindingPattern { .. } => pat,
        TreeKind::GuardPattern { patt, .. } => {
            primary_pattern_tree(ctx, (*patt).expect("guard pattern has no pattern"))
        }
        TreeKind::ParenthesizedPattern { pattern } => primary_pattern_tree(
            ctx,
            (*pattern).expect("parenthesized pattern has no pattern"),
        ),
        _ => panic!("Unexpected pattern tree: {:?}", pat),
    }
}


// ---------------------------------------------------------------------------
// TreeInfo.containsTypeAnnotation / TypeAnnotationFinder (TreeInfo.java:1304-1322)
// ---------------------------------------------------------------------------

/// Java: private static class TreeInfo.TypeAnnotationFinder extends TreeScanner
/// (TreeInfo.java:1304-1317).
pub struct TypeAnnotationFinder {
    pub found_type_anno: bool,
}

impl TypeAnnotationFinder {
    pub fn new() -> TypeAnnotationFinder {
        TypeAnnotationFinder {
            found_type_anno: false,
        }
    }
}

impl Default for TypeAnnotationFinder {
    fn default() -> Self {
        TypeAnnotationFinder::new()
    }
}

tree_scanner_visitor_delegate!(TypeAnnotationFinder);

impl crate::tree::tree_scanner::TreeScannerVisitor for TypeAnnotationFinder {
    fn scan(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) {
        if self.found_type_anno || tree.is_none() {
            return;
        }
        if let Some(tree) = tree {
            crate::tree::jc_tree::accept(ctx, tree, self);
        }
    }

    fn visit_annotation(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let _ = ctx;
        self.found_type_anno = self.found_type_anno || tree_has_tag(ctx, tree, Tag::TYPE_ANNOTATION);
    }
}

fn tree_has_tag(ctx: &CompilerContext, tree: TreeId, tag: Tag) -> bool {
    ctx.tree(tree).has_tag(tag)
}

/// Java: public static boolean containsTypeAnnotation(JCTree e)
/// (TreeInfo.java:1319-1323)
pub fn contains_type_annotation(ctx: &mut CompilerContext, e: TreeId) -> bool {
    let mut finder = TypeAnnotationFinder::new();
    use crate::tree::tree_scanner::TreeScannerVisitor;
    finder.scan(ctx, Some(e));
    finder.found_type_anno
}


/// helper (Porting rules registered): mirrors "tree instanceof JCExpression"
/// (JCTree.java class hierarchy) by the node tag. Used by Attr.PostAttrAnalyzer
/// (Attr.java:5824-5930) which only initializes types of expression nodes.
pub fn is_expression(ctx: &CompilerContext, tree: TreeId) -> bool {
    matches!(
        ctx.tree(tree).tag,
        Tag::IDENT
            | Tag::SELECT
            | Tag::LITERAL
            | Tag::INDEXED
            | Tag::TYPEAPPLY
            | Tag::TYPEARRAY
            | Tag::TYPEIDENT
            | Tag::TYPEPARAMETER
            | Tag::WILDCARD
            | Tag::TYPEUNION
            | Tag::TYPEINTERSECTION
            | Tag::TYPECAST
            | Tag::TYPETEST
            | Tag::PARENS
            | Tag::ASSIGN
            | Tag::PLUS_ASG
            | Tag::MINUS_ASG
            | Tag::MUL_ASG
            | Tag::DIV_ASG
            | Tag::MOD_ASG
            | Tag::BITAND_ASG
            | Tag::BITOR_ASG
            | Tag::BITXOR_ASG
            | Tag::SL_ASG
            | Tag::SR_ASG
            | Tag::USR_ASG
            | Tag::POS
            | Tag::NEG
            | Tag::NOT
            | Tag::COMPL
            | Tag::PREINC
            | Tag::PREDEC
            | Tag::POSTINC
            | Tag::POSTDEC
            | Tag::AND
            | Tag::OR
            | Tag::BITAND
            | Tag::BITOR
            | Tag::BITXOR
            | Tag::EQ
            | Tag::NE
            | Tag::LT
            | Tag::GT
            | Tag::LE
            | Tag::GE
            | Tag::SL
            | Tag::SR
            | Tag::USR
            | Tag::PLUS
            | Tag::MINUS
            | Tag::MUL
            | Tag::DIV
            | Tag::MOD
            | Tag::APPLY
            | Tag::NEWCLASS
            | Tag::NEWARRAY
            | Tag::LAMBDA
            | Tag::CONDEXPR
            | Tag::SWITCH_EXPRESSION
            | Tag::REFERENCE
            | Tag::ANNOTATION
            | Tag::TYPE_ANNOTATION
            | Tag::ANNOTATED_TYPE
            | Tag::BINDINGPATTERN
            | Tag::GUARDPATTERN
            | Tag::PARENTHESIZEDPATTERN
            | Tag::ERRONEOUS
            | Tag::LETEXPR
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::symbol::{Symbol, SymbolKind};
    use crate::tree::jc_tree::{JCTree, LiteralValue};
    use crate::util::name::Name;
    use crate::util::name::NameTableHandle;
    use std::rc::Rc;

    fn make_name(table: &Rc<NameTableHandle>, text: &str) -> Name {
        let units: Vec<u16> = text.encode_utf16().collect();
        table.from_chars(&units, 0, units.len() as i32)
    }

    fn tree(ctx: &mut CompilerContext, tag: Tag, kind: TreeKind, pos: i32) -> TreeId {
        ctx.alloc_tree(JCTree {
            pos,
            ty: None,
            tag,
            kind,
        })
    }

    fn empty() -> List<TreeId> {
        List::nil()
    }

    fn literal(ctx: &mut CompilerContext, tag: Tag, pos: i32) -> TreeId {
        tree(
            ctx,
            tag,
            TreeKind::Literal {
                typetag: crate::code::type_tag::TypeTag::INT,
                value: Some(LiteralValue::Integer(1)),
            },
            pos,
        )
    }

    #[test]
    fn parens_and_names() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let ident = tree(
            &mut ctx,
            Tag::IDENT,
            TreeKind::Ident {
                name: make_name(&table, "foo"),
                sym: None,
            },
            5,
        );
        let parens = tree(
            &mut ctx,
            Tag::PARENS,
            TreeKind::Parens { expr: Some(ident) },
            6,
        );
        assert_eq!(skip_parens(&ctx, parens), ident);
        assert_eq!(name(&ctx, ident), Some(make_name(&table, "foo")));
        assert_eq!(full_name(&ctx, parens), Some(make_name(&table, "foo")));
        // The upstream getStartPos has no PARENS case: it returns tree.pos.
        assert_eq!(get_start_pos(&ctx, parens), 6);
    }

    #[test]
    fn expression_statements_and_statements() {
        let mut ctx = CompilerContext::new();
        let assign = tree(
            &mut ctx,
            Tag::ASSIGN,
            TreeKind::Assign {
                lhs: None,
                rhs: None,
            },
            1,
        );
        assert!(is_expression_statement(&ctx, assign));
        assert!(!is_statement(&ctx, assign));
        let block = tree(
            &mut ctx,
            Tag::BLOCK,
            TreeKind::Block {
                flags: 0,
                stats: empty(),
                endpos: NOPOS,
            },
            2,
        );
        assert!(!is_statement(&ctx, block));
        assert_eq!(first_stat_pos(&ctx, block), 2);
        assert_eq!(end_pos(&ctx, block), 2);
    }

    #[test]
    fn operator_precedence_and_tag_kind() {
        assert_eq!(op_prec(Tag::PLUS), ADD_PREC);
        assert_eq!(op_prec(Tag::MUL), MUL_PREC);
        assert_eq!(op_prec(Tag::ASSIGN), ASSIGN_PREC);
        assert_eq!(op_prec(Tag::PREINC), PREFIX_PREC);
        assert_eq!(or_prec(), OR_PREC);
        assert_eq!(tag_to_kind(Tag::PLUS), Some(Kind::PLUS));
        assert_eq!(tag_to_kind(Tag::BITAND_ASG), Some(Kind::AND_ASSIGNMENT));
        assert_eq!(tag_to_kind(Tag::CLASSDEF), None);
    }

    fn or_prec() -> i32 {
        OR_PREC
    }

    #[test]
    fn first_flag_picks_lowest_bit() {
        assert_eq!(first_flag(0b1000), 0b1000);
        assert_eq!(first_flag(0b1010), 0b0010);
        assert_eq!(first_flag(1), 1);
    }

    #[test]
    fn is_null_literal() {
        let mut ctx = CompilerContext::new();
        let null_literal = tree(
            &mut ctx,
            Tag::LITERAL,
            TreeKind::Literal {
                typetag: crate::code::type_tag::TypeTag::BOT,
                value: None,
            },
            0,
        );
        assert!(is_null(&ctx, null_literal));
        let int_literal = literal(&mut ctx, Tag::LITERAL, 0);
        assert!(!is_null(&ctx, int_literal));
    }

    #[test]
    fn constructor_helpers() {
        let mut ctx = CompilerContext::new();
        let names = crate::util::names::Names::new(false);
        let init_name = names.init.clone();
        let other_name = names.table.from_chars(&[0x6D], 0, 1);
        let method = tree(
            &mut ctx,
            Tag::METHODDEF,
            TreeKind::MethodDecl {
                mods: None,
                name: init_name,
                restype: None,
                typarams: empty(),
                recvparam: None,
                params: empty(),
                thrown: empty(),
                body: None,
                default_value: None,
                sym: None,
                completes_normally: true,
            },
            3,
        );
        assert!(is_constructor(&ctx, method));
        let other = tree(
            &mut ctx,
            Tag::METHODDEF,
            TreeKind::MethodDecl {
                mods: None,
                name: other_name,
                restype: None,
                typarams: empty(),
                recvparam: None,
                params: empty(),
                thrown: empty(),
                body: None,
                default_value: None,
                sym: None,
                completes_normally: true,
            },
            4,
        );
        assert!(!is_constructor(&ctx, other));
        assert!(has_constructors(&ctx, &List::of2(method, other)));
    }

    #[test]
    fn static_select_and_symbols() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let names = crate::util::names::Names::new(false);
        let class_sym = ctx.alloc_symbol(Symbol::new(
            KindsKind::TYP,
            0,
            make_name(&table, "C"),
            None,
            None,
            SymbolKind::ClassSymbol {
                members_field: None,
                fullname: make_name(&table, "C"),
                flatname: make_name(&table, "C"),
                sourcefile: None,
                classfile: None,
                trans_local: List::nil(),
                record_components: List::nil(),
                permitted: List::nil(),
                is_permitted_explicit: false,
                annotation_type_metadata: Default::default(),
            },
        ));
        let ident = tree(
            &mut ctx,
            Tag::IDENT,
            TreeKind::Ident {
                name: make_name(&table, "C"),
                sym: Some(class_sym),
            },
            0,
        );
        assert!(is_static_selector(&ctx, Some(ident), &names));
        assert_eq!(symbol(&ctx, ident), Some(class_sym));
        assert!(!is_declaration(&ctx, ident));
        let mut ctx2 = CompilerContext::new();
        let var = tree(
            &mut ctx2,
            Tag::VARDEF,
            TreeKind::VariableDecl {
                mods: None,
                name: make_name(&table, "x"),
                nameexpr: None,
                vartype: None,
                init: None,
                sym: None,
                start_pos: NOPOS,
            },
            1,
        );
        assert!(is_declaration(&ctx2, var));
        assert_eq!(flags_of(&ctx2, var), 0);
        assert!(!is_receiver_param(&ctx2, var));
    }

    #[test]
    fn record_fields_and_diamond() {
        let mut ctx = CompilerContext::new();
        let table = NameTableHandle::unshared();
        let modifiers = tree(
            &mut ctx,
            Tag::MODIFIERS,
            TreeKind::Modifiers {
                flags: flags::RECORD,
                annotations: empty(),
            },
            0,
        );
        let component = tree(
            &mut ctx,
            Tag::VARDEF,
            TreeKind::VariableDecl {
                mods: Some(modifiers),
                name: make_name(&table, "x"),
                nameexpr: None,
                vartype: None,
                init: None,
                sym: None,
                start_pos: NOPOS,
            },
            1,
        );
        let class = tree(
            &mut ctx,
            Tag::CLASSDEF,
            TreeKind::ClassDecl {
                mods: None,
                name: make_name(&table, "R"),
                typarams: empty(),
                extending: None,
                implementing: empty(),
                permitting: empty(),
                defs: List::of(component),
                sym: None,
            },
            0,
        );
        assert_eq!(record_fields(&ctx, class).length(), 1);
        assert_eq!(record_field_types(&ctx, class).length(), 1);
        let applied = tree(
            &mut ctx,
            Tag::TYPEAPPLY,
            TreeKind::TypeApply {
                clazz: None,
                arguments: empty(),
            },
            0,
        );
        assert!(is_diamond(&ctx, applied));
    }

    #[test]
    fn innermost_type_skips_annotated_arrays() {
        let mut ctx = CompilerContext::new();
        let ident = tree(
            &mut ctx,
            Tag::IDENT,
            TreeKind::Ident {
                name: NameTableHandle::unshared().from_chars(&[0x41], 0, 1),
                sym: None,
            },
            0,
        );
        let anno = tree(
            &mut ctx,
            Tag::ANNOTATED_TYPE,
            TreeKind::AnnotatedType {
                annotations: empty(),
                underlying_type: Some(ident),
            },
            0,
        );
        let array = tree(
            &mut ctx,
            Tag::TYPEARRAY,
            TreeKind::ArrayTypeTree {
                elemtype: Some(anno),
            },
            0,
        );
        assert_eq!(innermost_type(&ctx, array, false), Some(anno));
        assert_eq!(innermost_type(&ctx, array, true), Some(ident));
    }
}

// ---------------------------------------------------------------------------
// Java: TreeInfo.DeclScanner + declarationFor (TreeInfo.java:735-830)
// ---------------------------------------------------------------------------

/// Java: static class DeclScanner extends TreeScanner (TreeInfo.java:735-783)
struct DeclScanner {
    sym: crate::ids::SymbolId,
    result: Option<TreeId>,
}

impl DeclScanner {
    fn new(sym: crate::ids::SymbolId) -> DeclScanner {
        DeclScanner { sym, result: None }
    }

    /// Java: public void scan(JCTree tree) (only until the result is found)
    fn decl_scan(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) {
        if let Some(tree) = tree {
            if self.result.is_none() {
                crate::tree::jc_tree::accept(ctx, tree, self);
            }
        }
    }
}

use crate::tree::tree_scanner::{tree_scanner_visitor_delegate, TreeScannerVisitor};

tree_scanner_visitor_delegate!(DeclScanner);

impl TreeScannerVisitor for DeclScanner {
    fn visit_top_level(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let packge = match &ctx.tree(tree).kind {
            TreeKind::CompilationUnit { packge, .. } => *packge,
            _ => panic!("expected a compilation unit"),
        };
        if packge == Some(self.sym) {
            self.result = Some(tree);
        } else {
            <Self as TreeScannerVisitor>::visit_top_level(self, ctx, tree);
        }
    }

    fn visit_module_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sym = match &ctx.tree(tree).kind {
            TreeKind::ModuleDecl { sym, .. } => *sym,
            _ => panic!("expected a module declaration"),
        };
        if sym == Some(self.sym) {
            self.result = Some(tree);
        }
        // no need to scan within module declaration
    }

    fn visit_package_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let packge = match &ctx.tree(tree).kind {
            TreeKind::PackageDecl { packge, .. } => *packge,
            _ => panic!("expected a package declaration"),
        };
        if packge == Some(self.sym) {
            self.result = Some(tree);
        } else {
            <Self as TreeScannerVisitor>::visit_package_def(self, ctx, tree);
        }
    }

    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sym = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { sym, .. } => *sym,
            _ => panic!("expected a class declaration"),
        };
        if sym == Some(self.sym) {
            self.result = Some(tree);
        } else {
            <Self as TreeScannerVisitor>::visit_class_def(self, ctx, tree);
        }
    }

    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sym = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl { sym, .. } => *sym,
            _ => panic!("expected a method declaration"),
        };
        if sym == Some(self.sym) {
            self.result = Some(tree);
        } else {
            <Self as TreeScannerVisitor>::visit_method_def(self, ctx, tree);
        }
    }

    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sym = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl { sym, .. } => *sym,
            _ => panic!("expected a variable declaration"),
        };
        if sym == Some(self.sym) {
            self.result = Some(tree);
        } else {
            <Self as TreeScannerVisitor>::visit_var_def(self, ctx, tree);
        }
    }

    fn visit_type_parameter(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tsym = ctx.tree(tree).ty.and_then(|ty| ctx.typ(ty).tsym);
        if tsym == Some(self.sym) {
            self.result = Some(tree);
        } else {
            <Self as TreeScannerVisitor>::visit_type_parameter(self, ctx, tree);
        }
    }
}

/// Java: public static JCTree declarationFor(final Symbol sym, final JCTree
/// tree) (TreeInfo.java:785-789)
pub fn declaration_for(
    ctx: &mut CompilerContext,
    sym: crate::ids::SymbolId,
    tree: TreeId,
) -> Option<TreeId> {
    let mut scanner = DeclScanner::new(sym);
    scanner.decl_scan(ctx, Some(tree));
    scanner.result
}

#[cfg(test)]
mod type_annotation_finder_tests {
    use super::*;
    use crate::tree::jc_tree::{JCTree, Tag, TreeKind};

    #[test]
    fn contains_type_annotation_finds_type_use_annotations() {
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let annotation = ctx.alloc_tree(JCTree {
            pos: 0,
            ty: None,
            tag: Tag::ANNOTATION,
            kind: TreeKind::Annotation {
                annotation_type: None,
                args: List::nil(),
                attribute: None,
            },
        });
        assert!(!contains_type_annotation(&mut ctx, annotation));
        let type_annotation = ctx.alloc_tree(JCTree {
            pos: 0,
            ty: None,
            tag: Tag::TYPE_ANNOTATION,
            kind: TreeKind::Annotation {
                annotation_type: None,
                args: List::nil(),
                attribute: None,
            },
        });
        assert!(contains_type_annotation(&mut ctx, type_annotation));
        let annotated_type = ctx.alloc_tree(JCTree {
            pos: 0,
            ty: None,
            tag: Tag::ANNOTATED_TYPE,
            kind: TreeKind::AnnotatedType {
                annotations: List::of(type_annotation),
                underlying_type: Some(annotation),
            },
        });
        assert!(contains_type_annotation(&mut ctx, annotated_type));
    }
}
