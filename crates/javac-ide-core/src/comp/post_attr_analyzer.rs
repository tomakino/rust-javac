//! Post-attribution analyzer: fills in the types and symbols that a class or
//! method declaration can lack after attribution.

use crate::code::symtab::Symtab;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::tree::jc_tree::{Tag, TreeKind};
use crate::util::list::List;
use crate::util::names::Names;

/// /// Java: class Attr.PostAttrAnalyzer extends TreeScanner (Attr.java:5824-5930).
///
/// # Panics
///   - none.
///
/// Upstream: Attr.java:5824-5930.
///
pub struct PostAttrAnalyzer<'a> {
    pub syms: &'a Symtab,
    pub names: &'a Names,
    /// Java: preFlow's anonymous PostAttrAnalyzer override: None for the plain
    /// postAttr analyzer; Some(stuckType) skips stuck subtrees, class decls and
    /// lambdas (adapter: the override is this field).
    pub pre_flow_stuck_type: Option<TypeId>,
}

impl PostAttrAnalyzer<'_> {
    /// Java: private void initTypeIfNeeded(JCTree that)
    fn init_type_if_needed(&self, ctx: &mut CompilerContext, tree: TreeId) {
        if ctx.tree(tree).ty.is_none() {
            let ty = if ctx.tree(tree).tag == Tag::METHODDEF {
                self.dummy_method_type_for(ctx, Some(tree))
            } else {
                self.syms.unknown_type
            };
            ctx.tree_mut(tree).ty = Some(ty);
        }
    }

    /// Java: private Type dummyMethodType(JCMethodDecl md)
    fn dummy_method_type_for(&self, ctx: &mut CompilerContext, md: Option<TreeId>) -> TypeId {
        let mut restype = self.syms.unknown_type;
        if let Some(md) = md {
            let restype_tree = match &ctx.tree(md).kind {
                TreeKind::MethodDecl { restype, .. } => *restype,
                _ => None,
            };
            if let Some(restype_tree) = restype_tree
                && ctx.tree(restype_tree).tag == Tag::TYPEIDENT
                && let TreeKind::PrimitiveTypeTree { typetag } = ctx.tree(restype_tree).kind
                && typetag == crate::code::type_tag::TypeTag::VOID
            {
                restype = self.syms.void_type;
            }
        }
        let ty = crate::code::type_::Type::new_method_type(
            List::nil(),
            restype,
            List::nil(),
            self.syms.method_class,
        );
        ctx.alloc_type(ty)
    }

    fn dummy_method_type(&self, ctx: &mut CompilerContext) -> TypeId {
        self.dummy_method_type_for(ctx, None)
    }

    fn new_symbol(
        &self,
        ctx: &mut CompilerContext,
        kind: crate::code::kinds::Kind,
        flags: i64,
        name: crate::util::name::Name,
        ty: Option<TypeId>,
        kind_data: crate::code::symbol::SymbolKind,
    ) -> SymbolId {
        ctx.alloc_symbol(crate::code::symbol::Symbol::new(
            kind,
            flags,
            name,
            ty,
            Some(self.syms.no_symbol),
            kind_data,
        ))
    }
    fn operation_symbol(&self, ctx: &mut CompilerContext) -> SymbolId {
        let ty = self.dummy_method_type(ctx);
        self.new_symbol(
            ctx,
            crate::code::kinds::Kind::MTH,
            0,
            self.names.empty.clone(),
            Some(ty),
            crate::code::symbol::SymbolKind::OperatorSymbol {
                opcode: -1,
                access_code: 0,
            },
        )
    }
}

impl crate::tree::tree_scanner::TreeScannerVisitor for PostAttrAnalyzer<'_> {
    /// Java: public void scan(JCTree tree)
    fn scan(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) {
        if let Some(tree) = tree {
            if let Some(stuck_type) = self.pre_flow_stuck_type {
                // Java: preFlow's scanner: skip stuck expressions, class decls
                // and lambdas.
                if ctx.tree(tree).ty == Some(stuck_type)
                    || ctx.tree(tree).tag == Tag::CLASSDEF
                    || ctx.tree(tree).tag == Tag::LAMBDA
                {
                    return;
                }
            }
            if crate::tree::tree_info::is_expression(ctx, tree) {
                self.init_type_if_needed(ctx, tree);
            }
            crate::tree::jc_tree::accept(ctx, tree, self);
        }
    }

    /// Java: public void visitClassDef(JCClassDecl that)
    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.init_type_if_needed(ctx, tree);
        let needs_sym = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { sym, .. } => sym.is_none(),
            _ => panic!("expected a class declaration"),
        };
        if needs_sym {
            let (name, ty) = match &ctx.tree(tree).kind {
                TreeKind::ClassDecl { name, .. } => (name.clone(), ctx.tree(tree).ty),
                _ => crate::util::assert::error(),
            };
            let fullname =
                crate::code::symtab::form_full_name(ctx, &name, Some(self.syms.no_symbol));
            let flatname =
                crate::code::symtab::form_flat_name(ctx, &name, Some(self.syms.no_symbol));
            let sym = self.new_symbol(
                ctx,
                crate::code::kinds::Kind::TYP,
                0,
                name,
                ty,
                crate::code::symbol::SymbolKind::ClassSymbol {
                    members_field: None,
                    fullname,
                    flatname,
                    sourcefile: None,
                    classfile: None,
                    trans_local: List::nil(),
                    record_components: List::nil(),
                    permitted: List::nil(),
                    is_permitted_explicit: false,
                    annotation_type_metadata: Default::default(),
                },
            );
            if let TreeKind::ClassDecl { sym: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(sym);
            }
        }
        let (mods, typarams, extending, implementing, permitting, defs) = match &ctx.tree(tree).kind
        {
            TreeKind::ClassDecl {
                mods,
                typarams,
                extending,
                implementing,
                permitting,
                defs,
                ..
            } => (
                *mods,
                typarams.clone(),
                *extending,
                implementing.clone(),
                permitting.clone(),
                defs.clone(),
            ),
            _ => panic!("expected a class declaration"),
        };
        self.scan(ctx, mods);
        self.scan_list(ctx, &typarams);
        self.scan(ctx, extending);
        self.scan_list(ctx, &implementing);
        self.scan_list(ctx, &permitting);
        self.scan_list(ctx, &defs);
    }

    /// Java: public void visitMethodDef(JCMethodDecl that)
    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.init_type_if_needed(ctx, tree);
        let needs_sym = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl { sym, .. } => sym.is_none(),
            _ => panic!("expected a method declaration"),
        };
        if needs_sym {
            let (name, ty) = match &ctx.tree(tree).kind {
                TreeKind::MethodDecl { name, .. } => (name.clone(), ctx.tree(tree).ty),
                _ => crate::util::assert::error(),
            };
            let sym = self.new_symbol(
                ctx,
                crate::code::kinds::Kind::MTH,
                0,
                name,
                ty,
                crate::code::symbol::SymbolKind::MethodSymbol {
                    extra_params: List::nil(),
                    captured_locals: List::nil(),
                    params: List::nil(),
                    default_value: None,
                    original_enclosing_method: None,
                },
            );
            if let TreeKind::MethodDecl { sym: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(sym);
            }
        }
        let (mods, restype, typarams, recvparam, params, thrown, default_value, body) =
            match &ctx.tree(tree).kind {
                TreeKind::MethodDecl {
                    mods,
                    restype,
                    typarams,
                    recvparam,
                    params,
                    thrown,
                    default_value,
                    body,
                    ..
                } => (
                    *mods,
                    *restype,
                    typarams.clone(),
                    *recvparam,
                    params.clone(),
                    thrown.clone(),
                    *default_value,
                    *body,
                ),
                _ => panic!("expected a method declaration"),
            };
        self.scan(ctx, mods);
        self.scan(ctx, restype);
        self.scan_list(ctx, &typarams);
        self.scan(ctx, recvparam);
        self.scan_list(ctx, &params);
        self.scan_list(ctx, &thrown);
        self.scan(ctx, default_value);
        self.scan(ctx, body);
    }

    /// Java: public void visitVarDef(JCVariableDecl that)
    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.init_type_if_needed(ctx, tree);
        let (name, ty, needs_sym, vartype) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl {
                name, vartype, sym, ..
            } => (name.clone(), ctx.tree(tree).ty, sym.is_none(), *vartype),
            _ => panic!("expected a variable declaration"),
        };
        if needs_sym {
            let sym = self.new_symbol(
                ctx,
                crate::code::kinds::Kind::VAR,
                0,
                name,
                ty,
                crate::code::symbol::SymbolKind::VarSymbol {
                    lazy_const_value: None,
                    pos: 0,
                    adr: 0,
                    data_kind: None,
                    data: None,
                },
            );
            if let TreeKind::VariableDecl { sym: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(sym);
            }
        }
        if vartype.is_none() {
            // Java: that.vartype = make.at(Position.NOPOS).Erroneous();
            let erroneous = ctx.alloc_tree(crate::tree::jc_tree::JCTree {
                pos: crate::util::position::NOPOS,
                ty: None,
                tag: Tag::ERRONEOUS,
                kind: TreeKind::Erroneous { errs: List::nil() },
            });
            if let TreeKind::VariableDecl { vartype: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(erroneous);
            }
        }
        let (mods, vartype, nameexpr, init) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl {
                mods,
                vartype,
                nameexpr,
                init,
                ..
            } => (*mods, *vartype, *nameexpr, *init),
            _ => panic!("expected a variable declaration"),
        };
        self.scan(ctx, mods);
        self.scan(ctx, vartype);
        self.scan(ctx, nameexpr);
        self.scan(ctx, init);
    }

    /// Java: public void visitNewClass(JCNewClass that)
    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (constructor, constructor_type) = match &ctx.tree(tree).kind {
            TreeKind::NewClass {
                constructor,
                constructor_type,
                ..
            } => (*constructor, *constructor_type),
            _ => panic!("expected a new class"),
        };
        if constructor.is_none() {
            let ctor_type = self.dummy_method_type(ctx);
            let sym = self.new_symbol(
                ctx,
                crate::code::kinds::Kind::MTH,
                0,
                self.names.init.clone(),
                Some(ctor_type),
                crate::code::symbol::SymbolKind::MethodSymbol {
                    extra_params: List::nil(),
                    captured_locals: List::nil(),
                    params: List::nil(),
                    default_value: None,
                    original_enclosing_method: None,
                },
            );
            if let TreeKind::NewClass {
                constructor: slot, ..
            } = &mut ctx.tree_mut(tree).kind
            {
                *slot = Some(sym);
            }
        }
        if constructor_type.is_none()
            && let TreeKind::NewClass {
                constructor_type: slot,
                ..
            } = &mut ctx.tree_mut(tree).kind
        {
            *slot = Some(self.syms.unknown_type);
        }
        let (encl, typeargs, clazz, args, def) = match &ctx.tree(tree).kind {
            TreeKind::NewClass {
                encl,
                typeargs,
                clazz,
                args,
                def,
                ..
            } => (*encl, typeargs.clone(), *clazz, args.clone(), *def),
            _ => panic!("expected a new class"),
        };
        self.scan(ctx, encl);
        self.scan_list(ctx, &typeargs);
        self.scan(ctx, clazz);
        self.scan_list(ctx, &args);
        self.scan(ctx, def);
    }

    /// Java: public void visitAssignop(JCAssignOp that)
    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let needs_operator = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { operator, .. } => operator.is_none(),
            _ => panic!("expected a compound assignment"),
        };
        if needs_operator {
            let sym = self.operation_symbol(ctx);
            if let TreeKind::AssignOp { operator: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(sym);
            }
        }
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a compound assignment"),
        };
        self.scan(ctx, lhs);
        self.scan(ctx, rhs);
    }

    /// Java: public void visitUnary(JCUnary that)
    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let needs_operator = match &ctx.tree(tree).kind {
            TreeKind::Unary { operator, .. } => operator.is_none(),
            _ => panic!("expected a unary expression"),
        };
        if needs_operator {
            let sym = self.operation_symbol(ctx);
            if let TreeKind::Unary { operator: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(sym);
            }
        }
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, .. } => *arg,
            _ => panic!("expected a unary expression"),
        };
        self.scan(ctx, arg);
    }

    /// Java: public void visitBinary(JCBinary that)
    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let needs_operator = match &ctx.tree(tree).kind {
            TreeKind::Binary { operator, .. } => operator.is_none(),
            _ => panic!("expected a binary expression"),
        };
        if needs_operator {
            let sym = self.operation_symbol(ctx);
            if let TreeKind::Binary { operator: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(sym);
            }
        }
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Binary { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a binary expression"),
        };
        self.scan(ctx, lhs);
        self.scan(ctx, rhs);
    }

    /// Java: public void visitBindingPattern(JCBindingPattern that)
    fn visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.init_type_if_needed(ctx, tree);
        let var = match &ctx.tree(tree).kind {
            TreeKind::BindingPattern { var } => *var,
            _ => panic!("expected a binding pattern"),
        };
        if let Some(var) = var {
            self.init_type_if_needed(ctx, var);
            let (name, ty, needs_sym) = match &ctx.tree(var).kind {
                TreeKind::VariableDecl { name, sym, .. } => {
                    (name.clone(), ctx.tree(var).ty, sym.is_none())
                }
                _ => panic!("expected a variable declaration"),
            };
            if needs_sym {
                // Java: new BindingSymbol(0, name, type, syms.noSymbol)
                // (Attr.java:5915; the constructor adds HASINIT | MATCH_BINDING,
                // Symbol.java:1874-1876).
                let sym = self.new_symbol(
                    ctx,
                    crate::code::kinds::Kind::VAR,
                    crate::code::flags::HASINIT as i64 | crate::code::flags::MATCH_BINDING,
                    name,
                    ty,
                    crate::code::symbol::SymbolKind::BindingSymbol {
                        aliases: List::nil(),
                        lazy_const_value: None,
                        pos: 0,
                        adr: 0,
                        data_kind: None,
                        data: None,
                    },
                );
                if let TreeKind::VariableDecl { sym: slot, .. } = &mut ctx.tree_mut(var).kind {
                    *slot = Some(sym);
                }
            }
        }
        self.scan(ctx, var);
    }

    /// Java: public void visitSelect(JCFieldAccess that)
    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let TreeKind::FieldAccess { sym, .. } = &mut ctx.tree_mut(tree).kind
            && sym.is_none()
        {
            *sym = Some(self.syms.unknown_symbol);
        }
        let selected = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { selected, .. } => *selected,
            _ => panic!("expected a field access"),
        };
        self.scan(ctx, selected);
    }

    /// Java: public void visitReference(JCMemberReference that)
    fn visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        // Java: super.visitReference(that);
        let (expr, typeargs) = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { expr, typeargs, .. } => (*expr, typeargs.clone()),
            _ => panic!("expected a member reference"),
        };
        self.scan(ctx, expr);
        self.scan_list_opt(ctx, typeargs);
        let needs_sym = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { sym, .. } => sym.is_none(),
            _ => panic!("expected a member reference"),
        };
        if needs_sym {
            let ty = self.dummy_method_type(ctx);
            let sym = self.new_symbol(
                ctx,
                crate::code::kinds::Kind::MTH,
                0,
                self.names.empty.clone(),
                Some(ty),
                crate::code::symbol::SymbolKind::MethodSymbol {
                    extra_params: List::nil(),
                    captured_locals: List::nil(),
                    params: List::nil(),
                    default_value: None,
                    original_enclosing_method: None,
                },
            );
            if let TreeKind::MemberReference { sym: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(sym);
            }
        }
    }

    /// Java: public void visitIdent(JCIdent that)
    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if let TreeKind::Ident { sym, .. } = &mut ctx.tree_mut(tree).kind
            && sym.is_none()
        {
            *sym = Some(self.syms.unknown_symbol);
        }
    }
}
